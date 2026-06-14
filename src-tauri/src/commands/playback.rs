//! Playback Tauri commands (spec §16): stream URL resolution, external
//! player handoff, and the `mpv_*` control surface for the built-in player.

use crate::db::{self, Db};
use crate::models::MpvState;
use crate::mpv::player::{MpvConfig, MpvPlayer};
use sqlx::{Row, SqlitePool};
use std::process::Command;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

/// Lazily-created shared player instance.
#[derive(Default)]
pub struct PlayerHandle(pub Mutex<Option<Arc<MpvPlayer>>>);

/// Native video host window (0 = not created yet). Resized from the window
/// event handler in lib.rs.
#[derive(Default)]
pub struct VideoHost(pub AtomicIsize);

pub async fn resolve_stream_url_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<String, String> {
    let sql = match content_type {
        "live" => "SELECT stream_url FROM live_channels WHERE provider_id = ? AND id = ?",
        "movie" => "SELECT stream_url FROM movies WHERE provider_id = ? AND id = ?",
        "episode" => "SELECT stream_url FROM episodes WHERE provider_id = ? AND id = ?",
        other => return Err(format!("Unknown content type: {other}")),
    };
    let row = sqlx::query(sql)
        .bind(provider_id)
        .bind(content_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to resolve the stream URL: {e}"))?;
    row.map(|r| r.get::<String, _>("stream_url"))
        .ok_or_else(|| format!("No {content_type} content with id {content_id} in the catalog."))
}

/// Build the (executable, args) for an external player without spawning it.
pub fn resolve_external_player_command(
    player: &str,
    custom_command: Option<&str>,
    stream_url: &str,
) -> Result<(String, Vec<String>), String> {
    match player {
        "mpv" => Ok(("mpv".into(), vec![stream_url.to_string()])),
        "vlc" => {
            let candidates: &[&str] = if cfg!(target_os = "windows") {
                &[
                    "vlc",
                    r"C:\Program Files\VideoLAN\VLC\vlc.exe",
                    r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe",
                ]
            } else {
                &["vlc", "/Applications/VLC.app/Contents/MacOS/VLC"]
            };
            let exe = candidates
                .iter()
                .find(|c| !c.contains(['\\', '/']) || std::path::Path::new(c).exists())
                .copied()
                .ok_or("Could not find VLC. Install it or choose a different player.")?;
            Ok((exe.to_string(), vec![stream_url.to_string()]))
        }
        "custom" => {
            let template = custom_command
                .filter(|c| !c.trim().is_empty())
                .ok_or("No custom player command is configured in Settings.")?;
            let resolved = template.replace("{url}", stream_url);
            let parts = split_command_line(&resolved);
            let (exe, args) = parts
                .split_first()
                .ok_or("The custom player command is empty.")?;
            Ok((exe.clone(), args.to_vec()))
        }
        other => Err(format!("Unknown external player: {other}")),
    }
}

/// Minimal quote-aware splitter for the custom player command template.
fn split_command_line(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in input.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

pub async fn open_in_external_player_impl(
    pool: &SqlitePool,
    stream_url: &str,
    player: Option<String>,
) -> Result<(), String> {
    let player = match player {
        Some(p) => p,
        None => db::settings::get(pool, "default_external_player")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "mpv".to_string()),
    };
    let custom = db::settings::get(pool, "custom_player_command")
        .await
        .ok()
        .flatten();
    let (exe, args) = resolve_external_player_command(&player, custom.as_deref(), stream_url)?;
    Command::new(&exe)
        .args(&args)
        .spawn()
        .map_err(|e| format!("Failed to launch {exe}: {e}"))?;
    Ok(())
}

/// Get or lazily create the shared player. The video host window must be
/// created on the main thread; the mpv instance itself is thread-agnostic.
async fn ensure_player(app: &AppHandle) -> Result<Arc<MpvPlayer>, String> {
    if let Some(player) = app.state::<PlayerHandle>().0.lock().unwrap().clone() {
        return Ok(player);
    }

    let wid = ensure_video_host(app).await?;

    let pool = app.state::<Db>().0.clone();
    let hwdec = !matches!(
        db::settings::get(&pool, "hw_decode_enabled").await,
        Ok(Some(v)) if v == "false"
    );

    let emitter = app.clone();
    let player = MpvPlayer::new(
        MpvConfig {
            wid,
            hwdec,
            headless: false,
        },
        Box::new(move |state| {
            // Self-healing glue: reassert the video window's position right
            // below the app window a few times per second during playback.
            #[cfg(target_os = "windows")]
            {
                let host = emitter.state::<VideoHost>().0.load(Ordering::SeqCst);
                if host != 0 {
                    if let Some(window) = emitter.get_webview_window("main") {
                        if let Ok(parent) = window.hwnd() {
                            crate::mpv::video_host::fit_to_parent(host, parent.0 as isize);
                        }
                    }
                }
            }
            let _ = emitter.emit("mpv:state_changed", &state);
        }),
    )?;

    // macOS: mpv created its own video window; glue it behind the app window
    // before we hand back the player so the first frames land in the right
    // place. (No-op once the host window is already attached.)
    #[cfg(target_os = "macos")]
    glue_video_window(app).await;

    let slot = app.state::<PlayerHandle>();
    let mut guard = slot.0.lock().unwrap();
    // A concurrent call may have won the race; prefer the stored instance.
    if let Some(existing) = guard.clone() {
        return Ok(existing);
    }
    *guard = Some(player.clone());
    Ok(player)
}

#[cfg(target_os = "windows")]
async fn ensure_video_host(app: &AppHandle) -> Result<Option<isize>, String> {
    let host_state = app.state::<VideoHost>();
    let existing = host_state.0.load(Ordering::SeqCst);
    if existing != 0 {
        return Ok(Some(existing));
    }
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    let parent = window
        .hwnd()
        .map_err(|e| format!("could not get the window handle: {e}"))?
        .0 as isize;

    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::mpv::video_host::create(parent));
    })
    .map_err(|e| format!("could not reach the main thread: {e}"))?;
    let host = rx
        .recv()
        .map_err(|_| "video host creation did not respond")??;
    host_state.0.store(host, Ordering::SeqCst);
    Ok(Some(host))
}

#[cfg(not(target_os = "windows"))]
async fn ensure_video_host(_app: &AppHandle) -> Result<Option<isize>, String> {
    // macOS doesn't embed via `wid` (see `glue_video_window`); Linux embedding
    // is not implemented yet. Either way, mpv gets no host window handle here.
    Ok(None)
}

/// Run `f` on the AppKit main thread and wait for its result. AppKit calls
/// (window/level changes) must not happen off the main thread.
#[cfg(target_os = "macos")]
async fn run_on_main<T: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|e| format!("could not reach the main thread: {e}"))?;
    rx.recv()
        .map_err(|_| "the main thread did not respond".to_string())
}

/// macOS: mpv renders into its own window (`--force-window`); wait for it to
/// appear, then glue it behind the app window and remember it so the window
/// event handler can keep it fitted. Best-effort — if it never shows we let
/// playback proceed rather than block it.
#[cfg(target_os = "macos")]
async fn glue_video_window(app: &AppHandle) {
    let host_state = app.state::<VideoHost>();
    if host_state.0.load(Ordering::SeqCst) != 0 {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let main = match window.ns_window() {
        Ok(ptr) => ptr as isize,
        Err(_) => return,
    };
    // force-window=immediate makes mpv's window appear within ~1s; poll for it.
    for _ in 0..60 {
        if let Ok(found) =
            run_on_main(app, move || crate::mpv::video_host::find_video_window(main)).await
        {
            if found != 0 {
                let _ = run_on_main(app, move || crate::mpv::video_host::glue(main, found)).await;
                host_state.0.store(found, Ordering::SeqCst);
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn with_player<T>(
    app: &AppHandle,
    f: impl FnOnce(&Arc<MpvPlayer>) -> Result<T, String>,
) -> Result<T, String> {
    let slot = app.state::<PlayerHandle>();
    let guard = slot.0.lock().unwrap();
    match guard.as_ref() {
        Some(player) => f(player),
        None => Err("The player is not running.".into()),
    }
}

#[tauri::command]
pub async fn resolve_stream_url(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
) -> Result<String, String> {
    resolve_stream_url_impl(&state.0, &provider_id, &content_type, &content_id).await
}

#[tauri::command]
pub async fn open_in_external_player(
    state: State<'_, Db>,
    stream_url: String,
    player: Option<String>,
) -> Result<(), String> {
    open_in_external_player_impl(&state.0, &stream_url, player).await
}

#[tauri::command]
pub async fn mpv_load_url(
    app: AppHandle,
    url: String,
    start_seconds: Option<f64>,
) -> Result<(), String> {
    let player = ensure_player(&app).await?;
    player.load_url(&url, start_seconds)
}

#[tauri::command]
pub async fn mpv_play(app: AppHandle) -> Result<(), String> {
    with_player(&app, |p| p.play())
}

#[tauri::command]
pub async fn mpv_pause(app: AppHandle) -> Result<(), String> {
    with_player(&app, |p| p.pause())
}

#[tauri::command]
pub async fn mpv_stop(app: AppHandle) -> Result<(), String> {
    with_player(&app, |p| p.stop())
}

#[tauri::command]
pub async fn mpv_seek(app: AppHandle, seconds: f64) -> Result<(), String> {
    with_player(&app, |p| p.seek(seconds))
}

#[tauri::command]
pub async fn mpv_set_volume(app: AppHandle, volume: f64) -> Result<(), String> {
    with_player(&app, |p| p.set_volume(volume))
}

#[tauri::command]
pub async fn mpv_set_mute(app: AppHandle, muted: bool) -> Result<(), String> {
    with_player(&app, |p| p.set_mute(muted))
}

#[tauri::command]
pub async fn mpv_set_audio_track(app: AppHandle, track_id: i64) -> Result<(), String> {
    with_player(&app, |p| p.set_audio_track(track_id))
}

#[tauri::command]
pub async fn mpv_set_subtitle_track(app: AppHandle, track_id: i64) -> Result<(), String> {
    with_player(&app, |p| p.set_subtitle_track(track_id))
}

#[tauri::command]
pub async fn mpv_get_state(app: AppHandle) -> Result<MpvState, String> {
    with_player(&app, |p| Ok(p.get_state()))
}
