use hangar_core::accounts::epic::EpicClient;
use hangar_core::accounts::login::open_error;
use hangar_core::artwork::ensure_cover;
use hangar_core::db::Db;
use hangar_core::library::{launch_target, resolve, scan_all, LaunchTarget, OwnedLists};
use hangar_core::local_scan::{candidates as local_candidates, Candidate};
use hangar_core::merge::Override;
use hangar_core::model::{Game, Platform};
use hangar_core::providers::epic::EpicProvider;
use hangar_core::providers::gog::GogProvider;
use hangar_core::providers::local::LocalProvider;
use hangar_core::providers::riot::RiotProvider;
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
    /// Game ids already tried without success this session (no repeated network attempts).
    failed: Mutex<std::collections::HashSet<i64>>,
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
    let mut owned: OwnedLists = owned.into_iter().map(|r| (Platform::Steam, r)).collect();
    // Epic network calls use their own connection: the shared DB lock is not held.
    match Db::open(&app.state::<DbPath>().0) {
        Ok(epic_db) => {
            let (epic, epic_warn) = app.state::<EpicClient>().owned_for_scan(&epic_db);
            if let Some(r) = epic {
                owned.push((Platform::Epic, r));
            }
            extra.extend(epic_warn);
        }
        // Without its own connection Epic is not refreshed; the last known list comes from the shared one.
        Err(_) => {
            let g = db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())?;
            if let Ok(Some(v)) = g.owned_cache_get(Platform::Epic) {
                owned.push((Platform::Epic, Ok(v)));
            }
            extra.push("Epic oyunları güncellenemedi; son bilinen liste gösteriliyor.".to_string());
        }
    }
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
    if let Some(p) = RiotProvider::detect() {
        providers.push(Box::new(p));
    }
    if let Some(p) = GogProvider::detect() {
        providers.push(Box::new(p));
    }
    let g = db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())?;
    providers.push(Box::new(LocalProvider::new(g.local_games().map_err(dberr)?)));
    let mut r = scan_all(&g, &providers, owned).map_err(dberr)?;
    r.warnings.extend(extra);
    let games = g.games().map_err(dberr)?;
    drop(g);
    let _ = app.emit("library-updated", &games);
    Ok(ScanReport { games: r.games, warnings: r.warnings })
}

fn fetch_covers(app: AppHandle) {
    let job = app.state::<CoverJob>();
    // A new rescan-triggered pass retries earlier failures (e.g. tried while offline).
    if let Ok(mut f) = job.failed.lock() {
        f.clear();
    }
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
                        if job.failed.lock().map_or(false, |f| f.contains(&game.id)) {
                            continue;
                        }
                        let found = ensure_cover(&db, &dir, game, steam_root.as_deref(), Some(std::path::Path::new(hangar_core::providers::riot::DEFAULT_ROOT)));
                        match found {
                            Some(path) => {
                                let _ = app.emit("cover-ready", serde_json::json!({ "id": game.id, "path": path }));
                            }
                            None => {
                                if let Ok(mut f) = job.failed.lock() {
                                    f.insert(game.id);
                                }
                            }
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

/// Fixed script: values reach PowerShell only through the env vars returned by
/// `launch_env`, never through the script text. No `-Verb RunAs`: ShellExecute
/// shows the UAC prompt itself when the exe requires elevation.
const LAUNCH_SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
$a = @($env:HANGAR_ARGS | ConvertFrom-Json | ForEach-Object { '\"' + ($_ -replace '\"','\\\"') + '\"' }); \
if ($a.Count) { Start-Process -FilePath $env:HANGAR_EXE -WorkingDirectory $env:HANGAR_DIR -ArgumentList $a } \
else { Start-Process -FilePath $env:HANGAR_EXE -WorkingDirectory $env:HANGAR_DIR }";

fn launch_env(path: &std::path::Path, args: &[String]) -> Vec<(&'static str, String)> {
    let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    vec![
        ("HANGAR_EXE", path.to_string_lossy().into_owned()),
        ("HANGAR_DIR", dir.to_string_lossy().into_owned()),
        ("HANGAR_ARGS", serde_json::to_string(args).unwrap_or_else(|_| "[]".into())),
    ]
}

/// Starts the exe through a short-lived PowerShell so the game is not a child of
/// Hangar (killing Hangar's process tree must not kill the game).
fn launch_exe(path: &std::path::Path, args: &[String]) -> Result<(), String> {
    if !path.exists() {
        return Err("Oyun dosyası bulunamadı".into());
    }
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let ps = std::path::Path::new(&root).join("System32\\WindowsPowerShell\\v1.0\\powershell.exe");
    let mut cmd = std::process::Command::new(ps);
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", LAUNCH_SCRIPT]);
    for (k, v) in launch_env(path, args) {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let status = cmd.status().map_err(|e| format!("Açılamadı: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Açılamadı: oyun başlatılamadı".into())
    }
}

// Async + spawn_blocking: a UAC prompt keeps powershell alive until answered,
// which must not freeze the UI thread (sync commands run on it).
#[tauri::command]
pub async fn launch(app: AppHandle, db: Dbs<'_>, game_id: i64, platform: Option<Platform>, platform_id: Option<String>) -> Result<String, String> {
    let (installed, uri, target) = {
        let g = lock(&db)?;
        let games = g.games().map_err(dberr)?;
        let game = games.iter().find(|x| x.id == game_id).ok_or("Oyun bulunamadı")?;
        let (entry, uri) = resolve(game, platform, platform_id.as_deref()).ok_or("Başlatılacak platform yok")?;
        (entry.installed, uri.clone(), launch_target(entry, &uri))
    };
    match target {
        LaunchTarget::Uri(u) => app
            .opener()
            .open_url(&u, None::<&str>)
            .map_err(|e| open_error(&u, &e.to_string()))?,
        LaunchTarget::Exe { path, args } => {
            tauri::async_runtime::spawn_blocking(move || launch_exe(&path, &args))
                .await
                .map_err(|e| e.to_string())??
        }
    }
    if installed {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let g = lock(&db)?;
        g.record_launch(game_id, now).map_err(dberr)?;
        let fresh = g.games().map_err(dberr)?;
        let _ = app.emit("library-updated", &fresh);
    }
    Ok(uri)
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[test]
    fn script_is_constant_and_values_stay_out_of_it() {
        assert!(!LAUNCH_SCRIPT.contains("RunAs"));
        let env = launch_env(std::path::Path::new("C:\\Oyunlar\\Şeker \"Oyun\"\\g.exe"), &["-a b".into()]);
        for (_, v) in &env {
            assert!(!LAUNCH_SCRIPT.contains(v.as_str()));
        }
    }

    #[test]
    fn env_holds_exact_values() {
        let args = vec!["--ad=Çılgın Oyun".to_string(), "say \"hi\"".to_string()];
        let env = launch_env(std::path::Path::new("C:\\Oyunlar\\Şeker Oyun\\g.exe"), &args);
        let get = |k: &str| env.iter().find(|(n, _)| *n == k).unwrap().1.clone();
        assert_eq!(get("HANGAR_EXE"), "C:\\Oyunlar\\Şeker Oyun\\g.exe");
        assert_eq!(get("HANGAR_DIR"), "C:\\Oyunlar\\Şeker Oyun");
        let back: Vec<String> = serde_json::from_str(&get("HANGAR_ARGS")).unwrap();
        assert_eq!(back, args);
    }
}

#[tauri::command]
pub fn set_preferred(db: Dbs, game_id: i64, platform: Platform, platform_id: Option<String>) -> Result<(), String> {
    lock(&db)?.set_preferred(game_id, platform, platform_id.as_deref()).map_err(dberr)
}

#[tauri::command]
pub async fn set_cover(app: AppHandle, game_id: i64, path: String) -> Result<String, String> {
    // Own connection: the shared Mutex<Db> is not held during file I/O.
    let dir = app.state::<CoversDir>().0.clone();
    let db_path = app.state::<DbPath>().0.clone();
    let out = tauri::async_runtime::spawn_blocking(move || {
        let db = Db::open(&db_path).map_err(dberr)?;
        hangar_core::artwork::set_custom_cover(&db, &dir, game_id, std::path::Path::new(&path))
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("cover-ready", serde_json::json!({ "id": game_id, "path": out }));
    Ok(out)
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

fn clean_local(title: &str, exe_path: &str) -> Result<(String, String), String> {
    let (title, exe) = (title.trim(), exe_path.trim());
    if title.is_empty() || exe.is_empty() {
        return Err("Ad ve exe yolu gerekli".into());
    }
    Ok((title.to_string(), exe.to_string()))
}

#[tauri::command]
pub async fn add_local_game(app: AppHandle, title: String, exe_path: String) -> Result<ScanReport, String> {
    let (title, exe) = clean_local(&title, &exe_path)?;
    lock(&app.state::<Mutex<Db>>())?.add_local_game(&title, &exe, "manual").map_err(dberr)?;
    rescan_inner(app).await
}

#[tauri::command]
pub async fn update_local_game(app: AppHandle, id: i64, title: String, exe_path: String) -> Result<ScanReport, String> {
    let (title, exe) = clean_local(&title, &exe_path)?;
    lock(&app.state::<Mutex<Db>>())?.update_local_game(id, &title, &exe).map_err(dberr)?;
    rescan_inner(app).await
}

#[tauri::command]
pub async fn remove_local_game(app: AppHandle, id: i64) -> Result<ScanReport, String> {
    lock(&app.state::<Mutex<Db>>())?.remove_local_game(id).map_err(dberr)?;
    rescan_inner(app).await
}

fn scan_folders_of(db: &Db) -> Vec<String> {
    setting(db, "scan_folders")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_scan_folders(db: Dbs) -> Result<Vec<String>, String> {
    Ok(scan_folders_of(&*lock(&db)?))
}

#[tauri::command]
pub fn set_scan_folders(db: Dbs, folders: Vec<String>) -> Result<(), String> {
    let json = serde_json::to_string(&folders).map_err(|e| e.to_string())?;
    lock(&db)?.set_setting("scan_folders", &json).map_err(dberr)
}

#[tauri::command]
pub async fn scan_candidates(app: AppHandle) -> Result<Vec<Candidate>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (folders, known) = {
            let state = app.state::<Mutex<Db>>();
            let g = lock(&state)?;
            let mut known: Vec<PathBuf> = g
                .games()
                .map_err(dberr)?
                .iter()
                .flat_map(|game| game.entries.iter().filter(|e| e.platform != Platform::Local).filter_map(|e| e.install_path.clone()))
                .map(PathBuf::from)
                .collect();
            known.extend(g.local_games().map_err(dberr)?.into_iter().map(|l| PathBuf::from(l.exe_path)));
            let folders: Vec<PathBuf> = scan_folders_of(&g).into_iter().map(PathBuf::from).collect();
            (folders, known)
        };
        Ok(local_candidates(&folders, &known))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn add_local_games(app: AppHandle, items: Vec<Candidate>) -> Result<ScanReport, String> {
    let cleaned = items
        .iter()
        .map(|c| clean_local(&c.title, &c.exe_path))
        .collect::<Result<Vec<_>, _>>()?;
    lock(&app.state::<Mutex<Db>>())?.add_local_games(&cleaned, "scan").map_err(dberr)?;
    rescan_inner(app).await
}
