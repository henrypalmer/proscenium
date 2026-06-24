//! Milestone 21 acceptance tests: credential hardening (spec §5.1).
//!
//! Verifies that the provider password is never persisted in the catalog: an
//! Xtream refresh stores only the stream id + container/stream extension, the
//! playable URL is composed at playback time from the keychain secret, and any
//! password-bearing URLs left on disk by an earlier build are scrubbed on
//! schema apply while M3U direct URLs (no app-injected secret) are preserved.

use proscenium_lib::commands::catalog::refresh_catalog_impl;
use proscenium_lib::commands::playback::resolve_stream_url_impl;
use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::models::{Provider, ProviderInput, ProviderType};
use proscenium_lib::db;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::sync::Arc;

const PASSWORD: &str = "sup3r-s3cret-pw";

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m21-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn make_xtream_provider(pool: &SqlitePool, base: &str) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M21 xtream".into(),
            provider_type: ProviderType::Xtream,
            server_url: Some(base.to_string()),
            username: Some("u1".into()),
            password: Some(PASSWORD.into()),
            playlist_url: None,
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

/// Minimal Xtream mock returning one item per catalog endpoint. The handler
/// dispatches on the `action=` query value (categories variants first so they
/// don't match their base action's substring).
async fn spawn_xtream_server() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handler = Arc::new(|target: &str| -> Option<&'static str> {
        if target.contains("get_live_categories") {
            Some(r#"[{"category_id":"1","category_name":"News"}]"#)
        } else if target.contains("get_live_streams") {
            Some(r#"[{"stream_id":101,"name":"News HD","category_id":"1"}]"#)
        } else if target.contains("get_vod_categories") {
            Some(r#"[{"category_id":"2","category_name":"Action"}]"#)
        } else if target.contains("get_vod_streams") {
            Some(r#"[{"stream_id":201,"name":"Heat","category_id":"2","container_extension":"mkv"}]"#)
        } else if target.contains("get_series_categories") {
            Some(r#"[{"category_id":"3","category_name":"Crime"}]"#)
        } else if target.contains("get_series") {
            Some(r#"[{"series_id":301,"name":"Breaking Code","category_id":"3"}]"#)
        } else {
            None
        }
    });
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let target = request
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let response = match handler(&target) {
                    Some(body) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    ),
                    None => {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                    }
                };
                let _ = sock.write_all(response.as_bytes()).await;
            });
        }
    });
    format!("http://{addr}")
}

/// A full Xtream refresh persists no password-bearing URL, and the playable URL
/// is composed at playback from the keychain secret.
#[tokio::test]
async fn xtream_refresh_persists_no_password_and_composes_url() {
    let base = spawn_xtream_server().await;
    let path = temp_path("refresh");
    let pool = db::init(&path).await.expect("init");
    let provider = make_xtream_provider(&pool, &base).await;

    refresh_catalog_impl(&pool, &provider, |_, _| {})
        .await
        .expect("refresh");

    // No catalog row carries the password (or any non-empty stream_url) for Xtream.
    for table in ["live_channels", "movies", "episodes"] {
        let rows: Vec<String> = sqlx::query(&format!("SELECT stream_url FROM {table}"))
            .fetch_all(&pool)
            .await
            .unwrap()
            .iter()
            .map(|r| r.get::<String, _>("stream_url"))
            .collect();
        for url in &rows {
            assert!(
                !url.contains(PASSWORD),
                "{table}.stream_url leaked the password: {url}"
            );
            assert_eq!(url, "", "{table}.stream_url should be empty for Xtream");
        }
    }

    // The playable URL is composed at playback from the keychain secret.
    let live = resolve_stream_url_impl(&pool, &provider.id, "live", "101")
        .await
        .expect("live url");
    assert_eq!(live, format!("{base}/live/u1/{PASSWORD}/101.ts"));
    let movie = resolve_stream_url_impl(&pool, &provider.id, "movie", "201")
        .await
        .expect("movie url");
    assert_eq!(movie, format!("{base}/movie/u1/{PASSWORD}/201.mkv"));

    delete_provider_impl(&pool, &provider.id).await.unwrap(); // cleans keychain
    pool.close().await;
    cleanup_db(&path);
}

/// An existing install with password-bearing URLs already on disk is scrubbed
/// the next time the schema is applied; M3U direct URLs are preserved.
#[tokio::test]
async fn existing_password_urls_are_scrubbed_on_apply() {
    let path = temp_path("scrub");
    let pool = db::init(&path).await.expect("init");

    let xtream = make_xtream_provider(&pool, "http://srv.example").await;
    let m3u = upsert_provider_impl(
        &pool,
        ProviderInput {
            id: None,
            name: "M21 m3u".into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://srv.example/play.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .expect("m3u provider");

    // Simulate rows written by a pre-Milestone-21 build: the Xtream URL embeds
    // the password in cleartext; the M3U URL is the provider's own direct URL.
    let leaked = format!("http://srv.example/movie/u1/{PASSWORD}/9.mkv");
    sqlx::query(
        "INSERT INTO movies (id, provider_id, name, category_id, category_name, stream_url, container_ext)
         VALUES ('x9', ?, 'Leaked', 'c', 'Cat', ?, 'mkv')",
    )
    .bind(&xtream.id)
    .bind(&leaked)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO movies (id, provider_id, name, category_id, category_name, stream_url, container_ext)
         VALUES ('d9', ?, 'Direct', 'c', 'Cat', 'http://srv.example/direct/9.mp4', 'mp4')",
    )
    .bind(&m3u.id)
    .execute(&pool)
    .await
    .unwrap();

    // Re-apply the schema (as on the next launch) — the scrub migration runs.
    db::schema::apply(&pool).await.expect("re-apply schema");

    let xtream_url: String =
        sqlx::query_scalar("SELECT stream_url FROM movies WHERE id = 'x9'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(xtream_url, "", "the Xtream password URL must be scrubbed");

    let m3u_url: String = sqlx::query_scalar("SELECT stream_url FROM movies WHERE id = 'd9'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        m3u_url, "http://srv.example/direct/9.mp4",
        "M3U direct URLs (no app-injected secret) must be preserved"
    );

    delete_provider_impl(&pool, &xtream.id).await.unwrap();
    delete_provider_impl(&pool, &m3u.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}
