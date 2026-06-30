//! Stremio stream addon client (Milestone 41): manifest validation (slice 1) and
//! stream resolution (slice 2). Reuses the addon-client plumbing proven by
//! Cinemeta (itself a Stremio addon). Token-bearing manifest URLs are secrets —
//! they reach here only transiently (read from the keychain) and are never
//! logged. Parse functions are pure and unit-tested against sample payloads.

use serde_json::Value;

/// Non-secret declared metadata from an addon manifest.
#[derive(Debug, Clone)]
pub struct AddonManifest {
    pub name: String,
    pub types: Vec<String>,
    pub resources: Vec<String>,
    pub id_prefixes: Vec<String>,
}

fn string_array(v: &Value) -> Vec<String> {
    match v {
        Value::Array(items) => items
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}

/// `resources` may be plain strings (`["stream"]`) or objects
/// (`[{name, types, idPrefixes}]`). Returns (resource names, id prefixes gathered
/// from the `stream` resource).
fn parse_resources(v: &Value) -> (Vec<String>, Vec<String>) {
    let mut names = Vec::new();
    let mut prefixes = Vec::new();
    if let Value::Array(items) = v {
        for item in items {
            match item {
                Value::String(s) => names.push(s.clone()),
                Value::Object(_) => {
                    if let Some(name) = item["name"].as_str() {
                        names.push(name.to_string());
                        if name == "stream" {
                            prefixes.extend(string_array(&item["idPrefixes"]));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    (names, prefixes)
}

pub fn parse_manifest(body: &Value) -> AddonManifest {
    let (resources, mut id_prefixes) = parse_resources(&body["resources"]);
    // Some addons declare id prefixes at the top level too.
    for p in string_array(&body["idPrefixes"]) {
        if !id_prefixes.contains(&p) {
            id_prefixes.push(p);
        }
    }
    AddonManifest {
        name: body["name"].as_str().unwrap_or("Stremio addon").to_string(),
        types: string_array(&body["types"]),
        resources,
        id_prefixes,
    }
}

/// A usable stream addon must offer the `stream` resource. (Most accept the `tt`
/// IMDB prefix, and Cinemeta hands us the imdb id regardless.)
pub fn validate(m: &AddonManifest) -> Result<(), String> {
    if !m.resources.iter().any(|r| r == "stream") {
        return Err("This addon does not provide a stream resource.".into());
    }
    Ok(())
}

/// Base URL for stream requests = the manifest URL without the trailing
/// `/manifest.json`. (Carries the token in its path — keep it out of logs.)
pub fn base_url(manifest_url: &str) -> String {
    manifest_url
        .trim_end_matches("manifest.json")
        .trim_end_matches('/')
        .to_string()
}

async fn get_json(url: &str) -> Result<Value, String> {
    let client = crate::iptv::http_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Could not reach the addon. Check the URL.".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("The addon responded with HTTP {}.", resp.status()));
    }
    resp.json()
        .await
        .map_err(|_| "The addon returned an invalid response.".to_string())
}

pub async fn fetch_manifest(manifest_url: &str) -> Result<AddonManifest, String> {
    Ok(parse_manifest(&get_json(manifest_url).await?))
}

// --- stream resolution (slice 2) ---

use crate::models::StreamCandidate;

/// Direct (playable) streams shown per addon, and infoHash-only "needs debrid"
/// markers surfaced. Addons order best-first, so the head is the best.
const CAP_DIRECT: usize = 8;
const CAP_DEBRID: usize = 5;

fn non_empty_str(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

/// Pull a container extension out of a filename/title label.
fn parse_container(label: &str) -> Option<String> {
    let lower = label.to_ascii_lowercase();
    ["mkv", "mp4", "avi", "ts", "webm", "m4v", "mov"]
        .into_iter()
        .find(|ext| lower.contains(&format!(".{ext}")))
        .map(String::from)
}

/// Direct addon streams rank just under tmdb-confirmed IPTV (1.0); ordered by
/// resolution. infoHash-only sinks to the bottom (M42 does proper ranking).
fn quality_confidence(quality: &Option<String>) -> f64 {
    match quality.as_deref() {
        Some("2160p") => 0.97,
        Some("1080p") => 0.95,
        Some("720p") => 0.93,
        Some("480p") => 0.91,
        _ => 0.90,
    }
}

/// Parse a Stremio `/stream` body into candidates (pure; unit-tested). `url`
/// (or `externalUrl`) → a directly playable source; `infoHash`-only (no engine)
/// → a `needs_debrid` marker. `content_type` is the player kind ("movie" |
/// "episode"); `source` labels the addon.
pub fn parse_streams(body: &Value, source: &str, content_type: &str) -> Vec<StreamCandidate> {
    let Some(streams) = body["streams"].as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let (mut direct, mut debrid) = (0usize, 0usize);
    for s in streams {
        let label = format!(
            "{} {}",
            non_empty_str(&s["name"]).unwrap_or_default(),
            non_empty_str(&s["title"]).unwrap_or_default()
        );
        let quality = crate::canonical::resolver::parse_quality(&label);
        let container = non_empty_str(&s["behaviorHints"]["filename"])
            .as_deref()
            .and_then(parse_container)
            .or_else(|| parse_container(&label));
        let url = non_empty_str(&s["url"]).or_else(|| non_empty_str(&s["externalUrl"]));

        if let Some(url) = url {
            if direct < CAP_DIRECT {
                direct += 1;
                out.push(StreamCandidate {
                    source: source.to_string(),
                    provider_id: None,
                    content_type: content_type.to_string(),
                    content_id: None,
                    url: Some(url),
                    quality: quality.clone(),
                    container,
                    confidence: quality_confidence(&quality),
                    needs_debrid: false,
                });
            }
        } else if non_empty_str(&s["infoHash"]).is_some() && debrid < CAP_DEBRID {
            debrid += 1;
            out.push(StreamCandidate {
                source: source.to_string(),
                provider_id: None,
                content_type: content_type.to_string(),
                content_id: None,
                url: None,
                quality,
                container,
                confidence: 0.05,
                needs_debrid: true,
            });
        }
    }
    out
}

/// Stream candidates from one addon for a canonical target (Milestone 41).
/// **Tier-3**: results are returned to the caller and never persisted to disk.
/// On any failure (network/HTTP/parse) returns empty so the picker degrades to
/// the other sources. `base_url` carries the token in its path — never logged.
pub async fn fetch_streams(
    base_url: &str,
    request_type: &str,
    stremio_id: &str,
    content_type: &str,
    source: &str,
) -> Vec<StreamCandidate> {
    let url = format!("{base_url}/stream/{request_type}/{stremio_id}.json");
    match get_json(&url).await {
        Ok(body) => parse_streams(&body, source, content_type),
        Err(_) => Vec::new(),
    }
}
