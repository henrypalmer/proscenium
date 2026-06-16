//! Watch-progress Tauri commands (spec §5.9 / §16). Resume position, progress
//! bars, and watched markers for VOD. Live TV is never tracked.

use crate::db::{self, Db};
use crate::models::{ContinueWatchingItem, WatchProgress};
use sqlx::SqlitePool;
use std::collections::HashMap;
use tauri::State;

/// Fraction of the runtime past which an item is considered fully watched.
pub const COMPLETION_THRESHOLD: f64 = 0.95;

/// Default number of items returned by `get_continue_watching` (spec §16).
pub const DEFAULT_CONTINUE_WATCHING_LIMIT: i64 = 20;

fn validate_content_type(content_type: &str) -> Result<(), String> {
    match content_type {
        "movie" | "episode" => Ok(()),
        other => Err(format!("Watch progress is not tracked for '{other}'.")),
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn set_watch_progress_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    position_seconds: f64,
    duration_seconds: Option<f64>,
) -> Result<(), String> {
    validate_content_type(content_type)?;
    let position = position_seconds.max(0.0).round() as i64;
    let duration = duration_seconds.filter(|d| *d > 0.0).map(|d| d.round() as i64);
    // Completion is only meaningful when the runtime is known.
    let completed = match (duration_seconds, position_seconds) {
        (Some(d), p) if d > 0.0 => p / d >= COMPLETION_THRESHOLD,
        _ => false,
    };
    db::watch::upsert(
        pool,
        provider_id,
        content_type,
        content_id,
        position,
        duration,
        completed,
        now_unix(),
    )
    .await
    .map_err(|e| format!("Failed to save watch progress: {e}"))
}

#[tauri::command]
pub async fn get_watch_progress(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
) -> Result<Option<WatchProgress>, String> {
    validate_content_type(&content_type)?;
    db::watch::get(&state.0, &provider_id, &content_type, &content_id)
        .await
        .map_err(|e| format!("Failed to read watch progress: {e}"))
}

#[tauri::command]
pub async fn set_watch_progress(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
    position_seconds: f64,
    duration_seconds: Option<f64>,
) -> Result<(), String> {
    set_watch_progress_impl(
        &state.0,
        &provider_id,
        &content_type,
        &content_id,
        position_seconds,
        duration_seconds,
    )
    .await
}

/// Force an item to "watched" (Keep Watching → Mark as watched, spec §5.10).
/// Sets the completion flag regardless of whether the runtime is known (unlike
/// `set_watch_progress`, which can only infer completion from position/duration),
/// parking the position at the end when the duration is known.
pub async fn mark_watched_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    duration_seconds: Option<f64>,
) -> Result<(), String> {
    validate_content_type(content_type)?;
    let duration = duration_seconds.filter(|d| *d > 0.0).map(|d| d.round() as i64);
    let position = duration.unwrap_or(0);
    db::watch::upsert(
        pool,
        provider_id,
        content_type,
        content_id,
        position,
        duration,
        true,
        now_unix(),
    )
    .await
    .map_err(|e| format!("Failed to mark watched: {e}"))
}

#[tauri::command]
pub async fn mark_watched(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
    duration_seconds: Option<f64>,
) -> Result<(), String> {
    mark_watched_impl(
        &state.0,
        &provider_id,
        &content_type,
        &content_id,
        duration_seconds,
    )
    .await
}

#[tauri::command]
pub async fn list_watch_progress(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
) -> Result<HashMap<String, WatchProgress>, String> {
    validate_content_type(&content_type)?;
    db::watch::list(&state.0, &provider_id, &content_type)
        .await
        .map_err(|e| format!("Failed to list watch progress: {e}"))
}

#[tauri::command]
pub async fn get_continue_watching(
    state: State<'_, Db>,
    provider_id: String,
    limit: Option<i64>,
) -> Result<Vec<ContinueWatchingItem>, String> {
    db::watch::continue_watching(
        &state.0,
        &provider_id,
        limit.unwrap_or(DEFAULT_CONTINUE_WATCHING_LIMIT),
    )
    .await
    .map_err(|e| format!("Failed to load continue watching: {e}"))
}

#[tauri::command]
pub async fn clear_watch_progress(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
) -> Result<(), String> {
    validate_content_type(&content_type)?;
    db::watch::clear(&state.0, &provider_id, &content_type, &content_id)
        .await
        .map_err(|e| format!("Failed to clear watch progress: {e}"))
}
