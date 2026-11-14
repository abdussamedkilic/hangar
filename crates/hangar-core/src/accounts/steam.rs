//! Steam hesabı: giriş penceresinin teslim ettiği oyun listesi ve SteamID kimliği.
//! SteamID ve web erişim anahtarı asla Display/log edilmez.

use super::epic::{AccountState, AccountStatus};
use super::store::{load_steam_cred, save_steam_cred, SecretStore};
use super::{AuthError, SteamCred};
use crate::db::Db;
use crate::model::{Platform, PlatformEntry};
use crate::providers::ProviderError;

const DISPLAY_NAME: &str = "Steam hesabı";

#[derive(Clone, PartialEq)]
pub enum SteamWeb {
    Games { steamid: String, display_name: String, games_json: String },
    NoLogin,
    Failed(String),
}

impl std::fmt::Debug for SteamWeb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SteamWeb::Games { display_name, games_json, .. } => f
                .debug_struct("Games")
                .field("steamid", &"***")
                .field("display_name", display_name)
                .field("games_json_len", &games_json.len())
                .finish(),
            SteamWeb::NoLogin => write!(f, "NoLogin"),
            SteamWeb::Failed(m) => f.debug_tuple("Failed").field(m).finish(),
        }
    }
}

/// Decodes what the login window script delivered (`{games,steamid,name}` | `{nologin}` | `{failed}`).
pub fn parse_delivery(code: &str) -> SteamWeb {
    let fail = |m: &str| SteamWeb::Failed(m.to_string());
    let Ok(v) = serde_json::from_str::<serde_json::Value>(code) else {
        return fail("teslim okunamadı");
    };
    if v["nologin"].as_bool() == Some(true) {
        return SteamWeb::NoLogin;
    }
    if let Some(f) = v["failed"].as_str() {
        return SteamWeb::Failed(f.chars().take(60).collect());
    }
    let sid = v["steamid"].as_str().unwrap_or("");
    if !crate::providers::steam::is_steamid64(sid) || !v["games"].is_object() {
        return fail("teslim eksik");
    }
    let name = v["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(DISPLAY_NAME);
    SteamWeb::Games {
        steamid: sid.to_string(),
        display_name: name.chars().take(64).collect(),
        games_json: serde_json::json!({ "response": v["games"] }).to_string(),
    }
}

/// `{"response":{"games":[...]}}` -> entries (same mapping as the API-key path).
pub fn games_to_entries(games_json: &str) -> Result<Vec<PlatformEntry>, ProviderError> {
    crate::providers::steam::parse_owned_games(games_json)
}

pub struct SteamAccount {
    store: Box<dyn SecretStore>,
}

impl SteamAccount {
    pub fn new(store: Box<dyn SecretStore>) -> Self {
        Self { store }
    }

    pub fn save(&self, c: &SteamCred) -> Result<(), AuthError> {
        save_steam_cred(&*self.store, c).map_err(|_| AuthError::Save)
    }

    pub fn load(&self) -> Result<Option<SteamCred>, AuthError> {
        load_steam_cred(&*self.store)
    }

    /// Forgets the stored identity, the owned-games cache and the expired flag (the webview profile is the caller's to wipe).
    pub fn forget(&self, db: &Db) {
        let _ = self.store.delete(Platform::Steam);
        let _ = db.owned_cache_delete(Platform::Steam);
        let _ = db.set_account_expired(Platform::Steam, false);
    }

    /// Offline: reads only the stored credential and the expired flag.
    pub fn status(&self, db: &Db) -> AccountStatus {
        let (state, display_name) = match self.load().ok().flatten() {
            Some(c) => {
                let st = if db.account_expired(Platform::Steam).unwrap_or(false) {
                    AccountState::Expired
                } else {
                    AccountState::Connected
                };
                (st, Some(c.display_name.clone()))
            }
            None => (AccountState::Disconnected, None),
        };
        AccountStatus { platform: Platform::Steam, state, display_name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::login::{decide_with, steam_init_script, steam_init_script_hidden, NavDecision, STEAM_NAV_RULES};
    use crate::accounts::store::MemoryStore;
    use std::sync::Arc;
    const N: &str = "0123456789abcdef0123456789abcdef";
    const SID: &str = "76561198000000001";

    #[test]
    fn parse_delivery_games() {
        let code = format!(r#"{{"games":{{"game_count":1,"games":[{{"appid":620,"name":"Portal 2"}}]}},"steamid":"{SID}","name":"player"}}"#);
        match parse_delivery(&code) {
            SteamWeb::Games { steamid, display_name, games_json } => {
                assert_eq!(steamid, SID);
                assert_eq!(display_name, "player");
                assert!(games_json.contains(r#""response""#) && games_json.contains("620"));
            }
            _ => panic!("beklenmeyen teslim"),
        }
        let d = format!("{:?}", parse_delivery(&code));
        assert!(!d.contains("7656") && !d.contains(SID) && !d.contains("Portal"), "{d}");
        let c2 = format!(r#"{{"games":{{"games":[]}},"steamid":"{SID}"}}"#);
        assert!(matches!(parse_delivery(&c2), SteamWeb::Games { display_name, .. } if display_name == "Steam hesabı"));
        assert!(matches!(parse_delivery(r#"{"games":{"games":[]},"steamid":"12"}"#), SteamWeb::Failed(_)));
    }
    #[test]
    fn slim_delivery_and_script_mapping() {
        let code = format!(r#"{{"games":{{"games":[{{"appid":620,"name":"Portal 2","playtime_forever":90,"rtime_last_played":5}}]}},"steamid":"{SID}","name":"x"}}"#);
        let SteamWeb::Games { games_json, .. } = parse_delivery(&code) else { panic!("teslim") };
        let e = games_to_entries(&games_json).unwrap();
        assert_eq!((e[0].playtime_min, e[0].last_played), (Some(90), Some(5)));
        let js = steam_init_script(N);
        assert!(js.contains("deliver({games: {games: gm}") && js.contains("rtime_last_played: g.rtime_last_played"));
    }
    #[test]
    fn parse_delivery_nologin() {
        assert_eq!(parse_delivery(r#"{"nologin":true}"#), SteamWeb::NoLogin);
    }
    #[test]
    fn parse_delivery_failed() {
        assert_eq!(parse_delivery(r#"{"failed":"http 401"}"#), SteamWeb::Failed("http 401".into()));
        assert!(matches!(parse_delivery("not json"), SteamWeb::Failed(_)));
        assert!(matches!(parse_delivery("{}"), SteamWeb::Failed(_)));
    }
    #[test]
    fn games_to_entries_maps_playtime() {
        let j = r#"{"response":{"games":[{"appid":620,"name":"Portal 2","playtime_forever":600,"rtime_last_played":1700000000}]}}"#;
        let e = games_to_entries(j).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].platform_id, "620");
        assert_eq!(e[0].playtime_min, Some(600));
        assert_eq!(e[0].last_played, Some(1700000000));
    }
    #[test]
    fn cred_roundtrip_dpapi_memory() {
        let mem = Arc::new(MemoryStore::default());
        let a = SteamAccount::new(Box::new(mem.clone()));
        assert_eq!(a.load().unwrap(), None);
        let c = SteamCred { v: 1, steamid: SID.into(), display_name: "player".into() };
        a.save(&c).unwrap();
        assert_eq!(a.load().unwrap(), Some(c.clone()));
        assert!(!format!("{c:?}").contains(SID));
        let db = Db::open_in_memory().unwrap();
        assert_eq!(a.status(&db).state, AccountState::Connected);
        db.set_account_expired(Platform::Steam, true).unwrap();
        assert_eq!(a.status(&db).state, AccountState::Expired);
        a.forget(&db);
        assert_eq!(a.load().unwrap(), None);
        assert_eq!(a.status(&db).state, AccountState::Disconnected);
        assert!(!db.account_expired(Platform::Steam).unwrap());
        mem.fail_save.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(a.save(&c), Err(AuthError::Save));
    }
    #[cfg(windows)]
    #[test]
    fn cred_roundtrip_real_dpapi() {
        let dir = std::env::temp_dir().join(format!("hangar-steam-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = SteamAccount::new(Box::new(crate::accounts::store::DpapiStore::new(dir.clone())));
        let c = SteamCred { v: 1, steamid: SID.into(), display_name: "x".into() };
        a.save(&c).unwrap();
        assert_eq!(a.load().unwrap(), Some(c));
        let raw = std::fs::read(dir.join("steam.dat")).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains(SID));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn script_never_contains_token_in_delivery() {
        let s = steam_init_script(N);
        assert!(s.contains(N) && s.contains("GetOwnedGames") && s.contains("g_steamID") && s.contains("g_AccountID") && s.contains("application_config"));
        for line in s.lines().filter(|l| l.contains("deliver(")) {
            for bad in ["access_token", "webapi_token", "tok"] {
                assert!(!line.contains(bad), "{bad} in: {line}");
            }
        }
        assert!(!s.contains("cookie"));
        assert!(steam_init_script_hidden(N).contains("var HIDDEN = true;"));
        assert!(steam_init_script(N).contains("var HIDDEN = false;"));
    }
    #[test]
    #[should_panic]
    fn script_rejects_bad_nonce() {
        steam_init_script("a';alert(1);'");
    }
    #[test]
    fn nav_rules() {
        let d = |s: &str| decide_with(&url::Url::parse(s).unwrap(), N, STEAM_NAV_RULES);
        assert!(matches!(d("https://store.steampowered.com/login/"), NavDecision::Allow));
        assert!(matches!(d("https://steamcommunity.com/"), NavDecision::Allow));
        assert!(matches!(d("https://store.steampowered.com:8443/"), NavDecision::Block));
        assert!(matches!(d("https://evil.steampowered.com/"), NavDecision::Block));
        assert!(matches!(d("https://example.com/"), NavDecision::Block));
        let cb = format!("https://hangar.invalid/callback?n={N}&code=%7B%7D");
        assert!(matches!(d(&cb), NavDecision::Callback(c) if c == "{}"));
    }
}
