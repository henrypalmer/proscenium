//! Milestone 42 acceptance tests. Slice 1 (ranking + dedup + remember-pick):
//! candidate ranking by resolution/cached/seeders/preference, exact-duplicate
//! dedup, addon cached/seeders parsing, and the per-title source preference.

use proscenium_lib::canonical::resolver::{dedupe, rank_candidates};
use proscenium_lib::canonical::stremio;
use proscenium_lib::db;
use proscenium_lib::models::StreamCandidate;
use serde_json::json;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m42-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), s));
    }
}

fn cand(
    source: &str,
    quality: Option<&str>,
    cached: bool,
    seeders: Option<i64>,
    needs_debrid: bool,
) -> StreamCandidate {
    StreamCandidate {
        source: source.into(),
        provider_id: None,
        content_type: "movie".into(),
        content_id: None,
        url: Some("u".into()),
        quality: quality.map(String::from),
        container: None,
        confidence: 0.9,
        needs_debrid,
        cached,
        seeders,
    }
}

#[test]
fn rank_orders_by_quality_cached_seeders_then_preference() {
    let mut v = vec![
        cand("A", Some("1080p"), false, Some(10), false),
        cand("B", Some("2160p"), false, None, false), // best resolution
        cand("C", Some("1080p"), true, None, false), // cached beats un-cached at 1080p
        cand("D", Some("1080p"), false, Some(50), false), // more seeders than A
        cand("torrent", Some("2160p"), false, Some(99), true), // needs_debrid → last
    ];
    rank_candidates(&mut v, None);
    let order: Vec<&str> = v.iter().map(|c| c.source.as_str()).collect();
    assert_eq!(order, vec!["B", "C", "D", "A", "torrent"]);

    // The remembered source floats to the very top, above raw resolution.
    rank_candidates(&mut v, Some("A"));
    assert_eq!(v[0].source, "A");
}

#[test]
fn dedupe_drops_exact_duplicates_only() {
    let mut v = vec![
        cand("A", Some("1080p"), false, None, false),
        cand("A", Some("1080p"), false, None, false), // identical → dropped
        cand("A", Some("720p"), false, None, false), // different quality → kept
    ];
    dedupe(&mut v);
    assert_eq!(v.len(), 2);
}

#[test]
fn parse_streams_reads_cached_and_seeders() {
    let body = json!({ "streams": [
        { "name": "AIOStreams\n2160p [TB⚡]", "title": "Movie 2160p\n👤 89 💾 35 GB",
          "url": "https://x/a.mkv" },
        { "name": "Torrentio\n1080p", "title": "Movie 1080p\n👤 12 ⚙️ RARBG",
          "infoHash": "deadbeef" }
    ]});
    let c = stremio::parse_streams(&body, "AIOStreams", "movie");
    let direct = c.iter().find(|x| !x.needs_debrid).expect("direct");
    assert!(direct.cached, "[TB⚡] → cached");
    assert_eq!(direct.seeders, Some(89));
    let torrent = c.iter().find(|x| x.needs_debrid).expect("infohash");
    assert!(!torrent.cached);
    assert_eq!(torrent.seeders, Some(12));
}

#[tokio::test]
async fn source_pref_roundtrips_and_is_kind_scoped() {
    let path = temp_path("pref");
    let pool = db::init(&path).await.expect("init");

    assert!(db::canonical::source_pref_get(&pool, "tt1", "movie")
        .await
        .unwrap()
        .is_none());
    db::canonical::source_pref_set(&pool, "tt1", "movie", "AIOStreams", 5)
        .await
        .unwrap();
    assert_eq!(
        db::canonical::source_pref_get(&pool, "tt1", "movie").await.unwrap().as_deref(),
        Some("AIOStreams")
    );
    // The latest pick overwrites.
    db::canonical::source_pref_set(&pool, "tt1", "movie", "Torrentio", 9)
        .await
        .unwrap();
    assert_eq!(
        db::canonical::source_pref_get(&pool, "tt1", "movie").await.unwrap().as_deref(),
        Some("Torrentio")
    );
    // Scoped by kind — a movie pref doesn't leak to series.
    assert!(db::canonical::source_pref_get(&pool, "tt1", "series")
        .await
        .unwrap()
        .is_none());
    pool.close().await;
    cleanup_db(&path);
}

// --- Slice 2: availability cache ---

#[tokio::test]
async fn availability_cache_roundtrips_and_is_kind_scoped() {
    let path = temp_path("avail");
    let pool = db::init(&path).await.expect("init");
    let ids = vec!["tt1".to_string(), "tt2".to_string()];

    assert!(db::canonical::availability_get_many(&pool, &ids, "movie")
        .await
        .unwrap()
        .is_empty());
    db::canonical::availability_put(&pool, "tt1", "movie", 3, Some("2160p"), 100)
        .await
        .unwrap();
    db::canonical::availability_put(&pool, "tt2", "movie", 0, None, 100)
        .await
        .unwrap();

    let map = db::canonical::availability_get_many(&pool, &ids, "movie").await.unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map["tt1"].source_count, 3);
    assert_eq!(map["tt1"].best_quality.as_deref(), Some("2160p"));
    assert_eq!(map["tt2"].source_count, 0);
    assert!(map["tt2"].best_quality.is_none());

    // Kind-scoped, and the latest write upserts.
    assert!(db::canonical::availability_get_many(&pool, &ids, "series")
        .await
        .unwrap()
        .is_empty());
    db::canonical::availability_put(&pool, "tt1", "movie", 5, Some("1080p"), 200)
        .await
        .unwrap();
    let map = db::canonical::availability_get_many(&pool, &["tt1".to_string()], "movie")
        .await
        .unwrap();
    assert_eq!(map["tt1"].source_count, 5);
    assert_eq!(map["tt1"].best_quality.as_deref(), Some("1080p"));
    pool.close().await;
    cleanup_db(&path);
}
