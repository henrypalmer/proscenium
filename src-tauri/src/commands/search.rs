//! Search Tauri command (spec §16): FTS5 query over the local catalog
//! cache. Entirely local — no provider request is ever made.

use crate::db::{self, Db, ProviderScope};
use crate::models::{SearchContentType, SearchResults};
use sqlx::SqlitePool;
use tauri::State;

pub const DEFAULT_SEARCH_LIMIT: i64 = 20;

pub async fn search_impl(
    pool: &SqlitePool,
    provider_ids: impl ProviderScope,
    query: &str,
    content_type: Option<SearchContentType>,
    category_name: Option<&str>,
    limit: Option<i64>,
) -> Result<SearchResults, String> {
    db::catalog::search_catalog(
        pool,
        provider_ids,
        query,
        content_type.unwrap_or(SearchContentType::All),
        category_name,
        limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
    )
    .await
    .map_err(|e| format!("Search failed: {e}"))
}

#[tauri::command]
pub async fn search(
    state: State<'_, Db>,
    provider_ids: Vec<String>,
    query: String,
    content_type: Option<SearchContentType>,
    category_id: Option<String>,
    limit: Option<i64>,
) -> Result<SearchResults, String> {
    // `category_id` is the merged-category key (a category name, Milestone 39).
    search_impl(
        &state.0,
        &provider_ids,
        &query,
        content_type,
        category_id.as_deref(),
        limit,
    )
    .await
}
