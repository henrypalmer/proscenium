//! Xtream Codes API client: authentication and the six catalog fetch
//! endpoints (spec §5.2 / §6).

use crate::models::{
    CatalogData, Category, ConnectionTestResult, EpisodeItem, LiveChannel, MovieItem, SeriesItem,
    XtreamAccountInfo,
};
use serde_json::Value;
use std::collections::HashMap;

pub struct XtreamCreds<'a> {
    pub server_url: &'a str,
    pub username: &'a str,
    pub password: &'a str,
}

impl XtreamCreds<'_> {
    fn base(&self) -> &str {
        self.server_url.trim_end_matches('/')
    }
}

async fn get_action(
    client: &reqwest::Client,
    creds: &XtreamCreds<'_>,
    action: &str,
) -> Result<Value, String> {
    let url = format!("{}/player_api.php", creds.base());
    let response = client
        .get(&url)
        .query(&[
            ("username", creds.username),
            ("password", creds.password),
            ("action", action),
        ])
        .send()
        .await
        .map_err(|_| {
            format!(
                "Could not connect to {}. Check the server address and your internet connection.",
                creds.server_url
            )
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "The server responded with HTTP {} for {action}.",
            response.status()
        ));
    }
    response
        .json()
        .await
        .map_err(|_| format!("The server returned an invalid response for {action}."))
}

fn parse_categories(body: &Value) -> Vec<Category> {
    let Some(items) = body.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            Some(Category {
                id: value_to_string(&item["category_id"])?,
                name: value_to_string(&item["category_name"])?,
                sort_order: i as i64,
            })
        })
        .collect()
}

fn category_names(cats: &[Category]) -> HashMap<&str, &str> {
    cats.iter().map(|c| (c.id.as_str(), c.name.as_str())).collect()
}

fn lookup<'a>(names: &HashMap<&str, &'a str>, id: &str) -> String {
    names.get(id).copied().unwrap_or("Uncategorized").to_string()
}

/// `year` field, or the leading `YYYY` of a `releaseDate`-style field.
fn year_from(item: &Value) -> Option<i64> {
    if let Some(y) = value_to_i64(&item["year"]) {
        return Some(y);
    }
    for key in ["releaseDate", "release_date"] {
        if let Some(date) = item[key].as_str() {
            if date.len() >= 4 {
                if let Ok(y) = date[..4].parse() {
                    return Some(y);
                }
            }
        }
    }
    None
}

/// Run the full six-endpoint catalog refresh (spec §5.2). Episodes are not
/// fetched here: `get_series_info` is per-series and is requested on demand
/// when a series is opened (Milestone 5).
pub async fn fetch_catalog(
    creds: &XtreamCreds<'_>,
    mut on_stage: impl FnMut(&str, f32),
) -> Result<CatalogData, String> {
    let client = super::http_client()?;
    let mut data = CatalogData::default();

    on_stage("Live categories", 0.0 / 7.0);
    data.live_categories = parse_categories(&get_action(&client, creds, "get_live_categories").await?);

    on_stage("Live channels", 1.0 / 7.0);
    let names = category_names(&data.live_categories);
    if let Some(items) = get_action(&client, creds, "get_live_streams").await?.as_array() {
        for item in items {
            let Some(id) = value_to_string(&item["stream_id"]) else {
                continue;
            };
            let Some(name) = value_to_string(&item["name"]) else {
                continue;
            };
            let category_id = value_to_string(&item["category_id"]).unwrap_or_else(|| "0".into());
            data.live_channels.push(LiveChannel {
                stream_url: format!(
                    "{}/live/{}/{}/{}.ts",
                    creds.base(),
                    creds.username,
                    creds.password,
                    id
                ),
                id,
                name,
                category_name: lookup(&names, &category_id),
                category_id,
                logo_url: value_to_string(&item["stream_icon"]).filter(|s| !s.is_empty()),
                stream_ext: "ts".into(),
                epg_channel_id: value_to_string(&item["epg_channel_id"]).filter(|s| !s.is_empty()),
            });
        }
    }

    on_stage("Movie categories", 2.0 / 7.0);
    data.vod_categories = parse_categories(&get_action(&client, creds, "get_vod_categories").await?);

    on_stage("Movies", 3.0 / 7.0);
    let names = category_names(&data.vod_categories);
    if let Some(items) = get_action(&client, creds, "get_vod_streams").await?.as_array() {
        for item in items {
            let Some(id) = value_to_string(&item["stream_id"]) else {
                continue;
            };
            let Some(name) = value_to_string(&item["name"]) else {
                continue;
            };
            let category_id = value_to_string(&item["category_id"]).unwrap_or_else(|| "0".into());
            let ext = value_to_string(&item["container_extension"])
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "mp4".into());
            data.movies.push(MovieItem {
                stream_url: format!(
                    "{}/movie/{}/{}/{}.{}",
                    creds.base(),
                    creds.username,
                    creds.password,
                    id,
                    ext
                ),
                id,
                category_name: lookup(&names, &category_id),
                category_id,
                poster_url: value_to_string(&item["stream_icon"]).filter(|s| !s.is_empty()),
                container_ext: ext,
                release_year: year_from(item),
                rating: value_to_string(&item["rating"]).filter(|s| !s.is_empty()),
                added_at: value_to_i64(&item["added"]),
                name,
            });
        }
    }

    on_stage("Series categories", 4.0 / 7.0);
    data.series_categories =
        parse_categories(&get_action(&client, creds, "get_series_categories").await?);

    on_stage("Series", 5.0 / 7.0);
    let names = category_names(&data.series_categories);
    if let Some(items) = get_action(&client, creds, "get_series").await?.as_array() {
        for item in items {
            let Some(id) = value_to_string(&item["series_id"]) else {
                continue;
            };
            let Some(name) = value_to_string(&item["name"]) else {
                continue;
            };
            let category_id = value_to_string(&item["category_id"]).unwrap_or_else(|| "0".into());
            data.series.push(SeriesItem {
                id,
                name,
                category_name: lookup(&names, &category_id),
                category_id,
                poster_url: value_to_string(&item["cover"]).filter(|s| !s.is_empty()),
                release_year: year_from(item),
            });
        }
    }

    Ok(data)
}

async fn get_action_with_id(
    client: &reqwest::Client,
    creds: &XtreamCreds<'_>,
    action: &str,
    id_key: &str,
    id: &str,
) -> Result<Value, String> {
    let url = format!("{}/player_api.php", creds.base());
    let response = client
        .get(&url)
        .query(&[
            ("username", creds.username),
            ("password", creds.password),
            ("action", action),
            (id_key, id),
        ])
        .send()
        .await
        .map_err(|_| {
            format!(
                "Could not connect to {}. Check the server address and your internet connection.",
                creds.server_url
            )
        })?;
    if !response.status().is_success() {
        return Err(format!(
            "The server responded with HTTP {} for {action}.",
            response.status()
        ));
    }
    response
        .json()
        .await
        .map_err(|_| format!("The server returned an invalid response for {action}."))
}

/// `plot` is the canonical Xtream field; some panels use `description`.
fn description_from(info: &Value) -> Option<String> {
    ["plot", "description"]
        .iter()
        .find_map(|k| value_to_string(&info[*k]).filter(|s| !s.is_empty()))
}

/// On-demand movie metadata (spec §6 `get_vod_info`, Milestone 5).
#[derive(Debug, Clone, Default)]
pub struct VodInfo {
    pub description: Option<String>,
    pub genre: Option<String>,
    pub duration_seconds: Option<i64>,
}

pub async fn fetch_vod_info(creds: &XtreamCreds<'_>, vod_id: &str) -> Result<VodInfo, String> {
    let client = super::http_client()?;
    let body = get_action_with_id(&client, creds, "get_vod_info", "vod_id", vod_id).await?;
    let info = &body["info"];
    Ok(VodInfo {
        description: description_from(info),
        genre: value_to_string(&info["genre"]).filter(|s| !s.is_empty()),
        duration_seconds: value_to_i64(&info["duration_secs"]),
    })
}

/// On-demand series metadata and episode list (spec §6 `get_series_info`,
/// Milestone 5 — episodes are fetched per series when one is opened).
#[derive(Debug, Clone, Default)]
pub struct SeriesInfo {
    pub description: Option<String>,
    pub genre: Option<String>,
    pub episodes: Vec<EpisodeItem>,
}

pub async fn fetch_series_info(
    creds: &XtreamCreds<'_>,
    series_id: &str,
) -> Result<SeriesInfo, String> {
    let client = super::http_client()?;
    let body = get_action_with_id(&client, creds, "get_series_info", "series_id", series_id).await?;
    let info = &body["info"];

    // `episodes` is usually an object keyed by season number, but some
    // panels return an array of per-season arrays.
    let season_lists: Vec<&Value> = match &body["episodes"] {
        Value::Object(map) => map.values().collect(),
        Value::Array(items) => items.iter().collect(),
        _ => Vec::new(),
    };

    let mut episodes = Vec::new();
    for list in season_lists {
        let Some(items) = list.as_array() else {
            continue;
        };
        for (i, item) in items.iter().enumerate() {
            let Some(id) = value_to_string(&item["id"]) else {
                continue;
            };
            let episode = value_to_i64(&item["episode_num"]).unwrap_or(i as i64 + 1);
            let ext = value_to_string(&item["container_extension"])
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "mp4".into());
            episodes.push(EpisodeItem {
                stream_url: format!(
                    "{}/series/{}/{}/{}.{}",
                    creds.base(),
                    creds.username,
                    creds.password,
                    id,
                    ext
                ),
                id,
                series_id: series_id.to_string(),
                season: value_to_i64(&item["season"]).unwrap_or(1),
                title: value_to_string(&item["title"])
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("Episode {episode}")),
                episode,
                container_ext: ext,
                duration_seconds: value_to_i64(&item["info"]["duration_secs"]),
                poster_url: value_to_string(&item["info"]["movie_image"]).filter(|s| !s.is_empty()),
            });
        }
    }
    episodes.sort_by_key(|e| (e.season, e.episode));

    Ok(SeriesInfo {
        description: description_from(info),
        genre: value_to_string(&info["genre"]).filter(|s| !s.is_empty()),
        episodes,
    })
}

/// `GET {server}/player_api.php?username={u}&password={p}` and interpret the
/// account-info response (spec §5.1).
pub async fn test_connection(
    server_url: &str,
    username: &str,
    password: &str,
) -> ConnectionTestResult {
    let client = match super::http_client() {
        Ok(c) => c,
        Err(e) => return ConnectionTestResult::failure(e),
    };
    let base = server_url.trim_end_matches('/');
    let url = format!("{base}/player_api.php");

    let response = client
        .get(&url)
        .query(&[("username", username), ("password", password)])
        .send()
        .await;

    let response = match response {
        Ok(r) => r,
        Err(_) => {
            return ConnectionTestResult::failure(format!(
                "Could not connect to {server_url}. Check the server address and your internet connection."
            ));
        }
    };

    if !response.status().is_success() {
        return ConnectionTestResult::failure(format!(
            "The server at {server_url} responded with HTTP {}. Check the server address.",
            response.status()
        ));
    }

    let body: Value = match response.json().await {
        Ok(v) => v,
        Err(_) => {
            return ConnectionTestResult::failure(format!(
                "The server at {server_url} did not return a valid Xtream Codes response. Check the server address."
            ));
        }
    };

    parse_auth_response(&body)
}

fn parse_auth_response(body: &Value) -> ConnectionTestResult {
    let user_info = &body["user_info"];
    if !value_truthy(&user_info["auth"]) {
        return ConnectionTestResult::failure(
            "Authentication failed. Check your username and password.",
        );
    }

    let status = value_to_string(&user_info["status"]);
    let info = XtreamAccountInfo {
        status: status.clone(),
        exp_date: value_to_i64(&user_info["exp_date"]),
        max_connections: value_to_i64(&user_info["max_connections"]),
        active_connections: value_to_i64(&user_info["active_cons"]),
    };

    let message = match status.as_deref() {
        Some(s) if s.eq_ignore_ascii_case("expired") => {
            "Connected, but the subscription has expired.".to_string()
        }
        _ => "Connected successfully.".to_string(),
    };

    ConnectionTestResult {
        success: true,
        message,
        account_info: Some(info),
    }
}

/// Xtream servers are loose with types: `auth` may be `1`, `"1"`, or `true`.
fn value_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(s) => s == "1" || s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Numeric fields may arrive as numbers or numeric strings.
fn value_to_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
