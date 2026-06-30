//! Custom user lists / "playlists" storage (spec §5.11). Global since Milestone
//! 39 (not provider-scoped): a list may mix items from several providers, and
//! each membership row carries its own `(provider_id, content_id)` addressing a
//! catalog row, resolved by JOIN. Items cascade-delete with their list; orphaned
//! items (content dropped on refresh, or its provider removed) are retained but
//! filtered out at read time.

use crate::db::catalog::{row_to_live_channel, row_to_movie, row_to_series};
use crate::models::{ListSummary, UserList, UserListItem};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

fn row_to_list(row: &SqliteRow) -> UserList {
    UserList {
        id: row.get("id"),
        name: row.get("name"),
        sort_order: row.get("sort_order"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn poster_of(item: &UserListItem) -> Option<String> {
    match item {
        UserListItem::Movie { movie } => movie.poster_url.clone(),
        UserListItem::Series { series } => series.poster_url.clone(),
        UserListItem::Live { channel } => channel.logo_url.clone(),
    }
}

pub async fn create(pool: &SqlitePool, name: &str, now: i64) -> Result<UserList, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let sort_order: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(sort_order) + 1, 0) FROM user_lists")
            .fetch_one(pool)
            .await?;
    sqlx::query(
        "INSERT INTO user_lists (id, name, sort_order, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(sort_order)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(UserList {
        id,
        name: name.to_string(),
        sort_order,
        created_at: now,
        updated_at: now,
    })
}

pub async fn rename(
    pool: &SqlitePool,
    list_id: &str,
    name: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE user_lists SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(now)
        .bind(list_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, list_id: &str) -> Result<(), sqlx::Error> {
    // Membership rows cascade via the FK.
    sqlx::query("DELETE FROM user_lists WHERE id = ?")
        .bind(list_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn reorder(
    pool: &SqlitePool,
    ordered_ids: &[String],
    now: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        sqlx::query("UPDATE user_lists SET sort_order = ?, updated_at = ? WHERE id = ?")
            .bind(idx as i64)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn add_item(
    pool: &SqlitePool,
    list_id: &str,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let position: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(position) + 1, 0) FROM user_list_items WHERE list_id = ?")
            .bind(list_id)
            .fetch_one(pool)
            .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO user_list_items
           (list_id, provider_id, content_type, content_id, position, added_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(list_id)
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .bind(position)
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query("UPDATE user_lists SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(list_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_item(
    pool: &SqlitePool,
    list_id: &str,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM user_list_items
         WHERE list_id = ? AND provider_id = ? AND content_type = ? AND content_id = ?",
    )
    .bind(list_id)
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .execute(pool)
    .await?;
    sqlx::query("UPDATE user_lists SET updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(list_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Reorder items within a list. Keys are `"<content_type>:<provider_id>:<content_id>"`.
pub async fn reorder_items(
    pool: &SqlitePool,
    list_id: &str,
    ordered_keys: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (idx, key) in ordered_keys.iter().enumerate() {
        let mut parts = key.splitn(3, ':');
        if let (Some(content_type), Some(provider_id), Some(content_id)) =
            (parts.next(), parts.next(), parts.next())
        {
            sqlx::query(
                "UPDATE user_list_items SET position = ?
                 WHERE list_id = ? AND provider_id = ? AND content_type = ? AND content_id = ?",
            )
            .bind(idx as i64)
            .bind(list_id)
            .bind(provider_id)
            .bind(content_type)
            .bind(content_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// The resolved items of a list, in list order. Items whose catalog row no
/// longer exists are omitted (orphans hidden, membership retained). Each item is
/// joined on its own `(provider_id, content_id)` (Milestone 39).
pub async fn items(pool: &SqlitePool, list_id: &str) -> Result<Vec<UserListItem>, sqlx::Error> {
    // (position, item) across the three content tables, merged by position.
    let mut out: Vec<(i64, UserListItem)> = Vec::new();

    let movie_rows = sqlx::query(
        "SELECT m.*, li.position AS li_position
         FROM user_list_items li
         JOIN movies m ON m.provider_id = li.provider_id AND m.id = li.content_id
         WHERE li.list_id = ? AND li.content_type = 'movie'",
    )
    .bind(list_id)
    .fetch_all(pool)
    .await?;
    for r in &movie_rows {
        out.push((
            r.get("li_position"),
            UserListItem::Movie {
                movie: row_to_movie(r),
            },
        ));
    }

    let series_rows = sqlx::query(
        "SELECT s.*, li.position AS li_position
         FROM user_list_items li
         JOIN series s ON s.provider_id = li.provider_id AND s.id = li.content_id
         WHERE li.list_id = ? AND li.content_type = 'series'",
    )
    .bind(list_id)
    .fetch_all(pool)
    .await?;
    for r in &series_rows {
        out.push((
            r.get("li_position"),
            UserListItem::Series {
                series: row_to_series(r),
            },
        ));
    }

    let live_rows = sqlx::query(
        "SELECT c.*, li.position AS li_position
         FROM user_list_items li
         JOIN live_channels c ON c.provider_id = li.provider_id AND c.id = li.content_id
         WHERE li.list_id = ? AND li.content_type = 'live'",
    )
    .bind(list_id)
    .fetch_all(pool)
    .await?;
    for r in &live_rows {
        out.push((
            r.get("li_position"),
            UserListItem::Live {
                channel: row_to_live_channel(r),
            },
        ));
    }

    out.sort_by_key(|(pos, _)| *pos);
    Ok(out.into_iter().map(|(_, item)| item).collect())
}

/// All lists (in sort order) with item count and cover posters for the Home
/// "My Lists" row (spec §5.10). Global since Milestone 39.
pub async fn summaries(pool: &SqlitePool) -> Result<Vec<ListSummary>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, sort_order, created_at, updated_at
         FROM user_lists ORDER BY sort_order, created_at",
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let list = row_to_list(row);
        let resolved = items(pool, &list.id).await?;
        let cover_posters = resolved.iter().take(4).map(poster_of).collect();
        out.push(ListSummary {
            list,
            item_count: resolved.len() as i64,
            cover_posters,
        });
    }
    Ok(out)
}

/// IDs of the lists that already contain a given `(provider_id, content_id)`
/// item — backs the "Add to list" picker checkmarks (Milestone 39: provider-aware).
pub async fn lists_for_item(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT list_id FROM user_list_items
         WHERE provider_id = ? AND content_type = ? AND content_id = ?",
    )
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("list_id")).collect())
}
