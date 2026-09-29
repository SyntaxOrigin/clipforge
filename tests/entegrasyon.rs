//! ClipForge için uçtan uca (entegrasyon) testler.
//!
//! Bu dosya yalnızca `clipforge` **kütüphanesinin** genel API'sini kullanır;
//! `src/` içindeki hiçbir `pub(crate)` ya da gizli ayrıntıya erişmez. Amaç,
//! alt komutların gerçek dosya sistemi üzerinde birlikte çalıştığını doğrulamaktır.
//!
//! Test verisi, `clipforge::ornek` modülünün ürettiği sentetik kapsüllerdir.
//! Bu kapsüller **oynatılabilir video değildir**; yalnızca ayrıştırıcının okuduğu
//! kutu yapısını taşır. Gerçek medya dosyası gerektiren bir test yoktur.

mod yardimci;

use std::path::Path;

use yardimci::{iceriyor, GeciciDizin};

use clipforge::cli::{calistir, Secenek};
use clipforge::hata::ClipForgeHata;
use clipforge::kuyruk::UzantıFiltresi;
use clipforge::medya::arastir;
use clipforge::ornek;
use clipforge::plan::KirpPlani;
use clipforge::profil::ProfilKutusu;
use clipforge::zamanlama::KirpPenceresi;

use clap::Parser;

/// Sentetik MP4 üretip geçici klasöre yazar.
fn mp4_yaz(gecici: &GeciciDizin, ad: &str, genislik: u32, yukseklik: u32, kare: u64) -> String {
    let yol = gecici
        .yaz(ad, &ornek::mp4_ornegi(genislik, yukseklik, kare, 30))
        .expect("ornek yazilamadi");
    yol.to_string_lossy().into_owned()
}

/// Komut satırını metinden ayrıştırıp çalıştırır.
fn calistir_metinden(satirlar: &[&str]) -> Result<String, ClipForgeHata> {
    let secenek = Secenek::try_parse_from(satirlar).expect("argumanlar ayristirilamadi");
    calistir(&secenek).map(|cikti| cikti.metin)
}

#[test]
fn probe_mp4_kutu_hierarsisini_raporlar() {
    let gecici = GeciciDizin::yeni("clipforge-ent-probe-mp4").expect("gecici dizin");
    let dosya = mp4_yaz(&gecici, "yatay.mp4", 1920, 1080, 600);
    let metin = calistir_metinden(&["clipforge", "probe", &dosya]).expect("probe basarisiz");

    assert!(iceriyor(&metin, "bicim: iso-bmff"), "{metin}");
    assert!(iceriyor(&metin, "marka: isom"), "{metin}");
    assert!(iceriyor(&metin, "sure: 20.000 sn"), "{metin}");
    assert!(
        iceriyor(&metin, "parca sayisi: 2 (video 1, ses 1)"),
        "{metin}"
    );
    assert!(iceriyor(&metin, "faststart: evet"), "{metin}");
    assert!(
        iceriyor(&metin, "kutu=moov/trak[0]/mdia/minf/stbl"),
        "{metin}"
    );
    assert!(iceriyor(&metin, "cozunurluk=1920x1080"), "{metin}");
    assert!(iceriyor(&metin, "en_boy=16:9"), "{metin}");
    assert!(iceriyor(&metin, "kare_hizi=30 (30.000)"), "{metin}");
    assert!(iceriyor(&metin, "kare_sayisi=600"), "{metin}");
    assert!(iceriyor(&metin, "ornekleme=48000 Hz kanal=2"), "{metin}");
}

#[test]
fn probe_mkv_dogru_taninir_ve_suresi_okunur() {
    let gecici = GeciciDizin::yeni("clipforge-ent-probe-mkv").expect("gecici dizin");
    let yol = gecici
        .yaz("dikey.mkv", &ornek::mkv_ornegi(1080, 1920, 12.0, 25))
        .expect("ornek yazilamadi");

    // Değerler önce yapısal olarak doğrulanır: metin biçimine bağlı olmamak,
    // çıktı biçimindeki bir değişikliğin testi kırılmaz hâle getirir.
    let bilgi = arastir(&yol).expect("ayristirma basarisiz");
    assert_eq!(bilgi.bicim, clipforge::medya::Bicim::Matroska);
    assert!((bilgi.sure_sn - 12.0).abs() < 1e-9);
    assert_eq!(bilgi.faststart, None);
    let video = bilgi.video().expect("gorsuntu parcasi yok");
    assert_eq!(video.cozunurluk, Some((1080, 1920)));
    assert_eq!(
        video.kare_hazi.map(|h| h.to_string()),
        Some("25".to_string())
    );
    assert_eq!(video.kare_sayisi, Some(300));
    assert_eq!(video.codec, "V_MPEG4/ISO/AVC");
    assert_eq!(bilgi.parcalar[1].ornekleme_hizi, Some(48_000));
    assert_eq!(bilgi.parcalar[1].kanal, Some(2));

    // Ardından insan okunur çıktının kutu yolu doğrulanır.
    let metin = calistir_metinden(&["clipforge", "probe", &yol.to_string_lossy()])
        .expect("probe basarisiz");
    for parca in [
        "bicim: matroska",
        "faststart: belirlenemedi",
        "codec=V_MPEG4/ISO/AVC",
    ] {
        assert!(
            iceriyor(&metin, parca),
            "beklenen parca yok: {parca}\n---\n{metin}\n---"
        );
    }
}

#[test]
fn probe_json_yapilandirilabilir_ve_ayristirilabilir() {
    let gecici = GeciciDizin::yeni("clipforge-ent-probe-json").expect("gecici dizin");
    let dosya = mp4_yaz(&gecici, "a.mp4", 1080, 1080, 300);
    let metin =
        calistir_metinden(&["clipforge", "probe", &dosya, "--json"]).expect("probe basarisiz");
    let deger: serde_json::Value = serde_json::from_str(&metin).expect("gecerli JSON degil");

    assert_eq!(deger["bicim"], "iso-bmff");
    assert_eq!(deger["parcalar"].as_array().map(Vec::len), Some(2));
    assert_eq!(deger["parcalar"][0]["cozunurluk"][0], 1080);
    assert_eq!(
        deger["parcalar"][0]["kutu_yolu"],
        "moov/trak[0]/mdia/minf/stbl"
    );
    assert_eq!(deger["parcalar"][0]["kare_hazi"], "30");
    assert_eq!(deger["parcalar"][1]["tur"], "ses");
    assert_eq!(deger["uyarilar"].as_array().map(Vec::len), Some(0));
}

#[test]
fn plan_16x9_kaynaktan_9_16_hedefe_kirpma_uretir() {
    let gecici = GeciciDizin::yeni("clipforge-ent-plan-kirpma").expect("gecici dizin");
    // 3840x2160 -> reels 1080x1920: 2160*9/16 = 1215 tam sayidir, dolgu gerekmez.
    let dosya = mp4_yaz(&gecici, "4k.mp4", 3840, 2160, 600);
    let metin = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "reels",
        "--baslangic",
        "1",
        "--bitis",
        "5",
        "--json",
    ])
    .expect("plan basarisiz");
    let deger: serde_json::Value = serde_json::from_str(&metin).expect("gecerli JSON degil");

    assert_eq!(deger["profil"], "reels");
    assert_eq!(deger["kaynak_bilgi"]["cozunurluk"][0], 3840);
    assert_eq!(deger["profil_ozeti"]["cozunurluk"][0], 1080);
    assert_eq!(deger["kadraj"]["kip"], "kirp");
    assert_eq!(deger["kadraj"]["kirpma"]["genislik"], 1215);
    assert_eq!(deger["kadraj"]["kirpma"]["yukseklik"], 2160);
    assert_eq!(deger["kadraj"]["kirpma"]["x"], 1312);
    assert!(deger["kadraj"]["dolgu"].is_null());
    assert_eq!(deger["zamanlama"]["ilk_kare"], 30);
    assert_eq!(deger["zamanlama"]["son_kare"], 149);
    assert_eq!(deger["zamanlama"]["kare_sayisi"], 120);
    assert_eq!(deger["zamanlama"]["kare_harfasi_yeterli"], true);
}

#[test]
fn plan_1x1_kaynaktan_9_16_hedefe_dolgu_uretir() {
    let gecici = GeciciDizin::yeni("clipforge-ent-plan-dolgu").expect("gecici dizin");
    // 1080x1080 reels hedefine sigar: kirpma yapilmaz, dikeyde dolgu kalir.
    let dosya = mp4_yaz(&gecici, "kare.mp4", 1080, 1080, 300);
    let metin = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "reels",
        "--bitis",
        "4",
        "--json",
    ])
    .expect("plan basarisiz");
    let deger: serde_json::Value = serde_json::from_str(&metin).expect("gecerli JSON degil");

    assert_eq!(deger["kadraj"]["kip"], "dolgu");
    assert!(deger["kadraj"]["kirpma"].is_null());
    assert_eq!(deger["kadraj"]["dolgu"]["genislik"], 0);
    assert_eq!(deger["kadraj"]["dolgu"]["yukseklik"], 840);
    assert_eq!(deger["kadraj"]["dolgu"]["y"], 420);
    assert_eq!(deger["kadraj"]["olcek"], 1.0);
    assert_eq!(deger["zamanlama"]["toplam_kare"], 300);
}

#[test]
fn plan_dry_run_kipi_dosya_yazmaz() {
    let gecici = GeciciDizin::yeni("clipforge-ent-plan-kuru").expect("gecici dizin");
    let dosya = mp4_yaz(&gecici, "a.mp4", 1920, 1080, 600);
    let cikti_yolu = gecici.yol().join("plan.json");
    let metin = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "shorts",
        "--bitis",
        "6",
        "--cikti",
        &cikti_yolu.to_string_lossy(),
        "--dry-run",
    ])
    .expect("plan basarisiz");

    assert!(
        iceriyor(&metin, "profil: YouTube Shorts (shorts)"),
        "{metin}"
    );
    assert!(iceriyor(&metin, "--dry-run kullanildi"), "{metin}");
    assert!(!cikti_yolu.exists(), "kuru calisamada dosya yazilmamali");
}

#[test]
fn plan_dosyasi_yazilir_ve_yeniden_okunur() {
    let gecici = GeciciDizin::yeni("clipforge-ent-plan-belge").expect("gecici dizin");
    let dosya = mp4_yaz(&gecici, "a.mp4", 1920, 1080, 600);
    let cikti_yolu = gecici.yol().join("plan.json");
    let _ = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "x-video",
        "--baslangic",
        "0.5",
        "--bitis",
        "3.25",
        "--cikti",
        &cikti_yolu.to_string_lossy(),
    ])
    .expect("plan basarisiz");

    let metin = std::fs::read_to_string(&cikti_yolu).expect("plan dosyasi okunamadi");
    assert!(metin.ends_with('\n'), "plan dosyasi satir sonu ile bitmeli");
    let plan = KirpPlani::ayristir(&metin).expect("plan JSON olarak cozulemedi");

    assert_eq!(plan.profil, "x-video");
    assert_eq!(plan.baslangic_sn, 0.5);
    assert_eq!(plan.bitis_sn, 3.25);
    assert_eq!(plan.kutu_yolu, "moov/trak[0]/mdia/minf/stbl");
    assert_eq!(plan.profil_ozeti.cozunurluk, (1280, 720));
    assert_eq!(plan.zamanlama.kare_sayisi, 83);
    assert!(plan.zamanlama.kare_harfasi_yeterli);
    // Kartın zorunlu kıldığı üst düzey alanlar belge içinde yer alır.
    for alan in [
        "kaynak",
        "baslangic_sn",
        "bitis_sn",
        "profil",
        "kutu_yolu",
        "uyarilar",
    ] {
        assert!(
            metin.contains(&format!("\"{alan}\"")),
            "planda {alan} alani yok"
        );
    }
}

#[test]
fn plan_ayristirilamayan_girdide_acik_hata_uretir() {
    let gecici = GeciciDizin::yeni("clipforge-ent-plan-bozuk").expect("gecici dizin");
    let yol = gecici
        .yaz("bozuk.mp4", b"bu bir video degil")
        .expect("yazilamadi");
    let hata = calistir_metinden(&[
        "clipforge",
        "plan",
        &yol.to_string_lossy(),
        "--profil",
        "reels",
    ])
    .expect_err("bozuk dosya planlanmamali");
    match hata {
        ClipForgeHata::BilinmeyenKapsul { sebep, .. } => {
            assert!(iceriyor(&sebep, "imza taninmadi"), "{sebep}");
        }
        diger => panic!("beklenen BilinmeyenKapsul, gelen {diger:?}"),
    }
}

#[test]
fn plan_kayit_disi_araligi_ve_negatif_baslangici_reddeder() {
    let gecici = GeciciDizin::yeni("clipforge-ent-plan-sinir").expect("gecici dizin");
    let dosya = mp4_yaz(&gecici, "a.mp4", 1920, 1080, 600);

    let dis_da = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "reels",
        "--bitis",
        "99",
    ])
    .expect_err("kaynak suresini asan aralik reddedilmeli");
    assert!(matches!(dis_da, ClipForgeHata::KaynakAsildi { .. }));

    let negatif = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "reels",
        "--baslangic=-1",
        "--bitis",
        "5",
    ])
    .expect_err("negatif baslangic reddedilmeli");
    match negatif {
        ClipForgeHata::AralikGecersiz { ayrinti, .. } => assert!(iceriyor(&ayrinti, "negatif")),
        diger => panic!("beklenen AralikGecersiz, gelen {diger:?}"),
    }

    let sifir = calistir_metinden(&[
        "clipforge",
        "plan",
        &dosya,
        "--profil",
        "reels",
        "--bitis",
        "0",
    ])
    .expect_err("sifir uzunluklu aralik reddedilmeli");
    assert!(matches!(sifir, ClipForgeHata::AralikGecersiz { .. }));
}

#[test]
fn profiles_alt_komutu_gomulu_katalogu_ve_ozel_katalogu_listeler() {
    let metin = calistir_metinden(&["clipforge", "profiles"]).expect("profiles basarisiz");
    assert!(iceriyor(&metin, "katalog surumu: 1"), "{metin}");
    assert!(iceriyor(&metin, "profil sayisi: 6"), "{metin}");
    for kimlik in [
        "reels",
        "tiktok",
        "shorts",
        "x-video",
        "kare-akis",
        "youtube-16x9",
    ] {
        assert!(metin.contains(kimlik), "tabloda {kimlik} yok");
    }

    // Özel katalog: gömülü kataloğu diske yazıp doğrulanmış hâliyle geri yükle.
    let gecici = GeciciDizin::yeni("clipforge-ent-profiller-ozel").expect("gecici dizin");
    let katalog = ProfilKutusu::gomulu().expect("gomulu katalog");
    let katalog_metin = serde_json::to_string_pretty(&katalog).expect("katalog serilestirilemedi");
    let yol = gecici
        .yaz("profiller.json", katalog_metin.as_bytes())
        .expect("katalog yazilamadi");
    let metin = calistir_metinden(&["clipforge", "profiles", "--config", &yol.to_string_lossy()])
        .expect("ozel katalog basarisiz");
    assert!(iceriyor(&metin, "profil sayisi: 6"), "{metin}");

    // Bozuk katalog açık hata verir.
    let bozuk = gecici
        .yaz("bozuk.json", b"{ bu json degil")
        .expect("yazilamadi");
    let hata = calistir_metinden(&[
        "clipforge",
        "profiles",
        "--config",
        &bozuk.to_string_lossy(),
    ])
    .expect_err("bozuk katalog reddedilmeli");
    assert!(matches!(hata, ClipForgeHata::ProfilDosyasiHatali { .. }));
}

#[test]
fn batch_klasoru_tarar_bozuk_dosyayi_atlar_ve_others_isler() {
    let gecici = GeciciDizin::yeni("clipforge-ent-batch").expect("gecici dizin");
    let _iyi = gecici
        .yaz("iyi.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
        .expect("yazilamadi");
    let _ = gecici
        .yaz("bozuk.mp4", b"bu bir mp4 degil")
        .expect("yazilamadi");
    let _ = gecici
        .yaz("moovsuz.mp4", &ornek::mp4_moovsuz())
        .expect("yazilamadi");
    let _ = gecici.yaz("notlar.txt", b"metin").expect("yazilamadi");
    let _ = gecici
        .yaz("alt/ic.mkv", &ornek::mkv_ornegi(1920, 1080, 8.0, 25))
        .expect("yazilamadi");

    let metin = calistir_metinden(&[
        "clipforge",
        "batch",
        &gecici.yol().to_string_lossy(),
        "--profil",
        "reels",
    ])
    .expect("batch basarisiz");

    assert!(
        iceriyor(&metin, "planlanan=2  hatali=2  atlanan=1"),
        "{metin}"
    );
    assert!(iceriyor(&metin, "hatali dosyalar:"), "{metin}");
    assert!(iceriyor(&metin, "imza taninmadi"), "{metin}");
    assert!(iceriyor(&metin, "moov"), "{metin}");
    assert!(iceriyor(&metin, "notlar.txt (uzanti filtresi)"), "{metin}");
}

#[test]
fn batch_uzanti_filtresi_ve_dry_run_kipi() {
    let gecici = GeciciDizin::yeni("clipforge-ent-batch-filtre").expect("gecici dizin");
    let _ = gecici
        .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
        .expect("yazilamadi");
    let _ = gecici
        .yaz("b.mkv", &ornek::mkv_ornegi(1920, 1080, 8.0, 25))
        .expect("yazilamadi");

    let metin = calistir_metinden(&[
        "clipforge",
        "batch",
        &gecici.yol().to_string_lossy(),
        "--profil",
        "reels",
        "--uzanti",
        "mkv",
        "--dry-run",
    ])
    .expect("batch basarisiz");
    assert!(iceriyor(&metin, "kip: kuru calisma"), "{metin}");
    assert!(
        iceriyor(&metin, "planlanan=1  hatali=0  atlanan=1"),
        "{metin}"
    );
    assert_eq!(
        std::fs::read_dir(gecici.yol())
            .expect("dizin okunamadi")
            .count(),
        2,
        "kuru calisamada yeni dosya olusmamali"
    );
}

#[test]
fn batch_belgesi_yazilir_ve_kuyruk_ozeti_uyusur() {
    let gecici = GeciciDizin::yeni("clipforge-ent-batch-belge").expect("gecici dizin");
    let _ = gecici
        .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
        .expect("yazilamadi");
    let _ = gecici.yaz("bozuk.mp4", b"mp4 degil").expect("yazilamadi");
    let belge_yolu = gecici.yol().join("kuyruk.json");
    let _ = calistir_metinden(&[
        "clipforge",
        "batch",
        &gecici.yol().to_string_lossy(),
        "--profil",
        "kare-akis",
        "-o",
        &belge_yolu.to_string_lossy(),
    ])
    .expect("batch basarisiz");

    let metin = std::fs::read_to_string(&belge_yolu).expect("belge okunamadi");
    let deger: serde_json::Value = serde_json::from_str(&metin).expect("gecerli JSON degil");
    assert_eq!(deger["surum"], 1);
    assert_eq!(deger["profil"], "kare-akis");
    assert_eq!(deger["planlar"].as_array().map(Vec::len), Some(1));
    assert_eq!(deger["hatalilar"].as_array().map(Vec::len), Some(1));
    assert_eq!(deger["hatalilar"][0]["sinif"], "bicim");
    assert_eq!(deger["planlar"][0]["profil_ozeti"]["cozunurluk"][1], 1080);
}

#[test]
fn batch_tek_uclu_araligi_kaynak_suresine_tamamlar() {
    let gecici = GeciciDizin::yeni("clipforge-ent-batch-tek").expect("gecici dizin");
    let _ = gecici
        .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
        .expect("yazilamadi");
    let _ = gecici
        .yaz("b.mp4", &ornek::mp4_ornegi(1080, 1080, 150, 30))
        .expect("yazilamadi");
    let metin = calistir_metinden(&[
        "clipforge",
        "batch",
        &gecici.yol().to_string_lossy(),
        "--profil",
        "reels",
        "--baslangic",
        "1",
        "--json",
    ])
    .expect("batch basarisiz");
    let deger: serde_json::Value = serde_json::from_str(&metin).expect("gecerli JSON degil");
    let planlar = deger["planlar"].as_array().expect("planlar dizisi");
    assert_eq!(planlar.len(), 2);
    let sureler: Vec<f64> = planlar
        .iter()
        .map(|p| p["zamanlama"]["gercek_sure_sn"].as_f64().unwrap_or(-1.0))
        .collect();
    // 300 karelik dosya: 1..10 sn -> 9 sn; 150 karelik: 1..5 sn -> 4 sn.
    assert!(sureler.contains(&9.0), "{sureler:?}");
    assert!(sureler.contains(&4.0), "{sureler:?}");
}

#[test]
fn kuyruk_gzinin_tumu_ozellestirilmis_ayarlarla_calisir() {
    let gecici = GeciciDizin::yeni("clipforge-ent-kuyruk").expect("gecici dizin");
    let _ = gecici
        .yaz("a.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
        .expect("yazilamadi");
    let _ = gecici
        .yaz("b.mp4", &ornek::mp4_ornegi(1920, 1080, 300, 30))
        .expect("yazilamadi");

    let filtre = UzantıFiltresi::yeni(&[]);
    let katalog = ProfilKutusu::gomulu().expect("gomulu katalog");
    let toplama = clipforge::kuyruk::topla(gecici.yol(), &filtre).expect("gezinme basarisiz");
    assert_eq!(toplama.girdiler.len(), 2);
    assert!(toplama.atlananlar.is_empty());

    let sonuc = clipforge::kuyruk::planla(&toplama.girdiler, &katalog, "reels", None, None)
        .expect("planlama basarisiz");
    assert_eq!(sonuc.planlar.len(), 2);
    assert!(sonuc.hatalilar.is_empty());
    // Aynı klasör iki kez gezildiğinde sıra aynıdır (belirlenimcilik).
    let ikinci = clipforge::kuyruk::topla(gecici.yol(), &filtre).expect("gezinme basarisiz");
    assert_eq!(toplama.girdiler, ikinci.girdiler);
}

#[test]
fn kesirli_kare_hizi_korunur_ve_kayma_olmaz() {
    let gecici = GeciciDizin::yeni("clipforge-ent-ntsc").expect("gecici dizin");
    let mut uretici = ornek::Mp4Uretici::yeni();
    uretici
        .cozunurluk(1920, 1080)
        .kare_hizi(clipforge::olcu::KareHazi::ayrıştir("30000/1001").expect("gecerli kare hizi"))
        .kare(3_000);
    let yol = gecici.yaz("ntsc.mp4", &uretici.uret()).expect("yazilamadi");

    let bilgi = arastir(&yol).expect("ayristirma basarisiz");
    let video = bilgi.video().expect("gorsuntu parcasi yok");
    assert_eq!(
        video.kare_hazi.map(|h| h.to_string()),
        Some("30000/1001".to_string()),
        "kare hizi kesir olarak korunmali"
    );

    // Tam kare sayısına denk gelen bir aralıkta kare kayması olmaz.
    let hiz = video.kare_hazi.expect("kare hizi yok");
    let pencere = KirpPenceresi::yeni(0.0, hiz.kare_zamani(600)).expect("gecerli pencere");
    let zamanlama =
        clipforge::zamanlama::Zamanlama::hesapla(&pencere, bilgi.sure_sn, video.kare_hazi)
            .expect("zamanlama basarisiz");
    assert_eq!(zamanlama.ilk_kare, 0);
    assert_eq!(zamanlama.kare_sayisi, 600);
    assert!(zamanlama.sure_farki_sn().abs() < 1e-6);
}

#[test]
fn tum_gomulu_profiller_ayni_kutu_yoluyla_planlanir() {
    let gecici = GeciciDizin::yeni("clipforge-ent-profil-dogrulama").expect("gecici dizin");
    let katalog = ProfilKutusu::gomulu().expect("gomulu katalog");
    for profil in &katalog.profiller {
        profil.dogrula().unwrap_or_else(|hata| panic!("{hata}"));
    }
    // Profil doğrulaması kutu yolunu etkilemez: her plan aynı kutu yolunu taşır
    // ve profilin en-boy oranı kendi çözünürlüğüyle tutarlıdır.
    let dosya = mp4_yaz(&gecici, "a.mp4", 1920, 1080, 300);
    for profil in &katalog.profiller {
        let metin = calistir_metinden(&[
            "clipforge",
            "plan",
            &dosya,
            "--profil",
            &profil.kimlik,
            "--json",
        ])
        .unwrap_or_else(|hata| panic!("{} profili planlanamadi: {hata}", profil.kimlik));
        let deger: serde_json::Value = serde_json::from_str(&metin).expect("gecerli JSON degil");
        assert_eq!(deger["kutu_yolu"], "moov/trak[0]/mdia/minf/stbl");
        assert_eq!(deger["profil"], profil.kimlik.as_str());
        assert_eq!(deger["profil_ozeti"]["en_boy"], profil.en_boy.to_string());
        assert_eq!(
            deger["profil_ozeti"]["cozunurluk"][0].as_u64(),
            Some(u64::from(profil.genislik))
        );
    }
}

#[test]
fn yardimci_dizin_bittikten_sonra_temizlenir() {
    let yol;
    {
        let gecici = GeciciDizin::yeni("clipforge-ent-temizlik").expect("gecici dizin");
        let _ = gecici.yaz("a.txt", b"x").expect("yazilamadi");
        yol = gecici.yol().to_path_buf();
        assert!(yol.exists());
    }
    assert!(!yol.exists(), "gecici dizin Drop ile silinmeliydi");
    assert!(!Path::new(&yol).exists());
}
