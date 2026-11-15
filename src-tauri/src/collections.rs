use hangar_core::db::{Collection, NameError, SavedFilter};
use hangar_core::model::Platform;
use hangar_core::theme::{default_file, list_names, migrate_legacy, read_file, seed_presets, validate_name, ThemeRead};
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

fn app_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn themes_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_dir(app)?.join("themes"))
}

/// Active theme name for a stored preference: `""` (default) unless the file still exists.
fn resolve_active(pref: &str, names: &[String]) -> String {
    if names.iter().any(|n| n == pref) {
        pref.to_string()
    } else {
        String::new()
    }
}

/// Runs the legacy `theme.json` migration (making the result active), then returns the effective active name and all names.
fn theme_state(app: &AppHandle, db: &Dbs) -> Result<(String, Vec<String>), String> {
    let dir = app_dir(app)?;
    let g = lock(db)?;
    if let Some(name) = migrate_legacy(&dir) {
        g.set_setting("ui.theme", &name).map_err(dberr)?;
    }
    if g.get_setting("theme.presets_seeded").ok().flatten().as_deref() != Some("1") {
        seed_presets(&dir.join("themes")).map_err(|e| format!("Tema dosyası yazılamadı: {e}"))?;
        g.set_setting("theme.presets_seeded", "1").map_err(dberr)?;
    }
    let names = list_names(&dir.join("themes"));
    let pref = g.get_setting("ui.theme").ok().flatten().unwrap_or_default();
    Ok((resolve_active(&pref, &names), names))
}

fn theme_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    Ok(themes_dir(app)?.join(format!("{name}.json")))
}

#[derive(Serialize)]
pub struct ThemeList {
    active: String,
    names: Vec<String>,
}

#[tauri::command]
pub fn list_themes(app: AppHandle, db: Dbs) -> Result<ThemeList, String> {
    let (active, names) = theme_state(&app, &db)?;
    Ok(ThemeList { active, names })
}

#[tauri::command]
pub fn get_theme(app: AppHandle, db: Dbs) -> Result<ThemeRead, String> {
    let (active, _) = theme_state(&app, &db)?;
    if active.is_empty() {
        return Ok(ThemeRead::default());
    }
    Ok(read_file(&theme_file(&app, &active)?))
}

#[tauri::command]
pub fn set_active_theme(app: AppHandle, db: Dbs, name: String) -> Result<(), String> {
    let (_, names) = theme_state(&app, &db)?;
    if !name.is_empty() && !names.contains(&name) {
        return Err("Tema bulunamadı".to_string());
    }
    lock(&db)?.set_setting("ui.theme", &name).map_err(dberr)
}

#[tauri::command]
pub fn create_theme(app: AppHandle, db: Dbs, name: String) -> Result<String, String> {
    let (active, names) = theme_state(&app, &db)?;
    let name = validate_name(&name, &names)?;
    let contents = if active.is_empty() {
        default_file()
    } else {
        std::fs::read_to_string(theme_file(&app, &active)?).unwrap_or_else(|_| default_file())
    };
    let dir = themes_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Tema dosyası yazılamadı: {e}"))?;
    std::fs::write(dir.join(format!("{name}.json")), contents).map_err(|e| format!("Tema dosyası yazılamadı: {e}"))?;
    lock(&db)?.set_setting("ui.theme", &name).map_err(dberr)?;
    Ok(name)
}

#[tauri::command]
pub fn delete_theme(app: AppHandle, db: Dbs, name: String) -> Result<(), String> {
    let (active, names) = theme_state(&app, &db)?;
    if !names.contains(&name) {
        return Err("Tema bulunamadı".to_string());
    }
    std::fs::remove_file(theme_file(&app, &name)?).map_err(|e| format!("Tema silinemedi: {e}"))?;
    if active == name {
        lock(&db)?.set_setting("ui.theme", "").map_err(dberr)?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_theme_file(app: AppHandle, db: Dbs) -> Result<(), String> {
    let (active, _) = theme_state(&app, &db)?;
    if active.is_empty() {
        return Err("Varsayılan tema düzenlenemez; önce yeni tema oluşturun".to_string());
    }
    let path = theme_file(&app, &active)?;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Tema dosyası açılamadı: {e}"))
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
    fn resolve_active_falls_back_when_file_missing() {
        let names = vec!["Gece".to_string()];
        assert_eq!(resolve_active("Gece", &names), "Gece");
        assert_eq!(resolve_active("Silinmiş", &names), "");
        assert_eq!(resolve_active("", &names), "");
    }

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
