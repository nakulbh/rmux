#!/usr/bin/env bash
# install.sh — install rmux on macOS or Linux
#
# Usage (recommended):
#   curl -fsSL https://raw.githubusercontent.com/nakulbh/rmux/main/scripts/install.sh | bash
#
# Options (env vars):
#   RMUX_INSTALL_DIR   Binary install directory (default: ~/.local/bin)
#   RMUX_PREFIX        Desktop integration prefix (default: ~/.local)
#   RMUX_VERSION       Git ref to install (default: main)
#   RMUX_REPO          Git clone URL (default: https://github.com/nakulbh/rmux.git)
#   RMUX_SKIP_DESKTOP  Set to 1 to skip .app / .desktop + icon install
#
# Requires: curl, git, a C toolchain, and either cargo or the ability to install
# rustup. On Linux you also need the usual GUI/WebKit build deps (see README).

set -euo pipefail

RMUX_REPO="${RMUX_REPO:-https://github.com/nakulbh/rmux.git}"
RMUX_VERSION="${RMUX_VERSION:-main}"
RMUX_INSTALL_DIR="${RMUX_INSTALL_DIR:-${HOME}/.local/bin}"
RMUX_PREFIX="${RMUX_PREFIX:-${HOME}/.local}"
RMUX_SKIP_DESKTOP="${RMUX_SKIP_DESKTOP:-0}"
RAW_BASE="https://raw.githubusercontent.com/nakulbh/rmux/${RMUX_VERSION}"

# ── helpers ──────────────────────────────────────────────────────────────────

info()  { printf '==> %s\n' "$*"; }
warn()  { printf 'warning: %s\n' "$*" >&2; }
die()   { printf 'error: %s\n' "$*" >&2; exit 1; }
have()  { command -v "$1" >/dev/null 2>&1; }

need_cmd() {
  have "$1" || die "required command not found: $1"
}

download() {
  # download <url> <dest>
  local url="$1" dest="$2"
  if have curl; then
    curl -fsSL "$url" -o "$dest"
  elif have wget; then
    wget -qO "$dest" "$url"
  else
    die "need curl or wget to download files"
  fi
}

os_name() {
  case "$(uname -s)" in
    Darwin) echo macos ;;
    Linux)  echo linux ;;
    *)      die "unsupported OS: $(uname -s). This installer supports macOS and Linux only." ;;
  esac
}

# ── preflight ────────────────────────────────────────────────────────────────

OS="$(os_name)"
info "Installing rmux for ${OS} (ref: ${RMUX_VERSION})"

need_cmd git
need_cmd uname
need_cmd mktemp

# ── Rust toolchain ───────────────────────────────────────────────────────────

ensure_rust() {
  if have cargo && have rustc; then
    info "Found cargo $(cargo --version | awk '{print $2}')"
    return
  fi

  info "Rust toolchain not found — installing via rustup (default profile)"
  need_cmd curl
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default
  # shellcheck disable=SC1091
  if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck source=/dev/null
    source "${HOME}/.cargo/env"
  fi
  have cargo || die "rustup finished but cargo is still not on PATH"
}

ensure_rust

# ── Linux system packages (best-effort) ──────────────────────────────────────

install_linux_build_deps() {
  [[ "$OS" == linux ]] || return 0

  local pkgs=(
    build-essential pkg-config
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
    libxkbcommon-dev libssl-dev
    libgtk-3-dev libglib2.0-dev libatk1.0-dev
    libcairo2-dev libpango1.0-dev
    libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev
    libsoup-3.0-dev libgdk-pixbuf-2.0-dev
  )

  if have apt-get && have sudo; then
    info "Installing Linux build dependencies (apt)"
    sudo apt-get update -qq
    # shellcheck disable=SC2068
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq ${pkgs[@]} || \
      warn "some apt packages failed to install; build may still succeed"
  elif have dnf && have sudo; then
    info "Installing Linux build dependencies (dnf)"
    sudo dnf install -y gcc pkgconf-pkg-config openssl-devel \
      gtk3-devel webkit2gtk4.1-devel || \
      warn "some dnf packages failed to install; build may still succeed"
  elif have pacman && have sudo; then
    info "Installing Linux build dependencies (pacman)"
    sudo pacman -S --needed --noconfirm base-devel openssl gtk3 webkit2gtk-4.1 || \
      warn "some pacman packages failed to install; build may still succeed"
  else
    warn "could not detect a package manager; ensure GUI/WebKit dev libs are installed"
  fi
}

install_linux_build_deps

# ── build from source ────────────────────────────────────────────────────────

WORKDIR="$(mktemp -d -t rmux-install.XXXXXX)"
cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

info "Cloning ${RMUX_REPO} @ ${RMUX_VERSION}"
git clone --depth 1 --branch "${RMUX_VERSION}" "${RMUX_REPO}" "${WORKDIR}/rmux" \
  || git clone --depth 1 "${RMUX_REPO}" "${WORKDIR}/rmux"

cd "${WORKDIR}/rmux"
# If the requested branch/tag didn't exist, we cloned default — try checkout.
if git rev-parse --verify "refs/remotes/origin/${RMUX_VERSION}" >/dev/null 2>&1; then
  git checkout -q "${RMUX_VERSION}" || true
elif git rev-parse --verify "${RMUX_VERSION}" >/dev/null 2>&1; then
  git checkout -q "${RMUX_VERSION}" || true
fi

info "Building rmux (release) — this may take a few minutes"
cargo build --release -p rmux-app --bin rmux

mkdir -p "${RMUX_INSTALL_DIR}"
install -m 755 target/release/rmux "${RMUX_INSTALL_DIR}/rmux"
info "Installed binary → ${RMUX_INSTALL_DIR}/rmux"

# Ensure PATH hint
case ":${PATH}:" in
  *":${RMUX_INSTALL_DIR}:"*) ;;
  *)
    warn "${RMUX_INSTALL_DIR} is not on your PATH"
    printf '    Add this to your shell profile:\n'
    printf '      export PATH="%s:$PATH"\n' "${RMUX_INSTALL_DIR}"
    ;;
esac

# ── official logo + desktop integration ──────────────────────────────────────

install_logo_and_desktop() {
  [[ "${RMUX_SKIP_DESKTOP}" == "1" ]] && { info "Skipping desktop integration (RMUX_SKIP_DESKTOP=1)"; return; }

  local logo_src="${WORKDIR}/rmux/rmux_logo.jpg"
  local logo_png="${WORKDIR}/rmux/assets/icons/rmux_logo.png"
  if [[ ! -f "$logo_png" ]]; then
    logo_png="${WORKDIR}/rmux/assets/rmux_logo.png"
  fi
  if [[ ! -f "$logo_png" && -f "$logo_src" ]] && have sips; then
    sips -s format png "$logo_src" --out "${WORKDIR}/rmux_logo.png" >/dev/null
    logo_png="${WORKDIR}/rmux_logo.png"
  fi
  if [[ ! -f "$logo_png" ]]; then
    # Fallback: fetch from GitHub raw
    logo_png="${WORKDIR}/rmux_logo.png"
    download "${RAW_BASE}/assets/icons/rmux_logo.png" "$logo_png" 2>/dev/null \
      || download "${RAW_BASE}/rmux_logo.jpg" "${WORKDIR}/rmux_logo.jpg" || true
    if [[ -f "${WORKDIR}/rmux_logo.jpg" && ! -f "$logo_png" ]] && have sips; then
      sips -s format png "${WORKDIR}/rmux_logo.jpg" --out "$logo_png" >/dev/null
    fi
  fi
  [[ -f "$logo_png" ]] || { warn "could not locate official rmux logo; skipping icon install"; return; }

  if [[ "$OS" == macos ]]; then
    install_macos_app "$logo_png"
  else
    install_linux_desktop "$logo_png"
  fi
}

install_macos_app() {
  local logo_png="$1"
  local app_root="${HOME}/Applications/rmux.app"
  local contents="${app_root}/Contents"
  local macos_dir="${contents}/MacOS"
  local resources="${contents}/Resources"

  info "Creating macOS app bundle with official logo → ${app_root}"
  rm -rf "${app_root}"
  mkdir -p "${macos_dir}" "${resources}"

  # Prefer the just-built binary; fall back to the installed copy.
  if [[ -x "${WORKDIR}/rmux/target/release/rmux" ]]; then
    install -m 755 "${WORKDIR}/rmux/target/release/rmux" "${macos_dir}/rmux"
  else
    install -m 755 "${RMUX_INSTALL_DIR}/rmux" "${macos_dir}/rmux"
  fi

  # Build .icns from the official logo (sips + iconutil ship with macOS).
  if have sips && have iconutil; then
    local iconset="${WORKDIR}/rmux.iconset"
    mkdir -p "$iconset"
    for size in 16 32 64 128 256 512; do
      sips -z "$size" "$size" "$logo_png" --out "${iconset}/icon_${size}x${size}.png" >/dev/null
      # Retina @2x
      local two=$((size * 2))
      if [[ $two -le 1024 ]]; then
        sips -z "$two" "$two" "$logo_png" --out "${iconset}/icon_${size}x${size}@2x.png" >/dev/null
      fi
    done
    # Required naming variants for iconutil
    cp "${iconset}/icon_32x32.png" "${iconset}/icon_16x16@2x.png" 2>/dev/null || true
    iconutil -c icns "$iconset" -o "${resources}/AppIcon.icns" 2>/dev/null \
      || warn "iconutil failed; app will use the runtime-embedded icon"
  else
    # Store PNG as a fallback resource
    cp "$logo_png" "${resources}/rmux.png"
  fi

  cat > "${contents}/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>rmux</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>com.nakulbh.rmux</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>rmux</string>
  <key>CFBundleDisplayName</key>
  <string>rmux</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
</dict>
</plist>
PLIST

  # Touch so Launch Services refreshes the icon cache.
  touch "${app_root}"
  if have open; then
    /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
      -f "${app_root}" 2>/dev/null || true
  fi

  info "macOS app installed. Open with: open ~/Applications/rmux.app"
}

install_linux_desktop() {
  local logo_png="$1"
  local icons_base="${RMUX_PREFIX}/share/icons/hicolor"
  local apps_dir="${RMUX_PREFIX}/share/applications"

  info "Installing Linux desktop entry + official logo icons"

  mkdir -p "${apps_dir}"
  for size in 32 64 128 256 512; do
    local dir="${icons_base}/${size}x${size}/apps"
    mkdir -p "$dir"
    if have convert; then
      convert "$logo_png" -resize "${size}x${size}" "${dir}/rmux.png"
    elif have magick; then
      magick "$logo_png" -resize "${size}x${size}" "${dir}/rmux.png"
    elif have ffmpeg; then
      ffmpeg -y -i "$logo_png" -vf "scale=${size}:${size}" "${dir}/rmux.png" >/dev/null 2>&1 || \
        cp "$logo_png" "${dir}/rmux.png"
    else
      # Install full-res PNG at every size; desktop environments still pick it up.
      cp "$logo_png" "${dir}/rmux.png"
    fi
  done
  # Scalable fallback
  mkdir -p "${icons_base}/scalable/apps"
  cp "$logo_png" "${icons_base}/scalable/apps/rmux.png"

  cat > "${apps_dir}/rmux.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=rmux
GenericName=Terminal Multiplexer
Comment=Cross-platform, memory-efficient terminal multiplexer GUI
Exec=${RMUX_INSTALL_DIR}/rmux
Icon=rmux
Terminal=false
Categories=System;TerminalEmulator;Utility;
Keywords=terminal;multiplexer;tmux;shell;
StartupNotify=true
StartupWMClass=rmux
DESKTOP
  chmod 644 "${apps_dir}/rmux.desktop"

  if have update-desktop-database; then
    update-desktop-database "${apps_dir}" 2>/dev/null || true
  fi
  if have gtk-update-icon-cache; then
    gtk-update-icon-cache -f -t "${icons_base}" 2>/dev/null || true
  fi

  info "Desktop entry → ${apps_dir}/rmux.desktop"
  info "Icons → ${icons_base}/*/apps/rmux.png"
}

install_logo_and_desktop

# ── done ─────────────────────────────────────────────────────────────────────

cat <<EOF

rmux installed successfully.

  Binary:  ${RMUX_INSTALL_DIR}/rmux
  Version: $( "${RMUX_INSTALL_DIR}/rmux" --version 2>/dev/null || echo "${RMUX_VERSION}" )

Run:
  rmux

EOF

if [[ "$OS" == macos ]]; then
  cat <<EOF
macOS app (Dock / Launchpad):
  open ~/Applications/rmux.app

EOF
fi
