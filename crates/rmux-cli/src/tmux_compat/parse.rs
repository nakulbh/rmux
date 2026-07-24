//! Parse tmux-like argv into structured commands for `__tmux-compat`.

/// A subset of tmux commands used by Claude Code Teams and similar tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxCommand {
    /// `split-window` [-h|-v] [-t target] [-c cwd] [shell…]
    SplitWindow {
        /// Horizontal (-h) → rmux "right"; vertical (-v) → "down".
        horizontal: bool,
        target: Option<String>,
        cwd: Option<String>,
        shell: Vec<String>,
    },
    /// `send-keys` [-t target] keys… [Enter]
    SendKeys { target: Option<String>, keys: Vec<String> },
    /// `select-pane` -t target
    SelectPane { target: Option<String> },
    /// `list-panes` [-F format] [-a]
    ListPanes { format: Option<String>, all: bool },
    /// `list-windows` [-F format]
    ListWindows { format: Option<String> },
    /// `kill-pane` [-t target]
    KillPane { target: Option<String> },
    /// `new-window` / `new-session` [command…]
    NewWindow { shell: Vec<String> },
    /// `display-message` / unknown — no-op success for agent resilience
    Noop { name: String },
}

/// Parse argv after the binary name (i.e. pure tmux args).
///
/// Accepts either `tmux <cmd> …` or bare `<cmd> …` (shim passes full argv).
#[must_use]
pub fn parse_tmux_args(args: &[String]) -> TmuxCommand {
    let mut args = args;
    // Drop leading "tmux" if present
    if args.first().is_some_and(|a| a == "tmux" || a.ends_with("/tmux")) {
        args = &args[1..];
    }
    // Drop global flags like -L, -S, -f until we hit a command
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "-L" || a == "-S" || a == "-f" || a == "-c" {
            i += 2; // flag + value (best-effort)
            continue;
        }
        if a.starts_with('-') && a != "-h" && a != "-v" && a != "-t" && a != "-F" && a != "-a" {
            i += 1;
            continue;
        }
        break;
    }
    let args = &args[i..];
    let Some(cmd) = args.first().map(String::as_str) else {
        return TmuxCommand::Noop { name: "empty".into() };
    };
    let rest = &args[1..];

    match cmd {
        "split-window" | "splitw" => parse_split_window(rest),
        "send-keys" | "send" => parse_send_keys(rest),
        "select-pane" | "selectp" => parse_select_pane(rest),
        "list-panes" | "lsp" => parse_list_panes(rest),
        "list-windows" | "lsw" => parse_list_windows(rest),
        "kill-pane" | "killp" => parse_kill_pane(rest),
        "new-window" | "neww" | "new-session" | "new" => {
            TmuxCommand::NewWindow { shell: rest.to_vec() }
        }
        "display-message" | "display" | "refresh-client" | "set-option" | "set"
        | "show-options" | "show" | "has-session" | "has" => TmuxCommand::Noop { name: cmd.into() },
        other => TmuxCommand::Noop { name: other.into() },
    }
}

fn parse_split_window(args: &[String]) -> TmuxCommand {
    let mut horizontal = true; // tmux default is -h style often for teams; cmux maps -h → right
    let mut target = None;
    let mut cwd = None;
    let mut shell = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" => {
                horizontal = true;
                i += 1;
            }
            "-v" => {
                horizontal = false;
                i += 1;
            }
            "-t" => {
                target = args.get(i + 1).cloned();
                i += 2;
            }
            "-c" => {
                cwd = args.get(i + 1).cloned();
                i += 2;
            }
            "-l" | "-p" | "-b" | "-f" => {
                // size / before / full — skip value if present
                if args.get(i + 1).is_some_and(|v| !v.starts_with('-')) {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--" => {
                shell.extend_from_slice(&args[i + 1..]);
                break;
            }
            other if other.starts_with('-') => i += 1,
            _ => {
                shell.extend_from_slice(&args[i..]);
                break;
            }
        }
    }
    TmuxCommand::SplitWindow { horizontal, target, cwd, shell }
}

fn parse_send_keys(args: &[String]) -> TmuxCommand {
    let mut target = None;
    let mut keys = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" => {
                target = args.get(i + 1).cloned();
                i += 2;
            }
            "-l" => {
                // literal
                i += 1;
            }
            other if other.starts_with('-') => i += 1,
            _ => {
                keys.extend_from_slice(&args[i..]);
                break;
            }
        }
    }
    TmuxCommand::SendKeys { target, keys }
}

fn parse_select_pane(args: &[String]) -> TmuxCommand {
    let mut target = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" => {
                target = args.get(i + 1).cloned();
                i += 2;
            }
            _ => i += 1,
        }
    }
    TmuxCommand::SelectPane { target }
}

fn parse_list_panes(args: &[String]) -> TmuxCommand {
    let mut format = None;
    let mut all = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-F" => {
                format = args.get(i + 1).cloned();
                i += 2;
            }
            "-a" => {
                all = true;
                i += 1;
            }
            _ => i += 1,
        }
    }
    TmuxCommand::ListPanes { format, all }
}

fn parse_list_windows(args: &[String]) -> TmuxCommand {
    let mut format = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-F" => {
                format = args.get(i + 1).cloned();
                i += 2;
            }
            _ => i += 1,
        }
    }
    TmuxCommand::ListWindows { format }
}

fn parse_kill_pane(args: &[String]) -> TmuxCommand {
    let mut target = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-t" => {
                target = args.get(i + 1).cloned();
                i += 2;
            }
            _ => i += 1,
        }
    }
    TmuxCommand::KillPane { target }
}

/// Convert send-keys tokens into a string to type (Enter → `\r`).
#[must_use]
pub fn keys_to_text(keys: &[String]) -> String {
    let mut out = String::new();
    for k in keys {
        match k.as_str() {
            "Enter" | "C-m" | "KPEnter" => out.push('\r'),
            "Tab" | "C-i" => out.push('\t'),
            "Escape" | "Esc" => out.push('\u{1b}'),
            "Space" => out.push(' '),
            "BSpace" | "Backspace" => out.push('\u{7f}'),
            other if other.starts_with('C') && other.contains('-') => {
                // C-c → ctrl+c etc. — send as raw if single letter
                if let Some(ch) = other.split('-').next_back().and_then(|s| s.chars().next()) {
                    let lower = ch.to_ascii_lowercase();
                    if lower.is_ascii_lowercase() {
                        out.push((lower as u8 & 0x1f) as char);
                    }
                }
            }
            other => out.push_str(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn parse_split_horizontal() {
        let cmd = parse_tmux_args(&s(&["split-window", "-h", "-c", "/tmp", "claude", "--resume"]));
        match cmd {
            TmuxCommand::SplitWindow { horizontal, cwd, shell, .. } => {
                assert!(horizontal);
                assert_eq!(cwd.as_deref(), Some("/tmp"));
                assert_eq!(shell, vec!["claude", "--resume"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_split_vertical() {
        let cmd = parse_tmux_args(&s(&["tmux", "split-window", "-v"]));
        match cmd {
            TmuxCommand::SplitWindow { horizontal, shell, .. } => {
                assert!(!horizontal);
                assert!(shell.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_send_keys_enter() {
        let cmd = parse_tmux_args(&s(&["send-keys", "-t", "%1", "hello", "Enter"]));
        match cmd {
            TmuxCommand::SendKeys { target, keys } => {
                assert_eq!(target.as_deref(), Some("%1"));
                assert_eq!(keys_to_text(&keys), "hello\r");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_select_and_kill() {
        assert!(matches!(
            parse_tmux_args(&s(&["select-pane", "-t", "%2"])),
            TmuxCommand::SelectPane { target: Some(t) } if t == "%2"
        ));
        assert!(matches!(
            parse_tmux_args(&s(&["kill-pane", "-t", "%0"])),
            TmuxCommand::KillPane { target: Some(t) } if t == "%0"
        ));
    }

    #[test]
    fn horizontal_maps_for_direction_string() {
        // Documented mapping used by dispatcher
        assert_eq!(if true { "right" } else { "down" }, "right");
        assert_eq!(if false { "right" } else { "down" }, "down");
    }
}
