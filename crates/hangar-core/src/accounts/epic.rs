use super::epic_api::EpicApi;
use super::store::{load_cred, save_cred, SecretStore};
use super::{parse_token, AuthError, StoredCred};
use crate::db::Db;
use crate::model::{Platform, PlatformEntry};
use crate::providers::ProviderError;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use zeroize::Zeroizing;

const CATALOG_CHUNK: usize = 50;
const WARN_OFFLINE: &str = "Epic oyunları güncellenemedi; son bilinen liste gösteriliyor.";
const WARN_EXPIRED: &str = "Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan.";
const WARN_CORRUPT: &str = "Epic bağlantısı okunamadı; yeniden bağlan.";

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AccountState {
    Disconnected,
    Connected,
    Expired,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct AccountStatus {
    pub platform: Platform,
    pub state: AccountState,
    pub display_name: Option<String>,
}

pub struct EpicClient {
    api: Box<dyn EpicApi>,
    store: Box<dyn SecretStore>,
    /// Serializes refresh-token rotation and scan-result handling; never held across library/catalog calls.
    /// Holds a rotated credential whose save failed, so a valid connection is not lost (retried on next refresh).
    lock: Mutex<Option<StoredCred>>,
    /// Bumped by connect/disconnect; a scan whose generation changed meanwhile is discarded.
    generation: AtomicU64,
    catalog_pause: Duration,
}

impl EpicClient {
    pub fn new(api: Box<dyn EpicApi>, store: Box<dyn SecretStore>) -> Self {
        EpicClient {
            api,
            store,
            lock: Mutex::new(None),
            generation: AtomicU64::new(0),
            catalog_pause: Duration::from_millis(100),
        }
    }

    fn guard(&self) -> MutexGuard<'_, Option<StoredCred>> {
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn connect(&self, db: &Db, code: &str) -> Result<String, AuthError> {
        let mut pending = self.guard();
        let body = Zeroizing::new(self.api.token(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("token_type", "eg1"),
        ])?);
        let t = parse_token(&body)?;
        save_cred(&*self.store, &t.cred).map_err(|_| AuthError::Save)?;
        *pending = None;
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = db.set_account_expired(Platform::Epic, false);
        Ok(t.cred.display_name.clone())
    }

    /// Caller holds `lock` (passes its contents). Uses the pending credential if any, else the stored one.
    /// With `persist`, the rotated credential is saved before the access token is returned; if the save
    /// fails it stays in `pending` (the token is still returned) and the next refresh saves its rotation.
    fn refresh_locked(&self, pending: &mut Option<StoredCred>, persist: bool) -> Result<Zeroizing<String>, AuthError> {
        let cred = match pending.as_ref() {
            Some(c) => c.clone(),
            None => match load_cred(&*self.store) {
                Ok(Some(c)) => c,
                Ok(None) => return Err(AuthError::NotConnected),
                Err(AuthError::Corrupt) => {
                    let _ = self.store.delete(Platform::Epic);
                    return Err(AuthError::Corrupt);
                }
                Err(e) => return Err(e),
            },
        };
        let body = Zeroizing::new(self.api.token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &cred.refresh_token),
            ("token_type", "eg1"),
        ])?);
        let t = parse_token(&body)?;
        if persist {
            *pending = match save_cred(&*self.store, &t.cred) {
                Ok(()) => None,
                Err(_) => Some(t.cred.clone()),
            };
        }
        Ok(Zeroizing::new(t.access_token.clone()))
    }

    pub fn access_token(&self) -> Result<Zeroizing<String>, AuthError> {
        let mut pending = self.guard();
        self.refresh_locked(&mut pending, true)
    }

    pub fn fetch_owned(&self, db: &Db) -> Result<Vec<PlatformEntry>, AuthError> {
        let gen = self.generation.load(Ordering::SeqCst);
        let access = self.access_token()?;
        let mut items = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let body = self.api.library_page(&access, cursor.as_deref())?;
            let (page, next) = parse_library_page(&body)?;
            items.extend(page);
            match next {
                Some(c) if Some(&c) != cursor.as_ref() => cursor = Some(c),
                _ => break,
            }
        }
        let mut ids: Vec<String> = items.iter().map(|i| i.catalog_item_id.clone()).collect();
        ids.sort();
        ids.dedup();
        let mut catalog = db.catalog_get(&ids).map_err(|_| AuthError::Network("veritabanı".into()))?;
        let mut missing: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for i in &items {
            let group = missing.entry(i.namespace.as_str()).or_default();
            if !catalog.contains_key(&i.catalog_item_id) && !group.contains(&i.catalog_item_id) {
                group.push(i.catalog_item_id.clone());
            }
        }
        let mut fetched = HashMap::new();
        // A failed chunk does not discard the others; its ids stay uncached and are retried next scan.
        let mut failed = None;
        let mut first = true;
        for (ns, group) in &missing {
            for chunk in group.chunks(CATALOG_CHUNK) {
                if !first {
                    std::thread::sleep(self.catalog_pause);
                }
                first = false;
                match self.api.catalog(&access, ns, chunk).and_then(|b| parse_catalog(&b)) {
                    Ok(m) => fetched.extend(m),
                    Err(e) => {
                        failed.get_or_insert(e);
                    }
                }
            }
        }
        if !fetched.is_empty() {
            // Under the lock, so a disconnect that ran meanwhile is not undone by this write.
            // A failed cache write only costs a re-fetch next scan.
            let _g = self.guard();
            if self.generation.load(Ordering::SeqCst) == gen {
                let _ = db.catalog_put(&fetched);
            }
            catalog.extend(fetched);
        }
        let entries = owned_entries(&items, &catalog);
        match failed {
            Some(e) if entries.is_empty() => Err(e),
            _ => Ok(entries),
        }
    }

    pub fn owned_for_scan(&self, db: &Db) -> (Option<Result<Vec<PlatformEntry>, ProviderError>>, Vec<String>) {
        let gen = self.generation.load(Ordering::SeqCst);
        let r = self.fetch_owned(db);
        let _g = self.guard();
        if self.generation.load(Ordering::SeqCst) != gen {
            return (None, vec![]);
        }
        let cache = || db.owned_cache_get(Platform::Epic).ok().flatten().map(Ok);
        match r {
            Ok(v) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64);
                let _ = db.owned_cache_put(Platform::Epic, &v, now);
                let _ = db.set_account_expired(Platform::Epic, false);
                (Some(Ok(v)), vec![])
            }
            Err(AuthError::NotConnected) => (None, vec![]),
            Err(AuthError::Network(_) | AuthError::Parse(_) | AuthError::Save) => (cache(), vec![WARN_OFFLINE.into()]),
            Err(AuthError::InvalidGrant) => {
                let _ = db.set_account_expired(Platform::Epic, true);
                (cache(), vec![WARN_EXPIRED.into()])
            }
            Err(AuthError::Corrupt) => (None, vec![WARN_CORRUPT.into()]),
        }
    }

    /// Revokes the session if reachable (without persisting the rotation), then always forgets everything local.
    pub fn disconnect(&self, db: &Db) -> bool {
        let mut pending = self.guard();
        self.generation.fetch_add(1, Ordering::SeqCst);
        let revoked = self
            .refresh_locked(&mut pending, false)
            .and_then(|at| self.api.kill(&at))
            .is_ok();
        *pending = None;
        let _ = self.store.delete(Platform::Epic);
        let _ = db.owned_cache_delete(Platform::Epic);
        let _ = db.catalog_clear();
        let _ = db.set_account_expired(Platform::Epic, false);
        revoked
    }

    /// Offline: reads only the pending/stored credential and the expired flag.
    pub fn status(&self, db: &Db) -> AccountStatus {
        let pending = self.guard().clone();
        let cred = match pending {
            Some(c) => Some(c),
            None => load_cred(&*self.store).ok().flatten(),
        };
        let (state, display_name) = match cred {
            Some(c) => {
                let st = if db.account_expired(Platform::Epic).unwrap_or(false) {
                    AccountState::Expired
                } else {
                    AccountState::Connected
                };
                (st, Some(c.display_name.clone()))
            }
            None => (AccountState::Disconnected, None),
        };
        AccountStatus { platform: Platform::Epic, state, display_name }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibItem {
    pub namespace: String,
    pub catalog_item_id: String,
    pub app_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogInfo {
    pub title: String,
    pub is_game: bool,
    /// Tall box art (https only) from catalog `keyImages`.
    pub cover_url: Option<String>,
}

pub fn parse_library_page(json: &str) -> Result<(Vec<LibItem>, Option<String>), AuthError> {
    let bad = || AuthError::Parse("library".into());
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| bad())?;
    let records = v.get("records").and_then(|r| r.as_array()).ok_or_else(bad)?;
    let items = records
        .iter()
        .filter_map(|r| {
            let s = |k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("");
            let (ns, id, app) = (s("namespace"), s("catalogItemId"), s("appName"));
            if ns == "ue" || app.is_empty() {
                return None;
            }
            Some(LibItem { namespace: ns.into(), catalog_item_id: id.into(), app_name: app.into() })
        })
        .collect();
    let cursor = v
        .pointer("/responseMetadata/nextCursor")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    Ok((items, cursor))
}

/// Prefers DieselGameBoxTall, then OfferImageTall, then DieselGameBox; https only.
fn pick_cover(item: &serde_json::Value) -> Option<String> {
    let imgs = item.get("keyImages")?.as_array()?;
    ["DieselGameBoxTall", "OfferImageTall", "DieselGameBox"].iter().find_map(|ty| {
        imgs.iter().find_map(|i| {
            let url = i.get("url")?.as_str()?;
            (i.get("type")?.as_str()? == *ty && url.starts_with("https://")).then(|| url.to_string())
        })
    })
}

pub fn parse_catalog(json: &str) -> Result<HashMap<String, CatalogInfo>, AuthError> {
    let bad = || AuthError::Parse("catalog".into());
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| bad())?;
    let obj = v.as_object().ok_or_else(bad)?;
    let mut out = HashMap::new();
    for (id, item) in obj {
        let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
        let cats: Vec<&str> = item
            .get("categories")
            .and_then(|c| c.as_array())
            .map(|a| a.iter().filter_map(|c| c.get("path").and_then(|p| p.as_str())).collect())
            .unwrap_or_default();
        let has = |p: &str| cats.contains(&p);
        // A blank title is not a game (cached as such, so it is not re-fetched every scan).
        let is_game = !title.trim().is_empty()
            && item.get("mainGameItem").map_or(true, |m| m.is_null())
            && !has("addons")
            && !has("digitalextras")
            && (!has("applications") || has("games"));
        out.insert(id.clone(), CatalogInfo { title, is_game, cover_url: pick_cover(item) });
    }
    Ok(out)
}

pub fn owned_entries(items: &[LibItem], catalog: &HashMap<String, CatalogInfo>) -> Vec<PlatformEntry> {
    items
        .iter()
        .filter_map(|i| {
            let info = catalog.get(&i.catalog_item_id).filter(|c| c.is_game)?;
            let base = format!("com.epicgames.launcher://apps/{}%3A{}%3A{}", i.namespace, i.catalog_item_id, i.app_name);
            Some(PlatformEntry {
                platform: Platform::Epic,
                platform_id: i.app_name.clone(),
                title: info.title.clone(),
                installed: false,
                install_path: None,
                size_bytes: None,
                last_played: None,
                playtime_min: None,
                launch_uri: format!("{base}?action=launch&silent=true"),
                install_uri: Some(format!("{base}?action=install")),
                launch_args: vec![],
                cover_url: info.cover_url.clone(),
                edition: None,
                copy: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fx(n: &str) -> String { std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/epic").join(n)).unwrap() }
    #[test] fn library_pages_and_cursor() {
        let (a, c) = parse_library_page(&fx("synthetic-library-0.json")).unwrap();
        assert_eq!(c.as_deref(), Some("CUR1"));
        assert_eq!(a.iter().map(|i| i.app_name.as_str()).collect::<Vec<_>>(), vec!["Fig", "FigDlc"]); // ue skipped
        let (b, c2) = parse_library_page(&fx("synthetic-library-1.json")).unwrap();
        assert_eq!(c2, None);
        assert_eq!(b.len(), 2);
        assert!(parse_library_page("nope").is_err());
    }
    #[test] fn catalog_cover_preference() {
        let c = parse_catalog(r#"{
          "a":{"title":"A","keyImages":[{"type":"DieselGameBox","url":"https://i/box.jpg"},{"type":"OfferImageTall","url":"https://i/offer.jpg"},{"type":"DieselGameBoxTall","url":"https://i/tall.jpg"}]},
          "b":{"title":"B","keyImages":[{"type":"DieselGameBox","url":"https://i/box.jpg"},{"type":"OfferImageTall","url":"https://i/offer.jpg"}]},
          "c":{"title":"C","keyImages":[{"type":"DieselGameBoxTall","url":"http://i/tall.jpg"},{"type":"DieselGameBox","url":"https://i/box.jpg"}]},
          "d":{"title":"D","keyImages":[{"type":"Thumbnail","url":"https://i/t.jpg"}]},
          "e":{"title":"E"}}"#).unwrap();
        assert_eq!(c["a"].cover_url.as_deref(), Some("https://i/tall.jpg"));
        assert_eq!(c["b"].cover_url.as_deref(), Some("https://i/offer.jpg"));
        assert_eq!(c["c"].cover_url.as_deref(), Some("https://i/box.jpg"));
        assert_eq!(c["d"].cover_url, None);
        assert_eq!(c["e"].cover_url, None);
    }
    #[test] fn catalog_filters() {
        let c = parse_catalog(&fx("synthetic-catalog.json")).unwrap();
        assert!(c["game1"].is_game);
        assert_eq!(c["game1"].title, "Death's Door");
        assert!(!c["dlc1"].is_game && !c["extra1"].is_game && !c["app1"].is_game);
    }
    /// Live responses stay only on the user's PC; the test is skipped when `HANGAR_EPIC_LIVE_DIR` is unset.
    fn live_files(prefix: &str) -> Option<Vec<(u32, String)>> {
        let dir = std::path::PathBuf::from(std::env::var_os("HANGAR_EPIC_LIVE_DIR")?);
        let mut v: Vec<(u32, String)> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| {
                let p = e.unwrap().path();
                let name = p.file_name()?.to_str()?.to_string();
                let n = name.strip_prefix(prefix)?.strip_suffix(".json")?.parse::<u32>().ok()?;
                Some((n, std::fs::read_to_string(&p).unwrap()))
            })
            .collect();
        v.sort_by_key(|(n, _)| *n);
        Some(v)
    }
    #[test] #[ignore] fn live_library_parses() {
        let Some(pages) = live_files("live-library-") else { return };
        assert!(!pages.is_empty(), "no live-library-*.json files");
        assert!(pages.iter().enumerate().all(|(i, (n, _))| *n as usize == i), "page files must be numbered 0..n");
        let mut total = 0;
        for (i, (_, json)) in pages.iter().enumerate() {
            let (items, cursor) = parse_library_page(json).unwrap();
            total += items.len();
            if i + 1 < pages.len() { assert!(cursor.is_some(), "page {i} should have a cursor"); }
        }
        assert!(total > 0, "no records parsed");
    }
    #[test] #[ignore] fn live_catalog_parses() {
        let Some(files) = live_files("live-catalog-") else { return };
        assert!(!files.is_empty(), "no live-catalog-*.json files");
        let mut games = 0;
        for (_, json) in &files {
            games += parse_catalog(json).unwrap().values().filter(|c| c.is_game).count();
        }
        assert!(games > 0, "no is_game record");
    }
    #[test] fn catalog_blank_title_is_not_a_game() {
        let c = parse_catalog(r#"{"a":{"title":"  ","categories":[{"path":"games"}]},"b":{"categories":[{"path":"games"}]}}"#).unwrap();
        assert!(!c["a"].is_game && !c["b"].is_game);
    }
    #[test] fn catalog_missing_item_skipped() {
        let items = vec![LibItem { namespace: "ns".into(), catalog_item_id: "gone".into(), app_name: "Gone".into() }];
        assert!(owned_entries(&items, &HashMap::new()).is_empty());
    }
    #[test] fn entry_uris() {
        let items = vec![LibItem { namespace: "ns".into(), catalog_item_id: "cid".into(), app_name: "Fig".into() }];
        let cat = HashMap::from([("cid".to_string(), CatalogInfo { title: "Death's Door".into(), is_game: true, cover_url: None })]);
        let e = &owned_entries(&items, &cat)[0];
        assert_eq!((e.platform, e.platform_id.as_str(), e.installed), (Platform::Epic, "Fig", false));
        assert_eq!(e.launch_uri, "com.epicgames.launcher://apps/ns%3Acid%3AFig?action=launch&silent=true");
        assert_eq!(e.install_uri.as_deref(), Some("com.epicgames.launcher://apps/ns%3Acid%3AFig?action=install"));
    }

    // ---- EpicClient flow ----
    use super::super::store::{load_cred, save_cred, MemoryStore};
    use super::super::StoredCred;
    use crate::db::Db;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, OnceLock, Weak};
    use std::time::Duration;

    struct FakeApi {
        tokens: Mutex<VecDeque<Result<String, AuthError>>>,
        library: Vec<String>,
        catalog: Result<String, AuthError>,
        kill: Result<(), AuthError>,
        token_calls: AtomicUsize,
        catalog_calls: AtomicUsize,
        seen_refresh: Mutex<Vec<String>>,
        on_library: Option<Box<dyn Fn() + Send + Sync>>,
        token_delay: Duration,
        /// When set: this namespace's catalog call fails; others answer with exactly the requested ids.
        catalog_fail_ns: Option<String>,
        catalog_ns: Mutex<Vec<String>>,
    }

    impl FakeApi {
        fn new(tokens: Vec<Result<String, AuthError>>) -> Self {
            FakeApi {
                tokens: Mutex::new(tokens.into()),
                library: vec![page(&[("ns", "game1", "Fig")], None)],
                catalog: Ok(cat(&["game1"])),
                kill: Ok(()),
                token_calls: AtomicUsize::new(0),
                catalog_calls: AtomicUsize::new(0),
                seen_refresh: Mutex::new(vec![]),
                on_library: None,
                token_delay: Duration::ZERO,
                catalog_fail_ns: None,
                catalog_ns: Mutex::new(vec![]),
            }
        }
    }

    impl EpicApi for FakeApi {
        fn token(&self, form: &[(&str, &str)]) -> Result<String, AuthError> {
            self.token_calls.fetch_add(1, Ordering::SeqCst);
            if let Some((_, rt)) = form.iter().find(|(k, _)| *k == "refresh_token") {
                self.seen_refresh.lock().unwrap().push(rt.to_string());
            }
            std::thread::sleep(self.token_delay);
            self.tokens.lock().unwrap().pop_front().unwrap_or(Err(AuthError::Network("bitti".into())))
        }
        fn library_page(&self, _access: &str, cursor: Option<&str>) -> Result<String, AuthError> {
            if let Some(h) = &self.on_library {
                h();
            }
            let n = cursor.map_or(0, |c| c.trim_start_matches("CUR").parse::<usize>().unwrap());
            Ok(self.library[n].clone())
        }
        fn catalog(&self, _access: &str, ns: &str, ids: &[String]) -> Result<String, AuthError> {
            assert!(ids.len() <= 50);
            self.catalog_calls.fetch_add(1, Ordering::SeqCst);
            self.catalog_ns.lock().unwrap().push(ns.to_string());
            if let Some(bad) = &self.catalog_fail_ns {
                if bad == ns {
                    return Err(AuthError::Network("durum 500".into()));
                }
                return Ok(cat(&ids.iter().map(String::as_str).collect::<Vec<_>>()));
            }
            self.catalog.clone()
        }
        fn kill(&self, _access: &str) -> Result<(), AuthError> {
            self.kill.clone()
        }
    }

    /// Lets a test keep a handle to the fake (counters) while the client owns a Box.
    struct ArcApi(Arc<FakeApi>);
    impl EpicApi for ArcApi {
        fn token(&self, f: &[(&str, &str)]) -> Result<String, AuthError> { self.0.token(f) }
        fn library_page(&self, a: &str, c: Option<&str>) -> Result<String, AuthError> { self.0.library_page(a, c) }
        fn catalog(&self, a: &str, n: &str, i: &[String]) -> Result<String, AuthError> { self.0.catalog(a, n, i) }
        fn kill(&self, a: &str) -> Result<(), AuthError> { self.0.kill(a) }
    }

    fn tok(at: &str, rt: &str) -> String {
        serde_json::json!({
            "access_token": at,
            "refresh_token": rt,
            "refresh_expires_at": "2027-11-12T00:00:00Z",
            "account_id": "acc",
            "displayName": "oyuncu",
        })
        .to_string()
    }
    fn page(items: &[(&str, &str, &str)], next: Option<&str>) -> String {
        let records: Vec<_> = items
            .iter()
            .map(|(ns, id, app)| serde_json::json!({"namespace": ns, "catalogItemId": id, "appName": app}))
            .collect();
        serde_json::json!({"records": records, "responseMetadata": {"nextCursor": next}}).to_string()
    }
    fn cat(ids: &[&str]) -> String {
        let m: serde_json::Map<String, serde_json::Value> = ids
            .iter()
            .map(|id| (id.to_string(), serde_json::json!({"title": format!("Oyun {id}"), "categories": [{"path": "games"}]})))
            .collect();
        serde_json::Value::Object(m).to_string()
    }
    fn stored(rt: &str) -> Arc<MemoryStore> {
        let s = Arc::new(MemoryStore::default());
        let c = StoredCred { v: 1, refresh_token: rt.into(), refresh_expires_at: "x".into(), account_id: "acc".into(), display_name: "oyuncu".into() };
        save_cred(&*s, &c).unwrap();
        s
    }
    fn client_arc(api: &Arc<FakeApi>, store: &Arc<MemoryStore>) -> EpicClient {
        let mut c = EpicClient::new(Box::new(ArcApi(api.clone())), Box::new(store.clone()));
        c.catalog_pause = Duration::ZERO;
        c
    }
    fn client(api: FakeApi, store: &Arc<MemoryStore>) -> EpicClient {
        client_arc(&Arc::new(api), store)
    }
    fn mem() -> Db {
        Db::open_in_memory().unwrap()
    }
    fn temp_db(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hangar-epic-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }
    const OFFLINE: &str = "Epic oyunları güncellenemedi; son bilinen liste gösteriliyor.";
    const EXPIRED: &str = "Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan.";
    const CORRUPT: &str = "Epic bağlantısı okunamadı; yeniden bağlan.";

    #[test] fn connect_saves_and_returns_name() {
        let store = Arc::new(MemoryStore::default());
        let db = mem();
        let c = client(FakeApi::new(vec![Ok(tok("AT1", "RT1"))]), &store);
        assert_eq!(c.connect(&db, "CODE"), Ok("oyuncu".to_string()));
        assert_eq!(load_cred(&*store).unwrap().unwrap().refresh_token, "RT1");
        let st = c.status(&db);
        assert_eq!(st.platform, Platform::Epic);
        assert_eq!(st.state, AccountState::Connected);
        assert_eq!(st.display_name.as_deref(), Some("oyuncu"));
    }

    #[test] fn connect_save_failure_is_not_a_network_error() {
        let store = Arc::new(MemoryStore::default());
        store.fail_save.store(true, Ordering::SeqCst);
        let db = mem();
        let c = client(FakeApi::new(vec![Ok(tok("AT1", "RT1"))]), &store);
        let e = c.connect(&db, "CODE").unwrap_err();
        assert_eq!(e, AuthError::Save);
        assert_eq!(e.to_string(), "Bağlantı kaydedilemedi");
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test] fn rotated_token_saved_before_use() {
        let store = stored("RT1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2"))]);
        let s2 = store.clone();
        api.on_library = Some(Box::new(move || {
            assert_eq!(load_cred(&*s2).unwrap().unwrap().refresh_token, "RT2");
        }));
        let api = Arc::new(api);
        let c = client_arc(&api, &store);
        assert_eq!(c.fetch_owned(&db).unwrap().len(), 1);
        assert_eq!(*api.seen_refresh.lock().unwrap(), vec!["RT1".to_string()]);
    }

    fn at(c: &EpicClient) -> Result<String, AuthError> {
        c.access_token().map(|t| t.to_string())
    }

    #[test] fn failed_save_keeps_rotated_cred_in_memory() {
        let store = stored("RT1");
        store.fail_save.store(true, Ordering::SeqCst);
        let db = mem();
        let api = Arc::new(FakeApi::new(vec![Ok(tok("AT2", "RT2")), Ok(tok("AT3", "RT3"))]));
        let c = client_arc(&api, &store);
        assert_eq!(at(&c), Ok("AT2".to_string()));
        let st = c.status(&db);
        assert_eq!((st.state, st.display_name.as_deref()), (AccountState::Connected, Some("oyuncu")));
        store.fail_save.store(false, Ordering::SeqCst);
        assert_eq!(at(&c), Ok("AT3".to_string()));
        assert_eq!(*api.seen_refresh.lock().unwrap(), vec!["RT1".to_string(), "RT2".to_string()]);
        assert_eq!(load_cred(&*store).unwrap().unwrap().refresh_token, "RT3");
    }

    #[test] fn unknown_version_is_not_connected_and_kept() {
        let store = Arc::new(MemoryStore::default());
        store.save(Platform::Epic, br#"{"v":2,"refresh_token":"r","refresh_expires_at":"x","account_id":"a","display_name":"d"}"#).unwrap();
        let db = mem();
        let c = client(FakeApi::new(vec![]), &store);
        assert_eq!(at(&c), Err(AuthError::NotConnected));
        assert_eq!(c.owned_for_scan(&db), (None, vec![]));
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
        assert!(store.raw(Platform::Epic).is_some());
    }

    #[test] fn disconnect_does_not_save() {
        let store = stored("RT1");
        let db = mem();
        let c = client(FakeApi::new(vec![Ok(tok("AT2", "RT2"))]), &store);
        let before = store.saves();
        assert!(c.disconnect(&db));
        assert_eq!(store.saves(), before);
        assert_eq!(store.raw(Platform::Epic), None);
    }

    #[test] fn refresh_is_serialized() {
        let store = stored("RT1");
        let mut api = FakeApi::new(vec![Ok(tok("A2", "RT2")), Ok(tok("A3", "RT3"))]);
        api.token_delay = Duration::from_millis(50);
        let api = Arc::new(api);
        let c = Arc::new(client_arc(&api, &store));
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let c = c.clone();
                std::thread::spawn(move || at(&c).unwrap())
            })
            .collect();
        let mut ats: Vec<String> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        ats.sort();
        assert_eq!(ats, vec!["A2".to_string(), "A3".to_string()]);
        let mut seen = api.seen_refresh.lock().unwrap().clone();
        seen.sort();
        assert_eq!(seen, vec!["RT1".to_string(), "RT2".to_string()]);
        assert_eq!(api.token_calls.load(Ordering::SeqCst), 2);
    }

    #[test] fn offline_uses_cache_with_warning() {
        let store = stored("RT1");
        let db = mem();
        let c = client(FakeApi::new(vec![Ok(tok("AT2", "RT2")), Err(AuthError::Network("x".into()))]), &store);
        let (first, w) = c.owned_for_scan(&db);
        assert!(w.is_empty());
        let first = first.unwrap().unwrap();
        assert_eq!(first.len(), 1);
        let (second, w2) = c.owned_for_scan(&db);
        assert_eq!(second.unwrap().unwrap(), first);
        assert_eq!(w2, vec![OFFLINE.to_string()]);
    }

    #[test] fn invalid_grant_marks_expired() {
        let store = stored("RT1");
        let db = mem();
        let c = client(FakeApi::new(vec![Err(AuthError::InvalidGrant)]), &store);
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert_eq!(w, vec![EXPIRED.to_string()]);
        assert_eq!(c.status(&db).state, AccountState::Expired);
        assert!(store.raw(Platform::Epic).is_some());
    }

    #[test] fn corrupt_store_is_disconnected() {
        let store = stored("RT1");
        store.corrupt.store(true, Ordering::SeqCst);
        let db = mem();
        let c = client(FakeApi::new(vec![]), &store);
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert_eq!(w, vec![CORRUPT.to_string()]);
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test] fn disconnect_offline_still_deletes() {
        let store = stored("RT1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2"))]);
        api.kill = Err(AuthError::Network("x".into()));
        let c = client(api, &store);
        db.owned_cache_put(Platform::Epic, &[], 1).unwrap();
        db.set_account_expired(Platform::Epic, true).unwrap();
        let cached = HashMap::from([("game1".to_string(), CatalogInfo { title: "Oyun".into(), is_game: true, cover_url: None })]);
        db.catalog_put(&cached).unwrap();
        assert!(!c.disconnect(&db));
        assert_eq!(store.raw(Platform::Epic), None);
        assert_eq!(db.owned_cache_get(Platform::Epic).unwrap(), None);
        assert!(db.catalog_get(&["game1".to_string()]).unwrap().is_empty());
        assert!(!db.account_expired(Platform::Epic).unwrap());
        assert_eq!(c.status(&db).state, AccountState::Disconnected);
    }

    #[test] fn disconnect_wins_over_inflight_scan() {
        let path = temp_db("race");
        let db = Db::open(&path).unwrap();
        db.owned_cache_put(Platform::Epic, &[], 1).unwrap();
        db.set_account_expired(Platform::Epic, true).unwrap();
        let store = stored("RT1");
        let slot: Arc<OnceLock<Weak<EpicClient>>> = Arc::new(OnceLock::new());
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2")), Ok(tok("AT3", "RT3"))]);
        let (s2, p2) = (slot.clone(), path.clone());
        api.on_library = Some(Box::new(move || {
            let c = s2.get().and_then(Weak::upgrade).unwrap();
            let db2 = Db::open(&p2).unwrap();
            assert!(c.disconnect(&db2));
        }));
        let c = Arc::new(client(api, &store));
        slot.set(Arc::downgrade(&c)).unwrap();
        let (r, w) = c.owned_for_scan(&db);
        assert!(r.is_none());
        assert!(w.is_empty());
        assert_eq!(db.owned_cache_get(Platform::Epic).unwrap(), None);
        assert!(!db.account_expired(Platform::Epic).unwrap());
        assert_eq!(store.raw(Platform::Epic), None);
        assert!(db.catalog_get(&["game1".to_string()]).unwrap().is_empty()); // in-flight catalog not written back
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test] fn catalog_cached_between_scans() {
        let store = stored("RT1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2")), Ok(tok("AT3", "RT3"))]);
        api.library = vec![
            page(&[("ns", "game1", "Fig")], Some("CUR1")),
            page(&[("ns", "game2", "Fig2")], None),
        ];
        api.catalog = Ok(cat(&["game1", "game2"]));
        let api = Arc::new(api);
        let c = client_arc(&api, &store);
        assert_eq!(c.fetch_owned(&db).unwrap().len(), 2);
        assert_eq!(c.fetch_owned(&db).unwrap().len(), 2);
        assert_eq!(api.catalog_calls.load(Ordering::SeqCst), 1);
    }

    #[test] fn catalog_batched_per_namespace_in_chunks_of_50() {
        let store = stored("RT1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2"))]);
        let ids: Vec<String> = (0..120).map(|i| format!("g{i}")).collect();
        let mut items: Vec<(&str, &str, &str)> = ids.iter().map(|i| ("ns", i.as_str(), i.as_str())).collect();
        items.push(("ns2", "solo", "Solo"));
        api.library = vec![page(&items, None)];
        let mut all: Vec<&str> = ids.iter().map(String::as_str).collect();
        all.push("solo");
        api.catalog = Ok(cat(&all));
        let api = Arc::new(api);
        let c = client_arc(&api, &store);
        assert_eq!(c.fetch_owned(&db).unwrap().len(), 121);
        assert_eq!(api.catalog_calls.load(Ordering::SeqCst), 4); // ns: 50+50+20, ns2: 1
    }

    #[test] fn failed_catalog_chunk_keeps_the_rest() {
        let store = stored("RT1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2")), Ok(tok("AT3", "RT3"))]);
        api.library = vec![page(&[("a", "game1", "Fig"), ("b", "game2", "Fig2"), ("c", "game3", "Fig3")], None)];
        api.catalog_fail_ns = Some("b".into());
        let api = Arc::new(api);
        let c = client_arc(&api, &store);
        let names = |v: Vec<PlatformEntry>| v.into_iter().map(|e| e.platform_id).collect::<Vec<_>>();
        assert_eq!(names(c.fetch_owned(&db).unwrap()), vec!["Fig", "Fig3"]);
        let ids: Vec<String> = ["game1", "game2", "game3"].map(String::from).to_vec();
        let mut cached: Vec<String> = db.catalog_get(&ids).unwrap().into_keys().collect();
        cached.sort();
        assert_eq!(cached, vec!["game1", "game3"]);
        api.catalog_ns.lock().unwrap().clear();
        assert_eq!(names(c.fetch_owned(&db).unwrap()), vec!["Fig", "Fig3"]);
        assert_eq!(*api.catalog_ns.lock().unwrap(), vec!["b".to_string()]);
    }

    #[test] fn all_catalog_chunks_failing_is_an_error() {
        let store = stored("RT1");
        let db = mem();
        let mut api = FakeApi::new(vec![Ok(tok("AT2", "RT2"))]);
        api.catalog_fail_ns = Some("ns".into());
        let c = client(api, &store);
        assert_eq!(c.fetch_owned(&db), Err(AuthError::Network("durum 500".into())));
    }

    #[test] fn no_secret_leaks() {
        let path = temp_db("leak");
        let db = Db::open(&path).unwrap();
        let store = Arc::new(MemoryStore::default());
        let c = client(
            FakeApi::new(vec![
                Ok(tok("AT-SECRET", "RT-SECRET")),
                Ok(tok("AT-SECRET", "RT-SECRET")),
                Ok(tok("AT-SECRET", "RT-SECRET")),
                Err(AuthError::InvalidGrant),
                Err(AuthError::Network("x".into())),
            ]),
            &store,
        );
        let mut text = Vec::new();
        text.push(c.connect(&db, "CODE-SECRET").unwrap());
        text.push(format!("{:?}", c.fetch_owned(&db).unwrap()));
        let mut errors = Vec::new();
        for _ in 0..3 {
            let (r, w) = c.owned_for_scan(&db);
            text.push(format!("{r:?}"));
            text.extend(w);
        }
        // Each error path's AuthError, displayed and debugged.
        errors.push(c.connect(&db, "CODE-SECRET").unwrap_err()); // token queue empty → Network
        store.fail_save.store(true, Ordering::SeqCst);
        errors.push(client(FakeApi::new(vec![Ok(tok("AT-SECRET", "RT-SECRET"))]), &store).connect(&db, "CODE-SECRET").unwrap_err());
        let pend = client(FakeApi::new(vec![Ok(tok("AT-SECRET", "RT-SECRET")), Err(AuthError::Network("x".into()))]), &store);
        assert!(pend.access_token().is_ok()); // rotated cred now pending in memory
        text.push(format!("{:?}", pend.status(&db)));
        errors.push(pend.access_token().unwrap_err());
        store.fail_save.store(false, Ordering::SeqCst);
        errors.push(client(FakeApi::new(vec![Err(AuthError::InvalidGrant)]), &store).access_token().unwrap_err());
        errors.push(client(FakeApi::new(vec![Ok("{\"access_token\":\"AT-SECRET\"}".into())]), &store).access_token().unwrap_err());
        for e in &errors {
            text.push(format!("{e} {e:?}"));
        }
        text.push(format!("{:?}", c.status(&db)));
        drop(db);
        let conn = rusqlite::Connection::open(&path).unwrap();
        for t in ["owned_cache", "settings", "epic_catalog"] {
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
        assert!(all.contains("oyuncu") && all.contains("Fig"), "sanity: real output was collected");
        for bad in ["AT-SECRET", "RT-SECRET", "CODE-SECRET"] {
            assert!(!all.contains(bad), "leak: {bad}");
        }
    }
}
