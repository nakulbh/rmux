//! Binary-level tests: clap parse matrix + process exit codes via `assert` on
//! the library entry points used by `main`.
//!
//! Full process spawn is covered for `--help` / `--version` via `Command`.

use std::process::Command as ProcessCommand;

use clap::Parser;
use rmux_cli::commands::Command as CliCommand;

/// Mirror of the binary's clap root so parse tests live next to process tests.
#[derive(Parser, Debug)]
#[command(name = "rmux-cli", version)]
struct Cli {
    #[arg(long, global = true)]
    socket: Option<std::path::PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: CliCommand,
}

fn bin() -> ProcessCommand {
    // `CARGO_BIN_EXE_rmux-cli` is set by cargo for integration tests.
    ProcessCommand::new(env!("CARGO_BIN_EXE_rmux-cli"))
}

#[test]
fn binary_help_exits_zero() {
    let output = bin().arg("--help").output().expect("run --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("system"));
    assert!(stdout.contains("workspace"));
    assert!(stdout.contains("surface"));
    assert!(stdout.contains("browser"));
    assert!(stdout.contains("call"));
    assert!(stdout.contains("events"));
}

#[test]
fn binary_version_exits_zero() {
    let output = bin().arg("--version").output().expect("run --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rmux-cli"));
}

#[test]
fn binary_connect_failure_exits_two() {
    let sock =
        std::env::temp_dir().join(format!("rmux-cli-bin-missing-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let output = bin()
        .args(["--socket", sock.to_str().unwrap(), "system", "ping"])
        .output()
        .expect("run system ping");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot connect") || stderr.contains("is rmux running"));
}

#[test]
fn parse_matrix_covers_all_domains() {
    let samples: &[&[&str]] = &[
        &["rmux-cli", "system", "ping"],
        &["rmux-cli", "system", "capabilities"],
        &["rmux-cli", "system", "identify"],
        &["rmux-cli", "workspace", "list"],
        &["rmux-cli", "workspace", "create", "dev"],
        &["rmux-cli", "workspace", "select", "0"],
        &["rmux-cli", "workspace", "close", "1"],
        &["rmux-cli", "workspace", "rename", "1", "main"],
        &["rmux-cli", "surface", "list"],
        &["rmux-cli", "surface", "split", "right"],
        &["rmux-cli", "surface", "split", "down"],
        &["rmux-cli", "surface", "focus", "3"],
        &["rmux-cli", "surface", "close"],
        &["rmux-cli", "surface", "close", "4"],
        &["rmux-cli", "surface", "new", "--title", "tab"],
        &["rmux-cli", "surface", "send", "ls\\n"],
        &["rmux-cli", "surface", "key", "enter"],
        &["rmux-cli", "notification", "create", "--title", "T", "--body", "B"],
        &["rmux-cli", "notification", "list"],
        &["rmux-cli", "notification", "clear"],
        &["rmux-cli", "sidebar", "status", "set", "busy"],
        &["rmux-cli", "sidebar", "status", "set", "--workspace", "1", "busy"],
        &["rmux-cli", "sidebar", "status", "clear"],
        &["rmux-cli", "sidebar", "progress", "0.5"],
        &["rmux-cli", "browser", "open"],
        &["rmux-cli", "browser", "open", "https://example.com"],
        &["rmux-cli", "browser", "navigate", "https://x.ai"],
        &["rmux-cli", "browser", "back"],
        &["rmux-cli", "browser", "forward"],
        &["rmux-cli", "browser", "reload"],
        &["rmux-cli", "browser", "url"],
        &["rmux-cli", "app", "font-size", "1.0"],
        &["rmux-cli", "app", "font-size", "--reset"],
        &["rmux-cli", "app", "font-size", "-1.0"],
        &["rmux-cli", "app", "theme", "tokyo-night"],
        &["rmux-cli", "events", "stream"],
        &["rmux-cli", "call", "system.ping"],
        &["rmux-cli", "call", "workspace.create", r#"{"name":"x"}"#],
        // aliases
        &["rmux-cli", "ping"],
        &["rmux-cli", "capabilities"],
        &["rmux-cli", "notify", "--title", "hi"],
        &["rmux-cli", "new-workspace", "dev"],
        &["rmux-cli", "list-workspaces", "--json"],
        &["rmux-cli", "new-split", "right"],
        &["rmux-cli", "send", "echo hi\\n"],
        // global flags
        &["rmux-cli", "--json", "workspace", "list"],
        &["rmux-cli", "--socket", "/tmp/x.sock", "system", "ping"],
        &["rmux-cli", "workspace", "list", "--socket", "/tmp/y.sock"],
    ];

    for args in samples {
        let cli = Cli::try_parse_from(*args);
        assert!(cli.is_ok(), "failed to parse {args:?}: {cli:?}");
    }
}

#[test]
fn parse_rejects_unknown_command() {
    let err = Cli::try_parse_from(["rmux-cli", "does-not-exist"]).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unrecognized") || msg.contains("unexpected") || msg.contains("error"));
}

#[test]
fn parse_rejects_invalid_split_direction() {
    let err = Cli::try_parse_from(["rmux-cli", "surface", "split", "diagonal"]).unwrap_err();
    assert!(!err.to_string().is_empty());
}
