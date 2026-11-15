# Grouping Editions on a Single Card — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Editions of the same game (Original / Remastered / Complete…) and its copies (Amazon Prime) are collected on a single card. The card shows the base name, the Play menu is grouped by edition, and the selected entry is remembered.

**Architecture:** Labeling and grouping are done with pure functions in `hangar-core/src/merge.rs`. `Game` carries the `editions` and `default_entry` fields computed on the Rust side. The `preferred` field becomes an entry key (`[platform, platform_id]`) instead of a platform. The front end does not derive the edition itself; it only displays these fields.

**Tech Stack:** Rust (rusqlite, serde), Tauri 2, React 18 + TS + vitest.

**Spec:** `docs/superpowers/specs/2026-11-13-hangar-editions-design.md`

## Global Constraints

- Rust `merge::normalize` and TS `normalize` (`src/lib/library.ts`) use the same suffix list and give exactly the same result. Both sides are tested with the same example table.
- Existing merge/split overrides and their per-entry behavior do not change.
- Old `games.preferred` values (like `"steam"`) remain readable.
- UI copy is Turkish. The label is "Orijinal"; the copy label is "Prime" and appears in the menu as `GOG (Prime)`.
- Commit messages are English conventional and end with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Commands run in Windows Git Bash, at the repo root. For Node, first `export PATH="/c/Users/user/AppData/Roaming/fnm/node-versions/v26.13.0/installation:$PATH"` is needed. Checks: `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`.

## Review Focus

1. "BioShock" and "BioShock 2" never merge; a number suffix is not an edition suffix. Test: T1 `bioshock_family`.
2. Two entries with the same store, the same edition and the same copy label count as real duplicates and stay separate. Test: T1 `true_duplicate_stays_split`.
3. An old `preferred = "steam"` value is resolved deterministically on a card with multiple Steam entries. Test: T2 `legacy_preferred_resolves`.
4. For a title that is only a suffix the base name does not become empty; for example a game named "Remastered". Test: T1 `display_base_never_empty`.
5. An entry that has an existing "split" override stays separate in the new grouping too. Test: T1 `split_override_still_wins`.

---

### Task 1: Labeling and grouping (core + TS normalize)

**Files:** Modify `crates/hangar-core/src/merge.rs`, `src/lib/library.ts`, `src/lib/library.test.ts`

**Interfaces — Produces:**
- Additions to the `SUFFIXES` list: `"enhanced edition"`, `"special edition"`, `"gold edition"`, `"anniversary edition"`, `"amazon prime"`. The same list is added to TS.
- `#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)] pub struct Edition { pub label: String, pub copy: Option<String> }`
- `pub fn edition_of(title: &str) -> Edition`
  - Uses the same loop as `normalize` and collects the stripped suffixes in order.
  - If `amazon prime` was stripped, `copy = Some("Prime")`.
  - The edition label is the readable counterpart of the first edition suffix stripped:
    - `remastered` → `Remastered`
    - `complete edition` → `Complete Edition`
    - `goty` and `game of the year edition` → `GOTY`
    - `definitive edition` → `Definitive Edition`
    - `ultimate edition` → `Ultimate Edition`
    - `deluxe edition` → `Deluxe Edition`
    - `director s cut` → `Director's Cut`
    - `enhanced edition` → `Enhanced Edition`
    - `special edition` → `Special Edition`
    - `gold edition` → `Gold Edition`
    - `anniversary edition` → `Anniversary Edition`
  - If there is no edition suffix the label is `Orijinal`.
- `pub fn display_base(title: &str) -> String`
  - First `™`, `®` and `©` are removed.
  - Then the same suffixes that `normalize` stripped are trimmed from the end of the original title, ignoring case and punctuation differences. Example: "BioShock™ Remastered - Amazon Prime" → "BioShock", "BioShock Infinite: Complete Edition" → "BioShock Infinite".
  - Trailing ` -`, `:`, `,` and spaces are removed from the trimmed title.
  - If the result is empty, `title.trim()` is returned.
  - Algorithm: walk backward from the end of the title word by word; at each step compare the `clean()` result of the remaining prefix with `normalize(title)`. Take the longest prefix for which the two are equal.
- The dup rule in `group` changes. The new dup condition has three parts: `(norm, platform, edition_of(title))`. That is, the same store and the same `norm` is not enough, the `Edition` must also be the same. Local entries still always count as dups.

- [ ] **Step 1: Write the tests.** All in the `merge.rs` tests section:

```rust
#[test] fn edition_labels() {
    assert_eq!(edition_of("BioShock"), Edition { label: "Orijinal".into(), copy: None });
    assert_eq!(edition_of("BioShock™ Remastered"), Edition { label: "Remastered".into(), copy: None });
    assert_eq!(edition_of("BioShock™ Remastered - Amazon Prime"), Edition { label: "Remastered".into(), copy: Some("Prime".into()) });
    assert_eq!(edition_of("BioShock Infinite: Complete Edition"), Edition { label: "Complete Edition".into(), copy: None });
    assert_eq!(edition_of("Hades - Amazon Prime"), Edition { label: "Orijinal".into(), copy: Some("Prime".into()) });
}
#[test] fn display_base_cases() {
    assert_eq!(display_base("BioShock™ Remastered - Amazon Prime"), "BioShock");
    assert_eq!(display_base("BioShock Infinite: Complete Edition"), "BioShock Infinite");
    assert_eq!(display_base("BioShock® 2"), "BioShock 2");
    assert_eq!(display_base("Death's Door"), "Death's Door");
}
#[test] fn display_base_never_empty() { assert_eq!(display_base("Remastered"), "Remastered"); }
#[test] fn bioshock_family() {
    // steam 7670 "BioShock", steam 409710 "BioShock Remastered", gog 2022341186 "BioShock™",
    // gog 1439656515 "BioShock™ Remastered", gog 1157510446 "BioShock™ Remastered - Amazon Prime",
    // epic X "BioShock Remastered", steam 8850 "BioShock 2", gog 1806891286 "BioShock™ 2"
    // → exactly 2 groups: "bioshock" with 6 entries, "bioshock 2" with 2 entries.
}
#[test] fn true_duplicate_stays_split() { /* two steam entries both "Foo Remastered" (ids 1,2) → 2 groups */ }
#[test] fn split_override_still_wins() { /* bioshock family + Override{a: steam 409710, b: steam 7670, merged:false} → steam 7670 in its own group */ }
```

Equivalence table to add to the `library.test.ts` test: for `normalize`, `"BioShock™ Remastered - Amazon Prime" → "bioshock"`, `"Hades - Amazon Prime" → "hades"`, `"Baldur's Gate Enhanced Edition" → "baldur s gate"`, `"BioShock 2" → "bioshock 2"`. The same four rows are added to the Rust `normalize` test too.

- [ ] **Step 2:** Run `cargo test -p hangar-core merge` and `npm test` → FAIL is expected.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit` → all PASS.
- [ ] **Step 5:** Commit: `feat(core): edition labels and grouping editions into one family`.

---

### Task 2: Model, preferred entry, launch (core + Tauri)

**Files:** Modify `crates/hangar-core/src/model.rs`, `crates/hangar-core/src/db.rs`, `crates/hangar-core/src/library.rs`, `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes (T1): `edition_of`, `display_base`, `Edition`.
- Produces:
  - `pub type EntryKey = (Platform, String);` (JSON: `["steam","7670"]`)
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct EditionGroup { pub label: String, pub entries: Vec<EditionEntry> }`
  - `pub struct EditionEntry { pub platform: Platform, pub platform_id: String, pub copy: Option<String> }`, with the same derives.
  - `Game` fields:
    - `pub preferred: Option<EntryKey>` (the old type was `Option<Platform>`),
    - `#[serde(default)] pub editions: Vec<EditionGroup>`,
    - `#[serde(default)] pub default_entry: Option<EntryKey>`.
  - `pub fn editions_of(entries: &[PlatformEntry]) -> Vec<EditionGroup>` (`library.rs`)
    - Groups: non-Original first (alphabetical by label), `Orijinal` last.
    - Order within a group: platform priority (`Platform::ALL`), then those without a copy first, then `platform_id`.
  - `pub fn default_entry(game: &Game) -> Option<EntryKey>` (`library.rs`), in order:
    1. if `preferred` is a valid entry, that entry;
    2. among installed entries, the one with the largest `last_played`; on a tie, the `editions_of` order;
    3. if none is installed, the first entry in `editions_of` order.
  - `pub fn resolve(game: &Game, want: Option<&EntryKey>) -> Option<(&PlatformEntry, String)>`
    - If `want` exists that entry is used, otherwise `default_entry`.
    - URI selection as today: `launch_uri` if installed, otherwise `install_uri`.
  - `Db::games()`:
    - The `preferred` text is read like this: if it has the form `"<platform>:<id>"` it becomes the entry key; if it is only a platform name it becomes the first entry of that platform in `editions_of` order.
    - `editions` and `default_entry` are filled in.
  - `Db::set_preferred(&self, game_id: i64, key: &EntryKey) -> Result<()>` writes the value in the form `"<platform>:<id>"`.
  - `replace_entries` writes `display_base(<title of the entry chosen by today's selection>)` to `games.title`.
  - Tauri commands:
    - `launch(app, db, game_id: i64, platform: Option<Platform>, platform_id: Option<String>)` → `want`: `(platform, id)` if both exist, if only the platform exists the first entry of that platform in `editions_of` order.
    - `set_preferred(db, game_id: i64, platform: Platform, platform_id: Option<String>)` → if there is no id it is resolved by the same rule.

- [ ] **Step 1: Write the tests** (`library.rs` / `db.rs`):

```rust
#[test] fn editions_order_and_copy() { /* bioshock family entries → groups ["Remastered","Orijinal"]; Remastered entries order steam, epic, gog, gog(copy Prime) */ }
#[test] fn default_prefers_installed_last_played() { /* two installed, last_played 5 and 9 → the 9 one */ }
#[test] fn default_uninstalled_prefers_non_original() { /* none installed → first Remastered entry (steam) */ }
#[test] fn legacy_preferred_resolves() { /* db: set games.preferred='steam' raw, bioshock family → preferred == Some((Steam,"409710")) (Remastered first) */ }
#[test] fn set_preferred_roundtrip() { /* set_preferred(id,&(Gog,"1157510446".into())) → games()[..].preferred == that key, default_entry == that key */ }
#[test] fn card_title_is_base() { /* after scan_all of bioshock family, the game title == "BioShock" */ }
```

Existing `resolve_*` and `preferred` tests are adapted to the new signature.
- [ ] **Step 2:** FAIL is expected.
- [ ] **Step 3:** Implement. Then `launch` and `set_preferred` in `commands.rs` are updated; `main.rs` does not change.
- [ ] **Step 4:** `cargo test -p hangar-core`, `cargo check -p hangar` → PASS.
- [ ] **Step 5:** Commit: `feat(core): per-entry preference and edition groups`.

---

### Task 3: UI — grouped menu, card title

**Files:** Modify `src/lib/api.ts`, `src/lib/library.ts`, `src/components/PlayButton.tsx` (+ test), `src/components/Stage.tsx`, `src/components/GameCard.tsx`, `src/App.tsx`, related tests.

**Interfaces:**
- Consumes (T2), JSON fields:
  - `preferred: [Platform, string] | null`
  - `editions: { label: string; entries: { platform: Platform; platform_id: string; copy: string | null }[] }[]`
  - `default_entry: [Platform, string] | null`
- Produces:
  - `api.launch(gameId, platform?, platformId?)` → `invoke("launch", { gameId, platform, platformId })`
  - `api.setPreferred(gameId, platform, platformId)` → `invoke("set_preferred", { gameId, platform, platformId })`
  - `defaultPlatform` is deleted from `library.ts`. In its place comes `export function currentEntry(g: Game): PlatformEntry`: the entry matching `default_entry`, otherwise `g.entries[0]`.
- **`PlayButton`:**
  - Main button text: `{Oyna|Kur} · {PLATFORM_NAME}{copy ? " (" + copy + ")" : ""}`. If `editions.length > 1`, the edition label is also added before the platform: `Oyna · Remastered · Steam`.
  - Menu: if `editions.length > 1`, for each group first a `role="presentation"` heading (the label), under it a `menuitem` for each entry. With a single edition the current flat list stays.
  - A `✓` stands next to the current selection.
  - Props: `onLaunch(key: [Platform, string])`, `onChoose(key: [Platform, string])`.
- **`App.tsx`:** `launch` and `choose` now take a key and call `api.launch` and `api.setPreferred` with the id.
- **`Stage`:**
  - If `editions.length > 1`, the platform row at the top is written with editions: `Remastered: Steam + GOG · Orijinal: Steam`. Platforms are deduplicated within each edition.
  - With a single edition it stays as today.
- **`GameCard`:** platform badges are deduplicated; the same platform appears only once.

- [ ] **Step 1: Write the tests** (`PlayButton.test.tsx`, `library.test.ts`):
  - **Grouped menu:**
    - For a game with two editions the "Remastered" and "Orijinal" headings appear in the menu.
    - There is a `Kur · GOG (Prime)` row.
    - Clicking a row calls `onChoose(["gog","1157510446"])`.
    - The main button shows `Kur · Remastered · Steam`.
  - **Single edition:** the flat list and the old text (`Oyna · Steam`) are preserved.
  - **`currentEntry`:** returns the entry matching `default_entry`.
  - **Card badges:** on a card with two GOG entries a single `GOG` badge appears (`GameCard` test).
- [ ] **Step 2:** `npm test` → FAIL is expected.
- [ ] **Step 3:** Implement. The existing menu CSS is used; a single small class (`.play-menu .grp`) may be added for the group heading.
- [ ] **Step 4:** `npm test`, `npx tsc --noEmit`, `cargo check -p hangar` → PASS.
- [ ] **Step 5:** Commit: `feat(ui): edition groups in the play menu`.

---

### Task 4: Manual check (reviewer + user)

- [ ] Rescan. Searching for BioShock should show 3 cards: BioShock, BioShock 2, BioShock Infinite.
- [ ] BioShock menu: under Remastered Steam, Epic, GOG and GOG (Prime); under Orijinal Steam and GOG.
- [ ] After selecting GOG (Prime) and reopening the app, the selection must be kept.
- [ ] On an installed game "Oyna" must work as before. Death's Door must remain a single card.
