//! Cœur de Murmure, sans dépendance système : testable sur toutes les plateformes.
pub mod cleanup;
pub mod dictionary;
pub mod history;
pub mod live;
pub mod release;
pub mod resample;
pub mod segment;
pub mod settings;

use std::path::Path;

/// Écrit un fichier en passant par un fichier temporaire : jamais de fichier à moitié écrit.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}
