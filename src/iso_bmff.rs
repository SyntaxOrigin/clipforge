//! ISO/IEC 14496-12 (ISO Base Media File Format) kutu ayrıştırıcısı.
//!
//! Bu modül yalnızca **kapsul yapısını** okur; görüntü ya da ses verisine
//! dokunmaz ve hiçbir kod çözme yapmaz. `ffmpeg`/`ffprobe` çağrısı yapılmaz.
//!
//! # Bellek disiplini
//!
//! Dosya baştan sona belleğe alınmaz. Yürüyücü her zaman yalnızca 8 baytlık kutu
//! başlığını okur; yaprak kutuların verisi ancak gerçekten bir alan okunacaksa
//! **kendi boyutunda** okunur. Böylece 4 GB'lık bir `mdat` hiç dokunulmadan
//! atlanır, `moov` ise kademeli olarak çözülür.
//!
//! Ayrıştırıcı bozuk ya da kötü niyetli dosyalarda sonsuza kadar gezinmemek için
//! üç sınır uygular: tek kutu boyutu sınırı, toplam öge sayısı sınırı ve
//! derinlik sınırı (bkz. [`Ayarlar`]).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hata::ClipForgeHata;

/// `mdat` dışında kutu sınırlarını doğrulamak için okunan en küçük kutu boyutu.
pub const KUTU_BASLIK_BOYUTU: u64 = 8;

/// Ayar verilmemişse tek bir kutunun geçebileceği üst sınır (256 MiB).
///
/// Uzun kayıtların `moov` kutusu megabaytları bulabilir; bu sınır hem bozuk
/// boyut ilanlarını hem de bellek bütçesini (rapor `b08`: ≤ 300 MB tepe RSS)
/// korumak için vardır.
pub const VARSAYILAN_ASIRI_KUTU: u64 = 256 * 1024 * 1024;

/// Bir dosyada en fazla kaç kutu başlığı okunacağı.
pub const VARSAYILAN_ASIRI_OGE: usize = 200_000;

/// Bir kutu hiyerarşisinde en fazla kaç düzeye ineceği.
pub const VARSAYILAN_ASIRI_DERINLIK: usize = 16;

/// `stsd` içinde okunacak azami örnek girdisi sayısı.
///
/// `stsd` bir ses parçasında birden çok kanal girişi taşıyabilir; kanal sayısı
/// yalnızca ilk girişten okunur, dolayısıyla bu sınır yalnızca belleği korur.
pub const VARSAYILAN_ASIRI_ORNEK: usize = 64;

/// Yürüyücünü zorlayan güvenlik sınırları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ayarlar {
    /// Tek bir kutunun geçebileceği üst sınır (bayt).
    pub asiri_kutu: u64,
    /// Toplam kaç kutu başlığı okunabileceği.
    pub asiri_oge: usize,
    /// Kutu hiyerarşisinin en derin noktası.
    pub asiri_derinlik: usize,
    /// `stsd` içinde okunacak azami örnek girdisi sayısı.
    pub asiri_ornek: usize,
}

impl Default for Ayarlar {
    fn default() -> Self {
        Self {
            asiri_kutu: VARSAYILAN_ASIRI_KUTU,
            asiri_oge: VARSAYILAN_ASIRI_OGE,
            asiri_derinlik: VARSAYILAN_ASIRI_DERINLIK,
            asiri_ornek: VARSAYILAN_ASIRI_ORNEK,
        }
    }
}

/// Yapay olarak daraltılmış sınırlar içeren ayarlar (testlerde kullanılır).
pub fn dar_ayarlar(asiri_kutu: u64) -> Ayarlar {
    Ayarlar {
        asiri_kutu,
        ..Ayarlar::default()
    }
}

/// Bir kutu başlığının çözülmüş hâli.
///
/// `boyut` alanı ISO BMFF'te kutunun **başlık dahil toplam** boyutudur; bu
/// tipte [`KutuBasligi::boyut`] yalnızca veri bölümünü tutar, çünkü gezinme
/// veri ofsetiyle ilgilenir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KutuBasligi {
    /// Kutunun dosya içindeki başlangıç ofseti.
    pub ofset: u64,
    /// Kutu tipi (ISO BMFF'te dört ASCII karakter).
    pub tip: [u8; 4],
    /// Yalnızca veri bölümünün boyutu (başlık dahil değil).
    pub boyut: u64,
    /// Başlığın kapladığı bayt sayısı (8 veya 16).
    pub baslik_boyutu: u64,
    /// Başlıkta `boyut == 0` yazıyorsa kutu dosya sonuna kadar uzanır.
    pub sona_kadar: bool,
}

impl KutuBasligi {
    /// Kutunun veri bölümünün başladığı ofseti döndürür.
    pub fn veri_ofseti(&self) -> u64 {
        self.ofset + self.baslik_boyutu
    }

    /// Kutunun başlıktan sonraki ilk baytını gösteren ofseti döndürür.
    pub fn sonraki_ofset(&self) -> u64 {
        self.veri_ofseti() + self.boyut
    }

    /// Kutu tipini metin olarak döndürür.
    pub fn tip_metni(&self) -> String {
        String::from_utf8_lossy(&self.tip).into_owned()
    }
}

/// Hiyerarşide bulunan bir kutunun konum bilgisi (verisi henüz okunmamıştır).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulunanKutu {
    /// Kökten kutuya giden yol, örneğin `moov/trak/mdia/minf/stbl/stsd`.
    pub yol: String,
    /// Kutu tipi metni.
    pub tip: String,
    /// Kutu başlığının okunduğu ofset.
    pub ofset: u64,
    /// Veri bölümünün boyutu.
    pub boyut: u64,
    /// Başlık boyutu.
    pub baslik_boyutu: u64,
    /// Kutunun ait olduğu `trak` sırası (üst düzey kutular için `None`).
    pub trak: Option<usize>,
}

/// Parçanın türü (`hdlr` işleyicisinden türetilir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParcasiTuru {
    /// `vide` işleyicisi: görüntü parçası.
    Video,
    /// `soun` işleyicisi: ses parçası.
    Ses,
    /// Diğer her işleyici (altyazı, ipucu, zamanlama vb.).
    Diger,
}

impl ParcasiTuru {
    /// İşleyici kodundan türü çözümler.
    pub fn koddan(kod: &[u8; 4]) -> Self {
        match kod {
            b"vide" => Self::Video,
            b"soun" => Self::Ses,
            _ => Self::Diger,
        }
    }

    /// Türün plan çıktısındaki kısa adını döndürür.
    pub fn kod(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Ses => "ses",
            Self::Diger => "diger",
        }
    }
}

/// `ftyp` kutusunun çözülmüş hâli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ftyp {
    /// Ana marka (ör. `isom`, `mp42`, `qt  `).
    pub ana_marka: String,
    /// Sürüm alanı.
    pub kucuk_surum: u32,
    /// Uyumlu marka listesi.
    pub uyumlu_markalar: Vec<String>,
}

/// `mvhd` kutusunun zamanlama alanları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mvhd {
    /// Zaman ölçeği: saniye başına zaman birimi sayısı.
    pub zaman_olcegi: u32,
    /// Ham süre (zaman ölçeği biriminde).
    pub ham_sure: u64,
}

impl Mvhd {
    /// Süreyi saniyeye çevirir.
    pub fn sure_sn(&self) -> f64 {
        if self.zaman_olcegi == 0 {
            return 0.0;
        }
        self.ham_sure as f64 / f64::from(self.zaman_olcegi)
    }
}

/// `mdhd` kutusunun zamanlama alanları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mdhd {
    /// Parçanın zaman ölçeği.
    pub zaman_olcegi: u32,
    /// Ham süre.
    pub ham_sure: u64,
}

impl Mdhd {
    /// Parça süresini saniyeye çevirir.
    pub fn sure_sn(&self) -> f64 {
        if self.zaman_olcegi == 0 {
            return 0.0;
        }
        self.ham_sure as f64 / f64::from(self.zaman_olcegi)
    }
}

/// `stsd` içindeki ilk örnek girdisinden çözülen alanlar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stsd {
    /// Örnek girdisinin dört karakterlik biçim kodu (ör. `avc1`, `mp4a`).
    pub bicim: String,
    /// Video örnek girdisinde kodlanmış genişlik.
    pub genislik: u32,
    /// Video örnek girdisinde kodlanmış yükseklik.
    pub yukseklik: u32,
    /// Ses örnek girdisinde kanal sayısı.
    pub kanal: u16,
    /// Ses örnek girdisinde örnekleme hızı (Hz).
    pub ornekleme_hizi: u32,
    /// Ses örnek girdisinde örnek başına bit.
    pub ornekleme_boyutu: u16,
}

/// `stts` (time-to-sample) tablosunun özeti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stts {
    /// Toplam örnek (kare) sayısı.
    pub toplam_ornek: u64,
    /// Tablodaki giriş sayısı.
    pub giris_sayisi: u32,
    /// Bütün girişlerin artışı tek değer ise o değer, aksi halde `None`.
    pub sabit_artış: Option<u32>,
    /// İlk girişin artışı.
    pub ilk_artış: u32,
}

/// Bir `trak` için toplanan kutu bilgileri.
#[derive(Debug, Clone, PartialEq)]
pub struct ParcaKutusu {
    /// `tkhd` içindeki parça kimliği.
    pub iz: u32,
    /// Parçanın türü.
    pub tur: ParcasiTuru,
    /// Kutu hiyerarşisindeki yolu.
    pub kutu_yolu: String,
    /// `tkhd` içindeki ham süre ve zaman ölçeği yoksa sıfır.
    pub tkhd_sure_sn: f64,
    /// `tkhd` içindeki görüntü genişliği (16.16 sabit nokta).
    pub tkhd_genislik: u32,
    /// `tkhd` içindeki görüntü yüksekliği (16.16 sabit nokta).
    pub tkhd_yukseklik: u32,
    /// `mdhd` kutusu varsa zamanlama alanları.
    pub mdhd: Option<Mdhd>,
    /// `stsd` kutusu varsa örnek girdisi alanları.
    pub stsd: Option<Stsd>,
    /// `stts` kutusu varsa zamanlama özeti.
    pub stts: Option<Stts>,
    /// `stsz` kutusundaki toplam örnek sayısı.
    pub ornek_sayisi: Option<u64>,
    /// `stss` kutusundaki eşzamanlı (anahtar) kare sayısı.
    pub anahtar_kare_sayisi: Option<u32>,
}

impl ParcaKutusu {
    /// Parçanın saniye cinsinden süresini en güvenilir kaynaktan döndürür.
    ///
    /// Sıra: `mdhd` → `tkhd`. `mdhd` medya zamanını, `tkhd` ise düzeltme listesi
    /// uygulanmış süreyi taşır; ikisi de yoksa `0.0` döner.
    pub fn sure_sn(&self) -> f64 {
        if let Some(mdhd) = self.mdhd {
            let sure = mdhd.sure_sn();
            if sure > 0.0 {
                return sure;
            }
        }
        self.tkhd_sure_sn
    }
}

/// Bir MP4/MOV dosyasından toplanan tüm kutu bilgileri.
#[derive(Debug, Clone, PartialEq)]
pub struct KutuVerisi {
    /// `ftyp` kutusu (varsa).
    pub ftyp: Option<Ftyp>,
    /// `moov/mvhd` kutusu (varsa).
    pub mvhd: Option<Mvhd>,
    /// `trak` listesi.
    pub parcalar: Vec<ParcaKutusu>,
    /// `moov` kutusunun ofseti.
    pub moov_ofseti: Option<u64>,
    /// Dosyadaki ilk `mdat` kutusunun ofseti.
    pub ilk_mdat_ofseti: Option<u64>,
    /// Taranan toplam kutu sayısı.
    pub kutu_sayisi: usize,
    /// Taramada okunan toplam bayt sayısı (yalnızca başlıklar ve yaprak alanları).
    pub okunan_bayt: u64,
}

impl KutuVerisi {
    /// `moov` kutusunun ilk `mdat` kutusunun önünde olup olmadığını söyler.
    ///
    /// Bu, `faststart` (ön yükleme dostu yerleşim) gereksinimidir: `moov`
    /// sonda ise oynatıcı indirmeyi bitirmeden başlıkları göremez.
    pub fn faststart(&self) -> Option<bool> {
        match (self.moov_ofseti, self.ilk_mdat_ofseti) {
            (Some(moov), Some(mdat)) => Some(moov < mdat),
            _ => None,
        }
    }
}

/// Kutu tipi metnini dört bayta çevirir.
pub fn tip_donus(metin: &str) -> [u8; 4] {
    let baytlar = metin.as_bytes();
    let mut sonuc = [b' '; 4];
    for (hedef, kaynak) in sonuc.iter_mut().zip(baytlar.iter().take(4)) {
        *hedef = *kaynak;
    }
    sonuc
}

/// Yapay kutuların gezilmesi gereken üst düzey tipler.
const YAPISAL_TIPLER: [[u8; 4]; 7] = [
    *b"moov", *b"trak", *b"mdia", *b"minf", *b"stbl", *b"edts", *b"dinf",
];

/// Kutu okuyan, kademeli çalışan gezgin.
pub struct KutuOkuyucu<R: Read + Seek> {
    ic: R,
    ayarlar: Ayarlar,
    konum: u64,
    okunan: u64,
}

impl KutuOkuyucu<File> {
    /// Verilen yolu açarak bir okuyucu oluşturur.
    ///
    /// # Hatalar
    ///
    /// Dosya açılamazsa [`ClipForgeHata::GirdiHatasi`] döner.
    pub fn ac(yol: &Path, ayarlar: Ayarlar) -> Result<Self, ClipForgeHata> {
        let dosya = File::open(yol).map_err(|hata| ClipForgeHata::girdi(yol, hata))?;
        Ok(Self::yeni(dosya, ayarlar))
    }
}

impl<R: Read + Seek> KutuOkuyucu<R> {
    /// Hazır bir okuyucuyu sarmalar.
    pub fn yeni(ic: R, ayarlar: Ayarlar) -> Self {
        Self {
            ic,
            ayarlar,
            konum: 0,
            okunan: 0,
        }
    }

    /// Uygulanan sınırları döndürür.
    pub fn ayarlar(&self) -> &Ayarlar {
        &self.ayarlar
    }

    /// Şimdiye kadar okunan toplam bayt sayısını döndürür.
    pub fn okunan_bayt(&self) -> u64 {
        self.okunan
    }

    /// İç okuyucunun dosya boyutunu döndürür.
    pub fn dosya_boyutu(&mut self) -> Result<u64, ClipForgeHata> {
        let boyut = self
            .ic
            .seek(SeekFrom::End(0))
            .map_err(|hata| ClipForgeHata::GirdiHatasi {
                yol: std::path::PathBuf::new(),
                hata,
            })?;
        self.konum = boyut;
        Ok(boyut)
    }

    /// Kutunun yeterince büyük olduğunu doğrular.
    ///
    /// Bu denetim olmadan okuma, kutunun **sonraki** kutuların baytlarını sessizce
    /// okurdu. Kutu sınırları ISO BMFF'te kesin bir sözleşmedir, bu yüzden her
    /// yaprak alan okunmadan önce buradan geçirilir.
    fn boyut_kontrol(&self, kutu: &BulunanKutu, gereken: u64) -> Result<(), ClipForgeHata> {
        if kutu.boyut < gereken {
            return Err(ClipForgeHata::EksikKutu {
                yol: kutu.yol.clone(),
                ayrinti: format!(
                    "kutu yalnizca {} bayt, en az {gereken} gerekiyor",
                    kutu.boyut
                ),
            });
        }
        Ok(())
    }

    /// Sürüm baytını okur (zamanlama kutularında 0 ya da 1).
    fn surum_oku(&mut self, kutu: &BulunanKutu) -> Result<u8, ClipForgeHata> {
        let veri = self.veri_oku(kutu.ofset + kutu.baslik_boyutu, 1, &kutu.yol)?;
        Ok(veri[0])
    }

    /// `ofset` konumundaki kutu başlığını okur.
    ///
    /// # Hatalar
    ///
    /// Dosya sonuna ulaşıldıysa, tip baytları yazdırılabilir ASCII değilse,
    /// boyut başlıktan küçükse veya dosya sınırını aşıyorsa hata döner.
    pub fn basligi_oku(
        &mut self,
        ofset: u64,
        dosya_boyutu: u64,
    ) -> Result<KutuBasligi, ClipForgeHata> {
        let baslik = self.veri_oku(ofset, 8, "baslik")?;
        let boyut32 = u32::from_be_bytes([baslik[0], baslik[1], baslik[2], baslik[3]]);
        let tip: [u8; 4] = [baslik[4], baslik[5], baslik[6], baslik[7]];
        if !tip.iter().all(|b| (0x20..=0x7e).contains(b)) {
            return Err(ClipForgeHata::BozukBaslik {
                ofset,
                ayrinti: format!("kutu tipi yazdirilabilir ASCII degil: {tip:?}"),
            });
        }

        let (toplam_boyut, baslik_boyutu, sona_kadar) = match boyut32 {
            1 => {
                let genis = self.veri_oku(ofset + 8, 8, "genis boyut")?;
                (
                    u64::from_be_bytes([
                        genis[0], genis[1], genis[2], genis[3], genis[4], genis[5], genis[6],
                        genis[7],
                    ]),
                    16,
                    false,
                )
            }
            // `size == 0`: kutu dosya sonuna kadar uzanır.
            0 => (dosya_boyutu.saturating_sub(ofset), 8, true),
            diger => (u64::from(diger), 8, false),
        };

        // ISO/IEC 14496-12: `size` alanı **başlığı da içerir** ve başlıktan
        // küçük olamaz. Veri bölümünün boyutu toplamdan başlık düşülerek
        // bulunur.
        if toplam_boyut < baslik_boyutu {
            return Err(ClipForgeHata::GecersizBoyut {
                ofset,
                boyut: toplam_boyut,
                dosya_boyutu,
            });
        }
        let baslik = KutuBasligi {
            ofset,
            tip,
            boyut: toplam_boyut - baslik_boyutu,
            baslik_boyutu,
            sona_kadar,
        };
        self.dogrula_basligi(&baslik, dosya_boyutu)?;
        Ok(baslik)
    }

    /// Başlığı boyut sınırlarına karşı doğrular.
    fn dogrula_basligi(
        &self,
        baslik: &KutuBasligi,
        dosya_boyutu: u64,
    ) -> Result<(), ClipForgeHata> {
        if !baslik.sona_kadar {
            let toplam = baslik.baslik_boyutu + baslik.boyut;
            if baslik.ofset + toplam > dosya_boyutu {
                return Err(ClipForgeHata::GecersizBoyut {
                    ofset: baslik.ofset,
                    boyut: toplam,
                    dosya_boyutu,
                });
            }
        }
        if baslik.boyut > self.ayarlar.asiri_kutu && baslik.tip != *b"mdat" {
            // `mdat` istisnadır: içeriği hiç okunmaz, bu yüzden boyutu bellek
            // bütçesini ilgilendirmez. Sınırın amacı okunan veriyi korumaktır.
            return Err(ClipForgeHata::AsiriKutu {
                tip: baslik.tip_metni(),
                boyut: baslik.boyut,
                sinir: self.ayarlar.asiri_kutu,
            });
        }
        Ok(())
    }

    /// Belirtilen ofsetten tam olarak `uzunluk` bayt okur.
    fn veri_oku(
        &mut self,
        ofset: u64,
        uzunluk: usize,
        baglam: &str,
    ) -> Result<Vec<u8>, ClipForgeHata> {
        self.ic
            .seek(SeekFrom::Start(ofset))
            .map_err(|hata| ClipForgeHata::GirdiHatasi {
                yol: std::path::PathBuf::new(),
                hata,
            })?;
        let mut arabellek = vec![0_u8; uzunluk];
        self.ic.read_exact(&mut arabellek).map_err(|hata| {
            if hata.kind() == io::ErrorKind::UnexpectedEof {
                ClipForgeHata::EksikKutu {
                    yol: format!("@{ofset} ({baglam})"),
                    ayrinti: format!("{uzunluk} bayt bekleniyordu, dosya sonu"),
                }
            } else {
                ClipForgeHata::GirdiHatasi {
                    yol: std::path::PathBuf::new(),
                    hata,
                }
            }
        })?;
        self.konum = ofset + uzunluk as u64;
        self.okunan += uzunluk as u64;
        Ok(arabellek)
    }

    /// Bir kutu başlığından sonraki tüm kutuları sırayla okur.
    ///
    /// Yapısal tipler özyinelemeli gezilir; `mdat` gibi devasa yaprak kutular
    /// yalnızca başlıkları okunur, verilerine hiç dokunulmaz.
    pub fn tara(
        &mut self,
        ust_baslangic: u64,
        ust_bitis: u64,
        yol: &str,
        derinlik: usize,
        trak: Option<usize>,
        dosya_boyutu: u64,
    ) -> Result<Vec<BulunanKutu>, ClipForgeHata> {
        let mut tarama = Tarama::default();
        self.cocuklari(
            ust_baslangic,
            ust_bitis,
            yol,
            derinlik,
            trak,
            dosya_boyutu,
            &mut tarama,
        )?;
        Ok(tarama.cikti)
    }

    /// Konteynerin çocuk kutularını sırayla okur ve gerekirse özyinelemeye girer.
    #[allow(clippy::too_many_arguments)]
    fn cocuklari(
        &mut self,
        ust_baslangic: u64,
        ust_bitis: u64,
        yol: &str,
        derinlik: usize,
        trak: Option<usize>,
        dosya_boyutu: u64,
        tarama: &mut Tarama,
    ) -> Result<(), ClipForgeHata> {
        if derinlik > self.ayarlar.asiri_derinlik {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "derinlik".to_string(),
                sinir: self.ayarlar.asiri_derinlik as u64,
            });
        }
        let mut ofset = ust_baslangic;
        while ofset.saturating_add(KUTU_BASLIK_BOYUTU) <= ust_bitis {
            tarama.oge += 1;
            if tarama.oge > self.ayarlar.asiri_oge {
                return Err(ClipForgeHata::YinelemeSiniri {
                    tur: "oge".to_string(),
                    sinir: self.ayarlar.asiri_oge as u64,
                });
            }
            let baslik = self.basligi_oku(ofset, dosya_boyutu)?;
            let tip = baslik.tip_metni();
            // `trak` kutuları aynı yolu paylaşmasın diye yola sıra numarası
            // eklenir: `moov/trak[0]/tkhd`. Aksi hâlde iki parçanın kutuları
            // yol eşleşmesinde birbirine karışır.
            let (cocuk_yolu, parca) = if baslik.tip == *b"trak" {
                let sira = tarama.trak;
                tarama.trak += 1;
                let kok = if yol.is_empty() {
                    format!("trak[{sira}]")
                } else {
                    format!("{yol}/trak[{sira}]")
                };
                (kok, Some(sira))
            } else if yol.is_empty() {
                (tip.clone(), trak)
            } else {
                (format!("{yol}/{tip}"), trak)
            };
            tarama.cikti.push(BulunanKutu {
                yol: cocuk_yolu.clone(),
                tip,
                ofset: baslik.ofset,
                boyut: baslik.boyut,
                baslik_boyutu: baslik.baslik_boyutu,
                trak: parca,
            });
            if YAPISAL_TIPLER.contains(&baslik.tip) {
                self.cocuklari(
                    baslik.veri_ofseti(),
                    baslik.sonraki_ofset(),
                    &cocuk_yolu,
                    derinlik + 1,
                    parca,
                    dosya_boyutu,
                    tarama,
                )?;
            }
            if baslik.sona_kadar {
                break;
            }
            ofset = baslik.sonraki_ofset();
        }
        Ok(())
    }
}

/// Özyinelemeli gezinin taşıdığı durum.
#[derive(Debug, Default)]
struct Tarama {
    oge: usize,
    trak: usize,
    cikti: Vec<BulunanKutu>,
}

/// `ftyp` kutusunu okur.
pub fn ftyp_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<Ftyp, ClipForgeHata> {
    if kutu.boyut < 8 {
        return Err(ClipForgeHata::EksikKutu {
            yol: kutu.yol.clone(),
            ayrinti: "en az 8 bayt gerekiyor".to_string(),
        });
    }
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu, 8, &kutu.yol)?;
    let kucuk_surum = u32::from_be_bytes([veri[4], veri[5], veri[6], veri[7]]);
    let marka_sayisi = (kutu.boyut - 8) / 4;
    let okunacak = usize::try_from(marka_sayisi).unwrap_or(usize::MAX).min(64);
    let uyumlu_markalar = if okunacak == 0 {
        Vec::new()
    } else {
        let ham = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu + 8, okunacak * 4, &kutu.yol)?;
        ham.chunks_exact(4)
            .map(|parca| String::from_utf8_lossy(parca).into_owned())
            .collect()
    };
    Ok(Ftyp {
        ana_marka: String::from_utf8_lossy(&veri[0..4]).into_owned(),
        kucuk_surum,
        uyumlu_markalar,
    })
}

/// Zamanlama kutularının (`mvhd`, `mdhd`) sürüm 0 ve 1 gövdelerinde ortak
/// olan alanları çözer.
///
/// Sürüm 0: `creation(4) modification(4) timescale(4) duration(4)`.
/// Sürüm 1: `creation(8) modification(8) timescale(4) duration(8)`.
fn zamanlama_alanlari(surum: u8, veri: &[u8]) -> Mvhd {
    if surum == 1 {
        Mvhd {
            zaman_olcegi: u32::from_be_bytes([veri[16], veri[17], veri[18], veri[19]]),
            ham_sure: u64::from_be_bytes([
                veri[20], veri[21], veri[22], veri[23], veri[24], veri[25], veri[26], veri[27],
            ]),
        }
    } else {
        Mvhd {
            zaman_olcegi: u32::from_be_bytes([veri[8], veri[9], veri[10], veri[11]]),
            ham_sure: u64::from(u32::from_be_bytes([veri[12], veri[13], veri[14], veri[15]])),
        }
    }
}

/// `mvhd` kutusunu okur.
pub fn mvhd_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<Mvhd, ClipForgeHata> {
    let surum = okuyucu.surum_oku(kutu)?;
    let kalan: usize = if surum == 1 { 28 } else { 16 };
    okuyucu.boyut_kontrol(kutu, 4 + kalan as u64)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu + 4, kalan, &kutu.yol)?;
    Ok(zamanlama_alanlari(surum, &veri))
}

/// `mdhd` kutusunu okur.
///
/// `mdhd` ve `mvhd`, ilk dört bayttan sonra aynı düzendedir; ayrıştırma kodu
/// bilinçli olarak paylaşılmıştır.
pub fn mdhd_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<Mdhd, ClipForgeHata> {
    let surum = okuyucu.surum_oku(kutu)?;
    let kalan: usize = if surum == 1 { 28 } else { 16 };
    okuyucu.boyut_kontrol(kutu, 4 + kalan as u64)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu + 4, kalan, &kutu.yol)?;
    let alanlar = zamanlama_alanlari(surum, &veri);
    Ok(Mdhd {
        zaman_olcegi: alanlar.zaman_olcegi,
        ham_sure: alanlar.ham_sure,
    })
}

/// `tkhd` kutusunu okur ve `(parça izi, ham süre, genişlik, yükseklik)`
/// döndürür.
///
/// Genişlik ve yükseklik 16.16 sabit nokta biçimindedir; burada yalnızca tam
/// sayı kısmı alınır.
pub fn tkhd_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<(u32, u64, u32, u32), ClipForgeHata> {
    let surum = okuyucu.surum_oku(kutu)?;
    // Sürüm 0 gövdesi 80 bayt (süre 4 bayt), sürüm 1 gövdesi 92 bayt (süre 8 bayt).
    let kalan: usize = if surum == 1 { 92 } else { 80 };
    okuyucu.boyut_kontrol(kutu, 4 + kalan as u64)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu + 4, kalan, &kutu.yol)?;
    let (iz, ham_sure, ofset) = if surum == 1 {
        (
            u32::from_be_bytes([veri[16], veri[17], veri[18], veri[19]]),
            u64::from_be_bytes([
                veri[24], veri[25], veri[26], veri[27], veri[28], veri[29], veri[30], veri[31],
            ]),
            84_usize,
        )
    } else {
        (
            u32::from_be_bytes([veri[8], veri[9], veri[10], veri[11]]),
            u64::from(u32::from_be_bytes([veri[16], veri[17], veri[18], veri[19]])),
            72_usize,
        )
    };
    let sabit = |bastan: usize| {
        u32::from_be_bytes([
            veri[bastan],
            veri[bastan + 1],
            veri[bastan + 2],
            veri[bastan + 3],
        ]) >> 16
    };
    Ok((iz, ham_sure, sabit(ofset), sabit(ofset + 4)))
}

/// `hdlr` kutusundaki işleyici kodunu okur.
pub fn hdlr_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<ParcasiTuru, ClipForgeHata> {
    okuyucu.boyut_kontrol(kutu, 12)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu, 12, &kutu.yol)?;
    let kod: [u8; 4] = [veri[8], veri[9], veri[10], veri[11]];
    Ok(ParcasiTuru::koddan(&kod))
}

/// `stsd` kutusundaki ilk örnek girdisini okur.
///
/// # Hatalar
///
/// Kutu 44 bayttan kısaysa, giriş sayısı sıfırsa ya da ilk giriş beklenen
/// alanları taşımıyorsa [`ClipForgeHata::EksikKutu`] döner.
pub fn stsd_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
    tur: ParcasiTuru,
) -> Result<Stsd, ClipForgeHata> {
    // 8 bayt `stsd` başlığı (sürüm/bayraklar + giriş sayısı) + 36 baytlık
    // AudioSampleEntry gövdesi. VideoSampleEntry gövdesi daha uzundur, ancak
    // genişlik/yükseklik alanları ilk 36 baytın içindedir.
    const GIRIS_BOYUTU: u64 = 36;
    okuyucu.boyut_kontrol(kutu, 8 + GIRIS_BOYUTU)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu, 44, &kutu.yol)?;

    let giris_sayisi = u32::from_be_bytes([veri[4], veri[5], veri[6], veri[7]]);
    if giris_sayisi == 0 {
        return Err(ClipForgeHata::EksikKutu {
            yol: kutu.yol.clone(),
            ayrinti: "hic ornek girdisi yok".to_string(),
        });
    }
    let ilan_edilen = u32::from_be_bytes([veri[8], veri[9], veri[10], veri[11]]);
    if u64::from(ilan_edilen) < GIRIS_BOYUTU {
        return Err(ClipForgeHata::EksikKutu {
            yol: kutu.yol.clone(),
            ayrinti: format!(
                "ilk ornek girdisi {ilan_edilen} bayt, en az {GIRIS_BOYUTU} gerekiyor"
            ),
        });
    }
    let bicim = String::from_utf8_lossy(&veri[12..16]).into_owned();
    let mut sonuc = Stsd {
        bicim,
        ..Stsd::default()
    };
    match tur {
        // VideoSampleEntry: ... pre_defined[3] (12) sonra width(2), height(2).
        ParcasiTuru::Video => {
            sonuc.genislik = u32::from(u16::from_be_bytes([veri[40], veri[41]]));
            sonuc.yukseklik = u32::from(u16::from_be_bytes([veri[42], veri[43]]));
        }
        // AudioSampleEntry: reserved[2] (8) sonra channelcount(2), samplesize(2),
        // pre_defined(2), reserved(2), samplerate(4, 16.16 sabit nokta).
        ParcasiTuru::Ses => {
            sonuc.kanal = u16::from_be_bytes([veri[32], veri[33]]);
            sonuc.ornekleme_boyutu = u16::from_be_bytes([veri[34], veri[35]]);
            sonuc.ornekleme_hizi =
                u32::from_be_bytes([veri[40], veri[41], veri[42], veri[43]]) >> 16;
        }
        ParcasiTuru::Diger => {}
    }
    Ok(sonuc)
}

/// `stts` kutusundaki zamanlama özetini okur.
pub fn stts_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<Stts, ClipForgeHata> {
    okuyucu.boyut_kontrol(kutu, 8)?;
    let baslik = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu, 8, &kutu.yol)?;
    let giris_sayisi = u32::from_be_bytes([baslik[4], baslik[5], baslik[6], baslik[7]]);
    let gereken = 8 + u64::from(giris_sayisi) * 8;
    if kutu.boyut < gereken {
        return Err(ClipForgeHata::EksikKutu {
            yol: kutu.yol.clone(),
            ayrinti: format!(
                "{giris_sayisi} giris ilan edildi ama kutu yalnizca {} bayt",
                kutu.boyut
            ),
        });
    }
    if giris_sayisi == 0 {
        return Ok(Stts {
            toplam_ornek: 0,
            giris_sayisi: 0,
            sabit_artış: None,
            ilk_artış: 0,
        });
    }
    let veri = okuyucu.veri_oku(
        kutu.ofset + kutu.baslik_boyutu + 8,
        (giris_sayisi as usize) * 8,
        &kutu.yol,
    )?;
    let mut toplam_ornek: u64 = 0;
    let mut ilk_artış: u32 = 0;
    let sabit_artış = u32::from_be_bytes([veri[4], veri[5], veri[6], veri[7]]);
    let mut hepsi_ayni = true;
    for giris in 0..giris_sayisi as usize {
        let taban = giris * 8;
        let adet = u32::from_be_bytes([
            veri[taban],
            veri[taban + 1],
            veri[taban + 2],
            veri[taban + 3],
        ]) as u64;
        let artış = u32::from_be_bytes([
            veri[taban + 4],
            veri[taban + 5],
            veri[taban + 6],
            veri[taban + 7],
        ]);
        if giris == 0 {
            ilk_artış = artış;
        } else if artış != sabit_artış {
            hepsi_ayni = false;
        }
        toplam_ornek = toplam_ornek.saturating_add(adet);
    }
    Ok(Stts {
        toplam_ornek,
        giris_sayisi,
        sabit_artış: if hepsi_ayni && sabit_artış > 0 {
            Some(sabit_artış)
        } else {
            None
        },
        ilk_artış,
    })
}

/// `stsz` kutusundaki toplam örnek sayısını okur.
pub fn stsz_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<u64, ClipForgeHata> {
    okuyucu.boyut_kontrol(kutu, 12)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu, 12, &kutu.yol)?;
    let adet = u32::from_be_bytes([veri[8], veri[9], veri[10], veri[11]]);
    Ok(u64::from(adet))
}

/// `stss` kutusundaki eşzamanlı kare sayısını okur.
pub fn stss_oku<R: Read + Seek>(
    okuyucu: &mut KutuOkuyucu<R>,
    kutu: &BulunanKutu,
) -> Result<u32, ClipForgeHata> {
    okuyucu.boyut_kontrol(kutu, 8)?;
    let veri = okuyucu.veri_oku(kutu.ofset + kutu.baslik_boyutu, 8, &kutu.yol)?;
    Ok(u32::from_be_bytes([veri[4], veri[5], veri[6], veri[7]]))
}

/// Dosyanın kutu hiyerarşisini okur ve istenen alanları toplar.
///
/// # Hatalar
///
/// Okuma hatası, bozuk başlık, geçersiz boyut veya aşılan sınır durumlarında
/// [`ClipForgeHata`] türünün uygun varyantını döner. `moov` bulunamazsa
/// [`ClipForgeHata::MoovYok`] döner.
pub fn cozumle(yol: &Path, ayarlar: Ayarlar) -> Result<KutuVerisi, ClipForgeHata> {
    let mut okuyucu = KutuOkuyucu::ac(yol, ayarlar)?;
    let dosya_boyutu = okuyucu.dosya_boyutu()?;
    if dosya_boyutu < 16 {
        return Err(ClipForgeHata::BilinmeyenKapsul {
            yol: yol.to_path_buf(),
            sebep: format!("dosya yalnizca {dosya_boyutu} bayt"),
        });
    }
    let kok = okuyucu.tara(0, dosya_boyutu, "", 0, None, dosya_boyutu)?;

    let moov = kok
        .iter()
        .find(|k| k.tip == "moov")
        .ok_or_else(|| ClipForgeHata::MoovYok {
            yol: yol.to_path_buf(),
        })?;
    let moov_ofseti = Some(moov.ofset);
    let ilk_mdat_ofseti = kok.iter().find(|k| k.tip == "mdat").map(|k| k.ofset);

    let ftyp = match kok.iter().find(|k| k.tip == "ftyp") {
        Some(kutu) => Some(ftyp_oku(&mut okuyucu, kutu)?),
        None => None,
    };
    let mvhd = match kok.iter().find(|k| k.tip == "mvhd") {
        Some(kutu) => Some(mvhd_oku(&mut okuyucu, kutu)?),
        None => None,
    };

    let mut parcalar: Vec<ParcaKutusu> = Vec::new();
    for kutu in kok.iter().filter(|k| k.tip == "trak") {
        let kok_yol = kutu.yol.clone();
        let tkhd = kok.iter().find(|k| k.yol == format!("{kok_yol}/tkhd"));
        let (iz, tkhd_ham_sure, tkhd_genislik, tkhd_yukseklik) = match tkhd {
            Some(k) => tkhd_oku(&mut okuyucu, k)?,
            None => (0, 0, 0, 0),
        };
        let mdhd = match kok.iter().find(|k| k.yol == format!("{kok_yol}/mdia/mdhd")) {
            Some(k) => Some(mdhd_oku(&mut okuyucu, k)?),
            None => None,
        };
        let tur = match kok.iter().find(|k| k.yol == format!("{kok_yol}/mdia/hdlr")) {
            Some(k) => hdlr_oku(&mut okuyucu, k)?,
            None => ParcasiTuru::Diger,
        };
        let stsd_kutu = kok
            .iter()
            .find(|k| k.yol == format!("{kok_yol}/mdia/minf/stbl/stsd"));
        let stsd = match stsd_kutu {
            Some(k) => Some(stsd_oku(&mut okuyucu, k, tur)?),
            None => None,
        };
        let stts = match kok
            .iter()
            .find(|k| k.yol == format!("{kok_yol}/mdia/minf/stbl/stts"))
        {
            Some(k) => Some(stts_oku(&mut okuyucu, k)?),
            None => None,
        };
        let ornek_sayisi = match kok
            .iter()
            .find(|k| k.yol == format!("{kok_yol}/mdia/minf/stbl/stsz"))
        {
            Some(k) => Some(stsz_oku(&mut okuyucu, k)?),
            None => None,
        };
        let anahtar_kare_sayisi = match kok
            .iter()
            .find(|k| k.yol == format!("{kok_yol}/mdia/minf/stbl/stss"))
        {
            Some(k) => Some(stss_oku(&mut okuyucu, k)?),
            None => None,
        };
        parcalar.push(ParcaKutusu {
            iz,
            tur,
            kutu_yolu: format!("{kok_yol}/mdia/minf/stbl"),
            tkhd_sure_sn: tkhd_ham_sure as f64,
            tkhd_genislik,
            tkhd_yukseklik,
            mdhd,
            stsd,
            stts,
            ornek_sayisi,
            anahtar_kare_sayisi,
        });
    }

    Ok(KutuVerisi {
        ftyp,
        mvhd,
        parcalar,
        moov_ofseti,
        ilk_mdat_ofseti,
        kutu_sayisi: kok.len(),
        okunan_bayt: okuyucu.okunan_bayt(),
    })
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Bayt dizisinden `Cursor` tabanlı okuyucu yapar.
    fn okuyucu(veri: Vec<u8>) -> KutuOkuyucu<Cursor<Vec<u8>>> {
        KutuOkuyucu::yeni(Cursor::new(veri), Ayarlar::default())
    }

    /// Test kutusu: 8 bayt başlık + veri.
    fn kutu(tip: &str, veri: &[u8]) -> Vec<u8> {
        let mut sonuc = ((veri.len() + 8) as u32).to_be_bytes().to_vec();
        sonuc.extend_from_slice(tip_donus(tip).as_slice());
        sonuc.extend_from_slice(veri);
        sonuc
    }

    #[test]
    fn baslik_okuma_klasik_ve_genis_boyut() {
        // `size == 1` geniş boyut biçimini tetikler. Geniş boyut **kutu
        // başlığının tamamını** kapsar: 8 (temel başlık) + 8 (geniş boyut)
        // + 4 (gövde) = 20 bayt.
        let mut veri = 1_u32.to_be_bytes().to_vec();
        veri.extend_from_slice(b"free");
        veri.extend_from_slice(&20_u64.to_be_bytes());
        veri.extend_from_slice(&[1, 2, 3, 4]);
        veri.extend_from_slice(&[9, 9, 9, 9]);
        let mut o = okuyucu(veri);
        let baslik = o.basligi_oku(0, 24).unwrap();
        assert_eq!(baslik.tip_metni(), "free");
        assert_eq!(baslik.boyut, 4);
        assert_eq!(baslik.baslik_boyutu, 16);
        assert!(!baslik.sona_kadar);
        assert_eq!(baslik.veri_ofseti(), 16);
        assert_eq!(baslik.sonraki_ofset(), 20);
    }

    #[test]
    fn genis_boyut_basliktan_kucukse_gunceller() {
        // Toplam boyut başlıktan küçük ilan edilmiş: bozuk kutu reddedilir.
        let mut veri = 1_u32.to_be_bytes().to_vec();
        veri.extend_from_slice(b"free");
        veri.extend_from_slice(&4_u64.to_be_bytes());
        veri.extend_from_slice(&[1, 2, 3, 4]);
        let mut o = okuyucu(veri);
        let hata = o.basligi_oku(0, 24).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::GecersizBoyut { .. }));
    }

    #[test]
    fn baslik_sonu_dort_sifir_dosya_sonuna_kadar_uzatir() {
        let mut veri = kutu("mdat", &[1, 2, 3]);
        veri[0..4].copy_from_slice(&0_u32.to_be_bytes());
        let mut o = okuyucu(veri);
        let baslik = o.basligi_oku(0, 11).unwrap();
        assert!(baslik.sona_kadar);
        assert_eq!(baslik.boyut, 3);
    }

    #[test]
    fn baslik_yazdirilamayen_tipi_reddeder() {
        let mut veri = kutu("free", &[]);
        veri[4] = 0x00;
        veri[5] = 0x01;
        let mut o = okuyucu(veri);
        let hata = o.basligi_oku(0, 8).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::BozukBaslik { .. }));
    }

    #[test]
    fn baslik_bayt_bozuk_butut_gunceller() {
        let mut o = okuyucu(vec![1, 2, 3]);
        let hata = o.basligi_oku(0, 3).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::EksikKutu { .. }));
    }

    #[test]
    fn kutu_hiyerarsisi_ve_yollar() {
        let ic = kutu(
            "trak",
            &kutu("mdia", &kutu("minf", &kutu("stbl", &kutu("stsd", &[0; 8])))),
        );
        let mut o = okuyucu(ic.clone());
        let boyut = ic.len() as u64;
        let kutular = o.tara(0, boyut, "", 0, None, boyut).unwrap();
        let yollar: Vec<&str> = kutular.iter().map(|k| k.yol.as_str()).collect();
        assert_eq!(
            yollar,
            vec![
                "trak[0]",
                "trak[0]/mdia",
                "trak[0]/mdia/minf",
                "trak[0]/mdia/minf/stbl",
                "trak[0]/mdia/minf/stbl/stsd"
            ]
        );
        assert_eq!(kutular[0].trak, Some(0));
        assert_eq!(kutular.last().unwrap().ofset, 32);
    }

    #[test]
    fn iki_trak_kutu_yollari_ayirt_edilir() {
        // İki `trak` kutusu aynı iç hiyerarşiye sahiptir; ayrım yalnızca
        // yoldaki sıra numarasından gelmelidir.
        let parca = |isleyici: &[u8; 4]| {
            let mut mdia = vec![0_u8; 12];
            mdia[8..12].copy_from_slice(isleyici);
            let mdia = kutu("mdia", &kutu("hdlr", &mdia));
            let mut govde = kutu("tkhd", &[0; 20]);
            govde.extend_from_slice(&mdia);
            kutu("trak", &govde)
        };
        let mut moov = parca(b"vide");
        moov.extend_from_slice(&parca(b"soun"));
        let mut ic = kutu("moov", &moov);
        ic.extend_from_slice(&kutu("free", &[0; 4]));
        let mut o = okuyucu(ic.clone());
        let kutular = o
            .tara(0, ic.len() as u64, "", 0, None, ic.len() as u64)
            .unwrap();
        let yollar: Vec<&str> = kutular.iter().map(|k| k.yol.as_str()).collect();
        assert!(yollar.contains(&"moov/trak[0]/mdia/hdlr"), "{yollar:?}");
        assert!(yollar.contains(&"moov/trak[1]/mdia/hdlr"), "{yollar:?}");
    }

    #[test]
    fn devasa_mdat_gezgini_yormaz() {
        // 1 GiB'lik bir mdat: yalnizca 8 bayt basligi okunmali.
        const BOYUT: u64 = 1024 * 1024 * 1024;
        let mut veri = kutu("mdat", &[]);
        veri[0..4].copy_from_slice(&(BOYUT as u32).to_be_bytes());
        veri.extend_from_slice(&[0_u8; 8]);
        let mut o = okuyucu(veri);
        let kutular = o.tara(0, 20, "", 0, None, BOYUT).unwrap();
        assert_eq!(kutular.len(), 1);
        assert_eq!(kutular[0].boyut, BOYUT - 8);
        assert_eq!(o.okunan_bayt(), 8);
    }

    #[test]
    fn cok_buyuk_kutu_reddedilir() {
        let mut veri = kutu("moov", &[0; 8]);
        veri[0..4].copy_from_slice(&(100_000_u32).to_be_bytes());
        let mut o = KutuOkuyucu::yeni(Cursor::new(veri), dar_ayarlar(1024));
        let hata = o.basligi_oku(0, 100_008).unwrap_err();
        match hata {
            ClipForgeHata::AsiriKutu { boyut, sinir, .. } => {
                assert_eq!(boyut, 99_992);
                assert_eq!(sinir, 1024);
            }
            diger => panic!("beklenen AsiriKutu, gelen {diger:?}"),
        }
    }

    #[test]
    fn gecersiz_boyut_dosya_sinirini_asan_kutu_reddeder() {
        let mut veri = kutu("moov", &[0; 4]);
        veri[0..4].copy_from_slice(&999_999_u32.to_be_bytes());
        let mut o = okuyucu(veri);
        let hata = o.basligi_oku(0, 20).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::GecersizBoyut { .. }));
    }

    #[test]
    fn oge_siniri_asisinda_durulur() {
        let mut ic = Vec::new();
        for _ in 0..64 {
            ic.extend_from_slice(&kutu("free", &[0, 0, 0, 0]));
        }
        let ayarlar = Ayarlar {
            asiri_oge: 8,
            ..Ayarlar::default()
        };
        let mut o = KutuOkuyucu::yeni(Cursor::new(ic.clone()), ayarlar);
        let boyut = ic.len() as u64;
        let hata = o.tara(0, boyut, "", 0, None, boyut).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::YinelemeSiniri { .. }));
    }

    #[test]
    fn derinlik_siniri_asisinda_durulur() {
        let ic = kutu(
            "trak",
            &kutu(
                "mdia",
                &kutu(
                    "minf",
                    &kutu("stbl", &kutu("stbl", &kutu("stbl", &kutu("stbl", &[])))),
                ),
            ),
        );
        let ayarlar = Ayarlar {
            asiri_derinlik: 2,
            ..Ayarlar::default()
        };
        let mut o = KutuOkuyucu::yeni(Cursor::new(ic.clone()), ayarlar);
        let boyut = ic.len() as u64;
        let hata = o.tara(0, boyut, "", 0, None, boyut).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::YinelemeSiniri { .. }));
    }

    #[test]
    fn ftyp_varyantlari_okunur() {
        let mut veri = b"isom".to_vec();
        veri.extend_from_slice(&0x0000_0200_u32.to_be_bytes());
        veri.extend_from_slice(b"isom");
        veri.extend_from_slice(b"iso2");
        veri.extend_from_slice(b"avc1");
        let kutu_bilgi = BulunanKutu {
            yol: "ftyp".to_string(),
            tip: "ftyp".to_string(),
            ofset: 0,
            boyut: 20,
            baslik_boyutu: 8,
            trak: None,
        };
        let mut o = okuyucu(kutu("ftyp", &veri));
        let ftyp = ftyp_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(ftyp.ana_marka, "isom");
        assert_eq!(ftyp.kucuk_surum, 512);
        assert_eq!(ftyp.uyumlu_markalar, vec!["isom", "iso2", "avc1"]);
    }

    #[test]
    fn ftyp_markasiz_dosya_kabul_edilir() {
        let kutu_bilgi = BulunanKutu {
            yol: "ftyp".to_string(),
            tip: "ftyp".to_string(),
            ofset: 0,
            boyut: 8,
            baslik_boyutu: 8,
            trak: None,
        };
        let mut o = okuyucu(kutu("ftyp", b"qt  \x00\x00\x01\x00"));
        let ftyp = ftyp_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(ftyp.ana_marka, "qt  ");
        assert!(ftyp.uyumlu_markalar.is_empty());
    }

    #[test]
    fn ftyp_cok_kisa_kutu_reddedilir() {
        let kutu_bilgi = BulunanKutu {
            yol: "ftyp".to_string(),
            tip: "ftyp".to_string(),
            ofset: 0,
            boyut: 4,
            baslik_boyutu: 8,
            trak: None,
        };
        let mut o = okuyucu(kutu("ftyp", &[1, 2, 3, 4]));
        assert!(matches!(
            ftyp_oku(&mut o, &kutu_bilgi),
            Err(ClipForgeHata::EksikKutu { .. })
        ));
    }

    #[test]
    fn mvhd_surum_0_ve_1_aynisinda_okunur() {
        let mut v0 = vec![0_u8; 4];
        v0.extend_from_slice(&[0; 8]);
        v0.extend_from_slice(&600_u32.to_be_bytes());
        v0.extend_from_slice(&7_500_u32.to_be_bytes());
        v0.resize(32, 0);
        let kutu_bilgi = BulunanKutu {
            yol: "moov/mvhd".to_string(),
            tip: "mvhd".to_string(),
            ofset: 0,
            boyut: 40,
            baslik_boyutu: 8,
            trak: None,
        };
        let mut o = okuyucu(kutu("mvhd", &v0));
        let mvhd = mvhd_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(mvhd.zaman_olcegi, 600);
        assert_eq!(mvhd.ham_sure, 7_500);
        assert_eq!(mvhd.sure_sn(), 12.5);

        let mut v1 = vec![1_u8];
        v1.extend_from_slice(&[0; 3]);
        v1.extend_from_slice(&[0; 16]);
        v1.extend_from_slice(&600_u32.to_be_bytes());
        v1.extend_from_slice(&7_500_u64.to_be_bytes());
        v1.resize(32, 0);
        let mut o = okuyucu(kutu("mvhd", &v1));
        let mvhd = mvhd_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(mvhd.sure_sn(), 12.5);
    }

    #[test]
    fn mdhd_ve_tkhd_zamanlama_alanlari() {
        let mut mdhd = vec![0_u8; 4];
        mdhd.extend_from_slice(&[0; 8]);
        mdhd.extend_from_slice(&48_000_u32.to_be_bytes());
        mdhd.extend_from_slice(&2_400_000_u32.to_be_bytes());
        mdhd.resize(32, 0);
        let kutu_bilgi = BulunanKutu {
            yol: "mdia/mdhd".to_string(),
            tip: "mdhd".to_string(),
            ofset: 0,
            boyut: 40,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu("mdhd", &mdhd));
        let mdhd = mdhd_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(mdhd.sure_sn(), 50.0);

        // tkhd sürüm 0: version/flags(4) creation(4) modification(4) iz(4)
        // reserved(4) duration(4) reserved(8) layer(2) alt(2) volume(2)
        // reserved(2) matrix(36) width(4) height(4) = 84 bayt.
        let mut tkhd = vec![0_u8; 84];
        tkhd[12..16].copy_from_slice(&7_u32.to_be_bytes());
        tkhd[20..24].copy_from_slice(&5_000_u32.to_be_bytes());
        tkhd[76..80].copy_from_slice(&(1080_u32 << 16).to_be_bytes());
        tkhd[80..84].copy_from_slice(&(1920_u32 << 16).to_be_bytes());
        let tkhd_bilgi = BulunanKutu {
            yol: "trak/tkhd".to_string(),
            tip: "tkhd".to_string(),
            ofset: 0,
            boyut: 84,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu("tkhd", &tkhd));
        let (iz, sure, genislik, yukseklik) = tkhd_oku(&mut o, &tkhd_bilgi).unwrap();
        assert_eq!(iz, 7);
        assert_eq!(sure, 5_000);
        assert_eq!(genislik, 1080);
        assert_eq!(yukseklik, 1920);
    }

    #[test]
    fn kisa_tkhd_kutusu_reddedilir() {
        let kutu_bilgi = BulunanKutu {
            yol: "trak/tkhd".to_string(),
            tip: "tkhd".to_string(),
            ofset: 0,
            boyut: 40,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu("tkhd", &[0; 40]));
        assert!(matches!(
            tkhd_oku(&mut o, &kutu_bilgi),
            Err(ClipForgeHata::EksikKutu { .. })
        ));
    }

    #[test]
    fn hdlr_isleyici_kodlari() {
        let kutu_bilgi = BulunanKutu {
            yol: "mdia/hdlr".to_string(),
            tip: "hdlr".to_string(),
            ofset: 0,
            boyut: 24,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu(
            "hdlr",
            &[0, 0, 0, 0, 0, 0, 0, 0, b'v', b'i', b'd', b'e'],
        ));
        assert_eq!(hdlr_oku(&mut o, &kutu_bilgi).unwrap(), ParcasiTuru::Video);
        let mut o = okuyucu(kutu(
            "hdlr",
            &[0, 0, 0, 0, 0, 0, 0, 0, b's', b'o', b'u', b'n'],
        ));
        assert_eq!(hdlr_oku(&mut o, &kutu_bilgi).unwrap(), ParcasiTuru::Ses);
        let mut o = okuyucu(kutu(
            "hdlr",
            &[0, 0, 0, 0, 0, 0, 0, 0, b't', b'e', b'x', b't'],
        ));
        assert_eq!(hdlr_oku(&mut o, &kutu_bilgi).unwrap(), ParcasiTuru::Diger);
    }

    #[test]
    fn stsd_video_ve_ses_ornek_girdileri() {
        let kutu_bilgi = BulunanKutu {
            yol: "stbl/stsd".to_string(),
            tip: "stsd".to_string(),
            ofset: 0,
            boyut: 120,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        // VideoSampleEntry: 0..4 boyut, 4..8 bicim, 8..14 reserved, 14..16 data_ref,
        // 16..18 pre_defined, 18..20 reserved, 20..32 pre_defined[3],
        // 32..34 genislik, 34..36 yukseklik.
        let mut veri = vec![0_u8; 94];
        veri[4..8].copy_from_slice(&1_u32.to_be_bytes());
        veri[8..12].copy_from_slice(&86_u32.to_be_bytes());
        veri[12..16].copy_from_slice(b"avc1");
        // Girdi, kutu verisinin 8. baytında başlar: genişlik 8 + 32 = 40.
        veri[40..42].copy_from_slice(&1920_u16.to_be_bytes());
        veri[42..44].copy_from_slice(&1080_u16.to_be_bytes());
        let mut o = okuyucu(kutu("stsd", &veri));
        let stsd = stsd_oku(&mut o, &kutu_bilgi, ParcasiTuru::Video).unwrap();
        assert_eq!(stsd.bicim, "avc1");
        assert_eq!(stsd.genislik, 1920);
        assert_eq!(stsd.yukseklik, 1080);

        // AudioSampleEntry: 16..24 reserved[2], 24..26 kanal, 26..28 ornek boyutu,
        // 28..30 pre_defined, 30..32 reserved, 32..36 ornekleme hizi (16.16).
        let mut veri = vec![0_u8; 44];
        veri[4..8].copy_from_slice(&1_u32.to_be_bytes());
        veri[8..12].copy_from_slice(&36_u32.to_be_bytes());
        veri[12..16].copy_from_slice(b"mp4a");
        // Kanal 8 + 24 = 32, örnek boyutu 8 + 26 = 34, örnekleme 8 + 32 = 40.
        veri[32..34].copy_from_slice(&2_u16.to_be_bytes());
        veri[34..36].copy_from_slice(&16_u16.to_be_bytes());
        veri[40..44].copy_from_slice(&(48_000_u32 << 16).to_be_bytes());
        let mut o = okuyucu(kutu("stsd", &veri));
        let stsd = stsd_oku(&mut o, &kutu_bilgi, ParcasiTuru::Ses).unwrap();
        assert_eq!(stsd.bicim, "mp4a");
        assert_eq!(stsd.kanal, 2);
        assert_eq!(stsd.ornekleme_boyutu, 16);
        assert_eq!(stsd.ornekleme_hizi, 48_000);
    }

    #[test]
    fn stsd_kisa_girdi_veya_kisa_kutu_reddedilir() {
        let kutu_bilgi = BulunanKutu {
            yol: "stbl/stsd".to_string(),
            tip: "stsd".to_string(),
            ofset: 0,
            boyut: 20,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu("stsd", &[0, 0, 0, 0, 0, 0, 0, 1]));
        assert!(matches!(
            stsd_oku(&mut o, &kutu_bilgi, ParcasiTuru::Video),
            Err(ClipForgeHata::EksikKutu { .. })
        ));

        // Kutu yeterince büyük ama ilk örnek girdisi 36 bayttan kısa.
        let mut veri = vec![0_u8; 44];
        veri[4..8].copy_from_slice(&1_u32.to_be_bytes());
        veri[8..12].copy_from_slice(&16_u32.to_be_bytes());
        let genis = BulunanKutu {
            boyut: 44,
            ..kutu_bilgi.clone()
        };
        let mut o = okuyucu(kutu("stsd", &veri));
        assert!(matches!(
            stsd_oku(&mut o, &genis, ParcasiTuru::Video),
            Err(ClipForgeHata::EksikKutu { .. })
        ));
    }

    #[test]
    fn stts_sabit_ve_degisken_artislari_ayirt_edilir() {
        let kutu_bilgi = BulunanKutu {
            yol: "stbl/stts".to_string(),
            tip: "stts".to_string(),
            ofset: 0,
            boyut: 24,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut veri = vec![0_u8, 0, 0, 0, 0, 0, 0, 2];
        veri.extend_from_slice(&1000_u32.to_be_bytes());
        veri.extend_from_slice(&1001_u32.to_be_bytes());
        // İkinci girişin artışı farklı: değişken kare hızlı akış.
        veri.extend_from_slice(&500_u32.to_be_bytes());
        veri.extend_from_slice(&500_u32.to_be_bytes());
        let mut o = okuyucu(kutu("stts", &veri));
        let stts = stts_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(stts.toplam_ornek, 1500);
        assert_eq!(stts.sabit_artış, None);
        assert_eq!(stts.ilk_artış, 1001);

        let mut veri = vec![0_u8, 0, 0, 0, 0, 0, 0, 1];
        veri.extend_from_slice(&1000_u32.to_be_bytes());
        veri.extend_from_slice(&1001_u32.to_be_bytes());
        let mut o = okuyucu(kutu("stts", &veri));
        let stts = stts_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(stts.sabit_artış, Some(1001));
        assert_eq!(stts.toplam_ornek, 1000);
    }

    #[test]
    fn stts_sifir_artisli_kutu_bos_cevap_verir() {
        let kutu_bilgi = BulunanKutu {
            yol: "stbl/stts".to_string(),
            tip: "stts".to_string(),
            ofset: 0,
            boyut: 8,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu("stts", &[0, 0, 0, 0, 0, 0, 0, 0]));
        let stts = stts_oku(&mut o, &kutu_bilgi).unwrap();
        assert_eq!(stts.toplam_ornek, 0);
        assert_eq!(stts.sabit_artış, None);
    }

    #[test]
    fn stts_ilan_edilen_giris_sayisi_kutudan_fazla_ise_reddedilir() {
        let kutu_bilgi = BulunanKutu {
            yol: "stbl/stts".to_string(),
            tip: "stts".to_string(),
            ofset: 0,
            boyut: 16,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        let mut o = okuyucu(kutu("stts", &[0, 0, 0, 0, 0, 0, 0, 9]));
        assert!(matches!(
            stts_oku(&mut o, &kutu_bilgi),
            Err(ClipForgeHata::EksikKutu { .. })
        ));
    }

    #[test]
    fn stsz_ve_stss_ornek_sayilari() {
        let kutu_bilgi = BulunanKutu {
            yol: "stbl/stsz".to_string(),
            tip: "stsz".to_string(),
            ofset: 0,
            boyut: 20,
            baslik_boyutu: 8,
            trak: Some(0),
        };
        // stsz düzeni: version/flags(4) sample_size(4) sample_count(4).
        let mut o = okuyucu(kutu("stsz", &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x27, 0x10]));
        assert_eq!(stsz_oku(&mut o, &kutu_bilgi).unwrap(), 10_000);

        let mut o = okuyucu(kutu("stss", &[0, 0, 0, 0, 0, 0, 0, 25]));
        assert_eq!(stss_oku(&mut o, &kutu_bilgi).unwrap(), 25);
    }

    #[test]
    fn faststart_kurali_moov_ve_mdat_sirasina_bakar() {
        let on = KutuVerisi {
            ftyp: None,
            mvhd: None,
            parcalar: vec![],
            moov_ofseti: Some(32),
            ilk_mdat_ofseti: Some(1_000),
            kutu_sayisi: 2,
            okunan_bayt: 16,
        };
        assert_eq!(on.faststart(), Some(true));
        let son = KutuVerisi {
            ilk_mdat_ofseti: Some(32),
            moov_ofseti: Some(1_000),
            ..on.clone()
        };
        assert_eq!(son.faststart(), Some(false));
        let bilinmeyen = KutuVerisi {
            ilk_mdat_ofseti: None,
            moov_ofseti: Some(32),
            ..on
        };
        assert_eq!(bilinmeyen.faststart(), None);
    }

    #[test]
    fn parca_suresi_once_mdhd_sonra_tkhd_tercih_edilir() {
        let parca = ParcaKutusu {
            iz: 1,
            tur: ParcasiTuru::Video,
            kutu_yolu: "trak/mdia/minf/stbl".to_string(),
            tkhd_sure_sn: 11.0,
            tkhd_genislik: 0,
            tkhd_yukseklik: 0,
            mdhd: Some(Mdhd {
                zaman_olcegi: 1000,
                ham_sure: 12_000,
            }),
            stsd: None,
            stts: None,
            ornek_sayisi: None,
            anahtar_kare_sayisi: None,
        };
        assert_eq!(parca.sure_sn(), 12.0);
        let bos_mdhd = ParcaKutusu {
            mdhd: Some(Mdhd {
                zaman_olcegi: 1000,
                ham_sure: 0,
            }),
            ..parca
        };
        assert_eq!(bos_mdhd.sure_sn(), 11.0);
        let mdhdsiz = ParcaKutusu {
            mdhd: None,
            ..bos_mdhd
        };
        assert_eq!(mdhdsiz.sure_sn(), 11.0);
    }

    #[test]
    fn tip_donus_kisa_metinleri_doldurur() {
        assert_eq!(&tip_donus("mdat"), b"mdat");
        assert_eq!(&tip_donus("ab"), b"ab  ");
        assert_eq!(&tip_donus("abcdxyz"), b"abcd");
    }

    #[test]
    fn dosya_boyutu_sorgulanir() {
        let mut o = okuyucu(vec![0; 123]);
        assert_eq!(o.dosya_boyutu().unwrap(), 123);
    }

    #[test]
    fn cozumle_cok_kisa_dosyayi_kapsul_disi_sayar() {
        let gecici = crate::test_yardimci::GeciciDizin::yeni("clipforge-iso-kisa").unwrap();
        let yol = gecici.yol().join("kisa.mp4");
        std::fs::write(&yol, b"ftyp").unwrap();
        let hata = cozumle(&yol, Ayarlar::default()).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::BilinmeyenKapsul { .. }));
    }
}
