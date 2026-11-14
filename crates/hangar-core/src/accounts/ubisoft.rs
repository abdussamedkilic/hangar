use super::epic::{AccountState, AccountStatus};
use super::store::{load_ubi_cred, save_ubi_cred, SecretStore};
use super::ubi_demux::DemuxGame;
use super::ubisoft_api::{parse_login, UbiApi};
use super::{AuthError, UbiCred};
use crate::db::Db;
use crate::model::{Platform, PlatformEntry};
use crate::providers::ProviderError;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use zeroize::Zeroizing;

const WARN_OFFLINE: &str = "Ubisoft oyunları güncellenemedi; son bilinen liste gösteriliyor.";
const WARN_EXPIRED: &str = "Ubisoft bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan.";
const WARN_CORRUPT: &str = "Ubisoft bağlantısı okunamadı; yeniden bağlan.";

/// In-memory session from a refresh; the persisted part is `cred`.
struct Session {
    ticket: String,
    session_id: String,
    cred: UbiCred,
}

impl Drop for Session {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.ticket);
        zeroize::Zeroize::zeroize(&mut self.session_id);
    }
}

pub struct UbiClient {
    api: Box<dyn UbiApi>,
    store: Box<dyn SecretStore>,
    /// Serializes remember-me rotation; holds a rotated credential whose save failed.
    lock: Mutex<Option<UbiCred>>,
    /// Bumped by connect/disconnect; a scan whose generation changed meanwhile is discarded.
    generation: AtomicU64,
}

fn parse_session(body: &str, old: &UbiCred) -> Result<Session, AuthError> {
    let bad = || AuthError::Parse("session".into());
    let v: Value = serde_json::from_str(body).map_err(|_| bad())?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    let rotated = s("rememberMeTicket").unwrap_or_else(|| old.remember_me_ticket.clone());
    Ok(Session {
        ticket: s("ticket").ok_or_else(bad)?,
        session_id: s("sessionId").ok_or_else(bad)?,
        cred: UbiCred {
            v: 2,
            remember_me_ticket: rotated,
            user_id: s("userId").unwrap_or_else(|| old.user_id.clone()),
            display_name: old.display_name.clone(),
        },
    })
}

impl UbiClient {
    pub fn new(api: Box<dyn UbiApi>, store: Box<dyn SecretStore>) -> Self {
        UbiClient { api, store, lock: Mutex::new(None), generation: AtomicU64::new(0) }
    }

    fn guard(&self) -> MutexGuard<'_, Option<UbiCred>> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// `code` is the JSON the login window delivers (login + remember-me data).
    pub fn connect(&self, db: &Db, code: &str) -> Result<String, AuthError> {
        let mut pending = self.guard();
        let login = parse_login(code).map_err(|_| AuthError::Parse("giriş".into()))?;
        if login.remember_ticket.is_empty() {
            return Err(AuthError::Parse("giriş".into()));
        }
        let first = UbiCred {
            v: 2,
            remember_me_ticket: login.remember_ticket.clone(),
            user_id: login.user_id.clone(),
            display_name: login.display_name.clone(),
        };
        let body = Zeroizing::new(self.api.refresh(&first.remember_me_ticket)?);
        let sess = parse_session(&body, &first)?;
        save_ubi_cred(&*self.store, &sess.cred).map_err(|_| AuthError::Save)?;
        *pending = None;
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = db.set_account_expired(Platform::Ubisoft, false);
        Ok(sess.cred.display_name.clone())
    }

    /// Caller holds `lock`. With `persist`, the rotated credential is saved before the session is
    /// returned; a failed save keeps it in `pending` so the connection is not lost.
    fn refresh_locked(&self, pending: &mut Option<UbiCred>, persist: bool) -> Result<Session, AuthError> {
        let cred = match pending.as_ref() {
            Some(c) => c.clone(),
            None => match load_ubi_cred(&*self.store) {
                Ok(Some(c)) => c,
                Ok(None) => return Err(AuthError::NotConnected),
                Err(AuthError::Corrupt) => {
                    let _ = self.store.delete(Platform::Ubisoft);
                    return Err(AuthError::Corrupt);
                }
                Err(e) => return Err(e),
            },
        };
        let body = Zeroizing::new(self.api.refresh(&cred.remember_me_ticket)?);
        let sess = parse_session(&body, &cred)?;
        if persist {
            *pending = match save_ubi_cred(&*self.store, &sess.cred) {
                Ok(()) => None,
                Err(_) => Some(sess.cred.clone()),
            };
        }
        Ok(sess)
    }

    pub fn fetch_owned(&self) -> Result<Vec<PlatformEntry>, AuthError> {
        let sess = {
            let mut pending = self.guard();
            self.refresh_locked(&mut pending, true)?
        };
        let products = match self.api.owned_products(&sess.ticket) {
            Ok(p) => p,
            Err(AuthError::InvalidGrant) => return Err(AuthError::InvalidGrant),
            // Demux is unofficial: on any other failure fall back to the GraphQL list alone.
            Err(_) => {
                let body = Zeroizing::new(self.api.owned_games(&sess.ticket, &sess.session_id)?);
                return parse_owned(&body);
            }
        };
        let index = self
            .api
            .owned_games(&sess.ticket, &sess.session_id)
            .ok()
            .map(Zeroizing::new)
            .map(|b| graphql_index(&b))
            .unwrap_or_default();
        Ok(merge_products(products, &index))
    }

    /// Debug only: owned products over demux, summarized (same locked, persisting refresh).
    #[cfg(debug_assertions)]
    pub fn debug_demux_summary(&self, _db: &Db) -> Result<String, AuthError> {
        let sess = {
            let mut pending = self.guard();
            self.refresh_locked(&mut pending, true)?
        };
        let products = self.api.owned_products(&sess.ticket)?;
        Ok(super::ubi_demux::summarize(&products))
    }

    /// Debug only: raw GraphQL exploration with the stored connection (same locked, persisting refresh).
    #[cfg(debug_assertions)]
    pub fn debug_graphql(&self, _db: &Db, body: &str) -> Result<String, AuthError> {
        let sess = {
            let mut pending = self.guard();
            self.refresh_locked(&mut pending, true)?
        };
        self.api.graphql(&sess.ticket, &sess.session_id, body)
    }

    pub fn owned_for_scan(&self, db: &Db) -> (Option<Result<Vec<PlatformEntry>, ProviderError>>, Vec<String>) {
        let gen = self.generation.load(Ordering::SeqCst);
        let r = self.fetch_owned();
        let _g = self.guard();
        if self.generation.load(Ordering::SeqCst) != gen {
            return (None, vec![]);
        }
        let cache = || db.owned_cache_get(Platform::Ubisoft).ok().flatten().map(Ok);
        match r {
            Ok(v) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64);
                let _ = db.owned_cache_put(Platform::Ubisoft, &v, now);
                let _ = db.set_account_expired(Platform::Ubisoft, false);
                (Some(Ok(v)), vec![])
            }
            Err(AuthError::NotConnected) => (None, vec![]),
            Err(AuthError::Network(_) | AuthError::Parse(_) | AuthError::Save) => (cache(), vec![WARN_OFFLINE.into()]),
            Err(AuthError::InvalidGrant) => {
                let _ = db.set_account_expired(Platform::Ubisoft, true);
                (cache(), vec![WARN_EXPIRED.into()])
            }
            Err(AuthError::Corrupt) => (None, vec![WARN_CORRUPT.into()]),
        }
    }

    /// Logs the session out if reachable (without persisting the rotation), then always forgets everything local.
    pub fn disconnect(&self, db: &Db) -> bool {
        let mut pending = self.guard();
        self.generation.fetch_add(1, Ordering::SeqCst);
        let revoked = self
            .refresh_locked(&mut pending, false)
            .and_then(|s| self.api.logout(&s.ticket, &s.session_id))
            .is_ok();
        *pending = None;
        let _ = self.store.delete(Platform::Ubisoft);
        let _ = db.owned_cache_delete(Platform::Ubisoft);
        let _ = db.set_account_expired(Platform::Ubisoft, false);
        revoked
    }

    /// Offline: reads only the pending/stored credential and the expired flag.
    pub fn status(&self, db: &Db) -> AccountStatus {
        let pending = self.guard().clone();
        let cred = match pending {
            Some(c) => Some(c),
            None => load_ubi_cred(&*self.store).ok().flatten(),
        };
        let (state, display_name) = match cred {
            Some(c) => {
                let st = if db.account_expired(Platform::Ubisoft).unwrap_or(false) {
                    AccountState::Expired
                } else {
                    AccountState::Connected
                };
                (st, Some(c.display_name.clone()))
            }
            None => (AccountState::Disconnected, None),
        };
        AccountStatus { platform: Platform::Ubisoft, state, display_name }
    }
}

fn has_pc(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.get("type").and_then(Value::as_str) == Some("PC") || m.values().any(has_pc),
        Value::Array(a) => a.iter().any(has_pc),
        _ => false,
    }
}

const THUMB_BASE: &str = "https://ubistatic3-a.akamaihd.net/orbit/uplay_launcher_3_0/assets/";

type GqlIndex = HashMap<String, (String, Option<String>)>;

/// spaceId -> (name, https cover url) from an AllGames response; empty if it cannot be read.
fn graphql_index(json: &str) -> GqlIndex {
    let Ok(v) = serde_json::from_str::<Value>(json) else { return HashMap::new() };
    let Some(nodes) = v.pointer("/data/viewer/ownedGames/nodes").and_then(Value::as_array) else {
        return HashMap::new();
    };
    nodes
        .iter()
        .filter_map(|n| {
            let s = |k: &str| n.get(k).and_then(Value::as_str).unwrap_or("").trim();
            let (space, name, cover) = (s("spaceId"), s("name"), s("coverUrl"));
            (!space.is_empty() && !name.is_empty())
                .then(|| (space.to_string(), (name.to_string(), cover.starts_with("https://").then(|| cover.to_string()))))
        })
        .collect()
}

/// Owned base games from demux, named/covered via GraphQL where possible.
fn merge_products(products: Vec<DemuxGame>, index: &GqlIndex) -> Vec<PlatformEntry> {
    let mut seen = HashSet::new();
    products
        .into_iter()
        .filter(|p| p.owned && p.product_type == 0 && seen.insert(p.product_id))
        .filter_map(|p| {
            let gql = p.space_id.as_deref().and_then(|s| index.get(s));
            let title = gql
                .map(|(n, _)| n.clone())
                .or_else(|| p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string))?;
            let cover = gql.and_then(|(_, c)| c.clone()).or_else(|| {
                p.thumb_image
                    .as_deref()
                    .filter(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')))
                    .map(|f| format!("{THUMB_BASE}{f}"))
            });
            let uri = format!("uplay://launch/{}/0", p.product_id);
            Some(PlatformEntry {
                platform: Platform::Ubisoft,
                platform_id: p.product_id.to_string(),
                title,
                installed: false,
                install_path: None,
                size_bytes: None,
                last_played: None,
                playtime_min: None,
                launch_uri: uri.clone(),
                install_uri: Some(uri),
                launch_args: vec![],
                cover_url: cover,
                edition: None,
                copy: None,
            })
        })
        .collect()
}

/// Owned PC games from the AllGames response.
pub fn parse_owned(json: &str) -> Result<Vec<PlatformEntry>, AuthError> {
    let bad = || AuthError::Parse("games".into());
    let v: Value = serde_json::from_str(json).map_err(|_| bad())?;
    let nodes = v.pointer("/data/viewer/ownedGames/nodes").and_then(Value::as_array).ok_or_else(bad)?;
    Ok(nodes
        .iter()
        .filter(|n| has_pc(&n["viewer"]["meta"]["ownedPlatformGroups"]))
        .filter_map(|n| {
            let s = |k: &str| n.get(k).and_then(Value::as_str).unwrap_or("").trim();
            let (space, name) = (s("spaceId"), s("name"));
            if space.is_empty() || name.is_empty() {
                return None;
            }
            let uri = format!("uplay://launch/{space}/0");
            let cover = s("coverUrl");
            Some(PlatformEntry {
                platform: Platform::Ubisoft,
                platform_id: space.into(),
                title: name.into(),
                installed: false,
                install_path: None,
                size_bytes: None,
                last_played: None,
                playtime_min: None,
                launch_uri: uri.clone(),
                install_uri: Some(uri),
                launch_args: vec![],
                cover_url: cover.starts_with("https://").then(|| cover.to_string()),
                edition: None,
                copy: None,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::store::MemoryStore;
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, OnceLock, Weak};
    use std::time::Duration;

    struct FakeApi {
        refreshes: Mutex<VecDeque<Result<String, AuthError>>>,
        owned: Result<String, AuthError>,
        products: Result<Vec<DemuxGame>, AuthError>,
        logout: Result<(), AuthError>,
        calls: AtomicUsize,
        seen_rm: Mutex<Vec<String>>,
        on_owned: Option<Box<dyn Fn() + Send + Sync>>,
        delay: Duration,
    }
    impl FakeApi {
        fn new(r: Vec<Result<String, AuthError>>) -> Self {
            FakeApi {
                refreshes: Mutex::new(r.into()),
                owned: Ok(games()),
                products: Err(AuthError::Network("demux yok".into())),
                logout: Ok(()),
                calls: AtomicUsize::new(0),
                seen_rm: Mutex::new(vec![]),
                on_owned: None,
                delay: Duration::ZERO,
            }
        }
    }
    impl UbiApi for FakeApi {
        fn refresh(&self, rm: &str) -> Result<String, AuthError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen_rm.lock().unwrap().push(rm.to_string());
            std::thread::sleep(self.delay);
            self.refreshes.lock().unwrap().pop_front().unwrap_or(Err(AuthError::Network("bitti".into())))
        }
        fn owned_games(&self, _t: &str, _s: &str) -> Result<String, AuthError> {
            self.owned.clone()
        }
        fn owned_products(&self, _t: &str) -> Result<Vec<DemuxGame>, AuthError> {
            if let Some(h) = &self.on_owned {
                h();
            }
            self.products.clone()
        }
        fn logout(&self, _t: &str, _s: &str) -> Result<(), AuthError> {
            self.logout.clone()
        }
    }
    struct ArcApi(Arc<FakeApi>);
    impl UbiApi for ArcApi {
        fn refresh(&self, rm: &str) -> Result<String, AuthError> { self.0.refresh(rm) }
        fn owned_games(&self, t: &str, s: &str) -> Result<String, AuthError> { self.0.owned_games(t, s) }
        fn owned_products(&self, t: &str) -> Result<Vec<DemuxGame>, AuthError> { self.0.owned_products(t) }
        fn logout(&self, t: &str, s: &str) -> Result<(), AuthError> { self.0.logout(t, s) }
    }

    fn sess(t: &str, rm: Option<&str>) -> String {
        let mut v = serde_json::json!({"ticket": t, "sessionId": "SID-SECRET", "userId": "USER-SECRET"});
        if let Some(rm) = rm {
            v["rememberMeTicket"] = rm.into();
        }
        v.to_string()
    }
    fn games() -> String {
        serde_json::json!({"data":{"viewer":{"ownedGames":{"nodes":[
            {"id":"i1","spaceId":"sp1","name":"WATCH_DOGS™","coverUrl":"https://ubiservices.cdn.ubi.com/c1.jpg","viewer":{"meta":{"ownedPlatformGroups":[[{"type":"PC"}]]}}},
            {"id":"i2","spaceId":"sp2","name":"Konsol Oyunu","coverUrl":"https://ubiservices.cdn.ubi.com/c2.jpg","viewer":{"meta":{"ownedPlatformGroups":[[{"type":"XBOX"}]]}}}
        ]}}}})
        .to_string()
    }
    fn dg(id: u32, owned: bool, ty: u32, space: Option<&str>, name: Option<&str>, thumb: Option<&str>) -> DemuxGame {
        DemuxGame {
            product_id: id,
            owned,
            product_type: ty,
            space_id: space.map(str::to_string),
            name: name.map(str::to_string),
            thumb_image: thumb.map(str::to_string),
        }
    }
    fn code() -> String {
        r#"{"login":{"ticket":"T-SECRET","sessionId":"S","userId":"U-SECRET","nameOnPlatform":"oyuncu"},"remember":{"rememberMeTicket":"RM0-SECRET"}}"#.into()
    }
    fn stored(rm: &str) -> Arc<MemoryStore> {
        let s = Arc::new(MemoryStore::default());
        save_ubi_cred(&*s, &UbiCred { v: 2, remember_me_ticket: rm.into(), user_id: "u".into(), display_name: "oyuncu".into() }).unwrap();
        s
    }
    fn client_arc(api: &Arc<FakeApi>, store: &Arc<MemoryStore>) -> UbiClient {
        UbiClient::new(Box::new(ArcApi(api.clone())), Box::new(store.clone()))
    }
    fn client(api: FakeApi, store: &Arc<MemoryStore>) -> UbiClient {
        client_arc(&Arc::new(api), store)
    }
    fn rm_of(s: &MemoryStore) -> String {
        load_ubi_cred(s).unwrap().unwrap().remember_me_ticket.clone()
    }
    fn mem() -> Db {
        Db::open_in_memory().unwrap()
    }
    fn temp_db(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hangar-ubi-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn connect_saves_and_returns_name() {
        let store = Arc::new(MemoryStore::default());
        let db = mem();
        let api = Arc::new(FakeApi::new(vec![Ok(sess("T1", Some("RM1")))]));
        let c = client_arc(&api, &store);
        assert_eq!(c.connect(&db, &code()), Ok("oyuncu".to_string()));
        assert_eq!(rm_of(&store), "RM1");
        assert_eq!(*api.seen_rm.lock().unwrap(), vec!["RM0-SECRET".to_string()]);
        let st = c.status(&db);
        assert_eq!((st.platform, st.state, st.display_name.as_deref()), (Platform::Ubisoft, AccountState::Connected, Some("oyuncu")));
    }

    #[test]
    fn connect_stores_v2_cred() {
        let store = Arc::new(MemoryStore::default());
        let c = client(FakeApi::new(vec![Ok(sess("T1", Some("RM1")))]), &store);
        c.connect(&mem(), &code()).unwrap();
        assert_eq!(load_ubi_cred(&*store).unwrap().unwrap().v, 2);
    }

    #[test]
    fn v1_file_is_not_connected() {
        let store = Arc::new(MemoryStore::default());
        save_ubi_cred(&*store, &UbiCred { v: 1, remember_me_ticket: "RM".into(), user_id: "u".into(), display_name: "o".into() }).unwrap();
        assert_eq!(load_ubi_cred(&*store), Err(AuthError::NotConnected));
        let c = client(FakeApi::new(vec![]), &store);
        assert!(matches!(c.fetch_owned(), Err(AuthError::NotConnected)));
    }

    #[test]
    fn demux_merges_with_graphql() {
        let store = stored("RM1");
        let mut api = FakeApi::new(vec![Ok(sess("T2", Some("RM2")))]);
        api.products = Ok(vec![
            dg(541, true, 0, Some("sp1"), Some("Yaml Adi"), Some("a.jpg")),
            dg(3539, true, 0, Some("spX"), Some("Origins"), Some("orig-1_2.jpg")),
            dg(7, true, 0, None, Some("Bad Thumb"), Some("../x.jpg")),
            dg(8, true, 1, Some("sp1"), Some("Addon"), None),
            dg(9, false, 0, Some("sp2"), Some("Sahip degil"), None),
            dg(541, true, 0, Some("sp1"), Some("Kopya"), None),
            dg(10, true, 0, None, None, None),
        ]);
        let v = client(api, &store).fetch_owned().unwrap();
        let ids: Vec<_> = v.iter().map(|e| e.platform_id.as_str()).collect();
        assert_eq!(ids, vec!["541", "3539", "7"]);
        assert_eq!(v[0].title, "WATCH_DOGS™");
        assert_eq!(v[0].cover_url.as_deref(), Some("https://ubiservices.cdn.ubi.com/c1.jpg"));
        assert_eq!(v[0].launch_uri, "uplay://launch/541/0");
        assert_eq!(v[0].install_uri.as_deref(), Some("uplay://launch/541/0"));
        assert_eq!(v[1].title, "Origins");
        assert_eq!(v[1].cover_url.as_deref(), Some("https://ubistatic3-a.akamaihd.net/orbit/uplay_launcher_3_0/assets/orig-1_2.jpg"));
        assert_eq!(v[2].cover_url, None);
    }

    #[test]
    fn demux_error_falls_back_to_graphql() {
        let store = stored("RM1");
        let v = client(FakeApi::new(vec![Ok(sess("T2", None))]), &store).fetch_owned().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].platform_id, "sp1");
    }

    #[test]
    fn graphql_error_keeps_demux_entries() {
        let store = stored("RM1");
        let mut api = FakeApi::new(vec![Ok(sess("T2", None))]);
        api.owned = Err(AuthError::Network("x".into()));
        api.products = Ok(vec![dg(3539, true, 0, Some("spX"), Some("Origins"), None)]);
        let v = client(api, &store).fetch_owned().unwrap();
        assert_eq!((v.len(), v[0].title.as_str(), v[0].platform_id.as_str()), (1, "Origins", "3539"));
    }

    #[test]
    fn connect_without_remember_ticket_fails() {
        let store = Arc::new(MemoryStore::default());
        let c = client(FakeApi::new(vec![]), &store);
        assert!(matches!(c.connect(&mem(), r#"{"login":{"ticket":"T"}}"#), Err(AuthError::Parse(_))));
        assert_eq!(c.status(&mem()).state, AccountState::Disconnected);
    }

    #[test]
    fn connect_save_failure() {
        let store = Arc::new(MemoryStore::default());
        store.fail_save.store(true, Ordering::SeqCst);
        let c = client(FakeApi::new(vec![Ok(sess("T1", Some("RM1")))]), &store);
        assert_eq!(c.connect(&mem(), &code()), Err(AuthError::Save));
    }

    #[test]
    fn rotated_ticket_saved_before_use() {
        let store = stored("RM1");
        let mut api = FakeApi::new(vec![Ok(sess("T2", Some("RM2")))]);
        let s2 = store.clone();
        api.on_owned = Some(Box::new(move || assert_eq!(rm_of(&s2), "RM2")));
        let api = Arc::new(api);
        let c = client_arc(&api, &store);
        assert_eq!(c.fetch_owned().unwrap().len(), 1);
        assert_eq!(*api.seen_rm.lock().unwrap(), vec!["RM1".to_string()]);
    }

    #[test]
    fn failed_save_keeps_rotated_cred_in_memory() {
        let store = stored("RM1");
        store.fail_save.store(true, Ordering::SeqCst);
        let db = mem();
        let api = Arc::new(FakeApi::new(vec![Ok(sess("T2", Some("RM2"))), Ok(sess("T3", Some("RM3")))]));
        let c = client_arc(&api, &store);
        assert_eq!(c.fetch_owned().unwrap().len(), 1);
        assert_eq!(c.status(&db).state, AccountState::Connected);
        store.fail_save.store(false, Ordering::SeqCst);
        assert_eq!(c.fetch_owned().unwrap().len(), 1);
        assert_eq!(*api.seen_rm.lock().unwrap(), vec!["RM1".to_string(), "RM2".to_string()]);
        assert_eq!(rm_of(&store), "RM3");
    }

    #[test]
    fn refresh_without_rotation_keeps_ticket() {
        let store = stored("RM1");
        let c = client(FakeApi::new(vec![Ok(sess("T2", None))]), &store);
        c.fetch_owned().unwrap();
        assert_eq!(rm_of(&store), "RM1");
    }

    #[test]
    fn refresh_is_serialized() {
        let store = stored("RM1");
        let mut api = FakeApi::new(vec![Ok(sess("T2", Some("RM2"))), Ok(sess("T3", Some("RM3")))]);
        api.delay = Duration::from_millis(50);
        let api = Arc::new(api);
        let c = Arc::new(client_arc(&api, &store));
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let c = c.clone();
                std::thread::spawn(move || c.fetch_owned().unwrap())
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        let mut seen = api.seen_rm.lock().unwrap().clone();
        seen.sort();
        assert_eq!(seen, vec!["RM1".to_string(), "RM2".to_string()]);
        assert_eq!(api.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn offline_uses_cache_with_warning() {
        let store = stored("RM1");
        let db = mem();
        let c = client(FakeApi::new(vec![Ok(sess("T2", Some("RM2"))), Err(AuthError::Network("x".into()))]), &store);
        let (first, w) = c.owned_for_scan(&db);
        assert!(w.is_empty());
        let first = first.unwrap().unwrap();
        let (second, w2) = c.owned_for_scan(&db);
        assert_eq!(second.unwrap().unwrap(), first);
        assert_eq!(w2, vec!["Ubisoft oyunları güncellenemedi; son bilinen liste gösteriliyor.".to_string()]);
    }

    #[test]
    fn invalid_grant_marks_expired() {
        let store = stored("RM1");
        let db = mem();
        let c = client(FakeApi::new(vec![Err(AuthError::InvalidGrant)]), &store);
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert_eq!(w, vec!["Ubisoft bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan.".to_string()]);
        assert_eq!(c.status(&db).state, AccountState::Expired);
        assert!(store.raw(Platform::Ubisoft).is_some());
    }

    #[test]
    fn corrupt_store_is_disconnected() {
        let store = stored("RM1");
        store.corrupt.store(true, Ordering::SeqCst);
        let db = mem();
        let c = client(FakeApi::new(vec![]), &store);
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert_eq!(w, vec![WARN_CORRUPT.to_string()]);
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test]
    fn disconnect_offline_still_deletes() {
        let store = stored("RM1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(sess("T2", Some("RM2")))]);
        api.logout = Err(AuthError::Network("x".into()));
        let c = client(api, &store);
        db.owned_cache_put(Platform::Ubisoft, &[], 1).unwrap();
        db.set_account_expired(Platform::Ubisoft, true).unwrap();
        let before = store.saves();
        assert!(!c.disconnect(&db));
        assert_eq!(store.saves(), before);
        assert_eq!(store.raw(Platform::Ubisoft), None);
        assert_eq!(db.owned_cache_get(Platform::Ubisoft).unwrap(), None);
        assert!(!db.account_expired(Platform::Ubisoft).unwrap());
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test]
    fn disconnect_wins_over_inflight_scan() {
        let path = temp_db("race");
        let db = Db::open(&path).unwrap();
        db.owned_cache_put(Platform::Ubisoft, &[], 1).unwrap();
        db.set_account_expired(Platform::Ubisoft, true).unwrap();
        let store = stored("RM1");
        let slot: Arc<OnceLock<Weak<UbiClient>>> = Arc::new(OnceLock::new());
        let mut api = FakeApi::new(vec![Ok(sess("T2", Some("RM2"))), Ok(sess("T3", Some("RM3")))]);
        let (s2, p2) = (slot.clone(), path.clone());
        api.on_owned = Some(Box::new(move || {
            let c = s2.get().and_then(Weak::upgrade).unwrap();
            assert!(c.disconnect(&Db::open(&p2).unwrap()));
        }));
        let c = Arc::new(client(api, &store));
        slot.set(Arc::downgrade(&c)).unwrap();
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none() && w.is_empty());
        assert_eq!(db.owned_cache_get(Platform::Ubisoft).unwrap(), None);
        assert!(!db.account_expired(Platform::Ubisoft).unwrap());
        assert_eq!(store.raw(Platform::Ubisoft), None);
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pc_filter_and_entry_shape() {
        let v = parse_owned(&games()).unwrap();
        assert_eq!(v.len(), 1);
        let e = &v[0];
        assert_eq!((e.platform, e.platform_id.as_str(), e.title.as_str(), e.installed), (Platform::Ubisoft, "sp1", "WATCH_DOGS™", false));
        assert_eq!(e.launch_uri, "uplay://launch/sp1/0");
        assert_eq!(e.install_uri.as_deref(), Some("uplay://launch/sp1/0"));
        assert_eq!(e.cover_url.as_deref(), Some("https://ubiservices.cdn.ubi.com/c1.jpg"));
        let http = games().replace("https://ubiservices", "http://ubiservices");
        assert_eq!(parse_owned(&http).unwrap()[0].cover_url, None);
        assert!(parse_owned("nope").is_err());
    }

    #[test]
    fn no_secret_leaks() {
        let path = temp_db("leak");
        let db = Db::open(&path).unwrap();
        let store = Arc::new(MemoryStore::default());
        let c = client(
            FakeApi::new(vec![
                Ok(sess("T-SECRET", Some("RM-SECRET"))),
                Ok(sess("T-SECRET", Some("RM-SECRET"))),
                Err(AuthError::InvalidGrant),
                Err(AuthError::Network("x".into())),
            ]),
            &store,
        );
        let mut text = vec![c.connect(&db, &code()).unwrap(), format!("{:?}", c.status(&db))];
        for _ in 0..3 {
            let (r, w) = c.owned_for_scan(&db);
            text.push(format!("{r:?}"));
            text.extend(w);
        }
        let mut errors = vec![c.connect(&db, &code()).unwrap_err()];
        errors.push(c.connect(&db, "{}").unwrap_err());
        errors.push(client(FakeApi::new(vec![Ok("{\"ticket\":\"T-SECRET\"}".into())]), &store).fetch_owned().unwrap_err());
        for e in &errors {
            text.push(format!("{e} {e:?}"));
        }
        text.push(format!("{:?}", load_ubi_cred(&*store).unwrap().unwrap()));
        drop(db);
        let conn = rusqlite::Connection::open(&path).unwrap();
        for t in ["owned_cache", "settings"] {
            let mut st = conn.prepare(&format!("SELECT * FROM {t}")).unwrap();
            let n = st.column_count();
            let rows = st
                .query_map([], |r| (0..n).map(|i| Ok(format!("{:?}", r.get_ref(i)?))).collect::<rusqlite::Result<Vec<_>>>())
                .unwrap();
            for row in rows {
                text.extend(row.unwrap());
            }
        }
        drop(conn);
        let _ = std::fs::remove_file(&path);
        let all = text.join("\n");
        assert!(all.contains("oyuncu") && all.contains("sp1"), "sanity");
        for bad in ["T-SECRET", "RM-SECRET", "RM0-SECRET", "U-SECRET", "SID-SECRET", "USER-SECRET"] {
            assert!(!all.contains(bad), "leak: {bad}");
        }
    }
}
