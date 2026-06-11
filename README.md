# Proscenium

Cross-platform desktop IPTV client built with Tauri v2 (Rust) + React + TypeScript + Tailwind CSS. See [specs/v1/spec-v1.md](specs/v1/spec-v1.md) for the full product specification.

## Status

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
- `specs/` — product specification.
