//! Journal (`%APPDATA%\Murmure\murmure.log`) et garde-fous de démarrage.
//!
//! Sous Windows l'application n'a pas de console : sans ce journal, un arrêt brutal ne laisse aucune trace.
//! Les garde-fous notent le début d'une étape risquée (modèle sur la carte graphique, correcteur) et l'effacent à
//! la fin ; s'ils sont encore là au démarrage suivant, c'est que l'étape a fait planter Murmure, qui la saute alors.
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths;

static FILE: Mutex<()> = Mutex::new(());

pub fn path() -> PathBuf {
    paths::data_dir().join("murmure.log")
}

/// Ajoute une ligne au journal (et à la sortie d'erreur en développement).
pub fn write(line: &str) {
    eprintln!("{line}");
    let _lock = FILE.lock();
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    let _ = std::fs::create_dir_all(paths::data_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path()) {
        let _ = writeln!(f, "{h:02}:{m:02}:{s:02} UTC {line}");
    }
}

/// Au démarrage : le journal précédent est gardé à côté s'il devient gros.
pub fn rotate() {
    if std::fs::metadata(path()).is_ok_and(|m| m.len() > 1 << 20) {
        let _ = std::fs::rename(path(), path().with_extension("old.log"));
    }
}

#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => { $crate::log::write(&format!($($arg)*)) };
}

fn marker(name: &str) -> PathBuf {
    paths::data_dir().join(format!("en-cours-{name}"))
}

/// Étape risquée en cours : le repère disparaît quand la garde est relâchée.
pub struct Guard(PathBuf);

pub fn begin(name: &str) -> Guard {
    let path = marker(name);
    let _ = std::fs::create_dir_all(paths::data_dir());
    let _ = std::fs::write(&path, b"");
    Guard(path)
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Fermeture normale (Quitter, mise à jour) : une étape interrompue volontairement n'est pas un plantage.
pub fn clear_markers() {
    let Ok(entries) = std::fs::read_dir(paths::data_dir()) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with("en-cours-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Vrai si l'étape avait commencé sans finir (Murmure s'est arrêté pendant) ; le repère est effacé.
pub fn crashed_during(name: &str) -> bool {
    let path = marker(name);
    let found = path.exists();
    let _ = std::fs::remove_file(path);
    found
}

/// Message d'erreur visible (une erreur interne, sinon, fermerait Murmure sans rien dire).
pub fn alert(title: &str, text: &str) {
    if std::env::var_os("MURMURE_NO_ALERT").is_some() {
        return; // autotests de la construction : personne pour fermer la fenêtre
    }
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        unsafe {
            MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_OK | MB_ICONERROR);
        }
    }
    #[cfg(not(windows))]
    let _ = (title, text);
}

/// Toute erreur interne est notée dans le journal et affichée avant que Murmure ne s'arrête.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        write(&format!("[Plantage] {info}\n{backtrace}"));
        alert(
            "Murmure s'est arrêté",
            &format!("Erreur interne :\n{info}\n\nDétails dans le journal :\n{}", path().display()),
        );
    }));
    #[cfg(windows)]
    unsafe {
        windows::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter(Some(native_crash));
    }
}

/// Plantage hors du code Rust (pilote de la carte graphique, whisper.cpp…) : noté avec le module en cause.
#[cfg(windows)]
unsafe extern "system" fn native_crash(
    info: *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{
        GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    };
    let (code, address) = match unsafe { info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) } {
        Some(r) => (r.ExceptionCode.0 as u32, r.ExceptionAddress),
        None => (0, std::ptr::null_mut()),
    };
    let mut module = HMODULE::default();
    let mut name = [0u16; 260];
    let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
    let len = unsafe {
        match GetModuleHandleExW(flags, PCWSTR(address as *const u16), &mut module) {
            Ok(()) => GetModuleFileNameW(Some(module), &mut name) as usize,
            Err(_) => 0,
        }
    };
    let module = String::from_utf16_lossy(&name[..len]);
    let module = if module.is_empty() { "module inconnu".to_string() } else { module };
    write(&format!("[Plantage] erreur système 0x{code:08X} dans {module} (adresse {address:?})"));
    alert(
        "Murmure s'est arrêté",
        &format!("Erreur système 0x{code:08X} dans :\n{module}\n\nDétails dans le journal :\n{}", path().display()),
    );
    0 // EXCEPTION_CONTINUE_SEARCH : Windows termine le programme comme d'habitude
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_detects_an_unfinished_step() {
        std::env::set_var("MURMURE_DATA_DIR", std::env::temp_dir().join(format!("murmure-log-{}", std::process::id())));
        drop(begin("essai"));
        assert!(!crashed_during("essai"), "étape terminée normalement");
        std::mem::forget(begin("essai")); // comme un arrêt brutal : la garde n'est jamais relâchée
        assert!(crashed_during("essai"));
        assert!(!crashed_during("essai"), "repère effacé après lecture");
        write("ligne de test");
        assert!(std::fs::read_to_string(path()).unwrap().contains("ligne de test"));
    }
}
