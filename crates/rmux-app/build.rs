//! Embed build metadata for update checks and diagnostics.
//!
//! - `RMUX_GIT_SHA` — short commit hash at build time (empty if unavailable)
//! - `RMUX_GIT_DIRTY` — `"true"` if the working tree had local changes

use std::process::Command;

fn main() {
    // Re-run when git HEAD moves so release builds pick up the new SHA.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");

    let sha = git_stdout(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_default();
    let dirty =
        git_stdout(&["status", "--porcelain"]).map(|s| !s.trim().is_empty()).unwrap_or(false);

    println!("cargo:rustc-env=RMUX_GIT_SHA={sha}");
    println!("cargo:rustc-env=RMUX_GIT_DIRTY={}", if dirty { "true" } else { "false" });
}

fn git_stdout(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
}
