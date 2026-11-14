# Hangar — Koleksiyonlar, liste görünümü, tema dosyası · Tasarım

Tarih: 2026-11-14 · Durum: kullanıcı konuşmada onayladı ("okeyim"), belge onayı bekliyor
Dal: `feat/phase3` (main'den, Faz 2 = PR #5 sonrası)

## Niyet ve neden
Hangar "bitti sayılır"; kullanıcı kendi kütüphanesini kendi düzeninde görmek istiyor. Üç istek var:
1. Steam'deki gibi elle koleksiyonlar.
2. Kart görünümünün yanında liste görünümü.
3. VS Code tarzı JSON dosyasıyla renk özelleştirme.

Sıralama mantığı iyi çalışıyor, korunacak. Masaüstü exe her şey bitince yapılacak (bu belgenin dışında). Amazon Games yapılmayacak. Bilinen küçük açıklar ayrı ve paralel bir iş.

## Hedefler
- **Sekme şeridi:** Oyun alanının üstünde yatay bir şerit olur. Sıra her zaman şöyle:
  - `Tümü`
  - mağaza sekmeleri: Steam, Epic, GOG, Ubisoft, Riot, Yerel; yalnızca oyunu olanlar görünür
  - ayraç
  - kullanıcı koleksiyonları
  - `+`
- **Elle koleksiyon:**
  - Oluşturma, yeniden adlandırma ve silme.
  - Oyun ekleme ve çıkarma, karttaki sağ tık menüsünden yapılır.
  - Bir oyun birden çok koleksiyonda olabilir.
- **"Kurulu" filtresi** kenar çubuğunda kalır ve şeritteki sekmeyle birlikte uygulanır (ör. Ubisoft + Kurulu). Açıkken şeridin yanında "Kurulu ✕" etiketi görünür.
- **Görünüm seçimi:** Şeridin sağında Kart/Liste anahtarı olur, seçim kalıcıdır. Kart görünümü varsayılandır.
- **Liste görünümü:** Satır başına küçük kapak, ad, mağaza rozetleri, kurulu durumu, oynama süresi (varsa) ve Oyna/Kur düğmesi. Sağ tık menüsü ve sıralama kart görünümüyle aynıdır.
- **Tema dosyası:** `%APPDATA%\com.hangar.app\theme.json` yalnızca renkleri değiştirir.
  - Ayarlar'da iki düğme: "Tema dosyasını aç" ve "Varsayılana dön".
  - Hangar dosyayı yeniden okuyunca renkler hemen uygulanır.
  - Geçersiz değerler yok sayılır ve kısa bir uyarı gösterilir.

## Hedef dışı
- Dinamik (kurallı) koleksiyonlar. Kullanıcı gerek görmüyor; mağaza ve Kurulu filtreleri bu ihtiyacı karşılıyor.
- Steam koleksiyonlarını içe aktarma.
- Koleksiyonların sürükle-bırak ile sıralanması.
- Fontlar, boyutlar ve yerleşimin özelleştirilmesi.
- Birden çok tema arasında geçiş.

## Anti-hedefler (çalışsa da başarısızlık sayılır)
- Birleştirme ya da ayırma yapılınca oyunun koleksiyondan düşmesi.
- Yeniden taramanın koleksiyon üyeliğini silmesi.
- Sekme şeridinin kalabalıklaşıp kartları aşağı itmesi. Şerit tek satırdır ve sığmazsa yatay kayar.
- Hatalı bir tema dosyasının arayüzü okunmaz hale getirmesi. Geçersiz renk yok sayılır.
- Mevcut sıralama ya da arama davranışının değişmesi.

## Kısıtlar
- **Yığın:** React 18 + TS + Vite. Renkler `src/styles/tokens.css` içindeki CSS değişkenleridir. Veri SQLite'ta (`crates/hangar-core/src/db.rs`), komutlar `src-tauri/src/commands.rs` içindedir.
- **Kart kimliği:** Bir kart `games` satırıdır (`norm_title` = kart anahtarı). Birleştirme ya da ayırma kartın satırını değiştirebilir; birleştirilen satır silinir.
- **Kayıt kimliği:** Kayıt kimliği `"platform:platform_id"` biçimindedir (tercih de bu biçimde saklanır). Taramalar arasında sabittir.
- **Arayüz:** Görünüm tarzı "Kokpit + Vitrin" korunur (`docs/design/mockups-melez.html`). UI işlerinde design-taste-frontend ve ui-ux-pro-max kurallarına bakılır.
- **Test:** Yalnızca Tauri penceresinde test edilir (`localhost:1420` tarayıcıda çalışmaz).

## Yaklaşımlar
**Koleksiyon üyeliğinin anahtarı**
1. *Kayıt anahtarıyla (`platform:id`) — seçilen.* Bir kart, kayıtlarından herhangi biri koleksiyondaysa koleksiyonda sayılır. Birleştirme ve ayırmada üyelik kendiliğinden doğru kalır. Ekleme kartın tüm kayıtlarını, çıkarma ise tüm kayıtlarını yazar ya da siler.
2. *`games.id` ile.* En basiti, ama birleştirmede silinen satırla birlikte üyelik de kaybolur. Bu, anti-hedefe aykırı.
3. *Kart anahtarıyla (`norm_title`).* Ayırma ve elle birleştirmede kart anahtarı değişince üyelik kopar.

**Tema dosyasının uygulanması**
1. *Rust komutu dosyayı okur, ön yüz geçerli renkleri `:root` CSS değişkenlerine yazar; açılışta ve pencere odağa gelince yeniden okunur — seçilen.* Dosya düzenleyicide kaydedilip Hangar'a dönülünce değişiklik anında görünür. İzleyici süreç yok.
2. *Dosya izleyici (notify).* Daha "canlı", ama ek bağımlılık ve süreç gerektirir. Kazanç küçük.

**Koleksiyonların yeri:** konuşmada kullanıcı seçti; kenar çubuğu değil, oyun alanının üstündeki şerit.

## Kararlaştırılan tasarım
**Veri** (`db.rs`, yeni tablolar, eski veritabanında `IF NOT EXISTS`):
- `collections(id INTEGER PK, name TEXT NOT NULL, position INTEGER NOT NULL, created_at INTEGER NOT NULL)`. Ad kırpılır, boş olamaz, büyük-küçük harfe duyarsız olarak tekildir. Yeni koleksiyon sona eklenir.
- `collection_entries(collection_id INTEGER NOT NULL, platform TEXT NOT NULL, platform_id TEXT NOT NULL, PRIMARY KEY(collection_id, platform, platform_id))`. Koleksiyon silinince satırları da silinir.
- Tarama `collection_entries`'e dokunmaz. Bir kayıt kütüphaneden düşerse satırı kalır, oyun geri gelince üyelik de geri gelir.

**Komutlar** (`commands.rs`):
- `get_collections() -> [{id, name, entries:[EntryKey]}]`
- `create_collection(name) -> id`
- `rename_collection(id, name)`
- `delete_collection(id)`
- `set_in_collection(id, entries:[EntryKey], member: bool)` — kartın tüm kayıtları tek seferde gönderilir.
- `get_theme() -> {colors: {key: value}, warnings: [string]}`
- `open_theme_file()` — dosya yoksa varsayılan içerik ve açıklamayla oluşturulur, sonra sistem düzenleyicisiyle açılır.
- `reset_theme()` — dosyayı varsayılan içerikle yeniden yazar.

**Ön yüz**
- **Filtre durumu:** Mevcut `Filter` (`"all" | "installed" | Platform`) ikiye ayrılır:
  - sekme: `"all" | Platform | {collection: id}`
  - ayrı bir `installedOnly: boolean`
  - Mevcut `view(games, filter, query, sort)` buna göre uyarlanır. Arama ve sıralama aynı kalır.
- **Kenar çubuğu:** Mağaza listesi şeride taşınır. Kenar çubuğunda kalanlar:
  - "Kurulu" anahtarı, sayacıyla
  - "Oyun ekle", "Yenile", "Ayarlar"
- **Sekme şeridi:**
  - Her sekmede sayaç olur. Sayı, "Kurulu" açıksa kurulu oyunlara göre hesaplanır.
  - Sayısı 0 olan mağaza sekmesi gizlenir. Koleksiyon sekmeleri 0 olsa da görünür.
  - Aktif sekme gizlenirse `Tümü`'ne dönülür. Bu, kenar çubuğundaki eski "sıfırda kaybolan satır" açığının şeritteki karşılığını da kapatır.
- **Koleksiyon sekmesi:**
  - `+` tıklanınca şeritte satır içi bir ad alanı açılır.
  - Sekmeye sağ tıklayınca "Yeniden adlandır" ve "Sil" çıkar. Silme onay ister; oyunlar kütüphanede kalır.
- **Kart ve satır sağ tık menüsü:** "Koleksiyona ekle ›" alt menüsü olur. Kartın içinde olduğu koleksiyonlar işaretli görünür ve tıklanınca kart çıkarılır. Alt menüde "Yeni koleksiyon…" da bulunur.
- **Liste görünümü:** Mevcut sanal liste (`@tanstack/react-virtual`) satır yüksekliğiyle kullanılır. Seçili oyun ve Vitrin davranışı kart görünümündekiyle aynıdır.
- **Görünüm tercihi:** `settings` tablosunda `ui.view = "grid" | "list"` olarak saklanır.

**Tema anahtarları** (JSON adı → CSS değişkeni):

| JSON | CSS |
|---|---|
| `background` | `--bg` |
| `panel` | `--panel` |
| `raised` | `--raise` |
| `border` | `--line` |
| `text` | `--fg` |
| `textDim` | `--dim` |
| `accent` | `--accent` |
| `accentAlt` | `--accent-alt` |
| `onAccent` | `--on-accent` |
| `installed` | `--installed` |

- Kabul edilen değerler: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()/rgba()/hsl()` (ön yüzde `CSS.supports('color', v)` ile doğrulanır).
- Bilinmeyen anahtar ya da geçersiz değer, değişkeni varsayılanda bırakır ve uyarı satırı üretir.
- Dosya bozuk JSON ise hiçbir renk uygulanmaz ve uyarı gösterilir.
- Varsayılan dosyada her anahtarın açıklaması bulunur. JSON yorum desteklemediği için açıklamalar `"_yardım"` anahtarında durur ve yok sayılır.
- Uyarılar mevcut üst `notice` satırında gösterilir.

## Kurucu kontrolünden sonra netleşenler (Opus'un kararları, kullanıcı itiraz edebilir)
- **Mağaza sekmesi:** Bugünkü kural korunur: kartın herhangi bir kaydı o mağazadaysa kart sekmede görünür. Birleşik bir Steam+Ubisoft kartı iki sekmede de çıkar; mağaza sayaçlarının toplamı "Tümü"ne eşit olmaz.
- **Kurulu + mağaza sekmesi (kullanıcı: "her zaman öncelik Steam"):** Mağaza sekmesindeyken "Kurulu", o mağazanın kaydı kurulu olan kartları gösterir. Steam'den kurulu Watch_Dogs "Ubisoft + Kurulu"da görünmez, "Steam + Kurulu"da görünür. "Tümü" ve koleksiyon sekmelerinde kartın herhangi bir kaydının kurulu olması yeterlidir. Sayaçlar da aynı kurala göre hesaplanır.
- **Steam önceliği:** Bir kartta birden çok kurulu kopya varsa ve kullanıcı bir tercih seçmemişse "Oyna" Steam kopyasını açar. Bugünkü "en son oynanan kazanır" kuralı (`db.rs` `default_entry`), Steam dışındaki kopyalar arasında geçerli kalır. Rozet sırasında Steam zaten ilk sırada.
- **Koleksiyon üyeliği:**
  - Kartın herhangi bir kaydı koleksiyondaysa kart üye sayılır ve menüde işaretli görünür.
  - Ekleme kartın o anki tüm kayıtlarını yazar; çıkarma tüm kayıtlarını siler.
  - Sonradan gelen yeni bir mağaza kopyası ayrıca yazılmaz; kart eski kaydı üzerinden üye kalır.
- **Kalıcılık:** Aktif sekme ve "Kurulu" durumu `settings` tablosunda (`ui.tab`, `ui.installed`) saklanır. Kayıtlı sekme artık yoksa `Tümü` açılır.
- **Tema yeniden okuma:**
  - Galaxy'nin 10 sn'lik sınırına bağlı değildir; her odakta yapılır. Okuma ucuzdur.
  - Uyarı yalnızca uyarı listesi değişince gösterilir.
  - "Varsayılana dön" dosyayı yazar ve renkleri hemen yeniden uygular.
- **Varsayılan renkler:** Değerlerin kaynağı `tokens.css`'tir. Rust şablonu aynı değerleri içerir ve bir test, `tokens.css`'teki 10 değerle şablonun aynı olduğunu doğrular. `accent`'ten türetilmiş sabit rgba renkleri varsa `color-mix(... var(--accent) ...)`'e çevrilir; böylece accent değişince parıltılar da değişir.
- **Alt menü:** Bağlam menüsüne açılır alt menü desteği eklenir. Menü ve alt menü pencere kenarına sığacak biçimde konumlanır; bu, bilinen "menü kenardan taşar" açığını da kapatır. Aynı menü liste satırlarında da kullanılır.
- **Koleksiyon sırası ve adı:**
  - `position` yalnızca oluşturma sırasını tutar; yeniden adlandırma ve silme sırayı değiştirmez.
  - Tekillik kontrolü Rust'ta Türkçe duyarlı küçük harfle yapılır (İ/ı); SQLite NOCASE kullanılmaz.
- **"Yeni koleksiyon…" (sağ tık):** Koleksiyonu oluşturur ve kartı ekler. Aktif sekme değişmez.
- **Silme:** Aktif koleksiyon silinirse `Tümü`'ne dönülür.

## Kurucuya kalanlar
- Şerit, satır ve sekme bileşenlerinin dosya bölümü ve adlandırma.
- Liste satırının tam ölçüleri, ince görsel ayrıntılar (mevcut token'larla, "Kokpit + Vitrin" çizgisinde).
- Satır içi ad alanının klavye davranışı: Enter kaydeder, Esc iptal eder (önerilen).

## Test
- **Çekirdek:**
  - koleksiyon CRUD
  - tekil ad
  - birleştirme ve ayırma sonrası üyeliğin korunması
  - yeniden taramanın üyeliği silmemesi
  - tema dosyası: okuma, bozuk JSON, bilinmeyen anahtar
- **Ön yüz (vitest):**
  - sekme ve Kurulu birleşimi
  - sayaçlar
  - koleksiyon sekmesi filtrelemesi
  - liste ve kart anahtarı
  - geçersiz renk yok sayma
- **Canlı:**
  1. "AC" koleksiyonu oluştur, 3 oyun ekle.
  2. Yeniden tara: üyelik duruyor.
  3. Ubisoft + Kurulu.
  4. Liste görünümü, uygulama yeniden açılınca hatırlanıyor.
  5. `accent`'i değiştir, Hangar'a dön: renk değişti.
  6. Bozuk JSON: varsayılan renkler ve uyarı.
