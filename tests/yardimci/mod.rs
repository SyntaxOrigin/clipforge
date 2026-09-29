//! Entegrasyon testleri için geçici dosya/dizin yardımcısı.
//!
//! `tempfile` crate'i bağımlılık politikası gereği yasaktır
//! (`WORKER_CONTRACT.md` § 3.2); bu yüzden yardımcı kendi kodumuzla yazılır.
//!
//! Rastgelelik crate'i de yasak olduğundan benzersizlik `std::process::id()`
//! ve etiketten türetilir. **Etiketler testler arasında paylaşılmamalıdır:**
//! aynı etiketle açılan ikinci dizin öncekini siler, bu da eşzamanlı çalışan
//! testlerde yarış durumu yaratır.

use std::path::{Path, PathBuf};

/// Test içinde geçici dosya/dizin üreten, `Drop` ile temizleyen kapsayıcı.
#[derive(Debug)]
pub struct GeciciDizin {
    yol: PathBuf,
}

impl GeciciDizin {
    /// `std::env::temp_dir()` altında, etiketten türetilmiş benzersiz bir
    /// dizin oluşturur.
    ///
    /// # Hatalar
    ///
    /// Dizin oluşturulamazsa işletim sistemi hatası döner.
    pub fn yeni(etiket: &str) -> std::io::Result<Self> {
        let kok = std::env::temp_dir().join(format!("clipforge-{etiket}-{}", std::process::id()));
        // Aynı testin iki kez çalışması olasıdır; eski içerik temizlenir.
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(&kok)?;
        Ok(Self { yol: kok })
    }

    /// Dizinin yolunu döndürür.
    pub fn yol(&self) -> &Path {
        &self.yol
    }

    /// Dizin içine göreli yol verip dosyayı oluşturur, yolunu döndürür.
    ///
    /// # Hatalar
    ///
    /// Dosya oluşturulamazsa işletim sistemi hatası döner.
    pub fn yaz(&self, ad: &str, icerik: &[u8]) -> std::io::Result<PathBuf> {
        let yol = self.yol.join(ad);
        if let Some(ust) = yol.parent() {
            std::fs::create_dir_all(ust)?;
        }
        std::fs::write(&yol, icerik)?;
        Ok(yol)
    }
}

impl Drop for GeciciDizin {
    fn drop(&mut self) {
        // Temizlik başarısız olsa da testi düşürmemeli; `let _ =` bilinçlidir.
        // `Drop` içinden hata döndürülemez (WORKER_CONTRACT.md § 5.3).
        let _ = std::fs::remove_dir_all(&self.yol);
    }
}

/// `metin` içinde `parca` alt dizisinin bulunduğunu bayt bayt arar.
///
/// # Neden `str::contains` değil
///
/// Bu ortamdaki `rustc 1.98.1 (48a229cea)`, bir program içinde biri diğerinin
/// ön eki olan dize sabitleri üzerinde `str::contains`/`str::find` çağrılarını
/// bazı bağlamlarda hatalı derlemektedir. Testler bu yüzden alt dize
/// denetimini standart kitaplığa bırakmaz. Ayrıntı için `README.md` →
/// `## Bilinen Sınırlamalar`.
pub fn iceriyor(metin: &str, parca: &str) -> bool {
    let hedef = parca.as_bytes();
    if hedef.is_empty() {
        return true;
    }
    metin
        .as_bytes()
        .windows(hedef.len())
        .any(|pencere| pencere == hedef)
}
