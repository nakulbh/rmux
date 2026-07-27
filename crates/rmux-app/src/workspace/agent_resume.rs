//! Agent session resume — cmux-style relaunch of AI coding agents.
//!
//! On session save, rmux inspects each terminal's foreground process. When a
//! known agent CLI is running, it builds the agent's **native resume command**
//! (e.g. `claude --resume <id>`, `codex resume <id>`) and stores it on the
//! surface snapshot. On restore, that command is typed into the fresh shell so
//! the agent session reappears instead of leaving an empty prompt.
//!
//! Session IDs are taken from the process argv when present. When the agent is
//! running without a resume id on the command line, we fall back to the
//! agent-specific "continue last session" form (e.g. `claude --continue`).
//!
//! Mirrors the command shapes documented by cmux session restore / AgentResumeArgv.

/// Result of detecting a resumable agent in a process command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResume {
    /// Short agent kind (`claude`, `codex`, …).
    pub kind: &'static str,
    /// Shell command to type into the restored terminal (no trailing newline).
    pub command: String,
}

/// Build a resume command from a process `args` string (`ps` args column).
///
/// Returns `None` when the process is not a known agent.
pub fn resume_from_process_args(args: &str) -> Option<AgentResume> {
    let tokens = tokenize_args(args);
    if tokens.is_empty() {
        return None;
    }
    let (kind, exe_idx) = detect_agent(&tokens)?;
    let tail: Vec<&str> = tokens[exe_idx + 1..].to_vec();
    let session_id = extract_session_id(kind, &tokens[exe_idx..]);
    let command = build_resume_command(kind, session_id.as_deref(), &tail)?;
    Some(AgentResume { kind, command })
}

/// Known agent binary basenames → kind identifier.
fn detect_agent(tokens: &[&str]) -> Option<(&'static str, usize)> {
    for (i, tok) in tokens.iter().enumerate() {
        // Skip env VAR=val prefixes.
        if tok.contains('=') && !tok.starts_with('-') {
            continue;
        }
        let base = basename(tok);
        let kind = match base {
            "claude" => "claude",
            "codex" => "codex",
            "grok" => "grok",
            "opencode" => "opencode",
            "pi" => "pi",
            "omp" => "omp",
            "campfire" => "campfire",
            "amp" => "amp",
            "cursor-agent" | "cursor" => "cursor",
            "gemini" => "gemini",
            "agy" | "antigravity" => "antigravity",
            "acli" => "rovodev",
            "hermes" => "hermes-agent",
            "copilot" => "copilot",
            "codebuddy" => "codebuddy",
            "droid" => "factory",
            "qodercli" | "qoder" => "qoder",
            "kimi" => "kimi",
            "kiro-cli" | "kiro" => "kiro",
            _ => continue,
        };
        return Some((kind, i));
    }
    None
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Extract a session/thread id from argv for the given agent kind.
fn extract_session_id(kind: &str, tokens: &[&str]) -> Option<String> {
    match kind {
        "codex" => extract_after_token(tokens, "resume")
            .or_else(|| extract_flag(tokens, &["--resume", "-r"])),
        "claude" | "cursor" | "gemini" | "copilot" | "codebuddy" | "factory" | "qoder" | "kimi" => {
            extract_flag(tokens, &["--resume", "-r"])
        }
        "grok" => extract_flag(tokens, &["-r", "--resume"]),
        "opencode" | "pi" | "omp" | "campfire" => extract_flag(tokens, &["--session", "-s"]),
        "antigravity" => extract_flag(tokens, &["--conversation", "--session", "--resume"]),
        "amp" => {
            // `amp threads continue <id>`
            if let Some(i) = tokens.iter().position(|t| *t == "continue") {
                tokens.get(i + 1).map(|s| (*s).to_owned())
            } else {
                extract_flag(tokens, &["--session", "--resume"])
            }
        }
        "rovodev" => extract_flag(tokens, &["--restore", "--session", "--resume"]),
        "hermes-agent" => extract_flag(tokens, &["--resume", "--session"]),
        "kiro" => extract_flag(tokens, &["--resume-id", "--resume", "--session"]),
        _ => None,
    }
    .filter(|id| !id.is_empty() && !id.starts_with('-'))
}

fn extract_flag(tokens: &[&str], flags: &[&str]) -> Option<String> {
    for (i, t) in tokens.iter().enumerate() {
        for flag in flags {
            if *t == *flag {
                return tokens.get(i + 1).map(|s| (*s).to_owned());
            }
            let prefix = format!("{flag}=");
            if let Some(rest) = t.strip_prefix(&prefix)
                && !rest.is_empty()
            {
                return Some(rest.to_owned());
            }
        }
    }
    None
}

fn extract_after_token(tokens: &[&str], token: &str) -> Option<String> {
    tokens.iter().position(|t| *t == token).and_then(|i| tokens.get(i + 1).map(|s| (*s).to_owned()))
}

/// Build the shell command that resumes the agent (cmux AgentResumeArgv shapes).
fn build_resume_command(kind: &str, session_id: Option<&str>, _tail: &[&str]) -> Option<String> {
    match (kind, session_id) {
        ("claude", Some(id)) => Some(format!("claude --resume {}", shell_quote(id))),
        ("claude", None) => Some("claude --continue".to_owned()),

        ("codex", Some(id)) => {
            // Suppress blocking "Update available!" on bare resume (cmux).
            Some(format!("codex resume {} -c check_for_update_on_startup=false", shell_quote(id)))
        }
        ("codex", None) => Some("codex resume -c check_for_update_on_startup=false".to_owned()),

        ("grok", Some(id)) => Some(format!("grok -r {}", shell_quote(id))),
        ("grok", None) => Some("grok".to_owned()),

        ("opencode", Some(id)) => Some(format!("opencode --session {}", shell_quote(id))),
        ("opencode", None) => Some("opencode".to_owned()),

        ("pi", Some(id)) => Some(format!("pi --session {}", shell_quote(id))),
        ("pi", None) => Some("pi".to_owned()),

        ("omp", Some(id)) => Some(format!("omp --session {}", shell_quote(id))),
        ("omp", None) => Some("omp".to_owned()),

        ("campfire", Some(id)) => Some(format!("campfire --session {}", shell_quote(id))),
        ("campfire", None) => Some("campfire".to_owned()),

        ("amp", Some(id)) => Some(format!("amp threads continue {}", shell_quote(id))),
        ("amp", None) => Some("amp".to_owned()),

        ("cursor", Some(id)) => Some(format!("cursor-agent --resume {}", shell_quote(id))),
        ("cursor", None) => Some("cursor-agent".to_owned()),

        ("gemini", Some(id)) => Some(format!("gemini --resume {}", shell_quote(id))),
        ("gemini", None) => Some("gemini".to_owned()),

        ("antigravity", Some(id)) => Some(format!("agy --conversation {}", shell_quote(id))),
        ("antigravity", None) => Some("agy".to_owned()),

        ("rovodev", Some(id)) => Some(format!("acli rovodev run --restore {}", shell_quote(id))),
        ("rovodev", None) => Some("acli rovodev run".to_owned()),

        ("hermes-agent", Some(id)) => Some(format!("hermes --resume {}", shell_quote(id))),
        ("hermes-agent", None) => Some("hermes".to_owned()),

        ("copilot", Some(id)) => Some(format!("copilot --resume {}", shell_quote(id))),
        ("copilot", None) => Some("copilot".to_owned()),

        ("codebuddy", Some(id)) => Some(format!("codebuddy --resume {}", shell_quote(id))),
        ("codebuddy", None) => Some("codebuddy".to_owned()),

        ("factory", Some(id)) => Some(format!("droid --resume {}", shell_quote(id))),
        ("factory", None) => Some("droid".to_owned()),

        ("qoder", Some(id)) => Some(format!("qodercli --resume {}", shell_quote(id))),
        ("qoder", None) => Some("qodercli".to_owned()),

        ("kimi", Some(id)) => Some(format!("kimi --resume {}", shell_quote(id))),
        ("kimi", None) => Some("kimi".to_owned()),

        ("kiro", Some(id)) => Some(format!("kiro-cli chat --resume-id {}", shell_quote(id))),
        ("kiro", None) => Some("kiro-cli chat".to_owned()),

        _ => None,
    }
}

/// Minimal whitespace tokenizer (good enough for `ps` args).
fn tokenize_args(args: &str) -> Vec<&str> {
    args.split_whitespace().collect()
}

/// Quote a token for POSIX shells when it needs escaping.
fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_owned();
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/')) {
        return s.to_owned();
    }
    // Single-quote, escaping embedded quotes as '\''
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_with_resume_id() {
        let r = resume_from_process_args("claude --resume abc-123 --dangerously-skip-permissions")
            .expect("claude");
        assert_eq!(r.kind, "claude");
        assert_eq!(r.command, "claude --resume abc-123");
    }

    #[test]
    fn test_claude_without_id_uses_continue() {
        let r = resume_from_process_args("claude --dangerously-skip-permissions").expect("claude");
        assert_eq!(r.kind, "claude");
        assert_eq!(r.command, "claude --continue");
    }

    #[test]
    fn test_codex_resume() {
        let r = resume_from_process_args("codex resume sess-9").expect("codex");
        assert_eq!(r.kind, "codex");
        assert!(r.command.contains("codex resume sess-9"));
        assert!(r.command.contains("check_for_update_on_startup=false"));
    }

    #[test]
    fn test_opencode_session() {
        let r = resume_from_process_args("opencode --session xyz").expect("opencode");
        assert_eq!(r.command, "opencode --session xyz");
    }

    #[test]
    fn test_grok_short_flag() {
        let r = resume_from_process_args("grok -r my-id").expect("grok");
        assert_eq!(r.command, "grok -r my-id");
    }

    #[test]
    fn test_non_agent_returns_none() {
        assert!(resume_from_process_args("nvim src/main.rs").is_none());
        assert!(resume_from_process_args("cargo test").is_none());
        assert!(resume_from_process_args("").is_none());
    }

    #[test]
    fn test_path_prefixed_binary() {
        let r = resume_from_process_args("/usr/local/bin/claude --resume sid").expect("claude");
        assert_eq!(r.kind, "claude");
        assert_eq!(r.command, "claude --resume sid");
    }

    #[test]
    fn test_shell_quote_special() {
        assert_eq!(shell_quote("plain"), "plain");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert!(shell_quote("it's").contains('\\'));
    }

    #[test]
    fn test_amp_threads_continue() {
        let r = resume_from_process_args("amp threads continue thr-1").expect("amp");
        assert_eq!(r.command, "amp threads continue thr-1");
    }

    #[test]
    fn test_equals_form_flag() {
        let r = resume_from_process_args("claude --resume=uuid-here").expect("claude");
        assert_eq!(r.command, "claude --resume uuid-here");
    }
}
