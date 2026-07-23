//! Process-wide CEF runtime (initialize once, external message pump).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use tracing::info;

static RUNTIME_READY: AtomicBool = AtomicBool::new(false);
static INIT_ERROR: OnceLock<String> = OnceLock::new();

/// True after macOS framework was loaded via `cef_load_library`.
#[cfg(target_os = "macos")]
static FRAMEWORK_LOADED: AtomicBool = AtomicBool::new(false);

/// Resolve CEF install root from env, download-cef layout, or CWD.
#[must_use]
pub fn cef_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CEF_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return prefer_download_cef_layout(path);
        }
    }
    let manifest_rel = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("third_party")
        .join("cef")
        .join("current");
    if let Ok(canon) = manifest_rel.canonicalize()
        && canon.exists()
    {
        return prefer_download_cef_layout(canon);
    }
    let local = PathBuf::from("third_party/cef/current");
    if local.exists() {
        return prefer_download_cef_layout(local);
    }
    None
}

/// Prefer nested `download-cef` / cef-dll-sys layout (`…/150.x.y/cef_<platform>`)
/// which contains `archive.json` + the framework at the root.
fn prefer_download_cef_layout(root: PathBuf) -> Option<PathBuf> {
    if root.join("archive.json").exists()
        && framework_binary(&root).map(|p| p.exists()).unwrap_or(false)
    {
        return Some(root);
    }
    // Walk one–three levels for nested download-cef dirs.
    let candidates = std::fs::read_dir(&root).ok()?.flatten().filter_map(|e| {
        let p = e.path();
        if p.is_dir() { Some(p) } else { None }
    });
    for child in candidates {
        if child.join("archive.json").exists() {
            return Some(child);
        }
        if let Ok(entries) = std::fs::read_dir(&child) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() && p.join("archive.json").exists() {
                    return Some(p);
                }
            }
        }
    }
    // Fall back to root if it at least has the framework (Spotify minimal layout).
    if framework_binary(&root).map(|p| p.exists()).unwrap_or(false) {
        return Some(root);
    }
    Some(root)
}

/// Path to the loadable CEF library binary inside a CEF root.
fn framework_binary(cef_root: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(
            cef_root
                .join("Chromium Embedded Framework.framework")
                .join("Chromium Embedded Framework"),
        )
    }
    #[cfg(target_os = "linux")]
    {
        Some(cef_root.join("libcef.so"))
    }
    #[cfg(target_os = "windows")]
    {
        Some(cef_root.join("libcef.dll"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = cef_root;
        None
    }
}

/// User-data / profile directory for Chromium cookies & storage.
#[must_use]
pub fn profile_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("rmux").join("chromium");
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("rmux")
                .join("chromium");
        }
        #[cfg(not(target_os = "macos"))]
        {
            return PathBuf::from(home).join(".config").join("rmux").join("chromium");
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("rmux").join("chromium");
    }
    PathBuf::from(".rmux-chromium")
}

/// Whether CEF has been successfully initialized in this process.
#[must_use]
#[allow(dead_code)]
pub fn is_ready() -> bool {
    RUNTIME_READY.load(Ordering::Acquire)
}

/// Initialize CEF once on the UI thread. Idempotent.
pub fn ensure_runtime() -> Result<()> {
    if RUNTIME_READY.load(Ordering::Acquire) {
        return Ok(());
    }
    if let Some(err) = INIT_ERROR.get() {
        bail!("{err}");
    }

    match init_cef_once() {
        Ok(()) => {
            RUNTIME_READY.store(true, Ordering::Release);
            Ok(())
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = INIT_ERROR.set(msg.clone());
            Err(e)
        }
    }
}

fn init_cef_once() -> Result<()> {
    let cef = cef_path().context(
        "CEF_PATH missing — run ./scripts/fetch-cef.sh and eval \"$(./scripts/fetch-cef.sh --print-env)\"",
    )?;

    // Symlink Frameworks next to cargo target so CEF/helpers can resolve the
    // standard macOS layout without a full .app bundle (fixes icudtl.dat).
    ensure_dev_frameworks_symlink(&cef)?;

    // Ensure helpers re-launched by CEF can resolve the same tree.
    // SAFETY: single-threaded init before multi-process spawn; value is a path.
    unsafe {
        std::env::set_var("CEF_PATH", &cef);
    }
    #[cfg(target_os = "macos")]
    {
        let libs = cef
            .join("Chromium Embedded Framework.framework")
            .join("Libraries");
        if libs.exists() {
            let cur = std::env::var("DYLD_FALLBACK_LIBRARY_PATH").unwrap_or_default();
            let merged = if cur.is_empty() {
                format!("{}:{}", cef.display(), libs.display())
            } else {
                format!("{cur}:{}:{}", cef.display(), libs.display())
            };
            unsafe {
                std::env::set_var("DYLD_FALLBACK_LIBRARY_PATH", merged);
            }
        }
    }

    load_framework(&cef).with_context(|| {
        format!(
            "Failed to load CEF framework from {}. \
             export CEF_PATH to the download-cef directory (contains archive.json + framework).",
            cef.display()
        )
    })?;

    // Validate API hash (must match linked libcef).
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    let args = cef::args::Args::new();
    let mut app = super::handlers::AppBuilder::build(super::handlers::OsrApp::new());

    let root_cache = profile_dir();
    std::fs::create_dir_all(&root_cache)
        .with_context(|| format!("create CEF profile dir {}", root_cache.display()))?;

    let framework_dir = cef.join("Chromium Embedded Framework.framework");
    let resources = resource_path_for(&cef);
    let locales = locales_path_for(&cef);

    // Verify ICU data is where CEF expects it before initialize (fail with a
    // clear error instead of SIGTRAP from Chromium CHECK).
    if let Some(ref res) = resources {
        let icu = res.join("icudtl.dat");
        if !icu.exists() {
            bail!(
                "icudtl.dat missing at {}. CEF Resources are incomplete — re-run ./scripts/fetch-cef.sh",
                icu.display()
            );
        }
    }

    let mut settings = cef::Settings {
        windowless_rendering_enabled: true as _,
        external_message_pump: true as _,
        no_sandbox: true as _,
        // Avoid Chromium installing handlers that can turn CHECKs into hard traps
        // during early resource discovery failures.
        disable_signal_handlers: true as _,
        log_severity: cef::LogSeverity::from(cef::sys::cef_log_severity_t::LOGSEVERITY_WARNING),
        ..Default::default()
    };

    settings.cache_path = cef::CefString::from(root_cache.to_string_lossy().as_ref());
    settings.root_cache_path = cef::CefString::from(root_cache.to_string_lossy().as_ref());

    // macOS: without framework_dir_path CEF looks for Resources inside the
    // *host app* bundle (the bare cargo binary) → "icudtl.dat not found".
    #[cfg(target_os = "macos")]
    {
        if framework_dir.exists() {
            settings.framework_dir_path =
                cef::CefString::from(framework_dir.to_string_lossy().as_ref());
        }
        // Point main_bundle_path at the framework so NSBundle-style lookups
        // resolve Resources/icudtl.dat for a non-.app cargo binary.
        settings.main_bundle_path = cef::CefString::from(framework_dir.to_string_lossy().as_ref());
    }

    if let Some(ref r) = resources {
        settings.resources_dir_path = cef::CefString::from(r.to_string_lossy().as_ref());
    }
    if let Some(ref l) = locales {
        settings.locales_dir_path = cef::CefString::from(l.to_string_lossy().as_ref());
    }

    // With `--single-process` we do not rely on helper app bundles. Keep
    // subprocess path empty so CEF does not spawn bare GPU/network helpers
    // that crash without Helper.app packaging (see command-line switches).
    // Production E4 can set browser_subprocess_path + helper bundles.

    info!(
        framework = %framework_dir.display(),
        resources = ?resources.as_ref().map(|p| p.display().to_string()),
        "CEF settings ready — calling initialize"
    );

    let rc = cef::initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if rc != 1 {
        bail!("cef::initialize failed (code {rc}). Check CEF_PATH and binary ABI match (150.x).");
    }

    info!(
        cef = %cef.display(),
        profile = %root_cache.display(),
        "CEF runtime initialized (windowless + external message pump)"
    );
    Ok(())
}

/// Create `target/Frameworks/Chromium Embedded Framework.framework` → CEF framework
/// so CEF path resolution (and `LibraryLoader` layout) works for `cargo run`.
///
/// Layout expected by CEF relative to `target/debug/rmux`:
/// `../Frameworks/Chromium Embedded Framework.framework/...`
fn ensure_dev_frameworks_symlink(cef_root: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let framework_src = cef_root.join("Chromium Embedded Framework.framework");
        if !framework_src.exists() {
            bail!(
                "CEF framework missing at {} (set CEF_PATH correctly)",
                framework_src.display()
            );
        }
        let Ok(exe) = std::env::current_exe() else {
            return Ok(());
        };
        let Some(exe_dir) = exe.parent() else {
            return Ok(());
        };
        // target/debug → target/Frameworks
        let Some(target_dir) = exe_dir.parent() else {
            return Ok(());
        };
        let frameworks_dir = target_dir.join("Frameworks");
        std::fs::create_dir_all(&frameworks_dir)
            .with_context(|| format!("create {}", frameworks_dir.display()))?;

        let link = frameworks_dir.join("Chromium Embedded Framework.framework");
        if link.exists() || link.symlink_metadata().is_ok() {
            // Already present (symlink or real) — leave alone if it resolves.
            if link.exists() {
                return Ok(());
            }
            // Broken symlink — replace.
            let _ = std::fs::remove_file(&link);
        }

        let src = framework_src
            .canonicalize()
            .with_context(|| format!("canonicalize {}", framework_src.display()))?;
        std::os::unix::fs::symlink(&src, &link).with_context(|| {
            format!("symlink {} → {}", link.display(), src.display())
        })?;
        info!(
            link = %link.display(),
            target = %src.display(),
            "Created dev Frameworks symlink for CEF resources"
        );
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cef_root;
        Ok(())
    }
}

/// Load the CEF shared library from `CEF_PATH` without requiring an app-bundle layout.
///
/// `cef::library_loader::LibraryLoader` hard-codes `../Frameworks/...` relative to the
/// executable and panics on missing paths — unusable for `cargo run`. We call
/// `cef_load_library` with an absolute framework path instead.
fn load_framework(cef_root: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if FRAMEWORK_LOADED.load(Ordering::Acquire) {
            return Ok(());
        }
        let framework = framework_binary(cef_root)
            .context("platform has no framework path")?;
        if !framework.exists() {
            bail!(
                "Chromium Embedded Framework not found at {}. \
                 Set CEF_PATH to the directory that contains \
                 'Chromium Embedded Framework.framework'.",
                framework.display()
            );
        }
        use std::os::unix::ffi::OsStrExt;
        let c_path = std::ffi::CString::new(framework.as_os_str().as_bytes())
            .context("framework path contains interior NUL")?;
        // SAFETY: path is a valid C string pointing at the CEF framework binary.
        let rc = unsafe { cef::load_library(Some(&*c_path.as_ptr().cast())) };
        if rc != 1 {
            bail!(
                "cef_load_library returned {rc} for {}. \
                 Check architecture match (arm64 vs x86_64) and DYLD_FALLBACK_LIBRARY_PATH.",
                framework.display()
            );
        }
        FRAMEWORK_LOADED.store(true, Ordering::Release);
        info!(framework = %framework.display(), "Loaded CEF framework via cef_load_library");
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux/Windows typically link/load libcef from LD_LIBRARY_PATH / PATH.
        let _ = cef_root;
        Ok(())
    }
}

fn resource_path_for(cef: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let p = cef.join("Chromium Embedded Framework.framework").join("Resources");
        if p.exists() {
            return Some(p);
        }
    }
    if cef.join("Resources").exists() {
        return Some(cef.join("Resources"));
    }
    if cef.join("resources.pak").exists() || cef.join("chrome_100_percent.pak").exists() {
        return Some(cef.to_path_buf());
    }
    Some(cef.to_path_buf())
}

fn locales_path_for(cef: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let p = cef.join("Chromium Embedded Framework.framework").join("Resources");
        if p.exists() {
            return Some(p);
        }
    }
    let locales = cef.join("locales");
    if locales.exists() {
        return Some(locales);
    }
    None
}

/// Reentrancy guard — CEF single-process mode can re-enter the run loop from
/// inside `do_message_loop_work`, which panics winit ("event while handling event").
static PUMPING: AtomicBool = AtomicBool::new(false);

/// Pump CEF tasks — call once per egui frame when Chromium is enabled.
pub fn pump_message_loop() {
    if !RUNTIME_READY.load(Ordering::Acquire) {
        return;
    }
    if PUMPING.swap(true, Ordering::AcqRel) {
        return;
    }
    // Never let a CEF panic take down the whole terminal UI.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cef::do_message_loop_work();
    }));
    PUMPING.store(false, Ordering::Release);
}

/// True when argv indicates a CEF helper (`--type=renderer`, GPU, etc.).
fn is_cef_helper_process() -> bool {
    std::env::args().skip(1).any(|a| a.starts_with("--type="))
}

/// CEF helper process gate — call from `main` before eframe.
///
/// Returns `true` if this process was a CEF subprocess and should exit.
///
/// **Important:** must not call `LibraryLoader::new` on the browser process —
/// that API expects a macOS `.app` Frameworks layout and panics under `cargo run`.
#[must_use]
pub fn try_run_cef_subprocess() -> bool {
    // Fast path: normal UI process never touches CEF here.
    if !is_cef_helper_process() {
        return false;
    }

    let Some(cef) = cef_path() else {
        eprintln!("rmux: CEF helper process started but CEF_PATH is missing");
        return true;
    };
    if let Err(e) = load_framework(&cef) {
        eprintln!("rmux: failed to load CEF framework for helper: {e:#}");
        return true;
    }
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    let args = cef::args::Args::new();
    let mut app = super::handlers::AppBuilder::build(super::handlers::OsrApp::new());
    let ret = cef::execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    // Helper finished (ret >= 0) or failed — either way do not start the GUI.
    if ret < 0 {
        eprintln!("rmux: cef execute_process returned {ret} for helper process");
    }
    true
}

/// Shut down CEF (best-effort; called on process exit if needed).
#[allow(dead_code)]
pub fn shutdown() {
    if RUNTIME_READY.swap(false, Ordering::AcqRel) {
        cef::shutdown();
    }
    #[cfg(target_os = "macos")]
    if FRAMEWORK_LOADED.swap(false, Ordering::AcqRel) {
        let _ = cef::unload_library();
    }
}
