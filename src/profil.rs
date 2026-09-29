//! Platform profilleri: gönderilecek platformu seç, türetilmiş ayarları al.
//!
//! Bu, raporun (`b01` Yönetici Özeti) asıl fikridir: kullanıcı codec/CRF gibi
//! kodlayıcı terimleriyle değil, **klibi nereye göndereceğiyle** uğraşır.
//!
//! Gömülü katalog programla birlikte gelir ve diske yazılmaz; bu, "program
//! dizini dışına yazmama" taşınabilirlik ilkesini korur. Kullanıcı `--config`
//! ile kendi profil listesini (JSON) geçebilir.
//!
//! Profiller gömülü JSON metninden yüklenir. Böylece hem gömülü hem kullanıcı
//! profilleri aynı doğrulama ve aynı şema yolundan geçer; iki ayrı kod yolu
//! bulunmaz.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::hata::ClipForgeHata;
use crate::olcu::{EnBoy, KareHazi};

/// Profil kataloğunun şema sürümü.
pub const KATALOG_SURUMU: u32 = 1;

/// Platformun dayattığı ses kısıtları.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SesKisitlari {
    /// Tercih edilen örnekleme hızı (Hz).
    #[serde(default = "varsayilan_ornekleme")]
    pub ornekleme_hizi: u32,
    /// Tercih edilen kanal sayısı.
    #[serde(default = "varsayilan_kanal")]
    pub kanal: u16,
    /// Tercih edilen ses bit hızı (kbit/s).
    #[serde(default = "varsayilan_ses_biti")]
    pub bit_hizi_kbps: u32,
    /// Platformun kabul ettiği örnekleme hızları listesi.
    #[serde(default)]
    pub izinli_ornekleme_hizlari: Vec<u32>,
}

fn varsayilan_ornekleme() -> u32 {
    48_000
}
fn varsayilan_kanal() -> u16 {
    2
}
fn varsayilan_ses_biti() -> u32 {
    192
}

impl Default for SesKisitlari {
    fn default() -> Self {
        Self {
            ornekleme_hizi: varsayilan_ornekleme(),
            kanal: varsayilan_kanal(),
            bit_hizi_kbps: varsayilan_ses_biti(),
            izinli_ornekleme_hizlari: Vec::new(),
        }
    }
}

impl SesKisitlari {
    /// Örnekleme hızının izinli listede olup olmadığını söyler.
    ///
    /// İzinli liste boşsa her hız kabul edilir (kısıt bildirilmemiş demektir).
    pub fn ornekleme_izinli_mi(&self, hiz: u32) -> bool {
        self.izinli_ornekleme_hizlari.is_empty() || self.izinli_ornekleme_hizlari.contains(&hiz)
    }
}

/// Tek bir platform profili.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformProfili {
    /// Tekil kimlik (küçük harf, rakam ve tire). Komut satırında bu kullanılır.
    pub kimlik: String,
    /// İnsan okunur ad.
    pub ad: String,
    /// Hedef platformun adı.
    pub platform: String,
    /// Hedef en-boy oranı (ör. `9:16`).
    pub en_boy: EnBoy,
    /// Hedef çözünürlüğün genişliği (piksel).
    pub genislik: u32,
    /// Hedef çözünürlüğün yüksekliği (piksel).
    pub yukseklik: u32,
    /// Tercih edilen kare hızı. Verilmezse 30 kare/s varsayılır.
    #[serde(default)]
    pub tercih_edilen_kare_hazi: KareHazi,
    /// Platformun kabul ettiği en yüksek kare hızı. Verilmezse 60 kare/s varsayılır.
    #[serde(default = "varsayilan_azami_kare_hazi")]
    pub azami_kare_hazi: KareHazi,
    /// Önerilen video bit hızı (kbit/s).
    pub video_bit_hizi_kbps: u32,
    /// Platformun kabul ettiği en yüksek video bit hızı (kbit/s).
    #[serde(default)]
    pub azami_video_bit_hizi_kbps: Option<u32>,
    /// Anahtar kare aralığı (kare sayısı).
    pub gop_kare: u32,
    /// Ses kısıtları.
    #[serde(default)]
    pub ses: SesKisitlari,
    /// Profilin dayandığı birincil belge bağlantısı.
    #[serde(default)]
    pub kaynak: String,
    /// Profilin doğrulandığı tarih (ISO-8601).
    #[serde(default)]
    pub dogrulanma_tarihi: String,
}

/// Varsayılan en yüksek kare hızı: 60 kare/s.
fn varsayilan_azami_kare_hazi() -> KareHazi {
    KareHazi::tam(60)
}

impl PlatformProfili {
    /// Profil tanımını semantik olarak doğrular.
    ///
    /// # Hatalar
    ///
    /// Kimlik biçimi bozuksa, çözünürlük sıfırsa, çözünürlük en-boy oranıyla
    /// uyuşmuyorsa, bit hızı ya da GOP sıfırsa veya ses kısıtları çelişkiliyse
    /// [`ClipForgeHata::ProfilHatali`] döner.
    pub fn dogrula(&self) -> Result<(), ClipForgeHata> {
        let hatali = |ayrinti: &str| ClipForgeHata::ProfilHatali {
            kimlik: self.kimlik.clone(),
            ayrinti: ayrinti.to_string(),
        };
        if self.kimlik.is_empty()
            || !self
                .kimlik
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(hatali(
                "kimlik yalnizca kucuk harf, rakam ve tire icerebilir",
            ));
        }
        if self.ad.trim().is_empty() {
            return Err(hatali("ad bos olamaz"));
        }
        if self.genislik == 0 || self.yukseklik == 0 {
            return Err(hatali("cozunurluk sifir olamaz"));
        }
        if self.genislik % 2 != 0 || self.yukseklik % 2 != 0 {
            return Err(hatali("cozunurluk en-boy orani icin cift sayi olmali"));
        }
        if !self
            .en_boy
            .cozunurluge_uyuyor_mu(self.genislik, self.yukseklik)
        {
            return Err(hatali(&format!(
                "cozunurluk {}:{} en-boy oraniyla ({}) uyusmuyor",
                self.genislik, self.yukseklik, self.en_boy
            )));
        }
        if self.video_bit_hizi_kbps == 0 {
            return Err(hatali("video bit hizi sifir olamaz"));
        }
        if let Some(azami) = self.azami_video_bit_hizi_kbps {
            if azami < self.video_bit_hizi_kbps {
                return Err(hatali(
                    "azami video bit hizi onerilen bit hizindan kucuk olamaz",
                ));
            }
        }
        if self.gop_kare == 0 {
            return Err(hatali("GOP kare sayisi sifir olamaz"));
        }
        if self.tercih_edilen_kare_hazi.deger() > self.azami_kare_hazi.deger() {
            return Err(hatali(
                "tercih edilen kare hizi azami kare hizindan buyuk olamaz",
            ));
        }
        if self.ses.kanal == 0 {
            return Err(hatali("ses kanal sayisi sifir olamaz"));
        }
        if self.ses.bit_hizi_kbps == 0 {
            return Err(hatali("ses bit hizi sifir olamaz"));
        }
        if self.ses.ornekleme_hizi == 0 {
            return Err(hatali("ses ornekleme hizi sifir olamaz"));
        }
        if !self.ses.ornekleme_izinli_mi(self.ses.ornekleme_hizi) {
            return Err(hatali(
                "tercih edilen ses ornekleme hizi izinli listede degil",
            ));
        }
        Ok(())
    }

    /// Profilin hedef çözünürlüğünü döndürür.
    pub fn cozunurluk(&self) -> (u32, u32) {
        (self.genislik, self.yukseklik)
    }

    /// Kaynak kare hızı profilde tanımlı değilse `None` döner.
    pub fn kare_hizi_hedefi(&self) -> KareHazi {
        self.tercih_edilen_kare_hazi
    }
}

/// Bir dizi platform profilinden oluşan katalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilKutusu {
    /// Şema sürümü.
    pub surum: u32,
    /// Profiller.
    pub profiller: Vec<PlatformProfili>,
}

impl ProfilKutusu {
    /// Ham JSON metninden katalog oluşturur ve tüm profilleri doğrular.
    ///
    /// # Hatalar
    ///
    /// JSON çözülemezse [`ClipForgeHata::ProfilDosyasiHatali`], herhangi bir
    /// profil geçersizse [`ClipForgeHata::ProfilHatali`] döner. Aynı kimlik
    /// iki kez geçerse de hata verilir.
    pub fn ayristir(metin: &str) -> Result<Self, ClipForgeHata> {
        let kutu: Self =
            serde_json::from_str(metin).map_err(|hata| ClipForgeHata::ProfilDosyasiHatali {
                ayrinti: hata.to_string(),
            })?;
        kutu.dogrula()?;
        Ok(kutu)
    }

    /// Dosyadan katalog yükler.
    ///
    /// # Hatalar
    ///
    /// Dosya okunamazsa [`ClipForgeHata::GirdiHatasi`], içerik geçersizse
    /// [`ClipForgeHata::ProfilDosyasiHatali`] ya da [`ClipForgeHata::ProfilHatali`] döner.
    pub fn yukle(yol: &Path) -> Result<Self, ClipForgeHata> {
        let metin = std::fs::read_to_string(yol).map_err(|hata| ClipForgeHata::girdi(yol, hata))?;
        Self::ayristir(&metin)
    }

    /// Programla birlikte gelen gömülü katalogu döndürür.
    ///
    /// Katalog JSON metninden yeniden ayrıştırılır; bu, gömülü tanımların da
    /// kullanıcı tanımlarıyla aynı doğrulamadan geçmesini sağlar. Katalog
    /// küçüktür (altı profil), bu yüzden çağrı başına yeniden ayrıştırmak
    /// ölçülebilir bir maliyet getirmez.
    ///
    /// # Hatalar
    ///
    /// Gömülü tanım bir programlama hatası sonucu geçersizse
    /// [`ClipForgeHata::ProfilDosyasiHatali`] döner. Testler bunu yakalar.
    pub fn gomulu() -> Result<Self, ClipForgeHata> {
        Self::ayristir(GOMULU_KATALOG_JSON)
    }

    /// Bütün profilleri ve kimlik benzersizliğini doğrular.
    ///
    /// # Hatalar
    ///
    /// Sürüm desteklenmiyorsa, liste boşsa, profil geçersizse veya iki profil
    /// aynı kimliği taşıyorsa hata döner.
    pub fn dogrula(&self) -> Result<(), ClipForgeHata> {
        if self.surum != KATALOG_SURUMU {
            return Err(ClipForgeHata::ProfilDosyasiHatali {
                ayrinti: format!(
                    "desteklenmeyen katalog surumu {} (beklenen {KATALOG_SURUMU})",
                    self.surum
                ),
            });
        }
        if self.profiller.is_empty() {
            return Err(ClipForgeHata::ProfilDosyasiHatali {
                ayrinti: "katalogda profil yok".to_string(),
            });
        }
        for profil in &self.profiller {
            profil.dogrula()?;
        }
        for (sira, profil) in self.profiller.iter().enumerate() {
            if let Some(cakisma) = self.profiller[sira + 1..]
                .iter()
                .find(|diger| diger.kimlik == profil.kimlik)
            {
                return Err(ClipForgeHata::ProfilHatali {
                    kimlik: profil.kimlik.clone(),
                    ayrinti: format!("kimlik '{}' iki kez tanimli", cakisma.kimlik),
                });
            }
        }
        Ok(())
    }

    /// Kimliğe göre profil arar.
    ///
    /// # Hatalar
    ///
    /// Kimlik katalogda yoksa [`ClipForgeHata::ProfilYok`] döner.
    pub fn ara(&self, kimlik: &str) -> Result<&PlatformProfili, ClipForgeHata> {
        self.profiller
            .iter()
            .find(|p| p.kimlik == kimlik)
            .ok_or_else(|| ClipForgeHata::ProfilYok {
                kimlik: kimlik.to_string(),
            })
    }

    /// Katalogdaki tüm profil kimliklerini döndürür.
    pub fn kimlikler(&self) -> Vec<String> {
        self.profiller.iter().map(|p| p.kimlik.clone()).collect()
    }
}

/// Programla birlikte gelen altı profillik gömülü katalog.
///
/// Değerler platformların kamuya açık yardım sayfalarındaki önerilerden
/// türetilmiştir ve **bu depoda ölçülmüş değildir**; her profilin `kaynak`
/// alanında birincil belgesi ve `dogrulanma_tarihi` alanında doğrulama
/// tarihi taşınır. Profil kütüphanesi bayatlaması bu alanların tazelenmesiyle
/// yönetilir.
pub const GOMULU_KATALOG_JSON: &str = r##"{
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
    },
    {
      "kimlik": "tiktok",
      "ad": "TikTok",
      "platform": "TikTok",
      "en_boy": "9:16",
      "genislik": 1080,
      "yukseklik": 1920,
      "tercih_edilen_kare_hazi": "30",
      "azami_kare_hazi": "60",
      "video_bit_hizi_kbps": 6000,
      "azami_video_bit_hizi_kbps": 10000,
      "gop_kare": 60,
      "ses": {
        "ornekleme_hizi": 48000,
        "kanal": 2,
        "bit_hizi_kbps": 192,
        "izinli_ornekleme_hizlari": [44100, 48000]
      },
      "kaynak": "https://support.tiktok.com/",
      "dogrulanma_tarihi": "2026-09-29"
    },
    {
      "kimlik": "shorts",
      "ad": "YouTube Shorts",
      "platform": "YouTube",
      "en_boy": "9:16",
      "genislik": 1080,
      "yukseklik": 1920,
      "tercih_edilen_kare_hazi": "30",
      "azami_kare_hazi": "60",
      "video_bit_hizi_kbps": 8000,
      "azami_video_bit_hizi_kbps": 14000,
      "gop_kare": 60,
      "ses": {
        "ornekleme_hizi": 48000,
        "kanal": 2,
        "bit_hizi_kbps": 192,
        "izinli_ornekleme_hizlari": [44100, 48000]
      },
      "kaynak": "https://support.google.com/youtube/",
      "dogrulanma_tarihi": "2026-09-29"
    },
    {
      "kimlik": "x-video",
      "ad": "X dikey video",
      "platform": "X",
      "en_boy": "16:9",
      "genislik": 1280,
      "yukseklik": 720,
      "tercih_edilen_kare_hazi": "30",
      "azami_kare_hazi": "60",
      "video_bit_hizi_kbps": 5000,
      "azami_video_bit_hizi_kbps": 8000,
      "gop_kare": 60,
      "ses": {
        "ornekleme_hizi": 48000,
        "kanal": 2,
        "bit_hizi_kbps": 192,
        "izinli_ornekleme_hizlari": [44100, 48000]
      },
      "kaynak": "https://help.x.com/",
      "dogrulanma_tarihi": "2026-09-29"
    },
    {
      "kimlik": "kare-akis",
      "ad": "Kare (1:1) akis",
      "platform": "coklu",
      "en_boy": "1:1",
      "genislik": 1080,
      "yukseklik": 1080,
      "tercih_edilen_kare_hazi": "30",
      "azami_kare_hazi": "60",
      "video_bit_hizi_kbps": 5000,
      "azami_video_bit_hizi_kbps": 8000,
      "gop_kare": 60,
      "ses": {
        "ornekleme_hizi": 48000,
        "kanal": 2,
        "bit_hizi_kbps": 192,
        "izinli_ornekleme_hizlari": [44100, 48000]
      },
      "kaynak": "https://help.instagram.com/",
      "dogrulanma_tarihi": "2026-09-29"
    },
    {
      "kimlik": "youtube-16x9",
      "ad": "YouTube yatay",
      "platform": "YouTube",
      "en_boy": "16:9",
      "genislik": 1920,
      "yukseklik": 1080,
      "tercih_edilen_kare_hazi": "30",
      "azami_kare_hazi": "60",
      "video_bit_hizi_kbps": 8000,
      "azami_video_bit_hizi_kbps": 16000,
      "gop_kare": 60,
      "ses": {
        "ornekleme_hizi": 48000,
        "kanal": 2,
        "bit_hizi_kbps": 192,
        "izinli_ornekleme_hizlari": [44100, 48000]
      },
      "kaynak": "https://support.google.com/youtube/",
      "dogrulanma_tarihi": "2026-09-29"
    }
  ]
}"##;

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_yardimci::iceriyor;
    use crate::test_yardimci::GeciciDizin;

    /// Gömülü katalogda geçerli bir profil kopyası üretir.
    fn ornek_profil() -> PlatformProfili {
        PlatformProfili {
            kimlik: "test-dikey".to_string(),
            ad: "Test dikey".to_string(),
            platform: "test".to_string(),
            en_boy: EnBoy::ayrıştir("9:16").unwrap(),
            genislik: 1080,
            yukseklik: 1920,
            tercih_edilen_kare_hazi: KareHazi::yeni(30, 1).unwrap(),
            azami_kare_hazi: KareHazi::yeni(60, 1).unwrap(),
            video_bit_hizi_kbps: 6000,
            azami_video_bit_hizi_kbps: Some(9000),
            gop_kare: 60,
            ses: SesKisitlari {
                ornekleme_hizi: 48_000,
                kanal: 2,
                bit_hizi_kbps: 192,
                izinli_ornekleme_hizlari: vec![44_100, 48_000],
            },
            kaynak: "https://ornek.invalid/".to_string(),
            dogrulanma_tarihi: "2026-09-29".to_string(),
        }
    }

    #[test]
    fn gomulu_katalog_alti_profil_icerir_ve_gecerlidir() {
        let kutu = ProfilKutusu::gomulu().unwrap();
        assert_eq!(kutu.surum, KATALOG_SURUMU);
        assert_eq!(kutu.profiller.len(), 6);
        for profil in &kutu.profiller {
            profil.dogrula().unwrap();
        }
        let kimlikler = kutu.kimlikler();
        assert!(kimlikler.contains(&"reels".to_string()));
        assert!(kimlikler.contains(&"tiktok".to_string()));
        assert!(kimlikler.contains(&"shorts".to_string()));
        assert!(kimlikler.contains(&"x-video".to_string()));
        assert!(kimlikler.contains(&"kare-akis".to_string()));
        assert!(kimlikler.contains(&"youtube-16x9".to_string()));
    }

    #[test]
    fn gomulu_katalog_dikey_yatay_ve_kare_oranlarini_kapsar() {
        let kutu = ProfilKutusu::gomulu().unwrap();
        let reels = kutu.ara("reels").unwrap();
        assert_eq!(reels.en_boy.to_string(), "9:16");
        let kare = kutu.ara("kare-akis").unwrap();
        assert_eq!(kare.en_boy.to_string(), "1:1");
        let yatay = kutu.ara("x-video").unwrap();
        assert_eq!(yatay.en_boy.to_string(), "16:9");
    }

    #[test]
    fn olmayan_profil_profilyok_hatasi_uretir() {
        let kutu = ProfilKutusu::gomulu().unwrap();
        let hata = kutu.ara("linkedin").unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilYok { .. }));
    }

    #[test]
    fn profil_dogrulama_hatalari_ayirt_edilir() {
        let mut profil = ornek_profil();

        profil.kimlik = "Kötü Kimlik".to_string();
        assert!(matches!(
            profil.dogrula(),
            Err(ClipForgeHata::ProfilHatali { .. })
        ));

        profil = ornek_profil();
        profil.genislik = 0;
        assert!(matches!(
            profil.dogrula(),
            Err(ClipForgeHata::ProfilHatali { .. })
        ));

        profil = ornek_profil();
        profil.yukseklik = 1080;
        assert!(profil.dogrula().is_err(), "9:16 oraninda 1080x1080 olmaz");

        profil = ornek_profil();
        profil.gop_kare = 0;
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.video_bit_hizi_kbps = 0;
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.azami_video_bit_hizi_kbps = Some(100);
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.tercih_edilen_kare_hazi = KareHazi::yeni(120, 1).unwrap();
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.ses.kanal = 0;
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.ses.ornekleme_hizi = 96_000;
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.ad = "   ".to_string();
        assert!(profil.dogrula().is_err());

        profil = ornek_profil();
        profil.genislik = 1081;
        assert!(
            profil.dogrula().is_err(),
            "tek sayili cozunurluk reddedilir"
        );

        assert!(ornek_profil().dogrula().is_ok());
    }

    #[test]
    fn bozuk_profil_dosyasi_ayristirma_hatasi_uretir() {
        let hata = ProfilKutusu::ayristir("{ bu json degil").unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilDosyasiHatali { .. }));
    }

    #[test]
    fn bos_katalog_reddedilir() {
        let hata = ProfilKutusu::ayristir(r#"{"surum":1,"profiller":[]}"#).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilDosyasiHatali { .. }));
    }

    #[test]
    fn desteklenmeyen_surum_reddedilir() {
        let hata = ProfilKutusu::ayristir(r#"{"surum":99,"profiller":[]}"#).unwrap_err();
        match hata {
            ClipForgeHata::ProfilDosyasiHatali { ayrinti } => assert!(iceriyor(&ayrinti, "99")),
            diger => panic!("beklenen ProfilDosyasiHatali, gelen {diger:?}"),
        }
    }

    #[test]
    fn ayni_kimlik_iki_kere_define_edilemez() {
        let mut kutu = ProfilKutusu::gomulu().unwrap();
        kutu.profiller.push(kutu.profiller[0].clone());
        let hata = kutu.dogrula().unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilHatali { .. }));
    }

    #[test]
    fn katalog_json_gidis_donusu_korunur() {
        let kutu = ProfilKutusu::gomulu().unwrap();
        let metin = serde_json::to_string_pretty(&kutu).unwrap();
        let geri = ProfilKutusu::ayristir(&metin).unwrap();
        assert_eq!(geri, kutu);
        assert!(iceriyor(&metin, "\"en_boy\": \"9:16\""));
        assert!(iceriyor(&metin, "\"tercih_edilen_kare_hazi\": \"30\""));
    }

    #[test]
    fn dosyadan_yukleme_calisir_ve_yoksa_hata_verir() {
        let gecici = GeciciDizin::yeni("clipforge-profil-dosya").unwrap();
        let yol = gecici.yol().join("profiller.json");
        std::fs::write(&yol, GOMULU_KATALOG_JSON).unwrap();
        let kutu = ProfilKutusu::yukle(&yol).unwrap();
        assert_eq!(kutu.profiller.len(), 6);

        let bozuk = gecici.yol().join("bozuk.json");
        std::fs::write(&bozuk, b"{\"surum\": 1, \"profiller\": 5}").unwrap();
        let hata = ProfilKutusu::yukle(&bozuk).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilDosyasiHatali { .. }));

        let hata = ProfilKutusu::yukle(&gecici.yol().join("yok.json")).unwrap_err();
        assert!(matches!(hata, ClipForgeHata::GirdiHatasi { .. }));
    }

    #[test]
    fn ses_kisitlari_izinli_mi_kurali() {
        let ses = SesKisitlari {
            ornekleme_hizi: 48_000,
            kanal: 2,
            bit_hizi_kbps: 192,
            izinli_ornekleme_hizlari: vec![44_100, 48_000],
        };
        assert!(ses.ornekleme_izinli_mi(44_100));
        assert!(ses.ornekleme_izinli_mi(48_000));
        assert!(!ses.ornekleme_izinli_mi(96_000));
        let kisitsiz = SesKisitlari::default();
        assert!(kisitsiz.ornekleme_izinli_mi(96_000));
    }

    #[test]
    fn profil_erisimcileri_kullanilir_ve_kaynak_tasir() {
        let kutu = ProfilKutusu::gomulu().unwrap();
        for profil in &kutu.profiller {
            assert!(!profil.kaynak.is_empty(), "{} kaynagi bos", profil.kimlik);
            assert!(!profil.dogrulanma_tarihi.is_empty());
            assert_eq!(profil.cozunurluk().0, profil.genislik);
            assert_eq!(profil.kare_hizi_hedefi(), profil.tercih_edilen_kare_hazi);
        }
    }

    #[test]
    fn profile_enum_alanlari_varsayilan_deger_alir() {
        let metin = r#"{
            "surum": 1,
            "profiller": [{
                "kimlik": "mini",
                "ad": "Mini",
                "platform": "test",
                "en_boy": "1:1",
                "genislik": 640,
                "yukseklik": 640,
                "video_bit_hizi_kbps": 2000,
                "gop_kare": 30
            }]
        }"#;
        let kutu = ProfilKutusu::ayristir(metin).unwrap();
        let profil = kutu.ara("mini").unwrap();
        assert_eq!(
            profil.tercih_edilen_kare_hazi,
            KareHazi::yeni(30, 1).unwrap()
        );
        assert_eq!(profil.azami_kare_hazi, KareHazi::yeni(60, 1).unwrap());
        assert_eq!(profil.ses, SesKisitlari::default());
        assert_eq!(profil.azami_video_bit_hizi_kbps, None);
    }

    #[test]
    fn profil_eksik_alanlarla_gelisir() {
        let metin = r#"{"surum":1,"profiller":[{"kimlik":"a"}]}"#;
        assert!(ProfilKutusu::ayristir(metin).is_err());
    }
}
