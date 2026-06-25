//! Cover-art cache index (spec §15 `image_cache`). The blobs live on disk;
//! this table maps a source URL to its local path with a 30-day TTL (eviction)
//! and an `last_accessed` / `size_bytes` pair backing the Milestone 27 LRU
//! size cap. The download + serve pipeline lives in `commands/images.rs`.

use sqlx::{Row, SqlitePool};

/// 30 days in seconds — the spec §5.7 / §15 image cache lifetime.
pub const TTL_SECONDS: i64 = 30 * 24 * 3600;

/// One LRU candidate: its source URL, on-disk path, and byte size.
pub struct CacheRow {
    pub url: String,
    pub local_path: String,
    pub size_bytes: i64,
}

/// Insert (or refresh) a cache entry. `expires_at` is `cached_at + 30 days`;
/// `last_accessed` starts at `cached_at` so a freshly-cached image is the most
/// recently used.
pub async fn put(
    pool: &SqlitePool,
    url: &str,
    local_path: &str,
    size_bytes: i64,
    cached_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO image_cache (url, local_path, cached_at, expires_at, size_bytes, last_accessed)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(url) DO UPDATE SET
           local_path    = excluded.local_path,
           cached_at     = excluded.cached_at,
           expires_at    = excluded.expires_at,
           size_bytes    = excluded.size_bytes,
           last_accessed = excluded.last_accessed",
    )
    .bind(url)
    .bind(local_path)
    .bind(cached_at)
    .bind(cached_at + TTL_SECONDS)
    .bind(size_bytes)
    .bind(cached_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Look up a cached entry's local path, bumping its `last_accessed` so it
/// survives LRU eviction longer. Returns `None` when the URL is not cached.
pub async fn lookup(
    pool: &SqlitePool,
    url: &str,
    now: i64,
) -> Result<Option<String>, sqlx::Error> {
    let path: Option<String> =
        sqlx::query_scalar("SELECT local_path FROM image_cache WHERE url = ?")
            .bind(url)
            .fetch_optional(pool)
            .await?;
    if path.is_some() {
        sqlx::query("UPDATE image_cache SET last_accessed = ? WHERE url = ?")
            .bind(now)
            .bind(url)
            .execute(pool)
            .await?;
    }
    Ok(path)
}

/// Total bytes currently tracked by the cache.
pub async fn total_size(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let total: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(size_bytes), 0) FROM image_cache")
        .fetch_one(pool)
        .await?;
    Ok(total)
}

/// All rows ordered least-recently-accessed first — the LRU eviction order.
pub async fn lru_rows(pool: &SqlitePool) -> Result<Vec<CacheRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT url, local_path, size_bytes FROM image_cache ORDER BY last_accessed ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| CacheRow {
            url: r.get("url"),
            local_path: r.get("local_path"),
            size_bytes: r.get("size_bytes"),
        })
        .collect())
}

/// Delete the given URLs' rows (their files are removed by the caller).
pub async fn delete_urls(pool: &SqlitePool, urls: &[String]) -> Result<(), sqlx::Error> {
    for url in urls {
        sqlx::query("DELETE FROM image_cache WHERE url = ?")
            .bind(url)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Empty the whole index (the "Clear image cache" action). Returns the row count.
pub async fn clear(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM image_cache").execute(pool).await?;
    Ok(result.rows_affected())
}

/// Local paths of every entry whose TTL has elapsed (`expires_at <= now`).
pub async fn expired(pool: &SqlitePool, now: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT local_path FROM image_cache WHERE expires_at <= ?")
        .bind(now)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| r.get("local_path")).collect())
}

/// Remove every expired row. Returns the number of rows deleted.
pub async fn delete_expired(pool: &SqlitePool, now: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM image_cache WHERE expires_at <= ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
