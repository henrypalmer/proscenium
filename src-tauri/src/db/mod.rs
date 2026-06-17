pub mod catalog;
pub mod image_cache;
pub mod lists;
pub mod providers;
pub mod schema;
pub mod settings;
pub mod watch;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

/// Tauri-managed state wrapping the connection pool.
pub struct Db(pub SqlitePool);

/// Open (creating if needed) the database at `db_path` and apply the schema.
pub async fn init(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
    }
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    schema::apply(&pool).await?;
    Ok(pool)
}
