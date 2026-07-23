# fetch-cef.ps1 — Windows CEF fetch helper (Phase E1).
# See scripts/fetch-cef.sh and docs/CHROMIUM_BROWSER_PLAN.md.

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$CefVersion = if ($env:CEF_VERSION) { $env:CEF_VERSION } else { "137.0.17+gf354b0e+chromium-137.0.7151.104" }
$DestBase = if ($env:CEF_DEST) { $env:CEF_DEST } else { Join-Path $Root "third_party\cef" }
$Platform = "windows64"
$ArchiveName = "cef_binary_${CefVersion}_${Platform}_minimal"
$Url = "https://cef-builds.spotifycdn.com/${ArchiveName}.tar.bz2"
$OutDir = Join-Path $DestBase "${CefVersion}_${Platform}"
$ArchivePath = Join-Path $DestBase "${ArchiveName}.tar.bz2"
$Current = Join-Path $DestBase "current"

New-Item -ItemType Directory -Force -Path $DestBase | Out-Null

if (Test-Path $OutDir) {
    Write-Host "CEF already present at $OutDir"
} else {
    Write-Host "Downloading $Url ..."
    try {
        Invoke-WebRequest -Uri $Url -OutFile $ArchivePath
    } catch {
        Write-Error @"
CEF archive not found or download failed: $Url
Open https://cef-builds.spotifycdn.com/index.html and set CEF_VERSION, or use:
  cargo run -p export-cef-dir -- --force `$env:USERPROFILE/.local/share/cef
See docs/CHROMIUM_BROWSER_PLAN.md
"@
    }
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    # Requires tar (Windows 10+).
    tar -xjf $ArchivePath -C $OutDir --strip-components=1
    Remove-Item $ArchivePath -Force
}

if (Test-Path $Current) { Remove-Item $Current -Recurse -Force -ErrorAction SilentlyContinue }
cmd /c mklink /J "$Current" "$OutDir" | Out-Null

Write-Host "`$env:CEF_PATH = '$Current'"
Write-Host "`$env:PATH = `"`$env:PATH;$Current`""
Write-Host "Then: cargo run -p rmux-app --no-default-features --features browser-chromium"
