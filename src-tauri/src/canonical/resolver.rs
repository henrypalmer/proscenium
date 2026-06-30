//! Stream-resolver registry (Milestone 40 slice 2). Generalizes
//! `resolve_stream_url` into a registry of `StreamResolver`s — one per enabled
//! IPTV provider in v1; Stremio addons join the same registry in M41. Resolving
//! a canonical title returns ranked `StreamCandidate`s for the source picker.
//!
//! Movie matching: a Cinemeta title → an FTS name shortlist over the provider's
//! cached catalog → a year ±1 filter → for Xtream, a `get_vod_info`
//! `tmdb_id == moviedb_id` confirmation. Confirmed matches are recorded in
//! `content_match` (the side table that survives catalog refresh), so the next
//! resolution is a cheap index read rather than a re-search.

use crate::db::canonical::ContentMatch;
use crate::db::{self};
use crate::iptv::xtream;
use crate::keychain;
use crate::models::{Provider, ProviderType, SearchContentType, StreamCandidate};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

/// A canonical title to resolve provider sources for (Cinemeta-derived).
pub struct CanonicalRef {
    pub imdb_id: String,
    pub kind: String,
    /// Cinemeta `moviedb_id` — the tmdb confirm anchor (movies).
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub year: Option<i64>,
    pub season: Option<i64>,
    pub episode: Option<i64>,
}

/// Name-token Jaccard threshold for a name+year match when no tmdb is available.
const NAME_SIM_THRESHOLD: f64 = 0.6;
/// FTS candidates per provider to consider.
const SHORTLIST: i64 = 25;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// --- pure matching helpers (unit-tested) ---

/// Lowercase alphanumeric tokens, with bracketed segments and common
/// quality/language noise dropped, for fuzzy title comparison.
pub fn normalize_title(s: &str) -> Vec<String> {
    const NOISE: &[&str] = &[
        "the", "a", "an", "uhd", "hd", "sd", "fhd", "4k", "1080p", "720p", "480p", "2160p",
        "multi", "vf", "vo", "vostfr", "hevc", "x264", "x265", "web", "dl",
    ];
    let mut cleaned = String::with_capacity(s.len());
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = (depth - 1).max(0),
            _ if depth > 0 => {}
            c if c.is_alphanumeric() => cleaned.push(c.to_ascii_lowercase()),
            _ => cleaned.push(' '),
        }
    }
    cleaned
        .split_whitespace()
        .filter(|t| !NOISE.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Token-set Jaccard similarity (0..1) of two titles after normalization.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let sa: HashSet<String> = normalize_title(a).into_iter().collect();
    let sb: HashSet<String> = normalize_title(b).into_iter().collect();
    if sa.is_empty() || sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    inter / union
}

/// Year match within ±1 (a missing target year passes — it can't disqualify).
pub fn year_ok(candidate: Option<i64>, target: Option<i64>) -> bool {
    match (candidate, target) {
        (Some(c), Some(t)) => (c - t).abs() <= 1,
        _ => true,
    }
}

/// Decide a match from the available signals, or `None` to reject. A tmdb id on
/// both sides is decisive (equal → accept, differ → reject — this kills the
/// wrong-year/remake collisions name matching alone produces). Otherwise a
/// strong name plus an acceptable year is a medium-confidence match.
pub fn classify_match(
    provider_tmdb: Option<i64>,
    target_tmdb: Option<i64>,
    name_sim: f64,
    year_ok: bool,
) -> Option<(f64, &'static str)> {
    if let (Some(p), Some(t)) = (provider_tmdb, target_tmdb) {
        return if p == t { Some((1.0, "tmdb")) } else { None };
    }
    if year_ok && name_sim >= NAME_SIM_THRESHOLD {
        // 0.5..0.9 — always below a tmdb-confirmed 1.0.
        Some(((0.5 + name_sim * 0.4).min(0.9), "name_year"))
    } else {
        None
    }
}

/// Parse a quality tag from a provider title ("… 1080p", "[2160p]", "4K").
pub fn parse_quality(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    for (needle, label) in [
        ("2160p", "2160p"),
        ("4k", "2160p"),
        ("1080p", "1080p"),
        ("720p", "720p"),
        ("480p", "480p"),
    ] {
        if lower.contains(needle) {
            return Some(label.to_string());
        }
    }
    None
}

// --- resolver registry ---

/// A source of streams for canonical titles. v1: one per IPTV provider; M41 adds
/// a Stremio addon resolver implementing the same trait.
#[allow(async_fn_in_trait)] // used only with the concrete types below
pub trait StreamResolver {
    fn label(&self) -> &str;
    async fn resolve(&self, pool: &SqlitePool, target: &CanonicalRef) -> Vec<StreamCandidate>;
}

/// Resolves canonical titles against one IPTV provider's cached catalog.
pub struct ProviderResolver {
    pub provider: Provider,
}

impl ProviderResolver {
    /// Owned Xtream credentials, or `None` for M3U / missing fields.
    fn xtream_creds(&self) -> Option<(String, String, String)> {
        if self.provider.provider_type != ProviderType::Xtream {
            return None;
        }
        let server = self.provider.server_url.clone()?;
        let username = self.provider.username.clone()?;
        let password = keychain::get_secret(&self.provider.id).ok()?;
        Some((server, username, password))
    }

    /// Xtream `get_vod_info.tmdb_id` for a candidate movie, or `None` (M3U has no
    /// such endpoint, or the fetch failed). One network call, first match only.
    async fn provider_tmdb(&self, movie_id: &str) -> Option<i64> {
        let (server, username, password) = self.xtream_creds()?;
        let creds = xtream::XtreamCreds {
            server_url: &server,
            username: &username,
            password: &password,
        };
        xtream::fetch_vod_info(&creds, movie_id)
            .await
            .ok()
            .and_then(|i| i.tmdb_id)
    }

    async fn resolve_movie(&self, pool: &SqlitePool, target: &CanonicalRef) -> Vec<StreamCandidate> {
        let pid = &self.provider.id;
        // Fast path: provider items already matched to this canonical id.
        let mut matched: Vec<(String, f64)> = db::canonical::matches_for_imdb(
            pool,
            &target.imdb_id,
            "movie",
            std::slice::from_ref(pid),
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| (m.content_id, m.confidence))
        .collect();

        // Discover + record matches when none are cached yet.
        if matched.is_empty() {
            let shortlist = db::catalog::search_catalog(
                pool,
                std::slice::from_ref(pid),
                &target.name,
                SearchContentType::Movies,
                None,
                SHORTLIST,
            )
            .await
            .map(|r| r.movies)
            .unwrap_or_default();

            for m in shortlist {
                if !year_ok(m.release_year, target.year) {
                    continue;
                }
                let sim = title_similarity(&m.name, &target.name);
                let provider_tmdb = self.provider_tmdb(&m.id).await;
                if let Some((confidence, method)) =
                    classify_match(provider_tmdb, target.tmdb_id, sim, true)
                {
                    let _ = db::canonical::match_put(
                        pool,
                        &ContentMatch {
                            provider_id: pid.clone(),
                            content_type: "movie".into(),
                            content_id: m.id.clone(),
                            imdb_id: target.imdb_id.clone(),
                            tmdb_id: provider_tmdb.or(target.tmdb_id),
                            confidence,
                            method: method.into(),
                            matched_at: now_unix(),
                        },
                    )
                    .await;
                    matched.push((m.id, confidence));
                }
            }
        }

        // Build a candidate per matched movie row.
        let mut out = Vec::new();
        for (content_id, confidence) in matched {
            if let Ok(Some(movie)) = db::catalog::movie_by_id(pool, pid, &content_id).await {
                out.push(StreamCandidate {
                    source: self.provider.name.clone(),
                    provider_id: Some(pid.clone()),
                    content_type: "movie".into(),
                    content_id: Some(content_id),
                    url: None,
                    quality: parse_quality(&movie.name),
                    container: Some(movie.container_ext),
                    confidence,
                    needs_debrid: false,
                });
            }
        }
        out
    }

    /// The provider episode id + container for `(season, episode)` of a matched
    /// provider series, fetching the episode list on demand for Xtream.
    async fn episode_id_for(
        &self,
        pool: &SqlitePool,
        series_id: &str,
        season: i64,
        episode: i64,
    ) -> Option<(String, String)> {
        let mut eps = db::catalog::episodes_for_series(pool, &self.provider.id, series_id)
            .await
            .unwrap_or_default();
        if eps.is_empty() {
            if let Some((server, username, password)) = self.xtream_creds() {
                let creds = xtream::XtreamCreds {
                    server_url: &server,
                    username: &username,
                    password: &password,
                };
                if let Ok(info) = xtream::fetch_series_info(&creds, series_id).await {
                    let _ = db::catalog::replace_series_episodes(
                        pool,
                        &self.provider.id,
                        series_id,
                        &info.episodes,
                    )
                    .await;
                    eps = info.episodes;
                }
            }
        }
        // Map the canonical (season, episode) numbers onto the provider's.
        eps.into_iter()
            .find(|e| e.season == season && e.episode == episode)
            .map(|e| (e.id, e.container_ext))
    }

    /// Resolve one episode of a canonical series. Series carry no tmdb backstop,
    /// so the series-level match is name+year (plus the manual override); the
    /// episode is then addressed by `(season, episode)`.
    async fn resolve_series(&self, pool: &SqlitePool, target: &CanonicalRef) -> Vec<StreamCandidate> {
        let (Some(season), Some(episode)) = (target.season, target.episode) else {
            return Vec::new(); // a specific episode is required
        };
        let pid = &self.provider.id;

        let mut series: Vec<(String, f64)> = db::canonical::matches_for_imdb(
            pool,
            &target.imdb_id,
            "series",
            std::slice::from_ref(pid),
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| (m.content_id, m.confidence))
        .collect();

        if series.is_empty() {
            let shortlist = db::catalog::search_catalog(
                pool,
                std::slice::from_ref(pid),
                &target.name,
                SearchContentType::Series,
                None,
                SHORTLIST,
            )
            .await
            .map(|r| r.series)
            .unwrap_or_default();
            for s in shortlist {
                if !year_ok(s.release_year, target.year) {
                    continue;
                }
                let sim = title_similarity(&s.name, &target.name);
                if let Some((confidence, method)) = classify_match(None, None, sim, true) {
                    let _ = db::canonical::match_put(
                        pool,
                        &ContentMatch {
                            provider_id: pid.clone(),
                            content_type: "series".into(),
                            content_id: s.id.clone(),
                            imdb_id: target.imdb_id.clone(),
                            tmdb_id: None,
                            confidence,
                            method: method.into(),
                            matched_at: now_unix(),
                        },
                    )
                    .await;
                    series.push((s.id, confidence));
                }
            }
        }

        let mut out = Vec::new();
        for (series_id, confidence) in series {
            if let Some((episode_id, container)) =
                self.episode_id_for(pool, &series_id, season, episode).await
            {
                out.push(StreamCandidate {
                    source: self.provider.name.clone(),
                    provider_id: Some(pid.clone()),
                    content_type: "episode".into(),
                    content_id: Some(episode_id),
                    url: None,
                    quality: None,
                    container: Some(container),
                    confidence,
                    needs_debrid: false,
                });
            }
        }
        out
    }
}

impl StreamResolver for ProviderResolver {
    fn label(&self) -> &str {
        &self.provider.name
    }
    async fn resolve(&self, pool: &SqlitePool, target: &CanonicalRef) -> Vec<StreamCandidate> {
        match target.kind.as_str() {
            "movie" => self.resolve_movie(pool, target).await,
            "series" => self.resolve_series(pool, target).await,
            _ => Vec::new(),
        }
    }
}

/// One installed Stremio stream addon resolved to its (token-bearing) base URL.
/// `base_url` is a secret — never logged.
pub struct AddonSource {
    pub name: String,
    pub base_url: String,
}

/// Resolve sources for a canonical title across the given IPTV providers **and**
/// Stremio addons (the registry), ranked best-confidence first. Addon failures
/// degrade to the other sources (Milestone 41).
pub async fn resolve_sources(
    pool: &SqlitePool,
    target: &CanonicalRef,
    providers: &[Provider],
    addons: &[AddonSource],
) -> Vec<StreamCandidate> {
    let mut out = Vec::new();
    for provider in providers {
        let resolver = ProviderResolver {
            provider: provider.clone(),
        };
        out.extend(resolver.resolve(pool, target).await);
    }

    // Stremio addons (M41): each resolves the canonical id to direct streams.
    // Movies query by imdb id; series need a specific episode (`imdb:s:e`).
    let (request_type, content_type) = match target.kind.as_str() {
        "series" => ("series", "episode"),
        _ => ("movie", "movie"),
    };
    let stremio_id = match (target.season, target.episode) {
        (Some(s), Some(e)) if request_type == "series" => {
            format!("{}:{}:{}", target.imdb_id, s, e)
        }
        _ => target.imdb_id.clone(),
    };
    let query_addons = request_type != "series" || (target.season.is_some() && target.episode.is_some());
    if query_addons {
        for addon in addons {
            out.extend(
                crate::canonical::stremio::fetch_streams(
                    &addon.base_url,
                    request_type,
                    &stremio_id,
                    content_type,
                    &addon.name,
                )
                .await,
            );
        }
    }

    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}
