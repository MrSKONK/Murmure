//! Dictionnaire personnel : vocabulaire donné à Whisper et remplacement de ce qu'il entend souvent de travers.
use fancy_regex::{NoExpand, Regex};

use crate::settings::DictEntry;

fn terms(entries: &[DictEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.term.trim()).filter(|t| !t.is_empty()).collect()
}

/// Vocabulaire passé à Whisper pour qu'il orthographie correctement ces mots.
pub fn initial_prompt(entries: &[DictEntry]) -> Option<String> {
    let terms = terms(entries);
    (!terms.is_empty()).then(|| format!("Vocabulaire : {}.", terms.join(", ")))
}

/// Mots dont la casse doit être gardée telle quelle.
pub fn keep_case(entries: &[DictEntry]) -> Vec<String> {
    terms(entries).into_iter().map(str::to_string).collect()
}

/// Remplace ce que Whisper entend habituellement (champ « heard ») par le mot voulu.
pub fn apply(text: &str, entries: &[DictEntry]) -> String {
    let mut text = text.to_string();
    for entry in entries {
        let term = entry.term.trim();
        if term.is_empty() {
            continue;
        }
        for heard in entry.heard.split(',').map(str::trim).filter(|h| !h.is_empty()) {
            let pattern = format!(r"(?i)(?<!\w){}(?!\w)", fancy_regex::escape(heard));
            if let Ok(regex) = Regex::new(&pattern) {
                text = regex.replace_all(&text, NoExpand(term)).into_owned();
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(term: &str, heard: &str) -> DictEntry {
        DictEntry { term: term.into(), heard: heard.into() }
    }

    #[test]
    fn replaces_heard_words() {
        let d = [entry("Kubernetes", "cuber nette, cuber nétesse"), entry("", "x")];
        assert_eq!(apply("J'utilise Cuber nette et cuber nétesse.", &d), "J'utilise Kubernetes et Kubernetes.");
        assert_eq!(apply("Le cuber nettement", &d), "Le cuber nettement");
        assert_eq!(initial_prompt(&d).as_deref(), Some("Vocabulaire : Kubernetes."));
        assert_eq!(initial_prompt(&[]), None);
    }

    #[test]
    fn replacement_is_literal() {
        assert_eq!(apply("prix", &[entry("$1 €", "prix")]), "$1 €");
    }
}
