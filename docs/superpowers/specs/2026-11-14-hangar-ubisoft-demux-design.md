# Hangar — Ubisoft full ownership list (demux) · Short design

Date: 2026-11-14 · Status: approved by the user ("Yes, go ahead")
Basis: `2026-11-14-hangar-ubisoft-account-design.md` (connection, DPAPI and security rules apply as is).

## Problem
GraphQL (`games(filterBy:{isOwned:true})`) returns only the 4 games activated/played on the account. AC Origins and AC Syndicate, obtained through a giveaway, do not come.

## Verified in the live trial (2026-11-14, the user's account)
- Demux (`dmx.upc.ubisoft.com:443`, TLS 1.2, 4-byte BE length + protobuf; YoobieRE/ubisoft-demux-node schemas) returned 313 records through `ownership_service` → `InitializeRsp`. The 7 records with `owned=true` and `product_type=0` are exactly the expected list: 420 Far Cry 4, 541 Watch_Dogs, 857 AC Unity, 1875 AC Syndicate, 3539 AC Origins, 5092 AC Odyssey, 5184 AC III Remastered.
- Demux accepts only a ticket obtained with the **Ubisoft Connect PC application id** (`f68a4bb5-608a-4ff2-8123-be8ef797e0a6`); a ticket obtained with the web id (`82b650c0-…`) is "rejected (expired:false, banned:false)".
- The sign-in page does not accept the PC id (ERROR). But an `rm_v1` refresh (`rememberMe:true`) with the PC id, using the `rememberMeTicket` from the web sign-in, works and gives a new PC token; this token can then be refreshed repeatedly on its own. GraphQL also returns the same 4 games with the PC id.
- This conversion invalidates the web token (the next web refresh gives 401). This was why the saved connection broke in the earlier trial.
- The server often cuts the TLS handshake immediately (UnexpectedEof); it connects within a few attempts. The installed `upc.exe` build number 13416 was accepted as `clientVersion`.
- Demux `product_id` = the installed Ubisoft id in the registry (541 Watch_Dogs) and `ubiservices_space_id` = GraphQL `spaceId`.

## Design
**Ids:** The sign-in window stays with the web id (`UBI_APP_ID`). All API calls (refresh, GraphQL, ending the session) are made with the new constant `UBI_LAUNCHER_APP_ID`.

**Connecting:** `connect` refreshes the web `rememberMeTicket` from sign-in once with the PC id. The returned PC token is saved; the web token is not stored. The token format becomes `v: 2`. A `v: 1` file counts as unreadable and the user does not appear connected; they need to reconnect. The existing token is already invalid on this PC.

**Scan (`fetch_owned`):**
1. Refresh (PC id, same persistent rotation rules).
2. Demux: owned products, only `owned == true && product_type == 0`.
3. GraphQL (PC id, best effort): a `spaceId → (name, coverUrl)` map.
4. Record: `platform_id = product_id`, `launch_uri = install_uri = uplay://launch/<product_id>/0`.
   - Title: the GraphQL name (by space_id), otherwise the name in the configuration YAML. If the name is a localization key (like `GAMENAME`, `l1`), its equivalent under `localizations: default:` is looked up. If the name cannot be resolved, the record is skipped.
   - Cover: GraphQL `coverUrl`, otherwise from the YAML `thumb_image` file name `https://ubistatic3-a.akamaihd.net/orbit/uplay_launcher_3_0/assets/<file>`. This host is added to the cover allowlist. If it is not in the YAML, the existing cover chain kicks in.
5. If demux fails (network, handshake, parsing), it falls back to today's GraphQL list (GUID id, existing title matching). No warning is shown in this case.

**Demux client:** becomes production code (`cfg(debug_assertions)` is removed). It only reads: clientVersion, authenticate, open `ownership_service`, initialize; then it closes. The handshake is tried at most 6 times; the duration of a single request is limited. The ticket is never logged or written. Errors do not contain server data.

**Matching with the installed record:** ids now match, so `scan_all` merges by id. The title fallback, which exists only for Ubisoft, is harmless; it stays for the fallback path.

**UI:** no change. Expected cards:
- Syndicate and Origins become new Ubisoft cards.
- AC III Remastered joins the Steam card with a Ubisoft badge; like Watch_Dogs.
- Other cards stay the same.

**Cleanup:** the trial commands (the step chain of `debug_ubisoft_demux`, `ubi_probe::launcher_session`/`graphql_app`) are removed. In their place `hangarUbiDemux()` becomes a thin debug command that gives a summary with the saved connection.

**README:** the following is added to the Ubisoft section:
- Ubisoft Connect's internal protocol (demux) is used.
- The token is stored with the ID of the Ubisoft Connect PC application; it may appear that way on the Ubisoft account page.
- This path is unofficial and may change.

## Non-goals
- DLC/add-on cards.
- The Ubisoft+ subscription.
- `claimed_games` (empty on this account).
- The old unencrypted cache.

## Tests
**Core** (fake API):
- connect stores the PC token (v2), v1 is not read
- demux list + GraphQL map → title/cover/id
- localization key resolution
- type/ownership filter
- fallback to GraphQL on demux failure
- rotation and leak tests as is

**Live:**
1. Reconnect.
2. In the dry run 7 Ubisoft records; Syndicate and Origins are cards, AC III Remastered is on the Steam card.
3. Origins "Kur" opens Ubisoft Connect.
