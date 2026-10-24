//! Emplacements des fichiers. Réglages et historique : mêmes fichiers que la version 1 (`%APPDATA%\Murmure`).
//! Modèles : `%LOCALAPPDATA%\Murmure\models`, propre à Murmure (le cache Hugging Face de la version 1 n'est pas
//! partagé : aucun autre programme ne peut verrouiller ou abîmer ces fichiers).
use std::path::PathBuf;

fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn home() -> PathBuf {
    env_dir("USERPROFILE").or_else(|| env_dir("HOME")).unwrap_or_else(|| PathBuf::from("."))
}

/// `MURMURE_DATA_DIR` remplace tous les dossiers (tests automatiques).
pub fn data_dir() -> PathBuf {
    env_dir("MURMURE_DATA_DIR")
        .or_else(|| env_dir("APPDATA").map(|d| d.join("Murmure")))
        .unwrap_or_else(|| home().join(".config").join("Murmure"))
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn history_path() -> PathBuf {
    data_dir().join("history.json")
}

pub fn models_dir() -> PathBuf {
    env_dir("MURMURE_DATA_DIR")
        .map(|d| d.join("models"))
        .or_else(|| env_dir("LOCALAPPDATA").map(|d| d.join("Murmure").join("models")))
        .unwrap_or_else(|| home().join(".cache").join("Murmure").join("models"))
}
