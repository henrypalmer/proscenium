//! Playback Tauri commands (spec §16): stream URL resolution, external
//! player handoff, and the `mpv_*` control surface for the built-in player.

use crate::db::{self, Db};
use crate::models::{MpvState, ProviderType};
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

/// The render compositor (Milestone 37): one GL context + render thread drawing
/// N mpv render contexts into the host surface. Lazily created with the first
/// player; shared across all tiles. Windows + macOS.
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Default)]
pub(crate) struct CompositorState(pub Mutex<Option<Arc<crate::mpv::compositor::Compositor>>>);

/// The compositor tile id of the single/primary player (Milestone 37), so
/// multi-view can give it a rect (tile 0) and restore it to fill on exit.
/// 0 = none.
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Default)]
pub(crate) struct PrimaryTile(pub std::sync::atomic::AtomicU64);

/// Resolve the playable URL for a catalog item (spec §16, Milestone 21).
///
/// The provider password is never persisted in the catalog (§5.1). For Xtream
/// the URL is composed here from `server_url`/`username` + the **keychain**
/// secret + the item's stream id and container extension. For M3U the stored
/// `stream_url` is the provider's own direct URL (no app-injected secret) and is
/// returned as-is.
pub async fn resolve_stream_url_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
) -> Result<String, String> {
    // `kind` is the Xtream path segment; `ext_col` is the per-table extension
    // column (live uses `stream_ext`, VOD/episodes use `container_ext`).
    let (kind, ext_col, table) = match content_type {
        "live" => ("live", "stream_ext", "live_channels"),
        "movie" => ("movie", "container_ext", "movies"),
        "episode" => ("series", "container_ext", "episodes"),
        other => return Err(format!("Unknown content type: {other}")),
    };

    let row = sqlx::query(&format!(
        "SELECT stream_url, {ext_col} AS ext FROM {table} WHERE provider_id = ? AND id = ?"
    ))
    .bind(provider_id)
    .bind(content_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to resolve the stream URL: {e}"))?
    .ok_or_else(|| format!("No {content_type} content with id {content_id} in the catalog."))?;

    let stored: String = row.get("stream_url");
    let ext: String = row.get("ext");

    let provider = db::providers::get(pool, provider_id)
        .await
        .map_err(|e| format!("Failed to load provider: {e}"))?
        .ok_or_else(|| format!("Provider {provider_id} does not exist."))?;

    match provider.provider_type {
        // Provider-supplied direct URL; carries no app-injected secret.
        ProviderType::M3u => {
            if stored.is_empty() {
                return Err("This item has no playable URL.".into());
            }
            Ok(stored)
        }
        // Compose `{base}/{kind}/{user}/{password}/{id}.{ext}` from the keychain
        // secret. The password is used transiently and never logged or stored.
        ProviderType::Xtream => {
            let base = provider
                .server_url
                .as_deref()
                .map(|s| s.trim_end_matches('/'))
                .filter(|s| !s.is_empty())
                .ok_or("Provider has no server URL.")?;
            let user = provider
                .username
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or("Provider has no username.")?;
            let password = crate::keychain::get_secret(provider_id)?;
            Ok(format!("{base}/{kind}/{user}/{password}/{content_id}.{ext}"))
        }
    }
}

// --- Stream-failure diagnosis (spec §12, Milestone 22) ---

/// Outcome of the lightweight reachability probe.
enum ProbeOutcome {
    /// The provider answered with this HTTP status.
    Status(u16),
    /// The request timed out before any response.
    Timeout,
    /// Connection/DNS/TLS failure — the provider could not be reached.
    Network,
}

/// Map a probe outcome to a user-facing reason for a failed stream load,
/// distinguishing 4xx / 5xx / network / timeout (spec §12). Reachable (2xx/3xx)
/// means the bytes flow but mpv couldn't play them — usually an unsupported
/// container/codec rather than a provider block.
fn classify_failure(outcome: &ProbeOutcome) -> String {
    match outcome {
        ProbeOutcome::Timeout => {
            "The stream timed out before any data arrived. Check your connection, or the provider may be slow — try again.".into()
        }
        ProbeOutcome::Network => {
            "Could not reach the provider for this stream. Check your internet connection.".into()
        }
        ProbeOutcome::Status(403) => {
            "Provider denied this video (HTTP 403). Live TV is unaffected — VOD may be temporarily restricted by the provider.".into()
        }
        ProbeOutcome::Status(401) => {
            "The provider rejected the request (HTTP 401 — authentication). Re-check the provider's credentials in Settings.".into()
        }
        ProbeOutcome::Status(404) => {
            "This video was not found on the provider (HTTP 404). The catalog may be out of date — try refreshing it.".into()
        }
        ProbeOutcome::Status(s) if (400..500).contains(s) => {
            format!("The provider refused this video (HTTP {s}).")
        }
        ProbeOutcome::Status(s) if (500..600).contains(s) => {
            format!("The provider had a server error (HTTP {s}). Try again later.")
        }
        ProbeOutcome::Status(s) => {
            format!("The stream is reachable (HTTP {s}) but could not be played — the format may be unsupported.")
        }
    }
}

/// Mask provider credentials before a URL is logged (spec §5.1 / Milestone 21).
/// Replaces the keychain password (Xtream path-embedded) and any `password=`
/// query value (M3U `get.php` style) with `***`.
pub fn redact_secrets(url: &str, password: Option<&str>) -> String {
    let mut out = url.to_string();
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        out = out.replace(pw, "***");
    }
    if let Some(start) = out.to_lowercase().find("password=") {
        let val_start = start + "password=".len();
        let val_end = out[val_start..]
            .find('&')
            .map(|i| val_start + i)
            .unwrap_or(out.len());
        out.replace_range(val_start..val_end, "***");
    }
    out
}

/// Probe the stream URL for reachability without downloading it: a 1-byte
/// ranged GET surfaces the provider's HTTP status (or a network/timeout class).
async fn probe_stream(url: &str) -> ProbeOutcome {
    let client = match crate::iptv::http_client() {
        Ok(c) => c,
        Err(_) => return ProbeOutcome::Network,
    };
    match client
        .get(url)
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await
    {
        Ok(resp) => ProbeOutcome::Status(resp.status().as_u16()),
        Err(e) if e.is_timeout() => ProbeOutcome::Timeout,
        Err(_) => ProbeOutcome::Network,
    }
}

/// Diagnose why a stream failed to load and return a user-facing reason
/// (spec §12, Milestone 22). Re-resolves the URL, probes the provider, logs a
/// secret-redacted diagnostic line (failing URL + HTTP status + mpv error), and
/// classifies the failure. Never returns an error — it always yields a message
/// to show the user.
pub async fn diagnose_playback_failure_impl(
    pool: &SqlitePool,
    provider_id: &str,
    content_type: &str,
    content_id: &str,
    mpv_error: Option<&str>,
) -> String {
    let url = match resolve_stream_url_impl(pool, provider_id, content_type, content_id).await {
        Ok(u) => u,
        // URL resolution itself failed — that message is already user-facing.
        Err(e) => return e,
    };

    let password = crate::keychain::get_secret(provider_id).ok();
    let redacted = redact_secrets(&url, password.as_deref());
    let mpv_detail = mpv_error.map(str::trim).filter(|e| !e.is_empty());

    let outcome = probe_stream(&url).await;
    let status_log = match &outcome {
        ProbeOutcome::Status(s) => format!("HTTP {s}"),
        ProbeOutcome::Timeout => "timeout".into(),
        ProbeOutcome::Network => "network error".into(),
    };

    // Diagnostic log line (secret-redacted) so a failed load is field-debuggable.
    eprintln!(
        "[playback] stream failure: url={redacted} status={status_log} mpv={}",
        mpv_detail.unwrap_or("(none)")
    );

    classify_failure(&outcome)
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

/// Get or lazily create the shared player. The video host surface must be
/// created on the main thread; the mpv instance itself is thread-agnostic.
async fn ensure_player(app: &AppHandle) -> Result<Arc<MpvPlayer>, String> {
    if let Some(player) = app.state::<PlayerHandle>().0.lock().unwrap().clone() {
        return Ok(player);
    }

    // Create the platform host surface on the main thread, stored in `VideoHost`
    // for the window-event re-fit. Windows: a WS_POPUP host window (HWND). macOS:
    // a borderless NSWindow + NSOpenGLContext + view glued behind the app window,
    // whose (context, view) drive the compositor below.
    #[cfg(target_os = "windows")]
    let host_hwnd = ensure_video_host(app).await?;
    #[cfg(target_os = "macos")]
    let gl_host = ensure_gl_host(app).await?;

    let pool = app.state::<Db>().0.clone();
    let hwdec = !matches!(
        db::settings::get(&pool, "hw_decode_enabled").await,
        Ok(Some(v)) if v == "false"
    );

    let emitter = app.clone();
    let player = MpvPlayer::new(
        MpvConfig {
            composited: cfg!(any(target_os = "windows", target_os = "macos")),
            hwdec,
            headless: false,
        },
        Box::new(move |state| {
            // Windows self-healing glue: reassert the host window's position right
            // below the app window a few times per second during playback. macOS
            // tracks moves via the child-window attachment + the `on_window_event`
            // re-fit, so it needs no per-frame glue here.
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

    let slot = app.state::<PlayerHandle>();
    let mut guard = slot.0.lock().unwrap();
    // A concurrent call may have won the race; prefer the stored instance.
    if let Some(existing) = guard.clone() {
        return Ok(existing);
    }

    // Register this player with the shared compositor as a full-window tile (the
    // N=1 single-player case), and arrange for its render context to be freed
    // before the handle is destroyed (ordered teardown). Windows + macOS.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        #[cfg(target_os = "windows")]
        let compositor = ensure_compositor(app, host_hwnd, &player)?;
        #[cfg(target_os = "macos")]
        let compositor = ensure_compositor(app, gl_host.0, gl_host.1, &player)?;

        let tile = compositor.add(player.raw_handle(), None)?;
        app.state::<PrimaryTile>().0.store(tile, Ordering::SeqCst);
        let comp = compositor.clone();
        let app_for_drop = app.clone();
        player.set_pre_terminate(Box::new(move || {
            comp.remove(tile);
            app_for_drop
                .state::<PrimaryTile>()
                .0
                .store(0, Ordering::SeqCst);
        }));
    }

    *guard = Some(player.clone());
    Ok(player)
}

/// Windows: get or lazily create the shared compositor on the host window. The
/// first player's libmpv API is reused (its render-context functions are global).
#[cfg(target_os = "windows")]
fn ensure_compositor(
    app: &AppHandle,
    host_hwnd: isize,
    player: &Arc<MpvPlayer>,
) -> Result<Arc<crate::mpv::compositor::Compositor>, String> {
    let state = app.state::<CompositorState>();
    let mut guard = state.0.lock().unwrap();
    if let Some(c) = guard.clone() {
        return Ok(c);
    }
    let comp = Arc::new(crate::mpv::compositor::Compositor::new(host_hwnd, player.api())?);
    *guard = Some(comp.clone());
    Ok(comp)
}

/// macOS: get or lazily create the shared compositor on the `(NSOpenGLContext,
/// NSView)` host created on the main thread (`ensure_gl_host`). The first
/// player's libmpv API is reused (its render-context functions are global).
#[cfg(target_os = "macos")]
fn ensure_compositor(
    app: &AppHandle,
    gl_context: isize,
    gl_view: isize,
    player: &Arc<MpvPlayer>,
) -> Result<Arc<crate::mpv::compositor::Compositor>, String> {
    let state = app.state::<CompositorState>();
    let mut guard = state.0.lock().unwrap();
    if let Some(c) = guard.clone() {
        return Ok(c);
    }
    let comp = Arc::new(crate::mpv::compositor::Compositor::new(
        gl_context,
        gl_view,
        player.api(),
    )?);
    *guard = Some(comp.clone());
    Ok(comp)
}

/// Spawn an additional compositor-backed video player at `rect` (Milestone 37
/// multi-view). The host window + compositor must already exist (multi-view is
/// always entered from single playback). Returns the player and its compositor
/// tile id; the caller loads the stream and tracks the tile. The player frees
/// its compositor tile on drop (ordered teardown via the pre-terminate hook).
#[cfg(target_os = "windows")]
pub(crate) async fn spawn_compositor_tile(
    app: &AppHandle,
    rect: crate::mpv::compositor::Rect,
    on_state: crate::mpv::player::StateCallback,
) -> Result<(Arc<MpvPlayer>, u64), String> {
    let host = app.state::<VideoHost>().0.load(Ordering::SeqCst);
    if host == 0 {
        return Err("the video host window is not initialized".into());
    }
    let compositor = app
        .state::<CompositorState>()
        .0
        .lock()
        .unwrap()
        .clone()
        .ok_or("the compositor is not running")?;

    let pool = app.state::<Db>().0.clone();
    let hwdec = !matches!(
        db::settings::get(&pool, "hw_decode_enabled").await,
        Ok(Some(v)) if v == "false"
    );

    let player = MpvPlayer::new(
        MpvConfig {
            composited: true,
            hwdec,
            headless: false,
        },
        on_state,
    )?;
    let comp_tile = compositor.add(player.raw_handle(), Some(rect))?;
    let comp = compositor.clone();
    player.set_pre_terminate(Box::new(move || comp.remove(comp_tile)));
    Ok((player, comp_tile))
}

/// Set the destination rect of the primary (single-player) tile, or restore it
/// to fill — used by multi-view to lay out / release tile 0.
#[cfg(target_os = "windows")]
pub(crate) fn set_primary_rect(app: &AppHandle, rect: Option<crate::mpv::compositor::Rect>) {
    let primary = app.state::<PrimaryTile>().0.load(Ordering::SeqCst);
    if primary == 0 {
        return;
    }
    if let Some(comp) = app.state::<CompositorState>().0.lock().unwrap().clone() {
        match rect {
            Some(r) => comp.set_rect(primary, r),
            None => comp.set_fill(primary),
        }
    }
}

#[cfg(target_os = "windows")]
async fn ensure_video_host(app: &AppHandle) -> Result<isize, String> {
    let host_state = app.state::<VideoHost>();
    let existing = host_state.0.load(Ordering::SeqCst);
    if existing != 0 {
        return Ok(existing);
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
    Ok(host)
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

/// macOS (Milestone 37): create our borderless GL host window + NSOpenGLContext
/// on the main thread and glue it behind the app window, remembering the window
/// in `VideoHost` so the window-event handler can keep it fitted. Returns the
/// `(context, view)` pointers, which drive the shared `mpv::compositor`.
#[cfg(target_os = "macos")]
async fn ensure_gl_host(app: &AppHandle) -> Result<(isize, isize), String> {
    let host_state = app.state::<VideoHost>();
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    let main = window
        .ns_window()
        .map_err(|e| format!("could not get the ns_window handle: {e}"))? as isize;

    let (host_window, gl_context, gl_view) =
        run_on_main(app, move || crate::mpv::render_mac::create_gl_host(main)).await??;
    host_state.0.store(host_window, Ordering::SeqCst);
    Ok((gl_context, gl_view))
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
pub async fn diagnose_playback_failure(
    state: State<'_, Db>,
    provider_id: String,
    content_type: String,
    content_id: String,
    mpv_error: Option<String>,
) -> Result<String, String> {
    Ok(diagnose_playback_failure_impl(
        &state.0,
        &provider_id,
        &content_type,
        &content_id,
        mpv_error.as_deref(),
    )
    .await)
}

#[tauri::command]
pub async fn mpv_load_url(
    app: AppHandle,
    url: String,
    start_seconds: Option<f64>,
) -> Result<(), String> {
    let player = ensure_player(&app).await?;
    // Apply the current Hardware-decode setting before loading so the toggle
    // takes effect on the next stream (the player instance is reused, so reading
    // it only at creation would freeze the setting until restart).
    let pool = app.state::<Db>().0.clone();
    let hwdec = !matches!(
        db::settings::get(&pool, "hw_decode_enabled").await,
        Ok(Some(v)) if v == "false"
    );
    let _ = player.set_hwdec(hwdec);
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
