//! Back-compat façade over [`crate::shortcut_manager`].
//!
//! New code should use [`crate::shortcut_manager`] directly.

#![allow(dead_code)]

#[cfg(test)]
pub use crate::shortcut_manager::{ActionTarget, action_target};
pub use crate::shortcut_manager::{AppCommand, ShortcutManager};

/// Historical name for [`AppCommand`].
pub type ShortcutAction = AppCommand;

/// Thin registry wrapper for tests and legacy call sites.
///
/// Internally delegates to [`ShortcutManager`] with logical modifier matching
/// (same semantics as `consume_shortcut`).
#[derive(Debug, Clone, Default)]
pub struct ShortcutRegistry {
    manager: ShortcutManager,
}

impl ShortcutRegistry {
    /// Look up a command for pressed modifiers + key.
    pub fn lookup(&self, modifiers: egui::Modifiers, key: egui::Key) -> Option<ShortcutAction> {
        self.manager.resolve(modifiers, key)
    }
}

/// Canonical app-shortcut modifier: always [`egui::Modifiers::COMMAND`].
///
/// egui maps this to ⌘ on macOS and Ctrl on Linux/Windows.
pub(crate) fn cmd_ctrl() -> egui::Modifiers {
    egui::Modifiers::COMMAND
}

pub(crate) fn cmd_ctrl_shift() -> egui::Modifiers {
    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
}

pub(crate) fn cmd_ctrl_alt() -> egui::Modifiers {
    egui::Modifiers::COMMAND | egui::Modifiers::ALT
}

pub(crate) fn cmd_alt() -> egui::Modifiers {
    egui::Modifiers::COMMAND | egui::Modifiers::ALT
}

pub(crate) fn cmd_alt_shift() -> egui::Modifiers {
    egui::Modifiers::COMMAND | egui::Modifiers::ALT | egui::Modifiers::SHIFT
}

pub(crate) fn ctrl_only() -> egui::Modifiers {
    egui::Modifiers::CTRL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shortcut_manager::primary_mod_pressed;
    use egui::{Key, Modifiers};

    #[test]
    fn registry_default_quit() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(primary_mod_pressed(), Key::Q), Some(ShortcutAction::Quit));
    }

    #[test]
    fn registry_unknown() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(Modifiers::NONE, Key::A), None);
    }

    #[test]
    fn registry_new_surface() {
        let reg = ShortcutRegistry::default();
        assert_eq!(reg.lookup(primary_mod_pressed(), Key::T), Some(ShortcutAction::NewSurface));
    }

    #[test]
    fn registry_switch_workspace() {
        let reg = ShortcutRegistry::default();
        assert_eq!(
            reg.lookup(primary_mod_pressed(), Key::Num3),
            Some(ShortcutAction::SwitchWorkspace(2))
        );
    }

    #[test]
    fn registry_focus_arrows() {
        let reg = ShortcutRegistry::default();
        let mut mods = primary_mod_pressed();
        mods.alt = true;
        assert_eq!(reg.lookup(mods, Key::ArrowLeft), Some(ShortcutAction::FocusLeft));
        assert_eq!(reg.lookup(mods, Key::ArrowRight), Some(ShortcutAction::FocusRight));
        assert_eq!(reg.lookup(mods, Key::ArrowUp), Some(ShortcutAction::FocusUp));
        assert_eq!(reg.lookup(mods, Key::ArrowDown), Some(ShortcutAction::FocusDown));
    }

    #[test]
    fn linux_ctrl_bits_match_command_bindings() {
        let reg = ShortcutRegistry::default();
        let linux_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        assert_eq!(reg.lookup(linux_ctrl, Key::T), Some(ShortcutAction::NewSurface));
        assert_eq!(reg.lookup(linux_ctrl, Key::D), Some(ShortcutAction::SplitRight));
        assert_eq!(reg.lookup(linux_ctrl, Key::W), Some(ShortcutAction::CloseTab));
    }

    #[test]
    fn action_target_wired() {
        assert_eq!(action_target(ShortcutAction::NewSurface), ActionTarget::Workspace);
        assert_eq!(action_target(ShortcutAction::PasteImage), ActionTarget::Terminal);
    }
}
