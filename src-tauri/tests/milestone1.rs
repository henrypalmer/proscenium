//! Milestone 1 acceptance tests: schema, provider CRUD, keychain storage,
//! persistence across reopen, cascade delete, and connection testing.

use proscenium_lib::commands::providers::{
    delete_provider_impl, list_providers_impl, test_provider_connection_impl,
    upsert_provider_impl,
};
use proscenium_lib::models::{ProviderInput, ProviderType};
use proscenium_lib::{db, keychain};
use sqlx::Row;
use std::path::PathBuf;

fn temp_db_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "proscenium-test-{tag}-{}.db",
        uuid::Uuid::new_v4()
    ))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

fn xtream_input(name: &str, server_url: &str, password: Option<&str>) -> ProviderInput {
    ProviderInput {
        id: None,
        name: name.into(),
        provider_type: ProviderType::Xtream,
        server_url: Some(server_url.into()),
        username: Some("user1".into()),
        password: password.map(Into::into),
        playlist_url: None,
        local_file_path: None,
    }
}

fn m3u_input(name: &str, url: Option<&str>, file: Option<&str>) -> ProviderInput {
    ProviderInput {
        id: None,
        name: name.into(),
        provider_type: ProviderType::M3u,
        server_url: None,
        username: None,
        password: None,
        playlist_url: url.map(Into::into),
        local_file_path: file.map(Into::into),
    }
}

/// Minimal one-shot HTTP server for connection tests.
async fn spawn_http_server(status: &'static str, content_type: &'static str, body: &'static str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let mut buf = [0u8; 8192];
            let _ = sock.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(response.as_bytes()).await;
        }
    });
    format!("http://{addr}")
}

// --- Schema & persistence (criteria: schema applies, providers persist across restarts) ---

#[tokio::test]
async fn schema_applies_and_providers_persist_across_reopen() {
    let path = temp_db_path("persist");

    let pool = db::init(&path).await.expect("first init");
    let saved = upsert_provider_impl(
        &pool,
        m3u_input("My M3U", Some("http://example.com/playlist.m3u"), None),
    )
    .await
    .expect("upsert");
    pool.close().await;

    // Simulate an app restart: reopen the same file (also exercises
    // idempotent schema application).
    let pool = db::init(&path).await.expect("second init");
    let providers = list_providers_impl(&pool).await.expect("list");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, saved.id);
    assert_eq!(providers[0].name, "My M3U");
    assert_eq!(
        providers[0].playlist_url.as_deref(),
        Some("http://example.com/playlist.m3u")
    );
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn fts5_tables_are_available() {
    let path = temp_db_path("fts5");
    let pool = db::init(&path).await.expect("init");
    // Schema creation already fails if the bundled SQLite lacks FTS5; this
    // exercises the external-content wiring end to end.
    let saved = upsert_provider_impl(
        &pool,
        m3u_input("FTS", Some("http://example.com/list.m3u"), None),
    )
    .await
    .expect("provider");
    sqlx::query(
        "INSERT INTO movies (id, provider_id, name, category_id, category_name, stream_url, container_ext)
         VALUES ('m1', ?, 'Alien', 'cat1', 'Sci-Fi', 'http://example.com/m1.mp4', 'mp4')",
    )
    .bind(&saved.id)
    .execute(&pool)
    .await
    .expect("insert movie");
    // External-content FTS tables index on demand; 'rebuild' syncs from the
    // content table (refresh will do this in Milestone 2).
    sqlx::query("INSERT INTO fts_movies(fts_movies) VALUES('rebuild')")
        .execute(&pool)
        .await
        .expect("fts5 rebuild");
    let row = sqlx::query("SELECT name FROM fts_movies WHERE fts_movies MATCH 'alien'")
        .fetch_one(&pool)
        .await
        .expect("fts5 query");
    assert_eq!(row.get::<String, _>("name"), "Alien");
    pool.close().await;
    cleanup_db(&path);
}

// --- Xtream provider + keychain (criterion: credentials stored in OS keychain) ---

#[tokio::test]
async fn xtream_password_is_stored_in_keychain_not_in_sqlite() {
    let path = temp_db_path("keychain");
    let pool = db::init(&path).await.expect("init");

    let saved = upsert_provider_impl(
        &pool,
        xtream_input("Xtream Test", "http://example.com:8080", Some("s3cret-pw")),
    )
    .await
    .expect("upsert");

    // The DB column must hold only the keychain reference key.
    let row = sqlx::query("SELECT password FROM providers WHERE id = ?")
        .bind(&saved.id)
        .fetch_one(&pool)
        .await
        .expect("fetch row");
    let stored: String = row.get("password");
    assert_eq!(stored, keychain::reference_key(&saved.id));
    assert!(!stored.contains("s3cret-pw"));

    // The actual secret round-trips through the OS keychain.
    assert_eq!(keychain::get_secret(&saved.id).expect("secret"), "s3cret-pw");

    // Updating without a password keeps the existing keychain reference.
    let mut update = xtream_input("Xtream Renamed", "http://example.com:8080", None);
    update.id = Some(saved.id.clone());
    let updated = upsert_provider_impl(&pool, update).await.expect("update");
    assert_eq!(updated.name, "Xtream Renamed");
    assert_eq!(keychain::get_secret(&saved.id).expect("secret"), "s3cret-pw");

    // Deleting the provider removes the keychain entry too.
    delete_provider_impl(&pool, &saved.id).await.expect("delete");
    assert!(keychain::get_secret(&saved.id).is_err());

    pool.close().await;
    cleanup_db(&path);
}

// --- Delete cascades (criterion: all associated data removed from SQLite) ---

#[tokio::test]
async fn delete_provider_cascades_to_catalog_tables() {
    let path = temp_db_path("cascade");
    let pool = db::init(&path).await.expect("init");

    let saved = upsert_provider_impl(
        &pool,
        m3u_input("Cascade", Some("http://example.com/list.m3u"), None),
    )
    .await
    .expect("upsert");

    sqlx::query(
        "INSERT INTO live_channels (id, provider_id, name, category_id, category_name, stream_url, stream_ext)
         VALUES ('ch1', ?, 'Channel One', 'cat1', 'News', 'http://example.com/1.ts', 'ts')",
    )
    .bind(&saved.id)
    .execute(&pool)
    .await
    .expect("insert channel");
    sqlx::query(
        "INSERT INTO movies (id, provider_id, name, category_id, category_name, stream_url, container_ext)
         VALUES ('m1', ?, 'A Movie', 'cat2', 'Drama', 'http://example.com/m1.mp4', 'mp4')",
    )
    .bind(&saved.id)
    .execute(&pool)
    .await
    .expect("insert movie");

    delete_provider_impl(&pool, &saved.id).await.expect("delete");

    for table in ["providers", "live_channels", "movies"] {
        let row = sqlx::query(&format!("SELECT COUNT(*) AS n FROM {table}"))
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(row.get::<i64, _>("n"), 0, "{table} not emptied");
    }

    pool.close().await;
    cleanup_db(&path);
}

// --- M3U providers by URL and by local file path ---

#[tokio::test]
async fn m3u_provider_saved_by_url_and_by_file_path() {
    let path = temp_db_path("m3u");
    let pool = db::init(&path).await.expect("init");

    upsert_provider_impl(
        &pool,
        m3u_input("By URL", Some("http://example.com/playlist.m3u"), None),
    )
    .await
    .expect("url provider");
    upsert_provider_impl(
        &pool,
        m3u_input("By File", None, Some("C:\\playlists\\channels.m3u")),
    )
    .await
    .expect("file provider");

    let providers = list_providers_impl(&pool).await.expect("list");
    assert_eq!(providers.len(), 2);
    assert!(providers.iter().any(|p| p.playlist_url.is_some()));
    assert!(providers.iter().any(|p| p.local_file_path.is_some()));

    // Missing both sources is rejected with a descriptive error.
    let err = upsert_provider_impl(&pool, m3u_input("Bad", None, None))
        .await
        .unwrap_err();
    assert!(err.contains("playlist URL or local file path"));

    pool.close().await;
    cleanup_db(&path);
}

// --- Connection tests (criterion: success or descriptive error for both types) ---

#[tokio::test]
async fn xtream_test_connection_success_parses_account_info() {
    let body = r#"{"user_info":{"auth":1,"status":"Active","exp_date":"1781000000","max_connections":"2","active_cons":"0"},"server_info":{"url":"example.com"}}"#;
    let base = spawn_http_server("200 OK", "application/json", body).await;

    let result = test_provider_connection_impl(xtream_input("T", &base, Some("pw")))
        .await
        .expect("test runs");
    assert!(result.success, "expected success, got: {}", result.message);
    let info = result.account_info.expect("account info");
    assert_eq!(info.status.as_deref(), Some("Active"));
    assert_eq!(info.exp_date, Some(1781000000));
    assert_eq!(info.max_connections, Some(2));
    assert_eq!(info.active_connections, Some(0));
}

#[tokio::test]
async fn xtream_test_connection_reports_auth_failure() {
    let body = r#"{"user_info":{"auth":0}}"#;
    let base = spawn_http_server("200 OK", "application/json", body).await;

    let result = test_provider_connection_impl(xtream_input("T", &base, Some("wrong")))
        .await
        .expect("test runs");
    assert!(!result.success);
    assert!(
        result.message.contains("Authentication failed"),
        "unexpected message: {}",
        result.message
    );
}

#[tokio::test]
async fn xtream_test_connection_reports_unreachable_server() {
    // Port 9 (discard) is assumed closed locally.
    let result = test_provider_connection_impl(xtream_input(
        "T",
        "http://127.0.0.1:9",
        Some("pw"),
    ))
    .await
    .expect("test runs");
    assert!(!result.success);
    assert!(
        result.message.contains("Could not connect"),
        "unexpected message: {}",
        result.message
    );
}

#[tokio::test]
async fn m3u_test_connection_url_success_and_invalid_content() {
    let base = spawn_http_server(
        "200 OK",
        "application/x-mpegurl",
        "#EXTM3U\n#EXTINF:-1 tvg-name=\"Ch\",Ch\nhttp://example.com/1.ts\n",
    )
    .await;
    let result = test_provider_connection_impl(m3u_input("T", Some(&base), None))
        .await
        .expect("test runs");
    assert!(result.success, "expected success, got: {}", result.message);

    let html = spawn_http_server("200 OK", "text/html", "<html>not a playlist</html>").await;
    let result = test_provider_connection_impl(m3u_input("T", Some(&html), None))
        .await
        .expect("test runs");
    assert!(!result.success);
    assert!(result.message.contains("#EXTM3U"));
}

#[tokio::test]
async fn m3u_test_connection_local_file_success_and_missing() {
    let file = std::env::temp_dir().join(format!("proscenium-{}.m3u", uuid::Uuid::new_v4()));
    std::fs::write(&file, "#EXTM3U\n#EXTINF:-1,Ch\nhttp://example.com/1.ts\n").unwrap();

    let result =
        test_provider_connection_impl(m3u_input("T", None, Some(file.to_str().unwrap())))
            .await
            .expect("test runs");
    assert!(result.success, "expected success, got: {}", result.message);
    let _ = std::fs::remove_file(&file);

    let result = test_provider_connection_impl(m3u_input(
        "T",
        None,
        Some("C:\\does\\not\\exist\\playlist.m3u"),
    ))
    .await
    .expect("test runs");
    assert!(!result.success);
    assert!(
        result.message.contains("File not found"),
        "unexpected message: {}",
        result.message
    );
}
