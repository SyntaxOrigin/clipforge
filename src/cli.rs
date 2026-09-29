//! Komut satırı arayüzü: `probe`, `plan`, `profiles` ve `batch` alt komutları.
//!
//! Bu modül hem `clap` tanımlarını hem de komutların gövdesini taşır ve
//! [`calistir`] işlevi **dış dünyadan bağımsızdır**: girdiyi alır, metni döndürür,
//! diske yazmaz. Böylece tüm alt komutlar entegrasyon testlerinde gerçek
//! dosyalarla çalıştırılabilir.
//!
//! Disk yazma yalnızca `--cikti` verildiğinde ve bu yazma işi
//! [`crate::plan`] ile [`crate::kuyruk`] modüllerindedir.

use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};

use crate::hata::ClipForgeHata;
use crate::kuyruk::{self, UzantıFiltresi};
use crate::medya;
use crate::plan::{KirpPlani, TopluBelge, PLAN_SURUMU, URETIM_ARACI};
use crate::profil::ProfilKutusu;
use crate::rapor;
use crate::zamanlama::KirpPenceresi;

/// `clipforge` komut satırı tanımı.
#[derive(Debug, Parser)]
#[command(
    name = "clipforge",
    version,
    about = "Platform profillerine gore MP4/MKV kapsul ayristirir ve kirp plani uretir.",
    long_about = "ClipForge video kapsulunu (MP4/MKV) saf Rust ile ayristirir, secilen platform \
profiline gore kirp ve kadraj planini JSON olarak uretir. Kodlama yapmaz, ffmpeg/ffprobe \
cagirmaz; yalnizca kapsul yapisini okur ve planlar.",
    disable_help_subcommand = true,
    // Kırp aralığı girişi negatif olabilir; bu durum uygulama tarafından
    // `AralikGecersiz` hatasıyla bildirilir, komut satırı ayrıştırıcısı değil.
    allow_negative_numbers = true
)]
pub struct Secenek {
    #[command(subcommand)]
    /// Çalıştırılacak alt komut.
    pub komut: Komut,
}

/// Alt komutlar.
#[derive(Debug, Subcommand)]
pub enum Komut {
    /// Bir kapsul dosyasini inceler ve kutu bilgilerini raporlar.
    Probe {
        /// İncelenecek dosya.
        #[arg(value_name = "DOSYA")]
        dosya: PathBuf,

        /// Çıktıyı JSON olarak üretir.
        #[arg(long)]
        json: bool,
    },

    /// Bir dosya icin kirp plani uretir.
    ///
    /// Negatif kırp noktaları burada değil, `KirpPenceresi` doğrulamasında
    /// `AralikGecersiz` hatası olarak bildirilir.
    #[command(allow_negative_numbers = true)]
    Plan(PlanArgumlari),

    /// Platform profillerini listeler veya dogrular.
    Profiles {
        /// Çıktıyı JSON olarak üretir.
        #[arg(long)]
        json: bool,

        /// Kullanici tanimli profil katalogu (JSON). Verilmezse gomulu katalog kullanilir.
        #[arg(long, value_name = "DOSYA")]
        config: Option<PathBuf>,
    },

    /// Bir klasordeki dosyalari toplu olarak planlar.
    #[command(allow_negative_numbers = true)]
    Batch(BatchArgumlari),
}

/// `plan` alt komutunun argümanları.
#[derive(Debug, Args)]
pub struct PlanArgumlari {
    /// Planlanacak dosya.
    #[arg(value_name = "DOSYA")]
    pub dosya: PathBuf,

    /// Hedef platform profili (kimlik). `clipforge profiles` ile listelenir.
    #[arg(long, value_name = "KIMLIK")]
    pub profil: String,

    /// Kirp baslangici (saniye). Verilmezse 0.
    #[arg(long, value_name = "SANIYE")]
    pub baslangic: Option<f64>,

    /// Kirp bitisi (saniye). Verilmezse kaynagin tamami kullanilir.
    #[arg(long, value_name = "SANIYE")]
    pub bitis: Option<f64>,

    /// Planin yazilacagi dosya. Verilmezse yalnizca ekrana basilir.
    #[arg(short = 'o', long, value_name = "DOSYA")]
    pub cikti: Option<PathBuf>,

    /// Plani diske yazmadan ekrana basan salt okunur kip.
    #[arg(long)]
    pub dry_run: bool,

    /// Kullanici tanimli profil katalogu (JSON).
    #[arg(long, value_name = "DOSYA")]
    pub config: Option<PathBuf>,

    /// Ozet yerine tam JSON plani basar.
    #[arg(long)]
    pub json: bool,
}

/// `batch` alt komutunun argümanları.
#[derive(Debug, Args)]
pub struct BatchArgumlari {
    /// Taranacak klasor.
    #[arg(value_name = "KLASOR")]
    pub klasor: PathBuf,

    /// Hedef platform profili (kimlik).
    #[arg(long, value_name = "KIMLIK")]
    pub profil: String,

    /// Butun dosyalar icin kirp baslangici (saniye).
    #[arg(long, value_name = "SANIYE")]
    pub baslangic: Option<f64>,

    /// Butun dosyalar icin kirp bitisi (saniye).
    #[arg(long, value_name = "SANIYE")]
    pub bitis: Option<f64>,

    /// Kabul edilecek uzantilar (virgulle ayrilmis). Verilmezse mp4, m4v, mov, mkv, webm.
    #[arg(long, value_name = "LISTE")]
    pub uzanti: Option<String>,

    /// Sonuclari yazilacagi dosya. Verilmezse yalnizca ekrana basilir.
    #[arg(short = 'o', long, value_name = "DOSYA")]
    pub cikti: Option<PathBuf>,

    /// Sonuclari diske yazmadan ekrana basan salt okunur kip.
    #[arg(long)]
    pub dry_run: bool,

    /// Kullanici tanimli profil katalogu (JSON).
    #[arg(long, value_name = "DOSYA")]
    pub config: Option<PathBuf>,

    /// Ozet yerine tam JSON belgesi basar.
    #[arg(long)]
    pub json: bool,
}

/// Bir alt komutun çalıştırılmasıyla üretilen çıktı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cikti {
    /// Süreç çıkış kodu.
    pub kod: i32,
    /// Standart çıktıya yazılacak metin.
    pub metin: String,
}

impl Cikti {
    /// Başarılı çıktı oluşturur.
    pub fn basarili(metin: String) -> Self {
        Self { kod: 0, metin }
    }
}

/// Virgülle ayrılmış uzantı listesini ayrıştırır.
fn uzanti_listesi(ham: &Option<String>) -> Vec<String> {
    match ham {
        Some(metin) => metin
            .split(',')
            .map(str::trim)
            .filter(|parca| !parca.is_empty())
            .map(|parca| parca.trim_start_matches('.').to_lowercase())
            .collect(),
        None => Vec::new(),
    }
}

/// Profil kataloğunu çözer: `--config` verilmişse dosyadan, yoksa gömülüden.
fn katalog_yukle(config: Option<&Path>) -> Result<ProfilKutusu, ClipForgeHata> {
    match config {
        Some(yol) => ProfilKutusu::yukle(yol),
        None => ProfilKutusu::gomulu(),
    }
}

/// Kırp penceresini istenen değerlerden kurar.
fn pencere_kur(
    baslangic: Option<f64>,
    bitis: Option<f64>,
    kaynak_sure_sn: f64,
) -> Result<KirpPenceresi, ClipForgeHata> {
    match (baslangic, bitis) {
        (Some(bas), Some(bit)) => KirpPenceresi::yeni(bas, bit),
        (Some(bas), None) => KirpPenceresi::yeni(bas, kaynak_sure_sn),
        (None, Some(bit)) => KirpPenceresi::yeni(0.0, bit),
        (None, None) => Ok(KirpPenceresi::tam(kaynak_sure_sn)),
    }
}

/// Ayrıştırılmış bir alt komutu çalıştırır ve üretilecek metni döndürür.
///
/// Bu işlev dosya sistemi dışında hiçbir yan etki üretmez; tek yan etki
/// `--cikti` bayrağıyla açıkça istenen JSON yazımıdır.
///
/// # Hatalar
///
/// Geçersiz argüman, okunamayan dosya, geçersiz profil veya yazma hatası
/// durumlarında [`ClipForgeHata`] döner.
pub fn calistir(secenek: &Secenek) -> Result<Cikti, ClipForgeHata> {
    match &secenek.komut {
        Komut::Probe { dosya, json } => probe_calistir(dosya, *json),
        Komut::Plan(plan) => plan_calistir(plan),
        Komut::Profiles { json, config } => profiles_calistir(*json, config.as_deref()),
        Komut::Batch(batch) => batch_calistir(batch),
    }
}

/// `probe` alt komutunun gövdesi.
fn probe_calistir(dosya: &Path, json: bool) -> Result<Cikti, ClipForgeHata> {
    let bilgi = medya::arastir(dosya)?;
    let metin = if json {
        serde_json::to_string_pretty(&bilgi)
            .map_err(|hata| ClipForgeHata::cikti(dosya, std::io::Error::other(hata)))?
    } else {
        rapor::probe_metni(&bilgi)
    };
    Ok(Cikti::basarili(metin))
}

/// `profiles` alt komutunun gövdesi.
fn profiles_calistir(json: bool, config: Option<&Path>) -> Result<Cikti, ClipForgeHata> {
    let katalog = katalog_yukle(config)?;
    let metin = if json {
        serde_json::to_string_pretty(&katalog).map_err(|hata| {
            ClipForgeHata::cikti(Path::new("<katalog>"), std::io::Error::other(hata))
        })?
    } else {
        rapor::profiller_metni(&katalog)
    };
    Ok(Cikti::basarili(metin))
}

/// `plan` alt komutunun gövdesi.
fn plan_calistir(arg: &PlanArgumlari) -> Result<Cikti, ClipForgeHata> {
    let katalog = katalog_yukle(arg.config.as_deref())?;
    let profil = katalog.ara(&arg.profil)?.clone();
    let bilgi = medya::arastir(&arg.dosya)?;
    let pencere = pencere_kur(arg.baslangic, arg.bitis, bilgi.sure_sn)?;
    let plan = KirpPlani::olustur(&bilgi, &pencere, &profil)?;

    // `--dry-run` salt okunur rapor kipidir: plan ne diske yazılır ne de
    // kuyruk durumu değiştirilir; yalnızca ekrana basılır.
    let atlanan_yazim = arg.cikti.is_some() && arg.dry_run;
    if let Some(yol) = arg.cikti.as_ref().filter(|_| !arg.dry_run) {
        plan.yaz(yol)?;
    }
    let govde = if arg.json {
        plan.metin()?
    } else {
        rapor::plan_ozeti(&plan)
    };
    let metin = if atlanan_yazim {
        format!(
            "{govde}\nuyari: --dry-run kullanildi, {} yazilmadi",
            arg.cikti
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        )
    } else {
        govde
    };
    Ok(Cikti::basarili(metin))
}

/// `batch` alt komutunun gövdesi.
fn batch_calistir(arg: &BatchArgumlari) -> Result<Cikti, ClipForgeHata> {
    let katalog = katalog_yukle(arg.config.as_deref())?;
    let profil = katalog.ara(&arg.profil)?.clone();
    let filtre = UzantıFiltresi::yeni(&uzanti_listesi(&arg.uzanti));
    let toplama = kuyruk::topla(&arg.klasor, &filtre)?;

    // Boyut farkı olan dosyalarda ortak bir iki uçlu pencere yanlış kırpma
    // üretirdi; bu yüzden verilen tek uç dosya başında kaynak süresiyle
    // tamamlanır (bkz. KuyrukGirdisi::planla).
    let mut sonuc = kuyruk::planla(
        &toplama.girdiler,
        &katalog,
        &arg.profil,
        arg.baslangic,
        arg.bitis,
    )?;
    sonuc.atlananlar.extend(toplama.atlananlar);
    sonuc.atlananlar.extend(toplama.derinlik_asimlari);

    // Özet, sonuç tüketilmeden önce üretilir: `belge` yapısı sonucu sahiplenir.
    let ozet = rapor::toplu_ozet(&sonuc, arg.dry_run);
    let belge = TopluBelge {
        surum: PLAN_SURUMU,
        uretim_araci: URETIM_ARACI.to_string(),
        profil: profil.kimlik.clone(),
        planlar: sonuc.planlar,
        hatalilar: sonuc.hatalilar,
        atlananlar: sonuc.atlananlar,
    };

    // `--dry-run` salt okunur rapor kipidir: hiçbir belge diske yazılmaz.
    let atlanan_yazim = arg.cikti.is_some() && arg.dry_run;
    if let Some(yol) = arg.cikti.as_ref().filter(|_| !arg.dry_run) {
        let mut metin = serde_json::to_string_pretty(&belge)
            .map_err(|hata| ClipForgeHata::cikti(yol, std::io::Error::other(hata)))?;
        metin.push('\n');
        std::fs::write(yol, metin).map_err(|hata| ClipForgeHata::cikti(yol, hata))?;
    }
    let metin = if arg.json {
        serde_json::to_string_pretty(&belge).map_err(|hata| {
            ClipForgeHata::cikti(Path::new("<belge>"), std::io::Error::other(hata))
        })?
    } else {
        ozet
    };
    let metin = if atlanan_yazim {
        format!(
            "{metin}\nuyari: --dry-run kullanildi, {} yazilmadi",
            arg.cikti
                .as_ref()
                .map_or_else(String::new, |p| p.display().to_string())
        )
    } else {
        metin
    };
    Ok(Cikti::basarili(metin))
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ornek;
    use crate::test_yardimci::iceriyor;
    use crate::test_yardimci::GeciciDizin;

    /// Komut satırını metinden ayrıştırır.
    fn ayrıştir(satirlar: &[&str]) -> Secenek {
        Secenek::try_parse_from(satirlar).unwrap()
    }

    /// Tam bir saniye biçimli metin döndürür.
    fn metin(saniye: f64) -> String {
        format!("{saniye}")
    }

    #[test]
    fn yardimci_alt_komut_adlari_dogru_ayristirilir() {
        let secenek = ayrıştir(&["clipforge", "probe", "a.mp4", "--json"]);
        assert!(matches!(secenek.komut, Komut::Probe { json: true, .. }));
        let secenek = ayrıştir(&["clipforge", "profiles"]);
        assert!(matches!(secenek.komut, Komut::Profiles { .. }));
        // `help` alt komutu bilinçli olarak kapatıldı; yanlış komut reddedilir.
        assert!(Secenek::try_parse_from(["clipforge", "help"]).is_err());
    }

    #[test]
    fn uzanti_listesi_ayristirilir_ve_normalize_edilir() {
        assert_eq!(uzanti_listesi(&None), Vec::<String>::new());
        assert_eq!(
            uzanti_listesi(&Some("mp4, .MKV ,webm".to_string())),
            vec!["mp4".to_string(), "mkv".to_string(), "webm".to_string()]
        );
        assert_eq!(
            uzanti_listesi(&Some(" , ".to_string())),
            Vec::<String>::new()
        );
    }

    #[test]
    fn katalog_yukleme_gomulu_ve_dosyadan_calisir() {
        assert_eq!(katalog_yukle(None).unwrap().profiller.len(), 6);
        let gecici = GeciciDizin::yeni("clipforge-cli-katalog").unwrap();
        let yol = gecici
            .yaz("p.json", crate::profil::GOMULU_KATALOG_JSON.as_bytes())
            .unwrap();
        assert_eq!(katalog_yukle(Some(&yol)).unwrap().profiller.len(), 6);
        let bozuk = gecici.yaz("b.json", b"bozuk").unwrap();
        assert!(katalog_yukle(Some(&bozuk)).is_err());
    }

    #[test]
    fn pencere_kur_eksik_degerleri_kaynak_suresine_tamamlar() {
        assert_eq!(pencere_kur(None, None, 10.0).unwrap().sure_sn(), 10.0);
        assert_eq!(pencere_kur(Some(2.0), None, 10.0).unwrap().sure_sn(), 8.0);
        assert_eq!(pencere_kur(None, Some(4.0), 10.0).unwrap().sure_sn(), 4.0);
        assert_eq!(
            pencere_kur(Some(1.0), Some(2.0), 10.0).unwrap().sure_sn(),
            1.0
        );
        assert!(pencere_kur(Some(20.0), Some(5.0), 10.0).is_err());
        assert!(pencere_kur(Some(-1.0), Some(5.0), 10.0).is_err());
    }

    #[test]
    fn probe_alt_komutu_metin_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-cli-probe").unwrap();
        let yol = gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let cikti = calistir(&ayrıştir(&["clipforge", "probe", yol.to_str().unwrap()])).unwrap();
        assert_eq!(cikti.kod, 0);
        assert!(iceriyor(&cikti.metin, "bicim: iso-bmff"));
        assert!(iceriyor(&cikti.metin, "sure: 20.000 sn"));
    }

    #[test]
    fn probe_json_bayragi_yapilandirilabilir_cikti_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-cli-probe-json").unwrap();
        let yol = gecici
            .yaz("a.mkv", &ornek::mkv_ornegi(1920, 1080, 12.0, 25))
            .unwrap();
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "probe",
            yol.to_str().unwrap(),
            "--json",
        ]))
        .unwrap();
        let cozulen: serde_json::Value = serde_json::from_str(&cikti.metin).unwrap();
        assert_eq!(cozulen["bicim"], "matroska");
        assert_eq!(cozulen["parcalar"][0]["cozunurluk"][0], 1920);
    }

    #[test]
    fn plan_dry_run_kipi_dosya_yazmaz() {
        let gecici = GeciciDizin::yeni("clipforge-cli-plan-kuru").unwrap();
        let yol = gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let cikti_dir = gecici.yol().join("plan.json");
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "plan",
            yol.to_str().unwrap(),
            "--profil",
            "reels",
            "--baslangic",
            "1",
            "--bitis",
            "5",
            "--cikti",
            cikti_dir.to_str().unwrap(),
            "--dry-run",
        ]))
        .unwrap();
        assert!(iceriyor(&cikti.metin, "profil: Instagram Reels (reels)"));
        assert!(iceriyor(&cikti.metin, "--dry-run kullanildi"));
        assert!(!cikti_dir.exists(), "kuru calisamada dosya yazilmamali");
    }

    #[test]
    fn plan_cikti_verilmezse_dosya_yazmaz() {
        let gecici = GeciciDizin::yeni("clipforge-cli-plan-yok").unwrap();
        let yol = gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "plan",
            yol.to_str().unwrap(),
            "--profil",
            "reels",
        ]))
        .unwrap();
        assert!(iceriyor(&cikti.metin, "kirpma: 0.000 sn .. 20.000 sn"));
        assert_eq!(std::fs::read_dir(gecici.yol()).unwrap().count(), 1);
    }

    #[test]
    fn plan_gecersiz_araligi_reddeder() {
        let gecici = GeciciDizin::yeni("clipforge-cli-plan-aralik").unwrap();
        let yol = gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let hata = calistir(&ayrıştir(&[
            "clipforge",
            "plan",
            yol.to_str().unwrap(),
            "--profil",
            "reels",
            "--baslangic",
            "5",
            "--bitis",
            "2",
        ]))
        .unwrap_err();
        assert!(matches!(hata, ClipForgeHata::AralikGecersiz { .. }));
    }

    #[test]
    fn plan_olmayan_profili_reddeder() {
        let gecici = GeciciDizin::yeni("clipforge-cli-plan-profil").unwrap();
        let yol = gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let hata = calistir(&ayrıştir(&[
            "clipforge",
            "plan",
            yol.to_str().unwrap(),
            "--profil",
            "linkedin",
        ]))
        .unwrap_err();
        assert!(matches!(hata, ClipForgeHata::ProfilYok { .. }));
    }

    #[test]
    fn plan_json_bayragi_tam_belge_basar() {
        let gecici = GeciciDizin::yeni("clipforge-cli-plan-json").unwrap();
        let yol = gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 600, 30))
            .unwrap();
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "plan",
            yol.to_str().unwrap(),
            "--profil",
            "shorts",
            "--bitis",
            "4",
            "--json",
        ]))
        .unwrap();
        let cozulen: serde_json::Value = serde_json::from_str(&cikti.metin).unwrap();
        assert_eq!(cozulen["profil"], "shorts");
        assert_eq!(cozulen["kutu_yolu"], "moov/trak[0]/mdia/minf/stbl");
        assert_eq!(cozulen["zamanlama"]["kare_sayisi"], 120);
    }

    #[test]
    fn profiles_alt_komutu_gomulu_katalogu_listeler() {
        let cikti = calistir(&ayrıştir(&["clipforge", "profiles"])).unwrap();
        assert!(iceriyor(&cikti.metin, "profil sayisi: 6"));
        assert!(iceriyor(&cikti.metin, "reels"));
    }

    #[test]
    fn profiles_json_bayragi_ile_katalog_dokumumunu_verir() {
        let cikti = calistir(&ayrıştir(&["clipforge", "profiles", "--json"])).unwrap();
        let cozulen: serde_json::Value = serde_json::from_str(&cikti.metin).unwrap();
        assert_eq!(cozulen["surum"], 1);
        assert_eq!(cozulen["profiller"].as_array().unwrap().len(), 6);
        assert_eq!(cozulen["profiller"][0]["en_boy"], "9:16");
    }

    #[test]
    fn batch_alt_komutu_klasoru_tarar_ve_ozet_basar() {
        let gecici = GeciciDizin::yeni("clipforge-cli-batch").unwrap();
        gecici
            .yaz("iyi.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
            .unwrap();
        gecici.yaz("bozuk.mp4", b"bu mp4 degil").unwrap();
        gecici.yaz("notlar.txt", b"x").unwrap();
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "batch",
            gecici.yol().to_str().unwrap(),
            "--profil",
            "reels",
            "--uzanti",
            "mp4",
        ]))
        .unwrap();
        assert!(iceriyor(&cikti.metin, "planlanan=1  hatali=1  atlanan=1"));
        assert!(iceriyor(&cikti.metin, "notlar.txt"));
    }

    #[test]
    fn batch_dry_run_sonuclari_yazmaz() {
        let gecici = GeciciDizin::yeni("clipforge-cli-batch-kuru").unwrap();
        gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
            .unwrap();
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "batch",
            gecici.yol().to_str().unwrap(),
            "--profil",
            "reels",
            "--dry-run",
        ]))
        .unwrap();
        assert!(iceriyor(&cikti.metin, "kip: kuru calisma"));
        assert_eq!(std::fs::read_dir(gecici.yol()).unwrap().count(), 1);
    }

    #[test]
    fn batch_belge_dosyasina_yazilir() {
        let gecici = GeciciDizin::yeni("clipforge-cli-batch-belge").unwrap();
        gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
            .unwrap();
        gecici.yaz("bozuk.mkv", b"mkv degil").unwrap();
        let belge_yolu = gecici.yol().join("kuyruk.json");
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "batch",
            gecici.yol().to_str().unwrap(),
            "--profil",
            "kare-akis",
            "-o",
            belge_yolu.to_str().unwrap(),
        ]))
        .unwrap();
        assert!(iceriyor(&cikti.metin, "kip: yazma"));
        let metin = std::fs::read_to_string(&belge_yolu).unwrap();
        let belge: TopluBelge = serde_json::from_str(&metin).unwrap();
        assert_eq!(belge.profil, "kare-akis");
        assert_eq!(belge.planlar.len(), 1);
        assert_eq!(belge.hatalilar.len(), 1);
    }

    #[test]
    fn batch_yazilamayan_cikti_dosyasinda_hata_verir() {
        let gecici = GeciciDizin::yeni("clipforge-cli-batch-yok").unwrap();
        gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
            .unwrap();
        let hata = calistir(&ayrıştir(&[
            "clipforge",
            "batch",
            gecici.yol().to_str().unwrap(),
            "--profil",
            "reels",
            "-o",
            gecici
                .yol()
                .join("yok")
                .join("kuyruk.json")
                .to_str()
                .unwrap(),
        ]))
        .unwrap_err();
        assert!(matches!(hata, ClipForgeHata::CiktiHatasi { .. }));
    }

    #[test]
    fn batch_tek_uclu_araligi_kaynak_suresine_tamamlar() {
        let gecici = GeciciDizin::yeni("clipforge-cli-batch-tek").unwrap();
        gecici
            .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
            .unwrap();
        gecici
            .yaz("b.mp4", &ornek::mp4_ornegi(1080, 1080, 150, 30))
            .unwrap();
        let cikti = calistir(&ayrıştir(&[
            "clipforge",
            "batch",
            gecici.yol().to_str().unwrap(),
            "--profil",
            "reels",
            "--baslangic",
            &metin(1.0),
            "--json",
        ]))
        .unwrap();
        let belge: TopluBelge = serde_json::from_str(&cikti.metin).unwrap();
        assert_eq!(belge.planlar.len(), 2);
        // 150 karelik dosyada da bitiş kaynak süredir (5 sn), 300 karelikte 10 sn.
        let sureler: Vec<f64> = belge
            .planlar
            .iter()
            .map(|p| p.zamanlama.gercek_sure_sn)
            .collect();
        assert!(sureler.contains(&4.0), "{sureler:?}");
        assert!(sureler.contains(&9.0), "{sureler:?}");
    }

    #[test]
    fn cikti_basarili_kodu_sifirdir() {
        let cikti = Cikti::basarili("metin".to_string());
        assert_eq!(cikti.kod, 0);
        assert_eq!(cikti.metin, "metin");
    }
}
