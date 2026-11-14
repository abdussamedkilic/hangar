use crate::commands::DbPath;
use crate::login::{close_login_window, STEAM_LABEL, login, login_flow, login_flow_with, login_ubisoft, FlowOpts, LoginError};
use crate::steam_scan;
use hangar_core::accounts::epic::{AccountStatus, EpicClient};
use hangar_core::accounts::gog::GogClient;
use hangar_core::accounts::login::{
    gog_init_script, gog_login_url, gog_window_title, steam_init_script, steam_init_script_hidden, steam_login_url,
    steam_window_title, GOG_NAV_RULES, STEAM_NAV_RULES,
};
use hangar_core::accounts::steam::{parse_delivery, SteamAccount, SteamWeb};
use hangar_core::accounts::ubisoft::UbiClient;
use hangar_core::accounts::{AuthError, SteamCred};
use hangar_core::db::Db;
use hangar_core::model::Platform;
use tauri::{AppHandle, Manager};

fn dberr(e: impl std::fmt::Display) -> String {
    format!("Veritabanı hatası: {e}")
}

/// Runs `f` on a blocking thread with its own DB connection (never the shared Mutex<Db>).
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppHandle, &Db) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = Db::open(&app.state::<DbPath>().0).map_err(dberr)?;
        f(&app, &db)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn login_err(e: LoginError) -> String {
    match e {
        LoginError::Cancelled => "cancelled".to_string(),
        LoginError::Timeout => "Giriş zaman aşımına uğradı".to_string(),
        LoginError::AlreadyOpen => "Giriş penceresi zaten açık".to_string(),
        LoginError::Window(e) => format!("Giriş penceresi açılamadı: {e}"),
    }
}

fn auth_err(e: AuthError) -> String {
    match e {
        AuthError::InvalidGrant => "Giriş kodu geçersiz; tekrar dene".to_string(),
        e => e.to_string(),
    }
}

#[tauri::command]
pub async fn get_accounts(app: AppHandle) -> Result<Vec<AccountStatus>, String> {
    blocking(app, |a, db| {
        Ok(vec![
            a.state::<SteamAccount>().status(db),
            a.state::<EpicClient>().status(db),
            a.state::<GogClient>().status(db),
            a.state::<UbiClient>().status(db),
        ])
    })
    .await
}

#[tauri::command]
pub async fn connect_account(app: AppHandle, platform: Platform) -> Result<AccountStatus, String> {
    match platform {
        Platform::Steam => connect_steam(app).await,
        Platform::Gog => {
            let code = zeroize::Zeroizing::new(
                login_flow(&app, gog_login_url(), GOG_NAV_RULES, gog_init_script, gog_window_title)
                    .await
                    .map_err(login_err)?
                    .code,
            );
            blocking(app, move |a, db| {
                let c = a.state::<GogClient>();
                c.connect(db, &code).map_err(auth_err)?;
                Ok(c.status(db))
            })
            .await
        }
        Platform::Epic => {
            let code = login(&app).await.map_err(login_err)?;
            blocking(app, move |a, db| {
                let c = a.state::<EpicClient>();
                c.connect(db, &code).map_err(auth_err)?;
                Ok(c.status(db))
            })
            .await
        }
        Platform::Ubisoft => {
            let code = zeroize::Zeroizing::new(login_ubisoft(&app).await.map_err(login_err)?);
            blocking(app, move |a, db| {
                let c = a.state::<UbiClient>();
                c.connect(db, &code).map_err(auth_err)?;
                Ok(c.status(db))
            })
            .await
        }
        _ => Err("Desteklenmiyor".into()),
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn steam_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("webview-steam"))
}

/// Görünür Steam girişi: kalıcı profil; teslim gelirse cred + liste kaydedilir.
async fn connect_steam(app: AppHandle) -> Result<AccountStatus, String> {
    let dir = steam_dir(&app)?;
    // Süren gizli alım varsa önce kapat; sonucu atılır.
    if app.get_webview_window(STEAM_LABEL).is_some() {
        steam_scan::bump_generation();
        close_login_window(&app, STEAM_LABEL);
        for _ in 0..20 {
            if app.get_webview_window(STEAM_LABEL).is_none() {
                break;
            }
            let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_millis(100))).await;
        }
    }
    let outcome = login_flow_with(
        &app,
        steam_login_url(),
        STEAM_NAV_RULES,
        steam_init_script,
        steam_window_title,
        FlowOpts { data_dir: Some(dir), hidden: false, label: Some(STEAM_LABEL) },
    )
    .await
    .map_err(login_err)?;
    match parse_delivery(&outcome.code) {
        SteamWeb::Games { steamid, display_name, games_json } => {
            let entries = hangar_core::accounts::steam::games_to_entries(&games_json).map_err(|_| "Steam oyun listesi okunamadı".to_string())?;
            blocking(app, move |a, db| {
                let acct = a.state::<SteamAccount>();
                acct.save(&SteamCred { v: 1, steamid, display_name }).map_err(auth_err)?;
                steam_scan::bump_generation();
                let _ = db.set_account_expired(Platform::Steam, false);
                let now = now_secs();
                let _ = db.owned_cache_put(Platform::Steam, &entries, now);
                let _ = db.set_setting(steam_scan::LAST_FETCH_KEY, &now.to_string());
                Ok(acct.status(db))
            })
            .await
        }
        SteamWeb::NoLogin => Err("Steam girişi tamamlanmadı".into()),
        SteamWeb::Failed(m) => Err(format!("Steam girişi başarısız: {m}")),
    }
}

/// Gizli pencereyle sessiz Steam alımı (en fazla 20 sn). `None` = zaman aşımı.
pub async fn fetch_steam_hidden(app: &AppHandle) -> Option<SteamWeb> {
    let Ok(dir) = steam_dir(app) else { return None };
    let (tx, rx) = std::sync::mpsc::channel::<SteamWeb>();
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let r = login_flow_with(
            &a,
            steam_login_url(),
            STEAM_NAV_RULES,
            steam_init_script_hidden,
            steam_window_title,
            FlowOpts { data_dir: Some(dir), hidden: true, label: Some(STEAM_LABEL) },
        )
        .await;
        let _ = tx.send(match r {
            Ok(o) => parse_delivery(&o.code),
            Err(e) => SteamWeb::Failed(e.to_string()),
        });
    });
    let got = tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(hangar_core::accounts::login::STEAM_HIDDEN_TIMEOUT_SECS))
    })
    .await
    .ok()
    .and_then(|r| r.ok());
    if got.is_none() {
        close_login_window(app, STEAM_LABEL); // pencere kapanınca akış biter
    }
    got
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_ubisoft_graphql(app: AppHandle, body: String) -> Result<String, String> {
    blocking(app, move |a, db| a.state::<UbiClient>().debug_graphql(db, &body).map_err(auth_err)).await
}

/// Debug only: owned products over demux, summarized; also written to `u0\demux.txt` in the app data dir.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_ubisoft_demux(app: AppHandle) -> Result<String, String> {
    blocking(app, |a, db| {
        let out = a.state::<UbiClient>().debug_demux_summary(db).map_err(auth_err)?;
        let dir = a.path().app_data_dir().map_err(|e| e.to_string())?.join("u0");
        std::fs::create_dir_all(&dir).map_err(|e| format!("klasör oluşturulamadı: {e}"))?;
        std::fs::write(dir.join("demux.txt"), &out).map_err(|e| format!("yazılamadı: {e}"))?;
        Ok(out)
    })
    .await
}

#[tauri::command]
pub async fn disconnect_account(app: AppHandle, platform: Platform) -> Result<bool, String> {
    match platform {
        Platform::Steam => {
            steam_scan::bump_generation();
            close_login_window(&app, STEAM_LABEL);
            let dir = steam_dir(&app)?;
            blocking(app, move |a, db| {
                // WebView2 kapanırken dosyaları kısa süre tutabilir.
                for i in 0..10 {
                    match std::fs::remove_dir_all(&dir) {
                        Ok(()) => break,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                        Err(_) if i < 9 => std::thread::sleep(std::time::Duration::from_millis(300)),
                        Err(_) => return Err("Steam profil klasörü silinemedi; Hangar'ı kapatıp tekrar dene.".to_string()),
                    }
                }
                a.state::<SteamAccount>().forget(db);
                let _ = db.set_setting(steam_scan::LAST_FETCH_KEY, "");
                let _ = db.set_setting(steam_scan::LAST_ATTEMPT_KEY, "");
                Ok(true)
            })
            .await
        }
        Platform::Gog => blocking(app, |a, db| Ok(a.state::<GogClient>().disconnect(db))).await,
        Platform::Epic => blocking(app, |a, db| Ok(a.state::<EpicClient>().disconnect(db))).await,
        Platform::Ubisoft => blocking(app, |a, db| Ok(a.state::<UbiClient>().disconnect(db))).await,
        _ => Err("Desteklenmiyor".into()),
    }
}

#[tauri::command]
pub async fn get_gog_source(app: AppHandle) -> Result<String, String> {
    blocking(app, |_, db| Ok(gog_source(db))).await
}

pub fn gog_source(db: &Db) -> String {
    match db.get_setting("gog.source").ok().flatten().as_deref() {
        Some("account") => "account".into(),
        _ => "galaxy".into(),
    }
}

#[tauri::command]
pub async fn set_gog_source(app: AppHandle, source: String) -> Result<(), String> {
    if source != "galaxy" && source != "account" {
        return Err("Geçersiz GOG kaynağı".into());
    }
    blocking(app, move |_, db| db.set_setting("gog.source", &source).map_err(dberr)).await
}
