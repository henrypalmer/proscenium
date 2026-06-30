//! Cinemeta client (Milestone 40): the canonical metadata backbone at
//! `v3-cinemeta.strem.io`. Cinemeta is itself a Stremio addon, so the request
//! plumbing here is shared with the Stremio stream addons added in M41.
//!
//! The fetchers return the app's own `Canonical*` models (not raw Cinemeta
//! JSON); the parse functions are pure so they're unit-tested against sample
//! payloads without touching the network.

use crate::models::{CanonicalItem, CanonicalMeta, CanonicalVideo};
use serde_json::Value;

pub const BASE: &str = "https://v3-cinemeta.strem.io";

/// Genre options Cinemeta's "Popular" catalog accepts, per content kind (from
/// its manifest). Static so the genre sidebar renders offline; Cinemeta changes
/// this set rarely.
pub fn genres(kind: &str) -> Vec<String> {
    let common = [
        "Action",
        "Adventure",
        "Animation",
        "Biography",
        "Comedy",
        "Crime",
        "Documentary",
        "Drama",
        "Family",
        "Fantasy",
        "History",
        "Horror",
        "Mystery",
        "Romance",
        "Sci-Fi",
        "Sport",
        "Thriller",
        "War",
        "Western",
    ];
    let mut out: Vec<String> = common.iter().map(|s| s.to_string()).collect();
    if kind == "series" {
        for extra in ["Reality-TV", "Talk-Show", "Game-Show"] {
            out.push(extra.to_string());
        }
    }
    out
}

// --- value helpers (Cinemeta mixes string/number types) ---

fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Leading 4-digit year from "1999" or a range like "2011–2019".
fn year_of(v: &Value) -> Option<i64> {
    let s = as_string(v)?;
    s.split(|c: char| !c.is_ascii_digit())
        .find(|p| p.len() == 4)
        .and_then(|p| p.parse().ok())
}

fn string_array(v: &Value) -> Vec<String> {
    match v {
        Value::Array(items) => items.iter().filter_map(as_string).collect(),
        _ => Vec::new(),
    }
}

/// Percent-encode an extra value for the catalog path segment (genre/search).
fn pct(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// --- parsing (pure; unit-tested in tests/milestone40.rs) ---

/// One catalog/`metas` entry → a canonical card. `None` for non-IMDB entries
/// (Cinemeta occasionally mixes in `kitsu:`/other ids).
pub fn item_from_meta(meta: &Value) -> Option<CanonicalItem> {
    let imdb_id = as_string(&meta["id"]).or_else(|| as_string(&meta["imdb_id"]))?;
    if !imdb_id.starts_with("tt") {
        return None;
    }
    Some(CanonicalItem {
        kind: as_string(&meta["type"]).unwrap_or_else(|| "movie".into()),
        name: as_string(&meta["name"])?,
        poster_url: as_string(&meta["poster"]),
        release_year: year_of(&meta["year"]).or_else(|| year_of(&meta["releaseInfo"])),
        imdb_id,
    })
}

/// A Cinemeta catalog body (`{ metas: [...] }`) → canonical cards.
pub fn parse_catalog(body: &Value) -> Vec<CanonicalItem> {
    body["metas"]
        .as_array()
        .map(|metas| metas.iter().filter_map(item_from_meta).collect())
        .unwrap_or_default()
}

/// A Cinemeta meta object (the inner `meta`, not the `{ meta: … }` envelope).
pub fn parse_meta(meta: &Value, kind: &str) -> CanonicalMeta {
    let genres = {
        let g = string_array(&meta["genres"]);
        if g.is_empty() {
            string_array(&meta["genre"])
        } else {
            g
        }
    };
    CanonicalMeta {
        imdb_id: as_string(&meta["id"])
            .or_else(|| as_string(&meta["imdb_id"]))
            .unwrap_or_default(),
        kind: as_string(&meta["type"]).unwrap_or_else(|| kind.to_string()),
        name: as_string(&meta["name"]).unwrap_or_default(),
        poster_url: as_string(&meta["poster"]),
        backdrop_url: as_string(&meta["background"]).or_else(|| as_string(&meta["poster"])),
        description: as_string(&meta["description"]),
        release_year: year_of(&meta["year"]).or_else(|| year_of(&meta["releaseInfo"])),
        release_info: as_string(&meta["releaseInfo"]),
        genres,
        cast: string_array(&meta["cast"]),
        director: string_array(&meta["director"]),
        runtime: as_string(&meta["runtime"]),
        imdb_rating: as_string(&meta["imdbRating"]).and_then(|s| s.parse().ok()),
        tmdb_id: as_i64(&meta["moviedb_id"]),
        videos: parse_videos(&meta["videos"]),
    }
}

/// Cinemeta `videos[]` → sorted canonical episodes (season 0 specials kept).
fn parse_videos(v: &Value) -> Vec<CanonicalVideo> {
    let Some(items) = v.as_array() else {
        return Vec::new();
    };
    let mut out: Vec<CanonicalVideo> = items
        .iter()
        .filter_map(|video| {
            let id = as_string(&video["id"])?;
            let episode = as_i64(&video["episode"])
                .or_else(|| as_i64(&video["number"]))
                .unwrap_or(0);
            Some(CanonicalVideo {
                season: as_i64(&video["season"]).unwrap_or(0),
                name: as_string(&video["name"]).unwrap_or_else(|| format!("Episode {episode}")),
                overview: as_string(&video["overview"])
                    .or_else(|| as_string(&video["description"])),
                thumbnail: as_string(&video["thumbnail"]),
                released: as_string(&video["released"])
                    .or_else(|| as_string(&video["firstAired"])),
                episode,
                id,
            })
        })
        .collect();
    out.sort_by_key(|e| (e.season, e.episode));
    out
}

// --- HTTP (live; not unit-tested) ---

async fn get_json(url: &str) -> Result<Value, String> {
    let client = crate::iptv::http_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Could not reach the canonical metadata service (Cinemeta).".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Cinemeta responded with HTTP {}.", resp.status()));
    }
    resp.json()
        .await
        .map_err(|_| "Cinemeta returned an invalid response.".to_string())
}

/// `/catalog/{kind}/top[/{extra}].json`; `extra` is `key=value` pairs joined by
/// `&` (Stremio addon convention), e.g. `genre=Action&skip=100`. `pub` so the M43
/// search wiring (search URL construction/encoding) is unit-testable.
pub fn catalog_url(kind: &str, genre: Option<&str>, search: Option<&str>, skip: i64) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(g) = genre.filter(|s| !s.is_empty()) {
        parts.push(format!("genre={}", pct(g)));
    }
    if let Some(s) = search.filter(|s| !s.is_empty()) {
        parts.push(format!("search={}", pct(s)));
    }
    if skip > 0 {
        parts.push(format!("skip={skip}"));
    }
    if parts.is_empty() {
        format!("{BASE}/catalog/{kind}/top.json")
    } else {
        format!("{BASE}/catalog/{kind}/top/{}.json", parts.join("&"))
    }
}

pub async fn fetch_catalog(
    kind: &str,
    genre: Option<&str>,
    search: Option<&str>,
    skip: i64,
) -> Result<Vec<CanonicalItem>, String> {
    let body = get_json(&catalog_url(kind, genre, search, skip)).await?;
    Ok(parse_catalog(&body))
}

pub async fn fetch_meta(kind: &str, imdb_id: &str) -> Result<CanonicalMeta, String> {
    let body = get_json(&format!("{BASE}/meta/{kind}/{imdb_id}.json")).await?;
    let meta = &body["meta"];
    if meta.is_null() {
        return Err("Cinemeta has no metadata for this title.".into());
    }
    Ok(parse_meta(meta, kind))
}
