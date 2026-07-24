//! Application state and main egui rendering logic.
//!
//! The `RmuxApp` struct owns the top-level application state including the
//! workspace manager, sidebar view, notification manager, and the socket
//! API channel endpoints. It implements `eframe::App` to drive the UI.
//! Keyboard shortcut handling lives in [`crate::shortcuts`]; socket API
//! request handling lives in [`crate::api_dispatch`].

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rmux_api::ApiEvent;
use rmux_config::{AppearanceConfig, Config};
use serde_json::json;

use crate::api;
use crate::browser::BrowserPane;
use crate::notifications::NotificationManager;
use crate::ui::DEFAULT_FONT_SIZE;
use crate::ui::sidebar::SidebarView;
use crate::ui::{
    HelpMenu, NotificationPanel, SettingsPanel, TerminalPane, Wallpaper, workspace_view,
};
use crate::workspace::WorkspaceManager;
use crate::workspace::session::{
    self, CaptureUiState, DEFAULT_AUTOSAVE_SECS, LoadOutcome, RestoreOptions, SessionSnapshot,
    SessionStore,
};
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
    /// File-backed session store (primary + previous).
    session_store: SessionStore,
    /// Autosave interval.
    session_autosave_secs: u64,
    /// Last successful autosave fingerprint (skip unchanged writes).
    last_session_fingerprint: Option<String>,
    /// Instant of last autosave attempt.
    last_session_save_at: Instant,
    /// True while applying a session restore (skip nested saves).
    is_applying_session_restore: bool,
    /// Desired window size from the restored session (applied once).
    pending_window_size: Option<[f32; 2]>,
    /// Loaded/saved `rmux.json` (terminal + appearance + session).
    pub(crate) config: Config,
    /// Shared workspace wallpaper (one image behind every pane).
    pub(crate) wallpaper: Wallpaper,
}

/// Peek at the saved session's window size before the egui window opens.
pub fn peek_session_window_size() -> Option<[f32; 2]> {
    if !session::should_attempt_restore(cli_session_path().as_deref()) {
        return None;
    }
    let store = session_store_for_launch();
    match load_launch_snapshot(&store) {
        Some(snap) => Some(snap.window.inner_size),
        None => None,
    }
}

fn cli_session_path() -> Option<String> {
    crate::CLI_SESSION_PATH.get().cloned().flatten()
}

fn session_store_for_launch() -> SessionStore {
    SessionStore::default_store()
}

fn load_launch_snapshot(store: &SessionStore) -> Option<SessionSnapshot> {
    if let Some(path) = cli_session_path() {
        match store.load_outcome(PathBuf::from(path).as_path()) {
            LoadOutcome::Loaded(s) => return Some(s),
            other => {
                tracing::warn!(?other, "explicit --session path not usable");
                return None;
            }
        }
    }
    match store.load_startup() {
        LoadOutcome::Loaded(s) => Some(s),
        LoadOutcome::Missing => None,
        LoadOutcome::Unusable => None,
    }
}

impl RmuxApp {
    /// Create a new application state with a default workspace and terminal pane.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let channels = api::start_server();
        let config = match rmux_config::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load config; using defaults");
                Config::default()
            }
        };
        let font_size = if config.terminal.font_size > 0.0 {
            config.terminal.font_size
        } else {
            DEFAULT_FONT_SIZE
        };
        let session_store = session_store_for_launch();
        let session_autosave_secs = if config.session.autosave_secs > 0 {
            config.session.autosave_secs
        } else {
            DEFAULT_AUTOSAVE_SECS
        };

        let restore_opts = RestoreOptions {
            cols: INITIAL_COLS,
            rows: INITIAL_ROWS,
            font_size,
            theme: rmux_terminal::NamedTheme::default(),
        };

        let mut pending_window_size = None;
        let mut sidebar = SidebarView::new();
        let mut notification_panel = NotificationPanel::new();

        // Honor config.session.auto_restore unless an explicit --session path
        // is provided (or RMUX_DISABLE_SESSION_RESTORE is set).
        let attempt_restore = if cli_session_path().is_some() {
            session::should_attempt_restore(cli_session_path().as_deref())
        } else {
            config.session.auto_restore
                && session::should_attempt_restore(cli_session_path().as_deref())
        };

        let (workspace_manager, restored) = if attempt_restore {
            match load_launch_snapshot(&session_store) {
                Some(snap) => match session::restore_session(&snap, &restore_opts) {
                    Ok(manager) => {
                        pending_window_size = Some(snap.window.inner_size);
                        sidebar.visible = snap.window.sidebar_visible;
                        sidebar.right_sidebar_visible = snap.window.right_sidebar_visible;
                        notification_panel.visible = snap.window.notification_panel_visible;
                        tracing::info!(
                            workspaces = manager.workspace_count(),
                            panes = manager.total_pane_count(),
                            "Restored previous session"
                        );
                        (manager, true)
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "Session restore failed; starting fresh");
                        (WorkspaceManager::new(), false)
                    }
                },
                None => (WorkspaceManager::new(), false),
            }
        } else {
            (WorkspaceManager::new(), false)
        };

        let mut app = Self {
            workspace_manager,
            sidebar,
            notifications: NotificationManager::with_system_notifier(),
            notification_panel,
            settings_panel: SettingsPanel::new(),
            help_menu: HelpMenu::new(),
            font_size,
            terminal_theme: rmux_terminal::NamedTheme::default(),
            last_copied_text: None,
            api_request_rx: channels.request_rx,
            api_event_tx: channels.event_tx,
            last_active_workspace: 0,
            shortcut_manager: crate::shortcut_manager::ShortcutManager::with_defaults(),
            session_store,
            session_autosave_secs,
            last_session_fingerprint: None,
            last_session_save_at: Instant::now(),
            is_applying_session_restore: false,
            pending_window_size,
            config,
            wallpaper: Wallpaper::new(),
        };

        // Load wallpaper texture early if configured.
        app.sync_wallpaper_from_config(&cc.egui_ctx);

        if !restored {
            let pane_id = app.workspace_manager.active().active_pane;
            let bg_opacity = app.effective_bg_opacity();
            attach_terminal(
                &mut app.workspace_manager,
                pane_id,
                font_size,
                app.terminal_theme,
                None,
                bg_opacity,
            );
        } else {
            // Restored terminals need the same wallpaper opacity as new ones.
            app.propagate_bg_opacity();
        }
        app.last_active_workspace = app.workspace_manager.active().id.0;

        // Seed fingerprint so the first autosave is skipped until the user
        // changes something (or quit forces a save).
        let seed = session::capture_session(
            &app.workspace_manager,
            CaptureUiState {
                inner_size: app.pending_window_size.unwrap_or([1200.0, 800.0]),
                sidebar_visible: app.sidebar.visible,
                right_sidebar_visible: app.sidebar.right_sidebar_visible,
                notification_panel_visible: app.notification_panel.visible,
            },
        );
        app.last_session_fingerprint = Some(seed.content_fingerprint());

        tracing::info!(
            workspaces = app.workspace_manager.workspace_count(),
            panes = app.workspace_manager.total_pane_count(),
            restored,
            wallpaper = app.config.appearance.wallpaper_active(),
            "Application initialized"
        );
        app
    }

    /// Capture + write the current session (used by autosave, quit, manual restore prep).
    pub(crate) fn save_session_now(&mut self, ctx: Option<&egui::Context>) {
        if self.is_applying_session_restore {
            return;
        }
        let inner_size = ctx
            .map(|c| {
                let rect = c.input(|i| i.screen_rect());
                [rect.width(), rect.height()]
            })
            .or(self.pending_window_size)
            .unwrap_or([1200.0, 800.0]);
        let snap = session::capture_session(
            &self.workspace_manager,
            CaptureUiState {
                inner_size,
                sidebar_visible: self.sidebar.visible,
                right_sidebar_visible: self.sidebar.right_sidebar_visible,
                notification_panel_visible: self.notification_panel.visible,
            },
        );
        let fp = snap.content_fingerprint();
        if self.last_session_fingerprint.as_ref() == Some(&fp) {
            return;
        }
        match self.session_store.save(&snap) {
            Ok(()) => {
                self.last_session_fingerprint = Some(fp);
                self.last_session_save_at = Instant::now();
                tracing::debug!(
                    workspaces = snap.workspaces.len(),
                    path = %self.session_store.primary_path().display(),
                    "Session saved"
                );
            }
            Err(err) => tracing::warn!(error = %err, "Failed to save session"),
        }
    }

    /// Force save even when fingerprint matches (quit path still wants a write
    /// of timestamps; we keep skip-on-identical bytes inside the store).
    pub(crate) fn save_session_on_exit(&mut self, ctx: Option<&egui::Context>) {
        if self.is_applying_session_restore {
            return;
        }
        // Clear fingerprint so quit always attempts a write of the latest tree.
        self.last_session_fingerprint = None;
        self.save_session_now(ctx);
    }

    /// Replace the live session with `session-previous.json` (⌘⇧O).
    pub(crate) fn reopen_previous_session(&mut self) {
        let Some(snap) = self.session_store.load_previous() else {
            tracing::warn!("No previous session snapshot to restore");
            return;
        };
        // Save current into primary first so the user can recover.
        self.save_session_on_exit(None);
        self.apply_session_snapshot(snap);
    }

    fn apply_session_snapshot(&mut self, snap: SessionSnapshot) {
        let opts = RestoreOptions {
            cols: INITIAL_COLS,
            rows: INITIAL_ROWS,
            font_size: self.font_size,
            theme: self.terminal_theme,
        };
        self.is_applying_session_restore = true;
        match session::restore_session(&snap, &opts) {
            Ok(manager) => {
                self.workspace_manager = manager;
                self.sidebar.visible = snap.window.sidebar_visible;
                self.sidebar.right_sidebar_visible = snap.window.right_sidebar_visible;
                self.notification_panel.visible = snap.window.notification_panel_visible;
                self.pending_window_size = Some(snap.window.inner_size);
                self.last_active_workspace = self.workspace_manager.active().id.0;
                self.propagate_bg_opacity();
                let seed = session::capture_session(
                    &self.workspace_manager,
                    CaptureUiState {
                        inner_size: snap.window.inner_size,
                        sidebar_visible: self.sidebar.visible,
                        right_sidebar_visible: self.sidebar.right_sidebar_visible,
                        notification_panel_visible: self.notification_panel.visible,
                    },
                );
                self.last_session_fingerprint = Some(seed.content_fingerprint());
                tracing::info!(
                    workspaces = self.workspace_manager.workspace_count(),
                    "Applied session snapshot"
                );
            }
            Err(err) => tracing::error!(error = %err, "Failed to apply session snapshot"),
        }
        self.is_applying_session_restore = false;
    }

    fn maybe_autosave_session(&mut self, ctx: &egui::Context) {
        let interval = Duration::from_secs(self.session_autosave_secs.max(2));
        if self.last_session_save_at.elapsed() < interval {
            return;
        }
        self.save_session_now(Some(ctx));
    }

    /// Opacity applied to default terminal backgrounds (`1.0` when wallpaper off).
    ///
    /// Floored at `0.25` so text stays readable and the UI never "disappears"
    /// into a pure wallpaper (users can still go quite transparent).
    fn effective_bg_opacity(&self) -> f32 {
        if self.config.appearance.wallpaper_active() {
            self.config.appearance.clamped_opacity().max(0.25)
        } else {
            1.0
        }
    }

    /// Load or clear the wallpaper texture from the current appearance config.
    fn sync_wallpaper_from_config(&mut self, ctx: &egui::Context) {
        if self.config.appearance.wallpaper_active() {
            let path =
                self.config.appearance.background_image.as_deref().map(rmux_config::expand_tilde);
            self.wallpaper.ensure_loaded(ctx, path.as_deref());
        } else {
            self.wallpaper.clear();
        }
        self.propagate_bg_opacity();
    }

    /// Push background opacity to every terminal pane (all workspaces).
    fn propagate_bg_opacity(&mut self) {
        let opacity = self.effective_bg_opacity();
        for workspace in self.workspace_manager.workspaces_mut() {
            workspace.root.for_each_terminal_mut(&mut |t| t.set_bg_opacity(opacity));
        }
    }

    /// Apply appearance changes from settings and persist to disk.
    fn apply_appearance(
        &mut self,
        ctx: &egui::Context,
        appearance: AppearanceConfig,
        reload: bool,
    ) {
        self.config.appearance = appearance;
        if reload {
            if let Some(ref p) = self.config.appearance.background_image {
                let path = rmux_config::expand_tilde(p);
                self.wallpaper.reload(ctx, &path);
            } else {
                self.wallpaper.clear();
            }
            // Still respect enabled flag after reload.
            if !self.config.appearance.wallpaper_active() {
                self.wallpaper.clear();
            }
        } else {
            self.sync_wallpaper_from_config(ctx);
        }
        self.propagate_bg_opacity();
        if let Err(e) = rmux_config::save(&self.config) {
            tracing::warn!(error = %e, "Failed to save config");
        }
    }
}

impl eframe::App for RmuxApp {
    /// Called each frame to update the UI.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply shadcn-inspired theme every frame
        crate::ui::theme::Theme::dark().apply(ctx);

        // Apply restored window size once after the window exists.
        if let Some(size) = self.pending_window_size.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
        }

        // Process PTY output for all terminal panes (exit detection, grid).
        // OSC → notification generation is disabled for now.
        self.workspace_manager.process_all_panes();
        // cmux-style dynamic sidebar titles from focused process / path.
        self.workspace_manager.refresh_auto_titles();

        // Consume app shortcuts BEFORE UI so reserved chords never reach the
        // terminal PTY. On Linux egui sets both `ctrl` and `command` for Ctrl;
        // if the terminal reads the key first, the shortcut appears to need a
        // double-press. Dispatch runs immediately; commands only touch app state.
        //
        // text_sink still holds last frame's mark here (rename/find/URL) so
        // bare Escape/Enter gating matches the previous-frame focus model.
        self.handle_keyboard_shortcuts(ctx);

        // Clear the sink for this frame; TextEdits re-mark while drawing.
        crate::ui::text_sink::begin_frame(ctx);

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
            let bg_opacity = self.effective_bg_opacity();
            let font_size = self.font_size;
            let theme = self.terminal_theme;
            attach_terminal(
                &mut self.workspace_manager,
                crate::workspace::splits::PaneId(pane_id),
                font_size,
                theme,
                None,
                bg_opacity,
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

        // Keep wallpaper texture ready (painted only inside panels — never on a
        // free-floating background layer that can cover top bar / chrome).
        if self.config.appearance.wallpaper_active() {
            let path =
                self.config.appearance.background_image.as_deref().map(rmux_config::expand_tilde);
            self.wallpaper.ensure_loaded(ctx, path.as_deref());
        }

        // Render the settings panel (theme + workspace wallpaper).
        let wall_status = self.wallpaper.status_message().map(str::to_owned);
        let settings_changes = self.settings_panel.show(
            ctx,
            self.terminal_theme,
            &self.config.appearance,
            wall_status.as_deref(),
        );
        if let Some(new_theme) = settings_changes.theme {
            self.set_terminal_theme(new_theme);
        }
        if let Some(appearance) = settings_changes.appearance {
            self.apply_appearance(ctx, appearance, settings_changes.reload_wallpaper);
        } else if settings_changes.reload_wallpaper {
            self.sync_wallpaper_from_config(ctx);
        }

        // Render the sidebar (left panel). New workspaces are created from
        // the top-bar `+` button (or Cmd/Ctrl+N). Hover × closes a card.
        // Help circle-question sits in the footer bottom-left.
        // Sidebar glass only when wallpaper is active; image is painted inside
        // the panel so it never covers the top/status chrome.
        let sidebar_opacity = if self.config.appearance.wallpaper_active() {
            self.config.appearance.clamped_sidebar_opacity()
        } else {
            1.0
        };
        let sidebar_wall = if self.config.appearance.wallpaper_active() && self.wallpaper.is_ready()
        {
            Some(&self.wallpaper)
        } else {
            None
        };
        let mut help_button_rect = None;
        if let Some(crate::ui::sidebar::SidebarAction::CloseWorkspace(id)) = self.sidebar.show(
            ctx,
            &mut self.workspace_manager,
            &self.notifications,
            &mut self.help_menu,
            &mut help_button_rect,
            sidebar_opacity,
            sidebar_wall,
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

        // Periodic session autosave (layout + cwd; no heavy scrollback).
        self.maybe_autosave_session(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        tracing::info!("Saving session on exit");
        self.save_session_on_exit(None);
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
        let bg_opacity = self.effective_bg_opacity();
        let font_size = self.font_size;
        let theme = self.terminal_theme;
        attach_terminal(&mut self.workspace_manager, pane_id, font_size, theme, None, bg_opacity);
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
        let bg_opacity = self.effective_bg_opacity();
        let font_size = self.font_size;
        let theme = self.terminal_theme;
        let new_id = match direction {
            SplitDirection::Horizontal => self.workspace_manager.split_active_right()?,
            SplitDirection::Vertical => self.workspace_manager.split_active_down()?,
        };
        attach_terminal(
            &mut self.workspace_manager,
            new_id,
            font_size,
            theme,
            cwd.as_deref(),
            bg_opacity,
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
    /// current font size, color theme, and wallpaper opacity applied.
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
        let bg_opacity = self.effective_bg_opacity();
        if let Some(term) = self.workspace_manager.active_mut().active_terminal() {
            term.set_font_size(font_size);
            term.set_theme(theme);
            term.set_bg_opacity(bg_opacity);
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
        // Keep wallpaper texture in sync if path was set before ctx existed
        // (already loaded in new) and ensure opacity is applied.
        if self.config.appearance.wallpaper_active() {
            let path =
                self.config.appearance.background_image.as_deref().map(rmux_config::expand_tilde);
            self.wallpaper.ensure_loaded(ctx, path.as_deref());
        }

        let mut new_tab = false;
        let wallpaper_on = self.config.appearance.wallpaper_active() && self.wallpaper.is_ready();
        // Transparent panel fill so the shared wallpaper shows through the
        // central area instead of egui's default opaque panel background.
        let panel_frame =
            if wallpaper_on { egui::Frame::NONE } else { egui::Frame::central_panel(&ctx.style()) };
        egui::CentralPanel::default().frame(panel_frame).show(ctx, |ui| {
            // Snapshot the zoomed pane id with an immutable borrow, then
            // hand the manager (and the snapshot) to the renderer. The
            // renderer buffers tab-bar actions internally and replays
            // them after the tree-walk's `&mut Workspace` borrow ends.
            let zoomed = self.workspace_manager.active().zoomed_pane;
            let bg = workspace_view::WorkspaceBackground {
                wallpaper: &self.wallpaper,
                active: wallpaper_on,
            };
            new_tab =
                workspace_view::render_pane_tree(ui, &mut self.workspace_manager, zoomed, Some(bg));
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
    bg_opacity: f32,
) {
    match TerminalPane::spawn_with_cwd(INITIAL_COLS, INITIAL_ROWS, font_size, cwd) {
        Ok(mut terminal) => {
            terminal.set_theme(rmux_terminal::TerminalTheme::default().named(named_theme));
            terminal.set_bg_opacity(bg_opacity);
            manager.active_mut().set_terminal(pane_id, terminal);
        }
        Err(e) => tracing::error!(pane_id = pane_id.0, "Failed to spawn terminal pane: {e}"),
    }
}
