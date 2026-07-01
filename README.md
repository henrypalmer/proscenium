# Proscenium

Cross-platform desktop IPTV client built with Tauri v2 (Rust) + React + TypeScript + Tailwind CSS. See [SPEC.md](SPEC.md) for the full product specification.

## Status

**Version 1.0** — Proscenium is feature-complete for its 1.0 release. On top of the milestones detailed below, 1.0 adds: a full-screen search results screen and Live TV channel filter (M9); a floating top navigation bar and a curated Home screen with Popular and Keep Watching rows (M10); Keep Watching refinements — series cards show the series artwork, clicking a series offers resume-last-episode or go-to-series, and each card can be marked watched or removed (M11, M13); sleek theme-matching scrollbars (M12); and user-created custom lists/playlists, with an "Add to list" affordance throughout the catalog and a "My Lists" row on Home (M14–M15); card hover/press reactivity and shared-element poster morphs into detail views, plus ambient content-entrance and route cross-fade motion, all honoring `prefers-reduced-motion` (M16–M17); and a cinematic detail-view redesign — a full-bleed hero backdrop (the provider's real backdrop art when available, a blurred-poster fallback otherwise) with the season/episode and synopsis content given fuller presence below it (M18); and a set of immersive browse refinements — fuller near-edge-to-edge Home rows with larger cards, a collapsible (default-expanded) genre panel, and a per-genre row stack for the "All Movies/All Shows" view (Popular first, then alphabetical, lazy-loaded) whose row titles jump to the full genre grid (M19). See [SPEC.md](SPEC.md) §5 and §19 for the full feature set and milestone history.

Milestone 8 (Resume Playback & Watch Progress) is implemented: clicking Play on a movie/episode with prior progress prompts to resume from the saved spot or start over (resume seeks via mpv before frames flow, so there's no jump from 0); position is persisted to a new `watch_progress` SQLite table (throttled during playback, flushed on close) and survives restarts. Movie cards and episode rows show a progress bar for in-progress items and a watched checkmark once past ~95%, at which point the resume prompt is skipped. Live TV is never tracked. Progress is provider-scoped (cascade-deleted) and entirely local — no provider requests. A "Skip Intro" button for TV series remains exploration only (§14, Q5) — IPTV providers supply no intro markers, so only a limited hybrid approach is feasible.

Milestone 7 (Polish, Settings & Distribution) is implemented: a working Settings UI (Providers, plus Playback — default external player with a `{url}` custom-command field and a hardware-decode toggle — and Appearance density), all wired to `get_settings`/`set_setting` over the §15 settings table with §15 defaults. A startup provider probe drives a `WarningBanner` for unreachable providers (with a Retry that re-probes and refills the catalog) and expired Xtream subscriptions. Stale `image_cache` entries past their 30-day TTL are evicted (rows + files) on launch. The Tauri bundle pipeline builds a signed WiX `.msi` and NSIS `-setup.exe` (bundling `libmpv-2.dll`), and `tauri-plugin-updater` checks for updates on launch. macOS `.dmg`/`.app` targets are configured but can only be produced on macOS hardware.

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

## Roadmap — Media-Hub direction (post-1.0)

Beyond 1.0, Proscenium is evolving from a provider-centric IPTV client into a **canonical, catalog-first media application**: browse a canonical movie/series catalog (external metadata via **Cinemeta**) and **resolve playback on click** across *all* configured IPTV providers **and** Stremio addons — adopting the Stremio addon model internally (a canonical catalog keyed by IMDB/TMDB id + a registry of stream resolvers), so "multiple active providers" and "Stremio support" become one architecture. **Milestones 39–44** are **implemented** (completing the Media-Hub arc): M39 added a merged, multi-provider catalog across every section; **M40** flips Movies/Series onto a **Cinemeta-backed canonical catalog** and **resolves playback on click** across all enabled providers into a **source picker** (movies confirmed by the provider's `tmdb_id` against Cinemeta's `moviedb_id`; series by name+year with a manual "wrong match?" override), with watch progress that **follows the title across sources** and a "My Providers" tab that keeps the pre-M40 provider grid for un-matchable VOD; **M41** adds **Stremio stream addons** (add-by-URL, with token-bearing manifest URLs kept in the OS keychain) as resolvers that fold direct/debrid streams into the same picker, flagging infoHash-only torrents as "needs a debrid service" (no torrent engine); **M42** polishes the merged picker with source **ranking** (resolution / debrid-cached / seeders / remembered-pick) and cross-source **dedup**, plus opt-in background **availability badges** on canonical cards; and **M43** folds the canonical catalog into **search** — both the overlay and the full results screen surface Cinemeta hits in an "All Sources" group alongside the local provider results (ungated from having a provider), routed to the same canonical detail + source picker, so addon-/multi-source titles are findable from search and not only Browse; and **M44** dedups those search results across sources — when a provider hit and a canonical "All Sources" hit are the same title (authoritative `content_match` imdb when recorded, else a name+year match), the provider duplicate is hidden and the canonical entry (whose picker already lists that provider plus any addons) is kept. Validated by the [2026-06-29 spike](docs/spikes/2026-06-29-multi-source-and-stremio.md): stream resolution is ~100% direct URLs (no torrent engine), movie matching is near-exact via the provider's `tmdb_id`, and series uses name+year with a manual override.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for full setup, run, test, and troubleshooting instructions.

Prerequisites: Rust 1.85+ and Node.js 22+. `rust-toolchain.toml` pins `channel = "stable"` for the host's default target, so the same file builds on macOS, Linux, and Windows. On Windows you need either the Visual Studio C++ Build Tools (MSVC) or MinGW-w64 gcc; if you use MinGW (no MSVC), set the rustup default host once with `rustup set default-host x86_64-pc-windows-gnu`. See [DEVELOPMENT.md](DEVELOPMENT.md) → Toolchain notes.

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
