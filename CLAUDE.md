# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Proscenium is a cross-platform desktop IPTV client: Tauri v2 (Rust backend) + React + TypeScript + Tailwind CSS v4 + Zustand. The product spec is `SPEC.md` (repo root); work proceeds in milestones (M1 providers/auth, M2 catalog refresh/storage, M3 live TV browser, M4 built-in mpv player, M5 VOD browser, M6 search, M7 settings/error-handling/distribution, M8 resume playback/watch progress are done — see SPEC.md §5.9 + Milestone 8). `DEVELOPMENT.md` has full setup and troubleshooting; `README.md` summarizes milestone status.

## Commands

```powershell
npm run tauri dev        # full app with hot reload (Vite on fixed port 1420)
npm run dev              # frontend only in a browser — backed by src/lib/devMock.ts
npm run build            # TypeScript type-check + production frontend build

cd src-tauri
cargo test                          # all backend tests
cargo test --test milestone3        # one test file (tests/milestone{1..6}.rs)
cargo test --test milestone4 NAME   # one test by name
```

Release build: `npm run tauri build`, or manually `cargo build --release --features custom-protocol` (without the feature the exe loads the dev URL, not the embedded assets) and copy `WebView2Loader.dll` + `src-tauri/lib/libmpv-2.dll` next to the exe.

### Distribution & auto-update (M7)

- `npm run tauri build` produces a WiX `.msi` and an NSIS `-setup.exe` under `src-tauri/target/release/bundle/`; platform bundling is split into `tauri.windows.conf.json` (maps `lib/libmpv-2.dll` **and `lib/WebView2Loader.dll`** next to the installed exe) and `tauri.macos.conf.json` (embeds `lib/libmpv.2.dylib` as a framework), merged over `tauri.conf.json`. `mpv/player.rs::open_libmpv` searches next-to-exe and (macOS) `../Frameworks`. Full cross-platform steps live in `RELEASE.md`.
- **GNU-toolchain gotcha:** because the build is `*-pc-windows-gnu`, the exe dynamically imports `WebView2Loader.dll` (MSVC would static-link it). Tauri's NSIS template does **not** ship it, so it's bundled as an explicit resource and must be staged in `src-tauri/lib/` (gitignored, alongside `libmpv-2.dll`). Missing → the installed app dies on launch with "WebView2Loader.dll was not found". The WebView2 *runtime* (separate) installs via the download bootstrapper.
- The updater is signed: set `TAURI_SIGNING_PRIVATE_KEY` (contents of `src-tauri/proscenium-updater.key`, gitignored) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""` before building, or the build fails because `bundle.createUpdaterArtifacts` is on. The matching `plugins.updater.pubkey` is committed in `tauri.conf.json`; regenerate the pair with `npx tauri signer generate --ci -p "" -w src-tauri/proscenium-updater.key -f`.
- The launch-time update check is `src/lib/updater.ts::checkForUpdatesOnLaunch` (called from `App.tsx`); it no-ops outside Tauri and swallows every failure so a bad endpoint never blocks startup. `plugins.updater.endpoints` is a placeholder host.

### Windows toolchain quirks (this machine)

- `rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu` (no MSVC Build Tools installed). MinGW gcc comes from scoop.
- Node is managed by fnm and may not be on PATH: `$env:PATH = "$env:APPDATA\fnm\node-versions\v22.16.0\installation;$env:PATH"`.
- If the dev app window never appears (`STATUS_DLL_NOT_FOUND`), copy `WebView2Loader.dll` into `src-tauri/target/debug/` (see DEVELOPMENT.md).
- **All Rust tests must live in `src-tauri/tests/`**, never as unit tests: the lib target has `test = false` because only `tests/` binaries get the Common-Controls v6 manifest from `build.rs` that Tauri-linked executables need to load on Windows.
- The keychain test writes a real Windows Credential Manager entry (service `Proscenium`); connection tests bind throwaway local HTTP servers — no internet needed.

## Architecture

### IPC: one path, five places to touch

Every frontend↔backend interaction goes through typed wrappers in `src/lib/tauri.ts`, which dispatch to real `invoke()` inside Tauri or to `src/lib/devMock.ts` in a plain browser (the mock mirrors real behavior: pagination, filtering, ordering — keep it in sync). Adding a command means touching:

1. Handler in `src-tauri/src/commands/{providers,catalog,search,playback,settings,watch}.rs`
2. Registration in `generate_handler![]` in `src-tauri/src/lib.rs`
3. Rust types in `src-tauri/src/models.rs` ↔ TS types in `src/types/index.ts` (serde camelCase must match)
4. Typed wrapper in `src/lib/tauri.ts`
5. Mock implementation in `src/lib/devMock.ts`

Backend→frontend push uses Tauri events consumed inside Zustand stores (`src/store/`): `catalog:refresh_progress` / `catalog:refresh_complete` and `provider:status` (startup unreachable/expired warning banner, §12) in `catalogStore.ts`, `mpv:state_changed` in `playerStore.ts`.

### Backend layers (`src-tauri/src/`)

- `commands/` — Tauri command handlers plus managed state registered in `lib.rs` setup: `Db` (sqlx pool), `RefreshGuard` (prevents concurrent refreshes), `PlayerHandle`, `VideoHost` (HWND of the native video window).
- `db/` — SQLite via sqlx at `%APPDATA%\proscenium\proscenium.db` (WAL mode, FTS5 indexing); `schema.rs` applies the spec §15 schema on startup. Catalog refresh persists atomically. Delete the `%APPDATA%\proscenium` folder to simulate a clean install.
- `iptv/` — protocol clients: `xtream.rs` (6-endpoint catalog fetch) and `m3u.rs` (parsing, gzip, content-type inference).
- `keychain.rs` — Xtream passwords live in the OS keychain only; SQLite stores a reference key, never the secret.
- `mpv/` — libmpv loaded dynamically via `libloading` at runtime (LGPL compliance; `libmpv-2.dll` must sit next to the exe).

### Player rendering (the non-obvious part)

mpv does not render into the WebView. `mpv/mod.rs::video_host` creates a separate *top-level* native window (a child window would be clipped out of DWM composition) glued directly behind the transparent main window in z-order. The HTML page only goes transparent over the player area once the stream delivers frames. `lib.rs`'s `on_window_event` re-fits the video window on move/resize/focus, and the player's state callback self-heals the z-order. Anything touching window layering, transparency, or the player overlay needs to respect this sandwich.

### Catalog refresh flow

`refresh_catalog` runs async with progress events; a stale-cache check (6h) is spawned on startup (`commands::catalog::startup_stale_check`). The live channel list is paginated server-side (`get_live_channels`) and virtualized in the UI (`@tanstack/react-virtual`) to handle ~12k channels.

## Utility scripts (`scripts/`)

Python helpers operate on the live app DB: `inspect_db.py`, `check_catalog.py`, `seed_provider.py`, etc. Node `.mjs` scripts are CDP-based e2e probes for the player (`player_e2e.mjs`, `player_visual_check.mjs`).
