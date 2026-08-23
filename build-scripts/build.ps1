<#
.SYNOPSIS
  CrossTerm build script — Windows.

.DESCRIPTION
  Builds a native Windows desktop bundle (.msi/.exe). Tauri bundles OS-
  specific WebView/signing/installer tooling, so a real macOS .app/.dmg or
  Linux .deb/.AppImage can't be produced from a Windows host, or vice versa
  — see .github/workflows/release.yml's build matrix, which builds on
  ubuntu-latest/macos-latest/windows-latest for exactly this reason.
  Android is the one exception: it cross-compiles from any host with the
  Android SDK/NDK installed, so this same script can also produce an
  Android APK/AAB from Windows.

  On macOS/Linux, use build-scripts/build.sh instead.

.PARAMETER Target
  'desktop' (default) or 'android'.

.PARAMETER Aab
  When -Target android, build a Play Store .aab instead of a .apk.

.EXAMPLE
  ./build-scripts/build.ps1
.EXAMPLE
  ./build-scripts/build.ps1 -Target android
.EXAMPLE
  ./build-scripts/build.ps1 -Target android -Aab
#>
param(
    [ValidateSet('desktop', 'android')]
    [string]$Target = 'desktop',
    [switch]$Aab
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "npm not found — install Node.js first"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found — install Rust first (https://rustup.rs)"
}

Write-Host "==> Installing npm dependencies..."
npm ci
if ($LASTEXITCODE -ne 0) { throw "npm ci failed" }

switch ($Target) {
    'desktop' {
        Write-Host "==> Building CrossTerm desktop bundle for Windows..."
        npx tauri build
        if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }
        Write-Host "==> Done. Bundles: target/release/bundle/"
    }

    'android' {
        if (-not (Get-Command java -ErrorAction SilentlyContinue)) {
            throw "java not found — Android builds need a JDK"
        }
        if (-not $env:ANDROID_HOME) {
            Write-Warning "`$env:ANDROID_HOME not set — the Android SDK may not be found."
        }
        if (-not $env:NDK_HOME) {
            Write-Warning "`$env:NDK_HOME not set — the Android NDK may not be found."
        }

        if (-not (Test-Path "src-tauri/gen/android")) {
            Write-Host "==> Initializing Android project (first run only)..."
            npx tauri android init
            if ($LASTEXITCODE -ne 0) { throw "tauri android init failed" }
        }

        if ($Aab) {
            Write-Host "==> Building CrossTerm Android App Bundle (.aab)..."
            npx tauri android build --aab
            if ($LASTEXITCODE -ne 0) { throw "tauri android build --aab failed" }
            Write-Host "==> Done. Output: src-tauri/gen/android/app/build/outputs/bundle/"
        } else {
            Write-Host "==> Building CrossTerm Android APK..."
            npx tauri android build --apk
            if ($LASTEXITCODE -ne 0) { throw "tauri android build --apk failed" }
            Write-Host "==> Done. Output: src-tauri/gen/android/app/build/outputs/apk/"
        }
    }
}
