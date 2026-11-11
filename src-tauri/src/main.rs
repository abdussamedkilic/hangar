#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use hangar_core::db::Db;
use std::sync::Mutex;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("hangar.db"))?;
            app.manage(Mutex::new(db));
            app.manage(commands::CoversDir(dir.join("covers")));
            app.manage(commands::DbPath(dir.join("hangar.db")));
            app.manage(commands::CoverJob::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_library,
            commands::rescan,
            commands::launch,
            commands::set_preferred,
            commands::set_cover,
            commands::get_settings,
            commands::save_settings,
            commands::split_game,
            commands::merge_games,
            commands::add_local_game,
            commands::update_local_game,
            commands::remove_local_game,
            commands::get_scan_folders,
            commands::set_scan_folders,
            commands::scan_candidates,
            commands::add_local_games,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hangar");
}
