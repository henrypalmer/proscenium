//! Milestone 8 acceptance tests: watch progress (spec §5.9). Position save +
//! read survives reopen, completion at the ~95% threshold, bulk section
//! listing, live TV rejection, provider-cascade deletion, and clearing.

use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::commands::watch::{mark_watched_impl, set_watch_progress_impl};
use proscenium_lib::db;
use proscenium_lib::models::{Provider, ProviderInput, ProviderType};
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

    let movies = db::watch::list(&pool, &provider.id, "movie").await.unwrap();
    assert_eq!(movies.len(), 2);
    assert_eq!(movies.get("m1").unwrap().position_seconds, 100);
    assert!(movies.get("m2").unwrap().completed);
    assert!(!movies.contains_key("e1"));

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
