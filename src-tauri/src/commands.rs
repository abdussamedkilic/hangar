use hangar_core::artwork::ensure_cover;
use hangar_core::db::Db;
use hangar_core::library::{resolve, scan_all};
use hangar_core::merge::Override;
use hangar_core::model::{Game, Platform};
use hangar_core::providers::epic::EpicProvider;
use hangar_core::providers::steam::{self, SteamProvider};
use hangar_core::providers::ubisoft::UbisoftProvider;
use hangar_core::providers::LibraryProvider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

pub struct CoversDir(pub PathBuf);
pub struct DbPath(pub PathBuf);

#[derive(Default)]
pub struct CoverJob {
    running: AtomicBool,
    rerun: AtomicBool,
}

type Dbs<'a> = State<'a, Mutex<Db>>;

#[derive(Serialize)]
pub struct ScanReport {
    games: usize,
    warnings: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    steam_api_key: Option<String>,
    steam_id: Option<String>,
}

fn lock<'a>(db: &'a Dbs) -> Result<MutexGuard<'a, Db>, String> {
    db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())
}

fn dberr(e: impl std::fmt::Display) -> String {
    format!("Veritabanı hatası: {e}")
}

fn setting(db: &Db, k: &str) -> Option<String> {
    db.get_setting(k).ok().flatten().filter(|s| !s.is_empty())
}

fn steam_id_from_disk(root: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(root.join("config").join("loginusers.vdf")).ok()?;
    steam::most_recent_steamid(&src)
}

fn scan(app: &AppHandle) -> Result<ScanReport, String> {
    let db = app.state::<Mutex<Db>>();
    let steam_p = SteamProvider::detect();
    let (key, saved_id) = {
        let g = db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())?;
        (setting(&g, "steam_api_key"), setting(&g, "steam_id").filter(|s| steam::is_steamid64(s)))
    };
    // Network call happens without holding the DB lock.
    let mut extra = Vec::new();
    let owned = key.and_then(|key| {
        let id = saved_id.or_else(|| steam_p.as_ref().and_then(|s| steam_id_from_disk(s.root())));
        match id {
            Some(id) => Some(steam::fetch_owned(&key, &id)),
            None => {
                extra.push("SteamID bulunamadı: Ayarlar'dan SteamID gir.".to_string());
                None
            }
        }
    });
    let mut providers: Vec<Box<dyn LibraryProvider>> = Vec::new();
    if let Some(p) = steam_p {
        providers.push(Box::new(p));
    }
    if let Some(p) = EpicProvider::detect() {
        providers.push(Box::new(p));
    }
    if let Some(p) = UbisoftProvider::detect() {
        providers.push(Box::new(p));
    }
    let g = db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())?;
    let mut r = scan_all(&g, &providers, owned).map_err(dberr)?;
    r.warnings.extend(extra);
    let games = g.games().map_err(dberr)?;
    drop(g);
    let _ = app.emit("library-updated", &games);
    Ok(ScanReport { games: r.games, warnings: r.warnings })
}

fn fetch_covers(app: AppHandle) {
    let job = app.state::<CoverJob>();
    if job.running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        job.rerun.store(true, Ordering::SeqCst);
        return;
    }
    std::thread::spawn(move || {
        let job = app.state::<CoverJob>();
        let dir = app.state::<CoversDir>().0.clone();
        let db_path = app.state::<DbPath>().0.clone();
        let steam_root = SteamProvider::detect().map(|s| s.root().to_path_buf());
        loop {
            job.rerun.store(false, Ordering::SeqCst);
            // Own connection: the shared Mutex<Db> is never held during network I/O.
            if let Ok(db) = Db::open(&db_path) {
                if let Ok(games) = db.games() {
                    for game in games.iter().filter(|g| g.cover_path.is_none()) {
                        if let Some(path) = ensure_cover(&db, &dir, game, steam_root.as_deref()) {
                            let _ = app.emit("cover-ready", serde_json::json!({ "id": game.id, "path": path }));
                        }
                    }
                }
            }
            if !job.rerun.load(Ordering::SeqCst) {
                job.running.store(false, Ordering::SeqCst);
                // Close the race where a request arrived just before the flag was cleared.
                if job.rerun.load(Ordering::SeqCst)
                    && job.running.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok()
                {
                    continue;
                }
                return;
            }
        }
    });
}

async fn rescan_inner(app: AppHandle) -> Result<ScanReport, String> {
    let a = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || scan(&a))
        .await
        .map_err(|e| e.to_string())??;
    fetch_covers(app);
    Ok(report)
}

#[tauri::command]
pub fn get_library(db: Dbs) -> Result<Vec<Game>, String> {
    lock(&db)?.games().map_err(dberr)
}

#[tauri::command]
pub async fn rescan(app: AppHandle) -> Result<ScanReport, String> {
    rescan_inner(app).await
}

#[tauri::command]
pub fn launch(app: AppHandle, db: Dbs, game_id: i64, platform: Option<Platform>) -> Result<String, String> {
    let g = lock(&db)?;
    let games = g.games().map_err(dberr)?;
    let game = games.iter().find(|x| x.id == game_id).ok_or("Oyun bulunamadı")?;
    let (entry, uri) = resolve(game, platform).ok_or("Başlatılacak platform yok")?;
    app.opener()
        .open_url(&uri, None::<&str>)
        .map_err(|e| format!("Açılamadı: {e}"))?;
    if entry.installed {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        g.record_launch(game_id, now).map_err(dberr)?;
        let fresh = g.games().map_err(dberr)?;
        let _ = app.emit("library-updated", &fresh);
    }
    Ok(uri)
}

#[tauri::command]
pub fn set_preferred(db: Dbs, game_id: i64, platform: Platform) -> Result<(), String> {
    lock(&db)?.set_preferred(game_id, platform).map_err(dberr)
}

#[tauri::command]
pub fn get_settings(db: Dbs) -> Result<Settings, String> {
    let g = lock(&db)?;
    Ok(Settings { steam_api_key: setting(&g, "steam_api_key"), steam_id: setting(&g, "steam_id") })
}

#[tauri::command]
pub fn save_settings(
    db: Dbs,
    steam_api_key: Option<String>,
    steam_id: Option<String>,
) -> Result<(), String> {
    let mut id = steam_id.map(|s| s.trim().to_string()).unwrap_or_default();
    if !steam::is_steamid64(&id) {
        id = SteamProvider::detect()
            .and_then(|s| steam_id_from_disk(s.root()))
            .unwrap_or_default();
    }
    let g = lock(&db)?;
    g.set_setting("steam_api_key", steam_api_key.as_deref().unwrap_or("").trim())
        .map_err(dberr)?;
    g.set_setting("steam_id", &id).map_err(dberr)
}

async fn override_and_rescan(
    app: AppHandle,
    a: (Platform, String),
    b: (Platform, String),
    merged: bool,
) -> Result<ScanReport, String> {
    app.state::<Mutex<Db>>()
        .lock()
        .map_err(|_| "Veritabanı kilitlendi".to_string())?
        .add_override(Override { a, b, merged })
        .map_err(dberr)?;
    rescan_inner(app).await
}

#[tauri::command]
pub async fn split_game(app: AppHandle, a: (Platform, String), b: (Platform, String)) -> Result<ScanReport, String> {
    override_and_rescan(app, a, b, false).await
}

#[tauri::command]
pub async fn merge_games(app: AppHandle, a: (Platform, String), b: (Platform, String)) -> Result<ScanReport, String> {
    override_and_rescan(app, a, b, true).await
}
