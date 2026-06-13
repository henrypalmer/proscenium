# Development Guide

How to build, run, and test Proscenium locally. Commands are PowerShell unless noted.

## Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust (rustup) | 1.85+ | [rust-toolchain.toml](rust-toolchain.toml) pins `channel = "stable"` (host default target). On **Windows without MSVC** you must run `rustup set default-host x86_64-pc-windows-gnu` once, or `stable` resolves to the MSVC triple and the link fails. macOS/Linux need nothing. See [Toolchain notes](#toolchain-notes). |
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

> **Why `WebView2Loader.dll` matters here:** this repo builds with the **GNU/MinGW** toolchain (no MSVC). Unlike MSVC — which statically links the WebView2 loader — the GNU build leaves the exe with a runtime dependency on `WebView2Loader.dll`. It must sit next to the exe both for dev runs *and* in packaged installers, or startup dies with "The code execution cannot proceed because WebView2Loader.dll was not found" (`STATUS_DLL_NOT_FOUND` / `STATUS_ENTRYPOINT_NOT_FOUND`).
>
> - **Dev runs:** copy it next to the debug exe once (after first build / `cargo clean`):
>   ```powershell
>   Copy-Item "$env:USERPROFILE\.cargo\registry\src\index.crates.io-*\webview2-com-sys-*\x64\WebView2Loader.dll" src-tauri\target\debug\
>   ```
> - **Installers:** it is bundled explicitly via `bundle.resources` in `tauri.windows.conf.json` (Tauri's NSIS template does *not* ship it for GNU builds), so staging it in `src-tauri/lib/` is required — see [RELEASE.md](RELEASE.md).

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

**Stage the bundled DLLs first.** `src-tauri/lib/` (gitignored) must contain **both**
`libmpv-2.dll` (the player engine) and `WebView2Loader.dll` (the WebView2 loader the
GNU-built exe links against) before bundling, or the installer will be missing them:

```powershell
Copy-Item "$env:USERPROFILE\.cargo\registry\src\index.crates.io-*\webview2-com-sys-*\x64\WebView2Loader.dll" src-tauri\lib\
# libmpv-2.dll: from your mpv-winbuild (see the dev-setup section)
```

Platform-specific bundling lives in `tauri.windows.conf.json` (puts `lib/libmpv-2.dll`
and `lib/WebView2Loader.dll` next to the exe) and `tauri.macos.conf.json` (embeds
`lib/libmpv.2.dylib` as a framework), merged over the shared `tauri.conf.json`.

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

- **Cross-platform toolchain pin.** [rust-toolchain.toml](rust-toolchain.toml) pins only `channel = "stable"` (no target triple) so the one committed file works on macOS, Linux, and Windows — each host resolves stable for its *default* target. This replaced an earlier `stable-x86_64-pc-windows-gnu` pin, which broke `cargo` on macOS (`error: target tuple in channel name`).
- **Windows without MSVC (this project's original machine):** the GNU toolchain is required because there are no Visual Studio C++ Build Tools — the MSVC target fails to link (Git's coreutils `link.exe` shadows the missing MSVC linker). Because the pin no longer names the GNU triple, you must make GNU the rustup **default host once per machine**:
  ```powershell
  rustup set default-host x86_64-pc-windows-gnu
  rustup show   # confirm: "Default host: x86_64-pc-windows-gnu"
  ```
  If you skip this, `stable` resolves to `stable-x86_64-pc-windows-msvc` and the build fails to link. On a machine *with* Build Tools, no action is needed (MSVC is fine).
- **Test binaries and `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139):** executables that link Tauri import `TaskDialogIndirect` from comctl32, which only exists in the v6 side-by-side assembly. [src-tauri/build.rs](src-tauri/build.rs) compiles and links a Common-Controls v6 manifest into test binaries (`windres` required). The lib target has `test = false` because cargo's `rustc-link-arg-tests` doesn't reach the lib's unit-test harness — keep tests under `src-tauri/tests/`.
- **Port 1420 busy:** `tauri dev` fails if another process holds the port (`strictPort` is on). Find it with `Get-NetTCPConnection -LocalPort 1420 | Select-Object OwningProcess`.
