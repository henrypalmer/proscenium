//! Milestone 29 (polish bundle) backend tests: the two slices with a backend —
//! recently-watched channels (record + recency-ordered join, orphan drop, cap)
//! and the custom per-provider category order (set/get/replace + scoping). All
//! local; no provider request.

use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, LiveChannel, Provider, ProviderInput, ProviderType,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m29-{tag}-{}.db", uuid::Uuid::new_v4()))
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

fn channel(id: &str, name: &str) -> LiveChannel {
    LiveChannel {
        id: id.into(),
        name: name.into(),
        category_id: "cat".into(),
        category_name: "Cat".into(),
        logo_url: None,
        stream_url: String::new(),
        stream_ext: "ts".into(),
        epg_channel_id: None,
    }
}

#[tokio::test]
async fn recent_channels_order_by_recency_bump_drop_orphans_and_cap() {
    let path = temp_path("recents");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M29 recents").await;

    let data = CatalogData {
        live_categories: vec![Category {
            id: "cat".into(),
            name: "Cat".into(),
            sort_order: 0,
        }],
        live_channels: vec![channel("c1", "One"), channel("c2", "Two"), channel("c3", "Three")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    db::catalog::record_recent_channel(&pool, &provider.id, "c1", 100).await.unwrap();
    db::catalog::record_recent_channel(&pool, &provider.id, "c2", 200).await.unwrap();
    let recent = db::catalog::recent_channels(&pool, &provider.id, 15).await.unwrap();
    let ids: Vec<&str> = recent.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["c2", "c1"], "most-recent first");

    // Re-watching c1 bumps it to the top.
    db::catalog::record_recent_channel(&pool, &provider.id, "c1", 300).await.unwrap();
    let recent = db::catalog::recent_channels(&pool, &provider.id, 15).await.unwrap();
    let ids: Vec<&str> = recent.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["c1", "c2"]);

    // A recency entry for a channel no longer in the catalog is dropped by the join.
    db::catalog::record_recent_channel(&pool, &provider.id, "ghost", 400).await.unwrap();
    let recent = db::catalog::recent_channels(&pool, &provider.id, 15).await.unwrap();
    let ids: Vec<&str> = recent.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["c1", "c2"], "orphaned recency rows are skipped");

    // The cap limits the result.
    let capped = db::catalog::recent_channels(&pool, &provider.id, 1).await.unwrap();
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].id, "c1");

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn category_order_set_get_replace_and_scope() {
    let path = temp_path("catorder");
    let pool = db::init(&path).await.expect("init");
    let provider = make_provider(&pool, "M29 order A").await;
    let other = make_provider(&pool, "M29 order B").await;

    // Empty until the user sets one.
    assert!(db::catalog::category_order(&pool, &provider.id, "movie")
        .await
        .unwrap()
        .is_empty());

    let order = ["b".to_string(), "a".to_string(), "c".to_string()];
    db::catalog::set_category_order(&pool, &provider.id, "movie", &order).await.unwrap();
    assert_eq!(
        db::catalog::category_order(&pool, &provider.id, "movie").await.unwrap(),
        order
    );

    // Re-setting replaces (no duplicate/leftover rows).
    let next = ["c".to_string(), "b".to_string()];
    db::catalog::set_category_order(&pool, &provider.id, "movie", &next).await.unwrap();
    assert_eq!(
        db::catalog::category_order(&pool, &provider.id, "movie").await.unwrap(),
        next
    );

    // Section-scoped: the "live" section is untouched by the "movie" order.
    assert!(db::catalog::category_order(&pool, &provider.id, "live")
        .await
        .unwrap()
        .is_empty());

    // Provider-scoped: a second provider sees no custom order.
    assert!(db::catalog::category_order(&pool, &other.id, "movie")
        .await
        .unwrap()
        .is_empty());

    pool.close().await;
    cleanup_db(&path);
}
