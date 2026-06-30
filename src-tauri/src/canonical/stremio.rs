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
