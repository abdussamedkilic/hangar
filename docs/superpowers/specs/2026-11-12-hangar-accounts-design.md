# Hangar — Epic için yerel hesap bağlantısı · Tasarım

Tarih: 2026-11-12 · Durum: konseyden geçti, kullanıcı onayı bekliyor · Önceki: `2026-11-11-hangar-step1-design.md`

## Niyet ve neden

Hangar bugün Epic ve Ubisoft'ta yalnızca **kurulu** oyunları gösteriyor (Epic `.item` manifestleri, Ubisoft kayıt defteri). Kullanıcı bu iki hesaptaki oyunların **tamamını**, kurulu olmayanlar dahil, Steam ve GOG'daki gibi tek kütüphanede görmek istiyor.

Kullanıcının kararı (2026-11-12): **GOG Galaxy kullanılmayacak.** Yol Playnite'ınki gibi olacak:
- Bağlantı uygulamanın içinde kurulur.
- Hiçbir veri kullanıcının bilgisayarından çıkmaz; Hangar'ın kendi sunucusu ve telemetrisi yok.
- Giriş platformun **kendi sayfasında** yapılır, Hangar şifreyi hiç görmez.

Bu belge **yalnızca Epic'i** kapsar. Ubisoft ayrı bir keşif görevine (U0) ve ardından ayrı bir plana bırakıldı (bkz. "Sonraki iş: Ubisoft"). Bu, konseyin ortak kararı.

Başarı ölçütü:
- Ayarlar › Hesaplar'dan "Epic'e bağlan"a tıklanır, Epic'in giriş penceresinde giriş yapılır, pencere kendiliğinden kapanır.
- Bundan sonraki taramada Epic hesabındaki bütün oyunlar gelir. Kurulu olanlar mevcut kartlarla birleşir, olmayanlar "Kur" kartı olur.
- Uygulama kapatılıp açıldığında yeniden giriş istenmez.
- "Bağlantıyı kes" hem yerel dosyayı siler hem Epic'teki oturumu kapatır.

## Hedefler

1. **Epic bağlantısı:** uygulama içi giriş penceresi → yetki kodu → OAuth belgesi.
   - Sahip olunan oyunlar Epic'in kütüphane ve katalog servislerinden alınır.
   - Bedava dağıtılan oyunlar dahildir. DLC, eklenti ve Unreal Engine varlıkları dahil değildir.
2. **Yerel ve şifreli belge:** yenileme belgesi yalnızca `%APPDATA%\com.hangar.app\accounts\epic.dat` dosyasında, Windows DPAPI (CurrentUser) ile şifreli tutulur.
3. **Ayarlar › Hesaplar bölümü:** Epic için durum ("Bağlı değil" / "Bağlı · <görünen ad>" / "Süresi doldu"), "Bağlan" ve "Bağlantıyı kes" düğmeleri.
4. **Kurulu olmayanı kurma:** "Kur" düğmesi, Steam ve GOG'da olduğu gibi Epic launcher'ında kurulumu açar.
5. **Çevrimdışı dayanıklılık:** ağ yoksa ya da belge geçersizse son bilinen sahip olunan liste kullanılır. Üstteki uyarı satırında ne yapılması gerektiği yazar.
6. **README güvenlik notu:** belgenin nerede durduğu, nasıl silindiği, riskler (bkz. §7).

## Hedef dışı

- GOG Galaxy entegrasyonu. Kullanıcı reddetti; bir daha önerilmeyecek.
- **Ubisoft** (bu planda ne arayüz satırı ne README metni olacak; yalnızca ortak altyapı platform parametresi alır).
- EA app, Amazon Games, Battle.net ve diğer platformlar. Genel bir "sahip olunan sağlayıcı" katmanı da hedef dışı.
- Katalogdaki görselleri kapak olarak kullanmak. Olası sonraki iş olarak not edildi, bu planda yok.
- Oynama süresi, başarımlar, arkadaşlar, bulut kayıtları.
- Hangar'ın oyunu kendisinin indirmesi ya da kurması.
- Birden fazla Epic hesabı.
- Belgeyi saklamayan ("her seferinde giriş") mod. Konseyde önerildi, kullanıcı kalıcı belgeyi seçmişti; README'de "istemezsen bağlantıyı kes" notu yeterli.

## Anti-hedefler (çalışsa bile başarısızlık sayılacaklar)

- Belgenin, access token'ın ya da yetki kodunun **herhangi bir yere** yazılması:
  - log, hata mesajı, uyarı satırı, SQLite, `settings` tablosu, panik çıktısı, test fixture'ı.
  - Hata mesajları en fazla HTTP durum kodunu ve kısa bir açıklamayı içerir; yanıt gövdesini içermez.
- Giriş sayfasının (uzak bir web sayfasının) Hangar'ın Tauri komutlarını çağırabilmesi.
- Giriş penceresinin diskte çerez ya da oturum bırakması. Hangar'ın şifreli dosyası dışında ikinci bir kalıcı oturum olmamalı.
- Giriş penceresinin Epic'in giriş akışı dışında rastgele sitelere gidebilmesi.
- Bağlantı koptuğunda kullanıcının kütüphanesindeki oyunların sessizce kaybolması.
- Şifre isteyen kendi giriş formu.
- Başarılı girişten sonra uygulamanın "donmuş" görünmesi.

## Kısıtlar

- Windows 11, Tauri **2.12.5** (wry 0.57.1; `Cargo.lock`'ta doğrulandı), Rust, React 18 + TS.
  - `WebviewWindowBuilder` bu sürümde `incognito()`, `initialization_script()`, `on_navigation()` ve `on_new_window()` sunuyor; sürüm yükseltmek gerekmiyor.
  - Windows'ta `incognito()`, WebView2 InPrivate'a karşılık geliyor.
- `hangar-core` saf ve Tauri'siz; Linux/WSL'de test edilir.
  - Ağ çağrıları ve DPAPI trait arkasında olmalı.
  - Ayrıştırma kodu fixture'larla test edilir.
- HTTP için mevcut `ureq = "2"` kullanılır. Çağrılar engelleyici olduğu için `spawn_blocking` içinde çalışır.
- DPAPI için `windows-sys` (`cfg(windows)` altında) kullanılır; başka yeni bağımlılık en az tutulur.
- `scan_all` bugün yalnızca Steam'in sahip olunan listesini alıyor (`owned: Option<Result<Vec<PlatformEntry>, ProviderError>>`). Bu imza platform başına bir haritaya genişletilir.
- `tauri dev` açıkken Rust düzenlemek uygulamayı yeniden başlatır; elle testlerde dikkat.
- Kullanıcının kuralları:
  - İstenmeyen özellik yok, cerrahi değişiklik.
  - Alt ajanlar Sonnet ve aynı anda en fazla 2–3.
  - Commit mesajları Türkçe conventional.

## Yaklaşımlar

Ana soru girişin nasıl yakalanacağıydı.

- **A — Hangar penceresi, otomatik yakalama (seçildi).** Hangar Epic'in giriş sayfasını kendi ayrı penceresinde açar. Giriş bitince kodu kendisi alır ve pencereyi kapatır. Kullanıcı açısından tek tık. Bedeli: Epic sayfayı değiştirirse ya da gömülü tarayıcıyı engellerse yakalama kırılabilir.
- **B — Sistem tarayıcısı + kodu elle yapıştırma (Legendary yolu).** Basit ve daha az kırılgan, ama her bağlanmada elle iş var. Yaklaşık 30 satırlık bir iş; E0 başarısız olursa kullanıcıya önerilecek yedek yol budur.
- **C — GOG Galaxy'den okumak.** Kullanıcı reddetti.

Kullanıcı **A**'yı seçti (2026-11-12).

## Kullanıcının kararlaştırdıkları

- Galaxy yok; uygulama içi bağlantı; veri yalnızca bu PC'de.
- Giriş Epic'in kendi sayfasında, Hangar'ın ayrı penceresinde (yaklaşım A).
- Belge yerelde, DPAPI ile şifreli; "Bağlantıyı kes" siler; asla loglanmaz; sunucu ya da telemetri yok.
- Ayarlar'da "Hesaplar" bölümü; README'de güvenlik notu.

## Tasarım

### 0. Görev E0 — keşif (önce bu; en fazla bir oturum)

Önce yalnızca giriş penceresi ve yakalama yapılır, gizli bir geliştirme komutunun arkasında (ör. `debug_assertions` altında). Kullanıcı gerçek Epic hesabıyla bir kez giriş yapar.

**Başarılı sayılması için hepsi gerekli:**
1. Giriş, iki adımlı doğrulama dahil, gömülü pencerede tamamlanıyor.
2. Başlatma betiği `authorizationCode`'u okuyor. `on_navigation`, `https://hangar.invalid/callback` adresine yapılan `location.replace`'i görüp iptal edebiliyor.
3. Kod, token ucunda belgeye çevrilebiliyor. Kütüphane ve katalog çağrıları yanıt veriyor.
4. Pencere kapandıktan sonra WebView2 veri klasöründe Epic çerezi ya da oturumu kalmıyor (InPrivate doğrulaması).
5. Token, kütüphane ve katalog yanıtları bir **temizleme betiğiyle** fixture'a çevriliyor. Fixture'larda hesap kimliği, görünen ad, e-posta ve belgeler bulunmuyor; betik bunları sabit sahte değerlerle değiştiriyor ve çıktıda gerçek değer kalmadığını denetliyor.

**Başarısız sayılır:** captcha ya da bot kontrolü gömülü pencerede geçilemiyor, Epic girişi reddediyor ya da 2 numaralı yakalama çalışmıyor. Bu durumda iş durur. Kullanıcıya B (kodu elle yapıştırma) önerilir; kendiliğinden B'ye geçilmez.

E0'dan önce uç adresleri ve istemci kimliği Legendary'nin güncel kaynağıyla karşılaştırıldı (2026-11-12). Aşağıdaki adresler ve parametreler aynı.

### 1. Giriş penceresi ve kodun yakalanması

`src-tauri` bir komut sunar: `connect_account(platform)`. Bu komut aşağıdakileri yapar.

1. `WebviewWindowBuilder` ile `account-login` etiketli **ayrı** bir pencere açılır. Adres Epic'in giriş sayfasıdır. Pencere `incognito(true)` ile açılır.
2. **Pencere başlığı gerçek adresi gösterir** (ör. "Epic Games girişi — www.epicgames.com"). Böylece kullanıcı nerede olduğunu görür ve pencere kimlik avı gibi görünmez.
3. **Başlatma betiği** (initialization script) şunları yapar:
   - Yalnızca üst çerçevede (`window.top === window`) çalışır.
   - `location.origin === 'https://www.epicgames.com'` ve `location.pathname === '/id/api/redirect'` koşulunu arar.
   - `DOMContentLoaded`'dan sonra `document.body.innerText`'i JSON olarak ayrıştırır.
   - `authorizationCode` doluysa `location.replace('https://hangar.invalid/callback?code=' + encodeURIComponent(code))` yapar. Boşsa hiçbir şey yapmaz.
4. **`on_navigation`** işleyicisi şöyle davranır:
   - `hangar.invalid` gelince gezinmeyi iptal eder (`false`), kodu ayrıştırır ve bir `oneshot` kanalıyla komuta iletir, ardından pencereyi kapatır.
   - **İzin listesi:** diğer gezinmelerde yalnızca `https` ve izinli alan adlarına izin verilir: `epicgames.com` ve alt alan adları, ayrıca E0'da gözlenen captcha ve kimlik sağlayıcı alanları (ör. hCaptcha/Arkose, Google, Apple, Xbox/Microsoft, Steam, PlayStation, Nintendo, Facebook). Liste kodda tek bir sabit dizidir.
5. **`on_new_window`:** açılır pencereler aynı pencerede açılır (izin listesine tabi) ya da engellenir. Hangisinin olacağını E0 belirler: Google, Apple gibi kimlik sağlayıcılarla giriş açılır pencere istiyorsa aynı pencerede açılır.
6. **Pencerenin `Destroyed` olayı:** kanal hâlâ boşsa komut "iptal edildi" ile döner. Kullanıcıya uyarı gösterilmez.
7. **Zaman aşımı:** 5 dakika içinde kod gelmezse pencere kapanır, komut "Giriş zaman aşımına uğradı" ile döner.

Uzak sayfa için **hiçbir capability tanımlanmaz**. `capabilities/default.json` yalnızca `main` penceresine uygulanmaya devam eder.

Yetki kodu tek kullanımlıktır ve kısa ömürlüdür. Yalnızca bu yönlendirme sırasında bellekte bulunur. İptal edilen gezinme olduğu için pencere geçmişine girmez; pencere de zaten InPrivate.

Aynı anda tek bir giriş penceresi açık olabilir. Zaten açıksa öne getirilir.

### 2. Epic servisleri

**Giriş adresi:**
`https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D34a02cf8f4414e29b15921876da36f9a%26responseType%3Dcode`

**Belge alma** (Epic Games Launcher'ın herkese açık istemcisi; Legendary, Heroic ve Playnite de bunu kullanıyor):
- `POST https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token`
- Basic auth: `34a02cf8f4414e29b15921876da36f9a:daafbccc737745039dffe53d94fc76cf` (kodda sabit; zaten herkese açık)
- Form: `grant_type=authorization_code&code=<kod>&token_type=eg1`
- Kullanılan alanlar: `access_token`, `expires_at`, `refresh_token`, `refresh_expires_at`, `account_id`, `displayName`.
- Yenileme: aynı uç, `grant_type=refresh_token&refresh_token=<…>&token_type=eg1`.

**Sahip olunan oyunlar:**
- `GET https://library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true[&cursor=<…>]`, `Authorization: bearer <access_token>`.
  - `responseMetadata.nextCursor` bitene kadar sayfalanır.
  - Her kayıttan `namespace`, `catalogItemId` ve `appName` alınır.
  - `namespace == "ue"` olan kayıtlar (Unreal Engine varlıkları) atlanır.
- Katalog: `GET https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/<ns>/bulk/items?id=<id>[,<id>…]&includeDLCDetails=true&includeMainGameDetails=true&country=US&locale=en-US`. Yanıt `catalogItemId`'ye göre anahtarlanır.
  - Aynı namespace'teki kimlikler tek çağrıda istenir.
  - **Hız sınırı:** çağrılar arasında en az 100 ms beklenir.
- **Filtre** (kesin kurallar E0 fixture'larıyla sabitlenir):
  - `mainGameItem` varsa (DLC) kayıt atlanır.
  - `categories` `addons` ya da `digitalextras` içeriyorsa kayıt atlanır.
  - `categories` `applications` içerip `games` içermiyorsa kayıt atlanır.
- **Katalog önbelleği:** `catalogItemId` başına başlık ve "oyun mu" bilgisi SQLite'ta tutulur. Sonraki taramalarda yalnızca yeni kimlikler için katalog çağrılır.

**Kayıt biçimi** (kurulu manifestle aynı kimlik, böylece birleşir):
- `platform: Epic`, `platform_id: appName`, `title: katalog başlığı`, `installed: false`
- `launch_uri`: `com.epicgames.launcher://apps/<ns>%3A<catalogItemId>%3A<appName>?action=launch&silent=true`
- `install_uri`: `com.epicgames.launcher://apps/<ns>%3A<catalogItemId>%3A<appName>?action=install`

**Kurulu kayıtla birleşme:**
- Kurulu kayıt kazanır.
- Başlık manifestteki `DisplayName` olarak kalır; böylece normalleştirilmiş ad ve birleştirme/ayırma kayıtları bozulmaz.
- Kurulu kaydın `install_uri`'si, sahip olunan kaydın `action=install` bağlantısıyla güncellenir.

**"Kur" ve launcher:**
- Bağlantı `opener` ile açılır.
- Epic launcher kapalıysa Windows onu başlatır.
- Bağlantı açılamazsa (launcher kurulu değil) mevcut başlatma hata yolu "Epic Games Launcher açılamadı — kurulu mu?" mesajını gösterir.

**Bağlantıyı kes:**
1. Önce geçerli bir access token alınır (gerekirse yenilenir).
2. `DELETE https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/<access_token>` çağrılır. Başarısız olursa bu önemsenmez, ama sonuç arayüzde söylenir ("Epic'teki oturum kapatılamadı; Epic hesap ayarlarından kapatabilirsin").
3. Ardından `epic.dat`, sahip olunan liste önbelleği ve "süresi doldu" durumu silinir.
4. Yeniden tarama yapılır; kurulu oyunlar yerinde kalır.

### 3. Belge deposu

- `hangar-core` bir trait tanımlar: `SecretStore` (`load`, `save` ve `delete`, platform başına). Testlerde bellek içi sahte kullanılır.
- **Windows uygulaması:**
  - `CryptProtectData` / `CryptUnprotectData` kullanılır, CurrentUser kapsamında, sabit ek entropi `b"hangar-accounts-v1"` ile.
  - Şifreli bayt dizisi `%APPDATA%\com.hangar.app\accounts\<platform>.dat` dosyasına yazılır.
  - Yazma geçici dosya + yeniden adlandırma ile yapılır.
- Şifrelemeden önceki içerik küçük bir JSON'dur: `refresh_token`, `refresh_expires_at`, `account_id`, `display_name`.
- Kısa ömürlü `access_token` yalnızca bellekte tutulur.
- Belge metinleri kullanımdan sonra bellekte sıfırlanır (`zeroize` ya da elle). Bunu ucuzsa yapmak yeterli; aynı kullanıcıdaki zararlı yazılıma karşı tam koruma değildir.
- **Yenileme sırası:** yenileme tek bir `Mutex` arkasında sıralanır. Yeni `refresh_token` gelir gelmez, bu belge **başka bir istek için kullanılmadan önce** `save` ile diske yazılır; böylece çökme durumunda dönmüş bir belge kaybolmaz.
- Dosya çözülemiyorsa (başka Windows kullanıcısı ya da bozuk dosya) silinir ve platform "Bağlı değil" sayılır. Uyarı satırında "Epic bağlantısı okunamadı; yeniden bağlan" yazar.

### 4. Tarama ile birleşme

- `scan` sırasında Epic bağlıysa ağ işi DB kilidi tutulmadan yapılır (bugünkü Steam deseni gibi).
- `scan_all`'un `owned` parametresi platformdan bağımsız hale gelir. Birden fazla platformun sahip olunan listesini alır ve her birini kendi platformunun kurulu kayıtlarıyla `platform_id` üzerinden birleştirir. Steam'in mevcut davranışı ve testleri aynen korunur.
- **Sahip olunan liste önbelleği:** her başarılı alımdan sonra Epic'in sahip olunan listesi SQLite'ta saklanır. Önbellekte yalnızca oyun kayıtları bulunur, belge bulunmaz. Alım başarısız olursa önbellekteki liste kullanılır ve bir uyarı eklenir:
  - Ağ hatası: "Epic oyunları güncellenemedi; son bilinen liste gösteriliyor."
  - Belge geçersiz (`invalid_grant` vb.): "Epic bağlantısının süresi doldu: Ayarlar › Hesaplar'dan yeniden bağlan." Platform "Süresi doldu" olarak işaretlenir. Bu durum `settings` tablosunda yalnızca bir durum değeri olarak tutulur, belge değil. Yeniden bağlanınca sıfırlanır.
- `get_accounts` ağa çıkmaz; dosyanın varlığı ile saklı durum değerine bakar.
- **İlk tarama:** 100'den fazla katalog çağrısı nedeniyle yavaş olabilir. Bağlantıdan hemen sonraki taramada üstteki uyarı satırında "Epic oyunları yükleniyor…" görünür. Bunun için tarama başlarken ve bitince birer olay yeterli; ayrıntılı ilerleme çubuğu yapılmaz.

### 5. Arayüz — Ayarlar › Hesaplar

Mevcut Ayarlar penceresine, Steam alanlarının altına bir bölüm eklenir:

```
Hesaplar
  Epic Games     Bağlı · <görünen ad>          [Bağlantıyı kes]
  Giriş Epic'in kendi sayfasında yapılır; Hangar şifreni görmez.
  Bağlantı belgesi yalnızca bu bilgisayarda, şifreli tutulur.
```

- **Bağlan:**
  - `connect_account` çağrılır.
  - Düğme "Epic penceresi açık…" bekleme durumuna geçer.
  - Dönüşte durum güncellenir ve App'in `rescan`'i çağrılır, böylece uyarılar üstteki satırda görünür (Ayarlar'daki mevcut düzeltmeyle aynı yol).
  - Zaman aşımı ve diğer hatalar düğmenin altında kısa bir metin olarak görünür.
- **Bağlantıyı kes:**
  - Onay sorulur: "Epic bağlantısı silinecek ve Epic'teki oturum kapatılacak; kurulu oyunlar kalır."
  - Onaydan sonra `disconnect_account` ve `rescan` çağrılır.
- **Süresi doldu:** düğme "Yeniden bağlan" olur.
- **Komutlar:** `get_accounts() -> [{platform, state, display_name?}]`, `connect_account(platform)`, `disconnect_account(platform)`.
- Görsel dil mevcut Ayarlar penceresini izler.

### 6. Hata ve durum metinleri

Bütün metinler kısa ve Türkçedir. Hiçbiri belge, kod ya da yanıt gövdesi içermez. Kesin metinler kurucuya bırakılır; yukarıdakiler yönü gösterir.

### 7. Güvenlik ve README

README'ye "Hesap bağlantıları" bölümü eklenir. İçeriği:

- **Ne saklanıyor ve nerede:** yalnızca yenileme belgesi ve görünen ad, `%APPDATA%\com.hangar.app\accounts\` altında. Windows DPAPI ile korunur; yalnızca bu Windows kullanıcısı çözebilir.
- **Nasıl silinir:** "Bağlantıyı kes" hem dosyayı siler hem Epic'teki oturumu kapatır. Elle de klasör silinebilir.
- **Epic'te nasıl görünür:** Hangar, Epic Games Launcher'ın herkese açık istemci kimliğini kullanır. Bu yüzden Epic hesap ayarlarındaki oturum ve uygulama listelerinde bağlantı **"Epic Games Launcher"** olarak görünür, "Hangar" olarak değil.
- **Riskler, açıkça:**
  - Bunlar resmi olmayan uçlar. Epic bu istemci kimliğini engelleyebilir ya da yanıt biçimini değiştirebilir; o zaman Epic'in kurulu olmayan oyunları gelmez, kurulu olanlar çalışmaya devam eder.
  - Başka bir istemcinin kimliğini kullanmak Epic'in kullanım koşullarına aykırı sayılabilir. En kötü durumda hesap işaretlenebilir. Legendary, Heroic ve Playnite yıllardır aynı yolu kullanıyor ve bilinen yaygın bir yaptırım yok, ama bu bir garanti değil.
  - Belge hesabın tamamına erişim verir. DPAPI, aynı Windows kullanıcısıyla çalışan zararlı yazılıma karşı koruma sağlamaz.
- **Sunucu yok:** Hangar'ın sunucusu yok; veri yalnızca Epic'in servislerine gider.
- **Kalıcı belge istemeyenler için:** işi bitince "Bağlantıyı kes" yeterli. Sahip olunan liste önbellekte kalmaz, kesince silinir.

## Test ve doğrulama

**`hangar-core` (Linux'ta):**
- Token yanıtı ayrıştırma; kütüphane sayfalaması; katalog filtreleme (DLC, eklenti, `ue`, applications); kayıt üretimi.
- Kurulu kayıtla birleşme: başlık manifestten gelir, `install_uri` güncellenir.
- `scan_all` genellemesi: Steam testleri değişmeden geçer.
- Önbellekten geri dönüş ve uyarı metinleri.
- `invalid_grant` gelince "süresi doldu" işaretlenir.
- Yeni `refresh_token`, kullanılmadan önce `save` edilir (sahte depo ile sıra testi).
- Bozuk ya da çözülemeyen depo "Bağlı değil" sonucunu verir.
- **Belge sızıntısı testi:** sahte belge değerleri hiçbir hata mesajında, uyarıda ya da DB satırında geçmez.
- Ağ katmanı trait arkasındadır ve E0 fixture'larıyla beslenir.

**`src-tauri`:**
- `cargo check`.
- `cfg(windows)` altında DPAPI gidiş-dönüş testi ile bozuk blob testi.
- Yakalama adresi ayrıştırma ve izin listesi eşleştirmesi saf fonksiyonlar olarak birim testli olur.

**Ön yüz (vitest, mevcut `SettingsDialog.test.tsx` deseniyle):** Hesaplar bölümünün durumları, düğmeleri, onay sorusu ve hata metinleri.

**Elle (Windows, `npm run tauri dev`):**
1. Epic'e bağlan; pencere başlığı adresi gösteriyor; giriş sonrası pencere kapanıyor.
2. "Epic oyunları yükleniyor…" görünüyor, sonra kurulu olmayan Epic oyunları geliyor.
3. Death's Door hâlâ tek kart.
4. Uygulamayı yeniden aç; giriş istenmiyor.
5. Bir oyunda "Kur" Epic launcher'da kurulumu açıyor.
6. Wi-Fi kapalı tarama: son bilinen liste ve uyarı.
7. "Bağlantıyı kes": yalnızca kurulu oyunlar kalıyor, `accounts\epic.dat` silinmiş, Epic hesap sayfasında oturum kapanmış.
8. WebView2 veri klasöründe Epic çerezi yok.

## Sonraki iş: Ubisoft (bu planın dışında)

- Ubisoft'un yerel önbelleği bu PC'de şifreli (`cache\configuration\configurations.vdat`, `cache\ownership\<id>`), bu yüzden yalnızca web yolu kaldı.
- **U0 (süresi sınırlı, kullanıcının gerçek girişiyle)** şunları gösterecek:
  1. connect.ubisoft.com girişi ve ticket'ın nereden okunacağı (`ticket`, `sessionId`, `userId`, `rememberMeTicket`).
  2. `rm_v1` ile yenileme (`POST https://public-ubiservices.ubi.com/v3/profiles/sessions`).
  3. Sahip olunan oyunlar ve kayıt defteriyle eşleşen ürün kimliği.
  4. Kurulumu açan `uplay://` bağlantısı.
- U0 başarılı olursa, bu belgedeki ortak altyapı (giriş penceresi, `SecretStore`, `scan_all` haritası, Hesaplar listesi) üzerine ayrı, kısa bir plan yazılır. Başarısız olursa Ubisoft'ta yalnızca kurulu oyunlar kalır.

## Kurucuya bırakılanlar

- Modül ve dosya adları (ör. `providers/epic_account.rs`), trait şekilleri, yeni SQLite tablolarının adları ve şeması.
- `oneshot` kanalının ve pencere durumunun nasıl tutulacağı.
- İzin listesindeki kesin alan adları (E0 gözlemine göre).
- Yükleniyor olayının adı ve biçimi.
- Hata metinlerinin kesin hali.
