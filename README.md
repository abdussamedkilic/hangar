# Hangar

Hangar is a Windows desktop game launcher that brings your Steam, Epic Games and Ubisoft Connect libraries into a single window. It reads installed games from local files, merges copies of the same game from different stores, and launches them with one click. Built with Tauri 2 (Rust) and React.

## Requirements (Windows)

- Node.js (LTS)
- Rust (stable, MSVC toolchain)
- Visual Studio Build Tools ("Desktop development with C++" workload)

## Commands

```
npm install            # install dependencies
npm run tauri dev      # run in development mode
npm run tauri build    # produce the NSIS installer
```

Installer: `target/release/bundle/nsis/Hangar_0.1.1_x64-setup.exe`

Tests: `cargo test -p hangar-core` and `npm test`.

## Data location

The database and cover images are stored under `%APPDATA%\com.hangar.app\`. Linked-account credentials are in the `accounts\` folder.

## Steam API key (optional)

Without a key, only installed Steam games are listed. To see all games you own, get a key at https://steamcommunity.com/dev/apikey and enter it in the app's settings.

In the SteamID field, enter the 17-digit SteamID64 number, not the username; you can also leave it empty. If empty, Hangar reads the SteamID from Steam's `loginusers.vdf` file.

## Account connections (Steam)

If you connect your Steam account, Steam games you own but have not installed are listed without needing an API key. Connecting is optional.

- **Sign-in:** Hangar opens Steam's own sign-in page in a separate window (QR or password); Hangar never sees your password.
- **Where the session lives:** the session is kept in a persistent browser profile owned by Hangar (`%APPDATA%\com.hangar.app\webview-steam`); on later scans Hangar opens an invisible window with this profile and fetches the list. Hangar does not touch cookies; Steam's temporary web token never leaves the page, only the game list is retrieved.
- **What is stored:** the SteamID and display name, in `accounts\steam.dat`, encrypted with Windows DPAPI.
- **How to delete:** "Bağlantıyı kes" (Disconnect) deletes the profile folder, `steam.dat` and the Steam list cache; the session ends in Hangar (you are not signed out on Steam's servers).
- **API key is optional:** if a key is entered under Hesaplar › Steam › "Gelişmiş" (Accounts › Steam › Advanced), the key is used instead of sign-in.

## Account connections (GOG)

The default source is Galaxy's local database. If you set Hesaplar › GOG › "Kaynak" (Source) to "GOG hesabı" (GOG account), the owned list comes from your GOG account.

- **Sign-in:** GOG's own sign-in page opens in a separate window; Hangar never sees your password. Only GOG-account sign-in is supported.
- **What is stored:** the refresh token, GOG user ID and display name, in `accounts\gog.dat`, encrypted with Windows DPAPI. "Bağlantıyı kes" deletes the file and the list.
- **Unofficial path:** Hangar uses the public client ID of the GOG Galaxy client. GOG may change or block this; it may be considered a violation of GOG's terms of use.

## Account connections (Epic)

If you connect your Epic account, Epic games you own but have not installed are listed too. Connecting is optional.

- **Sign-in:** Hangar opens Epic's own sign-in page in a separate window; Hangar never sees your password. You can connect with your Epic email or with the Apple, Xbox/Microsoft, PlayStation, Steam, Nintendo, Facebook and Google sign-ins Epic offers. Google sometimes rejects sign-in in embedded windows; if so, sign in with your Epic email.
- **What is stored:** only the refresh token, its expiry, the Epic account ID and display name, in `%APPDATA%\com.hangar.app\accounts\epic.dat`. Encrypted with Windows DPAPI; only this Windows user can decrypt it. The token lives about one year and is renewed on every use.
- **If saving fails:** if the token cannot be written to disk while connecting, a "Bağlantı kaydedilemedi" (Connection could not be saved) error appears. If a later-renewed token cannot be written, Hangar keeps it in memory until restart; after a restart you may need to connect again.
- **How to delete:** "Bağlantıyı kes" deletes the file, signs out the session on Epic, and clears Hangar's Epic game list and title caches. The file can also be deleted by hand.
- **How it appears on Epic:** Hangar uses the public client ID of the Epic Games Launcher, so the connection appears as "Epic Games Launcher" on your Epic account page.
- **Giveaway/reward games:** Ubisoft's normal game list does not show games that were never launched or that came from giveaways. For these, Hangar uses Ubisoft Connect's internal protocol (demux), read-only.
- **Application ID:** the stored refresh token belongs to the Ubisoft Connect desktop application's ID; it may appear that way on the Ubisoft account page.
- **After updating:** if you connected before this version, you need to reconnect your Ubisoft account once.
- **Unofficial service:** Hangar uses Ubisoft's undocumented endpoints and protocol. Ubisoft may change or block them; if demux does not work, only the normal (GraphQL) list is shown, and if that fails too, only installed Ubisoft games appear.
- **Terms of use:** this path may be considered a violation of Epic's terms of use. In the worst case the account may be flagged. Legendary, Heroic and Playnite use the same path, but this is not a guarantee.
- **Scope of the token:** the token can access the whole account. DPAPI does not protect against malware running as the same user.
- **No server:** Hangar has no server; data goes only to Epic.
- **If you do not want a persistent token:** disconnect when you are done.

## Account connections (Ubisoft)

If you connect your Ubisoft Connect account, Ubisoft games you own but have not installed are listed too. Connecting is optional.

- **Sign-in:** Hangar opens Ubisoft's own sign-in page in a separate window; Hangar never sees your password.
- **What is stored:** only the refresh token, its expiry, the Ubisoft account ID and display name, in `%APPDATA%\com.hangar.app\accounts\ubisoft.dat`. Encrypted with Windows DPAPI; only this Windows user can decrypt it.
- **How to delete:** "Bağlantıyı kes" deletes the file, signs out the session on Ubisoft, and clears Hangar's Ubisoft game list. The file can also be deleted by hand.
- **Unofficial service:** Hangar uses Ubisoft's undocumented endpoints and Ubisoft Connect's application ID. Ubisoft may change or block them; then only installed Ubisoft games appear.
- **Scope of the token:** the token can access the whole account. DPAPI does not protect against malware running as the same user.
- **No server:** Hangar has no server; data goes only to Ubisoft.

## Architecture

There are three layers: `crates/hangar-core` holds UI-independent business logic (providers, SQLite, merging, cover images); `src-tauri` exposes this core as Tauri commands; the React UI under `src` calls these commands.

```
crates/hangar-core/src/
  model.rs        shared types such as Platform, Game
  providers/      steam.rs, epic.rs, ubisoft.rs (LibraryProvider)
  db.rs           SQLite storage
  merge.rs        merging multi-store games
  library.rs      scanning and the library flow
  artwork.rs      cover images
  vdf.rs          Steam VDF parser
src-tauri/src/    commands.rs (Tauri commands), main.rs
src/              React UI (components, lib, state, styles)
```

## Adding a new provider

1. Implement the `LibraryProvider` trait in `crates/hangar-core/src/providers/<name>.rs` and declare the module in `providers/mod.rs`.
2. Add the new variant to the `Platform` enum in `model.rs`; update `as_str` and the order of `ALL`.
3. Register the new provider in the provider list (`providers`) in `src-tauri/src/commands.rs`.
4. Add fixture files and tests.
5. Add the badge label in the UI (`ABBR` in `src/components/GameCard.tsx`, `PLATFORMS` and the filter counters in `src/lib/library.ts`, `Sidebar.tsx`).
