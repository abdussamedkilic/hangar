# Hangar — Sürümleri tek kartta toplama · Tasarım

> **REVİZYON 2 (2026-11-13, geçerli olan budur).** Kullanıcı ilk uygulamayı (d23fb2d) gördü ve Remastered ile Orijinal'in aynı karta düşmesini reddetti. Ardından konsey toplandı ve kullanıcı bir karar verdi. Aşağıdaki kurallar bu belgenin geri kalanında farklı yazan her şeyin yerine geçer.
>
> **Kart kuralı:** Bir kart, bir yapıya karşılık gelir.
> - Kart anahtarı:
>   - etiket "Orijinal" ise `normalize(başlık)` (mevcut `games.norm_title` korunur);
>   - değilse `normalize(başlık) + "|" + etiket`.
> - **Ayrı karta düşen ekler:** sürüm ekleri ve paket ekleri. Remastered, Definitive, Director's Cut, Enhanced, Anniversary, Complete Edition, GOTY, Deluxe, Gold, Ultimate, Special her biri ayrı kart olur. Kullanıcı (2026-11-13): "BioShock Infinite" ile "BioShock Infinite Complete Edition" ayrı kart.
> - **Aynı karta düşen kopya gürültüsü:** farklı mağazalar, `™`/`®`/`©` farkı ve "- Amazon Prime" eki. "- Amazon Prime" kopya etiketine ("Prime") dönüşür, anahtara girmez.
> - **Aynı mağazadaki gerçek kopya:** mağaza, anahtar ve kopya etiketi aynı olan iki kayıt bugünkü gibi ayrı kart kalır.
> - **Elle birleştir/ayır** tercihleri aynen geçerlidir.
>
> **Kart adı:** `display_base(başlık)`; etiket "Orijinal" değilse sonuna `" " + etiket` eklenir. Örnekler: "BioShock", "BioShock Remastered", "BioShock Infinite Complete Edition".
>
> **Rozetler:**
> - Her mağaza bir kez gösterilir. Sıra: Steam, Epic, GOG, Ubisoft, Riot, Yerel.
> - En fazla 3 rozet; fazlası "+N" olur.
> - Rozet satırı taşmaz.
> - İpucu (`title`) tüm kopyaları listeler.
>
> **Oyna/Kur menüsü:**
> - Satır biçimi: `Oyna|Kur · Mağaza`, ardından kopya etiketi varsa ` (Prime)`. Kart içindeki kayıtların sürüm etiketleri farklıysa (yalnızca elle birleştirmede olur) ` · Etiket` de eklenir.
> - İki satır hâlâ aynı okunuyorsa, altına soluk ikinci satır olarak mağazadaki tam başlık yazılır.
> - Sıra: önce kurulu, sonra tercih edilen, sonra kopya etiketi olmayan, sonra platform önceliği.
>
> **Tercih ve başlatma:**
> - Tercih kayıt bazında saklanır: `"platform:id"`. Eski değer (`"steam"`), o platformdaki ilk kayıt olarak okunur.
> - Başlatma ve tercih komutları isteğe bağlı `platformId` alır.
>
> **Koruma:** Orijinal kartlar eski `norm_title` satırını, kapağını ve tercihini korur. Yeni yapı kartları kendi satırını alır ve kapağını bir kez indirir.
>
> **Test ve onay:**
> - BioShock'un 16 gerçek kaydı için kesin beklenen sonuç: 6 kart.
>   - BioShock 2 kayıt, BioShock Remastered 4, BioShock 2 2, BioShock 2 Remastered 4, BioShock Infinite 1, BioShock Infinite Complete Edition 3.
> - Arayüz yayımlanmadan önce, canlı kütüphane kopyası üzerinde kartların listesi kullanıcıya gösterilir.

Tarih: 2026-11-13 · Durum: onay bekliyor · Kullanıcı yaklaşım A'yı seçti.

## Niyet ve neden

Aynı oyunun farklı sürümleri bugün kütüphanede ayrı kartlar olarak görünüyor ve kullanıcının kafası karışıyor. Örnek: BioShock için 4 ayrı kart var. Steam'de "BioShock" ile "BioShock Remastered", GOG'da "BioShock™" ile "BioShock™ Remastered - Amazon Prime".

Kullanıcı tek bir kart istiyor. Kartta "BioShock" yazacak, sürüm kartın içinden seçilecek.

Bugün bunun iki nedeni var:

1. **Aynı mağazada aynı anahtar kuralı.** "Remastered" sondan atıldığı için orijinal ve Remastered aynı anahtara (`bioshock`) düşüyor. Ama aynı mağazada aynı anahtarlı iki kayıt bilerek birleştirilmiyor. Bu kural, `merge.rs`'teki `group` fonksiyonundaki "dup" kuralı. Sonuçta biri ayrı karta itiliyor.
2. **"- Amazon Prime" eki atılmıyor.** Bu ekin Galaxy'den gelen GOG başlıklarında 75 kaydı var. Bu yüzden Prime kopyaları her zaman ayrı kart oluyor.

Başarı ölçütü:
- BioShock, BioShock 2 ve BioShock Infinite birer kart oluyor.
- Kart adı sürüm ekleri olmadan yazılıyor ("BioShock").
- Oyna menüsünde sürümler gruplanıyor, her sürümün altında platformlar sıralanıyor.
- Seçilen sürüm ve platform kart için hatırlanıyor.

## Hedefler

1. **Sürüm ve kopya etiketi:** her kaydın başlığından bir sürüm etiketi ("Orijinal", "Remastered", "Complete Edition", "GOTY" vb.) ve isteğe bağlı bir kopya etiketi ("Prime") çıkarılır.
2. **Tek kart:** aynı aileye düşen kayıtlar, aynı mağazadan olsalar bile tek kartta toplanır. Aile, mevcut normalleştirme ve atılan ekler ile belirlenir. İki istisna var:
   - Aynı mağazada **sürümü ve kopya etiketi de aynı** olan iki kayıt gerçekten kopyadır; bugünkü gibi ayrı kalır.
   - Yerel oyunlar yine kendiliğinden birleştirilmez.
3. **Kart adı:** temel ad gösterilir. Ek, `™`, `®` ve sondaki ayraçlar (` -`, `:`) atılır, büyük/küçük harf korunur ("BioShock").
4. **Oyna menüsü sürüme göre gruplanır.** Ana düğme yalnızca birden çok sürüm varsa sürümü de yazar: `Oyna · Remastered · Steam`.
5. **Hatırlanan seçim:** tercih artık "platform" değil **kayıt** (platform + kimlik) olarak saklanır. Eski kayıtlarda yazan platform değeri geçerliliğini korur.
6. **Varsayılan kayıt:**
   1. hatırlanan kayıt;
   2. yoksa kurulu kayıtlar arasından en son oynanan, eşitse platform önceliği (Steam > Epic > Ubisoft > GOG > Riot > Yerel);
   3. hiçbiri kurulu değilse, Orijinal olmayan sürümler önce, sonra platform önceliği.

   Son kural benim kararım; gerekçesi, yeniden yapılmış sürümün çoğu zaman tercih edilen sürüm olması.
7. **Ayrıntı paneli:** platform satırı sürümle birlikte yazılır, örneğin `Remastered: Steam + GOG · Orijinal: Steam`.

## Hedef dışı

- Farklı oyunları "seri" olarak toplamak. BioShock 2 ayrı bir oyundur, ayrı kart kalır.
- Sürümler arası kayıt/oynama süresi birleştirme gibi ayrıntılar.
- Ek listesinin elle düzenlenmesi için bir arayüz.
- Bir sürümü karttan ayrı karta taşımak için yeni bir arayüz. Mevcut "Ayır" ve "Birleştir" geçerli kalır, kayıt bazında çalışmaya devam eder.

## Anti-hedefler

- Mevcut "Ayır" ve "Birleştir" tercihlerinin bozulması.
- Hatırlanan platform tercihlerinin kaybolması.
- Farklı oyunların birleşmesi. Örneğin "BioShock" ile "BioShock 2" asla birleşmez; ek listesi yalnızca sondaki tam kelimeleri atar.
- Rust `merge::normalize` ile TS `normalize`'ın birbirinden ayrılması. CLAUDE.md'de bu kural yazılı.

## Yaklaşımlar

- **A — Tek kart, içinde sürümler (seçildi).** Yukarıdaki tasarım.
- **B — Ayrı kartlar, ızgarada yığın halinde.** Daha küçük bir iş, ama kullanıcının istediği "tek kart" olmuyor.

## Tasarım

### 1. Etiketleme (`merge.rs`)

- `SUFFIXES` listesine şunlar eklenir:
  - kopya eki: `amazon prime` (başındaki ` - ` ayraçla birlikte normalleştirildiğinde sondaki `amazon prime` olur);
  - sürüm ekleri: `enhanced edition`, `special edition`, `gold edition`, `anniversary edition`.
- Yeni saf fonksiyon: `pub fn edition_of(title: &str) -> Edition`. Dönüş tipi: `Edition { label: String, copy: Option<String> }`.
  - `normalize` ile aynı sondan atma döngüsünü kullanır ve atılan ekleri sırasıyla toplar.
  - `amazon prime` → `copy = Some("Prime")`.
  - İlk sürüm eki, okunur haliyle etiket olur. Okunur karşılıklar sabit bir tabloda durur: `remastered` → "Remastered", `complete edition` → "Complete Edition", `goty` ve `game of the year edition` → "GOTY", `director s cut` → "Director's Cut" vb.
  - Sürüm eki yoksa `label = "Orijinal"`.
- Yeni saf fonksiyon: `pub fn display_base(title: &str) -> String`. Orijinal başlıktan `™`/`®`/`©` ile atılan ekleri büyük/küçük harf duyarsız olarak sondan kırpar, sondaki ` -`, `:` ve boşlukları siler. Sonuç boş olursa orijinal başlık döner.
- `normalize`'ın davranışı yalnızca yeni ekler kadar değişir. TS `normalize` aynı listeyle güncellenir ve iki tarafa aynı test tablosu eklenir.

### 2. Gruplama (`merge.rs::group`)

- "Dup" kuralı değişir: aynı mağaza ve aynı `normalize` değeri **yetmez**; `edition_of` sonucu (label + copy) da aynı olmalıdır.
  - Böylece Steam'deki "BioShock" ile "BioShock Remastered" artık aynı aileye girer: ikisinin anahtarı da `bioshock`, ama sürümleri farklı, dolayısıyla dup sayılmazlar.
- Override'lar (birleştir/ayır) aynen çalışır; kayıt kimliğine bağlılar.
- Mevcut kart kimlikleri `games.norm_title` üzerinden korunur. Ailenin anahtarı yine `normalize` değeridir, dolayısıyla BioShock kartı kapağını ve son oynamasını korur. Dup olmaktan çıkan `bioshock#steam:7670` gibi kartlar silinir; bu beklenen bir sonuç.

### 3. Model ve veritabanı

- `Game` yeni alan alır: `display_title`. Kart adı budur ve `display_base(<gösterilecek kaydın başlığı>)` ile hesaplanır. Mevcut `title` alanı eskisi gibi kalır.
  - Bunun yerine `replace_entries` `games.title`'a doğrudan temel adı yazabilir. Hangisinin seçileceği kurucuya bırakılır; seçim plan sırasında yazılır.
- `PlatformEntry` sürümü taşımaz. Sürüm, başlıktan her yerde aynı fonksiyonla türetilir. Arayüz için `Game` içinde `editions: { label, copy, keys[] }` gibi hazır bir gruplama da döndürülebilir; seçim kurucuya bırakılır.
- `games.preferred` TEXT olarak kalır. Yeni değer biçimi `"<platform>:<platform_id>"`; eski değer (`"steam"`) "o platformdaki ilk kayıt" olarak okunur.
  - Rust tarafında `preferred` yeni bir tipe dönüşür: `Preferred::Entry(Platform, String) | Preferred::Platform(Platform)`.
  - TS tarafında `preferred: string | null` olur ve yardımcı bir fonksiyonla çözülür.

### 4. Başlatma

- `library::resolve(game, want: Option<(Platform, String)>)` belirli bir kaydı ya da yukarıdaki varsayılan kuralı uygular.
- `launch` ve `set_preferred` komutları `platform` ile birlikte isteğe bağlı `platformId` alır. Eski çağrılar (yalnızca platform) çalışmaya devam eder.

### 5. Arayüz

- **`GameCard`:** başlık olarak `display_title` gösterilir; platform rozetleri tekilleştirilir (aynı platform bir kez).
- **`PlayButton`:**
  - Menü sürüm başlıklarıyla gruplanır. Tek sürüm varsa bugünkü düz liste kalır.
  - Öğe metni `Oyna · Steam`, kopya etiketi varsa `Kur · GOG (Prime)`.
  - Seçim `onChoose(platform, platformId)` ile yapılır.
- **`Stage`:** başlık `display_title`; platform satırı birden çok sürüm varsa sürümlerle yazılır.
- **Arama:** `display_title` ve tüm kayıt başlıklarında arar.
- **Taslak:**

```
BioShock                                [Oyna · Remastered · Steam ▾]
                                        ┌──────────────────────────┐
                                        │ Remastered               │
                                        │   Oyna · Steam ✓         │
                                        │   Kur · GOG              │
                                        │   Kur · GOG (Prime)      │
                                        │   Kur · Epic             │
                                        │ Orijinal                 │
                                        │   Kur · Steam            │
                                        │   Kur · GOG              │
                                        └──────────────────────────┘
```

## Test ve doğrulama

**`merge.rs`:**
- `edition_of`: Orijinal, Remastered, Complete Edition, GOTY, Prime kopyası ve "Remastered - Amazon Prime" birleşimi.
- `display_base`: `BioShock™ Remastered - Amazon Prime` → `BioShock`, `BioShock Infinite: Complete Edition` → `BioShock Infinite`.
- `group`: BioShock'un 7 kaydı tek kart oluyor. "BioShock 2" ayrı kalıyor. Aynı mağaza, aynı sürüm ve aynı kopya etiketi hâlâ dup sayılıyor. Override'lar korunuyor.

**`library.rs`:**
- `resolve` varsayılan kuralları.
- Eski `preferred` değerinin okunması.

**TS:**
- `normalize` için eşdeğerlik tablosu.
- `defaultEntry`.
- `PlayButton` gruplu menü testleri.

**Elle:** BioShock'ta kart sayısı 3'e iner (BioShock, BioShock 2, BioShock Infinite). Menüden GOG (Prime) seçilince Galaxy açılır. Uygulama yeniden açıldığında seçim hatırlanır.

## Kurucu kontrolünden sonra verdiğim kararlar

- **Kart adı:** ayrı bir `display_title` alanı açılmaz. `replace_entries` `games.title`'a doğrudan temel adı yazar. Böylece `Cover`, `MergePicker` ve aramada ek değişiklik gerekmez. Bölüm 3 ve 5'te geçen `display_title`, `game.title` olarak okunmalıdır.
- **Sürüm bilgisi:** Rust `Game` için hazır bir `editions` listesi döndürür. Her öğe şunları taşır: `label`, `copy` ve sıralı `entries` anahtarları (`[platform, platform_id]`). Ön yüz `edition_of`'un bir kopyasını tutmaz. Sürümlerin sırası: Orijinal olmayanlar önce (etikete göre), Orijinal en sonda.
- **Eski `preferred` değeri** (yalnızca platform), o platformun kayıtları arasında varsayılan kural 3'ün sırasıyla çözülür: Orijinal olmayan önce, sonra kayıt kimliği.
- **Kural 3'te eşitlik:** önce etikete, sonra platform önceliğine bakılır.
- **Aynı mağazadan iki aynı sürüm** ancak elle birleştirilirse aynı kartta olur. Menüde iki "Steam" satırı görünür; bu nadir bir durum ve kabul edilebilir.
- "Ayır"/"Birleştir" bugünkü gibi çalışır, davranışları değişmez.

## Kurucuya bırakılanlar

- Okunur etiket tablosunun tam içeriği.
- CSS ayrıntıları; mevcut menü stilleri kullanılır.
