//! Toplu iş kuyruğu: klasör gezme, uzantı filtresi ve dosya başına hata ayrımı.
//!
//! Raporun `b03` kabul kriteri: "tek bir dosyanın hatası kuyruktaki diğer işleri
//! durdurmaz; hatalı dosya ayrı bir listede birikir." Bu modül tam olarak bunu
//! yapar: gezme hatası, ayrıştırma hatası ve planlama hatası ayrı listelere
//! yazılır, hiçbiri zinciri durdurmaz.
//!
//! Klasör gezme kendi özyinelemeli `read_dir` yürüyüşüyle yapılır; `walkdir`
//! bağımlılık politikası gereği yasaktır (`WORKER_CONTRACT.md` § 3.2-F).

use std::path::{Path, PathBuf};

use crate::hata::ClipForgeHata;
use crate::medya;
use crate::plan::{KayitliAtlanan, KayitliHata, KirpPlani};
use crate::profil::{PlatformProfili, ProfilKutusu};
use crate::zamanlama::KirpPenceresi;

/// Uzantı filtresi tanımlanmazsa kabul edilen uzantılar.
pub const VARSAYILAN_UZANTILAR: [&str; 5] = ["mp4", "m4v", "mov", "mkv", "webm"];

/// Klasör gezme sırasında uygulanan azami derinlik.
pub const VARSAYILAN_AZAMI_DERINLIK: usize = 8;

/// Kuyruğa alınacak tek bir dosya.
#[derive(Debug, Clone, PartialEq)]
pub struct KuyrukGirdisi {
    /// Dosya yolu.
    pub yol: PathBuf,
    /// Dosyaya özel başlangıç saniyesi (verilmezse kuyruk varsayılanı kullanılır).
    pub baslangic_sn: Option<f64>,
    /// Dosyaya özel bitiş saniyesi.
    pub bitis_sn: Option<f64>,
}

impl KuyrukGirdisi {
    /// Varsayılan kırp penceresiyle girdi oluşturur.
    pub fn yeni(yol: PathBuf) -> Self {
        Self {
            yol,
            baslangic_sn: None,
            bitis_sn: None,
        }
    }
}

/// Uzantı filtresi: yalnızca listelenen uzantılar kuyruğa alınır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UzantıFiltresi {
    izinli: Vec<String>,
    azami_derinlik: usize,
}

impl UzantıFiltresi {
    /// Verilen uzantılarla filtre oluşturur; liste boşsa
    /// [`VARSAYILAN_UZANTILAR`] kullanılır.
    pub fn yeni(uzantilar: &[String]) -> Self {
        let izinli: Vec<String> = if uzantilar.is_empty() {
            VARSAYILAN_UZANTILAR
                .iter()
                .map(|u| (*u).to_string())
                .collect()
        } else {
            uzantilar
                .iter()
                .map(|u| u.trim_start_matches('.').to_lowercase())
                .collect()
        };
        Self {
            izinli,
            azami_derinlik: VARSAYILAN_AZAMI_DERINLIK,
        }
    }

    /// Azami gezinme derinliğini ayarlar.
    pub fn derinlik_ayarla(&mut self, derinlik: usize) -> &mut Self {
        self.azami_derinlik = derinlik;
        self
    }

    /// Filtrenin izin verdiği uzantıları döndürür.
    pub fn izinliler(&self) -> &[String] {
        &self.izinli
    }

    /// Verilen yolun uzantısı kabul ediliyorsa `true` döner.
    pub fn kabul_ediyor_mu(&self, yol: &Path) -> bool {
        yol.extension()
            .and_then(|u| u.to_str())
            .map(|u| self.izinli.iter().any(|izinli| izinli == &u.to_lowercase()))
            .unwrap_or(false)
    }
}

/// Bir klasörün toplanmış gezme sonucu.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Toplama {
    /// Kuyruğa alınacak dosyalar (sıralanmış, kararlı).
    pub girdiler: Vec<KuyrukGirdisi>,
    /// Filtreye takılan dosyalar.
    pub atlananlar: Vec<KayitliAtlanan>,
    /// Azami derinlik nedeniyle içeri alınamayan dizinler.
    pub derinlik_asimlari: Vec<KayitliAtlanan>,
}

/// Bir klasörü özyinelemeli olarak gezer ve kuyruk girdilerini toplar.
///
/// Dosya adları kültürel sıraya göre sıralanır; böylece aynı klasör iki kez
/// gezildiğinde aynı sıra elde edilir (testlerin ve `faststart` raporunun
/// belirlenimci olması için).
///
/// # Hatalar
///
/// Kök dizin okunamazsa [`ClipForgeHata::DizinHatasi`] döner. Alt dizinlerin
/// okunamaması gezmeyi durdurmaz; bunlar `atlananlar` listesine yazılır.
pub fn topla(kok: &Path, filtre: &UzantıFiltresi) -> Result<Toplama, ClipForgeHata> {
    let mut sonuc = Toplama::default();
    gez(kok, 0, filtre, &mut sonuc)?;
    sonuc.girdiler.sort_by(|a, b| a.yol.cmp(&b.yol));
    sonuc.atlananlar.sort_by(|a, b| a.yol.cmp(&b.yol));
    Ok(sonuc)
}

/// `read_dir` üzerine kurulu özyinelemeli gezme.
fn gez(
    yol: &Path,
    derinlik: usize,
    filtre: &UzantıFiltresi,
    sonuc: &mut Toplama,
) -> Result<(), ClipForgeHata> {
    if derinlik > filtre.azami_derinlik {
        sonuc.derinlik_asimlari.push(KayitliAtlanan {
            yol: yol.to_string_lossy().into_owned(),
            sebep: format!("azami derinlik ({}) asildi", filtre.azami_derinlik),
        });
        return Ok(());
    }
    let girdiler = std::fs::read_dir(yol).map_err(|hata| ClipForgeHata::dizin(yol, hata))?;
    for giris in girdiler {
        // Bozuk dizin girdileri (izin hatası vb.) tüm kuyruğu düşürmemelidir.
        let Ok(giris) = giris else { continue };
        let alt = giris.path();
        let tur = match giris.file_type() {
            Ok(tur) => tur,
            Err(_) => {
                sonuc.atlananlar.push(KayitliAtlanan {
                    yol: alt.to_string_lossy().into_owned(),
                    sebep: "dosya turu okunamadi".to_string(),
                });
                continue;
            }
        };
        if tur.is_dir() {
            gez(&alt, derinlik + 1, filtre, sonuc)?;
        } else if tur.is_file() {
            if filtre.kabul_ediyor_mu(&alt) {
                sonuc.girdiler.push(KuyrukGirdisi::yeni(alt));
            } else {
                sonuc.atlananlar.push(KayitliAtlanan {
                    yol: alt.to_string_lossy().into_owned(),
                    sebep: "uzanti filtresi".to_string(),
                });
            }
        }
    }
    Ok(())
}

/// Toplu planlama sonucu.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KuyrukSonucu {
    /// Başarıyla planlanan dosyalar.
    pub planlar: Vec<KirpPlani>,
    /// Planlanamayan dosyalar.
    pub hatalilar: Vec<KayitliHata>,
    /// Atlanan dosyalar.
    pub atlananlar: Vec<KayitliAtlanan>,
}

impl KuyrukSonucu {
    /// Toplam giriş sayısını döndürür.
    pub fn toplam(&self) -> usize {
        self.planlar.len() + self.hatalilar.len() + self.atlananlar.len()
    }
}

/// Hata sınıfını belirler: kullanıcı "dosyam bozuk" ile "ayar geçersiz" mesajlarını
/// farklı ele almalıdır (rapor `b07` — Hata yönetimi).
fn hata_sinifi(hata: &ClipForgeHata) -> &'static str {
    match hata {
        ClipForgeHata::GirdiHatasi { .. } | ClipForgeHata::DizinHatasi { .. } => "girdi",
        ClipForgeHata::ProfilYok { .. }
        | ClipForgeHata::ProfilHatali { .. }
        | ClipForgeHata::ProfilDosyasiHatali { .. }
        | ClipForgeHata::ArgumanHatasi { .. } => "profil",
        ClipForgeHata::CiktiHatasi { .. } => "cikti",
        _ => "bicim",
    }
}

/// Toplu planlamayı çalıştırır.
///
/// Her dosya bağımsızca işlenir: bir dosyanın hatası diğerlerini etkilemez.
/// Kırp penceresi dosya başında belirlenir; verilen tek uç kaynak süresiyle
/// tamamlanır, böylece süreleri farklı dosyalar aynı pencerede işlenebilir.
///
/// # Hatalar
///
/// Profil kimliği katalogda yoksa [`ClipForgeHata::ProfilYok`] döner. Dosya
/// düzeyindeki hatalar sonuca yazılır, işlev hata döndürmez.
pub fn planla(
    girdiler: &[KuyrukGirdisi],
    katalog: &ProfilKutusu,
    profil_kimlik: &str,
    varsayilan_baslangic: Option<f64>,
    varsayilan_bitis: Option<f64>,
) -> Result<KuyrukSonucu, ClipForgeHata> {
    let profil: &PlatformProfili = katalog.ara(profil_kimlik)?;
    let mut sonuc = KuyrukSonucu::default();
    for girdi in girdiler {
        match girdi.planla(profil, varsayilan_baslangic, varsayilan_bitis) {
            Ok(plan) => sonuc.planlar.push(plan),
            Err(hata) => sonuc.hatalilar.push(KayitliHata {
                yol: girdi.yol.to_string_lossy().into_owned(),
                sinif: hata_sinifi(&hata).to_string(),
                mesaj: hata.ozet(),
            }),
        }
    }
    Ok(sonuc)
}

impl KuyrukGirdisi {
    /// Tek bir girdi için kırp planı üretir.
    ///
    /// Önce kapsül okunur, sonra pencere belirlenir. Öncelik sırası: dosyaya özel
    /// değer, kuyruk varsayılanı, kaynak süresi. Verilen tek uç, kaynağın
    /// tamamıyla tamamlanır.
    ///
    /// # Hatalar
    ///
    /// Dosya okunamazsa, kapsül ayrıştırılamazsa, aralık geçersizse veya
    /// profil uyumsuzluğu varsa ilgili [`ClipForgeHata`] döner.
    pub fn planla(
        &self,
        profil: &PlatformProfili,
        varsayilan_baslangic: Option<f64>,
        varsayilan_bitis: Option<f64>,
    ) -> Result<KirpPlani, ClipForgeHata> {
        let medya = medya::arastir(&self.yol)?;
        let bas = self.baslangic_sn.or(varsayilan_baslangic);
        let bit = self.bitis_sn.or(varsayilan_bitis);
        let pencere = match (bas, bit) {
            (Some(bas), Some(bit)) => KirpPenceresi::yeni(bas, bit)?,
            (Some(bas), None) => KirpPenceresi::yeni(bas, medya.sure_sn)?,
            (None, Some(bit)) => KirpPenceresi::yeni(0.0, bit)?,
            (None, None) => KirpPenceresi::tam(medya.sure_sn),
        };
        KirpPlani::olustur(&medya, &pencere, profil)
    }
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_yardimci::iceriyor;
    use crate::test_yardimci::GeciciDizin;

    /// Filtreye göre uzantı kabulü.
    #[test]
    fn varsayilan_uzanti_filtresi_bes_uzantiyi_kabul_edilir() {
        let filtre = UzantıFiltresi::yeni(&[]);
        assert_eq!(filtre.izinliler().len(), 5);
        for uzanti in ["a.mp4", "a.MP4", "b.mkv", "c.mov", "d.m4v", "e.webm"] {
            assert!(
                filtre.kabul_ediyor_mu(Path::new(uzanti)),
                "{uzanti} kabul edilmeliydi"
            );
        }
        for uzanti in ["a.txt", "a.avi", "a", "a.mp3"] {
            assert!(
                !filtre.kabul_ediyor_mu(Path::new(uzanti)),
                "{uzanti} reddedilmeliydi"
            );
        }
    }

    #[test]
    fn ozel_uzanti_filtresi_nokta_ve_buyuk_harf_normalize_edilir() {
        let filtre = UzantıFiltresi::yeni(&[".AVI".to_string(), "mkv".to_string()]);
        assert_eq!(filtre.izinliler(), &["avi".to_string(), "mkv".to_string()]);
        assert!(filtre.kabul_ediyor_mu(Path::new("x.avi")));
        assert!(!filtre.kabul_ediyor_mu(Path::new("x.mp4")));
    }

    #[test]
    fn klasor_gezme_uzanti_filtresi_ve_siralamayi_uygular() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-gezme").unwrap();
        gecici.yaz("b.mp4", b"x").unwrap();
        gecici.yaz("a.mp4", b"x").unwrap();
        gecici.yaz("notlar.txt", b"x").unwrap();
        gecici.yaz("alt/c.mkv", b"x").unwrap();
        gecici.yaz("alt/derin/d.webm", b"x").unwrap();

        let filtre = UzantıFiltresi::yeni(&[]);
        let sonuc = topla(gecici.yol(), &filtre).unwrap();
        // Beklenen sıra kültürel sıraya göredir; yol ayracı platforma bağlıdır.
        let adlar: Vec<String> = sonuc
            .girdiler
            .iter()
            .map(|g| {
                g.yol
                    .strip_prefix(gecici.yol())
                    .unwrap_or(&g.yol)
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            })
            .collect();
        assert_eq!(
            adlar,
            vec!["a.mp4", "alt/c.mkv", "alt/derin/d.webm", "b.mp4"]
        );
        assert_eq!(sonuc.atlananlar.len(), 1);
        assert_eq!(sonuc.atlananlar[0].sebep, "uzanti filtresi");
    }

    #[test]
    fn derinlik_siniri_asilirsa_dizin_atlanir() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-derinlik").unwrap();
        gecici.yaz("a/b/c/d/x.mp4", b"x").unwrap();
        let mut filtre = UzantıFiltresi::yeni(&[]);
        filtre.derinlik_ayarla(2);
        let sonuc = topla(gecici.yol(), &filtre).unwrap();
        assert!(sonuc.girdiler.is_empty());
        assert_eq!(sonuc.derinlik_asimlari.len(), 1);
        assert!(iceriyor(
            &sonuc.derinlik_asimlari[0].sebep,
            "azami derinlik"
        ));
    }

    #[test]
    fn olmayan_kok_dizin_hata_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-yok").unwrap();
        let filtre = UzantıFiltresi::yeni(&[]);
        let hata = topla(&gecici.yol().join("yok"), &filtre).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::DizinHatasi { .. }));
    }

    #[test]
    fn bozuk_dosya_atlanir_ve_digerleri_islenir() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-bozuk").unwrap();
        let iyi = crate::ornek::mp4_ornegi(1920, 1080, 600, 30);
        gecici.yaz("iyi.mp4", &iyi).unwrap();
        gecici.yaz("bozuk.mp4", b"bu bir mp4 degil").unwrap();
        gecici
            .yaz("eksik-moov.mp4", &crate::ornek::mp4_moovsuz())
            .unwrap();

        let filtre = UzantıFiltresi::yeni(&[]);
        let toplama = topla(gecici.yol(), &filtre).unwrap();
        assert_eq!(toplama.girdiler.len(), 3);

        let katalog = ProfilKutusu::gomulu().unwrap();
        let sonuc = planla(&toplama.girdiler, &katalog, "reels", None, None).unwrap();
        assert_eq!(sonuc.planlar.len(), 1);
        assert_eq!(sonuc.hatalilar.len(), 2);
        assert!(sonuc.hatalilar.iter().all(|h| h.sinif == "bicim"));
        assert!(iceriyor(&sonuc.hatalilar[0].mesaj, "imza taninmadi"));
        assert!(iceriyor(&sonuc.hatalilar[1].mesaj, "moov"));
        assert_eq!(sonuc.toplam(), 3);
    }

    #[test]
    fn dosyaya_ozel_aralıklar_kullanilir() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-aralik").unwrap();
        let iyi = crate::ornek::mp4_ornegi(1920, 1080, 600, 30);
        let yol = gecici.yaz("a.mp4", &iyi).unwrap();
        let mut girdi = KuyrukGirdisi::yeni(yol);
        girdi.baslangic_sn = Some(1.0);
        girdi.bitis_sn = Some(3.0);
        let katalog = ProfilKutusu::gomulu().unwrap();
        let sonuc = planla(&[girdi.clone()], &katalog, "reels", None, None).unwrap();
        assert_eq!(sonuc.planlar.len(), 1);
        assert_eq!(sonuc.planlar[0].baslangic_sn, 1.0);
        assert_eq!(sonuc.planlar[0].bitis_sn, 3.0);

        // Yalnızca başlangıç verilirse bitiş kaynak süredir.
        let mut girdi2 = girdi;
        girdi2.bitis_sn = None;
        let sonuc = planla(&[girdi2], &katalog, "reels", None, None).unwrap();
        assert_eq!(sonuc.planlar[0].bitis_sn, 20.0);
    }

    #[test]
    fn varsayilan_pencere_uygulanir() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-varsayilan").unwrap();
        let yol = gecici
            .yaz("a.mp4", &crate::ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let katalog = ProfilKutusu::gomulu().unwrap();
        let sonuc = planla(
            &[KuyrukGirdisi::yeni(yol)],
            &katalog,
            "shorts",
            Some(2.0),
            Some(4.0),
        )
        .unwrap();
        assert_eq!(sonuc.planlar[0].baslangic_sn, 2.0);
        assert_eq!(sonuc.planlar[0].bitis_sn, 4.0);
        assert_eq!(sonuc.planlar[0].profil, "shorts");
    }

    #[test]
    fn olmayan_profil_tum_kuyrugu_durdurur() {
        let katalog = ProfilKutusu::gomulu().unwrap();
        let hata = planla(&[], &katalog, "linkedin", None, None).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilYok { .. }));
    }

    #[test]
    fn araligi_kaynak_suresini_asan_dosya_hata_sinifi_ile_kaydedilir() {
        let gecici = GeciciDizin::yeni("clipforge-kuyruk-asim").unwrap();
        let yol = gecici
            .yaz("a.mp4", &crate::ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let mut girdi = KuyrukGirdisi::yeni(yol);
        girdi.baslangic_sn = Some(1.0);
        girdi.bitis_sn = Some(999.0);
        let katalog = ProfilKutusu::gomulu().unwrap();
        let sonuc = planla(&[girdi], &katalog, "reels", None, None).unwrap();
        assert!(sonuc.planlar.is_empty());
        assert_eq!(sonuc.hatalilar.len(), 1);
        assert!(iceriyor(
            &sonuc.hatalilar[0].mesaj,
            "kaynak suresini asiyor"
        ));
    }

    #[test]
    fn hata_siniflari_ayirt_edilir() {
        let girdi = ClipForgeHata::girdi(Path::new("a"), std::io::Error::other("x"));
        assert_eq!(hata_sinifi(&girdi), "girdi");
        assert_eq!(
            hata_sinifi(&ClipForgeHata::ProfilYok { kimlik: "x".into() }),
            "profil"
        );
        assert_eq!(
            hata_sinifi(&ClipForgeHata::cikti(
                Path::new("a"),
                std::io::Error::other("x")
            )),
            "cikti"
        );
        assert_eq!(
            hata_sinifi(&ClipForgeHata::MoovYok {
                yol: PathBuf::from("a")
            }),
            "bicim"
        );
        assert_eq!(
            hata_sinifi(&ClipForgeHata::ArgumanHatasi {
                ayrinti: "x".into()
            }),
            "profil"
        );
    }

    #[test]
    fn bos_kuyruk_bos_sonuc_doner() {
        let katalog = ProfilKutusu::gomulu().unwrap();
        let sonuc = planla(&[], &katalog, "reels", None, None).unwrap();
        assert_eq!(sonuc.toplam(), 0);
        assert!(sonuc.planlar.is_empty());
    }
}
