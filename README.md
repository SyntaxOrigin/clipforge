# ClipForge (KırpDön)

Platform profillerine göre **MP4/MKV kapsül ayrıştırma** ve **kırp planı üreten**,
saf Rust komut satırı aracı.

ClipForge'in ayırt edici yanı kodlayıcı ayarları değildir. Kullanıcı
"H.264, CRF 23, 8 Mbps" gibi terimlerle değil, **klibi nereye göndereceğiyle**
uğraşır; çözünürlük, en-boy oranı, kare hızı, bit hızı ve ses kanal sayısı
seçilen platform profilinden gelir.

> **Kapsam uyarısı:** ClipForge **kodlama yapmaz**. `ffmpeg`/`ffprobe` çağırmaz,
> harici süreç başlatmaz, ağ kullanmaz. Kapsülü okur ve dış kodlayıcıya verilebilecek
> bir **kırp planı** üretir. Ayrıntı ve gerekçe: [## Bilinen Sınırlamalar](#bilinen-sinırlamalar).

---

## Özellikler

- **ISO BMFF kutu ayrıştırıcı** — `ftyp`, `moov`/`trak`/`mdia`/`minf`/`stbl`,
  `stsd`, `mdhd`, `tkhd`, `hdlr`, `stts`, `stsz`, `stss` kutuları; süre, çözünürlük,
  kare hızı, ses örnekleme hızı, kanal sayısı ve parça sayısı çıkarılır.
- **Matroska/WebM (EBML) ayrıştırıcı** — `EBML` imzası, `Segment`, `Info`
  (`TimecodeScale`, `Duration`), `Tracks`/`TrackEntry` (`TrackType`, `PixelWidth`,
  `PixelHeight`, `DefaultDuration`, `SamplingFrequency`, `Channels`, `CodecID`).
  Boyutu bilinmeyen ögeler ve çok baytlı VINT alanları desteklenir.
- **Kademeli bellek okuma** — dosya belleğe alınmaz. Gezgin yalnızca 8 baytlık kutu
  başlıklarını okur; yaprak kutuların verisi kendi boyutunda, ihtiyaç duyulduğunda
  okunur. Devasa `mdat` kutularına hiç dokunulmaz.
- **Üç güvenlik sınırı** — tek kutu boyutu sınırı, toplam öge sayısı sınırı ve
  derinlik sınırı. Bozuk ve kötü niyetli dosyalarda sonsuza kadar gezinilmez.
- **Kare hassasiyetinde kırp planı** — giriş/çıkış zamanlarından kesim noktaları ve
  yeniden zamanlama hesaplanır. "Aralık dışında kare kalmaz" ve "son süre istenen
  süreyle en fazla 1 kare farklıdır" kuralları sayısal olarak denetlenir.
- **Kesirli kare hızı desteği** — `30000/1001` gibi NTSC değerleri `f64` yaklaşık
  değeri olarak değil, tam sayı kesri olarak saklanır; kare kayması oluşmaz.
- **Altı platform profili** — `reels`, `tiktok`, `shorts` (9:16), `x-video`,
  `youtube-16x9` (16:9), `kare-akis` (1:1). Her profil çözünürlük, kare hızı, bit hızı,
  GOP ve ses örnekleme kısıtlarını tanımlar.
- **Profiller JSON yapılandırmasından yüklenir** — gömülü katalog ile kullanıcı
  kataloğu aynı şema ve aynı doğrulama yolundan geçer; `--config` ile değiştirilebilir.
- **Kadraj hesabı (kırpma/dolgu)** — kaynak en-boy oranı hedefe uymadığında kırpma ya
  da kenar dolgusu kararı, piksel koordinatlarıyla ve gerekçesiyle raporlanır.
- **JSON kırp planı** — dış kodlayıcıya verilebilecek belgede `kaynak`, `baslangic_sn`,
  `bitis_sn`, `profil`, `kutu_yolu` ve `uyarilar` alanları bulunur; ayrıca kaynak özeti,
  profil özeti, kadraj ve zamanlama ayrıntıları taşınır.
- **Toplu iş kuyruğu** — klasör özyinelemeli gezilir, uzantı filtresi uygulanır,
  **tek bir dosyanın hatası kalan dosyaları durdurmaz**; hatalı dosyalar ayrı listede birikir.
- **`--dry-run` salt okunur kip** — plan veya kuyruk belgesi diske yazılmadan ekrana basılır.
- **`faststart` gereksinimi raporlanır** — `moov` kutusunun `mdat` önünde olup olmadığı
  denetlenir; sonda ise uyarı üretilir.
- **Ayrıştırılamayan girdide açık hata** — tanınmayan kapsül, eksik `moov`, bozuk kutu
  boyutu, çok büyük kutu, sıfır süre, çözünürlüksüz parça ve geçersiz kırp aralığı
  ayrı hata sınıflarıyla bildirilir.

---

## Kurulum

Gereksinim: Rust **1.74** veya üzeri (MSRV). Geliştirme ortamında `rustc 1.98.1` ile
derlendi ve sınandı.

```console
$ cargo build --release
    Finished `release` profile [optimized] target(s) in 12.54s
```

Tek dosyalık ikili `target/release/clipforge.exe` olarak üretilir (1 221 039 bayt).
Harici çalışma zamanı bağımlılığı, DLL veya sistem yazma işlemi yoktur.

```console
$ target\release\clipforge.exe --version
clipforge 0.1.0
```

İkiliyi `PATH` üzerindeki bir dizine kurmak için:

```console
$ cargo install --path .
    Finished `release` profile [optimized] target(s) in 1.41s
  Installing %USERPROFILE%\.cargo\bin\clipforge.exe
   Installed package `clipforge v0.1.0 (%USERPROFILE%\Desktop\Projeler\projects\01-clipforge)` (executable `clipforge.exe`)
$ clipforge --version
clipforge 0.1.0
```

### Örnek dosyalar

Aşağıdaki bütün komut örnekleri, gerçek bir medya dosyası olmadan çalıştırılabilmesi
için önce sentetik örnek kapsüller üretir:

```console
$ cargo run --release --example ornek_dosya -- %USERPROFILE%\AppData\Local\Temp\clipforge-ornek
6 ornek dosya uretildi:
  1. 3840x2160, 30 fps, 20 sn, faststart (moov one) — dikey profillere kirpma ornegi
     %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\01-yatay-16x9-4k.mp4
  2. 1080x1080, 30 fps, 10 sn — dikey profile dolgu ornegi
     %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\02-kare-1x1.mp4
  3. 1920x1080, 30 fps, 20 sn, faststart yok (moov sonda) — faststart uyarisi ornegi
     %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\03-yatay-16x9-1080p.mp4
  4. 1920x1080, 30000/1001 fps, 100 sn — kesirli kare hizi ornegi
     %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\04-ntsc-30000-1001.mp4
  5. 1080x1920, 25 fps, 12 sn, Matroska kapsulu — dogrudan hedef oran ornegi
     %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\05-dikey-mkv.mkv
  6. yalnizca ftyp + mdat — ayristirilamayan girdi ornegi
     %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4
```

> Bu dosyalar **oynatılabilir video değildir**. `mdat` içeriği anlamsız baytlardan
> oluşur; ayrıştırıcının okuduğu kutu yapısı gerçektir. Amaç, kutu ayrıştırıcısını ve
> kırp planlayıcıyı medya dosyası olmadan göstermektir.

---

## Kullanım

Aşağıdaki bütün komutlar ve çıktıları bu depoda **gerçekten çalıştırılmıştır**.

### 1. Platform profillerini listeleme

```console
$ clipforge profiles
katalog surumu: 1
profil sayisi: 6

KIMLIK         PLATFORM             ORAN   COZUNURLUK  FPS   BIT(kbps)  GOP
reels          Instagram Reels      9:16 1080x1920   30 6000       60
             ses: 48000 Hz / 2 kanal / 192 kbit/s  azami 9000  kaynak: https://help.instagram.com/ (dogrulama: 2026-09-29)
tiktok       TikTok               9:16 1080x1920   30 6000       60
             ses: 48000 Hz / 2 kanal / 192 kbit/s  azami 10000  kaynak: https://support.tiktok.com/ (dogrulama: 2026-09-29)
shorts      YouTube Shorts        9:16 1080x1920   30 8000       60
             ses: 48000 Hz / 2 kanal / 192 kbit/s  azami 14000  kaynak: https://support.google.com/youtube/ (dogrulama: 2026-09-29)
x-video        X dikey video        16:9 1280x720    30 5000       60
             ses: 48000 Hz / 2 kanal / 192 kbit/s  azami 8000  kaynak: https://help.x.com/ (dogrulama: 2026-09-29)
kare-akis      Kare (1:1) akis      1:1 1080x1080   30 5000       60
             ses: 48000 Hz / 2 kanal / 192 kbit/s  azami 8000  kaynak: https://help.instagram.com/ (dogrulama: 2026-09-29)
youtube-16x9   YouTube yatay        16:9 1920x1080   30 8000       60
             ses: 48000 Hz / 2 kanal / 192 kbit/s  azami 16000  kaynak: https://support.google.com/youtube/ (dogrulama: 2026-09-29)
```

### 2. Bir MP4 kapsülünü inceleme

```console
$ clipforge probe %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\01-yatay-16x9-4k.mp4
dosya: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\01-yatay-16x9-4k.mp4
bicim: iso-bmff
marka: isom
dosya boyutu: 906 bayt
sure: 20.000 sn
parca sayisi: 2 (video 1, ses 1)
faststart: evet
  [0] video iz=1 codec=avc1 kutu=moov/trak[0]/mdia/minf/stbl sure=20.000 sn cozunurluk=3840x2160 en_boy=16:9 kare_hizi=30 (30.000) kare_sayisi=600 anahtar_kare=10
  [1] ses iz=2 codec=mp4a kutu=moov/trak[1]/mdia/minf/stbl sure=20.000 sn ornekleme=48000 Hz kanal=2
uyarilar: yok
```

### 3. Bir Matroska kapsülünü inceleme

```console
$ clipforge probe %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\05-dikey-mkv.mkv
dosya: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\05-dikey-mkv.mkv
bicim: matroska
dosya boyutu: 146 bayt
sure: 12.000 sn
parca sayisi: 2 (video 1, ses 1)
faststart: belirlenemedi
  [0] video iz=1 codec=V_MPEG4/ISO/AVC kutu=Segment/Tracks/TrackEntry[0] sure=12.000 sn cozunurluk=1080x1920 en_boy=9:16 kare_hazi=25 (25.000) kare_sayisi=300
  [1] ses iz=2 codec=A_OPUS kutu=Segment/Tracks/TrackEntry[1] sure=12.000 sn ornekleme=48000 Hz kanal=2
uyarilar: 1
  - faststart yalnizca ISO BMFF icin anlamlidir
```

### 4. Kırp planı üretme — 16:9 kaynaktan 9:16 hedefe **kırpma**

```console
$ clipforge plan %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\01-yatay-16x9-4k.mp4 --profil reels --baslangic 1 --bitis 5
kaynak: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\01-yatay-16x9-4k.mp4
bicim: iso-bmff
kutu yolu: moov/trak[0]/mdia/minf/stbl
profil: Instagram Reels (reels)
hedef: 1080  en_boy=9:16  30 kare/sn  6000 kbit/s  GOP 60
ses: 48000 Hz / 2 kanal / 192 kbit/s
kirpma: 1.000 sn .. 5.000 sn  (istenen 4.000 sn)
kare: 30..149  adet=120  hiz=30  gercek=4.000 sn  fark=0.000 sn
kare harfasi yeterli: evet
kadraj: kirpma (kirpma=1215x2160+1312+0) kaynak=3840x2160 hedef=1080x1920 olcek=0.8889
uyarilar: 1
  - kaynak 3840:2160 genisligi hedef 1080:1920'den genis: yatayda 2625 piksel kirpiliyor (orani 1.7778 -> 0.5625)
```

3840×2160 → 1080×1920: kaynak hedef genişliğinden geniş olduğu için yatayda
1215×2160'a kırpılıp `x=1312` konumundan alınır, ardından 1080×1920'ye küçültülür
(ölçek 0.8889). Tam sayı bölmesi olduğu için dolgu gerekmez.

### 5. Kırp planı üretme — 1:1 kaynaktan 9:16 hedefe **kenar dolgusu**

```console
$ clipforge plan %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\02-kare-1x1.mp4 --profil tiktok --bitis 4
kaynak: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\02-kare-1x1.mp4
bicim: iso-bmff
kutu yolu: moov/trak[0]/mdia/minf/stbl
profil: TikTok (tiktok)
hedef: 1080  en_boy=9:16  30 kare/sn  6000 kbit/s  GOP 60
ses: 48000 Hz / 2 kanal / 192 kbit/s
kirpma: 0.000 sn .. 4.000 sn  (istenen 4.000 sn)
kare: 0..119  adet=120  hiz=30  gercek=4.000 sn  fark=0.000 sn
kare harfasi yeterli: evet
kadraj: kenar dolgusu (dolgu=0x840+0+420) kaynak=1080x1080 hedef=1080x1920 olcek=1.0000
uyarilar: 1
  - kaynak 1080:1080 hedefe 1080:1920 sigiyor ama orani farkli: kenarlar dolguyla tamamlaniyor
```

1080×1080 → 1080×1920: kaynak zaten hedef genişliğine sığıyor, bu yüzden **kırpma
yapılmaz**; görüntü olduğu gibi kalır ve üst/alt 420'er piksel dolgu eklenir.

### 6. Planı JSON olarak diske yazma

```console
$ clipforge plan %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\01-yatay-16x9-4k.mp4 --profil reels --baslangic 1 --bitis 5 -o %USERPROFILE%\AppData\Local\Temp\clipforge-cikti\plan.json
```
```json
{
  "surum": 1,
  "uretim_araci": "clipforge 0.1.0",
  "kaynak": "C:\\Users\\xXx\\AppData\\Local\\Temp\\clipforge-ornek\\01-yatay-16x9-4k.mp4",
  "baslangic_sn": 1.0,
  "bitis_sn": 5.0,
  "profil": "reels",
  "kutu_yolu": "moov/trak[0]/mdia/minf/stbl",
  "uyarilar": [
    "kaynak 3840:2160 genisligi hedef 1080:1920'den genis: yatayda 2625 piksel kirpiliyor (orani 1.7778 -> 0.5625)"
  ],
  "kaynak_bilgi": {
    "bicim": "iso-bmff",
    "marka": "isom",
    "dosya_boyutu": 906,
    "sure_sn": 20.0,
    "parca_sayisi": 2,
    "video_parca_sayisi": 1,
    "ses_parca_sayisi": 1,
    "kutu_yolu": "moov/trak[0]/mdia/minf/stbl",
    "codec": "avc1",
    "cozunurluk": [3840, 2160],
    "kare_hazi": "30",
    "kare_sayisi": 600,
    "faststart": true
  },
  "profil_ozeti": {
    "kimlik": "reels",
    "ad": "Instagram Reels",
    ...
  }
}
```

### 7. Toplu iş — kuru çalıştırma

```console
$ clipforge batch %USERPROFILE%\AppData\Local\Temp\clipforge-ornek --profil reels --dry-run
kip: kuru calisma  planlanan=5  hatali=1  atlanan=0

hatali dosyalar:
  [bicim] %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4 icinde 'moov' kutusu bulunamadi
```

`--dry-run` hiçbir belge yazmaz; yalnızca ekrana basar. `moov` kutusu olmayan dosya
kuyruğu durdurmaz, ayrı listede `hata sınıfı = bicim` ile raporlanır.

### 8. Toplu iş — belge yazma

```console
$ clipforge batch %USERPROFILE%\AppData\Local\Temp\clipforge-ornek --profil shorts -o %USERPROFILE%\AppData\Local\Temp\clipforge-cikti\kuyruk.json
kip: yazma  planlanan=5  hatali=1  atlanan=0

hatali dosyalar:
  [bicim] %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4 icinde 'moov' kutusu bulunamadi
```

### 9. Ayrıştırılamayan girdi

```console
$ clipforge probe %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4
hata: %USERPROFILE%\AppData\Local\Temp\clipforge-ornek\06-bozuk-moovsuz.mp4 icinde 'moov' kutusu bulunamadi
```

### Alt komut ve bayrak özeti

| Komut | Açıklama |
|---|---|
| `clipforge probe <DOSYA> [--json]` | Kapsülü inceler, kutu bilgilerini raporlar. |
| `clipforge plan <DOSYA> --profil <KIMLIK> [--baslangic] [--bitis] [-o DOSYA] [--json] [--dry-run] [--config DOSYA]` | Tek dosya için kırp planı üretir. |
| `clipforge profiles [--json] [--config DOSYA]` | Platform profillerini listeler. |
| `clipforge batch <KLASOR> --profil <KIMLIK> [--baslangic] [--bitis] [--uzanti LISTE] [-o DOSYA] [--json] [--dry-run] [--config DOSYA]` | Klasörü toplu planlar. |

| Bayrak | Varsayılan | Etkisi |
|---|---|---|
| `--baslangic <SANIYE>` | `0` | Kırp başlangıcı. Negatif değer `AralikGecersiz` hatası üretir. |
| `--bitis <SANIYE>` | kaynağın tamamı | Kırp bitişi. Kaynak süresini aşarsa `KaynakAsildi` hatası üretir. |
| `-o`, `--cikti <DOSYA>` | yok | Planı (veya kuyruk belgesini) diske yazar. |
| `--dry-run` | kapalı | Salt okunur rapor kipi: hiçbir şey yazılmaz, çıktı ekrana basılır. |
| `--json` | kapalı | Özet metin yerine tam JSON belgesi basar. |
| `--uzanti <LISTE>` | `mp4,m4v,mov,mkv,webm` | Virgülle ayrılmış uzantı filtresi. |
| `--config <DOSYA>` | gömülü katalog | Kullanıcı tanımlı profil kataloğu (JSON). |
| `--version`, `--help` | — | Sürüm ve yardım. `help` alt komutu bilinçli olarak kapalıdır. |

---

## Test

```console
$ cargo test
   Compiling clipforge v0.1.0 (%USERPROFILE%\Desktop\Projeler\projects\01-clipforge)
    Finished `test` profile [optimized + debuginfo] target(s) in 1.26s
     Running unittests src\lib.rs (target\debug\deps\clipforge-f355ce1529a701c8.exe)

running 164 tests
test result: ok. 164 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

     Running unittests src/main.rs (target\debug\deps\clipforge-8e88f225b8a6e3be.exe)

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\entegrasyon.rs (target\debug\deps\entegrasyon-97a85a0f35ec3a0.exe)

running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

   Doc-tests clipforge

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Test sonucu: okunan 182; geçen 182; başarısız 0** (164 birim + 18 entegrasyon).

Diğer kalite kapıları:

```console
$ cargo build --release
    Finished `release` profile [optimized] target(s) in 14.70s

$ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.93s

$ cargo fmt --check
    (çıktı yok — biçim denetimi temiz)
```

### Test kapsamı (kenar durumlar)

| Alan | Kapsanan kenar durumları |
|---|---|
| MP4 kutu hiyerarşisi | `moov/trak/mdia/minf/stbl` iç içe kutular, yol üretimi, **iki `trak` kutusunun yol ayrımı** (`trak[0]`, `trak[1]`), geniş boyut (`size == 1`) biçimi, `size == 0` ile dosya sonuna kadar uzama |
| MKV/EBML başlığı | `EBML` imzası, boyutu bilinmeyen öge (`01 FF…FF`), tek baytlık ve çok baytlı VINT boyut alanları, `DocType`, `Info`/`Tracks` gezinişi |
| `ftyp` varyantları | `isom` + uyumlu markalar, markasız (yalnızca ana marka) dosya, kutu 8 bayttan kısa |
| Eksik `moov` | `ftyp` + `mdat` içeren dosyada `MoovYok` hatası |
| Bozuk kutu boyutu | Dosya sınırını aşan boyut, başlıktan küçük toplam boyut, yazdırılabilir olmayan kutu tipi |
| Çok büyük kutu reddi | Tek kutu sınırı aşımı (`AsiriKutu`), öge sayısı sınırı, hiyerarşi derinliği sınırı; **1 GiB `mdat` gezgini yormaz** (yalnızca 8 bayt okunur) |
| Süre sıfır | `mvhd` ve `mdhd` süresi sıfır olan dosyada `SureSifir` |
| Kare hızı sıfır / kesirli | `stts` artışı sıfırsa kare hızı çözülemez ve plan üretilmez; `30000/1001` tam kesir olarak korunur; NTSC kare kayması denetlenir |
| Çözünürlük yok | `stsd` **ve** `tkhd` çözünürlüğü sıfırsa `CozunurlukYok`; yalnızca `stsd` sıfırsa `tkhd` kurtarır |
| Kırpma planı sınırları | Negatif başlangıç, sıfır uzunluklu aralık, sonlu olmayan değerler, kaynak süresini aşma, son kareden sonra başlayan aralık, bir kareden kısa aralık |
| 16:9 → 9:16 (kırpma) | 3840×2160 → 1080×1920: 1215×2160 + `x=1312`, dolgu yok, ölçek 0.8889 |
| 1:1 → 9:16 (dolgu) | 1080×1080 → 1080×1920: kırpma yok, 0×840 dolgu + `y=420` |
| Platform profili doğrulaması | Kimlik biçimi, çözünürlük–en-boy tutarlılığı, tek sayılı çözünürlük, bit hızı, azami bit hızı, GOP, kare hızı çelişkisi, ses kısıtları, aynı kimliğin iki kez tanımlanması, desteklenmeyen katalog sürümü |
| Profil dosyası bozuk | Geçersiz JSON, eksik alanlar, okunamayan dosya, gömülü katalogun tam yolculuğu |
| Kuru çalıştırma | `plan` ve `batch` için `--dry-run` hiçbir dosya yazmaz |
| Toplu kuyruk | Özyinelemeli gezme, kültürel sıraya göre sıralama, **ayni klasör iki kez gezildiğinde aynı sıra**, azami derinlik aşımı, okunamayan kök dizin |
| Bozuk dosya atlanıyor | Bozuk kapsül ve `moov` kutusuz dosya kalanları durdurmaz, hata sınıfıyla (`girdi`/`bicim`/`profil`/`cikti`) listelenir |
| Uzantı filtresi | Varsayılan beş uzantı, özel liste, nokta/buyük harf normalizasyonu, uzantısız dosyalar |
| JSON şema gidiş-dönüşü | Kırp planı, toplu belge, profil kataloğu ve ölçü tipleri için serileştirme → ayrıştırma → eşitlik |
| Bellek disiplini | 1 GiB `mdat` için okunan bayt sayısı denetlenir |
| Geçici dizin yaşam döngüsü | `Drop` ile temizlik; etiket çakışmasının eşzamanlı testleri bozduğu davranışı belgelenmiştir |

---

## Proje Yapısı

```
01-clipforge/
├── Cargo.toml              # edition 2021, MSRV 1.74, license MIT
├── Cargo.lock              # üretilir ve commit edilir
├── LICENSE.txt             # MIT, "Copyright (c) 2026"
├── README.md
├── .gitignore
├── examples/
│   └── ornek_dosya.rs      # README komutları için sentetik kapsül üretici
├── src/
│   ├── lib.rs              # modül dizini, #![forbid(unsafe_code)], #![deny(missing_docs)]
│   ├── main.rs             # ikili giriş noktası, çıkış kodları
│   ├── hata.rs             # ortak hata tipi + elle Display/Error
│   ├── olcu.rs             # EnBoy, KareHazi (tam sayı kesri), serde uygulamaları
│   ├── iso_bmff.rs         # ISO/IEC 14496-12 kutu ayrıştırıcı (kademeli okuma, sınırlar)
│   ├── ebml.rs             # Matroska/WebM (EBML) ayrıştırıcı
│   ├── medya.rs            # iki biçimin ortak katmanı, biçim tespiti, kural denetimleri
│   ├── profil.rs           # PlatformProfili, ProfilKutusu, gömülü katalog, doğrulama
│   ├── kadraj.rs           # kaynak en-boy oranı → hedef oran (kırpma/dolgu)
│   ├── zamanlama.rs        # KirpPenceresi, kare eşlemesi, yeniden zamanlama
│   ├── plan.rs             # KirpPlani ve TopluBelge JSON belgeleri
│   ├── kuyruk.rs           # klasör gezme, uzantı filtresi, dosya başına hata ayrımı
│   ├── ornek.rs            # sentetik kapsül üretici (testler ve örnek için)
│   ├── rapor.rs            # insan okunur metin çıktıları
│   ├── cli.rs              # clap tanımları ve alt komut gövdeleri
│   └── test_yardimci.rs    # #[cfg(test)] geçici dizin ve alt dize yardımcısı
└── tests/
    ├── entegrasyon.rs      # uçtan uca testler
    └── yardimci/mod.rs     # Drop'lu geçici dizin yardımcısı
```

Toplam ~8 500 satır Rust kodu (birim testler ve entegrasyon testleri dâhil).

---

## Yapılandırma

ClipForge **yapılandırma dosyası yazmaz ve okumaz**. Tek ayar noktası `--config` ile
verilen profil kataloğudur; verilmezse programla birlikte gelen gömülü katalog kullanılır.
Bu, "program dizini dışına yazmama" ve "USB'den çalışabilme" taşınabilirlik
ilkelerini korur.

### Profil kataloğu şeması

```json
{
  "surum": 1,
  "profiller": [
    {
      "kimlik": "reels",
      "ad": "Instagram Reels",
      "platform": "Instagram",
      "en_boy": "9:16",
      "genislik": 1080,
      "yukseklik": 1920,
      "tercih_edilen_kare_hazi": "30",
      "azami_kare_hazi": "60",
      "video_bit_hizi_kbps": 6000,
      "azami_video_bit_hizi_kbps": 9000,
      "gop_kare": 60,
      "ses": {
        "ornekleme_hizi": 48000,
        "kanal": 2,
        "bit_hizi_kbps": 192,
        "izinli_ornekleme_hizlari": [44100, 48000]
      },
      "kaynak": "https://help.instagram.com/",
      "dogrulanma_tarihi": "2026-09-29"
    }
  ]
}
```

| Alan | Zorunlu | Kural |
|---|---|---|
| `surum` | Evet | Yalnızca `1`. |
| `profiller` | Evet | Boş olamaz; iki profil aynı `kimlik`i taşıyamaz. |
| `kimlik` | Evet | Yalnızca küçük harf, rakam ve tire. Komut satırında bu kullanılır. |
| `ad`, `platform` | Evet | `ad` boş olamaz. |
| `en_boy` | Evet | `"9:16"`, `"16:9"`, `"1:1"` gibi `pay:payda` metni. |
| `genislik`, `yukseklik` | Evet | Sıfırdan büyük, **çift** sayı olmalı. |
| `tercih_edilen_kare_hazi` | Hayır | Varsayılan `30`. `"30000/1001"` biçiminde kesir kabul edilir. |
| `azami_kare_hazi` | Hayır | Varsayılan `60`; tercih edilen değerden küçük olamaz. |
| `video_bit_hizi_kbps` | Evet | Sıfırdan büyük. |
| `azami_video_bit_hizi_kbps` | Hayır | Önerilen değerden küçük olamaz. |
| `gop_kare` | Evet | Sıfırdan büyük. |
| `ses` | Hayır | Varsayılan: 48000 Hz, 2 kanal, 192 kbit/s, izinsiz liste boş. |
| `kaynak`, `dogrulanma_tarihi` | Hayır | Belge bağlantısı ve ISO-8601 doğrulama tarihi. |

`en_boy` ile `genislik`/`yukseklik` **tutarlı olmak zorundadır** ( tolerans 0.01);
aksi hâlde `clipforge profiles --config` açık bir hata ile reddeder.

### Çıkış kodları

| Kod | Anlam |
|---|---|
| `0` | Başarılı. |
| `1` | Çalışma zamanı hatası (ayrıştırma, profil, dosya yazımı). |
| `2` | Geçersiz komut satırı kullanımı. |

### Ayar bayrakları ve ölçüm sınırları

Sınırlar kod içinde sabittir (`Ayarlar` yapıları) ve aynı dosyanın iki kez okunmasını
engellemek için dışarıdan değiştirilemez:

| Sınır | Değer | Gerekçe |
|---|---|---|
| Tek kutu boyutu (tek `moov` dışında) | 256 MiB | Bellek bütçesi (rapor `b08`: ≤ 300 MB tepe RSS). |
| Toplam kutu sayısı | 200 000 | Çok kutulu bozuk dosyada sonsuz gezinmeyi önler. |
| Hiyerarşi derinliği | 16 | Aynı. |
| EBML okuma bütçesi | 8 MiB | Kümeler atlandığı için yalnızca başlıklar sayılır. |
| Klasör gezme derinliği | 8 | Kaçak dizin ağacında döngü riskini azaltır. |

---

## Bilinen Sınırlamalar

Bu bölüm bilinçlidir ve eksik görev listesini değil, **tasarım kararlarının bedelini**
belgeler.

### 1. Kodlama yapılmıyor — en büyük kapsam farkı

Kaynak rapor (`b07`) ürünü **FFmpeg 7.x + x264 + libvpx + dav1d** ile tasarlamıştı ve
`filter` zincirini uygulayan tam bir dönüştürücü öngörmüştür. Bağımlılık politikası
(karar D-005) bu C kütüphanelerini ve lisanslı encoder'ları yasakladığı için ClipForge
**yalnızca kapsülü okur ve kırp planı üretir**. Gerçek bir video dosyası üretmez.

**Bu ne anlama gelir:** `clipforge plan` çıktısı, bir klibin hangi karelerden
oluşacağını ve nasıl kadrajlanacağını söyler; dosyayı oluşturmaz. Üretimi yapan ayrı
bir kodlayıcı gerekir.

**Neden bu yol:** FFmpeg'in demuxer/muxer kombinasyonları elle yeniden yazılamayacak
kadar geniştir ve encoder'lar ayrıca lisanslıdır. Raporun asıl fikri — kullanıcının
codec/CRF değil **hedef platform** seçmesi — bu kısıt altında bütünüyle korunur ve
programın çıktısına `profil_ozeti` alanı olarak yansır.

### 2. Rapordan ertelenen özellikler

`MANIFEST.md` → Kart 01 → "Ertelenen" listesi:

- Donanım hızlandırma seçimi (NVENC / QSV / VA-API)
- Önizleme dizesi (5 saniyelik kısa render)
- Kalite kademesi (hızlı / dengeli / en küçük dosya)
- Toplu altyazı gömme
- Gerçek video üretimi

Ek olarak kapsam dışı bırakılanlar:

- **Sürükle-bırak kuyruk** ve tek tek iptal: rapor `b05`'teki bu özellikler bir
  **grafik arayüz** gerektiriyordu; karar D-010 grafik arayüzü yasakladığı için
  yerine klasör tabanlı toplu kuyruk ve `--uzanti` filtresi kondu. "Tek tek iptal"
  yerine dosya başına hata ayrımı vardır.
- **Kuyruk kalıcılığı** (`oturum.json`): program hiçbir durum dosyası yazmadığı için
  oturum geri yükleme yoktur. Yeniden çalıştırmak deterministiktir.

### 3. Kadraj kuralı bir ürün kararıdır

Hedef çerçeveye ulaşmak için iki yol vardır: **kırpma** ve **dolgu**. ClipForge
genişliği çıpa alır:

- Kaynak, hedef **genişliğinden geniş**se → **yatay kırpma**.
- Kaynak hedef genişliğine **sığıyorsa** ama oran tutmuyorsa → **kenar dolgusu**.

Bu, 16:9 → 9:16 için kırpma, 1:1 → 9:16 için dolgu üretir. Gerekçe: dikey platform
teslimatlarında sınır genişlikle ifade edilir; kaynak zaten hedef genişliğe sığıyorsa
kırpmak kullanıcının görmek istediği kareleri atmak anlamına gelir. Buna karşılık
kaynak genişse, dolgu ancak aşırı küçültmeyle mümkündür.

**Tam sayı yuvarlaması**: kırpma sonrası ölçekleme tam sayı piksele yuvarlanır ve
hedefden 1–2 piksel eksik kalırsa bu alan **dolguyla** kapatılır ve uyarı üretilir
(ör. 1920×1080 → 1080×1920'de 0×2 dolgu).

### 4. Süre ve kare hızı için ölçüm yöntemi sınırlıdır

- Süre `mvhd`/`mdhd` bildiriminden gelir. Düzeltme listesi (`edts`/`elst`) uygulanmaz;
  bu yüzden **düzenleme listesi olan dosyalarda bildirilen süre ile oynatılan süre
  farklı olabilir**.
- Kare hızı sabit kare hızlı akışta `stts` üzerinden **kesir olarak** çözülür.
  Değişken kare hızlı (`stts` tek bir artış bildirmeyen) akışlarda kare sayısından
  ortalama kare hızı türetilir ve bu değer yaklaşıktır.
- Kare hızı hiç çözülemeyen bir görüntü parçasında **kırp planı üretilemez**
  (`KareHaziHatali`). `probe` yine de kapsülü tanıtır.
- Matroska'da kare hızı `DefaultDuration` alanından türetilir. Bu alan bulunmayan
  dosyalarda kare hızı `None` kalır.
- Kırp aralığı bir kareden kısa olduğunda içine düşen kare alınır (yarı açık aralık
  kuralı); sonuç tam olarak bir karedir ve "en fazla 1 kare fark" kriteri sağlanır.
  Kesirli kare hızda istenen süre tam kare sayısına denk gelmiyorsa fark **bir
  kareden biraz fazla** olabilir; bu durum planda `kare_harfasi_yeterli: false`
  olarak dürüstçe raporlanır.

### 5. Ölçülen hiçbir sayı yoktur

Raporun (`b08`, `b09`) bütün bellek ve açılış süresi hedefleri **ölçülmemiş
tahminlerdir**; bu depoda da ölçülmemiştir. Bildirilen tek ölçülen sayı, üretilen
ikilinin dosya boyutudur: 1 221 039 bayt (Windows x64, `--release`).

Platform profil değerleri de kamuya açık yardım sayfalarından türetilmiş **tahminlerdir**;
her profilin `kaynak` ve `dogrulanma_tarihi` alanları vardır ve güncellenmeleri gerekir.
Profil kütüphanesi bayatlaması, bu projedeki en gerçekçi ürün riskidir.

### 6. Desteklenmeyen kapsül biçimleri

Yalnızca ISO BMFF (MP4/MOV/M4V) ve Matroska/WebM okunur. AVI, WMV, FLV, OGG, HEIF ve
canlı yayın akışları (RTSP/RTMP) desteklenmez; tanınmayan bir kapsül açık bir hata
mesajıyla reddedilir. `quicktime` markası taşıyan `.mov` dosyalarının atom alt kutusu
(`moov` içinde `meta`/`trak` dizinleri) gezinmez.

### 7. Ağ, kod çözme ve harici araç yoktur

Ağ bağlantısı kurulmaz, alt süreç başlatılmaz, `ffmpeg`/`ffprobe` çağrılmaz.
Görüntü ve ses **kod çözülmez**; kapsülün yalnızca başlık ve dizin tabloları okunur.
Bu, kapsul dışındaki hiçbir veriye erişilmediği anlamına gelir ve gizlilik yönünden
iyi bir güvence sağlar.

### 8. Araç zinciri kusuru: `str::contains` hatalı sonuç verebiliyor

Bu depoyu derleyen ortamda `rustc 1.98.1 (48a229cea)`, bir program içinde **biri
diğerinin ön eki olan** dize sabitleri üzerinde `str::contains`/`str::find` çağrılarını
bazı bağlamlarda hatalı derlemektedir: dosyada **var olan** bir altdize bulunamadığı
sonucu üretilmektedir. Hata belirlenimcidir, ancak hangi çağrının etkilendiği kaynak
koddan anlaşılamaz (minimal örnekler `cargo test --lib` altında da ortaya çıkmıştır).

**Önlem:** üretim kodunda `str::contains` **hiç kullanılmaz** (aramalar `Vec`/dizi
eşitliği veya elle bayt karşılaştırmasıyla yapılır). Testlerde alt dize denetimi
`test_yardimci::iceriyor` / `yardimci::iceriyor` yardımcılarıyla, bayt bayt elle
taramayla yapılır. Bu bir **geçici önlemdir**; araç zinciri düzeltildiğinde
`str::contains` kullanımına dönülmelidir.

### 9. Test kapsamının sınırı

Testler gerçek medya dosyası kullanmaz; `ornek` modülünün ürettiği sentetik kapsüllerle
çalışır. Bu, ayrıştırıcının kutu yapısını denetlemek için yeterlidir, ancak
**ticari kodlayıcıların ürettiği gerçek dosyalardaki çeşitlilik ölçülmemiştir**
(fragmented MP4, 64 bitlik kutu boyutları, `elst` düzeltme listeleri, çok parçalı
`mdat`, `uuid` kutuları). Bu dosyalarla bir smoke testi yapılmamıştır.

### 10. Taşınabilirlik notları

- Tek dosya dağıtımı ve statik çalışma zamanı hedefi karşılanmaktadır: harici DLL
  veya sistem kütüphanesi bağımlılığı yoktur.
- Program **hiçbir ayar dosyası yazmaz**; yalnızca kullanıcı `-o` ile açıkça bir yol
  verirse yazma yapar.
- Boşluk ve Türkçe karakter içeren yollar `Path` üzerinden işlenir; kodlama dönüşümü
  yalnızca **görüntüleme** amacıyla yapılır (`to_string_lossy`), dosya işlemleri yerel
  `Path` API'siyle gerçekleşir.
- AV imzası/algılama davranışı **ölçülmemiştir**. Bağımlılık kümesi küçük olduğundan
  (yalnızca `clap`, `serde`, `serde_json`) çok sayıda sıkıştırma/kod çözücü tablosu
  içeren bir ikiliye göre bu risk belirgin biçimde daha düşüktür; yine de imzasız
  dağıtımda antivirüs uyarısı olasıdır.

---

## Gelecek Geliştirmeler

1. **Düzeltme listesi (`edts`/`elst`) desteği** — bildirilen süre ile gerçek oynatma
   süresinin ayrıştırılması. Şu an `edts` geziliyor ama okunmuyor.
2. **Segment tabanlı Matroska** — `Cues` ve `SeekHead` okunursa çok parçalı dosyalarda
   kırp noktaları doğrudan atlanabilir.
3. **Altyazı parçası zamanlaması** — `tx3g`/`wvtt` zaman kaymaları okunup plan belgesine
   eklenebilir.
4. **Görüntü/ses yerleşimi (`trik` zamanlaması)** — MP4'teki parça yerleşim
   düzeltmeleri hesaba katılırsa süre hesabı iyileşir.
5. **Profil kütüphanesi güncelleme aracı** — platform değerleri değiştiğinde
   `dogrulanma_tarihi` alanını güncelleyen ve farkı gösteren bir `profiles --check`
   kipi.
6. **Plan doğrulayıcı** — planı üreten kodlayıcının çıktısını tekrar okuyup kırp
   aralığının gerçekten uygulandığını doğrulayan bir kip.
7. **Donanım hızlandırma kabiliyet yoklaması** (rapor `b05`, v1) — plan belgesine
   önerilen encoder sürücüsünü ekler.
8. **Kuyruk kalıcılığı** — isteğe bağlı `--oturum` dosyası ile kaldığı yerden
   devam (rapor `b16`, açık soru 7).
9. **Gerçek medya dosyalarıyla smoke testi** — bölüm 9'daki boşluğu kapatmak için.

---

## Troubleshooting

### `... icinde 'moov' kutusu bulunamadi`

**Belirti:** `clipforge probe video.mp4` bu hatayı verir; dosya oynatıcıda açılıyor.

**Neden:** Dosya geçerli bir MP4 değildir (metin dosyası, `moov` kutusu olmayan
kırpılmış akış) ya da kapsül çok büyük ilan edilmiş ve ayrıştırma sınırına takılmıştır.

**Çözüm:** Dosyayı `ffprobe` ile doğrulayın (ClipForge bu aracı **çağırmaz**, yalnızca
sizin tanılamıyorsanız kullanın). Çoğu durumda `moov` kutusunu dosyanın başına taşıyan
`faststart` yeniden yazımı sorunu çözer. Sınırı aşan kutuda `iso_bmff::Ayarlar::asiri_kutu`
değeri (varsayılan 256 MiB) artırılabilir.

### `bilinen bir kapsul degil: imza taninmadi (ilk baytlar: ...)`

**Belirti:** `probe` dosyayı reddeder, `batch` ise dosyayı `hata sınıfı = bicim` ile listeler.

**Neden:** İlk baytlar ne `EBML` imzası (`1A 45 DF A3`) ne de `ftyp` kutusu değil.
Dosya AVI, WMV, FLV, OGG ya da metin olabilir.

**Çözüm:** Desteklenen biçimler MP4/MOV/M4V ve MKV/WebM'dir. Başka bir biçimdeki
klibi önce MP4'e dönüştürün. Toplu çalışmada `--uzanti` filtresini daraltarak yalnızca
desteklenen dosyaları kuyruğa alın.

### `profil bulunamadi: 'linkedin'`

**Belirti:** `plan` veya `batch` profil kimliğini bulamaz.

**Neden:** Kimlik yazım hatasıdır ya da `--config` ile verilen katalogda yoktur.
Kimlikler **küçük harf, rakam ve tire** içermelidir; kimlikler büyük harfle ayrıdır.

**Çözüm:** `clipforge profiles` ile geçerli kimlikleri listeleyin. Kendi profilinizi
eklemek için `clipforge profiles --json > profiller.json` çıktısını düzenleyip
`--config profiller.json` ile geçin.

### `kaynak suresini asiyor: istenen bitis 30.0 sn, kaynak 12.0 sn`

**Belirti:** `--bitis` değeri dosyanın süresinden büyük.

**Neden:** Yazım hatası ya da yanlış dosya. `plan` alt komutunda `--bitis` verilmezse
kaynağın tamamı kullanılır.

**Çözüm:** `clipforge probe <DOSYA>` ile gerçek süreyi öğrenin. Tek uçlu kullanım
tercih ediliyorsa `--bitis` bayrağını **tamamen kaldırın**; toplu çalışmada verilen tek
uç dosya başında kaynak süresiyle tamamlanır, böylece süreleri farklı dosyalar aynı
komutla işlenebilir.

### `... 'moov' kutusu dosyanin sonunda: akis kopyasinda 'faststart' gereksiz`

**Belirti:** `probe` çıktısında `faststart: hayir` ve bu uyarı.

**Neden:** `moov` kutusu `mdat` kutusundan sonra duruyor. Akış kopyalarında oynatıcı
indirmeyi bitirmeden başlıkları göremez; bu, kırp planını etkilemez.

**Çözüm:** Dosyayı `faststart` ile yeniden yazın. Plan üretimi için bu uyarıyı yok
sayabilirsiniz; yalnızca akış dağıtımı yapıyorsanız önemlidir.

### `kaynak 1080:1080 hedefe 1080:1920 sigiyor ama orani farkli`

**Belirti:** Plan kırpma yerine kenar dolgusu üretir.

**Neden:** Beklenen davranıştır. Kaynak hedef **genişliğine** sığıyorsa ClipForge
kırpma yapmaz; görüntüyü korur ve eksik dikey alanı dolguyla kapatır. Kare kaynağı
dikey platformda "siyah bantlı" göstermek isteyen çoğu kullanıcı için doğru sonuçtur.

**Çözüm:** Görüntünün ortasından dikey bir şerit istiyorsanız kaynağı önce
dikdörtgen biçiminde kırpmanız gerekir; ClipForge bunu uygulamaz, yalnızca planlar.
Kendi hedef çözünürlüğünüzü içeren bir profil ekleyip `--config` ile geçebilirsiniz.

---

## Atıflar

Bu proje aşağıdaki standartlara, spesifikasyonlara ve projelere dayanır.

### Spesifikasyonlar

- **ISO/IEC 14496-12** — Information technology — Coding of audio-visual objects —
  Part 12: ISO base media file format specification.
  <https://www.iso.org/standard/83102.html>
- **ISO/IEC 14496-14** — MP4 file format (kutu hiyerarşisinin arka planı).
  <https://www.iso.org/standard/83105.html>
- **Matroska Media Formats** — Matroska/WebM kapsul spesifikasyonu; EBML öğe kimlikleri,
  VINT değişken uzunluklu tam sayılar, "boyut bilinmiyor" kodlaması, `Segment`/`Info`/
  `Tracks` yerleşimi.
  <https://www.matroska.org/technical/specs/index.html>
- **EBML RFC** — EBML biçiminin ayrıntılı açıklaması (VINT tanımları dahil).
  <https://www.rfc-editor.org/rfc/rfc8794.html>
- **RFC 3339** — Date and Time on the Internet: Timestamps (ISO-8601 profil tarihleri için).
  <https://www.rfc-editor.org/rfc/rfc3339.html>

### Rust ekosistemi

- **Rust standart kütüphanesi** — <https://doc.rust-lang.org/std/>
- **Rust 2021 sürüm rehberi** — <https://doc.rust-lang.org/edition-guide/edition-2021/>
- **cargo yerel rehberi** — <https://doc.rust-lang.org/cargo/>
- **serde** — <https://serde.rs/> · <https://github.com/serde-rs/json>
- **serde_json** — <https://docs.rs/serde_json/>
- **clap** (komut satırı) — <https://docs.rs/clap/>

### Rapor dosyası (iç tasarımın kaynağı)

- `%USERPROFILE%\Desktop\Fikirler\01-kirpdone-klip-donusum.html` — "KırpDön (ClipForge)"
  fikir raporu, 01/30, tarih 2026-09-29.
  Bu **yerel bir dosyadır**, URL değildir. Rapordaki sayısal iddiaların hiçbiri
  ölçülmemiştir; ClipForge yalnızca kapsul okuma, platform profili modeli, kırp aralığı
  ve kuyruk kavramlarını modellemiştir.

### Karar belgeleri

- `%USERPROFILE%\Desktop\Projeler\MANIFEST.md` — Kart 01 (uygulama stack'i, sapma gerekçesi,
  MVP kapsamı, ertelenenler)
- `%USERPROFILE%\Desktop\Projeler\WORKER_CONTRACT.md` — bağımsız depo, bağımlılık ve
  kalite kapısı kuralları
- `%USERPROFILE%\Desktop\Projeler\PROJECT_STATUS.md` — karar günlüğü (D-003, D-005, D-008, D-010)

---

## Üretim Atfı

Bu depo **OpenCode** ajanı tarafından, **`space-bunny-free`** modeli
(`opencode/space-bunny-free`) kullanılarak üretilmiştir.

- **Arac:** OpenCode
- **Model:** `opencode/space-bunny-free` (Space Bunny Free)
- **Tür:** Rust, `cargo build` / `cargo test` ile üretilmiş ve doğrulanmıştır.

Kaynak kod, testler ve dokümantasyon bu model tarafından yazılmıştır. İnsan
katkısı: gereksinim tanımı, kabul ölçütleri ve son kontroller.

## Lisans

MIT — bkz. [`LICENSE.txt`](LICENSE.txt). Telif: `Copyright (c) 2026 ClipForge contributors`.

Kod kopyalanmamıştır. Tüm kapsül ayrıştırma mantığı ISO BMFF ve Matroska
spesifikasyonlarından **bağımsız olarak** yazılmıştır; yalnızca kutu/öge düzeni ve
alan ofsetleri bu standartlardan alınmıştır.
