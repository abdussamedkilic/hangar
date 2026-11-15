# Epic Account Connection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** All games on the Epic account (including uninstalled ones) arrive in the Hangar library by connecting in-app through Epic's own login page; the credential stays only on this PC, encrypted with DPAPI.

**Architecture:** An `accounts` module is added to `hangar-core`: the Epic HTTP client (behind a trait), pure login logic (navigation decision, init script), parsing, the credential store (DPAPI) and the `EpicClient` flow. `scan_all` takes owned lists per platform. `src-tauri` is only thin glue: the login window (`login.rs`) and account commands (`accounts.rs`). On the React side an "Hesaplar" section is added to Settings.

**Tech Stack:** Rust (ureq 2, rusqlite, serde_json, url 2, zeroize 1, windows-sys 0.61), Tauri 2.12.5, React 18 + TS + vitest, `@tauri-apps/plugin-dialog` (`ask`).

**Spec:** `docs/superpowers/specs/2026-11-12-hangar-accounts-design.md`

This plan went through two council rounds (design and plan).

## Global Constraints

- **The credential never leaks:** the refresh token, access token, authorization code and id_token are never written anywhere. This includes logs, `println!`/`eprintln!`/`dbg!`, error messages, warnings, SQLite, `settings`, fixtures, commit messages and test output.
  - Error texts contain at most an HTTP status code and a short description; never a response body or URL.
  - Never print a `ureq::Error` with `Display`/`Debug`. Follow the `net_err` pattern in Steam; the kill URL contains the access token.
- **Epic client:** id `34a02cf8f4414e29b15921876da36f9a`, secret `daafbccc737745039dffe53d94fc76cf`, `token_type=eg1`. These values are public and are hardcoded.
- **Endpoints:**
  - Token: `https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token`
  - Kill: `DELETE https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/<access_token>`
  - Library: `https://library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true[&cursor=…]`
  - Catalog: `https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/<ns>/bulk/items?id=<id>[,<id>…]&includeDLCDetails=true&includeMainGameDetails=true&country=US&locale=en-US`
- **Login URL:** `https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode`
- **Capture URL:** `https://hangar.invalid/callback?code=<code>`
- **Credential file:** `<app_data_dir>\accounts\epic.dat`
  - DPAPI CurrentUser, entropy `b"hangar-accounts-v1"`.
  - The JSON before encryption contains `"v": 1`.
  - Writes go through a temp file + rename.
- **Timing:** at least 100 ms between catalog calls; network timeout 15 s; login timeout 5 min.
- **Dependencies:** `hangar-core` does not depend on Tauri. Windows-specific code sits under `#[cfg(windows)]` with a counterpart that compiles on Linux.
- **Scope:** Ubisoft is not in this plan. No UI row, README text or repo file is added for it.
- **Fixtures:** only **hand-written synthetic** fixtures go into the repo. Live E0 responses stay only on the user's PC, under `<app_data_dir>\e0\`; they are not committed.
- **Language:** UI copy is Turkish. Commit messages are English conventional (`feat(core): …`) and end with this line: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Environment:** commands run in Windows Git Bash, at the repo root. Node is not on PATH; first run:
  `export PATH="/c/Users/user/AppData/Roaming/fnm/node-versions/v26.13.0/installation:$PATH"`
- **Tests:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`.
  - No tests are written for `src-tauri`; Tauri test binaries are brittle on Windows. Testable logic lives in `hangar-core`.
- **Leak check before commit:** before every commit this command must come out empty:
  `git diff --cached -- . ':!Cargo.lock' | grep -nEi '"(access_token|refresh_token|id_token)"\s*:\s*"[^"R]|eg1~|[0-9a-f]{32}' | grep -v 34a02cf8f4414e29b15921876da36f9a | grep -v daafbccc737745039dffe53d94fc76cf`
  If a result appears and you are sure it is a synthetic test id, write the explanation in the commit message.

## Review Focus

1. **Removed game:** an id that is absent from the catalog response is skipped silently, the scan does not fail. Test: T4 `catalog_missing_item_skipped`.
2. **Concurrent refresh or crash:**
   - The new `refresh_token` is saved before it is used.
   - If the save fails, the old credential is not deleted.
   - A second refresh does not use the old credential.
   - Test: T5c `rotated_token_saved_before_use`, `failed_save_keeps_old_cred`, `refresh_is_serialized`.
3. **Scan during disconnect:** a scan in progress cannot write back the cache or the "expired" state after "Bağlantıyı kes". Test: T5c `disconnect_wins_over_inflight_scan`.
4. **Offline scan and disconnect:**
   - A scan shows the last known list and a warning appears.
   - Disconnect still deletes the file and says that the session on Epic could not be closed.
   - Test: T5c `offline_uses_cache_with_warning`, `disconnect_offline_still_deletes`.
5. **Different names:** if the manifest name of an installed Epic game differs from the catalog name, a single card remains, the manifest name is kept, and `install_uri` is updated. Test: T1 `owned_epic_keeps_manifest_title`.

## Order and parallelism

| Order | Task | Depends on | Can run in parallel |
|---|---|---|---|
| 1 | T1 `scan_all` | — | T2, T3a |
| 2 | T2 Epic client + types + sanitizer | — | T1, T3a |
| 3 | T3a Pure login logic (core) | — | T1, T2 |
| 4 | T3b Login window + E0 probe → **E0 (user)** | T2, T3a | T4 |
| 5 | T4 Parsing (synthetic fixture) | T2 | T3b / E0 |
| 6 | T4b Verification with live responses (local) | T4, E0 | — |
| 7 | T5a `SecretStore` + DPAPI | T2 | T5b |
| 8 | T5b DB tables | T4 | T5a |
| 9 | T5c `EpicClient` flow — **reviewer gate** | T4, T5a, T5b | — |
| 10 | T6 Tauri commands + scan wiring | T1, T3b, T5c | T7 |
| 11 | T7 Accounts UI | the `AccountStatus` shape from T5c | T6 |
| 12 | T8 README, manual check, release smoke test | all | — |

**If E0 fails:** T6 and T7 stop, and the window part of T3b is dropped. The user is asked about approach B (pasting the code by hand). T1, T2, T4 and T5 remain valid on both paths.

---

### Task 1: `scan_all` takes owned lists per platform

**Files:**
- Modify: `crates/hangar-core/src/library.rs` (`scan_all` and its tests)
- Modify: `src-tauri/src/commands.rs` (the `scan_all` call inside `scan`)

**Interfaces:**
- Produces:
  - `pub type OwnedLists = Vec<(Platform, Result<Vec<PlatformEntry>, ProviderError>)>;`
  - `pub fn scan_all(db: &Db, providers: &[Box<dyn LibraryProvider>], owned: OwnedLists) -> rusqlite::Result<ScanReport>`
- **Merge rule:** if an installed entry with the same `platform` and `platform_id` exists, the installed entry stays, but these fields are updated:
  - `playtime_min`: taken from the owned entry if it is Some.
  - `last_played`: `installed.last_played.or(owned.last_played)`.
  - `install_uri`: replaced with the owned entry's if it is Some.
  - `title` never changes.
  - If there is no match, the owned entry is added.
- **`Err` warnings:**
  - For Steam the existing text stays as is: `"Steam sahip olunan oyunlar alınamadı: API anahtarını kontrol et."`.
  - For other platforms `format!("{:?} sahip olunan oyunlar alınamadı.", platform)`.

- [ ] **Step 1: Convert the existing tests to the new signature and add the new tests.** Conversion rules: `scan_all(.., None)` → `vec![]`, `Some(x)` → `vec![(Platform::Steam, x)]`. Tests to add:

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

- [ ] **Step 2:** `cargo test -p hangar-core library` → compile error or FAIL.
- [ ] **Step 3:** Write `scan_all` according to the rule. In `commands.rs::scan`, pass the Steam result like this: `let owned: OwnedLists = owned.into_iter().map(|r| (Platform::Steam, r)).collect();`.
- [ ] **Step 4:** `cargo test -p hangar-core` and `cargo check -p hangar` → PASS.
- [ ] **Step 5:** Run the leak check, then commit: `refactor(core): scan_all takes owned lists per platform`.

---

### Task 2: Epic HTTP client, credential types and sanitizer

**Files:**
- Create: `crates/hangar-core/src/accounts/mod.rs`, `crates/hangar-core/src/accounts/epic_api.rs`, `crates/hangar-core/src/accounts/sanitize.rs`
- Modify: `crates/hangar-core/src/lib.rs` (`pub mod accounts;`)
- Modify: `crates/hangar-core/Cargo.toml`: `zeroize = "1"`; `[dev-dependencies] base64 = "0.22"`

**Interfaces:**
- `accounts/mod.rs`:
  - `#[derive(Debug, Clone, PartialEq)] pub enum AuthError { NotConnected, Corrupt, InvalidGrant, Network(String), Parse(String) }` and `impl Display`. Display texts:
    - `NotConnected`: "Epic bağlı değil"
    - `Corrupt`: "Epic bağlantısı okunamadı"
    - `InvalidGrant`: "Epic bağlantısının süresi doldu"
    - `Network(s)`: `format!("Epic'e ulaşılamadı ({s})")`
    - `Parse(s)`: `format!("Epic yanıtı anlaşılamadı ({s})")`
  - `#[derive(Serialize, Deserialize, Clone, PartialEq)] pub struct StoredCred { pub v: u32, pub refresh_token: String, pub refresh_expires_at: String, pub account_id: String, pub display_name: String }`
    - Hand-written `Debug`: the `refresh_token` and `account_id` fields show `"***"`.
    - `impl Drop`: zeroes `refresh_token` with `zeroize`.
  - `pub struct Tokens { pub access_token: String, pub cred: StoredCred }`: hand-written `Debug` (`***`) and `zeroize` for `access_token` on `Drop`.
  - `pub fn parse_token(json: &str) -> Result<Tokens, AuthError>`
    - Fields read: `access_token`, `refresh_token`, `refresh_expires_at`, `account_id`, `displayName`.
    - `v` is always 1.
    - If a field is missing it returns `Err(Parse("token"))`; the body does not enter the message.
- `accounts/epic_api.rs`:
  - `pub trait EpicApi: Send + Sync { fn token(&self, form: &[(&str, &str)]) -> Result<String, AuthError>; fn library_page(&self, access: &str, cursor: Option<&str>) -> Result<String, AuthError>; fn catalog(&self, access: &str, namespace: &str, ids: &[String]) -> Result<String, AuthError>; fn kill(&self, access: &str) -> Result<(), AuthError>; }`
    - Response bodies return as raw strings.
  - `pub struct HttpEpicApi;`, uses ureq.
    - On the token endpoint HTTP 400/401 → `AuthError::InvalidGrant`.
    - Other HTTP statuses → `Network(format!("durum {code}"))`.
    - Transport error → `Network(format!("{:?}", t.kind()))`.
    - Timeout 15 s.
  - `pub const BASIC_AUTH: &str = "MzRhMDJjZjhmNDQxNGUyOWIxNTkyMTg3NmRhMzZmOWE6ZGFhZmJjY2M3Mzc3NDUwMzlkZmZlNTNkOTRmYzc2Y2Y=";` (header `Authorization: basic <BASIC_AUTH>`)
  - `pub const LOGIN_URL: &str`: the login URL from Global Constraints.
- `accounts/sanitize.rs`: `pub fn sanitize(body: &str, secrets: &[&str]) -> Result<String, String>`
  - If the body is JSON, the values of these keys become `"REDACTED"`: `access_token`, `refresh_token`, `id_token`, `account_id`, `accountId`, `displayName`, `email`, `ownerId`, `authorizationCode`, `sid`. Nested objects are traversed too.
  - Then every **non-empty** value in the `secrets` list is replaced with `"REDACTED"` in the text.
  - If a non-empty secret still appears in the result, it returns `Err("sızıntı")`.

- [ ] **Step 1: Write the tests.**

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
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS.
- [ ] **Step 5:** Leak check, then commit: `feat(core): epic http client, credential types and sanitizer`.

---

### Task 3a: Pure login logic (core)

**Files:**
- Create: `crates/hangar-core/src/accounts/login.rs` (open the module in `accounts/mod.rs` with `pub mod login;`)
- Modify: `crates/hangar-core/Cargo.toml`: `url = "2"`

**Interfaces:**
- `pub enum NavDecision { Callback(String), Allow, Block }`
- `pub fn decide(url: &url::Url) -> NavDecision`
  - Returns `Callback(code)` if all of these hold: scheme `https`, host `hangar.invalid`, path `/callback`, the `code` parameter is non-empty. The code is returned percent-decoded.
  - If only the host is `hangar.invalid` and the other conditions do not hold, `Block`.
  - If the scheme is `https` and the host equals a value in `ALLOWED_HOSTS` or is a subdomain of one (`host == h || host.ends_with(&format!(".{h}"))`), `Allow`.
  - In every other case `Block`.
- `pub const ALLOWED_HOSTS: &[&str] = &["epicgames.com", "epicgames.dev", "unrealengine.com", "hcaptcha.com", "arkoselabs.com", "funcaptcha.com", "google.com", "apple.com", "live.com", "microsoftonline.com", "xbox.com", "steampowered.com", "steamcommunity.com", "playstation.com", "sonyentertainmentnetwork.com", "nintendo.com", "facebook.com"];`
  - Rationale: Epic redirects sign-in with these providers from its own page. The list restricts only top-frame navigations; in-page resources and iframes are not affected.
- `pub const INIT_SCRIPT: &str`: the behavior below, entirely inside `try/catch`:

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
  - If `uri` starts with `com.epicgames.launcher://`, `"Epic Games Launcher açılamadı — kurulu mu?"`.
  - Otherwise `format!("Açılamadı: {err}")`.

- [ ] **Step 1: Write the tests.**

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
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS.
- [ ] **Step 5:** Commit: `feat(core): pure navigation rules for the epic login window`.

---

### Task 3b: Login window and E0 probe (thin glue) → E0 checkpoint

**Files:**
- Create: `src-tauri/src/login.rs`
- Modify: `src-tauri/src/main.rs`, `src/lib/api.ts` (DEV line only)

**Interfaces:**
- Consumes:
  - T3a: `accounts::login::{decide, NavDecision, INIT_SCRIPT, window_title}`
  - T2: `accounts::epic_api::{LOGIN_URL, HttpEpicApi, EpicApi}`, `accounts::parse_token`, `accounts::sanitize::sanitize`
- Produces:
  - `pub enum LoginError { Cancelled, Timeout, AlreadyOpen, Window(String) }`
  - `pub async fn login(app: &AppHandle) -> Result<String, LoginError>`: returns the authorization code on success.
- **`login` behavior:**
  1. If a window labeled `account-login` exists, it calls `set_focus` and returns `Err(AlreadyOpen)`.
  2. `let (tx, rx) = std::sync::mpsc::channel::<Result<String, LoginError>>();`
  3. Window: `WebviewWindowBuilder::new(app, "account-login", WebviewUrl::External(LOGIN_URL.parse().unwrap()))`, with these settings:
     - `.title(window_title(..))`, `.incognito(true)`, `.initialization_script(INIT_SCRIPT)`, `.inner_size(520.0, 760.0)`.
     - `.on_navigation(move |url| …)`: depending on the `decide(url)` result:
       - `Callback(c)` → `tx.send(Ok(c))` and returns `false`.
       - `Allow` → sets the window title to `window_title(url)` (via `app.get_webview_window("account-login")`; this is done inside `async_runtime::spawn`), returns `true`.
       - `Block` → returns `false`.
       - Nothing is logged inside the closure.
     - `.on_new_window(...)` → `NewWindowResponse::Deny`. If the URL is allowed, `navigate(url)` is done in the same window via `async_runtime::spawn`.
     - A `.build()` error returns `Err(Window(..))`. In E0 the error text shows the incognito/user data folder conflict (0x8007139F).
  4. `window.on_window_event`: `WindowEvent::Destroyed` → `tx2.send(Err(Cancelled))`. The first message to arrive wins.
  5. `spawn_blocking(move || rx.recv_timeout(Duration::from_secs(300)))`. On timeout `Err(Timeout)`.
  6. After the result, the window (if it still exists) is closed with `close()`.
- **`debug_epic_probe`:** `#[cfg(debug_assertions)] #[tauri::command] pub async fn debug_epic_probe(app: AppHandle) -> Result<String, String>`
  1. `login` is run.
  2. `token(authorization_code)` is obtained.
  3. A `KillOnDrop` guard is set up; `api.kill(access)` is called even on error.
  4. Library pages are traversed with `serde_json::Value` (`responseMetadata.nextCursor`). In this task there is no dependency on T4.
  5. Catalog is requested for the first 3 namespaces from the incoming records.
  6. Each body is cleaned with `sanitize(body, &[code, access, refresh, account_id, display_name])`. The token response is also written after being cleaned.
  7. Files are written under `<app_data_dir>/e0/`: `live-token.json`, `live-library-<n>.json`, `live-catalog-<n>.json`. **Nothing is written to the repo.**
  8. Return text: the number of files written and the folder path.
  9. The credential is not written to any file.
- **`main.rs`:**
  - `mod login;`
  - `debug_epic_probe` is registered with two separate `invoke_handler` branches: the command is in the list under `#[cfg(debug_assertions)]` and absent under `#[cfg(not(debug_assertions))]`.
- **End of `api.ts`:** `if (import.meta.env.DEV) (window as unknown as { hangarProbe: () => Promise<string> }).hangarProbe = () => invoke<string>("debug_epic_probe");`

- [ ] **Step 1:** Implement. No tests; the logic was tested in T3a.
- [ ] **Step 2:** `cargo check -p hangar` and `cargo build -p hangar --release` → PASS. The release binary must not contain the string `debug_epic_probe`: `grep -c debug_epic_probe target/release/hangar.exe` → `0`.
- [ ] **Step 3:** Leak check, then commit: `feat(app): epic login window and E0 discovery command`.
- [ ] **Step 4: E0 CHECKPOINT (reviewer + user, Windows).**
  1. `npm run tauri dev` is started.
  2. In the main window, open DevTools with right-click › "Inspect" or `Ctrl+Shift+I`, and type `await hangarProbe()` in the Console.
  3. In the Epic window the user signs in with their own account (including 2FA).
  - **All of these are required to count as passed:**
    1. The window title shows the host; after login the window closes on its own.
    2. Under `%APPDATA%\com.hangar.app\e0\` there are `live-token.json` and at least one `live-library-0.json` and `live-catalog-0.json`.
    3. In this folder the user searches for their own Epic username and email with `Select-String -Path "$env:APPDATA\com.hangar.app\e0\*" -Pattern "<username>","<email>"` and the **result is empty**. In `live-token.json` the token fields are `REDACTED`.
    4. First verify that the `%LOCALAPPDATA%\com.hangar.app\EBWebView` folder **exists**; if it does not, this item counts as "could not be verified", not passed. Then `Get-ChildItem -Recurse "$env:LOCALAPPDATA\com.hangar.app\EBWebView" -Filter Cookies* | Select-String -Pattern "epicgames" -SimpleMatch` comes out empty.
    5. In the console output there is no blocked login provider; if there is, its host is added to `ALLOWED_HOSTS` (a small fix commit to T3a).
  - **Failed (stop):** captcha or bot check cannot be passed; Epic rejects the client id (token call 400/401 but a code was obtained); the window does not open with `0x8007139F` or a similar error; the 5-minute timeout repeats. In this case T6 and T7 do not start and the user is asked about approach B.

---

### Task 4: Library and catalog parsing, filter, entry generation (synthetic fixture)

**Files:**
- Create: `crates/hangar-core/src/accounts/epic.rs` (`pub mod epic;`)
- Create: `crates/hangar-core/tests/fixtures/epic/synthetic-library-0.json`, `synthetic-library-1.json`, `synthetic-catalog.json`

**Interfaces:**
- `#[derive(Debug, Clone, PartialEq)] pub struct LibItem { pub namespace: String, pub catalog_item_id: String, pub app_name: String }`
- `pub fn parse_library_page(json: &str) -> Result<(Vec<LibItem>, Option<String>), AuthError>`
  - `namespace`, `catalogItemId`, `appName` are read from `records[]`.
  - The cursor comes from `responseMetadata.nextCursor`; `None` if absent or empty.
  - Entries with `namespace == "ue"` and those with an empty `appName` are skipped.
- `#[derive(Debug, Clone, PartialEq)] pub struct CatalogInfo { pub title: String, pub is_game: bool }`
- `pub fn parse_catalog(json: &str) -> Result<HashMap<String, CatalogInfo>, AuthError>`
  - The root object's key is `catalogItemId`; `title`, `categories[].path` and `mainGameItem` are read from the value.
  - `is_game` is true when all three conditions hold: no `mainGameItem`; `categories` has neither `addons` nor `digitalextras`; if `categories` contains `applications`, it also contains `games`.
- `pub fn owned_entries(items: &[LibItem], catalog: &HashMap<String, CatalogInfo>) -> Vec<PlatformEntry>`
  - Entries that are not in the catalog or are `!is_game` are skipped.
  - `platform: Epic`, `platform_id: app_name`, `title`, `installed: false`, other optional fields `None`, `launch_args: vec![]`.
  - `launch_uri = format!("com.epicgames.launcher://apps/{ns}%3A{id}%3A{app}?action=launch&silent=true")`
  - `install_uri = Some(format!("com.epicgames.launcher://apps/{ns}%3A{id}%3A{app}?action=install"))`
- **Synthetic fixture contents:**
  - `synthetic-library-0.json`:
    `{"responseMetadata":{"nextCursor":"CUR1"},"records":[{"namespace":"ns1","catalogItemId":"game1","appName":"Fig","sandboxType":"PUBLIC"},{"namespace":"ue","catalogItemId":"ue1","appName":"UeAsset"},{"namespace":"ns2","catalogItemId":"dlc1","appName":"FigDlc"}]}`
  - `synthetic-library-1.json`:
    `{"responseMetadata":{},"records":[{"namespace":"ns3","catalogItemId":"app1","appName":"Tool"},{"namespace":"ns4","catalogItemId":"gone","appName":"Gone"}]}`
  - `synthetic-catalog.json` (key → value):
    - `game1`: `{"title":"Death's Door","categories":[{"path":"games"},{"path":"applications"}]}`
    - `dlc1`: `{"title":"Fig DLC","categories":[{"path":"addons"}],"mainGameItem":{"id":"game1"}}`
    - `extra1`: `{"title":"Soundtrack","categories":[{"path":"digitalextras"}]}`
    - `app1`: `{"title":"Tool","categories":[{"path":"applications"}]}`

- [ ] **Step 1: Write the tests.**

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
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS.
- [ ] **Step 5:** Commit: `feat(core): epic library and catalog parsing`.

---

### Task 4b: Verification with live responses (local only, after E0)

**Files:** Modify `crates/hangar-core/src/accounts/epic.rs` (tests)

- [ ] **Step 1:** Add two tests marked `#[ignore]`. The tests read the folder in the `HANGAR_EPIC_LIVE_DIR` environment variable; if the variable is absent they return early.
  - `live_library_parses`: all `live-library-*.json` files parse. On all but the last page the cursor returns `Some`. The total record count is greater than zero.
  - `live_catalog_parses`: all `live-catalog-*.json` files parse and there is at least one `is_game` entry.
- [ ] **Step 2:** The reviewer runs on Windows: `HANGAR_EPIC_LIVE_DIR="$APPDATA/com.hangar.app/e0" cargo test -p hangar-core live_ -- --ignored` → PASS.
  - If it fails, the parsing or filter rules are fixed to match the live shape. The corresponding **synthetic** fixtures are updated; the T4 tests must pass again.
  - T5c is not started before this step is finished.
- [ ] **Step 3:** Commit: `test(core): epic live response verification (local, ignored)`.

---

### Task 5a: Credential store and DPAPI

**Files:**
- Create: `crates/hangar-core/src/accounts/store.rs` (`pub mod store;`)
- Modify: `crates/hangar-core/Cargo.toml`: `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_Security_Cryptography"] }`

**Interfaces:**
- `pub trait SecretStore: Send + Sync { fn load(&self, p: Platform) -> Result<Option<Vec<u8>>, AuthError>; fn save(&self, p: Platform, data: &[u8]) -> std::io::Result<()>; fn delete(&self, p: Platform) -> std::io::Result<()>; }`
  - `load` returns `Err(AuthError::Corrupt)` for a file that cannot be decrypted.
  - `delete` returns `Ok` even if the file does not exist.
- `#[derive(Default)] pub struct MemoryStore { inner: Mutex<HashMap<Platform, Vec<u8>>>, pub fail_save: AtomicBool, pub corrupt: AtomicBool, saves: AtomicUsize }`
  - `pub fn saves(&self) -> usize`, `pub fn raw(&self, p: Platform) -> Option<Vec<u8>>`.
  - If `fail_save` is true, `save` returns `Err`. If `corrupt` is true, `load` returns `Err(Corrupt)`.
- `#[cfg(windows)] pub struct DpapiStore { dir: PathBuf }`, `pub fn new(dir: PathBuf) -> Self`.
  - `CryptProtectData` / `CryptUnprotectData`, entropy `b"hangar-accounts-v1"`, `CRYPTPROTECT_UI_FORBIDDEN`, the output is released with `LocalFree`.
  - File `<dir>/<p.as_str()>.dat`; when writing, first `.dat.tmp`, then `std::fs::rename`. The folder is created with `create_dir_all`.
- `pub fn save_cred(store: &dyn SecretStore, c: &StoredCred) -> std::io::Result<()>` and `pub fn load_cred(store: &dyn SecretStore) -> Result<Option<StoredCred>, AuthError>`: for Epic, JSON. `v != 1` or a JSON error → `Corrupt`.

- [ ] **Step 1: Write the tests.**

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
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS (including the DPAPI test on Windows).
- [ ] **Step 5:** Commit: `feat(core): credential store and windows dpapi`.

---

### Task 5b: Cache tables

**Files:** Modify `crates/hangar-core/src/db.rs`

**Interfaces** (new tables are added to the `execute_batch` in `init`):
- `epic_catalog(item_id TEXT PRIMARY KEY, title TEXT NOT NULL, is_game INTEGER NOT NULL)`
  - `pub fn catalog_get(&self, ids: &[String]) -> Result<HashMap<String, CatalogInfo>>`: returns only those found.
  - `pub fn catalog_put(&self, items: &HashMap<String, CatalogInfo>) -> Result<()>`: `INSERT OR REPLACE`.
- `owned_cache(platform TEXT PRIMARY KEY, json TEXT NOT NULL, fetched_at INTEGER NOT NULL)`
  - `pub fn owned_cache_get(&self, p: Platform) -> Result<Option<Vec<PlatformEntry>>>`
  - `pub fn owned_cache_put(&self, p: Platform, v: &[PlatformEntry], at: i64) -> Result<()>`
  - `pub fn owned_cache_delete(&self, p: Platform) -> Result<()>`
- No new table for account state: the `settings` key `account_state.epic`, with value `"expired"`. Helpers: `pub fn account_expired(&self, p: Platform) -> Result<bool>`, `pub fn set_account_expired(&self, p: Platform, on: bool) -> Result<()>` (false → the row is deleted).

- [ ] **Step 1: Tests:**
  - `catalog_roundtrip`: two entries are `put`, `get` with three ids returns two, values equal.
  - `owned_cache_roundtrip`: `put` → `get` equal; `delete` → `None`.
  - `account_expired_flag`: default false → true → false; once false, `get_setting("account_state.epic")` returns `None`.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** Commit: `feat(core): epic catalog and owned-games cache`.

---

### Task 5c: `EpicClient` flow (reviewer gate)

**Files:** Modify `crates/hangar-core/src/accounts/epic.rs`

**Interfaces:**
- `pub struct EpicClient { api: Box<dyn EpicApi>, store: Box<dyn SecretStore>, lock: Mutex<()>, generation: AtomicU64 }`
- `impl EpicClient`:
  - `pub fn new(api: Box<dyn EpicApi>, store: Box<dyn SecretStore>) -> Self`
  - `pub fn connect(&self, db: &Db, code: &str) -> Result<String, AuthError>`
    - Under `lock`: `token([("grant_type","authorization_code"),("code",code),("token_type","eg1")])` → `parse_token` → `save_cred`.
    - A save error is returned as `Network("kayıt")`.
    - Then `set_account_expired(false)`, `generation += 1`; returns the display name.
  - `fn refresh_locked(&self) -> Result<String, AuthError>` (the caller holds `lock`):
    - `load_cred`: `None` → `NotConnected`; `Corrupt` → `store.delete` and `Corrupt`.
    - `token([("grant_type","refresh_token"),("refresh_token",&cred.refresh_token),("token_type","eg1")])` → `parse_token`.
    - The new credential is saved with `save_cred` **before the access token is returned**. If the save fails the old file is not touched and `Network("kayıt")` is returned.
    - Returns the access token.
  - `pub fn access_token(&self) -> Result<String, AuthError>`: takes `lock` and calls `refresh_locked`.
  - `pub fn fetch_owned(&self, db: &Db) -> Result<Vec<PlatformEntry>, AuthError>`
    - `access_token` is obtained, all library pages are read.
    - Ids not in the cache per `catalog_get` are requested in bulk per namespace; `sleep(100ms)` between calls.
    - `catalog_put`, then `owned_entries`.
  - `pub fn owned_for_scan(&self, db: &Db) -> (Option<Result<Vec<PlatformEntry>, ProviderError>>, Vec<String>)`
    - `generation` is read at the start, then `fetch_owned`.
    - The result is handled **under `lock`**; if `generation` changed the result is discarded and `(None, [])` is returned. If it did not change:
      - `NotConnected` → `(None, [])`
      - `Ok(v)` → `owned_cache_put` and `set_account_expired(false)`; `(Some(Ok(v)), [])`
      - `Network(_)`/`Parse(_)` → `Some(Ok(cache))` if there is a cache, otherwise `None`; warning `"Epic oyunları güncellenemedi; son bilinen liste gösteriliyor."`
      - `InvalidGrant` → `set_account_expired(true)`; uses the cache if there is one; warning `"Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan."`
      - `Corrupt` → `(None, ["Epic bağlantısı okunamadı; yeniden bağlan."])`
  - `pub fn disconnect(&self, db: &Db) -> bool`
    - Under `lock`: `generation += 1`, then `refresh_locked` → `api.kill`; the result is kept as `revoked`.
    - In every case `store.delete`, `owned_cache_delete` and `set_account_expired(false)` run.
    - Returns `revoked`.
  - `pub fn status(&self, db: &Db) -> AccountStatus`: does not go to the network.
    - If `load_cred` is `Ok(Some(c))`, `Connected` or `Expired` (depending on the flag) and `display_name: Some(c.display_name)`.
    - Otherwise `Disconnected`.
- `#[derive(Serialize, Debug, PartialEq)] #[serde(rename_all = "lowercase")] pub enum AccountState { Disconnected, Connected, Expired }`
- `#[derive(Serialize, Debug, PartialEq)] pub struct AccountStatus { pub platform: Platform, pub state: AccountState, pub display_name: Option<String> }`

**Test double** (`#[cfg(test)] struct FakeApi`, in the same file):
- Fields: `tokens: Mutex<VecDeque<Result<String, AuthError>>>` (token bodies returned in order), `library: Vec<String>` (page bodies; if the cursor is `"CUR<n>"` the n-th page is returned), `catalog: Result<String, AuthError>`, `kill: Result<(), AuthError>`.
- Counters: `token_calls: AtomicUsize`, `catalog_calls: AtomicUsize`, `seen_refresh: Mutex<Vec<String>>` (the `refresh_token` values in the token form).
- Optional hook: `on_library: Option<Box<dyn Fn() + Send + Sync>>` (runs during the library call; for race tests).
- The helper `fn tok(at: &str, rt: &str) -> String` produces a valid token body (`displayName: "oyuncu"`, `account_id: "acc"`).
- So that `EpicClient` can share an `Arc<MemoryStore>` in tests, `SecretStore` is also implemented for `Arc<T>` (`impl<T: SecretStore> SecretStore for Arc<T>`).

- [ ] **Step 1: Write the tests** (bodies according to these descriptions; asserts verbatim):

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

For `disconnect_wins_over_inflight_scan`, the `Db` is opened on a file under `std::env::temp_dir()` (two connections use the same file). The hook calls `disconnect` while `lock` is free; `fetch_owned` does not hold `lock` during the library call.

- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p hangar-core` → PASS (run the tests 3 times in a row; there must be no flakiness).
- [ ] **Step 5:** Commit: `feat(core): epic account flow (connect, refresh, scan, disconnect)`.
- [ ] **Step 6: Reviewer gate.** The reviewer reads this task's diff with these questions:
  - Is there a credential leak on any error path?
  - Is the lock order correct?
  - Is the old credential preserved on a save error?
  If there is a problem, T6 is not started.

---

### Task 6: Tauri commands and scan wiring

**Files:**
- Create: `src-tauri/src/accounts.rs`
- Modify: `src-tauri/src/commands.rs` (`scan`, `launch`), `src-tauri/src/main.rs`, `src-tauri/capabilities/default.json` (add `"dialog:allow-ask"`)

**Interfaces:**
- Consumes: T3b `login::login`/`LoginError`; T5c `EpicClient`, `AccountStatus`; T5a `DpapiStore`; T3a `open_error`.
- **`main.rs` setup:**
  - `#[cfg(windows)] let store: Box<dyn SecretStore> = Box::new(DpapiStore::new(dir.join("accounts")));`
  - `#[cfg(not(windows))] let store: Box<dyn SecretStore> = Box::new(MemoryStore::default());`
  - `app.manage(EpicClient::new(Box::new(HttpEpicApi), store));`
- **Commands (`accounts.rs`):**
  - `get_accounts(app) -> Result<Vec<AccountStatus>, String>`: `vec![client.status(&db)]`.
  - `connect_account(app, platform: Platform) -> Result<AccountStatus, String>`
    - `platform != Epic` → `Err("Desteklenmiyor")`.
    - `login` errors: `Cancelled` → `Err("cancelled")`, `Timeout` → `Err("Giriş zaman aşımına uğradı")`, `AlreadyOpen` → `Err("Giriş penceresi zaten açık")`, `Window(e)` → `Err(format!("Giriş penceresi açılamadı: {e}"))`.
    - Then `client.connect` inside `spawn_blocking` with its own `Db::open(DbPath)` connection; `status` on return.
  - `disconnect_account(app, platform) -> Result<bool, String>`: `client.disconnect` inside `spawn_blocking` with its own `Db` connection.
  - `EpicClient` is accessed with `app.state::<EpicClient>()`; since `State` is not `'static`, use `app.clone().state::<EpicClient>()` inside `spawn_blocking`.
- **`commands.rs::scan`:**
  - Currently the shared `Mutex<Db>` is locked briefly to read settings, and the network call is made outside the lock.
  - With the same pattern, right after the Steam `fetch_owned` and **before the shared lock is taken**, this runs: `let epic_db = Db::open(&app.state::<DbPath>().0).map_err(dberr)?; let (epic, epic_warn) = app.state::<EpicClient>().owned_for_scan(&epic_db);`
  - If `epic` is `Some(r)`, `owned.push((Platform::Epic, r))`; `extra.extend(epic_warn)`.
- **`commands.rs::launch`:** a URI open error returns via `.map_err(|e| open_error(&u, &e.to_string()))`.
- `accounts::get_accounts`, `accounts::connect_account` and `accounts::disconnect_account` are added to the `main.rs` `generate_handler!` list (in both cfg branches).

- [ ] **Step 1:** Implement. No tests in src-tauri since the logic is tested in core.
- [ ] **Step 2:** `cargo check -p hangar` and `cargo test -p hangar-core` → PASS.
- [ ] **Step 3:** Leak check, then commit: `feat(app): epic account commands and scan wiring`.

---

### Task 7: Settings › Accounts UI and loading line

**Files:**
- Create: `src/components/AccountsSection.tsx`, `src/components/AccountsSection.test.tsx`
- Modify: `src/lib/api.ts`, `src/components/SettingsDialog.tsx`, `src/components/SettingsDialog.test.tsx` (`getAccounts` in the mock), `src/state/useLibrary.ts`, `src/App.tsx`

**Interfaces:**
- **`api.ts`:**
  - `export interface AccountStatus { platform: Platform; state: "disconnected" | "connected" | "expired"; display_name: string | null }`
  - `getAccounts: () => invoke<AccountStatus[]>("get_accounts")`
  - `connectAccount: (platform: Platform) => invoke<AccountStatus>("connect_account", { platform })`
  - `disconnectAccount: (platform: Platform) => invoke<boolean>("disconnect_account", { platform })`
- **`AccountsSection`** (`{ onChanged: (label?: string) => Promise<void> }`):
  - On open `getAccounts` is called; only the `platform === "epic"` row is shown.
  - Row heading "Epic Games".
  - **Status text:** "Bağlı değil" / `"Bağlı · " + display_name` / "Süresi doldu".
  - **Button:** "Bağlan" / "Bağlantıyı kes" / "Yeniden bağlan". While waiting the text becomes "Epic penceresi açık…" and the button is disabled.
  - Two fixed `<small>` lines at the bottom: "Giriş Epic'in kendi sayfasında yapılır; Hangar şifreni görmez." and "Bağlantı belgesi yalnızca bu bilgisayarda, şifreli tutulur."
  - **Connect:**
    - Success → the status is updated and `onChanged("Epic oyunları yükleniyor…")` is called.
    - Error `"cancelled"` → nothing is shown.
    - Other errors → the text is shown with `role="alert"`.
  - **Disconnect:**
    - `ask("Epic bağlantısı silinecek ve Epic'teki oturum kapatılacak; kurulu oyunlar kalır.", { title: "Bağlantıyı kes", kind: "warning" })` (`@tauri-apps/plugin-dialog`).
    - If `true`, `disconnectAccount`. If the result is `false`, "Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin." is shown with `role="status"`.
    - Then the status is refreshed and `onChanged()` is called.
- **`SettingsDialog`:**
  - New prop: `onAccountsChanged?: (label?: string) => Promise<void>`.
  - `<AccountsSection onChanged={onAccountsChanged ?? (async () => {})} />` is added below the "Oyun klasörleri" section.
- **`useLibrary`:**
  - `rescan` stays as is.
  - New: `const [scanLabel, setScanLabel] = useState<string | null>(null)` and `rescanLabeled = useCallback(async (label: string) => { setScanLabel(label); try { await rescan(); } finally { if (mounted.current) setScanLabel(null); } }, [rescan])`.
  - Both are returned.
- **`App.tsx`:**
  - `{rescanning && <span className="scan" role="status">{scanLabel ?? "Taranıyor…"}</span>}`
  - `<SettingsDialog … onAccountsChanged={(l) => (l ? rescanLabeled(l) : rescan())} />`

- [ ] **Step 1: `AccountsSection.test.tsx`.** Written with `vi.mock("../lib/api")` and `vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }))`:
  - `connects and triggers labeled rescan`: disconnected → "Bağlan" is clicked → `connectAccount("epic")` is called and `onChanged("Epic oyunları yükleniyor…")` is called; then "Bağlı · oyuncu" appears.
  - `cancel is silent`: `connectAccount` rejects with `"cancelled"` → `queryByRole("alert")` returns null and `onChanged` is not called.
  - `other error shows alert`: `"Giriş zaman aşımına uğradı"` → the alert contains this text.
  - `disconnect asks first`: if `ask` → false, `disconnectAccount` is not called. If `ask` → true and `disconnectAccount` → true, `onChanged()` is called with no argument.
  - `server revoke failure is shown`: if `disconnectAccount` → false, status shows the text "Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin.".
  - `expired shows reconnect`: state expired → the "Yeniden bağlan" button and the text "Süresi doldu".
- [ ] **Step 2:** `npm test` → FAIL.
- [ ] **Step 3:** Implement; use the existing `fld`, `row`, `ghost` classes, no new CSS.
- [ ] **Step 4:** `npm test` and `npx tsc --noEmit` → PASS.
- [ ] **Step 5:** Commit: `feat(ui): accounts section in settings`.

---

### Task 8: README, manual verification, release smoke test

**Files:** Modify `README.md`. The reviewer updates `CLAUDE.md`; a subagent does not.

- [ ] **Step 1: README.**
  - The `accounts\` folder is added to the "Veri konumu" section.
  - A new "Hesap bağlantıları (Epic)" section is written. Ubisoft is not mentioned. Its content:
    - **What is stored:** only the refresh credential and the display name, in the file `%APPDATA%\com.hangar.app\accounts\epic.dat`. Encrypted with Windows DPAPI; only this Windows user can decrypt it.
    - **How to delete it:** "Bağlantıyı kes" both deletes the file and closes the session on Epic. The file can also be deleted by hand.
    - **How it appears on Epic:** Hangar uses the Epic Games Launcher's public client id. So on the Epic account page the connection appears as "Epic Games Launcher".
    - **Unofficial service:** Epic may block this route; then only installed Epic games are shown.
    - **Terms of use:** this route may be considered a violation of Epic's terms of use. In the worst case the account may be flagged. Legendary, Heroic and Playnite use the same route, but that is not a guarantee.
    - **Scope of the credential:** the credential has access to the whole account. DPAPI does not protect against malware running as the same user.
    - **No server:** Hangar has no server; data goes only to Epic.
    - **If you do not want a persistent credential:** disconnect when you are done.
- [ ] **Step 2:** All tests PASS: `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`.
- [ ] **Step 3:** Release smoke test: `npm run tauri build` succeeds and `grep -c debug_epic_probe target/release/hangar.exe` → `0`.
- [ ] **Step 4: Manual check (with the user, `npm run tauri dev`):**
  1. Settings › Accounts › "Bağlan": the window title shows "Epic Games girişi — www.epicgames.com"; after login the window closes on its own; status "Bağlı · <name>".
  2. "Epic oyunları yükleniyor…" appears at the top, then uninstalled Epic games arrive with "Kur" cards.
  3. Death's Door is still a single card and "Oyna" works.
  4. After closing and reopening the app, no login is requested, status "Bağlı".
  5. On an uninstalled Epic game, "Kur" opens that game's page or its installation in the Epic launcher.
  6. With Wi-Fi off, "Yeniden tara" shows the last known list and the warning "Epic oyunları güncellenemedi; son bilinen liste gösteriliyor.".
  7. "Bağlantıyı kes" asks for confirmation, then only installed Epic games remain. `%APPDATA%\com.hangar.app\accounts\epic.dat` is gone; the session is signed out on the epicgames.com account page.
  8. The `EBWebView` folder exists and contains no `epicgames` cookie (with the command from E0).
- [ ] **Step 5:** Commit: `docs: security note for the epic account connection`.
