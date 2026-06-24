//! M3U playlist support: connection testing, playlist download (with gzip
//! support), and `#EXTINF` parsing with content-type inference (spec §5.2/§6).

use crate::models::{
    CatalogData, Category, ConnectionTestResult, EpisodeItem, LiveChannel, MovieItem, SeriesItem,
};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// Download a playlist. Returns raw bytes; callers pass them through
/// [`decode_playlist_bytes`] which handles gzip.
pub async fn fetch_playlist_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = super::http_client()?;
    let response = client.get(url).send().await.map_err(|_| {
        format!("Could not connect to {url}. Check the playlist URL and your internet connection.")
    })?;
    if !response.status().is_success() {
        return Err(format!(
            "The playlist server responded with HTTP {}.",
            response.status()
        ));
    }
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|_| format!("Failed to download the playlist from {url}."))
}

pub fn read_playlist_file(path: &str) -> Result<Vec<u8>, String> {
    if !Path::new(path).is_file() {
        return Err(format!("File not found: {path}. Check the file path."));
    }
    std::fs::read(path).map_err(|e| format!("Could not read {path}: {e}"))
}

/// Decode playlist bytes to text, transparently un-gzipping when the file
/// carries the gzip magic header (spec §6: gzip-compressed M3U files).
pub fn decode_playlist_bytes(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(&GZIP_MAGIC) {
        let mut decoder = flate2::read::GzDecoder::new(bytes);
        let mut out = String::new();
        decoder
            .read_to_string(&mut out)
            .map_err(|e| format!("Failed to decompress the gzip playlist: {e}"))?;
        Ok(out)
    } else {
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ExtInf {
    attrs: HashMap<String, String>,
    display_name: String,
}

#[derive(Debug, PartialEq)]
enum EntryKind {
    Live,
    Movie,
    SeriesEpisode,
}

pub struct ParseOutcome {
    pub catalog: CatalogData,
    /// Malformed `#EXTINF` lines that were skipped (spec §12: logged, not fatal).
    pub skipped_lines: usize,
}

/// Parse an M3U/M3U8 playlist into catalog data.
///
/// - `#EXTINF` attributes parsed: `tvg-id`, `tvg-name`, `tvg-logo`,
///   `group-title`, plus extended `type`/`tvg-type`, `series-id`,
///   `episode-num` (both `key="value"` and bare `key:value` forms).
/// - Content type inference: explicit `type` attribute, then `series-id`,
///   then group-title keywords, then SxxEyy patterns in the name, then the
///   stream URL's container extension.
/// - Malformed `#EXTINF` lines are skipped; URLs without a preceding
///   `#EXTINF` and entries without a URL are discarded (spec §12).
pub fn parse_playlist(text: &str) -> ParseOutcome {
    let mut builder = CatalogBuilder::default();
    let mut skipped = 0usize;
    let mut pending: Option<ExtInf> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF") {
            // A missing colon is itself a malformed line; parse_extinf will
            // reject it via the duration check.
            let rest = rest.strip_prefix(':').unwrap_or(rest);
            match parse_extinf(rest) {
                Some(info) => pending = Some(info),
                None => {
                    skipped += 1;
                    pending = None;
                }
            }
        } else if line.starts_with('#') {
            // Other directives (#EXTM3U, #EXTVLCOPT, ...) are ignored.
            continue;
        } else if let Some(info) = pending.take() {
            builder.add(info, line);
        }
    }

    ParseOutcome {
        catalog: builder.finish(),
        skipped_lines: skipped,
    }
}

/// Split an `#EXTINF` payload (`-1 tvg-id="x" group-title="y",Name`) into
/// attributes and display name. Returns `None` for lines that are malformed
/// beyond use (no duration, or unterminated quoting that swallows the line).
fn parse_extinf(rest: &str) -> Option<ExtInf> {
    // Duration must lead (integer or float, possibly negative).
    let duration_end = rest
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '-' || *c == '+' || *c == '.'))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    if duration_end == 0 || rest[..duration_end].parse::<f64>().is_err() {
        return None;
    }

    // Find the first comma outside quotes: attrs before, display name after.
    let tail = &rest[duration_end..];
    let mut in_quotes = false;
    let mut split_at = None;
    for (i, c) in tail.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                split_at = Some(i);
                break;
            }
            _ => {}
        }
    }
    let (attr_part, name_part) = match split_at {
        Some(i) => (&tail[..i], &tail[i + 1..]),
        // No display name; tolerate and fall back to attributes later.
        None => (tail, ""),
    };
    if in_quotes {
        return None; // unterminated quote — treat the line as malformed
    }

    let attrs = parse_attrs(attr_part);
    let display_name = name_part.trim().to_string();
    Some(ExtInf {
        attrs,
        display_name,
    })
}

/// Parse `key="value"` pairs and bare `key:value` tokens from the attribute
/// section of an `#EXTINF` line. Keys are lowercased.
fn parse_attrs(s: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        // Read a key: alphanumeric plus '-' and '_'.
        let key_start = i;
        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
        {
            i += 1;
        }
        if i == key_start {
            i += 1;
            continue;
        }
        let key: String = chars[key_start..i].iter().collect::<String>().to_lowercase();

        if i < chars.len() && chars[i] == '=' {
            i += 1;
            if i < chars.len() && chars[i] == '"' {
                i += 1;
                let val_start = i;
                while i < chars.len() && chars[i] != '"' {
                    i += 1;
                }
                let value: String = chars[val_start..i].iter().collect();
                i += 1; // closing quote
                attrs.insert(key, value);
            } else {
                let val_start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                attrs.insert(key, chars[val_start..i].iter().collect());
            }
        } else if i < chars.len() && chars[i] == ':' {
            // Bare `type:movie` style token used by some providers.
            i += 1;
            let val_start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            attrs.insert(key, chars[val_start..i].iter().collect());
        }
        // A bare token without '=' or ':' is ignored.
    }
    attrs
}

fn infer_kind(attrs: &HashMap<String, String>, group: &str, name: &str, url: &str) -> EntryKind {
    if let Some(t) = attrs.get("type").or_else(|| attrs.get("tvg-type")) {
        let t = t.to_lowercase();
        if t.contains("movie") || t.contains("vod") {
            return EntryKind::Movie;
        }
        if t.contains("series") {
            return EntryKind::SeriesEpisode;
        }
        if t.contains("live") {
            return EntryKind::Live;
        }
    }
    if attrs.contains_key("series-id") {
        return EntryKind::SeriesEpisode;
    }
    let g = group.to_lowercase();
    if g.contains("series") {
        return EntryKind::SeriesEpisode;
    }
    if g.contains("movie") || g.contains("vod") || g.contains("film") {
        return EntryKind::Movie;
    }
    if detect_season_episode(name).is_some() {
        return EntryKind::SeriesEpisode;
    }
    match ext_from_url(url).as_deref() {
        Some("mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv") => EntryKind::Movie,
        _ => EntryKind::Live,
    }
}

/// Detect `S01E02`-style markers (also `S1 E2`, `S01.E02`). Returns
/// (series name prefix, season, episode).
fn detect_season_episode(name: &str) -> Option<(String, i64, i64)> {
    let chars: Vec<char> = name.chars().collect();
    for i in 0..chars.len() {
        if !chars[i].eq_ignore_ascii_case(&'s') {
            continue;
        }
        if i > 0 && chars[i - 1].is_alphanumeric() {
            continue;
        }
        let mut j = i + 1;
        let mut season = String::new();
        while j < chars.len() && chars[j].is_ascii_digit() && season.len() < 2 {
            season.push(chars[j]);
            j += 1;
        }
        if season.is_empty() {
            continue;
        }
        let mut k = j;
        while k < chars.len() && matches!(chars[k], ' ' | '.' | '-' | '_') {
            k += 1;
        }
        if k >= chars.len() || !chars[k].eq_ignore_ascii_case(&'e') {
            continue;
        }
        let mut m = k + 1;
        let mut episode = String::new();
        while m < chars.len() && chars[m].is_ascii_digit() && episode.len() < 3 {
            episode.push(chars[m]);
            m += 1;
        }
        if episode.is_empty() {
            continue;
        }
        let prefix: String = chars[..i].iter().collect();
        let prefix = prefix
            .trim_end_matches([' ', '-', '_', '.', ':', '|'])
            .trim()
            .to_string();
        return Some((prefix, season.parse().ok()?, episode.parse().ok()?));
    }
    None
}

fn ext_from_url(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let segment = path.trim_end_matches('/').rsplit('/').next()?;
    let (_, ext) = segment.rsplit_once('.')?;
    if !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(ext.to_lowercase())
    } else {
        None
    }
}

/// `Movie Title (2021)` → 2021.
fn year_from_name(name: &str) -> Option<i64> {
    let trimmed = name.trim_end();
    let open = trimmed.rfind('(')?;
    let inner = trimmed[open + 1..].strip_suffix(')')?;
    if inner.len() == 4 && inner.chars().all(|c| c.is_ascii_digit()) {
        let year: i64 = inner.parse().ok()?;
        (1880..=2100).contains(&year).then_some(year)
    } else {
        None
    }
}

fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true;
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[derive(Default)]
struct CategorySet {
    list: Vec<Category>,
    index: HashMap<String, ()>,
}

impl CategorySet {
    /// Category id for M3U content is the group title itself; sort order is
    /// playlist insertion order (provider-defined ordering, spec §5.3).
    fn intern(&mut self, group: &str) -> String {
        let name = if group.is_empty() { "Uncategorized" } else { group };
        if !self.index.contains_key(name) {
            self.index.insert(name.to_string(), ());
            self.list.push(Category {
                id: name.to_string(),
                name: name.to_string(),
                sort_order: self.list.len() as i64,
            });
        }
        name.to_string()
    }
}

#[derive(Default)]
struct CatalogBuilder {
    live_cats: CategorySet,
    vod_cats: CategorySet,
    series_cats: CategorySet,
    channels: Vec<LiveChannel>,
    movies: Vec<MovieItem>,
    series_order: Vec<String>,
    series_map: HashMap<String, SeriesItem>,
    episodes: Vec<EpisodeItem>,
    counter: usize,
}

impl CatalogBuilder {
    fn add(&mut self, info: ExtInf, url: &str) {
        self.counter += 1;
        let n = self.counter;
        let group = info.attrs.get("group-title").cloned().unwrap_or_default();
        let name = if info.display_name.is_empty() {
            info.attrs
                .get("tvg-name")
                .cloned()
                .unwrap_or_else(|| format!("Item {n}"))
        } else {
            info.display_name.clone()
        };
        let logo = info.attrs.get("tvg-logo").cloned().filter(|s| !s.is_empty());

        match infer_kind(&info.attrs, &group, &name, url) {
            EntryKind::Live => {
                let category_id = self.live_cats.intern(&group);
                self.channels.push(LiveChannel {
                    id: format!("live-{n}"),
                    name,
                    category_name: category_id.clone(),
                    category_id,
                    logo_url: logo,
                    stream_url: url.to_string(),
                    stream_ext: ext_from_url(url).unwrap_or_else(|| "ts".into()),
                    epg_channel_id: info.attrs.get("tvg-id").cloned().filter(|s| !s.is_empty()),
                });
            }
            EntryKind::Movie => {
                let category_id = self.vod_cats.intern(&group);
                self.movies.push(MovieItem {
                    id: format!("movie-{n}"),
                    release_year: year_from_name(&name),
                    name,
                    category_name: category_id.clone(),
                    category_id,
                    poster_url: logo,
                    stream_url: url.to_string(),
                    container_ext: ext_from_url(url).unwrap_or_else(|| "mp4".into()),
                    rating: None,
                    added_at: None,
                });
            }
            EntryKind::SeriesEpisode => {
                let category_id = self.series_cats.intern(&group);
                let detected = detect_season_episode(&name);
                let series_name = detected
                    .as_ref()
                    .map(|(prefix, _, _)| prefix.clone())
                    .filter(|p| !p.is_empty())
                    .or_else(|| info.attrs.get("tvg-name").cloned().filter(|s| !s.is_empty()))
                    .unwrap_or_else(|| if group.is_empty() { name.clone() } else { group.clone() });
                let series_id = info
                    .attrs
                    .get("series-id")
                    .cloned()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("series-{}", slugify(&series_name)));

                self.series_map.entry(series_id.clone()).or_insert_with(|| {
                    self.series_order.push(series_id.clone());
                    SeriesItem {
                        id: series_id.clone(),
                        name: series_name,
                        category_id: category_id.clone(),
                        category_name: category_id.clone(),
                        poster_url: logo.clone(),
                        release_year: None,
                    }
                });

                let (season, episode) = match (&detected, info.attrs.get("episode-num")) {
                    (Some((_, s, e)), _) => (*s, *e),
                    (None, Some(num)) => (1, num.trim().parse().unwrap_or(n as i64)),
                    (None, None) => (1, n as i64),
                };
                self.episodes.push(EpisodeItem {
                    id: format!("ep-{n}"),
                    series_id,
                    season,
                    episode,
                    title: name,
                    stream_url: url.to_string(),
                    container_ext: ext_from_url(url).unwrap_or_else(|| "mp4".into()),
                    duration_seconds: None,
                    poster_url: logo,
                    overview: None,
                });
            }
        }
    }

    fn finish(self) -> CatalogData {
        let series_map = self.series_map;
        CatalogData {
            live_categories: self.live_cats.list,
            live_channels: self.channels,
            vod_categories: self.vod_cats.list,
            movies: self.movies,
            series_categories: self.series_cats.list,
            series: self
                .series_order
                .iter()
                .filter_map(|id| series_map.get(id).cloned())
                .collect(),
            episodes: self.episodes,
        }
    }
}

// ---------------------------------------------------------------------------
// Connection testing (Milestone 1)
// ---------------------------------------------------------------------------

/// Fetch the start of the playlist and sanity-check it looks like an M3U
/// file. Gzip-compressed playlists are accepted without decompressing here.
pub async fn test_playlist_url(url: &str) -> ConnectionTestResult {
    let client = match super::http_client() {
        Ok(c) => c,
        Err(e) => return ConnectionTestResult::failure(e),
    };

    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => {
            return ConnectionTestResult::failure(format!(
                "Could not connect to {url}. Check the playlist URL and your internet connection."
            ));
        }
    };

    if !response.status().is_success() {
        return ConnectionTestResult::failure(format!(
            "The playlist server responded with HTTP {}. Check the playlist URL.",
            response.status()
        ));
    }

    // Only the first chunk is needed to validate the header; playlists can
    // be tens of megabytes.
    let mut response = response;
    match response.chunk().await {
        Ok(Some(bytes)) => check_playlist_head(&bytes),
        Ok(None) => ConnectionTestResult::failure(
            "The playlist URL is reachable but returned an empty file.",
        ),
        Err(_) => ConnectionTestResult::failure(format!(
            "Failed to read the playlist from {url}. Check your internet connection."
        )),
    }
}

/// Validate a local playlist file exists and looks like an M3U file.
pub fn test_local_file(path: &str) -> ConnectionTestResult {
    let file_path = Path::new(path);
    if !file_path.is_file() {
        return ConnectionTestResult::failure(format!(
            "File not found: {path}. Check the file path."
        ));
    }
    match std::fs::read(file_path) {
        Ok(bytes) if bytes.is_empty() => {
            ConnectionTestResult::failure("The playlist file is empty.")
        }
        Ok(bytes) => check_playlist_head(&bytes[..bytes.len().min(4096)]),
        Err(e) => ConnectionTestResult::failure(format!("Could not read {path}: {e}")),
    }
}

fn check_playlist_head(head: &[u8]) -> ConnectionTestResult {
    if head.starts_with(&GZIP_MAGIC) {
        return ConnectionTestResult::success(
            "Playlist is reachable (gzip-compressed M3U detected).",
        );
    }
    let text = String::from_utf8_lossy(head);
    if text.contains("#EXTM3U") {
        ConnectionTestResult::success("Playlist is reachable and looks like a valid M3U file.")
    } else {
        ConnectionTestResult::failure(
            "The playlist was found but does not look like an M3U file (missing #EXTM3U header).",
        )
    }
}
