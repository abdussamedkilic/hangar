# Hangar Step 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show and launch Riot, GOG and launcher-less local games (manual add + opt-in folder scan) next to Steam/Epic/Ubisoft, with Steam staying the default on merged cards.

**Architecture:** Each new source is a `LibraryProvider` in `crates/hangar-core` (pure, unit-tested). A launch is either a URI (opened by tauri-plugin-opener, as today) or an exe + args (spawned by `src-tauri`). Local games live in their own `local_games` table, so rescans never touch them; `LocalProvider` turns rows into entries. UI changes are small additions to the existing React components.

**Tech Stack:** Rust (rusqlite, winreg 0.52, serde_json), Tauri 2 (+ new `tauri-plugin-dialog` 2), React 18 + TS + Vitest.

**Spec:** `docs/superpowers/specs/2026-11-11-hangar-step1-design.md`

## Global Constraints
- Platform priority (`Platform::ALL`): Steam, Epic, Ubisoft, Gog, Riot, Local. Serde names: `steam`, `epic`, `ubisoft`, `gog`, `riot`, `local`. Card badges: `STE`, `EPI`, `UBI`, `GOG`, `RIO`, `LOC`. Sidebar labels: Steam, Epic, Ubisoft, GOG, Riot, Yerel.
- Local entries are never auto-merged with other entries (explicit merge overrides still apply).
- A launch string containing `://` is a URI; anything else is an exe path launched with `launch_args`, working directory = the exe's folder.
- No new features beyond the spec; no Astryx/React 19; UI copy in Turkish.
- Before the first run that creates `local_games`, copy `%APPDATA%\com.hangar.app\hangar.db` to `hangar.db.bak-v1` (manual, once).
- One task = one branch commit series; after each task: `cargo test -p hangar-core`, `npx tsc --noEmit`, `npx vitest run`, then the user tries it in `npm run tauri dev`. Commit messages: English conventional (`feat(core): ...`).
- npm/npx are only on PATH in PowerShell after `fnm env --shell powershell | Out-String | Invoke-Expression`.

## Review Focus
1. Exe paths with spaces or Turkish characters (`C:\Oyunlar\Şehir Yapımı\game.exe`) must launch unchanged → test in Task 1.
2. A local game whose exe was deleted stays as a card, shows not-installed, and Play returns `"Oyun dosyası bulunamadı"` instead of failing silently → test in Task 2.
3. Rescanning twice never duplicates or drops local games → test in Task 2.
4. A local game with the same title as a Steam game stays a separate card; a merged Steam+other card still launches Steam by default → tests in Task 1.
5. Missing/corrupt Riot metadata or GOG registry, or an unreadable scan folder, yields an empty result (or a skipped folder), never a scan failure → tests in Tasks 3, 4, 5.

---

### Task 1: Shared foundation — new platforms and exe launching

**Files:**
- Modify: `crates/hangar-core/src/model.rs` (Platform, PlatformEntry)
- Modify: `crates/hangar-core/src/merge.rs` (`group`: Local never auto-grouped)
- Modify: `crates/hangar-core/src/library.rs` (add `launch_target`)
- Modify: `src-tauri/src/commands.rs` (`launch`)
- Modify: `src/lib/api.ts`, `src/lib/library.ts` (`PLATFORMS`), `src/components/GameCard.tsx` (`ABBR`), `src/components/Sidebar.tsx` (`PLAT`)
- Modify: every existing `PlatformEntry { .. }` literal in core tests/providers (add `launch_args: vec![]`)

**Interfaces:**
- Produces: `Platform::{Gog, Riot, Local}`; `PlatformEntry.launch_args: Vec<String>` with `#[serde(default)]`; `pub enum LaunchTarget { Uri(String), Exe { path: PathBuf, args: Vec<String> } }` and `pub fn launch_target(entry: &PlatformEntry, launch: &str) -> LaunchTarget` in `library.rs`.

- [ ] **Step 1: Write failing core tests** (in the existing `#[cfg(test)]` modules)
```rust
#[test] fn platform_priority() { assert_eq!(Platform::ALL, [Steam, Epic, Ubisoft, Gog, Riot, Local]); }
#[test] fn old_json_without_launch_args_still_loads() { /* serde_json::from_str of a v1 entry JSON */ assert!(e.launch_args.is_empty()); }
#[test] fn uri_vs_exe() {
    assert_eq!(launch_target(&e, "steam://rungameid/1"), LaunchTarget::Uri("steam://rungameid/1".into()));
    let e = PlatformEntry { launch_args: vec!["--x".into()], ..e };
    assert_eq!(launch_target(&e, r"C:\Oyunlar\Şehir Yapımı\game.exe"),
        LaunchTarget::Exe { path: r"C:\Oyunlar\Şehir Yapımı\game.exe".into(), args: vec!["--x".into()] });
}
#[test] fn local_never_auto_merges() { /* group([steam "Hades", local "Hades"], &[]) */ assert_eq!(groups.len(), 2); }
#[test] fn local_merges_with_explicit_override() { /* Override{merged:true} steam↔local */ assert_eq!(groups.len(), 1); }
#[test] fn steam_default_on_merged_card() { /* game with Riot + Steam + Local entries */ assert_eq!(resolve(&g, None).unwrap().0.platform, Platform::Steam); }
```
- [ ] **Step 2:** `cargo test -p hangar-core` → new tests FAIL to compile/assert.
- [ ] **Step 3: Implement.** Enum variants + `ALL` + `as_str`; `launch_args`; in `group`, mark `Platform::Local` entries like a "dup" (own key `norm#local:<id>`, excluded from the seed loop); `launch_target` = `Uri` if `launch.contains("://")`, else `Exe`.
- [ ] **Step 4: `commands::launch`.** Match `launch_target(entry, &uri)`: `Uri` → opener as today; `Exe` → if `!path.exists()` return `Err("Oyun dosyası bulunamadı".into())`; else `std::process::Command::new(&path).args(&args).current_dir(<exe folder>).spawn()`; if the error's `raw_os_error() == Some(740)` (elevation required) retry via `powershell -NoProfile -Command Start-Process -FilePath '<path>' -ArgumentList <args> -WorkingDirectory '<dir>' -Verb RunAs` (single quotes in values doubled). Other errors → `Err(format!("Açılamadı: {e}"))`. `record_launch` stays as is.
- [ ] **Step 5: UI types.** `Platform` union + `PLATFORMS` + `ABBR` + `PLAT` per Global Constraints. Sidebar shows a platform row only when its count > 0 for the new three (Steam/Epic/Ubisoft rows unchanged).
- [ ] **Step 6:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npx tsc --noEmit`, `npx vitest run` → all PASS.
- [ ] **Step 7: Commit** `feat(core): gog/riot/local platforms and exe launching`

### Task 2: Add Game (manual local games)

**Files:**
- Modify: `crates/hangar-core/src/db.rs` (table + CRUD)
- Create: `crates/hangar-core/src/providers/local.rs`; Modify: `providers/mod.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/main.rs` (plugin + handlers), `src-tauri/capabilities/default.json` (`"dialog:allow-open"`), `src-tauri/src/commands.rs`
- Create: `src/components/AddGameDialog.tsx` + `AddGameDialog.test.tsx`; Modify: `src/lib/api.ts`, `src/components/Sidebar.tsx`, `src/components/Stage.tsx`, `src/App.tsx`; `package.json` (`@tauri-apps/plugin-dialog`)

**Interfaces:**
- Consumes: Task 1 `Platform::Local`, `launch_args`.
- Produces: `pub struct LocalGame { pub id: i64, pub title: String, pub exe_path: String, pub args: Vec<String>, pub source: String }`; `Db::{add_local_game(&self, title:&str, exe:&str, source:&str) -> Result<i64>, update_local_game(&self, id:i64, title:&str, exe:&str) -> Result<()>, remove_local_game(&self, id:i64) -> Result<()>, local_games(&self) -> Result<Vec<LocalGame>>}`; `LocalProvider::new(Vec<LocalGame>)` (platform_id = id as string, installed = exe exists, launch_uri = exe_path, install_path = exe folder). Tauri commands `add_local_game(title, exe_path)`, `update_local_game(id, title, exe_path)`, `remove_local_game(id)` → each returns `ScanReport` after a rescan. TS `api.addLocalGame/updateLocalGame/removeLocalGame`.

- [ ] **Step 1: Failing tests.** `db`: add→list→update→remove round-trip; `local_games` survives `replace_entries`. `local.rs`: missing exe → `installed == false`; existing temp file → `installed == true`, `launch_uri == path`. `library`: `scan_all` twice with the same `LocalProvider` → game count unchanged and one local entry.
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: Implement** table `local_games(id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, exe_path TEXT NOT NULL, args TEXT NOT NULL DEFAULT '[]', source TEXT NOT NULL, added_at INTEGER NOT NULL)` in `init` (CREATE IF NOT EXISTS), CRUD, `LocalProvider`; `scan()` in commands reads `local_games` under the DB lock and pushes `LocalProvider` last. Removing a local game also deletes its merge overrides is NOT required (stale overrides are harmless).
- [ ] **Step 4: UI test** (`AddGameDialog.test.tsx`, dialog plugin mocked): choosing `C:\Games\Hades\Hades.exe` pre-fills name `Hades`; "Ekle" calls `api.addLocalGame("Hades", "C:\\Games\\Hades\\Hades.exe")`; empty name disables "Ekle".
- [ ] **Step 5: Implement UI.** Sidebar button "Oyun ekle" (above "Yenile"); dialog via existing `Modal`: "Exe seç" (dialog `open({ filters: [{ name: "Oyun", extensions: ["exe"] }] })`), "Ad", "Ekle"/"Vazgeç". Name suggestion: exe file name without extension. Stage "…" menu for games with a local entry: "Düzenle" (same dialog, prefilled, calls update) and "Kaldır" (calls remove). Results flow into the existing `notice`/library refresh like `rescan`.
- [ ] **Step 6:** all checks PASS; user adds any exe in `tauri dev`, plays it, edits, removes, presses Yenile.
- [ ] **Step 7: Commit** `feat: add game (manual local game), edit and remove`

### Task 3: Riot provider

**Files:**
- Create: `crates/hangar-core/src/providers/riot.rs`, `crates/hangar-core/tests/fixtures/riot/` (`RiotClientInstalls.json`, `league_of_legends.live.product_settings.yaml`, `league_of_legends.pbe.product_settings.yaml` — trimmed copies of the real files: keep `product_install_full_path`, `product_install_root`)
- Modify: `providers/mod.rs`, `src-tauri/src/commands.rs` (push provider)

**Interfaces:**
- Produces: `pub fn parse_client_path(json:&str) -> Option<String>` (`rc_default`), `pub fn install_path_from_yaml(yaml:&str) -> Option<String>` (top-level `product_install_full_path`, quotes stripped), `pub fn product_title(product:&str, patchline:&str) -> Option<String>`, `RiotProvider::detect() -> Option<RiotProvider>` (root `C:\ProgramData\Riot Games`), `RiotProvider::with_root(PathBuf)` for tests.
- Entry: `platform_id = "<product>.<patchline>"`, `launch_uri = rc_default`, `launch_args = ["--launch-product=<product>", "--launch-patchline=<patchline>"]`, `installed = true` (only installed ones are emitted), `install_path` from YAML.

- [ ] **Step 1: Failing tests.** `product_title("league_of_legends","live") == Some("League of Legends")`, `("teamfighttactics","live") → "Teamfight Tactics"`, `("valorant","live") → "VALORANT"`, `("league_of_legends","pbe") → "League of Legends (PBE)"`, `("bacon","live") → None`; fixture JSON → `"C:/Riot Games/Riot Client/RiotClientServices.exe"`; YAML → `"C:/Riot Games/League of Legends"`; `with_root(temp)` where only the LoL live install dir exists → exactly one entry with the args above; root with no `RiotClientInstalls.json` → `scan()` returns `Ok(vec![])`.
- [ ] **Step 2:** run → FAIL. **Step 3:** implement (line-based YAML read, no new crate). **Step 4:** PASS.
- [ ] **Step 5:** user check: League of Legends and Teamfight Tactics cards appear; Oyna opens Riot Client and starts LoL.
- [ ] **Step 6: Commit** `feat(core): riot games (league of legends, tft)`

### Task 4: Folder scan suggestions

**Files:**
- Create: `crates/hangar-core/src/local_scan.rs`; Modify: `lib.rs`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`
- Create: `src/components/ScanDialog.tsx` + test; Modify: `src/components/SettingsDialog.tsx`, `src/lib/api.ts`

**Interfaces:**
- Consumes: Task 2 `Db::add_local_game(.., "scan")`, `local_games()`.
- Produces: `pub struct Candidate { pub title: String, pub exe_path: String }`; `pub fn is_excluded_exe(file_name:&str) -> bool`; `pub fn candidates(folders:&[PathBuf], known:&[PathBuf]) -> Vec<Candidate>` (for each first-level subfolder, the largest non-excluded `.exe` within 2 levels; title = subfolder name; skip a subfolder whose path is inside any `known` path or whose chosen exe is in `known`; unreadable folders skipped). Settings key `scan_folders` (JSON array of strings). Commands `get_scan_folders() -> Vec<String>`, `set_scan_folders(folders: Vec<String>)`, `scan_candidates() -> Vec<Candidate>` (known = all entries' `install_path` + local exe paths), `add_local_games(items: Vec<Candidate>) -> ScanReport`.

- [ ] **Step 1: Failing tests.** `is_excluded_exe` true for `unins000.exe`, `Setup.exe`, `vc_redist.x64.exe`, `UnityCrashHandler64.exe`, `DXSETUP.exe`, `GameUpdater.exe`, `install.exe`; false for `Hades.exe`, `RiotClientLauncher.exe`, `Launcher.exe`. Temp tree `Games/Hades/{Hades.exe (2 KB), unins000.exe (5 KB)}`, `Games/Empty/`, `Games/Known/a.exe` with `known=[Games/Known]` → exactly `[Candidate{title:"Hades", exe_path:".../Hades.exe"}]`. Non-existent folder → empty, no panic.
- [ ] **Step 2:** FAIL. **Step 3:** implement. **Step 4:** PASS.
- [ ] **Step 5: UI.** Settings gets "Oyun klasörleri": list with "Kaldır", "Klasör ekle" (dialog `open({ directory: true })`), "Klasörleri tara" → `ScanDialog`: checklist (all ticked by default, names editable), "Seçilenleri ekle"; zero candidates → text `Bulunamadı — Oyun ekle ile elle ekleyebilirsin.` Test: two candidates, untick one, submit → `api.addLocalGames` called with the ticked one only.
- [ ] **Step 6:** checks PASS; user picks a real games folder; list holds no setup/redist tools; an added game launches.
- [ ] **Step 7: Commit** `feat: game suggestion scan in selected folders`

### Task 5: GOG provider

**Files:**
- Create: `crates/hangar-core/src/providers/gog.rs`; Modify: `providers/mod.rs`, `src-tauri/src/commands.rs`
- Modify: `CLAUDE.md` (status: GOG "unverified" until a real game is tried)

**Interfaces:**
- Produces: `pub struct GogRecord { pub game_id: String, pub name: String, pub path: String, pub exe: String }`; `pub fn entries_from(records: Vec<GogRecord>, galaxy: Option<&Path>) -> Vec<PlatformEntry>`; `GogProvider::detect()` reads `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\*` (`gameID`, `gameName`, `path`, `exe`) and `...\GOG.com\GalaxyClient\paths\client`; no Games key → `Ok(vec![])`.
- Entry: `platform_id = game_id`, `installed = Path::new(&exe).exists()`, with Galaxy: `launch_uri = <client>\GalaxyClient.exe`, `launch_args = ["/command=runGame", "/gameId=<id>", "/path=<path>"]`; without Galaxy: `launch_uri = exe`, `launch_args = []`.

- [ ] **Step 1: Failing tests** for `entries_from`: with Galaxy path → args exactly as above; without → `launch_uri == exe`; record with empty `name` skipped.
- [ ] **Step 2:** FAIL. **Step 3:** implement. **Step 4:** PASS.
- [ ] **Step 5:** update `CLAUDE.md` (Step 1 status, GOG unverified; v1.1 ideas list → done items), commit `feat(core): gog games (unverified)`.
- [ ] **Step 6:** open PR `feat/step1` → `main`, squash merge after the user's final check.
