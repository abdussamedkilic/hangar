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

Veritabanı ve kapak görselleri `%APPDATA%\com.hangar.app\` altında tutulur.

## Steam API anahtarı (isteğe bağlı)

Anahtar girilmezse yalnızca kurulu Steam oyunları listelenir. Sahip olunan tüm oyunları görmek için https://steamcommunity.com/dev/apikey adresinden bir anahtar alıp uygulamanın ayarlarına girin.

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
