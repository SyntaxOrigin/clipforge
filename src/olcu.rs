//! Ölçü tipleri: en-boy oranı ve kare hızı.
//!
//! Bu modül kutu ayrıştırıcısı, profil kataloğu, kadraj hesabı ve kırp
//! planlayıcı tarafından paylaşılır. Bu yüzden nötr bir yerde durur ve hiçbir
//! modüle bağımlılık getirmez.
//!
//! Her iki tip de tam sayı kesri olarak saklanır: kare hızı `29.97` gibi bir
//! `f64` yaklaşık değer olarak değil, tam olarak `30000/1001` olarak tutulur.
//! Böylece NTSC kayıtlarında kare hızı kayması oluşmaz.
//!
//! Her iki tip de JSON'da **metin** olarak görünür (`"9:16"`, `"30000/1001"`);
//! `Serialize`/`Deserialize` uygulamaları bu yüzden elle yazılmıştır.

use serde::{Deserialize, Serialize};

/// Kesirli en-boy oranı (ör. `9:16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnBoy {
    pay: u32,
    payda: u32,
}

impl EnBoy {
    /// `pay/payda` biçiminde yeni oran oluşturur ve en küçük biçime indirir.
    ///
    /// # Hatalar
    ///
    /// `payda` ya da `pay` sıfırsa hata metni döndürür.
    pub fn yeni(pay: u32, payda: u32) -> Result<Self, String> {
        if pay == 0 || payda == 0 {
            return Err(format!("en-boy orani gecersiz: {pay}:{payda}"));
        }
        let bolen = en_buyuk_ortak_bolen(pay, payda);
        Ok(Self {
            pay: pay / bolen,
            payda: payda / bolen,
        })
    }

    /// `genislik x yukseklik` çiftinden oran üretir.
    ///
    /// # Hatalar
    ///
    /// Çözünürlüğün herhangi bir bileşeni sıfırsa hata metni döndürür.
    pub fn cozunurlukten(genislik: u32, yukseklik: u32) -> Result<Self, String> {
        if genislik == 0 || yukseklik == 0 {
            return Err(format!("cozunurluk gecersiz: {genislik}x{yukseklik}"));
        }
        Self::yeni(genislik, yukseklik)
    }

    /// `"9:16"` ya da `"9/16"` biçimindeki metni çözümler.
    ///
    /// # Hatalar
    ///
    /// Beklenen biçimde değilse veya bileşenler sıfırsa hata metni döndürür.
    pub fn ayrıştir(metin: &str) -> Result<Self, String> {
        let parcalar: Vec<&str> = metin.split([':', '/']).map(str::trim).collect();
        if parcalar.len() != 2 {
            return Err(format!("'{metin}' en-boy orani degil (beklenen '9:16')"));
        }
        let pay: u32 = parcalar[0]
            .parse()
            .map_err(|_| format!("'{metin}' icinde sayi olmayan kisim: '{}'", parcalar[0]))?;
        let payda: u32 = parcalar[1]
            .parse()
            .map_err(|_| format!("'{metin}' icinde sayi olmayan kisim: '{}'", parcalar[1]))?;
        Self::yeni(pay, payda)
    }

    /// Oranın payını (genişlik bileşeni) döndürür.
    pub fn pay(self) -> u32 {
        self.pay
    }

    /// Oranın paydasını (yükseklik bileşeni) döndürür.
    pub fn payda(self) -> u32 {
        self.payda
    }

    /// Oranın `f64` değerini döndürür.
    pub fn deger(self) -> f64 {
        f64::from(self.pay) / f64::from(self.payda)
    }

    /// İki oranın belirtilen toleransla aynı olup olmadığını söyler.
    pub fn esit_mi(self, diger: Self, tolerans: f64) -> bool {
        (self.deger() - diger.deger()).abs() <= tolerans
    }

    /// Verilen çözünürlüğün bu orana, tolerans dahilinde uyup uymadığını söyler.
    pub fn cozunurluge_uyuyor_mu(self, genislik: u32, yukseklik: u32) -> bool {
        match Self::cozunurlukten(genislik, yukseklik) {
            Ok(diger) => self.esit_mi(diger, 0.01),
            Err(_) => false,
        }
    }
}

impl std::fmt::Display for EnBoy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.pay, self.payda)
    }
}

/// Kesirli kare hızı (ör. `30000/1001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KareHazi {
    pay: u32,
    payda: u32,
}

impl KareHazi {
    /// Tam sayı kare hızı oluşturur (`payda == 1`).
    ///
    /// `pay` sıfır verilirse sonuç geçersizdir; bu işlev yalnızca sabit
    /// varsayılanlar (30, 60) için kullanılmalıdır. Değişken girdilerde
    /// [`KareHazi::yeni`] kullanılır.
    pub const fn tam(pay: u32) -> Self {
        Self { pay, payda: 1 }
    }

    /// `pay/payda` biçiminde yeni kare hızı oluşturur ve en küçük biçime indirger.
    ///
    /// # Hatalar
    ///
    /// `payda` ya da `pay` sıfırsa hata metni döndürür.
    pub fn yeni(pay: u32, payda: u32) -> Result<Self, String> {
        if pay == 0 || payda == 0 {
            return Err(format!("kare hizi gecersiz: {pay}/{payda}"));
        }
        let bolen = en_buyuk_ortak_bolen(pay, payda);
        Ok(Self {
            pay: pay / bolen,
            payda: payda / bolen,
        })
    }

    /// `f64` bir saniye/kare değerinden kare hızı üretir.
    ///
    /// Bu işlev yalnızca Matroska'nın `DefaultDuration` gibi ondalık alanları
    /// için kullanılır. Değer 1e6 katına ölçeklenip yeniden yorumlanır; bu
    /// yüzden sonuç `30000/1001` gibi bir kesre tam oturmaz, en yakın
    /// altı basamaklı ondalığa indirgenir.
    ///
    /// # Hatalar
    ///
    /// Değer sıfıra yakınsa veya sonlu değilse hata metni döndürür.
    pub fn sabit(fps: f64) -> Result<Self, String> {
        if !fps.is_finite() || fps <= 0.0 {
            return Err(format!("kare hizi gecersiz: {fps}"));
        }
        let yuvarlanmis = (fps * 1_000_000.0).round();
        if !yuvarlanmis.is_finite() || yuvarlanmis < 1.0 || yuvarlanmis > f64::from(u32::MAX) {
            return Err(format!("kare hizi gecersiz: {fps}"));
        }
        Self::yeni(yuvarlanmis as u32, 1_000_000)
    }

    /// `"30000/1001"`, `"30/1"` ya da `"30"` biçimindeki metni çözümler.
    ///
    /// # Hatalar
    ///
    /// Biçim yanlışsa veya bileşenler sıfırsa hata metni döndürür.
    pub fn ayrıştir(metin: &str) -> Result<Self, String> {
        let parcalar: Vec<&str> = metin.split('/').map(str::trim).collect();
        let pay: u32 = match parcalar.first() {
            Some(parca) => parca.parse().map_err(|_| format!("'{metin}' sayi degil"))?,
            None => return Err("bos kare hizi".to_string()),
        };
        let payda: u32 = match parcalar.get(1) {
            Some(parca) => parca.parse().map_err(|_| format!("'{metin}' sayi degil"))?,
            None => 1,
        };
        if parcalar.len() > 2 {
            return Err(format!("'{metin}' kare hizi degil (beklenen '30000/1001')"));
        }
        Self::yeni(pay, payda)
    }

    /// Kare hızının payını döndürür.
    pub fn pay(self) -> u32 {
        self.pay
    }

    /// Kare hızının paydasını döndürür.
    pub fn payda(self) -> u32 {
        self.payda
    }

    /// Saniye başına kare sayısını `f64` olarak döndürür.
    pub fn deger(self) -> f64 {
        f64::from(self.pay) / f64::from(self.payda)
    }

    /// Verilen sürede kaç kare bulunduğunu **aşağı yuvarlayarak** döndürür.
    pub fn kare_sayisi(self, sure_sn: f64) -> u64 {
        if !sure_sn.is_finite() || sure_sn <= 0.0 {
            return 0;
        }
        let ham = sure_sn * self.deger();
        if ham >= u64::MAX as f64 {
            u64::MAX
        } else {
            ham.floor() as u64
        }
    }

    /// Kare numarasının başlangıç zamanını (saniye) döndürür.
    pub fn kare_zamani(self, kare_no: u64) -> f64 {
        kare_no as f64 * f64::from(self.payda) / f64::from(self.pay)
    }

    /// Verilen zamanın kare sınırına **aşağı** yuvarlanmış numarasını döndürür.
    ///
    /// Bu, "kırpma sonrası bu aralık dışında kare kalmaz" kuralının başlangıç
    /// tarafını garanti eder: kare `i` zamanı `i / fps` olduğundan, `i >= t*fps`
    /// koşulunu sağlayan ilk kare aranan karedir.
    pub fn kare_no(self, zaman_sn: f64) -> u64 {
        self.sinir_kare(zaman_sn, YuvarlamaYonu::Asagi)
    }

    /// Verilen zamanı içeren kareyi bulur: tam kare sınırındaki bir zaman
    /// bir sonraki karenin başlangıcı sayılır (yarı açık aralık kuralı).
    ///
    /// Bu, "bitiş anı dışında kare kalmaz" kuralının karşılığıdır.
    pub fn kare_siniri(self, zaman_sn: f64) -> u64 {
        self.sinir_kare(zaman_sn, YuvarlamaYonu::Yukari)
    }

    /// Kare sınırı hesabının ortak gövdesi.
    ///
    /// Kayan nokta yuvarlama artıkları, tam kare sınırındaki bir zamanı bir kare
    /// kaydırabilir (`1.4 * 30.0 = 42.00000000000001`). Bu yüzden tam sayıya
    /// `KARE_EPSILON` kadar yakın değerler yönlendirmeden tam kare sınırına
    /// oturtulur.
    fn sinir_kare(self, zaman_sn: f64, yon: YuvarlamaYonu) -> u64 {
        if !zaman_sn.is_finite() || zaman_sn <= 0.0 {
            return 0;
        }
        let ham = zaman_sn * self.deger();
        if ham >= u64::MAX as f64 {
            return u64::MAX;
        }
        let yuvarlanmis = ham.round();
        let tam_sinir = if (ham - yuvarlanmis).abs() < KARE_EPSILON {
            yuvarlanmis
        } else {
            match yon {
                YuvarlamaYonu::Asagi => ham.floor(),
                YuvarlamaYonu::Yukari => ham.ceil(),
            }
        };
        tam_sinir as u64
    }
}

/// Tam kare sınırına oturtma toleransı (saniye cinsinden değil, kare cinsinden).
const KARE_EPSILON: f64 = 1e-6;

/// Kare sınırı hesabında kullanılan yuvarlama yönü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum YuvarlamaYonu {
    /// Sınırdan önceki kare.
    Asagi,
    /// Sınırdan sonraki kare.
    Yukari,
}

impl std::fmt::Display for KareHazi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.payda == 1 {
            write!(f, "{}", self.pay)
        } else {
            write!(f, "{}/{}", self.pay, self.payda)
        }
    }
}

impl Default for KareHazi {
    /// 30 kare/s: video standartı ve platform profillerinin varsayılanı.
    fn default() -> Self {
        Self::tam(30)
    }
}

/// İki tam sayının en büyük ortak bölenini döndürür (Öklid algoritması).
fn en_buyuk_ortak_bolen(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let kalan = a % b;
        a = b;
        b = kalan;
    }
    if a == 0 {
        1
    } else {
        a
    }
}

impl Serialize for EnBoy {
    /// Oranı `"9:16"` metni olarak serileştirir.
    fn serialize<S: serde::Serializer>(&self, serilestirici: S) -> Result<S::Ok, S::Error> {
        serilestirici.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EnBoy {
    /// `"9:16"` metninden oranı çözümler.
    fn deserialize<D: serde::Deserializer<'de>>(cozucu: D) -> Result<Self, D::Error> {
        let metin = String::deserialize(cozucu)?;
        EnBoy::ayrıştir(&metin).map_err(serde::de::Error::custom)
    }
}

impl Serialize for KareHazi {
    /// Kare hızını `"30000/1001"` metni olarak serileştirir.
    fn serialize<S: serde::Serializer>(&self, serilestirici: S) -> Result<S::Ok, S::Error> {
        serilestirici.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for KareHazi {
    /// `"30000/1001"` metninden kare hızını çözümler.
    fn deserialize<D: serde::Deserializer<'de>>(cozucu: D) -> Result<Self, D::Error> {
        let metin = String::deserialize(cozucu)?;
        KareHazi::ayrıştir(&metin).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` kullanımını üretim
// kodunda yasaklar ama testlerde "gerekçeyle" serbest bırakır. Testler bu modülün
// dönüş değerlerini doğrudan karşılaştırdığı için `unwrap` okunabilirliği
// belirgin biçimde artırır; üretim kodunda hiç kullanılmaz.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn en_boy_kisaltilir_ve_yazilir() {
        let oran = EnBoy::yeni(1920, 1080).unwrap();
        assert_eq!(oran.pay(), 16);
        assert_eq!(oran.payda(), 9);
        assert!((oran.deger() - 16.0 / 9.0).abs() < 1e-9);
        assert_eq!(oran.to_string(), "16:9");
    }

    #[test]
    fn en_boy_ayristirma_ve_hatalari() {
        assert_eq!(EnBoy::ayrıştir("9:16").unwrap().to_string(), "9:16");
        assert_eq!(EnBoy::ayrıştir(" 4 / 3 ").unwrap().to_string(), "4:3");
        assert!(EnBoy::ayrıştir("9-16").is_err());
        assert!(EnBoy::ayrıştir("0:16").is_err());
        assert!(EnBoy::ayrıştir("9:0").is_err());
        assert!(EnBoy::cozunurlukten(0, 100).is_err());
        assert!(EnBoy::cozunurlukten(100, 0).is_err());
    }

    #[test]
    fn en_boy_toleransli_karsilastirma() {
        let dikey = EnBoy::cozunurlukten(1080, 1920).unwrap();
        let referans = EnBoy::ayrıştir("9:16").unwrap();
        let yatay = EnBoy::ayrıştir("16:9").unwrap();
        assert!(dikey.esit_mi(referans, 0.001));
        assert!(!dikey.esit_mi(yatay, 0.01));
        assert!(yatay.cozunurluge_uyuyor_mu(1920, 1080));
        assert!(!yatay.cozunurluge_uyuyor_mu(1080, 1920));
        assert!(!yatay.cozunurluge_uyuyor_mu(0, 0));
    }

    #[test]
    fn kare_hazi_ayristirma_ve_gosterim() {
        let hiz = KareHazi::ayrıştir("30000/1001").unwrap();
        assert_eq!(hiz.pay(), 30000);
        assert_eq!(hiz.payda(), 1001);
        assert!((hiz.deger() - 29.970_029_970_029_972).abs() < 1e-9);
        assert_eq!(hiz.to_string(), "30000/1001");
        assert_eq!(KareHazi::ayrıştir("30").unwrap().to_string(), "30");
        assert_eq!(KareHazi::ayrıştir("60/1").unwrap().to_string(), "60");
        assert!(KareHazi::ayrıştir("0/0").is_err());
        assert!(KareHazi::ayrıştir("a/b").is_err());
        assert!(KareHazi::ayrıştir("1/2/3").is_err());
    }

    #[test]
    fn kare_hazi_kare_hesaplari() {
        let hiz = KareHazi::yeni(30, 1).unwrap();
        assert_eq!(hiz.kare_zamani(15), 0.5);
        assert_eq!(hiz.kare_no(0.5), 15);
        assert_eq!(hiz.kare_siniri(0.5), 15);
        assert_eq!(hiz.kare_no(0.51), 15);
        assert_eq!(hiz.kare_siniri(0.51), 16);
        assert_eq!(hiz.kare_sayisi(10.0), 300);
        assert_eq!(hiz.kare_no(-1.0), 0);
        assert_eq!(hiz.kare_siniri(f64::NAN), 0);
        assert_eq!(hiz.kare_sayisi(f64::INFINITY), 0);
        assert_eq!(hiz.kare_sayisi(1.0e30), u64::MAX);
        assert_eq!(
            hiz.kare_no(f64::INFINITY),
            0,
            "sonlu olmayan zaman reddedilir"
        );
    }

    #[test]
    fn kesirli_kare_hizi_ile_kare_siniri_kararlari() {
        let hiz = KareHazi::ayrıştir("30000/1001").unwrap();
        // 0.5 sn = 14.985... kare -> asagi 14, yukari 15
        assert_eq!(hiz.kare_no(0.5), 14);
        assert_eq!(hiz.kare_siniri(0.5), 15);
        // 1.0 sn tam kare sinirinda: bir sonraki karenin baslangici
        assert_eq!(hiz.kare_siniri(1.0), 30);
        assert_eq!(hiz.kare_no(1.0), 29, "kare_no tabana dogru yuvarlar");
        assert_eq!(
            hiz.kare_siniri(1.0),
            30,
            "kare_siniri yukari dogru yuvarlar"
        );
    }

    #[test]
    fn kare_hazi_sabit_dondurmesi_bozuk_degerleri_reddeder() {
        assert_eq!(
            KareHazi::sabit(29.97).unwrap(),
            KareHazi::yeni(2997, 100).unwrap()
        );
        assert!(KareHazi::sabit(0.0).is_err());
        assert!(KareHazi::sabit(-1.0).is_err());
        assert!(KareHazi::sabit(f64::NAN).is_err());
        assert!(KareHazi::sabit(f64::INFINITY).is_err());
        assert!(KareHazi::sabit(1.0e30).is_err());
    }

    #[test]
    fn en_boy_ve_kare_hazi_serde_metin_temelli() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Kabuk {
            oran: EnBoy,
            hiz: KareHazi,
            belirsiz_hiz: Option<KareHazi>,
        }
        let kabuk = Kabuk {
            oran: EnBoy::ayrıştir("9:16").unwrap(),
            hiz: KareHazi::ayrıştir("30000/1001").unwrap(),
            belirsiz_hiz: None,
        };
        let metin = serde_json::to_string(&kabuk).unwrap();
        assert_eq!(
            metin,
            r#"{"oran":"9:16","hiz":"30000/1001","belirsiz_hiz":null}"#
        );
        let geri: Kabuk = serde_json::from_str(&metin).unwrap();
        assert_eq!(geri, kabuk);
        assert!(serde_json::from_str::<Kabuk>(r#"{"oran":"9-16","hiz":"30"}"#).is_err());
        assert!(serde_json::from_str::<Kabuk>(r#"{"oran":"9:16","hiz":"0/0"}"#).is_err());
    }

    #[test]
    fn en_buyuk_ortak_bolen_sifira_karsi_guvenli() {
        assert_eq!(en_buyuk_ortak_bolen(0, 0), 1);
        assert_eq!(en_buyuk_ortak_bolen(0, 7), 7);
        assert_eq!(en_buyuk_ortak_bolen(7, 0), 7);
        assert_eq!(en_buyuk_ortak_bolen(12, 18), 6);
    }
}
