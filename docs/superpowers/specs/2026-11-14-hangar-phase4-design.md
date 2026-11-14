# Hangar Faz 4 — Steam/GOG girişi, filtreler, daraltılabilir menü, sürükle-bırak, bölümlü Ayarlar · Tasarım

Tarih: 2026-11-14 · Durum: kullanıcı yaklaşımları onayladı ("a ile devam", "diğer söylediklerin iyi", GOG girişi eklendi); S0/G0 canlı denemeleri başarılı
Dal: `feat/phase4` (main = Faz 3, PR #6)

## Niyet
Kullanıcı projeyi bitirmek istiyor. Hangar dört istemciyi (Steam, Epic, GOG, Ubisoft) "native" desteklemeli; bunların hepsi kullanıcının kullandıkları ("yeterli olması lazım"). Arayüz tarafı da son halini almalı:
- elle filtreler,
- daralabilen sol menü,
- kartları sürükleyerek koleksiyona ekleme,
- bölümlere ayrılmış Ayarlar.

## Hedefler
1. **Steam girişi** (kullanıcı: a seçeneği). Varsayılan bağlanma yolu Steam'in kendi giriş sayfasıdır.
   - Hangar SteamID'yi ve sahip olunan oyunları API anahtarı olmadan alır.
   - API anahtarı "Gelişmiş" altında isteğe bağlı kalır. Anahtar girilirse bugünkü GetOwnedGames yolu kullanılır.
   - Hangar'ın kendi anahtarını dağıtmak hedef dışı (Steam koşulları, ortak kota).
2. **GOG girişi.** GOG için kaynak seçimi:
   - "Galaxy veritabanından bul" (bugünkü yol) ya da "Hesapla bağlan" (Galaxy istemcisinin açık OAuth akışı).
   - Kullanıcı: "tik ile ister db'den bulsun ister giriş yapsın".
   - Önerim: iki seçenek tek bir seçim kutusu olur, varsayılan Galaxy veritabanıdır.
3. **Filtreler.**
   - Kenar çubuğunda "Filtreler +" başlığı olur. "+" adlandırılmış bir filtre oluşturur.
   - Ölçütler:
     - mağaza (bir veya birkaçı)
     - durum: kurulu / kurulu değil
     - oynanma: hiç oynanmamış / oynanmış
     - sürüm: Remastered/Definitive vb. / yalnız orijinal
     - koleksiyon: içinde / dışında
   - Tıklanan filtre açık sekmenin üstüne uygulanır. Aynı anda tek filtre etkin olur, tekrar tıklayınca kapanır.
   - "Kurulu" hazır (silinemez) bir filtreye dönüşür.
   - Düzenleme ve silme sağ tıkla yapılır.
   - Bugünkü `ui.installed` tercihi, "Kurulu" filtresinin etkin olmasına göç eder.
4. **Daraltılabilir menü.**
   - "«" ile menü yalnız ikonlardan oluşan bir şeride dönüşür (~64px). İkonlar ipucunda adını gösterir.
   - Filtreler ikon ya da baş harf olarak görünür.
   - Durum `ui.sidebar = "open"|"collapsed"` olarak kalıcıdır.
   - Daraltılmış hal design-taste-frontend kurallarıyla, gerçek pencerenin ekran görüntüsüyle denetlenir.
5. **Sürükle-bırak.**
   - Kart ya da liste satırı bir koleksiyon sekmesine bırakılınca kartın tüm kayıtları o koleksiyona eklenir (Faz 3 üyelik kuralı).
   - Sürüklenirken bırakılabilecek sekmeler vurgulanır.
   - Bırakma sonrası kısa bir durum metni gösterilir ("AC'ye eklendi").
   - Klavye ve erişilebilirlik için sağ tık menüsü yolu kalır.
6. **Ayarlar.** Pencerenin solunda iki sekme olur:
   - **Hesaplar:** Steam, Epic, Ubisoft ve GOG satırları, aynı satır kalıbıyla. Steam satırında "Gelişmiş: API anahtarı, SteamID", GOG satırında kaynak seçimi bulunur.
   - **Özelleştir:** tema ("Tema dosyasını aç" / "Varsayılana dön"), menü (açık/daraltılmış varsayılanı), oyun klasörleri.
   - Önerim: "Profil" yerine "Özelleştir". Profil kimlik bilgisini çağrıştırıyor, buradaki içerik ise kişiselleştirme.

## Hedef dışı
- Amazon Games.
- Steam aile paylaşımı.
- GOG Galaxy entegrasyonları (Galaxy'nin diğer platformları).
- Filtrelerde VE/VEYA ağacı: ölçütler her zaman VE ile birleşir, mağaza ölçütü içinde VEYA.
- Birden çok filtreyi aynı anda açmak.
- Koleksiyon sekmelerini sürükleyerek sıralamak.

## Anti-hedefler
- Giriş bilgisinin (çerez, token, SteamID) loglanması ya da düz metin yazılması. Saklanan her şey DPAPI ile şifrelenir (Epic/Ubisoft kalıbı).
- Steam girişi bozulunca kütüphanenin boşalması. Bu durumda önbelleğe ve kurulu oyunlara düşülür, uyarı Epic/Ubisoft'taki gibi gösterilir.
- Daraltılmış menüde işlevin kaybolması: her düğmeye ikonla ulaşılabilmeli.
- Sürükle-bırakın tıklama/seçim davranışını bozması.

## Kısıtlar
- Mevcut hesap altyapısı aynen kullanılır:
  - `login_flow` (nonce'lu geri dönüş, gezinme kuralları)
  - `SecretStore`/DPAPI
  - `owned_cache` ve "süresi doldu" kuralları
  - `AccountsSection` satır kalıbı
  - mevcut hesap komutlarında `Platform` dağıtımı
- Ön yüz Faz 3 yapısı üzerine kurulur:
  - `Tab` + filtre modeli (`library.ts`); `installedOnly` genelleşip etkin filtreye dönüşür
  - `TabStrip`, `Sidebar`, `ContextMenu`
  - `ui.*` tercihleri (`set_ui_pref` beyaz listesine `ui.sidebar` ve `ui.filter` eklenir)
- Renkler yalnız token. Yeni metinler Türkçe.

## Yaklaşımlar
- **Steam:**
  - Seçilen: (1) Gömülü pencerede Steam girişi; sahip olunanlar `store.steampowered.com/dynamicstore/userdata/`'dan. Yöntem S0 denemesiyle kesinleşir.
  - Elenen: (2) Steam OpenID — yalnız SteamID verir, liste için yine anahtar gerekir.
  - Elenen: (3) Gömülü ortak anahtar — Steam koşullarına aykırı.
- **GOG:**
  - Seçilen: (1) Galaxy açık OAuth istemcisi (Heroic/Playnite/lgogdownloader yolu); `embed.gog.com/user/data/games`.
  - Elenen: (2) Yalnız Galaxy DB — bugünkü yol; giriş istenmiş.
- **Filtrelerin saklanması:**
  - Seçilen: (1) Veritabanında `filters(id, name, rule_json, position)`. Kural JSON'u ön yüzde değerlendirilir; `view` saf fonksiyonu genişler.
  - Elenen: (2) Ayar anahtarında JSON dizisi — sayaçlar/silme/yeniden adlandırma koleksiyonlarla aynı kalıpta olsun diye tablo seçildi.
- **Sürükle-bırak:**
  - Seçilen: (1) Yerel HTML5 sürükle-bırak (`draggable`, `dataTransfer` içinde kart kimliği).
  - Elenen: (2) Kütüphane (dnd-kit) — tek hedef türü için gereksiz bağımlılık.

## Kurucuya kalanlar
- İkon seti: mevcut projede ne varsa (yoksa inline SVG).
- Filtre düzenleyicisinin (Modal) yerleşimi.
- Sürükleme önizleme görseli.
- GOG kaynağı "Hesapla" seçildiğinde Galaxy DB'den gelen kurulu bilgisiyle birleşme: kimlikler aynı GOG ürün kimliği olduğu için doğal birleşme beklenir; G0 sonrası doğrulanır.

## Denemelerle kesinleşen tasarım (S0/G0, 2026-11-14 canlı)

### Steam
- **Doğrulananlar** (S0):
  - Mağaza giriş sayfasında QR ile giriş çalıştı; Steam'e ayrılmış kalıcı WebView2 profili (`app_data_dir/webview-steam`, gizli değil) sayesinde ikinci çalıştırma girişsiz oldu (1 sn).
  - Görünmez pencere (`visible(false)`) de girişsiz sonuç verdi.
  - Sayfa betiği `#application_config[data-loyalty_webapi_token]`'daki geçici erişim anahtarıyla aynı sayfadan `IPlayerService/GetOwnedGames/v1` çağırdı: 243 oyun, hepsi adlı, 68'i oynanmış. Bu liste, userdata'daki 724 kimliğin oyun olan alt kümesi; geri kalan 481'i DLC/araç vb.
  - Oturum çerezleri (`steamLoginSecure`, `steamRefresh_steam`) yaklaşık bir yıl geçerli ve httpOnly; Hangar onlara hiç dokunmaz.
- **Bağlan:** Hesaplar'da "Bağlan", Steam penceresini kalıcı profille görünür açar. Kullanıcı Steam'in sayfasında girer (QR/şifre/Steam Guard). Betik `{steamid, games}` teslim eder; erişim anahtarı asla dışarı çıkmaz.
- **Saklama:** DPAPI dosyası `accounts\steam.dat` yalnızca `{v:1, steamid, display_name}` tutar. Oturumun kendisi WebView2 profilinde kalır (Chromium çerezleri Windows'ta DPAPI ile korunur).
- **Tarama:**
  - Steam bağlıysa ve API anahtarı yoksa, görünmez pencere kalıcı profille açılır ve en fazla 20 sn içinde listeyi teslim eder.
  - Liste mevcut `steam::parse_owned_games` biçimine (`response.games[]`) çevrilip aynı yoldan kayıtlara dönüşür.
  - Oturum düşmüşse hesap "süresi doldu" olur, önbellek gösterilir ve uyarı Epic/Ubisoft kalıbıyla çıkar.
  - Aynı anda tek bir Steam penceresi açılır.
- **API anahtarı:** Hesaplar › Steam › "Gelişmiş" altında (anahtar + SteamID). Anahtar girilirse bugünkü yol kullanılır ve giriş penceresi açılmaz.
- **Bağlantıyı kes:** Pencere kapalıyken `webview-steam` klasörü ve `steam.dat` silinir, Steam önbelleği temizlenir. Sunucuda oturum kapatma denenmez; yerel profil silinince oturum Hangar'da biter, README'de not düşülür.
- **Gezinme kuralları:** store/login/help/checkout.steampowered.com, steamcommunity.com, captcha hostları (S0'daki liste).

### GOG
- **Doğrulananlar** (G0):
  - Galaxy açık OAuth istemcisiyle giriş ve token takası çalıştı (`expires_in` 3600).
  - `getFilteredProducts?mediaType=1` sayfalı olarak 82 oyunu başlığıyla verdi.
  - `user/data/games` 163 kimlik döndü (DLC dahil).
  - Yenileme HTTP 200 döndü; refresh token değişmiyor.
  - Ziyaret edilen hostlar: auth/login/embed.gog.com.
- **Kaynak seçimi:** Hesaplar › GOG satırında "Kaynak: Galaxy veritabanı / GOG hesabı" seçimi; `settings.gog.source = "galaxy"|"account"`, varsayılan `galaxy`.
  - `galaxy`: bugünkü davranış aynen.
  - `account`: sahip olunan liste hesaptan gelir. Galaxy DB yalnız kurulu oyunlar için okunmaya devam eder.
  - Hesaptan gelen kayıtlar Galaxy DB'deki sahip olunan kayıtlarla aynı `platform_id`'yi (ürün kimliği) taşır, bu yüzden kendiliğinden birleşir.
  - Kayıt biçimi: `goggalaxy://openGameView/<id>`. Kapak `getFilteredProducts` ürünündeki `image` alanından gelir: örnek `//images-3.gog-statics.com/<hash>` → `https:` + değer + `_392.jpg` (kurucu en uygun dikey boyutu doğrular). Kapak izin listesi `images-<n>.gog-statics.com` alt alan adlarını da kabul etmelidir; bugün yalnız `images.gog-statics.com` var.
- **İstemci:** `GogClient` (Epic kalıbı): `connect(code)`, `owned_for_scan`, `disconnect`, `status`. DPAPI `accounts\gog.dat` = `{v:1, refresh_token, user_id, display_name}`. Erişim token'ı yalnız bellekte tutulur.

## Kurucu kontrolünden sonra netleşenler (Opus kararları)
1. **"Oynanmış":** Kartın herhangi bir kaydında `playtime_min > 0` ya da `last_played` varsa, veya kartın `last_launched` değeri doluysa kart oynanmış sayılır. Steam (anahtarlı da, girişli de) süreyi veriyor. Epic, GOG ve Ubisoft'ta yalnız Hangar'dan başlatma sayılır. Canlı test 3 bu yüzden "Hiç oynanmamış Steam" olarak değişti. Filtre düzenleyicisinde kısa not: "Oynama süresi yalnız Steam'de var; diğerlerinde Hangar'dan başlatılanlar sayılır."
2. **Steam taraması:**
   - Görünmez pencere taramanın parçasıdır ve en fazla 20 sn sürer. Ekran bu sırada veritabanındaki listeyi göstermeye devam eder.
   - Betik açık bir "giriş yok" sinyali verirse hesap "süresi doldu" olur.
   - Zaman aşımı ya da ağ hatası olursa yalnız "güncellenemedi" uyarısı ve önbellek gelir; süresi doldu sayılmaz.
   - Odak tetikli otomatik taramalarda Steam penceresi en fazla 10 dakikada bir açılır. "Yenile" düğmesi bu sınırı atlar.
   - API anahtarı girilmişse her zaman anahtar yolu kullanılır.
3. **SteamID:**
   - Girişte sayfadan alınır (`application_config` / `g_steamID`) ve `steam.dat`'ta durur.
   - "Gelişmiş" altındaki elle SteamID yalnız anahtar yolunda kullanılır; `loginusers.vdf` yedeği de yalnız orada kalır.
4. **GOG hesap kaynağı:**
   - Doğruluk kaynağı `getFilteredProducts?mediaType=1` (sayfalı). `user/data/games` kullanılmaz.
   - `account` modunda Galaxy DB'nin kurulu olmayan kayıtları atılır; kurulu kayıtlar kalır.
   - Hesap düşerse Epic kalıbı uygulanır: önbellek + uyarı. Galaxy DB'ye geri dönülmez.
   - Kimliklerin eşit olduğu planda canlı veri karşılaştırmasıyla doğrulanır (g0 çıktısı ↔ Galaxy DB).
5. **GOG girişi:** Mevcut `login_flow` nonce'lu geri dönüşü yeterli; G0 bununla çalıştı. Yalnız GOG hesabıyla giriş yapılır, captcha hostlarına izin verilir; sosyal girişler hedef dışı.
6. **Filtre modeli:**
   - **Sürüm:** üç durumlu (hepsi / yalnız orijinal / yalnız özel sürüm).
   - **Koleksiyon:** belirli bir koleksiyon kimliği, içinde ya da dışında.
   - **Uygulanma:** Filtre sekme kapsamının içinde uygulanır. Sekme sayaçları etkin filtreyi yansıtır (bugünkü Kurulu davranışı); sekme görünürlüğü filtresiz sayıyla belirlenir (Faz 3 kararı).
   - **Eksik başvuru:** Silinmiş koleksiyona başvuran ölçüt yok sayılır.
   - **Kalıcılık:** Etkin filtre `ui.filter`'da saklanır (`"installed"` ya da filtre id'si, boş = yok).
   - **"Kurulu":** Sanal ve yerleşiktir, tabloda tutulmaz.
   - **Göç:** `ui.installed = "1"` ise ilk açılışta `ui.filter = "installed"` olur.
7. **Sürükle-bırak:**
   - Bırakma hedefi yalnız şeritteki koleksiyon sekmeleridir. Otomatik kaydırma yok.
   - `tauri.conf.json` içinde `dragDropEnabled: false` olur. Hangar dosya sürükleme kullanmıyor (dosyalar seçim penceresiyle ekleniyor); kurucu bunu doğrular.
   - Kart koleksiyonda zaten varsa kısa bir mesaj gösterilir: "zaten <ad> içinde".
8. **Ayarlar ve ilk deneyim:**
   - "Oyun ekle" ve "Yenile" kenar çubuğunda kalır.
   - Steam yönlendirmesi (nudge) değişir: Steam bağlı değilse ve anahtar da yoksa "Steam'e bağlan" der ve Ayarlar › Hesaplar'ı açar.

## Test
- **Çekirdek:**
  - Steam/GOG istemcileri (sahte API): bağlanma, yenileme, süresi doldu, önbellek, sızıntı.
  - Filtre tablosu CRUD.
  - `ui.*` beyaz listesi.
- **Ön yüz:**
  - Filtre değerlendirme (her ölçüt, birleşim, Kurulu göçü).
  - Kenar çubuğunu daraltma/açma ve kalıcılık.
  - Sürükle-bırak (bırakma koleksiyona ekler, seçim bozulmaz).
  - Ayarlar sekmeleri.
- **Canlı:**
  1. Steam'e girişle bağlan → kurulu olmayan Steam oyunları anahtarsız gelir.
  2. GOG "Hesapla" → liste gelir, kurulu oyunlarla birleşir.
  3. "Hiç oynanmamış Steam" filtresi.
  4. Menüyü daralt, yeniden başlat → daraltılmış açılır.
  5. Kartı "AC" sekmesine sürükle.
  6. Ayarlar'da iki sekme.
