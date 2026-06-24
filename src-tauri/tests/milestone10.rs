//! Milestone 10 acceptance tests: the Home "Keep Watching" backend join
//! (spec §5.10 / §16 `get_continue_watching`). In-progress movies and episodes
//! are returned most-recently-watched first, completed items are excluded,
//! episodes carry their parent series, and progress for items no longer in the
//! catalog is dropped by the join. The floating nav and the Popular rows (which
//! reuse existing catalog commands) are verified in the browser preview.

use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, ContinueWatchingItem, EpisodeItem, MovieItem, Provider, ProviderInput,
    ProviderType, SeriesItem,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_db(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m10-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

fn movie(id: &str, name: &str) -> MovieItem {
    MovieItem {
        id: id.into(),
        name: name.into(),
        category_id: "cat".into(),
        category_name: "Cat".into(),
        poster_url: Some(format!("http://poster.example/{id}.jpg")),
        stream_url: format!("http://stream.example/{id}.mp4"),
        container_ext: "mp4".into(),
        release_year: Some(2020),
        rating: None,
        added_at: None,
    }
}

fn series(id: &str, name: &str) -> SeriesItem {
    SeriesItem {
        id: id.into(),
        name: name.into(),
        category_id: "cat".into(),
        category_name: "Cat".into(),
        poster_url: Some(format!("http://poster.example/{id}.jpg")),
        release_year: Some(2019),
    }
}

fn episode(id: &str, series_id: &str, season: i64, ep: i64) -> EpisodeItem {
    EpisodeItem {
        id: id.into(),
        series_id: series_id.into(),
        season,
        episode: ep,
        title: format!("Episode {ep}"),
        stream_url: format!("http://stream.example/{id}.mkv"),
        container_ext: "mkv".into(),
        duration_seconds: Some(1500),
        poster_url: None, // forces series-poster fallback in the UI
        overview: None,
    }
}

async fn seed(pool: &SqlitePool, data: &CatalogData) -> Provider {
    let provider = upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M10".into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://example.com/p.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .expect("provider");
    db::catalog::replace_catalog(pool, &provider.id, data, 1_000_000)
        .await
        .expect("seed catalog");
    provider
}

#[tokio::test]
async fn continue_watching_orders_by_recency_excludes_completed_and_joins_series() {
    let path = temp_db("keepwatching");
    let pool = db::init(&path).await.expect("init");

    let data = CatalogData {
        movies: vec![movie("m1", "Alpha"), movie("m2", "Bravo")],
        series: vec![series("s1", "Sample Show")],
        episodes: vec![episode("e1", "s1", 1, 2)],
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;
    let pid = &provider.id;

    // m1 in-progress @100, m2 completed @300 (excluded), e1 in-progress @200,
    // plus a ghost movie progress whose catalog row doesn't exist (dropped).
    db::watch::upsert(&pool, pid, "movie", "m1", 600, Some(5400), false, 100)
        .await
        .unwrap();
    db::watch::upsert(&pool, pid, "movie", "m2", 5300, Some(5400), true, 300)
        .await
        .unwrap();
    db::watch::upsert(&pool, pid, "episode", "e1", 700, Some(1500), false, 200)
        .await
        .unwrap();
    db::watch::upsert(&pool, pid, "movie", "ghost", 10, Some(100), false, 250)
        .await
        .unwrap();

    let items = db::watch::continue_watching(&pool, pid, 20).await.unwrap();

    // Most-recent first: e1 (@200) then m1 (@100). m2 (completed) and the ghost
    // movie (no catalog row) are both absent.
    assert_eq!(items.len(), 2, "completed + ghost rows excluded");

    match &items[0] {
        ContinueWatchingItem::Episode {
            episode,
            series,
            progress,
        } => {
            assert_eq!(episode.id, "e1");
            assert_eq!(progress.position_seconds, 700);
            assert!(!progress.completed);
            let series = series.as_ref().expect("parent series joined");
            assert_eq!(series.id, "s1");
            assert_eq!(series.name, "Sample Show");
        }
        other => panic!("expected episode first, got {other:?}"),
    }

    match &items[1] {
        ContinueWatchingItem::Movie { movie, progress } => {
            assert_eq!(movie.id, "m1");
            assert_eq!(movie.name, "Alpha");
            assert_eq!(progress.position_seconds, 600);
        }
        other => panic!("expected movie second, got {other:?}"),
    }

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn continue_watching_is_empty_without_history_and_respects_limit() {
    let path = temp_db("empty");
    let pool = db::init(&path).await.expect("init");

    let data = CatalogData {
        movies: vec![movie("m1", "Alpha"), movie("m2", "Bravo"), movie("m3", "Cosmo")],
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;
    let pid = &provider.id;

    // No watch history yet → empty.
    let empty = db::watch::continue_watching(&pool, pid, 20).await.unwrap();
    assert!(empty.is_empty());

    // Three in-progress movies, but a limit of 2 returns only the two newest.
    db::watch::upsert(&pool, pid, "movie", "m1", 60, Some(5400), false, 100)
        .await
        .unwrap();
    db::watch::upsert(&pool, pid, "movie", "m2", 60, Some(5400), false, 200)
        .await
        .unwrap();
    db::watch::upsert(&pool, pid, "movie", "m3", 60, Some(5400), false, 300)
        .await
        .unwrap();

    let limited = db::watch::continue_watching(&pool, pid, 2).await.unwrap();
    assert_eq!(limited.len(), 2);
    match (&limited[0], &limited[1]) {
        (
            ContinueWatchingItem::Movie { movie: a, .. },
            ContinueWatchingItem::Movie { movie: b, .. },
        ) => {
            assert_eq!(a.id, "m3", "newest first");
            assert_eq!(b.id, "m2");
        }
        _ => panic!("expected two movies"),
    }

    pool.close().await;
    cleanup_db(&path);
}
