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
mod shortcut_manager;
mod shortcuts;
mod ui;
mod update;
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

/// Official rmux logo (256×256 PNG) — used as the window / dock / taskbar icon.
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/rmux_logo.png");

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let sha = env!("RMUX_GIT_SHA");
    if sha.is_empty() {
        tracing::info!("rmux starting (version {})", env!("CARGO_PKG_VERSION"));
    } else {
        tracing::info!("rmux starting (version {} @ {})", env!("CARGO_PKG_VERSION"), sha);
    }

    let mut viewport =
        egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]).with_title("rmux");
    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let native_options = eframe::NativeOptions { viewport, ..Default::default() };

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

/// Decode the bundled official logo into an `egui::IconData` for the OS window.
fn load_app_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(APP_ICON_PNG)
        .map_err(|err| tracing::warn!(error = %err, "failed to decode app icon"))
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData { rgba: image.into_raw(), width, height })
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

/// System symbol / dingbat fonts for geometric shapes, branch glyphs, ballot
/// boxes, arrows, etc. that JetBrains Mono + Nerd PUA do not cover.
///
/// Without this cascade, characters like ⎇ (U+2387), ◌, many Geometric Shapes
/// (U+25A0–), and Misc Symbols render as hollow □ tofu — we kept patching
/// those one-by-one. Loading a broad symbol face is the general fix.
const SYMBOL_FONT_CANDIDATES: &[&str] = &[
    // macOS — full geometric + technical symbol coverage
    "/System/Library/Fonts/Apple Symbols.ttf",
    "/System/Library/Fonts/Supplemental/Apple Symbols.ttf",
    // Windows
    "C:\\Windows\\Fonts\\seguisym.ttf", // Segoe UI Symbol
    "C:\\Windows\\Fonts\\segmdl2.ttf",  // Segoe MDL2 Assets
    // Linux
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
    "/usr/share/fonts/OTF/NotoSansSymbols2-Regular.otf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", // decent symbol coverage
    "/usr/share/fonts/truetype/ancient-scripts/Symbola_hint.ttf",
    "/usr/share/fonts/truetype/symbola/Symbola.ttf",
];

/// Load custom fonts for the UI and terminal.
///
/// Matches Ghostty/cmux defaults as closely as egui allows:
/// 1. **JetBrains Mono** as the primary monospace face (bundled) — crisp
///    coding font with consistent cell metrics for TUIs like LazyVim.
/// 2. **Symbols Nerd Font Mono** as fallback for PUA icons (devicons, powerline).
/// 3. **System symbol font** (Apple Symbols / Segoe UI Symbol / Noto) for
///    Geometric Shapes, arrows, ballot boxes — general anti-tofu cascade.
/// 4. System sans for proportional UI chrome.
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

    let has_system_symbols = if let Some(bytes) = load_first_readable(SYMBOL_FONT_CANDIDATES) {
        fonts.font_data.insert(
            "SystemSymbols".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        true
    } else {
        tracing::warn!(
            "no system symbol font found; uncommon glyphs may still tofu. \
             install Noto Sans Symbols 2 or use Apple/Windows system fonts"
        );
        false
    };

    // Monospace cascade (egui walks the list for each missing codepoint):
    // JetBrains → Nerd PUA → SystemSymbols → Hack.
    {
        let mono = fonts.families.entry(egui::FontFamily::Monospace).or_default();
        mono.clear();
        mono.push("JetBrainsMono".to_owned());
        mono.push("NerdFontSymbols".to_owned());
        if has_system_symbols {
            mono.push("SystemSymbols".to_owned());
        }
        mono.push("Hack".to_owned());
    }
    {
        let mut bold = vec!["JetBrainsMonoBold".to_owned(), "NerdFontSymbols".to_owned()];
        if has_system_symbols {
            bold.push("SystemSymbols".to_owned());
        }
        fonts.families.insert(egui::FontFamily::Name("JetBrainsMonoBold".into()), bold);
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
    }

    // Proportional: system sans → nerd PUA → system symbols (sidebar/icons).
    {
        let prop = fonts.families.entry(egui::FontFamily::Proportional).or_default();
        prop.push("NerdFontSymbols".to_owned());
        if has_system_symbols {
            prop.push("SystemSymbols".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

/// Return the bytes of the first path in `candidates` that exists and
/// reads successfully. `.ttc` font collections (e.g. `Helvetica.ttc`) work
/// here because `ab_glyph`/egui transparently reads the first face of a
/// TrueType collection.
fn load_first_readable(candidates: &[&str]) -> Option<Vec<u8>> {
    candidates.iter().find_map(|path| std::fs::read(path).ok())
}
