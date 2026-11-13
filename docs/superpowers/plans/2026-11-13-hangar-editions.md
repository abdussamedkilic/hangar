# Sürümleri Tek Kartta Toplama — Uygulama Planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Aynı oyunun sürümleri (Orijinal / Remastered / Complete…) ve kopyaları (Amazon Prime) tek kartta toplansın. Kart temel adı göstersin, Oyna menüsü sürüme göre gruplu olsun, seçilen kayıt hatırlansın.

**Architecture:** Etiketleme ve gruplama `hangar-core/src/merge.rs`'te saf fonksiyonlarla yapılır. `Game`, Rust tarafında hesaplanan `editions` ve `default_entry` alanlarını taşır. `preferred` alanı platform yerine kayıt anahtarı (`[platform, platform_id]`) olur. Ön yüz sürümü kendisi türetmez; yalnızca bu alanları gösterir.

**Tech Stack:** Rust (rusqlite, serde), Tauri 2, React 18 + TS + vitest.

**Spec:** `docs/superpowers/specs/2026-11-13-hangar-editions-design.md`

## Global Constraints

- Rust `merge::normalize` ile TS `normalize` (`src/lib/library.ts`) aynı ek listesini kullanır ve birebir aynı sonucu verir. Her iki taraf aynı örnek tabloyla test edilir.
- Mevcut birleştir/ayır override'ları ve kayıt bazındaki davranışları değişmez.
- Eski `games.preferred` değerleri (`"steam"` gibi) okunabilir kalır.
- Arayüz metinleri Türkçedir. Etiket "Orijinal"dir; kopya etiketi "Prime"dır ve menüde `GOG (Prime)` biçiminde görünür.
- Commit mesajları Türkçe conventional olur ve `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` satırıyla biter.
- Komutlar Windows Git Bash'te, repo kökünde çalışır. Node için önce `export PATH="/c/Users/user/AppData/Roaming/fnm/node-versions/v26.13.0/installation:$PATH"` gerekir. Kontroller: `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit`.

## Review Focus

1. "BioShock" ile "BioShock 2" asla birleşmez; sayı soneki ek değildir. Test: T1 `bioshock_family`.
2. Aynı mağaza, aynı sürüm ve aynı kopya etiketi olan iki kayıt gerçek kopya sayılır ve ayrı kalır. Test: T1 `true_duplicate_stays_split`.
3. Eski `preferred = "steam"` değeri birden çok Steam kaydı olan kartta deterministik olarak çözülür. Test: T2 `legacy_preferred_resolves`.
4. Yalnızca eki olan bir başlıkta temel ad boş kalmaz; örneğin "Remastered" adlı bir oyun. Test: T1 `display_base_never_empty`.
5. Mevcut "ayır" override'ı olan bir kayıt yeni gruplamada da ayrı kalır. Test: T1 `split_override_still_wins`.

---

### Task 1: Etiketleme ve gruplama (core + TS normalize)

**Files:** Modify `crates/hangar-core/src/merge.rs`, `src/lib/library.ts`, `src/lib/library.test.ts`

**Interfaces — Produces:**
- `SUFFIXES` listesine eklenecekler: `"enhanced edition"`, `"special edition"`, `"gold edition"`, `"anniversary edition"`, `"amazon prime"`. Aynı liste TS'e de eklenir.
- `#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)] pub struct Edition { pub label: String, pub copy: Option<String> }`
- `pub fn edition_of(title: &str) -> Edition`
  - `normalize` ile aynı döngüyü kullanır ve atılan ekleri sırayla toplar.
  - `amazon prime` atıldıysa `copy = Some("Prime")`.
  - Sürüm etiketi, atılan ilk sürüm ekinin okunur karşılığıdır:
    - `remastered` → `Remastered`
    - `complete edition` → `Complete Edition`
    - `goty` ve `game of the year edition` → `GOTY`
    - `definitive edition` → `Definitive Edition`
    - `ultimate edition` → `Ultimate Edition`
    - `deluxe edition` → `Deluxe Edition`
    - `director s cut` → `Director's Cut`
    - `enhanced edition` → `Enhanced Edition`
    - `special edition` → `Special Edition`
    - `gold edition` → `Gold Edition`
    - `anniversary edition` → `Anniversary Edition`
  - Sürüm eki yoksa etiket `Orijinal` olur.
- `pub fn display_base(title: &str) -> String`
  - Önce `™`, `®` ve `©` silinir.
  - Ardından `normalize` hangi ekleri attıysa, aynı ekler orijinal başlığın sonundan büyük/küçük harf ve noktalama farkı gözetmeden kırpılır. Örnek: "BioShock™ Remastered - Amazon Prime" → "BioShock", "BioShock Infinite: Complete Edition" → "BioShock Infinite".
  - Kırpılan başlığın sonundaki ` -`, `:`, `,` ve boşluklar silinir.
  - Sonuç boş kalırsa `title.trim()` döner.
  - Algoritma: başlığın sonundan geriye doğru kelime kelime ilerle; her adımda kalan önekin `clean()` sonucunu `normalize(title)` ile karşılaştır. İkisinin eşit olduğu en uzun öneki al.
- `group` içindeki dup kuralı değişir. Yeni dup koşulu üç parçalı: `(norm, platform, edition_of(title))`. Yani aynı mağaza ve aynı `norm` yetmez, `Edition` da aynı olmalı. Yerel kayıtlar yine her zaman dup sayılır.

- [ ] **Step 1: Testleri yaz.** Hepsi `merge.rs` tests bölümüne:

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

`library.test.ts` testine eklenecek eşdeğerlik tablosu: `normalize` için `"BioShock™ Remastered - Amazon Prime" → "bioshock"`, `"Hades - Amazon Prime" → "hades"`, `"Baldur's Gate Enhanced Edition" → "baldur s gate"`, `"BioShock 2" → "bioshock 2"`. Aynı dört satır Rust `normalize` testine de eklenir.

- [ ] **Step 2:** `cargo test -p hangar-core merge` ve `npm test` çalıştır → FAIL beklenir.
- [ ] **Step 3:** Uygula.
- [ ] **Step 4:** `cargo test -p hangar-core`, `cargo check -p hangar`, `npm test`, `npx tsc --noEmit` → hepsi PASS.
- [ ] **Step 5:** Commit at: `feat(core): sürüm etiketleri ve sürümleri tek ailede gruplama`.

---

### Task 2: Model, tercih edilen kayıt, başlatma (core + Tauri)

**Files:** Modify `crates/hangar-core/src/model.rs`, `crates/hangar-core/src/db.rs`, `crates/hangar-core/src/library.rs`, `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes (T1): `edition_of`, `display_base`, `Edition`.
- Produces:
  - `pub type EntryKey = (Platform, String);` (JSON: `["steam","7670"]`)
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct EditionGroup { pub label: String, pub entries: Vec<EditionEntry> }`
  - `pub struct EditionEntry { pub platform: Platform, pub platform_id: String, pub copy: Option<String> }`, aynı derive'larla.
  - `Game` alanları:
    - `pub preferred: Option<EntryKey>` (eski tip `Option<Platform>`),
    - `#[serde(default)] pub editions: Vec<EditionGroup>`,
    - `#[serde(default)] pub default_entry: Option<EntryKey>`.
  - `pub fn editions_of(entries: &[PlatformEntry]) -> Vec<EditionGroup>` (`library.rs`)
    - Gruplar: Orijinal olmayanlar önce (etikete göre alfabetik), `Orijinal` en sonda.
    - Grup içi sıra: platform önceliği (`Platform::ALL`), sonra kopyası olmayan önce, sonra `platform_id`.
  - `pub fn default_entry(game: &Game) -> Option<EntryKey>` (`library.rs`), sırayla:
    1. `preferred` geçerli bir kayıtsa o kayıt;
    2. kurulu kayıtlar arasından `last_played` en büyük olan; eşitlikte `editions_of` sırası;
    3. hiçbiri kurulu değilse `editions_of` sırasındaki ilk kayıt.
  - `pub fn resolve(game: &Game, want: Option<&EntryKey>) -> Option<(&PlatformEntry, String)>`
    - `want` varsa o kayıt kullanılır, yoksa `default_entry`.
    - URI seçimi bugünkü gibi: kuruluysa `launch_uri`, değilse `install_uri`.
  - `Db::games()`:
    - `preferred` metni şöyle okunur: `"<platform>:<id>"` biçimiyse kayıt anahtarı olur; yalnızca platform adıysa o platformun `editions_of` sırasındaki ilk kaydı olur.
    - `editions` ve `default_entry` doldurulur.
  - `Db::set_preferred(&self, game_id: i64, key: &EntryKey) -> Result<()>` değeri `"<platform>:<id>"` biçiminde yazar.
  - `replace_entries`, `games.title`'a `display_base(<bugünkü seçimle seçilen kayıt başlığı>)` yazar.
  - Tauri komutları:
    - `launch(app, db, game_id: i64, platform: Option<Platform>, platform_id: Option<String>)` → `want`: ikisi de varsa `(platform, id)`, yalnızca platform varsa o platformun `editions_of` sırasındaki ilk kaydı.
    - `set_preferred(db, game_id: i64, platform: Platform, platform_id: Option<String>)` → id yoksa aynı kuralla çözülür.

- [ ] **Step 1: Testleri yaz** (`library.rs` / `db.rs`):

```rust
#[test] fn editions_order_and_copy() { /* bioshock family entries → groups ["Remastered","Orijinal"]; Remastered entries order steam, epic, gog, gog(copy Prime) */ }
#[test] fn default_prefers_installed_last_played() { /* two installed, last_played 5 and 9 → the 9 one */ }
#[test] fn default_uninstalled_prefers_non_original() { /* none installed → first Remastered entry (steam) */ }
#[test] fn legacy_preferred_resolves() { /* db: set games.preferred='steam' raw, bioshock family → preferred == Some((Steam,"409710")) (Remastered first) */ }
#[test] fn set_preferred_roundtrip() { /* set_preferred(id,&(Gog,"1157510446".into())) → games()[..].preferred == that key, default_entry == that key */ }
#[test] fn card_title_is_base() { /* after scan_all of bioshock family, the game title == "BioShock" */ }
```

Mevcut `resolve_*` ve `preferred` testleri yeni imzaya uyarlanır.
- [ ] **Step 2:** FAIL beklenir.
- [ ] **Step 3:** Uygula. Ardından `commands.rs`'teki `launch` ve `set_preferred` güncellenir; `main.rs` değişmez.
- [ ] **Step 4:** `cargo test -p hangar-core`, `cargo check -p hangar` → PASS.
- [ ] **Step 5:** Commit at: `feat(core): kayıt bazında tercih ve sürüm grupları`.

---

### Task 3: Arayüz — gruplu menü, kart başlığı

**Files:** Modify `src/lib/api.ts`, `src/lib/library.ts`, `src/components/PlayButton.tsx` (+ test), `src/components/Stage.tsx`, `src/components/GameCard.tsx`, `src/App.tsx`, ilgili testler.

**Interfaces:**
- Consumes (T2), JSON alanları:
  - `preferred: [Platform, string] | null`
  - `editions: { label: string; entries: { platform: Platform; platform_id: string; copy: string | null }[] }[]`
  - `default_entry: [Platform, string] | null`
- Produces:
  - `api.launch(gameId, platform?, platformId?)` → `invoke("launch", { gameId, platform, platformId })`
  - `api.setPreferred(gameId, platform, platformId)` → `invoke("set_preferred", { gameId, platform, platformId })`
  - `library.ts` içinde `defaultPlatform` silinir. Yerine `export function currentEntry(g: Game): PlatformEntry` gelir: `default_entry` ile eşleşen kayıt, yoksa `g.entries[0]`.
- **`PlayButton`:**
  - Ana düğme metni: `{Oyna|Kur} · {PLATFORM_NAME}{copy ? " (" + copy + ")" : ""}`. `editions.length > 1` ise platformun önüne sürüm etiketi de eklenir: `Oyna · Remastered · Steam`.
  - Menü: `editions.length > 1` ise her grup için önce `role="presentation"` bir başlık (etiket), altında her kayıt için bir `menuitem`. Tek sürüm varsa bugünkü düz liste kalır.
  - Mevcut seçimin yanında `✓` durur.
  - Props: `onLaunch(key: [Platform, string])`, `onChoose(key: [Platform, string])`.
- **`App.tsx`:** `launch` ve `choose` artık anahtar alır, `api.launch` ve `api.setPreferred`'ı id ile çağırır.
- **`Stage`:**
  - `editions.length > 1` ise üstteki platform satırı sürümlerle yazılır: `Remastered: Steam + GOG · Orijinal: Steam`. Her sürümde platformlar tekilleştirilir.
  - Tek sürüm varsa bugünkü gibi kalır.
- **`GameCard`:** platform rozetleri tekilleştirilir; aynı platform yalnızca bir kez görünür.

- [ ] **Step 1: Testleri yaz** (`PlayButton.test.tsx`, `library.test.ts`):
  - **Gruplu menü:**
    - İki sürümlü oyun için menüde "Remastered" ve "Orijinal" başlıkları görünür.
    - `Kur · GOG (Prime)` satırı vardır.
    - Bir satıra tıklayınca `onChoose(["gog","1157510446"])` çağrılır.
    - Ana düğme `Kur · Remastered · Steam` gösterir.
  - **Tek sürüm:** düz liste ve eski metin (`Oyna · Steam`) korunur.
  - **`currentEntry`:** `default_entry` ile eşleşen kaydı döner.
  - **Kart rozetleri:** iki GOG kaydı olan bir kartta tek bir `GOG` rozeti görünür (`GameCard` testi).
- [ ] **Step 2:** `npm test` → FAIL beklenir.
- [ ] **Step 3:** Uygula. Mevcut menü CSS'i kullanılır; grup başlığı için tek bir küçük sınıf (`.play-menu .grp`) eklenebilir.
- [ ] **Step 4:** `npm test`, `npx tsc --noEmit`, `cargo check -p hangar` → PASS.
- [ ] **Step 5:** Commit at: `feat(ui): oyna menüsünde sürüm grupları`.

---

### Task 4: Elle kontrol (denetçi + kullanıcı)

- [ ] Yeniden tara. BioShock araması 3 kart göstermeli: BioShock, BioShock 2, BioShock Infinite.
- [ ] BioShock menüsü: Remastered altında Steam, Epic, GOG ve GOG (Prime); Orijinal altında Steam ve GOG.
- [ ] GOG (Prime) seçilip uygulama yeniden açıldığında seçim korunmalı.
- [ ] Kurulu bir oyunda "Oyna" eskisi gibi çalışmalı. Death's Door tek kart kalmalı.
