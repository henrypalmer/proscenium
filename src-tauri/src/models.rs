use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Xtream,
    M3u,
}

impl ProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderType::Xtream => "xtream",
            ProviderType::M3u => "m3u",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "xtream" => Some(ProviderType::Xtream),
            "m3u" => Some(ProviderType::M3u),
            _ => None,
        }
    }
}

/// Provider profile as returned to the frontend. The real password never
/// crosses the IPC boundary; it lives in the OS keychain.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub playlist_url: Option<String>,
    pub local_file_path: Option<String>,
    /// Unix seconds.
    pub last_refreshed: Option<i64>,
    /// Unix seconds.
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub playlist_url: Option<String>,
    pub local_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveChannel {
    pub id: String,
    /// Origin provider (Milestone 39 multi-provider). Populated on read; empty
    /// in the fetch structs (the persist layer binds it separately).
    pub provider_id: String,
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub logo_url: Option<String>,
    /// Internal only: never serialized to the frontend (spec §5.1 / Milestone 21).
    /// For M3U it holds the provider's direct URL; for Xtream it is empty and the
    /// playable URL is composed at playback time from the keychain secret.
    #[serde(skip_serializing)]
    pub stream_url: String,
    pub stream_ext: String,
    pub epg_channel_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieItem {
    pub id: String,
    /// Origin provider (Milestone 39 multi-provider).
    pub provider_id: String,
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub poster_url: Option<String>,
    /// Internal only: never serialized to the frontend (Milestone 21). Empty for
    /// Xtream (composed at playback from the keychain secret); the direct URL for M3U.
    #[serde(skip_serializing)]
    pub stream_url: String,
    pub container_ext: String,
    pub release_year: Option<i64>,
    pub rating: Option<String>,
    pub added_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesItem {
    pub id: String,
    /// Origin provider (Milestone 39 multi-provider).
    pub provider_id: String,
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub poster_url: Option<String>,
    pub release_year: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeItem {
    pub id: String,
    /// Origin provider (Milestone 39 multi-provider).
    pub provider_id: String,
    pub series_id: String,
    pub season: i64,
    pub episode: i64,
    pub title: String,
    /// Internal only: never serialized to the frontend (Milestone 21). Empty for
    /// Xtream (composed at playback from the keychain secret); the direct URL for M3U.
    #[serde(skip_serializing)]
    pub stream_url: String,
    pub container_ext: String,
    pub duration_seconds: Option<i64>,
    pub poster_url: Option<String>,
    /// Short episode synopsis (Xtream `info.plot`/`overview`; spec §5.4, M20).
    pub overview: Option<String>,
}

/// Movie row enriched with on-demand metadata (Xtream `get_vod_info`).
/// §15 has no description column, so this metadata is session-cached only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieDetail {
    #[serde(flatten)]
    pub movie: MovieItem,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub duration_seconds: Option<i64>,
    /// Wide hero backdrop (spec §5.4, Milestone 18); null falls back to the poster.
    pub backdrop_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDetail {
    #[serde(flatten)]
    pub series: SeriesItem,
    pub description: Option<String>,
    pub genre: Option<String>,
    /// Wide hero backdrop (spec §5.4, Milestone 18); null falls back to the poster.
    pub backdrop_url: Option<String>,
}

/// Everything one full refresh produces; persisted atomically.
#[derive(Debug, Clone, Default)]
pub struct CatalogData {
    pub live_categories: Vec<Category>,
    pub live_channels: Vec<LiveChannel>,
    pub vod_categories: Vec<Category>,
    pub movies: Vec<MovieItem>,
    pub series_categories: Vec<Category>,
    pub series: Vec<SeriesItem>,
    pub episodes: Vec<EpisodeItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: i64,
    /// 1-based page number this result corresponds to.
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSummary {
    pub live_channels: i64,
    pub movies: i64,
    pub series: i64,
}

/// Saved watch progress for one VOD item (spec §5.9). Live TV is never tracked.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchProgress {
    /// Last playback position, seconds.
    pub position_seconds: i64,
    /// Total runtime when known, seconds.
    pub duration_seconds: Option<i64>,
    /// True once watched to the completion threshold (~95%).
    pub completed: bool,
    /// Unix seconds of the last write.
    pub updated_at: i64,
}

/// One in-progress item for the Home "Keep Watching" row (spec §5.10 / §16):
/// a movie or episode joined with its catalog row plus the saved progress.
/// Serialized as a `kind`-tagged union (`movie` / `episode`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContinueWatchingItem {
    Movie {
        movie: MovieItem,
        progress: WatchProgress,
    },
    Episode {
        episode: EpisodeItem,
        /// Parent series (for poster/title fallback); `None` if it's gone.
        series: Option<SeriesItem>,
        progress: WatchProgress,
    },
}

/// A custom user list / "playlist" (spec §5.11 / §16). Provider-scoped.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserList {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A list plus the data the Home "My Lists" cover card needs (spec §5.10):
/// the count of *resolvable* items and up to four cover posters.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSummary {
    #[serde(flatten)]
    pub list: UserList,
    pub item_count: i64,
    pub cover_posters: Vec<Option<String>>,
}

/// One resolved list item for the List Detail grid (spec §5.11), discriminated
/// by `kind` so the UI renders the matching card. Live channels carry no
/// watch progress (§5.9).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UserListItem {
    Movie { movie: MovieItem },
    Series { series: SeriesItem },
    Live { channel: LiveChannel },
}

/// Content-type narrowing for the `search` command (spec §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchContentType {
    All,
    Live,
    Movies,
    Series,
}

/// Results of a local FTS5 search, grouped by content type (spec §16).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub live_channels: Vec<LiveChannel>,
    pub movies: Vec<MovieItem>,
    pub series: Vec<SeriesItem>,
}

/// Related-titles for the "More like this" detail row (spec §5.4 / §16,
/// Milestone 28). Only the list matching the requested content type is filled;
/// the other stays empty. Resolved locally from the cached catalog.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedResults {
    pub movies: Vec<MovieItem>,
    pub series: Vec<SeriesItem>,
}

// --- Canonical catalog (Milestone 40: Cinemeta-backed) ---

/// One canonical catalog card (a Cinemeta meta preview), keyed by IMDB id.
/// Browse rows (Home/Movies/Series) are lists of these; clicking one resolves
/// playback sources across the enabled providers (M40 slice 3). `Deserialize`
/// so cached rows round-trip through the Tier-2 cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalItem {
    /// IMDB id (`tt…`) — the canonical key.
    pub imdb_id: String,
    /// "movie" | "series".
    pub kind: String,
    pub name: String,
    pub poster_url: Option<String>,
    pub release_year: Option<i64>,
}

/// Full canonical title metadata (Cinemeta `/meta`), loaded on detail open.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMeta {
    pub imdb_id: String,
    pub kind: String,
    pub name: String,
    pub poster_url: Option<String>,
    /// Wide hero backdrop; falls back to the poster.
    pub backdrop_url: Option<String>,
    pub description: Option<String>,
    pub release_year: Option<i64>,
    /// Raw Cinemeta release info ("1999" or a range like "2011–2019").
    pub release_info: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    /// Human-readable runtime ("136 min").
    pub runtime: Option<String>,
    pub imdb_rating: Option<f64>,
    /// TMDB id when Cinemeta exposes `moviedb_id` — the tmdb↔imdb bridge used to
    /// confirm provider matches (M40 slice 2). `None` when absent.
    pub tmdb_id: Option<i64>,
    /// Series episodes (Cinemeta `videos`); empty for movies.
    pub videos: Vec<CanonicalVideo>,
}

/// One canonical episode (Cinemeta `videos[]`). Season 0 = specials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalVideo {
    /// "tt…:season:episode".
    pub id: String,
    pub season: i64,
    pub episode: i64,
    pub name: String,
    pub overview: Option<String>,
    pub thumbnail: Option<String>,
    /// ISO release date when known.
    pub released: Option<String>,
}

/// One playable source for a canonical title (Milestone 40 source picker). An
/// IPTV provider source addresses `(provider_id, content_type, content_id)` and
/// plays through the existing player path; a direct-URL source (Stremio addons,
/// M41) instead carries `url`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamCandidate {
    /// Display label for the source (the provider or addon name).
    pub source: String,
    /// Provider source: the origin provider id (`None` for a direct-URL source).
    pub provider_id: Option<String>,
    /// "movie" | "episode".
    pub content_type: String,
    /// Provider source: the provider content id (`None` for a direct-URL source).
    pub content_id: Option<String>,
    /// Direct-URL source (addons): the playable URL (`None` for a provider source).
    pub url: Option<String>,
    /// Parsed quality tag ("2160p"/"1080p"/…) when known.
    pub quality: Option<String>,
    pub container: Option<String>,
    /// Match confidence 0..1 — drives picker ordering and a low-confidence hint.
    pub confidence: f64,
    /// True for an addon stream that is `infoHash`-only (no direct/debrid URL):
    /// surfaced as "needs a debrid service", not directly playable (M41).
    #[serde(default)]
    pub needs_debrid: bool,
}

/// An installed Stremio stream addon (Milestone 41). The token-bearing manifest
/// URL is the secret — it lives in the OS keychain, never in this struct or in
/// SQLite. `Deserialize` so the dev mock and tests can build one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StremioAddon {
    pub id: String,
    pub name: String,
    /// Declared content types ("movie"/"series"/…).
    pub types: Vec<String>,
    /// Declared resources ("stream"/…).
    pub resources: Vec<String>,
    /// Accepted id prefixes ("tt"/"tmdb"/…).
    pub id_prefixes: Vec<String>,
    pub position: i64,
    pub created_at: i64,
}

/// Payload for the `catalog:refresh_progress` event (spec §16).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshProgress {
    pub stage: String,
    pub progress: f32,
}

/// Payload for the `catalog:refresh_complete` event (spec §16).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshComplete {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub id: i64,
    pub title: Option<String>,
    pub lang: Option<String>,
    pub codec: Option<String>,
}

/// Built-in player state (spec §16 `MpvState`, plus `hwdecCurrent` so the
/// active hardware decoder is observable).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvState {
    pub playing: bool,
    pub paused: bool,
    /// Seconds.
    pub position: f64,
    /// Seconds; None for live streams.
    pub duration: Option<f64>,
    /// 0–100.
    pub volume: f64,
    pub muted: bool,
    pub buffering: bool,
    pub audio_tracks: Vec<TrackInfo>,
    pub subtitle_tracks: Vec<TrackInfo>,
    pub active_audio_track: Option<i64>,
    pub active_subtitle_track: Option<i64>,
    pub error: Option<String>,
    pub hwdec_current: Option<String>,
}

impl Default for MpvState {
    fn default() -> Self {
        Self {
            playing: false,
            paused: false,
            position: 0.0,
            duration: None,
            volume: 100.0,
            muted: false,
            buffering: false,
            audio_tracks: Vec::new(),
            subtitle_tracks: Vec::new(),
            active_audio_track: None,
            active_subtitle_track: None,
            error: None,
            hwdec_current: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XtreamAccountInfo {
    pub status: Option<String>,
    /// Unix seconds.
    pub exp_date: Option<i64>,
    pub max_connections: Option<i64>,
    pub active_connections: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    pub account_info: Option<XtreamAccountInfo>,
}

// --- Multi-view (Milestone 37) ---

/// Per-tile playback state pushed on the `mpv:tile_state` event so each grid
/// cell can show its own buffering/error/track state independently.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileState {
    pub tile_id: u32,
    pub state: MpvState,
}

/// A tile's destination rectangle reported by the frontend grid as **fractions
/// (0..1)** of the player area (top-left origin, +y down). Fractions, not pixels,
/// so the compositor maps them onto its own live drawable size — robust to the
/// WebView viewport and the host surface differing in size/scale (e.g. macOS,
/// where the CSS viewport is not the content size in points). See `mpv::compositor`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileRect {
    pub tile_id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// App settings (spec §15 settings keys), returned as a single object so the
/// UI can load every value with one call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub active_provider_id: Option<String>,
    pub cache_ttl_hours: i64,
    pub default_external_player: String,
    pub custom_player_command: Option<String>,
    pub ui_density: String,
    pub ui_theme: String,
    pub hw_decode_enabled: bool,
    /// Image cache ceiling in MB (spec §5.7, Milestone 27); LRU-evicted past this.
    pub image_cache_max_mb: i64,
}

impl Default for AppSettings {
    /// Spec §15 default values.
    fn default() -> Self {
        Self {
            active_provider_id: None,
            cache_ttl_hours: 6,
            default_external_player: "mpv".into(),
            custom_player_command: None,
            ui_density: "comfortable".into(),
            ui_theme: "dark".into(),
            hw_decode_enabled: true,
            image_cache_max_mb: crate::commands::images::DEFAULT_MAX_MB,
        }
    }
}

/// Health of the active provider, surfaced as a startup warning banner
/// (spec §12). For M3U providers `expired` is always false.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    /// The provider's server answered (HTTP reachable). False → "unreachable"
    /// banner with a retry action.
    pub reachable: bool,
    /// Xtream account status is "expired".
    pub expired: bool,
    /// Banner copy; `None` when the provider is healthy.
    pub message: Option<String>,
}

impl ConnectionTestResult {
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            account_info: None,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            account_info: None,
        }
    }
}
