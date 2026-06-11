pub mod m3u;
pub mod xtream;

use std::time::Duration;

pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to initialize HTTP client: {e}"))
}
