# Development Guide

How to build, run, and test Proscenium locally. Commands are PowerShell unless noted.

## Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust (rustup) | 1.85+ | This repo pins `stable-x86_64-pc-windows-gnu` in [rust-toolchain.toml](rust-toolchain.toml) — rustup installs it automatically on first `cargo` invocation. See [Toolchain notes](#toolchain-notes). |
| Node.js | 22+ | On this machine Node is managed by **fnm** and is *not* on PATH by default — see below. |
| MinGW-w64 gcc | 13+ | Needed by the GNU toolchain for linking and `windres` (already present via scoop: `~\scoop\apps\gcc\current\bin`). |
| WebView2 Runtime | any | Preinstalled on Windows 11. |

### Getting `node`/`npm` on PATH (fnm)

If `npm` is "not recognized", prepend the fnm Node install for the current session:

```powershell
$env:PATH = "$env:APPDATA\fnm\node-versions\v22.16.0\installation;$env:PATH"
```

Or set up fnm shell integration permanently (add to your PowerShell `$PROFILE`):

```powershell
fnm env --use-on-cd | Out-String | Invoke-Expression
```

## First-time setup

```powershell
npm install
```

## Running the app in development

```powershell
npm run tauri dev
```

This starts the Vite dev server on **port 1420** (fixed — see `tauri.conf.json` `devUrl`), then compiles and launches the Rust app pointing at it. Frontend changes hot-reload; Rust changes trigger a rebuild/relaunch.

> **First run / after `cargo clean`:** the unbundled exe needs `WebView2Loader.dll` next to it. If the app window never appears (or the process exits with `STATUS_DLL_NOT_FOUND` / `STATUS_ENTRYPOINT_NOT_FOUND`), copy it once:
>
> ```powershell
> Copy-Item "$env:USERPROFILE\.cargo\registry\src\index.crates.io-*\webview2-com-sys-*\x64\WebView2Loader.dll" src-tauri\target\debug\
> ```
>
> (The Tauri bundler handles this automatically for packaged installers in Milestone 7.)

### Frontend only

```powershell
npm run dev
```

Serves the UI in a browser at `http://localhost:1420`. Useful for pure styling work only — every `invoke()` call fails outside the Tauri shell, so the app will sit on its loading state.

## Tests

Backend integration tests (schema, provider CRUD, keychain, connection tests):

```powershell
cd src-tauri
cargo test
```

Notes:
- The keychain test writes/removes a real entry in Windows Credential Manager (service `Proscenium`).
- Connection tests bind a throwaway HTTP server on `127.0.0.1` — no internet access needed.

Frontend type-check + production build:

```powershell
npm run build
```

## Release build

```powershell
npm run build                                      # produces dist/ (embedded into the exe)
cd src-tauri
cargo build --release --features custom-protocol   # WITHOUT the feature the exe loads the dev URL!
Copy-Item "$env:USERPROFILE\.cargo\registry\src\index.crates.io-*\webview2-com-sys-*\x64\WebView2Loader.dll" target\release\
Copy-Item lib\libmpv-2.dll target\release\         # built-in player engine
.\target\release\proscenium.exe
```

(`npm run tauri build` handles all of this automatically except the libmpv copy.)

## Packaged installers & auto-update (M7)

`npm run tauri build` produces the platform installers under
`src-tauri/target/release/bundle/` (Windows: `.msi` + `-setup.exe`; macOS: `.app` +
`.dmg`), each with a `.sig` minisign signature for the auto-updater. Because
`bundle.createUpdaterArtifacts` is on, the build needs the updater signing key in the
environment or it fails:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content src-tauri\proscenium-updater.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri build
```

Platform-specific bundling lives in `tauri.windows.conf.json` (bundles `lib/libmpv-2.dll`
next to the exe) and `tauri.macos.conf.json` (embeds `lib/libmpv.2.dylib` as a
framework), merged over the shared `tauri.conf.json`.

**Full cross-platform release steps — Windows, macOS, code signing, notarization, the
update feed, and CI — are in [RELEASE.md](RELEASE.md).**

## App data

Everything the app writes lives in `%APPDATA%\proscenium\`:

- `proscenium.db` (+ `-wal`/`-shm`) — SQLite catalog/provider/settings store.
- `startup.log` — time from process start to `RunEvent::Ready`, written on every launch.

Delete the folder to simulate a clean install. Xtream passwords are *not* in the DB — they're in Windows Credential Manager under service **Proscenium** (`Win+R` → `control keymgr.dll`, or Credential Manager → Windows Credentials → Generic Credentials).

## Utility scripts

- `python scripts\make_icon.py` — regenerates the placeholder `src-tauri/icons/icon.ico` (requires Pillow).
- `python scripts\inspect_db.py` — prints the table/index inventory and provider count of the live app database.

## Toolchain notes

- **Why the GNU toolchain?** This machine has no Visual Studio C++ Build Tools (and no admin rights to install them). The MSVC target fails to link — Git's coreutils `link.exe` shadows the missing MSVC linker. On a machine *with* Build Tools, you can delete `rust-toolchain.toml` and build with the default MSVC toolchain.
- **Test binaries and `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139):** executables that link Tauri import `TaskDialogIndirect` from comctl32, which only exists in the v6 side-by-side assembly. [src-tauri/build.rs](src-tauri/build.rs) compiles and links a Common-Controls v6 manifest into test binaries (`windres` required). The lib target has `test = false` because cargo's `rustc-link-arg-tests` doesn't reach the lib's unit-test harness — keep tests under `src-tauri/tests/`.
- **Port 1420 busy:** `tauri dev` fails if another process holds the port (`strictPort` is on). Find it with `Get-NetTCPConnection -LocalPort 1420 | Select-Object OwningProcess`.
