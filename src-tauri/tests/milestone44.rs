//! Milestone 44 acceptance tests (cross-source search dedup). A provider search
//! hit that is the same title as a canonical ("All Sources") hit is hidden from
//! the provider group. Coverage: the pure decision (`is_search_dupe`) across the
//! name+year and authoritative-`content_match` paths, and the DB-backed command
//! impl that wires a recorded match into that decision.

use proscenium_lib::canonical::resolver::is_search_dupe;
use proscenium_lib::commands::canonical::dedup_search_hits_impl;
use proscenium_lib::commands::providers::upsert_provider_impl;
use proscenium_lib::db;
use proscenium_lib::db::canonical::{match_put, ContentMatch};
use proscenium_lib::models::{DedupCanonical, DedupProviderHit, ProviderInput, ProviderType};
use sqlx::SqlitePool;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("proscenium-m44-{tag}-{}.db", uuid::Uuid::new_v4()))
}

fn cleanup_db(path: &PathBuf) {
    for s in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), s));
    }
}

fn canon(imdb: &str, name: &str, year: Option<i64>) -> DedupCanonical {
    DedupCanonical {
        imdb_id: imdb.into(),
        name: name.into(),
        year,
    }
}

fn prov(pid: &str, key: &str, cid: &str, name: &str, year: Option<i64>) -> DedupProviderHit {
    DedupProviderHit {
        key: key.into(),
        provider_id: pid.into(),
        content_id: cid.into(),
        name: name.into(),
        year,
    }
}

/// A minimal M3U provider so `content_match` rows satisfy the provider FK.
async fn m3u_provider(pool: &SqlitePool, name: &str) -> String {
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
    .id
}

// --- pure decision: is_search_dupe ---

#[test]
fn name_and_year_match_is_a_dupe_without_a_recorded_match() {
    let hits = [canon("tt0133093", "Matrix", Some(1999))];
    // "The Matrix" ~ "Matrix" (article is noise) and 1999 within ±1 → dupe.
    assert!(is_search_dupe("The Matrix (1999)", Some(1999), None, &hits));
}

#[test]
fn a_remake_year_apart_is_not_a_dupe() {
    let hits = [canon("tt0000001", "The Matrix", Some(2021))];
    // Same name, but a 22-year gap → not the same title.
    assert!(!is_search_dupe("The Matrix", Some(1999), None, &hits));
}

#[test]
fn a_different_title_is_not_a_dupe() {
    let hits = [canon("tt0000002", "Silent River", Some(2000))];
    assert!(!is_search_dupe("Golden Empire", Some(2001), None, &hits));
}

#[test]
fn a_recorded_match_present_among_the_hits_is_a_dupe_regardless_of_name() {
    let hits = [canon("tt0133093", "The Matrix", Some(1999))];
    // The provider title is unrecognizable, but it's confirmed as tt0133093,
    // which is one of the canonical hits → dupe.
    assert!(is_search_dupe(
        "MATRIX.1999.REMUX.FRENCH",
        None,
        Some("tt0133093"),
        &hits
    ));
}

#[test]
fn a_recorded_match_absent_from_the_hits_overrides_a_name_lookalike() {
    // Provider hit is confirmed as tt0000009 (not among the canonical hits), yet
    // its name+year would otherwise match the "The Matrix" hit. The confirmed
    // identity wins → NOT a dupe (this is the false-merge guard).
    let hits = [canon("tt0133093", "The Matrix", Some(1999))];
    assert!(!is_search_dupe("The Matrix", Some(1999), Some("tt0000009"), &hits));
}

#[test]
fn no_canonical_hits_means_nothing_is_a_dupe() {
    assert!(!is_search_dupe("The Matrix", Some(1999), None, &[]));
    assert!(!is_search_dupe("The Matrix", Some(1999), Some("tt0133093"), &[]));
}

// --- command impl: recorded match + name/year over a real DB ---

#[tokio::test]
async fn dedup_hides_recorded_and_name_year_dupes_only() {
    let path = temp_path("dedup");
    let pool = db::init(&path).await.expect("init");
    let pid = m3u_provider(&pool, "Prov").await;

    // c1 is confirmed as tt100 via content_match (a tmdb-confirmed identity whose
    // provider title bears no resemblance to the canonical name).
    match_put(
        &pool,
        &ContentMatch {
            provider_id: pid.clone(),
            content_type: "movie".into(),
            content_id: "c1".into(),
            imdb_id: "tt100".into(),
            tmdb_id: Some(42),
            confidence: 1.0,
            method: "tmdb".into(),
            matched_at: 0,
        },
    )
    .await
    .unwrap();

    let canonical = vec![
        canon("tt100", "Whatever It Is Called", Some(2000)),
        canon("tt200", "Golden Empire", Some(2010)),
    ];
    let provider = vec![
        prov(&pid, "p1:c1", "c1", "Totally Unrelated Provider Name", None), // hidden: recorded tt100
        prov(&pid, "p1:c2", "c2", "Golden Empire", Some(2010)),             // hidden: name+year → tt200
        prov(&pid, "p1:c3", "c3", "Unmatched Thing", Some(1980)),           // kept
    ];

    let hide = dedup_search_hits_impl(&pool, "movie", &canonical, &provider).await;
    assert_eq!(hide, vec!["p1:c1".to_string(), "p1:c2".to_string()]);

    // Empty inputs short-circuit to no dedup.
    assert!(dedup_search_hits_impl(&pool, "movie", &canonical, &[])
        .await
        .is_empty());
    assert!(dedup_search_hits_impl(&pool, "movie", &[], &provider)
        .await
        .is_empty());

    pool.close().await;
    cleanup_db(&path);
}
