//! Correcteur grammatical facultatif : Grammalecte en JavaScript (version de son extension Firefox, GPL v3).
//!
//! Il n'est pas fourni avec Murmure : l'extension est téléchargée à l'activation depuis addons.mozilla.org,
//! vérifiée par SHA-256, et seul son moteur (`grammalecte/`) est gardé dans le dossier des réglages. Une fenêtre
//! cachée (`ui/grammar.html`) le fait tourner ; Murmure lui sert ses fichiers par le protocole `murmure-gc`, lui
//! envoie le texte et choisit les corrections avec les règles de la version 1 (`murmure_core::grammar`).
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use murmure_core::grammar::{apply_corrections, GrammarError};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::paths;

pub const VERSION: &str = "2.3.0";
const URL: &str = "https://addons.mozilla.org/firefox/downloads/file/4643560/grammalecte_fr-2.3.0.xpi";
const SHA256: &str = "443030004dbf62ecd5fc42f7b96872b96c9e3765f1cba4704a2a4170bb945256";
pub const SCHEME: &str = "murmure-gc";
/// Fichiers inutiles ici (dictionnaires d'autres variantes, synonymes, tests) : non extraits.
const SKIP: &[&str] = &["fr-classic.json", "fr-reform.json", "thesaurus", "tests_data.json"];

pub fn dir() -> PathBuf {
    paths::data_dir().join("grammalecte-js").join(VERSION)
}

pub fn installed() -> bool {
    dir().join("grammalecte").join("fr").join("gc_engine.js").is_file()
}

/// Adresse du protocole vue depuis la page (elle diffère selon le système).
fn base_url() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost/")
    } else {
        format!("{SCHEME}://localhost/")
    }
}

/// Réponse du protocole `murmure-gc` : un fichier du moteur, jamais en dehors de son dossier.
pub fn serve(request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    let path = request.uri().path().trim_start_matches('/');
    let safe = !path.is_empty() && path.split('/').all(|p| !p.is_empty() && p != ".." && !p.contains('\\'));
    let file = dir().join(path);
    let builder = tauri::http::Response::builder().header("Access-Control-Allow-Origin", "*");
    match std::fs::read(&file) {
        Ok(data) if safe => {
            let kind = match file.extension().and_then(|e| e.to_str()) {
                Some("js") => "text/javascript; charset=utf-8",
                Some("json") => "application/json; charset=utf-8",
                _ => "application/octet-stream",
            };
            builder.header("Content-Type", kind).body(data)
        }
        _ => builder.status(404).body(Vec::new()),
    }
    .unwrap_or_default()
}

pub struct Grammar {
    handle: AppHandle,
    status: Mutex<Value>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<String>>>,
    ready: Mutex<Option<Sender<String>>>,
    busy: Mutex<()>,
    on_change: Box<dyn Fn(&Value) + Send + Sync>,
}

impl Grammar {
    pub fn new(handle: AppHandle, on_change: impl Fn(&Value) + Send + Sync + 'static) -> Arc<Grammar> {
        let state = if installed() { "idle" } else { "absent" };
        Arc::new(Grammar {
            handle,
            status: Mutex::new(json!({"state": state, "progress": 0.0, "error": "", "version": VERSION})),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            ready: Mutex::new(None),
            busy: Mutex::new(()),
            on_change: Box::new(on_change),
        })
    }

    pub fn status(&self) -> Value {
        self.status.lock().unwrap().clone()
    }

    fn state(&self) -> String {
        self.status.lock().unwrap()["state"].as_str().unwrap_or("").to_string()
    }

    fn set(&self, changes: Value) {
        let snapshot = {
            let mut s = self.status.lock().unwrap();
            if let (Value::Object(s), Value::Object(c)) = (&mut *s, changes) {
                s.extend(c);
            }
            s.clone()
        };
        (self.on_change)(&snapshot);
    }

    /// Télécharge (si besoin) puis charge le correcteur, en arrière-plan.
    pub fn install(self: &Arc<Self>) {
        if matches!(self.state().as_str(), "downloading" | "loading" | "ready") {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let _ = me.install_blocking();
        });
    }

    /// Comme `install`, mais attend la fin (autotest de la construction).
    pub fn install_blocking(&self) -> Result<(), String> {
        let _busy = self.busy.lock().unwrap();
        if self.state() == "ready" {
            return Ok(());
        }
        if !installed() {
            self.set(json!({"state": "downloading", "progress": 0.0, "error": ""}));
            if let Err(e) = self.download() {
                let msg = format!("Téléchargement impossible : {e}");
                self.set(json!({"state": "error", "error": msg}));
                return Err(msg);
            }
        }
        self.set(json!({"state": "loading", "progress": 1.0, "error": ""}));
        match self.load() {
            Ok(()) => {
                self.set(json!({"state": "ready", "error": ""}));
                Ok(())
            }
            Err(e) => {
                let msg = format!("Chargement impossible : {e}");
                self.set(json!({"state": "error", "error": msg}));
                Err(msg)
            }
        }
    }

    fn download(&self) -> Result<(), String> {
        let resp = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(60))
            .user_agent(concat!("Murmure/", env!("CARGO_PKG_VERSION")))
            .try_proxy_from_env(true)
            .build()
            .get(URL)
            .call()
            .map_err(|e| e.to_string())?;
        let total: u64 = resp.header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut reader = resp.into_reader();
        let (mut data, mut buf, mut shown) = (Vec::new(), vec![0u8; 1 << 16], 0.0f64);
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            if total > 0 {
                let p = data.len() as f64 / total as f64;
                if p - shown >= 0.02 {
                    shown = p;
                    self.set(json!({"progress": (p * 100.0).round() / 100.0}));
                }
            }
        }
        let digest: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        if digest != SHA256 {
            return Err("fichier téléchargé corrompu (empreinte différente)".into());
        }
        extract(&data, &dir())
    }

    /// Ouvre la fenêtre cachée du correcteur et attend qu'il soit prêt.
    fn load(&self) -> Result<(), String> {
        let (tx, rx) = channel();
        *self.ready.lock().unwrap() = Some(tx);
        if let Some(old) = self.handle.get_webview_window("grammar") {
            let _ = old.destroy();
        }
        let url = format!("grammar.html?base={}", base_url());
        WebviewWindowBuilder::new(&self.handle, "grammar", WebviewUrl::App(url.into()))
            .title("Murmure — correcteur")
            .visible(false)
            .skip_taskbar(true)
            .focused(false)
            .build()
            .map_err(|e| e.to_string())?;
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(error) if error.is_empty() => Ok(()),
            Ok(error) => Err(error),
            Err(_) => Err("le correcteur ne répond pas".into()),
        }
    }

    /// Appelé par la page quand Grammalecte est chargé (ou a échoué).
    pub fn page_ready(&self, error: String) {
        if let Some(tx) = self.ready.lock().unwrap().take() {
            let _ = tx.send(error);
        }
    }

    /// Appelé par la page avec les erreurs relevées pour une demande.
    pub fn page_result(&self, id: u64, result: String) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
            let _ = tx.send(result);
        }
    }

    /// Texte corrigé, ou inchangé si le correcteur n'est pas prêt, échoue ou tarde.
    pub fn correct(&self, text: &str) -> String {
        if self.state() != "ready" || text.trim().is_empty() {
            return text.to_string();
        }
        let Some(window) = self.handle.get_webview_window("grammar") else { return text.to_string() };
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        self.pending.lock().unwrap().insert(id, tx);
        let arg = serde_json::to_string(text).unwrap_or_default();
        if window.eval(format!("window.murmureCheck({id}, {arg})")).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return text.to_string();
        }
        let reply = rx.recv_timeout(Duration::from_millis(1500));
        self.pending.lock().unwrap().remove(&id);
        let Ok(reply) = reply else {
            eprintln!("[Grammaire] pas de réponse : texte laissé tel quel");
            return text.to_string();
        };
        let parsed: Value = serde_json::from_str(&reply).unwrap_or_default();
        match serde_json::from_value::<Vec<GrammarError>>(parsed["errors"].clone()) {
            Ok(errors) => apply_corrections(text, &errors, true),
            Err(_) => {
                eprintln!("[Grammaire] ignorée : {}", parsed["error"].as_str().unwrap_or("réponse illisible"));
                text.to_string()
            }
        }
    }
}

/// Extrait le moteur (`grammalecte/…`) de l'extension, en refusant les chemins qui sortiraient du dossier.
fn extract(xpi: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(xpi)).map_err(|e| e.to_string())?;
    let tmp = dest.with_extension("part");
    let _ = std::fs::remove_dir_all(&tmp);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else { return Err(format!("archive invalide ({})", entry.name())) };
        let keep = name.starts_with("grammalecte")
            && !entry.is_dir()
            && !SKIP.iter().any(|s| name.to_string_lossy().contains(s));
        if !keep {
            continue;
        }
        let out = tmp.join(&name);
        std::fs::create_dir_all(out.parent().unwrap_or(&tmp)).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        std::fs::write(&out, data).map_err(|e| e.to_string())?;
    }
    if !tmp.join("grammalecte/fr/gc_engine.js").is_file() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("archive inattendue (moteur absent)".into());
    }
    let _ = std::fs::remove_dir_all(dest);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn xpi(files: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, body) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_only_the_engine() {
        let dest = std::env::temp_dir().join(format!("murmure-gc-{}", std::process::id())).join(VERSION);
        let data = xpi(&[
            ("manifest.json", "{}"),
            ("grammalecte/fr/gc_engine.js", "var gc_engine = {};"),
            ("grammalecte/graphspell/_dictionaries/fr-allvars.json", "{}"),
            ("grammalecte/graphspell/_dictionaries/fr-classic.json", "{}"),
            ("grammalecte/fr/thesaurus1_data.json", "{}"),
        ]);
        extract(&data, &dest).unwrap();
        assert!(dest.join("grammalecte/fr/gc_engine.js").is_file());
        assert!(dest.join("grammalecte/graphspell/_dictionaries/fr-allvars.json").is_file());
        assert!(!dest.join("grammalecte/graphspell/_dictionaries/fr-classic.json").exists());
        assert!(!dest.join("grammalecte/fr/thesaurus1_data.json").exists());
        assert!(!dest.join("manifest.json").exists());
        let _ = std::fs::remove_dir_all(dest.parent().unwrap());
    }

    #[test]
    fn refuses_unexpected_archives() {
        let dest = std::env::temp_dir().join(format!("murmure-gc-bad-{}", std::process::id())).join(VERSION);
        assert!(extract(&xpi(&[("manifest.json", "{}")]), &dest).is_err());
        assert!(extract(b"pas un zip", &dest).is_err());
        assert!(!dest.exists());
    }
}
