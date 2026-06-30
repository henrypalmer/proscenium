//! Milestone 39 acceptance tests: multiple active providers (merged catalog).
//! Merged reads across the enabled provider set with per-item provider tagging,
//! name-merged categories, merged search, the enabled-provider settings + the
//! pre-M39 migration from the legacy single active provider, and merged
//! continue-watching.

use proscenium_lib::commands::catalog::{
    get_enabled_provider_ids, set_enabled_provider_ids, ACTIVE_PROVIDER_KEY,
};
use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::commands::watch::set_watch_progress_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, MovieItem, Provider, ProviderInput, ProviderType, SearchContentType,
};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m39-{tag}-{}.db", uuid::Uuid::new_v4()))
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
            playlist_url: Some("http://example.local/p.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

fn movie(id: &str, name: &str, genre: &str) -> MovieItem {
    MovieItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        // Real catalogs denormalize the genre name onto the row; id == name keeps
        // the merged-by-name category filter consistent.
        category_id: genre.into(),
        category_name: genre.into(),
        poster_url: None,
        stream_url: String::new(),
        container_ext: "mp4".into(),
        release_year: Some(2000),
        rating: None,
        added_at: None,
    }
}

fn cat(genre: &str, order: i64) -> Category {
    Category { id: genre.into(), name: genre.into(), sort_order: order }
}

/// Seed one provider's movie catalog via the real persistence path (also builds
/// the FTS index, so search works).
async fn seed_movies(pool: &SqlitePool, provider_id: &str, cats: Vec<Category>, movies: Vec<MovieItem>) {
    let data = CatalogData { vod_categories: cats, movies, ..Default::default() };
    db::catalog::replace_catalog(pool, provider_id, &data, 1_700_000_000)
        .await
        .expect("seed");
}

#[tokio::test]
async fn merged_reads_tag_items_by_provider_and_dedupe_categories_by_name() {
    let path = temp_path("merge");
    let pool = db::init(&path).await.expect("init");
    let p1 = make_provider(&pool, "P1").await;
    let p2 = make_provider(&pool, "P2").await;

    // Shared genre "Action" on both providers; "Drama" only on p2.
    seed_movies(&pool, &p1.id, vec![cat("Action", 0)], vec![movie("a", "Alpha", "Action")]).await;
    seed_movies(
        &pool,
        &p2.id,
        vec![cat("Action", 0), cat("Drama", 1)],
        vec![movie("b", "Bravo", "Action"), movie("c", "Charlie", "Drama")],
    )
    .await;

    let ids = vec![p1.id.clone(), p2.id.clone()];

    // Categories merge by name (one "Action", not two) and hide empties.
    let cats = db::catalog::vod_categories(&pool, &ids).await.unwrap();
    let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Action", "Drama"]);

    // Movies merged across providers, alphabetical, each tagged with its provider.
    let page = db::catalog::movies_page(&pool, &ids, None, 1, 50).await.unwrap();
    assert_eq!(page.total, 3);
    let names: Vec<&str> = page.items.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Bravo", "Charlie"]);
    let providers: HashSet<&str> = page.items.iter().map(|m| m.provider_id.as_str()).collect();
    assert!(providers.contains(&p1.id.as_str()) && providers.contains(&p2.id.as_str()));

    // Filtering by the shared category name spans both providers.
    let action = db::catalog::movies_page(&pool, &ids, Some("Action"), 1, 50).await.unwrap();
    assert_eq!(action.total, 2);
    let action_providers: HashSet<&str> =
        action.items.iter().map(|m| m.provider_id.as_str()).collect();
    assert_eq!(action_providers.len(), 2, "Action spans both providers");

    // Summary sums over the set; a single-provider read is the subset.
    assert_eq!(db::catalog::summary(&pool, &ids).await.unwrap().movies, 3);
    assert_eq!(db::catalog::summary(&pool, &p1.id).await.unwrap().movies, 1);

    // Empty set → empty results, no SQL error.
    let none: Vec<String> = vec![];
    assert_eq!(db::catalog::movies_page(&pool, &none, None, 1, 50).await.unwrap().total, 0);
    assert!(db::catalog::vod_categories(&pool, &none).await.unwrap().is_empty());

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn search_merges_across_providers() {
    let path = temp_path("search");
    let pool = db::init(&path).await.expect("init");
    let p1 = make_provider(&pool, "P1").await;
    let p2 = make_provider(&pool, "P2").await;
    seed_movies(&pool, &p1.id, vec![cat("Action", 0)], vec![movie("a", "Matrix Reloaded", "Action")]).await;
    seed_movies(&pool, &p2.id, vec![cat("Action", 0)], vec![movie("b", "Matrix Revolutions", "Action")]).await;

    let ids = vec![p1.id.clone(), p2.id.clone()];
    let results = db::catalog::search_catalog(&pool, &ids, "matrix", SearchContentType::All, None, 20)
        .await
        .unwrap();
    assert_eq!(results.movies.len(), 2, "search spans both providers");
    let providers: HashSet<&str> = results.movies.iter().map(|m| m.provider_id.as_str()).collect();
    assert_eq!(providers.len(), 2);

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn enabled_provider_set_persists_and_migrates_from_active() {
    let path = temp_path("enabled");
    let pool = db::init(&path).await.expect("init");
    let p1 = make_provider(&pool, "P1").await;
    let p2 = make_provider(&pool, "P2").await;

    // Pre-M39: only the legacy active_provider_id is set → enabled falls back to it.
    db::settings::set(&pool, ACTIVE_PROVIDER_KEY, &p1.id).await.unwrap();
    assert_eq!(get_enabled_provider_ids(&pool).await.unwrap(), vec![p1.id.clone()]);

    // An explicit set is respected; the legacy "active" points at the first enabled.
    set_enabled_provider_ids(&pool, &[p2.id.clone(), p1.id.clone()]).await.unwrap();
    assert_eq!(
        get_enabled_provider_ids(&pool).await.unwrap(),
        vec![p2.id.clone(), p1.id.clone()]
    );
    assert_eq!(
        db::settings::get(&pool, ACTIVE_PROVIDER_KEY).await.unwrap(),
        Some(p2.id.clone())
    );

    // The empty set is respected literally (the user disabled every provider).
    set_enabled_provider_ids(&pool, &[]).await.unwrap();
    assert!(get_enabled_provider_ids(&pool).await.unwrap().is_empty());

    // Stale ids (a since-deleted provider) are filtered out at read.
    set_enabled_provider_ids(&pool, &[p1.id.clone(), "ghost".into()]).await.unwrap();
    assert_eq!(get_enabled_provider_ids(&pool).await.unwrap(), vec![p1.id.clone()]);

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn continue_watching_merges_across_providers() {
    let path = temp_path("cw");
    let pool = db::init(&path).await.expect("init");
    let p1 = make_provider(&pool, "P1").await;
    let p2 = make_provider(&pool, "P2").await;
    seed_movies(&pool, &p1.id, vec![cat("Action", 0)], vec![movie("a", "Alpha", "Action")]).await;
    seed_movies(&pool, &p2.id, vec![cat("Action", 0)], vec![movie("b", "Bravo", "Action")]).await;

    // Each provider has one in-progress movie.
    set_watch_progress_impl(&pool, &p1.id, "movie", "a", 100.0, Some(1000.0)).await.unwrap();
    set_watch_progress_impl(&pool, &p2.id, "movie", "b", 200.0, Some(1000.0)).await.unwrap();

    let ids = vec![p1.id.clone(), p2.id.clone()];
    let kw = db::watch::continue_watching(&pool, &ids, 20).await.unwrap();
    assert_eq!(kw.len(), 2, "Keep Watching merges across providers");

    // A single-provider read is the subset.
    assert_eq!(db::watch::continue_watching(&pool, &p1.id, 20).await.unwrap().len(), 1);

    pool.close().await;
    cleanup_db(&path);
}
