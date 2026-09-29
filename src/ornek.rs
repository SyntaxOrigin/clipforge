//! Test ve gösterim amaçlı sentetik kapsül üretici.
//!
//! # Ne üretir, ne üretmez
//!
//! Bu modül **oynatılabilir video üretmez**. Ürettiği dosyalar ayrıştırıcının
//! okuması gereken kutu yapısını (`ftyp`/`moov`/`trak`/… ve `EBML`/`Segment`/…)
//! ve zamanlama alanlarını taşır; `mdat` içeriği anlamsız baytlardan oluşur.
//! Bu yeterlidir: ClipForge yalnızca kapsülü okur, kare verisini açmaz.
//!
//! Amaç iki yönlüdür: birim ve entegrasyon testlerinin gerçek medya dosyasına
//! ihtiyaç duymadan kutu ayrıştırıcısını uç durumlarda sınaması, ve
//! `examples/ornek_dosya.rs` üzerinden README'deki komutların herkes tarafından
//! tekrar çalıştırılabilmesi.

use crate::olcu::KareHazi;

/// ISO BMFF kutusu oluşturur (8 bayt başlık + veri).
fn kutu(tip: &[u8; 4], veri: &[u8]) -> Vec<u8> {
    let mut sonuc = ((veri.len() + 8) as u32).to_be_bytes().to_vec();
    sonuc.extend_from_slice(tip);
    sonuc.extend_from_slice(veri);
    sonuc
}

/// `ftyp` kutusu üretir.
fn ftyp_kutusu(ana_marka: &str, uyumlu: &[&str]) -> Vec<u8> {
    let mut veri = ana_marka.as_bytes()[..4].to_vec();
    veri.extend_from_slice(&0x0000_0200_u32.to_be_bytes());
    for marka in uyumlu {
        veri.extend_from_slice(&marka.as_bytes()[..4]);
    }
    kutu(b"ftyp", &veri)
}

/// `mvhd` kutusu üretir (sürüm 0, gövde 100 bayt).
///
/// Düzen: `version/flags(4) creation(4) modification(4) timescale(4)
/// duration(4) rate(4) volume(2) reserved(2) reserved(8) matrix(36)
/// pre_defined(24) next_track_ID(4)`.
fn mvhd_kutusu(zaman_olcegi: u32, ham_sure: u64) -> Vec<u8> {
    let mut veri = vec![0_u8; 100];
    veri[12..16].copy_from_slice(&zaman_olcegi.to_be_bytes());
    let ham = u32::try_from(ham_sure).unwrap_or(u32::MAX);
    veri[16..20].copy_from_slice(&ham.to_be_bytes());
    kutu(b"mvhd", &veri)
}

/// `tkhd` kutusu üretir (sürüm 0, gövde 84 bayt).
fn tkhd_kutusu(iz: u32, ham_sure: u64, genislik: u32, yukseklik: u32) -> Vec<u8> {
    let mut veri = vec![0_u8; 84];
    veri[12..16].copy_from_slice(&iz.to_be_bytes());
    let ham = u32::try_from(ham_sure).unwrap_or(u32::MAX);
    veri[20..24].copy_from_slice(&ham.to_be_bytes());
    veri[76..80].copy_from_slice(&(genislik << 16).to_be_bytes());
    veri[80..84].copy_from_slice(&(yukseklik << 16).to_be_bytes());
    kutu(b"tkhd", &veri)
}

/// `mdhd` kutusu üretir (sürüm 0, gövde 24 bayt).
fn mdhd_kutusu(zaman_olcegi: u32, ham_sure: u64) -> Vec<u8> {
    let mut veri = vec![0_u8; 24];
    veri[12..16].copy_from_slice(&zaman_olcegi.to_be_bytes());
    let ham = u32::try_from(ham_sure).unwrap_or(u32::MAX);
    veri[16..20].copy_from_slice(&ham.to_be_bytes());
    kutu(b"mdhd", &veri)
}

/// `hdlr` kutusu üretir.
fn hdlr_kutusu(isleyici: &[u8; 4]) -> Vec<u8> {
    let mut veri = vec![0_u8; 12];
    veri[8..12].copy_from_slice(isleyici);
    kutu(b"hdlr", &veri)
}

/// Video örnek girdisi (`avc1`) üretir.
fn stsd_video_kutusu(genislik: u32, yukseklik: u32) -> Vec<u8> {
    let mut giris = vec![0_u8; 86];
    let boyut = (giris.len() as u32).to_be_bytes();
    giris[0..4].copy_from_slice(&boyut);
    giris[4..8].copy_from_slice(b"avc1");
    giris[32..34].copy_from_slice(&(u16::try_from(genislik).unwrap_or(0)).to_be_bytes());
    giris[34..36].copy_from_slice(&(u16::try_from(yukseklik).unwrap_or(0)).to_be_bytes());
    let mut veri = vec![0_u8; 8];
    veri[4..8].copy_from_slice(&1_u32.to_be_bytes());
    veri.extend_from_slice(&giris);
    kutu(b"stsd", &veri)
}

/// Ses örnek girdisi (`mp4a`) üretir.
fn stsd_ses_kutusu(kanal: u16, ornekleme: u32) -> Vec<u8> {
    let mut giris = vec![0_u8; 36];
    let boyut = (giris.len() as u32).to_be_bytes();
    giris[0..4].copy_from_slice(&boyut);
    giris[4..8].copy_from_slice(b"mp4a");
    giris[24..26].copy_from_slice(&kanal.to_be_bytes());
    giris[26..28].copy_from_slice(&16_u16.to_be_bytes());
    giris[32..36].copy_from_slice(&(ornekleme << 16).to_be_bytes());
    let mut veri = vec![0_u8; 8];
    veri[4..8].copy_from_slice(&1_u32.to_be_bytes());
    veri.extend_from_slice(&giris);
    kutu(b"stsd", &veri)
}

/// `stts` kutusu üretir (tek giriş: sabit kare hızı).
fn stts_kutusu(ornek_sayisi: u64, artış: u32) -> Vec<u8> {
    let adet = u32::try_from(ornek_sayisi).unwrap_or(u32::MAX);
    let mut veri = vec![0_u8; 8];
    veri[4..8].copy_from_slice(&1_u32.to_be_bytes());
    veri.extend_from_slice(&adet.to_be_bytes());
    veri.extend_from_slice(&artış.to_be_bytes());
    kutu(b"stts", &veri)
}

/// `stsz` kutusu üretir.
fn stsz_kutusu(ornek_sayisi: u64) -> Vec<u8> {
    let adet = u32::try_from(ornek_sayisi).unwrap_or(u32::MAX);
    let mut veri = vec![0_u8; 12];
    veri[8..12].copy_from_slice(&adet.to_be_bytes());
    kutu(b"stsz", &veri)
}

/// `stss` kutusu üretir: her `gop` karede bir eşzamanlı kare.
fn stss_kutusu(ornek_sayisi: u64, gop: u32) -> Vec<u8> {
    let gop = gop.max(1);
    let adet = (ornek_sayisi.div_ceil(u64::from(gop))) as usize;
    let mut veri = vec![0_u8; 8];
    veri[4..8].copy_from_slice(&u32::try_from(adet).unwrap_or(u32::MAX).to_be_bytes());
    for sira in 0..adet {
        let kare_no = u32::try_from(sira as u64 * u64::from(gop) + 1).unwrap_or(u32::MAX);
        veri.extend_from_slice(&kare_no.to_be_bytes());
    }
    kutu(b"stss", &veri)
}

/// Sentetik MP4/MOV üreticisinin ayarları.
#[derive(Debug, Clone, PartialEq)]
pub struct Mp4Uretici {
    /// Görüntü çözünürlüğü.
    pub genislik: u32,
    /// Görüntü çözünürlüğü.
    pub yukseklik: u32,
    /// Toplam kare sayısı.
    pub kare_sayisi: u64,
    /// Kare hızı (kesirli olabilir, ör. 30000/1001).
    pub kare_hazi: KareHazi,
    /// `tkhd`/`stsd` içine yazılacak çözünürlük (çözünürlük yok testi için 0).
    pub stsd_cozunurluk: Option<(u32, u32)>,
    /// Anahtar kare aralığı.
    pub gop_kare: u32,
    /// Ses parçası eklensin mi.
    pub ses_ekle: bool,
    /// Ses örnekleme hızı.
    pub ses_ornekleme: u32,
    /// Ses kanal sayısı.
    pub ses_kanal: u16,
    /// `moov`, `mdat` kutusundan **önce** gelsin mi (`faststart`).
    pub faststart: bool,
    /// `mvhd`/`mdhd` süresini bu değere zorla (süre sıfır testi için).
    pub sure_zorla: Option<f64>,
    /// `moov` kutusunda ilan edilecek sahte boyut (`None` ise gerçek boyut).
    pub moov_boyut_zorla: Option<u32>,
}

impl Default for Mp4Uretici {
    fn default() -> Self {
        Self {
            genislik: 1920,
            yukseklik: 1080,
            kare_sayisi: 600,
            kare_hazi: KareHazi::tam(30),
            stsd_cozunurluk: None,
            gop_kare: 60,
            ses_ekle: true,
            ses_ornekleme: 48_000,
            ses_kanal: 2,
            faststart: true,
            sure_zorla: None,
            moov_boyut_zorla: None,
        }
    }
}

impl Mp4Uretici {
    /// Varsayılan ayarlarla yeni üretici oluşturur.
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Çözünürlüğü ayarlar.
    pub fn cozunurluk(&mut self, genislik: u32, yukseklik: u32) -> &mut Self {
        self.genislik = genislik;
        self.yukseklik = yukseklik;
        self
    }

    /// Kare sayısını ayarlar.
    pub fn kare(&mut self, adet: u64) -> &mut Self {
        self.kare_sayisi = adet;
        self
    }

    /// Kare hızını ayarlar.
    pub fn kare_hizi(&mut self, hiz: KareHazi) -> &mut Self {
        self.kare_hazi = hiz;
        self
    }

    /// `stsd` içindeki çözünürlüğü geçersiz kılar (`Some((0, 0))`).
    pub fn stsd_cozunurluk(&mut self, cozunurluk: Option<(u32, u32)>) -> &mut Self {
        self.stsd_cozunurluk = cozunurluk;
        self
    }

    /// Ses parçası eklenip eklenmeyeceğini ayarlar.
    pub fn ses(&mut self, ekle: bool) -> &mut Self {
        self.ses_ekle = ekle;
        self
    }

    /// `moov`/`mdat` sırasını `faststart` yapmaz hâle getirir.
    pub fn faststart(&mut self, evet: bool) -> &mut Self {
        self.faststart = evet;
        self
    }

    /// Kapsül süresini verilen değere zorlar.
    pub fn sure_zorla(&mut self, sure_sn: f64) -> &mut Self {
        self.sure_zorla = Some(sure_sn);
        self
    }

    /// `moov` kutusunda ilan edilecek boyutu zorlar (bozuk boyut testi).
    pub fn moov_boyut_zorla(&mut self, boyut: u32) -> &mut Self {
        self.moov_boyut_zorla = Some(boyut);
        self
    }

    /// Üretilen dosyanın kapsül süresi (saniye).
    pub fn sure_sn(&self) -> f64 {
        match self.sure_zorla {
            Some(sure) => sure,
            None => {
                f64::from(self.kare_hazi.payda()) * self.kare_sayisi as f64
                    / f64::from(self.kare_hazi.pay())
            }
        }
    }

    /// Ayarlara göre dosya baytlarını üretir.
    pub fn uret(&self) -> Vec<u8> {
        let kare_artis = self.kare_hazi.payda();
        let zaman_olcegi = self.kare_hazi.pay();
        let ham_sure = match self.sure_zorla {
            Some(sure) => (sure * f64::from(zaman_olcegi)).round() as u64,
            None => self.kare_sayisi * u64::from(kare_artis),
        };

        let (stsd_g, stsd_y) = self
            .stsd_cozunurluk
            .unwrap_or((self.genislik, self.yukseklik));

        // Görüntü parçası
        let mut video_stbl = stsd_video_kutusu(stsd_g, stsd_y);
        video_stbl.extend_from_slice(&stts_kutusu(self.kare_sayisi, kare_artis));
        video_stbl.extend_from_slice(&stsz_kutusu(self.kare_sayisi));
        video_stbl.extend_from_slice(&stss_kutusu(self.kare_sayisi, self.gop_kare));
        let mut video_minf = kutu(b"vmhd", &[0; 12]);
        video_minf.extend_from_slice(&kutu(b"stbl", &video_stbl));
        let mut video_mdia = mdhd_kutusu(zaman_olcegi, ham_sure);
        video_mdia.extend_from_slice(&hdlr_kutusu(b"vide"));
        video_mdia.extend_from_slice(&kutu(b"minf", &video_minf));
        let mut video_govde = tkhd_kutusu(1, ham_sure, self.genislik, self.yukseklik);
        video_govde.extend_from_slice(&kutu(b"mdia", &video_mdia));
        let video_trak = kutu(b"trak", &video_govde);

        // Ses parçası
        let mut ses_parcalari = Vec::new();
        if self.ses_ekle {
            let ses_ornek_sayisi = (self.kare_sayisi as f64 * f64::from(zaman_olcegi)) as u64;
            let mut ses_stbl = stsd_ses_kutusu(self.ses_kanal, self.ses_ornekleme);
            ses_stbl.extend_from_slice(&stts_kutusu(
                ses_ornek_sayisi,
                (zaman_olcegi * 1_024 / self.ses_ornekleme.max(1)).max(1),
            ));
            ses_stbl.extend_from_slice(&stsz_kutusu(ses_ornek_sayisi));
            let mut ses_minf = kutu(b"smhd", &[0; 8]);
            ses_minf.extend_from_slice(&kutu(b"stbl", &ses_stbl));
            let mut ses_mdia = mdhd_kutusu(zaman_olcegi, ham_sure);
            ses_mdia.extend_from_slice(&hdlr_kutusu(b"soun"));
            ses_mdia.extend_from_slice(&kutu(b"minf", &ses_minf));
            let mut ses_govde = tkhd_kutusu(2, ham_sure, 0, 0);
            ses_govde.extend_from_slice(&kutu(b"mdia", &ses_mdia));
            ses_parcalari.extend_from_slice(&kutu(b"trak", &ses_govde));
        }

        let mut moov_govde = mvhd_kutusu(1_000, (self.sure_sn() * 1_000.0).round() as u64);
        moov_govde.extend_from_slice(&video_trak);
        moov_govde.extend_from_slice(&ses_parcalari);
        let mut moov = kutu(b"moov", &moov_govde);
        if let Some(zorlanan) = self.moov_boyut_zorla {
            moov[0..4].copy_from_slice(&zorlanan.to_be_bytes());
        }

        let ftyp = ftyp_kutusu("isom", &["isom", "iso2", "avc1", "mp41"]);
        let mdat = kutu(b"mdat", &[0_u8; 64]);

        let mut sonuc = Vec::with_capacity(ftyp.len() + moov.len() + mdat.len());
        sonuc.extend_from_slice(&ftyp);
        if self.faststart {
            sonuc.extend_from_slice(&moov);
            sonuc.extend_from_slice(&mdat);
        } else {
            sonuc.extend_from_slice(&mdat);
            sonuc.extend_from_slice(&moov);
        }
        sonuc
    }
}

/// `genislik x yukseklik`, `kare_sayisi` ve `fps` değerleriyle hızlıca MP4 üretir.
pub fn mp4_ornegi(genislik: u32, yukseklik: u32, kare_sayisi: u64, fps: u32) -> Vec<u8> {
    let mut uretici = Mp4Uretici::yeni();
    uretici.cozunurluk(genislik, yukseklik);
    uretici.kare(kare_sayisi);
    uretici.kare_hizi(KareHazi::tam(fps));
    uretici.uret()
}

/// `moov` kutusu olmayan, yalnızca `ftyp` + `mdat` içeren bozuk dosyayı üretir.
pub fn mp4_moovsuz() -> Vec<u8> {
    let mut sonuc = ftyp_kutusu("isom", &["isom"]);
    sonuc.extend_from_slice(&kutu(b"mdat", &[0_u8; 32]));
    sonuc
}

/// `moov` kutusu dosya sınırını aşan boyut ilan eden bozuk dosyayı üretir.
pub fn mp4_boyut_bozuk() -> Vec<u8> {
    let mut uretici = Mp4Uretici::yeni();
    uretici.moov_boyut_zorla(50_000_000);
    uretici.uret()
}

/// Süresi sıfır olan MP4 üretir.
pub fn mp4_suresiz() -> Vec<u8> {
    let mut uretici = Mp4Uretici::yeni();
    uretici.sure_zorla(0.0);
    uretici.uret()
}

// --- Matroska üretimi ---------------------------------------------------

/// EBML öğesi (boyut tek baytta) yazar.
fn ebml_oge(veri: &mut Vec<u8>, kimlik: &[u8], icerik: &[u8]) {
    veri.extend_from_slice(kimlik);
    veri.push(0x80 | (icerik.len() as u8));
    veri.extend_from_slice(icerik);
}

/// EBML öğesini çok baytlı boyutla yazar (içerik 127 bayttan büyükse).
fn ebml_oge_genis(veri: &mut Vec<u8>, kimlik: &[u8], icerik: &[u8]) {
    veri.extend_from_slice(kimlik);
    let boyut = icerik.len() as u64;
    if boyut < 0x7f {
        veri.push(0x80 | (boyut as u8));
    } else if boyut < 0x3fff {
        veri.push(0x40 | ((boyut >> 8) as u8));
        veri.push((boyut & 0xff) as u8);
    } else {
        veri.push(0x20 | ((boyut >> 16) as u8));
        veri.push(((boyut >> 8) & 0xff) as u8);
        veri.push((boyut & 0xff) as u8);
    }
    veri.extend_from_slice(icerik);
}

/// Boyutu bilinmeyen öge yazar.
///
/// Matroska'da "boyut bilinmiyor" bir VINT'in tüm değer bitleri 1 yapılarak
/// ifade edilir. Sekiz baytlık biçim (`01 FF ... FF`) gerçek dosyalarda en
/// yaygın olanıdır ve tek baytlık kısaltmaya göre belirsizlik bırakmaz.
fn ebml_bilinmeyen(veri: &mut Vec<u8>, kimlik: &[u8]) {
    veri.extend_from_slice(kimlik);
    veri.push(0x01);
    veri.extend_from_slice(&[0xFF; 7]);
}

/// En kısa büyük uç gösteriminde tam sayı alanı üretir.
fn ebml_tam_sayi(deger: u64) -> Vec<u8> {
    let baytlar = deger.to_be_bytes();
    match baytlar.iter().position(|bayt| *bayt != 0) {
        Some(baslangic) => baytlar[baslangic..].to_vec(),
        None => vec![0],
    }
}

/// Sentetik Matroska/WebM üreticisinin ayarları.
#[derive(Debug, Clone, PartialEq)]
pub struct MkvUretici {
    /// Görüntü çözünürlüğü.
    pub genislik: u32,
    /// Görüntü çözünürlüğü.
    pub yukseklik: u32,
    /// Kare hızı.
    pub kare_hazi: KareHazi,
    /// Kapsül süresi (saniye).
    pub sure_sn: f64,
    /// Ses parçası eklensin mi.
    pub ses_ekle: bool,
    /// `DocType` değeri.
    pub dokuman_tipi: String,
}

impl Default for MkvUretici {
    fn default() -> Self {
        Self {
            genislik: 1920,
            yukseklik: 1080,
            kare_hazi: KareHazi::tam(25),
            sure_sn: 12.0,
            ses_ekle: true,
            dokuman_tipi: "matroska".to_string(),
        }
    }
}

impl MkvUretici {
    /// Varsayılan ayarlarla yeni üretici oluşturur.
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Çözünürlüğü ayarlar.
    pub fn cozunurluk(&mut self, genislik: u32, yukseklik: u32) -> &mut Self {
        self.genislik = genislik;
        self.yukseklik = yukseklik;
        self
    }

    /// Süreyi ayarlar.
    pub fn sure(&mut self, sure_sn: f64) -> &mut Self {
        self.sure_sn = sure_sn;
        self
    }

    /// Ayarlara göre dosya baytlarını üretir.
    pub fn uret(&self) -> Vec<u8> {
        let mut govde = Vec::new();
        ebml_oge(&mut govde, &[0x42, 0x86], &[1]);
        ebml_oge(&mut govde, &[0x42, 0xF7], &[1]);
        ebml_oge(&mut govde, &[0x42, 0x82], self.dokuman_tipi.as_bytes());
        ebml_oge(&mut govde, &[0x42, 0x87], &[4]);

        let mut video = Vec::new();
        ebml_oge(
            &mut video,
            &[0xB0],
            &ebml_tam_sayi(u64::from(self.genislik)),
        );
        ebml_oge(
            &mut video,
            &[0xBA],
            &ebml_tam_sayi(u64::from(self.yukseklik)),
        );
        let kare_suresi_ns = (1_000_000_000.0 * self.kare_hazi.deger().recip()) as u64;
        let mut parca1 = Vec::new();
        ebml_oge(&mut parca1, &[0xD7], &ebml_tam_sayi(1));
        ebml_oge(&mut parca1, &[0x73, 0xC5], &ebml_tam_sayi(1));
        ebml_oge(&mut parca1, &[0x83], &ebml_tam_sayi(1));
        ebml_oge(&mut parca1, &[0x86], b"V_MPEG4/ISO/AVC");
        ebml_oge(
            &mut parca1,
            &[0x23, 0xE3, 0x83],
            &ebml_tam_sayi(kare_suresi_ns),
        );
        ebml_oge(&mut parca1, &[0xE0], &video);

        let mut parcalar = Vec::new();
        ebml_oge(&mut parcalar, &[0xAE], &parca1);
        if self.ses_ekle {
            let mut ses = Vec::new();
            ebml_oge(&mut ses, &[0xB5], &48_000.0_f64.to_be_bytes());
            ebml_oge(&mut ses, &[0x9F], &ebml_tam_sayi(2));
            let mut parca2 = Vec::new();
            ebml_oge(&mut parca2, &[0xD7], &ebml_tam_sayi(2));
            ebml_oge(&mut parca2, &[0x83], &ebml_tam_sayi(2));
            ebml_oge(&mut parca2, &[0x86], b"A_OPUS");
            ebml_oge(&mut parca2, &[0xE1], &ses);
            ebml_oge(&mut parcalar, &[0xAE], &parca2);
        }

        let mut info = Vec::new();
        ebml_oge(&mut info, &[0x2A, 0xD7, 0xB1], &ebml_tam_sayi(1_000_000));
        if self.sure_sn > 0.0 {
            ebml_oge(
                &mut info,
                &[0x44, 0x89],
                &(self.sure_sn * 1_000.0).to_be_bytes(),
            );
        }

        let mut segment = Vec::new();
        ebml_oge(&mut segment, &[0x15, 0x49, 0xA9, 0x66], &info);
        ebml_oge(&mut segment, &[0x16, 0x54, 0xAE, 0x6B], &parcalar);

        let mut sonuc = Vec::new();
        ebml_oge_genis(&mut sonuc, &[0x1A, 0x45, 0xDF, 0xA3], &govde);
        // Segment'in boyutu bilinmiyor: canlı akışlarda yaygın biçimdir.
        ebml_bilinmeyen(&mut sonuc, &[0x18, 0x53, 0x80, 0x67]);
        sonuc.extend_from_slice(&segment);
        sonuc
    }
}

/// `genislik x yukseklik`, süre ve `fps` ile hızlıca Matroska üretir.
pub fn mkv_ornegi(genislik: u32, yukseklik: u32, sure_sn: f64, fps: u32) -> Vec<u8> {
    let mut uretici = MkvUretici::yeni();
    uretici.cozunurluk(genislik, yukseklik);
    uretici.sure(sure_sn);
    uretici.kare_hazi = KareHazi::tam(fps);
    uretici.uret()
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::medya::{arastir, Bicim};
    use crate::test_yardimci::iceriyor;
    use crate::test_yardimci::GeciciDizin;

    /// Üretilen baytları geçici dosyaya yazıp yolunu döndürür.
    fn yaz(gecici: &GeciciDizin, ad: &str, veri: &[u8]) -> std::path::PathBuf {
        gecici.yaz(ad, veri).unwrap()
    }

    #[test]
    fn uretilen_mp4_okunabilir_ve_gecerli() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-mp4").unwrap();
        let yol = yaz(&gecici, "a.mp4", &mp4_ornegi(1920, 1080, 600, 30));
        let bilgi = arastir(&yol).unwrap();
        assert_eq!(bilgi.bicim, Bicim::IsoBmff);
        assert_eq!(bilgi.sure_sn, 20.0);
        assert_eq!(bilgi.marka.as_deref(), Some("isom"));
        assert_eq!(bilgi.faststart, Some(true));
        let video = bilgi.video().unwrap();
        assert_eq!(video.cozunurluk, Some((1920, 1080)));
        assert_eq!(video.codec, "avc1");
        assert_eq!(video.kare_sayisi, Some(600));
        assert_eq!(video.kare_hazi, Some(KareHazi::tam(30)));
        assert_eq!(bilgi.ses_parca_sayisi(), 1);
        let ses = &bilgi.parcalar[1];
        assert_eq!(ses.ornekleme_hizi, Some(48_000));
        assert_eq!(ses.kanal, Some(2));
    }

    #[test]
    fn uretilen_mkv_okunabilir_ve_gecerli() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-mkv").unwrap();
        let yol = yaz(&gecici, "a.mkv", &mkv_ornegi(1920, 1080, 12.0, 25));
        let bilgi = arastir(&yol).unwrap();
        assert_eq!(bilgi.bicim, Bicim::Matroska);
        assert!((bilgi.sure_sn - 12.0).abs() < 1e-9);
        let video = bilgi.video().unwrap();
        assert_eq!(video.cozunurluk, Some((1920, 1080)));
        assert_eq!(video.codec, "V_MPEG4/ISO/AVC");
        assert_eq!(video.kare_hazi, Some(KareHazi::tam(25)));
        assert_eq!(bilgi.ses_parca_sayisi(), 1);
        assert_eq!(bilgi.parcalar[1].ornekleme_hizi, Some(48_000));
    }

    #[test]
    fn moovsuz_dosya_hata_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-moovsuz").unwrap();
        let yol = yaz(&gecici, "a.mp4", &mp4_moovsuz());
        let hata = arastir(&yol).unwrap_err();
        assert!(matches!(hata, crate::hata::ClipForgeHata::MoovYok { .. }));
    }

    #[test]
    fn boyut_bozuk_dosya_hata_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-boyut").unwrap();
        let yol = yaz(&gecici, "a.mp4", &mp4_boyut_bozuk());
        let hata = arastir(&yol).unwrap_err();
        assert!(matches!(
            hata,
            crate::hata::ClipForgeHata::GecersizBoyut { .. }
        ));
    }

    #[test]
    fn suresiz_dosya_hata_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-suresiz").unwrap();
        let yol = yaz(&gecici, "a.mp4", &mp4_suresiz());
        let hata = arastir(&yol).unwrap_err();
        assert!(matches!(hata, crate::hata::ClipForgeHata::SureSifir { .. }));
    }

    #[test]
    fn cozunurluksuz_dosya_hata_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-cozunurluk").unwrap();
        let mut uretici = Mp4Uretici::yeni();
        // Hem `stsd` hem `tkhd` sıfır olmalı: çözünürlük başka bir yerden
        // kurtarılamamalıdır.
        uretici.cozunurluk(0, 0);
        uretici.stsd_cozunurluk(Some((0, 0)));
        uretici.kare(600);
        let yol = yaz(&gecici, "a.mp4", &uretici.uret());
        let hata = arastir(&yol).unwrap_err();
        assert!(matches!(
            hata,
            crate::hata::ClipForgeHata::CozunurlukYok { parca: 1 }
        ));
    }

    #[test]
    fn faststart_olmayan_dosya_uyarisi_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-faststart").unwrap();
        let mut uretici = Mp4Uretici::yeni();
        uretici.faststart(false);
        uretici.kare(120);
        let yol = yaz(&gecici, "a.mp4", &uretici.uret());
        let bilgi = arastir(&yol).unwrap();
        assert_eq!(bilgi.faststart, Some(false));
        assert!(bilgi.uyarilar.iter().any(|u| iceriyor(u, "faststart")));
    }

    #[test]
    fn kesirli_kare_hizi_korunur() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-ntsc").unwrap();
        let mut uretici = Mp4Uretici::yeni();
        uretici.kare_hizi(KareHazi::ayrıştir("30000/1001").unwrap());
        uretici.kare(3000);
        let yol = yaz(&gecici, "a.mp4", &uretici.uret());
        let bilgi = arastir(&yol).unwrap();
        assert_eq!(
            bilgi.video().unwrap().kare_hazi,
            Some(KareHazi::ayrıştir("30000/1001").unwrap())
        );
    }

    #[test]
    fn sessesiz_mp4_uretilebilir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-sessiz").unwrap();
        let mut uretici = Mp4Uretici::yeni();
        uretici.ses(false);
        uretici.kare(60);
        let yol = yaz(&gecici, "a.mp4", &uretici.uret());
        let bilgi = arastir(&yol).unwrap();
        assert_eq!(bilgi.ses_parca_sayisi(), 0);
        assert_eq!(bilgi.video_parca_sayisi(), 1);
    }

    #[test]
    fn sure_sn_uretici_ayarlarindan_hesaplanir() {
        let mut uretici = Mp4Uretici::yeni();
        uretici.kare_hizi(KareHazi::tam(30)).kare(600);
        assert!((uretici.sure_sn() - 20.0).abs() < 1e-9);
        uretici.sure_zorla(5.0);
        assert!((uretici.sure_sn() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn kutu_yardimcilari_dogru_boyut_uretir() {
        let k = kutu(b"free", &[1, 2, 3, 4]);
        assert_eq!(k.len(), 12);
        assert_eq!(&k[4..8], b"free");
        assert_eq!(u32::from_be_bytes([k[0], k[1], k[2], k[3]]), 12);
    }

    #[test]
    fn mkv_suresiz_dosya_hata_uretir() {
        let gecici = GeciciDizin::yeni("clipforge-ornek-mkv-suresiz").unwrap();
        let yol = yaz(&gecici, "a.mkv", &mkv_ornegi(1920, 1080, 0.0, 25));
        let hata = arastir(&yol).unwrap_err();
        assert!(matches!(hata, crate::hata::ClipForgeHata::SureSifir { .. }));
    }
}
