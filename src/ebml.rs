//! Matroska ve WebM kapsül ayrıştırıcısı (EBML).
//!
//! Bu modül yalnızca kapsul başlıklarını okur; küme (`Cluster`) yüklerini
//! gezmez ve ses/görüntü verisine dokunmaz.
//!
//! # EBML temelleri
//!
//! Her EBML öğesi iki alandan oluşur: bir **kimlik** (Element ID) ve bir
//! **boyut** (Data Size). İkisi de değişken uzunluklu tam sayılardır (VINT).
//! Değişken uzunluklu tam sayının ilk baytındaki işaret biti kaç baytın
//! kullanılacağını söyler: kimlikte işaret biti **korunur**, boyutta
//! **çıkarılır** ve kalan değerler `all-ones` ise boyut "bilinmiyor" demektir.
//!
//! Bu davranışın kaynağı Matroska spesifikasyonudur; ayrıntı için
//! `README.md` → `## Atıflar`.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::hata::ClipForgeHata;
use crate::olcu::KareHazi;

/// Bir dosyada en fazla kaç EBML ögesi başlığı okunacağı.
pub const VARSAYILAN_ASIRI_OGE: usize = 100_000;

/// Bir Matroska dosyasında en fazla kaç parça kaydı bulunabileceği.
pub const VARSAYILAN_ASIRI_PARCA: usize = 64;

/// Yürüyücüyü zorlayan güvenlik sınırları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ayarlar {
    /// Toplam kaç öge başlığı okunabileceği.
    pub asiri_oge: usize,
    /// En fazla kaç `TrackEntry` kabul edileceği.
    pub asiri_parca: usize,
    /// En fazla kaç bayt okunabileceği (kümelerin atlanması bu bütçeye girmez).
    pub okuma_butcesi: u64,
}

impl Default for Ayarlar {
    fn default() -> Self {
        Self {
            asiri_oge: VARSAYILAN_ASIRI_OGE,
            asiri_parca: VARSAYILAN_ASIRI_PARCA,
            okuma_butcesi: 8 * 1024 * 1024,
        }
    }
}

/// EBML büyük boyut öğesi (`0x1A45DFA3`).
pub const ID_EBML: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];
/// `Segment` öğesi (`0x18538067`).
pub const ID_SEGMENT: [u8; 4] = [0x18, 0x53, 0x80, 0x67];
/// `Info` öğesi (`0x1549A966`).
pub const ID_INFO: [u8; 4] = [0x15, 0x49, 0xA9, 0x66];
/// `Tracks` öğesi (`0x1654AE6B`).
pub const ID_TRACKS: [u8; 4] = [0x16, 0x54, 0xAE, 0x6B];
/// `TrackEntry` öğesi (`0xAE`). Tek baytlık bir EBML kimliğidir.
pub const ID_TRACK_ENTRY: [u8; 1] = [0xAE];
/// `DocType` öğesi (`0x4282`).
pub const ID_DOCTYPE: [u8; 2] = [0x42, 0x82];
/// `TimecodeScale` öğesi (`0x2AD7B1`).
pub const ID_TIMECODE_SCALE: [u8; 3] = [0x2A, 0xD7, 0xB1];
/// `Duration` öğesi (`0x4489`).
pub const ID_DURATION: [u8; 2] = [0x44, 0x89];
/// `TrackNumber` öğesi (`0xD7`).
pub const ID_TRACK_NUMBER: [u8; 1] = [0xD7];
/// `TrackType` öğesi (`0x83`).
pub const ID_TRACK_TYPE: [u8; 1] = [0x83];
/// `CodecID` öğesi (`0x86`).
pub const ID_CODEC_ID: [u8; 1] = [0x86];
/// `DefaultDuration` öğesi (`0x23E383`).
pub const ID_DEFAULT_DURATION: [u8; 3] = [0x23, 0xE3, 0x83];
/// `Video` öğesi (`0xE0`).
pub const ID_VIDEO: [u8; 1] = [0xE0];
/// `Audio` öğesi (`0xE1`).
pub const ID_AUDIO: [u8; 1] = [0xE1];
/// `PixelWidth` öğesi (`0xB0`).
pub const ID_PIXEL_WIDTH: [u8; 1] = [0xB0];
/// `PixelHeight` öğesi (`0xBA`).
pub const ID_PIXEL_HEIGHT: [u8; 1] = [0xBA];
/// `SamplingFrequency` öğesi (`0xB5`).
pub const ID_SAMPLING_FREQUENCY: [u8; 1] = [0xB5];
/// `Channels` öğesi (`0x9F`).
pub const ID_CHANNELS: [u8; 1] = [0x9F];

/// Bir EBML ögesinin çözülmüş başlığı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OgeBasligi {
    /// Ögenin dosya içindeki başlangıç ofseti.
    pub ofset: u64,
    /// Öge kimliğinin baytları (uzunluğu 1 ile 4 arasında).
    pub kimlik: Vec<u8>,
    /// Veri bölümünün boyutu.
    pub boyut: u64,
    /// Boyut alanının uzunluğu (bayt).
    pub boyut_uzunlugu: usize,
    /// Boyut alanında tüm değer bitleri `1` ise boyut bilinmiyor demektir.
    pub boyut_bilinmiyor: bool,
}

impl OgeBasligi {
    /// Veri bölümünün başladığı ofseti döndürür.
    pub fn veri_ofseti(&self) -> u64 {
        self.ofset + self.kimlik.len() as u64 + self.boyut_uzunlugu as u64
    }

    /// Başlıktan sonraki ögenin başlangıç ofsetini döndürür.
    ///
    /// Boyut bilinmiyorsa dosya sonu döner; bu durumda çağıran taraf
    /// gezinmeyi durdurmalıdır.
    pub fn sonraki_ofset(&self, dosya_boyutu: u64) -> u64 {
        if self.boyut_bilinmiyor {
            return dosya_boyutu;
        }
        self.veri_ofseti() + self.boyut
    }

    /// Kimliği dört bayta tamamlar (kısa kimlikler `0x00` ile doldurulur).
    pub fn kimlik4(&self) -> [u8; 4] {
        let mut sonuc = [0_u8; 4];
        for (hedef, kaynak) in sonuc.iter_mut().zip(self.kimlik.iter()) {
            *hedef = *kaynak;
        }
        sonuc
    }
}

/// Bir Matroska parçasından toplanan alanlar.
#[derive(Debug, Clone, PartialEq)]
pub struct ParcaKaydi {
    /// Parça numarası (`TrackNumber`).
    pub numara: u64,
    /// `TrackType`: 1 video, 2 audio, 17 subtitle.
    pub tur_kodu: u64,
    /// Codec kimliği (ör. `V_MPEG4/ISO/AVC`, `A_OPUS`).
    pub codec: String,
    /// Kodlanmış görüntü genişliği.
    pub genislik: Option<u32>,
    /// Kodlanmış görüntü yüksekliği.
    pub yukseklik: Option<u32>,
    /// Kare başına varsayılan süre (ns), kare hızı buradan türetilir.
    pub varsayilan_sure_ns: Option<u64>,
    /// Ses örnekleme hızı (Hz).
    pub ornekleme_hizi: Option<f64>,
    /// Ses kanal sayısı.
    pub kanal: Option<u64>,
}

impl ParcaKaydi {
    /// Parçanın türünü döndürür.
    pub fn tur(&self) -> crate::iso_bmff::ParcasiTuru {
        use crate::iso_bmff::ParcasiTuru;
        match self.tur_kodu {
            1 => ParcasiTuru::Video,
            2 => ParcasiTuru::Ses,
            _ => ParcasiTuru::Diger,
        }
    }

    /// Parçanın kare hızını `DefaultDuration` alanından türetir.
    pub fn kare_hazi(&self) -> Option<KareHazi> {
        let ns = self.varsayilan_sure_ns?;
        if ns == 0 {
            return None;
        }
        KareHazi::sabit(1_000_000_000.0 / ns as f64).ok()
    }
}

/// Bir Matroska/WebM dosyasından toplanan tüm bilgiler.
#[derive(Debug, Clone, PartialEq)]
pub struct MatroskaVerisi {
    /// `DocType` metni (`matroska` ya da `webm`).
    pub dokuman_tipi: String,
    /// `TimecodeScale` (ns, varsayılan 1 000 000).
    pub zaman_olcegi_ns: u64,
    /// `Duration` (zaman ölçeği biriminde).
    pub ham_sure: Option<f64>,
    /// `Segment` ögesinin ofseti.
    pub segment_ofseti: u64,
    /// Parça listesi.
    pub parcalar: Vec<ParcaKaydi>,
    /// Taranan öge sayısı.
    pub oge_sayisi: usize,
    /// Okunan toplam bayt sayısı.
    pub okunan_bayt: u64,
}

impl MatroskaVerisi {
    /// Süreyi saniyeye çevirir.
    pub fn sure_sn(&self) -> f64 {
        match self.ham_sure {
            Some(ham) if self.zaman_olcegi_ns > 0 => {
                ham * self.zaman_olcegi_ns as f64 / 1_000_000_000.0
            }
            _ => 0.0,
        }
    }
}

struct Okuma {
    yol: std::path::PathBuf,
    ic: File,
    ayarlar: Ayarlar,
    okunan: u64,
}

impl Okuma {
    fn ac(yol: &Path, ayarlar: Ayarlar) -> Result<Self, ClipForgeHata> {
        let ic = File::open(yol).map_err(|hata| ClipForgeHata::girdi(yol, hata))?;
        Ok(Self {
            yol: yol.to_path_buf(),
            ic,
            ayarlar,
            okunan: 0,
        })
    }

    fn dosya_boyutu(&mut self) -> Result<u64, ClipForgeHata> {
        self.ic
            .seek(SeekFrom::End(0))
            .map_err(|hata| ClipForgeHata::girdi(&self.yol, hata))
    }

    fn bayt_oku(&mut self, ofset: u64, adet: usize) -> Result<Vec<u8>, ClipForgeHata> {
        if self.okunan + adet as u64 > self.ayarlar.okuma_butcesi {
            return Err(ClipForgeHata::AsiriKutu {
                tip: "ebml".to_string(),
                boyut: self.okunan + adet as u64,
                sinir: self.ayarlar.okuma_butcesi,
            });
        }
        self.ic
            .seek(SeekFrom::Start(ofset))
            .map_err(|hata| ClipForgeHata::girdi(&self.yol, hata))?;
        let mut tampon = vec![0_u8; adet];
        self.ic.read_exact(&mut tampon).map_err(|hata| {
            if hata.kind() == io::ErrorKind::UnexpectedEof {
                ClipForgeHata::EksikKutu {
                    yol: format!("@{ofset} (ebml)"),
                    ayrinti: format!("{adet} bayt bekleniyordu, dosya sonu"),
                }
            } else {
                ClipForgeHata::girdi(&self.yol, hata)
            }
        })?;
        self.okunan += adet as u64;
        Ok(tampon)
    }

    /// Değişken uzunluklu tam sayının kaç bayttan oluşacağını ilk bayttan bulur.
    ///
    /// `None`, bayt `0x00` ise döner: bu, geçersiz bir VINT başlangıcıdır.
    fn uzunluk(bayt: u8) -> Option<usize> {
        match bayt {
            0x80..=0xfe => Some(1),
            0x40..=0x7f => Some(2),
            0x20..=0x3f => Some(3),
            0x10..=0x1f => Some(4),
            0x08..=0x0f => Some(5),
            0x04..=0x07 => Some(6),
            0x02..=0x03 => Some(7),
            0x01 => Some(8),
            _ => None,
        }
    }

    /// Bir öge başlığını okur.
    ///
    /// EBML'de **kimlik** ve **boyut** alanlarının her biri kendi VINT uzunluk
    /// tanımlayıcısını taşır. Uzunluklar birbirinden bağımsızdır: dört
    /// baytlık bir kimliğin ardından tek baytlık bir boyut gelebilir.
    fn basligi_oku(&mut self, ofset: u64, dosya_boyutu: u64) -> Result<OgeBasligi, ClipForgeHata> {
        let ilk = self.bayt_oku(ofset, 1)?;
        let kimlik_uzunluk = Self::uzunluk(ilk[0]).ok_or_else(|| ClipForgeHata::BozukBaslik {
            ofset,
            ayrinti: "EBML kimlik VINT baslangici gecersiz".to_string(),
        })?;
        if kimlik_uzunluk > 4 {
            return Err(ClipForgeHata::BozukBaslik {
                ofset,
                ayrinti: format!("EBML kimligi {kimlik_uzunluk} bayt olamaz (en fazla 4)"),
            });
        }
        let kimlik = self.bayt_oku(ofset, kimlik_uzunluk)?;
        let boyut_ofseti = ofset + kimlik_uzunluk as u64;
        let boyut_ilk = self.bayt_oku(boyut_ofseti, 1)?;
        let boyut_uzunluk =
            Self::uzunluk(boyut_ilk[0]).ok_or_else(|| ClipForgeHata::BozukBaslik {
                ofset: boyut_ofseti,
                ayrinti: "EBML boyut VINT baslangici gecersiz".to_string(),
            })?;
        let ham = self.bayt_oku(boyut_ofseti, boyut_uzunluk)?;
        // `L` baytlık bir VINT'te ilk bayttaki en yüksek `L` bit tanımlayıcıdır;
        // kalan `7L` bit değerdir.
        let deger_mas = (1_u64 << (7 * boyut_uzunluk)) - 1;
        let deger = ham
            .iter()
            .fold(0_u64, |birikim, bayt| (birikim << 8) | u64::from(*bayt))
            & deger_mas;
        let baslik = OgeBasligi {
            ofset,
            kimlik,
            boyut: deger,
            boyut_uzunlugu: boyut_uzunluk,
            // Tüm değer bitleri 1 ise boyut bilinmiyor demektir.
            boyut_bilinmiyor: deger == deger_mas,
        };
        if !baslik.boyut_bilinmiyor && baslik.veri_ofseti() + baslik.boyut > dosya_boyutu {
            return Err(ClipForgeHata::GecersizBoyut {
                ofset,
                boyut: baslik.boyut,
                dosya_boyutu,
            });
        }
        Ok(baslik)
    }
}

/// Bir tam sayı alanını bayt dizisinden okur (işaretsiz, büyük uç).
fn tam_sayi(baytlar: &[u8]) -> u64 {
    baytlar
        .iter()
        .fold(0_u64, |birikim, bayt| (birikim << 8) | u64::from(*bayt))
}

/// Bir IEEE-754 çift duyarlıklı alanı okur (Matroska `Duration` ve
/// `SamplingFrequency` alanları 4 ya da 8 bayttır).
fn kayan_sayi(baytlar: &[u8]) -> Option<f64> {
    match baytlar.len() {
        4 => Some(f64::from(f32::from_be_bytes([
            baytlar[0], baytlar[1], baytlar[2], baytlar[3],
        ]))),
        8 => Some(f64::from_be_bytes([
            baytlar[0], baytlar[1], baytlar[2], baytlar[3], baytlar[4], baytlar[5], baytlar[6],
            baytlar[7],
        ])),
        _ => None,
    }
}

fn metin(baytlar: &[u8]) -> String {
    String::from_utf8_lossy(baytlar)
        .trim_end_matches('\0')
        .to_string()
}

fn esit(baslik: &OgeBasligi, kimlik: &[u8]) -> bool {
    baslik.kimlik == kimlik
}

/// Bir Matroska/WebM dosyasının kapsul bilgilerini okur.
///
/// # Hatalar
///
/// `EBML` başlığı yoksa [`ClipForgeHata::BilinmeyenKapsul`], bozuk VINT ya da
/// aşılan sınır durumlarında ilgili [`ClipForgeHata`] varyantı döner.
pub fn cozumle(yol: &Path, ayarlar: Ayarlar) -> Result<MatroskaVerisi, ClipForgeHata> {
    let mut okuma = Okuma::ac(yol, ayarlar)?;
    let dosya_boyutu = okuma.dosya_boyutu()?;
    // İmza denetimi VINT çözümünden önce yapılır: ilk dört bayt `EBML`
    // (0x1A45DFA3) imzası değilse dosya Matroska değildir.
    let imza = okuma.bayt_oku(0, 4)?;
    if imza != ID_EBML {
        return Err(ClipForgeHata::BilinmeyenKapsul {
            yol: yol.to_path_buf(),
            sebep: format!("EBML imzasi yok (ilk baytlar: {imza:02x?})"),
        });
    }
    let kok = okuma.basligi_oku(0, dosya_boyutu)?;
    if !esit(&kok, &ID_EBML) {
        return Err(ClipForgeHata::BilinmeyenKapsul {
            yol: yol.to_path_buf(),
            sebep: "ilk oge 'EBML' degil".to_string(),
        });
    }

    let mut veri = MatroskaVerisi {
        dokuman_tipi: String::new(),
        zaman_olcegi_ns: 1_000_000,
        ham_sure: None,
        segment_ofseti: 0,
        parcalar: Vec::new(),
        oge_sayisi: 1,
        okunan_bayt: 0,
    };

    // EBML başlık öğesi içindeki DocType.
    let mut konum = kok.veri_ofseti();
    let bitis = kok.sonraki_ofset(dosya_boyutu);
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_DOCTYPE) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            veri.dokuman_tipi = metin(&ham);
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }

    // Segment ve içindeki Info/Tracks.
    let mut segment = None;
    let mut konum = 0;
    let bitis = dosya_boyutu;
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_SEGMENT) {
            veri.segment_ofseti = baslik.ofset;
            segment = Some(baslik.veri_ofseti());
            break;
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }

    let segment_baslangic = segment.ok_or_else(|| ClipForgeHata::EksikKutu {
        yol: "Segment".to_string(),
        ayrinti: "kapsulde Segment ogesi bulunamadi".to_string(),
    })?;
    let segment_bitis = dosya_boyutu;

    let mut konum = segment_baslangic;
    while konum + 2 <= segment_bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_INFO) {
            info_oku(&mut okuma, &baslik, &mut veri, dosya_boyutu)?;
        } else if esit(&baslik, &ID_TRACKS) {
            parcalar_oku(&mut okuma, &baslik, &mut veri, dosya_boyutu)?;
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }

    veri.okunan_bayt = okuma.okunan;
    Ok(veri)
}

/// `Info` ögesinden zaman ölçeği ve süreyi okur.
fn info_oku(
    okuma: &mut Okuma,
    info: &OgeBasligi,
    veri: &mut MatroskaVerisi,
    dosya_boyutu: u64,
) -> Result<(), ClipForgeHata> {
    let mut konum = info.veri_ofseti();
    let bitis = info.sonraki_ofset(dosya_boyutu);
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_TIMECODE_SCALE) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            veri.zaman_olcegi_ns = tam_sayi(&ham);
        } else if esit(&baslik, &ID_DURATION) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            veri.ham_sure = kayan_sayi(&ham);
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }
    Ok(())
}

/// `Tracks` ögesinden parça kayıtlarını okur.
fn parcalar_oku(
    okuma: &mut Okuma,
    tracks: &OgeBasligi,
    veri: &mut MatroskaVerisi,
    dosya_boyutu: u64,
) -> Result<(), ClipForgeHata> {
    let mut konum = tracks.veri_ofseti();
    let bitis = tracks.sonraki_ofset(dosya_boyutu);
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_TRACK_ENTRY) {
            if veri.parcalar.len() >= okuma.ayarlar.asiri_parca {
                return Err(ClipForgeHata::YinelemeSiniri {
                    tur: "parca".to_string(),
                    sinir: okuma.ayarlar.asiri_parca as u64,
                });
            }
            let kayit = track_entry_oku(okuma, &baslik, veri, dosya_boyutu)?;
            veri.parcalar.push(kayit);
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }
    Ok(())
}

/// Tek bir `TrackEntry` ögesini okur.
fn track_entry_oku(
    okuma: &mut Okuma,
    giris: &OgeBasligi,
    veri: &mut MatroskaVerisi,
    dosya_boyutu: u64,
) -> Result<ParcaKaydi, ClipForgeHata> {
    let mut kayit = ParcaKaydi {
        numara: 0,
        tur_kodu: 0,
        codec: String::new(),
        genislik: None,
        yukseklik: None,
        varsayilan_sure_ns: None,
        ornekleme_hizi: None,
        kanal: None,
    };
    let mut konum = giris.veri_ofseti();
    let bitis = giris.sonraki_ofset(dosya_boyutu);
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_TRACK_NUMBER) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.numara = tam_sayi(&ham);
        } else if esit(&baslik, &ID_TRACK_TYPE) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.tur_kodu = tam_sayi(&ham);
        } else if esit(&baslik, &ID_CODEC_ID) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.codec = metin(&ham);
        } else if esit(&baslik, &ID_DEFAULT_DURATION) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.varsayilan_sure_ns = Some(tam_sayi(&ham));
        } else if esit(&baslik, &ID_VIDEO) {
            video_oku(okuma, &baslik, &mut kayit, veri, dosya_boyutu)?;
        } else if esit(&baslik, &ID_AUDIO) {
            audio_oku(okuma, &baslik, &mut kayit, veri, dosya_boyutu)?;
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }
    Ok(kayit)
}

/// `Video` alt ögesinden genişlik ve yüksekliği okur.
fn video_oku(
    okuma: &mut Okuma,
    video: &OgeBasligi,
    kayit: &mut ParcaKaydi,
    veri: &mut MatroskaVerisi,
    dosya_boyutu: u64,
) -> Result<(), ClipForgeHata> {
    let mut konum = video.veri_ofseti();
    let bitis = video.sonraki_ofset(dosya_boyutu);
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_PIXEL_WIDTH) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.genislik = Some(tam_sayi(&ham) as u32);
        } else if esit(&baslik, &ID_PIXEL_HEIGHT) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.yukseklik = Some(tam_sayi(&ham) as u32);
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }
    Ok(())
}

/// `Audio` alt ögesinden örnekleme hızını ve kanal sayısını okur.
fn audio_oku(
    okuma: &mut Okuma,
    audio: &OgeBasligi,
    kayit: &mut ParcaKaydi,
    veri: &mut MatroskaVerisi,
    dosya_boyutu: u64,
) -> Result<(), ClipForgeHata> {
    let mut konum = audio.veri_ofseti();
    let bitis = audio.sonraki_ofset(dosya_boyutu);
    while konum + 2 <= bitis {
        veri.oge_sayisi += 1;
        if veri.oge_sayisi > okuma.ayarlar.asiri_oge {
            return Err(ClipForgeHata::YinelemeSiniri {
                tur: "ebml oge".to_string(),
                sinir: okuma.ayarlar.asiri_oge as u64,
            });
        }
        let baslik = okuma.basligi_oku(konum, dosya_boyutu)?;
        if esit(&baslik, &ID_SAMPLING_FREQUENCY) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.ornekleme_hizi = kayan_sayi(&ham);
        } else if esit(&baslik, &ID_CHANNELS) {
            let ham = okuma.bayt_oku(baslik.veri_ofseti(), baslik.boyut as usize)?;
            kayit.kanal = Some(tam_sayi(&ham));
        }
        if baslik.boyut_bilinmiyor {
            break;
        }
        konum = baslik.sonraki_ofset(dosya_boyutu);
    }
    Ok(())
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_yardimci::GeciciDizin;

    /// EBML kimliğini bayt dizisine yazar.
    fn kimlik_yaz(veri: &mut Vec<u8>, kimlik: &[u8]) {
        veri.extend_from_slice(kimlik);
    }

    /// Tek baytlık boyut alanı yazar ve içeriği ekler.
    fn oge_yaz(veri: &mut Vec<u8>, kimlik: &[u8], icerik: &[u8]) {
        kimlik_yaz(veri, kimlik);
        veri.push(0x80 | (icerik.len() as u8));
        veri.extend_from_slice(icerik);
    }

    /// Boyutu bilinmeyen öge yazar (`01 FF FF FF FF FF FF FF`).
    fn bilinmeyen_boyutlu_oge_yaz(veri: &mut Vec<u8>, kimlik: &[u8]) {
        kimlik_yaz(veri, kimlik);
        veri.push(0x01);
        veri.extend_from_slice(&[0xFF; 7]);
    }

    /// Geçerli bir `EBML` başlığı, `Segment` ve verilen `Segment` içeriğini
    /// içeren dosya baytlarını üretir.
    ///
    /// Başlık önce içeriği toplanarak yazılmalıdır: EBML boyut alanı gövde
    /// uzunluğunu bildirdiği için, gövde hazır olmadan başlık yazılamaz.
    fn dosya_uret(segment_icerik: &[u8]) -> Vec<u8> {
        let mut baslik = Vec::new();
        oge_yaz(&mut baslik, &[0x42, 0x86], &[1]);
        oge_yaz(&mut baslik, &[0x42, 0xF7], &[1]);
        oge_yaz(&mut baslik, &ID_DOCTYPE, b"matroska");
        oge_yaz(&mut baslik, &[0x42, 0x87], &[4]);

        let mut sonuc = Vec::new();
        kimlik_yaz(&mut sonuc, &ID_EBML);
        sonuc.push(0x80 | (baslik.len() as u8));
        sonuc.extend_from_slice(&baslik);
        bilinmeyen_boyutlu_oge_yaz(&mut sonuc, &ID_SEGMENT);
        sonuc.extend_from_slice(segment_icerik);
        sonuc
    }

    /// Ondalık (unsigned int) alanı en kısa büyük uç gösteriminde üretir.
    ///
    /// EBML, tam sayı alanlarında baştaki sıfır baytlarını taşımaz; sıfır
    /// değeri tek bayt `0x00` olarak yazılır.
    fn tam_sayi_bayt(deger: u64) -> Vec<u8> {
        let baytlar = deger.to_be_bytes();
        match baytlar.iter().position(|bayt| *bayt != 0) {
            Some(baslangic) => baytlar[baslangic..].to_vec(),
            None => vec![0],
        }
    }

    #[test]
    fn oge_basligi_vint_uzunlugu_cozulur() {
        assert_eq!(Okuma::uzunluk(0xA3), Some(1));
        assert_eq!(Okuma::uzunluk(0x42), Some(2));
        assert_eq!(Okuma::uzunluk(0x2A), Some(3));
        assert_eq!(Okuma::uzunluk(0x10), Some(4));
        assert_eq!(Okuma::uzunluk(0x01), Some(8));
        assert_eq!(Okuma::uzunluk(0x00), None);
    }

    #[test]
    fn tam_sayi_ve_kayan_sayi_ayristirilir() {
        assert_eq!(tam_sayi(&[0x01, 0x02, 0x03]), 0x010203);
        assert_eq!(tam_sayi(&[]), 0);
        assert_eq!(kayan_sayi(&1000.0_f64.to_be_bytes()), Some(1000.0));
        assert_eq!(kayan_sayi(&1.5_f32.to_be_bytes()), Some(1.5));
        assert_eq!(kayan_sayi(&[0, 0, 0]), None);
        assert_eq!(metin(b"matroska\0"), "matroska");
    }

    /// Test dosyasına ham bayt yazar ve yolu döndürür.
    fn yaz(gecici: &GeciciDizin, ad: &str, veri: &[u8]) -> std::path::PathBuf {
        let yol = gecici.yol().join(ad);
        std::fs::write(&yol, veri).unwrap();
        yol
    }

    /// Geçerli bir Matroska başlığı ve tek video parçası üretir.
    fn ornek_matroska() -> Vec<u8> {
        let mut ebml = Vec::new();
        oge_yaz(&mut ebml, &ID_DOCTYPE, b"matroska");
        oge_yaz(&mut ebml, &[0x42, 0x87], b"4");

        let mut video = Vec::new();
        oge_yaz(&mut video, &ID_PIXEL_WIDTH, &tam_sayi_bayt(1920));
        oge_yaz(&mut video, &ID_PIXEL_HEIGHT, &tam_sayi_bayt(1080));

        let mut ses = Vec::new();
        oge_yaz(
            &mut ses,
            &ID_SAMPLING_FREQUENCY,
            &48_000.0_f64.to_be_bytes(),
        );
        oge_yaz(&mut ses, &ID_CHANNELS, &tam_sayi_bayt(2));

        let mut parca1 = Vec::new();
        oge_yaz(&mut parca1, &ID_TRACK_NUMBER, &tam_sayi_bayt(1));
        oge_yaz(&mut parca1, &ID_TRACK_TYPE, &tam_sayi_bayt(1));
        oge_yaz(&mut parca1, &ID_CODEC_ID, b"V_MPEG4/ISO/AVC");
        oge_yaz(
            &mut parca1,
            &ID_DEFAULT_DURATION,
            &tam_sayi_bayt(33_333_333),
        );
        oge_yaz(&mut parca1, &ID_VIDEO, &video);

        let mut parca2 = Vec::new();
        oge_yaz(&mut parca2, &ID_TRACK_NUMBER, &tam_sayi_bayt(2));
        oge_yaz(&mut parca2, &ID_TRACK_TYPE, &tam_sayi_bayt(2));
        oge_yaz(&mut parca2, &ID_CODEC_ID, b"A_OPUS");
        oge_yaz(&mut parca2, &ID_AUDIO, &ses);

        let mut tracks = Vec::new();
        oge_yaz(&mut tracks, &ID_TRACK_ENTRY, &parca1);
        oge_yaz(&mut tracks, &ID_TRACK_ENTRY, &parca2);

        let mut info = Vec::new();
        oge_yaz(&mut info, &ID_TIMECODE_SCALE, &tam_sayi_bayt(1_000_000));
        oge_yaz(&mut info, &ID_DURATION, &20_000.0_f64.to_be_bytes());

        let mut segment = Vec::new();
        oge_yaz(&mut segment, &ID_INFO, &info);
        oge_yaz(&mut segment, &ID_TRACKS, &tracks);

        let mut kok = Vec::new();
        kimlik_yaz(&mut kok, &ID_EBML);
        kok.push(0x80 | (ebml.len() as u8));
        kok.extend_from_slice(&ebml);
        // Segment'in boyutu bilinmiyor olarak yazılır: canlı akışlarda yaygındır.
        bilinmeyen_boyutlu_oge_yaz(&mut kok, &ID_SEGMENT);
        kok.extend_from_slice(&segment);
        kok
    }

    #[test]
    fn matroska_basi_ve_parcalar_okunur() {
        let gecici = GeciciDizin::yeni("clipforge-mkv").unwrap();
        let yol = yaz(&gecici, "ornek.mkv", &ornek_matroska());
        let veri = cozumle(&yol, Ayarlar::default()).unwrap();
        assert_eq!(veri.dokuman_tipi, "matroska");
        assert_eq!(veri.zaman_olcegi_ns, 1_000_000);
        assert_eq!(veri.sure_sn(), 20.0);
        assert_eq!(veri.parcalar.len(), 2);

        let video = &veri.parcalar[0];
        assert_eq!(video.tur_kodu, 1);
        assert_eq!(video.tur(), crate::iso_bmff::ParcasiTuru::Video);
        assert_eq!(video.genislik, Some(1920));
        assert_eq!(video.yukseklik, Some(1080));
        assert_eq!(video.codec, "V_MPEG4/ISO/AVC");
        let hiz = video.kare_hazi().unwrap();
        assert!((hiz.deger() - 30.0).abs() < 0.001);

        let ses = &veri.parcalar[1];
        assert_eq!(ses.tur(), crate::iso_bmff::ParcasiTuru::Ses);
        assert_eq!(ses.ornekleme_hizi, Some(48_000.0));
        assert_eq!(ses.kanal, Some(2));
        assert!(ses.kare_hazi().is_none());
    }

    #[test]
    fn eble_basligi_olmayan_dosya_kapsul_disi_sayar() {
        let gecici = GeciciDizin::yeni("clipforge-mkv-yok").unwrap();
        let yol = yaz(&gecici, "bozuk.mkv", &[0u8; 32]);
        let hata = cozumle(&yol, Ayarlar::default()).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::BilinmeyenKapsul { .. }));
    }

    #[test]
    fn segment_olmayan_matroska_hata_verir() {
        let gecici = GeciciDizin::yeni("clipforge-mkv-segment").unwrap();
        // Geçerli EBML başlığı, ardından yalnızca bir `Void` ögesi (0xEC).
        let mut baslik = Vec::new();
        oge_yaz(&mut baslik, &ID_DOCTYPE, b"matroska");
        let mut veri = Vec::new();
        kimlik_yaz(&mut veri, &ID_EBML);
        veri.push(0x80 | (baslik.len() as u8));
        veri.extend_from_slice(&baslik);
        oge_yaz(&mut veri, &[0xEC], &[0, 0, 0, 0]);
        let yol = yaz(&gecici, "segment-yok.mkv", &veri);
        let hata = cozumle(&yol, Ayarlar::default()).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::EksikKutu { .. }));
    }

    #[test]
    fn parca_siniri_asisinda_durulur() {
        let gecici = GeciciDizin::yeni("clipforge-mkv-limit").unwrap();
        let mut parca = Vec::new();
        oge_yaz(&mut parca, &ID_TRACK_TYPE, &tam_sayi_bayt(1));
        let mut tracks = Vec::new();
        for _ in 0..4 {
            oge_yaz(&mut tracks, &ID_TRACK_ENTRY, &parca);
        }
        let mut segment = Vec::new();
        oge_yaz(&mut segment, &ID_TRACKS, &tracks);
        let yol = yaz(&gecici, "limit.mkv", &dosya_uret(&segment));
        let ayarlar = Ayarlar {
            asiri_parca: 2,
            ..Ayarlar::default()
        };
        let hata = cozumle(&yol, ayarlar).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::YinelemeSiniri { .. }));
    }

    #[test]
    fn okuma_butcesi_kutuyu_reddeder() {
        let gecici = GeciciDizin::yeni("clipforge-mkv-butce").unwrap();
        let yol = yaz(&gecici, "butce.mkv", &ornek_matroska());
        let ayarlar = Ayarlar {
            okuma_butcesi: 4,
            ..Ayarlar::default()
        };
        let hata = cozumle(&yol, ayarlar).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::AsiriKutu { .. }));
    }

    #[test]
    fn sure_yoksa_sifir_doner() {
        let gecici = GeciciDizin::yeni("clipforge-mkv-sure").unwrap();
        let mut info = Vec::new();
        oge_yaz(&mut info, &ID_TIMECODE_SCALE, &tam_sayi_bayt(1_000_000));
        let mut segment = Vec::new();
        oge_yaz(&mut segment, &ID_INFO, &info);
        let yol = yaz(&gecici, "suresiz.mkv", &dosya_uret(&segment));
        let cozulen = cozumle(&yol, Ayarlar::default()).unwrap();
        assert_eq!(cozulen.ham_sure, None);
        assert_eq!(cozulen.sure_sn(), 0.0);
    }

    #[test]
    fn oge_basligi_kimlik_doldurma_ve_sonraki_ofset() {
        let baslik = OgeBasligi {
            ofset: 10,
            kimlik: vec![0xAE],
            boyut: 4,
            boyut_uzunlugu: 1,
            boyut_bilinmiyor: false,
        };
        assert_eq!(baslik.kimlik4(), [0xAE, 0, 0, 0]);
        assert_eq!(baslik.veri_ofseti(), 12);
        assert_eq!(baslik.sonraki_ofset(100), 16);
        let bilinmeyen = OgeBasligi {
            boyut_bilinmiyor: true,
            ..baslik
        };
        assert_eq!(bilinmeyen.sonraki_ofset(77), 77);
    }
}
