# Hangar Step 1 — all games in one place (design)

Date: 2026-11-11 · Status: awaiting approval · Previous: `2026-11-09-hangar-design.md` (v1)
Decision process: the user's list + LLM Council (5 advisors, 3 anonymous reviews) + chair synthesis.

## Purpose and rationale
The user wants to see every game on their computer in Hangar and play it by pressing "Oyna" (Play), without opening anything else. v1 covers Steam, Epic and Ubisoft and has been verified on Windows. The real work of Step 1 is the remaining four sources: **GOG**, **games with their own client (Riot)**, **local games not tied to a launcher**, **manual add**.

Success: at least one game from each source appears in the library as a single card and "Oyna" launches it (except GOG: see the verification note below).

## Goals
1. **Add game (manual):** pick an exe + type a name → added to the library. Editable (name, exe) and deletable. Rescanning does not touch these.
2. **Riot:** installed Riot products (currently LoL, TFT) appear from the metadata files in `C:\ProgramData\Riot Games\` and launch via the Riot Client.
3. **Local scan:** folders chosen by the user (e.g. `D:\Games`) are scanned; findings are shown as a **suggestion list**, and the user adds the ones they tick. No automatic adding. For a game that is not found, it points to "Oyun ekle" (Add game).
4. **GOG:** games installed with GOG Galaxy appear and launch through Galaxy.
5. If the same game is in multiple sources, it is a **single card**; in a merged card the default is always **Steam** (then Epic, Ubisoft, GOG, Riot, Local).

## Non-goals
- Astryx / React 19 migration / rewriting the UI (council: beta, no payoff; after Step 1 visual improvement can be done with the existing CSS).
- Scanning the whole disk or Uninstall records in the background.
- Other launchers such as Battle.net, EA, Xbox.
- Playtime tracking, icon extraction from exe, download/update (Step 2), store (Step 3).

## Anti-goals (count as failure even if it works)
- The library filling up with non-game programs (Discord, 7-Zip, drivers).
- A manually added game disappearing or duplicating on rescan.
- Another platform than Steam being the default in a merged card.
- Declaring GOG "done" without trying it with a real game.

## Constraints
- The architecture does not change: `crates/hangar-core` (pure Rust, tested) + thin `src-tauri` + React 18 UI.
- Surgical changes, no unrequested features; each item gets its own branch/PR (squash), each with its own verification.
- The user is close to the usage limit: small steps, at most 2-3 Sonnet subagents.
- Before adding a new table, `%APPDATA%\com.hangar.app\hangar.db` is backed up by hand (`hangar.db.bak-v1`).

## Approaches (evaluated)
| Approach | Pro | Con | Decision |
|---|---|---|---|
| A. Each source has its own `LibraryProvider`; shared "exe + arguments" launching for Riot/GOG/manual | Fits the existing structure, each is tested separately | 3 new platform values (Rust+TS+DB) | **Chosen** |
| B. A single "custom launcher" entry (exe+arg by hand) instead of Riot/GOG | Least code | The user has to enter LoL by hand; the requested "automatic detection" is missing | Rejected |
| C. General scan: Uninstall records + Start menu + filters | Most automatic | No accuracy criterion, constant false-positive tuning | Rejected; replaced by selected folders + suggestion list |

## Decided design

### Model
- `Gog`, `Riot`, `Local` are added to the `Platform` enum (serde: `gog`, `riot`, `local`). `Platform::ALL` priority order: Steam, Epic, Ubisoft, Gog, Riot, Local. The TS `Platform` type and the badges (`GOG`, `RIO`, `LOC` abbreviations fit the existing 3-letter layout) are updated in the same order.
- **Launching is of two kinds:** the existing URI (`steam://` etc., opened via the opener) or **exe + arguments** (Rust `std::process::Command`, working directory = the exe's folder, Hangar detaches without waiting). An argument list is added to `PlatformEntry`; if `launch_uri` is an exe path rather than a URI, it is launched as an exe path. The exact field name/shape is the builder's decision; the rule: opener if it is a URI, exe otherwise. For an exe that requires admin rights (Windows error 740), launching goes through the Windows shell (so the UAC prompt appears); on error, a single-line message in Turkish.

### 1) Add game (Local, manual)
- New table `local_games(id INTEGER PK, title TEXT, exe_path TEXT, args TEXT NULL, source TEXT 'manual'|'scan', added_at INTEGER)`. Scanning does not delete this table; `LocalProvider` produces `PlatformEntry` from this table (`platform_id` = the table id).
- If the exe is not on disk, the entry counts as "not installed" and Oyna gives an error message (the card does not disappear).
- Commands: `add_local_game(title, exe_path)`, `update_local_game(id, title, exe_path)`, `remove_local_game(id)`. For exe selection, the Tauri dialog plugin (`tauri-plugin-dialog`, file-open permission only, `.exe` filter).
- UI: "Oyun ekle" button in the sidebar → small dialog (pick exe, the name is suggested automatically from the exe/folder name, can be changed). "Düzenle" (Edit) and "Kaldır" (Remove) in the Local card's "…" menu.
- **Merging:** Local entries do **not merge automatically** with other platforms (if the user added the same game on purpose, they merge manually with the existing "merge" action). The Steam default rule also applies in the merged card.

### 2) Riot
- `RiotProvider`: `C:\ProgramData\Riot Games\RiotClientInstalls.json` → `rc_default` (RiotClientServices.exe). `product_install_full_path` from `Metadata\<product>.<patchline>\<product>.<patchline>.product_settings.yaml`; **if this folder exists on disk, it is installed**. Name: a fixed table for known products (`league_of_legends` → "League of Legends", `teamfighttactics` → "Teamfight Tactics", `valorant` → "VALORANT"), unknown products are skipped. PBE patchlines become separate entries by appending " (PBE)" to the name.
- Launch: exe = `rc_default`, arguments `--launch-product=<product> --launch-patchline=<patchline>`.
- Instead of a full YAML parser, a small reader that reads only the needed top-level key is enough (builder's decision). Fixture tests: shortened samples from real files.
- Expected on this machine: League of Legends and Teamfight Tactics appear, PBE does not.

### 3) Local scan (suggestion list)
- A "Oyun klasörleri" (Game folders) list in Settings (add/remove, folder picker) — setting key `scan_folders` (JSON array).
- "Klasörleri tara" (Scan folders) → each **first-level subfolder** of every selected folder is a candidate; within the candidate (at most 2 levels deep) the **largest** suitable exe is the suggested exe. Excluded exe names: `unins*`, `*setup*`, `*install*`, `*redist*`, `vc_redist*`, `dxsetup*`, `*crash*`, `UnityCrashHandler*`, `*updater*`. `*launcher*` is not excluded (some games' main exe is named like that). The exact list is the builder's decision, pinned by tests.
- Exes under the `install_path` of already known entries, or already in `local_games`, are not candidates.
- The result is a tickable list in a dialog (names editable); "Ekle" (Add) writes the selected ones to `local_games` with `source='scan'`. If there are no candidates: "Bulunamadı — Oyun ekle ile elle ekleyebilirsin." (Not found — you can add manually with Add game.)

### 4) GOG
- `GogProvider`: `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<gameId>` keys (`gameName`, `path`, `exe`); Galaxy path `HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths\client`. Launch: `GalaxyClient.exe /command=runGame /gameId=<id> /path="<path>"`; if there is no Galaxy, the game's `exe` directly.
- On this machine Galaxy is installed but there is no GOG game → the test covers only the parsing logic. **Verification:** the user installs a free game from GOG and Oyna is tried; until then it stays "not verified" in CLAUDE.md.

## Order and the "done" check of each item
| # | Item | Counts as done when |
|---|---|---|
| 0 | Shared foundation: new platforms + exe launching | core tests (Steam default, exe/URI distinction) pass |
| 1 | Add game | Any exe is added, Oyna opens it, edit/remove works, it does not disappear after Refresh |
| 2 | Riot | LoL card appears, Oyna opens the Riot Client and starts LoL |
| 3 | Local scan | A candidate list comes from the selected folder, non-game tools are not in the list, the added game starts |
| 4 | GOG | Fixture tests pass; "verified" once Oyna is tried with a real GOG game |

Each item: `cargo test -p hangar-core`, `npx tsc --noEmit`, vitest, trying with the user via `npm run tauri dev` → separate PR, squash.

## Left to the builder
Field names and the exact data shape, the YAML reader method, the final form of the exe exclusion list, the visual detail of the dialogs (matching the existing "Kokpit + Vitrin" style), cover lookup for Riot/GOG/Local cards (the existing title-based cover search flow is used; the existing placeholder if not found).
