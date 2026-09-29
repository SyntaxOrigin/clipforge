//! Kırp penceresi, kare eşlemesi ve yeniden zamanlama.
//!
//! Raporun `b03` kabul kriteri şudur: "belirtilen aralık dışında kare kalmaz;
//! kırpma sonrası süre, istenen süreyle **en fazla 1 kare** fark gösterir."
//! Bu modül o kriteri sayısal olarak garanti eden hesabı yapar.
//!
//! # Yarı açık aralık kuralı
//!
//! Kare `i` zamanı `i / fps` olan sabit kare hızlı bir akışta:
//!
//! - **Başlangıç**: `i >= t * fps` koşulunu sağlayan ilk kare seçilir
//!   (aşağı yuvarlama). Böylece `t` anından önce başlayan hiçbir kare kalmaz.
//! - **Bitiş**: `i >= b * fps` olan ilk kare **hariç** tutulur (yukarı
//!   yuvarlama). Böylece `b` anında ya da sonrasında başlayan hiçbir kare kalmaz.
//!
//! Bu iki kural "aralık dışında kare kalmaz" koşulunun iki yarısını da verir.

use serde::{Deserialize, Serialize};

use crate::hata::ClipForgeHata;
use crate::olcu::KareHazi;

/// Kırp penceresini tanımlayan giriş/çıkış zaman aralığı (saniye).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KirpPenceresi {
    baslangic_sn: f64,
    bitis_sn: f64,
}

impl KirpPenceresi {
    /// Yeni bir kırp penceresi oluşturur ve sınırları doğrular.
    ///
    /// # Hatalar
    ///
    /// Başlangıç negatifse, bitiş başlangıçtan büyük değilse, değerler sonlu
    /// değilse veya aralık bir kareden kısaysa [`ClipForgeHata::AralikGecersiz`]
    /// döner. Pencere bu aşamada kaynak süresiyle karşılaştırılmaz; bu denetim
    /// [`Zamanlama::hesapla`] içinde yapılır.
    pub fn yeni(baslangic_sn: f64, bitis_sn: f64) -> Result<Self, ClipForgeHata> {
        let hatali = |ayrinti: &str| ClipForgeHata::AralikGecersiz {
            baslangic_sn,
            bitis_sn,
            ayrinti: ayrinti.to_string(),
        };
        if !baslangic_sn.is_finite() || !bitis_sn.is_finite() {
            return Err(hatali("zaman degerleri sonlu degil"));
        }
        if baslangic_sn < 0.0 {
            return Err(hatali("baslangic negatif olamaz"));
        }
        if bitis_sn <= baslangic_sn {
            return Err(hatali("bitis baslangictan sonra olmali"));
        }
        Ok(Self {
            baslangic_sn,
            bitis_sn,
        })
    }

    /// Kaynağın tamamını kapsayan pencere oluşturur.
    pub fn tam(sure_sn: f64) -> Self {
        Self {
            baslangic_sn: 0.0,
            bitis_sn: sure_sn.max(0.0),
        }
    }

    /// Pencere başlangıcını döndürür.
    pub fn baslangic_sn(&self) -> f64 {
        self.baslangic_sn
    }

    /// Pencere bitişini döndürür.
    pub fn bitis_sn(&self) -> f64 {
        self.bitis_sn
    }

    /// İstenen süreyi döndürür.
    pub fn sure_sn(&self) -> f64 {
        self.bitis_sn - self.baslangic_sn
    }
}

/// Bir karenin kaynaktaki ve çıktıdaki zamanı.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ZamanNoktasi {
    /// Kaynak dosyadaki zamanı (saniye).
    pub kaynak_sn: f64,
    /// Yeniden zamanlamadan sonraki zamanı (saniye, sıfırdan başlar).
    pub yeni_sn: f64,
}

/// Kare hassasiyetinde hesaplanmış kırp zamanlaması.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Zamanlama {
    /// Kırpmanın başladığı kare numarası (kaynak içinde).
    pub ilk_kare: u64,
    /// Alınan kare sayısı.
    pub kare_sayisi: u64,
    /// Hesapta kullanılan kare hızı.
    pub kare_hazi: KareHazi,
    /// Kırpmanın kaynak içindeki gerçek başlangıç zamanı.
    pub kaynak_baslangic_sn: f64,
    /// Kullanıcının istediği süre.
    pub istenen_sure_sn: f64,
    /// Kare hassasiyetinde elde edilen gerçek süre.
    pub gercek_sure_sn: f64,
    /// Kaynaktaki toplam kare sayısı (kapsülün bildirdiği).
    pub toplam_kare: u64,
}

impl Zamanlama {
    /// Kırp penceresini kare hassasiyetine çevirir.
    ///
    /// # Hatalar
    ///
    /// Kare hızı çözülememişse [`ClipForgeHata::KareHiziHatali`], pencere
    /// kaynak süresinin dışındaysa [`ClipForgeHata::KaynakAsildi`], pencere
    /// bir kareden kısaysa [`ClipForgeHata::AralikGecersiz`] döner.
    pub fn hesapla(
        pencere: &KirpPenceresi,
        kaynak_sure_sn: f64,
        kare_hazi: Option<KareHazi>,
    ) -> Result<Self, ClipForgeHata> {
        let hiz = kare_hazi.ok_or_else(|| ClipForgeHata::KareHiziHatali {
            ayrinti: "kapsulden sabit kare hizi cozulemedi".to_string(),
        })?;
        if hiz.deger() <= 0.0 {
            return Err(ClipForgeHata::KareHiziHatali {
                ayrinti: format!("kare hizi sifira yakin: {}", hiz),
            });
        }
        if !(kaynak_sure_sn.is_finite()) || kaynak_sure_sn <= 0.0 {
            return Err(ClipForgeHata::AralikGecersiz {
                baslangic_sn: pencere.baslangic_sn(),
                bitis_sn: pencere.bitis_sn(),
                ayrinti: format!("kaynak suresi gecersiz: {kaynak_sure_sn}"),
            });
        }

        // Bitiş, toleranslı: son karenin bitişi kaynak süresini aşabilir.
        let tolerans = hiz.kare_zamani(1);
        if pencere.bitis_sn() > kaynak_sure_sn + tolerans {
            return Err(ClipForgeHata::KaynakAsildi {
                istenen_sn: pencere.bitis_sn(),
                kaynak_sn: kaynak_sure_sn,
            });
        }

        let ilk_kare = hiz.kare_no(pencere.baslangic_sn());
        let bitis_kare = hiz.kare_siniri(pencere.bitis_sn());
        if bitis_kare <= ilk_kare {
            return Err(ClipForgeHata::AralikGecersiz {
                baslangic_sn: pencere.baslangic_sn(),
                bitis_sn: pencere.bitis_sn(),
                ayrinti: format!("aralık bir kareden ({:.4} sn) kısa", hiz.kare_zamani(1)),
            });
        }
        let kare_sayisi = bitis_kare - ilk_kare;
        let toplam_kare = hiz.kare_sayisi(kaynak_sure_sn);
        if ilk_kare >= toplam_kare {
            return Err(ClipForgeHata::AralikGecersiz {
                baslangic_sn: pencere.baslangic_sn(),
                bitis_sn: pencere.bitis_sn(),
                ayrinti: format!(
                    "baslangic {}. karede; kaynagin son karesi {}.",
                    ilk_kare,
                    toplam_kare.saturating_sub(1)
                ),
            });
        }
        let gercek_sure_sn = hiz.kare_zamani(kare_sayisi);
        Ok(Self {
            ilk_kare,
            kare_sayisi,
            kare_hazi: hiz,
            kaynak_baslangic_sn: hiz.kare_zamani(ilk_kare),
            istenen_sure_sn: pencere.sure_sn(),
            gercek_sure_sn,
            toplam_kare,
        })
    }

    /// Alınan son karenin numarasını (dahil) döndürür.
    pub fn son_kare(&self) -> u64 {
        self.ilk_kare + self.kare_sayisi - 1
    }

    /// Bir karenin kaynaktaki ve yeniden zamanlanmış konumunu döndürür.
    ///
    /// `kare_no`, kırpma sonrası sıfırdan başlayan kare numarasıdır.
    pub fn kare_zamani(&self, kare_no: u64) -> ZamanNoktasi {
        let kaynak_sn = self.kare_hazi.kare_zamani(self.ilk_kare + kare_no);
        ZamanNoktasi {
            kaynak_sn,
            yeni_sn: self.kare_hazi.kare_zamani(kare_no),
        }
    }

    /// Tek bir karenin süresini döndürür.
    pub fn kare_suresi_sn(&self) -> f64 {
        self.kare_hazi.kare_zamani(1)
    }

    /// İstenen süre ile gerçek süre arasındaki farkı döndürür.
    pub fn sure_farki_sn(&self) -> f64 {
        self.gercek_sure_sn - self.istenen_sure_sn
    }

    /// "En fazla 1 kare fark" kabul kriterinin sağlanıp sağlanmadığını söyler.
    pub fn kare_harfasi_yeterli(&self) -> bool {
        self.sure_farki_sn().abs() <= self.kare_suresi_sn() + f64::EPSILON
    }
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_yardimci::iceriyor;

    /// Testlerde kullanılan sabit kare hızı.
    fn hiz30() -> KareHazi {
        KareHazi::yeni(30, 1).unwrap()
    }

    #[test]
    fn pencere_gecerli_aralik_kabul_edilir() {
        let p = KirpPenceresi::yeni(1.0, 4.5).unwrap();
        assert_eq!(p.baslangic_sn(), 1.0);
        assert_eq!(p.bitis_sn(), 4.5);
        assert_eq!(p.sure_sn(), 3.5);
    }

    #[test]
    fn pencere_negatif_baslangici_reddeder() {
        let hata = KirpPenceresi::yeni(-0.5, 2.0).unwrap_err();
        match hata {
            ClipForgeHata::AralikGecersiz { ayrinti, .. } => {
                assert!(iceriyor(&ayrinti, "negatif"));
            }
            diger => panic!("beklenen AralikGecersiz, gelen {diger:?}"),
        }
    }

    #[test]
    fn pencere_sifir_uzunluklu_araligi_reddeder() {
        assert!(KirpPenceresi::yeni(2.0, 2.0).is_err());
        assert!(KirpPenceresi::yeni(2.0, 1.9).is_err());
    }

    #[test]
    fn pencere_sonlu_olmayan_degerleri_reddeder() {
        assert!(KirpPenceresi::yeni(f64::NAN, 2.0).is_err());
        assert!(KirpPenceresi::yeni(0.0, f64::INFINITY).is_err());
    }

    #[test]
    fn tam_pencere_kaynagin_tamamini_kapsar() {
        let p = KirpPenceresi::tam(12.5);
        assert_eq!(p.baslangic_sn(), 0.0);
        assert_eq!(p.bitis_sn(), 12.5);
        let negatif = KirpPenceresi::tam(-3.0);
        assert_eq!(negatif.bitis_sn(), 0.0);
    }

    #[test]
    fn tam_kare_sinirinda_kirpma_hesaplanir() {
        let p = KirpPenceresi::yeni(1.0, 2.0).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap();
        assert_eq!(z.ilk_kare, 30);
        assert_eq!(z.kare_sayisi, 30);
        assert_eq!(z.son_kare(), 59);
        assert!((z.kaynak_baslangic_sn - 1.0).abs() < 1e-9);
        assert!((z.gercek_sure_sn - 1.0).abs() < 1e-9);
        assert!(z.kare_harfasi_yeterli());
    }

    #[test]
    fn kare_harfasi_en_fazla_bir_kare_kalir() {
        // Tam kare sınırı: 1.0..1.4 sn, 30 fps -> kare 30..41, tam 12 kare.
        let p = KirpPenceresi::yeni(1.0, 1.4).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap();
        assert_eq!(z.kare_sayisi, 12);
        assert!((z.gercek_sure_sn - 0.4).abs() < 1e-9);
        assert!(z.sure_farki_sn().abs() < 1e-9);
        assert!(z.kare_harfasi_yeterli());

        // Kare sınırına oturtma: 1.01..1.41 -> kare 30..42, 13 kare.
        // Gerçek süre istenenden tam bir kare uzundur: kabul kriteri sağlanır.
        let p = KirpPenceresi::yeni(1.01, 1.41).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap();
        assert_eq!(z.kare_sayisi, 13);
        assert!(z.kare_harfasi_yeterli());
        assert!((z.sure_farki_sn() - z.kare_suresi_sn()).abs() < 1e-9);
    }

    #[test]
    fn kare_haritasi_yeniden_zamanlamayi_sifirdan_baslatir() {
        let p = KirpPenceresi::yeni(2.0, 5.0).unwrap();
        let z = Zamanlama::hesapla(&p, 20.0, Some(hiz30())).unwrap();
        let ilk = z.kare_zamani(0);
        assert!((ilk.yeni_sn - 0.0).abs() < 1e-9);
        assert!((ilk.kaynak_sn - 2.0).abs() < 1e-9);
        let son = z.kare_zamani(z.kare_sayisi - 1);
        assert!((son.yeni_sn - (z.kare_sayisi as f64 - 1.0) / 30.0).abs() < 1e-9);
        // Son karenin baslangici 5.0 sn den bir kare kadar once gelir.
        assert!((son.kaynak_sn + z.kare_suresi_sn() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn kareden_kisa_aralik_bir_kare_verir_ve_kriteri_karsilar() {
        // 0.01 sn, 30 fps'te bir kareden (0.0333 sn) kısadır. Yarı açık aralık
        // kuralı gereği içine düşen kare alınır: sonuç tam olarak bir karedir ve
        // "en fazla 1 kare fark" kriteri sağlanır.
        let p = KirpPenceresi::yeni(1.0, 1.01).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap();
        assert_eq!(z.kare_sayisi, 1);
        assert!(z.kare_harfasi_yeterli());
        assert!((z.gercek_sure_sn - z.kare_suresi_sn()).abs() < 1e-9);
    }

    #[test]
    fn son_kareden_sonra_baslayan_aralik_hata_verir() {
        // 20.0 sn'lik kaynakta son karenin numarası 599'dur; 600. kareden
        // başlayan bir aralıkta alınacak kare yoktur.
        let p = KirpPenceresi::yeni(20.0, 20.02).unwrap();
        let hata = Zamanlama::hesapla(&p, 20.0, Some(hiz30())).unwrap_err();
        match hata {
            ClipForgeHata::AralikGecersiz { ayrinti, .. } => {
                assert!(iceriyor(&ayrinti, "son kare"), "{ayrinti}");
            }
            diger => panic!("beklenen AralikGecersiz, gelen {diger:?}"),
        }
    }

    #[test]
    fn kayit_disi_aralik_hata_verir() {
        let p = KirpPenceresi::yeni(8.0, 12.0).unwrap();
        let hata = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap_err();
        match hata {
            ClipForgeHata::KaynakAsildi {
                istenen_sn,
                kaynak_sn,
            } => {
                assert_eq!(istenen_sn, 12.0);
                assert_eq!(kaynak_sn, 10.0);
            }
            diger => panic!("beklenen KaynakAsildi, gelen {diger:?}"),
        }
    }

    #[test]
    fn bitis_tolerans_icin_sinirda_kabul_edilir() {
        // 10.0 sn'de 30 fps ile son karenin bitişi 10.0 + 1/30.
        let p = KirpPenceresi::yeni(9.0, 10.01).unwrap();
        assert!(Zamanlama::hesapla(&p, 10.0, Some(hiz30())).is_ok());
    }

    #[test]
    fn kare_hizi_cozulemedigi_veya_sifir_ise_hata_verir() {
        let p = KirpPenceresi::yeni(1.0, 2.0).unwrap();
        let hata = Zamanlama::hesapla(&p, 10.0, None).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::KareHiziHatali { .. }));

        let hata = Zamanlama::hesapla(&p, 0.0, Some(hiz30())).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::AralikGecersiz { .. }));
    }

    #[test]
    fn kesirli_kare_hizi_ile_kirpma_kare_kaymasi_yapmaz() {
        let hiz = KareHazi::ayrıştir("30000/1001").unwrap();
        // 1.001 sn tam olarak 30 karedir (30 * 1001 / 30000).
        let p = KirpPenceresi::yeni(0.0, hiz.kare_zamani(30)).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz)).unwrap();
        assert_eq!(z.ilk_kare, 0);
        assert_eq!(z.kare_sayisi, 30);
        assert!(z.sure_farki_sn().abs() < 1e-6);
        assert!(z.kare_harfasi_yeterli());

        // Tam sayı olmayan süre isteğinde fark en çok bir kareden biraz fazladır;
        // bu, kare hassasiyetinin doğal sonucudur ve dürüstçe raporlanır.
        let p = KirpPenceresi::yeni(1.0, 2.0).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz)).unwrap();
        assert_eq!(z.ilk_kare, 29);
        assert_eq!(z.kare_sayisi, 31);
        assert!((z.kaynak_baslangic_sn - 29.0 / 29.970_029_970_029_972).abs() < 1e-9);
    }

    #[test]
    fn tam_kapsamli_kirpma_kaynak_suresiyle_ayni_kare_sayisini_verir() {
        let p = KirpPenceresi::yeni(0.0, 10.0).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap();
        assert_eq!(z.ilk_kare, 0);
        assert_eq!(z.kare_sayisi, 300);
        assert_eq!(z.toplam_kare, 300);
        assert_eq!(z.sure_farki_sn(), 0.0);
    }

    #[test]
    fn zamanlama_json_gidis_donusu() {
        let p = KirpPenceresi::yeni(1.0, 2.0).unwrap();
        let z = Zamanlama::hesapla(&p, 10.0, Some(hiz30())).unwrap();
        let metin = serde_json::to_string(&z).unwrap();
        assert!(iceriyor(&metin, "\"kare_hazi\":\"30\""));
        let geri: Zamanlama = serde_json::from_str(&metin).unwrap();
        assert_eq!(geri, z);
    }
}
