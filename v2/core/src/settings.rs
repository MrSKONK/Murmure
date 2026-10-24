//! Réglages, dans le même fichier que la version 1 (`%APPDATA%\Murmure\settings.json`) : la version 2 reprend
//! les préférences existantes, et les clés qu'elle ajoute sont ignorées par la version 1.
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::write_atomic;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DictEntry {
    #[serde(default)]
    pub term: String,
    #[serde(default)]
    pub heard: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub hotkey: String,
    /// hold = maintenir, toggle = appuyer pour démarrer / arrêter
    pub hotkey_mode: String,
    pub whisper_model: String,
    /// "auto" = détection automatique
    pub whisper_language: String,
    /// version 1 seulement (Faster-Whisper) ; gardés pour revenir en arrière sans perdre ses réglages
    pub device: String,
    pub compute_type: String,
    /// 0 = tous les cœurs
    pub cpu_threads: u64,
    /// "" = micro par défaut de Windows
    pub mic: String,
    /// 0 = garder le modèle en mémoire
    pub unload_after_min: u64,
    pub show_overlay: bool,
    pub autostart: bool,
    pub dictionary: Vec<DictEntry>,
    pub remove_fillers: bool,
    pub spoken_addresses: bool,
    pub short_no_period: bool,
    pub fit_to_context: bool,
    pub grammar_enabled: bool,
    pub spoken_punctuation: bool,
    pub french_nbsp: bool,
    pub history_enabled: bool,
    /// 0 = toutes
    pub history_limit: u64,
    pub auto_update: bool,
    pub log_transcripts: bool,
    /// version 2 : texte provisoire affiché dans la pastille pendant qu'on parle
    pub live_preview: bool,
    /// version 2 : carte graphique (Vulkan) quand elle est disponible
    pub gpu: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hotkey: "²".into(),
            hotkey_mode: "hold".into(),
            whisper_model: "medium".into(),
            whisper_language: "fr".into(),
            device: "cpu".into(),
            compute_type: "int8".into(),
            cpu_threads: 0,
            mic: String::new(),
            unload_after_min: 0,
            show_overlay: true,
            autostart: false,
            dictionary: Vec::new(),
            remove_fillers: true,
            spoken_addresses: true,
            short_no_period: true,
            fit_to_context: true,
            grammar_enabled: false,
            spoken_punctuation: true,
            french_nbsp: true,
            history_enabled: true,
            history_limit: 1000,
            auto_update: true,
            log_transcripts: false,
            live_preview: true,
            gpu: true,
        }
    }
}

pub const MODELS: &[&str] = &["tiny", "base", "small", "medium", "large-v3-turbo", "large-v3"];
pub const LANGUAGES: &[&str] = &["auto", "fr", "en", "es", "de", "it", "pt", "nl"];
const MODES: &[&str] = &["hold", "toggle"];

impl Settings {
    pub fn load(path: &Path) -> Settings {
        let Ok(data) = std::fs::read(path) else { return Settings::default() };
        let data = data.strip_prefix("\u{feff}".as_bytes()).unwrap_or(&data);
        match serde_json::from_slice::<Value>(data) {
            Ok(stored) => Settings::default().merged(&stored),
            Err(e) => {
                eprintln!("[Réglages] settings.json illisible ({e}) — valeurs par défaut utilisées.");
                Settings::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_atomic(path, &serde_json::to_vec_pretty(self).unwrap_or_default())
    }

    /// Applique les valeurs valides de `patch` (clés inconnues ou valeurs incorrectes ignorées).
    pub fn merged(&self, patch: &Value) -> Settings {
        let Value::Object(mut current) = serde_json::to_value(self).expect("réglages sérialisables") else {
            unreachable!()
        };
        if let Value::Object(patch) = patch {
            for (key, value) in patch {
                if let Some(slot) = current.get_mut(key) {
                    if let Some(v) = coerce(key, slot, value) {
                        *slot = v;
                    }
                }
            }
        }
        let mut s: Settings = serde_json::from_value(Value::Object(current)).unwrap_or_else(|_| self.clone());
        if s.hotkey.trim().is_empty() {
            s.hotkey = self.hotkey.clone();
        }
        s
    }
}

fn coerce(key: &str, current: &Value, value: &Value) -> Option<Value> {
    let allowed = match key {
        "hotkey_mode" => Some(MODES),
        "whisper_model" => Some(MODELS),
        "whisper_language" => Some(LANGUAGES),
        _ => None,
    };
    match current {
        Value::Bool(_) => Some(Value::Bool(match value {
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64() != Some(0.0),
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
        })),
        Value::Number(_) => {
            let n = match value {
                Value::Number(n) => n.as_f64()?,
                Value::String(s) => s.trim().parse::<f64>().ok()?,
                Value::Bool(b) => f64::from(u8::from(*b)),
                _ => return None,
            };
            Some(Value::from(n.max(0.0) as u64))
        }
        Value::String(_) => {
            let s = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
                _ => return None,
            };
            if allowed.is_some_and(|a| !a.contains(&s.as_str())) {
                return None;
            }
            Some(Value::String(s))
        }
        Value::Array(_) if key == "dictionary" => {
            let entries: Vec<Value> = value
                .as_array()?
                .iter()
                .filter_map(Value::as_object)
                .filter_map(|e| {
                    let field = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
                    let term = field("term");
                    (!term.is_empty()).then(|| {
                        let mut m = Map::new();
                        m.insert("term".into(), Value::String(term));
                        m.insert("heard".into(), Value::String(field("heard")));
                        Value::Object(m)
                    })
                })
                .collect();
            Some(Value::Array(entries))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn version_1_file_is_read_and_unknown_keys_dropped() {
        let v1 = json!({"hotkey": "ctrl+space", "hotkey_mode": "toggle", "whisper_model": "small", "device": "cuda",
                        "ollama_model": "x", "dictionary": [{"term": "Kubernetes", "heard": "cuber nette"}],
                        "autostart": true});
        let s = Settings::default().merged(&v1);
        assert_eq!(
            (s.hotkey.as_str(), s.hotkey_mode.as_str(), s.whisper_model.as_str()),
            ("ctrl+space", "toggle", "small")
        );
        assert_eq!(s.device, "cuda");
        assert!(s.autostart && s.live_preview);
        assert_eq!(s.dictionary, vec![DictEntry { term: "Kubernetes".into(), heard: "cuber nette".into() }]);
        let out = serde_json::to_value(&s).unwrap();
        assert!(out.get("ollama_model").is_none());
    }

    #[test]
    fn invalid_values_ignored() {
        let base = Settings::default();
        let s = base.merged(&json!({"whisper_model": "huge", "hotkey_mode": 3, "hotkey": "  ", "history_limit": -5,
                                    "cpu_threads": "4", "show_overlay": 0, "dictionary": [{"term": " "}, {"term": " A ", "heard": 2}, 7]}));
        assert_eq!(s.whisper_model, "medium");
        assert_eq!(s.hotkey_mode, "hold");
        assert_eq!(s.hotkey, "²");
        assert_eq!(s.history_limit, 0);
        assert_eq!(s.cpu_threads, 4);
        assert!(!s.show_overlay);
        assert_eq!(s.dictionary, vec![DictEntry { term: "A".into(), heard: String::new() }]);
    }

    #[test]
    fn load_missing_bad_and_bom_files() {
        let dir = std::env::temp_dir().join(format!("murmure-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(Settings::load(&dir.join("absent.json")), Settings::default());
        std::fs::write(dir.join("bad.json"), "{").unwrap();
        assert_eq!(Settings::load(&dir.join("bad.json")), Settings::default());
        std::fs::write(dir.join("bom.json"), "\u{feff}{\"hotkey\": \"f9\"}").unwrap();
        assert_eq!(Settings::load(&dir.join("bom.json")).hotkey, "f9");
        let path = dir.join("sub").join("settings.json");
        let mut s = Settings::default();
        s.hotkey = "ctrl+alt+d".into();
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
    }
}
