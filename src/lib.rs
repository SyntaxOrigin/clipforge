//! ClipForge — platform profillerine göre video kırp planlayıcı.
//!
//! # Ne yapar
//!
//! MP4/MOV (ISO BMFF) ve Matroska/WebM kapsüllerini **saf Rust** ile ayrıştırır,
//! kullanıcının seçtiği platform profiline göre kırp aralığını kare hassasiyetinde
//! hesaplar, kadraj (kırpma/dolgu) kararını verir ve bunu bir JSON kırp planı olarak
//! dışa aktarır.
//!
//! # Ne yapmaz
//!
//! **Kodlama yapmaz.** `ffmpeg`/`ffprobe` çağırmaz, harici süç başlatmaz, ağ
//! kullanmaz. Rapor (`b07`) bu ürünü FFmpeg 7.x ile kurgulamıştı; bağımlılık
//! politikası (karar D-005) bunu yasakladığı için ürünün "yeniden kodlama" yönü
//! yerine "kapsul düzenleme ve kırp planlama" yönü korunmuştur. Ayrıntı için
//! `README.md` → `## Bilinen Sınırlamalar`.
//!
//! # Modüller
//!
//! | Modül | Sorumluluk |
//! |---|---|
//! | [`olcu`] | En-boy oranı ve kare hızı kesirleri |
//! | [`iso_bmff`] | ISO/IEC 14496-12 kutu ayrıştırıcı (kademeli okuma) |
//! | [`ebml`] | Matroska/WebM (EBML) ayrıştırıcı |
//! | [`medya`] | İki biçimin ortak katmanı ve kural denetimleri |
//! | [`profil`] | Platform profilleri, JSON yükleme ve doğrulama |
//! | [`kadraj`] | Kaynak en-boy oranından hedef orana geçiş |
//! | [`zamanlama`] | Kırp penceresi, kare eşlemesi, yeniden zamanlama |
//! | [`plan`] | JSON kırp planı belgesi |
//! | [`kuyruk`] | Klasör gezme, uzantı filtresi, dosya başına hata ayrımı |
//! | [`ornek`] | Test ve gösterim için sentetik kapsül üretici |
//! | [`rapor`] | İnsan okunur metin çıktıları |
//! | [`cli`] | Komut satırı arayüzü |
//! | [`hata`] | Ortak hata tipi |

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

pub mod cli;
pub mod ebml;
pub mod hata;
pub mod iso_bmff;
pub mod kadraj;
pub mod kuyruk;
pub mod medya;
pub mod olcu;
pub mod ornek;
pub mod plan;
pub mod profil;
pub mod rapor;
pub mod zamanlama;

#[cfg(test)]
pub mod test_yardimci;

/// Program sürümü (`Cargo.toml` ile eşleşir).
pub const SURUM: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
// Gerekçe: WORKER_CONTRACT.md § 4.2 `unwrap`/`expect` yalnızca testlerde
// gerekçeyle kullanılabilir; testler dönüş değerlerini doğrudan karşılaştırır.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn surum_paket_versiyonuyla_ortusur() {
        assert_eq!(SURUM, env!("CARGO_PKG_VERSION"));
        assert!(!SURUM.is_empty());
    }

    #[test]
    fn kutuphane_yuzeyi_beklenen_modulleri_sunar() {
        // Modüllerin her biri en az bir genel tip dışa sunmalıdır; bu, `missing_docs`
        // ve yeniden adlandırma sonrası erişilebilirliği korur.
        let _: EnBoyTipi = olcu::EnBoy::yeni(16, 9).unwrap();
        let _ = iso_bmff::Ayarlar::default();
        let _ = ebml::Ayarlar::default();
        let _ = medya::Bicim::IsoBmff;
        let _ = kadraj::KadrajKipi::Kirpmasiz;
        let _ = zamanlama::KirpPenceresi::tam(1.0);
        let _ = plan::PLAN_SURUMU;
        let _ = kuyruk::VARSAYILAN_UZANTILAR;
        let _ = profil::KATALOG_SURUMU;
        assert!(ornek::Mp4Uretici::yeni().sure_sn() > 0.0);
    }

    type EnBoyTipi = olcu::EnBoy;
}
