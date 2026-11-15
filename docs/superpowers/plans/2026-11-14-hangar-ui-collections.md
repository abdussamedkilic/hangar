# Collections, list view, theme file + small gaps — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- Steam-style manual collections, a tab strip at the top, Card/List view, color customization via `theme.json`, and Steam priority.
- In parallel: the small gaps in CLAUDE.md that do not touch this feature.

**Architecture:**
- **Core (hangar-core):**
  - Collection tables and CRUD: `db.rs`.
  - Theme file parsing and default template: new `theme.rs`.
  - Steam priority: `default_entry`.
- **Tauri layer:** New commands go in a separate `src-tauri/src/collections.rs` file; `commands.rs` is the file of the parallel Track B.
- **Front end:**
  - The filter is split in two as `Tab` + `installedOnly`.
  - Pure functions in `library.ts`.
  - New components: `TabStrip`, `GameList`, `useTheme`.

**Tech Stack:** Rust (rusqlite, serde_json), Tauri 2 (`tauri-plugin-opener` already present), React 18 + TS, vitest, `@tanstack/react-virtual`.

**Spec:** `docs/superpowers/specs/2026-11-14-hangar-ui-collections-design.md` (including the section "Clarified after the founder's check").

## Global Constraints
- **UI text and commit language:**
  - UI text is Turkish. Commit messages are English conventional (`feat(ui): ...`).
  - Every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Visual line:** The "Kokpit + Vitrin" style and the `tokens.css` tokens are used. No new hardcoded colors are written (`var(--…)` or `color-mix`). In UI work the design-taste-frontend and ui-ux-pro-max rules are consulted.
- **Disjoint file ownership:**
  - Track A (Task 1–8): `db.rs`, `theme.rs`, `src-tauri/src/collections.rs`, `main.rs` (registration only), `src/**` (except the B files below).
  - Track B (Task 9): `src-tauri/src/commands.rs`, `local_scan.rs`, `Stage.tsx`, `PlayButton.tsx`, `Modal.tsx`, `README.md`, riot fixture/tests.
  - The tracks do not touch each other's files.
- **Old database:** New tables are added with `CREATE TABLE IF NOT EXISTS`; no migration code is needed.
- **Test commands:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`. If Node is not on PATH, `bun node_modules/typescript/bin/tsc --noEmit` and `bunx vitest run`.
- **Theme keys:** Exactly these:
  - `background→--bg`, `panel→--panel`, `raised→--raise`, `border→--line`, `text→--fg`
  - `textDim→--dim`, `accent→--accent`, `accentAlt→--accent-alt`, `onAccent→--on-accent`, `installed→--installed`
  - The `_yardım` key is ignored.
- **Setting keys:**
  - `ui.view` = `"grid"|"list"` (default `grid`)
  - `ui.tab` = `"all" | "p:<platform>" | "c:<id>"`
  - `ui.installed` = `"1"|"0"`

## Review Focus
1. **Merged card:** Add a Steam+Ubisoft card to a collection, then split it. Both cards stay in the collection. → Task 1 test `membership_survives_split_and_merge`.
2. **Deleted collection on a persistent tab:** Saved `ui.tab = "c:7"` and 7 is deleted. On startup `Tümü`. → Task 4 test `parseTab unknown collection falls back to all`.
3. **Broken theme file:**
   - Broken JSON, `{"accent": 5}` or `{"accent": "kırmızımsı"}` does not break the UI.
   - The default color stays, a single warning is shown.
   - → Task 2 tests and Task 8 test `invalid color is skipped`.
4. **Turkish name collision:** "İndie" and "indie" are the same name, "Işık" and "ışık" are the same name. A whitespace-only name is rejected. → Task 1 test `collection_names_turkish_fold`.
5. **Menu at the window edge:** The menu and submenu opened on a card at the bottom right do not overflow the screen. → Task 6 test `clampMenu keeps menu inside viewport`.

---

## Track A — Feature (in order)

### Task 1: Collection data (core)

**Files:**
- Modify: `crates/hangar-core/src/db.rs` (schema `init` ~95-128; new methods; tests at the end of the file)

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Clone, Debug, PartialEq)] pub struct Collection { pub id: i64, pub name: String, pub entries: Vec<(Platform, String)> }`
  - `pub fn fold_name(s: &str) -> String` (trim + Turkish lowercase: `İ→i`, `I→ı`, others `to_lowercase`)
  - `Db::collections(&self) -> Result<Vec<Collection>>` — `position` ascending.
  - `Db::create_collection(&self, name: &str) -> Result<Result<i64, NameError>>`
  - `Db::rename_collection(&self, id: i64, name: &str) -> Result<Result<(), NameError>>`
  - `Db::delete_collection(&self, id: i64) -> Result<()>` (also deletes memberships)
  - `Db::set_in_collection(&self, id: i64, entries: &[(Platform, String)], member: bool) -> Result<()>`
  - `#[derive(Debug, PartialEq)] pub enum NameError { Empty, Duplicate }`
  - `Result` = the existing rusqlite `Result`.

- [ ] **Step 1: Failing tests (`db.rs` tests module)**
  - `collections_crud`: Creation order is kept. Renaming to the old name, including going back to its own name, is `Ok(Ok(()))`. Delete removes the collection and the `collection_entries` rows.
  - `collection_names_turkish_fold`:
    - `create("İndie")` then `create("indie")` → `Duplicate`.
    - `create("Işık")` then `create("ışık")` → `Duplicate`.
    - `create("   ")` → `Empty`.
    - `create("  AC  ")` stores the name as `"AC"`.
  - `membership_survives_split_and_merge`:
    - A single card with `steam:1` + `ubisoft:541` (`one_card` helper) is added to the collection with its two entries.
    - Then a `split`/override + `scan_all`-like rewrite is done.
    - `collections()[0].entries` contains both entries.
  - `rescan_keeps_membership`: The membership row of an entry that drops out of the library is not deleted.
  - `set_in_collection_idempotent`: Adding the same entry twice leaves one row. `member=false` deletes all entries of the card.

- [ ] **Step 2:** `cargo test -p hangar-core collection` → FAIL (undefined).

- [ ] **Step 3: Implement**
  - Schema:
    - `collections(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, position INTEGER NOT NULL, created_at INTEGER NOT NULL)`
    - `collection_entries(collection_id INTEGER NOT NULL, platform TEXT NOT NULL, platform_id TEXT NOT NULL, PRIMARY KEY(collection_id, platform, platform_id))`
  - New `position` = `COALESCE(MAX(position),0)+1`.
  - Uniqueness is checked in Rust with `fold_name`; all names are read and compared.
  - Scan code does not touch these tables.

- [ ] **Step 4:** `cargo test -p hangar-core` → PASS (all tests).

- [ ] **Step 5: Commit**
  - `feat(core): manual collections (membership by entry key)`

### Task 2: Theme file + Steam priority (core)

**Files:**
- Create: `crates/hangar-core/src/theme.rs`
- Modify: `crates/hangar-core/src/lib.rs` (`pub mod theme;`)
- Modify: `crates/hangar-core/src/db.rs` (`default_entry` ~64-82 and the `default_entry_rules` test ~745)

**Interfaces:**
- Produces:
  - `pub const KEYS: [(&str, &str); 10]` (JSON name, CSS variable; in the order from Global Constraints)
  - `#[derive(Serialize, Debug, PartialEq, Default)] pub struct ThemeRead { pub colors: BTreeMap<String, String>, pub warnings: Vec<String> }` — `colors` CSS variable → raw value.
  - `pub fn parse(text: &str) -> ThemeRead`
  - `pub fn default_file() -> String` — all 10 keys with the `tokens.css` values, plus a `"_yardım"` array (one Turkish line for each key).
  - `pub fn read_file(path: &Path) -> ThemeRead` — if the file does not exist an empty `ThemeRead` is returned, no warning.

- [ ] **Step 1: Failing tests (`theme.rs`)**
  - `default_matches_tokens_css`:
    - The values of the 10 variables in `include_str!("../../../src/styles/tokens.css")` are read.
    - Exactly equal to `parse(&default_file()).colors`, `warnings` empty.
  - `unknown_key_and_non_string_warn`:
    - `{"accent":"#fff","foo":"#000","text":5}` → `colors == {"--accent":"#fff"}`.
    - There are two warnings; one contains "foo", the other "text".
  - `broken_json`: `"{"` → `colors` empty, 1 warning (starts with `"Tema dosyası okunamadı"`).
  - `help_key_ignored`: `{"_yardım":["x"]}` → no warning.

- [ ] **Step 2: Failing test (`db.rs`):** `default_entry_rules` is updated.
  - Installed Steam(10) + GOG(20) → Steam wins.
  - Installed GOG(10) + Epic(20), no Steam → Epic wins (last played).
  - Two installed Steam → the last played Steam.
  - If there is a preference (`preferred`) it wins (existing behavior).

- [ ] **Step 3:** Test → FAIL.

- [ ] **Step 4: Implement**
  - `theme.rs`:
    - Rust does not validate color syntax; the front end validates with `CSS.supports`.
    - Warning texts are Turkish and short: `Tema: bilinmeyen anahtar "foo"`, `Tema: "text" metin olmalı`.
  - `default_entry`: If Steam is among the installed entries, the last played Steam is chosen. Otherwise the existing "last played" rule applies.

- [ ] **Step 5:** `cargo test -p hangar-core` → PASS.

- [ ] **Step 6: Commit**
  - `feat(core): theme file parsing and Steam priority`

### Task 3: Tauri commands + api.ts

**Files:**
- Create: `src-tauri/src/collections.rs`
- Modify: `src-tauri/src/main.rs` (`mod collections;` + registration in two handler lists)
- Modify: `src/lib/api.ts`

**Interfaces:**
- Consumes: Task 1–2 APIs. The use of existing helpers like `Dbs`, `lock` is the same as the pattern in `commands.rs`. If these are `pub(crate)` they are reused; if not, only a visibility change in `commands.rs` may be made.
- Produces (Rust command → TS):
  - `get_collections() -> Vec<Collection>` → `api.getCollections(): Promise<Collection[]>`. TS type `interface Collection { id: number; name: string; entries: EntryKey[] }`.
  - `create_collection(name) -> i64` → `api.createCollection(name): Promise<number>`. Error texts:
    - `NameError::Empty` → `"Koleksiyon adı boş olamaz"`
    - `Duplicate` → `"Bu adda bir koleksiyon zaten var"`
  - `rename_collection(id, name)` → `api.renameCollection(id, name): Promise<void>`
  - `delete_collection(id)` → `api.deleteCollection(id): Promise<void>`
  - `set_in_collection(id, entries: Vec<(Platform,String)>, member: bool)` → `api.setInCollection(id, entries: EntryKey[], member: boolean): Promise<void>`
  - `get_theme() -> ThemeRead` → `api.getTheme(): Promise<{ colors: Record<string,string>; warnings: string[] }>`
    - File: `app_data_dir()/theme.json`.
  - `open_theme_file()` → `api.openThemeFile(): Promise<void>`
    - If the file does not exist `default_file()` is written, then it is opened in the default editor with `tauri-plugin-opener`.
  - `reset_theme() -> ThemeRead` → `api.resetTheme()`
    - `default_file()` is written and returned after reading.
  - `get_ui_prefs() -> UiPrefs { view: String, tab: String, installed: bool }` → `api.getUiPrefs()`
    - Defaults: `grid`, `all`, `false`.
  - `set_ui_pref(key: String, value: String)` → `api.setUiPref(key, value)`
    - Only `ui.view`, `ui.tab`, `ui.installed` are accepted; others `Err("Bilinmeyen ayar")`.

- [ ] **Step 1:** Write the commands and register them in the two handler lists (debug + release).
- [ ] **Step 2:** `cargo check -p hangar` and `cargo check -p hangar --release` → PASS.
- [ ] **Step 3:** `npx tsc --noEmit` → PASS.
- [ ] **Step 4: Commit**
  - `feat(app): collection, theme and view preference commands`

### Task 4: Filter model (`library.ts`)

**Files:**
- Modify: `src/lib/library.ts`, `src/lib/library.test.ts`

**Interfaces:**
- Consumes: `Collection` (api.ts).
- Produces:
  - `export type Tab = { kind: "all" } | { kind: "platform"; platform: Platform } | { kind: "collection"; id: number };`
  - `export function tabKey(t: Tab): string` → `"all" | "p:steam" | "c:12"`
  - `export function parseTab(s: string | null, cols: Collection[]): Tab` — unknown or deleted → `{kind:"all"}`.
  - `export function inCollection(g: Game, c: Collection): boolean` — if any entry of the card is in `c.entries`.
  - `export function installedIn(g: Game, t: Tab): boolean` — on a platform tab `entries.some(e => e.platform === p && e.installed)`, on other tabs `isInstalled(g)`.
  - `export function view(games: Game[], t: Tab, installedOnly: boolean, cols: Collection[], q: string, s: Sort): Game[]` — sorting and search logic as is.
  - `export function tabCounts(games: Game[], installedOnly: boolean, cols: Collection[]): { all: number; platform: Record<Platform, number>; collection: Record<number, number> }`
  - `export function installedCount(games: Game[], t: Tab, cols: Collection[]): number` — the "Kurulu" counter in the sidebar.
  - The old `Filter` and `counts` are removed; all use sites change in later tasks.

- [ ] **Step 1: Failing tests (`library.test.ts`)**
  - `ubisoft tab + installed excludes steam-installed merged card`: Watch_Dogs `steam` installed + `ubisoft` not installed.
    - `view(…, {platform:"ubisoft"}, true, …)` empty.
    - 1 with `{platform:"steam"}`.
    - 1 with `{kind:"all"}`.
  - `merged card counts in both store tabs`
  - `collection tab filters by any entry`
  - `parseTab unknown collection falls back to all`: `parseTab("c:7", [])` → `{kind:"all"}`. `parseTab("p:xbox", [])` → `{kind:"all"}`.
  - `tabCounts respects installedOnly`
  - Existing sorting and search tests give the same results with the new signature.

- [ ] **Step 2:** `npm test -- library` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `npm test -- library` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): tab + installed filter model`

### Task 5: Tab strip, sidebar, state and persistence

**Files:**
- Create: `src/components/TabStrip.tsx`, `src/components/TabStrip.test.tsx`
- Modify: `src/components/Sidebar.tsx`, `src/state/useLibrary.ts`, `src/App.tsx`, `src/styles/app.css`

**Interfaces:**
- Consumes: Task 3 APIs, Task 4 functions.
- Produces:
  - Added to the `useLibrary` return, replacing `filter`/`setFilter`:
    - `tab`, `setTab(t: Tab)`
    - `installedOnly`, `setInstalledOnly(b: boolean)`
    - `collections: Collection[]`, `reloadCollections(): Promise<void>`
  - `setTab` and `setInstalledOnly` write the setting with `api.setUiPref`. On startup `getUiPrefs` + `getCollections` → `parseTab`.
  - `TabStrip` props:
    - `{ tab, counts: ReturnType<typeof tabCounts>, collections, installedOnly, onTab(t), onClearInstalled(), onCreate(name): Promise<void>, onTabContext(c: Collection, x, y) }`
  - `Sidebar` props:
    - `installedOnly`, `installedCount`, `onToggleInstalled` instead of `filter/counts/onFilter`.
    - `onSettings`, `onRescan`, `onAdd`, `rescanning` stay the same.

- [ ] **Step 1: Failing tests (`TabStrip.test.tsx`)**
  - Order: `Tümü`, then only stores with a count >0, then a separator (`role="separator"`), then collections (visible even at 0), then `+`.
  - Clicking `+` opens a textbox. Enter → `onCreate("AC")`. Esc → closes, no call. Empty Enter → no call.
  - When `installedOnly` the label "Kurulu ✕" is visible and clicking it calls `onClearInstalled`.
  - `aria-selected="true"` on the active tab.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - **Strip:** Single row, `overflow-x: auto`.
  - **Sidebar:** The store group is removed. "Kurulu" is now a toggle (`aria-pressed`) and has a counter.
  - **App:**
    - If the active tab's count is 0 and the tab is hidden, switch to `Tümü`.
    - The collection tab's right-click menu has "Yeniden adlandır" and "Sil".
      - Rename: inline field; a `renamingId` prop may be added to TabStrip.
      - Delete: `window.confirm("“X” koleksiyonu silinsin mi? Oyunlar kütüphanede kalır.")`.
      - If the active collection is deleted, `Tümü`.
    - API errors fall into the existing `setError` notice.
  - **Empty state text:** "Hiç oyun bulunamadı. Steam, Epic, GOG, Ubisoft ya da Riot kurulu mu? Elle eklemek için “Oyun ekle”yi kullan."

- [ ] **Step 4:** `npm test` and `npx tsc --noEmit` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): tab strip and collection tabs`

### Task 6: Context menu submenu + fit to edge + add to collection

**Files:**
- Modify: `src/components/ContextMenu.tsx`, `src/App.tsx`, `src/styles/app.css`
- Create: `src/components/ContextMenu.test.tsx`

**Interfaces:**
- Produces:
  - `export type MenuItem = { label: string; onSelect?: () => void; checked?: boolean; items?: MenuItem[] }`
    - An item with `items` opens a submenu (hover/→ and click).
  - `export function clampMenu(x: number, y: number, w: number, h: number, vw: number, vh: number): { x: number; y: number }`
  - `ContextMenu` props: `{ x, y, items: MenuItem[], onClose }`. `clampMenu` is applied after measuring. If the submenu does not fit on the right it opens to the left.

- [ ] **Step 1: Failing tests**
  - `clampMenu keeps menu inside viewport`:
    - `clampMenu(790, 590, 200, 150, 800, 600)` → `{x: 590, y: 440}`.
    - A point inside is unchanged.
    - A negative result is clipped to 0.
  - `submenu item toggles`:
    - Under "Koleksiyona ekle ›" a `checked` item has `aria-checked="true"`.
    - On click `onSelect` is called and the menu closes.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - "Koleksiyona ekle ›" is added to the card menu in App. In the submenu each collection is listed with `checked: inCollection(g,c)`. On click `setInCollection(c.id, entries, !checked)` with all entries of the card, then `reloadCollections()`.
  - At the end of the submenu there is "Yeni koleksiyon…": `window.prompt("Koleksiyon adı")` → `createCollection` → add the card → `reloadCollections`. The active tab does not change.

- [ ] **Step 4:** `npm test` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): add-to-collection submenu, menu fits to edge`

### Task 7: List view

**Files:**
- Create: `src/components/GameList.tsx`, `src/components/GameList.test.tsx`
- Modify: `src/App.tsx`, `src/components/TabStrip.tsx` (view switch on the right), `src/state/useLibrary.ts` (`viewMode`, `setViewMode` → `ui.view`), `src/styles/app.css`

**Interfaces:**
- Produces:
  - `GameList` props, same as `GameGrid`: `{ games, selectedId, onSelect, onContext(g, x, y), onLaunch(g) }`.
  - `onLaunch` uses the existing `launch` flow via the card's `currentEntry`.
  - `viewMode: "grid" | "list"`.

- [ ] **Step 1: Failing tests (`GameList.test.tsx`)**
  - The row shows the name, store badges (Steam first), "Kurulu"/"Kurulu değil" and playtime. The time is the sum of `playtime_min`; "—" if 0; in the format `12 sa`.
  - The button says "Oyna" or "Kur".
  - Right-click calls `onContext`, click calls `onSelect`.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - Fixed row height with `@tanstack/react-virtual`. The cover is small, using the `Cover` component.
  - The view switch is a two-icon button (`aria-pressed`) to the right of TabStrip.

- [ ] **Step 4:** `npm test`, `npx tsc --noEmit` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): list view`

### Task 8: Applying the theme + Settings buttons

**Files:**
- Create: `src/state/useTheme.ts`, `src/state/useTheme.test.ts`
- Modify: `src/App.tsx`, `src/components/SettingsDialog.tsx`, `src/styles/tokens.css` / `app.css` (hardcoded rgba colors derived from the accent become `color-mix(in srgb, var(--accent) N%, transparent)`)

**Interfaces:**
- Produces:
  - `export function applyTheme(colors: Record<string,string>, root: HTMLElement, supports: (v: string) => boolean): string[]`
    - First clears the inline values of the 10 variables, then writes the valid ones with `setProperty`.
    - Returns a warning for invalid values: `Tema: "--accent" geçersiz renk`.
  - `export function useTheme(onWarnings: (w: string[]) => void): { reload(): Promise<void>; reset(): Promise<void> }`
    - Calls `api.getTheme()` on startup and on every `window` focus. It is independent of the Galaxy 10 s limit.
    - If the warning list is the same as the previous one, `onWarnings` is not called.

- [ ] **Step 1: Failing tests (`useTheme.test.ts`)**
  - `invalid color is skipped`: `applyTheme({"--accent":"kırmızımsı","--bg":"#000"}, el, v => v.startsWith("#"))`.
    - `--bg` is written, `--accent` is not written.
    - 1 warning is returned.
  - `reapply clears previous overrides`: On the second call a variable that is no longer present is removed from inline.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3: Implement**
  - App adds the `useTheme` warnings to the existing `warnings`/`notice` line.
  - An "Görünüm" section is added to SettingsDialog:
    - "Tema dosyasını aç" → `api.openThemeFile()`
    - "Varsayılana dön" → `reset()`

- [ ] **Step 4:** `npm test`, `npx tsc --noEmit`, `cargo check -p hangar` → PASS.
- [ ] **Step 5: Commit**
  - `feat(ui): theme file is applied`

## Track B — Small gaps (parallel with Task 1–8, a single subagent)

### Task 9: Known small gaps

**Files:** only Track B files (Global Constraints). Each item is a separate commit, `fix(...)`.

- [ ] **9a — Launch script** (`src-tauri/src/commands.rs` ~240-246):
  - When quoting an argument, runs of `\` before `"` are doubled (Windows CommandLineToArgvW rule); a trailing run of `\` is also doubled.
  - A pure `fn quote_arg(s: &str) -> String` is written.
  - Tests: `quote_arg(r#"a\"b"#) == r#""a\\\"b""#`, `quote_arg(r"C:\dir\") == r#""C:\dir\\""#`, `quote_arg("plain") == "plain"`.
- [ ] **9b — Scan exclusion** (`crates/hangar-core/src/local_scan.rs` ~10-20):
  - `crash` is excluded only if the file name contains `crash` and also contains `handler|report|pad`.
  - `unins` becomes a prefix match.
  - Tests: `CrashBandicoot.exe` is not excluded; `UnityCrashHandler64.exe` and `unins000.exe` are excluded; `MyUninstaller.exe` is not excluded.
- [ ] **9c — Broken `scan_folders` JSON** (`commands.rs` `scan_folders_of` ~439):
  - On a parse error, instead of an empty list `Err("Tarama klasörleri okunamadı; Ayarlar'dan yeniden seç")` is returned. This error is carried to `get_scan_folders` and the scan warning.
  - Test: with a broken value `get_scan_folders` returns `Err`.
- [ ] **9d — Riot PBE fixture:** A Riot provider test is added for the existing but unused PBE fixture. A PBE install should be detected as a separate entry or be excluded according to the expected behavior; a test that pins the current code's behavior is enough.
- [ ] **9e — "Kaldır" confirmation** (`src/components/Stage.tsx` ~109):
  - If `window.confirm("“<ad>” Hangar'dan kaldırılsın mı? Oyun dosyaları silinmez.")` is cancelled, `onRemove` is not called.
  - Vitest test.
- [ ] **9f — Local card with a missing exe** (`src/components/PlayButton.tsx`):
  - If the local entry's exe is missing, the label becomes "Exe bulunamadı · Yerel" instead of "Kur · Yerel" and the button is disabled.
  - A case is added to the existing PlayButton tests.
- [ ] **9g — Focus trap in the Settings window** (`src/components/Modal.tsx`):
  - Tab and Shift+Tab cycle inside the modal.
  - Test: Tab on the last item → the first item.
- [ ] **9h — README:** A SteamID note is added to the "Steam API anahtarı" section: instead of a username, the 17-digit SteamID64 or leave it empty; if empty it is taken from `loginusers.vdf`.
- [ ] **Verification:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit` → PASS.

---

## Finish
- A whole-branch Opus review is done.
- Live check with the user: the 6-step "Live" list in the spec.
- `CLAUDE.md` is updated: closed gaps are deleted.
- Squash merge PR.
