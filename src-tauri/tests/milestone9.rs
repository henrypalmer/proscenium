//! Milestone 9 acceptance tests: the Live TV in-section channel filter
//! (spec §5.3). The filter is a case-insensitive name substring match applied
//! at the database layer so it covers the whole category — not just the loaded
//! virtualization window — and composes with the category filter. The search
//! results screen (spec §5.5) reuses the existing `search` command (covered by
//! milestone6.rs) and is verified in the browser preview.

use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, LiveChannel, Provider, ProviderInput, ProviderType,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_db(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m9-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

fn channel(id: &str, name: &str, category_id: &str) -> LiveChannel {
    LiveChannel {
        id: id.into(),
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
            name: "M9".into(),
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
async fn channel_filter_matches_by_name_and_composes_with_category() {
    let path = temp_db("filter");
    let pool = db::init(&path).await.expect("init");

    let data = CatalogData {
        live_categories: vec![category("Sports", 0), category("News", 1)],
        live_channels: vec![
            channel("1", "ESPN HD", "Sports"),
            channel("2", "Sky Sports News", "Sports"),
            channel("3", "BBC News", "News"),
            channel("4", "CNN News HD", "News"),
            channel("5", "Discovery", "News"),
        ],
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;

    // "All Channels" + filter: matches across every category, case-insensitive.
    let news = db::catalog::live_channels_page(&pool, &provider.id, None, Some("news"), 1, 50)
        .await
        .unwrap();
    assert_eq!(news.total, 3);
    let names: Vec<_> = news.items.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["BBC News", "CNN News HD", "Sky Sports News"]);

    // Filter scoped to a category: only Sports channels whose name matches.
    let sports_news =
        db::catalog::live_channels_page(&pool, &provider.id, Some("Sports"), Some("news"), 1, 50)
            .await
            .unwrap();
    assert_eq!(sports_news.total, 1);
    assert_eq!(sports_news.items[0].name, "Sky Sports News");

    // A filter that matches nothing yields an empty page, not an error
    // (drives the "No channels match" inline state).
    let empty = db::catalog::live_channels_page(&pool, &provider.id, None, Some("zzz"), 1, 50)
        .await
        .unwrap();
    assert_eq!(empty.total, 0);
    assert!(empty.items.is_empty());

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn blank_filter_is_ignored_and_like_metacharacters_match_literally() {
    let path = temp_db("blank");
    let pool = db::init(&path).await.expect("init");

    let data = CatalogData {
        live_categories: vec![category("General", 0)],
        live_channels: vec![
            channel("1", "Channel One", "General"),
            channel("2", "100% Sport", "General"),
            channel("3", "Movies", "General"),
        ],
        ..Default::default()
    };
    let provider = seed(&pool, &data).await;

    // Whitespace-only filter behaves exactly like no filter.
    let all = db::catalog::live_channels_page(&pool, &provider.id, None, Some("   "), 1, 50)
        .await
        .unwrap();
    assert_eq!(all.total, 3);

    // "%" is a LIKE wildcard; it must be escaped and match the literal char,
    // not every channel.
    let percent = db::catalog::live_channels_page(&pool, &provider.id, None, Some("%"), 1, 50)
        .await
        .unwrap();
    assert_eq!(percent.total, 1);
    assert_eq!(percent.items[0].name, "100% Sport");

    pool.close().await;
    cleanup_db(&path);
}
