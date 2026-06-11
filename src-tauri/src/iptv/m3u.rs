//! M3U playlist support. Milestone 1 covers connection testing only; the
//! full parser arrives in Milestone 2.

use crate::models::ConnectionTestResult;
use std::path::Path;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

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
