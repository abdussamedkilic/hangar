# Faz 4 — Steam/GOG girişi, filtreler, daraltılabilir menü, sürükle-bırak, bölümlü Ayarlar · Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Steam'i API anahtarsız girişle ve GOG'u hesapla bağlamak; ayrıca adlandırılmış filtreler, daralabilen kenar çubuğu, koleksiyon sekmesine sürükle-bırak ve iki sekmeli Ayarlar.

**Architecture:**
- **Hesaplar (İz A, ayrı worktree `feat/phase4-accounts`):**
  - Çekirdekte `GogClient` (Epic kalıbı) ve `SteamAccount` (kimlik dosyası + web oyun listesi ayrıştırma).
  - Uygulamada kalıcı WebView2 profilli Steam penceresi (`login_flow_with`, S0'da yazıldı), tarama sırasında görünmez Steam alımı, hesap satırları.
- **Arayüz (İz B, ana ağaç `feat/phase4`):**
  - `filters` tablosu ve kural değerlendirmesi (`library.ts`).
  - Kenar çubuğu, sürükle-bırak, Ayarlar sekmeleri.
- **Birleştirme:** İzler sonda birleşir. B4 (Ayarlar) birleştirmeden sonra yapılır.

**Tech Stack:** Rust (ureq, serde_json, rusqlite), Tauri 2.12 (WebviewWindowBuilder `data_directory`, `visible`), React 18 + TS, vitest.

**Spec:** `docs/superpowers/specs/2026-11-14-hangar-phase4-design.md`. "Denemelerle kesinleşen tasarım" ve "Kurucu kontrolünden sonra netleşenler" bölümleri bağlayıcıdır.

## Global Constraints
- **Gizli bilgi:**
  - Token, çerez, SteamID ve Steam web erişim anahtarı (`data-loyalty_webapi_token`) asla loglanmaz, yazdırılmaz, hata metnine girmez ve diske düz yazılmaz.
  - Kalıcı veri DPAPI `SecretStore` ile tutulur:
    - `accounts\steam.dat` = `{v:1, steamid, display_name}`
    - `accounts\gog.dat` = `{v:1, refresh_token, user_id, display_name}`
- **GOG istemci bilgileri:** client_id `46899977096215655`, client_secret `9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9`, redirect `https://embed.gog.com/on_login_success?origin=client`.
- **Steam profil klasörü:** `app_data_dir/webview-steam`. Gizli (incognito) değildir.
- **Tercih anahtarları:**
  - Beyaz liste `ui.view`, `ui.tab`, `ui.installed`, `ui.filter`, `ui.sidebar` olur.
  - `ui.sidebar` = `"open"|"collapsed"` (varsayılan `open`).
  - `ui.filter` = `""` | `"installed"` | `"<filter id>"`.
- **Ayar:** `gog.source` = `"galaxy"|"account"` (varsayılan `galaxy`).
- **Arayüz:** Türkçe metin. Yalnız token renkleri. design-taste-frontend ve ui-ux-pro-max kurallarına uyulur.
- **Commit ve testler:**
  - Commit Türkçe conventional olur ve `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` ile biter. Push yapılmaz.
  - vitest node ile çalıştırılır: `PATH="$APPDATA/fnm/node-versions/v26.13.0/installation:$PATH" node node_modules/vitest/vitest.mjs run`.
  - tsc aynı node ile: `node node_modules/typescript/bin/tsc --noEmit`.
- **Dosya sahipliği:**
  - İz A: `crates/hangar-core/src/accounts/{gog.rs,gog_api.rs,steam.rs,login.rs,store.rs,mod.rs}`, `providers/gog.rs`, `artwork.rs`, `src-tauri/src/{accounts.rs,commands.rs,login.rs}`, `src/components/AccountsSection*`, `tauri.conf.json`, README.
  - İz B: `db.rs`, `src-tauri/src/collections.rs`, `src/lib/library.ts`, `src/state/useLibrary.ts`, `Sidebar`, `TabStrip`, `GameGrid`, `GameList`, `App.tsx`, `app.css`, yeni filtre bileşenleri.
  - Ortak `main.rs` ve `api.ts`: her iz yalnız satır ekler.

## Review Focus
1. **Steam oturumu düşmüş, kullanıcı ağda:** Görünmez pencere 20 sn içinde açık "giriş yok" sinyali verir. Hesap "süresi doldu" olur, önbellek görünür ve uygulama donmaz. → A3 testi `steam_scan_nologin_marks_expired`.
2. **Ağ yok ya da Steam yavaş:** 20 sn zaman aşımı yalnız "güncellenemedi" uyarısı verir; hesap düşmüş sayılmaz. → A3 testi `steam_scan_timeout_is_offline`.
3. **Odak taramaları:** Pencereye her dönüşte Steam penceresi açılmaz; 10 dakika sınırı uygulanır, "Yenile" sınırı atlar. → A3 testi `steam_auto_throttle_10min`.
4. **Filtre silinmiş koleksiyona bakıyor:** Ölçüt yok sayılır, liste boşalmaz. → B1 testi `rule with deleted collection ignores criterion`.
5. **Daraltılmış menü:** Tüm eylemlere ikonla ulaşılır ve ipucu görünür; etkin filtre ikonda işaretlidir. → B2 testi `collapsed sidebar keeps all actions reachable`.

---

## İz A — Hesaplar (worktree `C:\Users\user\workdir\hangar-acc`, dal `feat/phase4-accounts`)

### Task A1: GOG hesap istemcisi (core)
**Files:**
- Create: `crates/hangar-core/src/accounts/gog_api.rs`, `crates/hangar-core/src/accounts/gog.rs`
- Modify: `accounts/mod.rs` (`GogCred` + modüller), `accounts/store.rs` (`load_gog_cred` / `save_gog_cred`, Epic kalıbı), `accounts/login.rs` (`GOG_NAV_RULES`, `gog_login_url()`, `gog_init_script(nonce)`, `gog_window_title`; G0'daki `gog_probe.rs` değerleri), `artwork.rs` (izin: `images.gog-statics.com` ve `images-<1..9>.gog-statics.com`), `providers/gog.rs`

**Interfaces:**
- Produces:
  - `pub trait GogApi: Send+Sync { fn exchange(&self, code:&str)->Result<String,AuthError>; fn refresh(&self, rt:&str)->Result<String,AuthError>; fn filtered_page(&self, token:&str, page:u32)->Result<String,AuthError>; }` ve `HttpGogApi`.
  - `pub struct GogClient` — `new(Box<dyn GogApi>, Box<dyn SecretStore>)`, `connect(&Db, code:&str)->Result<String,AuthError>`, `owned_for_scan(&Db)->(Option<Result<Vec<PlatformEntry>,ProviderError>>, Vec<String>)`, `disconnect(&Db)->bool`, `status(&Db)->AccountStatus`. EpicClient'taki rotation, pending, generation, Debug-redaction ve zeroize kalıbı aynen uygulanır.
  - `pub fn parse_filtered(json:&str)->Result<(Vec<PlatformEntry>, u32 /*totalPages*/), AuthError>`. Kayıt: platform Gog, platform_id = `id.to_string()`, title, `installed:false`, launch_uri = install_uri = `goggalaxy://openGameView/<id>`, cover = `https:` + image + `_392.jpg`.
  - `GogProvider`: yeni `pub fn with_owned_from_db(self, on: bool) -> Self`. `false` iken Galaxy DB'den sahip olunan ama kurulu olmayan kayıtları üretmez.
- WARN metinleri Epic metinlerinde "Epic" yerine "GOG" yazılarak üretilir.

- [ ] **Step 1: Failing tests (`gog.rs`)**
  - `parse_filtered_maps_products`: fixture = `%APPDATA%\com.hangar.app\g0\gog-filtered-page1.json` örneğinden elle küçültülmüş 2 ürün, `crates/hangar-core/tests/fixtures/gog/filtered.json`. Beklenen: id "2099051765", title "9 Years of Shadows", cover `https://images-3.gog-statics.com/346e…_392.jpg`, totalPages 1.
  - `connect_stores_refresh_token` — token yanıtı `{"access_token":"A","refresh_token":"R","expires_in":3600,"user_id":"U"}`.
  - `refresh_keeps_same_token`
  - `invalid_grant_marks_expired`
  - `offline_returns_cache`
  - `disconnect_during_scan_discards`
  - `pages_are_concatenated` — 2 sayfa.
  - `debug_redacts`
- [ ] **Step 2:** Artwork testi: `images-3.gog-statics.com` izinli, `images-x.evil.com` izinsiz. `providers/gog.rs` testi: `with_owned_from_db(false)` kurulu olmayan Galaxy kayıtlarını atar.
- [ ] **Step 3:** `cargo test -p hangar-core gog` → FAIL. Uygula → PASS. Ardından `cargo test -p hangar-core` → PASS.
- [ ] **Step 4:** Kimlik eşleşmesini doğrula (yalnız rapor). g0 çıktısındaki ilk 5 id'nin Galaxy DB'de `gog_<id>` olarak bulunduğunu, salt okunur bir SQL sorgusuyla ve DB'nin kopyası üzerinde kontrol et. Sonucu rapora yaz.
- [ ] **Step 5: Commit**
  - `feat(core): GOG hesap istemcisi`

### Task A2: Steam hesap çekirdeği (core)
**Files:**
- Create: `crates/hangar-core/src/accounts/steam.rs`
- Modify: `accounts/mod.rs` (`SteamCred`), `accounts/store.rs` (`load_steam_cred` / `save_steam_cred`), `accounts/login.rs` (`STEAM_NAV_RULES`, `steam_login_url()` = `https://store.steampowered.com/login/?redir=`, `steam_init_script(nonce)`)

**Interfaces:**
- Produces:
  - `pub enum SteamWeb { Games{steamid:String, display_name:String, games_json:String}, NoLogin, Failed(String) }`
  - `pub fn parse_delivery(code:&str)->SteamWeb` — betiğin teslim ettiği JSON'u çözer.
  - `pub fn games_to_entries(games_json:&str)->Result<Vec<PlatformEntry>, ProviderError>` — `{"response":{"games":[…]}}` biçimini mevcut `providers::steam::parse_owned_games` ile çevirir.
  - `pub struct SteamAccount { store }`: `new(Box<dyn SecretStore>)`, `save(&SteamCred)`, `load()->Result<Option<SteamCred>,AuthError>`, `forget()`, `status(&Db)->AccountStatus` (expired bayrağı db'den).
  - `steam_init_script` S0 betiğinden üretim sürümüdür:
    - Teşhis ve çerez izleyici yok.
    - Logged-in sinyali: application_config / `g_steamID` / `g_AccountID`.
    - Token ile aynı sayfadan `IPlayerService/GetOwnedGames/v1/?include_appinfo=1&include_played_free_games=1` çağrılır.
    - Teslim: `{"games":<response>,"steamid":..,"name":..}`, giriş yoksa `{"nologin":true}`, hata olursa `{"failed":"<kısa>"}`.
    - Her yol 20 sn içinde teslim eder. Token asla teslim edilmez.
- [ ] **Step 1: Failing tests**
  - `parse_delivery_games` / `parse_delivery_nologin` / `parse_delivery_failed`
  - `games_to_entries_maps_playtime`: appid 620, `playtime_forever` 600, `rtime_last_played` 1700000000 → playtime_min 600, last_played Some.
  - `cred_roundtrip_dpapi_memory`
  - `script_never_contains_token_in_delivery`: betik metninde teslim nesnesinin `access_token` ya da `webapi_token` anahtarı içermediğini metin olarak doğrular.
- [ ] **Step 2:** FAIL → uygula → `cargo test -p hangar-core` PASS.
- [ ] **Step 3: Commit**
  - `feat(core): Steam hesap çekirdeği ve giriş betiği`

### Task A3: Uygulama bağlantısı — Steam/GOG bağlan, tarama, hesap satırları
**Files:**
- Modify: `src-tauri/src/accounts.rs`, `src-tauri/src/commands.rs` (tarama), `src-tauri/src/login.rs` (`login_flow_with` S0'dan kalır; yalnız S0 debug komutu kaldırılır), `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json` (`"dragDropEnabled": false` pencerede), `src/components/AccountsSection.tsx` (+test), `src/lib/api.ts`, README
- Delete: `src-tauri/src/probe_steam.rs`, `src-tauri/src/probe_gog.rs`, `crates/hangar-core/src/accounts/{steam_probe.rs,gog_probe.rs}` ve kayıtları.

**Interfaces:**
- Consumes: A1 ve A2.
- Produces:
  - **`connect_account`:**
    - Steam: görünür pencere, `FlowOpts{data_dir: Some(webview-steam), hidden:false}`. `Games` gelirse cred kaydedilir, expired kalkar ve oyunlar `owned_cache_put` ile önbelleğe yazılır.
    - Gog: `login_flow` + `GogClient::connect`.
  - **`disconnect_account`:** Steam, Steam penceresi açıksa önce kapatır; sonra `webview-steam` klasörünü ve cred'i siler, önbelleği temizler.
  - **`get_accounts`:** `[steam, epic, gog, ubisoft]` döner.
  - **Yeni komutlar:**
    - `get_gog_source() -> String`, `set_gog_source(source)` — `gog.source` ayarı, yalnız `galaxy|account`.
    - `get_steam_advanced() -> {api_key, steam_id}`, `set_steam_advanced(api_key, steam_id)`. Bunlar SettingsDialog'daki mevcut alanların taşınmış halidir; mevcut ayar anahtarları kullanılır.
  - **Tarama:**
    - Steam anahtarı yoksa ve Steam bağlıysa: `fetch_steam_hidden(app, force:bool) -> SteamWeb`. Gizli pencere açılır, 20 sn zaman aşımı vardır.
    - `NoLogin` → expired + `WARN_EXPIRED` + önbellek. `Failed`/zaman aşımı → `WARN_OFFLINE` + önbellek.
    - `force=false` iken son başarılı alım 10 dakikadan yeniyse alım atlanır ve önbellek kullanılır.
    - `rescan` "Yenile"den `force=true`, odak taramasından `false` ile çağrılır. `rescan` komutuna `force: Option<bool>` parametresi eklenir; ön yüzde `useLibrary` odak çağrısı `false` geçer.
    - GOG: `gog.source == "account"` ise `GogClient::owned_for_scan` çağrılır ve `GogProvider::with_owned_from_db(false)` kullanılır.
  - **`AccountsSection`:**
    - Dört satır, mevcut satır kalıbında.
    - Steam satırında açılır "Gelişmiş" alanı: API anahtarı ve SteamID alanları + "Kaydet". Metin: "Anahtar girilirse giriş yerine anahtar kullanılır."
    - GOG satırında "Kaynak" seçimi: "Galaxy veritabanı" / "GOG hesabı". "GOG hesabı" seçiliyken Bağlan düğmesi görünür.
- [ ] **Step 1: Failing tests**
  - Rust (saf yardımcılar, `commands.rs` ya da yeni `steam_scan.rs` içinde):
    - `steam_scan_nologin_marks_expired`
    - `steam_scan_timeout_is_offline`
    - `steam_auto_throttle_10min`
    - Hepsi `fn steam_scan_decision(last_ok: Option<i64>, now:i64, force:bool) -> bool` ve `fn steam_outcome(r: Option<SteamWeb>) -> (Option<Result<..>>, Vec<String>, expired:Option<bool>)` gibi saf fonksiyonlar üzerinden.
  - vitest:
    - `AccountsSection` dört satırı Steam, Epic, GOG, Ubisoft sırasıyla gösterir.
    - Steam "Gelişmiş" alanı kaydeder (`setSteamAdvanced` çağrılır).
    - GOG kaynak seçimi `setGogSource("account")` çağırır.
- [ ] **Step 2:** FAIL → uygula → PASS. Komutlar: `cargo test -p hangar-core`, `cargo test -p hangar`, `cargo check -p hangar --release`, vitest, tsc.
- [ ] **Step 3:** README'ye ekle:
  - **Steam girişi:** kalıcı profil klasörü, bağlantıyı kesince silindiği, anahtarın isteğe bağlı olduğu.
  - **GOG girişi:** Galaxy açık istemcisi ve resmi olmayan yol uyarısı.
- [ ] **Step 4: Commit**
  - `feat(app): Steam ve GOG hesap bağlantısı`

## İz B — Arayüz (ana ağaç `C:\Users\user\workdir\hangar`, dal `feat/phase4`)

### Task B1: Filtre verisi ve değerlendirme
**Files:**
- Modify: `crates/hangar-core/src/db.rs` (`filters` tablosu), `src-tauri/src/collections.rs` (komutlar + beyaz liste `ui.filter`, `ui.sidebar`), `src-tauri/src/main.rs`, `src/lib/api.ts`, `src/lib/library.ts` (+test), `src/state/useLibrary.ts`

**Interfaces:**
- Produces:
  - Rust:
    - `filters(id INTEGER PK AUTOINCREMENT, name TEXT NOT NULL, rule TEXT NOT NULL, position INTEGER NOT NULL)`
    - `Db::filters()->Result<Vec<SavedFilter{id,name,rule}>>`
    - `create_filter(name, rule)->Result<Result<i64,NameError>>` — ad tekilliği koleksiyonlardaki `fold_name` ile.
    - `update_filter(id, name, rule)`, `delete_filter(id)`
  - Komutlar: `get_filters`, `create_filter`, `update_filter`, `delete_filter`. API: `getFilters()`, `createFilter(name, rule)`, `updateFilter(id, name, rule)`, `deleteFilter(id)`. Kural, TS'de JSON dizesi olarak saklanır.
  - TS:
    - `export interface FilterRule { stores?: Platform[]; installed?: "yes"|"no"; played?: "yes"|"no"; edition?: "original"|"special"; collection?: { id:number; in:boolean } }`
    - `export interface SavedFilter { id:number; name:string; rule: FilterRule }`
    - `export type ActiveFilter = { kind:"none" } | { kind:"installed" } | { kind:"saved"; id:number }`
    - `export function matchesRule(g:Game, r:FilterRule, t:Tab, cols:Collection[]):boolean`
      - stores: VEYA.
      - installed: `installedIn(g,t)`.
      - played: herhangi bir kayıtta `playtime_min > 0` ya da `last_played` var, veya `g.last_launched` dolu.
      - edition: `entries.some(e=>e.edition)` → özel sürüm.
      - collection: `inCollection`. Koleksiyon yoksa ölçüt yok sayılır.
    - `view(games, tab, filter:ActiveFilter, saved:SavedFilter[], cols, q, sort)` — `installedOnly` parametresinin yerini alır. `tabCounts` ve `installedCount` aynı şekilde güncellenir. Sekme görünürlüğü için `tabCounts(games, NONE, …)` kullanılır.
  - **useLibrary:**
    - `filter`, `setFilter(f:ActiveFilter)` (`ui.filter`'a yazar), `savedFilters`, `reloadFilters()`. `installedOnly` ve `setInstalledOnly` kaldırılır.
    - Göç: açılışta `ui.filter` boşsa ve `installed` true ise `filter = installed` olur ve `ui.filter = "installed"` yazılır.
    - `ui.filter` silinmiş bir filtreyi gösteriyorsa `none` olur.
- [ ] **Step 1: Failing tests (library.test.ts)**
  - `played rule counts steam playtime and hangar launches`
  - `stores rule is OR`
  - `rule with deleted collection ignores criterion`
  - `edition special vs original`
  - `installed rule respects store tab` (Faz 3 Watch_Dogs örneği)
- [ ] **Step 2: Failing tests**
  - Rust `filters_crud` + `filter_names_turkish_fold`.
  - useLibrary: göç testi ve `setFilter` yazım testi.
- [ ] **Step 3:** FAIL → uygula → tüm testler PASS. App ve Sidebar'ın derlenmesi için gereken en küçük uyarlamayı yap; asıl arayüz B2'de gelir. Bu uyarlama: Kurulu düğmesi `filter.kind === "installed"` durumunu açıp kapatır.
- [ ] **Step 4: Commit**
  - `feat(ui): kaydedilebilir filtreler (veri ve değerlendirme)`

### Task B2: Daraltılabilir kenar çubuğu + filtre listesi ve düzenleyicisi + nudge
**Files:**
- Modify: `src/components/Sidebar.tsx` (+test), `src/App.tsx`, `src/styles/app.css`, `src/state/useLibrary.ts` (`sidebar`, `setSidebar` → `ui.sidebar`)
- Create: `src/components/FilterEditor.tsx` (+test), `src/components/icons.tsx`. İkonlar inline SVG: filtre, kurulu, ekle, yenile, ayarlar, daralt/genişlet.

**Interfaces:**
- Consumes: B1.
- Produces:
  - `Sidebar` props: `{ collapsed, onToggleCollapsed, filter, savedFilters, counts:Record<string,number>, onFilter(f), onNewFilter(), onFilterContext(f,x,y), onAdd, onRescan, onSettings, rescanning }`
  - `FilterEditor` props: `{ initial?: SavedFilter, collections, onSave(name, rule): Promise<void>, onClose }`
    - Modal kullanır.
    - Ölçütler: mağaza onay kutuları; kurulu, oynanma ve sürüm için üç durumlu seçim; koleksiyon seçimi + içinde/dışında.
    - Not metni: "Oynama süresi yalnız Steam'de var; diğerlerinde Hangar'dan başlatılanlar sayılır."
- [ ] **Step 1: Failing tests**
  - `collapsed sidebar keeps all actions reachable`: daraltılmışken her düğmenin `aria-label`'ı var ve tıklanabilir.
  - `active filter toggles off when clicked again`
  - `Kurulu is builtin and has no context menu`
  - `FilterEditor saves rule`
  - Nudge: Steam bağlı değilse ve anahtar yoksa "Steam'e bağlan" metni görünür ve tıklanınca Ayarlar açılır. Bu, App testinde güncellenir.
- [ ] **Step 2:** FAIL → uygula → PASS.
  - Genişlik: açık 220px, daraltılmış 64px; geçiş `prefers-reduced-motion`'a uyar.
  - İpucu `title` + `aria-label` ile verilir.
  - Daraltılmış halde filtreler baş harf rozetiyle gösterilir.
- [ ] **Step 3: Görsel kontrol (controller yapar):**
  - Tauri penceresinde açık ve daraltılmış halin ekran görüntüsü alınır.
  - design-taste-frontend kurallarına göre değerlendirilir: hizalama, eşit kenarlar, ikon boyutu tutarlılığı, odak görünürlüğü.
  - Bulgular düzeltilir.
- [ ] **Step 4: Commit**
  - `feat(ui): daraltılabilir menü ve filtreler`

### Task B3: Koleksiyon sekmesine sürükle-bırak
**Files:**
- Modify: `src/components/GameCard.tsx`, `src/components/GameList.tsx`, `src/components/TabStrip.tsx` (+testler), `src/App.tsx`, `src/styles/app.css`

**Interfaces:**
- Produces:
  - Kart ve liste satırında `draggable` olur. `dataTransfer.setData("application/x-hangar-game", String(g.id))`.
  - TabStrip koleksiyon sekmesi: `onDropGame(collectionId:number, gameId:number)`. `dragover`'da yalnız bu tür kabul edilir ve sekmeye `.drop-ok` sınıfı eklenir.
  - App:
    - Kart koleksiyonda zaten varsa notice "zaten <ad> içinde" olur.
    - Yoksa `setInCollection(id, cardEntries(g), true)` + `reloadCollections()` çağrılır, notice "<ad> koleksiyonuna eklendi" olur.
    - Kısa durum metni 3 sn sonra silinir.
- [ ] **Step 1: Failing tests**
  - TabStrip: kartı koleksiyon sekmesine bırakmak `onDropGame(7, 42)` çağırır; mağaza sekmesine bırakmak hiçbir şey yapmaz.
  - GameCard: `dragstart` doğru veriyi koyar ve tıklama seçimi bozulmaz.
- [ ] **Step 2:** FAIL → uygula → PASS. Canlıda sürükle-bırakın çalıştığı kontrol edilir; A3'teki `dragDropEnabled:false` birleştirmeden sonra etkin olur.
- [ ] **Step 3: Commit**
  - `feat(ui): kartı koleksiyon sekmesine sürükle`

### Task B4: Ayarlar iki sekme (İz A birleştirildikten sonra)
**Files:**
- Modify: `src/components/SettingsDialog.tsx` (+test), `src/styles/app.css`

**Interfaces:**
- Consumes: A3 `AccountsSection` (Steam "Gelişmiş" dahil), Faz 3 tema düğmeleri, oyun klasörleri.
- Produces:
  - Solda dikey sekmeler: "Hesaplar", "Özelleştir".
  - Hesaplar = `AccountsSection`.
  - Özelleştir:
    - Tema: "Tema dosyasını aç", "Varsayılana dön".
    - Menü: "Açılışta menü: Açık / Daraltılmış" → `ui.sidebar`.
    - Oyun klasörleri: mevcut alan.
  - Steam anahtarı ve SteamID alanları SettingsDialog'dan kaldırılır (artık Hesaplar › Steam › Gelişmiş'te).
  - "Kaydet" yalnız gerekli yerde kalır: oyun klasörleri ve Steam gelişmiş.
- [ ] **Step 1: Failing tests:** iki sekme var; Özelleştir'de üç bölüm var; Hesaplar dört satırı gösterir.
- [ ] **Step 2:** FAIL → uygula → tüm testler PASS.
- [ ] **Step 3: Commit**
  - `feat(ui): ayarlar hesaplar ve özelleştir sekmeleri`

## Sıra
- A1 → A2 → A3, worktree'de.
- B1 → B2 → B3, ana ağaçta. İki iz paralel ilerler.
- Ardından `feat/phase4-accounts` birleştirilir, sonra B4.
- Son olarak tüm dalın Opus incelemesi, tek düzeltme turu ve kullanıcıyla canlı test (spec'teki 6 adım).
