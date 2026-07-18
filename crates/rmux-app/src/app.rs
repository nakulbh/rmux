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
use crate::ui::{HelpMenu, NotificationPanel, SettingsPanel, TerminalPane, workspace_view};
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
    /// The floating settings panel (terminal theme picker, etc).
    pub(crate) settings_panel: SettingsPanel,
    /// cmux-style help menu (circle-question in sidebar bottom-left).
    pub(crate) help_menu: HelpMenu,
    /// The current terminal font size (shared by all panes).
    pub(crate) font_size: f32,
    /// The current terminal color theme (shared by all panes).
    pub(crate) terminal_theme: rmux_terminal::NamedTheme,
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
    /// Cross-platform shortcut manager (`KeyboardShortcut` → `AppCommand`).
    pub(crate) shortcut_manager: crate::shortcut_manager::ShortcutManager,
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
            settings_panel: SettingsPanel::new(),
            help_menu: HelpMenu::new(),
            font_size,
            terminal_theme: rmux_terminal::NamedTheme::default(),
            last_copied_text: None,
            api_request_rx: channels.request_rx,
            api_event_tx: channels.event_tx,
            last_active_workspace: 0,
            shortcut_manager: crate::shortcut_manager::ShortcutManager::with_defaults(),
        };

        let pane_id = app.workspace_manager.active().active_pane;
        attach_terminal(&mut app.workspace_manager, pane_id, font_size, app.terminal_theme, None);
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
        // Process PTY output for all terminal panes (exit detection, grid).
        // OSC → notification generation is disabled for now.
        self.workspace_manager.process_all_panes();
        // cmux-style dynamic sidebar titles from focused process / path.
        self.workspace_manager.refresh_auto_titles();

        // Consume app shortcuts BEFORE UI so reserved chords never reach the
        // terminal PTY. On Linux egui sets both `ctrl` and `command` for Ctrl;
        // if the terminal reads the key first, the shortcut appears to need a
        // double-press. Dispatch runs immediately; commands only touch app state.
        self.handle_keyboard_shortcuts(ctx);

        // Auto-close tabs/panes whose process has exited; respawn the last
        // shell of the last workspace so the window is never left dead.
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
        for (workspace_id, pane_id) in cleanup.panes_needing_respawn {
            // Switch to the workspace that owns the empty pane, then attach.
            if let Some(idx) =
                self.workspace_manager.workspaces().iter().position(|w| w.id.0 == workspace_id)
            {
                self.workspace_manager.switch_to(idx);
            }
            attach_terminal(
                &mut self.workspace_manager,
                crate::workspace::splits::PaneId(pane_id),
                self.font_size,
                self.terminal_theme,
                None,
            );
            tracing::info!(workspace_id, pane_id, "Respawned terminal after shell exit");
        }

        // Handle any pending socket API requests on the main thread
        self.process_api_requests();

        // Request continuous repaints for terminal updates (PTY output, cursor blink)
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        // Render the top bar and status bar first so they span the full
        // window width (egui panel order: top/bottom before side panels).
        if let Some(action) = crate::ui::top_bar::show(
            ctx,
            &self.workspace_manager,
            &self.notifications,
            self.sidebar.visible,
            self.notification_panel.visible,
            self.settings_panel.open,
        ) {
            self.handle_top_bar_action(action);
        }
        crate::ui::status_bar::show(ctx, &self.workspace_manager, &self.notifications);

        // Render the settings panel (floating window); apply any theme
        // change picked this frame to every terminal pane.
        if let Some(new_theme) = self.settings_panel.show(ctx, self.terminal_theme) {
            self.set_terminal_theme(new_theme);
        }

        // Render the sidebar (left panel). New workspaces are created from
        // the top-bar `+` button (or Cmd/Ctrl+N). Hover × closes a card.
        // Help circle-question sits in the footer bottom-left.
        let mut help_button_rect = None;
        if let Some(crate::ui::sidebar::SidebarAction::CloseWorkspace(id)) = self.sidebar.show(
            ctx,
            &mut self.workspace_manager,
            &self.notifications,
            &mut self.help_menu,
            &mut help_button_rect,
        ) {
            match self.workspace_manager.close_workspace(id) {
                Ok(()) => {
                    self.publish_event("workspace.closed", json!({ "id": id.0 }));
                    tracing::info!(workspace_id = id.0, "Closed workspace via sidebar ×");
                }
                Err(err) => {
                    tracing::warn!(workspace_id = id.0, error = %err, "Could not close workspace");
                }
            }
        }

        // Help popup, welcome dialog, shortcuts window, update toast.
        self.help_menu.show_overlays(ctx, help_button_rect);

        // Render the notification panel (right panel, before the central
        // panel). The panel is shown when EITHER `Cmd+Opt+B` (right
        // sidebar toggle) OR `Cmd+I` (legacy notification bell) is on.
        // Both toggles remain independently owned — the right sidebar
        // temporarily forces visibility for this frame only so the
        // Show() self-gate lets the call through, without mutating
        // `self.notification_panel.visible` permanently.
        let right_drive = self.sidebar.is_right_visible();
        if right_drive || self.notification_panel.visible {
            if right_drive {
                self.notification_panel.visible = true;
            }
            self.notification_panel.show(ctx, &mut self.notifications, &mut self.workspace_manager);
        }

        // Render the workspace view (central panel)
        self.render_workspace(ctx);

        // Publish workspace.changed if the active workspace switched this
        // frame (keyboard, sidebar click, or API request).
        self.emit_workspace_change();
    }
}

impl RmuxApp {
    /// Apply a click from the cmux-style top bar toolbar / workspace tabs.
    fn handle_top_bar_action(&mut self, action: crate::ui::top_bar::TopBarAction) {
        use crate::ui::top_bar::TopBarAction;
        match action {
            TopBarAction::ToggleSidebar => {
                self.sidebar.toggle();
            }
            TopBarAction::ToggleNotifications => {
                self.notification_panel.toggle();
            }
            TopBarAction::ToggleSettings => {
                self.settings_panel.open = !self.settings_panel.open;
            }
            TopBarAction::NewWorkspace => {
                let ws = self.create_workspace_with_terminal("Terminal".to_string());
                tracing::info!(workspace_id = ws, "Created workspace via top bar");
            }
            TopBarAction::PrevWorkspace => {
                self.workspace_manager.switch_prev();
            }
            TopBarAction::NextWorkspace => {
                self.workspace_manager.switch_next();
            }
            TopBarAction::SelectWorkspace(index) => {
                self.workspace_manager.switch_to(index);
            }
        }
    }

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

    /// Create a workspace with a live terminal in its initial pane.
    ///
    /// Shared by the Cmd/Ctrl+N shortcut and the `workspace.create` API
    /// method. Publishes `workspace.created` and `pane.created` events.
    /// Returns the raw id of the new workspace.
    pub(crate) fn create_workspace_with_terminal(&mut self, name: String) -> u64 {
        let ws = self.workspace_manager.create_workspace(name);
        let pane_id = self.workspace_manager.active().active_pane;
        attach_terminal(
            &mut self.workspace_manager,
            pane_id,
            self.font_size,
            self.terminal_theme,
            None,
        );
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
        // Capture the focused terminal's cwd *before* the split mutates the
        // tree, so the new pane opens in the same directory (not $HOME).
        let cwd = self
            .workspace_manager
            .active()
            .root
            .find_pane(self.workspace_manager.active().active_pane)
            .and_then(|n| n.active_terminal())
            .and_then(|t| t.working_directory());
        let new_id = match direction {
            SplitDirection::Horizontal => self.workspace_manager.split_active_right()?,
            SplitDirection::Vertical => self.workspace_manager.split_active_down()?,
        };
        attach_terminal(
            &mut self.workspace_manager,
            new_id,
            self.font_size,
            self.terminal_theme,
            cwd.as_deref(),
        );
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

        // Propagate to every terminal (legacy slots + multi-surface tabs).
        let size = self.font_size;
        for workspace in self.workspace_manager.workspaces_mut() {
            workspace.root.for_each_terminal_mut(&mut |t| t.set_font_size(size));
        }
    }

    /// Change the terminal color theme, propagating it to every pane
    /// across all workspaces (legacy leaf slots and Cmd+T surfaces).
    /// New terminals pick it up via [`Self::new_surface_with_terminal`]
    /// and `attach_terminal`. Also updates the app-wide UI palette.
    pub(crate) fn set_terminal_theme(&mut self, named: rmux_terminal::NamedTheme) {
        self.terminal_theme = named;
        crate::ui::theme::set_named_theme(named);
        tracing::debug!(?named, "Terminal theme changed");

        let theme = rmux_terminal::TerminalTheme::default().named(named);
        for workspace in self.workspace_manager.workspaces_mut() {
            workspace.root.for_each_terminal_mut(&mut |t| t.set_theme(theme));
        }
    }

    /// Create a new terminal tab (Cmd+T / tab-bar `+`) with the app's
    /// current font size and color theme applied.
    ///
    /// `Workspace::new_surface` alone would spawn with defaults and leave
    /// the tab on the default palette after a theme change.
    pub(crate) fn new_surface_with_terminal(
        &mut self,
        title: Option<String>,
    ) -> Result<u64, crate::workspace::model::WorkspaceError> {
        let id = self.workspace_manager.new_surface_in_active(title)?;
        let theme = rmux_terminal::TerminalTheme::default().named(self.terminal_theme);
        let font_size = self.font_size;
        if let Some(term) = self.workspace_manager.active_mut().active_terminal() {
            term.set_font_size(font_size);
            term.set_theme(theme);
        }
        let workspace_id = self.workspace_manager.active().id.0;
        self.publish_event(
            "pane.created",
            json!({ "pane_id": id.0, "workspace_id": workspace_id, "kind": "surface" }),
        );
        tracing::info!(surface_id = id.0, "Created new surface with current theme");
        Ok(id.0)
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
        let mut new_tab = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            // Snapshot the zoomed pane id with an immutable borrow, then
            // hand the manager (and the snapshot) to the renderer. The
            // renderer buffers tab-bar actions internally and replays
            // them after the tree-walk's `&mut Workspace` borrow ends.
            let zoomed = self.workspace_manager.active().zoomed_pane;
            new_tab = workspace_view::render_pane_tree(ui, &mut self.workspace_manager, zoomed);
        });
        // Tab-bar "+" — same path as Cmd+T so theme/font match.
        if new_tab && let Err(e) = self.new_surface_with_terminal(None) {
            tracing::warn!(error = %e, "tab-bar new surface failed");
        }
    }
}

/// Spawn a terminal and attach it to `pane_id` in the active workspace.
///
/// When `cwd` is `Some`, the shell starts in that directory (used to inherit
/// the focused pane's path on split / new tab). Spawn failures are logged;
/// the pane then shows the "Spawning terminal..." placeholder indefinitely.
fn attach_terminal(
    manager: &mut WorkspaceManager,
    pane_id: PaneId,
    font_size: f32,
    named_theme: rmux_terminal::NamedTheme,
    cwd: Option<&std::path::Path>,
) {
    match TerminalPane::spawn_with_cwd(INITIAL_COLS, INITIAL_ROWS, font_size, cwd) {
        Ok(mut terminal) => {
            terminal.set_theme(rmux_terminal::TerminalTheme::default().named(named_theme));
            manager.active_mut().set_terminal(pane_id, terminal);
        }
        Err(e) => tracing::error!(pane_id = pane_id.0, "Failed to spawn terminal pane: {e}"),
    }
}
