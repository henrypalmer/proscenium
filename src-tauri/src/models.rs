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
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub logo_url: Option<String>,
    pub stream_url: String,
    pub stream_ext: String,
    pub epg_channel_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieItem {
    pub id: String,
    pub name: String,
    pub category_id: String,
    pub category_name: String,
    pub poster_url: Option<String>,
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
    pub series_id: String,
    pub season: i64,
    pub episode: i64,
    pub title: String,
    pub stream_url: String,
    pub container_ext: String,
    pub duration_seconds: Option<i64>,
    pub poster_url: Option<String>,
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
