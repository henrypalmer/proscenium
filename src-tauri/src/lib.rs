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
        .setup(|app| {
            let data_dir = app_data_dir(&app.handle().clone())
                .ok_or("could not resolve the platform app data directory")?;
            let db_path = data_dir.join("proscenium.db");
            let pool = tauri::async_runtime::block_on(db::init(&db_path))?;
            app.manage(db::Db(pool));
            app.manage(commands::catalog::RefreshGuard::default());
            app.manage(commands::playback::PlayerHandle::default());
            app.manage(commands::playback::VideoHost::default());
            // Background stale-cache check (spec §5.2 startup trigger).
            tauri::async_runtime::spawn(commands::catalog::startup_stale_check(
                app.handle().clone(),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::providers::upsert_provider,
            commands::providers::list_providers,
            commands::providers::delete_provider,
            commands::providers::test_provider_connection,
            commands::catalog::get_active_provider,
            commands::catalog::set_active_provider,
            commands::catalog::refresh_catalog,
            commands::catalog::get_catalog_summary,
            commands::catalog::get_live_categories,
            commands::catalog::get_live_channels,
            commands::playback::resolve_stream_url,
            commands::playback::open_in_external_player,
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
            #[cfg(not(target_os = "windows"))]
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
