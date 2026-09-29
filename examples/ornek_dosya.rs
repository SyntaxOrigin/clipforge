//! Sentetik örnek kapsül dosyaları üretir.
//!
//! # Neden var
//!
//! ClipForge yalnızca kapsul okur, kodlama yapmaz; dolayısıyla gerçek bir video
//! dosyasına ihtiyaç duymadan da tam olarak gösterilebilir. Bu örnek, README'deki
//! `probe` / `plan` / `batch` komutlarının **herkes tarafından tekrar
//! çalıştırılabilmesi** için gerekli olan örnek dosyaları üretir.
//!
//! # Kullanım
//!
//! ```text
//! cargo run --release --example ornek_dosya -- <cikti-klasoru>
//! ```
//!
//! Üretilen dosyalar **oynatılabilir video değildir**: `mdat` içeriği anlamsız
//! baytlardan oluşur, `mdat` dışında ayrıştırıcının okuduğu kutu yapısı
//! (`ftyp`/`moov`/`trak`/… ve `EBML`/`Segment`/…) gerçektir. Amaç, kutu
//! ayrıştırıcısını ve kırp planlayıcıyı göstermektir.

use std::path::Path;
use std::process::ExitCode;

use clipforge::ornek::{MkvUretici, Mp4Uretici};

/// Üretilen dosyaların adı ve içeriğini tanımlayan tablo.
const ORNEKLER: [(&str, &str); 6] = [
    (
        "01-yatay-16x9-4k.mp4",
        "3840x2160, 30 fps, 20 sn, faststart (moov one) — dikey profillere kirpma ornegi",
    ),
    (
        "02-kare-1x1.mp4",
        "1080x1080, 30 fps, 10 sn — dikey profile dolgu ornegi",
    ),
    (
        "03-yatay-16x9-1080p.mp4",
        "1920x1080, 30 fps, 20 sn, faststart yok (moov sonda) — faststart uyarisi ornegi",
    ),
    (
        "04-ntsc-30000-1001.mp4",
        "1920x1080, 30000/1001 fps, 100 sn — kesirli kare hizi ornegi",
    ),
    (
        "05-dikey-mkv.mkv",
        "1080x1920, 25 fps, 12 sn, Matroska kapsulu — dogrudan hedef oran ornegi",
    ),
    (
        "06-bozuk-moovsuz.mp4",
        "yalnizca ftyp + mdat — ayristirilamayan girdi ornegi",
    ),
];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(klasor) = args.next() else {
        eprintln!("kullanim: cargo run --release --example ornek_dosya -- <cikti-klasoru>");
        return ExitCode::from(2);
    };
    let kok = Path::new(&klasor);
    if let Err(hata) = std::fs::create_dir_all(kok) {
        eprintln!("hata: {} olusturulamadi: {hata}", kok.display());
        return ExitCode::from(1);
    }
    uret(kok)
}

/// Tüm örnek dosyaları üretir ve yollarını ekrana basar.
fn uret(kok: &Path) -> ExitCode {
    let mut yatay = Mp4Uretici::yeni();
    yatay.cozunurluk(3840, 2160).kare(600);
    let mut kare = Mp4Uretici::yeni();
    kare.cozunurluk(1080, 1080).kare(300);
    let mut yavas = Mp4Uretici::yeni();
    yavas.cozunurluk(1920, 1080).kare(600).faststart(false);
    let mut ntsc = Mp4Uretici::yeni();
    ntsc.cozunurluk(1920, 1080).kare(3_000);
    ntsc.kare_hazi = clipforge::olcu::KareHazi::ayrıştir("30000/1001").unwrap_or_default();
    let mut dikey = MkvUretici::yeni();
    dikey.cozunurluk(1080, 1920).sure(12.0);

    let govde: [(&str, Vec<u8>); 6] = [
        ("01-yatay-16x9-4k.mp4", yatay.uret()),
        ("02-kare-1x1.mp4", kare.uret()),
        ("03-yatay-16x9-1080p.mp4", yavas.uret()),
        ("04-ntsc-30000-1001.mp4", ntsc.uret()),
        ("05-dikey-mkv.mkv", dikey.uret()),
        ("06-bozuk-moovsuz.mp4", clipforge::ornek::mp4_moovsuz()),
    ];

    for (ad, veri) in &govde {
        let yol = kok.join(ad);
        if let Err(hata) = std::fs::write(&yol, veri) {
            eprintln!("hata: {} yazilamadi: {hata}", yol.display());
            return ExitCode::from(1);
        }
    }
    println!("{} ornek dosya uretildi:", govde.len());
    for (i, (ad, _)) in govde.iter().enumerate() {
        println!("  {}. {}", i + 1, ORNEKLER[i].1);
        println!("     {}", kok.join(ad).display());
    }
    ExitCode::SUCCESS
}
