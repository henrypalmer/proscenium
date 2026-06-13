# Proscenium

Cross-platform desktop IPTV client built with Tauri v2 (Rust) + React + TypeScript + Tailwind CSS. See [SPEC.md](SPEC.md) for the full product specification.

## Status

Milestone 6 (Search) is implemented: a global search overlay (Ctrl/Cmd+F from any section, plus a Header button) backed by the SQLite FTS5 tables — prefix-matching, case-insensitive, entirely local with no provider requests. Results arrive within ~240ms of the last keystroke (200ms debounce included), grouped into Live TV / Movies / TV Shows with 5 inline results per group and a "Show all" expander, content-type filter tabs with per-type genre narrowing, and a friendly no-results state. Live results start playback directly; VOD results open their detail view.

Milestone 5 (VOD Browser — Movies & TV Shows) is implemented: Movies and TV Shows sections with genre sidebars, a shared virtualized poster grid (responsive column count, lazy poster art with placeholder fallback, ~48 cells in the DOM for 12k items), movie and series detail views with on-demand Xtream metadata (`get_vod_info` / `get_series_info`, session-cached; episodes fetched per series and persisted), a season selector with per-season episode lists, and play / external-player launch from detail views and episode rows.

Milestone 4 (Built-in Player & External Player Handoff) is implemented: libmpv loaded dynamically at runtime (LGPL-compliant) renders into a native window behind a transparent WebView, with hardware decode (D3D11VA) on by default, full transport controls, audio/subtitle track selection, all §5.6 keyboard shortcuts, 10s/30s buffering thresholds, and external player handoff (mpv/VLC/custom). `libmpv-2.dll` (from mpv-winbuild) must sit next to the executable — see DEVELOPMENT.md.

Milestone 3 (Live TV Browser) is implemented: category sidebar with "All Channels" and A–Z/provider-order sorting, virtualized channel list paged on demand (60fps+ with 12k channels, ~25 rows in the DOM), lazy logos with placeholder fallback, skeleton loading rows, and a right-click context menu (Play / Open in External Player — wired to the player in Milestone 4). The frontend also gained a browser-only mock backend (`src/lib/devMock.ts`) so `npm run dev` works outside the Tauri shell.

Milestone 2 (Content Refresh & Catalog Storage) is implemented: full Xtream catalog fetch (6 endpoints), M3U parsing with gzip support and content-type inference, atomic catalog persistence with FTS5 indexing, refresh progress UI with failure toasts, and the 6-hour stale-cache background refresh on startup.

Milestone 1 (Project Scaffold & Provider Authentication) is implemented:

- Tauri v2 shell with React + TypeScript + Tailwind frontend.
- SQLite database (via `sqlx`) created at `%APPDATA%\proscenium\proscenium.db` with the full spec §15 schema applied on first launch.
- Provider management: add/edit/delete Xtream Codes and M3U providers (URL or local file).
- Xtream passwords are stored in the OS keychain (Windows Credential Manager / macOS Keychain); SQLite only holds a reference key.
- "Test Connection" validates Xtream credentials (with account status, expiry, and connection counts) and M3U playlist reachability.
- First-launch flow shows the Add Provider form when no providers exist.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for full setup, run, test, and troubleshooting instructions.

Prerequisites: Rust 1.85+ and Node.js 22+. On Windows, either the Visual Studio C++ Build Tools (MSVC) or MinGW-w64 gcc are required; this repo pins the GNU toolchain in `rust-toolchain.toml` because it was scaffolded on a machine without MSVC — remove that pin if you have Build Tools installed.

```sh
npm install
npm run tauri dev    # run the app with hot reload
npm run build        # type-check + build the frontend
cargo test           # run backend tests (from src-tauri/)
```

## Layout

- `src/` — React frontend (components, pages, store, typed Tauri bindings).
- `src-tauri/` — Rust backend: Tauri commands (`commands/`), SQLite layer (`db/`), IPTV protocol clients (`iptv/`), OS keychain integration (`keychain.rs`).
- `SPEC.md` — product specification.
