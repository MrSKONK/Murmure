//! Mises à jour depuis GitHub Releases : même dépôt et même installateur que la version 1.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use murmure_core::release::{is_newer, pick_release, Release, RELEASES_REPO};
use sha2::{Digest, Sha256};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("Murmure/{VERSION}"))
        .try_proxy_from_env(true)
        .build()
}

/// La release plus récente que la version installée, sinon `None`.
pub fn check() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{RELEASES_REPO}/releases/latest");
    let resp = agent().get(&url).set("Accept", "application/vnd.github+json").call().map_err(|e| e.to_string())?;
    let data: serde_json::Value = serde_json::from_reader(resp.into_reader()).map_err(|e| e.to_string())?;
    Ok(pick_release(&data).filter(|r| is_newer(&r.version, VERSION)))
}

/// Télécharge l'installateur dans le dossier temporaire et vérifie son empreinte SHA-256.
pub fn download(release: &Release, mut progress: impl FnMut(f32)) -> Result<PathBuf, String> {
    let mut digest_text = String::new();
    agent()
        .get(&release.sha256_url)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader()
        .take(4096)
        .read_to_string(&mut digest_text)
        .map_err(|e| e.to_string())?;
    let expected = digest_text.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("empreinte de la mise à jour illisible".into());
    }
    let path = std::env::temp_dir().join(format!("Murmure-Setup-{}.exe", release.version));
    let part = path.with_extension("exe.part");
    let resp = agent().get(&release.url).call().map_err(|e| e.to_string())?;
    let total = resp.header("content-length").and_then(|v| v.parse().ok()).unwrap_or(release.size);
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if total > 0 {
            progress(done as f32 / total as f32);
        }
    }
    drop(file);
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if digest != expected {
        let _ = std::fs::remove_file(&part);
        return Err("le fichier téléchargé est corrompu (empreinte différente)".into());
    }
    std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Installe en silence ; l'installateur ferme Murmure puis le relance une fois terminé.
pub fn launch_installer(path: &std::path::Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new(path);
    cmd.args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0008 | 0x0000_0200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}
