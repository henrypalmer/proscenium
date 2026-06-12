# Proscenium — Product Specification

**Version:** 0.5.0 (Draft)
**Status:** In Progress
**Last Updated:** 2026-06-11

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
│       └── Channel List
├── Movies
│   ├── All Movies
│   └── [Genre]
│       └── Movie Grid → Movie Detail
├── TV Shows
│   ├── All Shows
│   └── [Genre]
│       └── Show Grid → Show Detail → Season → Episode List
├── Search (global overlay)
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
| Continue Watching | Medium | Track playback position in SQLite |
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

-- Indexes for common query patterns
CREATE INDEX idx_live_channels_provider    ON live_channels(provider_id);
CREATE INDEX idx_live_channels_category    ON live_channels(provider_id, category_id);
CREATE INDEX idx_movies_provider           ON movies(provider_id);
CREATE INDEX idx_movies_category           ON movies(provider_id, category_id);
CREATE INDEX idx_series_provider           ON series(provider_id);
CREATE INDEX idx_series_category           ON series(provider_id, category_id);
CREATE INDEX idx_episodes_series           ON episodes(series_id, provider_id);

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

// Fetch paginated live channels, optionally filtered by category.
invoke('get_live_channels', {
  providerId: string,
  categoryId?: string,
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
| `ChannelList` | `live/ChannelList.tsx` | Virtualized list of `ChannelCard` items for the active category |
| `ChannelCard` | `live/ChannelCard.tsx` | Channel logo, name, category label; click to play, right-click for context menu |
| `MovieGrid` | `vod/MovieGrid.tsx` | Virtualized grid of `MovieCard` items |
| `MovieCard` | `vod/MovieCard.tsx` | Poster, title, year, IMDB badge (when available) |
| `MovieDetail` | `vod/MovieDetail.tsx` | Full detail panel: banner, metadata, play/external buttons |
| `SeriesGrid` | `vod/SeriesGrid.tsx` | Virtualized grid of `SeriesCard` items |
| `SeriesCard` | `vod/SeriesCard.tsx` | Poster, title, year, IMDB badge |
| `SeriesDetail` | `vod/SeriesDetail.tsx` | Series banner, metadata, season selector, renders `EpisodeList` |
| `EpisodeList` | `vod/EpisodeList.tsx` | List of episodes for the selected season; each row has a play button |
| `SearchOverlay` | `search/SearchOverlay.tsx` | Modal overlay; opens on Cmd/Ctrl+F; contains `SearchBar` and `SearchResults` |
| `SearchBar` | `search/SearchBar.tsx` | Debounced input + content type filter tabs |
| `SearchResults` | `search/SearchResults.tsx` | Renders three `SearchResultGroup` sections |
| `SearchResultGroup` | `search/SearchResultGroup.tsx` | Single content-type result group with inline limit and "Show all" expander |
| `PlayerOverlay` | `player/PlayerOverlay.tsx` | Full-screen container for libmpv embed + controls; handles keyboard shortcuts |
| `PlayerControls` | `player/PlayerControls.tsx` | Play/pause, seek bar, volume, track selectors, fullscreen, close |
| `VolumeControl` | `player/VolumeControl.tsx` | Volume slider + mute toggle |
| `TrackSelector` | `player/TrackSelector.tsx` | Dropdown for audio and subtitle track selection |
| `BufferingOverlay` | `player/BufferingOverlay.tsx` | Spinner + timeout message + error state with retry/external player options |
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
- [ ] Clicking a channel opens the built-in player and begins streaming.
- [ ] Play/pause, seek, volume, mute all function correctly.
- [ ] Audio and subtitle track selectors populate and switch tracks.
- [ ] All keyboard shortcuts work as specified.
- [ ] Full-screen toggle works on both platforms.
- [ ] Hardware decode is active for H.264 and H.265 streams (verifiable via mpv stats overlay).
- [ ] Buffering spinner appears; timeout message shows at 10s; error state at 30s.
- [ ] "Open in External Player" launches mpv or VLC with the correct stream URL.
- [ ] Closing the player returns to the content browser without state loss.

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
- [ ] Movies section displays all movies; genre filter works correctly.
- [ ] TV Shows section displays all series; genre filter works correctly.
- [ ] Selecting a movie opens its detail view with title, year, genre, and description (if available).
- [ ] Selecting a series opens its detail view; season selector shows correct episodes per season.
- [ ] Play button on a movie starts the built-in player with the correct stream.
- [ ] Play button on an episode starts the built-in player with the correct episode stream.
- [ ] "Open in External Player" works from movie and episode detail views.
- [ ] Grid scrolls at 60 fps with 10,000+ items.

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
- [ ] Cmd/Ctrl+F opens the search overlay from any section of the app.
- [ ] Results appear within 300ms of the user stopping typing.
- [ ] Results are correctly grouped by content type.
- [ ] Content type filter correctly limits results to the selected type.
- [ ] "Show all" expander reveals all results for a group.
- [ ] Clicking a Live TV result starts playback immediately.
- [ ] Clicking a VOD result navigates to the detail view.
- [ ] No-results state displays a friendly message.
- [ ] Search is performed entirely locally — no network requests.

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
- [ ] All settings persist across restarts.
- [ ] Changing default external player is reflected immediately.
- [ ] Hardware decode can be toggled off in Settings > Playback.
- [ ] Warning banner appears when provider is unreachable at startup.
- [ ] Warning banner appears when subscription is expired (Xtream providers).
- [ ] Stale images older than 30 days are evicted from the cache on startup.
- [ ] Windows `.msi` installer builds successfully and installs the app cleanly.
- [ ] macOS `.dmg` builds successfully; app launches without Gatekeeper errors (note: requires code signing in production).
- [ ] Auto-updater checks for updates on launch.

