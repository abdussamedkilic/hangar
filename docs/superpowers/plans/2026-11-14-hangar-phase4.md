# Phase 4 — Steam/GOG login, filters, collapsible menu, drag-and-drop, tabbed Settings · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect Steam with a login that needs no API key and GOG with an account; also named filters, a collapsible sidebar, drag-and-drop onto a collection tab and a two-tab Settings.

**Architecture:**
- **Accounts (Track A, separate worktree `feat/phase4-accounts`):**
  - In the core, `GogClient` (Epic pattern) and `SteamAccount` (credential file + web game list parsing).
  - In the app, a Steam window with a persistent WebView2 profile (`login_flow_with`, written in S0), an invisible Steam fetch during scans, account rows.
- **UI (Track B, main tree `feat/phase4`):**
  - The `filters` table and rule evaluation (`library.ts`).
  - Sidebar, drag-and-drop, Settings tabs.
- **Merge:** The tracks are merged at the end. B4 (Settings) is done after the merge.

**Tech Stack:** Rust (ureq, serde_json, rusqlite), Tauri 2.12 (WebviewWindowBuilder `data_directory`, `visible`), React 18 + TS, vitest.

**Spec:** `docs/superpowers/specs/2026-11-14-hangar-phase4-design.md`. The sections "Design finalized by experiments" and "Clarified after the founder's check" are binding.

## Global Constraints
- **Secrets:**
  - The token, cookies, SteamID and the Steam web access key (`data-loyalty_webapi_token`) are never logged, printed, put into error text or written to disk in plain text.
  - Persistent data is kept with the DPAPI `SecretStore`:
    - `accounts\steam.dat` = `{v:1, steamid, display_name}`
    - `accounts\gog.dat` = `{v:1, refresh_token, user_id, display_name}`
- **GOG client details:** client_id `46899977096215655`, client_secret `9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9`, redirect `https://embed.gog.com/on_login_success?origin=client`.
- **Steam profile folder:** `app_data_dir/webview-steam`. It is not incognito.
- **Preference keys:**
  - The whitelist is `ui.view`, `ui.tab`, `ui.installed`, `ui.filter`, `ui.sidebar`.
  - `ui.sidebar` = `"open"|"collapsed"` (default `open`).
  - `ui.filter` = `""` | `"installed"` | `"<filter id>"`.
- **Setting:** `gog.source` = `"galaxy"|"account"` (default `galaxy`).
- **UI:** Turkish copy. Token colors only. The design-taste-frontend and ui-ux-pro-max rules are followed.
- **Commits and tests:**
  - Commits are English conventional and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push.
  - vitest runs with node: `PATH="$APPDATA/fnm/node-versions/v26.13.0/installation:$PATH" node node_modules/vitest/vitest.mjs run`.
  - tsc with the same node: `node node_modules/typescript/bin/tsc --noEmit`.
- **File ownership:**
  - Track A: `crates/hangar-core/src/accounts/{gog.rs,gog_api.rs,steam.rs,login.rs,store.rs,mod.rs}`, `providers/gog.rs`, `artwork.rs`, `src-tauri/src/{accounts.rs,commands.rs,login.rs}`, `src/components/AccountsSection*`, `tauri.conf.json`, README.
  - Track B: `db.rs`, `src-tauri/src/collections.rs`, `src/lib/library.ts`, `src/state/useLibrary.ts`, `Sidebar`, `TabStrip`, `GameGrid`, `GameList`, `App.tsx`, `app.css`, new filter components.
  - Shared `main.rs` and `api.ts`: each track only adds lines.

## Review Focus
1. **Steam session dropped, user online:** The invisible window gives an explicit "no login" signal within 20 s. The account becomes "expired", the cache stays visible and the app does not freeze. → A3 test `steam_scan_nologin_marks_expired`.
2. **No network or Steam slow:** The 20 s timeout gives only a "could not be updated" warning; the account is not considered dropped. → A3 test `steam_scan_timeout_is_offline`.
3. **Focus scans:** The Steam window is not opened on every return to the window; a 10-minute limit applies, "Yenile" skips the limit. → A3 test `steam_auto_throttle_10min`.
4. **A filter refers to a deleted collection:** The criterion is ignored, the list does not become empty. → B1 test `rule with deleted collection ignores criterion`.
5. **Collapsed menu:** All actions are reachable via icons and a tooltip is shown; the active filter is marked on its icon. → B2 test `collapsed sidebar keeps all actions reachable`.

---

## Track A — Accounts (worktree `C:\Users\user\workdir\hangar-acc`, branch `feat/phase4-accounts`)

### Task A1: GOG account client (core)
**Files:**
- Create: `crates/hangar-core/src/accounts/gog_api.rs`, `crates/hangar-core/src/accounts/gog.rs`
- Modify: `accounts/mod.rs` (`GogCred` + modules), `accounts/store.rs` (`load_gog_cred` / `save_gog_cred`, Epic pattern), `accounts/login.rs` (`GOG_NAV_RULES`, `gog_login_url()`, `gog_init_script(nonce)`, `gog_window_title`; values from `gog_probe.rs` in G0), `artwork.rs` (allow: `images.gog-statics.com` and `images-<1..9>.gog-statics.com`), `providers/gog.rs`

**Interfaces:**
- Produces:
  - `pub trait GogApi: Send+Sync { fn exchange(&self, code:&str)->Result<String,AuthError>; fn refresh(&self, rt:&str)->Result<String,AuthError>; fn filtered_page(&self, token:&str, page:u32)->Result<String,AuthError>; }` and `HttpGogApi`.
  - `pub struct GogClient` — `new(Box<dyn GogApi>, Box<dyn SecretStore>)`, `connect(&Db, code:&str)->Result<String,AuthError>`, `owned_for_scan(&Db)->(Option<Result<Vec<PlatformEntry>,ProviderError>>, Vec<String>)`, `disconnect(&Db)->bool`, `status(&Db)->AccountStatus`. The rotation, pending, generation, Debug-redaction and zeroize pattern from EpicClient is applied as is.
  - `pub fn parse_filtered(json:&str)->Result<(Vec<PlatformEntry>, u32 /*totalPages*/), AuthError>`. Entry: platform Gog, platform_id = `id.to_string()`, title, `installed:false`, launch_uri = install_uri = `goggalaxy://openGameView/<id>`, cover = `https:` + image + `_392.jpg`.
  - `GogProvider`: new `pub fn with_owned_from_db(self, on: bool) -> Self`. When `false`, it does not produce owned-but-uninstalled entries from the Galaxy DB.
- WARN texts are produced by writing "GOG" instead of "Epic" in the Epic texts.

- [ ] **Step 1: Failing tests (`gog.rs`)**
  - `parse_filtered_maps_products`: fixture = 2 products hand-trimmed from the sample `%APPDATA%\com.hangar.app\g0\gog-filtered-page1.json`, `crates/hangar-core/tests/fixtures/gog/filtered.json`. Expected: id "2099051765", title "9 Years of Shadows", cover `https://images-3.gog-statics.com/346e…_392.jpg`, totalPages 1.
  - `connect_stores_refresh_token` — token response `{"access_token":"A","refresh_token":"R","expires_in":3600,"user_id":"U"}`.
  - `refresh_keeps_same_token`
  - `invalid_grant_marks_expired`
  - `offline_returns_cache`
  - `disconnect_during_scan_discards`
  - `pages_are_concatenated` — 2 pages.
  - `debug_redacts`
- [ ] **Step 2:** Artwork test: `images-3.gog-statics.com` is allowed, `images-x.evil.com` is not allowed. `providers/gog.rs` test: `with_owned_from_db(false)` drops uninstalled Galaxy entries.
- [ ] **Step 3:** `cargo test -p hangar-core gog` → FAIL. Implement → PASS. Then `cargo test -p hangar-core` → PASS.
- [ ] **Step 4:** Verify the id match (report only). Check that the first 5 ids in the g0 output are found in the Galaxy DB as `gog_<id>`, with a read-only SQL query on a copy of the DB. Write the result in the report.
- [ ] **Step 5: Commit**
  - `feat(core): GOG account client`

### Task A2: Steam account core (core)
**Files:**
- Create: `crates/hangar-core/src/accounts/steam.rs`
- Modify: `accounts/mod.rs` (`SteamCred`), `accounts/store.rs` (`load_steam_cred` / `save_steam_cred`), `accounts/login.rs` (`STEAM_NAV_RULES`, `steam_login_url()` = `https://store.steampowered.com/login/?redir=`, `steam_init_script(nonce)`)

**Interfaces:**
- Produces:
  - `pub enum SteamWeb { Games{steamid:String, display_name:String, games_json:String}, NoLogin, Failed(String) }`
  - `pub fn parse_delivery(code:&str)->SteamWeb` — decodes the JSON the script delivers.
  - `pub fn games_to_entries(games_json:&str)->Result<Vec<PlatformEntry>, ProviderError>` — converts the `{"response":{"games":[…]}}` shape with the existing `providers::steam::parse_owned_games`.
  - `pub struct SteamAccount { store }`: `new(Box<dyn SecretStore>)`, `save(&SteamCred)`, `load()->Result<Option<SteamCred>,AuthError>`, `forget()`, `status(&Db)->AccountStatus` (expired flag from the db).
  - `steam_init_script` is the production version of the S0 script:
    - No diagnostics and no cookie watcher.
    - Logged-in signal: application_config / `g_steamID` / `g_AccountID`.
    - `IPlayerService/GetOwnedGames/v1/?include_appinfo=1&include_played_free_games=1` is called from the same page with the token.
    - Delivery: `{"games":<response>,"steamid":..,"name":..}`, `{"nologin":true}` if not logged in, `{"failed":"<short>"}` on error.
    - Every path delivers within 20 s. The token is never delivered.
- [ ] **Step 1: Failing tests**
  - `parse_delivery_games` / `parse_delivery_nologin` / `parse_delivery_failed`
  - `games_to_entries_maps_playtime`: appid 620, `playtime_forever` 600, `rtime_last_played` 1700000000 → playtime_min 600, last_played Some.
  - `cred_roundtrip_dpapi_memory`
  - `script_never_contains_token_in_delivery`: verifies, as text, that the script text's delivery object does not contain an `access_token` or `webapi_token` key.
- [ ] **Step 2:** FAIL → implement → `cargo test -p hangar-core` PASS.
- [ ] **Step 3: Commit**
  - `feat(core): Steam account core and login script`

### Task A3: App wiring — Steam/GOG connect, scan, account rows
**Files:**
- Modify: `src-tauri/src/accounts.rs`, `src-tauri/src/commands.rs` (scan), `src-tauri/src/login.rs` (`login_flow_with` stays from S0; only the S0 debug command is removed), `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json` (`"dragDropEnabled": false` on the window), `src/components/AccountsSection.tsx` (+test), `src/lib/api.ts`, README
- Delete: `src-tauri/src/probe_steam.rs`, `src-tauri/src/probe_gog.rs`, `crates/hangar-core/src/accounts/{steam_probe.rs,gog_probe.rs}` and their registrations.

**Interfaces:**
- Consumes: A1 and A2.
- Produces:
  - **`connect_account`:**
    - Steam: visible window, `FlowOpts{data_dir: Some(webview-steam), hidden:false}`. If `Games` arrives, the cred is saved, expired is cleared and the games are written to the cache with `owned_cache_put`.
    - Gog: `login_flow` + `GogClient::connect`.
  - **`disconnect_account`:** Steam first closes the Steam window if it is open; then deletes the `webview-steam` folder and the cred, and clears the cache.
  - **`get_accounts`:** returns `[steam, epic, gog, ubisoft]`.
  - **New commands:**
    - `get_gog_source() -> String`, `set_gog_source(source)` — the `gog.source` setting, only `galaxy|account`.
    - `get_steam_advanced() -> {api_key, steam_id}`, `set_steam_advanced(api_key, steam_id)`. These are the moved version of the existing fields in SettingsDialog; the existing setting keys are used.
  - **Scan:**
    - If there is no Steam key and Steam is connected: `fetch_steam_hidden(app, force:bool) -> SteamWeb`. An invisible window is opened, with a 20 s timeout.
    - `NoLogin` → expired + `WARN_EXPIRED` + cache. `Failed`/timeout → `WARN_OFFLINE` + cache.
    - If `force=false` and the last successful fetch is newer than 10 minutes, the fetch is skipped and the cache is used.
    - `rescan` is called with `force=true` from "Yenile" and with `false` from the focus scan. A `force: Option<bool>` parameter is added to the `rescan` command; on the front end the `useLibrary` focus call passes `false`.
    - GOG: if `gog.source == "account"`, `GogClient::owned_for_scan` is called and `GogProvider::with_owned_from_db(false)` is used.
  - **`AccountsSection`:**
    - Four rows, in the existing row pattern.
    - A collapsible "Gelişmiş" area in the Steam row: API key and SteamID fields + "Kaydet". Text: "Anahtar girilirse giriş yerine anahtar kullanılır."
    - A "Kaynak" choice in the GOG row: "Galaxy veritabanı" / "GOG hesabı". When "GOG hesabı" is selected the Connect button is visible.
- [ ] **Step 1: Failing tests**
  - Rust (pure helpers, in `commands.rs` or a new `steam_scan.rs`):
    - `steam_scan_nologin_marks_expired`
    - `steam_scan_timeout_is_offline`
    - `steam_auto_throttle_10min`
    - All via pure functions like `fn steam_scan_decision(last_ok: Option<i64>, now:i64, force:bool) -> bool` and `fn steam_outcome(r: Option<SteamWeb>) -> (Option<Result<..>>, Vec<String>, expired:Option<bool>)`.
  - vitest:
    - `AccountsSection` shows the four rows in the order Steam, Epic, GOG, Ubisoft.
    - The Steam "Gelişmiş" area saves (`setSteamAdvanced` is called).
    - The GOG source choice calls `setGogSource("account")`.
- [ ] **Step 2:** FAIL → implement → PASS. Commands: `cargo test -p hangar-core`, `cargo test -p hangar`, `cargo check -p hangar --release`, vitest, tsc.
- [ ] **Step 3:** Add to the README:
  - **Steam login:** the persistent profile folder, that it is deleted when the connection is cut, that the key is optional.
  - **GOG login:** Galaxy's public client and the unofficial route warning.
- [ ] **Step 4: Commit**
  - `feat(app): Steam and GOG account connection`

## Track B — UI (main tree `C:\Users\user\workdir\hangar`, branch `feat/phase4`)

### Task B1: Filter data and evaluation
**Files:**
- Modify: `crates/hangar-core/src/db.rs` (`filters` table), `src-tauri/src/collections.rs` (commands + whitelist `ui.filter`, `ui.sidebar`), `src-tauri/src/main.rs`, `src/lib/api.ts`, `src/lib/library.ts` (+test), `src/state/useLibrary.ts`

**Interfaces:**
- Produces:
  - Rust:
    - `filters(id INTEGER PK AUTOINCREMENT, name TEXT NOT NULL, rule TEXT NOT NULL, position INTEGER NOT NULL)`
    - `Db::filters()->Result<Vec<SavedFilter{id,name,rule}>>`
    - `create_filter(name, rule)->Result<Result<i64,NameError>>` — name uniqueness with `fold_name` from collections.
    - `update_filter(id, name, rule)`, `delete_filter(id)`
  - Commands: `get_filters`, `create_filter`, `update_filter`, `delete_filter`. API: `getFilters()`, `createFilter(name, rule)`, `updateFilter(id, name, rule)`, `deleteFilter(id)`. The rule is stored in TS as a JSON string.
  - TS:
    - `export interface FilterRule { stores?: Platform[]; installed?: "yes"|"no"; played?: "yes"|"no"; edition?: "original"|"special"; collection?: { id:number; in:boolean } }`
    - `export interface SavedFilter { id:number; name:string; rule: FilterRule }`
    - `export type ActiveFilter = { kind:"none" } | { kind:"installed" } | { kind:"saved"; id:number }`
    - `export function matchesRule(g:Game, r:FilterRule, t:Tab, cols:Collection[]):boolean`
      - stores: OR.
      - installed: `installedIn(g,t)`.
      - played: any entry has `playtime_min > 0` or `last_played`, or `g.last_launched` is filled.
      - edition: `entries.some(e=>e.edition)` → special edition.
      - collection: `inCollection`. If the collection does not exist the criterion is ignored.
    - `view(games, tab, filter:ActiveFilter, saved:SavedFilter[], cols, q, sort)` — replaces the `installedOnly` parameter. `tabCounts` and `installedCount` are updated the same way. For tab visibility `tabCounts(games, NONE, …)` is used.
  - **useLibrary:**
    - `filter`, `setFilter(f:ActiveFilter)` (writes to `ui.filter`), `savedFilters`, `reloadFilters()`. `installedOnly` and `setInstalledOnly` are removed.
    - Migration: on startup if `ui.filter` is empty and `installed` is true, `filter = installed` and `ui.filter = "installed"` is written.
    - If `ui.filter` points to a deleted filter it becomes `none`.
- [ ] **Step 1: Failing tests (library.test.ts)**
  - `played rule counts steam playtime and hangar launches`
  - `stores rule is OR`
  - `rule with deleted collection ignores criterion`
  - `edition special vs original`
  - `installed rule respects store tab` (the Phase 3 Watch_Dogs example)
- [ ] **Step 2: Failing tests**
  - Rust `filters_crud` + `filter_names_turkish_fold`.
  - useLibrary: migration test and `setFilter` write test.
- [ ] **Step 3:** FAIL → implement → all tests PASS. Make the smallest adaptation needed for App and Sidebar to compile; the real UI comes in B2. This adaptation: the Installed button toggles the `filter.kind === "installed"` state.
- [ ] **Step 4: Commit**
  - `feat(ui): savable filters (data and evaluation)`

### Task B2: Collapsible sidebar + filter list and editor + nudge
**Files:**
- Modify: `src/components/Sidebar.tsx` (+test), `src/App.tsx`, `src/styles/app.css`, `src/state/useLibrary.ts` (`sidebar`, `setSidebar` → `ui.sidebar`)
- Create: `src/components/FilterEditor.tsx` (+test), `src/components/icons.tsx`. Icons are inline SVG: filter, installed, add, refresh, settings, collapse/expand.

**Interfaces:**
- Consumes: B1.
- Produces:
  - `Sidebar` props: `{ collapsed, onToggleCollapsed, filter, savedFilters, counts:Record<string,number>, onFilter(f), onNewFilter(), onFilterContext(f,x,y), onAdd, onRescan, onSettings, rescanning }`
  - `FilterEditor` props: `{ initial?: SavedFilter, collections, onSave(name, rule): Promise<void>, onClose }`
    - Uses a Modal.
    - Criteria: store checkboxes; a three-state choice for installed, played and edition; collection choice + in/out.
    - Note text: "Oynama süresi yalnız Steam'de var; diğerlerinde Hangar'dan başlatılanlar sayılır."
- [ ] **Step 1: Failing tests**
  - `collapsed sidebar keeps all actions reachable`: when collapsed every button has an `aria-label` and is clickable.
  - `active filter toggles off when clicked again`
  - `Kurulu is builtin and has no context menu`
  - `FilterEditor saves rule`
  - Nudge: if Steam is not connected and there is no key, the text "Steam'e bağlan" is visible and clicking it opens Settings. This is updated in the App test.
- [ ] **Step 2:** FAIL → implement → PASS.
  - Width: open 220px, collapsed 64px; the transition respects `prefers-reduced-motion`.
  - The tooltip is given with `title` + `aria-label`.
  - When collapsed, filters are shown with an initial-letter badge.
- [ ] **Step 3: Visual check (the controller does it):**
  - Screenshots of the open and collapsed states are taken in the Tauri window.
  - They are evaluated against the design-taste-frontend rules: alignment, equal edges, icon size consistency, focus visibility.
  - Findings are fixed.
- [ ] **Step 4: Commit**
  - `feat(ui): collapsible menu and filters`

### Task B3: Drag-and-drop onto a collection tab
**Files:**
- Modify: `src/components/GameCard.tsx`, `src/components/GameList.tsx`, `src/components/TabStrip.tsx` (+tests), `src/App.tsx`, `src/styles/app.css`

**Interfaces:**
- Produces:
  - The card and list row become `draggable`. `dataTransfer.setData("application/x-hangar-game", String(g.id))`.
  - TabStrip collection tab: `onDropGame(collectionId:number, gameId:number)`. On `dragover` only this type is accepted and the `.drop-ok` class is added to the tab.
  - App:
    - If the card is already in the collection the notice is "zaten <name> içinde".
    - Otherwise `setInCollection(id, cardEntries(g), true)` + `reloadCollections()` is called and the notice is "<name> koleksiyonuna eklendi".
    - The short status text is cleared after 3 s.
- [ ] **Step 1: Failing tests**
  - TabStrip: dropping a card onto a collection tab calls `onDropGame(7, 42)`; dropping onto a store tab does nothing.
  - GameCard: `dragstart` puts the correct data and clicking does not disturb the selection.
- [ ] **Step 2:** FAIL → implement → PASS. It is checked live that drag-and-drop works; `dragDropEnabled:false` from A3 takes effect after the merge.
- [ ] **Step 3: Commit**
  - `feat(ui): drag a card onto a collection tab`

### Task B4: Settings two tabs (after Track A is merged)
**Files:**
- Modify: `src/components/SettingsDialog.tsx` (+test), `src/styles/app.css`

**Interfaces:**
- Consumes: A3 `AccountsSection` (including Steam "Gelişmiş"), Phase 3 theme buttons, game folders.
- Produces:
  - Vertical tabs on the left: "Hesaplar", "Özelleştir".
  - Hesaplar = `AccountsSection`.
  - Özelleştir:
    - Theme: "Tema dosyasını aç", "Varsayılana dön".
    - Menu: "Açılışta menü: Açık / Daraltılmış" → `ui.sidebar`.
    - Game folders: the existing area.
  - The Steam key and SteamID fields are removed from SettingsDialog (now in Hesaplar › Steam › Gelişmiş).
  - "Kaydet" stays only where needed: game folders and Steam advanced.
- [ ] **Step 1: Failing tests:** there are two tabs; Özelleştir has three sections; Hesaplar shows four rows.
- [ ] **Step 2:** FAIL → implement → all tests PASS.
- [ ] **Step 3: Commit**
  - `feat(ui): settings accounts and customize tabs`

## Order
- A1 → A2 → A3, in the worktree.
- B1 → B2 → B3, in the main tree. The two tracks proceed in parallel.
- Then `feat/phase4-accounts` is merged, then B4.
- Finally an Opus review of the whole branch, a single fix round and a live test with the user (the 6 steps in the spec).
