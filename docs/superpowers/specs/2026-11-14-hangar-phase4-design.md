# Hangar Phase 4 — Steam/GOG sign-in, filters, collapsible menu, drag-and-drop, sectioned Settings · Design

Date: 2026-11-14 · Status: the user approved the approaches ("continue with a", "the other things you said are good", GOG sign-in added); the S0/G0 live trials succeeded
Branch: `feat/phase4` (main = Phase 3, PR #6)

## Intent
The user wants to finish the project. Hangar must support four clients (Steam, Epic, GOG, Ubisoft) "natively"; these are all the ones the user uses ("it needs to be enough"). The UI side must also reach its final form:
- manual filters,
- a collapsible left menu,
- adding cards to a collection by dragging,
- Settings split into sections.

## Goals
1. **Steam sign-in** (user: option a). The default connection path is Steam's own sign-in page.
   - Hangar gets the SteamID and owned games without an API key.
   - The API key stays optional under "Gelişmiş" (Advanced). If a key is entered, today's GetOwnedGames path is used.
   - Distributing Hangar's own key is a non-goal (Steam terms, shared quota).
2. **GOG sign-in.** Source choice for GOG:
   - "Galaxy veritabanından bul" (Find from the Galaxy database) (today's path) or "Hesapla bağlan" (Connect with account) (the Galaxy client's public OAuth flow).
   - User: "with a tick, either let it find from the db or sign in".
   - My suggestion: the two options become a single select box, the default is the Galaxy database.
3. **Filters.**
   - The sidebar gets a "Filtreler +" (Filters +) heading. "+" creates a named filter.
   - Criteria:
     - store (one or several)
     - status: installed / not installed
     - played: never played / played
     - edition: Remastered/Definitive etc. / original only
     - collection: in / outside
   - A clicked filter is applied on top of the open tab. Only one filter is active at a time; clicking again turns it off.
   - "Kurulu" (Installed) becomes a built-in (undeletable) filter.
   - Editing and deleting are done with right-click.
   - Today's `ui.installed` preference migrates to the "Kurulu" filter being active.
4. **Collapsible menu.**
   - With "«" the menu turns into a strip made only of icons (~64px). Icons show their name in a tooltip.
   - Filters appear as an icon or an initial letter.
   - The state is persistent as `ui.sidebar = "open"|"collapsed"`.
   - The collapsed form is checked against the design-taste-frontend rules, with a screenshot of the real window.
5. **Drag-and-drop.**
   - When a card or list row is dropped onto a collection tab, all of the card's records are added to that collection (Phase 3 membership rule).
   - While dragging, the tabs that can accept a drop are highlighted.
   - After the drop a short status text is shown ("AC'ye eklendi") (Added to AC).
   - The right-click menu path remains for keyboard and accessibility.
6. **Settings.** The window gets two tabs on the left:
   - **Hesaplar (Accounts):** Steam, Epic, Ubisoft and GOG rows, with the same row pattern. The Steam row has "Gelişmiş: API anahtarı, SteamID" (Advanced: API key, SteamID), the GOG row has the source choice.
   - **Özelleştir (Customize):** theme ("Tema dosyasını aç" / "Varsayılana dön") (Open theme file / Back to default), menu (open/collapsed default), game folders.
   - My suggestion: "Özelleştir" instead of "Profil". Profile suggests identity information, while the content here is personalization.

## Non-goals
- Amazon Games.
- Steam family sharing.
- GOG Galaxy integrations (Galaxy's other platforms).
- An AND/OR tree in filters: criteria always combine with AND, and within the store criterion OR.
- Opening several filters at the same time.
- Reordering collection tabs by dragging.

## Anti-goals
- Logging sign-in information (cookie, token, SteamID) or writing it as plain text. Everything stored is encrypted with DPAPI (the Epic/Ubisoft pattern).
- The library emptying when Steam sign-in breaks. In that case it falls back to the cache and installed games, and the warning is shown as for Epic/Ubisoft.
- Loss of function in the collapsed menu: every button must be reachable by its icon.
- Drag-and-drop breaking click/selection behavior.

## Constraints
- The existing account infrastructure is used as is:
  - `login_flow` (return with nonce, navigation rules)
  - `SecretStore`/DPAPI
  - `owned_cache` and the "expired" rules
  - the `AccountsSection` row pattern
  - `Platform` dispatch in the existing account commands
- The front end builds on the Phase 3 structure:
  - the `Tab` + filter model (`library.ts`); `installedOnly` generalizes into the active filter
  - `TabStrip`, `Sidebar`, `ContextMenu`
  - `ui.*` preferences (`ui.sidebar` and `ui.filter` are added to the `set_ui_pref` whitelist)
- Colors are tokens only. New texts are Turkish.

## Approaches
- **Steam:**
  - Chosen: (1) Steam sign-in in an embedded window; owned games from `store.steampowered.com/dynamicstore/userdata/`. The method is finalized with the S0 trial.
  - Rejected: (2) Steam OpenID — it gives only the SteamID, a key is still needed for the list.
  - Rejected: (3) Embedded shared key — violates Steam's terms.
- **GOG:**
  - Chosen: (1) Galaxy's public OAuth client (the Heroic/Playnite/lgogdownloader path); `embed.gog.com/user/data/games`.
  - Rejected: (2) Galaxy DB only — today's path; sign-in was requested.
- **Storing filters:**
  - Chosen: (1) `filters(id, name, rule_json, position)` in the database. The rule JSON is evaluated in the front end; the pure `view` function expands.
  - Rejected: (2) A JSON array in a setting key — a table was chosen so that counters/deleting/renaming follow the same pattern as collections.
- **Drag-and-drop:**
  - Chosen: (1) Native HTML5 drag-and-drop (`draggable`, the card id in `dataTransfer`).
  - Rejected: (2) A library (dnd-kit) — an unnecessary dependency for a single target type.

## Left to the builder
- Icon set: whatever the existing project has (inline SVG if none).
- The layout of the filter editor (Modal).
- The drag preview image.
- Merging with the installed info coming from the Galaxy DB when the GOG source is "Hesapla": since the ids are the same GOG product id, natural merging is expected; verified after G0.

## Design finalized by trials (S0/G0, live 2026-11-14)

### Steam
- **Verified** (S0):
  - QR sign-in worked on the store sign-in page; thanks to the persistent WebView2 profile reserved for Steam (`app_data_dir/webview-steam`, not incognito) the second run was without sign-in (1 s).
  - The invisible window (`visible(false)`) also gave a result without sign-in.
  - The page script called `IPlayerService/GetOwnedGames/v1` from the same page with the temporary access token in `#application_config[data-loyalty_webapi_token]`: 243 games, all named, 68 played. This list is the game subset of the 724 ids in userdata; the remaining 481 are DLC/tools etc.
  - The session cookies (`steamLoginSecure`, `steamRefresh_steam`) are valid for about a year and are httpOnly; Hangar never touches them.
- **Connect:** "Bağlan" in Hesaplar opens the Steam window visibly with the persistent profile. The user signs in on Steam's page (QR/password/Steam Guard). The script delivers `{steamid, games}`; the access token never leaves.
- **Storage:** the DPAPI file `accounts\steam.dat` holds only `{v:1, steamid, display_name}`. The session itself stays in the WebView2 profile (Chromium cookies are protected with DPAPI on Windows).
- **Scan:**
  - If Steam is connected and there is no API key, the invisible window opens with the persistent profile and delivers the list within at most 20 s.
  - The list is converted to the existing `steam::parse_owned_games` format (`response.games[]`) and turned into records through the same path.
  - If the session has dropped, the account becomes "süresi doldu" (expired), the cache is shown and the warning appears with the Epic/Ubisoft pattern.
  - Only one Steam window is opened at a time.
- **API key:** under Hesaplar › Steam › "Gelişmiş" (key + SteamID). If a key is entered, today's path is used and the sign-in window is not opened.
- **Disconnect:** while the window is closed, the `webview-steam` folder and `steam.dat` are deleted and the Steam cache is cleared. Ending the session on the server is not attempted; when the local profile is deleted the session ends in Hangar, a note is added to the README.
- **Navigation rules:** store/login/help/checkout.steampowered.com, steamcommunity.com, captcha hosts (the list from S0).

### GOG
- **Verified** (G0):
  - Sign-in and token exchange with Galaxy's public OAuth client worked (`expires_in` 3600).
  - `getFilteredProducts?mediaType=1` gave 82 games with titles, paged.
  - `user/data/games` returned 163 ids (including DLC).
  - Refresh returned HTTP 200; the refresh token does not change.
  - Hosts visited: auth/login/embed.gog.com.
- **Source choice:** a "Kaynak: Galaxy veritabanı / GOG hesabı" (Source: Galaxy database / GOG account) choice in the Hesaplar › GOG row; `settings.gog.source = "galaxy"|"account"`, default `galaxy`.
  - `galaxy`: today's behavior as is.
  - `account`: the owned list comes from the account. The Galaxy DB continues to be read only for installed games.
  - Records from the account carry the same `platform_id` (product id) as the owned records in the Galaxy DB, so they merge by themselves.
  - Record format: `goggalaxy://openGameView/<id>`. The cover comes from the `image` field of the `getFilteredProducts` product: example `//images-3.gog-statics.com/<hash>` → `https:` + value + `_392.jpg` (the builder verifies the best vertical size). The cover allowlist must also accept `images-<n>.gog-statics.com` subdomains; today only `images.gog-statics.com` exists.
- **Client:** `GogClient` (Epic pattern): `connect(code)`, `owned_for_scan`, `disconnect`, `status`. DPAPI `accounts\gog.dat` = `{v:1, refresh_token, user_id, display_name}`. The access token is kept only in memory.

## Points clarified after the builder check (Opus decisions)
1. **"Played":** a card counts as played if any of its records has `playtime_min > 0` or `last_played`, or if the card's `last_launched` value is filled. Steam (both with key and with sign-in) provides the time. For Epic, GOG and Ubisoft only launches from Hangar count. Live test 3 changed to "Never-played Steam" for this reason. A short note in the filter editor: "Oynama süresi yalnız Steam'de var; diğerlerinde Hangar'dan başlatılanlar sayılır." (Playtime exists only on Steam; for others, launches from Hangar count.)
2. **Steam scan:**
   - The invisible window is part of the scan and takes at most 20 s. The screen keeps showing the list in the database during this time.
   - If the script gives an explicit "no sign-in" signal, the account becomes "süresi doldu".
   - On a timeout or network error only an "update failed" warning and the cache arrive; it does not count as expired.
   - In focus-triggered automatic scans the Steam window opens at most once every 10 minutes. The "Yenile" (Refresh) button skips this limit.
   - If an API key is entered, the key path is always used.
3. **SteamID:**
   - At sign-in it is taken from the page (`application_config` / `g_steamID`) and sits in `steam.dat`.
   - The manual SteamID under "Gelişmiş" is used only on the key path; the `loginusers.vdf` fallback also stays only there.
4. **GOG account source:**
   - The source of truth is `getFilteredProducts?mediaType=1` (paged). `user/data/games` is not used.
   - In `account` mode the uninstalled records of the Galaxy DB are discarded; installed records stay.
   - If the account drops, the Epic pattern applies: cache + warning. It does not fall back to the Galaxy DB.
   - That the ids are equal is verified in the plan by a live data comparison (g0 output ↔ Galaxy DB).
5. **GOG sign-in:** The existing `login_flow` return with nonce is enough; G0 worked with it. Sign-in is done only with a GOG account, captcha hosts are allowed; social sign-ins are a non-goal.
6. **Filter model:**
   - **Edition:** three-state (all / original only / special editions only).
   - **Collection:** a specific collection id, in or outside.
   - **Application:** the filter is applied inside the tab scope. Tab counters reflect the active filter (today's Installed behavior); tab visibility is determined by the unfiltered count (Phase 3 decision).
   - **Missing reference:** a criterion referring to a deleted collection is ignored.
   - **Persistence:** the active filter is stored in `ui.filter` (`"installed"` or the filter id, empty = none).
   - **"Kurulu":** Virtual and built-in, not kept in the table.
   - **Migration:** if `ui.installed = "1"`, then on first launch `ui.filter = "installed"`.
7. **Drag-and-drop:**
   - The drop target is only the collection tabs in the strip. No auto-scrolling.
   - `dragDropEnabled: false` goes into `tauri.conf.json`. Hangar does not use file dragging (files are added through the picker window); the builder verifies this.
   - If the card is already in the collection, a short message is shown: "zaten <name> içinde" (already in <name>).
8. **Settings and first experience:**
   - "Oyun ekle" and "Yenile" stay in the sidebar.
   - The Steam nudge changes: if Steam is not connected and there is no key either, it says "Steam'e bağlan" (Connect to Steam) and opens Settings › Hesaplar.

## Tests
- **Core:**
  - Steam/GOG clients (fake API): connecting, refreshing, expired, cache, leak.
  - Filter table CRUD.
  - The `ui.*` whitelist.
- **Front end:**
  - Filter evaluation (each criterion, combination, Installed migration).
  - Collapsing/expanding the sidebar and persistence.
  - Drag-and-drop (a drop adds to the collection, selection is not broken).
  - Settings tabs.
- **Live:**
  1. Connect to Steam with sign-in → uninstalled Steam games arrive without a key.
  2. GOG "Hesapla" → the list arrives, merges with installed games.
  3. The "Never-played Steam" filter.
  4. Collapse the menu, restart → it opens collapsed.
  5. Drag a card to the "AC" tab.
  6. Two tabs in Settings.
