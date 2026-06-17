//! Custom user lists / "playlists" storage (spec §5.11). Provider-scoped; rows
//! cascade-delete with their list, and lists cascade-delete with their provider.
//! Membership references catalog rows by `(content_type, content_id)` and is
//! resolved by JOIN; orphaned items (content dropped on refresh) are retained
//! but filtered out at read time.

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

pub async fn create(
    pool: &SqlitePool,
    provider_id: &str,
    name: &str,
    now: i64,
) -> Result<UserList, sqlx::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let sort_order: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(sort_order) + 1, 0) FROM user_lists WHERE provider_id = ?")
            .bind(provider_id)
            .fetch_one(pool)
            .await?;
    sqlx::query(
        "INSERT INTO user_lists (id, provider_id, name, sort_order, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(provider_id)
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
    provider_id: &str,
    ordered_ids: &[String],
    now: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE user_lists SET sort_order = ?, updated_at = ? WHERE id = ? AND provider_id = ?",
        )
        .bind(idx as i64)
        .bind(now)
        .bind(id)
        .bind(provider_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn add_item(
    pool: &SqlitePool,
    list_id: &str,
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
        "INSERT OR IGNORE INTO user_list_items (list_id, content_type, content_id, position, added_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(list_id)
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
    content_type: &str,
    content_id: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM user_list_items WHERE list_id = ? AND content_type = ? AND content_id = ?",
    )
    .bind(list_id)
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

/// Reorder items within a list. Keys are `"<content_type>:<content_id>"`.
pub async fn reorder_items(
    pool: &SqlitePool,
    list_id: &str,
    ordered_keys: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (idx, key) in ordered_keys.iter().enumerate() {
        if let Some((content_type, content_id)) = key.split_once(':') {
            sqlx::query(
                "UPDATE user_list_items SET position = ?
                 WHERE list_id = ? AND content_type = ? AND content_id = ?",
            )
            .bind(idx as i64)
            .bind(list_id)
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
/// longer exists are omitted (orphans hidden, membership retained).
pub async fn items(pool: &SqlitePool, list_id: &str) -> Result<Vec<UserListItem>, sqlx::Error> {
    let provider_id: Option<String> =
        sqlx::query_scalar("SELECT provider_id FROM user_lists WHERE id = ?")
            .bind(list_id)
            .fetch_optional(pool)
            .await?;
    let Some(provider_id) = provider_id else {
        return Ok(Vec::new());
    };

    // (position, item) across the three content tables, merged by position.
    let mut out: Vec<(i64, UserListItem)> = Vec::new();

    let movie_rows = sqlx::query(
        "SELECT m.*, li.position AS li_position
         FROM user_list_items li
         JOIN movies m ON m.provider_id = ? AND m.id = li.content_id
         WHERE li.list_id = ? AND li.content_type = 'movie'",
    )
    .bind(&provider_id)
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
         JOIN series s ON s.provider_id = ? AND s.id = li.content_id
         WHERE li.list_id = ? AND li.content_type = 'series'",
    )
    .bind(&provider_id)
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
         JOIN live_channels c ON c.provider_id = ? AND c.id = li.content_id
         WHERE li.list_id = ? AND li.content_type = 'live'",
    )
    .bind(&provider_id)
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

/// All of a provider's lists (in sort order) with item count and cover posters
/// for the Home "My Lists" row (spec §5.10).
pub async fn summaries(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Vec<ListSummary>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, sort_order, created_at, updated_at
         FROM user_lists WHERE provider_id = ? ORDER BY sort_order, created_at",
    )
    .bind(provider_id)
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

/// IDs of the provider's lists that already contain a given item — backs the
/// "Add to list" picker checkmarks.
pub async fn lists_for_item(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT li.list_id FROM user_list_items li
         JOIN user_lists l ON l.id = li.list_id
         WHERE l.provider_id = ? AND li.content_type = ? AND li.content_id = ?",
    )
    .bind(provider_id)
    .bind(content_type)
    .bind(content_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("list_id")).collect())
}
