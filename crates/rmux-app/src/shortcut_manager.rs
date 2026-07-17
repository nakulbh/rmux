//! Cross-platform keyboard shortcut manager.
//!
//! # Architecture
//!
//! ```text
//! Keyboard (macOS / Linux / Windows)
//!         │
//!         ▼
//!   egui Input Events  (logical Key + Modifiers)
//!         │
//!         ▼
//!   ShortcutManager    (KeyboardShortcut → AppCommand)
//!         │
//!         ▼
//!   AppCommand enum
//!         │
//!         ▼
//!   Application logic  (never inspects raw keyboard state)
//! ```
//!
//! # Design rules
//!
//! - Bindings use [`Modifiers::COMMAND`] for standard app shortcuts so egui
//!   maps them to **⌘** on macOS and **Ctrl** on Linux/Windows automatically.
//! - Matching uses [`Modifiers::matches_logically`] (same as
//!   [`egui::InputState::consume_shortcut`]) — never raw OS key codes.
//! - More-specific chords are registered first so `Cmd+Shift+S` wins over
//!   `Cmd+S` when both could match.
//! - The manager is pure mapping logic; it is unit-testable without a window.
//! - Future user-configurable bindings: call [`ShortcutManager::bind`] /
//!   [`ShortcutManager::clear_and_rebind`] without touching app logic.

use egui::{Context, Key, KeyboardShortcut, Modifiers};

/// High-level application command produced by the shortcut manager.
///
/// Application code should react to these values only — never to raw
/// `egui::Key` / modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum AppCommand {
    Quit,
    FontSizeUp,
    FontSizeDown,
    FontSizeReset,
    /// Copy the active terminal selection to the system clipboard.
    Copy,
    /// Paste system clipboard text into the active terminal.
    Paste,
    /// Open / toggle find, or close find when fired as bare Escape.
    Find,
    FindNext,
    FindPrev,
    UseSelectionForFind,
    ClearScrollback,
    ClearScreen,
    ToggleSidebar,
    ToggleNotifications,
    NewWorkspace,
    SplitRight,
    SplitDown,
    ClosePane,
    OpenBrowserSplit,
    FocusBrowserUrlBar,
    ReloadBrowser,
    SwitchWorkspace(usize),
    CloseWorkspace,
    RenameWorkspace,
    ToggleZoom,
    EqualizeSplits,
    PrevWorkspace,
    NextWorkspace,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    NewSurface,
    NextSurface,
    PreviousSurface,
    SelectSurface(usize),
    RenameTab,
    CloseTab,
    CloseOtherTabs,
    ReopenLastClosed,
    ToggleCopyMode,
    SplitBrowserRight,
    SplitBrowserDown,
    ToggleRightSidebar,
    NewWindow,
    CloseWindow,
    EqualizeSplitsAlt,
    PrevWorkspaceAlt,
    NextWorkspaceAlt,
    PasteImage,
}

/// Options that gate context-sensitive bindings during a poll.
#[derive(Debug, Clone, Copy, Default)]
pub struct PollOptions {
    /// An egui text widget currently owns the keyboard (TextEdit, etc.).
    pub text_focused: bool,
    /// Terminal find bar is open — bare Escape/Enter act as find controls.
    pub find_visible: bool,
    /// Active terminal has a non-empty text selection (gates Copy vs SIGINT).
    pub has_selection: bool,
}

/// One binding: logical shortcut → command.
#[derive(Debug, Clone)]
pub struct ShortcutBinding {
    pub shortcut: KeyboardShortcut,
    pub command: AppCommand,
    /// When true, the binding only fires if [`PollOptions::find_visible`].
    pub requires_find: bool,
    /// When true, the binding is suppressed while a text widget has focus
    /// (bare Escape/Enter so TextEdit keeps those keys).
    pub suppress_when_text_focused: bool,
    /// When true, only fire if [`PollOptions::has_selection`] (e.g. Copy).
    pub requires_selection: bool,
}

impl ShortcutBinding {
    pub const fn new(shortcut: KeyboardShortcut, command: AppCommand) -> Self {
        Self {
            shortcut,
            command,
            requires_find: false,
            suppress_when_text_focused: false,
            requires_selection: false,
        }
    }

    pub const fn find_only(mut self) -> Self {
        self.requires_find = true;
        self.suppress_when_text_focused = true;
        self
    }

    /// Only fire when the terminal has an active selection.
    pub const fn selection_only(mut self) -> Self {
        self.requires_selection = true;
        self
    }
}

/// Maps keyboard input to [`AppCommand`]s.
///
/// Owns the full binding table. Application logic never reads keyboard state
/// directly — it only executes the commands returned by [`Self::poll`].
#[derive(Debug, Clone)]
pub struct ShortcutManager {
    bindings: Vec<ShortcutBinding>,
}

impl ShortcutManager {
    /// Empty manager (for custom / test tables).
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    /// Built-in default bindings (cmux-compatible, cross-platform).
    pub fn with_defaults() -> Self {
        let mut m = Self { bindings: Vec::new() };
        m.install_defaults();
        m
    }

    /// Register a binding. More-specific chords should be added first when
    /// they share a key with a less-specific chord.
    pub fn bind(&mut self, binding: ShortcutBinding) {
        self.bindings.push(binding);
    }

    /// Convenience: bind `modifiers + key → command`.
    pub fn bind_chord(&mut self, modifiers: Modifiers, key: Key, command: AppCommand) {
        self.bind(ShortcutBinding::new(KeyboardShortcut::new(modifiers, key), command));
    }

    /// Replace all bindings (for future user config reload).
    #[allow(dead_code)]
    pub fn clear_and_rebind(&mut self, bindings: Vec<ShortcutBinding>) {
        self.bindings = bindings;
    }

    /// Immutable view of bindings (config UI / debugging).
    #[allow(dead_code)]
    pub fn bindings(&self) -> &[ShortcutBinding] {
        &self.bindings
    }

    /// Pure lookup: given pressed modifiers + key, return the first matching
    /// command. Uses the same logical matching as `consume_shortcut`.
    ///
    /// Does **not** apply find/selection/text-focus gates — use
    /// [`Self::resolve_with_options`] for those. Suitable for unit tests
    /// without an egui context.
    #[allow(dead_code)]
    pub fn resolve(&self, pressed: Modifiers, key: Key) -> Option<AppCommand> {
        for b in &self.bindings {
            // Skip gated bindings so a bare resolve doesn't claim Copy without
            // selection context (callers should use resolve_with_options).
            if b.requires_find || b.requires_selection {
                continue;
            }
            if b.shortcut.logical_key == key && pressed.matches_logically(b.shortcut.modifiers) {
                return Some(b.command);
            }
        }
        None
    }

    /// Like [`Self::resolve`] but respects [`PollOptions`] gates (find bar /
    /// text focus) the same way [`Self::poll`] does.
    #[allow(dead_code)]
    pub fn resolve_with_options(
        &self,
        pressed: Modifiers,
        key: Key,
        opts: PollOptions,
    ) -> Option<AppCommand> {
        for b in &self.bindings {
            if b.requires_find && !opts.find_visible {
                continue;
            }
            if b.suppress_when_text_focused && opts.text_focused {
                continue;
            }
            if b.requires_selection && !opts.has_selection {
                continue;
            }
            if b.shortcut.logical_key == key && pressed.matches_logically(b.shortcut.modifiers) {
                return Some(b.command);
            }
        }
        None
    }

    /// Consume matching shortcuts from the egui input state and return the
    /// commands that fired this frame (at most one per binding, typically
    /// one total because more-specific bindings consume first).
    ///
    /// Call **before** rendering the terminal so reserved chords never reach
    /// the PTY — this is the fix for Linux double-press / stolen keys.
    pub fn poll(&self, ctx: &Context, opts: PollOptions) -> Vec<AppCommand> {
        let mut commands = Vec::new();
        ctx.input_mut(|input| {
            for b in &self.bindings {
                if b.requires_find && !opts.find_visible {
                    continue;
                }
                if b.suppress_when_text_focused && opts.text_focused {
                    continue;
                }
                if b.requires_selection && !opts.has_selection {
                    continue;
                }
                if input.consume_shortcut(&b.shortcut) {
                    commands.push(b.command);
                }
            }
        });
        commands
    }

    /// Install the full default binding table.
    ///
    /// Standard app shortcuts use [`Modifiers::COMMAND`] (⌘ / Ctrl). Dual
    /// physical-Ctrl+⌘ chords (cmux window-management aliases) use
    /// [`Modifiers::MAC_CMD`] so they only match on macOS and never steal
    /// plain Ctrl chords on Linux.
    fn install_defaults(&mut self) {
        // ── Most-specific first ──────────────────────────────────────────
        // Shift/Alt variants before bare COMMAND on the same key.

        // Font size
        self.bind_chord(Modifiers::COMMAND, Key::Plus, AppCommand::FontSizeUp);
        self.bind_chord(Modifiers::COMMAND, Key::Equals, AppCommand::FontSizeUp);
        self.bind_chord(Modifiers::COMMAND, Key::Minus, AppCommand::FontSizeDown);
        self.bind_chord(Modifiers::COMMAND, Key::Num0, AppCommand::FontSizeReset);

        // Find (modified)
        self.bind_chord(Modifiers::COMMAND | Modifiers::ALT, Key::G, AppCommand::FindPrev);
        self.bind_chord(Modifiers::COMMAND, Key::G, AppCommand::FindNext);
        self.bind_chord(Modifiers::COMMAND, Key::F, AppCommand::Find);
        self.bind_chord(Modifiers::COMMAND, Key::E, AppCommand::UseSelectionForFind);

        // Clear
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::K, AppCommand::ClearScreen);
        self.bind_chord(Modifiers::COMMAND, Key::K, AppCommand::ClearScrollback);

        // Chrome
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::ALT,
            Key::B,
            AppCommand::ToggleRightSidebar,
        );
        self.bind_chord(Modifiers::COMMAND, Key::B, AppCommand::ToggleSidebar);
        self.bind_chord(Modifiers::COMMAND, Key::I, AppCommand::ToggleNotifications);

        // Workspace / window
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::N, AppCommand::NewWindow);
        self.bind_chord(Modifiers::COMMAND, Key::N, AppCommand::NewWorkspace);
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::W, AppCommand::CloseWorkspace);

        // macOS-only dual chord: physical Ctrl+⌘W → close window
        self.bind_chord(mac_ctrl_cmd(), Key::W, AppCommand::CloseWindow);
        self.bind_chord(Modifiers::COMMAND, Key::W, AppCommand::CloseTab);

        // Splits
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT,
            Key::D,
            AppCommand::SplitBrowserDown,
        );
        self.bind_chord(Modifiers::COMMAND | Modifiers::ALT, Key::D, AppCommand::SplitBrowserRight);
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::D, AppCommand::SplitDown);
        self.bind_chord(Modifiers::COMMAND, Key::D, AppCommand::SplitRight);

        // Browser
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::L,
            AppCommand::OpenBrowserSplit,
        );
        self.bind_chord(Modifiers::COMMAND, Key::L, AppCommand::FocusBrowserUrlBar);
        self.bind_chord(Modifiers::COMMAND, Key::R, AppCommand::ReloadBrowser);
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::R, AppCommand::RenameWorkspace);

        // Zoom / equalize
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter, AppCommand::ToggleZoom);
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::Equals,
            AppCommand::EqualizeSplits,
        );
        self.bind_chord(mac_ctrl_cmd(), Key::Equals, AppCommand::EqualizeSplitsAlt);

        // Focus panes (⌘⌥ arrows)
        self.bind_chord(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowLeft, AppCommand::FocusLeft);
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::ALT,
            Key::ArrowRight,
            AppCommand::FocusRight,
        );
        self.bind_chord(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowUp, AppCommand::FocusUp);
        self.bind_chord(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowDown, AppCommand::FocusDown);

        // Surfaces / tabs
        self.bind_chord(Modifiers::COMMAND, Key::T, AppCommand::NewSurface);
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::CloseBracket,
            AppCommand::NextSurface,
        );
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::OpenBracket,
            AppCommand::PreviousSurface,
        );
        // Workspace nav aliases: physical Ctrl+⌘ [ / ]
        self.bind_chord(mac_ctrl_cmd(), Key::OpenBracket, AppCommand::PrevWorkspace);
        self.bind_chord(mac_ctrl_cmd(), Key::CloseBracket, AppCommand::NextWorkspace);

        self.bind_chord(Modifiers::COMMAND | Modifiers::ALT, Key::T, AppCommand::CloseOtherTabs);
        self.bind_chord(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::T,
            AppCommand::ReopenLastClosed,
        );
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::M, AppCommand::ToggleCopyMode);
        self.bind_chord(Modifiers::COMMAND | Modifiers::SHIFT, Key::I, AppCommand::PasteImage);

        // Workspace switch COMMAND+1..9 — registered before CTRL+1..9 so on
        // Linux (where both bits are set for Ctrl) workspace wins. On macOS
        // Command and Ctrl are distinct, so both bindings work.
        for (i, key) in NUM_KEYS.iter().enumerate() {
            self.bind_chord(Modifiers::COMMAND, *key, AppCommand::SwitchWorkspace(i));
        }
        // Physical Ctrl+1..9 → select surface (distinct from ⌘ only on macOS;
        // on Linux COMMAND above already consumed the chord as SwitchWorkspace).
        for (i, key) in NUM_KEYS.iter().enumerate() {
            self.bind_chord(Modifiers::CTRL, *key, AppCommand::SelectSurface(i));
        }

        self.bind_chord(Modifiers::COMMAND, Key::Q, AppCommand::Quit);
        // Copy only when a selection exists — otherwise Ctrl+C must reach the
        // PTY as SIGINT (Linux/Windows set both ctrl+command for Ctrl).
        self.bind(
            ShortcutBinding::new(
                KeyboardShortcut::new(Modifiers::COMMAND, Key::C),
                AppCommand::Copy,
            )
            .selection_only(),
        );
        self.bind_chord(Modifiers::COMMAND, Key::V, AppCommand::Paste);

        // Bare Escape / Enter — only when find bar is open, never steal from TextEdit.
        self.bind(
            ShortcutBinding::new(
                KeyboardShortcut::new(Modifiers::NONE, Key::Escape),
                AppCommand::Find,
            )
            .find_only(),
        );
        self.bind(
            ShortcutBinding::new(
                KeyboardShortcut::new(Modifiers::NONE, Key::Enter),
                AppCommand::FindNext,
            )
            .find_only(),
        );
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

const NUM_KEYS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

/// Physical Ctrl + ⌘ — matches only when macOS Command is held with Ctrl.
/// On Linux/Windows `mac_cmd` is never set, so these chords never fire
/// (avoids stealing plain Ctrl bindings).
fn mac_ctrl_cmd() -> Modifiers {
    Modifiers { ctrl: true, command: true, mac_cmd: true, ..Modifiers::NONE }
}

/// Simulate the modifiers egui reports when the user holds the platform
/// primary shortcut key (⌘ on macOS, Ctrl on Linux/Windows).
///
/// On Linux/Windows, egui sets **both** `ctrl` and `command` for Ctrl.
/// Tests should use this helper instead of bare `Modifiers::CTRL`.
#[cfg(test)]
pub fn primary_mod_pressed() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::COMMAND | Modifiers::MAC_CMD
    } else {
        Modifiers { ctrl: true, command: true, ..Modifiers::NONE }
    }
}

/// Subsystem that handles a command (for integration tests / routing docs).
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ActionTarget {
    Workspace,
    Terminal,
    Sidebar,
    Browser,
    Window,
    NoOp,
}

/// Map a command to its subsystem (pure; no UI required).
#[allow(dead_code)]
pub fn action_target(command: AppCommand) -> ActionTarget {
    use AppCommand::*;
    match command {
        NewSurface | NextSurface | PreviousSurface | SelectSurface(_) | CloseTab
        | CloseOtherTabs | ReopenLastClosed | EqualizeSplitsAlt | PrevWorkspaceAlt
        | NextWorkspaceAlt | Quit | Find | FindNext | FindPrev | UseSelectionForFind
        | ClearScrollback | ClearScreen | FontSizeUp | FontSizeDown | FontSizeReset
        | NewWorkspace | SplitRight | SplitDown | ClosePane | OpenBrowserSplit
        | FocusBrowserUrlBar | ReloadBrowser | SwitchWorkspace(_) | CloseWorkspace
        | RenameWorkspace | ToggleZoom | EqualizeSplits | PrevWorkspace | NextWorkspace
        | FocusLeft | FocusRight | FocusUp | FocusDown | ToggleNotifications => {
            ActionTarget::Workspace
        }
        Copy | Paste | ToggleCopyMode | PasteImage => ActionTarget::Terminal,
        ToggleRightSidebar | ToggleSidebar => ActionTarget::Sidebar,
        SplitBrowserRight | SplitBrowserDown => ActionTarget::Browser,
        NewWindow | CloseWindow => ActionTarget::Window,
        RenameTab => ActionTarget::NoOp,
    }
}

// ─── Unit tests (no window / event loop) ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> ShortcutManager {
        ShortcutManager::with_defaults()
    }

    fn cmd() -> Modifiers {
        primary_mod_pressed()
    }

    fn cmd_shift() -> Modifiers {
        let mut m = cmd();
        m.shift = true;
        m
    }

    fn cmd_alt() -> Modifiers {
        let mut m = cmd();
        m.alt = true;
        m
    }

    #[test]
    fn primary_mod_quit() {
        assert_eq!(mgr().resolve(cmd(), Key::Q), Some(AppCommand::Quit));
    }

    #[test]
    fn primary_mod_new_surface() {
        assert_eq!(mgr().resolve(cmd(), Key::T), Some(AppCommand::NewSurface));
    }

    #[test]
    fn primary_mod_split_right() {
        assert_eq!(mgr().resolve(cmd(), Key::D), Some(AppCommand::SplitRight));
    }

    #[test]
    fn primary_mod_shift_split_down() {
        assert_eq!(mgr().resolve(cmd_shift(), Key::D), Some(AppCommand::SplitDown));
    }

    #[test]
    fn primary_mod_close_tab() {
        assert_eq!(mgr().resolve(cmd(), Key::W), Some(AppCommand::CloseTab));
    }

    #[test]
    fn primary_mod_new_workspace() {
        assert_eq!(mgr().resolve(cmd(), Key::N), Some(AppCommand::NewWorkspace));
    }

    #[test]
    fn primary_mod_toggle_sidebar() {
        assert_eq!(mgr().resolve(cmd(), Key::B), Some(AppCommand::ToggleSidebar));
    }

    #[test]
    fn primary_mod_workspace_1() {
        assert_eq!(mgr().resolve(cmd(), Key::Num1), Some(AppCommand::SwitchWorkspace(0)));
    }

    #[test]
    fn primary_mod_workspace_9() {
        assert_eq!(mgr().resolve(cmd(), Key::Num9), Some(AppCommand::SwitchWorkspace(8)));
    }

    #[test]
    fn shift_is_more_specific_than_bare_command() {
        // Cmd+Shift+D → SplitDown, not SplitRight
        assert_eq!(mgr().resolve(cmd_shift(), Key::D), Some(AppCommand::SplitDown));
        assert_eq!(mgr().resolve(cmd(), Key::D), Some(AppCommand::SplitRight));
    }

    #[test]
    fn focus_arrows_require_alt() {
        assert_eq!(mgr().resolve(cmd_alt(), Key::ArrowLeft), Some(AppCommand::FocusLeft));
        assert_eq!(mgr().resolve(cmd(), Key::ArrowLeft), None);
    }

    #[test]
    fn bare_escape_only_when_find_visible() {
        let m = mgr();
        assert_eq!(
            m.resolve_with_options(Modifiers::NONE, Key::Escape, PollOptions::default()),
            None
        );
        assert_eq!(
            m.resolve_with_options(
                Modifiers::NONE,
                Key::Escape,
                PollOptions { find_visible: true, text_focused: false, has_selection: false },
            ),
            Some(AppCommand::Find)
        );
    }

    #[test]
    fn bare_escape_suppressed_when_text_focused() {
        let m = mgr();
        assert_eq!(
            m.resolve_with_options(
                Modifiers::NONE,
                Key::Escape,
                PollOptions { find_visible: true, text_focused: true, has_selection: false },
            ),
            None
        );
    }

    #[test]
    fn command_f_find_even_when_text_focused() {
        let m = mgr();
        assert_eq!(
            m.resolve_with_options(
                cmd(),
                Key::F,
                PollOptions { find_visible: false, text_focused: true, has_selection: false },
            ),
            Some(AppCommand::Find)
        );
    }

    #[test]
    fn unknown_chord_returns_none() {
        assert_eq!(mgr().resolve(Modifiers::NONE, Key::A), None);
    }

    #[test]
    fn copy_only_when_selection_exists() {
        let m = mgr();
        // No selection → Ctrl/⌘+C is not an app shortcut (SIGINT goes to PTY).
        assert_eq!(
            m.resolve_with_options(
                cmd(),
                Key::C,
                PollOptions { has_selection: false, ..Default::default() }
            ),
            None
        );
        assert_eq!(
            m.resolve_with_options(
                cmd(),
                Key::C,
                PollOptions { has_selection: true, ..Default::default() }
            ),
            Some(AppCommand::Copy)
        );
    }

    #[test]
    fn paste_is_always_command_v() {
        assert_eq!(mgr().resolve(cmd(), Key::V), Some(AppCommand::Paste));
    }

    #[test]
    fn custom_bind_extends_manager() {
        let mut m = ShortcutManager::new();
        m.bind_chord(Modifiers::COMMAND, Key::S, AppCommand::Copy);
        assert_eq!(m.resolve(cmd(), Key::S), Some(AppCommand::Copy));
        assert_eq!(m.resolve(cmd(), Key::T), None);
    }

    #[test]
    fn clear_and_rebind_replaces_table() {
        let mut m = ShortcutManager::with_defaults();
        m.clear_and_rebind(vec![ShortcutBinding::new(
            KeyboardShortcut::new(Modifiers::COMMAND, Key::X),
            AppCommand::Quit,
        )]);
        assert_eq!(m.resolve(cmd(), Key::X), Some(AppCommand::Quit));
        assert_eq!(m.resolve(cmd(), Key::Q), None);
    }

    #[test]
    fn action_target_new_surface_is_workspace() {
        assert_eq!(action_target(AppCommand::NewSurface), ActionTarget::Workspace);
    }

    #[test]
    fn action_target_paste_image_is_terminal() {
        assert_eq!(action_target(AppCommand::PasteImage), ActionTarget::Terminal);
    }

    /// Linux regression: primary modifier is reported as both ctrl+command.
    /// Resolving with that combined state must hit COMMAND bindings.
    #[test]
    fn linux_style_ctrl_command_bits_resolve_primary_shortcuts() {
        let pressed = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        let m = mgr();
        assert_eq!(m.resolve(pressed, Key::T), Some(AppCommand::NewSurface));
        assert_eq!(m.resolve(pressed, Key::D), Some(AppCommand::SplitRight));
        assert_eq!(m.resolve(pressed, Key::W), Some(AppCommand::CloseTab));
        assert_eq!(m.resolve(pressed, Key::N), Some(AppCommand::NewWorkspace));
        assert_eq!(m.resolve(pressed, Key::Num1), Some(AppCommand::SwitchWorkspace(0)));
    }

    #[test]
    fn next_previous_surface_shift_brackets() {
        let m = mgr();
        assert_eq!(m.resolve(cmd_shift(), Key::CloseBracket), Some(AppCommand::NextSurface));
        assert_eq!(m.resolve(cmd_shift(), Key::OpenBracket), Some(AppCommand::PreviousSurface));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_physical_ctrl_selects_surface() {
        // Physical Ctrl only (no command) on macOS
        let ctrl = Modifiers::CTRL;
        assert_eq!(mgr().resolve(ctrl, Key::Num1), Some(AppCommand::SelectSurface(0)));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_cmd_and_ctrl_numbers_are_distinct() {
        let m = mgr();
        assert_eq!(m.resolve(Modifiers::COMMAND, Key::Num2), Some(AppCommand::SwitchWorkspace(1)));
        assert_eq!(m.resolve(Modifiers::CTRL, Key::Num2), Some(AppCommand::SelectSurface(1)));
    }
}
