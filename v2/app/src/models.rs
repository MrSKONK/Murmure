//! Modèles whisper.cpp (format ggml, quantifiés) : téléchargement depuis Hugging Face, vérifié par SHA-256.
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::paths;

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// (réglage, fichier). Versions quantifiées : 2 à 3 fois plus légères, qualité quasi identique.
const FILES: &[(&str, &str)] = &[
    ("tiny", "ggml-tiny.bin"),
    ("base", "ggml-base-q5_1.bin"),
    ("small", "ggml-small-q5_1.bin"),
    ("medium", "ggml-medium-q5_0.bin"),
    ("large-v3-turbo", "ggml-large-v3-turbo-q5_0.bin"),
    ("large-v3", "ggml-large-v3-q5_0.bin"),
];

pub fn file_name(model: &str) -> &'static str {
    FILES.iter().find(|(id, _)| *id == model).map_or("ggml-medium-q5_0.bin", |(_, f)| f)
}

pub fn path(model: &str) -> PathBuf {
    paths::models_dir().join(file_name(model))
}

fn agent(redirects: u32) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(redirects)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(60))
        .user_agent(concat!("Murmure/", env!("CARGO_PKG_VERSION")))
        .try_proxy_from_env(true)
        .build()
}

/// Empreinte et taille annoncées par Hugging Face pour un fichier LFS (avant la redirection vers le stockage).
fn expected(url: &str) -> (Option<String>, Option<u64>) {
    let Ok(resp) = agent(0).head(url).call() else { return (None, None) };
    let sha = resp
        .header("x-linked-etag")
        .map(|v| v.trim_matches('"').to_ascii_lowercase())
        .filter(|v| v.len() == 64 && v.chars().all(|c| c.is_ascii_hexdigit()));
    let size = resp.header("x-linked-size").and_then(|v| v.parse().ok());
    (sha, size)
}

/// Télécharge le modèle s'il manque. `progress` reçoit l'avancement entre 0 et 1.
pub fn ensure(model: &str, mut progress: impl FnMut(f32)) -> Result<PathBuf, String> {
    let dest = path(model);
    if dest.is_file() {
        return Ok(dest);
    }
    std::fs::create_dir_all(paths::models_dir()).map_err(|e| e.to_string())?;
    let url = format!("{BASE_URL}/{}", file_name(model));
    let (sha, size) = expected(&url);
    let resp = agent(5).get(&url).call().map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("téléchargement refusé par huggingface.co (erreur {code})"),
        ureq::Error::Transport(_) => "pas de connexion à huggingface.co, vérifiez Internet puis réessayez".into(),
    })?;
    let total = size.or_else(|| resp.header("content-length").and_then(|v| v.parse().ok())).unwrap_or(0);
    let part = dest.with_extension("bin.part");
    let mut file = File::create(&part).map_err(|e| e.to_string())?;
    let mut reader = resp.into_reader();
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let (mut done, mut shown) = (0u64, 0.0f32);
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("téléchargement interrompu : {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if total > 0 {
            let p = done as f32 / total as f32;
            if p - shown >= 0.01 {
                shown = p;
                progress(p.min(1.0));
            }
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let bad = sha.as_ref().is_some_and(|s| *s != digest) || size.is_some_and(|s| s != done);
    if bad || done == 0 {
        let _ = std::fs::remove_file(&part);
        return Err("fichier du modèle corrompu (empreinte différente), réessayez".into());
    }
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}
