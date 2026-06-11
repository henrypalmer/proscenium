use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Xtream,
    M3u,
}

impl ProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderType::Xtream => "xtream",
            ProviderType::M3u => "m3u",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "xtream" => Some(ProviderType::Xtream),
            "m3u" => Some(ProviderType::M3u),
            _ => None,
        }
    }
}

/// Provider profile as returned to the frontend. The real password never
/// crosses the IPC boundary; it lives in the OS keychain.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub playlist_url: Option<String>,
    pub local_file_path: Option<String>,
    /// Unix seconds.
    pub last_refreshed: Option<i64>,
    /// Unix seconds.
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub server_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub playlist_url: Option<String>,
    pub local_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XtreamAccountInfo {
    pub status: Option<String>,
    /// Unix seconds.
    pub exp_date: Option<i64>,
    pub max_connections: Option<i64>,
    pub active_connections: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    pub account_info: Option<XtreamAccountInfo>,
}

impl ConnectionTestResult {
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            account_info: None,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            account_info: None,
        }
    }
}
