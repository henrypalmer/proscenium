//! Multi-view commands (Milestone 37). Additional live-TV tiles drawn by the
//! shared compositor alongside the single (primary) player. The primary is tile
//! id **0** (its player lives in `playback::PlayerHandle`); secondary tiles get
//! ids 1.. and live in this registry. Each secondary emits its own
//! `mpv:tile_state` so its grid cell can show buffering/error independently.
//!
//! Supported on Windows + macOS (both render through `mpv::compositor`); the
//! commands return an error on other platforms and the frontend gates entry to
//! those two.

use crate::models::TileRect;
use crate::mpv::player::MpvPlayer;
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tauri::{AppHandle, Emitter, Manager};

/// Hard ceiling = the 2×2 grid. The provider's `max_connections` is deliberately
/// NOT enforced (its semantics are fuzzy — often IP/MAC scoped); a provider that
/// refuses an extra stream surfaces as that tile's own error instead.
const MAX_TILES: u32 = 4;

/// Registry of secondary multi-view tiles (the primary, tile 0, is the single
/// player in `playback::PlayerHandle`).
pub struct MultiView(pub Mutex<MvInner>);

impl Default for MultiView {
    fn default() -> Self {
        Self(Mutex::new(MvInner::default()))
    }
}

pub struct MvInner {
    tiles: Vec<MvTile>,
    next_id: u32,
    /// Tile id that currently has audio (0 = primary); exactly one is unmuted.
    active_audio: u32,
}

impl Default for MvInner {
    fn default() -> Self {
        Self {
            tiles: Vec::new(),
            next_id: 1,
            active_audio: 0,
        }
    }
}

#[allow(dead_code)] // fields used only on the compositor render path (Windows + macOS)
struct MvTile {
    id: u32,
    player: Arc<MpvPlayer>,
    comp_tile: u64,
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn unsupported() -> String {
    "Multi-view is currently available on Windows and macOS only.".to_string()
}

// --- commands (cross-platform wrappers; Windows-only behavior) ---

#[tauri::command]
pub async fn mv_add_tile(
    app: AppHandle,
    provider_id: String,
    content_id: String,
) -> Result<u32, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        imp::add_tile(&app, provider_id, content_id).await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (&app, provider_id, content_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_remove_tile(app: AppHandle, tile_id: u32) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        imp::remove_tile(&app, tile_id);
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (&app, tile_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_set_rects(app: AppHandle, rects: Vec<TileRect>) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        imp::set_rects(&app, rects);
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (&app, rects);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_set_active_audio(app: AppHandle, tile_id: u32) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        imp::set_active_audio(&app, tile_id)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (&app, tile_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_set_volume(app: AppHandle, tile_id: u32, volume: f64) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        imp::set_volume(&app, tile_id, volume)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (&app, tile_id, volume);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_close(app: AppHandle) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        imp::close(&app);
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = &app;
        Err(unsupported())
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod imp {
    use super::{MultiView, MvTile, MAX_TILES};
    use crate::commands::playback::{self, CompositorState, PlayerHandle};
    use crate::db::Db;
    use crate::models::{TileRect, TileState};
    use crate::mpv::compositor::Rect;
    use tauri::{AppHandle, Emitter, Manager};

    pub(super) async fn add_tile(
        app: &AppHandle,
        provider_id: String,
        content_id: String,
    ) -> Result<u32, String> {
        // Only the 2×2 grid is capped (primary + secondaries). The provider's
        // own connection limit is left to the provider — if it refuses the extra
        // stream, that tile shows its own classified error instead.
        {
            let mv = app.state::<MultiView>();
            let reg = mv.0.lock().unwrap();
            let in_use = 1 + reg.tiles.len() as u32;
            if in_use >= MAX_TILES {
                return Err(format!("Multi-view shows up to {MAX_TILES} streams at once."));
            }
        }

        let pool = app.state::<Db>().0.clone();
        let url = playback::resolve_stream_url_impl(&pool, &provider_id, "live", &content_id).await?;

        let id = {
            let mv = app.state::<MultiView>();
            let mut reg = mv.0.lock().unwrap();
            let id = reg.next_id;
            reg.next_id += 1;
            id
        };

        let emitter = app.clone();
        let on_state: crate::mpv::player::StateCallback = Box::new(move |state| {
            let _ = emitter.emit("mpv:tile_state", TileState { tile_id: id, state });
        });
        // Placeholder rect; the grid reports the tile's real fractional rect via
        // mv_set_rects the moment it mounts (before any frames flow).
        let placeholder = Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        };
        let (player, comp_tile) =
            playback::spawn_compositor_tile(app, placeholder, on_state).await?;
        // New tiles start muted — only the active-audio tile has sound.
        let _ = player.set_mute(true);
        player.load_url(&url, None)?;

        let mv = app.state::<MultiView>();
        mv.0.lock().unwrap().tiles.push(MvTile {
            id,
            player,
            comp_tile,
        });
        Ok(id)
    }

    pub(super) fn remove_tile(app: &AppHandle, tile_id: u32) {
        // Take the tile out under the lock, then drop the player outside it (its
        // drop blocks while the compositor frees the render context).
        let removed = {
            let mv = app.state::<MultiView>();
            let mut reg = mv.0.lock().unwrap();
            let removed = reg
                .tiles
                .iter()
                .position(|t| t.id == tile_id)
                .map(|pos| reg.tiles.remove(pos));
            if reg.active_audio == tile_id {
                reg.active_audio = 0;
            }
            removed
        };
        drop(removed);
        apply_audio_focus(app);
    }

    pub(super) fn set_rects(app: &AppHandle, rects: Vec<TileRect>) {
        let compositor = app.state::<CompositorState>().0.lock().unwrap().clone();
        let mv = app.state::<MultiView>();
        let reg = mv.0.lock().unwrap();
        for r in rects {
            let rect = Rect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            };
            if r.tile_id == 0 {
                playback::set_primary_rect(app, Some(rect));
            } else if let Some(t) = reg.tiles.iter().find(|t| t.id == r.tile_id) {
                if let Some(c) = &compositor {
                    c.set_rect(t.comp_tile, rect);
                }
            }
        }
    }

    pub(super) fn set_active_audio(app: &AppHandle, tile_id: u32) -> Result<(), String> {
        {
            let mv = app.state::<MultiView>();
            let mut reg = mv.0.lock().unwrap();
            if tile_id != 0 && !reg.tiles.iter().any(|t| t.id == tile_id) {
                return Err("no such tile".into());
            }
            reg.active_audio = tile_id;
        }
        apply_audio_focus(app);
        Ok(())
    }

    pub(super) fn set_volume(app: &AppHandle, tile_id: u32, volume: f64) -> Result<(), String> {
        if tile_id == 0 {
            if let Some(p) = app.state::<PlayerHandle>().0.lock().unwrap().clone() {
                return p.set_volume(volume);
            }
            return Ok(());
        }
        let mv = app.state::<MultiView>();
        let reg = mv.0.lock().unwrap();
        let t = reg
            .tiles
            .iter()
            .find(|t| t.id == tile_id)
            .ok_or("no such tile")?;
        t.player.set_volume(volume)
    }

    pub(super) fn close(app: &AppHandle) {
        let removed: Vec<MvTile> = {
            let mv = app.state::<MultiView>();
            let mut reg = mv.0.lock().unwrap();
            reg.active_audio = 0;
            reg.next_id = 1;
            std::mem::take(&mut reg.tiles)
        };
        drop(removed); // tear down the players (frees their compositor tiles)
                       // Restore the primary tile to fill the window and give it audio back.
        playback::set_primary_rect(app, None);
        if let Some(p) = app.state::<PlayerHandle>().0.lock().unwrap().clone() {
            let _ = p.set_mute(false);
        }
    }

    /// Mute every tile except the one with audio focus (0 = primary).
    fn apply_audio_focus(app: &AppHandle) {
        let mv = app.state::<MultiView>();
        let reg = mv.0.lock().unwrap();
        let active = reg.active_audio;
        if let Some(p) = app.state::<PlayerHandle>().0.lock().unwrap().clone() {
            let _ = p.set_mute(active != 0);
        }
        for t in reg.tiles.iter() {
            let _ = t.player.set_mute(active != t.id);
        }
    }
}
