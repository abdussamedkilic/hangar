//! Epic giriş penceresi: ince Tauri yapıştırıcısı. Kurallar `hangar_core::accounts::login`'de.
//! Kodu, token'ları ya da sorgulu URL'leri asla loglamaz.

use hangar_core::accounts::epic_api::LOGIN_URL;
use hangar_core::accounts::login::{decide, init_script, new_nonce, window_title, NavDecision};
use std::collections::BTreeSet;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const LABEL: &str = "account-login";

#[derive(Debug)]
pub enum LoginError {
    Cancelled,
    Timeout,
    AlreadyOpen,
    Window(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::Cancelled => write!(f, "Giriş iptal edildi"),
            LoginError::Timeout => write!(f, "Giriş zaman aşımına uğradı"),
            LoginError::AlreadyOpen => write!(f, "Giriş penceresi zaten açık"),
            LoginError::Window(e) => write!(f, "Giriş penceresi açılamadı: {e}"),
        }
    }
}

/// Yetki kodu ile gezinilen (yalnızca host) izinli/engelli hostlar.
#[cfg_attr(not(debug_assertions), allow(dead_code))] // host listeleri yalnızca debug probe okur
pub struct LoginOutcome {
    pub code: String,
    pub allowed_hosts: BTreeSet<String>,
    pub blocked_hosts: BTreeSet<String>,
}

pub async fn login(app: &AppHandle) -> Result<String, LoginError> {
    login_with_hosts(app).await.map(|o| o.code)
}

pub async fn login_with_hosts(app: &AppHandle) -> Result<LoginOutcome, LoginError> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_focus();
        return Err(LoginError::AlreadyOpen);
    }
    // Her deneme için tek nonce; betik ve her decide çağrısı aynı değeri kullanır.
    let nonce = new_nonce();
    let (tx, rx) = mpsc::channel::<Result<String, LoginError>>();
    let tx_nav = tx.clone();
    let tx_destroy = tx;
    let allowed = Arc::new(Mutex::new(BTreeSet::<String>::new()));
    let blocked = Arc::new(Mutex::new(BTreeSet::<String>::new()));

    let (nonce_nav, app_nav, allowed_nav, blocked_nav) = (nonce.clone(), app.clone(), allowed.clone(), blocked.clone());
    let (nonce_nw, app_nw) = (nonce.clone(), app.clone());

    let login_url: tauri::Url = LOGIN_URL.parse().expect("LOGIN_URL");
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(login_url.clone()))
        .title(window_title(&login_url))
        .incognito(true)
        .initialization_script(init_script(&nonce))
        .inner_size(520.0, 760.0)
        .on_navigation(move |url| {
            let host = url.host_str().unwrap_or("").to_string();
            match decide(url, &nonce_nav) {
                // Geri dönüş yalnızca bu pencere açıkken kabul edilir; ilk mesaj kazanır
                // (alıcı ilk mesajdan sonra okunmaz, sonraki gönderimler yok sayılır).
                NavDecision::Callback(c) => {
                    let _ = tx_nav.send(Ok(c));
                    false
                }
                NavDecision::Allow => {
                    if let Ok(mut s) = allowed_nav.lock() {
                        s.insert(host);
                    }
                    let (app, title) = (app_nav.clone(), window_title(url));
                    tauri::async_runtime::spawn(async move {
                        if let Some(w) = app.get_webview_window(LABEL) {
                            let _ = w.set_title(&title);
                        }
                    });
                    true
                }
                NavDecision::Block => {
                    if let Ok(mut s) = blocked_nav.lock() {
                        s.insert(host);
                    }
                    false
                }
            }
        })
        .on_new_window(move |url, _features| {
            if decide(&url, &nonce_nw) == NavDecision::Allow {
                let app = app_nw.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(w) = app.get_webview_window(LABEL) {
                        let _ = w.navigate(url);
                    }
                });
            }
            NewWindowResponse::Deny
        })
        .build()
        .map_err(|e| LoginError::Window(e.to_string()))?;

    window.on_window_event(move |e| {
        if let WindowEvent::Destroyed = e {
            let _ = tx_destroy.send(Err(LoginError::Cancelled));
        }
    });

    let res = tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(300)))
        .await
        .map_err(|e| LoginError::Window(e.to_string()))?;
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
    let code = match res {
        Ok(r) => r?,
        Err(_) => return Err(LoginError::Timeout),
    };
    let take = |m: &Arc<Mutex<BTreeSet<String>>>| m.lock().map(|g| g.clone()).unwrap_or_default();
    Ok(LoginOutcome { code, allowed_hosts: take(&allowed), blocked_hosts: take(&blocked) })
}

#[cfg(debug_assertions)]
struct KillOnDrop<'a> {
    api: &'a dyn hangar_core::accounts::epic_api::EpicApi,
    access: String,
}

#[cfg(debug_assertions)]
impl Drop for KillOnDrop<'_> {
    fn drop(&mut self) {
        let _ = self.api.kill(&self.access);
    }
}

/// E0 keşfi: gerçek girişle yanıt şekillerini temizlenmiş olarak <app_data>/e0 altına yazar.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn debug_epic_probe(app: AppHandle) -> Result<String, String> {
    use hangar_core::accounts::epic_api::{EpicApi, HttpEpicApi};
    use hangar_core::accounts::parse_token;
    use hangar_core::accounts::sanitize::sanitize;

    let outcome = login_with_hosts(&app).await.map_err(|e| e.to_string())?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("e0");
    let dir2 = dir.clone();
    let hosts = (outcome.allowed_hosts, outcome.blocked_hosts);
    let code = outcome.code;

    let n = tauri::async_runtime::spawn_blocking(move || -> Result<usize, String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("klasör oluşturulamadı: {e}"))?;
        let api = HttpEpicApi;
        let raw = api
            .token(&[("grant_type", "authorization_code"), ("code", &code), ("token_type", "eg1")])
            .map_err(|e| e.to_string())?;
        let tokens = parse_token(&raw).map_err(|e| e.to_string())?;
        let access = tokens.access_token.clone();
        let _guard = KillOnDrop { api: &api, access: access.clone() };
        let (refresh, acc_id, name) =
            (tokens.cred.refresh_token.clone(), tokens.cred.account_id.clone(), tokens.cred.display_name.clone());
        let secrets: [&str; 5] = [&code, &access, &refresh, &acc_id, &name];
        let mut written = 0usize;
        let mut write = |file: String, body: &str| -> Result<(), String> {
            let c = sanitize(body, &secrets).map_err(|_| "temizleme başarısız, dosya yazılmadı".to_string())?;
            std::fs::write(dir.join(file), c).map_err(|e| format!("yazılamadı: {e}"))?;
            written += 1;
            Ok(())
        };
        write("live-token.json".into(), &raw)?;

        let mut cursor: Option<String> = None;
        let mut pages = 0usize;
        let mut ns_ids: Vec<(String, Vec<String>)> = Vec::new();
        loop {
            let body = api.library_page(&access, cursor.as_deref()).map_err(|e| e.to_string())?;
            write(format!("live-library-{pages}.json"), &body)?;
            let v: serde_json::Value =
                serde_json::from_str(&body).map_err(|_| "kütüphane yanıtı JSON değil".to_string())?;
            for r in v["records"].as_array().into_iter().flatten() {
                if let (Some(ns), Some(id)) = (r["namespace"].as_str(), r["catalogItemId"].as_str()) {
                    match ns_ids.iter_mut().find(|(n, _)| n == ns) {
                        Some((_, ids)) => ids.push(id.to_string()),
                        None => ns_ids.push((ns.to_string(), vec![id.to_string()])),
                    }
                }
            }
            pages += 1;
            cursor = v["responseMetadata"]["nextCursor"].as_str().map(str::to_string);
            if cursor.is_none() || pages >= 50 {
                break;
            }
        }
        for (i, (ns, ids)) in ns_ids.iter().take(3).enumerate() {
            if i > 0 {
                std::thread::sleep(Duration::from_millis(100));
            }
            let ids: Vec<String> = ids.iter().take(20).cloned().collect();
            let body = api.catalog(&access, ns, &ids).map_err(|e| e.to_string())?;
            write(format!("live-catalog-{i}.json"), &body)?;
        }
        Ok(written)
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(format!(
        "{n} dosya yazıldı: {}\nizinli hostlar: {}\nengellenen hostlar: {}",
        dir2.display(),
        hosts.0.iter().cloned().collect::<Vec<_>>().join(", "),
        hosts.1.iter().cloned().collect::<Vec<_>>().join(", "),
    ))
}
