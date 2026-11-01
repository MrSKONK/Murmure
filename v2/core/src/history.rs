//! Historique des dictées, enregistré en JSON à côté des réglages (jamais envoyé ailleurs).
//! Même format que la version 1 : `[{"id", "time", "text", "duration", "language"}]`.
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::write_atomic;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub time: f64,
    pub text: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub language: String,
}

pub struct History {
    path: PathBuf,
    inner: Mutex<(Vec<Entry>, usize)>, // (du plus ancien au plus récent, limite ; 0 = illimité)
}

impl History {
    pub fn new(path: PathBuf, limit: usize) -> Self {
        let entries = Self::load(&path);
        History { path, inner: Mutex::new((entries, limit)) }
    }

    fn load(path: &PathBuf) -> Vec<Entry> {
        let Ok(data) = std::fs::read(path) else { return Vec::new() };
        match serde_json::from_slice::<Vec<serde_json::Value>>(&data) {
            Ok(items) => items.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect(),
            Err(e) => {
                eprintln!("[Historique] illisible ({e}) — historique vide.");
                Vec::new()
            }
        }
    }

    fn save(&self, entries: &[Entry]) -> std::io::Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(entries).unwrap_or_default())
    }

    fn trim(entries: &mut Vec<Entry>, limit: usize) {
        if limit > 0 && entries.len() > limit {
            entries.drain(..entries.len() - limit);
        }
    }

    pub fn add(&self, text: &str, duration: f64, language: &str) -> std::io::Result<Entry> {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        let entry = Entry {
            id: uuid::Uuid::new_v4().simple().to_string(),
            time,
            text: text.to_string(),
            duration: (duration * 10.0).round() / 10.0,
            language: language.to_string(),
        };
        let mut guard = self.inner.lock().unwrap();
        let limit = guard.1;
        guard.0.push(entry.clone());
        Self::trim(&mut guard.0, limit);
        self.save(&guard.0)?;
        Ok(entry)
    }

    /// Du plus récent au plus ancien.
    pub fn entries(&self) -> Vec<Entry> {
        self.inner.lock().unwrap().0.iter().rev().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<Entry> {
        self.inner.lock().unwrap().0.iter().find(|e| e.id == id).cloned()
    }

    pub fn delete(&self, id: &str) -> bool {
        let mut guard = self.inner.lock().unwrap();
        let before = guard.0.len();
        guard.0.retain(|e| e.id != id);
        let changed = guard.0.len() != before;
        if changed {
            let _ = self.save(&guard.0);
        }
        changed
    }

    pub fn clear(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.0.clear();
        let _ = self.save(&guard.0);
    }

    pub fn set_limit(&self, limit: usize) {
        let mut guard = self.inner.lock().unwrap();
        guard.1 = limit;
        let before = guard.0.len();
        Self::trim(&mut guard.0, limit);
        if guard.0.len() != before {
            let _ = self.save(&guard.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("murmure-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("history.json")
    }

    #[test]
    fn add_list_delete_persist() {
        let path = temp_path("basic");
        let h = History::new(path.clone(), 0);
        let a = h.add("un", 1.26, "fr").unwrap();
        h.add("deux", 2.0, "fr").unwrap();
        assert_eq!(a.duration, 1.3);
        assert_eq!(h.entries().iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["deux", "un"]);
        assert_eq!(h.get(&a.id).unwrap().text, "un");
        assert!(h.delete(&a.id));
        assert!(!h.delete(&a.id));
        let reloaded = History::new(path.clone(), 0);
        assert_eq!(reloaded.entries().len(), 1);
        reloaded.clear();
        assert!(History::new(path, 0).entries().is_empty());
    }

    #[test]
    fn limit_keeps_most_recent() {
        let h = History::new(temp_path("limit"), 3);
        for i in 0..5 {
            h.add(&format!("t{i}"), 0.0, "").unwrap();
        }
        assert_eq!(h.entries().iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["t4", "t3", "t2"]);
        h.set_limit(1);
        assert_eq!(h.entries().len(), 1);
    }

    #[test]
    fn reads_version_1_file_and_skips_bad_items() {
        let path = temp_path("v1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"[{"id": "a", "time": 1700000000.5, "text": "Bonjour", "duration": 3.2, "language": "fr"},
            {"id": "b"}, 42, {"text": "sans id"}]"#,
        )
        .unwrap();
        let h = History::new(path, 0);
        assert_eq!(h.entries().iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["sans id", "Bonjour"]);
    }

    #[test]
    fn unreadable_file_gives_empty_history() {
        let path = temp_path("bad");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{pas du json").unwrap();
        assert!(History::new(path, 0).entries().is_empty());
    }
}
