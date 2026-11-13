# Epic Hesap Bağlantısı — Uygulama Planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Epic hesabındaki tüm oyunlar (kurulu olmayanlar dahil), Epic'in kendi giriş sayfasıyla uygulama içinden bağlanarak Hangar kütüphanesine gelsin; belge yalnızca bu PC'de, DPAPI ile şifreli dursun.

**Architecture:** `hangar-core`'a `accounts` modülü eklenir: Epic HTTP istemcisi (trait arkasında), saf giriş mantığı (gezinme kararı, başlatma betiği), ayrıştırma, belge deposu (DPAPI) ve `EpicClient` akışı. `scan_all` platform başına sahip olunan listeleri alır. `src-tauri` yalnızca ince yapıştırıcıdır: giriş penceresi (`login.rs`), hesap komutları (`accounts.rs`). React tarafında Ayarlar'a "Hesaplar" bölümü gelir.

**Tech Stack:** Rust (ureq 2, rusqlite, serde_json, url 2, zeroize 1, windows-sys 0.61), Tauri 2.12.5, React 18 + TS + vitest, `@tauri-apps/plugin-dialog` (`ask`).

**Spec:** `docs/superpowers/specs/2026-11-12-hangar-accounts-design.md`

Bu plan iki konsey turundan geçti (tasarım ve plan).

## Global Constraints

- **Belge sızmaz:** refresh token, access token, yetki kodu ve id_token hiçbir yere yazılmaz. Buna log, `println!`/`eprintln!`/`dbg!`, hata mesajı, uyarı, SQLite, `settings`, fixture, commit mesajı ve test çıktısı dahildir.
  - Hata metinleri en fazla HTTP durum kodu ile kısa bir açıklama içerir; yanıt gövdesi ya da URL içermez.
  - `ureq::Error`'u asla `Display`/`Debug` ile yazdırma. Steam'deki `net_err` deseni izlenir; kill URL'i access token içerir.
- **Epic istemcisi:** id `34a02cf8f4414e29b15921876da36f9a`, secret `daafbccc737745039dffe53d94fc76cf`, `token_type=eg1`. Bu değerler herkese açıktır ve kodda sabit durur.
- **Uç noktalar:**
  - Token: `https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token`
  - Kill: `DELETE https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/<access_token>`
  - Kütüphane: `https://library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true[&cursor=…]`
  - Katalog: `https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/<ns>/bulk/items?id=<id>[,<id>…]&includeDLCDetails=true&includeMainGameDetails=true&country=US&locale=en-US`
- **Giriş adresi:** `https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode`
- **Yakalama adresi:** `https://hangar.invalid/callback?code=<kod>`
- **Belge dosyası:** `<app_data_dir>\accounts\epic.dat`
  - DPAPI CurrentUser, entropi `b"hangar-accounts-v1"`.
  - Şifrelemeden önceki JSON'da `"v": 1` bulunur.
  - Yazma geçici dosya + rename ile yapılır.
- **Zamanlama:** katalog çağrıları arasında en az 100 ms; ağ zaman aşımı 15 s; giriş zaman aşımı 5 dk.
- **Bağımlılıklar:** `hangar-core` Tauri'ye bağımlı olmaz. Windows'a özgü kod `#[cfg(windows)]` altında, Linux için derlenen bir karşılığı vardır.
- **Kapsam:** Ubisoft bu planda yok. Arayüz satırı, README metni ya da depo dosyası eklenmez.
- **Fixture'lar:** depoya yalnızca **elle yazılmış sentetik** fixture'lar girer. Canlı E0 yanıtları yalnızca kullanıcının PC'sinde, `<app_data_dir>\e0\` altında kalır; commit edilmez.
- **Dil:** arayüz metinleri Türkçe. Commit mesajları Türkçe conventional (`feat(core): …`) ve şu satırla biter: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Ortam:** komutlar Windows Git Bash'te, repo kökünde çalışır. Node PATH'te yok; önce şunu çalıştır:
  `export PATH="/c/Users/user/AppData/Roaming/fnm/node-versions/v26.13.0/installation:$PATH"`
- **Testler:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`.
  - `src-tauri`'ye test yazılmaz; Tauri test ikilileri Windows'ta kırılgan. Test edilebilir mantık `hangar-core`'da durur.
- **Commit öncesi sızıntı kontrolü:** her commit öncesi şu komut boş çıkmalı:
  `git diff --cached -- . ':!Cargo.lock' | grep -nEi '"(access_token|refresh_token|id_token)"\s*:\s*"[^"R]|eg1~|[0-9a-f]{32}' | grep -v 34a02cf8f4414e29b15921876da36f9a | grep -v daafbccc737745039dffe53d94fc76cf`
  Bir sonuç çıkarsa ve bunun bir sentetik test kimliği olduğundan eminsen, açıklamasını commit mesajına yaz.

## Review Focus

1. **Kaldırılmış oyun:** katalog yanıtında hiç bulunmayan bir kimlik sessizce atlanır, tarama düşmez. Test: T4 `catalog_missing_item_skipped`.
2. **Eşzamanlı yenileme ya da çökme:**
   - Yeni `refresh_token` kullanılmadan önce kaydedilir.
   - Kayıt başarısız olursa eski belge silinmez.
   - İkinci yenileme eski belgeyi kullanmaz.
   - Test: T5c `rotated_token_saved_before_use`, `failed_save_keeps_old_cred`, `refresh_is_serialized`.
3. **Kesme sırasında tarama:** sürmekte olan bir tarama, "Bağlantıyı kes"ten sonra önbelleği ya da "süresi doldu" durumunu geri yazamaz. Test: T5c `disconnect_wins_over_inflight_scan`.
4. **Çevrimdışı tarama ve kesme:**
   - Taramada son bilinen liste gösterilir ve uyarı çıkar.
   - Kesme yine dosyayı siler ve Epic'teki oturumun kapatılamadığını söyler.
   - Test: T5c `offline_uses_cache_with_warning`, `disconnect_offline_still_deletes`.
5. **Farklı adlar:** kurulu Epic oyununun manifest adı katalog adından farklıysa tek kart kalır, manifest adı korunur, `install_uri` güncellenir. Test: T1 `owned_epic_keeps_manifest_title`.

## Sıra ve paralellik

| Sıra | Görev | Bağlı olduğu | Paralel çalışabilir |
|---|---|---|---|
| 1 | T1 `scan_all` | — | T2, T3a |
| 2 | T2 Epic istemcisi + tipler + temizleyici | — | T1, T3a |
| 3 | T3a Saf giriş mantığı (core) | — | T1, T2 |
| 4 | T3b Giriş penceresi + E0 probe → **E0 (kullanıcı)** | T2, T3a | T4 |
| 5 | T4 Ayrıştırma (sentetik fixture) | T2 | T3b / E0 |
| 6 | T4b Canlı yanıtlarla doğrulama (yerel) | T4, E0 | — |
| 7 | T5a `SecretStore` + DPAPI | T2 | T5b |
| 8 | T5b DB tabloları | T4 | T5a |
| 9 | T5c `EpicClient` akışı — **denetçi inceleme kapısı** | T4, T5a, T5b | — |
| 10 | T6 Tauri komutları + tarama bağlantısı | T1, T3b, T5c | T7 |
| 11 | T7 Hesaplar arayüzü | T5c'nin `AccountStatus` şekli | T6 |
| 12 | T8 README, elle kontrol, release duman testi | hepsi | — |

**E0 başarısız olursa:** T6 ve T7 durur, T3b'nin pencere kısmı atılır. Kullanıcıya yaklaşım B (kodu elle yapıştırma) sorulur. T1, T2, T4 ve T5 her iki yolda da geçerlidir.

---

### Task 1: `scan_all` sahip olunan listeleri platform başına alsın

**Files:**
- Modify: `crates/hangar-core/src/library.rs` (`scan_all` ve testleri)
- Modify: `src-tauri/src/commands.rs` (`scan` içindeki `scan_all` çağrısı)

**Interfaces:**
- Produces:
  - `pub type OwnedLists = Vec<(Platform, Result<Vec<PlatformEntry>, ProviderError>)>;`
  - `pub fn scan_all(db: &Db, providers: &[Box<dyn LibraryProvider>], owned: OwnedLists) -> rusqlite::Result<ScanReport>`
- **Birleşme kuralı:** aynı `platform` ve `platform_id` ile kurulu bir kayıt varsa kurulu kayıt kalır, ama şu alanlar güncellenir:
  - `playtime_min`: sahip olunanda Some ise oradan alınır.
  - `last_played`: `installed.last_played.or(owned.last_played)`.
  - `install_uri`: sahip olunanda Some ise onunla değiştirilir.
  - `title` asla değişmez.
  - Eşleşme yoksa sahip olunan kayıt eklenir.
- **`Err` uyarıları:**
  - Steam için mevcut metin aynen kalır: `"Steam sahip olunan oyunlar alınamadı: API anahtarını kontrol et."`.
  - Diğer platformlar için `format!("{:?} sahip olunan oyunlar alınamadı.", platform)`.

- [ ] **Step 1: Mevcut testleri yeni imzaya çevir ve yeni testleri ekle.** Dönüşüm kuralları: `scan_all(.., None)` → `vec![]`, `Some(x)` → `vec![(Platform::Steam, x)]`. Eklenecek testler:

```rust
#[test]
fn owned_epic_keeps_manifest_title() {
    let db = Db::open_in_memory().unwrap();
    let installed = entry(Platform::Epic, "Fig", "Death's Door", true);
    let mut owned = entry(Platform::Epic, "Fig", "Deaths Door", false);
    owned.install_uri = Some("com.epicgames.launcher://apps/ns%3Aid%3AFig?action=install".into());
    let other = entry(Platform::Epic, "Owl", "Owlboy", false);
    let r = scan_all(&db, &[fake(Platform::Epic, vec![installed])],
        vec![(Platform::Epic, Ok(vec![owned, other]))]).unwrap();
    assert_eq!(r.games, 2);
    let games = db.games().unwrap();
    let g = games.iter().find(|g| g.title == "Death's Door").unwrap();
    assert_eq!(g.entries.len(), 1);
    assert!(g.entries[0].installed);
    assert_eq!(g.entries[0].install_uri.as_deref(), Some("com.epicgames.launcher://apps/ns%3Aid%3AFig?action=install"));
    assert!(games.iter().any(|g| g.title == "Owlboy" && !g.entries[0].installed));
}

#[test]
fn owned_ids_do_not_cross_platforms() {
    let db = Db::open_in_memory().unwrap();
    scan_all(&db, &[fake(Platform::Steam, vec![entry(Platform::Steam, "1", "Hades", true)])],
        vec![(Platform::Epic, Ok(vec![entry(Platform::Epic, "1", "Hades", false)]))]).unwrap();
    let g = &db.games().unwrap()[0];
    assert_eq!(g.entries.len(), 2);
    assert!(g.entries.iter().any(|e| e.platform == Platform::Steam && e.installed));
    assert!(g.entries.iter().any(|e| e.platform == Platform::Epic && !e.installed));
}

#[test]
fn non_steam_owned_error_warning() {
    let db = Db::open_in_memory().unwrap();
    let r = scan_all(&db, &[], vec![(Platform::Epic, Err(ProviderError::Network("x".into())))]).unwrap();
    assert_eq!(r.warnings, vec!["Epic sahip olunan oyunlar alınamadı.".to_string()]);
}
```

- [ ] **Step 2:** `cargo test -p hangar-core library` → derleme hatası ya da FAIL.
- [ ] **Step 3:** `scan_all`'u kurala göre yaz. `commands.rs::scan` içinde Steam sonucunu şöyle geçir: `let owned: OwnedLists = owned.into_iter().map(|r| (Platform::Steam, r)).collect();`.
- [ ] **Step 4:** `cargo test -p hangar-core` ve `cargo check -p hangar` → PASS.
- [ ] **Step 5:** Sızıntı kontrolü yap, sonra commit et: `refactor(core): scan_all sahip olunan listeleri platform başına alır`.

---

### Task 2: Epic HTTP istemcisi, belge tipleri ve temizleyici

**Files:**
- Create: `crates/hangar-core/src/accounts/mod.rs`, `crates/hangar-core/src/accounts/epic_api.rs`, `crates/hangar-core/src/accounts/sanitize.rs`
- Modify: `crates/hangar-core/src/lib.rs` (`pub mod accounts;`)
- Modify: `crates/hangar-core/Cargo.toml`: `zeroize = "1"`; `[dev-dependencies] base64 = "0.22"`

**Interfaces:**
- `accounts/mod.rs`:
  - `#[derive(Debug, Clone, PartialEq)] pub enum AuthError { NotConnected, Corrupt, InvalidGrant, Network(String), Parse(String) }` ve `impl Display`. Display metinleri:
    - `NotConnected`: "Epic bağlı değil"
    - `Corrupt`: "Epic bağlantısı okunamadı"
    - `InvalidGrant`: "Epic bağlantısının süresi doldu"
    - `Network(s)`: `format!("Epic'e ulaşılamadı ({s})")`
    - `Parse(s)`: `format!("Epic yanıtı anlaşılamadı ({s})")`
  - `#[derive(Serialize, Deserialize, Clone, PartialEq)] pub struct StoredCred { pub v: u32, pub refresh_token: String, pub refresh_expires_at: String, pub account_id: String, pub display_name: String }`
    - Elle yazılmış `Debug`: `refresh_token` ve `account_id` alanları `"***"` gösterir.
    - `impl Drop`: `refresh_token`'ı `zeroize` ile sıfırlar.
  - `pub struct Tokens { pub access_token: String, pub cred: StoredCred }`: elle `Debug` (`***`) ve `Drop`'ta `access_token` için `zeroize`.
  - `pub fn parse_token(json: &str) -> Result<Tokens, AuthError>`
    - Okunan alanlar: `access_token`, `refresh_token`, `refresh_expires_at`, `account_id`, `displayName`.
    - `v` her zaman 1'dir.
    - Eksik alan varsa `Err(Parse("token"))` döner; mesaja gövde girmez.
- `accounts/epic_api.rs`:
  - `pub trait EpicApi: Send + Sync { fn token(&self, form: &[(&str, &str)]) -> Result<String, AuthError>; fn library_page(&self, access: &str, cursor: Option<&str>) -> Result<String, AuthError>; fn catalog(&self, access: &str, namespace: &str, ids: &[String]) -> Result<String, AuthError>; fn kill(&self, access: &str) -> Result<(), AuthError>; }`
    - Yanıt gövdeleri ham dize olarak döner.
  - `pub struct HttpEpicApi;`, ureq kullanır.
    - Token ucunda HTTP 400/401 → `AuthError::InvalidGrant`.
    - Diğer HTTP durumları → `Network(format!("durum {code}"))`.
    - Taşıma hatası → `Network(format!("{:?}", t.kind()))`.
    - Zaman aşımı 15 s.
  - `pub const BASIC_AUTH: &str = "MzRhMDJjZjhmNDQxNGUyOWIxNTkyMTg3NmRhMzZmOWE6ZGFhZmJjY2M3Mzc3NDUwMzlkZmZlNTNkOTRmYzc2Y2Y=";` (başlık `Authorization: basic <BASIC_AUTH>`)
  - `pub const LOGIN_URL: &str`: Global Constraints'teki giriş adresi.
- `accounts/sanitize.rs`: `pub fn sanitize(body: &str, secrets: &[&str]) -> Result<String, String>`
  - Gövde JSON ise şu anahtarların değerleri `"REDACTED"` yapılır: `access_token`, `refresh_token`, `id_token`, `account_id`, `accountId`, `displayName`, `email`, `ownerId`, `authorizationCode`, `sid`. İç içe nesneler de dolaşılır.
  - Sonra `secrets` listesindeki her **boş olmayan** değer metinde `"REDACTED"` ile değiştirilir.
  - Sonuçta boş olmayan bir secret hâlâ geçiyorsa `Err("sızıntı")` döner.

- [ ] **Step 1: Testleri yaz.**

```rust
// accounts/mod.rs
#[test]
fn parse_token_reads_fields() {
    let j = r#"{"access_token":"AT","expires_at":"2026-11-12T20:00:00Z","refresh_token":"RT","refresh_expires_at":"2026-12-12T20:00:00Z","account_id":"acc1","displayName":"oyuncu"}"#;
    let t = parse_token(j).unwrap();
    assert_eq!(t.access_token, "AT");
    assert_eq!(t.cred.refresh_token, "RT");
    assert_eq!(t.cred.display_name, "oyuncu");
    assert_eq!(t.cred.v, 1);
    assert_eq!(parse_token("{}"), Err(AuthError::Parse("token".into())).map(|_: Tokens| unreachable!()));
}
#[test]
fn debug_and_errors_never_show_tokens() {
    let t = parse_token(r#"{"access_token":"AT-SECRET","refresh_token":"RT-SECRET","refresh_expires_at":"x","account_id":"ACC-SECRET","displayName":"d"}"#).unwrap();
    let s = format!("{t:?} {:?}", t.cred);
    for bad in ["AT-SECRET", "RT-SECRET", "ACC-SECRET"] { assert!(!s.contains(bad)); }
    assert!(!AuthError::Network("durum 500".into()).to_string().contains("http"));
}
#[test]
fn basic_auth_matches_client() {
    use base64::Engine;
    assert_eq!(base64::engine::general_purpose::STANDARD.encode("34a02cf8f4414e29b15921876da36f9a:daafbccc737745039dffe53d94fc76cf"), epic_api::BASIC_AUTH);
}
// accounts/sanitize.rs
#[test]
fn redacts_keys_nested_and_secrets() {
    let out = sanitize(r#"{"account_id":"acc1","displayName":"Samet","records":[{"appName":"Fig","note":"acc1","x":{"authorizationCode":"C0DE"}}]}"#, &["acc1", "Samet"]).unwrap();
    for bad in ["acc1", "Samet", "C0DE"] { assert!(!out.contains(bad)); }
    assert!(out.contains("Fig"));
}
#[test]
fn non_json_and_empty_secret() {
    assert_eq!(sanitize("token=RT-SECRET", &["RT-SECRET"]).unwrap(), "token=REDACTED");
    assert_eq!(sanitize("abc", &[""]).unwrap(), "abc");
}
```

- [ ] **Step 2:** `cargo test -p hangar-core accounts` → FAIL.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS.
- [ ] **Step 5:** Sızıntı kontrolü, ardından commit: `feat(core): epic http istemcisi, belge tipleri ve temizleyici`.

---

### Task 3a: Saf giriş mantığı (core)

**Files:**
- Create: `crates/hangar-core/src/accounts/login.rs` (modülü `accounts/mod.rs`'te `pub mod login;` ile aç)
- Modify: `crates/hangar-core/Cargo.toml`: `url = "2"`

**Interfaces:**
- `pub enum NavDecision { Callback(String), Allow, Block }`
- `pub fn decide(url: &url::Url) -> NavDecision`
  - Şu koşulların hepsi tutarsa `Callback(code)` döner: şema `https`, host `hangar.invalid`, yol `/callback`, `code` parametresi boş değil. Kod yüzde çözülmüş haliyle döner.
  - Yalnızca host `hangar.invalid` ise ve diğer koşullar tutmuyorsa `Block`.
  - Şema `https` ise ve host `ALLOWED_HOSTS`'taki bir değere eşit ya da onun alt alanıysa (`host == h || host.ends_with(&format!(".{h}"))`) `Allow`.
  - Diğer her durumda `Block`.
- `pub const ALLOWED_HOSTS: &[&str] = &["epicgames.com", "epicgames.dev", "unrealengine.com", "hcaptcha.com", "arkoselabs.com", "funcaptcha.com", "google.com", "apple.com", "live.com", "microsoftonline.com", "xbox.com", "steampowered.com", "steamcommunity.com", "playstation.com", "sonyentertainmentnetwork.com", "nintendo.com", "facebook.com"];`
  - Gerekçe: Epic, bu sağlayıcılarla girişi kendi sayfasından yönlendirir. Liste yalnızca üst çerçeve gezinmelerini sınırlar; sayfa içi kaynaklar ve iframe'ler bundan etkilenmez.
- `pub const INIT_SCRIPT: &str`: aşağıdaki davranış, tümüyle `try/catch` içinde:

```js
(function(){try{
  if (window.top !== window) return;
  if (location.origin !== 'https://www.epicgames.com' || location.pathname !== '/id/api/redirect') return;
  var run = function(){ try {
    var j = JSON.parse(document.body.innerText);
    if (j && j.authorizationCode) location.replace('https://hangar.invalid/callback?code=' + encodeURIComponent(j.authorizationCode));
  } catch(e){} };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run); else run();
}catch(e){}})();
```

- `pub fn window_title(url: &url::Url) -> String`: `format!("Epic Games girişi — {}", url.host_str().unwrap_or(""))`.
- `pub fn open_error(uri: &str, err: &str) -> String`
  - `uri` `com.epicgames.launcher://` ile başlıyorsa `"Epic Games Launcher açılamadı — kurulu mu?"`.
  - Aksi halde `format!("Açılamadı: {err}")`.

- [ ] **Step 1: Testleri yaz.**

```rust
fn u(s: &str) -> url::Url { url::Url::parse(s).unwrap() }
#[test] fn callback_extracts_code() { assert!(matches!(decide(&u("https://hangar.invalid/callback?code=abc%2B1")), NavDecision::Callback(c) if c == "abc+1")); }
#[test] fn bad_callbacks_block() {
    for s in ["https://hangar.invalid/callback?code=", "https://hangar.invalid/other?code=x", "http://hangar.invalid/callback?code=x"] {
        assert!(matches!(decide(&u(s)), NavDecision::Block), "{s}");
    }
}
#[test] fn allowlist() {
    for ok in ["https://www.epicgames.com/id/login", "https://accounts.google.com/o", "https://newassets.hcaptcha.com/x", "https://epicgames.com/"] {
        assert!(matches!(decide(&u(ok)), NavDecision::Allow), "{ok}");
    }
    for bad in ["http://www.epicgames.com/", "https://evil-epicgames.com/", "https://example.com/", "https://epicgames.com.evil.io/"] {
        assert!(matches!(decide(&u(bad)), NavDecision::Block), "{bad}");
    }
}
#[test] fn init_script_is_scoped() {
    for s in ["window.top", "https://www.epicgames.com", "/id/api/redirect", "DOMContentLoaded", "https://hangar.invalid/callback?code="] { assert!(INIT_SCRIPT.contains(s), "{s}"); }
}
#[test] fn title_shows_host() { assert_eq!(window_title(&u("https://www.epicgames.com/id/login")), "Epic Games girişi — www.epicgames.com"); }
#[test] fn open_error_texts() {
    assert_eq!(open_error("com.epicgames.launcher://apps/x?action=install", "e"), "Epic Games Launcher açılamadı — kurulu mu?");
    assert_eq!(open_error("steam://run/1", "x"), "Açılamadı: x");
}
```

- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS.
- [ ] **Step 5:** Commit: `feat(core): epic giriş penceresi için saf gezinme kuralları`.

---

### Task 3b: Giriş penceresi ve E0 probe (ince yapıştırıcı) → E0 kontrol noktası

**Files:**
- Create: `src-tauri/src/login.rs`
- Modify: `src-tauri/src/main.rs`, `src/lib/api.ts` (yalnızca DEV satırı)

**Interfaces:**
- Consumes:
  - T3a: `accounts::login::{decide, NavDecision, INIT_SCRIPT, window_title}`
  - T2: `accounts::epic_api::{LOGIN_URL, HttpEpicApi, EpicApi}`, `accounts::parse_token`, `accounts::sanitize::sanitize`
- Produces:
  - `pub enum LoginError { Cancelled, Timeout, AlreadyOpen, Window(String) }`
  - `pub async fn login(app: &AppHandle) -> Result<String, LoginError>`: başarıda yetki kodunu döner.
- **`login` davranışı:**
  1. `account-login` etiketli pencere varsa `set_focus` yapar ve `Err(AlreadyOpen)` döner.
  2. `let (tx, rx) = std::sync::mpsc::channel::<Result<String, LoginError>>();`
  3. Pencere: `WebviewWindowBuilder::new(app, "account-login", WebviewUrl::External(LOGIN_URL.parse().unwrap()))`, şu ayarlarla:
     - `.title(window_title(..))`, `.incognito(true)`, `.initialization_script(INIT_SCRIPT)`, `.inner_size(520.0, 760.0)`.
     - `.on_navigation(move |url| …)`: `decide(url)` sonucuna göre:
       - `Callback(c)` → `tx.send(Ok(c))` ve `false` döner.
       - `Allow` → pencere başlığını `window_title(url)` yapar (`app.get_webview_window("account-login")` ile; bu `async_runtime::spawn` içinde yapılır), `true` döner.
       - `Block` → `false` döner.
       - Kapanış içinde hiçbir şey loglanmaz.
     - `.on_new_window(...)` → `NewWindowResponse::Deny`. URL izinliyse `async_runtime::spawn` ile aynı pencerede `navigate(url)` yapılır.
     - `.build()` hatası `Err(Window(..))` döner. Hata metni E0'da incognito/kullanıcı veri klasörü çakışmasını (0x8007139F) gösterir.
  4. `window.on_window_event`: `WindowEvent::Destroyed` → `tx2.send(Err(Cancelled))`. İlk gelen mesaj kazanır.
  5. `spawn_blocking(move || rx.recv_timeout(Duration::from_secs(300)))`. Zaman aşımında `Err(Timeout)`.
  6. Sonuçtan sonra pencere (hâlâ varsa) `close()` ile kapatılır.
- **`debug_epic_probe`:** `#[cfg(debug_assertions)] #[tauri::command] pub async fn debug_epic_probe(app: AppHandle) -> Result<String, String>`
  1. `login` çalıştırılır.
  2. `token(authorization_code)` alınır.
  3. Bir `KillOnDrop` koruyucusu kurulur; hata olsa bile `api.kill(access)` çağrılır.
  4. Kütüphane sayfaları `serde_json::Value` ile dolaşılır (`responseMetadata.nextCursor`). Bu görevde T4'e bağımlılık yok.
  5. Gelen kayıtlardan ilk 3 namespace için katalog istenir.
  6. Her gövde `sanitize(body, &[code, access, refresh, account_id, display_name])` ile temizlenir. Token yanıtı da temizlenerek yazılır.
  7. Dosyalar `<app_data_dir>/e0/` altına yazılır: `live-token.json`, `live-library-<n>.json`, `live-catalog-<n>.json`. **Repoya yazılmaz.**
  8. Dönüş metni: yazılan dosya sayısı ve klasör yolu.
  9. Belge hiçbir dosyaya yazılmaz.
- **`main.rs`:**
  - `mod login;`
  - `debug_epic_probe` iki ayrı `invoke_handler` dalıyla kaydedilir: `#[cfg(debug_assertions)]` dalında komut listede, `#[cfg(not(debug_assertions))]` dalında yok.
- **`api.ts` sonu:** `if (import.meta.env.DEV) (window as unknown as { hangarProbe: () => Promise<string> }).hangarProbe = () => invoke<string>("debug_epic_probe");`

- [ ] **Step 1:** Uygula. Test yok; mantık T3a'da test edildi.
- [ ] **Step 2:** `cargo check -p hangar` ve `cargo build -p hangar --release` → PASS. Release ikilisinde `debug_epic_probe` dizesi olmamalı: `grep -c debug_epic_probe target/release/hangar.exe` → `0`.
- [ ] **Step 3:** Sızıntı kontrolü, ardından commit: `feat(app): epic giriş penceresi ve E0 keşif komutu`.
- [ ] **Step 4: E0 KONTROL NOKTASI (denetçi + kullanıcı, Windows).**
  1. `npm run tauri dev` başlatılır.
  2. Ana pencerede sağ tık › "Inspect" ile ya da `Ctrl+Shift+I` ile DevTools açılır, Console'a `await hangarProbe()` yazılır.
  3. Epic penceresinde kullanıcı kendi hesabıyla girer (2FA dahil).
  - **Geçti sayılması için hepsi gerekli:**
    1. Pencere başlığı host'u gösteriyor; giriş sonrası pencere kendiliğinden kapanıyor.
    2. `%APPDATA%\com.hangar.app\e0\` altında `live-token.json` ve en az bir `live-library-0.json` ve `live-catalog-0.json` var.
    3. Kullanıcı bu klasörde kendi Epic kullanıcı adını ve e-postasını `Select-String -Path "$env:APPDATA\com.hangar.app\e0\*" -Pattern "<kullanıcı adı>","<e-posta>"` ile arıyor ve **sonuç boş** çıkıyor. `live-token.json`'da token alanları `REDACTED`.
    4. Önce `%LOCALAPPDATA%\com.hangar.app\EBWebView` klasörünün **var olduğu** doğrulanır; yoksa bu madde "doğrulanamadı" sayılır, geçti sayılmaz. Sonra `Get-ChildItem -Recurse "$env:LOCALAPPDATA\com.hangar.app\EBWebView" -Filter Cookies* | Select-String -Pattern "epicgames" -SimpleMatch` boş çıkıyor.
    5. Konsoldaki dönüşte engellenen bir giriş sağlayıcısı yok; varsa host'u `ALLOWED_HOSTS`'a eklenir (T3a'ya küçük düzeltme commit'i).
  - **Başarısız (dur):** captcha ya da bot kontrolü geçilemiyor; Epic istemci kimliğini reddediyor (token çağrısı 400/401 ama kod alınmış); pencere `0x8007139F` ya da benzeri bir hatayla açılmıyor; 5 dk zaman aşımı tekrarlanıyor. Bu durumda T6 ve T7 başlamaz ve kullanıcıya yaklaşım B sorulur.

---

### Task 4: Kütüphane ve katalog ayrıştırma, filtre, kayıt üretimi (sentetik fixture)

**Files:**
- Create: `crates/hangar-core/src/accounts/epic.rs` (`pub mod epic;`)
- Create: `crates/hangar-core/tests/fixtures/epic/synthetic-library-0.json`, `synthetic-library-1.json`, `synthetic-catalog.json`

**Interfaces:**
- `#[derive(Debug, Clone, PartialEq)] pub struct LibItem { pub namespace: String, pub catalog_item_id: String, pub app_name: String }`
- `pub fn parse_library_page(json: &str) -> Result<(Vec<LibItem>, Option<String>), AuthError>`
  - `records[]` içinden `namespace`, `catalogItemId`, `appName` okunur.
  - İmleç `responseMetadata.nextCursor`'dan gelir; yoksa ya da boşsa `None`.
  - `namespace == "ue"` olanlar ve `appName`'i boş olanlar atlanır.
- `#[derive(Debug, Clone, PartialEq)] pub struct CatalogInfo { pub title: String, pub is_game: bool }`
- `pub fn parse_catalog(json: &str) -> Result<HashMap<String, CatalogInfo>, AuthError>`
  - Kök nesnenin anahtarı `catalogItemId`; değerden `title`, `categories[].path` ve `mainGameItem` okunur.
  - `is_game` şu üç koşulun hepsi tutunca true: `mainGameItem` yok; `categories` içinde `addons` ya da `digitalextras` yok; `categories` `applications` içeriyorsa `games` da içeriyor.
- `pub fn owned_entries(items: &[LibItem], catalog: &HashMap<String, CatalogInfo>) -> Vec<PlatformEntry>`
  - Katalogda bulunmayan ya da `!is_game` olan kayıtlar atlanır.
  - `platform: Epic`, `platform_id: app_name`, `title`, `installed: false`, diğer isteğe bağlı alanlar `None`, `launch_args: vec![]`.
  - `launch_uri = format!("com.epicgames.launcher://apps/{ns}%3A{id}%3A{app}?action=launch&silent=true")`
  - `install_uri = Some(format!("com.epicgames.launcher://apps/{ns}%3A{id}%3A{app}?action=install"))`
- **Sentetik fixture içerikleri:**
  - `synthetic-library-0.json`:
    `{"responseMetadata":{"nextCursor":"CUR1"},"records":[{"namespace":"ns1","catalogItemId":"game1","appName":"Fig","sandboxType":"PUBLIC"},{"namespace":"ue","catalogItemId":"ue1","appName":"UeAsset"},{"namespace":"ns2","catalogItemId":"dlc1","appName":"FigDlc"}]}`
  - `synthetic-library-1.json`:
    `{"responseMetadata":{},"records":[{"namespace":"ns3","catalogItemId":"app1","appName":"Tool"},{"namespace":"ns4","catalogItemId":"gone","appName":"Gone"}]}`
  - `synthetic-catalog.json` (anahtar → değer):
    - `game1`: `{"title":"Death's Door","categories":[{"path":"games"},{"path":"applications"}]}`
    - `dlc1`: `{"title":"Fig DLC","categories":[{"path":"addons"}],"mainGameItem":{"id":"game1"}}`
    - `extra1`: `{"title":"Soundtrack","categories":[{"path":"digitalextras"}]}`
    - `app1`: `{"title":"Tool","categories":[{"path":"applications"}]}`

- [ ] **Step 1: Testleri yaz.**

```rust
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
#[test] fn catalog_filters() {
    let c = parse_catalog(&fx("synthetic-catalog.json")).unwrap();
    assert!(c["game1"].is_game);
    assert_eq!(c["game1"].title, "Death's Door");
    assert!(!c["dlc1"].is_game && !c["extra1"].is_game && !c["app1"].is_game);
}
#[test] fn catalog_missing_item_skipped() {
    let items = vec![LibItem { namespace: "ns".into(), catalog_item_id: "gone".into(), app_name: "Gone".into() }];
    assert!(owned_entries(&items, &HashMap::new()).is_empty());
}
#[test] fn entry_uris() {
    let items = vec![LibItem { namespace: "ns".into(), catalog_item_id: "cid".into(), app_name: "Fig".into() }];
    let cat = HashMap::from([("cid".to_string(), CatalogInfo { title: "Death's Door".into(), is_game: true })]);
    let e = &owned_entries(&items, &cat)[0];
    assert_eq!((e.platform, e.platform_id.as_str(), e.installed), (Platform::Epic, "Fig", false));
    assert_eq!(e.launch_uri, "com.epicgames.launcher://apps/ns%3Acid%3AFig?action=launch&silent=true");
    assert_eq!(e.install_uri.as_deref(), Some("com.epicgames.launcher://apps/ns%3Acid%3AFig?action=install"));
}
```

- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS.
- [ ] **Step 5:** Commit: `feat(core): epic kütüphane ve katalog ayrıştırma`.

---

### Task 4b: Canlı yanıtlarla doğrulama (yalnızca yerel, E0'dan sonra)

**Files:** Modify `crates/hangar-core/src/accounts/epic.rs` (testler)

- [ ] **Step 1:** `#[ignore]` işaretli iki test ekle. Testler `HANGAR_EPIC_LIVE_DIR` ortam değişkenindeki klasörü okur; değişken yoksa erken döner.
  - `live_library_parses`: tüm `live-library-*.json` dosyaları ayrışır. Son sayfa dışındakilerde imleç `Some` döner. Toplam kayıt sayısı sıfırdan büyüktür.
  - `live_catalog_parses`: tüm `live-catalog-*.json` dosyaları ayrışır ve en az bir `is_game` kaydı vardır.
- [ ] **Step 2:** Denetçi Windows'ta çalıştırır: `HANGAR_EPIC_LIVE_DIR="$APPDATA/com.hangar.app/e0" cargo test -p hangar-core live_ -- --ignored` → PASS.
  - Başarısızsa ayrıştırma ya da filtre kuralları canlı şekle göre düzeltilir. Düzeltmeye karşılık gelen **sentetik** fixture'lar güncellenir; T4 testleri yeniden geçmelidir.
  - Bu adım tamamlanmadan T5c'ye geçilmez.
- [ ] **Step 3:** Commit: `test(core): epic canlı yanıt doğrulaması (yerel, ignore)`.

---

### Task 5a: Belge deposu ve DPAPI

**Files:**
- Create: `crates/hangar-core/src/accounts/store.rs` (`pub mod store;`)
- Modify: `crates/hangar-core/Cargo.toml`: `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_Security_Cryptography"] }`

**Interfaces:**
- `pub trait SecretStore: Send + Sync { fn load(&self, p: Platform) -> Result<Option<Vec<u8>>, AuthError>; fn save(&self, p: Platform, data: &[u8]) -> std::io::Result<()>; fn delete(&self, p: Platform) -> std::io::Result<()>; }`
  - `load`, çözülemeyen bir dosya için `Err(AuthError::Corrupt)` döner.
  - `delete`, dosya yoksa da `Ok` döner.
- `#[derive(Default)] pub struct MemoryStore { inner: Mutex<HashMap<Platform, Vec<u8>>>, pub fail_save: AtomicBool, pub corrupt: AtomicBool, saves: AtomicUsize }`
  - `pub fn saves(&self) -> usize`, `pub fn raw(&self, p: Platform) -> Option<Vec<u8>>`.
  - `fail_save` true ise `save` `Err` döner. `corrupt` true ise `load` `Err(Corrupt)` döner.
- `#[cfg(windows)] pub struct DpapiStore { dir: PathBuf }`, `pub fn new(dir: PathBuf) -> Self`.
  - `CryptProtectData` / `CryptUnprotectData`, entropi `b"hangar-accounts-v1"`, `CRYPTPROTECT_UI_FORBIDDEN`, çıktı `LocalFree` ile serbest bırakılır.
  - Dosya `<dir>/<p.as_str()>.dat`; yazarken önce `.dat.tmp`, sonra `std::fs::rename`. Klasör `create_dir_all` ile oluşturulur.
- `pub fn save_cred(store: &dyn SecretStore, c: &StoredCred) -> std::io::Result<()>` ve `pub fn load_cred(store: &dyn SecretStore) -> Result<Option<StoredCred>, AuthError>`: Epic için, JSON. `v != 1` ya da JSON hatası → `Corrupt`.

- [ ] **Step 1: Testleri yaz.**

```rust
#[test] fn memory_roundtrip_and_flags() {
    let s = MemoryStore::default();
    let c = StoredCred { v: 1, refresh_token: "RT".into(), refresh_expires_at: "x".into(), account_id: "a".into(), display_name: "d".into() };
    save_cred(&s, &c).unwrap();
    assert_eq!(load_cred(&s).unwrap().unwrap().refresh_token, "RT");
    s.corrupt.store(true, Ordering::SeqCst);
    assert_eq!(load_cred(&s).unwrap_err(), AuthError::Corrupt);
}
#[test] fn wrong_version_is_corrupt() {
    let s = MemoryStore::default();
    s.save(Platform::Epic, br#"{"v":2,"refresh_token":"r","refresh_expires_at":"x","account_id":"a","display_name":"d"}"#).unwrap();
    assert_eq!(load_cred(&s).unwrap_err(), AuthError::Corrupt);
}
#[cfg(windows)] #[test] fn dpapi_roundtrip_and_corrupt() {
    let dir = std::env::temp_dir().join(format!("hangar-dpapi-{}", std::process::id()));
    let s = DpapiStore::new(dir.clone());
    s.save(Platform::Epic, b"RT-PLAIN").unwrap();
    let raw = std::fs::read(dir.join("epic.dat")).unwrap();
    assert!(!raw.windows(8).any(|w| w == b"RT-PLAIN"));
    assert_eq!(s.load(Platform::Epic).unwrap().unwrap(), b"RT-PLAIN");
    std::fs::write(dir.join("epic.dat"), b"garbage").unwrap();
    assert_eq!(s.load(Platform::Epic).unwrap_err(), AuthError::Corrupt);
    s.delete(Platform::Epic).unwrap();
    assert_eq!(s.load(Platform::Epic).unwrap(), None);
    s.delete(Platform::Epic).unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
```

- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS (Windows'ta DPAPI testi dahil).
- [ ] **Step 5:** Commit: `feat(core): belge deposu ve windows dpapi`.

---

### Task 5b: Önbellek tabloları

**Files:** Modify `crates/hangar-core/src/db.rs`

**Interfaces** (yeni tablolar `init` içindeki `execute_batch`'e eklenir):
- `epic_catalog(item_id TEXT PRIMARY KEY, title TEXT NOT NULL, is_game INTEGER NOT NULL)`
  - `pub fn catalog_get(&self, ids: &[String]) -> Result<HashMap<String, CatalogInfo>>`: yalnızca bulunanları döner.
  - `pub fn catalog_put(&self, items: &HashMap<String, CatalogInfo>) -> Result<()>`: `INSERT OR REPLACE`.
- `owned_cache(platform TEXT PRIMARY KEY, json TEXT NOT NULL, fetched_at INTEGER NOT NULL)`
  - `pub fn owned_cache_get(&self, p: Platform) -> Result<Option<Vec<PlatformEntry>>>`
  - `pub fn owned_cache_put(&self, p: Platform, v: &[PlatformEntry], at: i64) -> Result<()>`
  - `pub fn owned_cache_delete(&self, p: Platform) -> Result<()>`
- Hesap durumu için yeni tablo yok: `settings` anahtarı `account_state.epic`, değeri `"expired"`. Yardımcılar: `pub fn account_expired(&self, p: Platform) -> Result<bool>`, `pub fn set_account_expired(&self, p: Platform, on: bool) -> Result<()>` (false → satır silinir).

- [ ] **Step 1: Testler:**
  - `catalog_roundtrip`: iki kayıt `put` edilir, üç kimlikle `get` iki döner, değerler eşit.
  - `owned_cache_roundtrip`: `put` → `get` eşit; `delete` → `None`.
  - `account_expired_flag`: varsayılan false → true → false; false olunca `get_setting("account_state.epic")` `None` döner.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** Commit: `feat(core): epic katalog ve sahip olunanlar önbelleği`.

---

### Task 5c: `EpicClient` akışı (denetçi inceleme kapısı)

**Files:** Modify `crates/hangar-core/src/accounts/epic.rs`

**Interfaces:**
- `pub struct EpicClient { api: Box<dyn EpicApi>, store: Box<dyn SecretStore>, lock: Mutex<()>, generation: AtomicU64 }`
- `impl EpicClient`:
  - `pub fn new(api: Box<dyn EpicApi>, store: Box<dyn SecretStore>) -> Self`
  - `pub fn connect(&self, db: &Db, code: &str) -> Result<String, AuthError>`
    - `lock` altında: `token([("grant_type","authorization_code"),("code",code),("token_type","eg1")])` → `parse_token` → `save_cred`.
    - Kayıt hatası `Network("kayıt")` olarak döner.
    - Ardından `set_account_expired(false)`, `generation += 1`; görünen adı döner.
  - `fn refresh_locked(&self) -> Result<String, AuthError>` (çağıran `lock`'u tutar):
    - `load_cred`: `None` → `NotConnected`; `Corrupt` → `store.delete` ve `Corrupt`.
    - `token([("grant_type","refresh_token"),("refresh_token",&cred.refresh_token),("token_type","eg1")])` → `parse_token`.
    - Yeni belge **access token döndürülmeden önce** `save_cred` ile kaydedilir. Kayıt başarısız olursa eski dosyaya dokunulmaz ve `Network("kayıt")` döner.
    - Access token döner.
  - `pub fn access_token(&self) -> Result<String, AuthError>`: `lock` alır ve `refresh_locked` çağırır.
  - `pub fn fetch_owned(&self, db: &Db) -> Result<Vec<PlatformEntry>, AuthError>`
    - `access_token` alınır, tüm kütüphane sayfaları okunur.
    - `catalog_get` ile önbellekte olmayan kimlikler namespace başına toplu istenir; çağrılar arasında `sleep(100ms)`.
    - `catalog_put`, ardından `owned_entries`.
  - `pub fn owned_for_scan(&self, db: &Db) -> (Option<Result<Vec<PlatformEntry>, ProviderError>>, Vec<String>)`
    - Başlangıçta `generation` okunur, sonra `fetch_owned`.
    - Sonuç **`lock` altında** ele alınır; `generation` değiştiyse sonuç atılır ve `(None, [])` döner. Değişmediyse:
      - `NotConnected` → `(None, [])`
      - `Ok(v)` → `owned_cache_put` ve `set_account_expired(false)`; `(Some(Ok(v)), [])`
      - `Network(_)`/`Parse(_)` → önbellek varsa `Some(Ok(cache))`, yoksa `None`; uyarı `"Epic oyunları güncellenemedi; son bilinen liste gösteriliyor."`
      - `InvalidGrant` → `set_account_expired(true)`; önbellek varsa onu kullanır; uyarı `"Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan."`
      - `Corrupt` → `(None, ["Epic bağlantısı okunamadı; yeniden bağlan."])`
  - `pub fn disconnect(&self, db: &Db) -> bool`
    - `lock` altında: `generation += 1`, ardından `refresh_locked` → `api.kill`; sonuç `revoked` olarak tutulur.
    - Her durumda `store.delete`, `owned_cache_delete` ve `set_account_expired(false)` çalışır.
    - `revoked` döner.
  - `pub fn status(&self, db: &Db) -> AccountStatus`: ağa çıkmaz.
    - `load_cred` `Ok(Some(c))` ise `Connected` ya da `Expired` (bayrağa göre) ve `display_name: Some(c.display_name)`.
    - Diğer durumlarda `Disconnected`.
- `#[derive(Serialize, Debug, PartialEq)] #[serde(rename_all = "lowercase")] pub enum AccountState { Disconnected, Connected, Expired }`
- `#[derive(Serialize, Debug, PartialEq)] pub struct AccountStatus { pub platform: Platform, pub state: AccountState, pub display_name: Option<String> }`

**Test sahtesi** (`#[cfg(test)] struct FakeApi`, aynı dosyada):
- Alanlar: `tokens: Mutex<VecDeque<Result<String, AuthError>>>` (sırayla dönen token gövdeleri), `library: Vec<String>` (sayfa gövdeleri; imleç `"CUR<n>"` ise n'inci sayfa döner), `catalog: Result<String, AuthError>`, `kill: Result<(), AuthError>`.
- Sayaçlar: `token_calls: AtomicUsize`, `catalog_calls: AtomicUsize`, `seen_refresh: Mutex<Vec<String>>` (token formundaki `refresh_token` değerleri).
- İsteğe bağlı kanca: `on_library: Option<Box<dyn Fn() + Send + Sync>>` (kütüphane çağrısı sırasında çalışır; yarış testleri için).
- `fn tok(at: &str, rt: &str) -> String` yardımcısı geçerli bir token gövdesi üretir (`displayName: "oyuncu"`, `account_id: "acc"`).
- `EpicClient` testlerde `Arc<MemoryStore>`'u paylaşabilsin diye `SecretStore`, `Arc<T>` için de uygulanır (`impl<T: SecretStore> SecretStore for Arc<T>`).

- [ ] **Step 1: Testleri yaz** (gövdeleri bu açıklamalara göre; assert'ler aynen):

```rust
#[test] fn connect_saves_and_returns_name() { /* tokens=[Ok(tok("AT1","RT1"))]; connect(db,"CODE") == Ok("oyuncu"); load_cred(store).refresh_token == "RT1"; status == Connected, display_name Some("oyuncu") */ }
#[test] fn rotated_token_saved_before_use() { /* stored RT1; tokens=[Ok(tok("AT2","RT2"))]; on_library hook asserts load_cred(store).refresh_token == "RT2"; fetch_owned ok; seen_refresh == ["RT1"] */ }
#[test] fn failed_save_keeps_old_cred() { /* stored RT1; store.fail_save=true; tokens=[Ok(tok("AT2","RT2"))]; access_token() == Err(Network("kayıt")); fail_save=false; load_cred.refresh_token == "RT1" */ }
#[test] fn refresh_is_serialized() { /* stored RT1; tokens=[Ok(tok("A2","RT2")), Ok(tok("A3","RT3"))]; Arc<EpicClient>; 2 threads call access_token(); join; seen_refresh sorted == ["RT1","RT2"] (second used rotated token) */ }
#[test] fn offline_uses_cache_with_warning() { /* first owned_for_scan Ok (1 game) → cache; then tokens returns Err(Network("x")); result == (Some(Ok(same 1 game)), vec!["Epic oyunları güncellenemedi; son bilinen liste gösteriliyor."]) */ }
#[test] fn invalid_grant_marks_expired() { /* tokens=[Err(InvalidGrant)]; warnings == ["Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan."]; status.state == Expired; store still has cred */ }
#[test] fn corrupt_store_is_disconnected() { /* store.corrupt=true; owned_for_scan == (None, ["Epic bağlantısı okunamadı; yeniden bağlan."]); status.state == Disconnected */ }
#[test] fn disconnect_offline_still_deletes() { /* stored; kill=Err(Network("x")); disconnect == false; store.raw(Epic)==None; owned_cache_get==None; status Disconnected */ }
#[test] fn disconnect_wins_over_inflight_scan() { /* on_library hook calls client.disconnect(db2) (own Db connection on same temp file); owned_for_scan returns (None, []); owned_cache_get == None; account_expired == false */ }
#[test] fn catalog_cached_between_scans() { /* two fetch_owned; catalog_calls == 1 */ }
#[test] fn no_secret_leaks() { /* all bodies use "AT-SECRET"/"RT-SECRET"; run connect, fetch_owned, an InvalidGrant scan and a Network scan; join all warnings + every AuthError Display + all rows of owned_cache/settings/epic_catalog (SELECT * as text); assert none contains "AT-SECRET" or "RT-SECRET" */ }
```

`disconnect_wins_over_inflight_scan` için `Db`, `std::env::temp_dir()` altında bir dosyada açılır (iki bağlantı aynı dosyayı kullanır). Kanca, `disconnect`'i `lock` boştayken çağırır; `fetch_owned` kütüphane çağrısı sırasında `lock`'u tutmaz.

- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS (testleri arka arkaya 3 kez çalıştır; titreme olmamalı).
- [ ] **Step 5:** Commit: `feat(core): epic hesap akışı (bağlan, yenile, tara, kes)`.
- [ ] **Step 6: Denetçi inceleme kapısı.** Denetçi bu görevin diff'ini şu gözle okur:
  - Her hata yolunda belge sızıntısı var mı?
  - Kilit sırası doğru mu?
  - Kayıt hatasında eski belge korunuyor mu?
  Sorun varsa T6'ya geçilmez.

---

### Task 6: Tauri komutları ve taramaya bağlama

**Files:**
- Create: `src-tauri/src/accounts.rs`
- Modify: `src-tauri/src/commands.rs` (`scan`, `launch`), `src-tauri/src/main.rs`, `src-tauri/capabilities/default.json` (`"dialog:allow-ask"` ekle)

**Interfaces:**
- Consumes: T3b `login::login`/`LoginError`; T5c `EpicClient`, `AccountStatus`; T5a `DpapiStore`; T3a `open_error`.
- **`main.rs` setup:**
  - `#[cfg(windows)] let store: Box<dyn SecretStore> = Box::new(DpapiStore::new(dir.join("accounts")));`
  - `#[cfg(not(windows))] let store: Box<dyn SecretStore> = Box::new(MemoryStore::default());`
  - `app.manage(EpicClient::new(Box::new(HttpEpicApi), store));`
- **Komutlar (`accounts.rs`):**
  - `get_accounts(app) -> Result<Vec<AccountStatus>, String>`: `vec![client.status(&db)]`.
  - `connect_account(app, platform: Platform) -> Result<AccountStatus, String>`
    - `platform != Epic` → `Err("Desteklenmiyor")`.
    - `login` hataları: `Cancelled` → `Err("cancelled")`, `Timeout` → `Err("Giriş zaman aşımına uğradı")`, `AlreadyOpen` → `Err("Giriş penceresi zaten açık")`, `Window(e)` → `Err(format!("Giriş penceresi açılamadı: {e}"))`.
    - Ardından `spawn_blocking` içinde kendi `Db::open(DbPath)` bağlantısıyla `client.connect`; dönüşte `status`.
  - `disconnect_account(app, platform) -> Result<bool, String>`: `spawn_blocking` içinde kendi `Db` bağlantısıyla `client.disconnect`.
  - `EpicClient`'a `app.state::<EpicClient>()` ile erişilir; `State` `'static` olmadığından `spawn_blocking` içinde `app.clone().state::<EpicClient>()` kullan.
- **`commands.rs::scan`:**
  - Mevcut durumda paylaşılan `Mutex<Db>` kısa süreli kilitlenip ayarlar okunuyor, ağ çağrısı kilit dışında yapılıyor.
  - Aynı desenle, Steam `fetch_owned`'ın hemen ardından ve paylaşılan kilit **alınmadan önce** şu çalışır: `let epic_db = Db::open(&app.state::<DbPath>().0).map_err(dberr)?; let (epic, epic_warn) = app.state::<EpicClient>().owned_for_scan(&epic_db);`
  - `epic` `Some(r)` ise `owned.push((Platform::Epic, r))`; `extra.extend(epic_warn)`.
- **`commands.rs::launch`:** URI açma hatası `.map_err(|e| open_error(&u, &e.to_string()))` ile döner.
- `main.rs` `generate_handler!` listesine `accounts::get_accounts`, `accounts::connect_account` ve `accounts::disconnect_account` eklenir (her iki cfg dalında).

- [ ] **Step 1:** Uygula. Mantık core'da test edildiği için src-tauri'de test yok.
- [ ] **Step 2:** `cargo check -p hangar` ve `cargo test -p hangar-core` → PASS.
- [ ] **Step 3:** Sızıntı kontrolü, ardından commit: `feat(app): epic hesap komutları ve taramaya bağlama`.

---

### Task 7: Ayarlar › Hesaplar arayüzü ve yükleniyor satırı

**Files:**
- Create: `src/components/AccountsSection.tsx`, `src/components/AccountsSection.test.tsx`
- Modify: `src/lib/api.ts`, `src/components/SettingsDialog.tsx`, `src/components/SettingsDialog.test.tsx` (mock'a `getAccounts`), `src/state/useLibrary.ts`, `src/App.tsx`

**Interfaces:**
- **`api.ts`:**
  - `export interface AccountStatus { platform: Platform; state: "disconnected" | "connected" | "expired"; display_name: string | null }`
  - `getAccounts: () => invoke<AccountStatus[]>("get_accounts")`
  - `connectAccount: (platform: Platform) => invoke<AccountStatus>("connect_account", { platform })`
  - `disconnectAccount: (platform: Platform) => invoke<boolean>("disconnect_account", { platform })`
- **`AccountsSection`** (`{ onChanged: (label?: string) => Promise<void> }`):
  - Açılışta `getAccounts` çağrılır; yalnızca `platform === "epic"` satırı gösterilir.
  - Satır başlığı "Epic Games".
  - **Durum metni:** "Bağlı değil" / `"Bağlı · " + display_name` / "Süresi doldu".
  - **Düğme:** "Bağlan" / "Bağlantıyı kes" / "Yeniden bağlan". Beklerken metin "Epic penceresi açık…" olur ve düğme devre dışı kalır.
  - Altta iki sabit `<small>` satırı: "Giriş Epic'in kendi sayfasında yapılır; Hangar şifreni görmez." ve "Bağlantı belgesi yalnızca bu bilgisayarda, şifreli tutulur."
  - **Bağlan:**
    - Başarılı → durum güncellenir ve `onChanged("Epic oyunları yükleniyor…")` çağrılır.
    - Hata `"cancelled"` → hiçbir şey gösterilmez.
    - Diğer hatalar → `role="alert"` ile metin gösterilir.
  - **Kes:**
    - `ask("Epic bağlantısı silinecek ve Epic'teki oturum kapatılacak; kurulu oyunlar kalır.", { title: "Bağlantıyı kes", kind: "warning" })` (`@tauri-apps/plugin-dialog`).
    - `true` ise `disconnectAccount`. Sonuç `false` olursa `role="status"` ile "Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin." gösterilir.
    - Ardından durum yenilenir ve `onChanged()` çağrılır.
- **`SettingsDialog`:**
  - Yeni prop: `onAccountsChanged?: (label?: string) => Promise<void>`.
  - `<AccountsSection onChanged={onAccountsChanged ?? (async () => {})} />` "Oyun klasörleri" bölümünün altına eklenir.
- **`useLibrary`:**
  - `rescan` aynen kalır.
  - Yeni: `const [scanLabel, setScanLabel] = useState<string | null>(null)` ve `rescanLabeled = useCallback(async (label: string) => { setScanLabel(label); try { await rescan(); } finally { if (mounted.current) setScanLabel(null); } }, [rescan])`.
  - İkisi de döndürülür.
- **`App.tsx`:**
  - `{rescanning && <span className="scan" role="status">{scanLabel ?? "Taranıyor…"}</span>}`
  - `<SettingsDialog … onAccountsChanged={(l) => (l ? rescanLabeled(l) : rescan())} />`

- [ ] **Step 1: `AccountsSection.test.tsx`.** `vi.mock("../lib/api")` ve `vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }))` ile yazılır:
  - `connects and triggers labeled rescan`: disconnected → "Bağlan" tıklanır → `connectAccount("epic")` çağrılır ve `onChanged("Epic oyunları yükleniyor…")` çağrılır; sonra "Bağlı · oyuncu" görünür.
  - `cancel is silent`: `connectAccount` `"cancelled"` ile reddedilir → `queryByRole("alert")` null döner ve `onChanged` çağrılmaz.
  - `other error shows alert`: `"Giriş zaman aşımına uğradı"` → alert bu metni içerir.
  - `disconnect asks first`: `ask` → false ise `disconnectAccount` çağrılmaz. `ask` → true ve `disconnectAccount` → true ise `onChanged()` argümansız çağrılır.
  - `server revoke failure is shown`: `disconnectAccount` → false ise status "Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin." metnini gösterir.
  - `expired shows reconnect`: state expired → "Yeniden bağlan" düğmesi ve "Süresi doldu" metni.
- [ ] **Step 2:** `npm test` → FAIL.
- [ ] **Step 3:** Uygula; mevcut `fld`, `row`, `ghost` sınıflarını kullan, yeni CSS yok.
- [ ] **Step 4:** `npm test` ve `npx tsc --noEmit` → PASS.
- [ ] **Step 5:** Commit: `feat(ui): ayarlarda hesaplar bölümü`.

---

### Task 8: README, elle doğrulama, release duman testi

**Files:** Modify `README.md`. `CLAUDE.md`'yi denetçi günceller, alt ajan güncellemez.

- [ ] **Step 1: README.**
  - "Veri konumu" bölümüne `accounts\` klasörü eklenir.
  - Yeni bir "Hesap bağlantıları (Epic)" bölümü yazılır. Ubisoft anılmaz. İçeriği:
    - **Ne saklanıyor:** yalnızca yenileme belgesi ve görünen ad, `%APPDATA%\com.hangar.app\accounts\epic.dat` dosyasında. Windows DPAPI ile şifreli; yalnızca bu Windows kullanıcısı çözebilir.
    - **Nasıl silinir:** "Bağlantıyı kes" hem dosyayı siler hem Epic'teki oturumu kapatır. Dosya elle de silinebilir.
    - **Epic'te görünüşü:** Hangar, Epic Games Launcher'ın herkese açık istemci kimliğini kullanır. Bu yüzden Epic hesap sayfasında bağlantı "Epic Games Launcher" olarak görünür.
    - **Resmi olmayan servis:** Epic bu yolu engelleyebilir; o zaman yalnızca kurulu Epic oyunları görünür.
    - **Kullanım koşulları:** bu yol Epic'in kullanım koşullarına aykırı sayılabilir. En kötü durumda hesap işaretlenebilir. Legendary, Heroic ve Playnite aynı yolu kullanıyor, ama bu bir garanti değil.
    - **Belgenin kapsamı:** belge hesabın tamamına erişir. DPAPI, aynı kullanıcıyla çalışan zararlı yazılıma karşı koruma sağlamaz.
    - **Sunucu yok:** Hangar'ın sunucusu yok; veri yalnızca Epic'e gider.
    - **Kalıcı belge istemiyorsan:** işin bitince bağlantıyı kes.
- [ ] **Step 2:** Tüm testler PASS: `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`.
- [ ] **Step 3:** Release duman testi: `npm run tauri build` başarılı ve `grep -c debug_epic_probe target/release/hangar.exe` → `0`.
- [ ] **Step 4: Elle kontrol (kullanıcıyla, `npm run tauri dev`):**
  1. Ayarlar › Hesaplar › "Bağlan": pencere başlığı "Epic Games girişi — www.epicgames.com" gösteriyor; giriş sonrası pencere kendiliğinden kapanıyor; durum "Bağlı · <ad>".
  2. Üstte "Epic oyunları yükleniyor…" görünüyor, ardından kurulu olmayan Epic oyunları "Kur" kartlarıyla geliyor.
  3. Death's Door hâlâ tek kart ve "Oyna" çalışıyor.
  4. Uygulama kapatılıp açılınca giriş istenmiyor, durum "Bağlı".
  5. Kurulu olmayan bir Epic oyununda "Kur" Epic launcher'da o oyunun sayfasını ya da kurulumunu açıyor.
  6. Wi-Fi kapalıyken "Yeniden tara" son bilinen listeyi ve "Epic oyunları güncellenemedi; son bilinen liste gösteriliyor." uyarısını gösteriyor.
  7. "Bağlantıyı kes" onay soruyor, sonra yalnızca kurulu Epic oyunları kalıyor. `%APPDATA%\com.hangar.app\accounts\epic.dat` yok; epicgames.com hesap sayfasında oturum kapanmış.
  8. `EBWebView` klasörü var ve içinde `epicgames` çerezi yok (E0'daki komutla).
- [ ] **Step 5:** Commit: `docs: epic hesap bağlantısı için güvenlik notu`.
