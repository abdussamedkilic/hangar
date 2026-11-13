use crate::commands::DbPath;
use crate::login::{login, LoginError};
use hangar_core::accounts::epic::{AccountStatus, EpicClient};
use hangar_core::accounts::AuthError;
use hangar_core::db::Db;
use hangar_core::model::Platform;
use tauri::{AppHandle, Manager};

fn dberr(e: impl std::fmt::Display) -> String {
    format!("Veritabanı hatası: {e}")
}

/// Runs `f` on a blocking thread with its own DB connection (never the shared Mutex<Db>).
async fn with_client<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&EpicClient, &Db) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = Db::open(&app.state::<DbPath>().0).map_err(dberr)?;
        let client = app.state::<EpicClient>();
        f(&client, &db)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_accounts(app: AppHandle) -> Result<Vec<AccountStatus>, String> {
    with_client(app, |c, db| Ok(vec![c.status(db)])).await
}

#[tauri::command]
pub async fn connect_account(app: AppHandle, platform: Platform) -> Result<AccountStatus, String> {
    if platform != Platform::Epic {
        return Err("Desteklenmiyor".into());
    }
    let code = login(&app).await.map_err(|e| match e {
        LoginError::Cancelled => "cancelled".to_string(),
        LoginError::Timeout => "Giriş zaman aşımına uğradı".to_string(),
        LoginError::AlreadyOpen => "Giriş penceresi zaten açık".to_string(),
        LoginError::Window(e) => format!("Giriş penceresi açılamadı: {e}"),
    })?;
    with_client(app, move |c, db| {
        c.connect(db, &code).map_err(|e| match e {
            AuthError::InvalidGrant => "Giriş kodu geçersiz; tekrar dene".to_string(),
            e => e.to_string(),
        })?;
        Ok(c.status(db))
    })
    .await
}

#[tauri::command]
pub async fn disconnect_account(app: AppHandle, platform: Platform) -> Result<bool, String> {
    if platform != Platform::Epic {
        return Err("Desteklenmiyor".into());
    }
    with_client(app, |c, db| Ok(c.disconnect(db))).await
}
