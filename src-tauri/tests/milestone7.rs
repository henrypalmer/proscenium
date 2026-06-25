//! Milestone 7 acceptance tests: settings persistence and defaults, the
//! external-player default flowing through `open_in_external_player`, the
//! hardware-decode toggle, provider-status classification for the warning
//! banner (unreachable / expired / healthy), and 30-day image-cache eviction.

use proscenium_lib::commands::playback::open_in_external_player_impl;
use proscenium_lib::commands::providers::{
    check_provider_status_impl, classify_provider_status, upsert_provider_impl,
};
use proscenium_lib::commands::settings::{evict_stale_images, get_settings_impl, set_setting_impl};
use proscenium_lib::db;
use proscenium_lib::models::{
    AppSettings, ConnectionTestResult, ProviderInput, ProviderType, XtreamAccountInfo,
};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m7-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn m3u_provider(pool: &SqlitePool, name: &str, url: &str) -> String {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: name.into(),
            provider_type: ProviderType::M3u,
            playlist_url: Some(url.into()),
            server_url: None,
            username: None,
            password: None,
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
    .id
}

// --- Settings: defaults, round-trip, persistence ---

#[tokio::test]
async fn settings_default_to_spec_values_and_persist_across_reopen() {
    let path = temp_path("settings");
    let pool = db::init(&path).await.expect("init");

    // A fresh database returns the §15 defaults.
    let defaults = get_settings_impl(&pool).await.expect("get");
    assert_eq!(defaults, AppSettings::default());
    assert_eq!(defaults.cache_ttl_hours, 6);
    assert_eq!(defaults.default_external_player, "mpv");
    assert_eq!(defaults.ui_density, "comfortable");
    assert!(defaults.hw_decode_enabled);

    // Write one of each writable key.
    set_setting_impl(&pool, "default_external_player", "vlc").await.unwrap();
    set_setting_impl(&pool, "hw_decode_enabled", "false").await.unwrap();
    set_setting_impl(&pool, "ui_density", "compact").await.unwrap();
    set_setting_impl(&pool, "cache_ttl_hours", "12").await.unwrap();
    set_setting_impl(&pool, "custom_player_command", "potplayer {url}").await.unwrap();

    // Unknown keys are rejected.
    assert!(set_setting_impl(&pool, "not_a_real_key", "x").await.is_err());

    pool.close().await;

    // Reopen the same file: every value survives the restart.
    let pool = db::init(&path).await.expect("reopen");
    let s = get_settings_impl(&pool).await.expect("get");
    assert_eq!(s.default_external_player, "vlc");
    assert!(!s.hw_decode_enabled);
    assert_eq!(s.ui_density, "compact");
    assert_eq!(s.cache_ttl_hours, 12);
    assert_eq!(s.custom_player_command.as_deref(), Some("potplayer {url}"));

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn changing_default_external_player_is_picked_up_immediately() {
    let path = temp_path("extplayer");
    let pool = db::init(&path).await.expect("init");

    // With "custom" as the default and no command configured, the very next
    // launch resolves to "custom" and reports the missing-command error —
    // proof the stored default is read fresh on each call, not cached.
    set_setting_impl(&pool, "default_external_player", "custom").await.unwrap();
    let err = open_in_external_player_impl(&pool, "http://stream.local/x.ts", None)
        .await
        .expect_err("custom with no command should fail");
    assert!(err.to_lowercase().contains("custom"), "got: {err}");

    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn hardware_decode_can_be_toggled_off() {
    let path = temp_path("hwdec");
    let pool = db::init(&path).await.expect("init");

    assert!(get_settings_impl(&pool).await.unwrap().hw_decode_enabled);
    set_setting_impl(&pool, "hw_decode_enabled", "false").await.unwrap();
    assert!(!get_settings_impl(&pool).await.unwrap().hw_decode_enabled);
    // Toggling back on restores the default behavior.
    set_setting_impl(&pool, "hw_decode_enabled", "true").await.unwrap();
    assert!(get_settings_impl(&pool).await.unwrap().hw_decode_enabled);

    pool.close().await;
    cleanup_db(&path);
}

// --- Provider status banner (spec §12) ---

fn account(status: &str) -> XtreamAccountInfo {
    XtreamAccountInfo {
        status: Some(status.into()),
        exp_date: None,
        max_connections: Some(1),
        active_connections: Some(0),
    }
}

#[test]
fn unreachable_provider_classifies_as_not_reachable() {
    let result = ConnectionTestResult::failure(
        "Could not connect to http://dead.example. Check the server address and your internet connection.",
    );
    let status = classify_provider_status(&result);
    assert!(!status.reachable);
    assert!(!status.expired);
    assert!(status.message.is_some());
}

#[test]
fn expired_subscription_classifies_as_expired() {
    let result = ConnectionTestResult {
        success: true,
        message: "Connected, but the subscription has expired.".into(),
        account_info: Some(account("Expired")),
    };
    let status = classify_provider_status(&result);
    assert!(status.reachable);
    assert!(status.expired);
    assert!(status.message.is_some());
}

#[test]
fn healthy_provider_has_no_banner() {
    let result = ConnectionTestResult {
        success: true,
        message: "Connected successfully.".into(),
        account_info: Some(account("Active")),
    };
    let status = classify_provider_status(&result);
    assert!(status.reachable);
    assert!(!status.expired);
    assert!(status.message.is_none());
}

#[tokio::test]
async fn check_status_reports_unreachable_for_a_dead_m3u_url() {
    let path = temp_path("status");
    let pool = db::init(&path).await.expect("init");
    let id = m3u_provider(&pool, "M7 dead", "http://unreachable.invalid/playlist.m3u").await;

    let status = check_provider_status_impl(&pool, &id).await.expect("status");
    assert!(!status.reachable);
    assert!(status.message.is_some());

    pool.close().await;
    cleanup_db(&path);
}

// --- Image cache eviction (spec §5.7) ---

#[tokio::test]
async fn stale_images_are_evicted_on_startup_fresh_ones_kept() {
    let path = temp_path("imgcache");
    let pool = db::init(&path).await.expect("init");

    let dir = std::env::temp_dir().join(format!("proscenium-m7-img-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let stale_file = dir.join("stale.jpg");
    let fresh_file = dir.join("fresh.jpg");
    std::fs::write(&stale_file, b"old").unwrap();
    std::fs::write(&fresh_file, b"new").unwrap();

    let now = 1_700_000_000;
    // Cached 40 days ago → already past the 30-day TTL.
    db::image_cache::put(
        &pool,
        "http://art.local/stale.jpg",
        stale_file.to_str().unwrap(),
        1024,
        now - 40 * 24 * 3600,
    )
    .await
    .unwrap();
    // Cached today → well within TTL.
    db::image_cache::put(
        &pool,
        "http://art.local/fresh.jpg",
        fresh_file.to_str().unwrap(),
        1024,
        now,
    )
    .await
    .unwrap();

    let evicted = evict_stale_images(&pool, now).await.expect("evict");
    assert_eq!(evicted, 1);

    // The stale row and its file are gone; the fresh ones remain.
    assert!(db::image_cache::expired(&pool, now).await.unwrap().is_empty());
    assert!(!stale_file.exists(), "stale file should be deleted");
    assert!(fresh_file.exists(), "fresh file should remain");

    pool.close().await;
    cleanup_db(&path);
    let _ = std::fs::remove_dir_all(&dir);
}
