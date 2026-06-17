//! Custom list / "playlist" Tauri commands (spec §5.11 / §16). The `_impl`
//! functions hold the logic so integration tests can exercise them without a
//! Tauri runtime. Lists are provider-scoped and entirely local.

use crate::db::{self, Db};
use crate::models::{ListSummary, UserList, UserListItem};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn validate_content_type(content_type: &str) -> Result<(), String> {
    match content_type {
        "live" | "movie" | "series" => Ok(()),
        other => Err(format!("'{other}' cannot be added to a list.")),
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("List name is required.".into());
    }
    Ok(())
}

// --- List management ---

pub async fn create_list_impl(
    pool: &SqlitePool,
    provider_id: &str,
    name: &str,
) -> Result<UserList, String> {
    validate_name(name)?;
    db::lists::create(pool, provider_id, name.trim(), now_unix())
        .await
        .map_err(|e| format!("Failed to create list: {e}"))
}

pub async fn rename_list_impl(
    pool: &SqlitePool,
    list_id: &str,
    name: &str,
) -> Result<(), String> {
    validate_name(name)?;
    db::lists::rename(pool, list_id, name.trim(), now_unix())
        .await
        .map_err(|e| format!("Failed to rename list: {e}"))
}

pub async fn delete_list_impl(pool: &SqlitePool, list_id: &str) -> Result<(), String> {
    db::lists::delete(pool, list_id)
        .await
        .map_err(|e| format!("Failed to delete list: {e}"))
}

pub async fn reorder_lists_impl(
    pool: &SqlitePool,
    provider_id: &str,
    ordered_list_ids: &[String],
) -> Result<(), String> {
    db::lists::reorder(pool, provider_id, ordered_list_ids, now_unix())
        .await
        .map_err(|e| format!("Failed to reorder lists: {e}"))
}

pub async fn get_lists_impl(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Vec<ListSummary>, String> {
    db::lists::summaries(pool, provider_id)
        .await
        .map_err(|e| format!("Failed to load lists: {e}"))
}

// --- Membership ---

pub async fn add_to_list_impl(
    pool: &SqlitePool,
    list_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<(), String> {
    validate_content_type(content_type)?;
    db::lists::add_item(pool, list_id, content_type, content_id, now_unix())
        .await
        .map_err(|e| format!("Failed to add to list: {e}"))
}

pub async fn remove_from_list_impl(
    pool: &SqlitePool,
    list_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<(), String> {
    validate_content_type(content_type)?;
    db::lists::remove_item(pool, list_id, content_type, content_id, now_unix())
        .await
        .map_err(|e| format!("Failed to remove from list: {e}"))
}

pub async fn reorder_list_items_impl(
    pool: &SqlitePool,
    list_id: &str,
    ordered_item_keys: &[String],
) -> Result<(), String> {
    db::lists::reorder_items(pool, list_id, ordered_item_keys)
        .await
        .map_err(|e| format!("Failed to reorder list items: {e}"))
}

pub async fn get_list_items_impl(
    pool: &SqlitePool,
    list_id: &str,
) -> Result<Vec<UserListItem>, String> {
    db::lists::items(pool, list_id)
        .await
        .map_err(|e| format!("Failed to load list items: {e}"))
}

pub async fn get_lists_for_item_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<Vec<String>, String> {
    validate_content_type(content_type)?;
    db::lists::lists_for_item(pool, provider_id, content_type, content_id)
        .await
        .map_err(|e| format!("Failed to load lists for item: {e}"))
}

// --- Tauri command wrappers ---

#[tauri::command]
pub async fn create_list(
    state: State<'_, Db>,
    provider_id: String,
    name: String,
) -> Result<UserList, String> {
    create_list_impl(&state.0, &provider_id, &name).await
}

#[tauri::command]
pub async fn rename_list(
    state: State<'_, Db>,
    list_id: String,
    name: String,
) -> Result<(), String> {
    rename_list_impl(&state.0, &list_id, &name).await
}

#[tauri::command]
pub async fn delete_list(state: State<'_, Db>, list_id: String) -> Result<(), String> {
    delete_list_impl(&state.0, &list_id).await
}

#[tauri::command]
pub async fn reorder_lists(
    state: State<'_, Db>,
    provider_id: String,
    ordered_list_ids: Vec<String>,
) -> Result<(), String> {
    reorder_lists_impl(&state.0, &provider_id, &ordered_list_ids).await
}

#[tauri::command]
pub async fn get_lists(
    state: State<'_, Db>,
    provider_id: String,
) -> Result<Vec<ListSummary>, String> {
    get_lists_impl(&state.0, &provider_id).await
}

#[tauri::command]
pub async fn add_to_list(
    state: State<'_, Db>,
    list_id: String,
    content_type: String,
    content_id: String,
) -> Result<(), String> {
    add_to_list_impl(&state.0, &list_id, &content_type, &content_id).await
}

#[tauri::command]
pub async fn remove_from_list(
    state: State<'_, Db>,
    list_id: String,
    content_type: String,
    content_id: String,
) -> Result<(), String> {
    remove_from_list_impl(&state.0, &list_id, &content_type, &content_id).await
}

#[tauri::command]
pub async fn reorder_list_items(
    state: State<'_, Db>,
    list_id: String,
    ordered_item_keys: Vec<String>,
) -> Result<(), String> {
    reorder_list_items_impl(&state.0, &list_id, &ordered_item_keys).await
}

#[tauri::command]
pub async fn get_list_items(
    state: State<'_, Db>,
    list_id: String,
) -> Result<Vec<UserListItem>, String> {
    get_list_items_impl(&state.0, &list_id).await
}

#[tauri::command]
pub async fn get_lists_for_item(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
) -> Result<Vec<String>, String> {
    get_lists_for_item_impl(&state.0, &provider_id, &content_type, &content_id).await
}
