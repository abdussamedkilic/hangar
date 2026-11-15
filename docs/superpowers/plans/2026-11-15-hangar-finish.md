# Hangar Finish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the last three items: no scrollbars in the collapsed sidebar, a theme selector with locally saved named themes, and the desktop installer.

**Architecture:** Task 1 is a CSS-only fix. Task 2 replaces the single `theme.json` with named files in `app_data_dir/themes/<name>.json`; the active name lives in the DB setting `ui.theme` ("" = built-in "Varsayılan", no file). Task 3 is the Özelleştir UI. Task 4 (controller, no subagent) runs `npm run tauri build`.

**Tech Stack:** Rust (hangar-core, Tauri 2), React 18 + TS, vitest.

**Spec:** No separate spec. The user's request (2026-11-14) is binding: "özelleştirde current theme bir selector içerisine gözüksün, yeni eklediği theme'de kaydedilsin local'de bu sayede istediği zaman theme'ler arası geçiş yapabilsin" and "daraltılmışta burada scroll çıkıyor".

## Global Constraints

- UI copy Turkish; code/identifiers English; commits English conventional, ending `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- vitest runs under node only: `PATH="$APPDATA/fnm/node-versions/v26.13.0/installation:$PATH" node node_modules/vitest/vitest.mjs run` (bun gives false failures). tsc: `... node node_modules/typescript/bin/tsc --noEmit`.
- Rust checks: `cargo test -p hangar-core`, `cargo test -p hangar`, `cargo check -p hangar --release`.
- Theme file format unchanged (`theme::parse`, the 10 `KEYS`, `_yardım` help key).
- Built-in theme display name: `Varsayılan`. It has no file and cannot be edited or deleted.
- Karpathy: no unrequested features, surgical diffs.

## Review Focus

1. Upgrading user with an existing `theme.json`: their colours must survive — migrated to `themes/Özel.json` and made active, once.
2. Theme names that are invalid filenames on Windows (`a/b`, `CON`, `x.`, empty, 41+ chars) or that collide case-insensitively with an existing theme or `Varsayılan` must be rejected with a Turkish message, never written.
3. Active theme file deleted by hand outside the app: `get_theme` must fall back to defaults (empty colours, no crash), and the selector must show `Varsayılan`.
4. Deleting the active theme switches to `Varsayılan` and applies defaults immediately.
5. Collapsed sidebar with many saved filters: vertical scroll still works (wheel), but no visible scrollbar and never a horizontal one.

---

### Task 1: Collapsed sidebar scrollbars

Cause: `.grp{overflow-y:auto}` makes `overflow-x` compute to `auto`; in the 64px rail the vertical scrollbar (~10px) steals width, the 44px buttons overflow and a horizontal bar appears.

**Files:**
- Modify: `src/styles/app.css:12` and the `.collapsed` rules (lines 28-30)
- Test: `src/components/Sidebar.test.tsx`

- [ ] **Step 1: Write the failing test** (CSS text assertion; jsdom cannot lay out)

```ts
import css from "../styles/app.css?raw";
it("grp never scrolls horizontally; collapsed rail hides scrollbar", () => {
  expect(css).toMatch(/\.grp\{[^}]*overflow-x:hidden/);
  expect(css).toMatch(/\.collapsed \.grp\{[^}]*scrollbar-width:none/);
});
```

- [ ] **Step 2: Run, expect FAIL.**
- [ ] **Step 3:** Add `overflow-x:hidden` to `.grp`; add `.collapsed .grp{scrollbar-width:none}` (WebView2 Chromium supports it). Do not touch the `padding:3px;margin:-3px` (it keeps focus rings visible).
- [ ] **Step 4: Run vitest + tsc, expect PASS.**
- [ ] **Step 5: Commit** `fix(ui): hide scrollbars in the collapsed sidebar`

### Task 2: Named themes backend

**Files:**
- Modify: `crates/hangar-core/src/theme.rs` (add pure helpers + tests)
- Modify: `src-tauri/src/collections.rs` (theme commands), `src-tauri/src/main.rs` (both handler lists, lines ~86 and ~127), `src/lib/api.ts`

**Interfaces:**
- Produces (core, `theme.rs`):
  - `pub const DEFAULT_NAME: &str = "Varsayılan";`
  - `pub fn validate_name(name: &str, existing: &[String]) -> Result<String, String>` — returns the trimmed name. Rejects: empty after trim → `"Tema adı boş olamaz"`; more than 40 chars → `"Tema adı en fazla 40 karakter olabilir"`; any of `\ / : * ? " < > |` or control chars, or ending with `.` → `"Tema adında geçersiz karakter var"`; Windows reserved stems (CON, PRN, AUX, NUL, COM1-9, LPT1-9, case-insensitive) or equal to `DEFAULT_NAME` case-insensitively, or equal case-insensitively to any `existing` → `"Bu adda bir tema zaten var"` (reserved → `"Bu ad kullanılamaz"`).
  - `pub fn list_names(dir: &Path) -> Vec<String>` — stems of `*.json` files, sorted case-insensitively; missing dir → empty.
  - `pub fn migrate_legacy(app_dir: &Path) -> Option<String>` — if `app_dir/theme.json` exists: create `app_dir/themes/`, move it to `themes/Özel.json` (if that exists, `Özel 2.json`, `Özel 3.json`, …), return `Some(name)`; else `None`.
- Produces (Tauri commands, all `#[tauri::command]` in `collections.rs`):
  - `list_themes(app, db) -> ThemeList` with `#[derive(Serialize)] pub struct ThemeList { active: String, names: Vec<String> }` — `active` is `""` for Varsayılan. Runs migration first (if it returns a name, set `ui.theme` to it). If `ui.theme` names a missing file, report `active: ""`.
  - `get_theme(app, db) -> ThemeRead` — reads active file (same migration first); `""` or missing → `ThemeRead::default()`.
  - `set_active_theme(app, db, name: String)` — `""` or an existing name; else `Err("Tema bulunamadı")`.
  - `create_theme(app, db, name: String) -> String` — `validate_name` against `list_names`; contents = copy of the active theme file, or `default_file()` when active is Varsayılan; writes `themes/<name>.json`; sets it active; returns the trimmed name.
  - `delete_theme(app, db, name: String)` — must exist; removes file; if it was active set `ui.theme` to `""`.
  - `open_theme_file(app, db)` — opens the active theme's file; Varsayılan → `Err("Varsayılan tema düzenlenemez; önce yeni tema oluşturun")`.
  - Remove `reset_theme` (selecting Varsayılan replaces it) from collections.rs, both lists in main.rs and api.ts.
  - `ui.theme` is set only by these commands (do not add it to `check_ui_key`).
- Produces (`src/lib/api.ts`): `export interface ThemeList { active: string; names: string[] }`; `listThemes(): Promise<ThemeList>`, `setActiveTheme(name: string)`, `createTheme(name: string): Promise<string>`, `deleteTheme(name: string)`; `getTheme`, `openThemeFile` kept (same names, now active-aware). Remove `resetTheme`.

- [ ] **Step 1: Failing core tests** in `theme.rs` (temp dirs under `std::env::temp_dir()` with a unique suffix; no new crates):
  - `validate_trims_and_accepts`: `validate_name("  Gece ", &[])` == `Ok("Gece")`.
  - `validate_rejects`: `""`, `"   "`, 41×`"a"`, `"a/b"`, `"x."`, `"CON"`, `"nul"`, `"varsayılan"`, and `"gece"` with existing `["Gece"]` — each `Err`.
  - `list_names_sorted_and_json_only`: files `b.json`, `A.json`, `note.txt` → `["A","b"]`; missing dir → `[]`.
  - `migrate_moves_once`: write `theme.json` → returns `Some("Özel")`, `themes/Özel.json` has the old contents, `theme.json` gone; second call → `None`.
  - `migrate_avoids_collision`: existing `themes/Özel.json` → returns `Some("Özel 2")`.
- [ ] **Step 2: Run `cargo test -p hangar-core theme`, expect FAIL.**
- [ ] **Step 3: Implement the core helpers.**
- [ ] **Step 4: Implement the Tauri commands; keep any branching logic in a small pure fn testable without `AppHandle` (e.g. `fn resolve_active(pref: &str, names: &[String]) -> String`) with a unit test for the missing-file case.**
- [ ] **Step 5: Keep tsc green with the smallest UI change:** remove `reset` from `useTheme` (return `{ reload }`), and in SettingsDialog rename prop `onResetTheme` → `onThemeChanged` and delete the "Varsayılana dön" button; App passes `onThemeChanged={theme.reload}`. The selector itself is Task 3.
- [ ] **Step 6: Run `cargo test -p hangar-core`, `cargo test -p hangar`, `cargo check -p hangar --release`, tsc, vitest — expect PASS.**
- [ ] **Step 7: Commit** `feat(theme): named themes (themes/ folder, ui.theme)`

### Task 3: Theme selector UI (Özelleştir)

**Files:**
- Modify: `src/components/SettingsDialog.tsx` (the `Tema` field, ~line 92), `src/state/useTheme.ts`, `src/App.tsx:308`, `src/styles/app.css` (only if a style is needed; reuse `.fld`, `.row`, `.ghost`)
- Test: `src/components/SettingsDialog.test.tsx`, `src/state/useTheme.test.ts`

**Interfaces:**
- Consumes: Task 2 api (`listThemes`, `setActiveTheme`, `createTheme`, `deleteTheme`, `openThemeFile`).
- `useTheme` returns `{ reload(): Promise<void> }`. SettingsDialog prop: `onThemeChanged?: () => Promise<void>` (App passes `theme.reload`).

UI (Tema field):
```
Tema   [ Gece            ▾ ]   (select: "Varsayılan" first, value "", then names)
       [Yeni tema] [Dosyayı aç] [Sil]
       (Yeni tema → inline: [ad girişi] [Kaydet] [Vazgeç]; Enter = Kaydet, Esc = Vazgeç without closing the dialog)
```
- Load `listThemes()` when the dialog mounts.
- Changing the select → `setActiveTheme(v)` then `onThemeChanged()`.
- Kaydet → `createTheme(name)`; on success refresh list (new one selected), `onThemeChanged()`, close inline input; on error show message in the existing `.err` line and keep the input.
- "Dosyayı aç" and "Sil" disabled when active is Varsayılan. Sil → `window.confirm(\`"${name}" teması silinsin mi?\`)`; yes → `deleteTheme`, refresh list, `onThemeChanged()`.
- Hint under buttons (`.dim` small text): `Yeni tema, açık olan temanın kopyasıdır. Dosyayı kaydedip uygulamaya dönünce renkler uygulanır.`
- Select has an accessible label (`<label>` or `aria-label="Tema"`).

- [ ] **Step 1: Failing tests** (extend the existing api mock with `listThemes, setActiveTheme, createTheme, deleteTheme`):
  - `shows active theme in select`: listThemes → `{active:"Gece", names:["Gece","Açık"]}`; Özelleştir tab; `getByLabelText("Tema")` has value `"Gece"` and options `Varsayılan, Gece, Açık` in order.
  - `switching theme calls api and onThemeChanged`: change select to `""` → `setActiveTheme("")`, `onThemeChanged` called once.
  - `new theme saves and selects`: click Yeni tema, type `Mor`, click Kaydet → `createTheme("Mor")`; after list refresh returns `{active:"Mor",...}` the select shows `Mor`.
  - `create error is shown`: createTheme rejects `"Bu adda bir tema zaten var"` → that text in `role=alert`, input still present.
  - `default theme disables open and delete`: active `""` → "Dosyayı aç" and "Sil" disabled.
  - `delete asks and switches`: confirm stubbed true → `deleteTheme("Gece")` and `onThemeChanged` called.
  - useTheme test: remove reset tests if any; reload still applies.
- [ ] **Step 2: Run vitest, expect FAIL.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run full vitest + tsc, expect PASS.**
- [ ] **Step 5: Commit** `feat(ui): theme selector in Özelleştir`

### Task 4: Desktop installer (controller)

- [ ] `npm run tauri build` (background, long). Expect `target/release/bundle/nsis/Hangar_0.1.0_x64-setup.exe`.
- [ ] NSIS creates a Start Menu shortcut; desktop shortcut: NSIS default installer offers it on the finish page in Tauri 2 — verify after install; if missing, report to user rather than adding a custom NSIS template.
- [ ] Update the CLAUDE.md "latest status" section, memory; squash merge.
