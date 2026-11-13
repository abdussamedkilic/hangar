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

## Hesap bağlantıları (Epic)

Epic hesabını bağlarsan sahip olduğun ama kurulu olmayan Epic oyunları da listelenir. Bağlanmak isteğe bağlıdır.

- **Giriş:** Hangar, Epic'in kendi giriş sayfasını ayrı bir pencerede açar; şifreni Hangar görmez. Epic e-postasıyla ya da Epic'in sunduğu Apple, Xbox/Microsoft, PlayStation, Steam, Nintendo, Facebook ve Google girişleriyle bağlanabilirsin. Google bazen gömülü pencerelerde girişi reddeder; öyle olursa Epic e-postanla gir.
- **Ne saklanıyor:** yalnızca yenileme belgesi, geçerlilik süresi, Epic hesap kimliği ve görünen ad, `%APPDATA%\com.hangar.app\accounts\epic.dat` dosyasında. Windows DPAPI ile şifreli; yalnızca bu Windows kullanıcısı çözebilir. Belgenin ömrü yaklaşık bir yıldır ve her kullanımda yenilenir.
- **Kaydedilemezse:** bağlanırken belge diske yazılamazsa "Bağlantı kaydedilemedi" hatası görünür. Sonradan yenilenen belge yazılamazsa Hangar onu yeniden başlatmaya kadar bellekte tutar; yeniden başlatınca tekrar bağlanman gerekebilir.
- **Nasıl silinir:** "Bağlantıyı kes" dosyayı siler, Epic'teki oturumu kapatır ve Hangar'ın Epic oyun listesi ile başlık önbelleklerini temizler. Dosya elle de silinebilir.
- **Epic'te görünüşü:** Hangar, Epic Games Launcher'ın herkese açık istemci kimliğini kullanır. Bu yüzden Epic hesap sayfasında bağlantı "Epic Games Launcher" olarak görünür.
- **Resmi olmayan servis:** Epic bu yolu engelleyebilir; o zaman yalnızca kurulu Epic oyunları görünür.
- **Kullanım koşulları:** bu yol Epic'in kullanım koşullarına aykırı sayılabilir. En kötü durumda hesap işaretlenebilir. Legendary, Heroic ve Playnite aynı yolu kullanıyor, ama bu bir garanti değil.
- **Belgenin kapsamı:** belge hesabın tamamına erişir. DPAPI, aynı kullanıcıyla çalışan zararlı yazılıma karşı koruma sağlamaz.
- **Sunucu yok:** Hangar'ın sunucusu yok; veri yalnızca Epic'e gider.
- **Kalıcı belge istemiyorsan:** işin bitince bağlantıyı kes.

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
