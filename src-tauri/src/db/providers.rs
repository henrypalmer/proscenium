use crate::models::{Provider, ProviderInput, ProviderType};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

fn row_to_provider(row: &SqliteRow) -> Provider {
    let type_str: String = row.get("type");
    Provider {
        id: row.get("id"),
        name: row.get("name"),
        provider_type: ProviderType::from_str(&type_str).unwrap_or(ProviderType::M3u),
        server_url: row.get("server_url"),
        username: row.get("username"),
        playlist_url: row.get("playlist_url"),
        local_file_path: row.get("local_file_path"),
        last_refreshed: row.get("last_refreshed"),
        created_at: row.get("created_at"),
    }
}

/// Insert or update a provider row. `password_ref` is the keychain reference
/// key (never the secret); when `None` on update, the existing value is kept.
pub async fn upsert(
    pool: &SqlitePool,
    id: &str,
    input: &ProviderInput,
    password_ref: Option<&str>,
    created_at: i64,
) -> Result<Provider, sqlx::Error> {
    sqlx::query(
        "INSERT INTO providers
           (id, name, type, server_url, username, password, playlist_url, local_file_path, last_refreshed, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           type = excluded.type,
           server_url = excluded.server_url,
           username = excluded.username,
           password = COALESCE(excluded.password, providers.password),
           playlist_url = excluded.playlist_url,
           local_file_path = excluded.local_file_path",
    )
    .bind(id)
    .bind(&input.name)
    .bind(input.provider_type.as_str())
    .bind(&input.server_url)
    .bind(&input.username)
    .bind(password_ref)
    .bind(&input.playlist_url)
    .bind(&input.local_file_path)
    .bind(created_at)
    .execute(pool)
    .await?;

    get(pool, id).await?.ok_or(sqlx::Error::RowNotFound)
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Provider>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM providers WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_provider))
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Provider>, sqlx::Error> {
    let rows = sqlx::query("SELECT * FROM providers ORDER BY created_at, name")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(row_to_provider).collect())
}

/// Delete a provider row. Catalog rows are removed by `ON DELETE CASCADE`.
/// Returns whether a row was actually deleted.
pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM providers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
