//! Kırp planı belgesi: dış kodlayıcıya verilebilecek JSON çıktısı.
//!
//! Plan **uygulanmaz**; planı üreten ve uygulayacak olan arasındaki sözleşmedir.
//! Bu yüzden belge, bir klibi yeniden üretmek için gereken her şeyi taşır:
//! kaynak kutu yolu, kesim noktaları, yeniden zamanlama, kadraj zinciri ve
//! platform profili türetilmiş parametreleri.
//!
//! Üst düzey alan adları (`kaynak`, `baslangic_sn`, `bitis_sn`, `profil`,
//! `kutu_yolu`, `uyarilar`) rapor kartının (MVP kapsamı madde 5) belirlediği
//! şemadır ve geriye dönük değiştirilmez.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hata::ClipForgeHata;
use crate::kadraj::{self, Kadraj, KaynakKadraj};
use crate::medya::MedyaBilgisi;
use crate::olcu::{EnBoy, KareHazi};
use crate::profil::PlatformProfili;
use crate::zamanlama::{KirpPenceresi, Zamanlama};

/// Plan belgesinin şema sürümü.
pub const PLAN_SURUMU: u32 = 1;

/// Planı üreten aracın adı ve sürümü (çıktının izlenebilirliği için).
pub const URETIM_ARACI: &str = concat!("clipforge ", env!("CARGO_PKG_VERSION"));

/// Kaynak dosyanın özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KaynakOzeti {
    /// Kapsül biçimi (`iso-bmff`, `matroska`).
    pub bicim: String,
    /// `ftyp` ana markası (yalnızca ISO BMFF).
    pub marka: Option<String>,
    /// Dosya boyutu (bayt).
    pub dosya_boyutu: u64,
    /// Kapsül süresi (saniye).
    pub sure_sn: f64,
    /// Toplam parça sayısı.
    pub parca_sayisi: usize,
    /// Görüntü parçası sayısı.
    pub video_parca_sayisi: usize,
    /// Ses parçası sayısı.
    pub ses_parca_sayisi: usize,
    /// Seçilen görüntü parçasının kutu yolu.
    pub kutu_yolu: String,
    /// Seçilen görüntü parçasının codec kodu.
    pub codec: String,
    /// Görüntü çözünürlüğü.
    pub cozunurluk: Option<(u32, u32)>,
    /// Görüntü kare hızı.
    pub kare_hazi: Option<KareHazi>,
    /// Toplam kare sayısı.
    pub kare_sayisi: Option<u64>,
    /// `moov` kutusunun `mdat` önünde olup olmadığı.
    pub faststart: Option<bool>,
}

/// Platform profilinin türetilmiş parametre özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilOzeti {
    /// Profil kimliği.
    pub kimlik: String,
    /// İnsan okunur ad.
    pub ad: String,
    /// Hedef platform.
    pub platform: String,
    /// Hedef en-boy oranı.
    pub en_boy: EnBoy,
    /// Hedef çözünürlük.
    pub cozunurluk: (u32, u32),
    /// Tercih edilen kare hızı.
    pub tercih_edilen_kare_hazi: KareHazi,
    /// Azami kare hızı.
    pub azami_kare_hazi: KareHazi,
    /// Önerilen video bit hızı (kbit/s).
    pub video_bit_hizi_kbps: u32,
    /// Anahtar kare aralığı (kare).
    pub gop_kare: u32,
    /// Ses örnekleme hızı.
    pub ses_ornekleme_hizi: u32,
    /// Ses kanal sayısı.
    pub ses_kanal: u16,
    /// Ses bit hızı (kbit/s).
    pub ses_bit_hizi_kbps: u32,
    /// Profilin birincil belgesi.
    pub kaynak: String,
    /// Profilin doğrulama tarihi.
    pub dogrulanma_tarihi: String,
}

/// Zamanlamanın makine-okunur özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZamanlamaOzeti {
    /// Kullanıcının istediği başlangıç.
    pub pencere_baslangic_sn: f64,
    /// Kullanıcının istediği bitiş.
    pub pencere_bitis_sn: f64,
    /// İstenen süre.
    pub istenen_sure_sn: f64,
    /// Kare hassasiyetinde elde edilen süre.
    pub gercek_sure_sn: f64,
    /// Süre farkı (gerçek − istenen).
    pub sure_farki_sn: f64,
    /// Tek karenin süresi.
    pub kare_suresi_sn: f64,
    /// "En fazla 1 kare fark" kriteri sağlandı mı?
    pub kare_harfasi_yeterli: bool,
    /// Kırpmanın başladığı kare (kaynak içinde).
    pub ilk_kare: u64,
    /// Alınan son kare (kaynak içinde, dahil).
    pub son_kare: u64,
    /// Alınan kare sayısı.
    pub kare_sayisi: u64,
    /// Kaynaktaki toplam kare sayısı.
    pub toplam_kare: u64,
    /// Hesapta kullanılan kare hızı.
    pub kare_hazi: KareHazi,
    /// Kırpmanın kaynak içindeki gerçek başlangıç zamanı.
    pub kaynak_baslangic_sn: f64,
}

/// Bir dosya için üretilen kırp planı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KirpPlani {
    /// Şema sürümü.
    pub surum: u32,
    /// Planı üreten araç ve sürümü.
    pub uretim_araci: String,
    /// Kaynak dosya yolu.
    pub kaynak: String,
    /// Kırp başlangıcı (saniye).
    pub baslangic_sn: f64,
    /// Kırp bitişi (saniye).
    pub bitis_sn: f64,
    /// Hedef platform profili kimliği.
    pub profil: String,
    /// Kırpma uygulanacak kutu yolu.
    pub kutu_yolu: String,
    /// Kaynak, kadraj ve zamanlama uyarıları.
    pub uyarilar: Vec<String>,
    /// Kaynak dosyanın özeti.
    pub kaynak_bilgi: KaynakOzeti,
    /// Hedef profilin türetilmiş parametreleri.
    pub profil_ozeti: ProfilOzeti,
    /// Hesaplanan kadraj planı.
    pub kadraj: Kadraj,
    /// Hesaplanan zamanlama planı.
    pub zamanlama: ZamanlamaOzeti,
}

impl KirpPlani {
    /// Okunmuş medya bilgisi, kırp penceresi ve profilden plan üretir.
    ///
    /// # Hatalar
    ///
    /// Profil geçersizse [`ClipForgeHata::ProfilHatali`], kapsülde görüntü
    /// parçası yoksa veya çözünürlüğü yoksa [`ClipForgeHata::CozunurlukYok`],
    /// kırp aralığı geçersizse [`ClipForgeHata::AralikGecersiz`] ya da
    /// [`ClipForgeHata::KaynakAsildi`], kare hızı çözülemediyse
    /// [`ClipForgeHata::KareHiziHatali`] döner.
    pub fn olustur(
        medya: &MedyaBilgisi,
        pencere: &KirpPenceresi,
        profil: &PlatformProfili,
    ) -> Result<Self, ClipForgeHata> {
        profil.dogrula()?;
        let parca = medya
            .video()
            .ok_or(ClipForgeHata::CozunurlukYok { parca: 0 })?;
        let (genislik, yukseklik) = parca
            .cozunurluk
            .ok_or(ClipForgeHata::CozunurlukYok { parca: parca.iz })?;

        let zamanlama = Zamanlama::hesapla(pencere, medya.sure_sn, parca.kare_hazi)?;
        let kadraj = kadraj::hesapla(
            &KaynakKadraj {
                parca_iz: parca.iz,
                genislik,
                yukseklik,
            },
            profil,
        )?;

        let mut uyarilar = medya.uyarilar.clone();
        uyarilar.extend(kadraj.uyarilar.iter().cloned());
        if !zamanlama.kare_harfasi_yeterli() {
            uyarilar.push(format!(
                "kırpma sonrası süre istenenden {:.4} sn farklı (bir kareden fazla)",
                zamanlama.sure_farki_sn()
            ));
        }
        if parca.kare_hazi.is_none() {
            uyarilar.push(
                "kare hizi kapsulden cozulemedi; zamanlama varsayilan olarak uretilemedi"
                    .to_string(),
            );
        }

        Ok(Self {
            surum: PLAN_SURUMU,
            uretim_araci: URETIM_ARACI.to_string(),
            kaynak: medya.yol.to_string_lossy().into_owned(),
            baslangic_sn: pencere.baslangic_sn(),
            bitis_sn: pencere.bitis_sn(),
            profil: profil.kimlik.clone(),
            kutu_yolu: parca.kutu_yolu.clone(),
            uyarilar,
            kaynak_bilgi: KaynakOzeti {
                bicim: medya.bicim.kod().to_string(),
                marka: medya.marka.clone(),
                dosya_boyutu: medya.dosya_boyutu,
                sure_sn: medya.sure_sn,
                parca_sayisi: medya.parcalar.len(),
                video_parca_sayisi: medya.video_parca_sayisi(),
                ses_parca_sayisi: medya.ses_parca_sayisi(),
                kutu_yolu: parca.kutu_yolu.clone(),
                codec: parca.codec.clone(),
                cozunurluk: parca.cozunurluk,
                kare_hazi: parca.kare_hazi,
                kare_sayisi: parca.kare_sayisi,
                faststart: medya.faststart,
            },
            profil_ozeti: ProfilOzeti {
                kimlik: profil.kimlik.clone(),
                ad: profil.ad.clone(),
                platform: profil.platform.clone(),
                en_boy: profil.en_boy,
                cozunurluk: profil.cozunurluk(),
                tercih_edilen_kare_hazi: profil.tercih_edilen_kare_hazi,
                azami_kare_hazi: profil.azami_kare_hazi,
                video_bit_hizi_kbps: profil.video_bit_hizi_kbps,
                gop_kare: profil.gop_kare,
                ses_ornekleme_hizi: profil.ses.ornekleme_hizi,
                ses_kanal: profil.ses.kanal,
                ses_bit_hizi_kbps: profil.ses.bit_hizi_kbps,
                kaynak: profil.kaynak.clone(),
                dogrulanma_tarihi: profil.dogrulanma_tarihi.clone(),
            },
            kadraj,
            zamanlama: ZamanlamaOzeti {
                pencere_baslangic_sn: pencere.baslangic_sn(),
                pencere_bitis_sn: pencere.bitis_sn(),
                istenen_sure_sn: zamanlama.istenen_sure_sn,
                gercek_sure_sn: zamanlama.gercek_sure_sn,
                sure_farki_sn: zamanlama.sure_farki_sn(),
                kare_suresi_sn: zamanlama.kare_suresi_sn(),
                kare_harfasi_yeterli: zamanlama.kare_harfasi_yeterli(),
                ilk_kare: zamanlama.ilk_kare,
                son_kare: zamanlama.son_kare(),
                kare_sayisi: zamanlama.kare_sayisi,
                toplam_kare: zamanlama.toplam_kare,
                kare_hazi: zamanlama.kare_hazi,
                kaynak_baslangic_sn: zamanlama.kaynak_baslangic_sn,
            },
        })
    }

    /// Planı okunabilir biçimde JSON olarak serileştirir.
    ///
    /// # Hatalar
    ///
    /// Serileştirme başarısız olursa hata döner; pratikte bu, veri modelinde
    /// bir tanım hatasıdır.
    pub fn metin(&self) -> Result<String, ClipForgeHata> {
        serde_json::to_string_pretty(self).map_err(|hata| ClipForgeHata::CiktiHatasi {
            yol: Path::new("<bellek>").to_path_buf(),
            hata: std::io::Error::other(hata),
        })
    }

    /// Planı diske yazar.
    ///
    /// # Hatalar
    ///
    /// Serileştirme veya dosya yazma hatasında [`ClipForgeHata::CiktiHatasi`] döner.
    pub fn yaz(&self, yol: &Path) -> Result<(), ClipForgeHata> {
        let metin = self.metin()?;
        let mut son = metin;
        son.push('\n');
        std::fs::write(yol, son).map_err(|hata| ClipForgeHata::cikti(yol, hata))
    }

    /// JSON metninden plan okur.
    ///
    /// # Hatalar
    ///
    /// Metin geçerli plan JSON'u değilse [`ClipForgeHata::ProfilDosyasiHatali`]
    /// döner. Bu işlev şema gidiş-dönüşü doğrulaması ve harici araçların
    /// çıktısını okuması için kullanılır.
    pub fn ayristir(metin: &str) -> Result<Self, ClipForgeHata> {
        serde_json::from_str(metin).map_err(|hata| ClipForgeHata::ProfilDosyasiHatali {
            ayrinti: hata.to_string(),
        })
    }
}

/// Toplu iş çıktısının JSON belgesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopluBelge {
    /// Şema sürümü.
    pub surum: u32,
    /// Planı üreten araç ve sürümü.
    pub uretim_araci: String,
    /// Kullanılan profil kimliği.
    pub profil: String,
    /// Üretilen planlar.
    pub planlar: Vec<KirpPlani>,
    /// İşlenemeyen dosyalar.
    pub hatalilar: Vec<KayitliHata>,
    /// Filtreye takılıp atlanan dosyalar.
    pub atlananlar: Vec<KayitliAtlanan>,
}

/// Üretilemeyen bir dosyanın kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KayitliHata {
    /// Dosya yolu.
    pub yol: String,
    /// Hata sınıfı (`girdi`, `bicim`, `plan`).
    pub sinif: String,
    /// Hata metni.
    pub mesaj: String,
}

/// Atlanan bir dosyanın kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KayitliAtlanan {
    /// Dosya ya da dizin yolu.
    pub yol: String,
    /// Atlanma nedeni.
    pub sebep: String,
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::medya::{Bicim, ParcaBilgisi};
    use crate::profil::ProfilKutusu;
    use crate::test_yardimci::iceriyor;
    use crate::test_yardimci::GeciciDizin;
    use std::path::PathBuf;

    /// Testlerde kullanılan sahte medya bilgisi üretir.
    fn ornek_medya() -> MedyaBilgisi {
        MedyaBilgisi {
            yol: PathBuf::from("ornek video.mp4"),
            bicim: Bicim::IsoBmff,
            dosya_boyutu: 12_345_678,
            sure_sn: 20.0,
            marka: Some("isom".to_string()),
            parcalar: vec![
                ParcaBilgisi {
                    sira: 0,
                    iz: 1,
                    tur: crate::iso_bmff::ParcasiTuru::Video,
                    kutu_yolu: "moov/trak/mdia/minf/stbl".to_string(),
                    codec: "avc1".to_string(),
                    sure_sn: 20.0,
                    kare_hazi: Some(KareHazi::yeni(30, 1).unwrap()),
                    kare_sayisi: Some(600),
                    cozunurluk: Some((1920, 1080)),
                    ornekleme_hizi: None,
                    kanal: None,
                    anahtar_kare_sayisi: Some(20),
                },
                ParcaBilgisi {
                    sira: 1,
                    iz: 2,
                    tur: crate::iso_bmff::ParcasiTuru::Ses,
                    kutu_yolu: "moov/trak/mdia/minf/stbl".to_string(),
                    codec: "mp4a".to_string(),
                    sure_sn: 20.0,
                    kare_hazi: None,
                    kare_sayisi: Some(960_000),
                    cozunurluk: None,
                    ornekleme_hizi: Some(48_000),
                    kanal: Some(2),
                    anahtar_kare_sayisi: None,
                },
            ],
            video_indeks: Some(0),
            faststart: Some(true),
            uyarilar: vec![],
        }
    }

    /// Gömülü katalogdan reels profilini döndürür.
    fn reels() -> PlatformProfili {
        ProfilKutusu::gomulu()
            .unwrap()
            .ara("reels")
            .unwrap()
            .clone()
    }

    #[test]
    fn plan_ust_duzey_alanlari_kart_semasiyla_uyusur() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(1.0, 5.0).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        assert_eq!(plan.surum, PLAN_SURUMU);
        assert_eq!(plan.uretim_araci, URETIM_ARACI);
        assert_eq!(plan.kaynak, "ornek video.mp4");
        assert_eq!(plan.baslangic_sn, 1.0);
        assert_eq!(plan.bitis_sn, 5.0);
        assert_eq!(plan.profil, "reels");
        assert_eq!(plan.kutu_yolu, "moov/trak/mdia/minf/stbl");
        assert!(plan.uyarilar.iter().any(|u| iceriyor(u, "kirpiliyor")));
    }

    #[test]
    fn plan_kaynak_ozeti_parca_sayilarini_tasiyor() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(0.0, 2.0).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        assert_eq!(plan.kaynak_bilgi.bicim, "iso-bmff");
        assert_eq!(plan.kaynak_bilgi.marka.as_deref(), Some("isom"));
        assert_eq!(plan.kaynak_bilgi.parca_sayisi, 2);
        assert_eq!(plan.kaynak_bilgi.video_parca_sayisi, 1);
        assert_eq!(plan.kaynak_bilgi.ses_parca_sayisi, 1);
        assert_eq!(plan.kaynak_bilgi.cozunurluk, Some((1920, 1080)));
        assert_eq!(plan.kaynak_bilgi.faststart, Some(true));
    }

    #[test]
    fn plan_profil_ozeti_turetilmis_parametreleri_tasiyor() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(0.0, 2.0).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        assert_eq!(plan.profil_ozeti.cozunurluk, (1080, 1920));
        assert_eq!(plan.profil_ozeti.en_boy.to_string(), "9:16");
        assert_eq!(plan.profil_ozeti.video_bit_hizi_kbps, 6000);
        assert_eq!(plan.profil_ozeti.gop_kare, 60);
        assert_eq!(plan.profil_ozeti.ses_ornekleme_hizi, 48_000);
        assert!(!plan.profil_ozeti.kaynak.is_empty());
    }

    #[test]
    fn plan_zamanlama_ozeti_kare_harfasini_denetler() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(1.0, 5.0).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        assert_eq!(plan.zamanlama.ilk_kare, 30);
        assert_eq!(plan.zamanlama.son_kare, 149);
        assert_eq!(plan.zamanlama.kare_sayisi, 120);
        assert_eq!(plan.zamanlama.toplam_kare, 600);
        assert!(plan.zamanlama.kare_harfasi_yeterli);
        assert!(plan.zamanlama.sure_farki_sn.abs() < 1e-9);
    }

    #[test]
    fn plan_json_gidis_donusu_korunur() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(1.0, 5.0).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        let metin = plan.metin().unwrap();
        // Kartın zorunlu tuttuğu üst düzey alanlar JSON'da görünmelidir.
        for alan in [
            "\"kaynak\"",
            "\"baslangic_sn\"",
            "\"bitis_sn\"",
            "\"profil\"",
            "\"kutu_yolu\"",
            "\"uyarilar\"",
        ] {
            assert!(iceriyor(&metin, alan), "plan metninde {alan} yok");
        }
        let geri = KirpPlani::ayristir(&metin).unwrap();
        assert_eq!(geri, plan);
    }

    #[test]
    fn plan_dosyaya_yazilir_ve_tekrar_okunur() {
        let gecici = GeciciDizin::yeni("clipforge-plan-yaz").unwrap();
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(0.5, 3.25).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        let yol = gecici.yol().join("plan.json");
        plan.yaz(&yol).unwrap();
        let icerik = std::fs::read_to_string(&yol).unwrap();
        assert!(icerik.ends_with('\n'));
        assert_eq!(KirpPlani::ayristir(&icerik).unwrap(), plan);
    }

    #[test]
    fn plan_yazilamayan_dosyada_cikti_hatasi_verir() {
        let gecici = GeciciDizin::yeni("clipforge-plan-yok").unwrap();
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(0.0, 1.0).unwrap();
        let plan = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap();
        let yol = gecici.yol().join("olmayan-dizin").join("plan.json");
        let hata = plan.yaz(&yol).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::CiktiHatasi { .. }));
    }

    #[test]
    fn plan_bozuk_json_okunamaz() {
        let hata = KirpPlani::ayristir("{ eksik").unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilDosyasiHatali { .. }));
    }

    #[test]
    fn gorsuntusuz_kapsul_planlanamaz() {
        let mut medya = ornek_medya();
        medya.video_indeks = None;
        let pencere = KirpPenceresi::yeni(0.0, 1.0).unwrap();
        let hata = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::CozunurlukYok { .. }));
    }

    #[test]
    fn cozunurluksuz_parca_planlanamaz() {
        let mut medya = ornek_medya();
        medya.parcalar[0].cozunurluk = None;
        let pencere = KirpPenceresi::yeni(0.0, 1.0).unwrap();
        let hata = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap_err();
        match hata {
            ClipForgeHata::CozunurlukYok { parca } => assert_eq!(parca, 1),
            diger => panic!("beklenen CozunurlukYok, gelen {diger:?}"),
        }
    }

    #[test]
    fn gecersiz_profil_plani_reddeder() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(0.0, 1.0).unwrap();
        let mut profil = reels();
        profil.gop_kare = 0;
        let hata = KirpPlani::olustur(&medya, &pencere, &profil).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilHatali { .. }));
    }

    #[test]
    fn kaynak_suresini_asan_aralik_plani_reddeder() {
        let medya = ornek_medya();
        let pencere = KirpPenceresi::yeni(15.0, 25.0).unwrap();
        let hata = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::KaynakAsildi { .. }));
    }

    #[test]
    fn kare_hizi_olmayan_kapsul_uyarili_ve_hata_verir() {
        let mut medya = ornek_medya();
        medya.parcalar[0].kare_hazi = None;
        let pencere = KirpPenceresi::yeni(0.0, 1.0).unwrap();
        let hata = KirpPlani::olustur(&medya, &pencere, &reels()).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::KareHiziHatali { .. }));
    }

    #[test]
    fn toplu_belge_json_gidis_donusu() {
        let belge = TopluBelge {
            surum: PLAN_SURUMU,
            uretim_araci: URETIM_ARACI.to_string(),
            profil: "reels".to_string(),
            planlar: vec![],
            hatalilar: vec![KayitliHata {
                yol: "bozuk.mp4".to_string(),
                sinif: "bicim".to_string(),
                mesaj: "moov bulunamadi".to_string(),
            }],
            atlananlar: vec![KayitliAtlanan {
                yol: "notlar.txt".to_string(),
                sebep: "uzanti filtresi".to_string(),
            }],
        };
        let metin = serde_json::to_string_pretty(&belge).unwrap();
        let geri: TopluBelge = serde_json::from_str(&metin).unwrap();
        assert_eq!(geri, belge);
    }
}
