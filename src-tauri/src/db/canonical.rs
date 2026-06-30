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

// --- content_match: the canonical↔provider match index (Milestone 40 slice 2) ---

/// A canonical↔provider match row.
#[derive(Debug, Clone)]
pub struct ContentMatch {
    pub provider_id: String,
    pub content_type: String,
    pub content_id: String,
    pub imdb_id: String,
    pub tmdb_id: Option<i64>,
    pub confidence: f64,
    pub method: String,
    pub matched_at: i64,
}

fn row_to_match(r: &sqlx::sqlite::SqliteRow) -> ContentMatch {
    ContentMatch {
        provider_id: r.get("provider_id"),
        content_type: r.get("content_type"),
        content_id: r.get("content_id"),
        imdb_id: r.get("imdb_id"),
        tmdb_id: r.get("tmdb_id"),
        confidence: r.get("confidence"),
        method: r.get("method"),
        matched_at: r.get("matched_at"),
    }
}

pub async fn match_get(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<Option<ContentMatch>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT * FROM content_match
         WHERE provider_id = ? AND content_type = ? AND content_id = ?",
    )
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(row_to_match))
}

/// Upsert a match (latest wins — re-matching or a manual override replaces it).
pub async fn match_put(pool: &SqlitePool, m: &ContentMatch) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO content_match
           (provider_id, content_type, content_id, imdb_id, tmdb_id, confidence, method, matched_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(provider_id, content_type, content_id) DO UPDATE SET
           imdb_id = excluded.imdb_id, tmdb_id = excluded.tmdb_id,
           confidence = excluded.confidence, method = excluded.method,
           matched_at = excluded.matched_at",
    )
    .bind(&m.provider_id)
    .bind(&m.content_type)
    .bind(&m.content_id)
    .bind(&m.imdb_id)
    .bind(m.tmdb_id)
    .bind(m.confidence)
    .bind(&m.method)
    .bind(m.matched_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// All provider items matched to a canonical IMDB id within the given provider
/// set — the reverse lookup that re-resolves sources without re-searching.
pub async fn matches_for_imdb(
    pool: &SqlitePool,
    imdb_id: &str,
    content_type: &str,
    provider_ids: &[String],
) -> Result<Vec<ContentMatch>, sqlx::Error> {
    if provider_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ph = vec!["?"; provider_ids.len()].join(", ");
    let sql = format!(
        "SELECT * FROM content_match
         WHERE imdb_id = ? AND content_type = ? AND provider_id IN ({ph})
         ORDER BY confidence DESC, provider_id"
    );
    let mut q = sqlx::query(&sql).bind(imdb_id).bind(content_type);
    for id in provider_ids {
        q = q.bind(id.as_str());
    }
    Ok(q.fetch_all(pool).await?.iter().map(row_to_match).collect())
}

/// Persist a manual override: first clear any existing match for this canonical
/// id on the provider (the wrong auto-match), then upsert the user's pick. So a
/// provider has exactly one match per canonical id afterward.
pub async fn set_manual_match(pool: &SqlitePool, m: &ContentMatch) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM content_match WHERE provider_id = ? AND content_type = ? AND imdb_id = ?")
        .bind(&m.provider_id)
        .bind(&m.content_type)
        .bind(&m.imdb_id)
        .execute(pool)
        .await?;
    match_put(pool, m).await
}
