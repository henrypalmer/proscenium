//! On-disk image cache pipeline (spec §5.7, Milestone 27).
//!
//! Catalog art (posters, backdrops, channel logos) is downloaded on first view
//! into `<app-data>/proscenium/images/` and served from disk thereafter via the
//! Tauri asset protocol, so the second view of any art makes no network request
//! and previously-cached art is available offline. Growth is bounded by an LRU
//! size cap (`image_cache_max_mb`, default 500 MB) layered on top of the 30-day
//! TTL eviction (`commands::settings`). Caching is lazy (driven by the UI as art
//! scrolls into view), never a bulk pre-fetch.

use crate::db::{self, Db};
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

/// Default cache ceiling when `image_cache_max_mb` is unset (spec §5.7).
pub const DEFAULT_MAX_MB: i64 = 500;

/// Managed state for the image cache: where blobs live, the HTTP client used to
/// fetch them, and the set of URLs currently downloading (so concurrent requests
/// for the same art don't download it twice).
pub struct ImageCache {
    pub dir: PathBuf,
    pub client: reqwest::Client,
    in_flight: Mutex<HashSet<String>>,
}

impl ImageCache {
    pub fn new(dir: PathBuf, client: reqwest::Client) -> Self {
        Self {
            dir,
            client,
            in_flight: Mutex::new(HashSet::new()),
        }
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Deterministic on-disk filename for a source URL: a hash of the URL plus its
/// original extension (the table keys on the URL, so the filename is only
/// storage). Keeps names short, collision-free in practice, and path-safe.
fn filename_for(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let stem = format!("{:016x}", hasher.finish());
    let ext = url
        .rsplit('/')
        .next()
        .and_then(|seg| seg.rsplit_once('.'))
        .map(|(_, ext)| ext)
        // Strip a trailing query string and keep only sane extension chars.
        .map(|ext| {
            ext.split(['?', '#'])
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(5)
                .collect::<String>()
        })
        .filter(|ext| !ext.is_empty())
        .unwrap_or_else(|| "img".into());
    format!("{stem}.{ext}")
}

/// The configured cache ceiling in bytes (default 500 MB).
pub async fn cache_cap_bytes(pool: &sqlx::SqlitePool) -> i64 {
    let mb = db::settings::get(pool, "image_cache_max_mb")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|&n| n >= 0)
        .unwrap_or(DEFAULT_MAX_MB);
    mb * 1024 * 1024
}

/// Cache hit only: return the local path for an already-cached URL whose file is
/// still on disk (bumping its LRU timestamp). A row pointing at a missing file is
/// dropped so the next `cache_image` re-downloads it. Never touches the network.
pub async fn resolve_cached_image_impl(
    pool: &sqlx::SqlitePool,
    url: &str,
    now: i64,
) -> Option<String> {
    let path = db::image_cache::lookup(pool, url, now).await.ok().flatten()?;
    if Path::new(&path).exists() {
        return Some(path);
    }
    let _ = db::image_cache::delete_urls(pool, &[url.to_string()]).await;
    None
}

/// Ensure `url` is cached and return its local path. On a hit, returns
/// immediately; on a miss, downloads to `dir`, records the row, and enforces the
/// size cap. Concurrent calls for the same URL collapse to one download. Returns
/// `None` (rather than erroring) if the download fails or is already in flight —
/// the caller falls back to the remote URL.
pub async fn cache_image_impl(
    state: &ImageCache,
    pool: &sqlx::SqlitePool,
    url: &str,
    now: i64,
    max_bytes: i64,
) -> Option<String> {
    if let Some(path) = resolve_cached_image_impl(pool, url, now).await {
        return Some(path);
    }
    // Claim the download; bail if another task already owns it.
    {
        let mut in_flight = state.in_flight.lock().unwrap();
        if !in_flight.insert(url.to_string()) {
            return None;
        }
    }
    let result = download_and_store(state, pool, url, now).await;
    {
        state.in_flight.lock().unwrap().remove(url);
    }
    // Keep the cache under its ceiling after a successful add (best-effort).
    if matches!(result, Ok(Some(_))) {
        let _ = enforce_size_cap(pool, max_bytes).await;
    }
    result.ok().flatten()
}

async fn download_and_store(
    state: &ImageCache,
    pool: &sqlx::SqlitePool,
    url: &str,
    now: i64,
) -> Result<Option<String>, String> {
    let resp = state
        .client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("image download failed: {e}"))?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("image read failed: {e}"))?;
    std::fs::create_dir_all(&state.dir).map_err(|e| format!("cache dir: {e}"))?;
    let path = state.dir.join(filename_for(url));
    std::fs::write(&path, &bytes).map_err(|e| format!("image write failed: {e}"))?;
    let local = path.to_string_lossy().to_string();
    db::image_cache::put(pool, url, &local, bytes.len() as i64, now)
        .await
        .map_err(|e| format!("image index write failed: {e}"))?;
    Ok(Some(local))
}

/// Evict least-recently-accessed entries (rows + files) until the cache is under
/// `max_bytes`. Returns the number of entries removed.
pub async fn enforce_size_cap(pool: &sqlx::SqlitePool, max_bytes: i64) -> Result<usize, String> {
    let total = db::image_cache::total_size(pool)
        .await
        .map_err(|e| format!("cache size: {e}"))?;
    if total <= max_bytes {
        return Ok(0);
    }
    let mut over = total - max_bytes;
    let rows = db::image_cache::lru_rows(pool)
        .await
        .map_err(|e| format!("cache rows: {e}"))?;
    let mut victims = Vec::new();
    for row in rows {
        if over <= 0 {
            break;
        }
        let _ = std::fs::remove_file(&row.local_path);
        over -= row.size_bytes;
        victims.push(row.url);
    }
    db::image_cache::delete_urls(pool, &victims)
        .await
        .map_err(|e| format!("cache evict: {e}"))?;
    Ok(victims.len())
}

/// Delete every cached file and empty the index (the "Clear image cache" action).
pub async fn clear_image_cache_impl(pool: &sqlx::SqlitePool, dir: &Path) -> Result<(), String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    db::image_cache::clear(pool)
        .await
        .map_err(|e| format!("clear cache: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn resolve_cached_image(
    db: State<'_, Db>,
    url: String,
) -> Result<Option<String>, String> {
    Ok(resolve_cached_image_impl(&db.0, &url, now_unix()).await)
}

#[tauri::command]
pub async fn cache_image(
    db: State<'_, Db>,
    cache: State<'_, ImageCache>,
    url: String,
) -> Result<Option<String>, String> {
    let max = cache_cap_bytes(&db.0).await;
    Ok(cache_image_impl(&cache, &db.0, &url, now_unix(), max).await)
}

#[tauri::command]
pub async fn image_cache_size(db: State<'_, Db>) -> Result<i64, String> {
    db::image_cache::total_size(&db.0)
        .await
        .map_err(|e| format!("cache size: {e}"))
}

#[tauri::command]
pub async fn clear_image_cache(
    db: State<'_, Db>,
    cache: State<'_, ImageCache>,
) -> Result<(), String> {
    clear_image_cache_impl(&db.0, &cache.dir).await
}
