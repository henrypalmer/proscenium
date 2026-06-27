//! Multi-view commands (Milestone 37, Windows). Additional live-TV tiles drawn
//! by the shared compositor alongside the single (primary) player. The primary
//! is tile id **0** (its player lives in `playback::PlayerHandle`); secondary
//! tiles get ids 1.. and live in this registry. Each secondary emits its own
//! `mpv:tile_state` so its grid cell can show buffering/error independently.
//!
//! Multi-view is Windows-only this milestone — the commands return an error on
//! other platforms (the frontend gates entry on Windows).

use crate::models::{MultiViewBudget, TileRect};
use crate::mpv::player::MpvPlayer;
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tauri::{AppHandle, Emitter, Manager};

/// Hard ceiling regardless of provider connections (spec §M37).
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
    /// Effective cap = `min(4, provider max_connections)` (refreshed by
    /// `mv_get_budget`); the primary counts as one against it.
    cap: u32,
    /// Tile id that currently has audio (0 = primary); exactly one is unmuted.
    active_audio: u32,
}

impl Default for MvInner {
    fn default() -> Self {
        Self {
            tiles: Vec::new(),
            next_id: 1,
            cap: MAX_TILES,
            active_audio: 0,
        }
    }
}

#[allow(dead_code)] // fields used only on the Windows render path
struct MvTile {
    id: u32,
    player: Arc<MpvPlayer>,
    comp_tile: u64,
}

#[cfg(not(target_os = "windows"))]
fn unsupported() -> String {
    "Multi-view is currently available on Windows only.".to_string()
}

// --- commands (cross-platform wrappers; Windows-only behavior) ---

#[tauri::command]
pub async fn mv_get_budget(app: AppHandle, provider_id: String) -> Result<MultiViewBudget, String> {
    #[cfg(target_os = "windows")]
    {
        win::get_budget(&app, &provider_id).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, &provider_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_add_tile(
    app: AppHandle,
    provider_id: String,
    content_id: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<u32, String> {
    #[cfg(target_os = "windows")]
    {
        win::add_tile(&app, provider_id, content_id, x, y, w, h).await
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, provider_id, content_id, x, y, w, h);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_remove_tile(app: AppHandle, tile_id: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::remove_tile(&app, tile_id);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, tile_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_set_rects(app: AppHandle, rects: Vec<TileRect>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::set_rects(&app, rects);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, rects);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_set_active_audio(app: AppHandle, tile_id: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::set_active_audio(&app, tile_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, tile_id);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_set_volume(app: AppHandle, tile_id: u32, volume: f64) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::set_volume(&app, tile_id, volume)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&app, tile_id, volume);
        Err(unsupported())
    }
}

#[tauri::command]
pub async fn mv_close(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        win::close(&app);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = &app;
        Err(unsupported())
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::{MultiView, MvTile, MAX_TILES};
    use crate::commands::playback::{self, CompositorState, PlayerHandle};
    use crate::db::{self, Db};
    use crate::models::{ProviderType, TileRect, TileState};
    use crate::mpv::compositor::Rect;
    use tauri::{AppHandle, Emitter, Manager};

    pub(super) async fn get_budget(
        app: &AppHandle,
        provider_id: &str,
    ) -> Result<crate::models::MultiViewBudget, String> {
        let pool = app.state::<Db>().0.clone();
        let max = fetch_max_connections(&pool, provider_id).await;
        let cap = match max {
            Some(m) if m > 0 => (m as u32).min(MAX_TILES),
            _ => MAX_TILES,
        };
        let state = app.state::<MultiView>();
        let mut reg = state.0.lock().unwrap();
        reg.cap = cap;
        let in_use = 1 + reg.tiles.len() as u32;
        Ok(crate::models::MultiViewBudget {
            cap,
            in_use,
            max_connections: max,
        })
    }

    async fn fetch_max_connections(pool: &sqlx::SqlitePool, provider_id: &str) -> Option<i64> {
        let provider = db::providers::get(pool, provider_id).await.ok()??;
        match provider.provider_type {
            ProviderType::Xtream => {
                let server = provider.server_url.as_deref()?;
                let user = provider.username.as_deref()?;
                let pass = crate::keychain::get_secret(provider_id).ok()?;
                let r = crate::iptv::xtream::test_connection(server, user, &pass).await;
                r.account_info.and_then(|a| a.max_connections)
            }
            ProviderType::M3u => None,
        }
    }

    pub(super) async fn add_tile(
        app: &AppHandle,
        provider_id: String,
        content_id: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> Result<u32, String> {
        // Enforce the connection budget (primary + existing secondaries).
        {
            let mv = app.state::<MultiView>();
            let reg = mv.0.lock().unwrap();
            let in_use = 1 + reg.tiles.len() as u32;
            if in_use >= reg.cap {
                return Err(format!(
                    "Connection limit reached — {} of {} streams in use.",
                    in_use, reg.cap
                ));
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
        let (player, comp_tile) =
            playback::spawn_compositor_tile(app, Rect { x, y, w, h }, on_state).await?;
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
