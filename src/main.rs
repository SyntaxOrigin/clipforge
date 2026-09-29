//! `clipforge` ikili giriş noktası.
//!
//! Tüm mantık `clipforge` kütüphanesindedir; bu dosya yalnızca komut satırını
//! ayrıştırır, çıktıyı standart akışlara yazar ve süreç çıkış kodunu belirler.
//!
//! Çıkış kodları:
//!
//! | Kod | Anlam |
//! |---|---|
//! | 0 | Başarılı |
//! | 1 | Çalışma zamanı hatası (ayrıştırma, profil, yazma) |
//! | 2 | Geçersiz komut satırı kullanımı (`clap`) |

#![forbid(unsafe_code)]

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;

use clipforge::cli::{calistir, Secenek};

fn main() -> ExitCode {
    let secenek = Secenek::parse();
    match calistir(&secenek) {
        Ok(cikti) => {
            let mut stdout = std::io::stdout().lock();
            // Çıktı yazımı kırılırsa bu bir boru hatasıdır (ör. `| head`); bu
            // durumda sessizce çıkmak yerine hata kodu döndürülür.
            if writeln!(stdout, "{}", cikti.metin).is_err() || stdout.flush().is_err() {
                return ExitCode::from(1);
            }
            ExitCode::from(u8::try_from(cikti.kod).unwrap_or(1))
        }
        Err(hata) => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "hata: {hata}");
            if let Some(kaynak) = std::error::Error::source(&hata) {
                let _ = writeln!(stderr, "ayrinti: {kaynak}");
            }
            ExitCode::from(1)
        }
    }
}
