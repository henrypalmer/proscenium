//! Milestone 43 acceptance tests (canonical search). The network fetch is shared
//! with M40's already-tested catalog path, so these cover the *new* wiring: the
//! Cinemeta **search** URL is built/encoded correctly, an absent search adds no
//! stray segment, and a Cinemeta search body parses into canonical hits (the
//! same `{ metas: [...] }` shape as a catalog page, non-IMDB ids filtered out).

use proscenium_lib::canonical::cinemeta;
use serde_json::json;

const BASE: &str = "https://v3-cinemeta.strem.io";

#[test]
fn search_url_encodes_query_for_both_kinds() {
    // A multi-word query → a `search=` extra segment with the space percent-encoded.
    assert_eq!(
        cinemeta::catalog_url("movie", None, Some("the matrix"), 0),
        format!("{BASE}/catalog/movie/top/search=the%20matrix.json"),
    );
    assert_eq!(
        cinemeta::catalog_url("series", None, Some("breaking bad"), 0),
        format!("{BASE}/catalog/series/top/search=breaking%20bad.json"),
    );
    // Reserved characters are encoded too, so the path stays well-formed.
    assert_eq!(
        cinemeta::catalog_url("movie", None, Some("d&d: honor"), 0),
        format!("{BASE}/catalog/movie/top/search=d%26d%3A%20honor.json"),
    );
}

#[test]
fn catalog_url_omits_absent_or_empty_search() {
    // No search and no genre → the plain catalog page, never a stray `search=`.
    assert_eq!(
        cinemeta::catalog_url("movie", None, None, 0),
        format!("{BASE}/catalog/movie/top.json"),
    );
    // An empty string is treated as absent (matches the command's empty-query
    // short-circuit), not encoded into `search=`.
    assert_eq!(
        cinemeta::catalog_url("series", None, Some(""), 0),
        format!("{BASE}/catalog/series/top.json"),
    );
}

#[test]
fn search_body_parses_into_canonical_hits() {
    // A Cinemeta search response is the same envelope as a catalog page.
    let body = json!({
        "metas": [
            { "id": "tt0133093", "type": "movie", "name": "The Matrix",
              "poster": "https://img/matrix.jpg", "releaseInfo": "1999" },
            { "id": "tt0234215", "type": "movie", "name": "The Matrix Reloaded",
              "poster": "https://img/reloaded.jpg", "year": "2003" },
            // Non-IMDB id (Cinemeta occasionally mixes these in) → dropped.
            { "id": "kitsu:1376", "type": "movie", "name": "Not An IMDB Title" },
        ]
    });

    let hits = cinemeta::parse_catalog(&body);

    assert_eq!(hits.len(), 2, "only the two tt-id entries survive");
    assert_eq!(hits[0].imdb_id, "tt0133093");
    assert_eq!(hits[0].name, "The Matrix");
    assert_eq!(hits[0].kind, "movie");
    assert_eq!(hits[0].release_year, Some(1999));
    // Year is read from `year` when `releaseInfo` is absent.
    assert_eq!(hits[1].release_year, Some(2003));
    assert!(hits.iter().all(|h| h.imdb_id.starts_with("tt")));
}
