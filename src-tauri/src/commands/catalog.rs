//! Catalog Tauri commands (spec §16): active provider selection and the
//! full catalog refresh with `catalog:refresh_progress` / `_complete` events.

use crate::db::{self, Db};
use crate::iptv::{m3u, xtream};
use crate::keychain;
use crate::models::{
    CatalogSummary, Category, EpisodeItem, LiveChannel, MovieDetail, MovieItem, PaginatedResult,
    Provider, ProviderType, RefreshComplete, RefreshProgress, RelatedResults, SeriesDetail,
    SeriesItem,
};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

pub const ACTIVE_PROVIDER_KEY: &str = "active_provider_id";
/// Milestone 39: the JSON array of enabled provider ids whose catalogs are
/// merged across every section. `active_provider_id` is kept as the "primary"
/// (first enabled) for the M37 multi-view default and the §12 status banner.
pub const ENABLED_PROVIDERS_KEY: &str = "enabled_provider_ids";
pub const CACHE_TTL_KEY: &str = "cache_ttl_hours";
pub const DEFAULT_CACHE_TTL_HOURS: i64 = 6;

/// Provider ids with a refresh currently in flight (prevents double refresh
/// from the manual button racing the startup stale check).
#[derive(Default)]
pub struct RefreshGuard(pub Mutex<HashSet<String>>);

/// Session cache for on-demand Xtream detail metadata, keyed by
/// `{provider_id}/{content_id}`. The §15 schema has no description/genre
/// columns, so this metadata is never persisted — it lives for the app run.
#[derive(Default)]
pub struct DetailCache {
    movies: Mutex<HashMap<String, xtream::VodInfo>>,
    series: Mutex<HashMap<String, xtream::SeriesInfo>>,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Spec §5.2: cache is stale when older than the TTL (default 6 hours).
/// A provider that has never refreshed is always stale.
pub fn is_cache_stale(last_refreshed: Option<i64>, ttl_hours: i64, now: i64) -> bool {
    match last_refreshed {
        None => true,
        Some(ts) => now - ts > ttl_hours * 3600,
    }
}

pub async fn cache_ttl_hours(pool: &SqlitePool) -> i64 {
    match db::settings::get(pool, CACHE_TTL_KEY).await {
        Ok(Some(v)) => v.parse().unwrap_or(DEFAULT_CACHE_TTL_HOURS),
        _ => DEFAULT_CACHE_TTL_HOURS,
    }
}

pub async fn get_active_provider_impl(pool: &SqlitePool) -> Result<Option<Provider>, String> {
    let Some(id) = db::settings::get(pool, ACTIVE_PROVIDER_KEY)
        .await
        .map_err(|e| format!("Failed to read settings: {e}"))?
    else {
        return Ok(None);
    };
    db::providers::get(pool, &id)
        .await
        .map_err(|e| format!("Failed to load provider: {e}"))
}

pub async fn set_active_provider_impl(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Provider, String> {
    let provider = db::providers::get(pool, provider_id)
        .await
        .map_err(|e| format!("Failed to load provider: {e}"))?
        .ok_or_else(|| format!("Provider {provider_id} does not exist."))?;
    db::settings::set(pool, ACTIVE_PROVIDER_KEY, provider_id)
        .await
        .map_err(|e| format!("Failed to save settings: {e}"))?;
    Ok(provider)
}

/// The enabled provider ids (Milestone 39), filtered to providers that still
/// exist. When the key has never been written (pre-M39 install) it falls back to
/// the legacy single active provider; once written it is respected literally
/// (including an empty set — the user disabling every provider).
pub async fn get_enabled_provider_ids(pool: &SqlitePool) -> Result<Vec<String>, String> {
    let all = db::providers::list(pool)
        .await
        .map_err(|e| format!("Failed to load providers: {e}"))?;
    let existing: HashSet<String> = all.into_iter().map(|p| p.id).collect();

    if let Some(json) = db::settings::get(pool, ENABLED_PROVIDERS_KEY)
        .await
        .map_err(|e| format!("Failed to read settings: {e}"))?
    {
        let ids: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
        return Ok(ids.into_iter().filter(|id| existing.contains(id)).collect());
    }
    // Pre-M39 fallback: the legacy single active provider, if it still exists.
    let active = db::settings::get(pool, ACTIVE_PROVIDER_KEY)
        .await
        .ok()
        .flatten()
        .filter(|id| existing.contains(id));
    Ok(active.into_iter().collect())
}

/// Persist the enabled provider set and keep `active_provider_id` pointing at the
/// first enabled provider (the M37/banner "primary").
pub async fn set_enabled_provider_ids(
    pool: &SqlitePool,
    provider_ids: &[String],
) -> Result<(), String> {
    let json = serde_json::to_string(provider_ids)
        .map_err(|e| format!("Failed to encode the provider set: {e}"))?;
    db::settings::set(pool, ENABLED_PROVIDERS_KEY, &json)
        .await
        .map_err(|e| format!("Failed to save settings: {e}"))?;
    match provider_ids.first() {
        Some(first) => {
            db::settings::set(pool, ACTIVE_PROVIDER_KEY, first)
                .await
                .map_err(|e| format!("Failed to save settings: {e}"))?;
        }
        None => {
            let _ = db::settings::delete(pool, ACTIVE_PROVIDER_KEY).await;
        }
    }
    Ok(())
}

/// Fetch the provider's full catalog and atomically replace the cached one.
/// `on_progress` receives (stage label, fraction 0..1).
pub async fn refresh_catalog_impl(
    pool: &SqlitePool,
    provider: &Provider,
    mut on_progress: impl FnMut(&str, f32),
) -> Result<(), String> {
    let data = match provider.provider_type {
        ProviderType::Xtream => {
            let server_url = provider
                .server_url
                .as_deref()
                .ok_or("Provider has no server URL.")?;
            let username = provider
                .username
                .as_deref()
                .ok_or("Provider has no username.")?;
            let password = keychain::get_secret(&provider.id)?;
            let creds = xtream::XtreamCreds {
                server_url,
                username,
                password: &password,
            };
            xtream::fetch_catalog(&creds, &mut on_progress).await?
        }
        ProviderType::M3u => {
            on_progress("Downloading playlist", 0.1);
            let bytes = if let Some(url) = provider.playlist_url.as_deref() {
                m3u::fetch_playlist_bytes(url).await?
            } else if let Some(path) = provider.local_file_path.as_deref() {
                m3u::read_playlist_file(path)?
            } else {
                return Err("Provider has no playlist URL or file path.".into());
            };
            on_progress("Parsing playlist", 0.5);
            let text = m3u::decode_playlist_bytes(&bytes)?;
            let outcome = m3u::parse_playlist(&text);
            if outcome.skipped_lines > 0 {
                eprintln!(
                    "m3u: skipped {} malformed #EXTINF line(s) for provider {}",
                    outcome.skipped_lines, provider.id
                );
            }
            outcome.catalog
        }
    };

    on_progress("Saving catalog", 6.0 / 7.0);
    db::catalog::replace_catalog(pool, &provider.id, &data, now_unix())
        .await
        .map_err(|e| format!("Failed to save the catalog: {e}"))?;
    on_progress("Done", 1.0);
    Ok(())
}

/// Run a refresh for `provider_id`, emitting progress and completion events.
/// Deduplicates concurrent refreshes per provider.
pub async fn run_refresh(app: AppHandle, provider_id: String) -> Result<(), String> {
    let pool = app.state::<Db>().0.clone();

    {
        let guard = app.state::<RefreshGuard>();
        let mut in_flight = guard.0.lock().unwrap();
        if !in_flight.insert(provider_id.clone()) {
            return Ok(()); // already refreshing this provider
        }
    }

    let result = async {
        let provider = db::providers::get(&pool, &provider_id)
            .await
            .map_err(|e| format!("Failed to load provider: {e}"))?
            .ok_or_else(|| format!("Provider {provider_id} does not exist."))?;
        refresh_catalog_impl(&pool, &provider, |stage, progress| {
            let _ = app.emit(
                "catalog:refresh_progress",
                RefreshProgress {
                    stage: stage.to_string(),
                    progress,
                },
            );
        })
        .await
    }
    .await;

    {
        let guard = app.state::<RefreshGuard>();
        guard.0.lock().unwrap().remove(&provider_id);
    }

    let _ = app.emit(
        "catalog:refresh_complete",
        RefreshComplete {
            success: result.is_ok(),
            error: result.as_ref().err().cloned(),
        },
    );
    result
}

/// Startup stale-cache check (spec §5.2): if the active provider's cache is
/// older than the TTL, refresh in the background.
pub async fn startup_stale_check(app: AppHandle) {
    let pool = app.state::<Db>().0.clone();
    // Give the WebView a moment to mount its event listeners.
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let ttl = cache_ttl_hours(&pool).await;
    let now = now_unix();
    let Ok(ids) = get_enabled_provider_ids(&pool).await else {
        return;
    };
    // Refresh each enabled provider whose cache is stale, sequentially so a
    // multi-provider setup doesn't fire every refresh at once (Milestone 39).
    for id in ids {
        if let Ok(Some(provider)) = db::providers::get(&pool, &id).await {
            if is_cache_stale(provider.last_refreshed, ttl, now) {
                let _ = run_refresh(app.clone(), id).await;
            }
        }
    }
}

/// Owned Xtream credentials for `provider`, or `None` when it's an M3U
/// provider (which has no on-demand detail endpoints).
async fn xtream_creds_for(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Option<(String, String, String)>, String> {
    let provider = db::providers::get(pool, provider_id)
        .await
        .map_err(|e| format!("Failed to load provider: {e}"))?
        .ok_or_else(|| format!("Provider {provider_id} does not exist."))?;
    if provider.provider_type != ProviderType::Xtream {
        return Ok(None);
    }
    let server = provider.server_url.ok_or("Provider has no server URL.")?;
    let username = provider.username.ok_or("Provider has no username.")?;
    let password = keychain::get_secret(&provider.id)?;
    Ok(Some((server, username, password)))
}

/// Episodes for a series, grouped by season. For Xtream providers whose
/// episodes are not cached yet, fetches `get_series_info` on demand and
/// persists the result (spec §16 / Milestone 5).
pub async fn get_episodes_impl(
    pool: &SqlitePool,
    provider_id: &str,
    series_id: &str,
) -> Result<BTreeMap<i64, Vec<EpisodeItem>>, String> {
    let mut episodes = db::catalog::episodes_for_series(pool, provider_id, series_id)
        .await
        .map_err(|e| format!("Failed to read episodes: {e}"))?;

    if episodes.is_empty() {
        if let Some((server, username, password)) = xtream_creds_for(pool, provider_id).await? {
            let creds = xtream::XtreamCreds {
                server_url: &server,
                username: &username,
                password: &password,
            };
            let info = xtream::fetch_series_info(&creds, series_id).await?;
            db::catalog::replace_series_episodes(pool, provider_id, series_id, &info.episodes)
                .await
                .map_err(|e| format!("Failed to save episodes: {e}"))?;
            episodes = info.episodes;
        }
    }

    let mut grouped: BTreeMap<i64, Vec<EpisodeItem>> = BTreeMap::new();
    for episode in episodes {
        grouped.entry(episode.season).or_default().push(episode);
    }
    Ok(grouped)
}

/// Movie detail: the cached row, enriched with `get_vod_info` metadata for
/// Xtream providers. A metadata fetch failure degrades to the bare row
/// (description is spec'd as "if available") and is not cached, so a later
/// open retries.
pub async fn get_movie_detail_impl(
    pool: &SqlitePool,
    cache: &DetailCache,
    provider_id: &str,
    movie_id: &str,
) -> Result<MovieDetail, String> {
    let movie = db::catalog::movie_by_id(pool, provider_id, movie_id)
        .await
        .map_err(|e| format!("Failed to read the movie: {e}"))?
        .ok_or_else(|| format!("No movie with id {movie_id} in the catalog."))?;

    let key = format!("{provider_id}/{movie_id}");
    let cached = cache.movies.lock().unwrap().get(&key).cloned();
    let meta = match cached {
        Some(meta) => meta,
        None => match xtream_creds_for(pool, provider_id).await? {
            Some((server, username, password)) => {
                let creds = xtream::XtreamCreds {
                    server_url: &server,
                    username: &username,
                    password: &password,
                };
                match xtream::fetch_vod_info(&creds, movie_id).await {
                    Ok(meta) => {
                        cache.movies.lock().unwrap().insert(key, meta.clone());
                        meta
                    }
                    Err(_) => xtream::VodInfo::default(),
                }
            }
            None => xtream::VodInfo::default(),
        },
    };

    Ok(MovieDetail {
        movie,
        description: meta.description,
        genre: meta.genre,
        duration_seconds: meta.duration_seconds,
        backdrop_url: meta.backdrop_url,
    })
}

/// Series detail, enriched with `get_series_info` metadata for Xtream
/// providers. The episode list from the same response is persisted so the
/// subsequent `get_episodes` call is served from the database.
pub async fn get_series_detail_impl(
    pool: &SqlitePool,
    cache: &DetailCache,
    provider_id: &str,
    series_id: &str,
) -> Result<SeriesDetail, String> {
    let series = db::catalog::series_by_id(pool, provider_id, series_id)
        .await
        .map_err(|e| format!("Failed to read the series: {e}"))?
        .ok_or_else(|| format!("No series with id {series_id} in the catalog."))?;

    let key = format!("{provider_id}/{series_id}");
    let cached = cache.series.lock().unwrap().get(&key).cloned();
    let meta = match cached {
        Some(meta) => meta,
        None => match xtream_creds_for(pool, provider_id).await? {
            Some((server, username, password)) => {
                let creds = xtream::XtreamCreds {
                    server_url: &server,
                    username: &username,
                    password: &password,
                };
                match xtream::fetch_series_info(&creds, series_id).await {
                    Ok(info) => {
                        if !info.episodes.is_empty() {
                            db::catalog::replace_series_episodes(
                                pool,
                                provider_id,
                                series_id,
                                &info.episodes,
                            )
                            .await
                            .map_err(|e| format!("Failed to save episodes: {e}"))?;
                        }
                        // Episodes live in SQLite now; cache only the metadata.
                        let meta = xtream::SeriesInfo {
                            episodes: Vec::new(),
                            ..info
                        };
                        cache.series.lock().unwrap().insert(key, meta.clone());
                        meta
                    }
                    Err(_) => xtream::SeriesInfo::default(),
                }
            }
            None => xtream::SeriesInfo::default(),
        },
    };

    Ok(SeriesDetail {
        series,
        description: meta.description,
        genre: meta.genre,
        backdrop_url: meta.backdrop_url,
    })
}

#[tauri::command]
pub async fn get_active_provider(state: State<'_, Db>) -> Result<Option<Provider>, String> {
    get_active_provider_impl(&state.0).await
}

#[tauri::command]
pub async fn set_active_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let pool = app.state::<Db>().0.clone();
    let provider = set_active_provider_impl(&pool, &provider_id).await?;
    // Spec §5.2: switching providers triggers a fetch when the cache is stale
    // (always-stale for never-refreshed providers).
    let ttl = cache_ttl_hours(&pool).await;
    if is_cache_stale(provider.last_refreshed, ttl, now_unix()) {
        tauri::async_runtime::spawn(run_refresh(app.clone(), provider_id));
    }
    Ok(())
}

/// The enabled providers whose catalogs are merged (Milestone 39), resolved to
/// full `Provider` objects in enabled order.
#[tauri::command]
pub async fn get_enabled_providers(state: State<'_, Db>) -> Result<Vec<Provider>, String> {
    let ids = get_enabled_provider_ids(&state.0).await?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(p) = db::providers::get(&state.0, &id)
            .await
            .map_err(|e| format!("Failed to load provider: {e}"))?
        {
            out.push(p);
        }
    }
    Ok(out)
}

/// Replace the enabled-provider set (Milestone 39) and background-refresh any
/// newly-enabled provider whose cache is stale (never blocking the UI).
#[tauri::command]
pub async fn set_enabled_providers(app: AppHandle, provider_ids: Vec<String>) -> Result<(), String> {
    let pool = app.state::<Db>().0.clone();
    let before = get_enabled_provider_ids(&pool).await?;
    set_enabled_provider_ids(&pool, &provider_ids).await?;

    let before_set: HashSet<&str> = before.iter().map(|s| s.as_str()).collect();
    let ttl = cache_ttl_hours(&pool).await;
    let now = now_unix();
    for id in &provider_ids {
        if before_set.contains(id.as_str()) {
            continue;
        }
        if let Ok(Some(p)) = db::providers::get(&pool, id).await {
            if is_cache_stale(p.last_refreshed, ttl, now) {
                tauri::async_runtime::spawn(run_refresh(app.clone(), id.clone()));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_catalog(app: AppHandle, provider_id: String) -> Result<(), String> {
    run_refresh(app, provider_id).await
}

#[tauri::command]
pub async fn get_live_categories(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
) -> Result<Vec<Category>, String> {
    db::catalog::live_categories(&state.0, &provider_ids)
        .await
        .map_err(|e| format!("Failed to read live categories: {e}"))
}

#[tauri::command]
pub async fn get_live_channels(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
    category_id: Option<String>,
    query: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<LiveChannel>, String> {
    // `category_id` is the merged-category key (a category name, Milestone 39).
    db::catalog::live_channels_page(
        &state.0,
        &provider_ids,
        category_id.as_deref(),
        query.as_deref(),
        page,
        page_size,
    )
    .await
    .map_err(|e| format!("Failed to read live channels: {e}"))
}

#[tauri::command]
pub async fn get_vod_categories(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
) -> Result<Vec<Category>, String> {
    db::catalog::vod_categories(&state.0, &provider_ids)
        .await
        .map_err(|e| format!("Failed to read movie genres: {e}"))
}

#[tauri::command]
pub async fn get_movies(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
    category_id: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<MovieItem>, String> {
    db::catalog::movies_page(&state.0, &provider_ids, category_id.as_deref(), page, page_size)
        .await
        .map_err(|e| format!("Failed to read movies: {e}"))
}

#[tauri::command]
pub async fn get_series_categories(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
) -> Result<Vec<Category>, String> {
    db::catalog::series_categories(&state.0, &provider_ids)
        .await
        .map_err(|e| format!("Failed to read series genres: {e}"))
}

#[tauri::command]
pub async fn get_series(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
    category_id: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<SeriesItem>, String> {
    db::catalog::series_page(&state.0, &provider_ids, category_id.as_deref(), page, page_size)
        .await
        .map_err(|e| format!("Failed to read series: {e}"))
}

#[tauri::command]
pub async fn get_episodes(
    state: State<'_, Db>,
    provider_id: String,
    series_id: String,
) -> Result<BTreeMap<i64, Vec<EpisodeItem>>, String> {
    get_episodes_impl(&state.0, &provider_id, &series_id).await
}

#[tauri::command]
pub async fn get_movie_detail(
    state: State<'_, Db>,
    cache: State<'_, DetailCache>,
    provider_id: String,
    movie_id: String,
) -> Result<MovieDetail, String> {
    get_movie_detail_impl(&state.0, &cache, &provider_id, &movie_id).await
}

#[tauri::command]
pub async fn get_series_detail(
    state: State<'_, Db>,
    cache: State<'_, DetailCache>,
    provider_id: String,
    series_id: String,
) -> Result<SeriesDetail, String> {
    get_series_detail_impl(&state.0, &cache, &provider_id, &series_id).await
}

/// "More like this" related titles (spec §5.4, Milestone 28): up to `limit`
/// other catalog items sharing the title's category, same content type,
/// provider-scoped, excluding the title itself. Local read — no provider request.
pub async fn get_related_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    limit: Option<i64>,
) -> Result<RelatedResults, String> {
    let limit = limit.unwrap_or(20);
    let mut out = RelatedResults::default();
    match content_type {
        "movie" => {
            out.movies = db::catalog::related_movies(pool, provider_id, content_id, limit)
                .await
                .map_err(|e| format!("Failed to read related movies: {e}"))?;
        }
        "series" => {
            out.series = db::catalog::related_series(pool, provider_id, content_id, limit)
                .await
                .map_err(|e| format!("Failed to read related series: {e}"))?;
        }
        other => return Err(format!("Unsupported content type for related: {other}")),
    }
    Ok(out)
}

#[tauri::command]
pub async fn get_related(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
    limit: Option<i64>,
) -> Result<RelatedResults, String> {
    get_related_impl(&state.0, &provider_id, &content_type, &content_id, limit).await
}

#[tauri::command]
pub async fn get_catalog_summary(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
) -> Result<CatalogSummary, String> {
    db::catalog::summary(&state.0, &provider_ids)
        .await
        .map_err(|e| format!("Failed to read the catalog: {e}"))
}

/// Record a live channel as just-watched (spec §13, Milestone 29). Local-only.
#[tauri::command]
pub async fn record_recent_channel(
    state: State<'_, Db>,
    provider_id: String,
    channel_id: String,
) -> Result<(), String> {
    db::catalog::record_recent_channel(&state.0, &provider_id, &channel_id, now_unix())
        .await
        .map_err(|e| format!("Failed to record the recent channel: {e}"))
}

/// The provider's recently-watched channels, most-recent first (spec §13, M29).
#[tauri::command]
pub async fn get_recent_channels(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
    limit: Option<i64>,
) -> Result<Vec<LiveChannel>, String> {
    db::catalog::recent_channels(&state.0, &provider_ids, limit.unwrap_or(15))
        .await
        .map_err(|e| format!("Failed to read recent channels: {e}"))
}

/// The user's custom category order for a provider+section (spec §13, M29).
#[tauri::command]
pub async fn get_category_order(
    state: State<'_, Db>,
    provider_id: String,
    section: String,
) -> Result<Vec<String>, String> {
    db::catalog::category_order(&state.0, &provider_id, &section)
        .await
        .map_err(|e| format!("Failed to read the category order: {e}"))
}

/// Persist the user's custom category order for a provider+section (spec §13, M29).
#[tauri::command]
pub async fn set_category_order(
    state: State<'_, Db>,
    provider_id: String,
    section: String,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    db::catalog::set_category_order(&state.0, &provider_id, &section, &ordered_ids)
        .await
        .map_err(|e| format!("Failed to save the category order: {e}"))
}
