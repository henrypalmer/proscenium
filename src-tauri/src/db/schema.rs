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

-- Cached cover art (local disk path index). `size_bytes` + `last_accessed`
-- back the Milestone 27 LRU size cap layered on top of the 30-day TTL.
CREATE TABLE IF NOT EXISTS image_cache (
  url           TEXT PRIMARY KEY,
  local_path    TEXT NOT NULL,
  cached_at     INTEGER NOT NULL,       -- Unix timestamp
  expires_at    INTEGER NOT NULL,       -- Unix timestamp (cached_at + 30 days)
  size_bytes    INTEGER NOT NULL DEFAULT 0,
  last_accessed INTEGER NOT NULL DEFAULT 0
);

-- Tier-2 disposable cache for canonical (Cinemeta) catalog/meta responses
-- (Milestone 40). Throwaway: a miss re-fetches, and on a Cinemeta failure a
-- stale row is still served so browse works offline. Not provider-scoped and
-- untouched by catalog refresh. Keyed by the request (kind + params).
CREATE TABLE IF NOT EXISTS canonical_cache (
  cache_key  TEXT PRIMARY KEY,
  body       TEXT NOT NULL,          -- JSON payload (app Canonical* models)
  cached_at  INTEGER NOT NULL,       -- Unix timestamp
  expires_at INTEGER NOT NULL        -- Unix timestamp
);

-- Canonical↔provider match index (Milestone 40). Maps a provider's stable
-- catalog item to a canonical IMDB id. Expensive to derive (name+year +
-- tmdb confirm), so it lives in this side table that catalog refresh does NOT
-- touch: refresh deletes+reinserts catalog rows by their stable ids, so the
-- match stays valid across refreshes. Cascade-deletes with the provider.
CREATE TABLE IF NOT EXISTS content_match (
  provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  content_type TEXT NOT NULL CHECK (content_type IN ('movie', 'series')),
  content_id   TEXT NOT NULL,
  imdb_id      TEXT NOT NULL,
  tmdb_id      INTEGER,
  confidence   REAL NOT NULL DEFAULT 0,
  method       TEXT NOT NULL,          -- 'tmdb' | 'name_year' | 'manual'
  matched_at   INTEGER NOT NULL,       -- Unix timestamp
  PRIMARY KEY (provider_id, content_type, content_id)
);
CREATE INDEX IF NOT EXISTS idx_content_match_imdb ON content_match(imdb_id, content_type);

-- Installed Stremio stream addons (Milestone 41). Durable Tier-1 metadata only:
-- the token-bearing manifest URL is a secret and lives in the OS keychain
-- (account `addon:{id}`), never here — `manifest_ref` is just the keychain
-- reference. `types`/`resources`/`id_prefixes` are JSON arrays of the addon's
-- declared (non-secret) manifest metadata.
CREATE TABLE IF NOT EXISTS stremio_addons (
  id           TEXT PRIMARY KEY,        -- app-generated UUID
  name         TEXT NOT NULL,
  manifest_ref TEXT NOT NULL,           -- keychain reference key, never the URL
  types        TEXT NOT NULL,           -- JSON array
  resources    TEXT NOT NULL,           -- JSON array
  id_prefixes  TEXT NOT NULL,           -- JSON array
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL
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

-- Custom user lists / "playlists" (§5.11). Global (Milestone 39): not scoped to
-- a provider, so a list may mix items from several providers, movies, series,
-- and live channels.
CREATE TABLE IF NOT EXISTS user_lists (
  id          TEXT PRIMARY KEY,                                     -- app-generated UUID
  name        TEXT NOT NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,                           -- user ordering of lists
  created_at  INTEGER NOT NULL,                                     -- Unix timestamp
  updated_at  INTEGER NOT NULL                                      -- Unix timestamp (membership/name changes)
);

-- Membership rows for user_lists. (provider_id, content_id) addresses a catalog
-- row in movies / series / live_channels by content_type (resolved by JOIN).
-- provider_id is part of the key so the same content id from two providers can
-- both be added (Milestone 39). Orphaned rows (content dropped on refresh, or
-- its provider removed) are retained but filtered out at read time.
CREATE TABLE IF NOT EXISTS user_list_items (
  list_id      TEXT NOT NULL REFERENCES user_lists(id) ON DELETE CASCADE,
  provider_id  TEXT NOT NULL,
  content_type TEXT NOT NULL CHECK (content_type IN ('live', 'movie', 'series')),
  content_id   TEXT NOT NULL,
  position     INTEGER NOT NULL,            -- order within the list (newest-added last by default)
  added_at     INTEGER NOT NULL,            -- Unix timestamp
  PRIMARY KEY (list_id, content_type, content_id, provider_id)
);

-- Recently-watched live channels (spec §13, Milestone 29). Local-only,
-- most-recent first; cascade-deletes with the provider. One row per channel
-- (re-watching bumps `watched_at`).
CREATE TABLE IF NOT EXISTS recent_channels (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  channel_id  TEXT NOT NULL,
  watched_at  INTEGER NOT NULL,        -- Unix timestamp of the last watch
  PRIMARY KEY (provider_id, channel_id)
);

-- User-defined category ordering per provider + section (spec §13, Milestone
-- 29 "custom M3U group ordering"). Absent → the default provider/sort order is
-- used. Cascade-deletes with the provider.
CREATE TABLE IF NOT EXISTS category_order (
  provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  section     TEXT NOT NULL CHECK (section IN ('live', 'movie', 'series')),
  category_id TEXT NOT NULL,
  position    INTEGER NOT NULL,
  PRIMARY KEY (provider_id, section, category_id)
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
CREATE INDEX IF NOT EXISTS idx_user_lists_order          ON user_lists(sort_order);
CREATE INDEX IF NOT EXISTS idx_user_list_items_list      ON user_list_items(list_id, position);
CREATE INDEX IF NOT EXISTS idx_recent_channels_recency   ON recent_channels(provider_id, watched_at DESC);

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
    // M27 §5.7 — LRU size cap on the image cache; backfill existing DBs.
    add_column_if_missing(pool, "image_cache", "size_bytes", "INTEGER NOT NULL DEFAULT 0").await?;
    add_column_if_missing(pool, "image_cache", "last_accessed", "INTEGER NOT NULL DEFAULT 0").await?;
    scrub_xtream_stream_urls(pool).await?; // M21 §5.1 — credential hardening
    migrate_lists_multi_provider(pool).await?; // M39 — global lists + per-item provider
    Ok(())
}

/// Milestone 39: custom lists become **global** (no longer provider-scoped) and
/// each membership row carries its own `provider_id`. When the pre-M39 shape is
/// detected (a `provider_id` column still on `user_lists`), rebuild both tables,
/// backfilling each item's `provider_id` from its parent list. Idempotent — a
/// no-op once migrated (and on fresh installs, which already have the new shape).
async fn migrate_lists_multi_provider(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let pre_m39 = sqlx::query("PRAGMA table_info(user_lists)")
        .fetch_all(pool)
        .await?
        .iter()
        .any(|row| row.get::<String, _>("name") == "provider_id");
    if !pre_m39 {
        return Ok(());
    }

    // FKs are enforced (foreign_keys=ON). Renaming a referenced table updates the
    // child's FK to the new name, so order matters: rename old → create new →
    // copy (parent before child) → drop child before parent.
    let mut tx = pool.begin().await?;
    sqlx::query("ALTER TABLE user_lists RENAME TO _m39_old_user_lists")
        .execute(&mut *tx)
        .await?;
    sqlx::query("ALTER TABLE user_list_items RENAME TO _m39_old_user_list_items")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "CREATE TABLE user_lists (
           id TEXT PRIMARY KEY, name TEXT NOT NULL, sort_order INTEGER NOT NULL DEFAULT 0,
           created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "CREATE TABLE user_list_items (
           list_id TEXT NOT NULL REFERENCES user_lists(id) ON DELETE CASCADE,
           provider_id TEXT NOT NULL,
           content_type TEXT NOT NULL CHECK (content_type IN ('live','movie','series')),
           content_id TEXT NOT NULL, position INTEGER NOT NULL, added_at INTEGER NOT NULL,
           PRIMARY KEY (list_id, content_type, content_id, provider_id))",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO user_lists (id, name, sort_order, created_at, updated_at)
         SELECT id, name, sort_order, created_at, updated_at FROM _m39_old_user_lists",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO user_list_items (list_id, provider_id, content_type, content_id, position, added_at)
         SELECT li.list_id, ul.provider_id, li.content_type, li.content_id, li.position, li.added_at
         FROM _m39_old_user_list_items li
         JOIN _m39_old_user_lists ul ON ul.id = li.list_id",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("DROP TABLE _m39_old_user_list_items").execute(&mut *tx).await?;
    sqlx::query("DROP TABLE _m39_old_user_lists").execute(&mut *tx).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_user_lists_order ON user_lists(sort_order)")
        .execute(&mut *tx)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_user_list_items_list ON user_list_items(list_id, position)")
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// Milestone 21: earlier builds persisted the full Xtream stream URL with the
/// provider password embedded in cleartext (`…/movie/<user>/<password>/<id>.ext`).
/// The playable URL is now composed at playback time from the keychain secret, so
/// scrub any such URLs already on disk. Idempotent (the `<> ''` guard makes it a
/// no-op once cleared) and scoped to Xtream rows — M3U URLs are provider-supplied
/// and carry no app-injected secret, so they are preserved.
async fn scrub_xtream_stream_urls(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for table in ["movies", "episodes", "live_channels"] {
        sqlx::query(&format!(
            "UPDATE {table} SET stream_url = ''
             WHERE stream_url <> ''
               AND provider_id IN (SELECT id FROM providers WHERE type = 'xtream')"
        ))
        .execute(pool)
        .await?;
    }
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
