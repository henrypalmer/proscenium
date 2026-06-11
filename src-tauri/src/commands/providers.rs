//! Provider Tauri commands (spec §16). The `_impl` functions hold the actual
//! logic so integration tests can exercise them without a Tauri runtime.

use crate::db::{self, Db};
use crate::iptv::{m3u, xtream};
use crate::keychain;
use crate::models::{ConnectionTestResult, Provider, ProviderInput, ProviderType};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn validate(input: &ProviderInput, is_new: bool) -> Result<(), String> {
    if input.name.trim().is_empty() {
        return Err("Provider name is required.".into());
    }
    match input.provider_type {
        ProviderType::Xtream => {
            if input.server_url.as_deref().unwrap_or("").trim().is_empty() {
                return Err("Server URL is required for Xtream providers.".into());
            }
            if input.username.as_deref().unwrap_or("").trim().is_empty() {
                return Err("Username is required for Xtream providers.".into());
            }
            if is_new && input.password.as_deref().unwrap_or("").is_empty() {
                return Err("Password is required for Xtream providers.".into());
            }
        }
        ProviderType::M3u => {
            let has_url = !input.playlist_url.as_deref().unwrap_or("").trim().is_empty();
            let has_file = !input
                .local_file_path
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty();
            if !has_url && !has_file {
                return Err(
                    "A playlist URL or local file path is required for M3U providers.".into(),
                );
            }
        }
    }
    Ok(())
}

pub async fn upsert_provider_impl(
    pool: &SqlitePool,
    mut input: ProviderInput,
) -> Result<Provider, String> {
    let is_new = input.id.is_none();
    validate(&input, is_new)?;

    let id = input
        .id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // The secret goes to the OS keychain; only the reference key reaches SQLite.
    let password = input.password.take().filter(|p| !p.is_empty());
    let password_ref = match (input.provider_type, password) {
        (ProviderType::Xtream, Some(secret)) => Some(keychain::store_secret(&id, &secret)?),
        _ => None,
    };

    db::providers::upsert(pool, &id, &input, password_ref.as_deref(), now_unix())
        .await
        .map_err(|e| format!("Failed to save provider: {e}"))
}

pub async fn list_providers_impl(pool: &SqlitePool) -> Result<Vec<Provider>, String> {
    db::providers::list(pool)
        .await
        .map_err(|e| format!("Failed to load providers: {e}"))
}

pub async fn delete_provider_impl(pool: &SqlitePool, provider_id: &str) -> Result<(), String> {
    db::providers::delete(pool, provider_id)
        .await
        .map_err(|e| format!("Failed to delete provider: {e}"))?;
    // Credential cleanup is best-effort; the provider row is already gone.
    let _ = keychain::delete_secret(provider_id);
    Ok(())
}

pub async fn test_provider_connection_impl(
    input: ProviderInput,
) -> Result<ConnectionTestResult, String> {
    match input.provider_type {
        ProviderType::Xtream => {
            let server_url = input
                .server_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or("Server URL is required to test the connection.")?;
            let username = input
                .username
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or("Username is required to test the connection.")?;
            // For saved providers the form omits the password; fall back to
            // the keychain entry stored at save time.
            let password = match input.password.clone().filter(|p| !p.is_empty()) {
                Some(p) => p,
                None => input
                    .id
                    .as_deref()
                    .and_then(|id| keychain::get_secret(id).ok())
                    .ok_or("Password is required to test the connection.")?,
            };
            Ok(xtream::test_connection(server_url, username, &password).await)
        }
        ProviderType::M3u => {
            if let Some(url) = input
                .playlist_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Ok(m3u::test_playlist_url(url).await)
            } else if let Some(path) = input
                .local_file_path
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Ok(m3u::test_local_file(path))
            } else {
                Err("A playlist URL or local file path is required to test the connection.".into())
            }
        }
    }
}

#[tauri::command]
pub async fn upsert_provider(
    state: State<'_, Db>,
    provider: ProviderInput,
) -> Result<Provider, String> {
    upsert_provider_impl(&state.0, provider).await
}

#[tauri::command]
pub async fn list_providers(state: State<'_, Db>) -> Result<Vec<Provider>, String> {
    list_providers_impl(&state.0).await
}

#[tauri::command]
pub async fn delete_provider(state: State<'_, Db>, provider_id: String) -> Result<(), String> {
    delete_provider_impl(&state.0, &provider_id).await
}

#[tauri::command]
pub async fn test_provider_connection(
    provider: ProviderInput,
) -> Result<ConnectionTestResult, String> {
    test_provider_connection_impl(provider).await
}
