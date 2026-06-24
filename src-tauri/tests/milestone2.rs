//! Milestone 2 acceptance tests: M3U parsing (attrs, gzip, type inference),
//! Xtream catalog refresh, atomic persistence with FTS5, staleness logic,
//! 50k-item scale, failure isolation, and cache read performance.

use proscenium_lib::commands::catalog::{
    get_active_provider_impl, is_cache_stale, refresh_catalog_impl, set_active_provider_impl,
};
use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::iptv::m3u;
use proscenium_lib::models::{Provider, ProviderInput, ProviderType};
use proscenium_lib::{db, keychain};
use sqlx::{Row, SqlitePool};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

fn temp_path(tag: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m2-{tag}-{}.{ext}", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn make_m3u_provider(pool: &SqlitePool, url: Option<&str>, file: Option<&str>) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M2".into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: url.map(Into::into),
            local_file_path: file.map(Into::into),
        },
    )
    .await
    .expect("provider")
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query(&format!("SELECT COUNT(*) AS n FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
        .get("n")
}

/// Multi-request mock HTTP server; the handler maps a request target
/// (path + query) to (content_type, body).
async fn spawn_server(
    handler: impl Fn(&str) -> Option<(&'static str, Vec<u8>)> + Send + Sync + 'static,
) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handler = Arc::new(handler);
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
                    Some((content_type, body)) => {
                        let mut r = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes();
                        r.extend_from_slice(&body);
                        r
                    }
                    None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                };
                let _ = sock.write_all(&response).await;
            });
        }
    });
    format!("http://{addr}")
}

const SAMPLE_PLAYLIST: &str = r#"#EXTM3U
#EXTINF:-1 tvg-id="news1.uk" tvg-name="News One" tvg-logo="http://logo/news1.png" group-title="News",News One HD
http://stream.example/live/1.ts
#EXTINF:-1 tvg-id="sport1" group-title="Sports",Sports Central
http://stream.example/live/2.m3u8
#EXTINF:-1 group-title="News",News Two
http://stream.example/live/3.ts
#EXTINF:-1 tvg-logo="http://logo/heat.jpg" group-title="VOD | Action",Heat (1995)
http://stream.example/vod/heat.mp4
#EXTINF:-1 type:movie group-title="Drama",Quiet River
http://stream.example/vod/quiet-river.mkv
#EXTINF:-1 series-id="bb" group-title="Series | Crime",Breaking Code S01 E01
http://stream.example/series/bb-s01e01.mp4
#EXTINF:-1 series-id="bb" group-title="Series | Crime",Breaking Code S01 E02
http://stream.example/series/bb-s01e02.mp4
#EXTINF:-1 group-title="Series | Crime",Night Watch S02E05
http://stream.example/series/nw-s02e05.mkv
#EXTINF banana malformed line without colon
#EXTINF:-1 group-title="News",Channel With No URL
#EXTINF:-1 group-title="Kids",Cartoon Town
http://stream.example/live/4.ts
"#;

// --- M3U parser ---

#[test]
fn m3u_parser_attributes_inference_and_malformed_lines() {
    let outcome = m3u::parse_playlist(SAMPLE_PLAYLIST);
    let c = &outcome.catalog;

    assert_eq!(outcome.skipped_lines, 1, "malformed #EXTINF skipped");

    // "Channel With No URL" had its URL line replaced by the next #EXTINF →
    // discarded silently. 4 live channels remain.
    assert_eq!(c.live_channels.len(), 4);
    let news1 = &c.live_channels[0];
    assert_eq!(news1.name, "News One HD");
    assert_eq!(news1.epg_channel_id.as_deref(), Some("news1.uk"));
    assert_eq!(news1.logo_url.as_deref(), Some("http://logo/news1.png"));
    assert_eq!(news1.category_name, "News");
    assert_eq!(news1.stream_ext, "ts");
    assert_eq!(c.live_channels[1].stream_ext, "m3u8");

    // Live categories in playlist order: News, Sports, Kids.
    let names: Vec<_> = c.live_categories.iter().map(|cat| cat.name.as_str()).collect();
    assert_eq!(names, vec!["News", "Sports", "Kids"]);

    // Movies: one inferred from group "VOD | Action" + .mp4, one via type:movie.
    assert_eq!(c.movies.len(), 2);
    assert_eq!(c.movies[0].name, "Heat (1995)");
    assert_eq!(c.movies[0].release_year, Some(1995));
    assert_eq!(c.movies[0].container_ext, "mp4");
    assert_eq!(c.movies[1].name, "Quiet River");
    assert_eq!(c.movies[1].container_ext, "mkv");

    // Series: "bb" (via series-id, 2 episodes) and Night Watch (via SxxExx).
    assert_eq!(c.series.len(), 2);
    assert_eq!(c.series[0].id, "bb");
    assert_eq!(c.series[0].name, "Breaking Code");
    assert_eq!(c.episodes.len(), 3);
    assert_eq!(c.episodes[0].season, 1);
    assert_eq!(c.episodes[0].episode, 1);
    assert_eq!(c.episodes[1].episode, 2);
    let nw_ep = &c.episodes[2];
    assert_eq!(nw_ep.season, 2);
    assert_eq!(nw_ep.episode, 5);
    assert_eq!(c.series[1].name, "Night Watch");
}

#[test]
fn m3u_gzip_decode_roundtrip() {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(SAMPLE_PLAYLIST.as_bytes()).unwrap();
    let gz = encoder.finish().unwrap();

    let decoded = m3u::decode_playlist_bytes(&gz).expect("gzip decode");
    assert_eq!(decoded, SAMPLE_PLAYLIST);
    // Plain text passes through untouched.
    assert_eq!(
        m3u::decode_playlist_bytes(SAMPLE_PLAYLIST.as_bytes()).unwrap(),
        SAMPLE_PLAYLIST
    );
}

// --- M3U refresh end-to-end (gzip over HTTP) + FTS population ---

#[tokio::test]
async fn m3u_refresh_over_http_gzip_persists_catalog_and_fts() {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(SAMPLE_PLAYLIST.as_bytes()).unwrap();
    let gz = encoder.finish().unwrap();
    let base = spawn_server(move |target| {
        (target == "/playlist.m3u.gz").then(|| ("application/octet-stream", gz.clone()))
    })
    .await;

    let path = temp_path("gzip", "db");
    let pool = db::init(&path).await.expect("init");
    let provider =
        make_m3u_provider(&pool, Some(&format!("{base}/playlist.m3u.gz")), None).await;

    refresh_catalog_impl(&pool, &provider, |_, _| {})
        .await
        .expect("refresh");

    assert_eq!(count(&pool, "live_channels").await, 4);
    assert_eq!(count(&pool, "movies").await, 2);
    assert_eq!(count(&pool, "series").await, 2);
    assert_eq!(count(&pool, "episodes").await, 3);
    assert_eq!(count(&pool, "live_categories").await, 3);

    // FTS5 tables are populated and queryable after refresh.
    let row = sqlx::query("SELECT name FROM fts_live_channels WHERE fts_live_channels MATCH 'sports'")
        .fetch_one(&pool)
        .await
        .expect("fts query");
    assert_eq!(row.get::<String, _>("name"), "Sports Central");
    let row = sqlx::query("SELECT name FROM fts_movies WHERE fts_movies MATCH 'heat'")
        .fetch_one(&pool)
        .await
        .expect("fts movie query");
    assert_eq!(row.get::<String, _>("name"), "Heat (1995)");

    // last_refreshed was stamped.
    let p = db::providers::get(&pool, &provider.id).await.unwrap().unwrap();
    assert!(p.last_refreshed.is_some());

    pool.close().await;
    cleanup_db(&path);
}

// --- Xtream refresh end-to-end ---

#[tokio::test]
async fn xtream_refresh_persists_full_catalog() {
    let base = spawn_server(|target| {
        if !target.starts_with("/player_api.php") {
            return None;
        }
        let action = target
            .split("action=")
            .nth(1)
            .unwrap_or("")
            .split('&')
            .next()
            .unwrap_or("");
        let body = match action {
            "get_live_categories" => {
                r#"[{"category_id":"10","category_name":"News","parent_id":0},
                    {"category_id":"11","category_name":"Sports","parent_id":0}]"#
            }
            "get_live_streams" => {
                r#"[{"num":1,"name":"News One","stream_type":"live","stream_id":101,"stream_icon":"http://logo/1.png","epg_channel_id":"news1","category_id":"10"},
                    {"num":2,"name":"Sports 24","stream_type":"live","stream_id":102,"stream_icon":"","epg_channel_id":null,"category_id":"11"},
                    {"num":3,"name":"Mystery Channel","stream_type":"live","stream_id":103,"category_id":"99"}]"#
            }
            "get_vod_categories" => {
                r#"[{"category_id":"20","category_name":"Action","parent_id":0}]"#
            }
            "get_vod_streams" => {
                r#"[{"stream_id":201,"name":"Heat","stream_icon":"http://poster/heat.jpg","category_id":"20","container_extension":"mkv","rating":"8.3","year":"1995","added":"1700000000"},
                    {"stream_id":202,"name":"New Release","category_id":"20","container_extension":"mp4","rating":7.1,"releaseDate":"2023-06-01","added":1710000000}]"#
            }
            "get_series_categories" => {
                r#"[{"category_id":"30","category_name":"Crime","parent_id":0}]"#
            }
            "get_series" => {
                r#"[{"series_id":301,"name":"Breaking Code","cover":"http://poster/bc.jpg","category_id":"30","release_date":"2019-01-20","rating":"9"}]"#
            }
            _ => return None,
        };
        Some(("application/json", body.as_bytes().to_vec()))
    })
    .await;

    let path = temp_path("xtream", "db");
    let pool = db::init(&path).await.expect("init");
    let provider = upsert_provider_impl(
        &pool,
        ProviderInput {
            id: None,
            name: "X".into(),
            provider_type: ProviderType::Xtream,
            server_url: Some(base.clone()),
            username: Some("u1".into()),
            password: Some("pw1".into()),
            playlist_url: None,
            local_file_path: None,
        },
    )
    .await
    .expect("provider");

    refresh_catalog_impl(&pool, &provider, |_, _| {})
        .await
        .expect("refresh");

    assert_eq!(count(&pool, "live_channels").await, 3);
    assert_eq!(count(&pool, "movies").await, 2);
    assert_eq!(count(&pool, "series").await, 1);
    assert_eq!(count(&pool, "live_categories").await, 2);

    // Category names resolved; unknown category falls back.
    let row = sqlx::query("SELECT category_name, stream_url, stream_ext FROM live_channels WHERE id = '101'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("category_name"), "News");
    // Milestone 21: the password-bearing URL is never persisted; only the id +
    // stream_ext are stored and the URL is composed at playback time.
    assert_eq!(row.get::<String, _>("stream_url"), "");
    assert_eq!(row.get::<String, _>("stream_ext"), "ts");
    let row = sqlx::query("SELECT category_name FROM live_channels WHERE id = '103'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("category_name"), "Uncategorized");

    // Movie metadata: container/year coercion from mixed JSON types.
    let row = sqlx::query("SELECT stream_url, container_ext, release_year FROM movies WHERE id = '201'")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Milestone 21: no password-bearing URL on disk; container_ext is what the
    // playback-time URL composition uses.
    assert_eq!(row.get::<String, _>("stream_url"), "");
    assert_eq!(row.get::<String, _>("container_ext"), "mkv");
    assert_eq!(row.get::<i64, _>("release_year"), 1995);
    let row = sqlx::query("SELECT release_year FROM movies WHERE id = '202'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<i64, _>("release_year"), 2023);

    // FTS queryable.
    let row = sqlx::query("SELECT name FROM fts_series WHERE fts_series MATCH 'breaking'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("name"), "Breaking Code");

    delete_provider_impl(&pool, &provider.id).await.unwrap(); // cleans keychain
    pool.close().await;
    cleanup_db(&path);
}

// --- Failure isolation & catalog replacement ---

#[tokio::test]
async fn refresh_failure_preserves_existing_catalog() {
    let playlist = temp_path("keepcache", "m3u");
    std::fs::write(&playlist, SAMPLE_PLAYLIST).unwrap();

    let path = temp_path("keepcache", "db");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool, None, Some(playlist.to_str().unwrap())).await;

    refresh_catalog_impl(&pool, &provider, |_, _| {})
        .await
        .expect("first refresh");
    let before = db::providers::get(&pool, &provider.id).await.unwrap().unwrap();

    // Repoint the provider at a dead endpoint and refresh again.
    let broken = upsert_provider_impl(
        &pool,
        ProviderInput {
            id: Some(provider.id.clone()),
            name: "M2".into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://127.0.0.1:9/playlist.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .unwrap();
    let err = refresh_catalog_impl(&pool, &broken, |_, _| {})
        .await
        .unwrap_err();
    assert!(err.contains("Could not connect"), "got: {err}");

    // Stale cache untouched, last_refreshed unchanged.
    assert_eq!(count(&pool, "live_channels").await, 4);
    assert_eq!(count(&pool, "movies").await, 2);
    let after = db::providers::get(&pool, &provider.id).await.unwrap().unwrap();
    assert_eq!(after.last_refreshed, before.last_refreshed);

    let _ = std::fs::remove_file(&playlist);
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn second_refresh_replaces_catalog_and_fts() {
    let playlist = temp_path("replace", "m3u");
    std::fs::write(&playlist, SAMPLE_PLAYLIST).unwrap();
    let path = temp_path("replace", "db");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool, None, Some(playlist.to_str().unwrap())).await;

    refresh_catalog_impl(&pool, &provider, |_, _| {}).await.unwrap();
    assert_eq!(count(&pool, "live_channels").await, 4);

    std::fs::write(
        &playlist,
        "#EXTM3U\n#EXTINF:-1 group-title=\"News\",Replacement Channel\nhttp://stream.example/live/9.ts\n",
    )
    .unwrap();
    refresh_catalog_impl(&pool, &provider, |_, _| {}).await.unwrap();

    assert_eq!(count(&pool, "live_channels").await, 1);
    assert_eq!(count(&pool, "movies").await, 0);
    assert_eq!(count(&pool, "episodes").await, 0);

    // FTS reflects the replacement: old names gone, new name found.
    let old = sqlx::query("SELECT name FROM fts_live_channels WHERE fts_live_channels MATCH 'sports'")
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(old.is_none());
    let new = sqlx::query("SELECT name FROM fts_live_channels WHERE fts_live_channels MATCH 'replacement'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(new.get::<String, _>("name"), "Replacement Channel");

    let _ = std::fs::remove_file(&playlist);
    pool.close().await;
    cleanup_db(&path);
}

// --- Staleness & active provider ---

#[test]
fn cache_staleness_logic() {
    let now = 1_000_000_000;
    assert!(is_cache_stale(None, 6, now), "never refreshed is stale");
    assert!(is_cache_stale(Some(now - 7 * 3600), 6, now), "7h old is stale");
    assert!(!is_cache_stale(Some(now - 5 * 3600), 6, now), "5h old is fresh");
    assert!(!is_cache_stale(Some(now), 6, now), "just refreshed is fresh");
}

#[tokio::test]
async fn active_provider_setting_roundtrip_and_delete() {
    let path = temp_path("active", "db");
    let pool = db::init(&path).await.expect("init");

    assert!(get_active_provider_impl(&pool).await.unwrap().is_none());

    let provider = make_m3u_provider(&pool, Some("http://example.com/a.m3u"), None).await;
    set_active_provider_impl(&pool, &provider.id).await.expect("set");
    let active = get_active_provider_impl(&pool).await.unwrap().unwrap();
    assert_eq!(active.id, provider.id);

    let err = set_active_provider_impl(&pool, "nope").await.unwrap_err();
    assert!(err.contains("does not exist"));

    // Deleting the active provider clears the setting.
    delete_provider_impl(&pool, &provider.id).await.unwrap();
    assert!(get_active_provider_impl(&pool).await.unwrap().is_none());

    pool.close().await;
    cleanup_db(&path);
}

// --- Scale: 50k+ items, and cache reads under 500ms ---

#[tokio::test]
async fn refresh_50k_items_completes_and_cache_reads_fast() {
    let playlist = temp_path("50k", "m3u");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&playlist).unwrap());
        writeln!(f, "#EXTM3U").unwrap();
        for i in 0..40_000 {
            writeln!(
                f,
                "#EXTINF:-1 tvg-id=\"ch{i}\" group-title=\"Group {}\",Channel {i}",
                i % 50
            )
            .unwrap();
            writeln!(f, "http://stream.example/live/{i}.ts").unwrap();
        }
        for i in 0..10_000 {
            writeln!(
                f,
                "#EXTINF:-1 group-title=\"VOD | Genre {}\",Movie {i} (20{:02})",
                i % 20,
                i % 30
            )
            .unwrap();
            writeln!(f, "http://stream.example/vod/{i}.mp4").unwrap();
        }
    }

    let path = temp_path("50k", "db");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool, None, Some(playlist.to_str().unwrap())).await;

    let started = std::time::Instant::now();
    refresh_catalog_impl(&pool, &provider, |_, _| {})
        .await
        .expect("refresh");
    let refresh_elapsed = started.elapsed();
    println!("50k refresh took {refresh_elapsed:?}");

    assert_eq!(count(&pool, "live_channels").await, 40_000);
    assert_eq!(count(&pool, "movies").await, 10_000);
    pool.close().await;

    // Simulate app restart: reopen and read the browsing queries from cache.
    // No HTTP server exists for this provider, so by construction these
    // reads make no network request.
    let pool = db::init(&path).await.expect("reopen");
    let started = std::time::Instant::now();
    let summary = db::catalog::summary(&pool, &provider.id).await.unwrap();
    let rows = sqlx::query(
        "SELECT id, name FROM live_channels WHERE provider_id = ? AND category_id = ? ORDER BY name LIMIT 100",
    )
    .bind(&provider.id)
    .bind("Group 7")
    .fetch_all(&pool)
    .await
    .unwrap();
    let read_elapsed = started.elapsed();
    println!("cache read took {read_elapsed:?}");

    assert_eq!(summary.live_channels, 40_000);
    assert_eq!(summary.movies, 10_000);
    assert_eq!(rows.len(), 100);
    assert!(
        read_elapsed.as_millis() < 500,
        "cache load took {read_elapsed:?}, expected < 500ms"
    );

    let _ = std::fs::remove_file(&playlist);
    pool.close().await;
    cleanup_db(&path);
}
