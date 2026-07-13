//! Application state and main egui rendering logic.
//!
//! The `RmuxApp` struct owns the top-level application state including the
//! workspace manager, sidebar view, notification manager, and the socket
//! API channel endpoints. It implements `eframe::App` to drive the UI.
//! Keyboard shortcut handling lives in [`crate::shortcuts`]; socket API
//! request handling lives in [`crate::api_dispatch`].

use rmux_api::ApiEvent;
use serde_json::json;

use crate::api;
use crate::browser::BrowserPane;
use crate::notifications::NotificationManager;
use crate::ui::DEFAULT_FONT_SIZE;
use crate::ui::sidebar::SidebarView;
use crate::ui::{NotificationPanel, TerminalPane, workspace_view};
use crate::workspace::WorkspaceManager;
use crate::workspace::splits::{PaneId, PaneTreeError, SplitDirection};

/// Terminal dimensions used when spawning a pane; the pane resizes to
/// its real cell grid on the first rendered frame.
const INITIAL_COLS: u16 = 80;
/// See [`INITIAL_COLS`].
const INITIAL_ROWS: u16 = 24;

/// The root application state.
///
/// Holds the workspace manager, sidebar view, and orchestrates all subsystems.
/// Implements `eframe::App` to render the UI each frame.
pub struct RmuxApp {
    /// Manages all workspaces, panes, and splits.
    pub(crate) workspace_manager: WorkspaceManager,
    /// The sidebar view for workspace tab navigation.
    pub(crate) sidebar: SidebarView,
    /// Stores notifications and emits desktop notifications.
    pub(crate) notifications: NotificationManager,
    /// The right-side notification list panel.
    pub(crate) notification_panel: NotificationPanel,
    /// The current terminal font size (shared by all panes).
    pub(crate) font_size: f32,
    /// Most recent text copied from a terminal selection.
    pub(crate) last_copied_text: Option<String>,
    /// Receives socket API requests, drained each frame.
    api_request_rx: tokio::sync::mpsc::Receiver<rmux_api::ApiRequestEnvelope>,
    /// Publishes application events to `events.stream` subscribers.
    api_event_tx: tokio::sync::broadcast::Sender<ApiEvent>,
    /// Active workspace id at the end of the previous frame, used to
    /// detect switches (keyboard, sidebar, or API) and publish
    /// `workspace.changed` exactly once per switch.
    last_active_workspace: u64,
    /// Global shortcut registry built once at startup.
    pub(crate) shortcut_registry: crate::shortcuts::ShortcutRegistry,
}

impl RmuxApp {
    /// Create a new application state with a default workspace and terminal pane.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let channels = api::start_server();
        let font_size = DEFAULT_FONT_SIZE;
        let mut app = Self {
            workspace_manager: WorkspaceManager::new(),
            sidebar: SidebarView::new(),
            notifications: NotificationManager::with_system_notifier(),
            notification_panel: NotificationPanel::new(),
            font_size,
            last_copied_text: None,
            api_request_rx: channels.request_rx,
            api_event_tx: channels.event_tx,
            last_active_workspace: 0,
            shortcut_registry: crate::shortcuts::ShortcutRegistry::default(),
        };

        let pane_id = app.workspace_manager.active().active_pane;
        attach_terminal(&mut app.workspace_manager, pane_id, font_size);
        app.last_active_workspace = app.workspace_manager.active().id.0;

        tracing::info!(
            workspaces = app.workspace_manager.workspace_count(),
            panes = app.workspace_manager.total_pane_count(),
            "Application initialized"
        );
        app
    }
}

impl eframe::App for RmuxApp {
    /// Called each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply shadcn-inspired theme every frame
        crate::ui::theme::Theme::dark().apply(ctx);
        // Process PTY output for all terminal panes; collect any OSC
        // notifications raised by pane output.
        let osc_notifications = self.workspace_manager.process_all_panes();
        for (workspace_id, pane_id, notification) in osc_notifications {
            self.add_pane_notification(workspace_id, pane_id, notification);
        }

        // Auto-close panes whose process has exited
        let cleanup = self.workspace_manager.close_exited_panes();
        for (workspace_id, pane_id) in cleanup.panes {
            self.publish_event(
                "pane.closed",
                json!({ "pane_id": pane_id, "workspace_id": workspace_id }),
            );
        }
        for workspace_id in cleanup.workspaces {
            self.publish_event("workspace.closed", json!({ "id": workspace_id }));
        }

        // Handle any pending socket API requests on the main thread
        self.process_api_requests();

        // Request continuous repaints for terminal updates (PTY output, cursor blink)
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        // Render the top bar and status bar first so they span the full
        // window width (egui panel order: top/bottom before side panels).
        crate::ui::top_bar::show(
            ctx,
            &self.workspace_manager,
            &self.notifications,
            &mut self.sidebar.visible,
            &mut self.notification_panel.visible,
            &mut self.sidebar.right_sidebar_visible,
        );
        crate::ui::status_bar::show(ctx, &self.workspace_manager, &self.notifications);

        // Render the sidebar (left panel); route its "+ New Workspace"
        // button through the same path as Cmd/Ctrl+N.
        let create_requested =
            self.sidebar.show(ctx, &mut self.workspace_manager, &self.notifications);
        if create_requested {
            let count = self.workspace_manager.workspace_count() + 1;
            let ws = self.create_workspace_with_terminal(format!("Workspace {count}"));
            tracing::info!(workspace_id = ws, "Created workspace via sidebar button");
        }

        // Render the notification panel (right panel, before the central
        // panel). The panel is shown when EITHER `Cmd+Opt+B` (right
        // sidebar toggle) OR `Cmd+I` (legacy notification bell) is on.
        // `visible = true` is forced for this frame so the panel's
        // self-gate lets the call through when the right sidebar is
        // the driver; both toggles remain independently owned.
        if self.sidebar.is_right_visible() || self.notification_panel.visible {
            self.notification_panel.visible = true;
            self.notification_panel.show(ctx, &mut self.notifications, &mut self.workspace_manager);
        }

        // Render the workspace view (central panel)
        self.render_workspace(ctx);

        // Process keyboard shortcuts AFTER UI render so ctx.wants_keyboard_input() works
        self.handle_keyboard_shortcuts(ctx);

        // Publish workspace.changed if the active workspace switched this
        // frame (keyboard, sidebar click, or API request).
        self.emit_workspace_change();
    }
}

impl RmuxApp {
    /// Drain and answer all pending socket API requests.
    ///
    /// Runs on the main thread inside `update()`, so the dispatcher has
    /// direct `&mut` access to application state. The `oneshot` respond
    /// send is synchronous and non-blocking.
    fn process_api_requests(&mut self) {
        while let Ok(envelope) = self.api_request_rx.try_recv() {
            tracing::debug!(method = %envelope.method, "handling API request");
            let result = crate::api_dispatch::dispatch(self, &envelope.method, envelope.params);
            let _ = envelope.respond.send(result);
        }
    }

    /// Publish an event to `events.stream` subscribers (best-effort:
    /// send errors just mean nobody is listening).
    pub(crate) fn publish_event(&self, event: &str, data: serde_json::Value) {
        let _ = self.api_event_tx.send(ApiEvent::new(event, data));
    }

    /// Store a notification raised by a pane's OSC output and publish
    /// the matching `notification` event.
    fn add_pane_notification(
        &mut self,
        workspace_id: u64,
        pane_id: u64,
        notification: rmux_terminal::OscNotification,
    ) {
        let id = self.notifications.add(
            notification.title.clone(),
            notification.body.clone(),
            Some(pane_id),
            Some(workspace_id),
        );
        tracing::debug!(id, pane_id, workspace_id, "OSC notification added");
        self.publish_event(
            "notification",
            json!({
                "id": id,
                "title": notification.title,
                "body": notification.body,
                "pane_id": pane_id,
                "workspace_id": workspace_id,
            }),
        );
    }

    /// Create a workspace with a live terminal in its initial pane.
    ///
    /// Shared by the Cmd/Ctrl+N shortcut and the `workspace.create` API
    /// method. Publishes `workspace.created` and `pane.created` events.
    /// Returns the raw id of the new workspace.
    pub(crate) fn create_workspace_with_terminal(&mut self, name: String) -> u64 {
        let ws = self.workspace_manager.create_workspace(name);
        let pane_id = self.workspace_manager.active().active_pane;
        attach_terminal(&mut self.workspace_manager, pane_id, self.font_size);
        self.publish_event("workspace.created", json!({ "id": ws.0 }));
        self.publish_event("pane.created", json!({ "pane_id": pane_id.0, "workspace_id": ws.0 }));
        ws.0
    }

    /// Split the active pane and spawn a terminal in the new pane.
    ///
    /// Shared by the Cmd/Ctrl+D shortcuts and the `surface.split` API
    /// method. Publishes a `pane.created` event. Returns the raw id of
    /// the new pane.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`PaneTreeError`] if the split fails.
    pub(crate) fn split_active_with_terminal(
        &mut self,
        direction: SplitDirection,
    ) -> Result<u64, PaneTreeError> {
        let new_id = match direction {
            SplitDirection::Horizontal => self.workspace_manager.split_active_right()?,
            SplitDirection::Vertical => self.workspace_manager.split_active_down()?,
        };
        attach_terminal(&mut self.workspace_manager, new_id, self.font_size);
        let workspace_id = self.workspace_manager.active().id.0;
        self.publish_event(
            "pane.created",
            json!({ "pane_id": new_id.0, "workspace_id": workspace_id }),
        );
        Ok(new_id.0)
    }

    /// Close the active pane and publish a `pane.closed` event.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`PaneTreeError`] if the pane cannot be
    /// closed (e.g. it is the last pane in the workspace).
    pub(crate) fn close_active_pane_with_event(&mut self) -> Result<(), PaneTreeError> {
        let workspace_id = self.workspace_manager.active().id.0;
        let pane_id = self.workspace_manager.active().active_pane.0;
        self.workspace_manager.close_active_pane()?;
        self.publish_event(
            "pane.closed",
            json!({ "pane_id": pane_id, "workspace_id": workspace_id }),
        );
        Ok(())
    }

    /// Open a browser pane split in the active workspace.
    ///
    /// Shared by the Cmd/Ctrl+Shift+L shortcut and the socket API.
    /// Publishes a `pane.created` event. Returns the raw id of the
    /// new browser pane.
    pub(crate) fn open_browser_split(&mut self, url: Option<&str>) -> Result<u64, PaneTreeError> {
        let new_id = self.workspace_manager.split_active_right()?;
        let browser = BrowserPane::new();
        self.workspace_manager.active_mut().set_browser(new_id, browser);
        let workspace_id = self.workspace_manager.active().id.0;
        if let Some(u) = url
            && let Some(b) = self.workspace_manager.active_mut().root.find_browser_mut(new_id)
        {
            let _ = b.navigate(u);
        }
        self.publish_event(
            "pane.created",
            json!({ "pane_id": new_id.0, "workspace_id": workspace_id }),
        );
        Ok(new_id.0)
    }

    /// Get a mutable reference to the active browser pane, if the active
    /// pane is a browser pane.
    pub(crate) fn active_browser_mut(&mut self) -> Option<&mut BrowserPane> {
        let pane_id = self.workspace_manager.active().active_pane;
        self.workspace_manager.active_mut().root.find_browser_mut(pane_id)
    }

    /// Close the active workspace and publish `workspace.closed`.
    ///
    /// Returns the closed workspace id, or an error if it is the last
    /// workspace remaining.
    pub(crate) fn close_active_workspace_with_event(&mut self) -> Result<u64, anyhow::Error> {
        let id = self.workspace_manager.close_active_workspace()?;
        self.publish_event("workspace.closed", json!({ "id": id.0 }));
        tracing::info!(workspace_id = id.0, "Closed active workspace via shortcut");
        Ok(id.0)
    }

    /// Start inline rename for the active workspace in the sidebar.
    pub(crate) fn start_workspace_rename(&mut self) {
        let index = self.workspace_manager.active_index();
        let name = self.workspace_manager.active().name.clone();
        self.sidebar.start_rename(index, name);
    }

    /// Change the terminal font size by the given delta.
    ///
    /// Pass `delta = 0.0` to reset to the default size. The effective
    /// font size is clamped to `[6.0, 60.0]`. After the change, all
    /// panes recalculate their cell grid and send a PTY resize.
    pub(crate) fn set_font_size(&mut self, delta: f32) {
        let new_size = if delta == 0.0 {
            DEFAULT_FONT_SIZE
        } else {
            (self.font_size + delta).clamp(6.0, 60.0)
        };

        if (new_size - self.font_size).abs() < f32::EPSILON {
            return; // no change
        }

        self.font_size = new_size;
        tracing::debug!(font_size = self.font_size, "Font size changed");

        // Propagate to every terminal pane across all workspaces
        for workspace in self.workspace_manager.workspaces_mut() {
            for (_, terminal) in workspace.root.leaf_panes_mut() {
                if let Some(t) = terminal.as_mut() {
                    t.set_font_size(self.font_size);
                }
            }
        }
    }

    /// Get a mutable reference to the active terminal pane, if any.
    pub(crate) fn active_terminal_mut(&mut self) -> Option<&mut TerminalPane> {
        self.workspace_manager.active_mut().active_terminal()
    }

    /// Publish `workspace.changed` if the active workspace differs from
    /// the previous frame.
    fn emit_workspace_change(&mut self) {
        let id = self.workspace_manager.active().id.0;
        if id != self.last_active_workspace {
            self.last_active_workspace = id;
            let index = self.workspace_manager.active_index();
            self.publish_event("workspace.changed", json!({ "id": id, "index": index }));
        }
    }

    /// Render the workspace area in the central panel of the window.
    fn render_workspace(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Snapshot the zoomed pane id with an immutable borrow, then
            // hand the manager (and the snapshot) to the renderer. The
            // renderer buffers tab-bar actions internally and replays
            // them after the tree-walk's `&mut Workspace` borrow ends.
            let zoomed = self.workspace_manager.active().zoomed_pane;
            workspace_view::render_pane_tree(ui, &mut self.workspace_manager, zoomed);
        });
    }
}

/// Spawn a terminal and attach it to `pane_id` in the active workspace.
///
/// Spawn failures are logged; the pane then shows the "Spawning
/// terminal..." placeholder indefinitely.
fn attach_terminal(manager: &mut WorkspaceManager, pane_id: PaneId, font_size: f32) {
    match TerminalPane::spawn(INITIAL_COLS, INITIAL_ROWS, font_size) {
        Ok(terminal) => manager.active_mut().set_terminal(pane_id, terminal),
        Err(e) => tracing::error!(pane_id = pane_id.0, "Failed to spawn terminal pane: {e}"),
    }
}
