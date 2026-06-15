//! Watch-progress storage (spec §5.9). Resume position + completion for VOD
//! only — live TV is never tracked. Rows cascade-delete with their provider.

use crate::db::catalog::{row_to_episode, row_to_movie};
use crate::models::{ContinueWatchingItem, SeriesItem, WatchProgress};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

fn row_to_progress(row: &SqliteRow) -> WatchProgress {
    WatchProgress {
        position_seconds: row.get("position_seconds"),
        duration_seconds: row.get("duration_seconds"),
        completed: row.get::<i64, _>("completed") != 0,
        updated_at: row.get("updated_at"),
    }
}

/// Progress columns selected under `wp_*` aliases (so they don't collide with
/// the joined catalog row's own columns).
fn row_to_progress_aliased(row: &SqliteRow) -> WatchProgress {
    WatchProgress {
        position_seconds: row.get("wp_position"),
        duration_seconds: row.get("wp_duration"),
        completed: row.get::<i64, _>("wp_completed") != 0,
        updated_at: row.get("wp_updated"),
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

/// In-progress (non-completed) movies and episodes for the Home "Keep
/// Watching" row (spec §5.10), joined with the catalog so each carries the
/// data to render a card. Most-recently-watched first; episodes include their
/// parent series (when still in the catalog) for poster/title fallback. The
/// JOINs drop progress rows whose catalog item no longer exists.
pub async fn continue_watching(
    pool: &SqlitePool,
    provider_id: &str,
    limit: i64,
) -> Result<Vec<ContinueWatchingItem>, sqlx::Error> {
    let limit = limit.clamp(1, 200);

    // (updated_at, item) so the two content types can be merged by recency.
    let mut items: Vec<(i64, ContinueWatchingItem)> = Vec::new();

    let movie_rows = sqlx::query(
        "SELECT m.*,
                wp.position_seconds AS wp_position, wp.duration_seconds AS wp_duration,
                wp.completed AS wp_completed, wp.updated_at AS wp_updated
         FROM watch_progress wp
         JOIN movies m ON m.provider_id = wp.provider_id AND m.id = wp.content_id
         WHERE wp.provider_id = ? AND wp.content_type = 'movie' AND wp.completed = 0
         ORDER BY wp.updated_at DESC
         LIMIT ?",
    )
    .bind(provider_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    for row in &movie_rows {
        let progress = row_to_progress_aliased(row);
        items.push((
            progress.updated_at,
            ContinueWatchingItem::Movie {
                movie: row_to_movie(row),
                progress,
            },
        ));
    }

    let episode_rows = sqlx::query(
        "SELECT e.*,
                s.id AS s_id, s.name AS s_name, s.category_id AS s_category_id,
                s.category_name AS s_category_name, s.poster_url AS s_poster_url,
                s.release_year AS s_release_year,
                wp.position_seconds AS wp_position, wp.duration_seconds AS wp_duration,
                wp.completed AS wp_completed, wp.updated_at AS wp_updated
         FROM watch_progress wp
         JOIN episodes e ON e.provider_id = wp.provider_id AND e.id = wp.content_id
         LEFT JOIN series s ON s.provider_id = wp.provider_id AND s.id = e.series_id
         WHERE wp.provider_id = ? AND wp.content_type = 'episode' AND wp.completed = 0
         ORDER BY wp.updated_at DESC
         LIMIT ?",
    )
    .bind(provider_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    for row in &episode_rows {
        let progress = row_to_progress_aliased(row);
        let series = row
            .get::<Option<String>, _>("s_id")
            .map(|id| SeriesItem {
                id,
                name: row.get("s_name"),
                category_id: row.get("s_category_id"),
                category_name: row.get("s_category_name"),
                poster_url: row.get("s_poster_url"),
                release_year: row.get("s_release_year"),
            });
        items.push((
            progress.updated_at,
            ContinueWatchingItem::Episode {
                episode: row_to_episode(row),
                series,
                progress,
            },
        ));
    }

    items.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(items.into_iter().take(limit as usize).map(|(_, it)| it).collect())
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
