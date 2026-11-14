# Hangar

Hangar, Steam, Epic Games ve Ubisoft Connect kütüphanelerini tek bir pencerede toplayan Windows masaüstü oyun başlatıcısıdır. Kurulu oyunları yerel dosyalardan okur, aynı oyunun farklı mağazalardaki kopyalarını birleştirir ve tek tıkla başlatır. Tauri 2 (Rust) ve React ile yazılmıştır.

## Gereksinimler (Windows)

- Node.js (LTS)
- Rust (stable, MSVC toolchain)
- Visual Studio Build Tools (C++ ile masaüstü geliştirme iş yükü)

## Komutlar

```
npm install            # bağımlılıkları kur
npm run tauri dev      # geliştirme modunda çalıştır
npm run tauri build    # NSIS kurulum paketini üret
```

Kurulum dosyası: `src-tauri/target/release/bundle/nsis/Hangar_0.1.0_x64-setup.exe`

Testler: `cargo test -p hangar-core` ve `npm test`.

## Veri konumu

Veritabanı ve kapak görselleri `%APPDATA%\com.hangar.app\` altında tutulur. Bağlı hesap belgesi `accounts\` klasöründedir.

## Steam API anahtarı (isteğe bağlı)

Anahtar girilmezse yalnızca kurulu Steam oyunları listelenir. Sahip olunan tüm oyunları görmek için https://steamcommunity.com/dev/apikey adresinden bir anahtar alıp uygulamanın ayarlarına girin.

SteamID alanına kullanıcı adını değil, 17 haneli SteamID64 numarasını yazın; boş bırakabilirsiniz. Boşsa Hangar SteamID'yi Steam'in `loginusers.vdf` dosyasından alır.

## Hesap bağlantıları (Steam)

Steam hesabını bağlarsan sahip olduğun ama kurulu olmayan Steam oyunları API anahtarı olmadan listelenir. Bağlanmak isteğe bağlıdır.

- **Giriş:** Hangar, Steam'in kendi giriş sayfasını ayrı bir pencerede açar (QR ya da şifre); şifreni Hangar görmez.
- **Oturum nerede:** oturum, Hangar'a ait kalıcı bir tarayıcı profilinde (`%APPDATA%\com.hangar.app\webview-steam`) durur; sonraki taramalarda Hangar bu profille görünmez bir pencere açıp listeyi çeker. Hangar çerezlere dokunmaz; Steam'in geçici web anahtarı sayfadan dışarı çıkmaz, yalnızca oyun listesi alınır.
- **Ne saklanıyor:** SteamID ve görünen ad, `accounts\steam.dat` dosyasında, Windows DPAPI ile şifreli.
- **Nasıl silinir:** "Bağlantıyı kes" profil klasörünü, `steam.dat` dosyasını ve Steam liste önbelleğini siler; oturum Hangar'da biter (Steam sunucusunda oturum kapatılmaz).
- **API anahtarı isteğe bağlı:** Hesaplar › Steam › "Gelişmiş" altına anahtar girilirse giriş yerine anahtar kullanılır.

## Hesap bağlantıları (GOG)

Varsayılan kaynak Galaxy'nin yerel veritabanıdır. Hesaplar › GOG › "Kaynak"ı "GOG hesabı" yaparsan sahip olunan liste GOG hesabından gelir.

- **Giriş:** GOG'un kendi giriş sayfası ayrı bir pencerede açılır; Hangar şifreni görmez. Yalnızca GOG hesabıyla giriş yapılır.
- **Ne saklanıyor:** yenileme belgesi, GOG kullanıcı kimliği ve görünen ad, `accounts\gog.dat` dosyasında, Windows DPAPI ile şifreli. "Bağlantıyı kes" dosyayı ve listeyi siler.
- **Resmi olmayan yol:** Hangar, GOG Galaxy istemcisinin herkese açık istemci kimliğini kullanır. GOG bunu değiştirebilir ya da engelleyebilir; bu GOG'un kullanım koşullarına aykırı sayılabilir.

## Hesap bağlantıları (Epic)

Epic hesabını bağlarsan sahip olduğun ama kurulu olmayan Epic oyunları da listelenir. Bağlanmak isteğe bağlıdır.

- **Giriş:** Hangar, Epic'in kendi giriş sayfasını ayrı bir pencerede açar; şifreni Hangar görmez. Epic e-postasıyla ya da Epic'in sunduğu Apple, Xbox/Microsoft, PlayStation, Steam, Nintendo, Facebook ve Google girişleriyle bağlanabilirsin. Google bazen gömülü pencerelerde girişi reddeder; öyle olursa Epic e-postanla gir.
- **Ne saklanıyor:** yalnızca yenileme belgesi, geçerlilik süresi, Epic hesap kimliği ve görünen ad, `%APPDATA%\com.hangar.app\accounts\epic.dat` dosyasında. Windows DPAPI ile şifreli; yalnızca bu Windows kullanıcısı çözebilir. Belgenin ömrü yaklaşık bir yıldır ve her kullanımda yenilenir.
- **Kaydedilemezse:** bağlanırken belge diske yazılamazsa "Bağlantı kaydedilemedi" hatası görünür. Sonradan yenilenen belge yazılamazsa Hangar onu yeniden başlatmaya kadar bellekte tutar; yeniden başlatınca tekrar bağlanman gerekebilir.
- **Nasıl silinir:** "Bağlantıyı kes" dosyayı siler, Epic'teki oturumu kapatır ve Hangar'ın Epic oyun listesi ile başlık önbelleklerini temizler. Dosya elle de silinebilir.
- **Epic'te görünüşü:** Hangar, Epic Games Launcher'ın herkese açık istemci kimliğini kullanır. Bu yüzden Epic hesap sayfasında bağlantı "Epic Games Launcher" olarak görünür.
- **Çekiliş/ödül oyunları:** Ubisoft'un normal oyun listesi hiç başlatılmamış ya da çekilişle gelen oyunları göstermiyor. Hangar bunlar için Ubisoft Connect'in iç protokolünü (demux) yalnızca okuma amacıyla kullanır.
- **Uygulama kimliği:** saklanan yenileme belgesi Ubisoft Connect masaüstü uygulamasının kimliğine aittir; Ubisoft hesap sayfasında bu şekilde görünebilir.
- **Güncelleme sonrası:** bu sürümden önce bağlandıysan Ubisoft hesabını bir kez yeniden bağlaman gerekir.
- **Resmi olmayan servis:** Hangar Ubisoft'un belgelenmemiş uç noktalarını ve protokolünü kullanır. Ubisoft bunları değiştirebilir ya da engelleyebilir; demux çalışmazsa yalnızca normal (GraphQL) liste, o da çalışmazsa yalnızca kurulu Ubisoft oyunları görünür.
- **Kullanım koşulları:** bu yol Epic'in kullanım koşullarına aykırı sayılabilir. En kötü durumda hesap işaretlenebilir. Legendary, Heroic ve Playnite aynı yolu kullanıyor, ama bu bir garanti değil.
- **Belgenin kapsamı:** belge hesabın tamamına erişir. DPAPI, aynı kullanıcıyla çalışan zararlı yazılıma karşı koruma sağlamaz.
- **Sunucu yok:** Hangar'ın sunucusu yok; veri yalnızca Epic'e gider.
- **Kalıcı belge istemiyorsan:** işin bitince bağlantıyı kes.

## Hesap bağlantıları (Ubisoft)

Ubisoft Connect hesabını bağlarsan sahip olduğun ama kurulu olmayan Ubisoft oyunları da listelenir. Bağlanmak isteğe bağlıdır.

- **Giriş:** Hangar, Ubisoft'un kendi giriş sayfasını ayrı bir pencerede açar; şifreni Hangar görmez.
- **Ne saklanıyor:** yalnızca yenileme belgesi, geçerlilik süresi, Ubisoft hesap kimliği ve görünen ad, `%APPDATA%\com.hangar.app\accounts\ubisoft.dat` dosyasında. Windows DPAPI ile şifreli; yalnızca bu Windows kullanıcısı çözebilir.
- **Nasıl silinir:** "Bağlantıyı kes" dosyayı siler, Ubisoft'taki oturumu kapatır ve Hangar'ın Ubisoft oyun listesini temizler. Dosya elle de silinebilir.
- **Resmi olmayan servis:** Hangar Ubisoft'un belgelenmemiş uç noktalarını ve Ubisoft Connect'in uygulama kimliğini kullanır. Ubisoft bunları değiştirebilir ya da engelleyebilir; o zaman yalnızca kurulu Ubisoft oyunları görünür.
- **Belgenin kapsamı:** belge hesabın tamamına erişir. DPAPI, aynı kullanıcıyla çalışan zararlı yazılıma karşı koruma sağlamaz.
- **Sunucu yok:** Hangar'ın sunucusu yok; veri yalnızca Ubisoft'a gider.

## Mimari

Üç katman vardır: `crates/hangar-core` arayüzden bağımsız iş mantığını (sağlayıcılar, SQLite, birleştirme, kapak görselleri) içerir; `src-tauri` bu çekirdeği Tauri komutları olarak dışarı açar; `src` altındaki React arayüzü bu komutları çağırır.

```
crates/hangar-core/src/
  model.rs        Platform, Game gibi ortak tipler
  providers/      steam.rs, epic.rs, ubisoft.rs (LibraryProvider)
  db.rs           SQLite saklama
  merge.rs        çok mağazalı oyunları birleştirme
  library.rs      tarama ve kütüphane akışı
  artwork.rs      kapak görselleri
  vdf.rs          Steam VDF ayrıştırıcı
src-tauri/src/    commands.rs (Tauri komutları), main.rs
src/              React arayüzü (components, lib, state, styles)
```

## Yeni sağlayıcı ekleme

1. `crates/hangar-core/src/providers/<ad>.rs` dosyasında `LibraryProvider` trait'ini uygulayın ve `providers/mod.rs` içinde modülü açın.
2. `model.rs` içinde `Platform` enum'una yeni varyantı ekleyin; `as_str` ve `ALL` sırasını güncelleyin.
3. `src-tauri/src/commands.rs` içindeki sağlayıcı listesine (`providers`) yeni sağlayıcıyı kaydedin.
4. Fixture dosyaları ve testleri ekleyin.
5. Arayüzde rozet etiketini ekleyin (`src/components/GameCard.tsx` içindeki `ABBR`, `src/lib/library.ts` içindeki `PLATFORMS` ve filtre sayaçları, `Sidebar.tsx`).
