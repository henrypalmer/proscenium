//! Catalog Tauri commands (spec §16): active provider selection and the
//! full catalog refresh with `catalog:refresh_progress` / `_complete` events.

use crate::db::{self, Db};
use crate::iptv::{m3u, xtream};
use crate::keychain;
use crate::models::{
    CatalogSummary, Provider, ProviderType, RefreshComplete, RefreshProgress,
};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

pub const ACTIVE_PROVIDER_KEY: &str = "active_provider_id";
pub const CACHE_TTL_KEY: &str = "cache_ttl_hours";
pub const DEFAULT_CACHE_TTL_HOURS: i64 = 6;

/// Provider ids with a refresh currently in flight (prevents double refresh
/// from the manual button racing the startup stale check).
#[derive(Default)]
pub struct RefreshGuard(pub Mutex<HashSet<String>>);

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
    let Ok(Some(provider)) = get_active_provider_impl(&pool).await else {
        return;
    };
    let ttl = cache_ttl_hours(&pool).await;
    if is_cache_stale(provider.last_refreshed, ttl, now_unix()) {
        let _ = run_refresh(app, provider.id).await;
    }
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

#[tauri::command]
pub async fn refresh_catalog(app: AppHandle, provider_id: String) -> Result<(), String> {
    run_refresh(app, provider_id).await
}

#[tauri::command]
pub async fn get_catalog_summary(
    state: State<'_, Db>,
    provider_id: String,
) -> Result<CatalogSummary, String> {
    db::catalog::summary(&state.0, &provider_id)
        .await
        .map_err(|e| format!("Failed to read the catalog: {e}"))
}
