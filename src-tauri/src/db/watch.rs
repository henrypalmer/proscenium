//! Watch-progress storage (spec §5.9). Resume position + completion for VOD
//! only — live TV is never tracked. Rows cascade-delete with their provider.

use crate::models::WatchProgress;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

fn row_to_progress(row: &sqlx::sqlite::SqliteRow) -> WatchProgress {
    WatchProgress {
        position_seconds: row.get("position_seconds"),
        duration_seconds: row.get("duration_seconds"),
        completed: row.get::<i64, _>("completed") != 0,
        updated_at: row.get("updated_at"),
    }
}

pub async fn get(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<Option<WatchProgress>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT position_seconds, duration_seconds, completed, updated_at
         FROM watch_progress
         WHERE provider_id = ? AND content_type = ? AND content_id = ?",
    )
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(row_to_progress))
}

/// Every in-progress / completed item for one section, keyed by content id.
pub async fn list(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
) -> Result<HashMap<String, WatchProgress>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT content_id, position_seconds, duration_seconds, completed, updated_at
         FROM watch_progress
         WHERE provider_id = ? AND content_type = ?",
    )
    .bind(provider_id)
    .bind(content_type)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get::<String, _>("content_id"), row_to_progress(r)))
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    position_seconds: i64,
    duration_seconds: Option<i64>,
    completed: bool,
    updated_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO watch_progress
           (provider_id, content_type, content_id, position_seconds,
            duration_seconds, completed, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(provider_id, content_type, content_id) DO UPDATE SET
           position_seconds = excluded.position_seconds,
           duration_seconds = excluded.duration_seconds,
           completed        = excluded.completed,
           updated_at       = excluded.updated_at",
    )
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .bind(position_seconds)
    .bind(duration_seconds)
    .bind(completed as i64)
    .bind(updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM watch_progress
         WHERE provider_id = ? AND content_type = ? AND content_id = ?",
    )
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .execute(pool)
    .await?;
    Ok(())
}
