//! Stremio addon management commands (Milestone 41). Add-by-URL with the
//! token-bearing manifest URL stored in the OS keychain; list and remove. The
//! manifest URL is never returned to the frontend or logged. Stream resolution
//! joins the M40 source picker in slice 2.

use crate::canonical::stremio;
use crate::db::{self, Db};
use crate::keychain;
use crate::models::StremioAddon;
use tauri::State;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Add a Stremio stream addon by manifest URL. Validates the manifest, stores the
/// (token-bearing) URL in the keychain, and persists only the non-secret declared
/// metadata. The URL is never returned or logged.
#[tauri::command]
pub async fn add_stremio_addon(
    state: State<'_, Db>,
    manifest_url: String,
) -> Result<StremioAddon, String> {
    let url = manifest_url.trim().to_string();
    if url.is_empty() {
        return Err("Enter an addon manifest URL.".into());
    }
    let manifest = stremio::fetch_manifest(&url).await?;
    stremio::validate(&manifest)?;

    let id = uuid::Uuid::new_v4().to_string();
    // Secret first; if the row insert fails we roll it back below.
    let manifest_ref = keychain::store_addon_secret(&id, &url)?;
    let addon = StremioAddon {
        id: id.clone(),
        name: manifest.name,
        types: manifest.types,
        resources: manifest.resources,
        id_prefixes: manifest.id_prefixes,
        position: db::stremio::next_position(&state.0)
            .await
            .map_err(|e| format!("Failed to save the addon: {e}"))?,
        created_at: now_unix(),
    };
    if let Err(e) = db::stremio::insert(&state.0, &addon, &manifest_ref).await {
        let _ = keychain::delete_addon_secret(&id);
        return Err(format!("Failed to save the addon: {e}"));
    }
    Ok(addon)
}

#[tauri::command]
pub async fn list_stremio_addons(state: State<'_, Db>) -> Result<Vec<StremioAddon>, String> {
    db::stremio::list(&state.0)
        .await
        .map_err(|e| format!("Failed to load addons: {e}"))
}

#[tauri::command]
pub async fn remove_stremio_addon(state: State<'_, Db>, id: String) -> Result<(), String> {
    db::stremio::delete(&state.0, &id)
        .await
        .map_err(|e| format!("Failed to remove the addon: {e}"))?;
    let _ = keychain::delete_addon_secret(&id);
    Ok(())
}
