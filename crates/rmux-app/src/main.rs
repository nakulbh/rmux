#![forbid(unsafe_code)]
//! rmux — A cross-platform, memory-efficient terminal multiplexer GUI.
//!
//! # Architecture
//!
//! ```text
//! rmux-app (binary) — Main application with egui window and event loop
//!   ├── rmux-terminal (library) — Terminal emulation (alacritty_terminal + portable-pty)
//!   ├── rmux-api (library) — Socket server (JSON-RPC protocol)
//!   └── rmux-config (library) — Configuration management
//! ```
//!
//! The main entry point parses CLI arguments, initializes logging,
//! creates the egui/eframe window, and runs the main event loop.

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod api_dispatch;
mod app;
mod notifications;
mod shortcuts;
mod ui;
mod workspace;

/// CLI arguments for the rmux terminal multiplexer.
#[derive(Parser, Debug)]
#[command(name = "rmux", version, about = "A cross-platform terminal multiplexer GUI", long_about = None)]
struct Cli {
    /// Enable verbose logging (debug level)
    #[arg(short, long)]
    verbose: bool,

    /// Path to a config file
    #[arg(short, long)]
    config: Option<String>,

    /// Path to a session file to restore
    #[arg(short, long)]
    session: Option<String>,
}

/// Initialize the tracing subscriber for structured logging.
///
/// Sets up `tracing-subscriber` with `env-filter` support so log levels
/// can be controlled via the `RUST_LOG` environment variable.
/// Falls back to `info` level by default, or `debug` if `--verbose` is passed.
fn init_logging(verbose: bool) {
    let default_level = if verbose { "debug" } else { "info" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    tracing::info!("rmux starting (version {})", env!("CARGO_PKG_VERSION"));

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "rmux",
        native_options,
        Box::new(|cc| {
            // Load custom font for terminal rendering
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(app::RmuxApp::new(cc)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run application: {e}"))?;

    Ok(())
}

/// Load custom fonts for terminal rendering.
///
/// egui's default `FontDefinitions` already includes platform-appropriate
/// monospace fonts (Menlo on macOS, Consolas on Windows, etc.). We use
/// the defaults rather than pushing specific font names that may not be installed.
fn setup_fonts(_ctx: &egui::Context) {
    // No-op: use egui's default font definitions.
}
