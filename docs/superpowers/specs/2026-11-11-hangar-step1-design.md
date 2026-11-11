# Hangar Step 1 — tüm oyunlar tek yerde (tasarım)

Tarih: 2026-11-11 · Durum: onay bekliyor · Önceki: `2026-11-09-hangar-design.md` (v1)
Karar süreci: kullanıcının listesi + LLM Council (5 danışman, 3 isimsiz inceleme) + başkan sentezi.

## Amaç ve neden
kullanıcı, bilgisayarındaki her oyunu Hangar'da görüp "Oyna"ya basarak başka bir şey açmadan oynayabilmek istiyor. v1 Steam, Epic ve Ubisoft'u kapsıyor ve Windows'ta doğrulandı. Step 1'in gerçek işi kalan dört kaynak: **GOG**, **kendi istemcisi olan oyunlar (Riot)**, **launcher'a bağlı olmayan yerel oyunlar**, **elle ekleme**.

Başarı: her kaynaktan en az bir oyun kütüphanede tek kart olarak görünür ve "Oyna" onu başlatır (GOG hariç: aşağıdaki doğrulama notu).

## Hedefler
1. **Oyun ekle (elle):** exe seç + ad yaz → kütüphaneye eklenir. Düzenlenebilir (ad, exe) ve silinebilir. Yeniden tarama bunlara dokunmaz.
2. **Riot:** `C:\ProgramData\Riot Games\` meta dosyalarından kurulu Riot ürünleri (şu an LoL, TFT) görünür, Riot Client üzerinden başlar.
3. **Yerel tarama:** kullanıcının seçtiği klasörler (ör. `D:\Games`) taranır; bulunanlar **öneri listesi** olarak gösterilir, kullanıcı işaretlediklerini ekler. Otomatik ekleme yok. Bulunamayan oyun için "Oyun ekle"ye yönlendirir.
4. **GOG:** GOG Galaxy ile kurulu oyunlar görünür ve Galaxy üzerinden başlar.
5. Aynı oyun birden fazla kaynaktaysa **tek kart**; birleşik kartta varsayılan her zaman **Steam** (sonra Epic, Ubisoft, GOG, Riot, Yerel).

## Hedef dışı
- Astryx / React 19 geçişi / arayüzün yeniden yazılması (council: beta, getirisi yok; Step 1 sonrası mevcut CSS ile görsel iyileştirme yapılabilir).
- Tüm diski ya da Uninstall kayıtlarını arka planda tarama.
- Battle.net, EA, Xbox vb. başka launcher'lar.
- Oyun süresi takibi, exe'den ikon çıkarma, indirme/güncelleme (Step 2), mağaza (Step 3).

## Anti-hedefler (çalışsa bile başarısızlık sayılır)
- Kütüphanenin oyun olmayan programlarla (Discord, 7-Zip, sürücüler) dolması.
- Elle eklenen bir oyunun yeniden taramada kaybolması veya çoğalması.
- Birleşik kartta Steam yerine başka bir platformun varsayılan olması.
- GOG'un gerçek oyunla denenmeden "bitti" denmesi.

## Kısıtlar
- Mimari değişmez: `crates/hangar-core` (saf Rust, testli) + ince `src-tauri` + React 18 arayüz.
- Cerrahi değişiklik, istenmeyen özellik yok; her madde ayrı dal/PR (squash), her biri kendi doğrulamasıyla.
- Kullanıcı usage limitine yakın: küçük adımlar, en fazla 2-3 Sonnet alt ajan.
- Yeni tablo eklenmeden önce `%APPDATA%\com.hangar.app\hangar.db` elle yedeklenir (`hangar.db.bak-v1`).

## Yaklaşımlar (değerlendirilen)
| Yaklaşım | Artı | Eksi | Karar |
|---|---|---|---|
| A. Her kaynak kendi `LibraryProvider`'ı; Riot/GOG/elle için ortak "exe + argüman" başlatma | Mevcut yapıya oturur, her biri ayrı test edilir | 3 yeni platform değeri (Rust+TS+DB) | **Seçildi** |
| B. Riot/GOG yerine tek "özel launcher" girdisi (exe+arg elle) | En az kod | Kullanıcı LoL'u elle girmek zorunda; istenen "otomatik görme" yok | Reddedildi |
| C. Genel tarama: Uninstall kayıtları + Başlat menüsü + filtreler | En otomatik | Doğruluk ölçütü yok, sürekli yanlış pozitif ayarı | Reddedildi; yerine seçili klasör + öneri listesi |

## Kararlaştırılan tasarım

### Model
- `Platform` enum'una `Gog`, `Riot`, `Local` eklenir (serde: `gog`, `riot`, `local`). `Platform::ALL` öncelik sırası: Steam, Epic, Ubisoft, Gog, Riot, Local. TS `Platform` tipi ve rozetler (`GOG`, `RIO`, `LOC` kısaltmaları mevcut 3 harf düzenine uyar) aynı sırayla güncellenir.
- **Başlatma iki türlü olur:** mevcut URI (`steam://` vb., opener ile açılır) veya **exe + argümanlar** (Rust `std::process::Command`, çalışma dizini = exe'nin klasörü, Hangar beklemeden ayrılır). `PlatformEntry`'ye argüman listesi eklenir; `launch_uri` bir URI değil de exe yolu ise exe yolu olarak başlatılır. Kesin alan adı/şekli inşacının kararı; kural: URI ise opener, değilse exe. Yönetici izni isteyen bir exe (Windows hata 740) için başlatma Windows kabuğu üzerinden (UAC penceresi çıkacak şekilde) yapılır; hata olursa Türkçe tek satır mesaj.

### 1) Oyun ekle (Local, elle)
- Yeni tablo `local_games(id INTEGER PK, title TEXT, exe_path TEXT, args TEXT NULL, source TEXT 'manual'|'scan', added_at INTEGER)`. Tarama bu tabloyu silmez; `LocalProvider` bu tablodan `PlatformEntry` üretir (`platform_id` = tablo id'si).
- Exe diskte yoksa giriş "kurulu değil" sayılır ve Oyna hata mesajı verir (kart kaybolmaz).
- Komutlar: `add_local_game(title, exe_path)`, `update_local_game(id, title, exe_path)`, `remove_local_game(id)`. Exe seçimi için Tauri dialog eklentisi (`tauri-plugin-dialog`, yalnızca dosya açma izni, filtre `.exe`).
- Arayüz: kenar çubuğunda "Oyun ekle" düğmesi → küçük diyalog (exe seç, ad otomatik exe/klasör adından önerilir, değiştirilebilir). Local kartın "…" menüsünde "Düzenle" ve "Kaldır".
- **Birleştirme:** Local girişler başka platformlarla **otomatik birleşmez** (kullanıcı aynı oyunu bilerek eklediyse mevcut "birleştir" eylemiyle elle birleştirir). Steam varsayılan kuralı birleşik kartta da geçerli.

### 2) Riot
- `RiotProvider`: `C:\ProgramData\Riot Games\RiotClientInstalls.json` → `rc_default` (RiotClientServices.exe). `Metadata\<ürün>.<patchline>\<ürün>.<patchline>.product_settings.yaml` içinden `product_install_full_path`; **bu klasör diskte varsa kurulu**. Ad: bilinen ürünler için sabit tablo (`league_of_legends` → "League of Legends", `teamfighttactics` → "Teamfight Tactics", `valorant` → "VALORANT"), bilinmeyen ürün atlanır. PBE patchline'ları adın sonuna " (PBE)" eklenerek ayrı giriş olur.
- Başlatma: exe = `rc_default`, argümanlar `--launch-product=<ürün> --launch-patchline=<patchline>`.
- YAML için tam bir ayrıştırıcı yerine yalnızca gereken üst düzey anahtarı okuyan küçük bir okuyucu yeterli (inşacının kararı). Fixture testleri: gerçek dosyalardan kısaltılmış örnekler.
- Bu makinede beklenen: League of Legends ve Teamfight Tactics görünür, PBE görünmez.

### 3) Yerel tarama (öneri listesi)
- Ayarlar'a "Oyun klasörleri" listesi (ekle/kaldır, klasör seçici) — ayar anahtarı `scan_folders` (JSON dizi).
- "Klasörleri tara" → her seçili klasörün **birinci seviye alt klasörü** bir aday; adayın içinde (en fazla 2 seviye derinlikte) uygun exe'lerin **en büyüğü** önerilen exe. Hariç tutulan exe adları: `unins*`, `*setup*`, `*install*`, `*redist*`, `vc_redist*`, `dxsetup*`, `*crash*`, `UnityCrashHandler*`, `*updater*`. `*launcher*` hariç tutulmaz (bazı oyunların asıl exe'si böyle adlanır). Kesin liste inşacının kararı, testle sabitlenir.
- Zaten bilinen girişlerin `install_path`'i altındaki ya da `local_games`'te olan exe'ler aday olmaz.
- Sonuç bir diyalogda işaretlenebilir liste (ad düzenlenebilir); "Ekle" seçilenleri `local_games`'e `source='scan'` ile yazar. Hiç aday yoksa: "Bulunamadı — Oyun ekle ile elle ekleyebilirsin."

### 4) GOG
- `GogProvider`: `HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<gameId>` anahtarları (`gameName`, `path`, `exe`); Galaxy yolu `HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths\client`. Başlatma: `GalaxyClient.exe /command=runGame /gameId=<id> /path="<path>"`; Galaxy yoksa oyunun `exe`'si doğrudan.
- Bu makinede Galaxy kurulu ama GOG oyunu yok → test yalnızca ayrıştırma mantığını kapsar. **Doğrulama:** kullanıcı GOG'dan ücretsiz bir oyun kurar ve Oyna denenir; o zamana kadar CLAUDE.md'de "doğrulanmadı" olarak kalır.

## Sıra ve her maddenin "bitti" kontrolü
| # | Madde | Bitti sayılır |
|---|---|---|
| 0 | Ortak temel: yeni platformlar + exe başlatma | core testleri (Steam varsayılan, exe/URI ayrımı) geçer |
| 1 | Oyun ekle | Herhangi bir exe eklenir, Oyna açar, düzenle/kaldır çalışır, Yenile sonrası kaybolmaz |
| 2 | Riot | LoL kartı görünür, Oyna Riot Client'ı açıp LoL'u başlatır |
| 3 | Yerel tarama | Seçili klasörde aday listesi gelir, oyun olmayan araçlar listede değil, eklenen oyun başlar |
| 4 | GOG | Fixture testleri geçer; gerçek bir GOG oyunuyla Oyna denenince "doğrulandı" |

Her madde: `cargo test -p hangar-core`, `npx tsc --noEmit`, vitest, `npm run tauri dev` ile kullanıcıyla deneme → ayrı PR, squash.

## İnşacıya bırakılanlar
Alan adları ve tam veri şekli, YAML okuyucu yöntemi, exe hariç tutma listesinin son hali, diyalogların görsel ayrıntısı (mevcut "Kokpit + Vitrin" stiline uyarak), Riot/GOG/Local kartları için kapak bulma (mevcut başlık ile kapak arama akışı kullanılır, bulunamazsa mevcut yer tutucu).
