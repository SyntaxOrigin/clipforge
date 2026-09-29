//! Kadraj hesabı: kaynak en-boy oranından hedef orana geçiş.
//!
//! Bu modül rapordaki `scale` → `crop` → `pad` filtre zincirinin
//! (`b07` — Teknik Tasarım) saf Rust karşılığıdır; hesabı yapar, **uygulamaz**.
//!
//! # Kural
//!
//! | Durum | Karar | Gerekçe |
//! |---|---|---|
//! | Oranlar eşit | Yalnızca **ölçekleme** | Kırpma ya da dolgu gerekmez. |
//! | Kaynak, hedef **genişliğinden geniş** | **Kırpma** | Kaynak zaten daraltılmadan hedefe sığmaz; dolgu için küçültmek görüntüyü hedef genişliğinin altına indirirdi. |
//! | Kaynak, hedef **genişliğine sığıyor** ama hedeften **dar** | **Kenar dolgusu** | Kaynak tamamen korunur; eksik olan yalnızca dikey alandır. |
//!
//> Genişlik bu kuralda çıpasıdır: dikey platform teslimatlarında sınır genişlikle
//! ifade edilir. Kaynak zaten hedef genişliğe sığıyorsa kırpmak kullanıcının
//! görmek istediği kareleri atmanın anlamı taşımaz; dolgu eklemek hem içeriği
//! korur hem de hedef genişliğini garanti eder. Buna karşılık kaynak genişse,
//> dolgu ancak aşırı küçültmeyle mümkündür; kırpma bu bedeli ödetmez.
//!
//! Bu, rapordaki `scale` → `crop` → `pad` zincirinin
//! (`b07` — Teknik Tasarım) saf Rust karşılığıdır; hesabı yapar, **uygulamaz**.

use serde::{Deserialize, Serialize};

use crate::hata::ClipForgeHata;
use crate::olcu::EnBoy;
use crate::profil::PlatformProfili;

/// En-boy oranı karşılaştırmasında kullanılan mutlak tolerans.
pub const ORAN_TOLERANSI: f64 = 0.01;

/// Kadrajın uygulanma biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KadrajKipi {
    /// Kırpma ya da dolgu gerekmiyor; yalnızca ölçekleme yapılır.
    Kirpmasiz,
    /// Kaynak görüntünün bir bölümü atılır.
    Kirp,
    /// Görüntü küçültülür, kalan alan dolguyla doldurulur.
    Dolgu,
}

impl KadrajKipi {
    /// Kipin Türkçe açıklamasını döndürür (insan okunur çıktı için).
    pub fn aciklama(self) -> &'static str {
        match self {
            Self::Kirpmasiz => "yalnizca olcekleme",
            Self::Kirp => "kirpma",
            Self::Dolgu => "kenar dolgusu",
        }
    }
}

/// Kaynakta atılacak dikdörtgen (piksel koordinatı, sol üst köşe esas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kirpma {
    /// Sol kenarın x konumu.
    pub x: u32,
    /// Üst kenarın y konumu.
    pub y: u32,
    /// Genişlik.
    pub genislik: u32,
    /// Yükseklik.
    pub yukseklik: u32,
}

/// Hedef çerçevede dolguyla doldurulacak bölge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dolgu {
    /// Sol kenarın x konumu.
    pub x: u32,
    /// Üst kenarın y konumu.
    pub y: u32,
    /// Genişlik.
    pub genislik: u32,
    /// Yükseklik.
    pub yukseklik: u32,
}

/// Kaynak görüntünün kadraj hesabı için gereken asgari bilgisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KaynakKadraj {
    /// Parçanın iz numarası (hata mesajlarında kullanılır).
    pub parca_iz: u32,
    /// Kaynak genişlik (piksel).
    pub genislik: u32,
    /// Kaynak yükseklik (piksel).
    pub yukseklik: u32,
}

impl KaynakKadraj {
    /// Kaynağın en-boy oranını döndürür.
    ///
    /// # Hatalar
    ///
    /// Çözünürlük sıfırsa hata metni döndürür.
    pub fn en_boy(&self) -> Result<EnBoy, String> {
        EnBoy::cozunurlukten(self.genislik, self.yukseklik)
    }
}

/// Hesaplanmış kadraj planı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Kadraj {
    /// Uygulanacak biçim.
    pub kip: KadrajKipi,
    /// Kaynak çözünürlük genişliği.
    pub kaynak_genislik: u32,
    /// Kaynak çözünürlük yüksekliği.
    pub kaynak_yukseklik: u32,
    /// Hedef (profil) çözünürlük genişliği.
    pub hedef_genislik: u32,
    /// Hedef (profil) çözünürlük yüksekliği.
    pub hedef_yukseklik: u32,
    /// Uygulanacak kırpma (yoksa `None`).
    pub kirpma: Option<Kirpma>,
    /// Uygulanacak dolgu (yoksa `None`).
    pub dolgu: Option<Dolgu>,
    /// Kaynaktan hedefe ölçek çarpanı (1'den küçükse küçültme yapılır).
    pub olcek: f64,
    /// Hesabı sırasında üretilen uyarılar.
    pub uyarilar: Vec<String>,
}

impl Kadraj {
    /// Ölçekleme sonrası elde edilen çerçeve boyutunu döndürür.
    pub fn cikti_boyutu(&self) -> (u32, u32) {
        match self.kirpma {
            Some(kirpma) => (
                (f64::from(kirpma.genislik) * self.olcek).round() as u32,
                (f64::from(kirpma.yukseklik) * self.olcek).round() as u32,
            ),
            None => (
                (f64::from(self.kaynak_genislik) * self.olcek).round() as u32,
                (f64::from(self.kaynak_yukseklik) * self.olcek).round() as u32,
            ),
        }
    }
}

/// Kaynak kadrajdan profil hedefine geçişi hesaplar.
///
/// # Hatalar
///
/// Kaynak ya da hedef çözünürlük sıfırsa [`ClipForgeHata::CozunurlukYok`]
/// döner. Profil tanımı doğrulanmamışsa davranış tanımsızdır; çağıran taraf
/// önce [`PlatformProfili::dogrula`] çalıştırmalıdır.
pub fn hesapla(kaynak: &KaynakKadraj, profil: &PlatformProfili) -> Result<Kadraj, ClipForgeHata> {
    if kaynak.genislik == 0 || kaynak.yukseklik == 0 {
        return Err(ClipForgeHata::CozunurlukYok {
            parca: kaynak.parca_iz,
        });
    }
    if profil.genislik == 0 || profil.yukseklik == 0 {
        return Err(ClipForgeHata::ProfilHatali {
            kimlik: profil.kimlik.clone(),
            ayrinti: "hedef cozunurluk sifir".to_string(),
        });
    }

    let hedef_genislik = profil.genislik;
    let hedef_yukseklik = profil.yukseklik;
    let kaynak_oran = f64::from(kaynak.genislik) / f64::from(kaynak.yukseklik);
    let hedef_oran = f64::from(hedef_genislik) / f64::from(hedef_yukseklik);
    let mut uyarilar: Vec<String> = Vec::new();

    // Adım 1: kırpma kararı. Çıpa hedef **genişliktir**.
    let oran_esit = (kaynak_oran - hedef_oran).abs() <= ORAN_TOLERANSI;
    let (kirpma, kalan_genislik, kalan_yukseklik) = if oran_esit {
        (None, kaynak.genislik, kaynak.yukseklik)
    } else if kaynak.genislik > hedef_genislik {
        // Kaynak hedef genişliğinden geniş: yükseklik korunur, genişlik
        // hedef orana göre kırpılır. Kırpma, hedeften daha geniş olduğu için
        // kaynak sınırının dışına taşmaz.
        let yeni_genislik =
            ((f64::from(kaynak.yukseklik) * hedef_oran).round() as u32).clamp(1, kaynak.genislik);
        let x = (kaynak.genislik - yeni_genislik) / 2;
        uyarilar.push(format!(
            "kaynak {}:{} genisligi hedef {}:{}'den genis: yatayda {} piksel kirpiliyor (orani {:.4} -> {:.4})",
            kaynak.genislik,
            kaynak.yukseklik,
            hedef_genislik,
            hedef_yukseklik,
            kaynak.genislik - yeni_genislik,
            kaynak_oran,
            hedef_oran
        ));
        (
            Some(Kirpma {
                x,
                y: 0,
                genislik: yeni_genislik,
                yukseklik: kaynak.yukseklik,
            }),
            yeni_genislik,
            kaynak.yukseklik,
        )
    } else {
        // Kaynak hedef genişliğine sığıyor: kırpma yapılmaz, ölçeklemeden
        // sonra kalan alan dolguyla kapatılır.
        (None, kaynak.genislik, kaynak.yukseklik)
    };

    // Adım 2: hedef çerçeveye sığdıracak ölçek çarpanı.
    let olcek = (f64::from(hedef_genislik) / f64::from(kalan_genislik))
        .min(f64::from(hedef_yukseklik) / f64::from(kalan_yukseklik));
    let cikti_genislik = (f64::from(kalan_genislik) * olcek).round() as u32;
    let cikti_yukseklik = (f64::from(kalan_yukseklik) * olcek).round() as u32;

    // Adım 3: ölçekleme sonrası kalan boşluk dolguyla kapatılır.
    let dolgu = if cikti_genislik < hedef_genislik || cikti_yukseklik < hedef_yukseklik {
        let dolgu_genislik = hedef_genislik - cikti_genislik;
        let dolgu_yukseklik = hedef_yukseklik - cikti_yukseklik;
        let x = (hedef_genislik - cikti_genislik) / 2;
        let y = (hedef_yukseklik - cikti_yukseklik) / 2;
        Some(Dolgu {
            x,
            y,
            genislik: dolgu_genislik,
            yukseklik: dolgu_yukseklik,
        })
    } else {
        None
    };

    if kirpma.is_none() {
        if dolgu.is_some() {
            uyarilar.push(format!(
                "kaynak {}:{} hedefe {}:{} sigiyor ama orani farkli: kenarlar dolguyla tamamlaniyor",
                kaynak.genislik, kaynak.yukseklik, hedef_genislik, hedef_yukseklik
            ));
        } else {
            uyarilar.push(format!(
                "kaynak orani hedefle ayni ({:.4}): yalnizca olcekleme",
                hedef_oran
            ));
        }
    } else if let Some(dolgulu) = dolgu {
        uyarilar.push(format!(
            "tam sayi yuvarlamasi nedeniyle {}x{} piksel dolgu eklendi",
            dolgulu.genislik, dolgulu.yukseklik
        ));
    }

    if olcek > 1.0 {
        uyarilar.push(format!(
            "kaynak hucre kucultulmuyor, buyutme orani {:.4}",
            olcek
        ));
    }

    let kip = if kirpma.is_some() {
        KadrajKipi::Kirp
    } else if dolgu.is_some() {
        KadrajKipi::Dolgu
    } else {
        KadrajKipi::Kirpmasiz
    };

    Ok(Kadraj {
        kip,
        kaynak_genislik: kaynak.genislik,
        kaynak_yukseklik: kaynak.yukseklik,
        hedef_genislik,
        hedef_yukseklik,
        kirpma,
        dolgu,
        olcek,
        uyarilar,
    })
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::profil::ProfilKutusu;
    use crate::test_yardimci::iceriyor;

    /// Verilen kimlikteki gömülü profili döndürür.
    fn profil(kimlik: &str) -> PlatformProfili {
        ProfilKutusu::gomulu().unwrap().ara(kimlik).unwrap().clone()
    }

    /// Kaynak çözünürlüğü tanımlar.
    fn kaynak(genislik: u32, yukseklik: u32) -> KaynakKadraj {
        KaynakKadraj {
            parca_iz: 1,
            genislik,
            yukseklik,
        }
    }

    #[test]
    fn yatay_kaynaktan_dikey_hedefe_kirpma_hesaplanir() {
        // 3840x2160 (16:9) -> 1080x1920 (9:16): 2160*9/16 = 1215 tam sayi.
        let k = hesapla(&kaynak(3840, 2160), &profil("reels")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Kirp);
        let kirpma = k.kirpma.expect("kirpma bekleniyordu");
        assert_eq!(kirpma.genislik, 1215);
        assert_eq!(kirpma.yukseklik, 2160);
        assert_eq!(kirpma.x, 1312);
        assert_eq!(kirpma.y, 0);
        assert!(k.dolgu.is_none(), "tam bolunen olcude dolgu olmamali");
        assert_eq!(k.cikti_boyutu(), (1080, 1920));
        assert!((k.olcek - 1080.0 / 1215.0).abs() < 1e-9);
        assert!(k.uyarilar.iter().any(|u| iceriyor(u, "kirpiliyor")));
    }

    #[test]
    fn kare_kaynaktan_dikey_hedefe_dolgu_hesaplanir() {
        // 1080x1080 (1:1) -> 1080x1920 (9:16): kırpma değil dolgu.
        let k = hesapla(&kaynak(1080, 1080), &profil("reels")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Dolgu);
        assert!(k.kirpma.is_none(), "dar kaynak kirpilmamali");
        let dolgu = k.dolgu.expect("dolgu bekleniyordu");
        assert_eq!(dolgu.genislik, 0);
        assert_eq!(dolgu.yukseklik, 840);
        assert_eq!(dolgu.x, 0);
        assert_eq!(dolgu.y, 420);
        assert_eq!(k.cikti_boyutu(), (1080, 1080));
        assert!((k.olcek - 1.0).abs() < 1e-9);
        assert!(k.uyarilar.iter().any(|u| iceriyor(u, "dolguyla")));
    }

    #[test]
    fn ayni_oranda_yalnizca_olcekleme_yapilir() {
        let k = hesapla(&kaynak(1920, 1080), &profil("youtube-16x9")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Kirpmasiz);
        assert!(k.kirpma.is_none());
        assert!(k.dolgu.is_none());
        assert_eq!(k.cikti_boyutu(), (1920, 1080));
        assert!((k.olcek - 1.0).abs() < 1e-9);
        assert!(k.uyarilar.iter().any(|u| iceriyor(u, "olcekleme")));
    }

    #[test]
    fn buyuk_kaynak_hedefe_kucultulur() {
        // 7680x4320 (8K 16:9) -> 1920x1080: kırpma yok, yalnız küçültme.
        let k = hesapla(&kaynak(7680, 4320), &profil("youtube-16x9")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Kirpmasiz);
        assert_eq!(k.cikti_boyutu(), (1920, 1080));
        assert!((k.olcek - 0.25).abs() < 1e-9);
    }

    #[test]
    fn kare_hedefe_dikey_kaynaktan_kirpma() {
        // 1920x1080 (16:9) -> 1080x1080 (1:1): yatay kırpma.
        let k = hesapla(&kaynak(1920, 1080), &profil("kare-akis")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Kirp);
        let kirpma = k.kirpma.expect("kirpma bekleniyordu");
        assert_eq!(kirpma.yukseklik, 1080);
        assert_eq!(kirpma.x, (1920 - 1080) / 2);
    }

    #[test]
    fn tam_sayma_yuvarlamasi_kucuk_dolgu_uretir() {
        // 1920x1080 -> 1080x1920: 1080*9/16 = 607.5 -> 608, kalan bir piksel dolgu.
        let k = hesapla(&kaynak(1920, 1080), &profil("reels")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Kirp);
        let kirpma = k.kirpma.expect("kirpma bekleniyordu");
        assert_eq!(kirpma.genislik, 608);
        let dolgu = k.dolgu.expect("1 piksel dolgu bekleniyordu");
        assert!(dolgu.genislik <= 2 && dolgu.yukseklik <= 2);
        assert!(k.uyarilar.iter().any(|u| iceriyor(u, "yuvarlamasi")));
    }

    #[test]
    fn cozunurluksuz_kaynak_hata_uretir() {
        let hata = hesapla(&kaynak(0, 1080), &profil("reels")).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::CozunurlukYok { parca: 1 }));
        assert!(hesapla(&kaynak(1920, 0), &profil("reels")).is_err());
    }

    #[test]
    fn hedef_cozunurluksuz_profil_hata_uretir() {
        let bozuk = PlatformProfili {
            genislik: 0,
            ..profil("reels")
        };
        let hata = hesapla(&kaynak(1920, 1080), &bozuk).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilHatali { .. }));
    }

    #[test]
    fn tolerans_ici_oran_kirpma_yapmaz() {
        // 1000x563 -> 1080x1920: oranlar 0.01 farkın içinde, dolgu beklenir.
        let k = hesapla(&kaynak(1000, 563), &profil("reels")).unwrap();
        assert!(k.kirpma.is_none());
        assert!(k.dolgu.is_some());
    }

    #[test]
    fn buyutme_uyarisi_uretilir() {
        // 360x640 dikey kaynak, 1080x1920 hedefe: büyütme gerekir.
        let k = hesapla(&kaynak(360, 640), &profil("reels")).unwrap();
        assert_eq!(k.kip, KadrajKipi::Kirpmasiz);
        assert!(k.olcek > 1.0);
        assert!(k.uyarilar.iter().any(|u| iceriyor(u, "buyutme")));
    }

    #[test]
    fn kaynak_kadraj_en_boy_hesaplar() {
        assert_eq!(
            kaynak(1920, 1080).en_boy().map(|o| o.to_string()),
            Ok("16:9".to_string())
        );
        assert!(kaynak(0, 0).en_boy().is_err());
    }

    #[test]
    fn kadraj_kipi_json_kisa_adlari() {
        let metin =
            serde_json::to_string(&[KadrajKipi::Kirpmasiz, KadrajKipi::Kirp, KadrajKipi::Dolgu])
                .unwrap();
        assert_eq!(metin, r#"["kirpmasiz","kirp","dolgu"]"#);
        assert_eq!(KadrajKipi::Dolgu.aciklama(), "kenar dolgusu");
    }
}
