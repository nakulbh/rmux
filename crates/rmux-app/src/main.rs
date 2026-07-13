#![forbid(unsafe_code)]
#![allow(unknown_lints)]
#![allow(ambiguous_float_literals)]
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
mod browser;
mod notifications;
mod shortcut_handler;
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

/// Monospace font candidates, in preference order. Used for terminal text
/// (`egui::FontFamily::Monospace`).
const MONOSPACE_FONT_CANDIDATES: &[&str] = &[
    // macOS: SF Mono (used by Terminal.app/iTerm2 by default on modern macOS)
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Monaco.ttf",
    // Windows
    "C:\\Windows\\Fonts\\consola.ttf",
    "C:\\Windows\\Fonts\\cascadiamono.ttf",
    // Linux (common distro font paths)
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
];

/// UI sans-serif font candidates, in preference order. Used for regular UI
/// text (`egui::FontFamily::Proportional`) — sidebar labels, buttons, etc.
const UI_SANS_FONT_CANDIDATES: &[&str] = &[
    // macOS: San Francisco (system UI font)
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    // Windows
    "C:\\Windows\\Fonts\\segoeui.ttf",
    // Linux
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
];

/// Load custom fonts for the UI and terminal.
///
/// egui's bundled default fonts ("Hack" for monospace, "Ubuntu-Light" for
/// proportional text) render noticeably heavier/blockier than the native
/// fonts they're meant to stand in for — egui does *not* pull platform
/// fonts automatically. We look for real system fonts (SF Mono/SF on
/// macOS, Consolas/Segoe UI on Windows, DejaVu/Liberation on Linux) and,
/// if found, install them as the first choice in the `Monospace` and
/// `Proportional` families so terminal text and the rest of the UI render
/// crisply like a native app. Falls back to egui's bundled fonts for
/// whichever family has no candidate present on disk.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut changed = false;

    if let Some(bytes) = load_first_readable(MONOSPACE_FONT_CANDIDATES) {
        fonts.font_data.insert(
            "SystemMono".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "SystemMono".to_owned());
        changed = true;
    }

    if let Some(bytes) = load_first_readable(UI_SANS_FONT_CANDIDATES) {
        fonts.font_data.insert(
            "SystemSans".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "SystemSans".to_owned());
        changed = true;
    }

    if changed {
        ctx.set_fonts(fonts);
    }
}

/// Return the bytes of the first path in `candidates` that exists and
/// reads successfully. `.ttc` font collections (e.g. `Helvetica.ttc`) work
/// here because `ab_glyph`/egui transparently reads the first face of a
/// TrueType collection.
fn load_first_readable(candidates: &[&str]) -> Option<Vec<u8>> {
    candidates.iter().find_map(|path| std::fs::read(path).ok())
}
