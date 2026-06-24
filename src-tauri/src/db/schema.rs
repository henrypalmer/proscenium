//! Schema definitions (spec §15). Applied idempotently on every launch.

use sqlx::{Row, SqlitePool};

const SCHEMA: &str = r#"
-- Providers
CREATE TABLE IF NOT EXISTS providers (
  id             TEXT PRIMARY KEY,       -- UUID
  name           TEXT NOT NULL,
  type           TEXT NOT NULL CHECK (type IN ('xtream', 'm3u')),
  server_url     TEXT,
  username       TEXT,
  password       TEXT,                   -- Keychain reference key, never the secret
  playlist_url   TEXT,
  local_file_path TEXT,
  last_refreshed INTEGER,                -- Unix timestamp, nullable
  created_at     INTEGER NOT NULL        -- Unix timestamp
);

-- Live channels
CREATE TABLE IF NOT EXISTS live_channels (
  id             TEXT NOT NULL,
  provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  category_id    TEXT NOT NULL,
  category_name  TEXT NOT NULL,
  logo_url       TEXT,
  stream_url     TEXT NOT NULL,
  stream_ext     TEXT NOT NULL,
  epg_channel_id TEXT,
  PRIMARY KEY (id, provider_id)
);

-- Live channel categories (for sidebar population)
CREATE TABLE IF NOT EXISTS live_categories (
  id           TEXT NOT NULL,
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (id, provider_id)
);

-- Movies
CREATE TABLE IF NOT EXISTS movies (
  id             TEXT NOT NULL,
  provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  category_id    TEXT NOT NULL,
  category_name  TEXT NOT NULL,
  poster_url     TEXT,
  stream_url     TEXT NOT NULL,
  container_ext  TEXT NOT NULL,
  release_year   INTEGER,
  rating         TEXT,
  imdb_id        TEXT,
  imdb_rating    REAL,
  added_at       INTEGER,               -- Unix timestamp
  PRIMARY KEY (id, provider_id)
);

-- VOD categories
CREATE TABLE IF NOT EXISTS vod_categories (
  id           TEXT NOT NULL,
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (id, provider_id)
);

-- TV series
CREATE TABLE IF NOT EXISTS series (
  id             TEXT NOT NULL,
  provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  category_id    TEXT NOT NULL,
  category_name  TEXT NOT NULL,
  poster_url     TEXT,
  release_year   INTEGER,
  imdb_id        TEXT,
  imdb_rating    REAL,
  PRIMARY KEY (id, provider_id)
);

-- Series categories
CREATE TABLE IF NOT EXISTS series_categories (
  id           TEXT NOT NULL,
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (id, provider_id)
);

-- Episodes
CREATE TABLE IF NOT EXISTS episodes (
  id               TEXT NOT NULL,
  series_id        TEXT NOT NULL,
  provider_id      TEXT NOT NULL,
  season           INTEGER NOT NULL,
  episode          INTEGER NOT NULL,
  title            TEXT NOT NULL,
  stream_url       TEXT NOT NULL,
  container_ext    TEXT NOT NULL,
  duration_seconds INTEGER,
  poster_url       TEXT,
  overview         TEXT,
  PRIMARY KEY (id, provider_id),
  FOREIGN KEY (series_id, provider_id) REFERENCES series(id, provider_id) ON DELETE CASCADE
);

-- App settings (key-value store)
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Cached cover art (local disk path index)
CREATE TABLE IF NOT EXISTS image_cache (
  url           TEXT PRIMARY KEY,
  local_path    TEXT NOT NULL,
  cached_at     INTEGER NOT NULL,       -- Unix timestamp
  expires_at    INTEGER NOT NULL        -- Unix timestamp (cached_at + 30 days)
);

-- Watch progress (§5.9). Resume position + completion for VOD only; live TV is
-- never tracked. Rows cascade-delete with their provider.
CREATE TABLE IF NOT EXISTS watch_progress (
  provider_id      TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  content_type     TEXT NOT NULL CHECK (content_type IN ('movie', 'episode')),
  content_id       TEXT NOT NULL,
  position_seconds INTEGER NOT NULL,            -- last playback position
  duration_seconds INTEGER,                     -- total runtime when known (for the progress bar)
  completed        INTEGER NOT NULL DEFAULT 0,  -- 1 once watched to the completion threshold (~95%)
  updated_at       INTEGER NOT NULL,            -- Unix timestamp of last write
  PRIMARY KEY (provider_id, content_type, content_id)
);

-- Custom user lists / "playlists" (§5.11). Provider-scoped; cascade-delete with
-- the provider. A list may mix movies, series, and live channels.
CREATE TABLE IF NOT EXISTS user_lists (
  id          TEXT PRIMARY KEY,                                     -- app-generated UUID
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,                           -- user ordering of lists
  created_at  INTEGER NOT NULL,                                     -- Unix timestamp
  updated_at  INTEGER NOT NULL                                      -- Unix timestamp (membership/name changes)
);

-- Membership rows for user_lists. content_id refers to movies.id / series.id /
-- live_channels.id depending on content_type (resolved by JOIN). Orphaned rows
-- (content dropped on refresh) are retained but filtered out at read time.
CREATE TABLE IF NOT EXISTS user_list_items (
  list_id      TEXT NOT NULL REFERENCES user_lists(id) ON DELETE CASCADE,
  content_type TEXT NOT NULL CHECK (content_type IN ('live', 'movie', 'series')),
  content_id   TEXT NOT NULL,
  position     INTEGER NOT NULL,            -- order within the list (newest-added last by default)
  added_at     INTEGER NOT NULL,            -- Unix timestamp
  PRIMARY KEY (list_id, content_type, content_id)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_live_channels_provider    ON live_channels(provider_id);
CREATE INDEX IF NOT EXISTS idx_live_channels_category    ON live_channels(provider_id, category_id);
CREATE INDEX IF NOT EXISTS idx_movies_provider           ON movies(provider_id);
CREATE INDEX IF NOT EXISTS idx_movies_category           ON movies(provider_id, category_id);
CREATE INDEX IF NOT EXISTS idx_series_provider           ON series(provider_id);
CREATE INDEX IF NOT EXISTS idx_series_category           ON series(provider_id, category_id);
CREATE INDEX IF NOT EXISTS idx_episodes_series           ON episodes(series_id, provider_id);
CREATE INDEX IF NOT EXISTS idx_watch_progress_section    ON watch_progress(provider_id, content_type);
CREATE INDEX IF NOT EXISTS idx_user_lists_provider       ON user_lists(provider_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_user_list_items_list      ON user_list_items(list_id, position);

-- Supplementary to §15: alphabetical paging over large catalogs (§10) needs
-- an ordered index or every page query re-sorts the full table.
CREATE INDEX IF NOT EXISTS idx_live_channels_name ON live_channels(provider_id, name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_movies_name        ON movies(provider_id, name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_series_name        ON series(provider_id, name COLLATE NOCASE);

-- Full-text search virtual tables (populated during catalog refresh, Milestone 2)
CREATE VIRTUAL TABLE IF NOT EXISTS fts_live_channels USING fts5(
  id, provider_id, name, category_name,
  content='live_channels', content_rowid='rowid'
);
CREATE VIRTUAL TABLE IF NOT EXISTS fts_movies USING fts5(
  id, provider_id, name, category_name,
  content='movies', content_rowid='rowid'
);
CREATE VIRTUAL TABLE IF NOT EXISTS fts_series USING fts5(
  id, provider_id, name, category_name,
  content='series', content_rowid='rowid'
);
"#;

pub async fn apply(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(SCHEMA).execute(pool).await?;
    // Columns added after the initial release need an idempotent backfill for
    // existing databases — `CREATE TABLE IF NOT EXISTS` above only covers fresh
    // installs, and SQLite's `ALTER TABLE ADD COLUMN` has no `IF NOT EXISTS`.
    add_column_if_missing(pool, "episodes", "overview", "TEXT").await?; // M20 §5.4
    Ok(())
}

/// Add `column` to `table` only when it is absent, so the migration is safe to
/// re-run on every launch (and on databases created before the column existed).
async fn add_column_if_missing(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    decl: &str,
) -> Result<(), sqlx::Error> {
    let present = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?
        .iter()
        .any(|row| row.get::<String, _>("name") == column);
    if !present {
        sqlx::query(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))
            .execute(pool)
            .await?;
    }
    Ok(())
}
