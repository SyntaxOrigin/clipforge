//! İnsan okunur metin çıktıları.
//!
//! Bu modül yalnızca **biçimlendirir**. Tüm sayısal kararlar
//! ([`crate::medya`], [`crate::kadraj`], [`crate::zamanlama`], [`crate::plan`])
//! başka modüllerde verilmiştir; burada yalnızca bunları okunur hâle getiririz.
//!
//! `--json` bayrağı kullanıldığında çıktı JSON olarak verilir; bu yol
//! [`crate::plan`] ve [`crate::kuyruk`] tarafından karşılanır.

use crate::kadraj::Kadraj;
use crate::medya::{Bicim, MedyaBilgisi, ParcaBilgisi};
use crate::olcu::EnBoy;
use crate::plan::KirpPlani;
use crate::profil::ProfilKutusu;

/// Üç ondalık basamağa yuvarlanmış süre biçimi.
fn sure(saniye: f64) -> String {
    format!("{saniye:.3} sn")
}

/// Ondalık basamakları koruyan kare hızı biçimi.
fn fps(hiz: &crate::olcu::KareHazi) -> String {
    format!("{} ({:.3})", hiz, hiz.deger())
}

/// En-boy oranı biçimi.
fn oran(deger: Option<EnBoy>) -> String {
    match deger {
        Some(deger) => deger.to_string(),
        None => "-".to_string(),
    }
}

/// Çözünürlük biçimi.
fn cozunurluk(parca: &ParcaBilgisi) -> String {
    match parca.cozunurluk {
        Some((g, y)) => format!("{g}x{y}"),
        None => "-".to_string(),
    }
}

/// `probe` alt komutunun insan okunur çıktısını üretir.
pub fn probe_metni(bilgi: &MedyaBilgisi) -> String {
    let mut satirlar: Vec<String> = Vec::new();
    satirlar.push(format!("dosya: {}", bilgi.yol.display()));
    satirlar.push(format!("bicim: {}", bilgi.bicim));
    if let Some(marka) = &bilgi.marka {
        satirlar.push(format!("marka: {marka}"));
    }
    satirlar.push(format!("dosya boyutu: {} bayt", bilgi.dosya_boyutu));
    satirlar.push(format!("sure: {}", sure(bilgi.sure_sn)));
    satirlar.push(format!(
        "parca sayisi: {} (video {}, ses {})",
        bilgi.parcalar.len(),
        bilgi.video_parca_sayisi(),
        bilgi.ses_parca_sayisi()
    ));
    satirlar.push(format!("faststart: {}", evet_hayir(bilgi.faststart)));

    for parca in &bilgi.parcalar {
        let mut satir = format!(
            "  [{}] {} iz={} codec={} kutu={} sure={}",
            parca.sira,
            parca.tur.kod(),
            parca.iz,
            if parca.codec.is_empty() {
                "-"
            } else {
                &parca.codec
            },
            parca.kutu_yolu,
            sure(parca.sure_sn)
        );
        if parca.tur == crate::iso_bmff::ParcasiTuru::Video {
            satir.push_str(&format!(
                " cozunurluk={} en_boy={}",
                cozunurluk(parca),
                oran(parca.en_boy())
            ));
            if let Some(hiz) = parca.kare_hazi {
                satir.push_str(&format!(" kare_hizi={}", fps(&hiz)));
            } else {
                satir.push_str(" kare_hazi=cozulemedi");
            }
            if let Some(adet) = parca.kare_sayisi {
                satir.push_str(&format!(" kare_sayisi={adet}"));
            }
            if let Some(anahtar) = parca.anahtar_kare_sayisi {
                satir.push_str(&format!(" anahtar_kare={anahtar}"));
            }
        } else if parca.tur == crate::iso_bmff::ParcasiTuru::Ses {
            satir.push_str(&format!(
                " ornekleme={} Hz kanal={}",
                parca
                    .ornekleme_hizi
                    .map_or_else(|| "-".to_string(), |h| h.to_string()),
                parca
                    .kanal
                    .map_or_else(|| "-".to_string(), |k| k.to_string())
            ));
        }
        satirlar.push(satir);
    }

    if bilgi.uyarilar.is_empty() {
        satirlar.push("uyarilar: yok".to_string());
    } else {
        satirlar.push(format!("uyarilar: {}", bilgi.uyarilar.len()));
        for uyar in &bilgi.uyarilar {
            satirlar.push(format!("  - {uyar}"));
        }
    }
    satirlar.join("\n")
}

/// `Option<bool>` değerini Türkçe metne çevirir.
fn evet_hayir(deger: Option<bool>) -> &'static str {
    match deger {
        Some(true) => "evet",
        Some(false) => "hayir",
        None => "belirlenemedi",
    }
}

/// `plan` alt komutunun insan okunur özetini üretir.
pub fn plan_ozeti(plan: &KirpPlani) -> String {
    let mut satirlar: Vec<String> = Vec::new();
    satirlar.push(format!("kaynak: {}", plan.kaynak));
    satirlar.push(format!("bicim: {}", plan.kaynak_bilgi.bicim));
    satirlar.push(format!("kutu yolu: {}", plan.kutu_yolu));
    satirlar.push(format!(
        "profil: {} ({})",
        plan.profil_ozeti.ad, plan.profil
    ));
    satirlar.push(format!(
        "hedef: {}  en_boy={}  {} kare/sn  {} kbit/s  GOP {}",
        plan.profil_ozeti.cozunurluk.0,
        plan.profil_ozeti.en_boy,
        plan.profil_ozeti.tercih_edilen_kare_hazi,
        plan.profil_ozeti.video_bit_hizi_kbps,
        plan.profil_ozeti.gop_kare
    ));
    satirlar.push(format!(
        "ses: {} Hz / {} kanal / {} kbit/s",
        plan.profil_ozeti.ses_ornekleme_hizi,
        plan.profil_ozeti.ses_kanal,
        plan.profil_ozeti.ses_bit_hizi_kbps
    ));
    satirlar.push(format!(
        "kirpma: {} .. {}  (istenen {})",
        sure(plan.baslangic_sn),
        sure(plan.bitis_sn),
        sure(plan.zamanlama.istenen_sure_sn)
    ));
    satirlar.push(format!(
        "kare: {}..{}  adet={}  hiz={}  gercek={}  fark={}",
        plan.zamanlama.ilk_kare,
        plan.zamanlama.son_kare,
        plan.zamanlama.kare_sayisi,
        plan.zamanlama.kare_hazi,
        sure(plan.zamanlama.gercek_sure_sn),
        sure(plan.zamanlama.sure_farki_sn)
    ));
    satirlar.push(format!(
        "kare harfasi yeterli: {}",
        evet_hayir(Some(plan.zamanlama.kare_harfasi_yeterli))
    ));
    satirlar.push(kadraj_ozeti(&plan.kadraj));
    if plan.uyarilar.is_empty() {
        satirlar.push("uyarilar: yok".to_string());
    } else {
        satirlar.push(format!("uyarilar: {}", plan.uyarilar.len()));
        for uyar in &plan.uyarilar {
            satirlar.push(format!("  - {uyar}"));
        }
    }
    satirlar.join("\n")
}

/// Kadraj hesabının tek satırlık özeti.
fn kadraj_ozeti(kadraj: &Kadraj) -> String {
    let mut parcalar: Vec<String> = Vec::new();
    if let Some(kirpma) = kadraj.kirpma {
        parcalar.push(format!(
            "kirpma={}x{}+{}+{}",
            kirpma.genislik, kirpma.yukseklik, kirpma.x, kirpma.y
        ));
    }
    if let Some(dolgu) = kadraj.dolgu {
        parcalar.push(format!(
            "dolgu={}x{}+{}+{}",
            dolgu.genislik, dolgu.yukseklik, dolgu.x, dolgu.y
        ));
    }
    let govde = if parcalar.is_empty() {
        "olcekleme yok".to_string()
    } else {
        parcalar.join(" ")
    };
    format!(
        "kadraj: {} ({}) kaynak={}x{} hedef={}x{} olcek={:.4}",
        kadraj.kip.aciklama(),
        govde,
        kadraj.kaynak_genislik,
        kadraj.kaynak_yukseklik,
        kadraj.hedef_genislik,
        kadraj.hedef_yukseklik,
        kadraj.olcek
    )
}

/// `profiles` alt komutunun tablo çıktısını üretir.
pub fn profiller_metni(katalog: &ProfilKutusu) -> String {
    let mut satirlar: Vec<String> = Vec::new();
    satirlar.push(format!("katalog surumu: {}", katalog.surum));
    satirlar.push(format!("profil sayisi: {}", katalog.profiller.len()));
    satirlar.push(String::new());
    satirlar.push(format!(
        "{:<14} {:<20} {:<6} {:<11} {:<5} {:<10} {:<5}",
        "KIMLIK", "PLATFORM", "ORAN", "COZUNURLUK", "FPS", "BIT(kbps)", "GOP"
    ));
    for profil in &katalog.profiller {
        satirlar.push(format!(
            "{:<14} {:<20} {:<6} {:<11} {:<5} {:<10} {:<5}",
            profil.kimlik,
            profil.ad,
            profil.en_boy,
            format!("{}x{}", profil.genislik, profil.yukseklik),
            profil.tercih_edilen_kare_hazi,
            profil.video_bit_hizi_kbps,
            profil.gop_kare
        ));
        satirlar.push(format!(
            "             ses: {} Hz / {} kanal / {} kbit/s  azami {}  kaynak: {} (dogrulama: {})",
            profil.ses.ornekleme_hizi,
            profil.ses.kanal,
            profil.ses.bit_hizi_kbps,
            profil.azami_video_bit_hizi_kbps.unwrap_or(0),
            profil.kaynak,
            profil.dogrulanma_tarihi
        ));
    }
    satirlar.join("\n")
}

/// Toplu iş sonucunun özetini üretir.
pub fn toplu_ozet(sonuc: &crate::kuyruk::KuyrukSonucu, kuru_calisma: bool) -> String {
    let mut satirlar: Vec<String> = Vec::new();
    let kip = if kuru_calisma {
        "kuru calisma"
    } else {
        "yazma"
    };
    satirlar.push(format!(
        "kip: {kip}  planlanan={}  hatali={}  atlanan={}",
        sonuc.planlar.len(),
        sonuc.hatalilar.len(),
        sonuc.atlananlar.len()
    ));
    if !sonuc.hatalilar.is_empty() {
        satirlar.push(String::new());
        satirlar.push("hatali dosyalar:".to_string());
        for hata in &sonuc.hatalilar {
            satirlar.push(format!("  [{}] {}: {}", hata.sinif, hata.yol, hata.mesaj));
        }
    }
    if !sonuc.atlananlar.is_empty() {
        satirlar.push(String::new());
        satirlar.push("atlanan dosyalar:".to_string());
        for atlanan in &sonuc.atlananlar {
            satirlar.push(format!("  {} ({})", atlanan.yol, atlanan.sebep));
        }
    }
    satirlar.join("\n")
}

/// Biçim adını kısaltılmış biçimde döndürür (`probe --json` başlığı için).
pub fn bicim_kodu(bicim: Bicim) -> &'static str {
    bicim.kod()
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::medya::arastir;
    use crate::olcu::KareHazi;
    use crate::ornek;
    use crate::profil::ProfilKutusu;
    use crate::test_yardimci::iceriyor;
    use crate::test_yardimci::GeciciDizin;
    use crate::zamanlama::KirpPenceresi;

    /// Sentetik bir MP4 üretip okur.
    ///
    /// `GeciciDizin` etiketi testler arasında paylaşılmamalıdır: aynı etiket
    /// eşzamanlı çalışan iki testte birbirinin dizinini siler.
    fn ornek_bilgi(etiket: &str) -> MedyaBilgisi {
        let gecici = GeciciDizin::yeni(etiket).unwrap();
        let yol = gecici
            .yaz("ornek.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        arastir(&yol).unwrap()
    }

    #[test]
    fn probe_metni_kutu_bilgilerini_icerir() {
        let metin = probe_metni(&ornek_bilgi("clipforge-rapor-probe"));
        assert!(iceriyor(&metin, "bicim: iso-bmff"));
        assert!(iceriyor(&metin, "marka: isom"));
        assert!(iceriyor(&metin, "sure: 20.000 sn"));
        assert!(iceriyor(&metin, "parca sayisi: 2 (video 1, ses 1)"));
        assert!(iceriyor(&metin, "faststart: evet"));
        assert!(iceriyor(&metin, "cozunurluk=1920x1080"));
        assert!(iceriyor(&metin, "en_boy=16:9"));
        assert!(iceriyor(&metin, "kare_hizi=30 (30.000)"));
        assert!(iceriyor(&metin, "kare_sayisi=600"));
        assert!(iceriyor(&metin, "ornekleme=48000 Hz kanal=2"));
    }

    #[test]
    fn probe_metni_uyarilari_sayar() {
        let gecici = GeciciDizin::yeni("clipforge-rapor-uyari").unwrap();
        let mut uretici = ornek::Mp4Uretici::yeni();
        uretici.faststart(false);
        uretici.kare(300);
        let yol = gecici.yaz("a.mp4", &uretici.uret()).unwrap();
        let metin = probe_metni(&arastir(&yol).unwrap());
        assert!(iceriyor(&metin, "faststart: hayir"));
        assert!(iceriyor(&metin, "uyarilar: 1"));
        assert!(iceriyor(&metin, "faststart"));
    }

    #[test]
    fn plan_ozeti_kirpma_ve_kadraj_ozetini_gosterir() {
        let bilgi = ornek_bilgi("clipforge-rapor-plan");
        let profil = ProfilKutusu::gomulu()
            .unwrap()
            .ara("reels")
            .unwrap()
            .clone();
        let pencere = KirpPenceresi::yeni(1.0, 5.0).unwrap();
        let plan = KirpPlani::olustur(&bilgi, &pencere, &profil).unwrap();
        let metin = plan_ozeti(&plan);
        assert!(iceriyor(&metin, "profil: Instagram Reels (reels)"));
        assert!(iceriyor(&metin, "kutu yolu: moov/trak[0]/mdia/minf/stbl"));
        assert!(iceriyor(&metin, "kirpma: 1.000 sn .. 5.000 sn"));
        assert!(iceriyor(&metin, "kare: 30..149"));
        assert!(iceriyor(&metin, "kare harfasi yeterli: evet"));
        assert!(iceriyor(&metin, "kadraj: kirpma"));
    }

    #[test]
    fn plan_ozeti_dolgulu_kadraji_gosterir() {
        let gecici = GeciciDizin::yeni("clipforge-rapor-dolgu").unwrap();
        let yol = gecici
            .yaz("kare.mp4", &ornek::mp4_ornegi(1080, 1080, 300, 30))
            .unwrap();
        let bilgi = arastir(&yol).unwrap();
        let profil = ProfilKutusu::gomulu()
            .unwrap()
            .ara("tiktok")
            .unwrap()
            .clone();
        let pencere = KirpPenceresi::yeni(0.0, 4.0).unwrap();
        let plan = KirpPlani::olustur(&bilgi, &pencere, &profil).unwrap();
        let metin = plan_ozeti(&plan);
        assert!(iceriyor(&metin, "kadraj: kenar dolgusu"));
        assert!(iceriyor(&metin, "dolgu=0x840+0+420"));
        assert!(iceriyor(&metin, "olcek=1.0000"));
    }

    #[test]
    fn profiller_metni_tum_profilleri_listeler() {
        let katalog = ProfilKutusu::gomulu().unwrap();
        let metin = profiller_metni(&katalog);
        assert!(iceriyor(&metin, "katalog surumu: 1"));
        assert!(iceriyor(&metin, "profil sayisi: 6"));
        for kimlik in katalog.kimlikler() {
            assert!(iceriyor(&metin, &kimlik), "tabloda {kimlik} yok");
        }
        assert!(iceriyor(&metin, "reels"));
        assert!(iceriyor(&metin, "9:16"));
        assert!(iceriyor(&metin, "kaynak: https://"));
    }

    #[test]
    fn toplu_ozet_hata_ve_atlananlari_sayar() {
        let gecici = GeciciDizin::yeni("clipforge-rapor-toplu").unwrap();
        gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
            .unwrap();
        gecici.yaz("bozuk.mp4", b"video degil").unwrap();
        gecici.yaz("notlar.txt", b"x").unwrap();
        let filtre = crate::kuyruk::UzantıFiltresi::yeni(&[]);
        let toplama = crate::kuyruk::topla(gecici.yol(), &filtre).unwrap();
        let katalog = ProfilKutusu::gomulu().unwrap();
        let sonuc =
            crate::kuyruk::planla(&toplama.girdiler, &katalog, "reels", None, None).unwrap();
        let mut sonuc = sonuc;
        sonuc.atlananlar.extend(toplama.atlananlar);
        let metin = toplu_ozet(&sonuc, true);
        assert!(iceriyor(
            &metin,
            "kip: kuru calisma  planlanan=1  hatali=1  atlanan=1"
        ));
        assert!(iceriyor(&metin, "hatali dosyalar:"));
        assert!(iceriyor(&metin, "atlanan dosyalar:"));
        assert!(iceriyor(&metin, "uzanti filtresi"));
    }

    #[test]
    fn bos_durumlarda_uyari_yok_yazilir() {
        let bilgi = ornek_bilgi("clipforge-rapor-bos");
        let metin = probe_metni(&bilgi);
        assert!(iceriyor(&metin, "uyarilar: yok"));
        let bos = crate::kuyruk::KuyrukSonucu::default();
        let ozet = toplu_ozet(&bos, false);
        assert!(
            iceriyor(&ozet, "kip: yazma  planlanan=0  hatali=0  atlanan=0"),
            "{ozet}"
        );
    }

    #[test]
    fn evet_hayir_kararlari_uc_degerlidir() {
        assert_eq!(evet_hayir(Some(true)), "evet");
        assert_eq!(evet_hayir(Some(false)), "hayir");
        assert_eq!(evet_hayir(None), "belirlenemedi");
    }

    #[test]
    fn bicim_kodu_ve_sure_bicimlendirme() {
        assert_eq!(bicim_kodu(Bicim::Matroska), "matroska");
        assert_eq!(sure(1.2345), "1.234 sn");
        assert_eq!(
            fps(&KareHazi::ayrıştir("30000/1001").unwrap()),
            "30000/1001 (29.970)"
        );
        assert_eq!(oran(None), "-");
    }
}
