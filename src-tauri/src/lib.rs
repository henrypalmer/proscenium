pub mod commands;
pub mod db;
pub mod iptv;
pub mod keychain;
pub mod models;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::providers::upsert_provider,
            commands::providers::list_providers,
            commands::providers::delete_provider,
            commands::providers::test_provider_connection,
        ])
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
