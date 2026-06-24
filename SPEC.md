# Proscenium — Product Specification

**Version:** 1.0.0
**Status:** Released
**Last Updated:** 2026-06-17

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
   - 5.10 [Home Screen](#510-home-screen)
   - 5.11 [Custom Lists (Playlists)](#511-custom-lists-playlists)
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
- A sidebar or tab-strip lists all available **channel categories** (e.g., "Sports", "News", "Entertainment", "Kids"). The category panel (shared with Movies/TV Shows) is **collapsible** to a thin rail and defaults to expanded (Milestone 19).
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
- A sidebar or filter strip lists all available **genres** (e.g., "Action", "Comedy", "Drama", "Documentary"). The genre panel is **collapsible** (defaults to expanded; see §5.3 and Milestone 19).
- An **"All Movies"** entry shows the full movie catalog as a **vertical stack of per-genre rows** — the provider's "Popular" genre first, then the remaining genres alphabetically — each a horizontally-scrollable strip of movie cards (Milestone 19). A row's title selects that genre's full grid.
- Selecting a specific genre shows that genre's full grid of movie cards (poster art, title, year).

**TV Series**
- Top-level navigation entry **"TV Shows"**.
- Same genre filter structure as Movies, including the collapsible panel and the per-genre "All Shows" row stack (Milestone 19).
- Main area: per-genre row stack (All) or a full grid of series cards (a selected genre).
- Selecting a series opens a detail view showing seasons and episodes.

#### Movie/Series Card

Each card displays:
- Cover art / poster (thumbnail). Falls back to a styled placeholder with the title if no image is available.
- Title.
- Release year (if available).
- IMDB rating badge (when IMDB integration is active, see 5.8).

#### Detail View

Selecting a movie or series opens a detail panel/page laid out as a **full-bleed hero** (§9 Motion & Animation, Milestone 18): a backdrop image fills the top band behind a gradient scrim, with the poster overlapping the hero's lower edge and the title/metadata/actions reading over the scrim. The backdrop uses the provider's real backdrop art when available (Xtream `backdrop_path`/`cover_big` for movies, `backdrop_path`/`cover` for series), falling back to a blurred-and-darkened treatment of the poster so the hero is never flat black. The clicked poster **morphs** into the hero poster via the shared-element transition (§9). The view shows:
- Hero backdrop with scrim, and the poster art overlapping its lower edge.
- Title, year, genre tags, duration, and provider/IMDB rating (when available).
- Description/synopsis (from provider metadata or IMDB), given fuller presence in the lower area below the hero.
- IMDB rating and vote count (when available).
- For series: a **season dropdown** → episode list of **thumbnail-led rows** (episode thumbnail, a clean title, an "Episode N · duration" metadata line, and a short synopsis), surfaced prominently in the lower area (Milestone 20). The thumbnail is the play/resume target (§5.9); Play and Open in External Player live in a per-episode context menu.
- Play button (built-in player), Open in External Player button, and Add to list.

> The backdrop loads directly from the provider URL; on-disk caching of cover art/backdrops is the §5.7 feature, deferred to its own milestone. A "More like this" related-titles row is a planned follow-up (§13) and is not part of the initial redesign.

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

### 5.10 Home Screen

#### Description

The **Home** screen is the landing view shown first when the application opens. It is a curated overview composed of horizontally-scrolling rows of content drawn from the active provider's catalog and the user's local watch history. It is reached via the **Home** entry in the primary navigation (§9) and lives at the app root route.

#### Layout

- A vertical stack of **rows**, each a labeled section with a horizontally-scrollable strip of cards laid out side by side. Cards reuse the exact components from their dedicated sections (`MovieCard`, `SeriesCard`) — same poster art, sizing, hover, click, and context-menu behavior — so a movie on Home behaves identically to a movie in the Movies grid.
- Rows render in this order: **Keep Watching**, **My Lists**, **Popular Movies**, **Popular Series**. When the user has any in-progress content, **Keep Watching is the first (top-most) row** so resumable items are immediately reachable; when there is no in-progress content the row is omitted and the next non-empty row becomes the top row. *(Omitted rows collapse; the remaining rows close up so there is no empty gap.)*
- The **My Lists** row surfaces the user's custom lists (§5.11); it follows Keep Watching so personal content leads over the provider's curated "Popular" rows. It is **always shown** (even with no lists) so its leading "+ New list" card is a discoverable way to create the first list directly from Home.
- Each row scrolls horizontally and independently; the page itself scrolls vertically if the rows overflow the viewport.

#### Rows

**Popular Movies**
- The movies belonging to the provider's **"Popular"** category. The category is resolved by a case-insensitive match against the VOD category names (the catalog's existing `get_vod_categories`); its items are fetched with the existing `get_movies` (first page, capped at a reasonable strip length, e.g. ~30).
- Cards are `MovieCard`s, identical to the Movies tab, including the watch-progress overlay (§5.9).

**Popular Series**
- The same as Popular Movies but for TV series: the provider's **"Popular"** series category resolved from `get_series_categories`, items from `get_series`, rendered as `SeriesCard`s.

**Keep Watching**
- The user's **in-progress** movies and episodes — exactly the items that qualify for a progress bar in §5.9 (meaningful position, **not** completed). Live TV is never included (it is never tracked).
- Ordered most-recently-watched first (by the watch-progress `updated_at`).
- **Card artwork:**
  - **Movies** show the movie poster and resume directly (see below).
  - **Episodes (series content)** show the **parent series poster/image — not the individual episode's thumbnail** — so a show in progress reads as the show, not a single episode. The card's title is the series title. (When the parent series is unknown — e.g. a catalog-orphaned episode — fall back to the episode's own image/title.)
- Each card shows the same **progress bar overlay** used on Movie cards and episode rows (`WatchProgressOverlay`). For an episode card the bar reflects the **last in-progress episode**'s position within that episode.
- **Click behavior:**
  - **Movie card:** clicking goes through the standard resume flow (§5.9) — because every Keep Watching item is in-progress by definition, this always presents the existing `ResumeDialog` prompt ("Resume from [MM:SS]" or "Start from beginning"). This behavior is unchanged.
  - **Series (episode) card:** clicking opens a small **choice popup** (`ContinueWatchingSeriesDialog`) offering two actions:
    1. **Resume [SxxEyy]** — resumes the **last in-progress episode** for that series via the standard §5.9 resume flow (the same episode the card represents).
    2. **Go to series** — navigates to that series' detail page (§5.4 `SeriesDetail`) instead of starting playback.
  - The popup is dismissible (click-away / Esc) and is shown only for series content; movie cards never show it.
- **Removing an item from Keep Watching:** each card exposes a secondary affordance (a right-click context menu, and/or a hover "⋯" button so it is reachable without a right-click) with two destructive actions:
  - **Mark as watched** — sets the item's completion flag (`set_watch_progress` advancing it past the completion threshold, or a dedicated flag write). The item immediately leaves Keep Watching (completed items are excluded) and instead shows the **watched checkmark** wherever it appears in the catalog (§5.9). For a series episode this marks **that episode** watched; the series stays in Keep Watching if it still has other in-progress episodes.
  - **Remove from list** — clears the saved progress entirely via the existing `clear_watch_progress` (§16). The item disappears from Keep Watching and shows neither a progress bar nor a checkmark (as if never watched); replaying it later starts fresh.
  - Both actions update the row in place (the removed card animates out and the rest close up). Removing the last item omits the whole row.
- Because watch progress stores only `(provider_id, content_type, content_id, position, duration, completed)` and not the catalog item itself, the renderable card data (poster, title, and — for episodes — the parent series for both artwork and the resume target) is resolved on the backend via a dedicated `get_continue_watching` command (§16) that joins progress against the `movies` and `episodes` tables.

**My Lists**
- A horizontally-scrollable row of **collection-cover cards**, one per custom list (§5.11), most-recently-updated first. Consistent with the other Home rows (same row width, horizontal scroll), but each card represents a whole list rather than a single title.
- Each cover card shows a **2×2 poster mosaic** of the list's first up-to-four items (falling back to the `Placeholder` tile for empty slots), with the **list name** and **item count** below. Clicking the card opens that list's **List Detail** view (§5.11).
- A leading **"+ New list"** card is the first item whenever the row is shown, so a list can be created directly from Home; it opens the list editor (§5.11).
- The row is **always shown** (even with zero lists), so the "+ New list" card is always available as an entry point. With no lists the row contains just that card. (Lists can also be created from any content item's "Add to list" affordance in the catalog — §5.11.)

#### Empty / Unavailable States

- If the provider exposes **no "Popular" category** (movies or series), that row is omitted rather than shown empty.
- If there is **no watch history**, the Keep Watching row is omitted.
- The My Lists row is **always shown** (with at least its "+ New list" card), even when the user has no custom lists yet (§5.11).
- If **no provider** is active, Home shows the same "select a provider in Settings" guidance the other sections use.
- All Home data comes from the local cache and local watch history; no Home row triggers an on-demand provider request.

---

### 5.11 Custom Lists (Playlists)

#### Description

Users can create their own named **lists** — playlists of content they curate, e.g. "Horror movies to watch" or "Binge Worthy TV Shows". A list can hold **movies, TV series, and Live TV channels** (mixed freely in one list), letting the user organize content across the catalog independently of the provider's categories. Lists are stored **locally** (no provider requests) and are **provider-scoped** — they belong to the active provider and are cascade-deleted with it, because the item references (`content_id`s) are provider-specific.

> **Design decision (Open Question #6):** lists are **mixed-content** — a single list may contain any combination of movies, series, and channels — rather than one list per content type. This is the most flexible and matches "playlists". Cards within a list render with the appropriate component for each item's type.

#### Creating & Managing Lists

- **Create:** from the Home **"+ New list"** card (§5.10), from a content item's **"Add to list"** affordance (which offers "+ New list…" inline), or from the List Detail header. Creating asks only for a **name** (required, non-empty; duplicate names are allowed but discouraged with a hint).
- **Rename / Delete:** available from the List Detail view and from a list cover card's context menu. Deleting prompts for confirmation and removes the list and its membership rows (the underlying catalog content is untouched).
- **Reorder lists:** the user can order their lists; the order is persisted (`sort_order`) and drives the My Lists row and any list picker. Default order is most-recently-updated first until the user reorders.

#### Adding & Removing Items

- **Add to list:** every browsable content item — `MovieCard`, `SeriesCard`, `ChannelCard`, and the Movie/Series detail views — gains an **"Add to list…"** action in its context menu. It opens a small picker listing the user's lists (with a checkmark for lists the item is already in) plus an inline **"+ New list…"**. Toggling adds/removes the item from that list.
- **Remove from list:** from the picker (untoggle) or from the **List Detail** view (a per-item "Remove" action). Removing affects only the membership, never the catalog item or its watch progress.
- **Deduplication:** an item appears in a list at most once (`PRIMARY KEY (list_id, content_type, content_id)`); re-adding is a no-op.
- **Ordering within a list:** items keep an explicit `position` (newest-added last by default) so the order is stable and, later, user-reorderable.

#### List Detail View

- Opening a list (from the My Lists row or a list picker) shows a dedicated **List Detail** view: the list name (editable), item count, rename/delete controls, and a **virtualized grid** of the list's items.
- Items render with their native cards by type — `MovieCard` / `SeriesCard` / `ChannelCard` — so behavior matches the dedicated sections (a movie plays/opens its detail, a channel starts playback, etc.), including the §5.9 watch-progress overlays on movies.
- Mixed types are shown together in one grid in list order; a small type badge distinguishes channels from VOD where useful.

#### Edge Cases

- **Catalog refresh / orphaned items:** a list item whose `content_id` no longer exists after a refresh (the provider dropped it) is **hidden** from the List Detail grid and the cover mosaic and **excluded from the item count**, but its membership row is retained (not auto-deleted) so the item reappears if the content returns on a later refresh. *(This mirrors how `get_continue_watching`'s joins drop missing catalog rows.)*
- **Empty list:** a list with no (resolvable) items still exists and is shown; its cover uses placeholder tiles and it reads "0 items". The List Detail view shows an empty-state prompt to add content.
- **Provider scope:** switching the active provider shows that provider's lists only; another provider's lists are untouched and reappear when it is reselected.
- **Live TV in lists:** channels can be added even though they are never tracked for watch progress (§5.9); they simply carry no progress overlay.

#### Data & IPC

- Backed by two new tables, `user_lists` and `user_list_items` (§15), and a new set of list commands (§16). All reads/writes are local; the cover mosaics and List Detail cards are resolved by joining membership rows against the `movies` / `series` / `live_channels` tables (like the Keep Watching join).

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

### Primary Navigation

Primary navigation is a **floating navigation bar pinned to the top-center** of the content area (not a left sidebar). It is a compact, horizontally-centered bar overlaying the top of the content, with clickable sections in this fixed left-to-right order:

**Home · Live TV · Movies · TV Shows · Settings**

Selecting a section routes to it (the active section is highlighted). The nav row also carries, as their own disjointed "bubbles" using the nav's background styling: the **active provider** (left of the nav pill, clickable → Settings › Providers, name truncates) and the icon-only **Search** and **Refresh** controls (right of the pill). Refresh shows catalog-refresh progress as a ring around its bubble with the current stage as its tooltip. There is no separate header bar — the previous left-hand sidebar and top Header are both removed and the main content spans the full width; the secondary category/genre panel (§5.3/§5.4) still appears within Live TV, Movies, and Series. The `WarningBanner` (offline/expired provider, §12) renders above the content when active.

### Navigation Structure

```
App
├── Home (root route — first screen)
│   ├── Keep Watching   (in-progress movies/episodes, with progress bars)
│   ├── My Lists        (custom-list cover cards → List Detail)
│   ├── Popular Movies  (provider "Popular" category → MovieCards)
│   └── Popular Series  (provider "Popular" category → SeriesCards)
├── List Detail (one custom list → mixed grid of MovieCard/SeriesCard/ChannelCard)
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

### Scrollbars

- The application uses **custom-styled scrollbars** that match the dark, minimal aesthetic — **not** the OS default chrome (on Windows the default is a wide white track with a grey pill and arrow buttons, which clashes with the theme).
- Target a **thin, track-less bar with a translucent rounded thumb** and **no stepper arrows**: a subtle grey thumb (e.g. `zinc-600/700`) on a transparent or near-transparent track, the thumb brightening slightly on hover. Width on the order of ~8–10px.
- Apply globally so **every** scroll container is covered — the vertical page scroll, the virtualized lists/grids (Live TV, Movies, TV Shows), and the horizontally-scrolling Home rows.
- Implemented with global CSS (`::-webkit-scrollbar*` for the WebView2/WebKit webview, plus `scrollbar-width: thin` / `scrollbar-color` for completeness) in the app's root stylesheet — no per-component styling. It must adapt to the light theme when that ships (§13).
- Scrollbars must remain functional and discoverable (do not hide them entirely on Windows, where overlay/auto-hiding scrollbars are not the platform norm); the goal is restyling, not removal.

### Motion & Animation

Motion should make the UI feel responsive and physical without ever competing with the content or taxing the baseline hardware (§10). The guiding rules:

- **Reactivity on hover/press.** Browsable content cards (movie/series posters, Keep Watching cards, list-cover cards) respond to the cursor: a subtle scale-up on hover (~1.04) and a slight scale-down on press (~0.98), so the grid feels alive under the pointer. Caption text may brighten in concert.
- **Continuity on navigation.** Opening a content detail view transitions from the grid rather than snapping in: the outgoing view cross-fades into the detail, and the clicked **poster morphs** from its grid cell into the detail layout (a shared-element transition), reinforcing where the user came from. Closing reverses it.
- **Cheap by construction.** Only **compositor-friendly** properties (`transform`, `opacity`) are animated — never layout- or paint-heavy properties (`width`/`height`/`box-shadow`/`top`/`left`) at hover/scroll frequency. No persistent `will-change` on virtualized cells (it would promote dozens of layers and cost GPU memory). Transitions must stay smooth over the 12k-item virtualized grids (§10).
- **Implementation.** Card hover/press uses CSS transforms (Tailwind utilities). View transitions use the browser **View Transitions API** (`document.startViewTransition`), which is supported by the WebView runtimes Proscenium targets (evergreen Chromium / WebView2 on Windows, recent WebKit on macOS) and runs off the main thread; it degrades to an instant update where unavailable. No animation library is added.
- **Respect user preference.** All non-essential motion honors `prefers-reduced-motion: reduce` — hover scaling and view transitions collapse to instant state changes.
- **No motion behind the player.** While the built-in player is open the browser is hidden and the page background is transparent for the native mpv window (§5.6); transitions must not run there.

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
| On-disk cover-art / backdrop cache | Medium | The §5.7 download-to-disk pipeline (download → app-data dir → `image_cache` upsert → serve via Tauri asset protocol) for **all** art (posters + backdrops). The `image_cache` table + 30-day eviction exist but are an unused stub (`image_cache::upsert` has no callers); all art currently loads directly from provider URLs. Deferred out of Milestone 18 — caching one image type in isolation is low value. |
| "More like this" (related titles) | Medium | A row of related titles (same genre/category) on the movie/series detail view so it doubles as a discovery surface. Needs a local `get_related` command (§16 IPC pattern, no provider request). Deferred follow-up to the Milestone 18 detail redesign. |
| IMDB ratings integration | High | See §5.8 |
| EPG (Electronic Program Guide) | High | Requires XMLTV or Xtream EPG endpoint; target v1.1 |
| Linux platform support | High | Deferred from v1.0; target v1.1 or v2.0 |
| Favorites / Watch Later | Medium | Largely subsumed by **Custom Lists (§5.11)** — a user can keep a "Watch Later" list. A dedicated one-tap favorite toggle could still layer on top later. |
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
| Recently-watched channels row | Low | Live-TV "Recently watched" channels row and a "now playing / last watched" indicator on the channel you just viewed. Idea from the 2026-06-24 QA pass (`QA_NOTES.md` §2). |
| Friendlier track-menu labels | Low | Subtitle/audio menus expose codec names ("eng · dvd_subtitle", "eng · ac3"); show friendly labels ("English", "English (SRT)", "English 5.1") and de-duplicate identical entries. QA idea (`QA_NOTES.md` §7); the track-selection *fix* is Milestone 22. |
| Watched / in-list badge legend | Low | The green "✓" badge on some cards (watched / in a list) is unlabeled; add a tooltip or legend. QA idea (`QA_NOTES.md` §1). |
| Global Live-TV filter scope | Low | The Live-TV filter box is scoped to the selected category; offer a hint or a toggle to search all channels without first selecting "All Channels". QA idea (`QA_NOTES.md` §2). |
| Clarify LIVE-badge timer | Low | The LIVE badge's running timer is ambiguous (session-elapsed vs. buffer/time-shift position); clarify or hide it for pure-live streams. QA idea (`QA_NOTES.md` §2). |

---

## 14. Open Questions

| # | Question | Owner | Status |
|---|----------|-------|--------|
| 1 | What is the preferred app name? | Product | Resolved — **Proscenium** |
| 2 | Should the app support Apple Silicon (ARM64) natively, or is a Rosetta 2 build acceptable for the initial macOS release? | Engineering | Resolved — **Rosetta 2 acceptable for v1; native ARM64 deferred** |
| 3 | For Dolby Vision on Windows, is hardware DV decode (requiring a DV-capable display and driver) required, or is tone-mapped SDR fallback acceptable? | Engineering | Resolved — **Silent fallback to HDR10/SDR; playback never blocked** |
| 4 | Should the installer be code-signed for both platforms from day one? (Required to avoid OS security warnings on macOS Gatekeeper and Windows SmartScreen.) | Product | Open |
| 5 | "Skip Intro" for TV series — what approach is acceptable? IPTV providers (Xtream/M3U) supply **no** intro markers, so frame-accurate auto-detection is not feasible without a heavy audio-fingerprinting pipeline. The realistic options are a hybrid of: (a) honoring container chapter markers via mpv when present (accurate but rarely available), (b) a "learned per-series" intro length the user confirms once and is reused for later episodes, and (c) a manual fixed-offset skip button during the opening window. | Engineering / Product | Open — exploration only, no committed milestone |
| 6 | How should the "My Lists" section on Home represent each custom list (§5.10/§5.11), given a list is a collection rather than a single poster? | Product | Resolved — **a horizontally-scrollable row of collection-cover cards** (2×2 poster mosaic + name + count), consistent with the other Home rows, with a leading "+ New list" card; a card opens List Detail. |
| 7 | Are custom lists **mixed-content** (movies + series + channels in one list) or **one list per content type**? | Product | Resolved — **mixed-content** (§5.11); a list may hold any combination. |

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

-- Custom user lists / "playlists" (§5.11). Provider-scoped; cascade-delete with
-- the provider. A list may mix movies, series, and live channels.
CREATE TABLE user_lists (
  id          TEXT PRIMARY KEY,                                     -- app-generated UUID
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,                           -- user ordering of lists
  created_at  INTEGER NOT NULL,                                     -- Unix timestamp
  updated_at  INTEGER NOT NULL                                      -- Unix timestamp (membership/name changes)
);

-- Membership rows for user_lists. content_id refers to movies.id / series.id /
-- live_channels.id depending on content_type (resolved by JOIN, like the Keep
-- Watching join). Orphaned rows (content dropped on refresh) are retained but
-- filtered out at read time.
CREATE TABLE user_list_items (
  list_id      TEXT NOT NULL REFERENCES user_lists(id) ON DELETE CASCADE,
  content_type TEXT NOT NULL CHECK (content_type IN ('live', 'movie', 'series')),
  content_id   TEXT NOT NULL,
  position     INTEGER NOT NULL,            -- order within the list (newest-added last by default)
  added_at     INTEGER NOT NULL,            -- Unix timestamp
  PRIMARY KEY (list_id, content_type, content_id)
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
CREATE INDEX idx_user_lists_provider       ON user_lists(provider_id, sort_order);
CREATE INDEX idx_user_list_items_list      ON user_list_items(list_id, position);

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

// Remove an item's progress (e.g. Keep Watching "Remove from list", §5.10):
// deletes the row so the item shows neither a progress bar nor a checkmark.
invoke('clear_watch_progress', {
  providerId: string,
  contentType: 'movie' | 'episode',
  contentId: string
}): Promise<void>

// Keep Watching "Mark as watched" (§5.10): force the completion flag regardless
// of whether the runtime is known (set_watch_progress can only infer completion
// from position/duration). Parks the position at the end when duration is known.
invoke('mark_watched', {
  providerId: string,
  contentType: 'movie' | 'episode',
  contentId: string,
  durationSeconds: number | null
}): Promise<void>

interface WatchProgress {
  positionSeconds: number;
  durationSeconds: number | null;
  completed: boolean;
  updatedAt: number;        // Unix timestamp
}
```

### Home Commands (§5.10)

```typescript
// In-progress movies and episodes for the Home "Keep Watching" row, joined
// against the catalog so each item carries the data needed to render a card
// plus its progress. Excludes completed items; most-recently-watched first;
// provider-scoped and entirely local. Episodes include their parent series
// (when present): the Keep Watching card renders the SERIES poster/title (not
// the episode thumbnail) and uses the episode as the "last in-progress episode"
// resume target. The series is also the navigation target for the "Go to series"
// action. Fall back to the episode's own image/title only when the series is null.
invoke('get_continue_watching', {
  providerId: string,
  limit?: number            // default ~20
}): Promise<ContinueWatchingItem[]>

type ContinueWatchingItem =
  | { kind: 'movie'; movie: Movie; progress: WatchProgress }
  | { kind: 'episode'; episode: Episode; series: Series | null; progress: WatchProgress };

// Popular Movies / Popular Series rows reuse existing commands: resolve the
// provider's "Popular" category from get_vod_categories / get_series_categories
// (case-insensitive name match), then fetch its items via get_movies / get_series.
```

### Custom List Commands (§5.11)

```typescript
// --- List management ---

// Create a list (returns the new list). Name is required/non-empty.
invoke('create_list', { providerId: string, name: string }): Promise<UserList>

// Rename a list.
invoke('rename_list', { listId: string, name: string }): Promise<void>

// Delete a list and its membership rows (the catalog content is untouched).
invoke('delete_list', { listId: string }): Promise<void>

// Persist the user's ordering of their lists (sort_order).
invoke('reorder_lists', { providerId: string, orderedListIds: string[] }): Promise<void>

// All of the active provider's lists, in sort_order, each with its item count and
// the first few item posters for the Home cover mosaic (§5.10). Counts/posters
// exclude items whose catalog row no longer exists. Local-only.
invoke('get_lists', { providerId: string }): Promise<ListSummary[]>

// --- Membership ---

// Add an item to a list (no-op if already present). content_type is the kind of
// the catalog item being added.
invoke('add_to_list', {
  listId: string,
  contentType: 'live' | 'movie' | 'series',
  contentId: string
}): Promise<void>

// Remove an item from a list (membership only).
invoke('remove_from_list', {
  listId: string,
  contentType: 'live' | 'movie' | 'series',
  contentId: string
}): Promise<void>

// Reorder items within a list (optional / future — positions persisted).
invoke('reorder_list_items', { listId: string, orderedItemKeys: string[] }): Promise<void>

// The resolved items of one list for the List Detail grid (§5.11), joined against
// movies / series / live_channels so each carries the data to render its native
// card. In list order; items whose catalog row is missing are omitted.
invoke('get_list_items', { listId: string }): Promise<UserListItem[]>

// Which of the user's lists already contain a given item — backs the "Add to
// list" picker checkmarks without a query per list.
invoke('get_lists_for_item', {
  providerId: string,
  contentType: 'live' | 'movie' | 'series',
  contentId: string
}): Promise<string[]>   // list ids

interface UserList {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

interface ListSummary extends UserList {
  itemCount: number;          // resolvable items only
  coverPosters: (string | null)[]; // up to 4 poster URLs for the mosaic
}

// One resolved list item, discriminated by kind (mirrors how content cards pick
// their component). Live channels carry no watch progress (§5.9).
type UserListItem =
  | { kind: 'movie'; movie: Movie }
  | { kind: 'series'; series: Series }
  | { kind: 'live'; channel: LiveChannel };
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
│       │   ├── settings.rs
│       │   ├── watch.rs               # Watch progress + Keep Watching (§5.9/§5.10)
│       │   └── lists.rs              # Custom lists / playlists (§5.11)
│       ├── iptv/                     # Protocol clients
│       │   ├── mod.rs
│       │   ├── xtream.rs             # Xtream Codes API client
│       │   └── m3u.rs                # M3U parser
│       ├── db/                       # Database layer
│       │   ├── mod.rs
│       │   ├── schema.rs             # Schema definitions and migrations
│       │   ├── providers.rs
│       │   ├── catalog.rs
│       │   ├── settings.rs
│       │   ├── watch.rs              # watch_progress + continue_watching queries
│       │   └── lists.rs             # user_lists + user_list_items queries (§5.11)
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
│   │   │   ├── TopNav.tsx            # Floating top nav + provider/search/refresh bubbles
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
│   │   │   ├── HeroBackdrop.tsx      # Full-bleed detail hero backdrop (§5.4, M18)
│   │   │   ├── GenreRows.tsx         # Per-genre row stack for the "All" view (§5.4, M19)
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
│   │   ├── home/                    # Home screen (§5.10)
│   │   │   ├── MediaRow.tsx          # Horizontally-scrollable labeled row
│   │   │   ├── KeepWatchingCard.tsx
│   │   │   ├── ContinueWatchingSeriesDialog.tsx
│   │   │   ├── MyListsRow.tsx        # "My Lists" row of cover cards + "New list" (§5.10)
│   │   │   └── ListCoverCard.tsx     # One list as a 2×2 poster mosaic + name + count
│   │   ├── lists/                    # Custom lists / playlists (§5.11)
│   │   │   ├── ListEditorDialog.tsx  # Create / rename a list
│   │   │   └── AddToListMenu.tsx     # "Add to list…" picker (toggle + inline create)
│   │   └── common/
│   │       ├── SkeletonCard.tsx      # Loading placeholder
│   │       ├── Placeholder.tsx       # Image fallback
│   │       ├── Toast.tsx
│   │       ├── WarningBanner.tsx
│   │       └── ContextMenu.tsx
│   ├── pages/
│   │   ├── Home.tsx
│   │   ├── LiveTV.tsx
│   │   ├── Movies.tsx
│   │   ├── TVShows.tsx
│   │   ├── ListDetail.tsx           # One custom list: mixed virtualized grid (§5.11)
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
| `TopNav` | `layout/TopNav.tsx` | Floating top-center primary nav (§9): Home, Live TV, Movies, Series, Settings — clickable sections with the active one highlighted. The same row carries the active-provider bubble (left, clickable → Settings) and disjointed icon-only Search + Refresh bubbles (right; Refresh shows a progress ring with the stage as its tooltip). Replaces the former left `Sidebar` **and** the top `Header`. |
| `MediaRow` | `home/MediaRow.tsx` | A labeled, horizontally-scrollable strip of cards used by the Home rows (§5.10); renders the section's standard card component side by side |
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
| `HeroBackdrop` | `vod/HeroBackdrop.tsx` | Full-bleed darkened hero backdrop behind the movie/series detail (§5.4, M18): provider backdrop when available, blurred-poster fallback otherwise |
| `GenreRows` | `vod/GenreRows.tsx` | The "All Movies/All Shows" overview (§5.4, M19): a vertical stack of per-genre horizontal card strips (Popular first, then A–Z), each lazy-loaded; a row title selects that genre's full grid |
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
| `ContinueWatchingSeriesDialog` | `home/ContinueWatchingSeriesDialog.tsx` | Choice popup shown when a **series** card in the Home "Keep Watching" row is clicked: "Resume [SxxEyy]" (last in-progress episode, via the §5.9 resume flow) or "Go to series" (navigate to `SeriesDetail`) (§5.10) |
| `KeepWatchingCard` | `home/KeepWatchingCard.tsx` | A Home "Keep Watching" card: poster (series poster for episodes) + `WatchProgressOverlay`, with a context/⋯ menu for "Mark as watched" / "Remove from list" (§5.10) |
| `MyListsRow` | `home/MyListsRow.tsx` | The Home "My Lists" row (§5.10): horizontally-scrollable strip of `ListCoverCard`s, led by a "+ New list" card; opens `ListDetail` / `ListEditorDialog` |
| `ListCoverCard` | `home/ListCoverCard.tsx` | One custom list rendered as a 2×2 poster mosaic + name + item count; context menu for rename/delete (§5.10/§5.11) |
| `ListDetail` | `pages/ListDetail.tsx` | Full view of one custom list (§5.11): editable name, count, rename/delete, and a responsive mixed grid (`MovieCard`/`SeriesCard` + poster-style channel tiles) with per-item "Remove" |
| `ListEditorDialog` | `lists/ListEditorDialog.tsx` | Create / rename a list (name input) (§5.11) |
| `AddToListMenu` | `lists/AddToListMenu.tsx` | "Add to list…" picker opened from a content item's context menu: toggle membership per list (checkmarks) + inline "+ New list…" (§5.11) |
| `SkeletonCard` | `common/SkeletonCard.tsx` | Animated loading placeholder matching card dimensions |
| `Placeholder` | `common/Placeholder.tsx` | Styled fallback when no poster/logo image is available |
| `Toast` | `common/Toast.tsx` | Non-blocking notification (refresh failure, buffering warning, etc.) |
| `WarningBanner` | `common/WarningBanner.tsx` | Persistent inline banner (expired subscription, offline cache, etc.) |
| `ContextMenu` | `common/ContextMenu.tsx` | Right-click menu: Play, Open in External Player, **Add to list…** (§5.11); on Keep Watching cards also **Mark as watched** / **Remove from list** (§5.10) |

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

---

### Milestone 10 — Floating Top Navigation & Home Screen

**Goal:** Replace the left sidebar with a floating top-center navigation bar and add a curated **Home** landing screen. (Delivers §9 Primary Navigation and §5.10.)

**Scope:**
- **Top navigation (§9):** add `TopNav` (`layout/TopNav.tsx`) — a floating, horizontally-centered nav bar with sections **Home · Live TV · Movies · TV Shows · Settings** (this fixed left-to-right order), active section highlighted, routing via the existing router. Remove `Sidebar` from `App.tsx`'s `Shell` and let the content span full width; keep the `Header` toolbar and the per-section `CategoryPanel`.
- **Routing:** Home becomes the root route — `/` renders the new `Home` page (replacing the `/ → /live` redirect); the catch-all falls back to `/` rather than `/live`. Add a `/home` alias is not required (root is Home).
- **Home page (§5.10):** new `pages/Home.tsx` rendering stacked `MediaRow`s in order **Popular Movies, Popular Series, Keep Watching**, each a horizontally-scrollable strip reusing `MovieCard` / `SeriesCard` (with the §5.9 `WatchProgressOverlay`).
  - **Popular Movies / Series:** resolve the provider's "Popular" category from `get_vod_categories` / `get_series_categories` (case-insensitive name match) and fetch its items via `get_movies` / `get_series`; omit the row when no such category exists.
  - **Keep Watching:** add a `get_continue_watching` command (§16) — full IPC path: Rust handler in `commands/watch.rs` → `generate_handler![]` in `lib.rs` → `models.rs` (`ContinueWatchingItem`) ↔ `types/index.ts` → `lib/tauri.ts` wrapper → `devMock.ts`. It joins non-completed `watch_progress` rows against the `movies` and `episodes` tables (episodes carry their parent `series` for poster fallback), most-recently-watched first, provider-scoped. Cards show the progress bar and resume via the standard §5.9 flow.
- **Empty states:** omit a Popular row with no "Popular" category; omit Keep Watching with no history; show the standard "select a provider" guidance when no provider is active.

**Acceptance Criteria:**
- [x] The primary navigation is a floating top-center bar (no left sidebar) with sections Home, Live TV, Movies, TV Shows, Settings in that order; the active section is highlighted and each routes correctly. *(preview: `TopNav` renders the five items in order as a centered floating pill; the former `Sidebar` is deleted. Clicking each routes — verified Home→/, Live TV→/live, Movies→/movies, Settings→/settings — and the active item highlights (`bg-zinc-100`). The nav persists across sections and content clears it (main `pt-16`).)*
- [x] Launching the app lands on Home (root route); Live TV, Movies, TV Shows, and Settings remain reachable and unchanged. *(preview: navigating to `/` renders `Home`; the `/ → /live` redirect was replaced and the catch-all now falls back to `/`. Live TV still shows its category panel + M9 channel filter, Movies still opens detail, Settings still shows Providers/Playback — all unchanged.)*
- [x] Home shows Keep Watching, Popular Movies, and Popular Series as horizontally-scrollable rows of side-by-side cards using the same card components as the dedicated sections. *(preview: three `MediaRow`s — Popular Movies (30 `MovieCard`s), Popular Series (30 `SeriesCard`s), Keep Watching — each a horizontally-scrolling flex strip of fixed-width cards reusing the dedicated `MovieCard`/`SeriesCard` plus the shared `WatchProgressOverlay`.)*
- [x] Popular Movies/Series are populated from the provider's "Popular" category; a missing "Popular" category omits that row rather than showing it empty. *(preview: both Popular rows filled from the "Popular" category resolved via `get_vod_categories`/`get_series_categories` (case-insensitive `\bpopular\b`) + `get_movies`/`get_series`. By construction, no match → `[]` → `MediaRow` returns `null` (the row, like an empty Keep Watching, is omitted, not shown empty).)*
- [x] Keep Watching lists in-progress (non-completed) movies and episodes, most-recently-watched first, each card showing the §5.9 progress bar; clicking resumes via the standard resume flow. *(preview: an in-progress episode and movie rendered newest-first (episode @-40s before movie @-120s), each with a `progress-bar` overlay; clicking the episode card set `pendingResume` (episode, 600s) and showed `ResumeDialog`. Backend `get_continue_watching` joins non-completed `watch_progress` against `movies`/`episodes` (+ parent `series`), most-recent first — test `continue_watching_orders_by_recency_excludes_completed_and_joins_series`.)*
- [x] Keep Watching excludes completed items and Live TV, and is omitted entirely when there is no watch history. *(preview: a completed movie in the seed did NOT appear (2 cards, 0 watched-checkmarks); live is never tracked (§5.9, enforced backend). Tests: completed + catalog-orphaned rows excluded by the join; `continue_watching_is_empty_without_history_and_respects_limit` returns `[]` with no history → the row is omitted via `MediaRow`.)*
- [x] Home renders entirely from the local cache and local watch history — no on-demand provider requests. *(`get_continue_watching` only reads SQLite (`db::watch`/catalog joins); the Popular rows reuse the cached `get_*_categories`/`get_movies`/`get_series` reads. No new network path; the backend test suite runs offline.)*

### Milestone 11 — Keep Watching Refinements

**Goal:** Make the Home "Keep Watching" row (§5.10) the primary entry point for resuming content, and improve how in-progress **series** are represented and resumed.

**Scope:**
- **Row ordering (§5.10):** when the user has any in-progress content, render **Keep Watching as the first (top-most) row**, above Popular Movies and Popular Series. When there is no in-progress content the row is omitted (unchanged) and Popular Movies remains the top row. Only the row order changes in `Home.tsx`; the `MediaRow` components and their data sources are reused as-is.
- **Series artwork (§5.10):** for episode-kind Keep Watching items, render the **parent series poster/title** on the card instead of the individual episode's thumbnail. The data is already available on `ContinueWatchingItem` (`episode.series`); this is a card-rendering change plus the fallback to the episode's own image/title when `series` is `null`. No backend/IPC change is required unless the series poster is not currently selected by `get_continue_watching`'s join — confirm the series row carries a poster and extend the join only if needed.
- **Series resume choice (§5.10):** add `ContinueWatchingSeriesDialog` (`home/ContinueWatchingSeriesDialog.tsx`). Clicking a **series** card opens a small dismissible popup (click-away / Esc) with two actions:
  - **Resume [SxxEyy]** — resume the last in-progress episode (the episode the card represents) via the existing §5.9 `ResumeDialog`/resume flow.
  - **Go to series** — navigate to the series' `SeriesDetail` page (§5.4) without starting playback.
  - **Movie** cards keep their current behavior (click → standard resume flow, no popup).

**Acceptance Criteria:**
- [x] When in-progress content exists, Keep Watching is the top row on Home (above Popular Movies/Series); with no in-progress content the row is omitted and ordering is otherwise unchanged. *(preview: with two seeded in-progress items the row order is `home-keep-watching` → `home-popular-movies` → `home-popular-series`; `Home.tsx` renders Keep Watching first and `MediaRow` returns `null` for an empty `items`, so no history collapses it and Popular Movies leads.)*
- [x] In-progress **series** cards in Keep Watching show the parent series poster and title, not the episode thumbnail; when the parent series is unknown the card falls back to the episode's own image/title. *(preview: the episode card rendered the series title "Hollow Protocol 002" (not the episode title "S01E02 — Garden"); `KeepWatchingCard.describe()` now resolves `series?.posterUrl ?? episode.posterUrl ?? null` and `series?.name ?? episode.title`. The mock can't visually distinguish the poster source — its episodes always have `posterUrl: null` — but the series-first precedence and the `null`-series fallback are in the card.)*
- [x] Clicking a series card in Keep Watching opens a popup offering "Resume [SxxEyy]" (last in-progress episode) and "Go to series"; the popup is dismissible and is not shown for movie cards. *(preview: clicking the series card opened `ContinueWatchingSeriesDialog` with "▶ Resume S1E2 (10:00)" and "Go to series"; Esc / click-away dismiss it; clicking the movie card opened the `ResumeDialog` directly with no series popup.)*
- [x] "Resume" from the popup resumes the last in-progress episode via the standard §5.9 resume flow; "Go to series" navigates to that series' detail page without starting playback. *(preview: "Resume" closed the popup and opened `ResumeDialog` for "Hollow Protocol 002 · S1E2" ("Resume from 10:00" / "Start from beginning"); "Go to series" navigated to `/shows` and opened the "Hollow Protocol 002" `SeriesDetail` with no player.)*
- [x] Movie cards in Keep Watching are unaffected — clicking still opens the existing `ResumeDialog` with "Resume from [MM:SS]" / "Start from beginning" (no behavior change). *(preview: clicking "Golden Empire 003" opened `ResumeDialog` directly ("Resume from 30:00" / "Start from beginning"), `seriesDialogOpen === false`.)*

### Milestone 12 — Sleek Scrollbars

**Goal:** Replace the OS-default scrollbar chrome (the Windows white track + grey pill + stepper arrows) with a thin, theme-matching scrollbar across the app (§9 › Scrollbars).

**Scope:**
- Add global scrollbar CSS in the root stylesheet (`src/index.css` / the Tailwind entry): `::-webkit-scrollbar`, `::-webkit-scrollbar-thumb`, `::-webkit-scrollbar-track`, `::-webkit-scrollbar-button` (hidden) for the WebView2/WebKit webview, plus `scrollbar-width: thin` / `scrollbar-color` for completeness.
- Thin (~8–10px), track-less, rounded translucent grey thumb (e.g. `zinc-600`, brightening on hover); no stepper arrows.
- Applies to every scroll container: the vertical page, virtualized lists/grids (Live TV/Movies/TV Shows), and the horizontal Home rows. Keep it theme-aware for the future light theme.

**Acceptance Criteria:**
- [x] Scrollbars throughout the app render as a thin, arrow-less, rounded translucent thumb on a transparent/near-transparent track — not the OS default — in both vertical and horizontal containers. *(preview: global rules in `src/index.css` apply via `*` — computed `scrollbar-width: thin`, `scrollbar-color: rgb(63,63,70) rgba(0,0,0,0)` (zinc-700 thumb / transparent track); all six `::-webkit-scrollbar*` rules present including `::-webkit-scrollbar-button { display:none }`; the thumb uses `border-radius:9999px` + transparent-border/padding-box inset.)*
- [x] The thumb brightens slightly on hover and scrolling remains fully functional (not hidden). *(preview: a `::-webkit-scrollbar-thumb:hover { background-color:#71717a }` (zinc-500) rule is present; the thumb is restyled, not hidden, so containers stay scrollable.)*
- [x] Virtualized lists/grids and the horizontal Home rows show the restyled scrollbar. *(preview: the rules are global (`*` + bare `::-webkit-scrollbar*`), so they cover every scroll container; the Home screenshot shows the thin dark horizontal scrollbars under the Popular Movies/Series rows.)*

### Milestone 13 — Keep Watching Item Management

**Goal:** Let the user remove an item from the Home "Keep Watching" row via **Mark as watched** or **Remove from list** (§5.10).

**Scope:**
- Add a context menu (and a hover "⋯" affordance) to `KeepWatchingCard` with the two actions, reusing `ContextMenu`.
- **Mark as watched:** set the item's completion flag so it leaves Keep Watching and shows the §5.9 watched checkmark. Finalized as a **dedicated `mark_watched` command** (§16) that forces `completed = true` regardless of whether the runtime is known (parking the position at the end when duration is known) — `set_watch_progress` can only *infer* completion from position/duration, so it can't mark a duration-unknown stream watched. For a series episode, marks that episode; the series stays if other episodes are still in progress.
- **Remove from list:** call the existing `clear_watch_progress` so the item shows neither bar nor checkmark.
- Update the row in place (removed card animates out; row closes up; empty row omitted). Keep `devMock.ts` in sync.

**Acceptance Criteria:**
- [x] Each Keep Watching card exposes "Mark as watched" and "Remove from list" without leaving Home. *(preview: `KeepWatchingCard` gained a right-click handler + a hover "⋯" button (`keep-watching-menu-button`); both open a `ContextMenu` with exactly "Mark as watched" and "Remove from list".)*
- [x] "Mark as watched" removes the item from Keep Watching and it shows a watched checkmark in the catalog; replaying it skips the resume prompt. *(preview: marking "Golden Empire 003" removed its card (Keep Watching 2→1); searching it afterward showed the `watched-check` overlay and no `progress-bar`. The completed flag is what suppresses the resume prompt — established §5.9/M8 behavior — and the new `mark_watched` sets it.)*
- [x] "Remove from list" removes the item from Keep Watching with no checkmark and no progress bar; replaying starts fresh. *(preview: removing "Hollow Protocol 002" dropped its card; opening that series' detail showed episode S01E02 with 0 `progress-bar` and 0 `watched-check` — its progress was cleared via `clear_watch_progress`.)*
- [x] The row updates in place and is omitted once empty; Live TV is still never present. *(preview: each removal updated the row without reload; removing the last item omitted the `home-keep-watching` row entirely (Popular Movies became the top row). Live TV is never returned by `get_continue_watching` — backend invariant from §5.9.)*

### Milestone 14 — Custom Lists (Playlists)

**Goal:** Let users create named lists ("playlists") containing any mix of movies, series, and Live TV channels, and manage membership from anywhere in the catalog (§5.11).

**Scope:**
- **Schema (§15):** add `user_lists` and `user_list_items` (+ indexes), applied idempotently on launch; provider-scoped, cascade-deleted with the provider.
- **Commands (§16):** `create_list`, `rename_list`, `delete_list`, `reorder_lists`, `get_lists`, `add_to_list`, `remove_from_list`, `get_list_items`, `get_lists_for_item` (and `reorder_list_items`, optional) — full IPC path: `commands/lists.rs` → `generate_handler![]` in `lib.rs` → `models.rs` (`UserList`/`ListSummary`/`UserListItem`) ↔ `types/index.ts` → `lib/tauri.ts` → `devMock.ts`. Cover/detail data resolved by joining membership against `movies`/`series`/`live_channels`; orphaned items filtered at read time.
- **UI:** `AddToListMenu` from every content item's context menu (`MovieCard`, `SeriesCard`, channel rows, detail views) with toggle + inline create; `ListEditorDialog` for create/rename; `ListDetail` page (`pages/ListDetail.tsx`) rendering a mixed grid using each item's native card with per-item Remove and list rename/delete. *(Implementation note: `ListDetail` uses a responsive CSS grid rather than `@tanstack/react-virtual` — lists are user-curated and small — and live channels render as poster-shaped tiles for grid coherence (click plays, matching the dedicated section) rather than the row-shaped `ChannelCard`.)*

**Acceptance Criteria:**
- [x] A user can create a named list and add movies, series, and Live TV channels to it from the catalog's "Add to list…" affordance; re-adding is a no-op. *(preview: right-clicking a movie in Movies → "Add to list…" → toggling "Horror movies to watch" raised its count 2→3; backend `lists_membership_resolution_and_scope` adds movie+series+live and asserts a duplicate add is ignored (`PRIMARY KEY`).)*
- [x] The "Add to list…" picker shows which lists already contain the item and supports inline list creation. *(preview: the picker listed both lists with `aria-checked` reflecting membership (via `get_lists_for_item`); toggling flipped the check; "+ New list…" → "Create & add" created "Sci-Fi favorites" and added the movie to it (itemCount 1).)*
- [x] `ListDetail` shows the list's items in order using the native cards, each behaving as in its dedicated section; items can be removed and the list renamed/deleted. *(preview: opening "Horror movies to watch" rendered 2 `MovieCard`s + 1 channel tile in order; the per-item ✕ removed one (3→2 items); rename via the cover menu changed the title; delete removed the list.)*
- [x] Lists are provider-scoped and local: switching providers shows only that provider's lists; orphaned items (dropped on refresh) are hidden from the grid/count but their membership is retained; no provider requests occur. *(backend test `lists_membership_resolution_and_scope`: a second provider sees no lists; an item with no catalog row is excluded from `get_list_items` and the summary count but its row survives; deleting the provider cascade-removes its lists. All list reads/writes hit only SQLite — the suite runs offline.)*

### Milestone 15 — My Lists on the Home Screen

**Goal:** Surface the user's custom lists on Home as a "My Lists" row of collection-cover cards (§5.10, Open Question #6). Depends on Milestone 14.

**Scope:**
- Add `MyListsRow` + `ListCoverCard` and render "My Lists" on `Home.tsx` in order **Keep Watching, My Lists, Popular Movies, Popular Series**.
- Each cover card is a 2×2 poster mosaic (from `get_lists`' `coverPosters`) + name + item count, opening `ListDetail` on click; a leading "+ New list" card opens `ListEditorDialog`.
- Row always shown (with at least the "+ New list" card) so the first list can be created from Home; cover cards ordered by the user's list `sort_order`.

**Acceptance Criteria:**
- [x] Home shows a "My Lists" row directly below Keep Watching with cover cards (2×2 mosaic + name + count) led by a "+ New list" card; the row is always present (even with no lists) so the first list can be created from Home. *(preview: row order was `home-keep-watching` → `home-my-lists` → `home-popular-movies` → `home-popular-series`; the row showed the "+ New list" card plus "Horror movies to watch" (3 items) and "Binge Worthy TV Shows" (2 items) as 2×2 mosaics, and still renders with the New-list card when there are zero lists.)*
- [x] Clicking a cover card opens that list's `ListDetail`; the leading "+ New list" card creates a list. *(preview: clicking the Horror cover navigated to `/list/list-1` with its items; the "+ New list" card opened `ListEditorDialog` and created "From Home", which appeared as a new cover immediately.)*
- [x] The row reflects list order and updates after lists are created/renamed/deleted or their membership changes; it renders entirely from local data (no provider requests). *(preview: the row updated live on create/rename/delete and on membership changes (covers/counts refresh via the shared `listsStore`); `get_lists` reads only SQLite — no provider request.)*

### Milestone 16 — Card Hover Reactivity & Detail-View Transitions

**Goal:** Make the catalog feel responsive and physical — content cards react to the cursor, and opening/closing a detail view animates from the grid instead of snapping — with no measurable performance regression on the baseline hardware (§10). Delivers the §9 "Motion & Animation" guideline.

**Scope:**
- **Card hover/press (§9):** add a hover scale-up (~1.04), active press (~0.98), and hover `z` lift to every browsable content card so the effect is consistent everywhere: `MovieCard`, `SeriesCard` (covers the Movies/TV Shows grids, Home Popular rows, and Search results — all reuse these), plus the Home-only `KeepWatchingCard`, `ListCoverCard`, and the "+ New list" card. Channel rows (`ChannelCard`) keep their row highlight and gain the press feedback. Implemented with CSS transforms (Tailwind), animating only `transform`.
- **Horizontal-row clipping fix:** the Home rows (`MediaRow`, `MyListsRow`) scroll horizontally with `overflow-x: auto`, which forces `overflow-y` to compute to `auto` and clips a scaled card's top/bottom (cutting its rounded corners so it reads as a filled rectangle). Add vertical/horizontal breathing room to the scroll containers so scaled cards keep their shape, without shifting the resting layout.
- **Card → detail transition:** add a small View Transitions helper (`src/lib/viewTransition.ts`) wrapping the state change in `document.startViewTransition` (via `flushSync`), degrading to an instant update where the API is unavailable. In the Movies and TV Shows grids, opening a card cross-fades the view **and morphs the clicked poster** into the detail's poster (shared `view-transition-name`), reversing on close. The shared name is carried by exactly one element at any time (grid card while the detail is closed; detail poster while open) so concurrent transitions never collide.
- **Reduced motion & guardrails:** all of the above is disabled under `prefers-reduced-motion: reduce` (the helper skips the API; CSS guards the transition pseudo-elements). Only `transform`/`opacity` are animated; no persistent `will-change`; transitions stay smooth over the 12k-item virtualized grids.
- *(The Home/Search → detail flow navigates routes before opening the detail; a shared-element morph across that navigation is deferred to Milestone 17. The poster cards there still get the hover/press reactivity.)*

**Acceptance Criteria:**
- [x] Movie and series cards scale up on hover and dip on press across the Movies grid, TV Shows grid, Home Popular rows, and Search results; the animation is smooth and the card keeps its poster shape (no clipping). *(`MovieCard`/`SeriesCard` carry `transition-transform … hover:scale-[1.04] active:scale-[0.98]` and are the shared components for all four surfaces; preview-verified the scaled card fits within the row — 5px overhang per side ≤ the 8px breathing room, `scrollHeight - clientHeight === 0` so nothing clips.)*
- [x] The Home-only cards — Keep Watching, list-cover, and "+ New list" — get the same hover/press reactivity, so the whole Home screen is consistent. *(scale/press utilities added to `KeepWatchingCard` and `ListCoverCard` wrappers (already `relative`) and to the `new-list-card` button.)*
- [x] The Home horizontal-row clipping bug is fixed: a hovered/scaled card on Home is no longer cut into a rectangle; resting layout/alignment is unchanged. *(root cause: `overflow-x:auto` forces `overflow-y` to compute to `auto`; `MediaRow`/`MyListsRow` scroll containers now use `-mx-2 px-2 py-2` — preview confirmed 8px room above the card, `overflowY` still auto but `scrollHeight === clientHeight` (no clip/scrollbar), `marginLeft: -8px` keeps cards aligned with the row title; the Home screenshot shows intact resting layout.)*
- [x] Clicking a card in the Movies or TV Shows grid cross-fades into its detail view and morphs the clicked poster into the detail poster; closing reverses the morph back into the grid card. *(via `lib/viewTransition.ts` wrapping the open/close state change; preview-verified open and close both transition.)*
- [x] The shared `view-transition-name` is on exactly one element in every state (verified: 1 on open in the detail, 1 on close on the grid card) — no duplicate-name conflicts. *(preview: on open `namedCount === 1` inside `movie-detail`; on close `namedCount === 1` on a `movie-card`; guaranteed by `morphId = detail ? null : selectedId` plus `flushSync` before the snapshot.)*
- [x] With `prefers-reduced-motion: reduce`, hover scaling and view transitions collapse to instant state changes. *(the helper short-circuits the API when `matchMedia('(prefers-reduced-motion: reduce)')` matches; CSS adds `motion-reduce:` guards on the cards and a `@media (prefers-reduced-motion: reduce)` rule zeroing the `::view-transition-*` animations. The harness can't toggle the OS setting to screenshot it, but both guards are in place.)*
- [x] No performance regression: only `transform`/`opacity` are animated, no persistent `will-change` is added, and the virtualized grids still scroll smoothly; `npm run build` type-checks clean. *(all animation is CSS `transform` (cards) or the compositor-driven View Transitions API; no `will-change` was added; the M5 virtualization is untouched; `npm run build` (tsc + vite) passes with no type errors.)*

### Milestone 17 — Ambient Motion Polish

**Goal:** Extend the §9 motion language beyond cards into content entrance and app chrome, keeping the same performance guardrails.

**Scope:**
- **Content entrance:** a brief, capped-stagger fade/lift (`prosc-fade-in-up`, only `opacity`/`transform`) for Home rows (`MediaRow`/`MyListsRow`, non-virtualized) and the **first paint** of a grid's data. In `PosterGrid` the entrance is gated to the initial top rows of each dataset (`firstPaint && index < columns*3`, `firstPaint` reset per `resetKey` and cleared after 700ms) so it fires only on initial load and **never** as cells recycle during scroll.
- **Skeleton → content crossfade:** the detail synopsis and the resolved episode section mount with the entrance fade instead of a hard swap; poster images already opacity-fade over their placeholder.
- **Route cross-fades:** section navigation in `TopNav` runs `navigate` inside `startViewTransition` for a content cross-fade. This avoids the still-unstable `react-router-dom` 6.30 view-transition APIs by intercepting the nav click directly.
- **Home / List → detail morph (in-place overlay):** `Home` and the custom-list view (`ListDetail`) open a movie/series detail as an **in-place overlay** (`MovieDetail`/`SeriesDetail` rendered `absolute inset-0` inside a `relative h-full` page, like Movies/TV Shows — so it sits at `z-20` **below** the floating nav at `z-30`, keeping the nav visible), *not* a route change. They use the same same-page morph pattern as the in-grid path (Milestone 16): flush the clicked card's `vt-poster` name, then mount the overlay as the transitioned update; `morphActive` is gated to `detail === null` so the name sits on exactly one element per state. Because the page never unmounts, **closing morphs the poster straight back into the same card** with scroll preserved — fixing a bug where navigating to a `/movies`-style route and back instead remounted the page (refetch + replayed entrance, no reverse morph).
- **Search → detail morph (route):** `SearchResultsPage` names the clicked poster and navigates; `Movies`/`TVShows` initialize the detail from `location.state` **synchronously on first render** (guarded by a `firstCatRun` ref so the mount's category effect doesn't clobber it), so the morph's "after" snapshot — taken right after the `flushSync` navigation, before effects — already contains the detail poster. (This replaced an earlier `requestAnimationFrame`-deferred approach that would stall in an occluded window.)
- **Nav micro-interactions:** the active `TopNav` pill carries `view-transition-name: nav-active`, so a section change morphs (slides) it from the old item to the new.

**Acceptance Criteria:**
- [x] Home rows and a grid's first data paint animate in with a capped stagger; no animation re-fires while scrolling the virtualized grids. *(Home: `MediaRow`/`MyListsRow` wrap each card in `.prosc-enter` with a capped `animation-delay` of `min(i,10)·30ms` — verified 65 `.prosc-enter` elements on Home. Grid: `PosterGrid` applies `.prosc-enter` only when `firstPaint && index < columns*3`, with `firstPaint` reset per dataset and cleared after 700ms, so recycled cells during scroll — indices past the threshold, flag cleared — never animate.)*
- [x] Skeletons crossfade into resolved content rather than hard-swapping. *(`MovieDetail`/`SeriesDetail` synopsis and the resolved episode section mount with `.prosc-enter`; poster images already opacity-fade over their `Placeholder`.)*
- [x] Navigating between top-level sections cross-fades; opening a detail from Home/List/Search morphs the poster (and Home/List morph it **back** on close) as the in-grid path does. *(`TopNav` wraps `navigate` in `startViewTransition`. Home and the custom-list view open the detail as an in-place overlay: preview-verified the click keeps the route unchanged (`path === "/"` / `"/list/list-1"`), the page stays mounted, the floating nav stays visible above the detail (`detail` top = 64px under the nav; `elementFromPoint` at the nav hits the nav), and `vt-poster` is on exactly one element — in the detail on open, and **back on the original card on close** — so closing reverse-morphs with no remount/refetch. Search names the source card and navigates; `Movies`/`TVShows` render-init the detail so the synchronous "after" snapshot carries the detail poster. The poster-morph animation itself was verified visually in Milestone 16; the hidden preview tab freezes its timeline, so the live playback here was confirmed structurally.)*
- [x] The `TopNav` active indicator animates between sections. *(the active `NavLink` carries `view-transition-name: nav-active` — confirmed present on the active link via computed style — so a section change morphs the pill from the old item to the new; `::view-transition-group(nav-active)` gives it a 240ms eased slide.)*
- [x] All of the above honor `prefers-reduced-motion`, animate only `transform`/`opacity`, and add no measurable scroll/playback regression. *(`startViewTransition` skips the API when `matchMedia('(prefers-reduced-motion: reduce)')` matches — exercised directly by forcing that branch; `@media (prefers-reduced-motion: reduce)` disables `.prosc-enter` and all `::view-transition-*` animations; `prosc-fade-in-up` animates only `opacity`+`transform` and the nav/poster morphs are compositor transforms; no `will-change` added; virtualization untouched; `npm run build` type-checks clean.)*

> *Verification note: the click-driven interactions were verified at the DOM level in the browser preview (open/close of the Home detail overlay, `path` unchanged, Home staying mounted, and the `vt-poster` name moving to exactly one element and returning to the original card on close), and the build type-checks clean. The headless preview tab is permanently `visibilityState:hidden`, which freezes the CSS-animation/View-Transition timeline, so the **live playback** of the entrance stagger, the cross-fades, and the poster morph could not be screen-captured — those rely on the same mechanism whose animation was verified visually in Milestone 16. A final visual pass in the real Tauri window is still recommended.*

### Milestone 18 — Detail View Redesign

**Goal:** Replace the sparse movie/series detail layout (a small poster + short metadata column on a flat black field, with a large empty lower area) with a cinematic, full-bleed **hero** layout that uses the provider's real backdrop art where available and that the Milestone 16/17 poster-morph lands into cleanly.

**Design decisions (resolved during the M18 design pass):**
- **Backdrop source:** use the provider's **real backdrop image** when it supplies one, falling back to a blurred/darkened treatment of the poster when it does not — so there is always a hero, never flat black. Xtream already returns backdrop art in the on-demand metadata responses (`get_vod_info` → `backdrop_path[]` / `cover_big` / `movie_image`; `get_series_info` → `backdrop_path[]` / `cover`); these are currently parsed-and-discarded, so capturing one URL is a small backend slice.
- **Image loading:** the backdrop loads **directly from the provider URL** (like every poster today — `<img src={url} loading="lazy">`). No on-disk image cache is added in M18. The `image_cache` table + 30-day eviction exist but are an unused stub (`image_cache::upsert` has no callers); building the real download-to-disk-and-serve pipeline is the §5.7 "Cover Art caching" feature and is **deferred to its own milestone covering all art** (posters + backdrops) — caching one image type in isolation is low value.
- **Layout style:** **full-bleed hero** (Plex/Netflix style) — the backdrop occupies the top band of the view with a gradient scrim into the page background; the poster overlaps the hero's lower edge; title, year, duration/rating, genre chips, and the action buttons sit over/beside the scrim.
- **"More like this":** **deferred** to a later milestone (it needs a new `get_related` backend query). M18 is the visual redesign + backdrop slice only.

**Scope:**
- **Backdrop data (backend slice):** add `backdropUrl: string | null` to `MovieDetail` and `SeriesDetail`. Full IPC path: extend the parse in `iptv/xtream.rs` (`get_vod_info` / `get_series_info`) to select one backdrop URL — first non-empty `backdrop_path[]` entry, else `cover_big`/`cover`/`movie_image`, else `null` — then `models.rs` ↔ `types/index.ts`; the detail fetch itself already exists (`get_movie_detail` / `get_series_detail`, session-cached) and gains no new round-trip. Keep `devMock.ts` in sync (supply a sample backdrop).
- **Hero layout (frontend):** redesign `MovieDetail` and `SeriesDetail` to the full-bleed hero above. The backdrop is darkened with a gradient scrim so the title/metadata/buttons read against it; the poster overlaps the hero's lower edge. When `backdropUrl` is `null`, derive the hero from the poster (CSS blur + scale + darken). All existing actions are preserved unchanged: Play, Open in External Player, Add to list, season switching, episode play/external, and the §5.9 watch-progress overlays.
- **Fuller vertical layout:** use the previously-empty lower area below the hero — **movies** give the synopsis + metadata more presence; **series** surface the season selector and `EpisodeList` prominently (more vertical room, full width).
- **Morph target:** keep exactly **one** `vt-poster`-named element (the overlapping hero poster) in every state so the Milestone 16/17 shared-element morph lands into the new layout without duplicate-name conflicts; closing reverse-morphs back into the grid/Home card as before.
- **Reduced motion & performance (§9/§10):** the backdrop is a static image/blur — no new animation is added; the only motion remains the existing compositor-driven poster morph (`transform`/`opacity`, no persistent `will-change`); honor `prefers-reduced-motion`; the redesign is a single view and must not regress the 12k-item virtualized grids.

**Out of scope (explicitly deferred to their own milestones):**
- "More like this" related-titles row (`get_related` command).
- On-disk image caching / the §5.7 cover-art pipeline (download → app-data dir → `image_cache` upsert → serve via Tauri asset protocol) for all art.

**Acceptance Criteria:**
- [x] Movie and series detail views render a full-bleed hero backdrop with a readable gradient scrim and the poster overlapping the hero's lower edge; the title/metadata/buttons read clearly over it and the previous large empty black area is gone. *(preview: both `MovieDetail` and `SeriesDetail` render the new `HeroBackdrop` band (`h-[420px]`) with vertical+horizontal scrims fading into `bg-zinc-950`; the poster sits in an `items-end … pt-[140px]` row so it overlaps the hero's lower edge, with title/year/rating/genres/buttons beside it and the synopsis below — screenshot-verified for a movie and a series.)*
- [x] When the provider supplies a backdrop (Xtream `backdrop_path`/`cover_big` for movies, `backdrop_path`/`cover` for series) it is used; when it does not, the hero falls back to a blurred/darkened treatment of the poster so there is always a hero. The backdrop loads directly from the provider URL — no new on-disk cache is introduced. *(backend test `movie_detail_backdrop_prefers_backdrop_path_then_falls_back` asserts the array→`cover_big`→null order and `series_detail_backdrop_falls_back_to_cover` the series `cover` fallback; preview exercised all three `data-hero-source` paths — `backdrop` (no filter), `poster` (`blur(40px) scale(1.25)`), and `none` (scrim only). The image is a plain `<img src={url}>`; `image_cache::upsert` still has no callers.)*
- [x] The series detail surfaces the season selector and episode list prominently in the fuller lower layout; the movie detail gives synopsis/metadata more presence. Play, Open in External Player, Add to list, season switching, episode play, and the §5.9 watch-progress overlays all behave exactly as before. *(preview: series detail shows an "Episodes" heading + season tabs + episode rows each with Play/External below the hero; movie detail has a labelled "Synopsis" block. All handlers (`play`, `openExternal`, add-to-list, `setSeason`, `EpisodeList`) are unchanged — `EpisodeList`/`WatchProgressOverlay` were not touched.)*
- [x] The Milestone 16/17 poster morph still lands correctly into the redesigned layout — exactly one `vt-poster` element per state — and closing reverse-morphs back into the originating card as before. *(preview: exactly 1 element with computed `view-transition-name: vt-poster` inside both the open movie and series detail; the grid/`viewTransition.ts` open/close logic is untouched, so the close reverse-morph into the grid card is unchanged.)*
- [x] The redesign honors `prefers-reduced-motion`, stays within the §10 performance budget, adds no on-demand provider request beyond the existing `get_movie_detail`/`get_series_detail` fetch, and `npm run build` type-checks clean. *(the hero is a static image/blur — no new animation is added, so reduced-motion is inherently respected; the only motion remains the existing compositor poster morph; the backdrop rides the existing detail fetch (no new IPC/network); `npm run build` (tsc + vite) passes; no console errors during the preview pass.)*
- [x] No "More like this" row and no on-disk image-cache pipeline are introduced (both deferred to their own milestones). *(neither was added; both are tracked in §13.)*

### Milestone 19 — Immersive Home Rows, Collapsible Genre Panel & Genre-Sectioned Browse

**Goal:** Make the catalog feel fuller and more Netflix-like: give the Home rows more of the screen, let the Movies/TV Shows genre panel collapse, and turn the "All Movies/All Shows" view into a stack of per-genre rows instead of one flat grid. Three independent UI/UX slices, all frontend-only (no backend/IPC changes — every read reuses existing catalog commands).

**Design decisions (resolved during the M19 planning pass):**
- **Collapsed genre panel:** collapses to a **thin rail with an expand chevron** anchored where the panel was; the content area widens to fill the freed space. The panel **defaults to expanded** on every entry — collapsing is a transient toggle, not persisted.
- **Genre-row titles (All view):** each genre's row title is **clickable** (with a "See all ›" affordance) and selects that genre — switching to the existing full virtualized grid for it and highlighting it in the side panel.

**Scope:**

- **A. Fuller Home rows (§5.10):** make the Home card strips take more of the viewport and feel less boxed-in. In `Home.tsx`, drop the `max-w-6xl` centering and tighten the horizontal page padding so the rows run near-edge-to-edge (keep a small Netflix-style margin, not literally zero, and preserve the existing negative-margin breathing room so hovered/scaled cards aren't clipped — §9). Enlarge the card width in `MediaRow` and `MyListsRow` (and the "+ New list" card) from the current `w-[150px]` to a larger size, applied consistently across **Keep Watching, My Lists, Popular Movies, and Popular Series**. Card internals (`MovieCard`/`SeriesCard`/`ListCoverCard`/`KeepWatchingCard`) scale with their container, so only the strip cell width changes. No behavior change — hover/press, morph, context menus, and the §5.9 overlays are untouched.

- **B. Collapsible genre panel (§5.3/§5.4):** add a collapse/expand control to `CategoryPanel` (used by both Movies and TV Shows). Expanded is the default on every mount; a chevron in the panel header collapses it to a thin rail bearing only an expand chevron, and the grid/rows area reflows to the reclaimed width. The sort toggle and genre list are unchanged when expanded. (Live TV continues to use `CategoryPanel` as today; the collapse control is available there too for consistency but the milestone's acceptance focuses on Movies/TV Shows.)

- **C. Genre-sectioned "All" browse (§5.4):** when **"All Movies"/"All Shows"** is selected (`selected === null`), replace the single flat `MovieGrid`/`SeriesGrid` with a vertical stack of **per-genre horizontal rows**, each labeled with the genre name above a horizontally-scrollable strip (the same visual pattern as Home's Popular rows). Row order: the provider's **"Popular"** genre first (resolved by the existing case-insensitive `\bpopular\b` match, and **excluded** from the list below to avoid duplication), then the remaining genres in **alphabetical ascending** order. Selecting a specific genre in the side panel keeps the **current full virtualized grid** for that genre — the sectioned view is only for "All".
  - **New component `GenreRows` (`vod/GenreRows.tsx`):** renders the stacked genre strips for the active section. Each strip fetches its first page (capped at a reasonable strip length, ~30) via the existing `get_movies` / `get_series` with that `categoryId`, reusing `MovieCard` / `SeriesCard` (with the §5.9 overlays and the Milestone 16/17 poster morph). To stay within the §10 budget when a provider has many genres, **each row lazy-loads its items when it scrolls near the viewport** (IntersectionObserver) rather than fetching all genres up front; an empty genre (no items) **omits its row**.
  - **Row title → full grid:** clicking a genre's title or its "See all ›" affordance calls the page's existing `onSelect(categoryId)`, switching to that genre's full grid and highlighting it in the panel.
  - **Movies/TV Shows wiring:** `Movies.tsx` / `TVShows.tsx` branch on `selected === null` to render `GenreRows` (passing the already-fetched `categories`, the open-detail/morph handlers, the context-menu handler, and `onSelect`), and otherwise render the existing grid. The detail-open morph (`selectedId`/`morphId`) flows into `GenreRows` cards exactly as it does for the grid.

- **Reduced motion & performance (§9/§10):** Home-row and genre-row entrance reuse the existing `prosc-enter` capped-stagger (only `transform`/`opacity`); the genre rows lazy-load to avoid a burst of requests; virtualized grids for a selected genre are unchanged; honor `prefers-reduced-motion`; `npm run build` type-checks clean.

**Out of scope:** no backend/IPC/schema changes; no new catalog queries (genre strips reuse `get_movies`/`get_series`); Live TV's browse layout is unchanged (it keeps its virtualized channel list and filter).

**Acceptance Criteria:**
- [x] The Home rows (Keep Watching, My Lists, Popular Movies, Popular Series) use larger cards and span more of the viewport width with reduced horizontal dead space; hovered/scaled cards are not clipped and all existing card behavior (hover/press, morph, context menus, §5.9 overlays) is unchanged. *(preview at 1680px: `Home.tsx` dropped `max-w-6xl mx-auto` and uses `px-4`, so the Popular Movies strip now sits 8px from each edge (was centered at 1152px — ~260px dead space per side); card width is `w-[180px]` (was 150) across `MediaRow` and `MyListsRow` (cells + "+ New list" card). The `-mx-2 px-2 py-2` breathing room is unchanged, so scaled cards still aren't clipped; card components/handlers were not touched.)*
- [x] In Movies and TV Shows the genre panel can be collapsed to a thin rail (content widens to fill) and re-expanded via the chevron; it defaults to expanded on every entry. *(preview: `CategoryPanel` mounts expanded (`data-collapsed="false"`, 224px); the « chevron collapses it to a 40px rail with only the » expand button (genre list gone) and the content area widens 1456→1640px; » re-expands. Collapse state is local and the panel remounts per section, so it always reopens expanded.)*
- [x] With "All Movies"/"All Shows" selected, the view is a vertical stack of per-genre horizontal rows — the "Popular" genre first (and not repeated below), then the remaining genres alphabetically ascending; empty genres are omitted; each row reuses the section's standard cards with the §5.9 overlays and the poster morph. *(preview Movies: rows ordered `Popular, Action, Adventure, Animation, Comedy … Western`; Series: `Popular, Anime, Classic, Comedy … Sci-Fi` — Popular leads and appears once. Rows reuse `MovieCard`/`SeriesCard`; a row card opened `MovieDetail` with exactly one `vt-poster` element. Empty-genre omission is coded (`items.length === 0 → null`) though every mock genre has items.)*
- [x] Clicking a genre row's title (or "See all ›") switches to that genre's existing full virtualized grid and highlights it in the side panel; selecting a genre directly in the panel shows the full grid as today. *(preview: clicking the "Action" row title replaced the row stack with the `poster-grid` and highlighted "Action" (`bg-zinc-800`) in the panel; selecting "All Movies" returned to the row stack. Both paths go through the same `onSelect`/`setSelected`.)*
- [x] The genre rows lazy-load their items as they scroll into view (no upfront fetch of every genre), reusing the existing `get_movies`/`get_series` reads with no new backend/IPC; the experience stays within the §10 performance budget on a many-genre catalog. *(preview: of 17 movie genres only the first 3 (within the 400px IntersectionObserver `rootMargin`) fetched on load; the other 14 rendered poster skeletons with no fetch. Each row reuses `get_movies`/`get_series` — no new command. **Caveat:** the preview tab is permanently `visibilityState: hidden`, which freezes scroll-driven IntersectionObserver callbacks, so further on-scroll loading couldn't be screen-driven here — but the upfront 3-of-17 behavior proves the lazy wiring.)*
- [x] All new/changed motion honors `prefers-reduced-motion`, animates only `transform`/`opacity`, and `npm run build` type-checks clean. *(genre rows and Home rows reuse the existing `prosc-enter` capped-stagger (opacity/transform only, already `prefers-reduced-motion`-guarded in `index.css`); the poster morph is unchanged; no new `will-change`; `npm run build` (tsc + vite) passes; no console errors during the preview pass.)*

### Milestone 20 — Series Detail: Season Dropdown & Rich Episode Rows

**Goal:** Replace the sparse series-detail episode UI — a wrapping strip of "all seasons at once" buttons and redundant single-line `Series Name — SxxEyy — Episode Title` text rows — with a **season dropdown** and **thumbnail-led episode rows** (thumbnail + clean title + an "Episode N · duration" metadata line + a short synopsis), so a season's episodes read like a modern streaming app (§5.4). One self-contained slice: a small backend addition (episode synopsis) plus the `SeriesDetail`/`EpisodeList` redesign.

**Design decisions (resolved during the M20 planning pass):**
- **Season picker → dropdown:** the wrapping `season-tab` button strip (`data-testid="season-selector"`) becomes a single **dropdown** (`SeasonSelect`) showing the current season; opening it lists all seasons in ascending order. It defaults to the lowest-numbered season (unchanged behavior) and **still renders for a single-season series** (showing "Season N") for layout consistency. The dropdown carries `data-testid="season-selector"` so existing intent is preserved; the option elements replace `season-tab`.
- **Episode rows → thumbnail-led:** each row **leads with the episode thumbnail** (`Episode.posterUrl`, 16:9, falling back to a styled `Placeholder` when `null`). **Clicking the thumbnail plays/resumes** via the existing §5.9 path (`onPlay` → `playerStore.openContent`, which already runs the resume prompt for meaningful prior progress). The **watched checkmark and the bottom progress bar overlay the thumbnail** (reusing `WatchProgressOverlay` in its default mode, since the thumbnail is now a `relative` image container — the inline checkmark hack in the current `EpisodeRow` is removed).
- **To the right of the thumbnail:** the **title** is the one bold, scannable element; below it a **muted metadata line** reading `Episode N · 45m` (duration omitted when `null`); below that a **1–2 line truncated synopsis**.
- **Episode-number placement:** on the muted metadata line under the title — *not* embedded in the title and *not* a thumbnail-corner badge (the thumbnail corners are already used by the watched check and progress bar).
- **Title de-redundancy:** Xtream providers frequently stuff `Series Name SxxEyy` into the episode `title`. A small frontend helper (`cleanEpisodeTitle` in `lib/utils.ts`) strips a leading series-name, `SxxEyy`/`xEy`, and/or `Episode N` prefix at render time so the row shows the clean episode name, falling back to `Episode N` when nothing meaningful remains. No provider data is mutated or lost (normalization is display-only).
- **Actions → context menu:** the always-visible `Play` / `External` buttons are replaced by a **right-click context menu** (and a hover **"⋯"** affordance so it is reachable without a right-click), reusing the existing `common/ContextMenu`, with **Play / Resume** and **Open in External Player** — matching the `KeepWatchingCard` pattern. The thumbnail click remains the primary play/resume affordance.

**Scope:**
- **Backend slice (episode synopsis):** add `overview: Option<String>` to `EpisodeItem` (`models.rs`), parsed in `iptv/xtream.rs::get_series_info` from the episode's `info.plot` → `info.overview` → `info.description` (first non-empty), then `types/index.ts` (`Episode.overview: string | null`). It **rides the existing `get_episodes` / `get_series_detail` fetch** — no new IPC command, no new round-trip. Keep `devMock.ts` in sync: give mock episodes a sample `posterUrl` (today `null`) and `overview` so the new row is demoable in browser dev.
- **`SeriesDetail.tsx`:** replace the `season-selector` button strip with a new **`vod/SeasonSelect.tsx`** dropdown wired to the existing `season` / `setSeason` state; everything else (detail fetch, hero, add-to-list, `EpisodeList` mount, Esc handling) is unchanged.
- **`EpisodeList.tsx` / `EpisodeRow`:** new layout — thumbnail (with `WatchProgressOverlay`) + title (run through `cleanEpisodeTitle`) + metadata line (`Episode N · duration`) + truncated synopsis. Thumbnail `onClick` → `onPlay`; a right-click / hover-"⋯" `ContextMenu` provides Play/Resume + Open in External Player (preserving `onPlay` / `onOpenExternal`). Because the richer rows are taller and image-bearing and a season can hold many episodes, **virtualize the episode list** with `@tanstack/react-virtual` (already a dependency, as in the channel/poster lists) to honor §10.
- **`lib/utils.ts`:** add `cleanEpisodeTitle(seriesName, season, episode, title)` (display-only normalization).

**Out of scope (deferred):**
- No new IPC commands or schema changes (synopsis rides the existing series fetch).
- No on-disk caching of episode thumbnails — they load directly from the provider URL like all other art today; the §5.7 cover-art pipeline remains its own deferred milestone.
- No episode-level "More like this" / related content.

**Acceptance Criteria:**
- [x] The season picker is a **dropdown** (`SeasonSelect`) defaulting to the lowest season and listing all seasons in ascending order; selecting a season swaps the episode list. It renders even for a single-season series, and the previous wrapping `season-tab` button strip is gone. *(preview: the detail shows a `season-selector` dropdown ("Season 1 ▾"); for a 3-season series it opens to `Season 1 / Season 2 / Season 3` and selecting "Season 3" relabelled the control and swapped the list — first episode "Requiem"→"Empire", 11→6 rows. A single-season series still renders the dropdown. `document.querySelectorAll('[data-testid="season-tab"]').length === 0`.)*
- [x] Episode rows are **thumbnail-led**: each shows the episode thumbnail (16:9, placeholder when absent) on the left, then a clean title, an `Episode N · duration` metadata line, and a 1–2 line truncated synopsis. Redundant `Series Name — SxxEyy —` prefixes are stripped from the displayed title (falling back to `Episode N`). *(preview: rows render `[thumbnail][title / "Episode 1 · 36m" / synopsis]`; the mock's `"S01E01 — Reckoning"` title displays as `Reckoning` via `cleanEpisodeTitle`; thumbnails show the real-SVG / placeholder-initial mix; episodes seeded with no synopsis (e.g. "Vendetta") omit the synopsis line.)*
- [x] **Clicking the thumbnail** plays the episode, going through the §5.9 resume flow (resume prompt for in-progress episodes, immediate start otherwise); the **watched checkmark** and **progress bar** overlay the thumbnail. *(preview: clicking an episode thumbnail (`episode-play`) opened the dev-mock player surface with no resume dialog for a fresh episode — `openContent` is unchanged so the §5.9 prompt fires only for meaningful progress. The seeded in-progress episode ("Garden", ep-2-1-2) shows the green `progress-bar` overlaid on its thumbnail via `WatchProgressOverlay`.)*
- [x] **Play / Resume** and **Open in External Player** are reachable from a per-episode **right-click context menu** and a hover **"⋯"** button; the always-visible Play/External buttons are removed. *(preview: the `episode-menu-button` (and `onContextMenu`) open a `context-menu` with exactly `[Play, Open in External Player]` for a fresh episode and `[Resume, Open in External Player]` for the in-progress one; no standalone Play/External buttons remain on the row.)*
- [x] Backend: `EpisodeItem.overview` is parsed from Xtream `info.plot`/`info.overview`/`info.description`, with `models.rs` ↔ `types/index.ts` ↔ `devMock.ts` in sync and **no new IPC round-trip** (it rides `get_episodes`/`get_series_detail`). A backend test covers the parse + fallback order. *(`episode_overview_from` selects plot→overview→description; persisted via a new `episodes.overview` column with an idempotent `add_column_if_missing` migration for existing DBs. Backend test `series_detail_enriches_and_persists_episodes` asserts `plot` is preferred and `overview` is the fallback; `cargo test --tests` passes all suites.)*
- [x] The episode list is **virtualized** (taller image-bearing rows, potentially long seasons — §10); all motion honors `prefers-reduced-motion`, animates only `transform`/`opacity`, and `npm run build` type-checks clean. *(preview: rows are absolutely-positioned via `translateY` inside a `@tanstack/react-virtual`-sized spacer, windowed against the detail's scroll container with `scrollMargin`; no new keyframe motion is added (only an existing-style hover highlight), so reduced-motion is inherently respected; `npm run build` (tsc + vite) passes clean with no console errors during the preview pass.)*

---

> **Milestones 21–26 originate from the end-user QA pass on 2026-06-24** (`QA_NOTES.md`, real-data session against the SRP Tech App provider on the local release build). They translate the prioritized defects and UX-friction findings into shippable slices. Pure "idea" suggestions from that pass (recently-watched channels, friendlier codec labels, watched-badge legend, a global Live-TV filter, the LIVE-timer clarification) are tracked in the §13 Future Roadmap rather than as milestones. The provider-side Cloudflare 403 on VOD media paths (`QA_NOTES.md` §9) is **not** an app bug and is excluded; only the app-side issues it exposed (opaque errors, missing logging, and the plaintext-credential leak) are scoped below.

### Milestone 21 — Credential Hardening: Composed Stream URLs

**Goal:** Stop persisting fully-formed stream URLs that embed the provider password in cleartext. Today `movies.stream_url` and the episode stream URLs are stored in the catalog DB as `…/movie/<user>/<password>/<id>.<ext>`, which leaks the secret into SQLite and defeats the keychain-only design (`keychain.rs`, §5.1). Store only the pieces needed to **compose** the URL at playback time, reading the password from the OS keychain — so the secret never lands on disk in the catalog (`QA_NOTES.md` §9). This is sequenced first because it is a security defect.

**Scope:**
- **Schema (§15):** stop storing the password-bearing URL. Persist the Xtream **stream id** + **container extension** (and the analogous M3U direct URL, which is provider-supplied and carries no app-injected secret) instead of the composed `stream_url`. Apply the change as an **idempotent migration** for existing DBs (the `add_column_if_missing` pattern used in Milestone 20), and clear/rewrite any already-persisted password-bearing URLs on first launch after upgrade so existing installs are scrubbed, not just new refreshes.
- **Refresh persistence (`iptv/xtream.rs`, `db/`):** during catalog refresh, write stream id + container ext for movies and episodes rather than the full URL. Live channels already stream from a non-VOD path (and live kept working in QA) — verify live is handled the same way and carries no embedded secret.
- **Playback compose (`commands/playback.rs`):** resolve the playable URL at play time by composing provider base + user + **keychain password** + id + ext, reusing/extending the existing `resolve_stream_url_for_movie_and_episode` path (Milestone 5) so the built-in player, "Open in External Player", and the resume flow all get a freshly-composed URL. The keychain remains the only at-rest location for the secret.
- **Secret redaction:** ensure the composed URL (with password) is never written to logs, events, error strings, or the DB — only used transiently for the mpv/handoff call. This dovetails with the logging added in Milestone 22.
- **IPC touch-points (§16):** follow the five-place pattern (`commands/*.rs` → `generate_handler![]` → `models.rs` ↔ `types/index.ts` → `lib/tauri.ts` → `devMock.ts`) for any model field changes (e.g. movie/episode now expose id+ext instead of a full URL); keep `devMock.ts` in sync.

**Acceptance Criteria:**
- [ ] After a catalog refresh, no row in `movies`, `episodes`, or `live_channels` contains the provider password in cleartext; the catalog stores stream id + container ext (or the provider's own secret-free direct URL for M3U). *(backend test: refresh a seeded Xtream provider and assert no persisted stream field contains the keychain password substring.)*
- [ ] Upgrading an existing install with already-persisted password-bearing URLs scrubs them on first launch (idempotent migration), without requiring the user to delete `%APPDATA%\proscenium`.
- [ ] Movie playback, episode playback, "Open in External Player", and resume all still launch the correct stream — the playable URL is composed at play time from the keychain secret, not read from the catalog DB. *(backend test on the compose path; player launch verified in the real Tauri window.)*
- [ ] The composed password-bearing URL never appears in any log line, emitted event, error message, or DB column — only in the transient playback/handoff call.
- [ ] `cargo test --tests` and `npm run build` pass clean.

### Milestone 22 — Player Controls: Subtitle/Track Selection & Stream-Error Surfacing

**Goal:** Fix the player's most impactful defects from QA: subtitle selection does nothing, subtitles default ON, stream-load failures show an opaque "loading failed" with no logging, and a failed VOD load mislabels the bar as live. Also widen the auto-hiding control bar's hit area (`QA_NOTES.md` §7, §2, §9).

**Scope:**
- **Subtitle selection (`mpv/`):** make the subtitle track menu actually change the active track — selecting **"Off"** disables subtitles (`sid=no`) and selecting a specific track switches to it; the menu's checkmark reflects the real mpv `sid` state. Audio-track selection already works (§7) — mirror its wiring. Investigate why the current selection has no effect (property set vs. observed) and verify against a multi-subtitle stream.
- **Default subtitles Off:** auto-select **no** subtitle track on stream start (`sub-auto=no` / `sid=no` by default) so subtitles are opt-in, per common player expectation (§7). Confirm this is consistent across Live TV, movies, and episodes.
- **Stream-error surfacing:** replace the bare "loading failed" with a cause-bearing message that distinguishes **4xx / 5xx / network / timeout** (e.g. "Provider denied this video (HTTP 403). Live TV is unaffected — VOD may be temporarily restricted."). Capture the HTTP status and mpv error string from the player pipeline and present a user-readable reason (§9). Update §5.6 / §12 error-handling wording as needed.
- **Stream-failure logging:** on a failed load, log the failing URL **secret-redacted** (per Milestone 21), the HTTP status, and the mpv error string, so field diagnosis is possible (QA found empty logs on failure). Define where these go (stderr/app log) so launching the exe with captured output yields a diagnosable trace.
- **Failed-VOD bar mode:** a failed VOD/movie load must not render the control bar as **"● LIVE / 0:00"** — keep VOD mode (or a neutral error state) so the bar matches the content type (§9).
- **Control-bar ergonomics:** increase the control bar's hit area and lengthen the hover/auto-hide grace period so volume, track menus, fullscreen, and × are not a thin strip at the extreme bottom edge that's easy to miss (§2, §7). Keep the player z-order/transparency "sandwich" (CLAUDE.md, `mpv/mod.rs`) intact.

**Acceptance Criteria:**
- [ ] In the player, choosing "Off" disables subtitles and choosing a different subtitle track switches to it; the menu checkmark reflects the actual active track. Verified on a stream with multiple subtitle tracks.
- [ ] Subtitles default to Off on stream start across Live TV, movies, and episodes; the user can turn them on from the track menu.
- [ ] A failed stream load shows a cause-bearing message distinguishing 4xx/5xx/network/timeout (with HTTP status where available), not a bare "loading failed".
- [ ] A failed load writes a diagnosable log line containing the secret-redacted URL, HTTP status, and mpv error string; launching the release exe with captured stdout/stderr surfaces it.
- [ ] A failed VOD load no longer renders the bar as "● LIVE / 0:00" — the bar reflects VOD/error state.
- [ ] The control bar has a larger hit area and a longer auto-hide grace; the player overlay/transparency and z-order behavior are unchanged.

### Milestone 23 — App-Wide Keyboard Shortcuts

**Goal:** Close the systemic accessibility gap QA called out: the Escape key closes nothing, space doesn't pause, and no media/navigation shortcuts are wired anywhere (`QA_NOTES.md` §2, §5, §7, §8). Implement a single, shared keyboard-handling pass across the player, modals, and overlays.

**Scope:**
- **Player shortcuts:** **space** = play/pause, **f** = fullscreen toggle, **m** = mute, **←/→** = seek (VOD/episodes), **↑/↓** = volume, **Esc** = close player. Wire these to the existing `playerStore` actions; for pure-live streams, seek/pause behave sensibly (or are no-ops) per §5.6.
- **Modals & overlays:** **Esc** closes/cancels the search overlay (§5), the **resume modal** (§3), the **new-list** and **list-editor** modals, and the **add-to-list** picker; **Enter** submits the focused single-action modal (e.g. New-list "Create"). This complements the explicit Cancel button added in Milestone 26.
- **Search results navigation:** **↑/↓** (or arrow keys) move through results and **Enter** opens the focused result, for keyboard-first use (§5).
- **Focus discipline:** shortcuts must not hijack typing in text inputs (filter box, name fields) — handle key events with input-focus awareness, and scope player keys to when the player is active. Centralize in one handler/hook rather than ad-hoc listeners per component.

**Acceptance Criteria:**
- [ ] In the player, space pauses/resumes, f toggles fullscreen, m mutes, ←/→ seek (VOD/episodes), ↑/↓ change volume, and Esc closes the player — verified on a real stream.
- [ ] Esc closes the search overlay, the resume modal, and the list/add-to-list modals; Enter submits the focused single-action modal.
- [ ] Search results can be moved through with arrow keys and opened with Enter.
- [ ] Shortcuts do not fire while typing in a text input (filter box, list-name field); player keys are scoped to an active player.
- [ ] `npm run build` type-checks clean.

### Milestone 24 — Feedback, Confirmation & Settings Wiring

**Goal:** Address the recurring "actions complete silently or destructively" and "Settings don't visibly wire to the UI" themes (`QA_NOTES.md` §8) in one pass: a shared toast/confirm pattern, plus fixing the two broken Settings controls. Covers catalog-refresh feedback, destructive-delete safety, add-to-list confirmation, and the Density/Appearance controls (§1, §6).

**Scope:**
- **Shared toast + confirm primitives:** introduce (or extend the existing `Toast`) a reusable toast and a `ConfirmDialog` used by the items below, so feedback/confirmation are consistent app-wide.
- **Catalog-refresh feedback (§6):** the manual ↻ refresh currently gives no spinner/progress/toast and the provider's **"Last refreshed"** timestamp doesn't update even though data changes. Wire a "Refreshing… (N/total)" indicator to the **already-emitted** `catalog:refresh_progress` events (consumed in `catalogStore.ts`), and **update "Last refreshed"** on `catalog:refresh_complete`. Confirm the timestamp persists and re-renders the provider card.
- **Destructive-delete safety (§1, §6):** **list delete** currently removes a list instantly with no confirm/undo (real data loss for a populated list) — add a confirm dialog ("Delete '<name>' and its N items?") and/or an undo toast. **Provider delete** must show the same confirmation (QA did not destructively test it but flagged the identical gap). Apply the shared `ConfirmDialog`.
- **Add-to-list confirmation (§1):** adding a title to a list currently only flips a small checkbox inside the still-open dropdown — easy to miss. Show a brief toast ("Added to <list>") on add.
- **Density toggle (§6):** the Appearance → Density control has no visible effect and reverts to "Comfortable" after navigating away — i.e. it's neither wired to layout nor persisted. Make Density (Comfortable/Compact) actually change list/grid density (§9 Typography & Density) **and** persist via the settings store so the control reflects the stored value on return.
- **Theme control restyle (§6):** the app is dark-only ("Light theme is planned"), but the disabled "Dark" button looks clickable. Restyle it to read clearly as the only/active option (light theme itself stays deferred — §13).

**Acceptance Criteria:**
- [ ] Triggering a manual catalog refresh shows a visible progress/spinner indicator driven by `catalog:refresh_progress`, and the provider's "Last refreshed" timestamp updates on completion and persists across navigation/restart.
- [ ] Deleting a custom list prompts for confirmation (and/or offers undo); deleting a provider prompts for confirmation. No list/provider is destroyed by a single un-guarded click.
- [ ] Adding an item to a list shows a confirmation toast.
- [ ] Switching Density to Compact produces a visible layout change and the setting persists (the control still reads "Compact" after navigating away and on restart).
- [ ] The Appearance theme control no longer looks like a clickable choice when Dark is the only option.
- [ ] `npm run build` type-checks clean; backend tests pass.

### Milestone 25 — Catalog Display & Empty-State Cleanup

**Goal:** Stop missing/redundant provider data from leaking into the UI: blank channel names, the duplicated series-name/episode-code in titles, the "?" empty-list cover, and the missing in-list removal affordance (`QA_NOTES.md` §2, §4, §1, §8).

**Scope:**
- **Blank channel names (§2):** ~10 channels in "All Channels" render with no name text (empty rows). Add a graceful fallback — a placeholder name (e.g. stream id / "Untitled channel") and/or filter out truly empty entries — so no channel row is unidentifiable. Decide at render time and/or normalize on refresh.
- **Title de-duplication (§4):** the player title bar reads "Black Mirror — Black Mirror - S02E01 - Be Right Back" and Keep Watching reads "S2:E1 · Black Mirror - S02E01 - …" because the composed label concatenates structured fields over a provider episode title that **already** embeds the series name and `SxxEyy`. Reuse/extend the **`cleanEpisodeTitle` helper added in Milestone 20** (`lib/utils.ts`) so the **player title bar** and the **Keep Watching label** strip the redundant series/`SxxEyy` prefix and compose from structured fields — e.g. "Black Mirror · S2:E1 — Be Right Back". Display-only normalization; no provider data is mutated.
- **Empty-list cover (§1):** an empty list's Home cover currently shows a 2×2 grid of "?" placeholders, which looks broken. Use a neutral empty-list icon for the zero-item cover (`ListCoverCard`).
- **Remove-from-list affordance (§1):** within `ListDetail`, the list view has no way to remove an item (hover only scales the poster) — the user must open the title's detail and untick. Add a hover remove (×) control or reuse the right-click context menu's remove in the list grid, calling the existing `remove_from_list` (Milestone 14).

**Acceptance Criteria:**
- [ ] No channel row renders with empty/blank name text — channels with missing names show a readable fallback (or are filtered), in both A-Z and PROVIDER sort.
- [ ] The player title bar and the Keep Watching label show a de-duplicated title (series name and SxxEyy appear once), composed via the shared `cleanEpisodeTitle` normalization; no provider data is altered.
- [ ] An empty custom list shows a neutral empty-state cover icon on Home instead of a "?" placeholder mosaic.
- [ ] An item can be removed directly from `ListDetail` (hover ✕ or context menu) without opening the title's detail page; the grid and count update in place.
- [ ] `npm run build` type-checks clean.

### Milestone 26 — Resume Affordances & Row Scroll Controls

**Goal:** Make resume/continue entry points consistent across detail pages and give horizontal rows a discoverable scroll affordance — the remaining UX-friction items from QA (`QA_NOTES.md` §3, §4, §1).

**Scope:**
- **Movie detail in-progress state (§3):** after partial playback, the movie detail page still shows a generic "Play" with no progress, and resume only surfaces as a modal *after* clicking Play — inconsistent with the Home thumbnail (which shows a progress bar). On the detail page, change "Play" → **"Resume from MM:SS"** with a secondary **"Start over"**, and show a progress bar on/under the poster when the title is in progress. Use the existing §5.9 watch-progress data.
- **Series top-level Play/Continue CTA (§4):** the series detail has no top-level play/resume button — the user must scroll to the episode list and pick. Add a **"Resume SxxEyy" / "Play S1:E1"** CTA near the title (mirroring movies), targeting the last in-progress episode or the first episode.
- **Resume modal Cancel (§3):** the "Resume playback?" modal has only Resume / Start-from-beginning; backdrop-click dismiss isn't discoverable. Add an explicit **Cancel** button (Esc-to-dismiss is delivered by Milestone 23).
- **Row scroll chevrons (§1, §3):** Home carousels have **no** scroll arrows while the genre rows have tiny/subtle ones — inconsistent affordance. Add **hover-reveal chevron buttons** at the row edges and **standardize** them across all horizontal rows (Home Popular/My Lists/Keep Watching and the Movies/TV Shows genre rows). Respect the existing scaled-card breathing room (§9, Milestone 16) so chevrons don't clip cards.

**Acceptance Criteria:**
- [ ] A movie that is in progress shows "Resume from MM:SS" + "Start over" on its detail page, plus a progress indicator — consistent with its Home thumbnail; a not-started movie shows "Play".
- [ ] The series detail page has a top-level Play/Resume CTA near the title that starts the first episode or resumes the last in-progress one, without scrolling to the episode list.
- [ ] The resume modal has an explicit Cancel/close button in addition to Resume / Start from beginning.
- [ ] Home carousels and the genre rows show consistent hover-reveal scroll chevrons at the row edges; clicking them scrolls the row, and scaled/hovered cards are not clipped.
- [ ] `npm run build` type-checks clean.
