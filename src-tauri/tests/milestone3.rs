//! Milestone 3 acceptance tests: live category listing (provider order,
//! empty categories hidden) and channel pagination with category filtering
//! and case-insensitive alphabetical ordering.

use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, LiveChannel, Provider, ProviderInput, ProviderType,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_db(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m3-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

fn channel(id: &str, name: &str, category_id: &str) -> LiveChannel {
    LiveChannel {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: category_id.into(),
        category_name: category_id.into(),
        logo_url: None,
        stream_url: format!("http://stream.example/{id}.ts"),
        stream_ext: "ts".into(),
        epg_channel_id: None,
    }
}

fn category(id: &str, sort_order: i64) -> Category {
    Category {
        id: id.into(),
        name: id.into(),
        sort_order,
    }
}

async fn seed(pool: &SqlitePool, data: &CatalogData) -> Provider {
    let provider = upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M3".into(),
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
async fn live_categories_keep_provider_order_and_hide_empty_ones() {
    let path = temp_db("cats");
    let pool = db::init(&path).await.expect("init");

    let data = CatalogData {
        // Provider order is intentionally non-alphabetical.
        live_categories: vec![category("Zebra", 0), category("Alpha", 1), category("Empty", 2)],
        live_channels: vec![
            channel("1", "Chan One", "Zebra"),
            channel("2", "Chan Two", "Alpha"),
        ],
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;

    let cats = db::catalog::live_categories(&pool, &provider.id).await.unwrap();
    let names: Vec<_> = cats.iter().map(|c| c.name.as_str()).collect();
    // Provider-defined order preserved; "Empty" hidden (spec §12).
    assert_eq!(names, vec!["Zebra", "Alpha"]);

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn channel_pagination_filtering_and_sorting() {
    let path = temp_db("page");
    let pool = db::init(&path).await.expect("init");

    // 25 channels: 15 in News, 10 in Sports. Mixed-case names to verify
    // case-insensitive alphabetical order.
    let mut channels = Vec::new();
    for i in 0..15 {
        let name = if i % 2 == 0 {
            format!("news channel {i:02}")
        } else {
            format!("News Channel {i:02}")
        };
        channels.push(channel(&format!("n{i}"), &name, "News"));
    }
    for i in 0..10 {
        channels.push(channel(&format!("s{i}"), &format!("Sport {i:02}"), "Sports"));
    }
    let data = CatalogData {
        live_categories: vec![category("News", 0), category("Sports", 1)],
        live_channels: channels,
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;

    // "All Channels": no category filter, total across categories.
    let page1 = db::catalog::live_channels_page(&pool, &provider.id, None, None, 1, 10)
        .await
        .unwrap();
    assert_eq!(page1.total, 25);
    assert_eq!(page1.items.len(), 10);
    let page3 = db::catalog::live_channels_page(&pool, &provider.id, None, None, 3, 10)
        .await
        .unwrap();
    assert_eq!(page3.items.len(), 5);

    // Case-insensitive alphabetical: "news channel 00/01/02..." interleaved
    // regardless of case.
    let names: Vec<_> = page1.items.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names[0], "news channel 00");
    assert_eq!(names[1], "News Channel 01");
    assert_eq!(names[2], "news channel 02");

    // Category filter.
    let sports = db::catalog::live_channels_page(&pool, &provider.id, Some("Sports"), None, 1, 50)
        .await
        .unwrap();
    assert_eq!(sports.total, 10);
    assert!(sports.items.iter().all(|c| c.category_id == "Sports"));

    // Out-of-range page: empty items, correct total.
    let beyond = db::catalog::live_channels_page(&pool, &provider.id, None, None, 99, 10)
        .await
        .unwrap();
    assert_eq!(beyond.total, 25);
    assert!(beyond.items.is_empty());

    // Degenerate inputs are clamped instead of erroring.
    let clamped = db::catalog::live_channels_page(&pool, &provider.id, None, None, 0, 0)
        .await
        .unwrap();
    assert_eq!(clamped.page, 1);
    assert_eq!(clamped.page_size, 1);
    assert_eq!(clamped.items.len(), 1);

    // Unknown category: empty but not an error.
    let none = db::catalog::live_channels_page(&pool, &provider.id, Some("Nope"), None, 1, 10)
        .await
        .unwrap();
    assert_eq!(none.total, 0);

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn deep_pagination_stays_fast_on_a_large_catalog() {
    let path = temp_db("perf");
    let pool = db::init(&path).await.expect("init");

    let mut channels = Vec::with_capacity(12_000);
    for i in 0..12_000 {
        channels.push(channel(
            &format!("c{i}"),
            &format!("Channel {:05}", (i * 7919) % 12_000),
            &format!("Group {}", i % 40),
        ));
    }
    let data = CatalogData {
        live_categories: (0..40).map(|i| category(&format!("Group {i}"), i)).collect(),
        live_channels: channels,
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;

    let started = std::time::Instant::now();
    let mid = db::catalog::live_channels_page(&pool, &provider.id, None, None, 30, 200)
        .await
        .unwrap();
    let elapsed = started.elapsed();
    println!("deep page query took {elapsed:?}");

    assert_eq!(mid.total, 12_000);
    assert_eq!(mid.items.len(), 200);
    assert!(
        elapsed.as_millis() < 100,
        "page query took {elapsed:?}, expected well under the 500ms browse budget"
    );

    pool.close().await;
    cleanup_db(&path);
}
