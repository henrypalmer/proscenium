//! Milestone 40 acceptance tests. Slice 1 (canonical browse): Cinemeta response
//! parsing (movies, series + episode mapping, non-IMDB filtering, loose types)
//! and the Tier-2 cache — fresh hit, stale-on-failure fallback (offline browse),
//! and hard failure when nothing is cached.

use proscenium_lib::canonical::cinemeta;
use proscenium_lib::commands::canonical::cached_or_fetch;
use proscenium_lib::db;
use proscenium_lib::db::canonical::{cache_get, cache_put};
use serde_json::json;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m40-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// --- parsing (pure, no network) ---

#[test]
fn catalog_parses_imdb_items_and_skips_non_imdb() {
    let body = json!({
        "metas": [
            { "id": "tt0133093", "type": "movie", "name": "The Matrix",
              "poster": "https://img/p.jpg", "year": "1999" },
            // Non-IMDB ids (kitsu/etc.) are not canonical → dropped.
            { "id": "kitsu:1376", "type": "movie", "name": "Anime Thing" },
            // Series with a release *range* — the leading year is taken.
            { "id": "tt0944947", "type": "series", "name": "Game of Thrones",
              "releaseInfo": "2011\u{2013}2019" },
        ]
    });
    let items = cinemeta::parse_catalog(&body);
    assert_eq!(items.len(), 2, "the kitsu entry must be skipped");
    assert_eq!(items[0].imdb_id, "tt0133093");
    assert_eq!(items[0].kind, "movie");
    assert_eq!(items[0].release_year, Some(1999));
    assert_eq!(items[0].poster_url.as_deref(), Some("https://img/p.jpg"));
    assert_eq!(items[1].imdb_id, "tt0944947");
    assert_eq!(items[1].kind, "series");
    assert_eq!(items[1].release_year, Some(2011), "range → leading year");
}

#[test]
fn movie_meta_parses_rating_tmdb_bridge_and_backdrop() {
    let meta = json!({
        "id": "tt0133093", "type": "movie", "name": "The Matrix",
        "poster": "https://img/p.jpg", "background": "https://img/bg.jpg",
        "description": "Neo learns the truth.", "releaseInfo": "1999", "year": "1999",
        "genres": ["Action", "Sci-Fi"], "imdbRating": "8.7", "runtime": "136 min",
        "moviedb_id": 603, "cast": ["Keanu Reeves"], "director": ["Lana Wachowski"]
    });
    let m = cinemeta::parse_meta(&meta, "movie");
    assert_eq!(m.imdb_id, "tt0133093");
    assert_eq!(m.release_year, Some(1999));
    assert_eq!(m.genres, vec!["Action", "Sci-Fi"]);
    assert_eq!(m.imdb_rating, Some(8.7));
    assert_eq!(m.runtime.as_deref(), Some("136 min"));
    // The tmdb↔imdb bridge: Cinemeta's moviedb_id is the provider match anchor.
    assert_eq!(m.tmdb_id, Some(603));
    assert_eq!(m.backdrop_url.as_deref(), Some("https://img/bg.jpg"));
    assert_eq!(m.cast, vec!["Keanu Reeves"]);
    assert!(m.videos.is_empty(), "movies have no episode list");
}

#[test]
fn series_meta_sorts_episodes_keeps_specials_and_falls_back_to_number() {
    let meta = json!({
        "id": "tt0944947", "type": "series", "name": "Game of Thrones",
        // Deliberately out of order; one entry uses `number` (no `episode`),
        // one is a season-0 special, plus an empty imdbRating string.
        "imdbRating": "",
        "videos": [
            { "id": "tt0944947:2:1", "season": 2, "episode": 1, "name": "Valar Dohaeris" },
            { "id": "tt0944947:1:1", "season": 1, "episode": 1, "name": "Winter Is Coming",
              "overview": "Ned." },
            { "id": "tt0944947:0:1", "season": 0, "number": 1, "name": "Inside GoT" },
        ]
    });
    let s = cinemeta::parse_meta(&meta, "series");
    assert_eq!(s.imdb_rating, None, "empty imdbRating string → None");
    let order: Vec<(i64, i64)> = s.videos.iter().map(|v| (v.season, v.episode)).collect();
    assert_eq!(order, vec![(0, 1), (1, 1), (2, 1)], "sorted; specials kept");
    assert_eq!(s.videos[0].episode, 1, "season-0 episode from `number` fallback");
    assert_eq!(s.videos[1].overview.as_deref(), Some("Ned."));
}

#[test]
fn genres_include_series_only_buckets() {
    let movie = cinemeta::genres("movie");
    let series = cinemeta::genres("series");
    assert!(movie.contains(&"Sci-Fi".to_string()));
    assert!(!movie.contains(&"Reality-TV".to_string()));
    assert!(series.contains(&"Reality-TV".to_string()), "series-only bucket");
}

// --- Tier-2 cache + cached_or_fetch (DB-backed, no network) ---

#[tokio::test]
async fn cache_put_get_roundtrips_and_upserts() {
    let path = temp_path("cache");
    let pool: SqlitePool = db::init(&path).await.expect("init");

    cache_put(&pool, "k", "v1", 100, 200).await.expect("put");
    let got = cache_get(&pool, "k").await.expect("get").expect("present");
    assert_eq!(got.body, "v1");
    assert_eq!(got.expires_at, 200);

    cache_put(&pool, "k", "v2", 300, 400).await.expect("put2");
    let got = cache_get(&pool, "k").await.expect("get2").expect("present");
    assert_eq!(got.body, "v2", "ON CONFLICT overwrites");
    assert_eq!(got.expires_at, 400);

    assert!(cache_get(&pool, "missing").await.expect("get3").is_none());
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn cached_or_fetch_fresh_hit_skips_the_fetch() {
    let path = temp_path("fresh");
    let pool = db::init(&path).await.expect("init");
    // A fresh row (expires in the future) holding ["cached"].
    let body = serde_json::to_string(&vec!["cached".to_string()]).unwrap();
    cache_put(&pool, "k", &body, now(), now() + 1000).await.unwrap();

    let r: Result<Vec<String>, String> = cached_or_fetch(&pool, "k", 1000, || async {
        // Must not run on a fresh hit.
        Err::<Vec<String>, String>("fetch should not be called".into())
    })
    .await;
    assert_eq!(r.unwrap(), vec!["cached".to_string()]);
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn cached_or_fetch_serves_stale_when_fetch_fails() {
    let path = temp_path("stale");
    let pool = db::init(&path).await.expect("init");
    // A stale row (already expired) — the offline fallback.
    let body = serde_json::to_string(&vec!["stale".to_string()]).unwrap();
    cache_put(&pool, "k", &body, now() - 5000, now() - 10).await.unwrap();

    let r: Result<Vec<String>, String> = cached_or_fetch(&pool, "k", 1000, || async {
        Err::<Vec<String>, String>("cinemeta down".into())
    })
    .await;
    assert_eq!(r.unwrap(), vec!["stale".to_string()], "stale served on failure");
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn cached_or_fetch_errors_with_no_cache_and_caches_on_success() {
    let path = temp_path("miss");
    let pool = db::init(&path).await.expect("init");

    // No row + failing fetch → propagate the error.
    let err: Result<Vec<String>, String> = cached_or_fetch(&pool, "k", 1000, || async {
        Err::<Vec<String>, String>("cinemeta down".into())
    })
    .await;
    assert_eq!(err.unwrap_err(), "cinemeta down");

    // Successful fetch returns the value and writes a fresh cache row.
    let ok: Result<Vec<String>, String> = cached_or_fetch(&pool, "k", 1000, || async {
        Ok::<Vec<String>, String>(vec!["fresh".to_string()])
    })
    .await;
    assert_eq!(ok.unwrap(), vec!["fresh".to_string()]);
    let cached = cache_get(&pool, "k").await.unwrap().expect("now cached");
    assert!(cached.expires_at > now(), "written with a future expiry");
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&cached.body).unwrap(),
        vec!["fresh".to_string()]
    );
    pool.close().await;
    cleanup_db(&path);
}
