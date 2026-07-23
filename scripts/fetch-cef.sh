#!/usr/bin/env bash
# fetch-cef.sh — download a pinned CEF build for local Chromium development.
#
# Phase E1 of docs/CHROMIUM_BROWSER_PLAN.md.
#
# Usage:
#   ./scripts/fetch-cef.sh              # download for this host
#   ./scripts/fetch-cef.sh --print-env  # print env exports only
#   CEF_VERSION=... ./scripts/fetch-cef.sh
#
# After fetch:
#   export CEF_PATH="$(pwd)/third_party/cef/current"
#   # then (later E1): cargo run -p rmux-app --no-default-features --features browser-chromium
#
# Notes:
# - CEF archives are large (hundreds of MB). Not committed to git.
# - Default download uses Spotify's CEF automated builds CDN.
# - When integrating tauri-apps/cef-rs, you may instead use:
#     cargo run -p export-cef-dir -- --force "$HOME/.local/share/cef"
#   and set CEF_PATH accordingly.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Pin with `cef` crate in workspace Cargo.toml (must match cef-rs ABI).
# cef 150.2.1+150.0.14 → CEF binary 150.0.14+…
CEF_VERSION="${CEF_VERSION:-150.0.14+g7c1aa68+chromium-150.0.7871.129}"
DEST_BASE="${CEF_DEST:-$ROOT/third_party/cef}"
CURRENT_LINK="$DEST_BASE/current"

detect_platform() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "macosarm64" ;;
        x86_64) echo "macosx64" ;;
        *) echo "unsupported macOS arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64|amd64) echo "linux64" ;;
        aarch64|arm64) echo "linuxarm64" ;;
        *) echo "unsupported Linux arch: $arch" >&2; exit 1 ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*)
      echo "windows64"
      ;;
    *)
      echo "unsupported OS: $os (use scripts/fetch-cef.ps1 on Windows)" >&2
      exit 1
      ;;
  esac
}

print_env() {
  local path="${1:-$CURRENT_LINK}"
  # Prefer download-cef layout produced by cef-dll-sys (has archive.json).
  local nested
  nested="$(find "$path" -maxdepth 3 -type d -name 'cef_*' 2>/dev/null | head -1 || true)"
  if [[ -n "$nested" && -f "$nested/archive.json" ]]; then
    path="$nested"
  fi
  echo "export CEF_PATH=\"$path\""
  echo "export PATH=\"/opt/homebrew/bin:\${PATH}\"  # cmake/ninja for cef-dll-sys on macOS"
  case "$(uname -s)" in
    Darwin)
      echo "export DYLD_FALLBACK_LIBRARY_PATH=\"\${DYLD_FALLBACK_LIBRARY_PATH:-}:\$CEF_PATH:\$CEF_PATH/Chromium Embedded Framework.framework/Libraries\""
      ;;
    Linux)
      echo "export LD_LIBRARY_PATH=\"\${LD_LIBRARY_PATH:-}:\$CEF_PATH\""
      ;;
  esac
}

if [[ "${1:-}" == "--print-env" ]]; then
  print_env
  exit 0
fi

PLATFORM="$(detect_platform)"
# Standard CEF minimal distribution naming (Spotify CDN).
# Format may need adjustment when locking to a specific cef-rs revision.
ARCHIVE_NAME="cef_binary_${CEF_VERSION}_${PLATFORM}_minimal"
# Spotify CDN: `+` in version must be percent-encoded.
CEF_VERSION_ENC="${CEF_VERSION//+/%2B}"
ARCHIVE_NAME_ENC="cef_binary_${CEF_VERSION_ENC}_${PLATFORM}_minimal"
# Spotify CDN pattern (see https://cef-builds.spotifycdn.com/index.html)
URL="https://cef-builds.spotifycdn.com/${ARCHIVE_NAME_ENC}.tar.bz2"

OUT_DIR="$DEST_BASE/${CEF_VERSION}_${PLATFORM}"
ARCHIVE_PATH="$DEST_BASE/${ARCHIVE_NAME}.tar.bz2"

mkdir -p "$DEST_BASE"

if [[ -d "$OUT_DIR" && -n "$(ls -A "$OUT_DIR" 2>/dev/null || true)" ]]; then
  echo "CEF already present at $OUT_DIR"
  ln -sfn "$OUT_DIR" "$CURRENT_LINK"
  print_env
  exit 0
fi

echo "Platform:  $PLATFORM"
echo "Version:   $CEF_VERSION"
echo "URL:       $URL"
echo "Dest:      $OUT_DIR"
echo
echo "WARNING: download is large (often 100–300+ MB compressed)."
echo

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 1
fi

# Probe URL (CDN layout changes; fail with a helpful message).
if ! curl -fsI "$URL" >/dev/null 2>&1; then
  cat <<EOF >&2
ERROR: CEF archive not found at:
  $URL

Next steps:
  1. Open https://cef-builds.spotifycdn.com/index.html
  2. Pick a minimal build for ${PLATFORM}
  3. Re-run with:
       CEF_VERSION='<version-from-cdn>' ./scripts/fetch-cef.sh
  4. Or use tauri-apps/cef-rs helper (when linked in Cargo):
       cargo run -p export-cef-dir -- --force "\$HOME/.local/share/cef"
       export CEF_PATH="\$HOME/.local/share/cef"

See docs/CHROMIUM_BROWSER_PLAN.md § E1.
EOF
  exit 1
fi

echo "Downloading..."
curl -fL --progress-bar -o "$ARCHIVE_PATH" "$URL"

echo "Extracting..."
mkdir -p "$OUT_DIR"
tar -xjf "$ARCHIVE_PATH" -C "$OUT_DIR" --strip-components=1
rm -f "$ARCHIVE_PATH"

ln -sfn "$OUT_DIR" "$CURRENT_LINK"

echo
echo "CEF ready."
print_env
echo
echo "Eval the exports in your shell, then build with:"
echo "  cargo run -p rmux-app --no-default-features --features browser-chromium"
