//! Milestone 40 acceptance tests. Slice 1 (canonical browse): Cinemeta response
//! parsing (movies, series + episode mapping, non-IMDB filtering, loose types)
//! and the Tier-2 cache — fresh hit, stale-on-failure fallback (offline browse),
//! and hard failure when nothing is cached.

use proscenium_lib::canonical::cinemeta;
use proscenium_lib::canonical::resolver::{
    classify_match, parse_quality, resolve_sources, title_similarity, year_ok, CanonicalRef,
};
use proscenium_lib::commands::canonical::cached_or_fetch;
use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::db::canonical::{
    cache_get, cache_put, match_get, match_put, matches_for_imdb, set_manual_match, ContentMatch,
};
use proscenium_lib::iptv::xtream;
use proscenium_lib::models::{
    CatalogData, EpisodeItem, MovieItem, Provider, ProviderInput, ProviderType, SeriesItem,
};
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

// --- Slice 2: resolver registry + content_match index ---

async fn m3u_provider(pool: &SqlitePool, name: &str) -> Provider {
    upsert_provider_impl(
        pool,
        ProviderInput {
            id: None,
            name: name.into(),
            provider_type: ProviderType::M3u,
            server_url: None,
            username: None,
            password: None,
            playlist_url: Some("http://example.local/p.m3u".into()),
            local_file_path: None,
        },
    )
    .await
    .expect("provider")
}

fn movie_row(id: &str, name: &str, year: i64) -> MovieItem {
    MovieItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: "Movies".into(),
        category_name: "Movies".into(),
        poster_url: None,
        stream_url: "http://example.local/x.mp4".into(),
        container_ext: "mp4".into(),
        release_year: Some(year),
        rating: None,
        added_at: None,
    }
}

async fn seed_movies(pool: &SqlitePool, provider_id: &str, movies: Vec<MovieItem>) {
    let data = CatalogData {
        movies,
        ..Default::default()
    };
    db::catalog::replace_catalog(pool, provider_id, &data, 0)
        .await
        .expect("seed");
}

#[test]
fn classify_match_tmdb_decisive_then_name_year_fallback() {
    // Both tmdb present + equal → decisive accept.
    assert_eq!(classify_match(Some(603), Some(603), 0.2, true), Some((1.0, "tmdb")));
    // Both tmdb present + differ → reject even with a perfect name (kills remakes).
    assert_eq!(classify_match(Some(603), Some(604), 1.0, true), None);
    // No provider tmdb → name+year fallback, medium confidence below 1.0.
    let m = classify_match(None, Some(603), 1.0, true).expect("name_year");
    assert_eq!(m.1, "name_year");
    assert!(m.0 < 1.0 && m.0 >= 0.6);
    // Weak name → reject; wrong year → reject.
    assert_eq!(classify_match(None, None, 0.3, true), None);
    assert_eq!(classify_match(None, None, 1.0, false), None);
}

#[test]
fn title_similarity_strips_noise_and_year_tolerance() {
    assert!(title_similarity("The Matrix (1999) 1080p", "Matrix") >= 0.9);
    assert!(title_similarity("Inception", "The Dark Knight") < 0.3);
    assert!(year_ok(Some(1999), Some(2000))); // within ±1
    assert!(!year_ok(Some(1999), Some(2005)));
    assert!(year_ok(None, Some(2000))); // missing candidate year can't disqualify
}

#[test]
fn parse_quality_reads_resolution_tags() {
    assert_eq!(parse_quality("Dune Part Two 2160p"), Some("2160p".into()));
    assert_eq!(parse_quality("Movie [4K]"), Some("2160p".into()));
    assert_eq!(parse_quality("Movie 1080p"), Some("1080p".into()));
    assert_eq!(parse_quality("Movie SD"), None);
}

#[test]
fn vod_info_parses_tmdb_id() {
    // Providers send tmdb_id as a string or a number; both parse.
    let s = json!({ "info": { "tmdb_id": "603", "plot": "x", "genre": "Sci-Fi" } });
    assert_eq!(xtream::parse_vod_info(&s).tmdb_id, Some(603));
    let n = json!({ "info": { "tmdb_id": 603 } });
    assert_eq!(xtream::parse_vod_info(&n).tmdb_id, Some(603));
    let none = json!({ "info": { "plot": "y" } });
    assert_eq!(xtream::parse_vod_info(&none).tmdb_id, None);
}

#[tokio::test]
async fn content_match_survives_catalog_refresh() {
    let path = temp_path("match-refresh");
    let pool = db::init(&path).await.expect("init");
    let p = m3u_provider(&pool, "P").await;
    seed_movies(&pool, &p.id, vec![movie_row("m1", "The Matrix", 1999)]).await;

    match_put(
        &pool,
        &ContentMatch {
            provider_id: p.id.clone(),
            content_type: "movie".into(),
            content_id: "m1".into(),
            imdb_id: "tt0133093".into(),
            tmdb_id: Some(603),
            confidence: 1.0,
            method: "tmdb".into(),
            matched_at: 1,
        },
    )
    .await
    .unwrap();

    // A refresh deletes + reinserts catalog rows by their stable ids…
    seed_movies(&pool, &p.id, vec![movie_row("m1", "The Matrix", 1999)]).await;

    // …but the match in the side table survives (it is not catalog-scoped).
    let got = match_get(&pool, &p.id, "movie", "m1")
        .await
        .unwrap()
        .expect("match survives refresh");
    assert_eq!(got.imdb_id, "tt0133093");
    let rev = matches_for_imdb(&pool, "tt0133093", "movie", std::slice::from_ref(&p.id))
        .await
        .unwrap();
    assert_eq!(rev.len(), 1);
    assert_eq!(rev[0].content_id, "m1");
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn resolve_sources_matches_by_name_year_across_providers() {
    let path = temp_path("resolve");
    let pool = db::init(&path).await.expect("init");
    let a = m3u_provider(&pool, "Provider A").await;
    let b = m3u_provider(&pool, "Provider B").await;
    // Both providers carry The Matrix (right year); A also has a year-decoy.
    seed_movies(
        &pool,
        &a.id,
        vec![
            movie_row("a1", "The Matrix 1080p", 1999),
            movie_row("a2", "The Matrix", 2030),
        ],
    )
    .await;
    seed_movies(&pool, &b.id, vec![movie_row("b1", "Matrix, The", 2000)]).await;

    let target = CanonicalRef {
        imdb_id: "tt0133093".into(),
        kind: "movie".into(),
        tmdb_id: Some(603),
        name: "The Matrix".into(),
        year: Some(1999),
        season: None,
        episode: None,
    };
    let sources = resolve_sources(&pool, &target, &[a.clone(), b.clone()]).await;

    // One candidate per provider's matching item; the wrong-year decoy excluded.
    let ids: Vec<(Option<String>, Option<String>)> = sources
        .iter()
        .map(|c| (c.provider_id.clone(), c.content_id.clone()))
        .collect();
    assert_eq!(sources.len(), 2, "Matrix from each provider, not the decoy");
    assert!(ids.contains(&(Some(a.id.clone()), Some("a1".into()))));
    assert!(ids.contains(&(Some(b.id.clone()), Some("b1".into()))));
    assert!(sources.iter().all(|c| c.content_id.as_deref() != Some("a2")));
    // Quality parsed from the title; provider-source addressing (no direct URL).
    let a1 = sources
        .iter()
        .find(|c| c.content_id.as_deref() == Some("a1"))
        .unwrap();
    assert_eq!(a1.quality.as_deref(), Some("1080p"));
    assert!(a1.url.is_none());

    // The match was recorded — a second resolve is a cheap index read.
    assert!(match_get(&pool, &a.id, "movie", "a1").await.unwrap().is_some());
    pool.close().await;
    cleanup_db(&path);
}

// --- Slice 4: series matching + episode mapping + manual override ---

fn series_row(id: &str, name: &str, year: i64) -> SeriesItem {
    SeriesItem {
        id: id.into(),
        provider_id: String::new(),
        name: name.into(),
        category_id: "Series".into(),
        category_name: "Series".into(),
        poster_url: None,
        release_year: Some(year),
    }
}

fn episode_row(id: &str, series_id: &str, season: i64, episode: i64) -> EpisodeItem {
    EpisodeItem {
        id: id.into(),
        provider_id: String::new(),
        series_id: series_id.into(),
        season,
        episode,
        title: format!("S{season}E{episode}"),
        stream_url: "http://example.local/e.mp4".into(),
        container_ext: "mkv".into(),
        duration_seconds: None,
        poster_url: None,
        overview: None,
    }
}

async fn seed_series_multi(
    pool: &SqlitePool,
    provider_id: &str,
    series: Vec<SeriesItem>,
    eps: Vec<(&str, Vec<EpisodeItem>)>,
) {
    let data = CatalogData {
        series,
        ..Default::default()
    };
    db::catalog::replace_catalog(pool, provider_id, &data, 0)
        .await
        .expect("seed series");
    for (sid, episodes) in eps {
        db::catalog::replace_series_episodes(pool, provider_id, sid, &episodes)
            .await
            .expect("seed episodes");
    }
}

fn series_target(season: i64, episode: i64) -> CanonicalRef {
    CanonicalRef {
        imdb_id: "tt0903747".into(),
        kind: "series".into(),
        tmdb_id: None,
        name: "Breaking Bad".into(),
        year: Some(2008),
        season: Some(season),
        episode: Some(episode),
    }
}

#[tokio::test]
async fn resolve_series_maps_canonical_episode_to_provider_episode() {
    let path = temp_path("series-map");
    let pool = db::init(&path).await.expect("init");
    let p = m3u_provider(&pool, "P").await;
    seed_series_multi(
        &pool,
        &p.id,
        vec![series_row("s1", "Breaking Bad", 2008)],
        vec![(
            "s1",
            vec![
                episode_row("e11", "s1", 1, 1),
                episode_row("e12", "s1", 1, 2),
                episode_row("e21", "s1", 2, 1),
            ],
        )],
    )
    .await;

    // Canonical S1:E2 → the provider's S1E2 episode id.
    let s1e2 = resolve_sources(&pool, &series_target(1, 2), &[p.clone()]).await;
    assert_eq!(s1e2.len(), 1);
    assert_eq!(s1e2[0].content_type, "episode");
    assert_eq!(s1e2[0].content_id.as_deref(), Some("e12"));
    assert_eq!(s1e2[0].provider_id.as_deref(), Some(p.id.as_str()));
    // The series-level match was recorded (name+year, no tmdb backstop).
    let m = match_get(&pool, &p.id, "series", "s1").await.unwrap().expect("series match");
    assert_eq!(m.method, "name_year");

    // A different canonical episode maps to a different provider episode.
    let s2e1 = resolve_sources(&pool, &series_target(2, 1), &[p.clone()]).await;
    assert_eq!(s2e1[0].content_id.as_deref(), Some("e21"));

    // An episode the provider doesn't carry yields no source.
    let missing = resolve_sources(&pool, &series_target(9, 9), &[p.clone()]).await;
    assert!(missing.is_empty());
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn manual_match_overrides_wrong_auto_match_and_persists() {
    let path = temp_path("manual");
    let pool = db::init(&path).await.expect("init");
    let p = m3u_provider(&pool, "P").await;
    // Two same-named series — the auto-match could pick the wrong one.
    seed_series_multi(
        &pool,
        &p.id,
        vec![
            series_row("wrong", "Sherlock", 2002),
            series_row("right", "Sherlock", 2010),
        ],
        vec![],
    )
    .await;

    // A wrong auto-match exists for the canonical id.
    match_put(
        &pool,
        &ContentMatch {
            provider_id: p.id.clone(),
            content_type: "series".into(),
            content_id: "wrong".into(),
            imdb_id: "tt1475582".into(),
            tmdb_id: None,
            confidence: 0.7,
            method: "name_year".into(),
            matched_at: 1,
        },
    )
    .await
    .unwrap();

    // The user overrides to the correct series.
    set_manual_match(
        &pool,
        &ContentMatch {
            provider_id: p.id.clone(),
            content_type: "series".into(),
            content_id: "right".into(),
            imdb_id: "tt1475582".into(),
            tmdb_id: None,
            confidence: 1.0,
            method: "manual".into(),
            matched_at: 2,
        },
    )
    .await
    .unwrap();

    // Only the manual match remains for this canonical id on the provider.
    let rev = matches_for_imdb(&pool, "tt1475582", "series", std::slice::from_ref(&p.id))
        .await
        .unwrap();
    assert_eq!(rev.len(), 1, "the wrong auto-match was cleared");
    assert_eq!(rev[0].content_id, "right");
    assert_eq!(rev[0].method, "manual");

    // And the correction survives a catalog refresh.
    seed_series_multi(
        &pool,
        &p.id,
        vec![
            series_row("right", "Sherlock", 2010),
            series_row("wrong", "Sherlock", 2002),
        ],
        vec![],
    )
    .await;
    let after = match_get(&pool, &p.id, "series", "right")
        .await
        .unwrap()
        .expect("manual match survives refresh");
    assert_eq!(after.method, "manual");
    pool.close().await;
    cleanup_db(&path);
}

// --- Slice 5: watch progress follows the canonical title across sources ---

#[tokio::test]
async fn canonical_progress_follows_a_movie_across_providers() {
    let path = temp_path("cprog-movie");
    let pool = db::init(&path).await.expect("init");
    let a = m3u_provider(&pool, "A").await;
    let b = m3u_provider(&pool, "B").await;
    seed_movies(&pool, &a.id, vec![movie_row("a1", "Heat", 1995)]).await;
    seed_movies(&pool, &b.id, vec![movie_row("b1", "Heat", 1995)]).await;
    for (pid, cid) in [(&a.id, "a1"), (&b.id, "b1")] {
        match_put(
            &pool,
            &ContentMatch {
                provider_id: pid.clone(),
                content_type: "movie".into(),
                content_id: cid.into(),
                imdb_id: "tt0113277".into(),
                tmdb_id: None,
                confidence: 1.0,
                method: "tmdb".into(),
                matched_at: 0,
            },
        )
        .await
        .unwrap();
    }
    // Watched 100s on A (older), then 600s on B (newer).
    db::watch::upsert(&pool, &a.id, "movie", "a1", 100, Some(3600), false, 10).await.unwrap();
    db::watch::upsert(&pool, &b.id, "movie", "b1", 600, Some(3600), false, 20).await.unwrap();

    // Resume follows the title — the freshest position across sources.
    let prog = db::watch::canonical_progress(&pool, "tt0113277", "movie", 0, 0)
        .await
        .unwrap()
        .expect("progress");
    assert_eq!(prog.position_seconds, 600);

    // Un-matched canonical id → none (the player falls back to per-item).
    assert!(db::watch::canonical_progress(&pool, "tt9999999", "movie", 0, 0)
        .await
        .unwrap()
        .is_none());
    pool.close().await;
    cleanup_db(&path);
}

#[tokio::test]
async fn canonical_progress_follows_an_episode_across_providers() {
    let path = temp_path("cprog-ep");
    let pool = db::init(&path).await.expect("init");
    let a = m3u_provider(&pool, "A").await;
    let b = m3u_provider(&pool, "B").await;
    seed_series_multi(
        &pool,
        &a.id,
        vec![series_row("sa", "Fargo", 2014)],
        vec![("sa", vec![episode_row("a-s2e3", "sa", 2, 3)])],
    )
    .await;
    seed_series_multi(
        &pool,
        &b.id,
        vec![series_row("sb", "Fargo", 2014)],
        vec![("sb", vec![episode_row("b-s2e3", "sb", 2, 3)])],
    )
    .await;
    for (pid, cid) in [(&a.id, "sa"), (&b.id, "sb")] {
        match_put(
            &pool,
            &ContentMatch {
                provider_id: pid.clone(),
                content_type: "series".into(),
                content_id: cid.into(),
                imdb_id: "tt2802850".into(),
                tmdb_id: None,
                confidence: 0.7,
                method: "name_year".into(),
                matched_at: 0,
            },
        )
        .await
        .unwrap();
    }
    // S2E3 watched on B (older) then A (newer).
    db::watch::upsert(&pool, &b.id, "episode", "b-s2e3", 200, Some(2400), false, 5).await.unwrap();
    db::watch::upsert(&pool, &a.id, "episode", "a-s2e3", 800, Some(2400), false, 50).await.unwrap();

    let prog = db::watch::canonical_progress(&pool, "tt2802850", "episode", 2, 3)
        .await
        .unwrap()
        .expect("progress");
    assert_eq!(prog.position_seconds, 800, "freshest S2E3 across providers");

    // A different episode of the same title has no progress yet.
    assert!(db::watch::canonical_progress(&pool, "tt2802850", "episode", 1, 1)
        .await
        .unwrap()
        .is_none());
    pool.close().await;
    cleanup_db(&path);
}
