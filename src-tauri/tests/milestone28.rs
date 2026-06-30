//! Milestone 28 acceptance tests: the local "More like this" related-titles
//! command (spec §5.4 / §16). Covers same-category selection, self-exclusion,
//! the cap, provider scoping, content-type routing, and that an unknown item or
//! type degrades gracefully. Entirely offline — no provider request.

use proscenium_lib::commands::catalog::get_related_impl;
use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, MovieItem, Provider, ProviderInput, ProviderType, SeriesItem,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m28-{tag}-{}.db", uuid::Uuid::new_v4()))
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
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://example.local/playlist.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

fn movie(id: &str, name: &str, category: &str) -> MovieItem {
    MovieItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        poster_url: None,
        stream_url: String::new(),
        container_ext: "mp4".into(),
        release_year: Some(2020),
        rating: None,
        added_at: Some(1_700_000_000),
    }
}

fn series(id: &str, name: &str, category: &str) -> SeriesItem {
    SeriesItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        poster_url: None,
        release_year: Some(2019),
    }
}

fn category(id: &str, name: &str, order: i64) -> Category {
    Category {
        id: id.into(),
        name: name.into(),
        sort_order: order,
    }
}

#[tokio::test]
async fn related_movies_share_category_exclude_self_and_respect_scope() {
    let path = temp_path("movies");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M28 A").await;
    let other = make_provider(&pool, "M28 B").await;

    let data = CatalogData {
        vod_categories: vec![category("act", "Action", 0), category("dra", "Drama", 1)],
        movies: vec![
            movie("m1", "Alpha Strike", "act"),
            movie("m2", "Bravo Run", "act"),
            movie("m3", "Comet Fall", "act"),
            movie("m4", "Quiet River", "dra"),
        ],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    // A second provider with an Action movie that must never leak across scope.
    let other_data = CatalogData {
        vod_categories: vec![category("act", "Action", 0)],
        movies: vec![movie("x1", "Other Action", "act")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &other.id, &other_data, 1_700_000_000)
        .await
        .expect("seed other");

    let related = get_related_impl(&pool, &provider.id, "movie", "m1", Some(20))
        .await
        .expect("related");
    assert!(related.series.is_empty(), "movie request fills only movies");
    let ids: Vec<&str> = related.movies.iter().map(|m| m.id.as_str()).collect();
    // Same category (act), excluding m1 itself, and not the Drama m4 or the
    // other provider's x1.
    assert_eq!(ids, ["m2", "m3"]);
}

#[tokio::test]
async fn related_respects_the_limit() {
    let path = temp_path("cap");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M28 cap").await;

    let movies: Vec<MovieItem> = (0..10)
        .map(|i| movie(&format!("m{i}"), &format!("Movie {i}"), "act"))
        .collect();
    let data = CatalogData {
        vod_categories: vec![category("act", "Action", 0)],
        movies,
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let related = get_related_impl(&pool, &provider.id, "movie", "m0", Some(3))
        .await
        .expect("related");
    assert_eq!(related.movies.len(), 3, "cap honored");
    assert!(
        related.movies.iter().all(|m| m.id != "m0"),
        "self excluded under the cap"
    );
}

#[tokio::test]
async fn related_series_route_and_unknowns_degrade() {
    let path = temp_path("series");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M28 series").await;

    let data = CatalogData {
        series_categories: vec![category("cri", "Crime", 0), category("sci", "Sci-Fi", 1)],
        series: vec![
            series("s1", "Night Watch", "cri"),
            series("s2", "Dark Alley", "cri"),
            series("s3", "Star Drift", "sci"),
        ],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let related = get_related_impl(&pool, &provider.id, "series", "s1", Some(20))
        .await
        .expect("related");
    assert!(related.movies.is_empty(), "series request fills only series");
    let ids: Vec<&str> = related.series.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["s2"], "same category, excluding self and other genres");

    // An unknown content id yields an empty (not erroring) result.
    let none = get_related_impl(&pool, &provider.id, "series", "nope", Some(20))
        .await
        .expect("unknown id is empty");
    assert!(none.series.is_empty() && none.movies.is_empty());

    // An unsupported content type is rejected.
    assert!(
        get_related_impl(&pool, &provider.id, "live", "s1", Some(20))
            .await
            .is_err(),
        "live has no related row"
    );

    pool.close().await;
    cleanup_db(&path);
}
