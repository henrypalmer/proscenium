//! Tier-2 canonical (Cinemeta) response cache (Milestone 40). A throwaway,
//! TTL'd JSON cache: a fresh row serves the request, a stale row is the offline
//! fallback when Cinemeta is unreachable. Not provider-scoped and untouched by
//! catalog refresh. The `content_match` side table (slice 2) joins this module.

use sqlx::{Row, SqlitePool};

/// A cached payload plus its expiry, so the caller can tell fresh from stale.
pub struct Cached {
    pub body: String,
    pub expires_at: i64,
}

pub async fn cache_get(pool: &SqlitePool, key: &str) -> Result<Option<Cached>, sqlx::Error> {
    let row = sqlx::query("SELECT body, expires_at FROM canonical_cache WHERE cache_key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| Cached {
        body: r.get("body"),
        expires_at: r.get("expires_at"),
    }))
}

pub async fn cache_put(
    pool: &SqlitePool,
    key: &str,
    body: &str,
    cached_at: i64,
    expires_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO canonical_cache (cache_key, body, cached_at, expires_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(cache_key) DO UPDATE SET
           body = excluded.body, cached_at = excluded.cached_at, expires_at = excluded.expires_at",
    )
    .bind(key)
    .bind(body)
    .bind(cached_at)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}
