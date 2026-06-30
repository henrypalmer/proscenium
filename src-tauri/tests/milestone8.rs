//! Milestone 8 acceptance tests: watch progress (spec §5.9). Position save +
//! read survives reopen, completion at the ~95% threshold, bulk section
//! listing, live TV rejection, provider-cascade deletion, and clearing.

use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::commands::watch::{mark_watched_impl, set_watch_progress_impl};
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, EpisodeItem, Provider, ProviderInput, ProviderType, SeriesItem,
};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m8-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn make_provider(pool: &SqlitePool) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M8 m3u".into(),
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

#[tokio::test]
async fn position_is_saved_read_and_survives_reopen() {
    let path = temp_path("persist");
    let provider_id;
    {
        let pool = db::init(&path).await.expect("init");
        let provider = make_provider(&pool).await;
        provider_id = provider.id.clone();

        set_watch_progress_impl(&pool, &provider.id, "movie", "m1", 615.4, Some(5400.0))
            .await
            .expect("save");

        let got = db::watch::get(&pool, &provider.id, "movie", "m1")
            .await
            .expect("get")
            .expect("present");
        assert_eq!(got.position_seconds, 615); // rounded
        assert_eq!(got.duration_seconds, Some(5400));
        assert!(!got.completed, "615/5400 is well under the threshold");
        pool.close().await;
    }

    // Reopen the same DB file: the row is still there (acceptance: survives restart).
    let pool = db::init(&path).await.expect("reopen");
    let got = db::watch::get(&pool, &provider_id, "movie", "m1")
        .await
        .expect("get")
        .expect("present after reopen");
    assert_eq!(got.position_seconds, 615);
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn mark_watched_forces_completion_even_without_duration() {
    // Milestone 13: Keep Watching "Mark as watched" sets the completion flag
    // regardless of whether the runtime is known — set_watch_progress can't.
    let path = temp_path("markwatched");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool).await;

    // Known duration: completed + position parked at the end.
    mark_watched_impl(&pool, &provider.id, "movie", "m1", Some(5400.0))
        .await
        .unwrap();
    let known = db::watch::get(&pool, &provider.id, "movie", "m1")
        .await
        .unwrap()
        .unwrap();
    assert!(known.completed);
    assert_eq!(known.duration_seconds, Some(5400));
    assert_eq!(known.position_seconds, 5400);

    // Unknown duration: still completed (the whole reason mark_watched exists).
    mark_watched_impl(&pool, &provider.id, "episode", "e1", None)
        .await
        .unwrap();
    let unknown = db::watch::get(&pool, &provider.id, "episode", "e1")
        .await
        .unwrap()
        .unwrap();
    assert!(unknown.completed, "mark_watched must complete even with no duration");
    assert_eq!(unknown.duration_seconds, None);

    // Live TV is still rejected.
    assert!(mark_watched_impl(&pool, &provider.id, "live", "c1", None)
        .await
        .is_err());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn completion_threshold_marks_watched() {
    let path = temp_path("complete");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool).await;

    // 94% → still in progress.
    set_watch_progress_impl(&pool, &provider.id, "episode", "e1", 940.0, Some(1000.0))
        .await
        .unwrap();
    let below = db::watch::get(&pool, &provider.id, "episode", "e1")
        .await
        .unwrap()
        .unwrap();
    assert!(!below.completed, "94% must not complete");

    // 96% → completed.
    set_watch_progress_impl(&pool, &provider.id, "episode", "e1", 960.0, Some(1000.0))
        .await
        .unwrap();
    let above = db::watch::get(&pool, &provider.id, "episode", "e1")
        .await
        .unwrap()
        .unwrap();
    assert!(above.completed, "96% must complete");

    // Unknown duration → never completes (can't compute a fraction).
    set_watch_progress_impl(&pool, &provider.id, "movie", "m2", 9999.0, None)
        .await
        .unwrap();
    let no_dur = db::watch::get(&pool, &provider.id, "movie", "m2")
        .await
        .unwrap()
        .unwrap();
    assert!(!no_dur.completed);
    assert_eq!(no_dur.duration_seconds, None);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn list_returns_section_keyed_by_content_id() {
    let path = temp_path("list");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool).await;

    set_watch_progress_impl(&pool, &provider.id, "movie", "m1", 100.0, Some(1000.0))
        .await
        .unwrap();
    set_watch_progress_impl(&pool, &provider.id, "movie", "m2", 990.0, Some(1000.0))
        .await
        .unwrap();
    // An episode in a different section must not leak into the movie list.
    set_watch_progress_impl(&pool, &provider.id, "episode", "e1", 50.0, Some(1000.0))
        .await
        .unwrap();

    // Milestone 39: progress is keyed by "<provider_id>:<content_id>".
    let movies = db::watch::list(&pool, &provider.id, "movie").await.unwrap();
    assert_eq!(movies.len(), 2);
    assert_eq!(movies.get(&format!("{}:m1", provider.id)).unwrap().position_seconds, 100);
    assert!(movies.get(&format!("{}:m2", provider.id)).unwrap().completed);
    assert!(!movies.contains_key(&format!("{}:e1", provider.id)));

    let episodes = db::watch::list(&pool, &provider.id, "episode").await.unwrap();
    assert_eq!(episodes.len(), 1);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn live_tv_is_never_tracked() {
    let path = temp_path("live");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool).await;

    let result = set_watch_progress_impl(&pool, &provider.id, "live", "c1", 30.0, None).await;
    assert!(result.is_err(), "live TV must be rejected for tracking");

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn refresh_preserves_in_progress_episode() {
    // A full catalog refresh re-fetches series but not their (on-demand)
    // episodes; deleting the series rows cascade-wipes `episodes`. The
    // in-progress episode backing a Keep Watching item (and its resumable
    // stream_url) must survive that refresh as long as its series still exists.
    let path = temp_path("refresh-keep");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool).await;

    let cat = Category { id: "30".into(), name: "Crime".into(), sort_order: 0 };
    let ser = SeriesItem {
        id: "s1".into(),
        provider_id: String::new(),
        name: "Night Watch".into(),
        category_id: "30".into(),
        category_name: "Crime".into(),
        poster_url: None,
        release_year: Some(2019),
    };
    let ep = EpisodeItem {
        id: "e1".into(),
        provider_id: String::new(),
        series_id: "s1".into(),
        season: 1,
        episode: 1,
        title: "Pilot".into(),
        stream_url: "http://stream.local/series/e1.mp4".into(),
        container_ext: "mp4".into(),
        duration_seconds: Some(1500),
        poster_url: None,
        overview: None,
    };

    // Initial catalog with the series + its episode cached (as if its detail was
    // opened once), then mark that episode in progress.
    let initial = CatalogData {
        series_categories: vec![cat.clone()],
        series: vec![ser.clone()],
        episodes: vec![ep],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &initial, 1).await.unwrap();
    set_watch_progress_impl(&pool, &provider.id, "episode", "e1", 300.0, Some(1500.0))
        .await
        .unwrap();

    // A full Xtream-style refresh: series re-fetched, episodes empty (on-demand).
    let refreshed = CatalogData {
        series_categories: vec![cat.clone()],
        series: vec![ser.clone()],
        episodes: vec![],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &refreshed, 2).await.unwrap();

    // Still in Keep Watching, and the stream_url is intact for resume.
    let kw = db::watch::continue_watching(&pool, &provider.id, 20).await.unwrap();
    assert_eq!(kw.len(), 1, "in-progress episode must survive a refresh");
    let stream: Option<String> = sqlx::query_scalar(
        "SELECT stream_url FROM episodes WHERE provider_id = ? AND id = 'e1'",
    )
    .bind(&provider.id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(stream.as_deref(), Some("http://stream.local/series/e1.mp4"));

    // Orphan-tolerance: if the series itself is dropped by a refresh, its
    // episode is not preserved and falls out of Keep Watching.
    db::catalog::replace_catalog(&pool, &provider.id, &CatalogData::default(), 3)
        .await
        .unwrap();
    let kw2 = db::watch::continue_watching(&pool, &provider.id, 20).await.unwrap();
    assert!(kw2.is_empty(), "episode whose series vanished is not preserved");

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn clearing_and_provider_delete_remove_rows() {
    let path = temp_path("cascade");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool).await;

    set_watch_progress_impl(&pool, &provider.id, "movie", "m1", 100.0, Some(1000.0))
        .await
        .unwrap();
    set_watch_progress_impl(&pool, &provider.id, "movie", "m2", 200.0, Some(1000.0))
        .await
        .unwrap();

    // clear removes only the targeted row.
    db::watch::clear(&pool, &provider.id, "movie", "m1")
        .await
        .unwrap();
    assert!(db::watch::get(&pool, &provider.id, "movie", "m1")
        .await
        .unwrap()
        .is_none());
    assert!(db::watch::get(&pool, &provider.id, "movie", "m2")
        .await
        .unwrap()
        .is_some());

    // Deleting the provider cascades to its remaining watch_progress rows.
    delete_provider_impl(&pool, &provider.id).await.unwrap();
    let remaining: i64 = sqlx::query("SELECT COUNT(*) AS n FROM watch_progress WHERE provider_id = ?")
        .bind(&provider.id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(remaining, 0, "watch_progress must cascade with the provider");

    pool.close().await;
    cleanup_db(&path);
}
