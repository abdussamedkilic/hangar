# Hangar — Grouping editions into one card · Design

> **REVISION 2 (2026-11-13, this is the one in force).** The user saw the first implementation (d23fb2d) and rejected Remastered and Original landing on the same card. Then the council convened and the user made a decision. The rules below replace anything written differently in the rest of this document.
>
> **Card rule:** One card corresponds to one build.
> - Card key:
>   - if the label is "Orijinal" (Original), `normalize(title)` (the existing `games.norm_title` is preserved);
>   - otherwise `normalize(title) + "|" + label`.
> - **Suffixes that land on a separate card:** edition suffixes and bundle suffixes. Remastered, Definitive, Director's Cut, Enhanced, Anniversary, Complete Edition, GOTY, Deluxe, Gold, Ultimate, Special each become a separate card. User (2026-11-13): "BioShock Infinite" and "BioShock Infinite Complete Edition" are separate cards.
> - **Copy noise that lands on the same card:** different stores, the `™`/`®`/`©` difference and the "- Amazon Prime" suffix. "- Amazon Prime" becomes a copy label ("Prime") and does not enter the key.
> - **A real copy in the same store:** two records with the same store, key and copy label stay separate cards, as today.
> - **Manual merge/split** preferences apply exactly as before.
>
> **Card name:** `display_base(title)`; if the label is not "Orijinal", `" " + label` is appended. Examples: "BioShock", "BioShock Remastered", "BioShock Infinite Complete Edition".
>
> **Badges:**
> - Each store is shown once. Order: Steam, Epic, GOG, Ubisoft, Riot, Local ("Yerel").
> - At most 3 badges; the rest become "+N".
> - The badge row does not overflow.
> - The tooltip (`title`) lists all copies.
>
> **Play/Install menu:**
> - Row format: `Oyna|Kur · Store` (Play|Install · Store), followed by ` (Prime)` if there is a copy label. If the edition labels of the records inside the card differ (happens only with manual merge), ` · Label` is added too.
> - If two rows still read the same, the full title in the store is written under it as a faded second line.
> - Order: installed first, then preferred, then without a copy label, then platform priority.
>
> **Preference and launching:**
> - The preference is stored per record: `"platform:id"`. The old value (`"steam"`) is read as the first record on that platform.
> - The launch and preference commands take an optional `platformId`.
>
> **Preservation:** Original cards keep the old `norm_title` row, its cover and its preference. New build cards get their own row and download their cover once.
>
> **Test and approval:**
> - Exact expected result for BioShock's 16 real records: 6 cards.
>   - BioShock 2 records, BioShock Remastered 4, BioShock 2 2, BioShock 2 Remastered 4, BioShock Infinite 1, BioShock Infinite Complete Edition 3.
> - Before the UI is published, the card list on a copy of the live library is shown to the user.

Date: 2026-11-13 · Status: awaiting approval · The user chose approach A.

## Intent and rationale

Different editions of the same game currently appear as separate cards in the library and the user gets confused. Example: BioShock has 4 separate cards. On Steam "BioShock" and "BioShock Remastered", on GOG "BioShock™" and "BioShock™ Remastered - Amazon Prime".

The user wants a single card. The card will say "BioShock", and the edition will be chosen from inside the card.

Today there are two reasons for this:

1. **Same-key rule in the same store.** Because "Remastered" is dropped from the end, the original and Remastered fall on the same key (`bioshock`). But two records with the same key in the same store are deliberately not merged. This rule is the "dup" rule in the `group` function in `merge.rs`. As a result one is pushed onto a separate card.
2. **The "- Amazon Prime" suffix is not dropped.** This suffix has 75 records in the GOG titles coming from Galaxy. So the Prime copies always form separate cards.

Success criteria:
- BioShock, BioShock 2 and BioShock Infinite each become one card.
- The card name is written without edition suffixes ("BioShock").
- In the Oyna (Play) menu editions are grouped, with platforms listed under each edition.
- The selected edition and platform are remembered for the card.

## Goals

1. **Edition and copy label:** from each record's title an edition label ("Orijinal", "Remastered", "Complete Edition", "GOTY" etc.) and an optional copy label ("Prime") are extracted.
2. **Single card:** records falling into the same family are gathered on one card, even if they are from the same store. The family is determined by the existing normalization and the dropped suffixes. There are two exceptions:
   - Two records in the same store with **the same edition and copy label** are truly copies; they stay separate as today.
   - Local games are still not merged automatically.
3. **Card name:** the base name is shown. The suffix, `™`, `®` and trailing separators (` -`, `:`) are dropped, case is preserved ("BioShock").
4. **The Oyna menu is grouped by edition.** The main button writes the edition too only if there are multiple editions: `Oyna · Remastered · Steam`.
5. **Remembered choice:** the preference is now stored as a **record** (platform + id), not a "platform". The platform value written in old records stays valid.
6. **Default record:**
   1. the remembered record;
   2. otherwise among installed records the most recently played, on a tie platform priority (Steam > Epic > Ubisoft > GOG > Riot > Local);
   3. if none is installed, non-Original editions first, then platform priority.

   The last rule is my decision; the reason is that the remade version is often the preferred one.
7. **Detail panel:** the platform row is written together with the edition, for example `Remastered: Steam + GOG · Orijinal: Steam`.

## Non-goals

- Grouping different games as a "series". BioShock 2 is a separate game and stays a separate card.
- Details such as merging record/playtime across editions.
- A UI for editing the suffix list by hand.
- A new UI for moving an edition from a card to a separate card. The existing "Ayır" (Split) and "Birleştir" (Merge) stay valid and keep working per record.

## Anti-goals

- Breaking the existing "Ayır" and "Birleştir" preferences.
- Losing the remembered platform preferences.
- Different games merging. For example "BioShock" and "BioShock 2" never merge; the suffix list drops only trailing whole words.
- Rust `merge::normalize` and TS `normalize` drifting apart. This rule is written in CLAUDE.md.

## Approaches

- **A — Single card, editions inside (chosen).** The design above.
- **B — Separate cards, stacked in the grid.** A smaller job, but it does not give the "single card" the user wants.

## Design

### 1. Labeling (`merge.rs`)

- The following are added to the `SUFFIXES` list:
  - copy suffix: `amazon prime` (when normalized together with the leading ` - ` separator it becomes a trailing `amazon prime`);
  - edition suffixes: `enhanced edition`, `special edition`, `gold edition`, `anniversary edition`.
- New pure function: `pub fn edition_of(title: &str) -> Edition`. Return type: `Edition { label: String, copy: Option<String> }`.
  - It uses the same trailing-drop loop as `normalize` and collects the dropped suffixes in order.
  - `amazon prime` → `copy = Some("Prime")`.
  - The first edition suffix, in readable form, becomes the label. The readable equivalents sit in a fixed table: `remastered` → "Remastered", `complete edition` → "Complete Edition", `goty` and `game of the year edition` → "GOTY", `director s cut` → "Director's Cut" etc.
  - If there is no edition suffix, `label = "Orijinal"`.
- New pure function: `pub fn display_base(title: &str) -> String`. From the original title it trims `™`/`®`/`©` and the dropped suffixes from the end, case-insensitively, and removes trailing ` -`, `:` and spaces. If the result is empty, the original title is returned.
- The behavior of `normalize` changes only by the new suffixes. TS `normalize` is updated with the same list and the same test table is added to both sides.

### 2. Grouping (`merge.rs::group`)

- The "dup" rule changes: same store and same `normalize` value is **not enough**; the `edition_of` result (label + copy) must also be the same.
  - So Steam's "BioShock" and "BioShock Remastered" now enter the same family: both have the key `bioshock`, but their editions differ, so they do not count as dup.
- Overrides (merge/split) work as before; they are bound to the record identity.
- Existing card identities are preserved via `games.norm_title`. The family key is still the `normalize` value, so the BioShock card keeps its cover and last played. Cards that stop being dups, such as `bioshock#steam:7670`, are deleted; this is an expected result.

### 3. Model and database

- `Game` gets a new field: `display_title`. This is the card name and is computed with `display_base(<title of the record to show>)`. The existing `title` field stays as before.
  - Alternatively `replace_entries` can write the base name directly to `games.title`. Which one to choose is left to the builder; the choice is written during planning.
- `PlatformEntry` does not carry the edition. The edition is derived from the title everywhere with the same function. For the UI a ready grouping such as `editions: { label, copy, keys[] }` can also be returned inside `Game`; the choice is left to the builder.
- `games.preferred` stays TEXT. The new value format is `"<platform>:<platform_id>"`; the old value (`"steam"`) is read as "the first record on that platform".
  - On the Rust side `preferred` becomes a new type: `Preferred::Entry(Platform, String) | Preferred::Platform(Platform)`.
  - On the TS side it is `preferred: string | null` and is resolved with a helper function.

### 4. Launching

- `library::resolve(game, want: Option<(Platform, String)>)` applies a specific record or the default rule above.
- The `launch` and `set_preferred` commands take an optional `platformId` together with `platform`. Old calls (platform only) keep working.

### 5. UI

- **`GameCard`:** `display_title` is shown as the title; platform badges are deduplicated (the same platform once).
- **`PlayButton`:**
  - The menu is grouped by edition headings. With a single edition the current flat list stays.
  - Item text is `Oyna · Steam`, and with a copy label `Kur · GOG (Prime)`.
  - The choice is made with `onChoose(platform, platformId)`.
- **`Stage`:** the title is `display_title`; the platform row is written with editions if there are multiple editions.
- **Search:** searches `display_title` and all record titles.
- **Sketch:**

```
BioShock                                [Oyna · Remastered · Steam ▾]
                                        ┌──────────────────────────┐
                                        │ Remastered               │
                                        │   Oyna · Steam ✓         │
                                        │   Kur · GOG              │
                                        │   Kur · GOG (Prime)      │
                                        │   Kur · Epic             │
                                        │ Orijinal                 │
                                        │   Kur · Steam            │
                                        │   Kur · GOG              │
                                        └──────────────────────────┘
```

## Tests and verification

**`merge.rs`:**
- `edition_of`: Original, Remastered, Complete Edition, GOTY, the Prime copy and the "Remastered - Amazon Prime" combination.
- `display_base`: `BioShock™ Remastered - Amazon Prime` → `BioShock`, `BioShock Infinite: Complete Edition` → `BioShock Infinite`.
- `group`: BioShock's 7 records become a single card. "BioShock 2" stays separate. The same store, same edition and same copy label still count as dup. Overrides are preserved.

**`library.rs`:**
- `resolve` default rules.
- Reading the old `preferred` value.

**TS:**
- Equivalence table for `normalize`.
- `defaultEntry`.
- `PlayButton` grouped-menu tests.

**By hand:** the card count for BioShock drops to 3 (BioShock, BioShock 2, BioShock Infinite). Choosing GOG (Prime) from the menu opens Galaxy. When the app is reopened the choice is remembered.

## Decisions I made after the builder check

- **Card name:** no separate `display_title` field is created. `replace_entries` writes the base name directly to `games.title`. This way no extra change is needed in `Cover`, `MergePicker` and search. `display_title` in sections 3 and 5 should be read as `game.title`.
- **Edition info:** Rust returns a ready `editions` list for `Game`. Each item carries: `label`, `copy` and the ordered `entries` keys (`[platform, platform_id]`). The front end does not keep a copy of `edition_of`. Order of editions: non-Original first (by label), Original last.
- **The old `preferred` value** (platform only) is resolved among that platform's records in the order of default rule 3: non-Original first, then record id.
- **Tie in rule 3:** the label is looked at first, then platform priority.
- **Two identical editions from the same store** are on the same card only if merged manually. Two "Steam" rows appear in the menu; this is a rare case and acceptable.
- "Ayır"/"Birleştir" work as today, their behavior does not change.

## Left to the builder

- The full content of the readable label table.
- CSS details; the existing menu styles are used.
