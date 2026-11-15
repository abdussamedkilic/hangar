# Hangar — Collections, list view, theme file · Design

Date: 2026-11-14 · Status: the user approved in conversation ("okeyim" — "fine by me"), document approval pending
Branch: `feat/phase3` (from main, after Phase 2 = PR #5)

## Intent and rationale
Hangar is "considered done"; the user wants to see their own library in their own arrangement. There are three requests:
1. Manual collections, like Steam's.
2. A list view next to the card view.
3. Color customization with a VS Code-style JSON file.

The sorting logic works well and will be kept. The desktop exe will be done when everything is finished (outside this document). Amazon Games will not be done. Known minor gaps are a separate, parallel job.

## Goals
- **Tab strip:** a horizontal strip above the game area. The order is always:
  - `Tümü` (All)
  - store tabs: Steam, Epic, GOG, Ubisoft, Riot, Yerel (Local); only those with games are visible
  - separator
  - user collections
  - `+`
- **Manual collection:**
  - Creating, renaming and deleting.
  - Adding and removing games is done from the right-click menu on the card.
  - A game can be in multiple collections.
- **The "Kurulu" (Installed) filter** stays in the sidebar and is applied together with the tab in the strip (e.g. Ubisoft + Installed). When on, a "Kurulu ✕" label appears next to the strip.
- **View choice:** a Card/List switch to the right of the strip; the choice is persistent. The card view is the default.
- **List view:** per row a small cover, name, store badges, installed status, playtime (if any) and the Oyna/Kur button. The right-click menu and sorting are the same as the card view.
- **Theme file:** `%APPDATA%\com.hangar.app\theme.json` changes only colors.
  - Two buttons in Settings: "Tema dosyasını aç" (Open theme file) and "Varsayılana dön" (Back to default).
  - When Hangar rereads the file, the colors are applied immediately.
  - Invalid values are ignored and a short warning is shown.

## Non-goals
- Dynamic (rule-based) collections. The user sees no need; the store and Installed filters meet this need.
- Importing Steam collections.
- Ordering collections by drag-and-drop.
- Customizing fonts, sizes and layout.
- Switching between multiple themes.

## Anti-goals (count as failure even if it works)
- A game dropping out of a collection when merged or split.
- A rescan deleting collection membership.
- The tab strip getting crowded and pushing the cards down. The strip is a single row and scrolls horizontally if it does not fit.
- A faulty theme file making the UI unreadable. An invalid color is ignored.
- A change in the existing sorting or search behavior.

## Constraints
- **Stack:** React 18 + TS + Vite. Colors are the CSS variables in `src/styles/tokens.css`. Data is in SQLite (`crates/hangar-core/src/db.rs`), commands are in `src-tauri/src/commands.rs`.
- **Card identity:** A card is a `games` row (`norm_title` = the card key). Merging or splitting can change the card's row; the merged row is deleted.
- **Record identity:** The record id has the format `"platform:platform_id"` (the preference is also stored in this format). It is stable across scans.
- **UI:** The visual style "Kokpit + Vitrin" (Cockpit + Showcase) is preserved (`docs/design/mockups-melez.html`). For UI work, the design-taste-frontend and ui-ux-pro-max rules are consulted.
- **Test:** Tested only in the Tauri window (`localhost:1420` does not work in a browser).

## Approaches
**The key of collection membership**
1. *By record key (`platform:id`) — chosen.* A card counts as in the collection if any of its records is in the collection. Membership stays correct automatically on merge and split. Adding writes all of the card's records, removing deletes all of its records.
2. *By `games.id`.* The simplest, but on merge the membership is lost together with the deleted row. This violates the anti-goal.
3. *By card key (`norm_title`).* Membership breaks when the card key changes on split and manual merge.

**Applying the theme file**
1. *A Rust command reads the file, the front end writes the valid colors to `:root` CSS variables; it is reread on startup and when the window gets focus — chosen.* When the file is saved in the editor and you return to Hangar, the change is visible instantly. No watcher process.
2. *A file watcher (notify).* More "live", but requires an extra dependency and process. The gain is small.

**Where collections live:** the user chose in conversation; not the sidebar, but the strip above the game area.

## Decided design
**Data** (`db.rs`, new tables, `IF NOT EXISTS` on an old database):
- `collections(id INTEGER PK, name TEXT NOT NULL, position INTEGER NOT NULL, created_at INTEGER NOT NULL)`. The name is trimmed, cannot be empty, and is unique case-insensitively. A new collection is added at the end.
- `collection_entries(collection_id INTEGER NOT NULL, platform TEXT NOT NULL, platform_id TEXT NOT NULL, PRIMARY KEY(collection_id, platform, platform_id))`. When a collection is deleted, its rows are deleted too.
- A scan does not touch `collection_entries`. If a record drops out of the library its row stays; when the game comes back, the membership comes back.

**Commands** (`commands.rs`):
- `get_collections() -> [{id, name, entries:[EntryKey]}]`
- `create_collection(name) -> id`
- `rename_collection(id, name)`
- `delete_collection(id)`
- `set_in_collection(id, entries:[EntryKey], member: bool)` — all of the card's records are sent at once.
- `get_theme() -> {colors: {key: value}, warnings: [string]}`
- `open_theme_file()` — if the file does not exist, it is created with the default content and explanations, then opened with the system editor.
- `reset_theme()` — rewrites the file with the default content.

**Front end**
- **Filter state:** The existing `Filter` (`"all" | "installed" | Platform`) is split in two:
  - tab: `"all" | Platform | {collection: id}`
  - a separate `installedOnly: boolean`
  - The existing `view(games, filter, query, sort)` is adapted accordingly. Search and sorting stay the same.
- **Sidebar:** The store list moves to the strip. What remains in the sidebar:
  - the "Kurulu" switch, with its counter
  - "Oyun ekle" (Add game), "Yenile" (Refresh), "Ayarlar" (Settings)
- **Tab strip:**
  - Each tab has a counter. The number is computed by installed games if "Kurulu" is on.
  - A store tab with a count of 0 is hidden. Collection tabs are visible even at 0.
  - If the active tab is hidden, it returns to `Tümü`. This also closes the strip's counterpart of the old sidebar "row that disappears at zero" gap.
- **Collection tab:**
  - When `+` is clicked, an inline name field opens in the strip.
  - Right-clicking the tab shows "Yeniden adlandır" (Rename) and "Sil" (Delete). Deleting asks for confirmation; the games stay in the library.
- **Right-click menu of the card and row:** has a "Koleksiyona ekle ›" (Add to collection) submenu. The collections the card is in appear checked, and clicking removes the card. The submenu also has "Yeni koleksiyon…" (New collection…).
- **List view:** The existing virtual list (`@tanstack/react-virtual`) is used with a row height. Selected game and Showcase behavior are the same as in the card view.
- **View preference:** Stored in the `settings` table as `ui.view = "grid" | "list"`.

**Theme keys** (JSON name → CSS variable):

| JSON | CSS |
|---|---|
| `background` | `--bg` |
| `panel` | `--panel` |
| `raised` | `--raise` |
| `border` | `--line` |
| `text` | `--fg` |
| `textDim` | `--dim` |
| `accent` | `--accent` |
| `accentAlt` | `--accent-alt` |
| `onAccent` | `--on-accent` |
| `installed` | `--installed` |

- Accepted values: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()/rgba()/hsl()` (validated in the front end with `CSS.supports('color', v)`).
- An unknown key or an invalid value leaves the variable at the default and produces a warning line.
- If the file is broken JSON, no color is applied and a warning is shown.
- The default file has an explanation for each key. Since JSON does not support comments, the explanations sit in a `"_yardım"` (help) key and are ignored.
- Warnings are shown in the existing top `notice` row.

## Points clarified after the builder check (Opus's decisions, the user can object)
- **Store tab:** Today's rule is preserved: a card appears in a tab if any of its records is in that store. A merged Steam+Ubisoft card shows up in both tabs; the sum of the store counters does not equal "Tümü".
- **Installed + store tab (user: "Steam always has priority"):** While in a store tab, "Kurulu" shows cards whose record in that store is installed. Watch_Dogs installed from Steam does not appear in "Ubisoft + Kurulu", it appears in "Steam + Kurulu". In "Tümü" and collection tabs, it is enough for any record of the card to be installed. Counters are computed by the same rule.
- **Steam priority:** If a card has multiple installed copies and the user has not chosen a preference, "Oyna" opens the Steam copy. Today's "most recently played wins" rule (`db.rs` `default_entry`) stays valid among copies other than Steam. In the badge order Steam is already first.
- **Collection membership:**
  - If any record of the card is in the collection, the card counts as a member and appears checked in the menu.
  - Adding writes all of the card's current records; removing deletes all of its records.
  - A new store copy arriving later is not written separately; the card stays a member through its old record.
- **Persistence:** The active tab and the "Kurulu" state are stored in the `settings` table (`ui.tab`, `ui.installed`). If the saved tab no longer exists, `Tümü` opens.
- **Rereading the theme:**
  - It is not bound to Galaxy's 10 s limit; it is done on every focus. Reading is cheap.
  - The warning is shown only when the warning list changes.
  - "Varsayılana dön" writes the file and reapplies the colors immediately.
- **Default colors:** The source of the values is `tokens.css`. The Rust template contains the same values and a test verifies that the template equals the 10 values in `tokens.css`. If there are fixed rgba colors derived from `accent`, they are converted to `color-mix(... var(--accent) ...)`; so when accent changes, the glows change too.
- **Submenu:** Open-out submenu support is added to the context menu. The menu and submenu are positioned to fit the window edge; this also closes the known "menu overflows the edge" gap. The same menu is used in list rows too.
- **Collection order and name:**
  - `position` keeps only the creation order; renaming and deleting do not change the order.
  - The uniqueness check is done in Rust with Turkish-aware lowercasing (İ/ı); SQLite NOCASE is not used.
- **"Yeni koleksiyon…" (right-click):** Creates the collection and adds the card. The active tab does not change.
- **Deleting:** If the active collection is deleted, it returns to `Tümü`.

## Left to the builder
- File division and naming of the strip, row and tab components.
- The exact dimensions of the list row, fine visual details (with the existing tokens, in the "Kokpit + Vitrin" line).
- Keyboard behavior of the inline name field: Enter saves, Esc cancels (recommended).

## Tests
- **Core:**
  - collection CRUD
  - unique name
  - membership preserved after merge and split
  - a rescan not deleting membership
  - theme file: reading, broken JSON, unknown key
- **Front end (vitest):**
  - tab and Installed combination
  - counters
  - collection tab filtering
  - list and card switch
  - ignoring an invalid color
- **Live:**
  1. Create an "AC" collection, add 3 games.
  2. Rescan: the membership is still there.
  3. Ubisoft + Installed.
  4. List view, remembered when the app is reopened.
  5. Change `accent`, return to Hangar: the color changed.
  6. Broken JSON: default colors and a warning.
