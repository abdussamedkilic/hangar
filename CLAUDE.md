# Hangar — project memory

Windows desktop app that merges Steam, Epic and Ubisoft games into a single library. User (speaks Turkish, Windows 11 with Turkish locale, user folder `C:\Users\user`, repo `C:\Users\user\workdir\hangar`).

## Status (2026-11-11)
- v1 is on `main` (PR #1 squash-merged, 2026-11-11; GitHub: <owner>/hangar). New work goes on a new branch from `main`.
- `npm run tauri dev` ran on Windows: Steam (DAVE THE DIVER, Watch_Dogs), Epic (Death's Door), Ubisoft (Watch_Dogs → merged with Steam) came through correctly, covers downloaded.
- Fixed: entering a username (`player`) instead of a SteamID in Settings made GetOwnedGames fail silently → `steam::is_steamid64`; if it is not 17 digits, it is taken from `loginusers.vdf`.
- 2026-11-11: Riot/folder scan/manual add/GOG added on the `feat/step1` branch (GOG not verified); PR after the user's check.
- Note: `localhost:1420` does not work in a browser (no Tauri `invoke`) — test only in the Tauri window.

## LATEST STATUS (2026-11-14 evening)
- Phase 4 is on main via PR #7 squash (Steam/GOG sign-in, filters, collapsible menu, drag-and-drop, Settings tabs). The user tried it live.
- 3 tasks remain to finish the project (new branch from main):
  1. Unwanted scroll in the collapsed sidebar (vertical + horizontal ◀▬▶) — overflow in the 64px rail.
  2. Theme picker in Özelleştir (Customize): active theme in the select box; a new theme is saved locally; switching between themes.
  3. Desktop exe: `npm run tauri build` → NSIS + desktop shortcut.

## Upcoming tasks (in order, step by step)
1. ✅ (2026-11-11) Uninstalled Steam games now come via the API.
2. ✅ (2026-11-11) Play: Steam, Epic, Ubisoft all launched. (Ubisoft Connect once got stuck on "initializing" — caused by Connect; Hangar opens `uplay://` on every click, there is no lock but also no feedback.)
3. ✅ (2026-11-11) Warnings show in the top `notice` row; warnings from a scan started in Settings used to disappear → the dialog now calls App's `rescan`.
4. ✅ (2026-11-11) PR #1 squash-merged into `main`.
5. ✅ (2026-11-11) Step 1 (`feat/step1`): Add game (manual add), Riot provider (League of Legends, TFT), game-suggestion scan in selected folders, GOG provider. Awaiting the user's manual check; then PR to `main`.
   - GOG is **not verified** — not tried with a real GOG game (this PC has Galaxy but no games; registry `GOG.com\Games`).
   - User tried (2026-11-11): Riot/LoL launches, TFT is found, Watch_Dogs (Steam) launches through Ubisoft Connect (normal).
   - Extra (same branch): uninstalled GOG games from the GOG Galaxy local DB (`C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db`, read-only) (161 on this PC); "Kapak seç…" (Pick cover…) from the card; games no longer start as a child process of Hangar (PowerShell `Start-Process`, values via env) — a `tauri dev` restart used to kill LoL.
   - Cover code from the Riot icon exists, but the LoL `.ico` on this PC has no PNG frame → cover set by hand with "Kapak seç…".
   - No uninstalled games for Epic: no reliable local list, account sign-in needed (separate design).
   - ✅ (2026-11-11) PR #2 squash-merged into `main`. User tried GOG: "Kur" (Install) on an uninstalled game opens the install in Galaxy.
6. **Next (2026-11-11, requested by the user):** (a) Uninstalled Epic games (no reliable local list; account sign-in or another source — design first), (b) all owned games in Ubisoft Connect (currently only installed ones, from the registry). Brainstorming + decision first, then plan.
   - **Decision (user, 2026-11-12): NO Galaxy. Playnite path — in-app connection, data stays only on the user's computer.** (The council had recommended Galaxy; the user rejected it.)
     - Epic: Hangar opens Epic's own sign-in page in a separate window (Hangar never sees the password), authorization code → OAuth (Legendary/Heroic/Playnite flow, the launcher's public client id), owned games from the Epic library + catalog service. Refresh token only local, encrypted with Windows DPAPI; "Bağlantıyı kes" (Disconnect) deletes it; never logged; no server/telemetry.
     - Ubisoft: first a short, time-boxed attempt (encrypted local cache vs Ubisoft's own sign-in page + ticket); if no working path is found, only installed games remain.
     - "Hesaplar" (Accounts) section in Settings; security note in the README (where the token is, how to delete, risk of an unofficial service).
     - Next session: brainstorming → short design doc → plan → subagent-driven (Sonnet). Risks from the council: Epic may block the client id, the token can access the whole account, DPAPI does not protect against malware under the same user.
   - ✅ (2026-11-13) **Epic done** — `feat/epic-account` branch (spec `docs/superpowers/specs/2026-11-12-hangar-accounts-design.md`, plan `docs/superpowers/plans/2026-11-12-hangar-epic-account.md`). Design and plan went through the council; all tasks reviewed; final whole-branch review clean. User tried it, PR #3 squash-merged into `main` (2026-11-13).
     - E0 (live sign-in, 2026-11-12): sign-in with email works, only www.epicgames.com is visited; 240 records (225 games, 15 DLC). Gotcha: the catalog bulk endpoint returns `{}` for comma-joined ids — every id gets its own `id=` parameter. The refresh token rotates on every use, valid ~1 year.
     - Sign-in window allowlist (`accounts/login.rs` NAV_RULES): Epic + captcha + only the sign-in paths of the providers (host + path prefix, no port). User tried the other sign-ins, they work.
   - ✅ **Phase 1 finale** — PR #4 squash-merged into `main` (2026-11-13). User liked the UI a lot.
     - ✅ Other Epic sign-in providers; ✅ Epic/GOG covers (Epic keyImages, Galaxy `originalImages.verticalCover`, fallback `api.gog.com/v2/games/<id>` `_links.boxArtImage`; download host allowlist, no redirects; WebP; per-session failure cache). Reviewed.
     - ✅ (2026-11-13) **Edition/copy cards — Revision 2** (spec `docs/superpowers/specs/2026-11-13-hangar-editions-design.md`, the REVISION 2 block at the top applies; went through the council). Rule: **one card = one build**. Remastered/Definitive/Enhanced/Complete/GOTY/Gold… are separate cards (user: Infinite and Infinite Complete are separate); copies of the same build (stores, ™, "- Amazon Prime" → "(Prime)", two same-named records in the same store → "· 2. kopya" (2nd copy)) form a single card. Card key: Original → `normalize`, others → `normalize|Label` (rows/covers of original cards are preserved). Preference is per record (`"platform:id"`, old platform value is still read). One badge per store, at most 3 + "+N", tooltip lists all copies. Candidates in the merge picker show store badges.
       - Dry run (copy of the live DB): `HANGAR_DB=<copy> HANGAR_DRY_OUT=<output.md> cargo test -p hangar-core dry_run_live_cards -- --ignored` — card list + multiple cards with the same base name.
       - The user had accidentally merged Mafia II DE with Mafia III DE; the override was deleted from the DB (backup in scratchpad).
     - In debug, `await hangarProbe()` (DevTools) writes cleaned live responses under `%APPDATA%\com.hangar.app\e0\`; it does not go into the repo. Local live test: `HANGAR_EPIC_LIVE_DIR="$APPDATA/com.hangar.app/e0" cargo test -p hangar-core live_ -- --ignored`.
   - ✅ **PHASE 2 done — PR #5 squash-merged into `main` (2026-11-14); ✅ PHASE 3 done — PR #6 squash-merged into `main`:** collections (membership by record key), tab strip, list view, `theme.json`, Steam priority (with multiple installed copies, Play opens Steam), small gaps.
   - **PHASE 4 — `feat/phase4` (in progress, not yet on `main`; 2026-11-14):**
     - Steam sign-in (no API key): persistent WebView2 profile `app_data_dir/webview-steam`; `IPlayerService/GetOwnedGames` from the same page using `data-loyalty_webapi_token` of `application_config` on the store page. Invisible window during scan, 20 s timeout, script no-login signal 12 s; 10 min limit for focus scans, Refresh skips it; if a key exists, the key path is used.
     - GOG account: Galaxy's public OAuth client `46899977096215655`; `getFilteredProducts mediaType=1`; source choice galaxy/account (galaxy behavior when not connected).
     - Savable filters (Installed is a virtual built-in; `ui.filter`), collapsible sidebar (`ui.sidebar`, 220/64px), drag a card onto a collection tab (`dragDropEnabled:false`), two Settings tabs: Hesaplar (Accounts) / Özelleştir (Customize).
     - Amazon Games will not be done (user decision). Desktop exe (NSIS) left for last.
     - Command/working notes: vitest with node (bun gives a misleading error): `PATH="$APPDATA/fnm/node-versions/v26.13.0/installation:$PATH" node node_modules/vitest/vitest.mjs run`. Separate git worktree + `node_modules` junction for parallel tracks. The user denied screen-control (computer-use) permission → visual checks are left to the user in live testing. The user said "you can always squash-merge into main".
   - (Old Phase 2 list, history:)
     1. ✅ (2026-11-14) **Ubisoft Connect sign-in** — the user connected, it works (spec `docs/superpowers/specs/2026-11-14-hangar-ubisoft-account-design.md`; Opus review + fixes done). `UbiClient` (rm_v1 refresh, GraphQL `games(filterBy:{isOwned:true})`, DPAPI `accounts\ubisoft.dat`), row in Hesaplar, single-candidate title match with the installed record for Ubisoft only, `uplay://launch/<spaceId>/0` (opens Connect; could not verify that it goes to the game page — Connect got stuck on "Initializing").
        - ✅ (2026-11-14) **Full list via demux** (spec `docs/superpowers/specs/2026-11-14-hangar-ubisoft-demux-design.md`): the user saw 7/7 (including Origins, Syndicate). Demux only accepts a ticket obtained with the PC app id `f68a4bb5-…`; the sign-in page stays with the web id, `connect` renews the web rememberMe once with the PC id (the web token becomes invalid), token v2. The TLS server often cuts the handshake → 6 attempts; only TLS1.2 + ECDHE-ECDSA-AES256-GCM-SHA384; clientVersion = installed upc.exe build (13416). `platform_id` is now the numeric product id (same as the registry). If demux fails, it falls back to the GraphQL list. Debug: `hangarUbiDemux()`, `hangarUbiProbe()`, `hangarUbiQuery(body)`.
     2. Small UI improvements (the user will give details).
     3. **Desktop exe** (last, when everything is done): `npm run tauri build` → NSIS installer + desktop shortcut.
     4. ✅ **New GOG game**: automatic rescan on window focus when the Galaxy DB (including WAL) changes (10 s limit).
     5. ✅ **"- Amazon Luna" suffix**: copy suffix, shown as "(Amazon)" in the menu (user decision); Prime stays "(Prime)".
     6. ~~Amazon Games provider~~ — the user did not want it (2026-11-14; does not want to install the app).
     - Working order: council → single question → spec/plan → parallel Sonnet implementation → Opus review → dry run/live check.
   - **Ubisoft (Phase 2/1):** U0 discovery (connect.ubisoft.com ticket, `rm_v1` refresh, owned games + product id matching the registry), with the user's sign-in; if successful, a short separate plan on top of the shared infrastructure (sign-in window, `SecretStore`, `OwnedLists`, Hesaplar).
   - Note (2026-11-13): the PC rebooted with a BSOD 14 times in 30 days (mostly 0x1a MEMORY_MANAGEMENT; possibly RAM/platform). One crash filled the git ref and index with zeros — repaired. Running `git fsck` before long jobs is a good idea.
7. Then the "Known minor gaps" list.

## v1.1 ideas (from the user, 2026-11-11 — not designed yet, brainstorming first)
- Feedback/status after "Oyna" (Play) (e.g. "Sent to Ubisoft Connect", try again).
- (Done: Riot provider, manual add, GOG — step 5 above.)
- Take the NVIDIA app's game scan as an example (rea-skill) — reading from local files is fragile; our own providers first.

## Documents
- Design (spec): `docs/superpowers/specs/2026-11-09-hangar-design.md`
- Plan (14 tasks, completed): `docs/superpowers/plans/2026-11-09-hangar-v1.md`
- Chosen UI direction: "5 · Kokpit + Vitrin" (Cockpit + Showcase) — reference mockup `docs/design/mockups-melez.html` (first tab).

## Architecture
- `crates/hangar-core` — pure Rust, no Tauri, tested on Linux: `vdf.rs`, `providers/{steam,epic,ubisoft}.rs` (`LibraryProvider`), `merge.rs` (name normalization + merge/split overrides), `db.rs` (SQLite), `library.rs` (scan, `resolve`), `artwork.rs` (covers).
- `src-tauri` — thin layer: `commands.rs` (get_library, rescan, launch, set_preferred, get/save_settings, split_game, merge_games; events `library-updated`, `cover-ready`).
- `src/` — React 18 + TS + Vite; GSAP, Lenis, @tanstack/react-virtual. `normalize` in `lib/library.ts` must match Rust `merge::normalize` exactly.
- Data: `%APPDATA%\com.hangar.app\hangar.db`, covers in `...\covers`.

## Commands
- On Linux/WSL: `cargo test -p hangar-core`, `cargo check -p hangar` (needs GTK/WebKit dev packages), `npm test`, `npx tsc --noEmit`, `npm run build`.
- On Windows: `npm install`, `npm run tauri dev`, `npm run tauri build` → `src-tauri/target/release/bundle/nsis/Hangar_0.1.0_x64-setup.exe` (note: since this is a workspace, `target/` is at the repo root).

## Rules (the user's)
- Primary language is English (code, comments, docs, commits). App UI copy stays Turkish. The user speaks Turkish (assistant replies in Turkish).
- Plan/design first, then code (superpowers: brainstorming → writing-plans → subagent-driven-development).
- Karpathy rules (`karpathy-guidelines` skill): no unrequested features, surgical changes, verifiable goal.
- Subagents: Sonnet, at most 2-3 at a time.
- Skills for UI work: taste-skill, unslop, ui-ux-pro-max, gsap-*; all installed under `~/.claude/skills` (Windows and WSL).
- Commits: English conventional (`feat(core): ...`), imperative.

## Known minor gaps (from Step 1 reviews, deferred)
Elevated-launch errors other than a UAC cancel are reported only in a limited way; the launch script does not double a `\` before `"` (today's arguments do not contain it); "Kaldır" (Remove) does not ask for confirmation; the row of the active filter in the sidebar disappears when its count is 0; the scan exclusion `*crash*` also drops "Crash Bandicoot", and `unins` is a contains match, not a prefix match; broken `scan_folders` JSON silently becomes an empty list; the Riot PBE fixture is unused; the empty-state text does not mention the new sources and "Oyun ekle" (Add game); a local card with a missing exe says "Kur · Yerel"; editing Rust while `tauri dev` is open restarts the app (games are no longer affected).

## Known minor gaps (post-v1)
The right-click menu can overflow the window edge; the settings dialog has no full focus trap; the README lacks a SteamID note; no feedback in Settings if getSettings fails twice; npm audit has 6 findings (all in dev tooling: the vite/esbuild/jsdom chain, no risk for local use).
