//! Canonical catalog commands (Milestone 40): Cinemeta-backed Home/Movies/Series
//! browse, served through a Tier-2 disposable cache that falls back to a stale
//! row when Cinemeta is unreachable (so cached browse keeps working offline,
//! spec §19 M40 AC1).

use crate::canonical::resolver::{self, AddonSource, CanonicalRef};
use crate::canonical::{cinemeta, stremio};
use crate::commands::catalog::get_enabled_provider_ids;
use crate::db::{self, Db};
use crate::keychain;
use crate::models::{AvailabilityInfo, CanonicalItem, CanonicalMeta, StreamCandidate};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
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
    let (providers, addons) = resolvers_for(&state.0).await;
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

/// The enabled IPTV providers + installed Stremio addons (base URLs read from the
/// keychain, never logged) that the resolver registry queries. Shared by
/// `resolve_sources` and the availability index.
async fn resolvers_for(pool: &SqlitePool) -> (Vec<crate::models::Provider>, Vec<AddonSource>) {
    let mut providers = Vec::new();
    if let Ok(ids) = get_enabled_provider_ids(pool).await {
        for id in ids {
            if let Ok(Some(p)) = db::providers::get(pool, &id).await {
                providers.push(p);
            }
        }
    }
    let mut addons = Vec::new();
    if let Ok(list) = db::stremio::list(pool).await {
        for a in list {
            if let Ok(url) = keychain::get_addon_secret(&a.id) {
                addons.push(AddonSource {
                    name: a.name,
                    base_url: stremio::base_url(&url),
                });
            }
        }
    }
    (providers, addons)
}

/// Cached availability for the given canonical titles (Milestone 42). Read-only —
/// returns whatever the background index has resolved; computes nothing.
#[tauri::command]
pub async fn get_availability(
    state: State<'_, Db>,
    kind: String,
    imdb_ids: Vec<String>,
) -> Result<HashMap<String, AvailabilityInfo>, String> {
    let map = db::canonical::availability_get_many(&state.0, &imdb_ids, &kind)
        .await
        .map_err(|e| format!("Failed to read availability: {e}"))?;
    Ok(map
        .into_iter()
        .map(|(id, a)| {
            (
                id,
                AvailabilityInfo {
                    source_count: a.source_count,
                    best_quality: a.best_quality,
                },
            )
        })
        .collect())
}

const AVAILABILITY_TTL_SECS: i64 = 6 * 3600;
/// Titles resolved per `index_availability` call — the rate limit on the pass.
const AVAILABILITY_BATCH: usize = 8;

/// Background availability pass (Milestone 42, opt-in): resolve sources for the
/// given titles whose cache is missing/stale (capped per call), cache the count
/// + best quality, and return the now-known availability. Series probe S1:E1 as a
/// representative episode. Non-blocking — the frontend calls this for the cards
/// in view when the badge setting is on.
#[tauri::command]
pub async fn index_availability(
    state: State<'_, Db>,
    kind: String,
    imdb_ids: Vec<String>,
) -> Result<HashMap<String, AvailabilityInfo>, String> {
    let now = now_unix();
    let cached = db::canonical::availability_get_many(&state.0, &imdb_ids, &kind)
        .await
        .unwrap_or_default();
    let (providers, addons) = resolvers_for(&state.0).await;

    let mut out: HashMap<String, AvailabilityInfo> = HashMap::new();
    let mut resolved = 0usize;
    for imdb_id in &imdb_ids {
        // Serve a fresh cache entry without spending the rate-limit budget.
        if let Some(a) = cached.get(imdb_id) {
            if now - a.checked_at < AVAILABILITY_TTL_SECS {
                out.insert(
                    imdb_id.clone(),
                    AvailabilityInfo {
                        source_count: a.source_count,
                        best_quality: a.best_quality.clone(),
                    },
                );
                continue;
            }
        }
        if resolved >= AVAILABILITY_BATCH {
            break;
        }
        resolved += 1;

        let Ok(meta) = fetch_canonical_meta(&state.0, &kind, imdb_id).await else {
            continue;
        };
        let (season, episode) = if kind == "series" {
            (Some(1), Some(1))
        } else {
            (None, None)
        };
        let target = CanonicalRef {
            imdb_id: meta.imdb_id.clone(),
            kind: meta.kind,
            tmdb_id: meta.tmdb_id,
            name: meta.name,
            year: meta.release_year,
            season,
            episode,
        };
        let candidates = resolver::resolve_sources(&state.0, &target, &providers, &addons).await;
        let playable: Vec<&StreamCandidate> =
            candidates.iter().filter(|c| !c.needs_debrid).collect();
        let source_count = playable.len() as i64;
        let best_quality = playable
            .iter()
            .max_by_key(|c| resolver::resolution_rank(&c.quality))
            .and_then(|c| c.quality.clone())
            .filter(|q| resolver::resolution_rank(&Some(q.clone())) > 0);
        let _ = db::canonical::availability_put(
            &state.0,
            imdb_id,
            &kind,
            source_count,
            best_quality.as_deref(),
            now,
        )
        .await;
        out.insert(
            imdb_id.clone(),
            AvailabilityInfo {
                source_count,
                best_quality,
            },
        );

        // Gentle on providers/addons — the "rate-limited" part of the pass.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Ok(out)
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
