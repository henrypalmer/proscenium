//! Catalog persistence: atomic full-catalog replacement and FTS5 sync.

use crate::models::{CatalogData, CatalogSummary, Category, LiveChannel, PaginatedResult};
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

    for chunk in data.episodes.chunks(CHUNK) {
        let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT OR REPLACE INTO episodes
             (id, provider_id, series_id, season, episode, title, stream_url, container_ext, duration_seconds, poster_url) ",
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
                .push_bind(&e.poster_url);
        });
        qb.build().execute(&mut *tx).await?;
    }

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

fn row_to_live_channel(row: &SqliteRow) -> LiveChannel {
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
pub async fn live_channels_page(
    pool: &SqlitePool,
    provider_id: &str,
    category_id: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<PaginatedResult<LiveChannel>, sqlx::Error> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);

    let (count_sql, items_sql) = match category_id {
        Some(_) => (
            "SELECT COUNT(*) FROM live_channels WHERE provider_id = ? AND category_id = ?",
            "SELECT * FROM live_channels WHERE provider_id = ? AND category_id = ?
             ORDER BY name COLLATE NOCASE, id LIMIT ? OFFSET ?",
        ),
        None => (
            "SELECT COUNT(*) FROM live_channels WHERE provider_id = ?",
            "SELECT * FROM live_channels WHERE provider_id = ?
             ORDER BY name COLLATE NOCASE, id LIMIT ? OFFSET ?",
        ),
    };

    let mut count_query = sqlx::query(count_sql).bind(provider_id);
    let mut items_query = sqlx::query(items_sql).bind(provider_id);
    if let Some(cat) = category_id {
        count_query = count_query.bind(cat);
        items_query = items_query.bind(cat);
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
