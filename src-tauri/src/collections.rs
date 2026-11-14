use hangar_core::db::{Collection, NameError, SavedFilter};
use hangar_core::model::Platform;
use hangar_core::theme::{default_file, read_file, ThemeRead};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::commands::{dberr, lock, Dbs};

fn name_err(e: NameError) -> String {
    match e {
        NameError::Empty => "Koleksiyon adı boş olamaz".to_string(),
        NameError::Duplicate => "Bu adda bir koleksiyon zaten var".to_string(),
    }
}

fn filter_name_err(e: NameError) -> String {
    match e {
        NameError::Empty => "Filtre adı boş olamaz".to_string(),
        NameError::Duplicate => "Bu adda bir filtre zaten var".to_string(),
    }
}

#[tauri::command]
pub fn get_filters(db: Dbs) -> Result<Vec<SavedFilter>, String> {
    lock(&db)?.filters().map_err(dberr)
}

#[tauri::command]
pub fn create_filter(db: Dbs, name: String, rule: String) -> Result<i64, String> {
    lock(&db)?.create_filter(&name, &rule).map_err(dberr)?.map_err(filter_name_err)
}

#[tauri::command]
pub fn update_filter(db: Dbs, id: i64, name: String, rule: String) -> Result<(), String> {
    lock(&db)?.update_filter(id, &name, &rule).map_err(dberr)?.map_err(filter_name_err)
}

#[tauri::command]
pub fn delete_filter(db: Dbs, id: i64) -> Result<(), String> {
    lock(&db)?.delete_filter(id).map_err(dberr)
}

#[tauri::command]
pub fn get_collections(db: Dbs) -> Result<Vec<Collection>, String> {
    lock(&db)?.collections().map_err(dberr)
}

#[tauri::command]
pub fn create_collection(db: Dbs, name: String) -> Result<i64, String> {
    lock(&db)?.create_collection(&name).map_err(dberr)?.map_err(name_err)
}

#[tauri::command]
pub fn rename_collection(db: Dbs, id: i64, name: String) -> Result<(), String> {
    lock(&db)?.rename_collection(id, &name).map_err(dberr)?.map_err(name_err)
}

#[tauri::command]
pub fn delete_collection(db: Dbs, id: i64) -> Result<(), String> {
    lock(&db)?.delete_collection(id).map_err(dberr)
}

#[tauri::command]
pub fn set_in_collection(db: Dbs, id: i64, entries: Vec<(Platform, String)>, member: bool) -> Result<(), String> {
    lock(&db)?.set_in_collection(id, &entries, member).map_err(dberr)
}

fn theme_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("theme.json"))
}

#[tauri::command]
pub fn get_theme(app: AppHandle) -> Result<ThemeRead, String> {
    Ok(read_file(&theme_path(&app)?))
}

#[tauri::command]
pub fn open_theme_file(app: AppHandle) -> Result<(), String> {
    let path = theme_path(&app)?;
    if !path.exists() {
        std::fs::write(&path, default_file()).map_err(|e| format!("Tema dosyası yazılamadı: {e}"))?;
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Tema dosyası açılamadı: {e}"))
}

#[tauri::command]
pub fn reset_theme(app: AppHandle) -> Result<ThemeRead, String> {
    let path = theme_path(&app)?;
    std::fs::write(&path, default_file()).map_err(|e| format!("Tema dosyası yazılamadı: {e}"))?;
    Ok(read_file(&path))
}

#[derive(Serialize)]
pub struct UiPrefs {
    view: String,
    tab: String,
    installed: bool,
    filter: String,
    sidebar: String,
}

fn ui_prefs_from(get: impl Fn(&str) -> Option<String>) -> UiPrefs {
    let val = |k: &str, d: &str| get(k).filter(|s| !s.is_empty()).unwrap_or_else(|| d.to_string());
    UiPrefs {
        view: val("ui.view", "grid"),
        tab: val("ui.tab", "all"),
        installed: val("ui.installed", "0") == "1",
        filter: get("ui.filter").unwrap_or_default(),
        sidebar: val("ui.sidebar", "open"),
    }
}

fn check_ui_key(key: &str) -> Result<(), String> {
    if matches!(key, "ui.view" | "ui.tab" | "ui.installed" | "ui.filter" | "ui.sidebar") {
        Ok(())
    } else {
        Err("Bilinmeyen ayar".to_string())
    }
}

#[tauri::command]
pub fn get_ui_prefs(db: Dbs) -> Result<UiPrefs, String> {
    let g = lock(&db)?;
    Ok(ui_prefs_from(|k| g.get_setting(k).ok().flatten()))
}

#[tauri::command]
pub fn set_ui_pref(db: Dbs, key: String, value: String) -> Result<(), String> {
    check_ui_key(&key)?;
    lock(&db)?.set_setting(&key, &value).map_err(dberr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_prefs_defaults() {
        let p = ui_prefs_from(|_| None);
        assert_eq!((p.view.as_str(), p.tab.as_str(), p.installed), ("grid", "all", false));
        assert_eq!((p.filter.as_str(), p.sidebar.as_str()), ("", "open"));
    }

    #[test]
    fn ui_prefs_installed_one_is_true() {
        let p = ui_prefs_from(|k| (k == "ui.installed").then(|| "1".to_string()));
        assert!(p.installed);
        let p = ui_prefs_from(|k| (k == "ui.installed").then(|| "0".to_string()));
        assert!(!p.installed);
    }

    #[test]
    fn ui_key_whitelist() {
        assert!(check_ui_key("ui.view").is_ok());
        assert!(check_ui_key("ui.tab").is_ok());
        assert!(check_ui_key("ui.installed").is_ok());
        assert!(check_ui_key("ui.filter").is_ok());
        assert!(check_ui_key("ui.sidebar").is_ok());
        assert_eq!(check_ui_key("steam_api_key"), Err("Bilinmeyen ayar".to_string()));
    }
}
