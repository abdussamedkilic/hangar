# Hangar — proje hafızası

Steam, Epic ve Ubisoft oyunlarını tek kütüphanede birleştiren Windows masaüstü uygulaması. Kullanıcı: kullanıcı (Türkçe konuşur, Windows 11 Türkçe sistem, kullanıcı klasörü `C:\Users\user`, repo `C:\Users\user\workdir\hangar`).

## Durum (2026-11-11)
- v1 `main`'de (PR #1 squash ile merge edildi, 2026-11-11; GitHub: <owner>/hangar). Yeni işler `main`'den yeni dalda.
- `npm run tauri dev` Windows'ta açıldı: Steam (DAVE THE DIVER, Watch_Dogs), Epic (Death's Door), Ubisoft (Watch_Dogs → Steam ile birleşti) doğru geldi, kapaklar indi.
- Düzeltildi: Ayarlar'a SteamID yerine kullanıcı adı (`player`) girilince GetOwnedGames sessizce başarısızdı → `steam::is_steamid64`; 17 haneli değilse `loginusers.vdf`'ten alınır.
- 2026-11-11: `feat/step1` dalında Riot/klasör taraması/elle ekleme/GOG eklendi (GOG doğrulanmadı); PR kullanıcı kontrolünden sonra.
- Not: `localhost:1420` tarayıcıda çalışmaz (Tauri `invoke` yok) — yalnızca Tauri penceresinde test edilir.

## Sıradaki görevler (sırayla, adım adım)
1. ✅ (2026-11-11) Kurulu olmayan Steam oyunları API ile geliyor.
2. ✅ (2026-11-11) Oyna: Steam, Epic, Ubisoft hepsi açıldı. (Ubisoft Connect bir kez "initializing"de takıldı — Connect kaynaklı; Hangar her tıkta `uplay://` açar, kilit yok ama geri bildirim de yok.)
3. ✅ (2026-11-11) Uyarılar üstteki `notice` satırında görünüyor; Ayarlar'dan taramanın uyarıları kayboluyordu → diyalog artık App'in `rescan`'ini çağırıyor.
4. ✅ (2026-11-11) PR #1 squash ile `main`'e alındı.
5. ✅ (2026-11-11) Adım 1 (`feat/step1`): Oyun ekle (elle ekleme), Riot sağlayıcısı (League of Legends, TFT), seçili klasörlerde oyun önerisi taraması, GOG sağlayıcısı. Kullanıcının elle kontrolü bekliyor; sonra `main`'e PR.
   - GOG **doğrulanmadı** — gerçek bir GOG oyunuyla denenmedi (bu PC'de Galaxy var, oyun yok; kayıt defteri `GOG.com\Games`).
   - Kullanıcı denedi (2026-11-11): Riot/LoL açılıyor, TFT bulunuyor, Watch_Dogs (Steam) Ubisoft Connect üzerinden açılıyor (normal).
   - Ek (aynı dal): GOG Galaxy yerel DB'sinden (`C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db`, salt okuma) kurulu olmayan GOG oyunları (bu PC'de 161); karttan "Kapak seç…"; oyunlar artık Hangar'ın alt süreci olarak başlamıyor (PowerShell `Start-Process`, değerler env ile) — `tauri dev` yeniden başlatması LoL'u öldürüyordu.
   - Riot ikonundan kapak kodu var ama bu PC'deki LoL `.ico` PNG çerçevesi içermiyor → kapak "Kapak seç…" ile elle.
   - Epic'te kurulu olmayan oyunlar yok: güvenilir yerel liste yok, hesap girişi gerekiyor (ayrı tasarım).
   - ✅ (2026-11-11) PR #2 squash ile `main`'e alındı. Kullanıcı GOG'u denedi: kurulu olmayan oyunda "Kur" Galaxy'de kurulumu açıyor.
6. **Sıradaki (2026-11-11, kullanıcı istedi):** (a) Epic'te kurulu olmayan oyunlar (yerel güvenilir liste yok; hesap girişi ya da başka kaynak — önce tasarım), (b) Ubisoft Connect'te sahip olunan oyunların tamamı (şu an yalnızca kurulu olanlar, kayıt defterinden). Önce brainstorming + karar, sonra plan.
   - **Karar (kullanıcı, 2026-11-12): Galaxy YOK. Playnite yolu — uygulama içi bağlantı, veriler yalnızca kullanıcının bilgisayarında kalır.** (Council Galaxy'yi önermişti; kullanıcı reddetti.)
     - Epic: Hangar ayrı pencerede Epic'in kendi giriş sayfasını açar (Hangar şifreyi görmez), yetki kodu → OAuth (Legendary/Heroic/Playnite akışı, launcher'ın açık client id'si), sahip olunan oyunlar Epic kütüphane + katalog servisinden. Yenileme belgesi yalnızca yerelde, Windows DPAPI ile şifreli; "Bağlantıyı kes" siler; asla loglanmaz; sunucu/telemetri yok.
     - Ubisoft: önce kısa, zaman sınırlı deneme (şifreli yerel önbellek vs Ubisoft'un kendi giriş sayfası + ticket); çalışan yol bulunamazsa yalnızca kurulu oyunlar kalır.
     - Ayarlar'da "Hesaplar" bölümü; README'de güvenlik notu (belge nerede, nasıl silinir, resmi olmayan servis riski).
     - Sıradaki oturum: brainstorming → kısa tasarım belgesi → plan → subagent-driven (Sonnet). Riskler council'dan: Epic client id'yi engelleyebilir, belge hesabın tamamına erişir, aynı kullanıcıdaki zararlı yazılıma karşı DPAPI korumaz.
   - ✅ (2026-11-13) **Epic yapıldı** — `feat/epic-account` dalı (spec `docs/superpowers/specs/2026-11-12-hangar-accounts-design.md`, plan `docs/superpowers/plans/2026-11-12-hangar-epic-account.md`). Tasarım ve plan konseyden geçti; tüm görevler incelendi; son tüm-dal incelemesi temiz. Kullanıcının elle kontrolü bekliyor, sonra `main`'e PR.
     - E0 (canlı giriş, 2026-11-12): e-posta ile giriş çalışıyor, yalnızca www.epicgames.com ziyaret ediliyor; 240 kayıt (225 oyun, 15 DLC). Gotcha: katalog bulk ucu virgülle birleştirilmiş kimlikte `{}` döner — her kimlik ayrı `id=` parametresi. Yenileme belgesi her kullanımda döner, ~1 yıl geçerli.
     - İzin listesi yalnızca epicgames.com + captcha alanları: Google/Apple vb. ile giriş bilerek desteklenmiyor.
     - Debug'da `await hangarProbe()` (DevTools) temizlenmiş canlı yanıtları `%APPDATA%\com.hangar.app\e0\` altına yazar; depoya girmez. Yerel canlı test: `HANGAR_EPIC_LIVE_DIR="$APPDATA/com.hangar.app/e0" cargo test -p hangar-core live_ -- --ignored`.
   - **Ubisoft sırada:** U0 keşfi (connect.ubisoft.com ticket, `rm_v1` yenileme, sahip olunanlar + kayıt defteriyle eşleşen ürün kimliği), kullanıcının girişiyle; başarılıysa ortak altyapı (giriş penceresi, `SecretStore`, `OwnedLists`, Hesaplar) üzerine kısa ayrı plan.
   - Not (2026-11-13): PC 30 günde 14 kez BSOD ile yeniden başladı (çoğu 0x1a MEMORY_MANAGEMENT; olası RAM/platform). Bir çökme git ref'ini ve index'i sıfırla doldurdu — onarıldı. Uzun işlerden önce `git fsck` iyi fikir.
7. Sonra "Bilinen küçük açıklar" listesi.

## v1.1 fikirleri (kullanıcıdan, 2026-11-11 — henüz tasarlanmadı, önce brainstorming)
- "Oyna" sonrası geri bildirim/durum (ör. "Ubisoft Connect'e gönderildi", tekrar dene).
- (Yapıldı: Riot sağlayıcısı, elle ekleme, GOG — yukarıda adım 5.)
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

## Bilinen küçük açıklar (Step 1 incelemelerinden, ertelendi)
UAC iptali dışında yükseltilmiş başlatma hataları sınırlı bildiriliyor; başlatma betiği `"`'den önceki `\`'yi çiftlemiyor (bugün argümanlar bunu içermiyor); "Kaldır" onay sormuyor; kenar çubuğunda aktif filtrenin satırı sayı 0 olunca kayboluyor; tarama hariç tutması `*crash*` "Crash Bandicoot"u da eler, `unins` önek değil içerir eşleşmesi; bozuk `scan_folders` JSON sessizce boş liste; Riot PBE fixture'ı kullanılmıyor; boş durum metni yeni kaynakları ve "Oyun ekle"yi anmıyor; eksik exe'li yerel kart "Kur · Yerel" diyor; `tauri dev` açıkken Rust düzenlemek uygulamayı yeniden başlatır (oyunlar artık etkilenmez).

## Bilinen küçük açıklar (v1 sonrası)
Sağ tık menüsü pencere kenarına taşabiliyor; ayarlar penceresinde tam odak tuzağı yok; README'de SteamID notu eksik; Ayarlar'da getSettings iki kez başarısız olursa geri bildirim yok; npm audit 6 açık (hepsi geliştirme araçlarında: vite/esbuild/jsdom zinciri, yerel kullanımda risk yok).
