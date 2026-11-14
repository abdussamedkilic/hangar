use crate::commands::DbPath;
use crate::login::{login, login_ubisoft, LoginError};
use hangar_core::accounts::epic::{AccountStatus, EpicClient};
use hangar_core::accounts::ubisoft::UbiClient;
use hangar_core::accounts::AuthError;
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
    blocking(app, |a, db| Ok(vec![a.state::<EpicClient>().status(db), a.state::<UbiClient>().status(db)])).await
}

#[tauri::command]
pub async fn connect_account(app: AppHandle, platform: Platform) -> Result<AccountStatus, String> {
    match platform {
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
        Platform::Epic => blocking(app, |a, db| Ok(a.state::<EpicClient>().disconnect(db))).await,
        Platform::Ubisoft => blocking(app, |a, db| Ok(a.state::<UbiClient>().disconnect(db))).await,
        _ => Err("Desteklenmiyor".into()),
    }
}
