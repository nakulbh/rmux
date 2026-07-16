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

/// JetBrains Mono — same primary terminal face Ghostty/cmux embed by default.
/// OFL-1.1; see `assets/fonts/JETBRAINS_MONO_LICENSE.txt`.
const JETBRAINS_MONO_REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");

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

/// Icon glyphs (Private Use Area code points) used by nvim-web-devicons,
/// lazy.nvim, starship/oh-my-posh, k9s, lsd/exa, and most modern CLI tool
/// UIs. Neither JetBrains Mono nor egui's bundled "Hack" contain these —
/// without a fallback they render as tofu boxes. This is the "Mono" variant
/// (single-width glyphs, sized to sit cleanly in one terminal cell) from the
/// Nerd Fonts symbols-only release, MIT licensed (see
/// `assets/fonts/NERD_FONTS_LICENSE.txt`).
const NERD_FONT_SYMBOLS: &[u8] = include_bytes!("../assets/fonts/SymbolsNerdFontMono-Regular.ttf");

/// Load custom fonts for the UI and terminal.
///
/// Matches Ghostty/cmux defaults as closely as egui allows:
/// 1. **JetBrains Mono** as the primary monospace face (bundled) — crisp
///    coding font with consistent cell metrics for TUIs like LazyVim.
/// 2. **Symbols Nerd Font Mono** as fallback for PUA icons (devicons, powerline).
/// 3. System sans for proportional UI chrome.
///
/// We deliberately do **not** prefer macOS SF Mono for the terminal: it is a
/// multi-named / variable face that ab_glyph can resolve poorly, and it lacks
/// the metrics Ghostty tunes around JetBrains Mono + Nerd symbols.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "JetBrainsMono".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(JETBRAINS_MONO_REGULAR)),
    );
    fonts.font_data.insert(
        "JetBrainsMonoBold".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(JETBRAINS_MONO_BOLD)),
    );
    fonts.font_data.insert(
        "NerdFontSymbols".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(NERD_FONT_SYMBOLS)),
    );

    // Monospace: JetBrains Mono first, then Nerd symbols for missing codepoints.
    // Bold face is registered under its own family name for the renderer.
    {
        let mono = fonts.families.entry(egui::FontFamily::Monospace).or_default();
        mono.clear();
        mono.push("JetBrainsMono".to_owned());
        mono.push("NerdFontSymbols".to_owned());
        // Keep egui's built-in Hack as last resort for rare coverage gaps.
        mono.push("Hack".to_owned());
    }
    fonts.families.insert(
        egui::FontFamily::Name("JetBrainsMonoBold".into()),
        vec!["JetBrainsMonoBold".to_owned(), "NerdFontSymbols".to_owned()],
    );

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
    }

    // Nerd symbols also on Proportional so UI chrome icons resolve.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("NerdFontSymbols".to_owned());

    ctx.set_fonts(fonts);
}

/// Return the bytes of the first path in `candidates` that exists and
/// reads successfully. `.ttc` font collections (e.g. `Helvetica.ttc`) work
/// here because `ab_glyph`/egui transparently reads the first face of a
/// TrueType collection.
fn load_first_readable(candidates: &[&str]) -> Option<Vec<u8>> {
    candidates.iter().find_map(|path| std::fs::read(path).ok())
}
