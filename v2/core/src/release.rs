//! Choix de la mise à jour dans une réponse GitHub « releases/latest » (même règles que la version 1).
use serde_json::Value;

pub const RELEASES_REPO: &str = "MrSKONK/Murmure";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub size: u64,
    pub sha256_url: String,
}

pub fn parse_version(text: &str) -> Vec<u64> {
    let mut nums = Vec::new();
    let mut current: Option<u64> = None;
    for ch in text.chars().chain(std::iter::once(' ')) {
        if let Some(d) = ch.to_digit(10) {
            current = Some(current.unwrap_or(0).saturating_mul(10).saturating_add(u64::from(d)));
        } else if let Some(n) = current.take() {
            nums.push(n);
        }
    }
    nums.truncate(4);
    if nums.is_empty() {
        nums.push(0);
    }
    nums
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

/// Version de « Murmure-Setup-1.2.0.exe ».
fn setup_version(name: &str) -> Option<&str> {
    let version = name.strip_prefix("Murmure-Setup-")?.strip_suffix(".exe")?;
    let parts: Vec<&str> = version.split('.').collect();
    let ok =
        (2..=4).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    ok.then_some(version)
}

/// L'installateur et son empreinte, ou `None` (brouillon, pré-version, fichier absent ou hébergé ailleurs).
pub fn pick_release(data: &Value) -> Option<Release> {
    let truthy = |k: &str| data.get(k).and_then(Value::as_bool).unwrap_or(false);
    if !data.is_object() || truthy("draft") || truthy("prerelease") {
        return None;
    }
    let assets: Vec<&Value> = data.get("assets")?.as_array()?.iter().filter(|a| a.is_object()).collect();
    let name = |a: &Value| a.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let url = |a: &Value| a.get("browser_download_url").and_then(Value::as_str).unwrap_or("").to_string();
    for asset in &assets {
        let asset_name = name(asset);
        let Some(version) = setup_version(&asset_name) else { continue };
        if !url(asset).starts_with("https://github.com/") {
            continue;
        }
        let digest = assets.iter().find(|a| name(a) == format!("{asset_name}.sha256"))?; // sans empreinte, rien
        return Some(Release {
            version: version.to_string(),
            notes: data.get("body").and_then(Value::as_str).unwrap_or("").trim().to_string(),
            url: url(asset),
            size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
            sha256_url: url(digest),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn release(names: &[&str]) -> Value {
        let assets: Vec<Value> = names
            .iter()
            .map(|n| {
                json!({"name": n, "size": 10,
                "browser_download_url": format!("https://github.com/MrSKONK/Murmure/releases/download/v1.2.0/{n}")})
            })
            .collect();
        json!({"tag_name": "v1.2.0", "body": "Nouveautés", "assets": assets})
    }

    #[test]
    fn compare_versions() {
        assert!(is_newer("1.2.0", "1.1.9"));
        assert!(is_newer("v1.10.0", "1.9.3"));
        assert!(is_newer("2.0.0", "1.0.2"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("1.0.0", "1.2.0"));
    }

    #[test]
    fn installer_with_digest() {
        let r = pick_release(&release(&["Murmure-Setup-1.2.0.exe", "Murmure-Setup-1.2.0.exe.sha256"])).unwrap();
        assert_eq!(r.version, "1.2.0");
        assert!(r.url.ends_with("Murmure-Setup-1.2.0.exe"));
        assert!(r.sha256_url.ends_with(".sha256"));
        assert_eq!(r.notes, "Nouveautés");
    }

    #[test]
    fn refused_cases() {
        let names = ["Murmure-Setup-1.2.0.exe", "Murmure-Setup-1.2.0.exe.sha256"];
        assert!(pick_release(&release(&names[..1])).is_none());
        let mut draft = release(&names);
        draft["draft"] = json!(true);
        assert!(pick_release(&draft).is_none());
        let mut pre = release(&names);
        pre["prerelease"] = json!(true);
        assert!(pick_release(&pre).is_none());
        let mut evil = release(&names);
        evil["assets"][0]["browser_download_url"] = json!("https://evil.example/Murmure-Setup-1.2.0.exe");
        assert!(pick_release(&evil).is_none());
        assert!(pick_release(&release(&["notes.txt", "Murmure-portable.zip", "Murmure-Setup-1.exe"])).is_none());
        assert!(pick_release(&json!({"message": "Not Found"})).is_none());
    }
}
