use super::epic::{AccountState, AccountStatus};
use super::gog_api::GogApi;
use super::store::{load_gog_cred, save_gog_cred, SecretStore};
use super::{AuthError, GogCred};
use crate::db::Db;
use crate::model::{Platform, PlatformEntry};
use crate::providers::ProviderError;
use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use zeroize::Zeroizing;

const WARN_OFFLINE: &str = "GOG oyunları güncellenemedi; son bilinen liste gösteriliyor.";
const WARN_EXPIRED: &str = "GOG bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan.";
const WARN_CORRUPT: &str = "GOG bağlantısı okunamadı; yeniden bağlan.";
/// The token response carries no account name.
const DISPLAY_NAME: &str = "GOG hesabı";
/// Safety stop against a server that keeps growing `totalPages`.
const MAX_PAGES: u32 = 200;

struct GogTokens {
    access: Zeroizing<String>,
    refresh: Zeroizing<String>,
    user_id: String,
}

impl fmt::Debug for GogTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GogTokens")
            .field("access", &"***")
            .field("refresh", &"***")
            .field("user_id", &"***")
            .finish()
    }
}

fn parse_tokens(json: &str) -> Result<GogTokens, AuthError> {
    let bad = || AuthError::Parse("token".into());
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| bad())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string).ok_or_else(bad);
    let user_id = v
        .get("user_id")
        .and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_i64().map(|n| n.to_string())))
        .unwrap_or_default();
    Ok(GogTokens { access: Zeroizing::new(s("access_token")?), refresh: Zeroizing::new(s("refresh_token")?), user_id })
}

pub struct GogClient {
    api: Box<dyn GogApi>,
    store: Box<dyn SecretStore>,
    /// Serializes refresh-token rotation and scan-result handling; never held across page fetches.
    /// Holds a rotated credential whose save failed, so a valid connection is not lost (retried on next refresh).
    lock: Mutex<Option<GogCred>>,
    /// Bumped by connect/disconnect; a scan whose generation changed meanwhile is discarded.
    generation: AtomicU64,
}

impl GogClient {
    pub fn new(api: Box<dyn GogApi>, store: Box<dyn SecretStore>) -> Self {
        GogClient { api, store, lock: Mutex::new(None), generation: AtomicU64::new(0) }
    }

    fn guard(&self) -> MutexGuard<'_, Option<GogCred>> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn connect(&self, db: &Db, code: &str) -> Result<String, AuthError> {
        let mut pending = self.guard();
        let body = Zeroizing::new(self.api.exchange(code)?);
        let t = parse_tokens(&body)?;
        let cred = GogCred {
            v: 1,
            refresh_token: t.refresh.to_string(),
            user_id: t.user_id.clone(),
            display_name: DISPLAY_NAME.into(),
        };
        save_gog_cred(&*self.store, &cred).map_err(|_| AuthError::Save)?;
        *pending = None;
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = db.set_account_expired(Platform::Gog, false);
        Ok(cred.display_name.clone())
    }

    /// Caller holds `lock`. Uses the pending credential if any, else the stored one. When the server
    /// rotated the refresh token (or a rotation is still pending), the new credential is saved before the
    /// access token is returned; if the save fails it stays in `pending` and the next refresh retries it.
    fn refresh_locked(&self, pending: &mut Option<GogCred>) -> Result<Zeroizing<String>, AuthError> {
        let cred = match pending.as_ref() {
            Some(c) => c.clone(),
            None => match load_gog_cred(&*self.store) {
                Ok(Some(c)) => c,
                Ok(None) => return Err(AuthError::NotConnected),
                Err(AuthError::Corrupt) => {
                    let _ = self.store.delete(Platform::Gog);
                    return Err(AuthError::Corrupt);
                }
                Err(e) => return Err(e),
            },
        };
        let body = Zeroizing::new(self.api.refresh(&cred.refresh_token)?);
        let t = parse_tokens(&body)?;
        if *t.refresh != cred.refresh_token || pending.is_some() {
            let next = GogCred {
                v: 1,
                refresh_token: t.refresh.to_string(),
                user_id: if t.user_id.is_empty() { cred.user_id.clone() } else { t.user_id.clone() },
                display_name: cred.display_name.clone(),
            };
            *pending = match save_gog_cred(&*self.store, &next) {
                Ok(()) => None,
                Err(_) => Some(next),
            };
        }
        Ok(t.access.clone())
    }

    pub fn access_token(&self) -> Result<Zeroizing<String>, AuthError> {
        let mut pending = self.guard();
        self.refresh_locked(&mut pending)
    }

    pub fn fetch_owned(&self) -> Result<Vec<PlatformEntry>, AuthError> {
        let access = self.access_token()?;
        let mut out: Vec<PlatformEntry> = Vec::new();
        let mut seen = HashSet::new();
        let mut page = 1;
        loop {
            let body = self.api.filtered_page(&access, page)?;
            let (items, total) = parse_filtered(&body)?;
            out.extend(items.into_iter().filter(|e| seen.insert(e.platform_id.clone())));
            if page >= total || page >= MAX_PAGES {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    pub fn owned_for_scan(&self, db: &Db) -> (Option<Result<Vec<PlatformEntry>, ProviderError>>, Vec<String>) {
        let gen = self.generation.load(Ordering::SeqCst);
        let r = self.fetch_owned();
        let _g = self.guard();
        if self.generation.load(Ordering::SeqCst) != gen {
            return (None, vec![]);
        }
        let cache = || db.owned_cache_get(Platform::Gog).ok().flatten().map(Ok);
        match r {
            Ok(v) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64);
                let _ = db.owned_cache_put(Platform::Gog, &v, now);
                let _ = db.set_account_expired(Platform::Gog, false);
                (Some(Ok(v)), vec![])
            }
            Err(AuthError::NotConnected) => (None, vec![]),
            Err(AuthError::Network(_) | AuthError::Parse(_) | AuthError::Save) => (cache(), vec![WARN_OFFLINE.into()]),
            Err(AuthError::InvalidGrant) => {
                let _ = db.set_account_expired(Platform::Gog, true);
                (cache(), vec![WARN_EXPIRED.into()])
            }
            Err(AuthError::Corrupt) => (None, vec![WARN_CORRUPT.into()]),
        }
    }

    /// GOG has no revoke endpoint: forgets everything local. Always `true`.
    pub fn disconnect(&self, db: &Db) -> bool {
        let mut pending = self.guard();
        self.generation.fetch_add(1, Ordering::SeqCst);
        *pending = None;
        let _ = self.store.delete(Platform::Gog);
        let _ = db.owned_cache_delete(Platform::Gog);
        let _ = db.set_account_expired(Platform::Gog, false);
        true
    }

    /// Offline: reads only the pending/stored credential and the expired flag.
    pub fn status(&self, db: &Db) -> AccountStatus {
        let pending = self.guard().clone();
        let cred = match pending {
            Some(c) => Some(c),
            None => load_gog_cred(&*self.store).ok().flatten(),
        };
        let (state, display_name) = match cred {
            Some(c) => {
                let st = if db.account_expired(Platform::Gog).unwrap_or(false) {
                    AccountState::Expired
                } else {
                    AccountState::Connected
                };
                (st, Some(c.display_name.clone()))
            }
            None => (AccountState::Disconnected, None),
        };
        AccountStatus { platform: Platform::Gog, state, display_name }
    }
}

/// `getFilteredProducts` page -> (entries, totalPages). Non-games (movies) and untitled products are skipped.
pub fn parse_filtered(json: &str) -> Result<(Vec<PlatformEntry>, u32), AuthError> {
    let bad = || AuthError::Parse("products".into());
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| bad())?;
    let products = v.get("products").and_then(|p| p.as_array()).ok_or_else(bad)?;
    let total = v.get("totalPages").and_then(|t| t.as_u64()).unwrap_or(1).clamp(1, u32::MAX as u64) as u32;
    let entries = products
        .iter()
        .filter_map(|p| {
            let id = p.get("id").and_then(|i| i.as_u64())?.to_string();
            let title = p.get("title").and_then(|t| t.as_str()).map(str::trim).filter(|t| !t.is_empty())?;
            if p.get("isGame").and_then(|g| g.as_bool()) == Some(false) {
                return None;
            }
            let cover_url = p
                .get("image")
                .and_then(|i| i.as_str())
                .and_then(|i| i.strip_prefix("//"))
                .filter(|i| !i.is_empty())
                .map(|i| format!("https://{i}_392.jpg"));
            let uri = format!("goggalaxy://openGameView/{id}");
            Some(PlatformEntry {
                platform: Platform::Gog,
                platform_id: id,
                title: title.to_string(),
                installed: false,
                install_path: None,
                size_bytes: None,
                last_played: None,
                playtime_min: None,
                launch_uri: uri.clone(),
                install_uri: Some(uri),
                launch_args: vec![],
                cover_url,
                edition: None,
                copy: None,
            })
        })
        .collect();
    Ok((entries, total))
}

#[cfg(test)]
mod tests {
    use super::super::store::MemoryStore;
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, OnceLock, Weak};

    fn fx(n: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gog").join(n)).unwrap()
    }

    struct FakeApi {
        exchange: Mutex<VecDeque<Result<String, AuthError>>>,
        refresh: Mutex<VecDeque<Result<String, AuthError>>>,
        pages: Vec<String>,
        seen_refresh: Mutex<Vec<String>>,
        page_calls: AtomicUsize,
        on_page: Option<Box<dyn Fn() + Send + Sync>>,
    }

    impl FakeApi {
        fn new(refresh: Vec<Result<String, AuthError>>) -> Self {
            FakeApi {
                exchange: Mutex::new(VecDeque::new()),
                refresh: Mutex::new(refresh.into()),
                pages: vec![page(&[(1, "Alpha")], 1)],
                seen_refresh: Mutex::new(vec![]),
                page_calls: AtomicUsize::new(0),
                on_page: None,
            }
        }
    }

    impl GogApi for FakeApi {
        fn exchange(&self, _code: &str) -> Result<String, AuthError> {
            self.exchange.lock().unwrap().pop_front().unwrap_or(Err(AuthError::Network("bitti".into())))
        }
        fn refresh(&self, rt: &str) -> Result<String, AuthError> {
            self.seen_refresh.lock().unwrap().push(rt.to_string());
            self.refresh.lock().unwrap().pop_front().unwrap_or(Err(AuthError::Network("bitti".into())))
        }
        fn filtered_page(&self, _token: &str, page: u32) -> Result<String, AuthError> {
            self.page_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(h) = &self.on_page {
                h();
            }
            Ok(self.pages[page as usize - 1].clone())
        }
    }

    struct ArcApi(Arc<FakeApi>);
    impl GogApi for ArcApi {
        fn exchange(&self, c: &str) -> Result<String, AuthError> { self.0.exchange(c) }
        fn refresh(&self, r: &str) -> Result<String, AuthError> { self.0.refresh(r) }
        fn filtered_page(&self, t: &str, p: u32) -> Result<String, AuthError> { self.0.filtered_page(t, p) }
    }

    fn tok(at: &str, rt: &str) -> String {
        serde_json::json!({"access_token": at, "refresh_token": rt, "expires_in": 3600, "user_id": "U"}).to_string()
    }
    fn page(items: &[(u64, &str)], total: u32) -> String {
        let products: Vec<_> = items
            .iter()
            .map(|(id, t)| serde_json::json!({"id": id, "title": t, "image": format!("//images-1.gog-statics.com/h{id}"), "isGame": true}))
            .collect();
        serde_json::json!({"page": 1, "totalPages": total, "products": products}).to_string()
    }
    fn stored(rt: &str) -> Arc<MemoryStore> {
        let s = Arc::new(MemoryStore::default());
        save_gog_cred(&*s, &GogCred { v: 1, refresh_token: rt.into(), user_id: "U".into(), display_name: "GOG hesabı".into() }).unwrap();
        s
    }
    fn client_arc(api: &Arc<FakeApi>, store: &Arc<MemoryStore>) -> GogClient {
        GogClient::new(Box::new(ArcApi(api.clone())), Box::new(store.clone()))
    }
    fn client(api: FakeApi, store: &Arc<MemoryStore>) -> GogClient {
        client_arc(&Arc::new(api), store)
    }
    fn mem() -> Db {
        Db::open_in_memory().unwrap()
    }
    fn temp_db(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hangar-gog-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn parse_filtered_maps_products() {
        let (v, total) = parse_filtered(&fx("filtered.json")).unwrap();
        assert_eq!(total, 1);
        assert_eq!(v.len(), 2);
        let e = &v[0];
        assert_eq!((e.platform, e.platform_id.as_str(), e.title.as_str(), e.installed), (Platform::Gog, "2099051765", "9 Years of Shadows", false));
        assert_eq!(e.launch_uri, "goggalaxy://openGameView/2099051765");
        assert_eq!(e.install_uri.as_deref(), Some("goggalaxy://openGameView/2099051765"));
        assert_eq!(
            e.cover_url.as_deref(),
            Some("https://images-3.gog-statics.com/346e28f68ac65fd83a31c81d3a99cbf272c23f5f61be330ef413664761fff26c_392.jpg")
        );
        assert_eq!(v[1].title, "A Plague Tale: Innocence");
        assert!(parse_filtered("nope").is_err());
        assert!(parse_filtered("{}").is_err());
    }

    #[test]
    fn parse_filtered_skips_movies_and_untitled() {
        let j = r#"{"totalPages":3,"products":[{"id":1,"title":"Film","isGame":false},{"id":2,"title":" "},{"id":3,"title":"Ok"}]}"#;
        let (v, total) = parse_filtered(j).unwrap();
        assert_eq!(total, 3);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].platform_id.as_str(), v[0].cover_url.as_deref()), ("3", None));
    }

    #[test]
    fn connect_stores_refresh_token() {
        let store = Arc::new(MemoryStore::default());
        let db = mem();
        let api = FakeApi::new(vec![]);
        api.exchange.lock().unwrap().push_back(Ok(tok("A", "R")));
        let c = client(api, &store);
        assert_eq!(c.connect(&db, "CODE"), Ok("GOG hesabı".to_string()));
        let cred = load_gog_cred(&*store).unwrap().unwrap();
        assert_eq!((cred.refresh_token.as_str(), cred.user_id.as_str(), cred.v), ("R", "U", 1));
        let st = c.status(&db);
        assert_eq!((st.platform, st.state), (Platform::Gog, AccountState::Connected));
        assert_eq!(st.display_name.as_deref(), Some("GOG hesabı"));
    }

    #[test]
    fn connect_save_failure_is_save_error() {
        let store = Arc::new(MemoryStore::default());
        store.fail_save.store(true, Ordering::SeqCst);
        let db = mem();
        let api = FakeApi::new(vec![]);
        api.exchange.lock().unwrap().push_back(Ok(tok("A", "R")));
        let c = client(api, &store);
        assert_eq!(c.connect(&db, "CODE"), Err(AuthError::Save));
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test]
    fn refresh_keeps_same_token() {
        let store = stored("R1");
        let api = Arc::new(FakeApi::new(vec![Ok(tok("A2", "R1")), Ok(tok("A3", "R1"))]));
        let c = client_arc(&api, &store);
        let before = store.saves();
        assert_eq!(c.access_token().unwrap().as_str(), "A2");
        assert_eq!(c.access_token().unwrap().as_str(), "A3");
        assert_eq!(store.saves(), before, "unchanged token is not rewritten");
        assert_eq!(*api.seen_refresh.lock().unwrap(), vec!["R1".to_string(), "R1".to_string()]);
    }

    #[test]
    fn rotated_token_saved_before_use() {
        let store = stored("R1");
        let mut api = FakeApi::new(vec![Ok(tok("A2", "R2"))]);
        let s2 = store.clone();
        api.on_page = Some(Box::new(move || {
            assert_eq!(load_gog_cred(&*s2).unwrap().unwrap().refresh_token, "R2");
        }));
        let c = client(api, &store);
        assert_eq!(c.fetch_owned().unwrap().len(), 1);
    }

    #[test]
    fn failed_save_keeps_rotated_cred_in_memory() {
        let store = stored("R1");
        store.fail_save.store(true, Ordering::SeqCst);
        let db = mem();
        let api = Arc::new(FakeApi::new(vec![Ok(tok("A2", "R2")), Ok(tok("A3", "R3"))]));
        let c = client_arc(&api, &store);
        assert_eq!(c.access_token().unwrap().as_str(), "A2");
        assert_eq!(c.status(&db).state, AccountState::Connected);
        store.fail_save.store(false, Ordering::SeqCst);
        assert_eq!(c.access_token().unwrap().as_str(), "A3");
        assert_eq!(*api.seen_refresh.lock().unwrap(), vec!["R1".to_string(), "R2".to_string()]);
        assert_eq!(load_gog_cred(&*store).unwrap().unwrap().refresh_token, "R3");
    }

    #[test]
    fn invalid_grant_marks_expired() {
        let store = stored("R1");
        let db = mem();
        let c = client(FakeApi::new(vec![Err(AuthError::InvalidGrant)]), &store);
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert_eq!(w, vec![WARN_EXPIRED.to_string()]);
        assert_eq!(c.status(&db).state, AccountState::Expired);
        assert!(store.raw(Platform::Gog).is_some());
    }

    #[test]
    fn offline_returns_cache() {
        let store = stored("R1");
        let db = mem();
        let c = client(FakeApi::new(vec![Ok(tok("A2", "R1")), Err(AuthError::Network("x".into()))]), &store);
        let (first, w) = c.owned_for_scan(&db);
        assert!(w.is_empty());
        let first = first.unwrap().unwrap();
        assert_eq!(first.len(), 1);
        let (second, w2) = c.owned_for_scan(&db);
        assert_eq!(second.unwrap().unwrap(), first);
        assert_eq!(w2, vec![WARN_OFFLINE.to_string()]);
    }

    #[test]
    fn corrupt_store_is_disconnected() {
        let store = stored("R1");
        store.corrupt.store(true, Ordering::SeqCst);
        let db = mem();
        let c = client(FakeApi::new(vec![]), &store);
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert_eq!(w, vec![WARN_CORRUPT.to_string()]);
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test]
    fn not_connected_is_silent() {
        let store = Arc::new(MemoryStore::default());
        let c = client(FakeApi::new(vec![]), &store);
        assert_eq!(c.owned_for_scan(&mem()), (None, vec![]));
    }

    #[test]
    fn disconnect_forgets_everything() {
        let store = stored("R1");
        let db = mem();
        let c = client(FakeApi::new(vec![]), &store);
        db.owned_cache_put(Platform::Gog, &[], 1).unwrap();
        db.set_account_expired(Platform::Gog, true).unwrap();
        assert!(c.disconnect(&db));
        assert_eq!(store.raw(Platform::Gog), None);
        assert_eq!(db.owned_cache_get(Platform::Gog).unwrap(), None);
        assert!(!db.account_expired(Platform::Gog).unwrap());
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test]
    fn disconnect_during_scan_discards() {
        let path = temp_db("race");
        let db = Db::open(&path).unwrap();
        db.owned_cache_put(Platform::Gog, &[], 1).unwrap();
        db.set_account_expired(Platform::Gog, true).unwrap();
        let store = stored("R1");
        let slot: Arc<OnceLock<Weak<GogClient>>> = Arc::new(OnceLock::new());
        let mut api = FakeApi::new(vec![Ok(tok("A2", "R1"))]);
        let (s2, p2) = (slot.clone(), path.clone());
        api.on_page = Some(Box::new(move || {
            let c = s2.get().and_then(Weak::upgrade).unwrap();
            let db2 = Db::open(&p2).unwrap();
            assert!(c.disconnect(&db2));
        }));
        let c = Arc::new(client(api, &store));
        slot.set(Arc::downgrade(&c)).unwrap();
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert!(w.is_empty());
        assert_eq!(db.owned_cache_get(Platform::Gog).unwrap(), None);
        assert!(!db.account_expired(Platform::Gog).unwrap());
        assert_eq!(store.raw(Platform::Gog), None);
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pages_are_concatenated() {
        let store = stored("R1");
        let mut api = FakeApi::new(vec![Ok(tok("A2", "R1"))]);
        api.pages = vec![page(&[(1, "Alpha"), (2, "Beta")], 2), page(&[(2, "Beta"), (3, "Gamma")], 2)];
        let api = Arc::new(api);
        let c = client_arc(&api, &store);
        let v = c.fetch_owned().unwrap();
        assert_eq!(v.iter().map(|e| e.platform_id.as_str()).collect::<Vec<_>>(), vec!["1", "2", "3"]);
        assert_eq!(api.page_calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn debug_redacts() {
        let cred = GogCred { v: 1, refresh_token: "RT-SECRET".into(), user_id: "UID-SECRET".into(), display_name: "d".into() };
        let t = parse_tokens(r#"{"access_token":"AT-SECRET","refresh_token":"RT-SECRET","user_id":"UID-SECRET"}"#).unwrap();
        let s = format!("{cred:?} {t:?}");
        for bad in ["AT-SECRET", "RT-SECRET", "UID-SECRET"] {
            assert!(!s.contains(bad), "{bad}");
        }
        let errs = [AuthError::Network("durum 500".into()), AuthError::InvalidGrant, AuthError::Save, AuthError::Parse("token".into())];
        for e in errs {
            assert!(!format!("{e} {e:?}").contains("http"));
        }
        assert_eq!(parse_tokens(r#"{"access_token":"A"}"#).unwrap_err(), AuthError::Parse("token".into()));
        assert_eq!(parse_tokens(r#"{"access_token":"A","refresh_token":"R","user_id":123}"#).unwrap().user_id, "123");
    }

    #[test]
    fn no_secret_leaks_to_db_or_text() {
        let path = temp_db("leak");
        let db = Db::open(&path).unwrap();
        let store = Arc::new(MemoryStore::default());
        let api = FakeApi::new(vec![Ok(tok("AT-SECRET", "RT-SECRET")), Err(AuthError::InvalidGrant), Err(AuthError::Network("x".into()))]);
        api.exchange.lock().unwrap().push_back(Ok(tok("AT-SECRET", "RT-SECRET")));
        let c = client(api, &store);
        let mut text = vec![c.connect(&db, "CODE-SECRET").unwrap()];
        for _ in 0..3 {
            let (r, w) = c.owned_for_scan(&db);
            text.push(format!("{r:?}"));
            text.extend(w);
        }
        text.push(format!("{:?}", c.status(&db)));
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
        assert!(all.contains("Alpha"), "sanity");
        for bad in ["AT-SECRET", "RT-SECRET", "CODE-SECRET"] {
            assert!(!all.contains(bad), "leak: {bad}");
        }
    }
}
