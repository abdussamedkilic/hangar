# Hangar — Ubisoft Connect account connection · Short design

Date: 2026-11-14 · Status: awaiting approval
Basis:
- Epic design (`2026-11-12-hangar-accounts-design.md`) — same pattern, same security rules.
- U0 discovery (2026-11-14, with the user's real account).

## Intent
All PC games on the Ubisoft account (including uninstalled ones) should come into Hangar. The connection is set up like Epic's:
- From Settings › Hesaplar, with Ubisoft's own sign-in page.
- The password never reaches Hangar.
- The token stays only on this PC, encrypted with DPAPI.

## Verified in U0
- **Sign-in:** `connect.ubisoft.com/login?appId=82b650c0-…&genomeId=42d07c95-…` works in the embedded window. Only connect.ubisoft.com was visited.
- **Session values:** after sign-in, `PRODloginData` (ticket, sessionId, userId) and `PRODrememberMe` (rememberMeTicket) are found in `localStorage`. The initialization script takes them with a return carrying a nonce.
- **Refresh:** the call `POST https://public-ubiservices.ubi.com/v3/profiles/sessions` with `Authorization: rm_v1 t=<rememberMeTicket>` and body `{"rememberMe":true}` returns HTTP 200.
- **Owned games:** `POST …/v1/profiles/me/uplay/graphql` (AllGames query) returned 4 PC games.
  - Each game has `id` and `spaceId` (GUID), `name` and `coverUrl` fields.
  - The launcher's numeric id does not come. The local cache (`configurations.vdat`) is encrypted. Schema introspection is closed.
- **Install link:** `uplay://launch/<spaceId>/0` opens Ubisoft Connect. Since the launcher stayed on the "Initializing" screen, it could not be verified whether it went to the game's page.

## Design (same structure as Epic)
**Token**
- A `Platform::Ubisoft` file in `SecretStore`: `accounts\ubisoft.dat`.
- Content `{v, remember_me_ticket, user_id, display_name}`.
- The short-lived ticket and sessionId are kept only in memory.
- The rotated token is saved before use; if it cannot be saved, it is kept in memory. Same as Epic's `pending` rule.

**Scan**
- On every scan an rm_v1 refresh is done, then the GraphQL AllGames is called.
- Only games with PC ownership are taken.
- One record is produced from each game:
  - `platform: Ubisoft`, `platform_id: spaceId`, `title: name`, `installed: false`
  - `launch_uri = install_uri = uplay://launch/<spaceId>/0`
  - `cover_url = coverUrl`; `ubiservices.cdn.ubi.com` is added to the cover allowlist.
- Cache and "expired" behavior is the same as Epic's.

**Matching with the installed record**
- The registry gives a numeric id, GraphQL a GUID; so the ids do not match.
- Rule: on the same platform, if the id does not match but `normalize(title)` is the same as an installed record, the owned record joins the installed record. The installed record wins; the owned one's cover is taken.
- This way the same game does not appear as "Ubisoft · 2. kopya" (2nd copy).

**Sign-in window**
- `UBI_NAV_RULES` and `ubi_init_script` from U0 are used as is.
- The application id lives in a single constant (Ubisoft can change this id).

**UI**
- A "Ubisoft Connect" row is added to Settings › Hesaplar: Bağlan / Bağlantıyı kes / Yeniden bağlan (Connect / Disconnect / Reconnect).
- Texts follow the same pattern as the Epic row.

**Disconnect**
- First ending the session on the server is tried: `DELETE https://public-ubiservices.ubi.com/v3/profiles/sessions`. If it fails it is ignored.
- Then the token and cache are deleted.

**README:** Ubisoft is added to "Account connections". Notes: unofficial endpoints, the application id may change, the token can access the whole account.

## Non-goals
- The Ubisoft+ subscription library.
- LUNA (cloud) ownerships.
- Finding the launcher's numeric id (the cache is encrypted).

## Tests and verification
**Core** (with a fake API):
- connecting, token rotation, keeping an unsaveable token in memory
- cache when offline
- `invalid_grant` → expired
- a scan in progress during disconnect cannot write back
- PC filter
- matching with the installed record by title
- leak test

**Live:** connect → 4 games → Watch_Dogs is a single card (Steam + Ubisoft) → Far Cry 4 "Kur" opens Ubisoft Connect → disconnect.
