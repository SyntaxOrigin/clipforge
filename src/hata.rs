//! Proje genelinde kullanılan hata tipi.
//!
//! Bu modül tek bir enum tanımlar ve `Display` uygulamasını elle yazar.
//! `thiserror` gibi bir türetme crate'i bağımlılık politikası
//! (`WORKER_CONTRACT.md` § 3.2) gereği kullanılamaz.
//!
//! Hata sınıfları ayrı tutulur: kullanıcı "dosyam bozuk" ile "sürücü yok"
//! mesajlarını farklı ele alabilmelidir (rapor `b07` — Hata yönetimi).

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// ClipForge'un tüm çalışma zamanı hatalarını taşıyan tip.
#[derive(Debug)]
#[non_exhaustive]
pub enum ClipForgeHata {
    /// Girdi dosyası okunamadı.
    GirdiHatasi {
        /// Okunamayan dosyanın yolu.
        yol: PathBuf,
        /// Altta yatan işletim sistemi hatası.
        hata: io::Error,
    },
    /// Çıktı dosyası ya da dizini yazılamadı.
    CiktiHatasi {
        /// Yazılamayan yol.
        yol: PathBuf,
        /// Altta yatan işletim sistemi hatası.
        hata: io::Error,
    },
    /// Dosya bilinen bir kapsül biçimi değil.
    BilinmeyenKapsul {
        /// İncelenen dosyanın yolu.
        yol: PathBuf,
        /// Neden tanınmadığına dair kısa açıklama.
        sebep: String,
    },
    /// Bir kutunun 8 baytlık başlığı okunamadı ya da anlamsız.
    BozukBaslik {
        /// Dosya içindeki bayt ofseti.
        ofset: u64,
        /// Gözlenen durumun açıklaması.
        ayrinti: String,
    },
    /// Kutu boyutu geçersiz: başlıktan küçük veya dosya sınırını aşıyor.
    GecersizBoyut {
        /// Kutunun başladığı bayt ofseti.
        ofset: u64,
        /// Başlıkta ilan edilen boyut.
        boyut: u64,
        /// Dosyanın toplam boyutu.
        dosya_boyutu: u64,
    },
    /// Kutu, ayarlanan üst sınırı aşıyor (bellek bütçesi koruması).
    AsiriKutu {
        /// Kutunun tipi.
        tip: String,
        /// İlan edilen boyut.
        boyut: u64,
        /// Uygulanan üst sınır.
        sinir: u64,
    },
    /// Kutu hiyerarşisindeki öge veya derinlik sayısı sınırı aştı.
    YinelemeSiniri {
        /// Aşılan sınırın türü: `oge` veya `derinlik`.
        tur: String,
        /// Uygulanan sınır.
        sinir: u64,
    },
    /// Kapsülde `moov` kutusu bulunamadı.
    MoovYok {
        /// Aranan dosyanın yolu.
        yol: PathBuf,
    },
    /// Bir kutu var ama içeriği beklenen uzunlukta değil.
    EksikKutu {
        /// Eksik içeriğe sahip kutunun kutu yolu.
        yol: String,
        /// Kutu boyutundan fazla bayt okunmaya çalışıldı.
        ayrinti: String,
    },
    /// Kapsülün bildirdiği süre sıfır.
    SureSifir {
        /// Süresi sıfır olan kapsülün yolu.
        yol: PathBuf,
    },
    /// Kare hızı kutu dizinlerinden güvenilir biçimde çözülemedi.
    KareHiziHatali {
        /// Hatanın ayrıntısı.
        ayrinti: String,
    },
    /// Video parçasının çözünürlüğü sıfır veya yok.
    CozunurlukYok {
        /// Parçanın iz numarası.
        parca: u32,
    },
    /// Kırp penceresi geçersiz: negatif, sıfır uzunluklu ya da tek kareden kısa.
    AralikGecersiz {
        /// İstenen başlangıç saniyesi.
        baslangic_sn: f64,
        /// İstenen bitiş saniyesi.
        bitis_sn: f64,
        /// Hatanın ayrıntısı.
        ayrinti: String,
    },
    /// Kırp penceresi kaynak kapsülün süresinin dışına taşıyor.
    KaynakAsildi {
        /// İstenen bitiş saniyesi.
        istenen_sn: f64,
        /// Kaynak kapsülün süresi.
        kaynak_sn: f64,
    },
    /// İstenen profil kimliği katalogda yok.
    ProfilYok {
        /// Aranan profil kimliği.
        kimlik: String,
    },
    /// Profil tanımı semantik olarak geçersiz.
    ProfilHatali {
        /// Hatalı profilin kimliği.
        kimlik: String,
        /// Doğrulama hatasının açıklaması.
        ayrinti: String,
    },
    /// Profil yapılandırma dosyası okunamadı ya da JSON olarak çözülemedi.
    ProfilDosyasiHatali {
        /// Hatanın ayrıntısı.
        ayrinti: String,
    },
    /// Dizin okunamadı.
    DizinHatasi {
        /// Okunamayan dizinin yolu.
        yol: PathBuf,
        /// Altta yatan işletim sistemi hatası.
        hata: io::Error,
    },
    /// Komut satırı argümanları anlamsız.
    ArgumanHatasi {
        /// Argüman hatasının açıklaması.
        ayrinti: String,
    },
}

impl fmt::Display for ClipForgeHata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GirdiHatasi { yol, hata } => write!(f, "{} okunamadi: {hata}", yol.gorunum()),
            Self::CiktiHatasi { yol, hata } => write!(f, "{} yazilamadi: {hata}", yol.gorunum()),
            Self::BilinmeyenKapsul { yol, sebep } => {
                write!(f, "{} desteklenen bir kapsul degil: {sebep}", yol.gorunum())
            }
            Self::BozukBaslik { ofset, ayrinti } => {
                write!(f, "bozuk kutu basligi @{ofset}: {ayrinti}")
            }
            Self::GecersizBoyut {
                ofset,
                boyut,
                dosya_boyutu,
            } => write!(
                f,
                "gecersiz kutu boyutu @{ofset}: ilan edilen {boyut} bayt, dosya {dosya_boyutu} bayt"
            ),
            Self::AsiriKutu { tip, boyut, sinir } => {
                write!(
                    f,
                    "'{tip}' kutusu cok buyuk: {boyut} bayt > {sinir} bayt siniri"
                )
            }
            Self::YinelemeSiniri { tur, sinir } => {
                write!(f, "{tur} siniri asildi: en fazla {sinir}")
            }
            Self::MoovYok { yol } => {
                write!(f, "{} icinde 'moov' kutusu bulunamadi", yol.gorunum())
            }
            Self::EksikKutu { yol, ayrinti } => write!(f, "eksik kutu verisi ({yol}): {ayrinti}"),
            Self::SureSifir { yol } => write!(f, "{} suresi sifir", yol.gorunum()),
            Self::KareHiziHatali { ayrinti } => write!(f, "kare hizi cozulemedi: {ayrinti}"),
            Self::CozunurlukYok { parca } => write!(f, "parca {parca} icin cozunurluk bulunamadi"),
            Self::AralikGecersiz {
                baslangic_sn,
                bitis_sn,
                ayrinti,
            } => write!(
                f,
                "gecersiz kirp araligi [{baslangic_sn}, {bitis_sn}]: {ayrinti}"
            ),
            Self::KaynakAsildi {
                istenen_sn,
                kaynak_sn,
            } => write!(
                f,
                "kaynak suresini asiyor: istenen bitis {istenen_sn} sn, kaynak {kaynak_sn} sn"
            ),
            Self::ProfilYok { kimlik } => write!(f, "profil bulunamadi: '{kimlik}'"),
            Self::ProfilHatali { kimlik, ayrinti } => {
                write!(f, "profil '{kimlik}' gecersiz: {ayrinti}")
            }
            Self::ProfilDosyasiHatali { ayrinti } => {
                write!(f, "profil dosyasi okunamadi: {ayrinti}")
            }
            Self::DizinHatasi { yol, hata } => write!(f, "{} okunamadi: {hata}", yol.gorunum()),
            Self::ArgumanHatasi { ayrinti } => write!(f, "arguman hatasi: {ayrinti}"),
        }
    }
}

impl Error for ClipForgeHata {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GirdiHatasi { hata, .. }
            | Self::CiktiHatasi { hata, .. }
            | Self::DizinHatasi { hata, .. } => Some(hata),
            _ => None,
        }
    }
}

impl ClipForgeHata {
    /// Bir dosya yoluyla `GirdiHatasi` üretir.
    pub fn girdi(yol: &Path, hata: io::Error) -> Self {
        Self::GirdiHatasi {
            yol: yol.to_path_buf(),
            hata,
        }
    }

    /// Bir dosya yoluyla `CiktiHatasi` üretir.
    pub fn cikti(yol: &Path, hata: io::Error) -> Self {
        Self::CiktiHatasi {
            yol: yol.to_path_buf(),
            hata,
        }
    }

    /// Bir dizin yoluyla `DizinHatasi` üretir.
    pub fn dizin(yol: &Path, hata: io::Error) -> Self {
        Self::DizinHatasi {
            yol: yol.to_path_buf(),
            hata,
        }
    }

    /// Kullanıcıya gösterilecek tek satırlık hata metnini döndürür.
    pub fn ozet(&self) -> String {
        self.to_string()
    }
}

/// `Path` değerini insan okunur biçimde yazdıran küçük yardımcı.
///
/// `Path::display()` Windows'ta ters eğik çizgi üretmez; `Debug` ise
/// kaçış dizisi ekler. Kullanıcıya gösterilen yollarda bu yardımcı tercih edilir.
trait Gorunum {
    fn gorunum(&self) -> String;
}

impl Gorunum for Path {
    fn gorunum(&self) -> String {
        self.to_string_lossy().into_owned()
    }
}
