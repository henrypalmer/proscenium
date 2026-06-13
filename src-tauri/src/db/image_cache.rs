//! Cover-art cache index (spec §15 `image_cache`). The blobs live on disk;
//! this table maps a source URL to its local path with a 30-day TTL.

use sqlx::{Row, SqlitePool};

/// 30 days in seconds — the spec §5.7 / §15 image cache lifetime.
pub const TTL_SECONDS: i64 = 30 * 24 * 3600;

/// Insert (or refresh) a cache entry. `expires_at` is `cached_at + 30 days`.
pub async fn put(
    pool: &SqlitePool,
    url: &str,
    local_path: &str,
    cached_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO image_cache (url, local_path, cached_at, expires_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(url) DO UPDATE SET
           local_path = excluded.local_path,
           cached_at  = excluded.cached_at,
           expires_at = excluded.expires_at",
    )
    .bind(url)
    .bind(local_path)
    .bind(cached_at)
    .bind(cached_at + TTL_SECONDS)
    .execute(pool)
    .await?;
    Ok(())
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
