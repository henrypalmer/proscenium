//! Milestone 22 acceptance tests: stream-failure diagnosis (spec §12).
//!
//! Verifies the classified, user-facing failure reasons (4xx / 5xx / network)
//! produced by `diagnose_playback_failure_impl` against a provider's HTTP status,
//! and that `redact_secrets` masks the provider password before it is logged.

use proscenium_lib::commands::playback::{diagnose_playback_failure_impl, redact_secrets};
use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::models::{CatalogData, Category, MovieItem, Provider, ProviderInput, ProviderType};
use proscenium_lib::db;
use sqlx::SqlitePool;
use std::path::PathBuf;

const PASSWORD: &str = "p@ss-w0rd-22";

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m22-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn seed_xtream_with_movie(pool: &SqlitePool, base: &str) -> Provider {
    let provider = upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M22".into(),
            provider_type: ProviderType::Xtream,
            server_url: Some(base.to_string()),
            username: Some("u1".into()),
            password: Some(PASSWORD.into()),
            playlist_url: None,
            local_file_path: None,
        },
    )
    .await
    .expect("provider");

    let data = CatalogData {
        vod_categories: vec![Category { id: "1".into(), name: "Action".into(), sort_order: 0 }],
        movies: vec![MovieItem {
            id: "m1".into(),
            provider_id: String::new(),
            name: "Heat".into(),
            category_id: "1".into(),
            category_name: "Action".into(),
            poster_url: None,
            stream_url: String::new(),
            container_ext: "mkv".into(),
            release_year: None,
            rating: None,
            added_at: None,
        }],
        ..Default::default()
    };
    db::catalog::replace_catalog(pool, &provider.id, &data, 1).await.expect("seed");
    provider
}

/// A mock server that answers every request with a fixed HTTP status.
async fn spawn_status_server(status: u16) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { break };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 {status} Status\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = sock.write_all(resp.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

#[test]
fn redacts_xtream_password_and_query_credentials() {
    // Path-embedded Xtream password.
    let xtream = format!("http://srv.example/movie/u1/{PASSWORD}/9.mkv");
    let red = redact_secrets(&xtream, Some(PASSWORD));
    assert!(!red.contains(PASSWORD), "password must be masked: {red}");
    assert!(red.contains("/u1/***/9.mkv"));

    // M3U `get.php` query credentials (no app-injected keychain secret).
    let m3u = "http://srv.example/get.php?username=u1&password=hunter2&type=m3u_plus";
    let red = redact_secrets(m3u, None);
    assert!(!red.contains("hunter2"), "query password must be masked: {red}");
    assert!(red.contains("password=***"));
    assert!(red.contains("username=u1"));

    // No secret present — unchanged.
    assert_eq!(redact_secrets("http://x/live/1.ts", None), "http://x/live/1.ts");
}

#[tokio::test]
async fn diagnose_classifies_403_forbidden() {
    let base = spawn_status_server(403).await;
    let path = temp_path("403");
    let pool = db::init(&path).await.expect("init");
    let provider = seed_xtream_with_movie(&pool, &base).await;

    let msg = diagnose_playback_failure_impl(&pool, &provider.id, "movie", "m1", Some("loading failed"))
        .await;
    assert!(msg.contains("403"), "expected an HTTP 403 reason, got: {msg}");
    assert!(msg.to_lowercase().contains("denied"), "got: {msg}");

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn diagnose_classifies_500_server_error() {
    let base = spawn_status_server(500).await;
    let path = temp_path("500");
    let pool = db::init(&path).await.expect("init");
    let provider = seed_xtream_with_movie(&pool, &base).await;

    let msg = diagnose_playback_failure_impl(&pool, &provider.id, "movie", "m1", None).await;
    assert!(msg.contains("500"), "expected an HTTP 500 reason, got: {msg}");
    assert!(msg.to_lowercase().contains("server error"), "got: {msg}");

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn diagnose_classifies_unreachable_provider_as_network() {
    // Bind then drop to obtain a port nothing is listening on.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let base = format!("http://{addr}");

    let path = temp_path("net");
    let pool = db::init(&path).await.expect("init");
    let provider = seed_xtream_with_movie(&pool, &base).await;

    let msg = diagnose_playback_failure_impl(&pool, &provider.id, "movie", "m1", None).await;
    assert!(
        msg.to_lowercase().contains("could not reach"),
        "expected a network-failure reason, got: {msg}"
    );

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}
