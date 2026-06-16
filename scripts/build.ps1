#!/usr/bin/env pwsh
<#
.SYNOPSIS
    One-shot release build for Proscenium: installers + a directly runnable app.

.DESCRIPTION
    Performs every step needed to produce a release build on the current platform:

      Windows -> .msi + -setup.exe installers, AND a double-clickable
                 src-tauri/target/release/proscenium.exe with libmpv-2.dll and
                 WebView2Loader.dll copied next to it (no install required).
      macOS   -> .app + .dmg, AND the self-contained
                 src-tauri/target/release/bundle/macos/Proscenium.app.

    Run this, then double-click the reported app/exe to launch it without going
    through an installer on every iteration.

    Mirrors the manual steps in RELEASE.md / DEVELOPMENT.md:
      - puts the fnm-managed Node on PATH if npm isn't already found,
      - stages the bundled native libs into src-tauri/lib/,
      - exports the updater signing key (required for full bundling),
      - runs the build, then stages the runnable app.

.PARAMETER Fast
    Skip installer packaging (and updater signing): build only the runnable
    exe/app via `cargo build --release --features custom-protocol`. Much quicker
    for iterating. No .msi/.dmg are produced.

.PARAMETER Run
    Launch the built app when the build finishes.

.EXAMPLE
    ./scripts/build.ps1
    Full build: installers + runnable app.

.EXAMPLE
    ./scripts/build.ps1 -Fast -Run
    Quick rebuild of just the runnable app, then launch it.
#>
[CmdletBinding()]
param(
    [switch]$Fast,
    [switch]$Run
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# --- Locate the repo (this script lives in <repo>/scripts) -------------------
$RepoRoot = Split-Path $PSScriptRoot -Parent
$Lib      = Join-Path $RepoRoot 'src-tauri/lib'
$Release  = Join-Path $RepoRoot 'src-tauri/target/release'
$SignKey  = Join-Path $RepoRoot 'src-tauri/proscenium-updater.key'
Set-Location $RepoRoot

# pwsh 5.1 doesn't define $IsWindows/$IsMacOS; assume Windows there.
$onWindows = if (Test-Path variable:IsWindows) { $IsWindows } else { $true }
$onMac     = (Test-Path variable:IsMacOS) -and $IsMacOS

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Info($msg) { Write-Host "    $msg" -ForegroundColor DarkGray }

# --- Ensure npm is on PATH (fnm-managed Node on this machine) ----------------
function Resolve-Node {
    if (Get-Command npm -ErrorAction SilentlyContinue) { return }
    if ($onWindows) {
        $base = Join-Path $env:APPDATA 'fnm/node-versions'
        if (Test-Path $base) {
            $latest = Get-ChildItem $base -Directory |
                Sort-Object Name -Descending | Select-Object -First 1
            if ($latest) {
                $install = Join-Path $latest.FullName 'installation'
                if (Test-Path $install) {
                    $env:PATH = "$install$([IO.Path]::PathSeparator)$env:PATH"
                    Info "Added fnm Node to PATH: $install"
                }
            }
        }
    }
    if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        throw "npm not found. Install Node 22+ (or configure fnm) and retry. See DEVELOPMENT.md."
    }
}

# --- Stage the native libs the bundle/exe need -------------------------------
function Stage-WindowsLibs {
    if (-not (Test-Path (Join-Path $Lib 'libmpv-2.dll'))) {
        throw "src-tauri/lib/libmpv-2.dll is missing. Copy it from your mpv-winbuild (it is gitignored and cannot be fetched automatically). See DEVELOPMENT.md."
    }
    $wv = Join-Path $Lib 'WebView2Loader.dll'
    if (-not (Test-Path $wv)) {
        Info 'WebView2Loader.dll not staged; copying from the cargo registry...'
        $src = Get-ChildItem "$env:USERPROFILE/.cargo/registry/src/index.crates.io-*/webview2-com-sys-*/x64/WebView2Loader.dll" -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if (-not $src) {
            throw "WebView2Loader.dll missing from src-tauri/lib and not found in the cargo registry. Build once (or run `cargo fetch`) so webview2-com-sys is present, then retry."
        }
        Copy-Item $src.FullName $wv
    }
    Info "Native libs staged in src-tauri/lib (libmpv-2.dll, WebView2Loader.dll)."
}

# --- Export the updater signing key (full bundling requires it) --------------
function Set-SigningKey {
    if (-not (Test-Path $SignKey)) {
        throw "Updater signing key not found at src-tauri/proscenium-updater.key. It is required because bundle.createUpdaterArtifacts is enabled. See RELEASE.md to generate one, or use -Fast to skip bundling."
    }
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $SignKey -Raw
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
    Info 'Updater signing key exported.'
}

# --- Report what was produced ------------------------------------------------
function Report-Artifacts([string]$runnable) {
    Step 'Done.'
    if (-not $Fast) {
        $bundle = Join-Path $Release 'bundle'
        if (Test-Path $bundle) {
            $installers = Get-ChildItem $bundle -Recurse -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Extension -in '.msi', '.dmg' -or $_.Name -like '*-setup.exe' }
            if ($installers) {
                Write-Host 'Installers:' -ForegroundColor Green
                $installers | ForEach-Object { Write-Host "  $($_.FullName)" }
            }
        }
    }
    Write-Host 'Double-click to run (no install needed):' -ForegroundColor Green
    Write-Host "  $runnable"
}

# =============================================================================
Resolve-Node

if ($onWindows) {
    Stage-WindowsLibs
    if ($Fast) {
        Step 'Building frontend (npm run build)...'
        npm run build
        if ($LASTEXITCODE) { throw "frontend build failed ($LASTEXITCODE)" }
        Step 'Compiling release exe (cargo build --release --features custom-protocol)...'
        Push-Location (Join-Path $RepoRoot 'src-tauri')
        try {
            cargo build --release --features custom-protocol
            if ($LASTEXITCODE) { throw "cargo build failed ($LASTEXITCODE)" }
        } finally { Pop-Location }
    } else {
        Set-SigningKey
        Step 'Building installers + exe (npm run tauri build)...'
        npm run tauri build
        if ($LASTEXITCODE) { throw "tauri build failed ($LASTEXITCODE)" }
    }

    # Make target/release/proscenium.exe double-clickable: the DLLs are bundled
    # into the installer but not left next to the dev exe (DEVELOPMENT.md).
    Step 'Staging the runnable exe...'
    Copy-Item (Join-Path $Lib 'libmpv-2.dll')      $Release -Force
    Copy-Item (Join-Path $Lib 'WebView2Loader.dll') $Release -Force
    $exe = Join-Path $Release 'proscenium.exe'
    if (-not (Test-Path $exe)) { throw "Expected exe not found at $exe" }
    Report-Artifacts $exe
    if ($Run) { Step "Launching $exe"; Start-Process $exe }
}
elseif ($onMac) {
    # Follows RELEASE.md > macOS. Requires: brew install mpv dylibbundler.
    Step 'Staging libmpv + its dependency tree (RELEASE.md)...'
    if (-not (Get-Command brew -ErrorAction SilentlyContinue)) {
        throw 'Homebrew not found. Install it and `brew install mpv dylibbundler`. See RELEASE.md.'
    }
    foreach ($tool in 'dylibbundler', 'install_name_tool', 'otool', 'codesign') {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool not found. Install Xcode Command Line Tools / `brew install dylibbundler`. See RELEASE.md."
        }
    }
    $brewPrefix = (brew --prefix).Trim()
    $dylib = Join-Path $Lib 'libmpv.2.dylib'
    Copy-Item (Join-Path $brewPrefix 'lib/libmpv.2.dylib') $dylib -Force
    chmod u+w $dylib
    dylibbundler -of -b -x $dylib -d "$Lib/" -p '@rpath'
    install_name_tool -id '@rpath/libmpv.2.dylib' $dylib

    # CRITICAL (RELEASE.md): strip the bogus `@rpath/` LC_RPATH dylibbundler adds,
    # or dyld refuses to load libmpv. Only the literal `@rpath/` rpath is removed.
    foreach ($f in Get-ChildItem "$Lib/*.dylib") {
        chmod u+w $f.FullName
        while ((otool -l $f.FullName | Select-String -Pattern 'path @rpath/ \(' -Quiet)) {
            install_name_tool -delete_rpath '@rpath/' $f.FullName
        }
        codesign --force -s - $f.FullName
    }

    Step 'Regenerating tauri.macos.conf.json framework list from src-tauri/lib...'
    $dylibs = Get-ChildItem "$Lib/*.dylib" | Sort-Object Name |
        ForEach-Object { "lib/$($_.Name)" }
    $macConf = [ordered]@{
        '$schema' = 'https://schema.tauri.app/config/2'
        bundle    = @{ macOS = @{ frameworks = @($dylibs) } }
    }
    $macConf | ConvertTo-Json -Depth 6 |
        Set-Content (Join-Path $RepoRoot 'src-tauri/tauri.macos.conf.json')

    if ($Fast) {
        Step 'Building frontend (npm run build)...'
        npm run build
        if ($LASTEXITCODE) { throw "frontend build failed ($LASTEXITCODE)" }
        Step 'Compiling release app (cargo build --release --features custom-protocol)...'
        Push-Location (Join-Path $RepoRoot 'src-tauri')
        try {
            cargo build --release --features custom-protocol
            if ($LASTEXITCODE) { throw "cargo build failed ($LASTEXITCODE)" }
        } finally { Pop-Location }
    } else {
        Set-SigningKey
        Step 'Building .app + .dmg (npm run tauri build)...'
        npm run tauri build
        if ($LASTEXITCODE) { throw "tauri build failed ($LASTEXITCODE)" }
    }

    $app = Join-Path $Release 'bundle/macos/Proscenium.app'
    if (-not (Test-Path $app)) { throw "Expected .app not found at $app" }
    Report-Artifacts $app
    if ($Run) { Step "Launching $app"; open $app }
}
else {
    throw 'Unsupported platform. This script handles Windows and macOS (see RELEASE.md for Linux).'
}
