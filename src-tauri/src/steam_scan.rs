//! Scan decisions for the Steam account: pure helpers without Tauri (tested). The window work is in `accounts.rs`.
//! The SteamID, cookies and access token are never logged.

use hangar_core::accounts::steam::{games_to_entries, SteamWeb};
use hangar_core::db::Db;
use hangar_core::model::{Platform, PlatformEntry};
use hangar_core::providers::ProviderError;

pub const WARN_EXPIRED: &str = "Steam bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan.";
pub const WARN_OFFLINE: &str = "Steam oyunları güncellenemedi; son bilinen liste gösteriliyor.";
pub const LAST_FETCH_KEY: &str = "steam.last_fetch";
/// Last attempt (successful or not); a failed session must not open a window on every scan.
pub const LAST_ATTEMPT_KEY: &str = "steam.last_attempt";
/// On focus scans the Steam window opens at most this often.
pub const THROTTLE_SECS: i64 = 600;

static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn generation() -> u64 {
    GENERATION.load(std::sync::atomic::Ordering::SeqCst)
}

/// Called when Steam connects/disconnects; the result of an in-flight fetch is discarded.
pub fn bump_generation() {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// The GOG account source is used only when the source is "account" and an account is connected.
pub fn use_gog_account(source: &str, connected: bool) -> bool {
    source == "account" && connected
}

/// `true` = open a hidden window and fetch the list. `force` skips the limit.
pub fn steam_scan_decision(last_ok: Option<i64>, now: i64, force: bool) -> bool {
    force || last_ok.map_or(true, |t| now - t >= THROTTLE_SECS || t > now)
}

/// The effect of a Steam fetch on the scan. `entries == None` -> the cache is used.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub entries: Option<Vec<PlatformEntry>>,
    pub warnings: Vec<String>,
    /// `Some(b)` -> the account's "expired" flag becomes `b`; `None` -> left untouched.
    pub expired: Option<bool>,
}

/// `None` = timeout.
pub fn steam_outcome(r: Option<SteamWeb>) -> Outcome {
    let offline = || Outcome { entries: None, warnings: vec![WARN_OFFLINE.into()], expired: None };
    match r {
        Some(SteamWeb::Games { games_json, .. }) => match games_to_entries(&games_json) {
            Ok(entries) => Outcome { entries: Some(entries), warnings: vec![], expired: Some(false) },
            Err(_) => offline(),
        },
        Some(SteamWeb::NoLogin) => Outcome { entries: None, warnings: vec![WARN_EXPIRED.into()], expired: Some(true) },
        Some(SteamWeb::Failed(_)) | None => offline(),
    }
}

/// Fetch skipped (limit): cache, no warning.
pub fn steam_skipped() -> Outcome {
    Outcome { entries: None, warnings: vec![], expired: None }
}

pub fn last_fetch(db: &Db) -> Option<i64> {
    db.get_setting(LAST_FETCH_KEY).ok().flatten().and_then(|s| s.parse().ok())
}

pub fn last_attempt(db: &Db) -> Option<i64> {
    db.get_setting(LAST_ATTEMPT_KEY).ok().flatten().and_then(|s| s.parse().ok())
}

/// For the limit: the larger of the last successful fetch and the last attempt.
pub fn last_activity(db: &Db) -> Option<i64> {
    last_fetch(db).max(last_attempt(db))
}

/// Processes the result: flag, cache and last successful fetch time. Returns the list to scan + warnings.
pub fn apply_outcome(db: &Db, o: Outcome, now: i64) -> (Option<Result<Vec<PlatformEntry>, ProviderError>>, Vec<String>) {
    if let Some(e) = o.expired {
        let _ = db.set_account_expired(Platform::Steam, e);
    }
    let list = match o.entries {
        Some(v) => {
            let _ = db.owned_cache_put(Platform::Steam, &v, now);
            let _ = db.set_setting(LAST_FETCH_KEY, &now.to_string());
            Some(Ok(v))
        }
        None => db.owned_cache_get(Platform::Steam).ok().flatten().map(Ok),
    };
    (list, o.warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "76561198000000001";
    fn games() -> SteamWeb {
        SteamWeb::Games {
            steamid: SID.into(),
            display_name: "x".into(),
            games_json: r#"{"response":{"games":[{"appid":620,"name":"Portal 2"}]}}"#.into(),
        }
    }

    #[test]
    fn steam_scan_nologin_marks_expired() {
        let o = steam_outcome(Some(SteamWeb::NoLogin));
        assert_eq!(o.expired, Some(true));
        assert_eq!(o.warnings, vec![WARN_EXPIRED.to_string()]);
        assert!(o.entries.is_none());
        let db = Db::open_in_memory().unwrap();
        db.owned_cache_put(Platform::Steam, &games_to_entries(r#"{"response":{"games":[{"appid":1,"name":"A"}]}}"#).unwrap(), 1).unwrap();
        let (list, w) = apply_outcome(&db, o, 100);
        assert_eq!(list.unwrap().unwrap().len(), 1); // cache
        assert_eq!(w, vec![WARN_EXPIRED.to_string()]);
        assert!(db.account_expired(Platform::Steam).unwrap());
        assert_eq!(last_fetch(&db), None);
    }

    #[test]
    fn steam_scan_timeout_is_offline() {
        for r in [None, Some(SteamWeb::Failed("http 500".into()))] {
            let o = steam_outcome(r);
            assert_eq!(o.expired, None); // not counted as expired
            assert_eq!(o.warnings, vec![WARN_OFFLINE.to_string()]);
            assert!(o.entries.is_none());
        }
        let db = Db::open_in_memory().unwrap();
        db.set_account_expired(Platform::Steam, true).unwrap();
        let _ = apply_outcome(&db, steam_outcome(None), 5);
        assert!(db.account_expired(Platform::Steam).unwrap()); // left untouched
    }

    #[test]
    fn steam_scan_success_caches_and_clears_expired() {
        let db = Db::open_in_memory().unwrap();
        db.set_account_expired(Platform::Steam, true).unwrap();
        let (list, w) = apply_outcome(&db, steam_outcome(Some(games())), 1234);
        assert_eq!(list.unwrap().unwrap().len(), 1);
        assert!(w.is_empty());
        assert!(!db.account_expired(Platform::Steam).unwrap());
        assert_eq!(db.owned_cache_get(Platform::Steam).unwrap().unwrap().len(), 1);
        assert_eq!(last_fetch(&db), Some(1234));
    }

    #[test]
    fn gog_account_needs_source_and_connection() {
        assert!(use_gog_account("account", true));
        assert!(!use_gog_account("account", false));
        assert!(!use_gog_account("galaxy", true));
        assert!(!use_gog_account("galaxy", false));
    }

    #[test]
    fn generation_bumps() {
        let g = generation();
        bump_generation();
        assert!(generation() > g);
    }

    #[test]
    fn failed_attempt_throttles_auto_scan() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting(LAST_ATTEMPT_KEY, "1000").unwrap();
        assert_eq!(last_fetch(&db), None);
        assert!(!steam_scan_decision(last_activity(&db), 1060, false));
        assert!(steam_scan_decision(last_activity(&db), 1060, true));
        assert!(steam_scan_decision(last_activity(&db), 1600, false));
    }

    #[test]
    fn steam_auto_throttle_10min() {
        assert!(steam_scan_decision(None, 1000, false));
        assert!(!steam_scan_decision(Some(1000), 1000 + 599, false));
        assert!(steam_scan_decision(Some(1000), 1000 + 600, false));
        assert!(steam_scan_decision(Some(1000), 1000 + 1, true)); // Refresh skips the limit
        assert!(steam_scan_decision(Some(5000), 1000, false)); // saat geri gittiyse
        assert_eq!(steam_skipped().warnings.len(), 0);
    }
}
