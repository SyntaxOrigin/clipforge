//! Biçimden bağımsız medya bilgisi ve iki ayrıştırıcının ortak katmanı.
//!
//! `iso_bmff` ve `ebml` modülleri yalnızca kendi kutu yapılarını okur; bu
//! modül ikisini tek bir [`MedyaBilgisi`] tipinde birleştirir ve kural
//! denetimlerini (süre sıfır olamaz, çözünürlük olmalı vb.) uygular.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ebml;
use crate::hata::ClipForgeHata;
use crate::iso_bmff::{self, ParcasiTuru};
use crate::olcu::{EnBoy, KareHazi};

/// Tanınan kapsül biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bicim {
    /// ISO/IEC 14496-12 temel medya dosya biçimi (MP4, MOV, M4V).
    #[serde(rename = "iso-bmff")]
    IsoBmff,
    /// Matroska/WebM (EBML tabanlı).
    #[serde(rename = "matroska")]
    Matroska,
}

impl Bicim {
    /// Biçimin plan çıktısındaki kısa adını döndürür.
    pub fn kod(self) -> &'static str {
        match self {
            Self::IsoBmff => "iso-bmff",
            Self::Matroska => "matroska",
        }
    }
}

impl std::fmt::Display for Bicim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kod())
    }
}

/// Bir kapsül içindeki tek bir parçanın (trak/track) özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParcaBilgisi {
    /// Parçanın sıra numarası (dosya içindeki konum).
    pub sira: usize,
    /// Parça kimliği (`tkhd` izi ya da Matroska `TrackNumber`).
    pub iz: u32,
    /// Parçanın türü.
    pub tur: ParcasiTuru,
    /// Kutu hiyerarşisindeki yolu (JSON çıktısında `kutu_yolu`).
    pub kutu_yolu: String,
    /// Codec dört karakterlik kodu (`avc1`, `mp4a`, `V_MPEG4/ISO/AVC` ...).
    pub codec: String,
    /// Parçanın süresi (saniye).
    pub sure_sn: f64,
    /// Kare hızı. Çözülemiyorsa `None` (ör. değişken kare hızlı akış).
    pub kare_hazi: Option<KareHazi>,
    /// Toplam kare/örnek sayısı.
    pub kare_sayisi: Option<u64>,
    /// Görüntü çözünürlüğü. Ses parçalarında `None`.
    pub cozunurluk: Option<(u32, u32)>,
    /// Ses örnekleme hızı. Görüntü parçalarında `None`.
    pub ornekleme_hizi: Option<u32>,
    /// Ses kanal sayısı.
    pub kanal: Option<u16>,
    /// Eşzamanlı (anahtar) kare sayısı.
    pub anahtar_kare_sayisi: Option<u32>,
}

impl ParcaBilgisi {
    /// Görüntü çözünürlüğünün genişliğini döndürür, yoksa `None`.
    pub fn genislik(&self) -> Option<u32> {
        self.cozunurluk.map(|(genislik, _)| genislik)
    }

    /// Görüntü çözünürlüğünün yüksekliğini döndürür, yoksa `None`.
    pub fn yukseklik(&self) -> Option<u32> {
        self.cozunurluk.map(|(_, yukseklik)| yukseklik)
    }

    /// Görüntü çözünürlüğünün en-boy oranını döndürür, yoksa `None`.
    pub fn en_boy(&self) -> Option<EnBoy> {
        let (genislik, yukseklik) = self.cozunurluk?;
        EnBoy::cozunurlukten(genislik, yukseklik).ok()
    }
}

/// Tek bir kapsül dosyasının iki biçimden bağımsız özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MedyaBilgisi {
    /// İncelenen dosyanın yolu.
    pub yol: PathBuf,
    /// Kapsül biçimi.
    pub bicim: Bicim,
    /// Dosyanın toplam boyutu (bayt).
    pub dosya_boyutu: u64,
    /// Kapsülün toplam süresi (saniye). Sıfır olamaz.
    pub sure_sn: f64,
    /// `ftyp` bilgisi (yalnızca ISO BMFF).
    pub marka: Option<String>,
    /// Tüm parçalar.
    pub parcalar: Vec<ParcaBilgisi>,
    /// Birincil görüntü parçasının sırası.
    pub video_indeks: Option<usize>,
    /// `moov` kutusunun ilk `mdat` kutusundan önce olup olmadığı.
    ///
    /// Yalnızca ISO BMFF için anlamlıdır; Matroska'da `None` döner.
    pub faststart: Option<bool>,
    /// Ayarlanan sınırların aşılması gibi uyarılar.
    pub uyarilar: Vec<String>,
}

impl MedyaBilgisi {
    /// Birincil görüntü parçasını döndürür, yoksa `None`.
    pub fn video(&self) -> Option<&ParcaBilgisi> {
        self.video_indeks.and_then(|i| self.parcalar.get(i))
    }

    /// Ses parçası sayısını döndürür.
    pub fn ses_parca_sayisi(&self) -> usize {
        self.parcalar
            .iter()
            .filter(|p| p.tur == ParcasiTuru::Ses)
            .count()
    }

    /// Görüntü parçası sayısını döndürür.
    pub fn video_parca_sayisi(&self) -> usize {
        self.parcalar
            .iter()
            .filter(|p| p.tur == ParcasiTuru::Video)
            .count()
    }
}

/// İki ayrıştırıcının sınırlarını tek yapıda toplayan ayar seti.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ayarlar {
    /// ISO BMFF yürüyücüsünün sınırları.
    pub iso: iso_bmff::Ayarlar,
    /// Matroska yürüyücüsünün sınırları.
    pub ebml: ebml::Ayarlar,
}

/// İlk baytlardan kapsül biçimini tanır.
///
/// Dönüş değeri `None` ise dosya ne ISO BMFF ne de Matroska kabul edilir ve
/// çağıran taraf açık bir hata üretmelidir.
///
/// # Hatalar
///
/// Bu işlev hata üretmez; yalnızca `Option` döner.
pub fn bicim_tespit(ilk_baytlar: &[u8]) -> Option<Bicim> {
    if ilk_baytlar.len() >= 4 && ilk_baytlar[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Some(Bicim::Matroska);
    }
    if ilk_baytlar.len() >= 8 && &ilk_baytlar[4..8] == b"ftyp" {
        return Some(Bicim::IsoBmff);
    }
    // `ftyp` kutusu olmayan (ör. MOV akışı) dosyalarda ilk kutu tipine bakılır.
    if ilk_baytlar.len() >= 8 {
        let tip: [u8; 4] = [
            ilk_baytlar[4],
            ilk_baytlar[5],
            ilk_baytlar[6],
            ilk_baytlar[7],
        ];
        const ISO_TIPLER: [&[u8; 4]; 6] = [b"moov", b"mdat", b"free", b"skip", b"wide", b"uuid"];
        if ISO_TIPLER.contains(&&tip) {
            return Some(Bicim::IsoBmff);
        }
    }
    None
}

/// Dosyanın kapsül bilgilerini okur; biçim otomatik olarak tanınır.
///
/// # Hatalar
///
/// Dosya açılamazsa [`ClipForgeHata::GirdiHatasi`], kapsül tanınamazsa
/// [`ClipForgeHata::BilinmeyenKapsul`], kutu hatalarında ilgili ayrıştırıcının
/// döndürdüğü hata, süre sıfırsa [`ClipForgeHata::SureSifir`] ve görüntü parçası
/// çözünürlüksüzse [`ClipForgeHata::CozunurlukYok`] döner.
pub fn arastir(yol: &Path) -> Result<MedyaBilgisi, ClipForgeHata> {
    arastir_ayarla(yol, Ayarlar::default())
}

/// [`arastir`] işlevinin sınırları ayarlanabilir hâli (testler ve sıkı mod için).
///
/// # Hatalar
///
/// [`arastir`] ile aynı koşullarda hata döner.
pub fn arastir_ayarla(yol: &Path, ayarlar: Ayarlar) -> Result<MedyaBilgisi, ClipForgeHata> {
    use std::io::Read;

    let mut ilk = [0_u8; 16];
    let okunan = {
        let mut dosya = std::fs::File::open(yol).map_err(|hata| ClipForgeHata::girdi(yol, hata))?;
        dosya
            .read(&mut ilk)
            .map_err(|hata| ClipForgeHata::girdi(yol, hata))?
    };
    let bicim = bicim_tespit(&ilk[..okunan]).ok_or_else(|| ClipForgeHata::BilinmeyenKapsul {
        yol: yol.to_path_buf(),
        sebep: format!(
            "imza taninmadi (ilk baytlar: {:02x?})",
            &ilk[..okunan.min(8)]
        ),
    })?;
    match bicim {
        Bicim::IsoBmff => iso_cozumle(yol, ayarlar.iso),
        Bicim::Matroska => matroska_cozumle(yol, ayarlar.ebml),
    }
}

/// ISO BMFF dosyasını okur ve kural denetimlerini uygular.
fn iso_cozumle(yol: &Path, ayarlar: iso_bmff::Ayarlar) -> Result<MedyaBilgisi, ClipForgeHata> {
    let veri = iso_bmff::cozumle(yol, ayarlar)?;
    let dosya_boyutu = std::fs::metadata(yol)
        .map_err(|hata| ClipForgeHata::girdi(yol, hata))?
        .len();

    let mut uyarilar: Vec<String> = Vec::new();
    let mut parcalar: Vec<ParcaBilgisi> = Vec::new();
    for (sira, parca) in veri.parcalar.iter().enumerate() {
        let sure = parca.sure_sn();
        let kare_sayisi = parca
            .stts
            .as_ref()
            .map(|t| t.toplam_ornek)
            .or(parca.ornek_sayisi)
            .filter(|adet| *adet > 0);

        // Sabit kare hızı: `stts` tek bir artış bildiriyorsa kare hızı
        // tam olarak `zaman_olcegi / artış` kesridir (ör. 30000/1001).
        let sabit_kare_hazi = parca
            .mdhd
            .as_ref()
            .map(|mdhd| mdhd.zaman_olcegi)
            .zip(parca.stts.as_ref().and_then(|t| t.sabit_artış))
            .and_then(|(olcek, artış)| KareHazi::yeni(olcek, artış).ok());
        let kare_hazi = sabit_kare_hazi.or_else(|| {
            // Sabit olmayan `stts` (değişken kare hızlı akış): kare sayısından
            // ortalama kare hızı hesaplanır.
            let adet = kare_sayisi?;
            if adet == 0 || sure <= 0.0 {
                return None;
            }
            KareHazi::sabit(adet as f64 / sure).ok()
        });

        let cozunurluk = parca
            .stsd
            .as_ref()
            .filter(|s| s.genislik > 0 && s.yukseklik > 0)
            .map(|s| (s.genislik, s.yukseklik))
            .or_else(|| {
                (parca.tkhd_genislik > 0 && parca.tkhd_yukseklik > 0)
                    .then_some((parca.tkhd_genislik, parca.tkhd_yukseklik))
            });

        let (ornekleme_hizi, kanal) = parca.stsd.as_ref().map_or((None, None), |s| {
            (
                (s.ornekleme_hizi > 0).then_some(s.ornekleme_hizi),
                (s.kanal > 0).then_some(s.kanal),
            )
        });

        parcalar.push(ParcaBilgisi {
            sira,
            iz: parca.iz,
            tur: parca.tur,
            kutu_yolu: parca.kutu_yolu.clone(),
            codec: parca
                .stsd
                .as_ref()
                .map(|s| s.bicim.clone())
                .unwrap_or_default(),
            sure_sn: sure,
            kare_hazi,
            kare_sayisi,
            cozunurluk,
            ornekleme_hizi,
            kanal,
            anahtar_kare_sayisi: parca.anahtar_kare_sayisi,
        });
    }

    let video_indeks = parcalar
        .iter()
        .position(|p| p.tur == ParcasiTuru::Video && p.cozunurluk.is_some());
    if parcalar.iter().any(|p| p.tur == ParcasiTuru::Video) && video_indeks.is_none() {
        let iz = parcalar
            .iter()
            .find(|p| p.tur == ParcasiTuru::Video)
            .map_or(0, |p| p.iz);
        return Err(ClipForgeHata::CozunurlukYok { parca: iz });
    }

    let sure_sn = veri
        .mvhd
        .as_ref()
        .map_or(0.0, iso_bmff::Mvhd::sure_sn)
        .max(parcalar.iter().map(|p| p.sure_sn).fold(0.0_f64, f64::max));
    if sure_sn <= 0.0 {
        return Err(ClipForgeHata::SureSifir {
            yol: yol.to_path_buf(),
        });
    }

    let faststart = veri.faststart();
    match faststart {
        Some(false) => uyarilar.push(
            "moov kutusu dosyanin sonunda: akis kopyasinda 'faststart' (moov one alma) gerekli"
                .to_string(),
        ),
        Some(true) => {}
        None => uyarilar.push("faststart durumu belirlenemedi".to_string()),
    }
    if veri.ftyp.is_none() {
        uyarilar.push("ftyp kutusu yok: dosya markasi belirsiz".to_string());
    }

    Ok(MedyaBilgisi {
        yol: yol.to_path_buf(),
        bicim: Bicim::IsoBmff,
        dosya_boyutu,
        sure_sn,
        marka: veri.ftyp.as_ref().map(|f| f.ana_marka.clone()),
        parcalar,
        video_indeks,
        faststart,
        uyarilar,
    })
}

/// Matroska/WebM dosyasını okur ve kural denetimlerini uygular.
fn matroska_cozumle(yol: &Path, ayarlar: ebml::Ayarlar) -> Result<MedyaBilgisi, ClipForgeHata> {
    let veri = ebml::cozumle(yol, ayarlar)?;
    let dosya_boyutu = std::fs::metadata(yol)
        .map_err(|hata| ClipForgeHata::girdi(yol, hata))?
        .len();

    let mut uyarilar = Vec::new();
    if !veri.dokuman_tipi.is_empty() && veri.dokuman_tipi != "matroska" {
        uyarilar.push(format!(
            "DocType '{}' (matroska/webma bekleniyordu)",
            veri.dokuman_tipi
        ));
    }
    uyarilar.push("faststart yalnizca ISO BMFF icin anlamlidir".to_string());

    let mut parcalar: Vec<ParcaBilgisi> = Vec::new();
    for (sira, parca) in veri.parcalar.iter().enumerate() {
        let kare_sayisi = parca
            .kare_hazi()
            .map(|hiz| hiz.kare_sayisi(veri.sure_sn()))
            .filter(|adet| *adet > 0);
        let cozunurluk = match (parca.genislik, parca.yukseklik) {
            (Some(g), Some(y)) if g > 0 && y > 0 => Some((g, y)),
            _ => None,
        };
        parcalar.push(ParcaBilgisi {
            sira,
            iz: u32::try_from(parca.numara).unwrap_or(u32::MAX),
            tur: parca.tur(),
            kutu_yolu: format!("Segment/Tracks/TrackEntry[{}]", sira),
            codec: parca.codec.clone(),
            sure_sn: veri.sure_sn(),
            kare_hazi: parca.kare_hazi(),
            kare_sayisi,
            cozunurluk,
            ornekleme_hizi: parca
                .ornekleme_hizi
                .filter(|hiz| hiz.is_finite() && *hiz > 0.0)
                .map(|hiz| hiz.round() as u32),
            kanal: parca
                .kanal
                .filter(|kanal| *kanal > 0)
                .and_then(|kanal| u16::try_from(kanal).ok()),
            anahtar_kare_sayisi: None,
        });
    }

    let video_indeks = parcalar
        .iter()
        .position(|p| p.tur == ParcasiTuru::Video && p.cozunurluk.is_some());
    if parcalar.iter().any(|p| p.tur == ParcasiTuru::Video) && video_indeks.is_none() {
        let iz = parcalar
            .iter()
            .find(|p| p.tur == ParcasiTuru::Video)
            .map_or(0, |p| p.iz);
        return Err(ClipForgeHata::CozunurlukYok { parca: iz });
    }

    let sure_sn = veri.sure_sn();
    if sure_sn <= 0.0 {
        return Err(ClipForgeHata::SureSifir {
            yol: yol.to_path_buf(),
        });
    }

    Ok(MedyaBilgisi {
        yol: yol.to_path_buf(),
        bicim: Bicim::Matroska,
        dosya_boyutu,
        sure_sn,
        marka: None,
        parcalar,
        video_indeks,
        faststart: None,
        uyarilar,
    })
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_yardimci::iceriyor;

    #[test]
    fn bicim_tespiti_matroska_ve_iso_imzalarini_tanir() {
        assert_eq!(
            bicim_tespit(&[0x1A, 0x45, 0xDF, 0xA3, 0x01, 0x00, 0x00, 0x00]),
            Some(Bicim::Matroska)
        );
        assert_eq!(
            bicim_tespit(&[0, 0, 0, 0x20, b'f', b't', b'y', b'p']),
            Some(Bicim::IsoBmff)
        );
        assert_eq!(
            bicim_tespit(&[0, 0, 0, 0x20, b'm', b'o', b'o', b'v']),
            Some(Bicim::IsoBmff)
        );
        assert_eq!(bicim_tespit(&[0; 16]), None);
        assert_eq!(bicim_tespit(&[1, 2, 3]), None);
        assert_eq!(bicim_tespit(&[0, 0, 0, 0, b'a', b'b', b'c', b'd']), None);
    }

    #[test]
    fn bicim_kodlari_json_icin_stabil() {
        assert_eq!(Bicim::IsoBmff.kod(), "iso-bmff");
        assert_eq!(Bicim::Matroska.kod(), "matroska");
        assert_eq!(Bicim::IsoBmff.to_string(), "iso-bmff");
    }

    #[test]
    fn parca_bilgisi_turetici_erisimcileri() {
        let parca = ParcaBilgisi {
            sira: 0,
            iz: 1,
            tur: ParcasiTuru::Video,
            kutu_yolu: "moov/trak/mdia/minf/stbl".to_string(),
            codec: "avc1".to_string(),
            sure_sn: 10.0,
            kare_hazi: Some(KareHazi::yeni(30, 1).unwrap()),
            kare_sayisi: Some(300),
            cozunurluk: Some((1920, 1080)),
            ornekleme_hizi: None,
            kanal: None,
            anahtar_kare_sayisi: Some(10),
        };
        assert_eq!(parca.genislik(), Some(1920));
        assert_eq!(parca.yukseklik(), Some(1080));
        assert_eq!(
            parca.en_boy().map(|o| o.to_string()),
            Some("16:9".to_string())
        );
        let cozunurluksuz = ParcaBilgisi {
            cozunurluk: None,
            ..parca
        };
        assert_eq!(cozunurluksuz.genislik(), None);
        assert_eq!(cozunurluksuz.en_boy(), None);
    }

    #[test]
    fn medya_bilgisi_parca_sayaclari() {
        let parca = |tur| ParcaBilgisi {
            sira: 0,
            iz: 1,
            tur,
            kutu_yolu: "yol".to_string(),
            codec: "x".to_string(),
            sure_sn: 1.0,
            kare_hazi: None,
            kare_sayisi: None,
            cozunurluk: None,
            ornekleme_hizi: None,
            kanal: None,
            anahtar_kare_sayisi: None,
        };
        let bilgi = MedyaBilgisi {
            yol: PathBuf::from("a.mp4"),
            bicim: Bicim::IsoBmff,
            dosya_boyutu: 10,
            sure_sn: 1.0,
            marka: None,
            parcalar: vec![
                parca(ParcasiTuru::Video),
                parca(ParcasiTuru::Ses),
                parca(ParcasiTuru::Ses),
                parca(ParcasiTuru::Diger),
            ],
            video_indeks: Some(0),
            faststart: Some(true),
            uyarilar: vec![],
        };
        assert_eq!(bilgi.video_parca_sayisi(), 1);
        assert_eq!(bilgi.ses_parca_sayisi(), 2);
        assert_eq!(bilgi.video().map(|p| p.tur), Some(ParcasiTuru::Video));
        let bos = MedyaBilgisi {
            video_indeks: None,
            parcalar: vec![],
            ..bilgi
        };
        assert!(bos.video().is_none());
    }

    #[test]
    fn taninmayan_dosya_acik_hata_uretir() {
        let gecici = crate::test_yardimci::GeciciDizin::yeni("clipforge-medya-taninmaz").unwrap();
        let yol = gecici.yol().join("metin.txt");
        std::fs::write(&yol, b"bu bir metin dosyasidir, video degildir").unwrap();
        let hata = arastir(&yol).unwrap_err();
        match hata {
            ClipForgeHata::BilinmeyenKapsul { sebep, .. } => assert!(iceriyor(&sebep, "imza")),
            diger => panic!("beklenen BilinmeyenKapsul, gelen {diger:?}"),
        }
    }

    #[test]
    fn olmayan_dosya_girdi_hatasi_uretir() {
        let gecici = crate::test_yardimci::GeciciDizin::yeni("clipforge-medya-yok").unwrap();
        let yol = gecici.yol().join("yok.mp4");
        let hata = arastir(&yol).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::GirdiHatasi { .. }));
    }
}
