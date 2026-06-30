//! Canonical catalog commands (Milestone 40): Cinemeta-backed Home/Movies/Series
//! browse, served through a Tier-2 disposable cache that falls back to a stale
//! row when Cinemeta is unreachable (so cached browse keeps working offline,
//! spec §19 M40 AC1).

use crate::canonical::resolver::{self, AddonSource, CanonicalRef};
use crate::canonical::{cinemeta, stremio};
use crate::commands::catalog::get_enabled_provider_ids;
use crate::db::{self, Db};
use crate::keychain;
use crate::models::{CanonicalItem, CanonicalMeta, StreamCandidate};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::SqlitePool;
use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

/// Catalog rows change often (new releases) → short TTL.
const CATALOG_TTL_SECS: i64 = 6 * 3600;
/// Per-title meta is near-static → long TTL.
const META_TTL_SECS: i64 = 7 * 24 * 3600;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Serve `key` from the Tier-2 cache when fresh; otherwise run `fetch`, cache
/// its result, and return it. If the fetch fails but a (stale) row exists,
/// return the stale payload — degraded/offline browse (spec §19 M40 AC1).
/// Generic over any serde model so catalog and meta share one path.
pub async fn cached_or_fetch<T, F, Fut>(
    pool: &SqlitePool,
    key: &str,
    ttl_secs: i64,
    fetch: F,
) -> Result<T, String>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let now = now_unix();
    // A present row is either a fresh hit (return it) or a stale fallback we
    // hold onto in case the fetch below fails.
    let stale = match db::canonical::cache_get(pool, key).await {
        Ok(Some(c)) => {
            if c.expires_at > now {
                if let Ok(value) = serde_json::from_str::<T>(&c.body) {
                    return Ok(value);
                }
            }
            Some(c.body)
        }
        _ => None,
    };

    match fetch().await {
        Ok(value) => {
            if let Ok(body) = serde_json::to_string(&value) {
                let _ = db::canonical::cache_put(pool, key, &body, now, now + ttl_secs).await;
            }
            Ok(value)
        }
        Err(e) => match stale.and_then(|b| serde_json::from_str::<T>(&b).ok()) {
            Some(value) => Ok(value),
            None => Err(e),
        },
    }
}

/// Cinemeta's accepted genre options for a content kind (static; offline-safe).
#[tauri::command]
pub async fn get_canonical_genres(kind: String) -> Result<Vec<String>, String> {
    Ok(cinemeta::genres(&kind))
}

/// A page of the canonical catalog (`kind` = "movie" | "series"), optionally
/// narrowed by `genre` or `search`, paged by `skip` (Cinemeta returns ~50–100
/// per page). Cached per (kind, genre, search, skip).
#[tauri::command]
pub async fn get_canonical_catalog(
    state: State<'_, Db>,
    kind: String,
    genre: Option<String>,
    search: Option<String>,
    skip: Option<i64>,
) -> Result<Vec<CanonicalItem>, String> {
    let skip = skip.unwrap_or(0);
    let key = format!(
        "cat:{kind}:{}:{}:{skip}",
        genre.as_deref().unwrap_or(""),
        search.as_deref().unwrap_or("")
    );
    cached_or_fetch(&state.0, &key, CATALOG_TTL_SECS, || async move {
        cinemeta::fetch_catalog(&kind, genre.as_deref(), search.as_deref(), skip).await
    })
    .await
}

/// Full canonical metadata for one title (poster/backdrop/overview/cast and, for
/// series, the episode list). Long-TTL cached.
/// Cached canonical meta fetch — the shared path behind `get_canonical_meta` and
/// source resolution.
pub async fn fetch_canonical_meta(
    pool: &SqlitePool,
    kind: &str,
    imdb_id: &str,
) -> Result<CanonicalMeta, String> {
    let key = format!("meta:{kind}:{imdb_id}");
    let (k, id) = (kind.to_string(), imdb_id.to_string());
    cached_or_fetch(pool, &key, META_TTL_SECS, || async move {
        cinemeta::fetch_meta(&k, &id).await
    })
    .await
}

#[tauri::command]
pub async fn get_canonical_meta(
    state: State<'_, Db>,
    kind: String,
    imdb_id: String,
) -> Result<CanonicalMeta, String> {
    fetch_canonical_meta(&state.0, &kind, &imdb_id).await
}

/// Resolve playback sources for a canonical title across the enabled providers
/// (Milestone 40 slice 3). Returns ranked `StreamCandidate`s for the source
/// picker; an **empty vec is the first-class "no sources found"** state.
#[tauri::command]
pub async fn resolve_sources(
    state: State<'_, Db>,
    kind: String,
    imdb_id: String,
    season: Option<i64>,
    episode: Option<i64>,
) -> Result<Vec<StreamCandidate>, String> {
    let meta = fetch_canonical_meta(&state.0, &kind, &imdb_id).await?;
    let target = CanonicalRef {
        imdb_id: meta.imdb_id,
        kind: meta.kind,
        tmdb_id: meta.tmdb_id,
        name: meta.name,
        year: meta.release_year,
        season,
        episode,
    };
    let ids = get_enabled_provider_ids(&state.0).await?;
    let mut providers = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(p) = db::providers::get(&state.0, &id)
            .await
            .map_err(|e| format!("Failed to load provider: {e}"))?
        {
            providers.push(p);
        }
    }

    // Stremio addons (M41): each installed addon's base URL comes from the
    // keychain (the token-bearing URL is never logged).
    let mut addons = Vec::new();
    for a in db::stremio::list(&state.0).await.unwrap_or_default() {
        if let Ok(url) = keychain::get_addon_secret(&a.id) {
            addons.push(AddonSource {
                name: a.name,
                base_url: stremio::base_url(&url),
            });
        }
    }

    let mut candidates = resolver::resolve_sources(&state.0, &target, &providers, &addons).await;
    // Float the source the user last chose for this title to the top (M42).
    let preferred = db::canonical::source_pref_get(&state.0, &target.imdb_id, &target.kind)
        .await
        .ok()
        .flatten();
    resolver::rank_candidates(&mut candidates, preferred.as_deref());
    Ok(candidates)
}

/// Remember the source the user chose for a canonical title (Milestone 42), so it
/// floats to the top of the picker next time. `kind` is "movie" | "series".
#[tauri::command]
pub async fn record_source_pick(
    state: State<'_, Db>,
    kind: String,
    imdb_id: String,
    source: String,
) -> Result<(), String> {
    db::canonical::source_pref_set(&state.0, &imdb_id, &kind, &source, now_unix())
        .await
        .map_err(|e| format!("Failed to save the source preference: {e}"))
}

/// Persist a manual canonical↔provider match (Milestone 40 slice 4 override):
/// the user picks the correct provider title when the auto-match is wrong. The
/// correction clears any prior (auto) match for this canonical id on that
/// provider and survives catalog refresh like any other match.
#[tauri::command]
pub async fn set_manual_match(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
    imdb_id: String,
) -> Result<(), String> {
    db::canonical::set_manual_match(
        &state.0,
        &db::canonical::ContentMatch {
            provider_id,
            content_type,
            content_id,
            imdb_id,
            tmdb_id: None,
            confidence: 1.0,
            method: "manual".into(),
            matched_at: now_unix(),
        },
    )
    .await
    .map_err(|e| format!("Failed to save the match: {e}"))
}
