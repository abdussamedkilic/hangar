# Hangar — Birleşik Oyun Kütüphanesi · Tasarım

Tarih: 2026-11-09 · Durum: onay bekliyor
(Çalışma adı "Hangar" benim seçimim; istediğin zaman değişir.)

## Niyet ve neden

Kullanıcı oyunlarını Steam ve Epic Games'te oynuyor; ileride Ubisoft Connect ve EA app hesapları da bağlanabilir. İstediği, tüm oyunlarını **tek bir yerde** görmek ve oradan başlatmak. Playnite bu işi yapıyor ama kullanıcı **kendi projesini**, arayüzü kendi tasarladığı ve istediği gibi geliştirebileceği bir uygulamayı istiyor. GOG Galaxy'nin arayüzünü sevmiyor, Galaxy entegrasyonu istemiyor. Playnite ilham kaynağı, fork değil. Gerekçe: açık kodu C#/WPF ile yazılmış eski bir sürüm (v10), v11 ise kapalı repoda sıfırdan yazılıyor ve WPF, istenen web animasyon kütüphanelerini çalıştıramaz.

Başarı ölçütü: uygulamayı açınca Steam ve Epic oyunlarının hepsi, kapak görselleriyle, tek bir akıcı ve "AI yapımı gibi görünmeyen" arayüzde listeleniyor. Tek tıkla başlatılabiliyor.

## Hedefler (sürüm 1)

1. **Steam:** kurulu oyunlar (yerel dosyalardan) ve sahip olunan ama kurulu olmayan oyunlar (Steam Web API ile, kullanıcının kendi API anahtarı ve SteamID'siyle).
2. **Epic:** kurulu oyunlar (yerel manifest dosyalarından).
3. **Ubisoft Connect:** kurulu oyunlar (registry'den). Kurulu oyun yoksa sessizce boş döner.
4. **Birleştirme:** aynı oyun birden fazla platformdaysa tek kart olarak gösterilir. "Başlat" varsayılan olarak Steam'den açar; kartın yanındaki seçiciyle başka platform seçilebilir. Oyun başına seçilen platform hatırlanır.
5. **Başlatma, kurma ve güncelleme işlerini platformların kendi launcher'ları yapar.** Bunun için platformun kendi linki açılır: `steam://rungameid/<id>`, `steam://install/<id>`, Epic ve Ubisoft için de onların link biçimleri. Hangar kendi indirme motoruna sahip değil.
6. **Kütüphane ekranı:** kapak ızgarası, arama, filtreler (platform, kurulu/kurulu değil), sıralama (ad, son oynanan). Oyun detay paneli: kapak, platformlar, kurulu boyut, son oynama ve oynama süresi (Steam'de varsa).
7. **Arayüz:** akıcı, karakterli, koyu tema öncelikli. Animasyonlar GSAP ve Lenis ile, react-bits bileşenleri yerinde kullanılır. Tasarım yönü birlikte belirlenecek (Dribbble referansları, taste/unslop/ui-ux-pro-max skill'leri).

## Hedef dışı (sürüm 1)

- GOG. Sağlayıcı mimarisi sonradan kendi GOG girişiyle eklemeye izin verecek.
- EA app. Yerel veri biçimi belirsiz; sağlayıcı olarak sonra eklenecek.
- Epic'in sahip olunan ama kurulu olmayan oyunları. Epic hesabına giriş (OAuth) gerektirir, sonraki sürüme kalır.
- Kendi indirme ve güncelleme motoru, mod yönetimi, başarımlar, arkadaş listesi.
- Linux ve macOS. Sadece Windows hedefleniyor.

## Anti-hedefler (çalışsa bile başarısızlık sayılacaklar)

- Arayüzün hazır şablon ya da "AI yapımı" görünmesi: mor gradyanlar, parlamalar, emoji etiketler.
- Takılan, kasan animasyonlar. 500+ oyunluk kütüphanede ızgara 60 fps kaymalı.
- Uygulamanın kullanıcı izni olmadan internete veri göndermesi. API anahtarı yalnızca yerelde saklanır.
- Bir platformun launcher'ı kapalıyken uygulamanın çökmesi ya da hata yağdırması.

## Kısıtlar

- Windows 11, Türkçe sistem. Kullanıcı adı `user`.
- Proje yolu: `C:\Users\user\workdir\hangar`.
- Ortam hazır: Node 26, npm 11, Rust 1.99 (MSVC), VS Build Tools 2022, git 2.56.
- Geliştirme akışı: Claude kodu bir köprü üzerinden doğrudan bu klasöre yazar, ama Windows'ta komut çalıştıramaz. `npm install` ve `npm run tauri dev` komutlarını kullanıcı Windows terminalinde çalıştırır ve çıktıyı paylaşır. Git commit için Claude'un klasörde silme izni olması gerekir; yoksa commit'leri kullanıcı yapar.
- Kodlamada en fazla 2-3 Sonnet alt ajanı kullanılır. Karpathy kuralları geçerli: önce düşün, sadelik, sadece gerekeni değiştir, doğrulanabilir hedef koy.

## Yaklaşımlar

| | Yaklaşım | Artı | Eksi |
|---|---|---|---|
| A | Playnite 10 forku (C#/WPF) | Entegrasyonlar hazır | Arayüz XAML temalarıyla sınırlı, GSAP/react-bits yok, kod tabanı büyük ve terk edilmek üzere |
| **B** | **Tauri 2 (Rust arka uç + React arayüz)** | Arayüz tamamen özgür, küçük (~10 MB) ve hızlı uygulama, proje tamamen kullanıcının | Rust öğrenme eğrisi (arka uç küçük kalacak) |
| C | Electron (her şey TypeScript) | Tek dil, kolay | Ağır uygulama (~150 MB, yüksek RAM) |

Python GUI elendi: istenen animasyon kütüphaneleri orada yok.
**Seçilen: B.** Kullanıcı seçti.

## Mimari

```
React UI (Vite + TS, GSAP, Lenis, react-bits)
      │ Tauri komutları (invoke) + olaylar
Rust çekirdek
  ├─ providers/        her platform bir LibraryProvider
  │    steam.rs  epic.rs  ubisoft.rs   (sonra: gog.rs, ea.rs)
  ├─ merge.rs          aynı oyunu tek kayda birleştirme
  ├─ db.rs             SQLite (rusqlite) — oyunlar, platform kayıtları, tercihler
  ├─ artwork.rs        kapak görselleri: bul, önbelleğe al
  └─ launch.rs         platform linklerini açma
```

- **LibraryProvider** sözleşmesi: `id()`, `scan() -> Vec<PlatformEntry>`, `launch(entry)`, `install(entry)`, `is_available()`. Yeni platform eklemek, yeni bir dosya eklemek demek.
- **Veri modeli:**
  - `games`: birleşik oyun, görünen ad, normalize ad, kapak yolu, tercih edilen platform.
  - `platform_entries`: platform, platform içi id, kurulu mu, kurulum yolu, boyut, son oynama, oynama süresi, `game_id`.
  - `settings`: Steam API anahtarı, SteamID ve benzeri ayarlar.
- **Birleştirme kuralı:** ad normalizasyonu (küçük harf, ™/®/: gibi işaretleri ve "Edition/Remastered" eklerini atma) ile eşleştirme. Kullanıcı yanlış birleşmeyi elle ayırabilir, eksik birleşmeyi elle birleştirebilir; bu kararlar kalıcıdır.
- **Tarama akışı:** açılışta önce önbellekteki veritabanı anında gösterilir, tarama arka planda çalışır, değişiklikler olayla arayüze akar. Ayrıca "Yenile" düğmesi var.

## Platform veri kaynakları (kodlamadan önce makinede doğrulanacak)

- **Steam:** kurulum yolu registry'den (`HKCU\Software\Valve\Steam\SteamPath`). Kütüphaneler `steamapps\libraryfolders.vdf` dosyasında, kurulu oyunlar `appmanifest_*.acf` dosyalarında. Kapaklar yerel `appcache\librarycache` klasöründen, yoksa Steam CDN'inden alınır. Sahip olunanlar `IPlayerService/GetOwnedGames` ile çekilir.
- **Epic:** `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON). Başlatma linki `com.epicgames.launcher://apps/<ns>%3A<itemId>%3A<appName>?action=launch&silent=true`.
- **Ubisoft:** `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<id>`. Başlatma linki `uplay://launch/<id>/0`.

## Kullanıcının belirledikleri

Tauri; kendi projesi (fork değil); GOG yok; varsayılan platform Steam ve yanında seçici; indir/güncelle platformun kendi launcher'ına devredilir; Windows'ta çalışır; animasyonlar GSAP + Lenis + react-bits; arayüzü birlikte tasarlayacağız.

## Claude'un kararları (itiraz edilebilir)

- Çalışma adı "Hangar"; proje yolu `workdir\hangar`.
- Sürüm 1'de GOG yok (soru iki kez cevapsız kaldı); EA ve Epic'in kurulu olmayan oyunları da sürüm 1 dışında.
- SQLite + rusqlite; arayüz tarafında React + TypeScript + Vite + Tailwind.
- Steam API anahtarı kullanıcıdan istenir ve yalnızca yerelde saklanır.

## İnceleme sonrası netleşenler (Claude'un kararları)

- **Doğrulama döngüsü:** kodlamadan önce kullanıcı, salt-okur bir "probe" PowerShell betiği çalıştırır. Betik Epic manifest listesini, Steam `libraryfolders.vdf`, `loginusers.vdf` ve örnek bir `.acf` dosyasını, Ubisoft registry anahtarlarını kaydeder; kişisel bilgiler (hesap adları) maskelenir. Bu örnekler test fixture'ı olur; sağlayıcılar bunlara karşı birim testleriyle yazılır. Sonrasında küçük kilometre taşları: Claude yazar, kullanıcı derleyip çıktıyı paylaşır.
- **Steam anahtarı isteğe bağlı.** Anahtar yoksa sadece kurulu oyunlar görünür ve kurulu olmayanlar için anahtarı isteyen sade bir boş durum ekranı çıkar. SteamID `loginusers.vdf` dosyasından otomatik bulunur, elle de değiştirilebilir. Anahtar SQLite'ta düz metin olarak durur (tek kullanıcılı, yerel uygulama); kimlik bilgisi kasası kullanılmaz.
- **İnternet:** telemetri yok. Ağ erişimi sadece Steam Web API ve görsel indirmek için. Uygulamayı kullanmak bu erişimlere onay sayılır; ayrıca bir onay ekranı yok.
- **Epic ve Ubisoft kapakları:** önce oyun adıyla Steam mağazasında eşleşme aranır (anahtarsız, herkese açık arama) ve Steam görseli kullanılır. Bulunmazsa oyun adıyla üretilmiş, tasarıma uygun bir yer tutucu kapak gösterilir. SteamGridDB ve elle kapak seçme sonraki sürümde.
- **Kurma düğmesi:** Steam'de `steam://install`. Epic ve Ubisoft'ta düğme ilgili launcher'ı açar; kurulumu kullanıcı orada yapar. Sağlayıcı sözleşmesinde `install()` isteğe bağlı.
- **Birleştirme:** boyut, son oynama ve süre gibi bilgiler detay panelinde platform platform gösterilir. Kartta en son oynanan platformun bilgisi görünür. Elle verilen ayırma/birleştirme kararları, platform kimlik çiftleriyle anahtarlanan bir `merge_overrides` tablosunda tutulur; böylece yeniden taramadan sonra da geçerli kalır. Bu kararlar detay paneldeki basit bir kontrolle verilir.
- **Son oynama:** Steam'de Web API'den ya da `.acf` dosyasından gelir. Diğer platformlarda Hangar sadece kendi başlattığı zamanı kaydeder; oyun süreci izlenmez, dolayısıyla oynama süresi yok.
- **Paketleme:** sürüm 1 sonunda `tauri build` ile bir NSIS kurulum dosyası üretilir; imzalama ve otomatik güncelleme yok. Veritabanı ve görsel önbelleği `%APPDATA%\Hangar` altında.
- **Arayüz sırası:** veri katmanı önce, işlevsel ama tasarım token'larına bağlı bir iskeletle kurulur. Görsel yön seçilince token'lar ve bileşenler giydirilir. Izgara sanallaştırılmış (virtualized) ve kendi kaydırma alanına sahip; Lenis bu alana bağlanır, GSAP giriş ve geçiş animasyonlarında kullanılır. Kaydırma sırasında her karta ayrı animasyon yapılmaz.

## Builder'a bırakılanlar

Dosya ve klasör düzeni, durum yönetimi kütüphanesi, test yaklaşımı, kapak önbelleği biçimi, ad normalizasyonunun ayrıntıları.

## Açık nokta — arayüz

Görsel yön, plan aşamasından önce ayrı bir tur olarak ele alınacak: Dribbble referansları ve 2-3 mockup seçeneği üzerinden kullanıcıyla birlikte seçilecek.

## Görsel yön (seçildi: 2026-11-09)

Kullanıcı 8 mockup arasından **"5 · Kokpit + Vitrin"** yönünü seçti. Referans: https://claude.ai/artifact/LKs8cVUuDnVjv11NVRFuo9 (ilk sekme).

- **Düzen:** solda sabit kenar çubuğu (Kütüphane: Tümü / Kurulu / Favoriler; Platformlar sayaçlı; Koleksiyonlar). Sağda ana alanın üstünde "sahne": seçili oyunun kapağından türeyen bulanık arka plan ışığı, büyük başlık, küçük istatistik şeridi (oynama, son, boyut), bölünmüş Oyna düğmesi ve sağda eğik gölgeli kapak. Altında arama çubuğu (Ctrl K) ve sıralama, onun altında sık kapak ızgarası. Kartlarda platform rozetleri ve kurulu noktası var. Kurulu olmayan oyunlar soluk gösterilir.
- **Tipografi:** başlıklar Chakra Petch (büyük harf), gövde metni Figtree.
- **Renk:** koyu arduvaz zemin (#0f1318 civarı), tek vurgu rengi kehribar (#f2a93b), kurulu durumu için yeşil nokta. Sürüm 1'de sadece koyu tema.
- **Hareket:** sahnede oyun değişince kısa bir çapraz geçiş ve içeriğin kayarak gelmesi (GSAP). Izgara ilk yüklemede kademeli belirir. Kartta hover'da hafif yükselme. Izgaranın kendi kaydırma alanı var ve Lenis o alana bağlanır. `prefers-reduced-motion` ayarına uyulur.
- **Dar pencerede:** kenar çubuğu gizlenir, sahne tek sütuna iner.
