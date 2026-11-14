#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod collections;
mod commands;
mod login;
mod steam_scan;

use hangar_core::accounts::epic::EpicClient;
use hangar_core::accounts::epic_api::HttpEpicApi;
use hangar_core::accounts::gog::GogClient;
use hangar_core::accounts::gog_api::HttpGogApi;
use hangar_core::accounts::steam::SteamAccount;
use hangar_core::accounts::ubisoft::UbiClient;
use hangar_core::accounts::ubisoft_api::HttpUbiApi;
use hangar_core::accounts::store::SecretStore;
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
            app.manage(commands::LastScan::default());
            let new_store = || -> Box<dyn SecretStore> {
                #[cfg(windows)]
                { Box::new(hangar_core::accounts::store::DpapiStore::new(dir.join("accounts"))) }
                #[cfg(not(windows))]
                { Box::new(hangar_core::accounts::store::MemoryStore::default()) }
            };
            app.manage(EpicClient::new(Box::new(HttpEpicApi), new_store()));
            app.manage(GogClient::new(Box::new(HttpGogApi), new_store()));
            app.manage(SteamAccount::new(new_store()));
            app.manage(UbiClient::new(Box::new(HttpUbiApi), new_store()));
            Ok(())
        })
        .invoke_handler({
            #[cfg(debug_assertions)]
            { tauri::generate_handler![
            login::debug_epic_probe,
            login::debug_ubisoft_probe,
            accounts::debug_ubisoft_graphql,
            accounts::debug_ubisoft_demux,
            commands::get_library,
            commands::rescan,
            commands::galaxy_changed_since_scan,
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
            accounts::get_accounts,
            accounts::connect_account,
            accounts::disconnect_account,
            collections::get_filters,
            collections::create_filter,
            collections::update_filter,
            collections::delete_filter,
            accounts::get_gog_source,
            accounts::set_gog_source,
            commands::get_steam_advanced,
            commands::set_steam_advanced,
            collections::get_collections,
            collections::create_collection,
            collections::rename_collection,
            collections::delete_collection,
            collections::set_in_collection,
            collections::get_theme,
            collections::open_theme_file,
            collections::reset_theme,
            collections::get_ui_prefs,
            collections::set_ui_pref,
            ] }
            #[cfg(not(debug_assertions))]
            { tauri::generate_handler![
            commands::get_library,
            commands::rescan,
            commands::galaxy_changed_since_scan,
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
            accounts::get_accounts,
            accounts::connect_account,
            accounts::disconnect_account,
            collections::get_filters,
            collections::create_filter,
            collections::update_filter,
            collections::delete_filter,
            accounts::get_gog_source,
            accounts::set_gog_source,
            commands::get_steam_advanced,
            commands::set_steam_advanced,
            collections::get_collections,
            collections::create_collection,
            collections::rename_collection,
            collections::delete_collection,
            collections::set_in_collection,
            collections::get_theme,
            collections::open_theme_file,
            collections::reset_theme,
            collections::get_ui_prefs,
            collections::set_ui_pref,
            ] }
        })
        .run(tauri::generate_context!())
        .expect("error while running Hangar");
}
