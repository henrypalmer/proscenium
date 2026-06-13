//! Settings Tauri commands (spec §16): the `get_settings` / `set_setting`
//! pair backing the Settings UI, plus the startup image-cache eviction task.

use crate::db::{self, Db};
use crate::models::AppSettings;
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

/// The setting keys the UI is allowed to write (spec §15). Anything else is
/// rejected so a typo can't quietly create a junk row.
const WRITABLE_KEYS: &[&str] = &[
    "cache_ttl_hours",
    "default_external_player",
    "custom_player_command",
    "ui_density",
    "ui_theme",
    "hw_decode_enabled",
];

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Read every settings row, falling back to the §15 defaults for any key the
/// user has not set yet.
pub async fn get_settings_impl(pool: &SqlitePool) -> Result<AppSettings, String> {
    let mut settings = AppSettings::default();
    let read = |key: &'static str| async move {
        db::settings::get(pool, key)
            .await
            .map_err(|e| format!("Failed to read settings: {e}"))
    };

    settings.active_provider_id = read("active_provider_id").await?;
    if let Some(v) = read("cache_ttl_hours").await? {
        if let Ok(n) = v.parse() {
            settings.cache_ttl_hours = n;
        }
    }
    if let Some(v) = read("default_external_player").await? {
        settings.default_external_player = v;
    }
    settings.custom_player_command = read("custom_player_command").await?;
    if let Some(v) = read("ui_density").await? {
        settings.ui_density = v;
    }
    if let Some(v) = read("ui_theme").await? {
        settings.ui_theme = v;
    }
    if let Some(v) = read("hw_decode_enabled").await? {
        settings.hw_decode_enabled = v != "false";
    }
    Ok(settings)
}

pub async fn set_setting_impl(pool: &SqlitePool, key: &str, value: &str) -> Result<(), String> {
    if !WRITABLE_KEYS.contains(&key) {
        return Err(format!("'{key}' is not a writable setting."));
    }
    db::settings::set(pool, key, value)
        .await
        .map_err(|e| format!("Failed to save settings: {e}"))
}

/// Startup image-cache eviction (spec §5.7 / §15): drop rows whose 30-day TTL
/// has elapsed and delete the backing files from disk. Returns the number of
/// entries removed.
pub async fn evict_stale_images(pool: &SqlitePool, now: i64) -> Result<usize, String> {
    let stale = db::image_cache::expired(pool, now)
        .await
        .map_err(|e| format!("Failed to read the image cache: {e}"))?;
    for path in &stale {
        let _ = std::fs::remove_file(path);
    }
    db::image_cache::delete_expired(pool, now)
        .await
        .map_err(|e| format!("Failed to evict stale images: {e}"))?;
    Ok(stale.len())
}

/// Spawned on launch from `lib.rs` setup.
pub async fn startup_image_cache_eviction(app: AppHandle) {
    let pool = app.state::<Db>().0.clone();
    match evict_stale_images(&pool, now_unix()).await {
        Ok(n) if n > 0 => eprintln!("image cache: evicted {n} stale entr(y/ies)"),
        Ok(_) => {}
        Err(e) => eprintln!("image cache eviction failed: {e}"),
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, Db>) -> Result<AppSettings, String> {
    get_settings_impl(&state.0).await
}

#[tauri::command]
pub async fn set_setting(state: State<'_, Db>, key: String, value: String) -> Result<(), String> {
    set_setting_impl(&state.0, &key, &value).await
}
