# Hangar — Ubisoft tam sahiplik listesi (demux) · Kısa tasarım

Tarih: 2026-11-14 · Durum: kullanıcı onayladı ("Evet, geç")
Dayanak: `2026-11-14-hangar-ubisoft-account-design.md` (bağlantı, DPAPI, güvenlik kuralları aynen geçerli).

## Sorun
GraphQL (`games(filterBy:{isOwned:true})`) yalnızca hesapta etkinleştirilmiş/oynanmış 4 oyunu döndürüyor. Çekilişle alınan AC Origins ve AC Syndicate gelmiyor.

## Canlı denemede doğrulananlar (2026-11-14, kullanıcının hesabı)
- Demux (`dmx.upc.ubisoft.com:443`, TLS 1.2, 4 bayt BE uzunluk + protobuf; YoobieRE/ubisoft-demux-node şemaları) `ownership_service` → `InitializeRsp` ile 313 kayıt döndürdü. `owned=true` ve `product_type=0` olan 7 kayıt tam olarak beklenen liste: 420 Far Cry 4, 541 Watch_Dogs, 857 AC Unity, 1875 AC Syndicate, 3539 AC Origins, 5092 AC Odyssey, 5184 AC III Remastered.
- Demux yalnızca **Ubisoft Connect PC uygulama kimliğiyle** (`f68a4bb5-608a-4ff2-8123-be8ef797e0a6`) alınmış bileti kabul ediyor; web kimliğiyle (`82b650c0-…`) alınan bilet "reddedildi (expired:false, banned:false)".
- Giriş sayfası PC kimliğini kabul etmiyor (ERROR). Ama web girişinden gelen `rememberMeTicket` ile PC kimliğinde `rm_v1` yenilemesi (`rememberMe:true`) çalışıyor ve yeni bir PC belgesi veriyor; bu belge kendi başına art arda yenilenebiliyor. GraphQL de PC kimliğiyle aynı 4 oyunu döndürüyor.
- Bu çevirme web belgesini geçersiz kılıyor (sonraki web yenilemesi 401). Önceki denemede kayıtlı bağlantının bozulma nedeni buydu.
- Sunucu TLS el sıkışmasını sık sık hemen kesiyor (UnexpectedEof); birkaç denemede bağlanıyor. `clientVersion` olarak kurulu `upc.exe` yapı numarası 13416 kabul edildi.
- Demux `product_id` = kayıt defterindeki kurulu Ubisoft kimliği (541 Watch_Dogs) ve `ubiservices_space_id` = GraphQL `spaceId`.

## Tasarım
**Kimlikler:** Giriş penceresi web kimliğiyle kalır (`UBI_APP_ID`). Tüm API çağrıları (yenileme, GraphQL, oturum kapatma) yeni sabit `UBI_LAUNCHER_APP_ID` ile yapılır.

**Bağlanma:** `connect` girişten gelen web `rememberMeTicket`'ı bir kez PC kimliğiyle yeniler. Dönen PC belgesi kaydedilir; web belgesi saklanmaz. Belge biçimi `v: 2` olur. `v: 1` dosyası okunamaz sayılır ve kullanıcı bağlı görünmez; yeniden bağlanması gerekir. Mevcut belge bu PC'de zaten geçersiz.

**Tarama (`fetch_owned`):**
1. Yenile (PC kimliği, kalıcı rotasyon kuralları aynı).
2. Demux: sahip olunan ürünler, yalnızca `owned == true && product_type == 0`.
3. GraphQL (PC kimliği, en iyi çaba): `spaceId → (name, coverUrl)` haritası.
4. Kayıt: `platform_id = product_id`, `launch_uri = install_uri = uplay://launch/<product_id>/0`.
   - Başlık: GraphQL adı (space_id ile), yoksa yapılandırma YAML'ındaki ad. Ad yerelleştirme anahtarıysa (`GAMENAME`, `l1` gibi) `localizations: default:` altındaki karşılığına bakılır. Ad çözülemezse kayıt atlanır.
   - Kapak: GraphQL `coverUrl`, yoksa YAML `thumb_image` dosya adından `https://ubistatic3-a.akamaihd.net/orbit/uplay_launcher_3_0/assets/<dosya>`. Bu host kapak izin listesine eklenir. YAML'da yoksa mevcut kapak zinciri devreye girer.
5. Demux başarısız olursa (ağ, el sıkışması, ayrıştırma) bugünkü GraphQL listesine dönülür (GUID kimlik, mevcut başlık eşleşmesi). Bu durumda uyarı gösterilmez.

**Demux istemcisi:** üretim kodu olur (`cfg(debug_assertions)` kalkar). Yalnızca okuma yapar: clientVersion, authenticate, open `ownership_service`, initialize; ardından kapanır. El sıkışması en fazla 6 kez denenir; tek bir isteğin süresi sınırlı. Bilet asla loglanmaz ya da yazılmaz. Hatalar sunucu verisi içermez.

**Kurulu kayıtla eşleşme:** kimlikler artık eşleşiyor, bu yüzden `scan_all` kimlikle birleştirir. Yalnızca Ubisoft için olan başlık yedeği zararsız; geri dönüş yolu için kalır.

**Arayüz:** değişiklik yok. Beklenen kartlar:
- Syndicate ve Origins yeni Ubisoft kartları olur.
- AC III Remastered, Steam kartına Ubisoft rozetiyle katılır; Watch_Dogs gibi.
- Diğer kartlar aynı kalır.

**Temizlik:** deneme komutları (`debug_ubisoft_demux`'un adım zinciri, `ubi_probe::launcher_session`/`graphql_app`) kaldırılır. Bunun yerine `hangarUbiDemux()` kayıtlı bağlantıyla özet veren ince bir debug komutu olur.

**README:** Ubisoft bölümüne şunlar eklenir:
- Ubisoft Connect'in iç protokolü (demux) kullanılıyor.
- Belge Ubisoft Connect PC uygulamasının kimliğiyle saklanıyor; Ubisoft hesap sayfasında öyle görünebilir.
- Bu yol resmi değil, değişebilir.

## Hedef dışı
- DLC/eklenti kartları.
- Ubisoft+ aboneliği.
- `claimed_games` (bu hesapta boş).
- Eski şifresiz önbellek.

## Test
**Çekirdek** (sahte API):
- connect PC belgesini saklar (v2), v1 okunmaz
- demux listesi + GraphQL haritası → başlık/kapak/kimlik
- yerelleştirme anahtarı çözümü
- tür/sahiplik filtresi
- demux hatasında GraphQL'e dönüş
- rotasyon ve sızıntı testleri aynen

**Canlı:**
1. Yeniden bağlan.
2. Kuru çalıştırmada 7 Ubisoft kaydı; Syndicate ve Origins kart, AC III Remastered Steam kartında.
3. Origins "Kur" Ubisoft Connect'i açar.
