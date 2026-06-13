//! Milestone 6 acceptance tests: FTS5-backed local search — grouping by
//! content type, prefix and case-insensitive matching, content-type and
//! category filters, the default/explicit result limit, empty and
//! special-character queries, provider scoping, and the guarantee that
//! search touches only SQLite (no network).

use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::commands::search::search_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, LiveChannel, MovieItem, Provider, ProviderInput, ProviderType,
    SearchContentType, SeriesItem,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m6-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn make_provider(pool: &SqlitePool, name: &str) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: name.into(),
            provider_type: ProviderType::M3u,
            // Unresolvable host: any accidental network use would error,
            // while a purely local search succeeds (spec §5.5).
            playlist_url: Some("http://unreachable.invalid/playlist.m3u".into()),
            server_url: None,
            username: None,
            password: None,
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

fn channel(id: &str, name: &str, category: &str) -> LiveChannel {
    LiveChannel {
        id: id.into(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        logo_url: None,
        stream_url: format!("http://stream.local/live/{id}.ts"),
        stream_ext: "ts".into(),
        epg_channel_id: None,
    }
}

fn movie(id: &str, name: &str, category: &str) -> MovieItem {
    MovieItem {
        id: id.into(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        poster_url: None,
        stream_url: format!("http://stream.local/movie/{id}.mp4"),
        container_ext: "mp4".into(),
        release_year: Some(2021),
        rating: None,
        added_at: None,
    }
}

fn series(id: &str, name: &str, category: &str) -> SeriesItem {
    SeriesItem {
        id: id.into(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        poster_url: None,
        release_year: Some(2018),
    }
}

fn category(id: &str, name: &str, order: i64) -> Category {
    Category {
        id: id.into(),
        name: name.into(),
        sort_order: order,
    }
}

/// Channels, movies, and series sharing the term "Falcon" so a single query
/// produces hits in every group.
fn seed_catalog() -> CatalogData {
    CatalogData {
        live_categories: vec![category("news", "News", 0), category("sport", "Sports", 1)],
        live_channels: vec![
            channel("c1", "Falcon News HD", "news"),
            channel("c2", "Falcon Sports", "sport"),
            channel("c3", "Orbit Weather", "news"),
        ],
        vod_categories: vec![category("act", "Action", 0), category("dra", "Drama", 1)],
        movies: vec![
            movie("m1", "The Falcon Heist", "act"),
            movie("m2", "falcon rising", "dra"),
            movie("m3", "Quiet River", "dra"),
        ],
        series_categories: vec![category("cri", "Crime", 0)],
        series: vec![
            series("s1", "Falcon Squad", "cri"),
            series("s2", "Night Watch", "cri"),
        ],
        ..Default::default()
    }
}

async fn search(
    pool: &SqlitePool,
    provider_id: &str,
    query: &str,
    content_type: Option<SearchContentType>,
    category_id: Option<&str>,
    limit: Option<i64>,
) -> proscenium_lib::models::SearchResults {
    search_impl(pool, provider_id, query, content_type, category_id, limit)
        .await
        .expect("search")
}

// --- Grouping, provider scoping, and ranking basics ---

#[tokio::test]
async fn results_group_by_content_type_and_stay_provider_scoped() {
    let path = temp_path("grouping");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M6 main").await;
    let other = make_provider(&pool, "M6 other").await;

    db::catalog::replace_catalog(&pool, &provider.id, &seed_catalog(), 1_700_000_000)
        .await
        .expect("seed");
    // A second provider with its own "Falcon" content that must not leak in.
    let other_data = CatalogData {
        live_categories: vec![category("news", "News", 0)],
        live_channels: vec![channel("c1", "Falcon Other Provider", "news")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &other.id, &other_data, 1_700_000_000)
        .await
        .expect("seed other");

    let results = search(&pool, &provider.id, "falcon", None, None, None).await;

    // Results come back best-match (BM25) first, so compare as sets.
    let mut live: Vec<&str> = results.live_channels.iter().map(|c| c.name.as_str()).collect();
    live.sort();
    assert_eq!(live, ["Falcon News HD", "Falcon Sports"]);
    let mut movies: Vec<&str> = results.movies.iter().map(|m| m.name.as_str()).collect();
    movies.sort();
    assert_eq!(movies, ["The Falcon Heist", "falcon rising"]);
    let series: Vec<&str> = results.series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(series, ["Falcon Squad"]);

    // The other provider still finds its own channel — and only its own.
    let other_results = search(&pool, &other.id, "falcon", None, None, None).await;
    assert_eq!(other_results.live_channels.len(), 1);
    assert_eq!(other_results.live_channels[0].name, "Falcon Other Provider");
    assert!(other_results.movies.is_empty());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    delete_provider_impl(&pool, &other.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Prefix, case-insensitive, and multi-token matching ---

#[tokio::test]
async fn matching_is_prefix_based_case_insensitive_and_multi_token() {
    let path = temp_path("matching");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M6 match").await;
    db::catalog::replace_catalog(&pool, &provider.id, &seed_catalog(), 1_700_000_000)
        .await
        .expect("seed");

    // Prefix of a word.
    let results = search(&pool, &provider.id, "falc", None, None, None).await;
    assert_eq!(results.live_channels.len(), 2);
    assert_eq!(results.movies.len(), 2);
    assert_eq!(results.series.len(), 1);

    // Case-insensitive.
    let results = search(&pool, &provider.id, "FALCON", None, None, None).await;
    assert_eq!(results.live_channels.len(), 2);

    // Multi-token queries AND together: "falcon ri" → only "falcon rising".
    let results = search(&pool, &provider.id, "falcon ri", None, None, None).await;
    assert!(results.live_channels.is_empty());
    let movies: Vec<&str> = results.movies.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(movies, ["falcon rising"]);

    // Category names match too (spec FTS columns include category_name).
    let results = search(&pool, &provider.id, "sports", None, None, None).await;
    assert_eq!(results.live_channels.len(), 1);
    assert_eq!(results.live_channels[0].name, "Falcon Sports");

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Content type filter ---

#[tokio::test]
async fn content_type_filter_limits_results_to_one_group() {
    let path = temp_path("type-filter");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M6 type").await;
    db::catalog::replace_catalog(&pool, &provider.id, &seed_catalog(), 1_700_000_000)
        .await
        .expect("seed");

    let live = search(&pool, &provider.id, "falcon", Some(SearchContentType::Live), None, None).await;
    assert_eq!(live.live_channels.len(), 2);
    assert!(live.movies.is_empty());
    assert!(live.series.is_empty());

    let movies =
        search(&pool, &provider.id, "falcon", Some(SearchContentType::Movies), None, None).await;
    assert!(movies.live_channels.is_empty());
    assert_eq!(movies.movies.len(), 2);
    assert!(movies.series.is_empty());

    let series =
        search(&pool, &provider.id, "falcon", Some(SearchContentType::Series), None, None).await;
    assert!(series.live_channels.is_empty());
    assert!(series.movies.is_empty());
    assert_eq!(series.series.len(), 1);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Category filter (spec §5.5 genre narrowing) ---

#[tokio::test]
async fn category_filter_narrows_within_a_content_type() {
    let path = temp_path("category-filter");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M6 category").await;
    db::catalog::replace_catalog(&pool, &provider.id, &seed_catalog(), 1_700_000_000)
        .await
        .expect("seed");

    let drama = search(
        &pool,
        &provider.id,
        "falcon",
        Some(SearchContentType::Movies),
        Some("dra"),
        None,
    )
    .await;
    let names: Vec<&str> = drama.movies.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["falcon rising"]);

    let no_match = search(
        &pool,
        &provider.id,
        "quiet",
        Some(SearchContentType::Movies),
        Some("act"),
        None,
    )
    .await;
    assert!(no_match.movies.is_empty());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Result limit ---

#[tokio::test]
async fn limit_defaults_to_20_per_group_and_is_adjustable() {
    let path = temp_path("limit");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M6 limit").await;

    let data = CatalogData {
        vod_categories: vec![category("act", "Action", 0)],
        movies: (0..30)
            .map(|i| movie(&format!("m{i}"), &format!("Falcon Movie {i:02}"), "act"))
            .collect(),
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let default = search(&pool, &provider.id, "falcon", None, None, None).await;
    assert_eq!(default.movies.len(), 20);

    let five = search(&pool, &provider.id, "falcon", None, None, Some(5)).await;
    assert_eq!(five.movies.len(), 5);

    let all = search(&pool, &provider.id, "falcon", None, None, Some(100)).await;
    assert_eq!(all.movies.len(), 30);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Empty, no-match, and special-character queries ---

#[tokio::test]
async fn empty_no_match_and_special_character_queries_are_safe() {
    let path = temp_path("edge-queries");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M6 edge").await;
    db::catalog::replace_catalog(&pool, &provider.id, &seed_catalog(), 1_700_000_000)
        .await
        .expect("seed");

    // Blank and whitespace-only queries return empty groups, not errors.
    for q in ["", "   ", "\t"] {
        let results = search(&pool, &provider.id, q, None, None, None).await;
        assert!(results.live_channels.is_empty());
        assert!(results.movies.is_empty());
        assert!(results.series.is_empty());
    }

    // No match → empty groups (UI shows the friendly no-results state).
    let results = search(&pool, &provider.id, "zzzzzz", None, None, None).await;
    assert!(results.live_channels.is_empty() && results.movies.is_empty() && results.series.is_empty());

    // FTS5 operators and quotes in user input must not break the query.
    for q in [r#""falcon"#, "falcon AND OR NOT", "f*l(c)on", "user-input: <weird>"] {
        let _ = search(&pool, &provider.id, q, None, None, None).await; // must not panic/error
    }
    // Quoted input still matches after sanitizing.
    let results = search(&pool, &provider.id, r#""falcon""#, None, None, None).await;
    assert_eq!(results.live_channels.len(), 2);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Locality: search never leaves SQLite ---

#[tokio::test]
async fn search_is_served_entirely_from_the_local_cache() {
    let path = temp_path("local-only");
    let pool = db::init(&path).await.expect("init");
    // The provider's playlist URL points at an unresolvable host, so any
    // network attempt would fail loudly. Search succeeding proves it is
    // answered from the SQLite cache alone.
    let provider = make_provider(&pool, "M6 offline").await;
    db::catalog::replace_catalog(&pool, &provider.id, &seed_catalog(), 1_700_000_000)
        .await
        .expect("seed");

    let started = std::time::Instant::now();
    let results = search(&pool, &provider.id, "falcon", None, None, None).await;
    assert_eq!(results.live_channels.len(), 2);
    assert_eq!(results.movies.len(), 2);
    assert_eq!(results.series.len(), 1);
    // Far below any network timeout — and well within the 300ms UI budget.
    assert!(
        started.elapsed() < std::time::Duration::from_millis(300),
        "local search took {:?}",
        started.elapsed()
    );

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}
