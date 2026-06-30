//! Milestone 5 acceptance tests: VOD browsing (movies/series pagination and
//! genre filtering), empty-genre hiding, episode grouping by season, the
//! on-demand Xtream `get_series_info` / `get_vod_info` fetches with session
//! caching, graceful metadata degradation, and stream URL resolution for
//! movies and episodes.

use proscenium_lib::commands::catalog::{
    get_episodes_impl, get_movie_detail_impl, get_series_detail_impl, DetailCache,
};
use proscenium_lib::commands::playback::resolve_stream_url_impl;
use proscenium_lib::commands::providers::{delete_provider_impl, upsert_provider_impl};
use proscenium_lib::db;
use proscenium_lib::models::{
    CatalogData, Category, EpisodeItem, MovieItem, Provider, ProviderInput, ProviderType,
    SeriesItem,
};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m5-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

async fn make_m3u_provider(pool: &SqlitePool) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M5 m3u".into(),
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

async fn make_xtream_provider(pool: &SqlitePool, base: &str) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: "M5 xtream".into(),
            provider_type: ProviderType::Xtream,
            server_url: Some(base.to_string()),
            username: Some("u1".into()),
            password: Some("pw1".into()),
            playlist_url: None,
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

fn movie(id: &str, name: &str, category: &str) -> MovieItem {
    MovieItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        poster_url: None,
        stream_url: format!("http://stream.local/movie/{id}.mp4"),
        container_ext: "mp4".into(),
        release_year: Some(2020),
        rating: None,
        added_at: None,
    }
}

fn series(id: &str, name: &str, category: &str) -> SeriesItem {
    SeriesItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: category.into(),
        category_name: category.into(),
        poster_url: None,
        release_year: Some(2019),
    }
}

fn episode(id: &str, series_id: &str, season: i64, ep: i64) -> EpisodeItem {
    EpisodeItem {
        id: id.into(),
        provider_id: String::new(),
        series_id: series_id.into(),
        season,
        episode: ep,
        title: format!("S{season:02}E{ep:02}"),
        stream_url: format!("http://stream.local/series/{id}.mp4"),
        container_ext: "mp4".into(),
        duration_seconds: None,
        poster_url: None,
        overview: None,
    }
}

fn category(id: &str, name: &str, order: i64) -> Category {
    Category {
        id: id.into(),
        name: name.into(),
        sort_order: order,
    }
}

/// Minimal mock HTTP server (same shape as the milestone2 one); the handler
/// maps a request target (path + query) to (content_type, body).
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

// --- Movies: pagination, ordering, genre filter ---

#[tokio::test]
async fn movies_paginate_alphabetically_and_filter_by_genre() {
    let path = temp_path("movies");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool).await;

    let data = CatalogData {
        vod_categories: vec![category("act", "Action", 0), category("dra", "Drama", 1)],
        movies: vec![
            movie("m1", "zebra heist", "act"),
            movie("m2", "Alpha Strike", "act"),
            movie("m3", "Quiet River", "dra"),
            movie("m4", "bravo Run", "act"),
        ],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    // All movies, alphabetical case-insensitive.
    let page = db::catalog::movies_page(&pool, &provider.id, None, 1, 10)
        .await
        .expect("page");
    assert_eq!(page.total, 4);
    let names: Vec<&str> = page.items.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Alpha Strike", "bravo Run", "Quiet River", "zebra heist"]);

    // Genre filter.
    let action = db::catalog::movies_page(&pool, &provider.id, Some("act"), 1, 10)
        .await
        .expect("filtered");
    assert_eq!(action.total, 3);
    assert!(action.items.iter().all(|m| m.category_id == "act"));

    // Pagination: page 2 of size 3 holds the single remaining item.
    let page2 = db::catalog::movies_page(&pool, &provider.id, None, 2, 3)
        .await
        .expect("page2");
    assert_eq!(page2.total, 4);
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].name, "zebra heist");

    // Out-of-range page: empty items, correct total.
    let beyond = db::catalog::movies_page(&pool, &provider.id, None, 99, 10)
        .await
        .expect("beyond");
    assert!(beyond.items.is_empty());
    assert_eq!(beyond.total, 4);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn series_paginate_and_filter_by_genre() {
    let path = temp_path("series");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool).await;

    let data = CatalogData {
        series_categories: vec![category("cri", "Crime", 0), category("sci", "Sci-Fi", 1)],
        series: vec![
            series("s1", "Night Watch", "cri"),
            series("s2", "breaking code", "cri"),
            series("s3", "Star Drift", "sci"),
        ],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let page = db::catalog::series_page(&pool, &provider.id, None, 1, 10)
        .await
        .expect("page");
    assert_eq!(page.total, 3);
    let names: Vec<&str> = page.items.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["breaking code", "Night Watch", "Star Drift"]);

    let crime = db::catalog::series_page(&pool, &provider.id, Some("cri"), 1, 10)
        .await
        .expect("filtered");
    assert_eq!(crime.total, 2);
    assert!(crime.items.iter().all(|s| s.category_id == "cri"));

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Genre lists hide empty categories (spec §12) ---

#[tokio::test]
async fn vod_and_series_genres_hide_empty_categories() {
    let path = temp_path("genres");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool).await;

    let data = CatalogData {
        vod_categories: vec![
            category("act", "Action", 0),
            category("emp", "Empty Genre", 1),
            category("dra", "Drama", 2),
        ],
        movies: vec![movie("m1", "A", "act"), movie("m2", "B", "dra")],
        series_categories: vec![category("cri", "Crime", 0), category("non", "Nothing Here", 1)],
        series: vec![series("s1", "Show", "cri")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    // Milestone 39: merged categories are keyed by name (the same genre across
    // providers collapses); empty genres are still hidden (spec §12).
    let vod = db::catalog::vod_categories(&pool, &provider.id).await.expect("vod");
    let ids: Vec<&str> = vod.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["Action", "Drama"]);

    let ser = db::catalog::series_categories(&pool, &provider.id).await.expect("series");
    let ids: Vec<&str> = ser.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["Crime"]);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Episodes grouped by season (cached/M3U path) ---

#[tokio::test]
async fn cached_episodes_group_by_season_in_order() {
    let path = temp_path("episodes");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool).await;

    let data = CatalogData {
        series_categories: vec![category("cri", "Crime", 0)],
        series: vec![series("s1", "Night Watch", "cri")],
        episodes: vec![
            episode("e3", "s1", 2, 1),
            episode("e1", "s1", 1, 2),
            episode("e2", "s1", 1, 1),
        ],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let grouped = get_episodes_impl(&pool, &provider.id, "s1").await.expect("episodes");
    assert_eq!(grouped.keys().copied().collect::<Vec<_>>(), [1, 2]);
    let season1: Vec<i64> = grouped[&1].iter().map(|e| e.episode).collect();
    assert_eq!(season1, [1, 2]); // episode order within the season
    assert_eq!(grouped[&2].len(), 1);

    // M3U providers have no on-demand endpoint: a series without episodes
    // returns an empty map rather than an error.
    let data2 = CatalogData {
        series_categories: vec![category("cri", "Crime", 0)],
        series: vec![series("s9", "No Episodes", "cri")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data2, 1_700_000_001)
        .await
        .expect("seed2");
    let empty = get_episodes_impl(&pool, &provider.id, "s9").await.expect("empty");
    assert!(empty.is_empty());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- On-demand Xtream series_info: fetch, persist, group ---

#[tokio::test]
async fn xtream_episodes_fetched_on_demand_and_persisted() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_in_handler = hits.clone();
    let base = spawn_server(move |target| {
        if !target.contains("action=get_series_info") || !target.contains("series_id=301") {
            return None;
        }
        hits_in_handler.fetch_add(1, Ordering::SeqCst);
        let body = r#"{
            "info": {"name":"Breaking Code","plot":"A coder breaks bad.","genre":"Crime, Thriller"},
            "episodes": {
                "1": [
                    {"id":"5001","episode_num":1,"title":"Pilot","container_extension":"mkv","season":1,"info":{"duration_secs":"1800","movie_image":"http://img/e1.jpg"}},
                    {"id":"5002","episode_num":"2","title":"","container_extension":"","season":1,"info":{}}
                ],
                "2": [
                    {"id":"5003","episode_num":1,"title":"Rebooted","container_extension":"mp4","season":2,"info":{"duration_secs":2400}}
                ]
            }
        }"#;
        Some(("application/json", body.as_bytes().to_vec()))
    })
    .await;

    let path = temp_path("xtream-episodes");
    let pool = db::init(&path).await.expect("init");
    let provider = make_xtream_provider(&pool, &base).await;

    let data = CatalogData {
        series_categories: vec![category("30", "Crime", 0)],
        series: vec![series("301", "Breaking Code", "30")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let grouped = get_episodes_impl(&pool, &provider.id, "301").await.expect("episodes");
    assert_eq!(grouped.keys().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(grouped[&1].len(), 2);
    assert_eq!(grouped[&2].len(), 1);

    let pilot = &grouped[&1][0];
    assert_eq!(pilot.title, "Pilot");
    // Milestone 21: the password-bearing URL is no longer persisted; the catalog
    // row carries only the id + container_ext, and the playable URL is composed
    // at playback time from the keychain secret.
    assert_eq!(pilot.stream_url, "");
    let resolved = resolve_stream_url_impl(&pool, &provider.id, "episode", &pilot.id)
        .await
        .expect("resolve episode");
    assert_eq!(resolved, format!("{base}/series/u1/pw1/5001.mkv"));
    assert_eq!(pilot.duration_seconds, Some(1800));
    assert_eq!(pilot.poster_url.as_deref(), Some("http://img/e1.jpg"));
    // Missing title/extension fall back.
    assert_eq!(grouped[&1][1].title, "Episode 2");
    assert_eq!(grouped[&1][1].container_ext, "mp4");

    // Persisted: the second call is served from SQLite, no extra request.
    let again = get_episodes_impl(&pool, &provider.id, "301").await.expect("again");
    assert_eq!(again[&1].len(), 2);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let n: i64 = sqlx::query("SELECT COUNT(*) AS n FROM episodes WHERE series_id = '301'")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(n, 3);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Movie detail: vod_info enrichment with session cache ---

#[tokio::test]
async fn movie_detail_enriched_from_vod_info_and_session_cached() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_in_handler = hits.clone();
    let base = spawn_server(move |target| {
        if !target.contains("action=get_vod_info") || !target.contains("vod_id=201") {
            return None;
        }
        hits_in_handler.fetch_add(1, Ordering::SeqCst);
        let body = r#"{
            "info": {"plot":"Two crews collide in LA.","genre":"Crime, Drama","duration_secs":"10260"},
            "movie_data": {"stream_id":201,"name":"Heat"}
        }"#;
        Some(("application/json", body.as_bytes().to_vec()))
    })
    .await;

    let path = temp_path("movie-detail");
    let pool = db::init(&path).await.expect("init");
    let provider = make_xtream_provider(&pool, &base).await;

    let data = CatalogData {
        vod_categories: vec![category("20", "Action", 0)],
        movies: vec![movie("201", "Heat", "20")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let cache = DetailCache::default();
    let detail = get_movie_detail_impl(&pool, &cache, &provider.id, "201")
        .await
        .expect("detail");
    assert_eq!(detail.movie.name, "Heat");
    assert_eq!(detail.description.as_deref(), Some("Two crews collide in LA."));
    assert_eq!(detail.genre.as_deref(), Some("Crime, Drama"));
    assert_eq!(detail.duration_seconds, Some(10260));

    // Session cache: a second open does not re-fetch.
    let again = get_movie_detail_impl(&pool, &cache, &provider.id, "201")
        .await
        .expect("again");
    assert_eq!(again.description.as_deref(), Some("Two crews collide in LA."));
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Unknown movie id is an error.
    assert!(get_movie_detail_impl(&pool, &cache, &provider.id, "999")
        .await
        .is_err());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn movie_detail_degrades_gracefully_without_metadata() {
    // Server 404s every vod_info request: detail still returns the base row.
    let base = spawn_server(|_| None).await;

    let path = temp_path("movie-detail-fallback");
    let pool = db::init(&path).await.expect("init");
    let xtream = make_xtream_provider(&pool, &base).await;
    let m3u = make_m3u_provider(&pool).await;

    let data = CatalogData {
        vod_categories: vec![category("20", "Action", 0)],
        movies: vec![movie("201", "Heat", "20")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &xtream.id, &data, 1_700_000_000)
        .await
        .expect("seed xtream");
    db::catalog::replace_catalog(&pool, &m3u.id, &data, 1_700_000_000)
        .await
        .expect("seed m3u");

    let cache = DetailCache::default();
    let detail = get_movie_detail_impl(&pool, &cache, &xtream.id, "201")
        .await
        .expect("xtream fallback");
    assert_eq!(detail.movie.name, "Heat");
    assert!(detail.description.is_none());

    // M3U providers have no vod_info endpoint at all.
    let detail = get_movie_detail_impl(&pool, &cache, &m3u.id, "201")
        .await
        .expect("m3u detail");
    assert_eq!(detail.movie.name, "Heat");
    assert!(detail.description.is_none());

    delete_provider_impl(&pool, &xtream.id).await.unwrap();
    delete_provider_impl(&pool, &m3u.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Detail hero backdrop selection (Milestone 18) ---

#[tokio::test]
async fn movie_detail_backdrop_prefers_backdrop_path_then_falls_back() {
    // Three movies exercise the selection order: backdrop_path array → cover_big
    // → none. The handler keys off the vod_id in the request target.
    let base = spawn_server(move |target| {
        if !target.contains("action=get_vod_info") {
            return None;
        }
        let body: &str = if target.contains("vod_id=201") {
            r#"{"info":{"backdrop_path":["http://img/bd1.jpg","http://img/bd2.jpg"],"cover_big":"http://img/big.jpg"}}"#
        } else if target.contains("vod_id=202") {
            r#"{"info":{"backdrop_path":[],"cover_big":"http://img/big.jpg"}}"#
        } else {
            r#"{"info":{"plot":"No art here."}}"#
        };
        Some(("application/json", body.as_bytes().to_vec()))
    })
    .await;

    let path = temp_path("movie-backdrop");
    let pool = db::init(&path).await.expect("init");
    let provider = make_xtream_provider(&pool, &base).await;

    let data = CatalogData {
        vod_categories: vec![category("20", "Action", 0)],
        movies: vec![
            movie("201", "Array", "20"),
            movie("202", "Fallback", "20"),
            movie("203", "Bare", "20"),
        ],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let cache = DetailCache::default();
    // First non-empty backdrop_path entry wins.
    let d201 = get_movie_detail_impl(&pool, &cache, &provider.id, "201")
        .await
        .expect("201");
    assert_eq!(d201.backdrop_url.as_deref(), Some("http://img/bd1.jpg"));
    // Empty backdrop_path → cover_big fallback.
    let d202 = get_movie_detail_impl(&pool, &cache, &provider.id, "202")
        .await
        .expect("202");
    assert_eq!(d202.backdrop_url.as_deref(), Some("http://img/big.jpg"));
    // Neither present → null (poster fallback handled on the frontend).
    let d203 = get_movie_detail_impl(&pool, &cache, &provider.id, "203")
        .await
        .expect("203");
    assert!(d203.backdrop_url.is_none());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn series_detail_backdrop_falls_back_to_cover() {
    let base = spawn_server(move |target| {
        if !target.contains("action=get_series_info") {
            return None;
        }
        // No backdrop_path; the series fallback key is `cover`.
        let body = r#"{"info":{"name":"Show","plot":"x","cover":"http://img/cover.jpg"},"episodes":{}}"#;
        Some(("application/json", body.as_bytes().to_vec()))
    })
    .await;

    let path = temp_path("series-backdrop");
    let pool = db::init(&path).await.expect("init");
    let provider = make_xtream_provider(&pool, &base).await;

    let data = CatalogData {
        series_categories: vec![category("30", "Crime", 0)],
        series: vec![series("301", "Show", "30")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let cache = DetailCache::default();
    let detail = get_series_detail_impl(&pool, &cache, &provider.id, "301")
        .await
        .expect("detail");
    assert_eq!(detail.backdrop_url.as_deref(), Some("http://img/cover.jpg"));

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Series detail: metadata + episode persistence in one fetch ---

#[tokio::test]
async fn series_detail_enriches_and_persists_episodes() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_in_handler = hits.clone();
    let base = spawn_server(move |target| {
        if !target.contains("action=get_series_info") {
            return None;
        }
        hits_in_handler.fetch_add(1, Ordering::SeqCst);
        let body = r#"{
            "info": {"name":"Breaking Code","plot":"A coder breaks bad.","genre":"Crime"},
            "episodes": {
                "1": [
                    {"id":"5001","episode_num":1,"title":"Pilot","container_extension":"mkv","season":1,"info":{"plot":"He cooks."}},
                    {"id":"5002","episode_num":2,"title":"Cat's in the Bag","container_extension":"mkv","season":1,"info":{"overview":"Cleanup duty."}}
                ]
            }
        }"#;
        Some(("application/json", body.as_bytes().to_vec()))
    })
    .await;

    let path = temp_path("series-detail");
    let pool = db::init(&path).await.expect("init");
    let provider = make_xtream_provider(&pool, &base).await;

    let data = CatalogData {
        series_categories: vec![category("30", "Crime", 0)],
        series: vec![series("301", "Breaking Code", "30")],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let cache = DetailCache::default();
    let detail = get_series_detail_impl(&pool, &cache, &provider.id, "301")
        .await
        .expect("detail");
    assert_eq!(detail.series.name, "Breaking Code");
    assert_eq!(detail.description.as_deref(), Some("A coder breaks bad."));
    assert_eq!(detail.genre.as_deref(), Some("Crime"));

    // The detail fetch persisted the episodes: get_episodes is served from
    // SQLite and the session cache absorbs a repeat detail open.
    let grouped = get_episodes_impl(&pool, &provider.id, "301").await.expect("episodes");
    assert_eq!(grouped[&1].len(), 2);
    // Episode synopsis (M20 §5.4) is parsed and persisted: `plot` is preferred,
    // `overview` is the fallback when `plot` is absent.
    assert_eq!(grouped[&1][0].overview.as_deref(), Some("He cooks."));
    assert_eq!(grouped[&1][1].overview.as_deref(), Some("Cleanup duty."));
    let _ = get_series_detail_impl(&pool, &cache, &provider.id, "301")
        .await
        .expect("again");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}

// --- Stream URL resolution for VOD content (play buttons) ---

#[tokio::test]
async fn resolve_stream_url_for_movie_and_episode() {
    let path = temp_path("resolve");
    let pool = db::init(&path).await.expect("init");
    let provider = make_m3u_provider(&pool).await;

    let data = CatalogData {
        vod_categories: vec![category("20", "Action", 0)],
        movies: vec![movie("m1", "Heat", "20")],
        series_categories: vec![category("30", "Crime", 0)],
        series: vec![series("s1", "Night Watch", "30")],
        episodes: vec![episode("e1", "s1", 1, 1)],
        ..Default::default()
    };
    db::catalog::replace_catalog(&pool, &provider.id, &data, 1_700_000_000)
        .await
        .expect("seed");

    let url = resolve_stream_url_impl(&pool, &provider.id, "movie", "m1")
        .await
        .expect("movie url");
    assert_eq!(url, "http://stream.local/movie/m1.mp4");

    let url = resolve_stream_url_impl(&pool, &provider.id, "episode", "e1")
        .await
        .expect("episode url");
    assert_eq!(url, "http://stream.local/series/e1.mp4");

    assert!(resolve_stream_url_impl(&pool, &provider.id, "movie", "nope")
        .await
        .is_err());

    delete_provider_impl(&pool, &provider.id).await.unwrap();
    pool.close().await;
    cleanup_db(&path);
}
