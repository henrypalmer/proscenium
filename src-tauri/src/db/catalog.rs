//! Catalog persistence: atomic full-catalog replacement and FTS5 sync.

use crate::models::{
    CatalogData, CatalogSummary, Category, EpisodeItem, LiveChannel, MovieItem, PaginatedResult,
    SearchContentType, SearchResults, SeriesItem,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool, Transaction};

pub const MAX_PAGE_SIZE: i64 = 500;

/// Rows per INSERT statement. SQLite's bind limit is 32k variables; the
/// widest table (movies) has 14 columns, so 400 rows stays well under it.
const CHUNK: usize = 400;

/// Replace the provider's entire cached catalog in one transaction and stamp
/// `last_refreshed`. On any error the transaction rolls back, leaving the
/// previous (possibly stale) catalog intact — spec §5.2 failure behavior.
pub async fn replace_catalog(
    pool: &SqlitePool,
    provider_id: &str,
    data: &CatalogData,
    refreshed_at: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Preserve the episode rows that back an in-progress "Keep Watching" item
    // (spec §5.9/§5.10). A full refresh re-fetches series but *not* their
    // episodes (those are fetched on demand per series), and deleting the series
    // rows below cascade-wipes `episodes` — which would drop the in-progress
    // episode from Keep Watching and break resume (its `stream_url` lives in
    // that row). Snapshot the watch-progress-referenced episodes whose series
    // still exists in the incoming catalog, then re-insert them after the series
    // rows are restored. Fresh `data.episodes` (M3U) overwrites these on insert.
    let new_series_ids: std::collections::HashSet<&str> =
        data.series.iter().map(|s| s.id.as_str()).collect();
    let preserved_episodes: Vec<EpisodeItem> = sqlx::query(
        "SELECT e.* FROM episodes e
         JOIN watch_progress wp
           ON wp.provider_id = e.provider_id
          AND wp.content_type = 'episode'
          AND wp.content_id = e.id
         WHERE e.provider_id = ?",
    )
    .bind(provider_id)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(row_to_episode)
    .filter(|e| new_series_ids.contains(e.series_id.as_str()))
    .collect();

    for table in [
        "episodes",
        "series",
        "series_categories",
        "movies",
        "vod_categories",
        "live_channels",
        "live_categories",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE provider_id = ?"))
            .bind(provider_id)
            .execute(&mut *tx)
            .await?;
    }

    insert_categories(&mut tx, "live_categories", provider_id, &data.live_categories).await?;
    insert_categories(&mut tx, "vod_categories", provider_id, &data.vod_categories).await?;
    insert_categories(&mut tx, "series_categories", provider_id, &data.series_categories).await?;

    for chunk in data.live_channels.chunks(CHUNK) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT OR REPLACE INTO live_channels
             (id, provider_id, name, category_id, category_name, logo_url, stream_url, stream_ext, epg_channel_id) ",
        );
        qb.push_values(chunk, |mut b, c| {
            b.push_bind(&c.id)
                .push_bind(provider_id)
                .push_bind(&c.name)
                .push_bind(&c.category_id)
                .push_bind(&c.category_name)
                .push_bind(&c.logo_url)
                .push_bind(&c.stream_url)
                .push_bind(&c.stream_ext)
                .push_bind(&c.epg_channel_id);
        });
        qb.build().execute(&mut *tx).await?;
    }

    for chunk in data.movies.chunks(CHUNK) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT OR REPLACE INTO movies
             (id, provider_id, name, category_id, category_name, poster_url, stream_url, container_ext, release_year, rating, imdb_id, imdb_rating, added_at) ",
        );
        qb.push_values(chunk, |mut b, m| {
            b.push_bind(&m.id)
                .push_bind(provider_id)
                .push_bind(&m.name)
                .push_bind(&m.category_id)
                .push_bind(&m.category_name)
                .push_bind(&m.poster_url)
                .push_bind(&m.stream_url)
                .push_bind(&m.container_ext)
                .push_bind(m.release_year)
                .push_bind(&m.rating)
                .push_bind(None::<String>)
                .push_bind(None::<f64>)
                .push_bind(m.added_at);
        });
        qb.build().execute(&mut *tx).await?;
    }

    for chunk in data.series.chunks(CHUNK) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT OR REPLACE INTO series
             (id, provider_id, name, category_id, category_name, poster_url, release_year, imdb_id, imdb_rating) ",
        );
        qb.push_values(chunk, |mut b, s| {
            b.push_bind(&s.id)
                .push_bind(provider_id)
                .push_bind(&s.name)
                .push_bind(&s.category_id)
                .push_bind(&s.category_name)
                .push_bind(&s.poster_url)
                .push_bind(s.release_year)
                .push_bind(None::<String>)
                .push_bind(None::<f64>);
        });
        qb.build().execute(&mut *tx).await?;
    }

    // Restore the preserved in-progress episodes first, then the fresh catalog
    // episodes (M3U supplies these inline; Xtream's are on-demand so the slice
    // is empty) so a fresh row always wins on an id conflict.
    insert_episodes(&mut tx, provider_id, &preserved_episodes).await?;
    insert_episodes(&mut tx, provider_id, &data.episodes).await?;

    // Re-index FTS from the content tables (spec: FTS5 populated on refresh).
    for fts in ["fts_live_channels", "fts_movies", "fts_series"] {
        sqlx::query(&format!("INSERT INTO {fts}({fts}) VALUES('rebuild')"))
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("UPDATE providers SET last_refreshed = ? WHERE id = ?")
        .bind(refreshed_at)
        .bind(provider_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}

/// Bulk-insert episode rows in chunks. Shared by full-catalog replacement and
/// per-series replacement. `INSERT OR REPLACE` means a later call overwrites
/// earlier rows that share an `(id, provider_id)`.
async fn insert_episodes(
    tx: &mut Transaction<'_, Sqlite>,
    provider_id: &str,
    episodes: &[EpisodeItem],
) -> Result<(), sqlx::Error> {
    for chunk in episodes.chunks(CHUNK) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT OR REPLACE INTO episodes
             (id, provider_id, series_id, season, episode, title, stream_url, container_ext, duration_seconds, poster_url, overview) ",
        );
        qb.push_values(chunk, |mut b, e| {
            b.push_bind(&e.id)
                .push_bind(provider_id)
                .push_bind(&e.series_id)
                .push_bind(e.season)
                .push_bind(e.episode)
                .push_bind(&e.title)
                .push_bind(&e.stream_url)
                .push_bind(&e.container_ext)
                .push_bind(e.duration_seconds)
                .push_bind(&e.poster_url)
                .push_bind(&e.overview);
        });
        qb.build().execute(&mut **tx).await?;
    }
    Ok(())
}

async fn insert_categories(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    provider_id: &str,
    categories: &[crate::models::Category],
) -> Result<(), sqlx::Error> {
    for chunk in categories.chunks(CHUNK) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(format!(
            "INSERT OR REPLACE INTO {table} (id, provider_id, name, sort_order) "
        ));
        qb.push_values(chunk, |mut b, c| {
            b.push_bind(&c.id)
                .push_bind(provider_id)
                .push_bind(&c.name)
                .push_bind(c.sort_order);
        });
        qb.build().execute(&mut **tx).await?;
    }
    Ok(())
}

/// Live categories in provider-defined order. Categories without any
/// channel are hidden (spec §12: empty categories are not shown).
pub async fn live_categories(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Vec<Category>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT c.id, c.name, c.sort_order FROM live_categories c
         WHERE c.provider_id = ?
           AND EXISTS (SELECT 1 FROM live_channels ch
                       WHERE ch.provider_id = c.provider_id AND ch.category_id = c.id)
         ORDER BY c.sort_order, c.name COLLATE NOCASE",
    )
    .bind(provider_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| Category {
            id: r.get("id"),
            name: r.get("name"),
            sort_order: r.get("sort_order"),
        })
        .collect())
}

pub(crate) fn row_to_live_channel(row: &SqliteRow) -> LiveChannel {
    LiveChannel {
        id: row.get("id"),
        name: row.get("name"),
        category_id: row.get("category_id"),
        category_name: row.get("category_name"),
        logo_url: row.get("logo_url"),
        stream_url: row.get("stream_url"),
        stream_ext: row.get("stream_ext"),
        epg_channel_id: row.get("epg_channel_id"),
    }
}

/// One page of channels, alphabetical (case-insensitive), optionally
/// filtered to a category. `page` is 1-based; out-of-range pages return an
/// empty `items` with the correct `total`.
/// Escape the SQL `LIKE` metacharacters (`%`, `_`, and the `\` escape char
/// itself) so a user's filter text matches literally. Paired with
/// `ESCAPE '\'` in the query.
fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

pub async fn live_channels_page(
    pool: &SqlitePool,
    provider_id: &str,
    category_id: Option<&str>,
    query: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<LiveChannel>, sqlx::Error> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);

    // In-section channel filter (spec §5.3): a case-insensitive name
    // substring match, applied across the whole category so the result is not
    // limited to the loaded virtualization window. Blank/whitespace = no
    // filter. SQLite's `LIKE` is case-insensitive for ASCII by default.
    let name_filter = query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{}%", escape_like(q)));

    let mut where_sql = String::from("provider_id = ?");
    if category_id.is_some() {
        where_sql.push_str(" AND category_id = ?");
    }
    if name_filter.is_some() {
        where_sql.push_str(r" AND name LIKE ? ESCAPE '\'");
    }

    let count_sql = format!("SELECT COUNT(*) FROM live_channels WHERE {where_sql}");
    let items_sql = format!(
        "SELECT * FROM live_channels WHERE {where_sql}
         ORDER BY name COLLATE NOCASE, id LIMIT ? OFFSET ?"
    );

    let mut count_query = sqlx::query(&count_sql).bind(provider_id);
    let mut items_query = sqlx::query(&items_sql).bind(provider_id);
    if let Some(cat) = category_id {
        count_query = count_query.bind(cat);
        items_query = items_query.bind(cat);
    }
    if let Some(filter) = &name_filter {
        count_query = count_query.bind(filter.clone());
        items_query = items_query.bind(filter.clone());
    }

    let total: i64 = count_query.fetch_one(pool).await?.get(0);
    let rows = items_query
        .bind(page_size)
        .bind((page - 1) * page_size)
        .fetch_all(pool)
        .await?;

    Ok(PaginatedResult {
        items: rows.iter().map(row_to_live_channel).collect(),
        total,
        page,
        page_size,
    })
}

pub(crate) fn row_to_movie(row: &SqliteRow) -> MovieItem {
    MovieItem {
        id: row.get("id"),
        name: row.get("name"),
        category_id: row.get("category_id"),
        category_name: row.get("category_name"),
        poster_url: row.get("poster_url"),
        stream_url: row.get("stream_url"),
        container_ext: row.get("container_ext"),
        release_year: row.get("release_year"),
        rating: row.get("rating"),
        added_at: row.get("added_at"),
    }
}

pub(crate) fn row_to_series(row: &SqliteRow) -> SeriesItem {
    SeriesItem {
        id: row.get("id"),
        name: row.get("name"),
        category_id: row.get("category_id"),
        category_name: row.get("category_name"),
        poster_url: row.get("poster_url"),
        release_year: row.get("release_year"),
    }
}

pub(crate) fn row_to_episode(row: &SqliteRow) -> EpisodeItem {
    EpisodeItem {
        id: row.get("id"),
        series_id: row.get("series_id"),
        season: row.get("season"),
        episode: row.get("episode"),
        title: row.get("title"),
        stream_url: row.get("stream_url"),
        container_ext: row.get("container_ext"),
        duration_seconds: row.get("duration_seconds"),
        poster_url: row.get("poster_url"),
        overview: row.get("overview"),
    }
}

/// Shared pagination plumbing for the three catalog tables: count + one
/// alphabetical (case-insensitive) page, optionally filtered by category.
async fn catalog_page<T>(
    pool: &SqlitePool,
    table: &str,
    provider_id: &str,
    category_id: Option<&str>,
    page: i64,
    page_size: i64,
    map: fn(&SqliteRow) -> T,
) -> Result<PaginatedResult<T>, sqlx::Error> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);

    let category_filter = if category_id.is_some() { " AND category_id = ?" } else { "" };
    let count_sql = format!("SELECT COUNT(*) FROM {table} WHERE provider_id = ?{category_filter}");
    let items_sql = format!(
        "SELECT * FROM {table} WHERE provider_id = ?{category_filter}
         ORDER BY name COLLATE NOCASE, id LIMIT ? OFFSET ?"
    );

    let mut count_query = sqlx::query(&count_sql).bind(provider_id);
    let mut items_query = sqlx::query(&items_sql).bind(provider_id);
    if let Some(category) = category_id {
        count_query = count_query.bind(category);
        items_query = items_query.bind(category);
    }

    let total: i64 = count_query.fetch_one(pool).await?.get(0);
    let rows = items_query
        .bind(page_size)
        .bind((page - 1) * page_size)
        .fetch_all(pool)
        .await?;

    Ok(PaginatedResult {
        items: rows.iter().map(map).collect(),
        total,
        page,
        page_size,
    })
}

pub async fn movies_page(
    pool: &SqlitePool,
    provider_id: &str,
    category_id: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<MovieItem>, sqlx::Error> {
    catalog_page(pool, "movies", provider_id, category_id, page, page_size, row_to_movie).await
}

pub async fn series_page(
    pool: &SqlitePool,
    provider_id: &str,
    category_id: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<SeriesItem>, sqlx::Error> {
    catalog_page(pool, "series", provider_id, category_id, page, page_size, row_to_series).await
}

/// Categories in provider order, hiding ones with no content (spec §12).
async fn non_empty_categories(
    pool: &SqlitePool,
    category_table: &str,
    content_table: &str,
    provider_id: &str,
) -> Result<Vec<Category>, sqlx::Error> {
    let sql = format!(
        "SELECT c.id, c.name, c.sort_order FROM {category_table} c
         WHERE c.provider_id = ?
           AND EXISTS (SELECT 1 FROM {content_table} x
                       WHERE x.provider_id = c.provider_id AND x.category_id = c.id)
         ORDER BY c.sort_order, c.name COLLATE NOCASE"
    );
    let rows = sqlx::query(&sql).bind(provider_id).fetch_all(pool).await?;
    Ok(rows
        .iter()
        .map(|r| Category {
            id: r.get("id"),
            name: r.get("name"),
            sort_order: r.get("sort_order"),
        })
        .collect())
}

pub async fn vod_categories(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Vec<Category>, sqlx::Error> {
    non_empty_categories(pool, "vod_categories", "movies", provider_id).await
}

pub async fn series_categories(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Vec<Category>, sqlx::Error> {
    non_empty_categories(pool, "series_categories", "series", provider_id).await
}

pub async fn movie_by_id(
    pool: &SqlitePool,
    provider_id: &str,
    movie_id: &str,
) -> Result<Option<MovieItem>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM movies WHERE provider_id = ? AND id = ?")
        .bind(provider_id)
        .bind(movie_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_movie))
}

pub async fn series_by_id(
    pool: &SqlitePool,
    provider_id: &str,
    series_id: &str,
) -> Result<Option<SeriesItem>, sqlx::Error> {
    let row = sqlx::query("SELECT * FROM series WHERE provider_id = ? AND id = ?")
        .bind(provider_id)
        .bind(series_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_series))
}

pub async fn episodes_for_series(
    pool: &SqlitePool,
    provider_id: &str,
    series_id: &str,
) -> Result<Vec<EpisodeItem>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT * FROM episodes WHERE provider_id = ? AND series_id = ?
         ORDER BY season, episode",
    )
    .bind(provider_id)
    .bind(series_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_episode).collect())
}

/// Replace one series' episodes (used by the on-demand Xtream fetch).
pub async fn replace_series_episodes(
    pool: &SqlitePool,
    provider_id: &str,
    series_id: &str,
    episodes: &[EpisodeItem],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM episodes WHERE provider_id = ? AND series_id = ?")
        .bind(provider_id)
        .bind(series_id)
        .execute(&mut *tx)
        .await?;
    insert_episodes(&mut tx, provider_id, episodes).await?;
    tx.commit().await
}

/// Build an FTS5 MATCH expression that prefix-matches every whitespace
/// token against the name and category_name columns (so neither the id nor
/// provider_id columns can produce accidental hits). Returns None when the
/// query has no usable tokens — the caller short-circuits to empty results
/// instead of handing FTS an invalid expression.
fn fts_match_expr(query: &str) -> Option<String> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if tokens.is_empty() {
        return None;
    }
    Some(format!("{{name category_name}} : ({})", tokens.join(" ")))
}

/// FTS5 search over one content table: best match first, ties alphabetical.
async fn search_table<T>(
    pool: &SqlitePool,
    fts_table: &str,
    content_table: &str,
    provider_id: &str,
    category_id: Option<&str>,
    match_expr: &str,
    limit: i64,
    map: fn(&SqliteRow) -> T,
) -> Result<Vec<T>, sqlx::Error> {
    let category_filter = if category_id.is_some() { " AND c.category_id = ?" } else { "" };
    let sql = format!(
        "SELECT c.* FROM {fts_table}
         JOIN {content_table} c ON c.rowid = {fts_table}.rowid
         WHERE {fts_table} MATCH ? AND c.provider_id = ?{category_filter}
         ORDER BY rank, c.name COLLATE NOCASE, c.id LIMIT ?"
    );
    let mut query = sqlx::query(&sql).bind(match_expr).bind(provider_id);
    if let Some(category) = category_id {
        query = query.bind(category);
    }
    let rows = query.bind(limit).fetch_all(pool).await?;
    Ok(rows.iter().map(map).collect())
}

/// Local full-text search across the cached catalog (spec §5.5 / §16).
/// Queries only SQLite — no network — and groups results by content type,
/// optionally narrowed to one type and/or one category.
pub async fn search_catalog(
    pool: &SqlitePool,
    provider_id: &str,
    query: &str,
    content_type: SearchContentType,
    category_id: Option<&str>,
    limit: i64,
) -> Result<SearchResults, sqlx::Error> {
    let mut results = SearchResults::default();
    let Some(expr) = fts_match_expr(query) else {
        return Ok(results);
    };
    let limit = limit.clamp(1, MAX_PAGE_SIZE);

    use SearchContentType::*;
    if matches!(content_type, All | Live) {
        results.live_channels = search_table(
            pool,
            "fts_live_channels",
            "live_channels",
            provider_id,
            category_id,
            &expr,
            limit,
            row_to_live_channel,
        )
        .await?;
    }
    if matches!(content_type, All | Movies) {
        results.movies = search_table(
            pool,
            "fts_movies",
            "movies",
            provider_id,
            category_id,
            &expr,
            limit,
            row_to_movie,
        )
        .await?;
    }
    if matches!(content_type, All | Series) {
        results.series = search_table(
            pool,
            "fts_series",
            "series",
            provider_id,
            category_id,
            &expr,
            limit,
            row_to_series,
        )
        .await?;
    }
    Ok(results)
}

pub async fn summary(pool: &SqlitePool, provider_id: &str) -> Result<CatalogSummary, sqlx::Error> {
    let count = |sql: &'static str| {
        let pool = pool.clone();
        let provider_id = provider_id.to_string();
        async move {
            let row = sqlx::query(sql).bind(provider_id).fetch_one(&pool).await?;
            Ok::<i64, sqlx::Error>(row.get::<i64, _>(0))
        }
    };
    Ok(CatalogSummary {
        live_channels: count("SELECT COUNT(*) FROM live_channels WHERE provider_id = ?").await?,
        movies: count("SELECT COUNT(*) FROM movies WHERE provider_id = ?").await?,
        series: count("SELECT COUNT(*) FROM series WHERE provider_id = ?").await?,
    })
}
