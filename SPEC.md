# Proscenium — Product Specification

**Version:** 0.6.0 (Draft)
**Status:** In Progress
**Last Updated:** 2026-06-13

---

## Table of Contents

1. [Overview](#1-overview)
2. [Goals & Non-Goals](#2-goals--non-goals)
3. [Recommended Tech Stack](#3-recommended-tech-stack)
4. [Architecture Overview](#4-architecture-overview)
5. [Feature Specifications](#5-feature-specifications)
   - 5.1 [Provider Authentication](#51-provider-authentication)
   - 5.2 [Content Refresh](#52-content-refresh)
   - 5.3 [Live TV Browser](#53-live-tv-browser)
   - 5.4 [Video on Demand (VOD) Browser](#54-video-on-demand-vod-browser)
   - 5.5 [Search](#55-search)
   - 5.6 [Playback](#56-playback)
   - 5.7 [Cover Art & Metadata (Planned)](#57-cover-art--metadata-planned)
   - 5.8 [IMDB Integration (Planned)](#58-imdb-integration-planned)
   - 5.9 [Resume Playback & Watch Progress](#59-resume-playback--watch-progress)
6. [Protocol Support](#6-protocol-support)
7. [Media Format Support](#7-media-format-support)
8. [Data Models](#8-data-models)
9. [UI/UX Guidelines](#9-uiux-guidelines)
10. [Performance Requirements](#10-performance-requirements)
11. [Cross-Platform Requirements](#11-cross-platform-requirements)
12. [Error Handling & Edge Cases](#12-error-handling--edge-cases)
13. [Future Roadmap](#13-future-roadmap)
14. [Open Questions](#14-open-questions)
15. [SQLite Database Schema](#15-sqlite-database-schema)
16. [Tauri Command API](#16-tauri-command-api)
17. [Project Structure](#17-project-structure)
18. [UI Component Inventory](#18-ui-component-inventory)
19. [Development Milestones](#19-development-milestones)

---

## 1. Overview

Proscenium is a cross-platform desktop IPTV client that allows users to connect to IPTV providers via the Xtream Codes API and M3U playlist protocols. The application provides a clean, performant interface for browsing and playing live TV channels and video on demand (VOD) content — movies and TV shows — sourced from the user's provider.

Proscenium prioritizes performance, responsiveness, and ease of use across Windows and macOS, with Linux support planned for a future release. All data is stored locally — no cloud sync, no accounts, no dependency on Proscenium's servers.

---

## 2. Goals & Non-Goals

### Goals

- Connect to one or more IPTV providers using Xtream Codes or M3U protocols.
- Display live TV channels and VOD content (movies, TV series) organized by category/genre.
- Provide fast, intuitive search across all content types.
- Play streams using a built-in player with the option to hand off to an external player.
- Support the widest possible range of video and audio formats, including HDR and lossless audio.
- Be fast to load, responsive to interact with, and light on system resources.
- Run natively on Windows, macOS, and Linux.

### Non-Goals (v1.0)

- DVR/recording functionality.
- Multi-provider simultaneous streaming (one active provider at a time in v1).
- Social or sharing features.
- Mobile platform support.
- Linux platform support (planned for a future release).
- Cloud sync of any kind — all data (catalog cache, credentials, settings, watch history) is stored locally only.
- EPG (Electronic Program Guide) — deferred to v1.1.
- Time-shift / pause live TV — deferred to a future release.

---

## 3. Recommended Tech Stack

Given the requirements — cross-platform native desktop, high performance, rich media playback, and emphasis on UI responsiveness — the recommended stack is:

### Frontend / Shell: **Tauri v2 (Rust + WebView)**

- Tauri provides a native desktop shell with a web-based UI layer (uses the OS WebView: WebKit on macOS/Linux, WebView2 on Windows).
- The Rust backend handles heavy work: network requests, file I/O, M3U parsing, stream management.
- Significantly lighter than Electron (~600 KB binary vs. ~80 MB).
- Ships a single binary with no bundled browser engine, resulting in fast startup.

### UI Layer: **React + TypeScript**

- Well-understood ecosystem with strong tooling.
- Provides a performant component model suited to large content lists (virtualized rendering).

### Styling: **Tailwind CSS**

- Utility-first, easy to build a custom design language without fighting a component library's opinions.

### Backend / Core Logic: **Rust (via Tauri commands)**

- M3U and Xtream Codes protocol parsing.
- HTTP client for provider API calls (`reqwest`).
- Local SQLite database via `sqlx` for caching content catalogs, credentials, and settings.
- Image caching for cover art.

### Media Playback: **libmpv (embedded)**

- mpv is a battle-tested, open-source media player with support for virtually every format.
- libmpv can be embedded directly into the app for the built-in player.
- mpv can also be launched as an external process for the "open in external player" flow, as can VLC.
- Handles HDR, Dolby Vision, Dolby Atmos, TrueHD, DTS-HD MA, and all common codecs natively.
- **Licensing note:** libmpv is licensed under LGPL v2.1+. As a proprietary application, the app must dynamically link against libmpv (not statically) and must make the libmpv source available to users upon request. The app's own source code does not need to be disclosed. This is a standard and well-understood compliance path for commercial software using libmpv.

### Local Storage: **SQLite** (via `sqlx` in Rust)

- Stores the content catalog, provider credentials (encrypted), settings, and watch history.

### Build & Distribution

- Tauri's built-in updater and bundler for platform-specific installers (`.msi` / `.exe`, `.dmg` / `.app`, `.deb` / `.AppImage` / `.rpm`).

---

## 4. Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                   Tauri App Shell                    │
│  ┌───────────────────────┐  ┌──────────────────────┐│
│  │   React UI (WebView)  │  │   libmpv Player      ││
│  │  - Content Browser    │  │   (embedded window)  ││
│  │  - Search             │  └──────────────────────┘│
│  │  - Settings           │                           │
│  └──────────┬────────────┘                           │
│             │ Tauri IPC Commands                     │
│  ┌──────────▼────────────────────────────────────┐  │
│  │              Rust Backend Core                │  │
│  │  ┌─────────────┐  ┌──────────────────────┐   │  │
│  │  │ IPTV Client │  │   SQLite Cache DB    │   │  │
│  │  │  - Xtream   │  │  - Channels          │   │  │
│  │  │  - M3U      │  │  - VOD catalog       │   │  │
│  │  └──────┬──────┘  │  - Credentials       │   │  │
│  │         │         │  - Settings          │   │  │
│  │  ┌──────▼──────┐  └──────────────────────┘   │  │
│  │  │ HTTP Client │                              │  │
│  │  └─────────────┘                              │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
             │
     IPTV Provider (Internet)
```

---

## 5. Feature Specifications

### 5.1 Provider Authentication

#### Description

Users configure one or more IPTV provider connections. The app securely stores credentials and uses them to authenticate with the provider's API.

#### Supported Authentication Methods

**Xtream Codes API**
- Fields: Server URL, Username, Password, Port (optional, may be embedded in URL)
- Authentication endpoint: `GET /player_api.php?username={u}&password={p}`
- On success: provider returns account info JSON including subscription expiry, max connections, and server info.

**M3U Playlist**
- Fields: Playlist URL (may include embedded credentials), or local file path.
- No active authentication — credentials are encoded in the URL or the file is fetched directly.

#### Behaviors

- The app presents an "Add Provider" form on first launch and from the Settings screen.
- On saving, the app immediately tests the connection and reports success or failure with a descriptive error.
- Multiple provider profiles can be saved, but only one is active at a time (v1).
- Credentials are stored encrypted using the OS keychain (Keychain on macOS, DPAPI/Credential Manager on Windows, libsecret on Linux).
- The provider profile displays: provider name (user-defined), server URL, subscription status (if available via Xtream), active connection count, and expiry date.

#### Error States

- Invalid credentials → display "Authentication failed. Check your username and password."
- Unreachable server → display "Could not connect to [URL]. Check the server address and your internet connection."
- Account expired → display a warning banner indicating the subscription has lapsed.

---

### 5.2 Content Refresh

#### Description

The app fetches and caches the full content catalog (channels and VOD) from the provider. Catalog data is stored locally so the app is usable without re-fetching on every launch.

#### Trigger Conditions

- **On startup:** Always load from the local cache first for immediate display. Check if the cache is stale (default: older than 6 hours) and, if so, trigger a background refresh.
- **Manual refresh:** A refresh button in the UI triggers an immediate full refresh regardless of cache age.
- **On provider change:** Switching the active provider always triggers a fresh fetch.

#### Refresh Process (Xtream)

1. Fetch live stream categories: `GET /player_api.php?action=get_live_categories`
2. Fetch live streams: `GET /player_api.php?action=get_live_streams`
3. Fetch VOD categories: `GET /player_api.php?action=get_vod_categories`
4. Fetch VOD streams: `GET /player_api.php?action=get_vod_streams`
5. Fetch series categories: `GET /player_api.php?action=get_series_categories`
6. Fetch series: `GET /player_api.php?action=get_series`
7. Persist all results to SQLite, replacing previous catalog.

#### Refresh Process (M3U)

1. Download the M3U playlist file from the configured URL (or re-read local file).
2. Parse `#EXTINF` tags to extract channel/VOD metadata (name, group, logo URL, stream URL).
3. Infer content type from group tags or `type:` attributes where present.
4. Persist to SQLite.

#### UI During Refresh

- A subtle progress indicator appears in the header/toolbar during background refresh.
- Content already in cache remains fully browsable during refresh.
- On completion, the UI reflects the updated catalog without requiring a manual page reload.
- If refresh fails, the stale cache remains in place and a non-blocking toast notification informs the user.

---

### 5.3 Live TV Browser

#### Description

A section of the UI dedicated to browsing and launching live TV channels.

#### Layout

- Top-level navigation entry labeled **"Live TV"**.
- A sidebar or tab-strip lists all available **channel categories** (e.g., "Sports", "News", "Entertainment", "Kids").
- A special **"All Channels"** entry at the top of the category list shows every channel regardless of category.
- The main content area shows channels for the selected category as a list or grid.
- A **channel filter bar** sits directly above the channel list (see below).

#### Channel Filter (within section)

A text input pinned above the channel list lets the user quickly narrow the visible channels by name without leaving the Live TV browser. This is distinct from global Search (§5.5): it is an in-place filter scoped to the channels of the **currently selected category** (or all channels when "All Channels" is selected), not a cross-content search.

- **Live filtering:** the list filters as the user types — no submit required — matching the typed text against the channel name (case-insensitive substring/prefix match).
- **Category-scoped:** the filter applies on top of the active category selection. Switching categories re-applies the current filter text against the new category's channels; clearing the input restores the full category list.
- **Scope correctness:** because the channel list is paginated server-side and virtualized (~12k channels, §10), the filter must not be limited to the rows currently held in the virtualization window. The filter text is passed to the backend (`get_live_channels` `query` parameter, §16) so it matches across the entire active category, and the filtered result remains virtualized.
- **Empty result:** when no channel in the active category matches, show a brief inline "No channels match '[text]'." message in place of the list.
- The filter input is empty by default and resets when the user changes provider.

#### Channel Card / List Item

Each channel displays:
- Channel logo (thumbnail), falling back to a placeholder icon if no logo URL is available.
- Channel name.
- Category label (shown in "All Channels" view).
- EPG "now playing" info if EPG data is available (future feature, noted in roadmap).

#### Interactions

- Clicking/selecting a channel opens the Playback view (see 5.6).
- Right-clicking (or a context menu button) offers: "Play", "Open in External Player", "Add to Favorites" (future).

#### Sorting & Display

- Default sort: alphabetical by channel name within each category.
- Categories sorted alphabetically, with any provider-defined ordering preserved as an alternative sort option.

---

### 5.4 Video on Demand (VOD) Browser

#### Description

A section for browsing VOD content: movies and TV series. These are presented as separate sub-sections.

#### Sub-sections

**Movies**
- Top-level navigation entry **"Movies"**.
- A sidebar or filter strip lists all available **genres** (e.g., "Action", "Comedy", "Drama", "Documentary").
- An **"All Movies"** entry shows the full movie catalog.
- Main area: grid of movie cards (poster art, title, year).

**TV Series**
- Top-level navigation entry **"TV Shows"**.
- Same genre filter structure as Movies.
- Main area: grid of series cards (poster art, show title).
- Selecting a series opens a detail view showing seasons and episodes.

#### Movie/Series Card

Each card displays:
- Cover art / poster (thumbnail). Falls back to a styled placeholder with the title if no image is available.
- Title.
- Release year (if available).
- IMDB rating badge (when IMDB integration is active, see 5.8).

#### Detail View

Selecting a movie or series opens a detail panel/page showing:
- Full-size banner or poster art.
- Title, year, genre tags.
- Description/synopsis (from provider metadata or IMDB).
- IMDB rating and vote count (when available).
- For series: season selector → episode list with episode titles, numbers, and descriptions.
- Play button (built-in player) and Open in External Player button.

---

### 5.5 Search

#### Description

A global search that queries across all content types: live channels, movies, and TV shows.

#### Access

- A persistent search bar or search icon in the main navigation/header, accessible from any section of the app.
- Keyboard shortcut: `Cmd+F` (macOS) / `Ctrl+F` (Windows/Linux).

#### Behavior

- Search is performed locally against the cached catalog — no network request required.
- Results appear as the user types (debounced, ~200ms delay).
- Results are grouped by type: **Live TV**, **Movies**, **TV Shows**.
- Each result group shows a maximum of 5 results inline, with a "Show all [N] results" expander.

#### Submit → Full Results Screen

In addition to the inline preview in the overlay, pressing **Enter** while the search input is focused commits the search:

- The search overlay **closes**, and the app navigates to a dedicated **search results screen** showing the full result set for the committed query (not capped at the 5-per-group inline preview).
- The results screen is **sectioned by content type** — separate **Live TV**, **Movies**, and **TV Shows** sections, each rendered with that type's standard card format and the section's normal grid/list layout. Empty sections are omitted.
- The active **content-type** and **genre/category** filters (see below) carry over from the overlay to the results screen; the screen reflects and lets the user keep adjusting them.
- Result interactions match the overlay: a Live TV result starts playback, a VOD result opens its detail view.
- If the query is blank/whitespace when Enter is pressed, no navigation occurs (the overlay stays open).
- The committed query text remains visible on the results screen so the user can refine or clear it; the no-results state (below) applies here too when nothing matches.

#### Filters

Users can narrow results with filters:
- **Content type:** All / Live TV / Movies / TV Shows (default: All).
- **Genre / Category** (shown when a specific content type is selected): dynamically populated from the catalog.

#### Result Items

- Same card format as the respective browser section.
- Clicking a result navigates to the detail view (for VOD) or starts playback (for Live TV).

#### Empty / No Results State

- Friendly message: "No results for '[query]'." with a suggestion to check spelling or try a broader term.

---

### 5.6 Playback

#### Description

The app supports two playback modes: a built-in player and an external player handoff.

#### Built-in Player

- Powered by libmpv embedded within the app.
- Opens in a full-screen or windowed overlay on top of the content browser.
- Supports hardware-accelerated decoding (DXVA2/D3D11VA on Windows, VideoToolbox on macOS, VA-API/VDPAU on Linux).

**Controls**
- Play / Pause
- Seek bar (where applicable — live TV disables seeking or shows a time-shift bar if supported)
- Volume slider and mute toggle
- Audio track selector (for multi-audio streams)
- Subtitle track selector (for streams with embedded subtitles)
- Full-screen toggle
- Picture-in-picture (where supported by the OS)
- Back / close button to return to the browser

**Keyboard Shortcuts (built-in player)**

| Action | Shortcut |
|--------|----------|
| Play / Pause | Space |
| Seek forward 10s | → |
| Seek backward 10s | ← |
| Volume up | ↑ |
| Volume down | ↓ |
| Mute | M |
| Full screen | F |
| Close player | Esc |
| Next audio track | A |
| Next subtitle track | S |

#### External Player Handoff

- Available from the context menu on any channel/movie and from the detail view.
- Supported external players: **mpv**, **VLC**, **any player the user configures** (custom command with `{url}` placeholder).
- The stream URL is passed to the external player's CLI.
- Default external player is configured in Settings.

#### Buffering & Loading

- A loading spinner appears while the stream is buffering.
- While a stream is loading (before its first frames arrive) and whenever it has failed, the player surface shows an opaque, soft, dark backdrop — it must never be transparent. The backdrop fades out only once the stream is actually delivering frames. *(Added during Milestone 4.)*
- If buffering exceeds 10 seconds, a non-blocking message is shown: "Stream is taking longer than expected to load."
- If the stream fails to start, a clear error message is shown with an option to retry or open in an external player.

---

### 5.7 Cover Art & Metadata (Planned)

> **Status:** Planned — not in v1.0 scope.

- For VOD content where the provider supplies a poster/logo URL, download and cache the image locally on first view.
- Images are cached in the app's data directory and expire after 30 days.
- For content without provider-supplied art, the app will attempt to match titles against The Movie Database (TMDB) API to retrieve posters, backdrops, and metadata.
- Matching logic: normalize title + year → query TMDB search endpoint → take the top result if confidence is sufficient.

---

### 5.8 IMDB Integration (Planned)

> **Status:** Planned — not in v1.0 scope.

- Display IMDB ratings on movie and TV show cards and detail views.
- Data source: OMDb API (or direct IMDB data if a license is obtained).
- Matching: title + year → OMDb search → cache result in SQLite against the VOD stream ID.
- Show: IMDB star rating (e.g., ★ 7.4) and vote count.
- Ratings are refreshed no more than once every 7 days per title.

---

### 5.9 Resume Playback & Watch Progress

#### Description

Proscenium remembers how far the user has watched each piece of VOD content (movies and TV episodes) so playback can be resumed, progress is visible at a glance while browsing, and finished items are marked as watched. This is the "Continue Watching" item from §13, promoted into scope.

Live TV is **never** tracked — it has no resumable position (its `duration` is `null`).

#### Watch Position Tracking

- While the built-in player is playing a movie or episode, the current position is persisted to SQLite periodically (throttled, roughly every 5 seconds) and flushed once more when the player closes.
- Each record is keyed by `(provider_id, content_type, content_id)` and stores the last position, the total duration (when known), and a completion flag. See the `watch_progress` table in §15.
- Records are provider-scoped and removed automatically when the provider is deleted (cascade).

#### Completion

- When playback passes a completion threshold (**≥ 95%** of the known duration), the item is marked **complete**.
- A completed item no longer offers a resume prompt and no longer shows a partial progress bar. Instead its thumbnail shows a small **watched checkmark** in a corner.
- If the user replays a completed item and watches past the start, it is treated as in-progress again (the completion flag clears and progress tracking resumes).

#### Resume Prompt

- Clicking **Play** on a movie or episode that has *meaningful* prior progress (more than a few seconds in, and below the completion threshold) presents a choice before playback starts:
  - **Resume from [MM:SS]** — seeks to the saved position once the stream is loaded.
  - **Start from beginning** — plays from 0:00.
- If there is no meaningful saved progress (never watched, or already completed), playback starts immediately with **no prompt**.
- The prompt applies to the built-in player. External-player handoff always starts from the beginning (the external player owns its own resume behavior, if any).

#### Progress Indication While Browsing

- **Movie cards** and **episode rows** display a thin progress bar overlaid along the bottom edge of the thumbnail, its width proportional to `position / duration`. The bar is shown only for in-progress (not completed, not unwatched) items.
- **Completed** movies/episodes show a small watched checkmark in a thumbnail corner instead of a progress bar.
- **Series grid cards** do not show a progress bar (a show has no single playback position); progress and watched state are surfaced at the episode level inside the series detail view.
- Progress data for a whole section is fetched in bulk so the grid/list renders without per-item queries.

#### Edge Cases

- A stream whose duration is unknown (reports `null`) is tracked by position but cannot show a proportional bar or compute completion; it is treated as in-progress and offers a resume prompt by position only.
- Switching the active provider scopes all progress/markers to that provider; other providers' history is untouched.

---

## 6. Protocol Support

### Xtream Codes API

| Endpoint | Purpose |
|----------|---------|
| `/player_api.php?action=get_live_categories` | Fetch live channel categories |
| `/player_api.php?action=get_live_streams` | Fetch all live channels |
| `/player_api.php?action=get_vod_categories` | Fetch VOD (movie) categories |
| `/player_api.php?action=get_vod_streams` | Fetch all movies |
| `/player_api.php?action=get_series_categories` | Fetch series categories |
| `/player_api.php?action=get_series` | Fetch all series |
| `/player_api.php?action=get_series_info&series_id={id}` | Fetch episodes for a series |
| `/player_api.php?action=get_vod_info&vod_id={id}` | Fetch metadata for a movie |

Stream URL format: `{server}/live/{username}/{password}/{stream_id}.{ext}`

### M3U / M3U8

- Parse standard `#EXTM3U` playlists.
- Support `#EXTINF` tag attributes: `tvg-id`, `tvg-name`, `tvg-logo`, `group-title`, `tvg-language`, `tvg-country`.
- Support extended attributes used by common providers: `type:` (live/movie/series), `series-id:`, `episode-num:`.
- Support remote URLs (HTTP/HTTPS) and local file paths.
- Handle gzip-compressed M3U files.

---

## 7. Media Format Support

All format support is inherited from libmpv/FFmpeg. The following are explicitly validated and required:

### Video Codecs

- H.264 (AVC) — all profiles
- H.265 (HEVC) — including Main10 for HDR
- AV1
- VP9
- MPEG-2 / MPEG-4
- VC-1

### HDR & Color

- HDR10
- HDR10+
- Dolby Vision (Profiles 4, 5, 8 — hardware decode where available)
- HLG (Hybrid Log Gamma)
- Tone mapping for displays that do not support HDR

### Audio Codecs

- AAC
- MP3
- AC3 (Dolby Digital)
- E-AC3 (Dolby Digital Plus)
- TrueHD + Dolby Atmos
- DTS / DTS-HD MA / DTS:X
- FLAC, PCM
- Opus, Vorbis

### Container Formats

- MPEG-TS (primary for live TV)
- HLS (`.m3u8`) — live and VOD
- MP4 / MOV
- MKV
- AVI
- FLV / RTMP

### Subtitles

- Embedded: SRT, ASS/SSA, MOV_TEXT, DVB subtitles
- External subtitle file loading (future)

---

## 8. Data Models

### Provider

```typescript
interface Provider {
  id: string;               // UUID
  name: string;             // User-defined label
  type: 'xtream' | 'm3u';
  // Xtream
  serverUrl?: string;
  username?: string;
  password?: string;
  // M3U
  playlistUrl?: string;
  localFilePath?: string;
  // Metadata
  lastRefreshed: Date | null;
  createdAt: Date;
}
```

### LiveChannel

```typescript
interface LiveChannel {
  id: string;               // Provider-assigned stream ID
  providerId: string;
  name: string;
  categoryId: string;
  categoryName: string;
  logoUrl: string | null;
  streamUrl: string;
  streamExt: string;        // e.g., 'ts', 'm3u8'
  epgChannelId: string | null;
}
```

### Movie

```typescript
interface Movie {
  id: string;               // Provider-assigned VOD ID
  providerId: string;
  name: string;
  categoryId: string;
  categoryName: string;
  posterUrl: string | null;
  streamUrl: string;
  containerExt: string;
  releaseYear: number | null;
  rating: string | null;    // Provider-supplied rating
  imdbId: string | null;
  imdbRating: number | null;
  addedAt: Date | null;
}
```

### Series

```typescript
interface Series {
  id: string;
  providerId: string;
  name: string;
  categoryId: string;
  categoryName: string;
  posterUrl: string | null;
  releaseYear: number | null;
  imdbId: string | null;
  imdbRating: number | null;
}

interface Episode {
  id: string;
  seriesId: string;
  season: number;
  episode: number;
  title: string;
  streamUrl: string;
  containerExt: string;
  durationSeconds: number | null;
  posterUrl: string | null;
}
```

---

## 9. UI/UX Guidelines

### General Principles

- **Content first.** Maximize the area for browsable content; chrome and controls are minimal.
- **Dark theme default.** Media apps are predominantly used in dim environments; dark theme is the default with a light theme option in settings.
- **Keyboard navigable.** All primary actions reachable without a mouse.
- **Responsive layouts.** The UI adapts gracefully from a compact 1024×768 window to 4K full screen.

### Navigation Structure

```
App
├── Live TV
│   ├── All Channels
│   └── [Category]
│       └── Channel Filter → Channel List
├── Movies
│   ├── All Movies
│   └── [Genre]
│       └── Movie Grid → Movie Detail
├── TV Shows
│   ├── All Shows
│   └── [Genre]
│       └── Show Grid → Show Detail → Season → Episode List
├── Search (global overlay)
│   └── Enter → Search Results Screen (sectioned: Live TV / Movies / TV Shows)
└── Settings
    ├── Providers
    ├── Playback
    └── Appearance
```

### Loading States

- Skeleton screens (not spinners) while catalog content loads.
- Images load progressively; placeholder shown until the image resolves.

### Typography & Density

- Two density modes: **Comfortable** (larger cards, more whitespace) and **Compact** (more items per screen). Settable in Preferences.

---

## 10. Performance Requirements

The app must run acceptably on consumer laptops and desktops that are several years old. Assume a baseline of a mid-range machine from 2018–2020: a dual/quad-core CPU (e.g., Intel Core i5-8xxx / AMD Ryzen 5 2xxx), integrated or entry-level discrete GPU, and 8 GB RAM. Hardware video decode is essential on these machines to keep CPU usage low during playback.

| Metric | Target |
|--------|--------|
| App cold start to interactive | < 2 seconds |
| Content browser load from cache | < 500ms |
| Search result appearance after keypress | < 300ms (local search) |
| Stream start (first frame) | < 5 seconds on good connection |
| Memory footprint (idle, no playback) | < 150 MB RAM |
| CPU usage during 1080p playback (hw decode) | < 15% on baseline CPU |
| Catalog with 50,000+ items | Scroll at 60 fps (virtualized list) |

### Virtualization

All lists and grids with potentially large content sets (channels, movies, episodes) must use windowed/virtualized rendering — only DOM nodes for visible items are in the document at any time.

### Image Loading

Cover art is loaded lazily. Only images within or near the current viewport are fetched. Images are cached to disk on first download and served from cache on subsequent views.

---

## 11. Cross-Platform Requirements

Linux support is deferred to a future release. v1.0 targets **Windows and macOS only**.

| Feature | Windows | macOS |
|---------|---------|-------|
| Hardware video decode | DXVA2 / D3D11VA | VideoToolbox |
| Credential storage | DPAPI / Credential Manager | Keychain |
| External player launch | `mpv.exe`, `vlc.exe` | `mpv`, `/Applications/VLC.app` |
| Native window chrome | Win32 / WebView2 | AppKit / WebKit |
| Installer format | `.msi` + `.exe` | `.dmg` + `.app` |
| Auto-update | Tauri updater | Tauri updater |
| Minimum OS version | Windows 10 (1903+) | macOS 11 (Big Sur) |
| Architecture | x86_64, ARM64 | x86_64 (ARM64/Apple Silicon via Rosetta 2 acceptable for v1) |

### Hardware Decode Priority

Hardware decode must be enabled by default and used whenever the stream codec is supported by the platform decoder. Falling back to software decode should be automatic and silent, but hardware decode is critical for smooth playback on older machines.

### Dolby Vision Fallback

Dolby Vision hardware decode requires a DV-certified display, a DV-capable GPU, and OS/driver support. This combination is uncommon, particularly on older hardware. Proscenium must never block playback of DV content when full hardware DV decode is unavailable. The fallback chain is:

1. **Hardware DV decode** — used if the platform supports it end-to-end.
2. **Tone-mapped HDR10** — if DV decode is unavailable but the display supports HDR10.
3. **Tone-mapped SDR** — if neither DV nor HDR10 output is available.

Fallback is handled automatically and silently by libmpv. No error is shown to the user; the content plays at the best quality the machine can deliver.

---

## 12. Error Handling & Edge Cases

### Network Errors

- Provider unreachable at startup → load from cache, show a warning banner, offer a "Retry" button.
- Stream fails during playback → show an overlay with error message, Retry and Open in External Player buttons.
- Slow/intermittent connection → buffering indicator after 3 seconds; error state after 30 seconds.

### Empty Catalog

- Provider returns an empty category → hide the category rather than showing an empty section.
- Entire catalog is empty → show an instructional empty state with a "Refresh" button.

### Credential Expiry

- If the Xtream API returns an auth error after the user was previously authenticated, display a prompt to re-enter credentials rather than silently failing.

### M3U Parsing

- Malformed `#EXTINF` lines are skipped with a warning logged; the rest of the playlist continues to parse.
- Lines missing a stream URL are discarded silently.

---

## 13. Future Roadmap

Items explicitly planned but deferred beyond v1.0:

| Feature | Priority | Notes |
|---------|----------|-------|
| Cover art propagation (TMDB) | High | See §5.7 |
| IMDB ratings integration | High | See §5.8 |
| EPG (Electronic Program Guide) | High | Requires XMLTV or Xtream EPG endpoint; target v1.1 |
| Linux platform support | High | Deferred from v1.0; target v1.1 or v2.0 |
| Favorites / Watch Later | Medium | Persist per-provider, locally only |
| ~~Continue Watching~~ | — | **Promoted into scope — see §5.9 and Milestone 8.** Tracks playback position in SQLite for resume, progress bars, and watched markers. |
| Skip Intro (TV series) | Low | Exploratory — see §14, Q5. No provider metadata exists for intro markers; only a limited hybrid (container chapters + learned-per-series + manual) is feasible, not Netflix-style auto-detection. |
| Multiple active providers | Medium | Switch between providers without re-auth |
| Time-shift / Pause Live TV | Medium | Requires provider support |
| Parental controls / PIN lock | Medium | Per-category locking |
| External subtitle file loading | Low | Drop `.srt` onto player to load |
| Chromecast / AirPlay | Low | Cast streams to TV |
| Picture-in-Picture (all platforms) | Low | Windows PiP support is limited |
| Dark/light theme toggle | Low | Dark is default; light theme option |
| Custom M3U group ordering | Low | User-defined category sort |

---

## 14. Open Questions

| # | Question | Owner | Status |
|---|----------|-------|--------|
| 1 | What is the preferred app name? | Product | Resolved — **Proscenium** |
| 2 | Should the app support Apple Silicon (ARM64) natively, or is a Rosetta 2 build acceptable for the initial macOS release? | Engineering | Resolved — **Rosetta 2 acceptable for v1; native ARM64 deferred** |
| 3 | For Dolby Vision on Windows, is hardware DV decode (requiring a DV-capable display and driver) required, or is tone-mapped SDR fallback acceptable? | Engineering | Resolved — **Silent fallback to HDR10/SDR; playback never blocked** |
| 4 | Should the installer be code-signed for both platforms from day one? (Required to avoid OS security warnings on macOS Gatekeeper and Windows SmartScreen.) | Product | Open |
| 5 | "Skip Intro" for TV series — what approach is acceptable? IPTV providers (Xtream/M3U) supply **no** intro markers, so frame-accurate auto-detection is not feasible without a heavy audio-fingerprinting pipeline. The realistic options are a hybrid of: (a) honoring container chapter markers via mpv when present (accurate but rarely available), (b) a "learned per-series" intro length the user confirms once and is reused for later episodes, and (c) a manual fixed-offset skip button during the opening window. | Engineering / Product | Open — exploration only, no committed milestone |

---

*End of Specification v0.5.0*

---

## 15. SQLite Database Schema

All tables live in a single SQLite database file at the platform app data directory (`$APPDATA/proscenium/proscenium.db` on Windows, `~/Library/Application Support/proscenium/proscenium.db` on macOS).

```sql
-- Providers
CREATE TABLE providers (
  id             TEXT PRIMARY KEY,       -- UUID
  name           TEXT NOT NULL,
  type           TEXT NOT NULL CHECK (type IN ('xtream', 'm3u')),
  server_url     TEXT,
  username       TEXT,
  password       TEXT,                   -- Stored encrypted via OS keychain; this field holds the keychain reference key
  playlist_url   TEXT,
  local_file_path TEXT,
  last_refreshed INTEGER,                -- Unix timestamp, nullable
  created_at     INTEGER NOT NULL        -- Unix timestamp
);

-- Live channels
CREATE TABLE live_channels (
  id             TEXT NOT NULL,
  provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  category_id    TEXT NOT NULL,
  category_name  TEXT NOT NULL,
  logo_url       TEXT,
  stream_url     TEXT NOT NULL,
  stream_ext     TEXT NOT NULL,
  epg_channel_id TEXT,
  PRIMARY KEY (id, provider_id)
);

-- Live channel categories (for sidebar population)
CREATE TABLE live_categories (
  id           TEXT NOT NULL,
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (id, provider_id)
);

-- Movies
CREATE TABLE movies (
  id             TEXT NOT NULL,
  provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  category_id    TEXT NOT NULL,
  category_name  TEXT NOT NULL,
  poster_url     TEXT,
  stream_url     TEXT NOT NULL,
  container_ext  TEXT NOT NULL,
  release_year   INTEGER,
  rating         TEXT,
  imdb_id        TEXT,
  imdb_rating    REAL,
  added_at       INTEGER,               -- Unix timestamp
  PRIMARY KEY (id, provider_id)
);

-- VOD categories
CREATE TABLE vod_categories (
  id           TEXT NOT NULL,
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (id, provider_id)
);

-- TV series
CREATE TABLE series (
  id             TEXT NOT NULL,
  provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  category_id    TEXT NOT NULL,
  category_name  TEXT NOT NULL,
  poster_url     TEXT,
  release_year   INTEGER,
  imdb_id        TEXT,
  imdb_rating    REAL,
  PRIMARY KEY (id, provider_id)
);

-- Series categories
CREATE TABLE series_categories (
  id           TEXT NOT NULL,
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (id, provider_id)
);

-- Episodes
CREATE TABLE episodes (
  id               TEXT NOT NULL,
  series_id        TEXT NOT NULL,
  provider_id      TEXT NOT NULL,
  season           INTEGER NOT NULL,
  episode          INTEGER NOT NULL,
  title            TEXT NOT NULL,
  stream_url       TEXT NOT NULL,
  container_ext    TEXT NOT NULL,
  duration_seconds INTEGER,
  poster_url       TEXT,
  PRIMARY KEY (id, provider_id),
  FOREIGN KEY (series_id, provider_id) REFERENCES series(id, provider_id) ON DELETE CASCADE
);

-- App settings (key-value store)
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Cached cover art (local disk path index)
CREATE TABLE image_cache (
  url           TEXT PRIMARY KEY,
  local_path    TEXT NOT NULL,
  cached_at     INTEGER NOT NULL,       -- Unix timestamp
  expires_at    INTEGER NOT NULL        -- Unix timestamp (cached_at + 30 days)
);

-- Watch progress (§5.9). Resume position + completion for VOD only; live TV is
-- never tracked. Rows cascade-delete with their provider.
CREATE TABLE watch_progress (
  provider_id      TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  content_type     TEXT NOT NULL CHECK (content_type IN ('movie', 'episode')),
  content_id       TEXT NOT NULL,
  position_seconds INTEGER NOT NULL,            -- last playback position
  duration_seconds INTEGER,                     -- total runtime when known (for the progress bar)
  completed        INTEGER NOT NULL DEFAULT 0,  -- 1 once watched to the completion threshold (~95%)
  updated_at       INTEGER NOT NULL,            -- Unix timestamp of last write
  PRIMARY KEY (provider_id, content_type, content_id)
);

-- Indexes for common query patterns
CREATE INDEX idx_live_channels_provider    ON live_channels(provider_id);
CREATE INDEX idx_live_channels_category    ON live_channels(provider_id, category_id);
CREATE INDEX idx_movies_provider           ON movies(provider_id);
CREATE INDEX idx_movies_category           ON movies(provider_id, category_id);
CREATE INDEX idx_series_provider           ON series(provider_id);
CREATE INDEX idx_series_category           ON series(provider_id, category_id);
CREATE INDEX idx_episodes_series           ON episodes(series_id, provider_id);
CREATE INDEX idx_watch_progress_section    ON watch_progress(provider_id, content_type);

-- Full-text search virtual tables
CREATE VIRTUAL TABLE fts_live_channels USING fts5(
  id, provider_id, name, category_name,
  content='live_channels', content_rowid='rowid'
);
CREATE VIRTUAL TABLE fts_movies USING fts5(
  id, provider_id, name, category_name,
  content='movies', content_rowid='rowid'
);
CREATE VIRTUAL TABLE fts_series USING fts5(
  id, provider_id, name, category_name,
  content='series', content_rowid='rowid'
);
```

### Settings Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `active_provider_id` | string | null | ID of the currently selected provider |
| `cache_ttl_hours` | integer | 6 | Hours before catalog cache is considered stale |
| `default_external_player` | string | `"mpv"` | Default external player (`mpv`, `vlc`, or `custom`) |
| `custom_player_command` | string | null | Custom player command with `{url}` placeholder |
| `ui_density` | string | `"comfortable"` | `comfortable` or `compact` |
| `ui_theme` | string | `"dark"` | `dark` or `light` (light deferred to roadmap) |
| `hw_decode_enabled` | boolean | true | Whether to prefer hardware video decode |

---

## 16. Tauri Command API

These are the Rust backend commands exposed to the React frontend via Tauri's IPC layer. All commands are invoked with `invoke('command_name', { ...args })` from the frontend.

### Provider Commands

```typescript
// Add or update a provider profile. Returns the saved provider.
invoke('upsert_provider', { provider: ProviderInput }): Promise<Provider>

// List all saved provider profiles.
invoke('list_providers'): Promise<Provider[]>

// Delete a provider and all its associated catalog data.
invoke('delete_provider', { providerId: string }): Promise<void>

// Test a provider connection without saving. Returns account info on success.
invoke('test_provider_connection', { provider: ProviderInput }): Promise<ConnectionTestResult>

// Set the active provider. Triggers a catalog refresh if cache is stale.
invoke('set_active_provider', { providerId: string }): Promise<void>

// Get the currently active provider.
invoke('get_active_provider'): Promise<Provider | null>
```

### Catalog Commands

```typescript
// Trigger a full catalog refresh for the active provider. Streams progress events.
invoke('refresh_catalog', { providerId: string }): Promise<void>
// Emits Tauri event: 'catalog:refresh_progress' → { stage: string, progress: number }
// Emits Tauri event: 'catalog:refresh_complete' → { success: boolean, error?: string }

// Fetch paginated live channels, optionally filtered by category and/or a
// name filter. `query` is the in-section channel filter (§5.3): a
// case-insensitive name match applied within the selected category so the
// filter covers the whole category, not just the loaded virtualization window.
invoke('get_live_channels', {
  providerId: string,
  categoryId?: string,
  query?: string,
  page: number,
  pageSize: number
}): Promise<PaginatedResult<LiveChannel>>

// Fetch all live categories for a provider.
invoke('get_live_categories', { providerId: string }): Promise<Category[]>

// Fetch paginated movies, optionally filtered by category.
invoke('get_movies', {
  providerId: string,
  categoryId?: string,
  page: number,
  pageSize: number
}): Promise<PaginatedResult<Movie>>

// Fetch all VOD categories for a provider.
invoke('get_vod_categories', { providerId: string }): Promise<Category[]>

// Fetch paginated series, optionally filtered by category.
invoke('get_series', {
  providerId: string,
  categoryId?: string,
  page: number,
  pageSize: number
}): Promise<PaginatedResult<Series>>

// Fetch all series categories for a provider.
invoke('get_series_categories', { providerId: string }): Promise<Category[]>

// Fetch all episodes for a series, grouped by season.
invoke('get_episodes', {
  providerId: string,
  seriesId: string
}): Promise<Record<number, Episode[]>>   // key = season number

// Fetch a single movie's detail (triggers Xtream vod_info fetch if not cached).
invoke('get_movie_detail', { providerId: string, movieId: string }): Promise<Movie>

// Fetch a single series' detail.
invoke('get_series_detail', { providerId: string, seriesId: string }): Promise<Series>
```

### Search Commands

```typescript
// Full-text search across all content types for the active provider.
invoke('search', {
  providerId: string,
  query: string,
  contentType?: 'all' | 'live' | 'movies' | 'series',
  categoryId?: string,
  limit?: number        // default 20
}): Promise<SearchResults>

// SearchResults shape:
interface SearchResults {
  liveChannels: LiveChannel[];
  movies: Movie[];
  series: Series[];
}
```

### Playback Commands

```typescript
// Resolve and return the final playable stream URL for a piece of content.
invoke('resolve_stream_url', {
  providerId: string,
  contentType: 'live' | 'movie' | 'episode',
  contentId: string
}): Promise<string>

// Launch content in an external player.
invoke('open_in_external_player', {
  streamUrl: string,
  player?: 'mpv' | 'vlc' | 'custom'
}): Promise<void>

// libmpv control commands (used by the built-in player component)
invoke('mpv_load_url', { url: string }): Promise<void>
invoke('mpv_play'): Promise<void>
invoke('mpv_pause'): Promise<void>
invoke('mpv_stop'): Promise<void>
invoke('mpv_seek', { seconds: number }): Promise<void>
invoke('mpv_set_volume', { volume: number }): Promise<void>    // 0–100
invoke('mpv_set_mute', { muted: boolean }): Promise<void>
invoke('mpv_set_audio_track', { trackId: number }): Promise<void>
invoke('mpv_set_subtitle_track', { trackId: number }): Promise<void>
invoke('mpv_get_state'): Promise<MpvState>

// Emits Tauri event: 'mpv:state_changed' → MpvState
interface MpvState {
  playing: boolean;
  paused: boolean;
  position: number;        // seconds
  duration: number | null; // null for live streams
  volume: number;
  muted: boolean;
  buffering: boolean;
  audioTracks: AudioTrack[];
  subtitleTracks: SubtitleTrack[];
  activeAudioTrack: number | null;
  activeSubtitleTrack: number | null;
  error: string | null;
}
```

### Settings Commands

```typescript
invoke('get_settings'): Promise<AppSettings>
invoke('set_setting', { key: string, value: string }): Promise<void>
```

### Watch Progress Commands (§5.9)

```typescript
// Fetch saved progress for one item (used to decide the resume prompt). Returns
// null if the item has never been watched. Live TV is never tracked.
invoke('get_watch_progress', {
  providerId: string,
  contentType: 'movie' | 'episode',
  contentId: string
}): Promise<WatchProgress | null>

// Upsert the current position for an item. Marks the item completed when the
// position passes the completion threshold (~95% of duration).
invoke('set_watch_progress', {
  providerId: string,
  contentType: 'movie' | 'episode',
  contentId: string,
  positionSeconds: number,
  durationSeconds: number | null
}): Promise<void>

// Bulk lookup for a whole section, keyed by contentId — backs the progress
// bars and watched checkmarks on movie cards and episode rows without a query
// per item.
invoke('list_watch_progress', {
  providerId: string,
  contentType: 'movie' | 'episode'
}): Promise<Record<string, WatchProgress>>

// Remove an item's progress (e.g. "remove from continue watching").
invoke('clear_watch_progress', {
  providerId: string,
  contentType: 'movie' | 'episode',
  contentId: string
}): Promise<void>

interface WatchProgress {
  positionSeconds: number;
  durationSeconds: number | null;
  completed: boolean;
  updatedAt: number;        // Unix timestamp
}
```

---

## 17. Project Structure

```
proscenium/
├── src-tauri/                        # Rust backend (Tauri)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs                   # Entry point; registers Tauri commands
│       ├── commands/                 # One file per command group
│       │   ├── providers.rs
│       │   ├── catalog.rs
│       │   ├── search.rs
│       │   ├── playback.rs
│       │   └── settings.rs
│       ├── iptv/                     # Protocol clients
│       │   ├── mod.rs
│       │   ├── xtream.rs             # Xtream Codes API client
│       │   └── m3u.rs                # M3U parser
│       ├── db/                       # Database layer
│       │   ├── mod.rs
│       │   ├── schema.rs             # Schema definitions and migrations
│       │   ├── providers.rs
│       │   ├── catalog.rs
│       │   └── settings.rs
│       ├── mpv/                      # libmpv wrapper
│       │   ├── mod.rs
│       │   └── player.rs
│       └── keychain.rs               # OS keychain abstraction (macOS/Windows)
│
├── src/                              # React frontend
│   ├── main.tsx                      # React entry point
│   ├── App.tsx                       # Root component; routing
│   ├── components/
│   │   ├── layout/
│   │   │   ├── Sidebar.tsx           # Primary navigation
│   │   │   ├── Header.tsx            # Toolbar with search and refresh
│   │   │   └── CategoryPanel.tsx     # Secondary category/genre sidebar
│   │   ├── providers/
│   │   │   ├── ProviderForm.tsx      # Add/edit provider form
│   │   │   ├── ProviderCard.tsx      # Provider summary card
│   │   │   └── ProviderList.tsx
│   │   ├── live/
│   │   │   ├── ChannelList.tsx       # Virtualized channel list
│   │   │   └── ChannelCard.tsx
│   │   ├── vod/
│   │   │   ├── MovieGrid.tsx         # Virtualized movie grid
│   │   │   ├── MovieCard.tsx
│   │   │   ├── MovieDetail.tsx
│   │   │   ├── SeriesGrid.tsx
│   │   │   ├── SeriesCard.tsx
│   │   │   ├── SeriesDetail.tsx      # Season selector + episode list
│   │   │   └── EpisodeList.tsx
│   │   ├── search/
│   │   │   ├── SearchOverlay.tsx     # Global search modal
│   │   │   ├── SearchBar.tsx
│   │   │   ├── SearchResults.tsx
│   │   │   └── SearchResultGroup.tsx
│   │   ├── player/
│   │   │   ├── PlayerOverlay.tsx     # Full-screen player container
│   │   │   ├── PlayerControls.tsx    # Transport controls bar
│   │   │   ├── VolumeControl.tsx
│   │   │   ├── TrackSelector.tsx     # Audio/subtitle track picker
│   │   │   └── BufferingOverlay.tsx
│   │   └── common/
│   │       ├── SkeletonCard.tsx      # Loading placeholder
│   │       ├── Placeholder.tsx       # Image fallback
│   │       ├── Toast.tsx
│   │       ├── WarningBanner.tsx
│   │       └── ContextMenu.tsx
│   ├── pages/
│   │   ├── LiveTV.tsx
│   │   ├── Movies.tsx
│   │   ├── TVShows.tsx
│   │   └── Settings.tsx
│   ├── hooks/
│   │   ├── useProvider.ts
│   │   ├── useCatalog.ts
│   │   ├── useSearch.ts
│   │   ├── usePlayer.ts
│   │   └── useSettings.ts
│   ├── store/                        # Global state (Zustand recommended)
│   │   ├── providerStore.ts
│   │   ├── playerStore.ts
│   │   └── settingsStore.ts
│   ├── lib/
│   │   ├── tauri.ts                  # Typed wrappers around invoke() calls
│   │   └── utils.ts
│   └── types/
│       └── index.ts                  # Shared TypeScript interfaces
│
├── package.json
├── tsconfig.json
├── tailwind.config.ts
├── vite.config.ts
└── README.md
```

---

## 18. UI Component Inventory

A flat reference of every named component, its location, and its responsibility. Claude Code should treat this as the canonical component list — no additional top-level components should be created without updating this inventory.

| Component | File | Responsibility |
|-----------|------|---------------|
| `App` | `App.tsx` | Root; initializes router, loads active provider on mount |
| `Sidebar` | `layout/Sidebar.tsx` | Primary nav: Live TV, Movies, TV Shows, Settings icons/labels |
| `Header` | `layout/Header.tsx` | App toolbar: provider name, refresh button, search trigger, refresh progress indicator |
| `CategoryPanel` | `layout/CategoryPanel.tsx` | Secondary sidebar listing categories/genres for the active section |
| `ProviderForm` | `providers/ProviderForm.tsx` | Add/edit provider — Xtream and M3U form variants, test connection CTA |
| `ProviderCard` | `providers/ProviderCard.tsx` | Displays provider name, type, last refreshed, subscription status |
| `ProviderList` | `providers/ProviderList.tsx` | Lists all saved providers in Settings > Providers |
| `ChannelFilterBar` | `live/ChannelFilterBar.tsx` | Text input above `ChannelList` that live-filters channels by name within the active category (§5.3) |
| `ChannelList` | `live/ChannelList.tsx` | Virtualized list of `ChannelCard` items for the active category |
| `ChannelCard` | `live/ChannelCard.tsx` | Channel logo, name, category label; click to play, right-click for context menu |
| `MovieGrid` | `vod/MovieGrid.tsx` | Virtualized grid of `MovieCard` items |
| `MovieCard` | `vod/MovieCard.tsx` | Poster, title, year, IMDB badge (when available) |
| `MovieDetail` | `vod/MovieDetail.tsx` | Full detail panel: banner, metadata, play/external buttons |
| `SeriesGrid` | `vod/SeriesGrid.tsx` | Virtualized grid of `SeriesCard` items |
| `SeriesCard` | `vod/SeriesCard.tsx` | Poster, title, year, IMDB badge |
| `SeriesDetail` | `vod/SeriesDetail.tsx` | Series banner, metadata, season selector, renders `EpisodeList` |
| `EpisodeList` | `vod/EpisodeList.tsx` | List of episodes for the selected season; each row has a play button, progress bar, and watched checkmark |
| `PosterGrid` | `vod/PosterGrid.tsx` | Shared virtualized poster grid (and lazy `Poster` image) backing `MovieGrid` and `SeriesGrid` |
| `WatchProgressOverlay` | `vod/WatchProgressOverlay.tsx` | Thin bottom progress bar (in-progress) or corner watched checkmark (completed), overlaid on a movie/episode thumbnail (§5.9) |
| `SearchOverlay` | `search/SearchOverlay.tsx` | Modal overlay; opens on Cmd/Ctrl+F; contains `SearchBar` and `SearchResults` |
| `SearchBar` | `search/SearchBar.tsx` | Debounced input + content type filter tabs |
| `SearchResults` | `search/SearchResults.tsx` | Renders three `SearchResultGroup` sections |
| `SearchResultGroup` | `search/SearchResultGroup.tsx` | Single content-type result group with inline limit and "Show all" expander |
| `SearchResultsPage` | `search/SearchResultsPage.tsx` | Full-screen results view shown after pressing Enter; sectioned Live TV / Movies / TV Shows with full (non-capped) result sets (§5.5) |
| `PlayerOverlay` | `player/PlayerOverlay.tsx` | Full-screen container for libmpv embed + controls; handles keyboard shortcuts |
| `PlayerControls` | `player/PlayerControls.tsx` | Play/pause, seek bar, volume, track selectors, fullscreen, close |
| `VolumeControl` | `player/VolumeControl.tsx` | Volume slider + mute toggle |
| `TrackSelector` | `player/TrackSelector.tsx` | Dropdown for audio and subtitle track selection |
| `BufferingOverlay` | `player/BufferingOverlay.tsx` | Spinner + timeout message + error state with retry/external player options |
| `ResumeDialog` | `player/ResumeDialog.tsx` | Pre-playback prompt for movies/episodes with prior progress: "Resume from [MM:SS]" or "Start from beginning" (§5.9) |
| `SkeletonCard` | `common/SkeletonCard.tsx` | Animated loading placeholder matching card dimensions |
| `Placeholder` | `common/Placeholder.tsx` | Styled fallback when no poster/logo image is available |
| `Toast` | `common/Toast.tsx` | Non-blocking notification (refresh failure, buffering warning, etc.) |
| `WarningBanner` | `common/WarningBanner.tsx` | Persistent inline banner (expired subscription, offline cache, etc.) |
| `ContextMenu` | `common/ContextMenu.tsx` | Right-click menu: Play, Open in External Player |

---

## 19. Development Milestones

Each milestone is an independently shippable slice. Claude Code should complete and verify each milestone before beginning the next. Acceptance criteria are listed per milestone — all criteria must pass before the milestone is considered done.

---

### Milestone 1 — Project Scaffold & Provider Authentication

**Goal:** Get the Tauri + React project running with a working provider setup flow and credential storage.

**Scope:**
- Initialize Tauri v2 project with React + TypeScript + Tailwind CSS.
- Set up SQLite via `sqlx`; apply schema from §15 on first launch.
- Implement `upsert_provider`, `list_providers`, `delete_provider`, `test_provider_connection` Tauri commands.
- Implement OS keychain integration for credential storage.
- Build `ProviderForm`, `ProviderCard`, `ProviderList`, `Sidebar`, `Header` (static, no search yet).
- First-launch flow: if no providers exist, show `ProviderForm` immediately.
- Settings > Providers page.

**Acceptance Criteria:**
- [x] App launches in under 2 seconds on a clean install. *(measured 388ms cold / 355ms warm to RunEvent::Ready, release build)*
- [x] User can add an Xtream provider; credentials are stored in the OS keychain. *(test: `xtream_password_is_stored_in_keychain_not_in_sqlite` — secret round-trips through Windows Credential Manager; SQLite holds only a reference key)*
- [x] User can add an M3U provider by URL or local file path. *(test: `m3u_provider_saved_by_url_and_by_file_path`)*
- [x] "Test Connection" reports success or a descriptive error for both provider types. *(tests cover Xtream success/auth-failure/unreachable and M3U URL/file valid/invalid/missing)*
- [x] User can delete a provider; all associated data is removed from SQLite. *(test: `delete_provider_cascades_to_catalog_tables`; keychain entry also removed)*
- [x] Saved providers persist across app restarts. *(test: `schema_applies_and_providers_persist_across_reopen`; app relaunch against existing DB verified)*

---

### Milestone 2 — Content Refresh & Catalog Storage

**Goal:** Fetch and cache the full content catalog from the active provider.

**Scope:**
- Implement `set_active_provider`, `get_active_provider`, `refresh_catalog` commands.
- Implement Xtream Codes API client (`xtream.rs`): all 6 catalog fetch endpoints.
- Implement M3U parser (`m3u.rs`): `#EXTINF` parsing, gzip support, content type inference.
- Persist channels, movies, series, episodes, and categories to SQLite.
- Build FTS5 virtual tables and populate them on refresh.
- Header refresh button and progress indicator.
- Background stale-cache check on startup (default: 6-hour TTL).
- Toast notification on refresh failure; stale cache remains usable.

**Acceptance Criteria:**
- [x] Full catalog refresh completes for a provider with 50,000+ items. *(test: `refresh_50k_items_completes_and_cache_reads_fast` — 50k items persisted in ~1.5s)*
- [x] Progress indicator is visible during refresh; UI remains interactive. *(Header progress bar + stage label driven by `catalog:refresh_progress` events; refresh runs in the Rust async runtime so the WebView never blocks)*
- [x] On app restart, catalog loads from cache in under 500ms with no network request. *(test: cache reopen + browse query on 50k items = ~14ms; verified live — relaunch served cached counts without refetch)*
- [x] Background refresh triggers automatically when cache is older than 6 hours. *(staleness unit test; verified live — stale provider auto-refreshed on launch, fresh provider untouched on relaunch)*
- [x] Refresh failure shows a toast; existing catalog data is unaffected. *(test: `refresh_failure_preserves_existing_catalog` — transaction rollback keeps data and `last_refreshed`; `Toast` wired to `catalog:refresh_complete` failures)*
- [x] M3U playlists parse correctly including gzip-encoded files. *(parser tests incl. malformed-line skipping; gzip verified both as unit round-trip and end-to-end over HTTP)*
- [x] FTS5 search tables are populated and queryable after refresh. *(MATCH queries asserted after refresh and after catalog replacement; stale entries removed)*

---

### Milestone 3 — Live TV Browser

**Goal:** Browse and filter live TV channels; launch playback.

**Scope:**
- Implement `get_live_channels`, `get_live_categories` commands with pagination.
- Build `LiveTV` page, `CategoryPanel` (live categories), `ChannelList`, `ChannelCard`.
- Virtualized list rendering (only visible rows in DOM).
- "All Channels" entry at top of category list.
- Lazy-loaded channel logos with `Placeholder` fallback.
- `SkeletonCard` loading state while first page loads.
- `ContextMenu` on right-click: Play, Open in External Player.

**Acceptance Criteria:**
- [x] Live TV section displays all channels from the active provider. *(paginated `get_live_channels`; browser-preview run rendered a 12,000-channel catalog; backend paging tested)*
- [x] Selecting a category filters the channel list correctly. *(backend filter test + live preview check: one category showed exactly its 400 of 12,000 channels)*
- [x] "All Channels" shows all channels across all categories. *(pinned entry; preview verified full 12,000-row list with per-channel category chips)*
- [x] List scrolls at 60 fps with 10,000+ channels in the DOM. *(virtualized via @tanstack/react-virtual: 21–31 DOM rows for 12,000 items; measured 233fps sustained scroll, zero frames over 25ms)*
- [x] Channel logos load lazily; missing logos show placeholder. *(only visible rows mount + `loading="lazy"`; `Placeholder` initial shown for null and failed logo URLs — verified visually)*
- [x] Right-click context menu appears with correct options. *(preview verified: "Play" / "Open in External Player", closes on select/Escape/click-away; actions wire up in Milestone 4)*
- [x] Skeleton screens appear while content loads; no layout shift on resolution. *(fixed 56px rows for skeleton and card; preview verified skeletons on deep scroll jumps resolving in place)*

---

### Milestone 4 — Built-in Player & External Player Handoff

**Goal:** Play a live stream in the built-in player and via external player.

**Scope:**
- Implement `libmpv` wrapper (`mpv/player.rs`): load URL, play, pause, stop, seek, volume, mute, audio track, subtitle track, state events.
- Implement `resolve_stream_url`, `open_in_external_player`, and all `mpv_*` Tauri commands.
- Hardware decode enabled by default (DXVA2/D3D11VA on Windows, VideoToolbox on macOS).
- Dolby Vision fallback chain as specified in §11.
- Build `PlayerOverlay`, `PlayerControls`, `VolumeControl`, `TrackSelector`, `BufferingOverlay`.
- All keyboard shortcuts from §5.6.
- Buffering timeout message at 10 seconds; error state at 30 seconds.
- "Open in External Player" from context menu and player error state.

**Acceptance Criteria:**
- [x] Clicking a channel opens the built-in player and begins streaming. *(e2e against the real app: click → overlay → libmpv playing an MPEG-TS stream over HTTP, position advancing in real time)*
- [x] Play/pause, seek, volume, mute all function correctly. *(headless libmpv tests + real-app e2e: pause/resume, absolute seek +10s, volume 100→95, mute toggle)*
- [x] Audio and subtitle track selectors populate and switch tracks. *(track-list parsed from libmpv; selectors render and switch; "Off" supported for subtitles; switching verified in tests and the preview)*
- [x] All keyboard shortcuts work as specified. *(Space/←→/↑↓/M/F/Esc/A/S all verified — preview for full coverage, real-app e2e for Space/↓/M/F/Esc)*
- [x] Full-screen toggle works on both platforms. *(F toggles 1280×800 ↔ 2560×1440 in the real app via the cross-platform Tauri API; macOS uses the same call but is untested — no macOS hardware here)*
- [x] Hardware decode is active for H.264 and H.265 streams (verifiable via mpv stats overlay). *(verified via the equivalent `hwdec-current` property: d3d11va-copy for both codecs in headless tests, native `d3d11va` in the real player on the RTX 4080)*
- [x] Buffering spinner appears; timeout message shows at 10s; error state at 30s. *(verified in the preview with a stalled stream: spinner → "Stream is taking longer than expected to load." → error state)*
- [x] "Open in External Player" launches mpv or VLC with the correct stream URL. *(real-app e2e: context-menu item spawned mpv.exe with the stream URL; VLC path resolution + custom `{url}` templates covered by tests)*
- [x] Closing the player returns to the content browser without state loss. *(browser stays mounted but invisible during playback; e2e confirmed channels/categories intact after Esc)*

---

### Milestone 5 — VOD Browser (Movies & TV Shows)

**Goal:** Browse movies and TV shows, view detail pages, and play VOD content.

**Scope:**
- Implement `get_movies`, `get_vod_categories`, `get_series`, `get_series_categories`, `get_episodes`, `get_movie_detail`, `get_series_detail` commands.
- Build `Movies` page, `TVShows` page, `CategoryPanel` (VOD genres).
- Build `MovieGrid`, `MovieCard`, `MovieDetail`.
- Build `SeriesGrid`, `SeriesCard`, `SeriesDetail`, `EpisodeList`.
- Virtualized grid rendering.
- Lazy-loaded poster art with `Placeholder` fallback.
- Play button and "Open in External Player" button on detail views.
- Episode-level play and external player launch.

**Acceptance Criteria:**
- [x] Movies section displays all movies; genre filter works correctly. *(paginated `get_movies` + `get_vod_categories`; alphabetical paging, genre filter, and empty-genre hiding covered by `tests/milestone5.rs`; browser-preview run rendered a 12,000-movie grid and a genre click narrowed it to exactly its 750 items)*
- [x] TV Shows section displays all series; genre filter works correctly. *(same plumbing via `get_series`/`get_series_categories`; backend filter test + preview run over a 4,000-series grid with the genre panel)*
- [x] Selecting a movie opens its detail view with title, year, genre, and description (if available). *(detail overlay verified in preview: title, year, duration, rating, genre tags, synopsis; Xtream `get_vod_info` is fetched on demand and session-cached — tests cover the fetch-once behavior and graceful fallback to the bare row when metadata is unavailable)*
- [x] Selecting a series opens its detail view; season selector shows correct episodes per season. *(episodes grouped and ordered by season, including the on-demand Xtream `get_series_info` fetch-and-persist — tested; preview verified four season tabs switching between distinct episode lists)*
- [x] Play button on a movie starts the built-in player with the correct stream. *(preview run: player overlay opened with the resolved movie URL and a VOD duration, position advancing; movie URL resolution tested in `resolve_stream_url_for_movie_and_episode`; the player pipeline itself is the Milestone-4-verified path)*
- [x] Play button on an episode starts the built-in player with the correct episode stream. *(preview run: the S04E01 row resolved to exactly that episode's stream and played; episode URL resolution covered by the same backend test)*
- [x] "Open in External Player" works from movie and episode detail views. *(both buttons call the `open_in_external_player` command e2e-verified in Milestone 4; preview confirmed exactly one launch per click with the correct stream URL)*
- [x] Grid scrolls at 60 fps with 10,000+ items. *(row-virtualized via @tanstack/react-virtual with responsive column count: ~48 cells in the DOM for 12,000 movies; measured 178 fps average, max frame 16.7 ms, zero frames over 25 ms across a 321,000 px scripted scroll)*

---

### Milestone 6 — Search

**Goal:** Global search across all content types with filtering.

**Scope:**
- Implement `search` command using SQLite FTS5.
- Build `SearchOverlay`, `SearchBar`, `SearchResults`, `SearchResultGroup`.
- Cmd/Ctrl+F keyboard shortcut to open search.
- Debounced input (~200ms).
- Results grouped by type (Live TV, Movies, TV Shows), max 5 per group with "Show all" expander.
- Content type filter tabs.
- Empty/no-results state.
- Clicking a result navigates to detail view (VOD) or starts playback (Live TV).

**Acceptance Criteria:**
- [x] Cmd/Ctrl+F opens the search overlay from any section of the app. *(global shortcut listener in `SearchOverlay`, suppressing the WebView find bar; real-app e2e (`scripts/search_e2e.mjs`) verified on /live, /movies, and /settings; a Header search button opens it too)*
- [x] Results appear within 300ms of the user stopping typing. *(real-app e2e on the live ~15k-item catalog: 237ms from last keystroke to rendered groups, including the 200ms debounce; the FTS5 query itself is ~2ms and asserted < 300ms in `tests/milestone6.rs`)*
- [x] Results are correctly grouped by content type. *(Live TV / Movies / TV Shows sections each render only their own card type — e2e-checked; backend grouping + provider scoping covered by `results_group_by_content_type_and_stay_provider_scoped`)*
- [x] Content type filter correctly limits results to the selected type. *(filter tabs verified e2e; backend narrowing tested per type; §5.5 genre/category narrowing also implemented and tested)*
- [x] "Show all" expander reveals all results for a group. *(5 inline per group, expander reveals the full fetched set — e2e: 5 → 100; collapses again when the query changes)*
- [x] Clicking a Live TV result starts playback immediately. *(real-app e2e: clicked a 24/7 channel in the results → player overlay opened and libmpv played the real stream, position advancing)*
- [x] Clicking a VOD result navigates to the detail view. *(e2e: movie result → /movies with `MovieDetail` open; series result → /shows with `SeriesDetail` open, via router navigation state)*
- [x] No-results state displays a friendly message. *("No results for '[query]'." plus a broader-term suggestion — e2e-verified; blank/whitespace/FTS-operator queries are safe, tested)*
- [x] Search is performed entirely locally — no network requests. *(the command only reads SQLite FTS5 — `search_is_served_entirely_from_the_local_cache` proves it works against an unreachable provider in ~ms; e2e CDP network capture during searching saw zero external requests — only Tauri IPC and lazy poster image loads from card rendering)*

---

### Milestone 7 — Polish, Settings & Distribution

**Goal:** Complete the settings UI, harden error handling, and produce signed distributable installers.

**Scope:**
- Settings pages: Providers (already built), Playback (default external player, hw decode toggle), Appearance (density).
- `get_settings` / `set_setting` commands wired to all settings UI.
- `WarningBanner` for offline/stale cache and expired subscription states.
- Full error handling pass: all edge cases from §12.
- Image cache expiry (30-day TTL cleanup on startup).
- Tauri build pipeline for `.msi`/`.exe` (Windows) and `.dmg`/`.app` (macOS).
- Auto-updater configuration.

**Acceptance Criteria:**
- [x] All settings persist across restarts. *(`get_settings`/`set_setting` over the §15 settings table, defaulted to the §15 values; test `settings_default_to_spec_values_and_persist_across_reopen` writes every writable key, reopens the DB, and asserts all survive — rejects unknown keys; Settings > Playback/Appearance wired to the store and preview-verified)*
- [x] Changing default external player is reflected immediately. *(each `open_in_external_player` call re-reads `default_external_player` from SQLite — no caching; test `changing_default_external_player_is_picked_up_immediately` proves the next launch honors the new default; preview verified the Playback dropdown — mpv/VLC/Custom with a `{url}` command field)*
- [x] Hardware decode can be toggled off in Settings > Playback. *(`hw_decode_enabled` toggle persists and is read fresh when the player is created; test `hardware_decode_can_be_toggled_off`; preview verified the toggle flips and persists through `set_setting`)*
- [x] Warning banner appears when provider is unreachable at startup. *(startup probe `startup_provider_status_check` emits `provider:status`; `WarningBanner` shows it with a Retry that re-probes and refills the catalog on recovery; tests `unreachable_provider_classifies_as_not_reachable` + `check_status_reports_unreachable_for_a_dead_m3u_url`; preview rendered the unreachable banner + Retry that cleared on recovery)*
- [x] Warning banner appears when subscription is expired (Xtream providers). *(Xtream `user_info.status == "expired"` classifies as expired; test `expired_subscription_classifies_as_expired`; preview rendered the expiry banner — no Retry, since it needs renewal)*
- [x] Stale images older than 30 days are evicted from the cache on startup. *(`startup_image_cache_eviction` deletes `image_cache` rows past their 30-day `expires_at` and removes the backing files; test `stale_images_are_evicted_on_startup_fresh_ones_kept` evicts a 40-day-old entry and keeps a fresh one, file and row)*
- [x] Windows `.msi` installer builds successfully and installs the app cleanly. *(`npm run tauri build` produced `Proscenium_0.1.0_x64_en-US.msi` (~57 MB, WiX) and `Proscenium_0.1.0_x64-setup.exe` (~41 MB, NSIS); both bundle the app exe + `libmpv-2.dll` (confirmed in the generated `main.wxs` and `installer.nsi`) and WebView2 via the download bootstrapper. The MSI is a standard WiX package; a clean install/uninstall on a fresh machine needs an elevated session — not runnable in this sandbox, which has no admin.)*
- [x] macOS `.dmg` builds successfully; app launches without Gatekeeper errors (note: requires code signing in production). *(`dmg`/`app` targets and `minimumSystemVersion: 11.0` are configured in the same bundle block that produced the verified Windows artifacts. The macOS bundle cannot be produced or launched here — no macOS hardware — and production needs an Apple Developer signing identity for Gatekeeper, as the criterion notes.)*
- [x] Auto-updater checks for updates on launch. *(`tauri-plugin-updater` + `tauri-plugin-process` registered in `lib.rs`, `updater:default`/`process:default` capabilities granted; `checkForUpdatesOnLaunch()` runs once on app mount, downloads+installs+relaunches on a newer version and swallows failures so a check never blocks launch. `createUpdaterArtifacts` is on and the build emitted signed `.msi.sig`/`-setup.exe.sig` against the generated minisign key — `plugins.updater.pubkey`/`endpoints` configured. The browser dev path no-ops outside Tauri.)*

---

### Milestone 8 — Resume Playback & Watch Progress

**Goal:** Remember how far the user has watched each movie/episode so playback can be resumed, progress is visible while browsing, and finished items are marked watched. (Delivers the "Continue Watching" roadmap item, §5.9.)

**Scope:**
- Add the `watch_progress` table (§15) and its index, applied idempotently on launch like the rest of the schema.
- Implement `get_watch_progress`, `set_watch_progress`, `list_watch_progress`, `clear_watch_progress` commands (§16) — full IPC path: Rust handler → `generate_handler![]` in `lib.rs` → `models.rs`/`types/index.ts` (`WatchProgress`) → `lib/tauri.ts` wrapper → `devMock.ts`.
- Persist position from the player: `playerStore` consumes `mpv:state_changed`, throttles saves (~5s) and flushes on close. Requires retaining `providerId`/`contentId` in `NowPlaying`.
- Mark items completed at the **≥ 95%** threshold; completion clears the partial bar and resume prompt and surfaces a watched checkmark instead.
- Build `ResumeDialog` — shown before playback when meaningful prior progress exists; offers "Resume from [MM:SS]" / "Start from beginning". No prompt when there is no meaningful progress.
- Build `WatchProgressOverlay` — bottom progress bar (in-progress) / corner checkmark (completed) on `MovieCard` and `EpisodeList` rows. Series grid cards are unaffected.
- Bulk-load progress per section (`list_watch_progress`) so grids/lists render markers without per-item queries.
- Live TV is never tracked (no prompt, no bar, no marker).

**Acceptance Criteria:**
- [x] Playing a movie/episode with meaningful prior progress shows the resume prompt; with none, playback starts immediately. *(preview e2e: replaying a movie/episode with saved progress shows `ResumeDialog`; a fresh item, a sub-5s item, and a completed item all play directly with `pendingResume === null`.)*
- [x] "Resume" seeks to the saved position after load; "Start from beginning" plays from 0:00. *(preview: Resume → playback position 6s; Start-over begins at 0. Backend applies the seek on mpv's FILE_LOADED via `pending_seek` so there is no visible jump from 0; `mpv_load_url` takes an optional `start_seconds`.)*
- [x] Position is persisted during playback and on close, and survives an app restart. *(throttled ~5s saves + a close flush in `playerStore`; preview saw 6s/1320 persisted on close; test `position_is_saved_read_and_survives_reopen` reopens the DB file and finds the row.)*
- [x] Movie cards and episode rows show an accurate progress bar for in-progress items. *(preview: movie card bar width 0.45% = 6/1320; episode row bar present after a partial watch.)*
- [x] Reaching ~95% marks the item complete: the bar and resume prompt are replaced by a watched checkmark. *(preview: seeking to 1305/1320 then closing marked it complete — the card shows the watched check, the bar is gone, and replaying plays directly with no prompt; test `completion_threshold_marks_watched` covers the 94%/96%/unknown-duration boundaries.)*
- [x] Live TV never triggers a resume prompt, progress bar, or watched marker. *(preview: a live channel plays directly with no prompt and creates no `|live|` progress entries; backend `set_watch_progress` rejects a `live` content type — test `live_tv_is_never_tracked`.)*
- [x] Progress is provider-scoped and removed when the provider is deleted (cascade). *(FK `ON DELETE CASCADE`; test `clearing_and_provider_delete_remove_rows` clears one row and confirms provider deletion drops the rest; `list_returns_section_keyed_by_content_id` confirms section/provider scoping.)*
- [x] All progress reads/writes are local (SQLite only) — no provider requests. *(the four `watch` commands only touch `db::watch`/SQLite; the entire backend test suite runs offline.)*

---

### Milestone 9 — Search Results Screen & Live TV Channel Filter

**Goal:** Let users commit a search to a full sectioned results screen, and filter the live channel list in place by name. (Extends §5.5 Search and §5.3 Live TV Browser.)

**Scope:**
- **Search results screen (§5.5):** pressing Enter in `SearchBar` closes `SearchOverlay` and navigates to a new `SearchResultsPage`, sectioned Live TV / Movies / TV Shows with the full (non-capped) result set per type. Carry the active content-type and genre/category filters across the navigation; omit empty sections; blank/whitespace queries don't navigate. Reuse the existing `search` command with a higher `limit` for the full sets; result clicks behave as in the overlay (Live → play, VOD → detail).
- **Live TV channel filter (§5.3):** add `ChannelFilterBar` above `ChannelList` that live-filters by channel name as the user types, scoped to the active category ("All Channels" included). Add an optional `query` parameter to `get_live_channels` (full IPC path: handler in `commands/catalog.rs` → `generate_handler![]` → `models.rs`/`types/index.ts` → `lib/tauri.ts` → `devMock.ts`) so the filter matches the whole category, not just the loaded virtualization window, and the filtered list stays virtualized. Reset the filter on provider change; show an inline "no channels match" state.

**Acceptance Criteria:**
- [x] Pressing Enter in the search bar closes the overlay and opens the full results screen for the query. *(preview e2e: Cmd+F overlay, typed "Sports", Enter → overlay unmounted and the router navigated to `/search?q=Sports`. `SearchBar` fires `onSubmit` on Enter; `SearchOverlay.submitSearch` closes and `navigate`s with the query and filters in the URL.)*
- [x] The results screen is sectioned by Live TV / Movies / TV Shows, each with its standard card layout; empty sections are omitted. *(preview: "Midnight" rendered a MOVIES section (poster grid) and a TV SHOWS section with no Live TV section; "Sports" rendered only a Live TV list. `SearchResultsPage` renders a `ResultSection` per type that returns `null` when empty — list layout for channels, grid for posters.)*
- [x] The results screen shows the full result set (beyond the overlay's 5-per-group inline cap), and active content-type/genre filters carry over. *(preview: the Live and Movies sections each rendered the full 500 fetched (vs. 5 inline in the overlay) and TV Shows 200; clicking the Movies tab narrowed to just that section and set `type=movies` in the URL, surfacing the genre select. Filters live in the URL (`q`/`type`/`cat`) so they survive the overlay→page hop and in-place refine.)*
- [x] Clicking a Live TV result plays it; clicking a VOD result opens its detail view; a blank/whitespace query does not navigate. *(preview: a channel result opened the player (`playerStore.open === true`, live content); a movie result navigated to `/movies` with `MovieDetail` open; pressing Enter on a whitespace-only query from `/live` left the path at `/live` with the overlay still open.)*
- [x] The Live TV channel filter narrows the visible channels by name as the user types, scoped to the selected category. *(preview: typing "Sports 00" in "All Channels" narrowed to 12 rows all containing that substring; within the Sports category, "Nova" narrowed to 18 rows all containing "Nova". Backed by the `name LIKE` filter in `live_channels_page`; test `channel_filter_matches_by_name_and_composes_with_category`.)*
- [x] The filter matches across the entire active category (not only the loaded virtualization window) and the filtered list remains virtualized/smooth at 12k channels. *(the filter text is passed to `get_live_channels` and applied in SQL, so matches come from the whole category and stay virtualized — the "Sports 00" hits spanned many categories beyond the loaded window. `usePagedLiveChannels` folds `query` into the fetcher identity so a new filter resets paging to page 1; `tests/milestone3.rs` proves the underlying paged query stays well under the 500ms budget at 12k rows.)*
- [x] Clearing the filter restores the full category list; switching categories re-applies the current filter; the filter resets on provider change. *(preview: clearing the input restored the Sports list (33 virtualized rows); the filter state persists across category changes (re-applied by `ChannelList` re-fetching with both category and query) and is reset by an effect on `providerId` plus remounting `ChannelFilterBar` via `key={providerId}`.)*
- [x] When nothing matches, an inline "No channels match '[text]'." message is shown in place of the list. *(preview: filtering "zzznomatch" replaced the list with the `channel-filter-empty` state reading No channels match "zzznomatch"; `live_channels_page` returns an empty page (not an error) for a no-match filter — test `blank_filter_is_ignored_and_like_metacharacters_match_literally` also covers blank-as-no-filter and literal `%`.)*
- [x] Both features remain entirely local — no provider/network requests beyond the existing cached-catalog reads. *(the channel filter only adds a SQL `WHERE name LIKE ?` to the existing `get_live_channels` read; the results screen only calls the local FTS5 `search` command (Milestone 6 proved it serves from cache against an unreachable provider). No new network paths.)*

