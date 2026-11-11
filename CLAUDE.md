# Hangar — proje hafızası

Steam, Epic ve Ubisoft oyunlarını tek kütüphanede birleştiren Windows masaüstü uygulaması. Kullanıcı: kullanıcı (Türkçe konuşur, Windows 11 Türkçe sistem, kullanıcı klasörü `C:\Users\user`, repo `C:\Users\user\workdir\hangar`).

## Durum (2026-11-09, ilk gerçek çalıştırma sonrası)
- v1 kodu `feat/v1` dalında (GitHub: <owner>/hangar). `main` sadece tasarım+plan belgeleri.
- `npm run tauri dev` Windows'ta açıldı: Steam (DAVE THE DIVER, Watch_Dogs), Epic (Death's Door), Ubisoft (Watch_Dogs → Steam ile birleşti) doğru geldi, kapaklar indi.
- Düzeltildi: Ayarlar'a SteamID yerine kullanıcı adı (`player`) girilince GetOwnedGames sessizce başarısızdı → `steam::is_steamid64`; 17 haneli değilse `loginusers.vdf`'ten alınır.
- Not: `localhost:1420` tarayıcıda çalışmaz (Tauri `invoke` yok) — yalnızca Tauri penceresinde test edilir.

## Sıradaki görevler (sırayla, adım adım)
1. ✅ (2026-11-11) Kurulu olmayan Steam oyunları API ile geliyor.
2. ✅ (2026-11-11) Oyna: Steam, Epic, Ubisoft hepsi açıldı. (Ubisoft Connect bir kez "initializing"de takıldı — Connect kaynaklı; Hangar her tıkta `uplay://` açar, kilit yok ama geri bildirim de yok.)
3. ✅ (2026-11-11) Uyarılar üstteki `notice` satırında görünüyor; Ayarlar'dan taramanın uyarıları kayboluyordu → diyalog artık App'in `rescan`'ini çağırıyor.
4. Her şey çalışınca `feat/v1` → `main` PR.
5. Sonra "Bilinen küçük açıklar" listesi.

## v1.1 fikirleri (kullanıcıdan, 2026-11-11 — henüz tasarlanmadı, önce brainstorming)
- "Oyna" sonrası geri bildirim/durum (ör. "Ubisoft Connect'e gönderildi", tekrar dene).
- Yeni sağlayıcı: kendi istemcisi olan oyunlar, ilk örnek Riot/League of Legends (`C:\ProgramData\Riot Games\` meta dosyaları).
- Tanınmayan oyunlar için elle ekleme (exe yolu + ad).
- NVIDIA app'in oyun taramasını örnek almak (rea-skill) — yerel dosyalardan okumak kırılgan; önce kendi sağlayıcılarımız.

## Belgeler
- Tasarım (spec): `docs/superpowers/specs/2026-11-09-hangar-design.md`
- Plan (14 görev, tamamlandı): `docs/superpowers/plans/2026-11-09-hangar-v1.md`
- Seçilen arayüz yönü: "5 · Kokpit + Vitrin" — referans mockup `docs/design/mockups-melez.html` (ilk sekme).

## Mimari
- `crates/hangar-core` — saf Rust, Tauri'siz, Linux'ta test edilir: `vdf.rs`, `providers/{steam,epic,ubisoft}.rs` (`LibraryProvider`), `merge.rs` (ad normalizasyonu + birleştirme/ayırma override'ları), `db.rs` (SQLite), `library.rs` (tarama, `resolve`), `artwork.rs` (kapaklar).
- `src-tauri` — ince katman: `commands.rs` (get_library, rescan, launch, set_preferred, get/save_settings, split_game, merge_games; olaylar `library-updated`, `cover-ready`).
- `src/` — React 18 + TS + Vite; GSAP, Lenis, @tanstack/react-virtual. `lib/library.ts` içindeki `normalize` Rust `merge::normalize` ile birebir aynı olmalı.
- Veri: `%APPDATA%\com.hangar.app\hangar.db`, kapaklar `...\covers`.

## Komutlar
- Linux/WSL'de: `cargo test -p hangar-core`, `cargo check -p hangar` (GTK/WebKit dev paketleri gerekir), `npm test`, `npx tsc --noEmit`, `npm run build`.
- Windows'ta: `npm install`, `npm run tauri dev`, `npm run tauri build` → `src-tauri/target/release/bundle/nsis/Hangar_0.1.0_x64-setup.exe` (not: workspace olduğu için `target/` repo kökünde).

## Kurallar (kullanıcının)
- Önce plan/tasarım, sonra kod (superpowers: brainstorming → writing-plans → subagent-driven-development).
- Karpathy kuralları (`karpathy-guidelines` skill): istenmeyen özellik yok, cerrahi değişiklik, doğrulanabilir hedef.
- Alt ajanlar: Sonnet, aynı anda en fazla 2-3.
- UI işleri için skill'ler: taste-skill, unslop, ui-ux-pro-max, gsap-*; hepsi `~/.claude/skills` altında kurulu (Windows ve WSL).
- Commit mesajları Türkçe conventional (`feat(core): ...`).

## Bilinen küçük açıklar (v1 sonrası)
Sağ tık menüsü pencere kenarına taşabiliyor; ayarlar penceresinde tam odak tuzağı yok; README'de SteamID notu eksik; Ayarlar'da getSettings iki kez başarısız olursa geri bildirim yok; npm audit 6 açık (hepsi geliştirme araçlarında: vite/esbuild/jsdom zinciri, yerel kullanımda risk yok).
