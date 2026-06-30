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
