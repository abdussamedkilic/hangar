use crate::steam_scan;
use hangar_core::accounts::epic::EpicClient;
use hangar_core::accounts::gog::GogClient;
use hangar_core::accounts::steam::SteamAccount;
use hangar_core::accounts::ubisoft::UbiClient;
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

/// Start time of the last finished scan (Galaxy edits made during a scan count as newer).
#[derive(Default)]
pub struct LastScan(Mutex<Option<std::time::SystemTime>>);

/// True when the Galaxy DB was modified after the last scan (or no scan has completed yet).
fn changed_since(mtime: Option<std::time::SystemTime>, last_scan: Option<std::time::SystemTime>) -> bool {
    match (mtime, last_scan) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(m), Some(l)) => m > l,
    }
}

/// Latest of the two mtimes (Galaxy uses SQLite WAL: writes land in the `-wal` file first).
fn latest_mtime(a: Option<std::time::SystemTime>, b: Option<std::time::SystemTime>) -> Option<std::time::SystemTime> {
    a.max(b)
}

pub(crate) type Dbs<'a> = State<'a, Mutex<Db>>;

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

pub(crate) fn lock<'a>(db: &'a Dbs) -> Result<MutexGuard<'a, Db>, String> {
    db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())
}

pub(crate) fn dberr(e: impl std::fmt::Display) -> String {
    format!("Veritabanı hatası: {e}")
}

fn setting(db: &Db, k: &str) -> Option<String> {
    db.get_setting(k).ok().flatten().filter(|s| !s.is_empty())
}

fn steam_id_from_disk(root: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(root.join("config").join("loginusers.vdf")).ok()?;
    steam::most_recent_steamid(&src)
}

fn scan(app: &AppHandle, steam_pre: Option<(steam_scan::Outcome, u64)>) -> Result<ScanReport, String> {
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
    // Alım sürerken Steam bağlandı/kesildiyse sonuç atılır (önbellek geri gelmesin).
    let mut steam_pre = steam_pre.filter(|(_, g)| *g == steam_scan::generation()).map(|(o, _)| o);
    let mut gog_account = false;
    // Epic network calls use their own connection: the shared DB lock is not held.
    match Db::open(&app.state::<DbPath>().0) {
        Ok(epic_db) => {
            if let Some(o) = steam_pre.take() {
                let (list, warn) = steam_scan::apply_outcome(&epic_db, o, now_secs());
                if let Some(r) = list {
                    owned.push((Platform::Steam, r));
                }
                extra.extend(warn);
            }
            let connected = app.state::<GogClient>().status(&epic_db).state != hangar_core::accounts::epic::AccountState::Disconnected;
            if steam_scan::use_gog_account(&crate::accounts::gog_source(&epic_db), connected) {
                gog_account = true;
                let (gog, gog_warn) = app.state::<GogClient>().owned_for_scan(&epic_db);
                if let Some(r) = gog {
                    owned.push((Platform::Gog, r));
                }
                extra.extend(gog_warn);
            }
            let (epic, epic_warn) = app.state::<EpicClient>().owned_for_scan(&epic_db);
            if let Some(r) = epic {
                owned.push((Platform::Epic, r));
            }
            extra.extend(epic_warn);
            let (ubi, ubi_warn) = app.state::<UbiClient>().owned_for_scan(&epic_db);
            if let Some(r) = ubi {
                owned.push((Platform::Ubisoft, r));
            }
            extra.extend(ubi_warn);
        }
        // Without its own connection Epic is not refreshed; the last known list comes from the shared one.
        Err(_) => {
            let g = db.lock().map_err(|_| "Veritabanı kilitlendi".to_string())?;
            if steam_pre.is_some() {
                if let Ok(Some(v)) = g.owned_cache_get(Platform::Steam) {
                    owned.push((Platform::Steam, Ok(v)));
                }
            }
            if let Ok(Some(v)) = g.owned_cache_get(Platform::Epic) {
                owned.push((Platform::Epic, Ok(v)));
            }
            if let Ok(Some(v)) = g.owned_cache_get(Platform::Ubisoft) {
                owned.push((Platform::Ubisoft, Ok(v)));
            }
            extra.push("Epic ve Ubisoft oyunları güncellenemedi; son bilinen liste gösteriliyor.".to_string());
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
        providers.push(Box::new(p.with_owned_from_db(!gog_account)));
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

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Steam bağlıysa ve API anahtarı yoksa tarama öncesi gizli pencereyle listeyi çeker (en fazla 20 sn).
/// `None` = Steam hesabı yolu kullanılmıyor.
async fn steam_prefetch(app: &AppHandle, force: bool) -> Option<(steam_scan::Outcome, u64)> {
    let a = app.clone();
    let plan = tauri::async_runtime::spawn_blocking(move || {
        let Ok(db) = Db::open(&a.state::<DbPath>().0) else { return Some(false) };
        if setting(&db, "steam_api_key").is_some() || a.state::<SteamAccount>().load().ok().flatten().is_none() {
            return None;
        }
        let go = steam_scan::steam_scan_decision(steam_scan::last_activity(&db), now_secs(), force);
        if go {
            // Deneme zamanı önceden yazılır: başarısız oturum her taramada pencere açmasın.
            let _ = db.set_setting(steam_scan::LAST_ATTEMPT_KEY, &now_secs().to_string());
        }
        Some(go)
    })
    .await
    .ok()??;
    let gen = steam_scan::generation();
    if !plan {
        return Some((steam_scan::steam_skipped(), gen));
    }
    Some((steam_scan::steam_outcome(crate::accounts::fetch_steam_hidden(app).await), gen))
}

async fn rescan_inner(app: AppHandle, force: bool) -> Result<ScanReport, String> {
    let a = app.clone();
    let started = std::time::SystemTime::now();
    let steam_pre = steam_prefetch(&app, force).await;
    let result = tauri::async_runtime::spawn_blocking(move || scan(&a, steam_pre)).await;
    // Recorded on failure too, so a persistent failure does not retrigger a scan on every focus.
    if let Ok(mut l) = app.state::<LastScan>().0.lock() {
        *l = Some(started);
    }
    let report = result.map_err(|e| e.to_string())??;
    fetch_covers(app);
    Ok(report)
}

#[tauri::command]
pub fn get_library(db: Dbs) -> Result<Vec<Game>, String> {
    lock(&db)?.games().map_err(dberr)
}

#[tauri::command]
pub fn galaxy_changed_since_scan(last: State<LastScan>) -> bool {
    let db = hangar_core::providers::gog::GALAXY_DB;
    let m = |p: &str| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mtime = latest_mtime(m(db), m(&format!("{db}-wal")));
    let last_scan = last.0.lock().ok().and_then(|l| *l);
    changed_since(mtime, last_scan)
}

#[tauri::command]
pub async fn rescan(app: AppHandle, force: Option<bool>) -> Result<ScanReport, String> {
    rescan_inner(app, force.unwrap_or(false)).await
}

/// Fixed script: values reach PowerShell only through the env vars returned by
/// `launch_env`, never through the script text. No `-Verb RunAs`: ShellExecute
/// shows the UAC prompt itself when the exe requires elevation.
const LAUNCH_SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
$a = @($env:HANGAR_ARGS | ConvertFrom-Json | ForEach-Object { $_ }); \
if ($a.Count) { Start-Process -FilePath $env:HANGAR_EXE -WorkingDirectory $env:HANGAR_DIR -ArgumentList $a } \
else { Start-Process -FilePath $env:HANGAR_EXE -WorkingDirectory $env:HANGAR_DIR }";

/// Quotes one argument per the Windows CommandLineToArgvW rules: backslash runs before a `"`
/// (and trailing ones) are doubled, `"` becomes `\"`. Plain tokens stay unquoted.
fn quote_arg(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "-_.=:/+@,".contains(c)) {
        return s.to_string();
    }
    let mut out = String::from("\"");
    let mut slashes = 0usize;
    for c in s.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

fn launch_env(path: &std::path::Path, args: &[String]) -> Vec<(&'static str, String)> {
    let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    vec![
        ("HANGAR_EXE", path.to_string_lossy().into_owned()),
        ("HANGAR_DIR", dir.to_string_lossy().into_owned()),
        ("HANGAR_ARGS", serde_json::to_string(&args.iter().map(|a| quote_arg(a)).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into())),
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
        let quoted: Vec<String> = args.iter().map(|a| quote_arg(a)).collect();
        assert_eq!(back, quoted);
    }

    #[test]
    fn quote_arg_follows_argv_rules() {
        assert_eq!(quote_arg(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(quote_arg(r"C:\dir\"), r#""C:\dir\\""#);
        assert_eq!(quote_arg("plain"), "plain");
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg(""), "\"\"");
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

#[derive(Serialize)]
pub struct SteamAdvanced {
    api_key: Option<String>,
    steam_id: Option<String>,
}

#[tauri::command]
pub fn get_steam_advanced(db: Dbs) -> Result<SteamAdvanced, String> {
    let g = lock(&db)?;
    Ok(SteamAdvanced { api_key: setting(&g, "steam_api_key"), steam_id: setting(&g, "steam_id") })
}

#[tauri::command]
pub fn set_steam_advanced(db: Dbs, api_key: Option<String>, steam_id: Option<String>) -> Result<(), String> {
    save_settings(db, api_key, steam_id)
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
    rescan_inner(app, false).await
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
    rescan_inner(app, false).await
}

#[tauri::command]
pub async fn update_local_game(app: AppHandle, id: i64, title: String, exe_path: String) -> Result<ScanReport, String> {
    let (title, exe) = clean_local(&title, &exe_path)?;
    lock(&app.state::<Mutex<Db>>())?.update_local_game(id, &title, &exe).map_err(dberr)?;
    rescan_inner(app, false).await
}

#[tauri::command]
pub async fn remove_local_game(app: AppHandle, id: i64) -> Result<ScanReport, String> {
    lock(&app.state::<Mutex<Db>>())?.remove_local_game(id).map_err(dberr)?;
    rescan_inner(app, false).await
}

fn scan_folders_of(db: &Db) -> Result<Vec<String>, String> {
    match setting(db, "scan_folders") {
        None => Ok(vec![]),
        Some(s) => serde_json::from_str(&s).map_err(|_| "Tarama klasörleri okunamadı; Ayarlar'dan yeniden seç".to_string()),
    }
}

#[tauri::command]
pub fn get_scan_folders(db: Dbs) -> Result<Vec<String>, String> {
    scan_folders_of(&*lock(&db)?)
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
            let folders: Vec<PathBuf> = scan_folders_of(&g)?.into_iter().map(PathBuf::from).collect();
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
    rescan_inner(app, false).await
}

#[cfg(test)]
mod galaxy_tests {
    use super::{changed_since, latest_mtime};
    use std::time::{Duration, SystemTime};

    #[test]
    fn latest_mtime_cases() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let later = t + Duration::from_secs(5);
        assert_eq!(latest_mtime(Some(t), Some(later)), Some(later));
        assert_eq!(latest_mtime(Some(later), Some(t)), Some(later));
        assert_eq!(latest_mtime(Some(t), None), Some(t));
        assert_eq!(latest_mtime(None, Some(t)), Some(t));
        assert_eq!(latest_mtime(None, None), None);
    }

    #[test]
    fn changed_since_cases() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let later = t + Duration::from_secs(5);
        assert!(!changed_since(None, Some(t)));
        assert!(!changed_since(None, None));
        assert!(changed_since(Some(t), None));
        assert!(changed_since(Some(later), Some(t)));
        assert!(!changed_since(Some(t), Some(later)));
        assert!(!changed_since(Some(t), Some(t)));
    }
}

#[cfg(test)]
mod scan_folder_tests {
    use super::*;

    #[test]
    fn corrupt_scan_folders_is_an_error() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(scan_folders_of(&db), Ok(vec![]));
        db.set_setting("scan_folders", r#"["C:/Oyunlar"]"#).unwrap();
        assert_eq!(scan_folders_of(&db), Ok(vec!["C:/Oyunlar".to_string()]));
        db.set_setting("scan_folders", "{bozuk").unwrap();
        assert_eq!(scan_folders_of(&db), Err("Tarama klasörleri okunamadı; Ayarlar'dan yeniden seç".to_string()));
    }
}

#[cfg(all(test, windows))]
mod launch_script_tests {
    use super::*;

    /// Runs the real script under Windows PowerShell 5.1 against a missing exe.
    #[test]
    #[ignore]
    fn launch_script_binds_args_in_ps51() {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let ps = std::path::Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let missing = std::path::Path::new(r"C:\hangar-no-such-dir\missing.exe");
        for args in [vec![], vec!["--a=1".to_string(), "\"x y\"".to_string()]] {
            let mut cmd = std::process::Command::new(&ps);
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", LAUNCH_SCRIPT]);
            for (k, v) in launch_env(missing, &args) {
                cmd.env(k, v);
            }
            let out = cmd.output().unwrap();
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(!err.contains("Cannot convert"), "args {args:?}: {err}");
            assert!(!out.status.success(), "missing exe must fail: {err}");
        }
    }
}
