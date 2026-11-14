# Koleksiyonlar, liste görünümü, tema dosyası + küçük açıklar — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- Steam tarzı elle koleksiyonlar, üstte sekme şeridi, Kart/Liste görünümü, `theme.json` ile renk özelleştirme ve Steam önceliği.
- Paralelde: CLAUDE.md'deki küçük açıkların bu özelliğe dokunmayanları.

**Architecture:**
- **Çekirdek (hangar-core):**
  - Koleksiyon tabloları ve CRUD: `db.rs`.
  - Tema dosyası ayrıştırma ve varsayılan şablon: yeni `theme.rs`.
  - Steam önceliği: `default_entry`.
- **Tauri katmanı:** Yeni komutlar ayrı `src-tauri/src/collections.rs` dosyasında; `commands.rs` paralel B izinin dosyası.
- **Ön yüz:**
  - Filtre `Tab` + `installedOnly` olarak ikiye ayrılır.
  - Saf fonksiyonlar `library.ts` içinde.
  - Yeni bileşenler: `TabStrip`, `GameList`, `useTheme`.

**Tech Stack:** Rust (rusqlite, serde_json), Tauri 2 (`tauri-plugin-opener` mevcut), React 18 + TS, vitest, `@tanstack/react-virtual`.

**Spec:** `docs/superpowers/specs/2026-11-14-hangar-ui-collections-design.md` ("Kurucu kontrolünden sonra netleşenler" bölümü dahil).

## Global Constraints
- **UI metni ve commit dili:**
  - UI metni Türkçe. Commit mesajı Türkçe conventional (`feat(ui): ...`).
  - Her commit `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` ile biter.
- **Görsel çizgi:** "Kokpit + Vitrin" tarzı ve `tokens.css` token'ları kullanılır. Yeni sabit renk yazılmaz (`var(--…)` veya `color-mix`). UI işinde design-taste-frontend ve ui-ux-pro-max kurallarına bakılır.
- **Ayrık dosya sahipliği:**
  - İz A (Task 1–8): `db.rs`, `theme.rs`, `src-tauri/src/collections.rs`, `main.rs` (yalnızca kayıt), `src/**` (aşağıdaki B dosyaları hariç).
  - İz B (Task 9): `src-tauri/src/commands.rs`, `local_scan.rs`, `Stage.tsx`, `PlayButton.tsx`, `Modal.tsx`, `README.md`, riot fixture/testleri.
  - İzler birbirinin dosyasına dokunmaz.
- **Eski veritabanı:** Yeni tablolar `CREATE TABLE IF NOT EXISTS` ile eklenir; geçiş kodu gerekmez.
- **Test komutları:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`. Node PATH'te değilse `bun node_modules/typescript/bin/tsc --noEmit` ve `bunx vitest run`.
- **Tema anahtarları:** Tam olarak şunlar:
  - `background→--bg`, `panel→--panel`, `raised→--raise`, `border→--line`, `text→--fg`
  - `textDim→--dim`, `accent→--accent`, `accentAlt→--accent-alt`, `onAccent→--on-accent`, `installed→--installed`
  - `_yardım` anahtarı yok sayılır.
- **Ayar anahtarları:**
  - `ui.view` = `"grid"|"list"` (varsayılan `grid`)
  - `ui.tab` = `"all" | "p:<platform>" | "c:<id>"`
  - `ui.installed` = `"1"|"0"`

## Review Focus
1. **Birleştirilmiş kart:** Steam+Ubisoft kartını koleksiyona ekle, sonra ayır. İki kart da koleksiyonda kalır. → Task 1 testi `membership_survives_split_and_merge`.
2. **Silinen koleksiyon kalıcı sekmede:** Kayıtlı `ui.tab = "c:7"` ve 7 silinmiş. Açılışta `Tümü`. → Task 4 testi `parseTab unknown collection falls back to all`.
3. **Bozuk tema dosyası:**
   - Bozuk JSON, `{"accent": 5}` ya da `{"accent": "kırmızımsı"}` arayüzü bozmaz.
   - Varsayılan renk kalır, tek uyarı görünür.
   - → Task 2 testleri ve Task 8 testi `invalid color is skipped`.
4. **Türkçe ad çakışması:** "İndie" ve "indie" aynı ad, "Işık" ve "ışık" aynı ad. Boşluk-yalnız ad reddedilir. → Task 1 testi `collection_names_turkish_fold`.
5. **Menü pencere kenarında:** Sağ alttaki kartta açılan menü ve alt menü ekran dışına taşmaz. → Task 6 testi `clampMenu keeps menu inside viewport`.

---

## İz A — Özellik (sırayla)

### Task 1: Koleksiyon verisi (core)

**Files:**
- Modify: `crates/hangar-core/src/db.rs` (şema `init` ~95-128; yeni yöntemler; testler dosya sonunda)

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Clone, Debug, PartialEq)] pub struct Collection { pub id: i64, pub name: String, pub entries: Vec<(Platform, String)> }`
  - `pub fn fold_name(s: &str) -> String` (trim + Türkçe küçük harf: `İ→i`, `I→ı`, diğerleri `to_lowercase`)
  - `Db::collections(&self) -> Result<Vec<Collection>>` — `position` artan.
  - `Db::create_collection(&self, name: &str) -> Result<Result<i64, NameError>>`
  - `Db::rename_collection(&self, id: i64, name: &str) -> Result<Result<(), NameError>>`
  - `Db::delete_collection(&self, id: i64) -> Result<()>` (üyelikleri de siler)
  - `Db::set_in_collection(&self, id: i64, entries: &[(Platform, String)], member: bool) -> Result<()>`
  - `#[derive(Debug, PartialEq)] pub enum NameError { Empty, Duplicate }`
  - `Result` = mevcut rusqlite `Result`.

- [ ] **Step 1: Failing tests (`db.rs` tests modülü)**
  - `collections_crud`: Oluşturma sırası korunur. Eski ada rename, kendi adına dönmek dahil, `Ok(Ok(()))`. Delete, koleksiyonu ve `collection_entries` satırlarını siler.
  - `collection_names_turkish_fold`:
    - `create("İndie")` sonra `create("indie")` → `Duplicate`.
    - `create("Işık")` sonra `create("ışık")` → `Duplicate`.
    - `create("   ")` → `Empty`.
    - `create("  AC  ")` adı `"AC"` olarak saklar.
  - `membership_survives_split_and_merge`:
    - Tek kartta `steam:1` + `ubisoft:541` (`one_card` yardımcısı) koleksiyona iki kaydıyla eklenir.
    - Ardından `split`/override + `scan_all` benzeri yeniden yazma yapılır.
    - `collections()[0].entries` iki kaydı da içerir.
  - `rescan_keeps_membership`: Kütüphaneden düşen kaydın üyelik satırı silinmez.
  - `set_in_collection_idempotent`: Aynı kaydı iki kez eklemek tek satır bırakır. `member=false` kartın tüm kayıtlarını siler.

- [ ] **Step 2:** `cargo test -p hangar-core collection` → FAIL (tanımsız).

- [ ] **Step 3: Implement**
  - Şema:
    - `collections(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, position INTEGER NOT NULL, created_at INTEGER NOT NULL)`
    - `collection_entries(collection_id INTEGER NOT NULL, platform TEXT NOT NULL, platform_id TEXT NOT NULL, PRIMARY KEY(collection_id, platform, platform_id))`
  - Yeni `position` = `COALESCE(MAX(position),0)+1`.
  - Tekillik `fold_name` ile Rust'ta kontrol edilir; tüm adlar okunup karşılaştırılır.
  - Tarama kodu bu tablolara dokunmaz.

- [ ] **Step 4:** `cargo test -p hangar-core` → PASS (tüm testler).

- [ ] **Step 5: Commit**
  - `feat(core): elle koleksiyonlar (kayıt anahtarıyla üyelik)`

### Task 2: Tema dosyası + Steam önceliği (core)

**Files:**
- Create: `crates/hangar-core/src/theme.rs`
- Modify: `crates/hangar-core/src/lib.rs` (`pub mod theme;`)
- Modify: `crates/hangar-core/src/db.rs` (`default_entry` ~64-82 ve `default_entry_rules` testi ~745)

**Interfaces:**
- Produces:
  - `pub const KEYS: [(&str, &str); 10]` (JSON adı, CSS değişkeni; Global Constraints'teki sırayla)
  - `#[derive(Serialize, Debug, PartialEq, Default)] pub struct ThemeRead { pub colors: BTreeMap<String, String>, pub warnings: Vec<String> }` — `colors` CSS değişkeni → ham değer.
  - `pub fn parse(text: &str) -> ThemeRead`
  - `pub fn default_file() -> String` — 10 anahtarın tamamı `tokens.css` değerleriyle, artı `"_yardım"` dizisi (her anahtar için bir Türkçe satır).
  - `pub fn read_file(path: &Path) -> ThemeRead` — dosya yoksa boş `ThemeRead` döner, uyarı yok.

- [ ] **Step 1: Failing tests (`theme.rs`)**
  - `default_matches_tokens_css`:
    - `include_str!("../../../src/styles/tokens.css")` içindeki 10 değişkenin değeri okunur.
    - `parse(&default_file()).colors` ile birebir eşit, `warnings` boş.
  - `unknown_key_and_non_string_warn`:
    - `{"accent":"#fff","foo":"#000","text":5}` → `colors == {"--accent":"#fff"}`.
    - İki uyarı var; biri "foo", diğeri "text" içerir.
  - `broken_json`: `"{"` → `colors` boş, 1 uyarı (`"Tema dosyası okunamadı"` ile başlar).
  - `help_key_ignored`: `{"_yardım":["x"]}` → uyarı yok.

- [ ] **Step 2: Failing test (`db.rs`):** `default_entry_rules` güncellenir.
  - Installed Steam(10) + GOG(20) → Steam kazanır.
  - Installed GOG(10) + Epic(20), Steam yok → Epic kazanır (son oynanan).
  - İki installed Steam → son oynanan Steam.
  - Tercih (`preferred`) varsa o kazanır (mevcut davranış).

- [ ] **Step 3:** Test → FAIL.

- [ ] **Step 4: Implement**
  - `theme.rs`:
    - Renk sözdizimini Rust doğrulamaz; ön yüz `CSS.supports` ile doğrular.
    - Uyarı metinleri Türkçe ve kısa: `Tema: bilinmeyen anahtar "foo"`, `Tema: "text" metin olmalı`.
  - `default_entry`: Installed kayıtlar içinde Steam varsa en son oynanan Steam seçilir. Yoksa mevcut "en son oynanan" kuralı uygulanır.

- [ ] **Step 5:** `cargo test -p hangar-core` → PASS.

- [ ] **Step 6: Commit**
  - `feat(core): tema dosyası ayrıştırma ve Steam önceliği`

### Task 3: Tauri komutları + api.ts

**Files:**
- Create: `src-tauri/src/collections.rs`
- Modify: `src-tauri/src/main.rs` (`mod collections;` + iki handler listesine kayıt)
- Modify: `src/lib/api.ts`

**Interfaces:**
- Consumes: Task 1–2 API'leri. `Dbs`, `lock` gibi mevcut yardımcıların kullanımı `commands.rs`'teki kalıpla aynı. Bu dosyalar `pub(crate)` ise yeniden kullanılır; değilse yalnızca `commands.rs`'te görünürlük değişikliği yapılabilir.
- Produces (Rust komut → TS):
  - `get_collections() -> Vec<Collection>` → `api.getCollections(): Promise<Collection[]>`. TS tipi `interface Collection { id: number; name: string; entries: EntryKey[] }`.
  - `create_collection(name) -> i64` → `api.createCollection(name): Promise<number>`. Hata metinleri:
    - `NameError::Empty` → `"Koleksiyon adı boş olamaz"`
    - `Duplicate` → `"Bu adda bir koleksiyon zaten var"`
  - `rename_collection(id, name)` → `api.renameCollection(id, name): Promise<void>`
  - `delete_collection(id)` → `api.deleteCollection(id): Promise<void>`
  - `set_in_collection(id, entries: Vec<(Platform,String)>, member: bool)` → `api.setInCollection(id, entries: EntryKey[], member: boolean): Promise<void>`
  - `get_theme() -> ThemeRead` → `api.getTheme(): Promise<{ colors: Record<string,string>; warnings: string[] }>`
    - Dosya: `app_data_dir()/theme.json`.
  - `open_theme_file()` → `api.openThemeFile(): Promise<void>`
    - Dosya yoksa `default_file()` yazılır, sonra `tauri-plugin-opener` ile varsayılan düzenleyicide açılır.
  - `reset_theme() -> ThemeRead` → `api.resetTheme()`
    - `default_file()` yazılır ve okunarak döner.
  - `get_ui_prefs() -> UiPrefs { view: String, tab: String, installed: bool }` → `api.getUiPrefs()`
    - Varsayılanlar: `grid`, `all`, `false`.
  - `set_ui_pref(key: String, value: String)` → `api.setUiPref(key, value)`
    - Yalnızca `ui.view`, `ui.tab`, `ui.installed` kabul edilir; diğerleri `Err("Bilinmeyen ayar")`.

- [ ] **Step 1:** Komutları yaz ve iki handler listesine (debug + release) kaydet.
- [ ] **Step 2:** `cargo check -p hangar` ve `cargo check -p hangar --release` → PASS.
- [ ] **Step 3:** `npx tsc --noEmit` → PASS.
- [ ] **Step 4: Commit**
  - `feat(app): koleksiyon, tema ve görünüm tercihi komutları`

### Task 4: Filtre modeli (`library.ts`)

**Files:**
- Modify: `src/lib/library.ts`, `src/lib/library.test.ts`

**Interfaces:**
- Consumes: `Collection` (api.ts).
- Produces:
  - `export type Tab = { kind: "all" } | { kind: "platform"; platform: Platform } | { kind: "collection"; id: number };`
  - `export function tabKey(t: Tab): string` → `"all" | "p:steam" | "c:12"`
  - `export function parseTab(s: string | null, cols: Collection[]): Tab` — bilinmeyen ya da silinmiş → `{kind:"all"}`.
  - `export function inCollection(g: Game, c: Collection): boolean` — kartın herhangi bir kaydı `c.entries` içindeyse.
  - `export function installedIn(g: Game, t: Tab): boolean` — platform sekmesinde `entries.some(e => e.platform === p && e.installed)`, diğer sekmelerde `isInstalled(g)`.
  - `export function view(games: Game[], t: Tab, installedOnly: boolean, cols: Collection[], q: string, s: Sort): Game[]` — sıralama ve arama mantığı aynen.
  - `export function tabCounts(games: Game[], installedOnly: boolean, cols: Collection[]): { all: number; platform: Record<Platform, number>; collection: Record<number, number> }`
  - `export function installedCount(games: Game[], t: Tab, cols: Collection[]): number` — kenar çubuğundaki "Kurulu" sayacı.
  - Eski `Filter` ve `counts` kaldırılır; tüm kullanım yerleri sonraki task'larda değişir.

- [ ] **Step 1: Failing tests (`library.test.ts`)**
  - `ubisoft tab + installed excludes steam-installed merged card`: Watch_Dogs `steam` kurulu + `ubisoft` kurulu değil.
    - `view(…, {platform:"ubisoft"}, true, …)` boş.
    - `{platform:"steam"}` ile 1.
    - `{kind:"all"}` ile 1.
  - `merged card counts in both store tabs`
  - `collection tab filters by any entry`
  - `parseTab unknown collection falls back to all`: `parseTab("c:7", [])` → `{kind:"all"}`. `parseTab("p:xbox", [])` → `{kind:"all"}`.
  - `tabCounts respects installedOnly`
  - Mevcut sıralama ve arama testleri yeni imzayla aynı sonuçları verir.

- [ ] **Step 2:** `npm test -- library` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `npm test -- library` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): sekme + kurulu filtre modeli`

### Task 5: Sekme şeridi, kenar çubuğu, durum ve kalıcılık

**Files:**
- Create: `src/components/TabStrip.tsx`, `src/components/TabStrip.test.tsx`
- Modify: `src/components/Sidebar.tsx`, `src/state/useLibrary.ts`, `src/App.tsx`, `src/styles/app.css`

**Interfaces:**
- Consumes: Task 3 API'leri, Task 4 fonksiyonları.
- Produces:
  - `useLibrary` dönüşüne eklenir, `filter`/`setFilter` yerine geçer:
    - `tab`, `setTab(t: Tab)`
    - `installedOnly`, `setInstalledOnly(b: boolean)`
    - `collections: Collection[]`, `reloadCollections(): Promise<void>`
  - `setTab` ve `setInstalledOnly` ayarı `api.setUiPref` ile yazar. Açılışta `getUiPrefs` + `getCollections` → `parseTab`.
  - `TabStrip` props:
    - `{ tab, counts: ReturnType<typeof tabCounts>, collections, installedOnly, onTab(t), onClearInstalled(), onCreate(name): Promise<void>, onTabContext(c: Collection, x, y) }`
  - `Sidebar` props:
    - `filter/counts/onFilter` yerine `installedOnly`, `installedCount`, `onToggleInstalled`.
    - `onSettings`, `onRescan`, `onAdd`, `rescanning` aynı kalır.

- [ ] **Step 1: Failing tests (`TabStrip.test.tsx`)**
  - Sıra: `Tümü`, sonra yalnızca sayısı >0 olan mağazalar, sonra ayraç (`role="separator"`), sonra koleksiyonlar (0 olsa da görünür), sonra `+`.
  - `+` tıklanınca bir textbox açılır. Enter → `onCreate("AC")`. Esc → kapanır, çağrı yok. Boş Enter → çağrı yok.
  - `installedOnly` iken "Kurulu ✕" etiketi görünür ve tıklanınca `onClearInstalled`.
  - Aktif sekmede `aria-selected="true"`.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - **Şerit:** Tek satır, `overflow-x: auto`.
  - **Kenar çubuğu:** Mağaza grubu kaldırılır. "Kurulu" artık bir anahtar (`aria-pressed`) ve sayacı var.
  - **App:**
    - Aktif sekmenin sayısı 0 olup sekme gizlenirse `Tümü`'ne geçilir.
    - Koleksiyon sekmesinin sağ tık menüsünde "Yeniden adlandır" ve "Sil" olur.
      - Yeniden adlandır: satır içi alan; TabStrip'e `renamingId` prop'u eklenebilir.
      - Sil: `window.confirm("“X” koleksiyonu silinsin mi? Oyunlar kütüphanede kalır.")`.
      - Aktif koleksiyon silinirse `Tümü`.
    - API hataları mevcut `setError` notice'ine düşer.
  - **Boş durum metni:** "Hiç oyun bulunamadı. Steam, Epic, GOG, Ubisoft ya da Riot kurulu mu? Elle eklemek için “Oyun ekle”yi kullan."

- [ ] **Step 4:** `npm test` ve `npx tsc --noEmit` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): sekme şeridi ve koleksiyon sekmeleri`

### Task 6: Bağlam menüsü alt menü + kenara sığdırma + koleksiyona ekle

**Files:**
- Modify: `src/components/ContextMenu.tsx`, `src/App.tsx`, `src/styles/app.css`
- Create: `src/components/ContextMenu.test.tsx`

**Interfaces:**
- Produces:
  - `export type MenuItem = { label: string; onSelect?: () => void; checked?: boolean; items?: MenuItem[] }`
    - `items` olan öğe alt menü açar (hover/→ ve tıklama).
  - `export function clampMenu(x: number, y: number, w: number, h: number, vw: number, vh: number): { x: number; y: number }`
  - `ContextMenu` props: `{ x, y, items: MenuItem[], onClose }`. Ölçüm sonrası `clampMenu` uygulanır. Alt menü sağa sığmazsa sola açılır.

- [ ] **Step 1: Failing tests**
  - `clampMenu keeps menu inside viewport`:
    - `clampMenu(790, 590, 200, 150, 800, 600)` → `{x: 590, y: 440}`.
    - İçerideki nokta değişmez.
    - Negatif sonuç 0'a kırpılır.
  - `submenu item toggles`:
    - "Koleksiyona ekle ›" altında `checked` öğe `aria-checked="true"`.
    - Tıklanınca `onSelect` çağrılır ve menü kapanır.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - App'teki kart menüsüne "Koleksiyona ekle ›" eklenir. Alt menüde her koleksiyon `checked: inCollection(g,c)` ile listelenir. Tıklanınca kartın tüm kayıtlarıyla `setInCollection(c.id, entries, !checked)`, ardından `reloadCollections()`.
  - Alt menünün sonunda "Yeni koleksiyon…" olur: `window.prompt("Koleksiyon adı")` → `createCollection` → kartı ekle → `reloadCollections`. Aktif sekme değişmez.

- [ ] **Step 4:** `npm test` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): koleksiyona ekle alt menüsü, menü kenara sığar`

### Task 7: Liste görünümü

**Files:**
- Create: `src/components/GameList.tsx`, `src/components/GameList.test.tsx`
- Modify: `src/App.tsx`, `src/components/TabStrip.tsx` (sağda görünüm anahtarı), `src/state/useLibrary.ts` (`viewMode`, `setViewMode` → `ui.view`), `src/styles/app.css`

**Interfaces:**
- Produces:
  - `GameList` props, `GameGrid` ile aynı: `{ games, selectedId, onSelect, onContext(g, x, y), onLaunch(g) }`.
  - `onLaunch`, kartın `currentEntry`'si üzerinden mevcut `launch` akışını kullanır.
  - `viewMode: "grid" | "list"`.

- [ ] **Step 1: Failing tests (`GameList.test.tsx`)**
  - Satırda ad, mağaza rozetleri (Steam ilk), "Kurulu"/"Kurulu değil" ve oynama süresi görünür. Süre `playtime_min` toplamı; 0 ise "—"; `12 sa` biçimi.
  - Düğme "Oyna" ya da "Kur" der.
  - Sağ tık `onContext`, tık `onSelect` çağırır.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - `@tanstack/react-virtual` ile sabit satır yüksekliği. Kapak `Cover` bileşeniyle küçük boy.
  - Görünüm anahtarı TabStrip'in sağında iki ikonlu düğme (`aria-pressed`).

- [ ] **Step 4:** `npm test`, `npx tsc --noEmit` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): liste görünümü`

### Task 8: Temanın uygulanması + Ayarlar düğmeleri

**Files:**
- Create: `src/state/useTheme.ts`, `src/state/useTheme.test.ts`
- Modify: `src/App.tsx`, `src/components/SettingsDialog.tsx`, `src/styles/tokens.css` / `app.css` (accent'ten türetilmiş sabit rgba renkleri `color-mix(in srgb, var(--accent) N%, transparent)` yapılır)

**Interfaces:**
- Produces:
  - `export function applyTheme(colors: Record<string,string>, root: HTMLElement, supports: (v: string) => boolean): string[]`
    - Önce 10 değişkenin inline değerini temizler, sonra geçerli olanları `setProperty` ile yazar.
    - Geçersiz değerler için uyarı döner: `Tema: "--accent" geçersiz renk`.
  - `export function useTheme(onWarnings: (w: string[]) => void): { reload(): Promise<void>; reset(): Promise<void> }`
    - Açılışta ve her `window` focus'unda `api.getTheme()` çağırır. Galaxy 10 sn sınırından bağımsızdır.
    - Uyarı listesi bir öncekiyle aynıysa `onWarnings` çağrılmaz.

- [ ] **Step 1: Failing tests (`useTheme.test.ts`)**
  - `invalid color is skipped`: `applyTheme({"--accent":"kırmızımsı","--bg":"#000"}, el, v => v.startsWith("#"))`.
    - `--bg` yazılır, `--accent` yazılmaz.
    - 1 uyarı döner.
  - `reapply clears previous overrides`: İkinci çağrıda olmayan değişken inline'dan silinir.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - App, `useTheme` uyarılarını mevcut `warnings`/`notice` satırına ekler.
  - SettingsDialog'a "Görünüm" bölümü eklenir:
    - "Tema dosyasını aç" → `api.openThemeFile()`
    - "Varsayılana dön" → `reset()`

- [ ] **Step 4:** `npm test`, `npx tsc --noEmit`, `cargo check -p hangar` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): tema dosyası uygulanır`

## İz B — Küçük açıklar (Task 1–8 ile paralel, tek alt ajan)

### Task 9: Bilinen küçük açıklar

**Files:** yalnızca İz B dosyaları (Global Constraints). Her madde ayrı commit, `fix(...)`.

- [ ] **9a — Başlatma betiği** (`src-tauri/src/commands.rs` ~240-246):
  - Argüman tırnaklanırken `"` öncesindeki `\` dizileri çiftlenir (Windows CommandLineToArgvW kuralı); sondaki `\` dizisi de çiftlenir.
  - Saf bir `fn quote_arg(s: &str) -> String` yazılır.
  - Testler: `quote_arg(r#"a\"b"#) == r#""a\\\"b""#`, `quote_arg(r"C:\dir\") == r#""C:\dir\\""#`, `quote_arg("plain") == "plain"`.
- [ ] **9b — Tarama hariç tutması** (`crates/hangar-core/src/local_scan.rs` ~10-20):
  - `crash` yalnızca dosya adı `crash` içerir ve aynı zamanda `handler|report|pad` içerirse elenir.
  - `unins` önek eşleşmesi olur.
  - Testler: `CrashBandicoot.exe` elenmez; `UnityCrashHandler64.exe` ve `unins000.exe` elenir; `MyUninstaller.exe` elenmez.
- [ ] **9c — Bozuk `scan_folders` JSON'u** (`commands.rs` `scan_folders_of` ~439):
  - Ayrıştırma hatasında boş liste yerine `Err("Tarama klasörleri okunamadı; Ayarlar'dan yeniden seç")` döner. Bu hata `get_scan_folders` ve tarama uyarısına taşınır.
  - Test: bozuk değerde `get_scan_folders` `Err` döner.
- [ ] **9d — Riot PBE fixture'ı:** Mevcut ama kullanılmayan PBE fixture'ı için Riot sağlayıcı testi eklenir. PBE kurulumu ayrı bir kayıt olarak algılanmalı ya da beklenen davranışa göre elenmeli; mevcut kodun davranışını sabitleyen bir test yeterli.
- [ ] **9e — "Kaldır" onayı** (`src/components/Stage.tsx` ~109):
  - `window.confirm("“<ad>” Hangar'dan kaldırılsın mı? Oyun dosyaları silinmez.")` iptal edilirse `onRemove` çağrılmaz.
  - Vitest testi.
- [ ] **9f — Eksik exe'li yerel kart** (`src/components/PlayButton.tsx`):
  - Yerel kaydın exe'si yoksa etiket "Kur · Yerel" yerine "Exe bulunamadı · Yerel" olur ve düğme devre dışı kalır.
  - Mevcut PlayButton testlerine bir vaka eklenir.
- [ ] **9g — Ayarlar penceresinde odak tuzağı** (`src/components/Modal.tsx`):
  - Tab ve Shift+Tab modal içinde döner.
  - Test: son öğede Tab → ilk öğe.
- [ ] **9h — README:** "Steam API anahtarı" bölümüne SteamID notu eklenir: kullanıcı adı yerine 17 haneli SteamID64 ya da boş bırakılabilir; boşsa `loginusers.vdf`'ten alınır.
- [ ] **Doğrulama:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit` → PASS.

---

## Son
- Tüm-dal Opus incelemesi yapılır.
- Kullanıcıyla canlı kontrol: spec'teki 6 adımlık "Canlı" listesi.
- `CLAUDE.md` güncellenir: kapanan açıklar silinir.
- Squash merge PR.
