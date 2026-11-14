# Hangar — Ubisoft Connect hesap bağlantısı · Kısa tasarım

Tarih: 2026-11-14 · Durum: onay bekliyor
Dayanaklar:
- Epic tasarımı (`2026-11-12-hangar-accounts-design.md`) — aynı kalıp, aynı güvenlik kuralları.
- U0 keşfi (2026-11-14, kullanıcının gerçek hesabıyla).

## Niyet
Ubisoft hesabındaki tüm PC oyunları (kurulu olmayanlar dahil) Hangar'a gelsin. Bağlantı Epic'teki gibi kurulur:
- Ayarlar › Hesaplar'dan, Ubisoft'un kendi giriş sayfasıyla.
- Şifre Hangar'a gelmez.
- Belge yalnızca bu PC'de, DPAPI ile şifreli durur.

## U0'da doğrulananlar
- **Giriş:** `connect.ubisoft.com/login?appId=82b650c0-…&genomeId=42d07c95-…` gömülü pencerede çalışıyor. Yalnızca connect.ubisoft.com ziyaret edildi.
- **Oturum değerleri:** girişten sonra `localStorage`'da `PRODloginData` (ticket, sessionId, userId) ve `PRODrememberMe` (rememberMeTicket) bulunuyor. Başlatma betiği bunları nonce'lu geri dönüşle alıyor.
- **Yenileme:** `POST https://public-ubiservices.ubi.com/v3/profiles/sessions` çağrısı `Authorization: rm_v1 t=<rememberMeTicket>` ve gövde `{"rememberMe":true}` ile HTTP 200 dönüyor.
- **Sahip olunan oyunlar:** `POST …/v1/profiles/me/uplay/graphql` (AllGames sorgusu) 4 PC oyunu döndü.
  - Her oyunda `id` ve `spaceId` (GUID), `name` ve `coverUrl` alanları var.
  - Launcher'ın sayısal kimliği gelmiyor. Yerel önbellek (`configurations.vdat`) şifreli. Yapı sorgusu (introspection) kapalı.
- **Kur bağlantısı:** `uplay://launch/<spaceId>/0` Ubisoft Connect'i açıyor. Launcher "Initializing" ekranında kaldığı için oyunun sayfasına gidip gitmediği doğrulanamadı.

## Tasarım (Epic'le aynı yapı)
**Belge**
- `SecretStore` içinde `Platform::Ubisoft` dosyası: `accounts\ubisoft.dat`.
- İçerik `{v, remember_me_ticket, user_id, display_name}`.
- Kısa ömürlü ticket ve sessionId yalnızca bellekte tutulur.
- Dönen belge kullanılmadan önce kaydedilir; kaydedilemezse bellekte tutulur. Epic'teki `pending` kuralının aynısı.

**Tarama**
- Her taramada rm_v1 yenilemesi yapılır, ardından GraphQL AllGames çağrılır.
- Yalnızca PC sahipliği olan oyunlar alınır.
- Her oyundan bir kayıt üretilir:
  - `platform: Ubisoft`, `platform_id: spaceId`, `title: name`, `installed: false`
  - `launch_uri = install_uri = uplay://launch/<spaceId>/0`
  - `cover_url = coverUrl`; kapak izin listesine `ubiservices.cdn.ubi.com` eklenir.
- Önbellek ve "süresi doldu" davranışı Epic'le aynıdır.

**Kurulu kayıtla eşleşme**
- Kayıt defteri sayısal kimlik veriyor, GraphQL GUID; bu yüzden kimlikler eşleşmez.
- Kural: aynı platformda, kimlik eşleşmiyor ama `normalize(başlık)` kurulu bir kayıtla aynıysa, sahip olunan kayıt kurulu kayda katılır. Kurulu kayıt kazanır; sahip olunanın kapağı alınır.
- Böylece aynı oyun "Ubisoft · 2. kopya" olarak görünmez.

**Giriş penceresi**
- U0'daki `UBI_NAV_RULES` ve `ubi_init_script` aynen kullanılır.
- Uygulama kimliği tek bir sabitte durur (Ubisoft bu kimliği değiştirebiliyor).

**Arayüz**
- Ayarlar › Hesaplar'a "Ubisoft Connect" satırı eklenir: Bağlan / Bağlantıyı kes / Yeniden bağlan.
- Metinler Epic satırıyla aynı kalıpta.

**Bağlantıyı kes**
- Önce sunucuda oturum kapatma denenir: `DELETE https://public-ubiservices.ubi.com/v3/profiles/sessions`. Başarısız olursa yok sayılır.
- Ardından belge ve önbellek silinir.

**README:** "Hesap bağlantıları"na Ubisoft eklenir. Notlar: resmi olmayan uçlar, uygulama kimliği değişebilir, belge hesabın tamamına erişir.

## Hedef dışı
- Ubisoft+ abonelik kütüphanesi.
- LUNA (bulut) sahiplikleri.
- Launcher'ın sayısal kimliğini bulmak (önbellek şifreli).

## Test ve doğrulama
**Çekirdek** (sahte API ile):
- bağlanma, belge dönüşü, kaydedilemeyen belgenin bellekte tutulması
- çevrimdışıyken önbellek
- `invalid_grant` → süresi doldu
- kesme sırasında süren taramanın geri yazamaması
- PC filtresi
- başlıkla kurulu kayıtla eşleşme
- sızıntı testi

**Canlı:** bağlan → 4 oyun → Watch_Dogs tek kart (Steam + Ubisoft) → Far Cry 4 "Kur" Ubisoft Connect'i açar → kes.
