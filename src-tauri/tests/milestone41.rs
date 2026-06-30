//! Milestone 41 acceptance tests. Slice 1 (addon storage): manifest parsing and
//! validation (object- and string-form `resources`, id prefixes) and the
//! `stremio_addons` storage CRUD. Stream parsing/resolution is slice 2.

use proscenium_lib::canonical::stremio;
use proscenium_lib::db;
use proscenium_lib::models::StremioAddon;
use serde_json::json;
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m41-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), s));
    }
}

#[test]
fn manifest_parses_object_resources_and_id_prefixes() {
    // Torrentio-style: `resources` are objects that carry `idPrefixes`.
    let body = json!({
        "id": "com.x", "name": "Torrentio", "types": ["movie", "series"],
        "resources": [
            { "name": "stream", "types": ["movie", "series"], "idPrefixes": ["tt", "kitsu"] }
        ]
    });
    let m = stremio::parse_manifest(&body);
    assert_eq!(m.name, "Torrentio");
    assert_eq!(m.resources, vec!["stream"]);
    assert!(m.id_prefixes.contains(&"tt".to_string()));
    assert!(stremio::validate(&m).is_ok());
}

#[test]
fn manifest_parses_string_resources_and_top_level_prefixes() {
    // AIOStreams-style: plain-string `resources` + top-level `idPrefixes`.
    let body = json!({
        "name": "AIOStreams", "types": ["movie", "series"],
        "resources": ["stream", "meta"], "idPrefixes": ["tt", "tmdb"]
    });
    let m = stremio::parse_manifest(&body);
    assert_eq!(m.resources, vec!["stream", "meta"]);
    assert_eq!(m.id_prefixes, vec!["tt", "tmdb"]);
    assert!(stremio::validate(&m).is_ok());
}

#[test]
fn manifest_without_a_stream_resource_is_rejected() {
    let body = json!({ "name": "Subs", "resources": ["subtitles"], "types": ["movie"] });
    let m = stremio::parse_manifest(&body);
    assert!(stremio::validate(&m).is_err());
}

#[test]
fn base_url_strips_manifest_json() {
    assert_eq!(
        stremio::base_url("https://h.example/abc/manifest.json"),
        "https://h.example/abc"
    );
    assert_eq!(stremio::base_url("https://h.example/abc/"), "https://h.example/abc");
    assert_eq!(stremio::base_url("https://h.example/abc"), "https://h.example/abc");
}

#[tokio::test]
async fn stremio_addon_storage_crud_roundtrips() {
    let path = temp_path("crud");
    let pool: SqlitePool = db::init(&path).await.expect("init");

    assert_eq!(db::stremio::next_position(&pool).await.unwrap(), 0);
    let a = StremioAddon {
        id: "a1".into(),
        name: "Torrentio".into(),
        types: vec!["movie".into(), "series".into()],
        resources: vec!["stream".into()],
        id_prefixes: vec!["tt".into()],
        position: 0,
        created_at: 100,
    };
    db::stremio::insert(&pool, &a, "keyring:Proscenium/addon:a1")
        .await
        .unwrap();
    assert_eq!(db::stremio::next_position(&pool).await.unwrap(), 1);

    let b = StremioAddon {
        id: "b1".into(),
        name: "Comet".into(),
        position: 1,
        created_at: 200,
        ..a.clone()
    };
    db::stremio::insert(&pool, &b, "keyring:Proscenium/addon:b1")
        .await
        .unwrap();

    let list = db::stremio::list(&pool).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "a1", "ordered by position");
    assert_eq!(list[1].name, "Comet");
    assert_eq!(list[0].types, vec!["movie", "series"], "JSON round-trip");
    assert_eq!(list[0].id_prefixes, vec!["tt"]);

    assert!(db::stremio::delete(&pool, "a1").await.unwrap());
    assert!(!db::stremio::delete(&pool, "a1").await.unwrap(), "idempotent");
    assert_eq!(db::stremio::list(&pool).await.unwrap().len(), 1);
    pool.close().await;
    cleanup_db(&path);
}

// --- Slice 2: stream parsing (direct vs infoHash) ---

#[test]
fn parse_streams_handles_direct_and_infohash() {
    let body = json!({
        "streams": [
            // Debrid-cached direct URL (AIOStreams/Torbox style).
            { "name": "AIOStreams\n2160p [TB⚡]",
              "title": "The.Matrix.1999.2160p.BluRay.x265\n💾 35 GB",
              "url": "https://torbox.example/dl/abc.mkv",
              "behaviorHints": { "filename": "The.Matrix.2160p.mkv" } },
            // Bare torrent — infoHash only, no debrid (Torrentio style).
            { "name": "Torrentio\n1080p",
              "title": "The Matrix 1999 1080p BluRay x264",
              "infoHash": "03dd34fea0ff15a451c1723062a901aa3a0ad458" }
        ]
    });
    let c = stremio::parse_streams(&body, "AIOStreams", "movie");
    assert_eq!(c.len(), 2);
    // Direct first (higher quality confidence than the infoHash marker).
    assert_eq!(c[0].url.as_deref(), Some("https://torbox.example/dl/abc.mkv"));
    assert_eq!(c[0].quality.as_deref(), Some("2160p"));
    assert_eq!(c[0].container.as_deref(), Some("mkv"));
    assert!(!c[0].needs_debrid);
    assert_eq!(c[0].content_type, "movie");
    assert_eq!(c[0].source, "AIOStreams");
    // infoHash-only → needs_debrid marker, not directly playable.
    let debrid = c.iter().find(|x| x.needs_debrid).expect("infohash candidate");
    assert!(debrid.url.is_none());
    assert_eq!(debrid.quality.as_deref(), Some("1080p"));
}

#[test]
fn parse_streams_caps_direct_and_degrades_on_empty() {
    // 12 direct streams → capped at 8.
    let streams: Vec<_> = (0..12)
        .map(|i| {
            json!({ "name": "Addon\n1080p", "title": format!("File {i} 1080p"),
                    "url": format!("https://x/{i}.mp4") })
        })
        .collect();
    let c = stremio::parse_streams(&json!({ "streams": streams }), "Addon", "movie");
    assert_eq!(c.len(), 8, "direct streams capped");
    assert!(c.iter().all(|x| !x.needs_debrid && x.url.is_some()));
    // No `streams` key → empty, never panics (the failure-degrade path).
    assert!(stremio::parse_streams(&json!({}), "Addon", "movie").is_empty());
}

#[test]
fn parse_streams_caps_infohash_markers() {
    let streams: Vec<_> = (0..20)
        .map(|i| json!({ "name": "Torrentio\n720p", "title": format!("T {i}"),
                         "infoHash": format!("hash{i}") }))
        .collect();
    let c = stremio::parse_streams(&json!({ "streams": streams }), "Torrentio", "episode");
    assert_eq!(c.len(), 5, "infoHash markers capped");
    assert!(c.iter().all(|x| x.needs_debrid && x.url.is_none()));
    assert_eq!(c[0].content_type, "episode");
}
