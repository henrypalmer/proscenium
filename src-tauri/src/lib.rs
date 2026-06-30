pub mod canonical;
pub mod commands;
pub mod db;
pub mod iptv;
pub mod keychain;
pub mod models;
pub mod mpv;

use std::time::Instant;
use tauri::Manager;

fn app_data_dir(handle: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    // Spec §15: $APPDATA/proscenium on Windows,
    // ~/Library/Application Support/proscenium on macOS.
    handle
        .path()
        .data_dir()
        .ok()
        .map(|d| d.join("proscenium"))
}

pub fn run() {
    let started = Instant::now();

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app_data_dir(&app.handle().clone())
                .ok_or("could not resolve the platform app data directory")?;
            let db_path = data_dir.join("proscenium.db");
            let pool = tauri::async_runtime::block_on(db::init(&db_path))?;
            app.manage(db::Db(pool));
            // Image cache (spec §5.7, Milestone 27): cached art lives next to the
            // DB and is served to the WebView via the asset protocol, so the
            // images directory must be inside the asset scope.
            let images_dir = data_dir.join("images");
            std::fs::create_dir_all(&images_dir).ok();
            app.asset_protocol_scope()
                .allow_directory(&images_dir, true)
                .ok();
            app.manage(commands::images::ImageCache::new(
                images_dir,
                iptv::http_client().map_err(|e| e.to_string())?,
            ));
            app.manage(commands::catalog::RefreshGuard::default());
            app.manage(commands::catalog::DetailCache::default());
            app.manage(commands::playback::PlayerHandle::default());
            app.manage(commands::playback::VideoHost::default());
            // Render compositor (Milestone 37): one GL context + render thread
            // compositing N mpv tiles into the host surface (Windows + macOS).
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            app.manage(commands::playback::CompositorState::default());
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            app.manage(commands::playback::PrimaryTile::default());
            // Multi-view tile registry (Milestone 37).
            app.manage(commands::multiview::MultiView::default());
            // Background stale-cache check (spec §5.2 startup trigger).
            tauri::async_runtime::spawn(commands::catalog::startup_stale_check(
                app.handle().clone(),
            ));
            // Provider reachability / subscription banner (spec §12).
            tauri::async_runtime::spawn(commands::providers::startup_provider_status_check(
                app.handle().clone(),
            ));
            // Image cache eviction: drop entries past their 30-day TTL (spec §5.7).
            tauri::async_runtime::spawn(commands::settings::startup_image_cache_eviction(
                app.handle().clone(),
            ));
            // Windows (Milestone 38): disable the DWM maximize/restore/fullscreen
            // transition animation on the main window. The video renders into a
            // separate top-level window (mpv::video_host) that we resize in one
            // SetWindowPos step, so it snaps to the final size immediately; with
            // the animation on, the main-window frame lags behind and the video
            // briefly pokes past it. Disabling transitions keeps the two in sync.
            #[cfg(target_os = "windows")]
            {
                use windows_sys::Win32::Graphics::Dwm::{
                    DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
                };
                if let Some(win) = app.get_webview_window("main") {
                    if let Ok(hwnd) = win.hwnd() {
                        let disable: i32 = 1; // BOOL TRUE
                        unsafe {
                            DwmSetWindowAttribute(
                                hwnd.0 as isize as *mut core::ffi::c_void,
                                DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                                &disable as *const i32 as *const core::ffi::c_void,
                                std::mem::size_of::<i32>() as u32,
                            );
                        }
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::providers::upsert_provider,
            commands::providers::list_providers,
            commands::providers::delete_provider,
            commands::providers::test_provider_connection,
            commands::providers::check_provider_status,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::images::resolve_cached_image,
            commands::images::cache_image,
            commands::images::image_cache_size,
            commands::images::clear_image_cache,
            commands::canonical::get_canonical_genres,
            commands::canonical::get_canonical_catalog,
            commands::canonical::get_canonical_meta,
            commands::canonical::resolve_sources,
            commands::canonical::set_manual_match,
            commands::canonical::record_source_pick,
            commands::stremio::add_stremio_addon,
            commands::stremio::list_stremio_addons,
            commands::stremio::remove_stremio_addon,
            commands::catalog::get_active_provider,
            commands::catalog::set_active_provider,
            commands::catalog::get_enabled_providers,
            commands::catalog::set_enabled_providers,
            commands::catalog::refresh_catalog,
            commands::catalog::get_catalog_summary,
            commands::catalog::get_live_categories,
            commands::catalog::get_live_channels,
            commands::catalog::get_vod_categories,
            commands::catalog::get_movies,
            commands::catalog::get_series_categories,
            commands::catalog::get_series,
            commands::catalog::get_episodes,
            commands::catalog::get_movie_detail,
            commands::catalog::get_series_detail,
            commands::catalog::get_related,
            commands::catalog::record_recent_channel,
            commands::catalog::get_recent_channels,
            commands::catalog::get_category_order,
            commands::catalog::set_category_order,
            commands::search::search,
            commands::playback::resolve_stream_url,
            commands::playback::open_in_external_player,
            commands::playback::diagnose_playback_failure,
            commands::playback::mpv_load_url,
            commands::playback::mpv_play,
            commands::playback::mpv_pause,
            commands::playback::mpv_stop,
            commands::playback::mpv_seek,
            commands::playback::mpv_set_volume,
            commands::playback::mpv_set_mute,
            commands::playback::mpv_set_audio_track,
            commands::playback::mpv_set_subtitle_track,
            commands::playback::mpv_get_state,
            commands::multiview::mv_add_tile,
            commands::multiview::mv_remove_tile,
            commands::multiview::mv_set_rects,
            commands::multiview::mv_set_active_audio,
            commands::multiview::mv_set_volume,
            commands::multiview::mv_close,
            commands::watch::get_watch_progress,
            commands::watch::get_canonical_progress,
            commands::watch::set_watch_progress,
            commands::watch::mark_watched,
            commands::watch::list_watch_progress,
            commands::watch::get_continue_watching,
            commands::watch::clear_watch_progress,
            commands::lists::create_list,
            commands::lists::rename_list,
            commands::lists::delete_list,
            commands::lists::reorder_lists,
            commands::lists::get_lists,
            commands::lists::add_to_list,
            commands::lists::remove_from_list,
            commands::lists::reorder_list_items,
            commands::lists::get_list_items,
            commands::lists::get_lists_for_item,
        ])
        .on_window_event(|window, event| {
            // Keep the native video window glued behind the app window.
            #[cfg(target_os = "windows")]
            if matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::Focused(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                use std::sync::atomic::Ordering;
                use tauri::Manager;
                let host = window
                    .app_handle()
                    .state::<commands::playback::VideoHost>()
                    .0
                    .load(Ordering::SeqCst);
                if host != 0 {
                    if let Ok(parent) = window.hwnd() {
                        mpv::video_host::fit_to_parent(host, parent.0 as isize);
                    }
                }
            }
            // macOS: keep mpv's glued child window fitted to the app window's
            // content area. Plain moves are handled by child-window tracking;
            // these events change the content size.
            #[cfg(target_os = "macos")]
            if matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                use std::sync::atomic::Ordering;
                use tauri::Manager;
                let mpv = window
                    .app_handle()
                    .state::<commands::playback::VideoHost>()
                    .0
                    .load(Ordering::SeqCst);
                if mpv != 0 {
                    if let Ok(main) = window.ns_window() {
                        mpv::video_host::fit_to_parent(mpv, main as isize);
                    }
                }
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            let _ = (window, event);
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |handle, event| {
            if let tauri::RunEvent::Ready = event {
                // Startup-time record for the <2s cold-start acceptance check.
                if let Some(dir) = app_data_dir(handle) {
                    let _ = std::fs::write(
                        dir.join("startup.log"),
                        format!("ready_in_ms={}\n", started.elapsed().as_millis()),
                    );
                }
            }
        });
}
