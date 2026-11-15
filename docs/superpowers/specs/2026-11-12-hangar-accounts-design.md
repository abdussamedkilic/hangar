# Hangar — Local account connection for Epic · Design

Date: 2026-11-12 · Status: passed the council, awaiting user approval · Previous: `2026-11-11-hangar-step1-design.md`

## Intent and rationale

Today Hangar shows only **installed** games for Epic and Ubisoft (Epic `.item` manifests, Ubisoft registry). The user wants to see **all** games on these two accounts, including uninstalled ones, in a single library, like Steam and GOG.

The user's decision (2026-11-12): **GOG Galaxy will not be used.** The path will be like Playnite's:
- The connection is set up inside the app.
- No data leaves the user's computer; Hangar has no server or telemetry of its own.
- Sign-in happens on the platform's **own page**; Hangar never sees the password.

This document covers **only Epic**. Ubisoft is left to a separate discovery task (U0) and then a separate plan (see "Next work: Ubisoft"). This is the council's joint decision.

Success criteria:
- From Settings › Hesaplar (Accounts), click "Epic'e bağlan" (Connect to Epic), sign in in Epic's sign-in window, the window closes by itself.
- On the next scan, all games on the Epic account arrive. Installed ones merge with the existing cards, the others become "Kur" (Install) cards.
- Closing and reopening the app does not ask for sign-in again.
- "Bağlantıyı kes" (Disconnect) both deletes the local file and ends the session on Epic.

## Goals

1. **Epic connection:** in-app sign-in window → authorization code → OAuth token.
   - Owned games are fetched from Epic's library and catalog services.
   - Games given away for free are included. DLC, add-ons and Unreal Engine assets are not included.
2. **Local and encrypted token:** the refresh token is kept only in `%APPDATA%\com.hangar.app\accounts\epic.dat`, encrypted with Windows DPAPI (CurrentUser).
3. **Settings › Hesaplar section:** status for Epic ("Bağlı değil" / "Bağlı · <display name>" / "Süresi doldu") (Not connected / Connected · <display name> / Expired), "Bağlan" (Connect) and "Bağlantıyı kes" buttons.
4. **Installing the uninstalled:** the "Kur" button opens the install in the Epic launcher, as on Steam and GOG.
5. **Offline resilience:** if there is no network or the token is invalid, the last known owned list is used. The top warning row says what to do.
6. **README security note:** where the token is kept, how to delete it, the risks (see §7).

## Non-goals

- GOG Galaxy integration. The user rejected it; it will not be suggested again.
- **Ubisoft** (this plan will have neither a UI row nor README text for it; only the shared infrastructure takes a platform parameter).
- EA app, Amazon Games, Battle.net and other platforms. A general "owned provider" layer is also a non-goal.
- Using catalog images as covers. Noted as possible later work, not in this plan.
- Playtime, achievements, friends, cloud saves.
- Hangar downloading or installing the game itself.
- Multiple Epic accounts.
- A mode that does not store the token ("sign in every time"). Proposed in the council, the user had chosen the persistent token; a "disconnect if you don't want it" note in the README is enough.

## Anti-goals (count as failure even if it works)

- The token, the access token or the authorization code being written to **anywhere**:
  - log, error message, warning row, SQLite, the `settings` table, panic output, test fixture.
  - Error messages contain at most the HTTP status code and a short description; they do not contain the response body.
- The sign-in page (a remote web page) being able to call Hangar's Tauri commands.
- The sign-in window leaving cookies or a session on disk. There must be no second persistent session apart from Hangar's encrypted file.
- The sign-in window being able to navigate to arbitrary sites outside Epic's sign-in flow.
- The games in the user's library silently disappearing when the connection drops.
- A custom sign-in form that asks for a password.
- The app looking "frozen" after a successful sign-in.

## Constraints

- Windows 11, Tauri **2.12.5** (wry 0.57.1; verified in `Cargo.lock`), Rust, React 18 + TS.
  - In this version `WebviewWindowBuilder` offers `incognito()`, `initialization_script()`, `on_navigation()` and `on_new_window()`; no version upgrade is needed.
  - On Windows, `incognito()` corresponds to WebView2 InPrivate.
- `hangar-core` is pure and Tauri-free; tested on Linux/WSL.
  - Network calls and DPAPI must be behind a trait.
  - Parsing code is tested with fixtures.
- For HTTP the existing `ureq = "2"` is used. Since the calls are blocking, they run inside `spawn_blocking`.
- For DPAPI, `windows-sys` (under `cfg(windows)`) is used; other new dependencies are kept to a minimum.
- `scan_all` today only fetches Steam's owned list (`owned: Option<Result<Vec<PlatformEntry>, ProviderError>>`). This signature is widened to a per-platform map.
- Editing Rust while `tauri dev` is open restarts the app; be careful in manual tests.
- The user's rules:
  - No unrequested features, surgical changes.
  - Subagents are Sonnet, at most 2-3 at a time.
  - Commit messages are Turkish conventional.

## Approaches

The main question was how the sign-in would be captured.

- **A — Hangar window, automatic capture (chosen).** Hangar opens Epic's sign-in page in its own separate window. When sign-in is done, it takes the code itself and closes the window. One click from the user's point of view. Cost: if Epic changes the page or blocks embedded browsers, the capture can break.
- **B — System browser + pasting the code by hand (the Legendary way).** Simple and less fragile, but there is manual work on every connect. About 30 lines of work; this is the fallback path to be offered to the user if E0 fails.
- **C — Reading from GOG Galaxy.** The user rejected it.

The user chose **A** (2026-11-12).

## What the user decided

- No Galaxy; in-app connection; data only on this PC.
- Sign-in on Epic's own page, in Hangar's separate window (approach A).
- Token local, encrypted with DPAPI; "Bağlantıyı kes" deletes it; never logged; no server or telemetry.
- "Hesaplar" section in Settings; security note in the README.

## Design

### 0. Task E0 — discovery (this first; at most one session)

First only the sign-in window and capture are built, behind a hidden development command (e.g. under `debug_assertions`). The user signs in once with a real Epic account.

**All of these are required for it to count as successful:**
1. Sign-in, including two-step verification, completes in the embedded window.
2. The initialization script reads `authorizationCode`. `on_navigation` can see and cancel the `location.replace` to `https://hangar.invalid/callback`.
3. The code can be exchanged for a token at the token endpoint. The library and catalog calls respond.
4. After the window closes, no Epic cookie or session remains in the WebView2 data folder (InPrivate verification).
5. The token, library and catalog responses are turned into fixtures by a **scrubbing script**. The fixtures contain no account ID, display name, email or tokens; the script replaces them with fixed fake values and checks that no real value remains in the output.

**Counts as failed:** the captcha or bot check cannot be passed in the embedded window, Epic rejects the sign-in, or capture no. 2 does not work. In that case the work stops. B (pasting the code by hand) is suggested to the user; it does not switch to B by itself.

Before E0, the endpoint addresses and the client ID were compared with Legendary's current source (2026-11-12). The addresses and parameters below are the same.

### 1. Sign-in window and capturing the code

`src-tauri` exposes a command: `connect_account(platform)`. This command does the following.

1. A **separate** window labeled `account-login` is opened with `WebviewWindowBuilder`. The address is Epic's sign-in page. The window is opened with `incognito(true)`.
2. **The window title shows the real address** (e.g. "Epic Games girişi — www.epicgames.com") (Epic Games sign-in — www.epicgames.com). This way the user sees where they are and the window does not look like phishing.
3. The **initialization script** does the following:
   - Runs only in the top frame (`window.top === window`).
   - Looks for the condition `location.origin === 'https://www.epicgames.com'` and `location.pathname === '/id/api/redirect'`.
   - After `DOMContentLoaded`, parses `document.body.innerText` as JSON.
   - If `authorizationCode` is filled, does `location.replace('https://hangar.invalid/callback?code=' + encodeURIComponent(code))`. If empty, does nothing.
4. The **`on_navigation`** handler behaves as follows:
   - When `hangar.invalid` arrives, it cancels the navigation (`false`), parses the code and passes it to the command through a `oneshot` channel, then closes the window.
   - **Allowlist:** for other navigations only `https` and allowed domains are permitted: `epicgames.com` and subdomains, plus the captcha and identity-provider domains observed in E0 (e.g. hCaptcha/Arkose, Google, Apple, Xbox/Microsoft, Steam, PlayStation, Nintendo, Facebook). The list is a single constant array in the code.
5. **`on_new_window`:** popups are opened in the same window (subject to the allowlist) or blocked. E0 decides which: if signing in with identity providers like Google or Apple wants a popup, it opens in the same window.
6. **The window's `Destroyed` event:** if the channel is still empty, the command returns with "cancelled". No warning is shown to the user.
7. **Timeout:** if no code arrives within 5 minutes, the window closes and the command returns with "Giriş zaman aşımına uğradı" (Sign-in timed out).

**No capability is defined** for the remote page. `capabilities/default.json` continues to apply only to the `main` window.

The authorization code is single-use and short-lived. It exists in memory only during this redirect. Since the navigation is cancelled, it does not enter the window history; the window is InPrivate anyway.

Only one sign-in window can be open at a time. If it is already open, it is brought to the front.

### 2. Epic services

**Sign-in address:**
`https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode`

**Getting the token** (the public client of the Epic Games Launcher; Legendary, Heroic and Playnite use it too):
- `POST https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token`
- Basic auth: `34a02cf8f4414e29b15921876da36f9a:daafbccc737745039dffe53d94fc76cf` (hard-coded; it is already public)
- Form: `grant_type=authorization_code&code=<code>&token_type=eg1`
- Fields used: `access_token`, `expires_at`, `refresh_token`, `refresh_expires_at`, `account_id`, `displayName`.
- Refresh: the same endpoint, `grant_type=refresh_token&refresh_token=<…>&token_type=eg1`.

**Owned games:**
- `GET https://library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true[&cursor=<…>]`, `Authorization: bearer <access_token>`.
  - Paged until `responseMetadata.nextCursor` runs out.
  - `namespace`, `catalogItemId` and `appName` are taken from each record.
  - Records with `namespace == "ue"` (Unreal Engine assets) are skipped.
- Catalog: `GET https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/<ns>/bulk/items?id=<id>[,<id>…]&includeDLCDetails=true&includeMainGameDetails=true&country=US&locale=en-US`. The response is keyed by `catalogItemId`.
  - IDs in the same namespace are requested in a single call.
  - **Rate limit:** at least 100 ms is waited between calls.
- **Filter** (exact rules are pinned with E0 fixtures):
  - If `mainGameItem` exists (DLC), the record is skipped.
  - If `categories` contains `addons` or `digitalextras`, the record is skipped.
  - If `categories` contains `applications` but not `games`, the record is skipped.
- **Catalog cache:** the title and "is it a game" information per `catalogItemId` are kept in SQLite. In later scans the catalog is called only for new IDs.

**Record format** (same identity as the installed manifest, so they merge):
- `platform: Epic`, `platform_id: appName`, `title: catalog title`, `installed: false`
- `launch_uri`: `com.epicgames.launcher://apps/<ns>%3A<catalogItemId>%3A<appName>?action=launch&silent=true`
- `install_uri`: `com.epicgames.launcher://apps/<ns>%3A<catalogItemId>%3A<appName>?action=install`

**Merging with the installed record:**
- The installed record wins.
- The title stays as the manifest's `DisplayName`; this way the normalized name and the merge/split records are not broken.
- The installed record's `install_uri` is updated with the owned record's `action=install` link.

**"Kur" and the launcher:**
- The link is opened with `opener`.
- If the Epic launcher is closed, Windows starts it.
- If the link cannot be opened (launcher not installed), the existing launch error path shows the message "Epic Games Launcher açılamadı — kurulu mu?" (Could not open Epic Games Launcher — is it installed?).

**Disconnecting:**
1. First a valid access token is obtained (renewed if necessary).
2. `DELETE https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/<access_token>` is called. If it fails this is ignored, but the result is stated in the UI ("Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin") (Could not end the session on Epic; you can end it from your Epic account settings).
3. Then `epic.dat`, the owned list cache and the "expired" state are deleted.
4. A rescan is done; installed games stay in place.

### 3. Token store

- `hangar-core` defines a trait: `SecretStore` (`load`, `save` and `delete`, per platform). An in-memory fake is used in tests.
- **Windows implementation:**
  - `CryptProtectData` / `CryptUnprotectData` are used, with CurrentUser scope and a fixed extra entropy `b"hangar-accounts-v1"`.
  - The encrypted byte array is written to `%APPDATA%\com.hangar.app\accounts\<platform>.dat`.
  - Writing is done with a temp file + rename.
- The content before encryption is a small JSON: `refresh_token`, `refresh_expires_at`, `account_id`, `display_name`.
- The short-lived `access_token` is kept only in memory.
- Token strings are zeroed in memory after use (`zeroize` or by hand). It is enough to do this if cheap; it is not full protection against malware under the same user.
- **Refresh order:** refreshing is serialized behind a single `Mutex`. As soon as a new `refresh_token` arrives, this token is written to disk with `save` **before being used for another request**; this way a rotated token is not lost in a crash.
- If the file cannot be decrypted (another Windows user or a corrupt file), it is deleted and the platform counts as "Bağlı değil". The warning row says "Epic bağlantısı okunamadı; yeniden bağlan" (Epic connection could not be read; reconnect).

### 4. Merging with the scan

- During `scan`, if Epic is connected, the network work is done without holding the DB lock (like today's Steam pattern).
- The `owned` parameter of `scan_all` becomes platform-independent. It takes the owned lists of multiple platforms and merges each with its own platform's installed records by `platform_id`. Steam's existing behavior and tests are preserved exactly.
- **Owned list cache:** after each successful fetch, Epic's owned list is stored in SQLite. The cache contains only game records, no tokens. If a fetch fails, the cached list is used and a warning is added:
  - Network error: "Epic oyunları güncellenemedi; son bilinen liste gösteriliyor." (Epic games could not be updated; showing the last known list.)
  - Invalid token (`invalid_grant` etc.): "Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan." (The Epic connection has expired: reconnect from Settings › Hesaplar.) The platform is marked "Süresi doldu". This is kept in the `settings` table only as a state value, not a token. It is reset on reconnect.
- `get_accounts` does not go to the network; it looks at the file's existence and the stored state value.
- **First scan:** may be slow because of 100+ catalog calls. In the scan right after connecting, "Epic oyunları yükleniyor…" (Loading Epic games…) appears in the top warning row. One event at scan start and one at the end is enough for this; no detailed progress bar is built.

### 5. UI — Settings › Hesaplar

A section is added to the existing Settings window, below the Steam fields:

```
Hesaplar
  Epic Games     Bağlı · <görünen ad>          [Bağlantıyı kes]
  Giriş Epic'in kendi sayfasında yapılır; Hangar şifreni görmez.
  Bağlantı belgesi yalnızca bu bilgisayarda, şifreli tutulur.
```

- **Connect:**
  - `connect_account` is called.
  - The button goes to a waiting state "Epic penceresi açık…" (Epic window is open…).
  - On return the state is updated and App's `rescan` is called, so warnings appear in the top row (same path as the existing fix in Settings).
  - Timeout and other errors appear as a short text under the button.
- **Disconnect:**
  - A confirmation is asked: "Epic bağlantısı silinecek ve Epic'teki oturum kapatılacak; kurulu oyunlar kalır." (The Epic connection will be deleted and the session on Epic will be ended; installed games stay.)
  - After confirmation `disconnect_account` and `rescan` are called.
- **Expired:** the button becomes "Yeniden bağlan" (Reconnect).
- **Commands:** `get_accounts() -> [{platform, state, display_name?}]`, `connect_account(platform)`, `disconnect_account(platform)`.
- The visual language follows the existing Settings window.

### 6. Error and status texts

All texts are short and in Turkish. None contains a token, code or response body. The exact texts are left to the builder; the ones above show the direction.

### 7. Security and README

A "Account connections" section is added to the README. Its content:

- **What is stored and where:** only the refresh token and display name, under `%APPDATA%\com.hangar.app\accounts\`. Protected with Windows DPAPI; only this Windows user can decrypt it.
- **How to delete:** "Bağlantıyı kes" both deletes the file and ends the session on Epic. The folder can also be deleted by hand.
- **How it appears on Epic:** Hangar uses the public client ID of the Epic Games Launcher. So in the session and app lists in Epic account settings the connection appears as **"Epic Games Launcher"**, not as "Hangar".
- **Risks, stated openly:**
  - These are unofficial endpoints. Epic may block this client ID or change the response format; then Epic's uninstalled games do not arrive, installed ones keep working.
  - Using another client's ID may be considered a violation of Epic's terms of use. In the worst case the account may be flagged. Legendary, Heroic and Playnite have used the same path for years and there is no known widespread enforcement, but this is not a guarantee.
  - The token gives access to the whole account. DPAPI does not protect against malware running as the same Windows user.
- **No server:** Hangar has no server; data goes only to Epic's services.
- **For those who do not want a persistent token:** "Bağlantıyı kes" when done is enough. The owned list does not stay in the cache; it is deleted on disconnect.

## Tests and verification

**`hangar-core` (on Linux):**
- Token response parsing; library paging; catalog filtering (DLC, add-on, `ue`, applications); record generation.
- Merging with the installed record: the title comes from the manifest, `install_uri` is updated.
- The `scan_all` generalization: Steam tests pass unchanged.
- Fallback to the cache and warning texts.
- When `invalid_grant` arrives, "expired" is marked.
- The new `refresh_token` is `save`d before being used (order test with a fake store).
- A corrupt or undecryptable store gives the "Bağlı değil" result.
- **Token leak test:** fake token values appear in no error message, warning or DB row.
- The network layer is behind a trait and is fed with E0 fixtures.

**`src-tauri`:**
- `cargo check`.
- Under `cfg(windows)` a DPAPI round-trip test and a corrupt-blob test.
- Capture address parsing and allowlist matching are unit-tested as pure functions.

**Front end (vitest, with the existing `SettingsDialog.test.tsx` pattern):** the states, buttons, confirmation question and error texts of the Hesaplar section.

**By hand (Windows, `npm run tauri dev`):**
1. Connect to Epic; the window title shows the address; the window closes after sign-in.
2. "Epic oyunları yükleniyor…" appears, then the uninstalled Epic games arrive.
3. Death's Door is still a single card.
4. Reopen the app; no sign-in is asked.
5. "Kur" on a game opens the install in the Epic launcher.
6. Scan with Wi-Fi off: the last known list and the warning.
7. "Bağlantıyı kes": only installed games remain, `accounts\epic.dat` is deleted, the session is ended on the Epic account page.
8. No Epic cookie in the WebView2 data folder.

## Next work: Ubisoft (outside this plan)

- Ubisoft's local cache is encrypted on this PC (`cache\configuration\configurations.vdat`, `cache\ownership\<id>`), so only the web path remains.
- **U0 (time-boxed, with the user's real sign-in)** will show:
  1. The connect.ubisoft.com sign-in and where the ticket is read from (`ticket`, `sessionId`, `userId`, `rememberMeTicket`).
  2. Renewal with `rm_v1` (`POST https://public-ubiservices.ubi.com/v3/profiles/sessions`).
  3. Owned games and the product ID that matches the registry.
  4. The `uplay://` link that opens the install.
- If U0 succeeds, a separate short plan is written on top of the shared infrastructure in this document (sign-in window, `SecretStore`, the `scan_all` map, the Hesaplar list). If it fails, only installed games remain for Ubisoft.

## Left to the builder

- Module and file names (e.g. `providers/epic_account.rs`), trait shapes, the names and schema of the new SQLite tables.
- How the `oneshot` channel and the window state are held.
- The exact domain names in the allowlist (per E0 observation).
- The name and format of the loading event.
- The exact wording of the error texts.
