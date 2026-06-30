//! Canonical catalog commands (Milestone 40): Cinemeta-backed Home/Movies/Series
//! browse, served through a Tier-2 disposable cache that falls back to a stale
//! row when Cinemeta is unreachable (so cached browse keeps working offline,
//! spec §19 M40 AC1).

use crate::canonical::cinemeta;
use crate::db::{self, Db};
use crate::models::{CanonicalItem, CanonicalMeta};
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
#[tauri::command]
pub async fn get_canonical_meta(
    state: State<'_, Db>,
    kind: String,
    imdb_id: String,
) -> Result<CanonicalMeta, String> {
    let key = format!("meta:{kind}:{imdb_id}");
    cached_or_fetch(&state.0, &key, META_TTL_SECS, || async move {
        cinemeta::fetch_meta(&kind, &imdb_id).await
    })
    .await
}
