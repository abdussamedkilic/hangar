# Hangar — Unified Game Library · Design

Date: 2026-11-09 · Status: awaiting approval
(The working name "Hangar" is my pick; it can change any time.)

## Intent and rationale

The user plays on Steam and Epic Games; Ubisoft Connect and EA app accounts may be linked later. What they want is to see all their games **in one place** and launch them from there. Playnite does this, but the user wants **their own project**: an app whose UI they design themselves and can develop as they like. They dislike GOG Galaxy's UI and do not want Galaxy integration. Playnite is an inspiration, not a fork. Reason: its open-source code is an old version (v10) written in C#/WPF, while v11 is being rewritten from scratch in a closed repo, and WPF cannot run the web animation libraries the user wants.

Success criterion: when the app opens, all Steam and Epic games are listed with cover art in a single fluid UI that does not look "AI-made". They can be launched with one click.

## Goals (version 1)

1. **Steam:** installed games (from local files) and owned-but-not-installed games (via the Steam Web API, with the user's own API key and SteamID).
2. **Epic:** installed games (from local manifest files).
3. **Ubisoft Connect:** installed games (from the registry). If none are installed, it silently returns empty.
4. **Merging:** if the same game exists on multiple platforms, it is shown as a single card. "Başlat" (Launch) opens from Steam by default; another platform can be chosen with the selector next to the card. The chosen platform is remembered per game.
5. **Launching, installing and updating are done by the platforms' own launchers.** For this, the platform's own link is opened: `steam://rungameid/<id>`, `steam://install/<id>`, and their link formats for Epic and Ubisoft. Hangar has no download engine of its own.
6. **Library screen:** cover grid, search, filters (platform, installed/not installed), sorting (name, last played). Game detail panel: cover, platforms, installed size, last played and playtime (where Steam has it).
7. **UI:** fluid, characterful, dark theme first. Animations with GSAP and Lenis, react-bits components used where fitting. The design direction will be decided together (Dribbble references, taste/unslop/ui-ux-pro-max skills).

## Non-goals (version 1)

- GOG. The provider architecture will allow adding it later with its own GOG sign-in.
- EA app. The local data format is unclear; it will be added later as a provider.
- Epic's owned-but-not-installed games. That requires Epic account sign-in (OAuth) and is left for a later version.
- Own download and update engine, mod management, achievements, friends list.
- Linux and macOS. Only Windows is targeted.

## Anti-goals (count as failure even if it works)

- The UI looking like a ready-made template or "AI-made": purple gradients, glows, emoji labels.
- Stuttering, laggy animations. In a library of 500+ games the grid must scroll at 60 fps.
- The app sending data to the internet without the user's permission. The API key is stored only locally.
- The app crashing or spewing errors when a platform's launcher is closed.

## Constraints

- Windows 11, Turkish locale. User name `user`.
- Project path: `C:\Users\user\workdir\hangar`.
- Environment ready: Node 26, npm 11, Rust 1.99 (MSVC), VS Build Tools 2022, git 2.56.
- Development flow: Claude writes the code directly into this folder via a bridge, but cannot run commands on Windows. The user runs `npm install` and `npm run tauri dev` in a Windows terminal and shares the output. For git commits, Claude needs delete permission in the folder; otherwise the user makes the commits.
- At most 2-3 Sonnet subagents are used for coding. The Karpathy rules apply: think first, simplicity, change only what is needed, set a verifiable goal.

## Approaches

| | Approach | Pro | Con |
|---|---|---|---|
| A | Playnite 10 fork (C#/WPF) | Integrations ready | UI limited to XAML themes, no GSAP/react-bits, large codebase about to be abandoned |
| **B** | **Tauri 2 (Rust backend + React UI)** | Fully free UI, small (~10 MB) and fast app, project fully the user's | Rust learning curve (the backend will stay small) |
| C | Electron (everything in TypeScript) | One language, easy | Heavy app (~150 MB, high RAM) |

Python GUI was eliminated: the desired animation libraries are not available there.
**Chosen: B.** The user chose it.

## Architecture

```
React UI (Vite + TS, GSAP, Lenis, react-bits)
      │ Tauri commands (invoke) + events
Rust core
  ├─ providers/        each platform is a LibraryProvider
  │    steam.rs  epic.rs  ubisoft.rs   (later: gog.rs, ea.rs)
  ├─ merge.rs          merging the same game into a single record
  ├─ db.rs             SQLite (rusqlite) — games, platform entries, preferences
  ├─ artwork.rs        cover images: find, cache
  └─ launch.rs         opening platform links
```

- **LibraryProvider** contract: `id()`, `scan() -> Vec<PlatformEntry>`, `launch(entry)`, `install(entry)`, `is_available()`. Adding a platform means adding a new file.
- **Data model:**
  - `games`: merged game, display name, normalized name, cover path, preferred platform.
  - `platform_entries`: platform, id within the platform, installed or not, install path, size, last played, playtime, `game_id`.
  - `settings`: Steam API key, SteamID and similar settings.
- **Merge rule:** matching by name normalization (lowercase, dropping marks like ™/®/: and suffixes like "Edition/Remastered"). The user can manually split a wrong merge and manually merge a missed one; these decisions are persistent.
- **Scan flow:** on startup the cached database is shown instantly, the scan runs in the background, changes flow to the UI as events. There is also a "Yenile" (Refresh) button.

## Platform data sources (to be verified on the machine before coding)

- **Steam:** install path from the registry (`HKCU\Software\Valve\Steam\SteamPath`). Libraries are in `steamapps\libraryfolders.vdf`, installed games in `appmanifest_*.acf` files. Covers come from the local `appcache\librarycache` folder, otherwise from the Steam CDN. Owned games are fetched with `IPlayerService/GetOwnedGames`.
- **Epic:** `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON). Launch link `com.epicgames.launcher://apps/<ns>%3A<itemId>%3A<appName>?action=launch&silent=true`.
- **Ubisoft:** `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>`. Launch link `uplay://launch/<id>/0`.

## What the user decided

Tauri; their own project (not a fork); no GOG; default platform Steam with a selector next to it; download/update delegated to the platform's own launcher; runs on Windows; animations GSAP + Lenis + react-bits; we will design the UI together.

## Claude's decisions (open to objection)

- Working name "Hangar"; project path `workdir\hangar`.
- No GOG in version 1 (the question went unanswered twice); EA and Epic's uninstalled games are also outside version 1.
- SQLite + rusqlite; on the UI side React + TypeScript + Vite + Tailwind.
- The Steam API key is requested from the user and stored only locally.

## Decisions clarified after review (Claude's decisions)

- **Verification loop:** before coding, the user runs a read-only "probe" PowerShell script. The script records the Epic manifest list, Steam `libraryfolders.vdf`, `loginusers.vdf` and a sample `.acf` file, and the Ubisoft registry keys; personal information (account names) is masked. These samples become test fixtures; providers are written against them with unit tests. After that, small milestones: Claude writes, the user builds and shares the output.
- **The Steam key is optional.** Without a key only installed games appear, and a plain empty-state screen asks for the key for uninstalled ones. The SteamID is found automatically from `loginusers.vdf` and can also be changed by hand. The key sits in SQLite as plain text (single-user, local app); no credential vault is used.
- **Internet:** no telemetry. Network access only for the Steam Web API and downloading images. Using the app counts as consent to these; there is no separate consent screen.
- **Epic and Ubisoft covers:** first a match is searched for by game name in the Steam store (keyless, public search) and the Steam image is used. If none is found, a placeholder cover generated from the game name and matching the design is shown. SteamGridDB and manual cover picking come in a later version.
- **Install button:** `steam://install` on Steam. On Epic and Ubisoft the button opens the respective launcher; the user does the install there. `install()` is optional in the provider contract.
- **Merging:** information such as size, last played and duration is shown platform by platform in the detail panel. The card shows the information of the most recently played platform. Manual split/merge decisions are kept in a `merge_overrides` table keyed by platform id pairs, so they remain valid after a rescan. These decisions are given with a simple control in the detail panel.
- **Last played:** on Steam it comes from the Web API or the `.acf` file. On other platforms Hangar only records the time it launched itself; the game process is not tracked, hence no playtime.
- **Packaging:** at the end of version 1 an NSIS installer is produced with `tauri build`; no signing and no auto-update. Database and image cache under `%APPDATA%\Hangar`.
- **UI order:** the data layer comes first, built with a functional skeleton bound to design tokens. Once the visual direction is chosen, tokens and components get dressed. The grid is virtualized and has its own scroll area; Lenis is attached to this area, GSAP is used for entrance and transition animations. No separate per-card animation during scrolling.

## Left to the builder

File and folder layout, state management library, test approach, cover cache format, details of name normalization.

## Open point — UI

The visual direction will be handled as a separate round before the planning stage: it will be chosen together with the user from Dribbble references and 2-3 mockup options.

## Visual direction (chosen: 2026-11-09)

The user chose the **"5 · Kokpit + Vitrin"** (Cockpit + Showcase) direction among 8 mockups. Reference: https://claude.ai/artifact/LKs8cVUuDnVjv11NVRFuo9 (first tab).

- **Layout:** fixed sidebar on the left (Library: All / Installed / Favorites; Platforms with counters; Collections). On the right, above the main area, a "stage": a blurred background glow derived from the selected game's cover, a large title, a small stats strip (playtime, last, size), a split Play button and a cover with a slanted shadow on the right. Below it a search bar (Ctrl K) and sorting, below that a dense cover grid. Cards have platform badges and an installed dot. Uninstalled games are shown faded.
- **Typography:** headings in Chakra Petch (uppercase), body text in Figtree.
- **Color:** dark slate background (around #0f1318), a single amber accent (#f2a93b), a green dot for the installed state. Only a dark theme in version 1.
- **Motion:** when the game changes on the stage, a short crossfade and the content sliding in (GSAP). The grid appears staggered on first load. A slight lift on card hover. The grid has its own scroll area and Lenis is attached to it. `prefers-reduced-motion` is respected.
- **In a narrow window:** the sidebar is hidden, the stage drops to a single column.
