//! « Cerveau » de Murmure : apprend de ses erreurs à partir des corrections faites dans l'historique.
//!
//! Quand on corrige une dictée, on compare le texte d'origine à la version corrigée mot à mot ; chaque endroit
//! corrigé devient une paire « entendu → voulu » ajoutée au dictionnaire personnel. Le dictionnaire s'occupe
//! ensuite du reste : remplacement, vocabulaire donné à Whisper, casse conservée.
//!
//! Seules les corrections de l'utilisateur servent : l'historique brut contient les erreurs du modèle, les
//! apprendre les renforcerait.
use std::collections::HashSet;

use crate::settings::DictEntry;

/// Au-delà, c'est une reformulation et non une erreur de reconnaissance : rien n'est appris.
const MAX_WORDS: usize = 4;
/// Distance d'édition maximale (en part de la longueur) entre ce qui a été entendu et ce qui est voulu.
const MAX_DISTANCE: f64 = 0.7;
/// Textes plus longs : comparaison abandonnée (le tableau de comparaison grossit au carré).
const MAX_TEXT_WORDS: usize = 600;
/// Correction automatique : écart maximal (en part de la longueur) pour reconnaître un mot du dictionnaire...
const AUTO_DISTANCE: f64 = 0.34;
/// ... et longueur minimale (lettres) du mot du dictionnaire comme de ce qui a été entendu.
const AUTO_MIN_LEN: usize = 4;

/// Mots du texte, sans la ponctuation autour ; `true` quand le mot ouvre une phrase.
fn words(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut starts = true;
    for raw in text.split_whitespace() {
        let word = raw.trim_matches(|c: char| !c.is_alphanumeric());
        if !word.is_empty() {
            out.push((word.to_string(), starts));
        }
        starts = raw.ends_with(['.', '!', '?', '…']);
    }
    out
}

fn compact(text: &str) -> Vec<char> {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Distance d'édition rapportée à la longueur du plus long des deux.
fn distance(a: &str, b: &str) -> f64 {
    let (a, b) = (compact(a), compact(b));
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 0.0;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()] as f64 / longest as f64
}

/// Un nom propre, une marque, un sigle : le remplacer partout est sans risque (« ZeroTier », « Claude »).
/// Un mot ordinaire (« roux » → « roue ») ne l'est pas : il faut le garder dans son contexte.
fn distinctive(term: &str, starts_sentence: bool) -> bool {
    let first_upper = term.chars().next().is_some_and(char::is_uppercase);
    let inner_upper = term.chars().skip(1).any(char::is_uppercase);
    inner_upper || term.chars().any(|c| c.is_ascii_digit()) || (first_upper && !starts_sentence)
}

/// Paires « entendu → voulu » déduites de la différence entre le texte d'origine et sa correction.
pub fn extract(original: &str, corrected: &str) -> Vec<(String, String)> {
    let (a, b) = (words(original), words(corrected));
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n > MAX_TEXT_WORDS || m > MAX_TEXT_WORDS {
        return Vec::new();
    }
    // lcs[i][j] : longueur de la plus longue suite de mots commune à a[i..] et b[j..]
    let mut lcs = vec![vec![0u16; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i].0 == b[j].0 { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    // plages (a, b) qui diffèrent, séparées par au moins un mot identique
    let mut hunks = Vec::new();
    let (mut i, mut j, mut open): (usize, usize, Option<(usize, usize)>) = (0, 0, None);
    while i < n || j < m {
        if i < n && j < m && a[i].0 == b[j].0 {
            if let Some((si, sj)) = open.take() {
                hunks.push((si..i, sj..j));
            }
            i += 1;
            j += 1;
            continue;
        }
        open.get_or_insert((i, j));
        if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    if let Some((si, sj)) = open {
        hunks.push((si..n, sj..m));
    }

    let mut pairs = Vec::new();
    for (ra, rb) in hunks {
        if ra.is_empty() || rb.is_empty() || ra.len() > MAX_WORDS || rb.len() > MAX_WORDS {
            continue; // simple ajout ou retrait : pas une confusion entre deux mots
        }
        let join = |w: &[(String, bool)]| w.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" ");
        let (mut heard, mut term) = (join(&a[ra.clone()]), join(&b[rb.clone()]));
        if heard.chars().count() < 3 || distance(&heard, &term) > MAX_DISTANCE {
            continue;
        }
        if !distinctive(&term, b[rb.start].1) {
            // mot ordinaire : appris avec le mot qui suit (ou qui précède), pour ne pas le changer partout
            let Some(next) = a.get(ra.end).or_else(|| ra.start.checked_sub(1).and_then(|k| a.get(k))) else {
                continue;
            };
            if ra.end < n {
                heard = format!("{heard} {}", next.0);
                term = format!("{term} {}", next.0);
            } else {
                heard = format!("{} {heard}", next.0);
                term = format!("{} {term}", next.0);
            }
        }
        pairs.push((heard, term));
    }
    pairs
}

/// Ajoute les paires au dictionnaire ; rend celles qui sont réellement nouvelles.
pub fn merge(dictionary: &mut Vec<DictEntry>, pairs: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut learned = Vec::new();
    for (heard, term) in pairs {
        let known = |h: &str| h.trim().to_lowercase() == heard.to_lowercase();
        if dictionary.iter().any(|e| e.term == term && e.heard.split(',').any(known)) {
            continue;
        }
        // une même expression entendue ne mène qu'à un seul mot voulu : la dernière correction l'emporte
        for entry in dictionary.iter_mut().filter(|e| e.term != term) {
            let kept: Vec<&str> = entry.heard.split(',').map(str::trim).filter(|h| !h.is_empty() && !known(h)).collect();
            entry.heard = kept.join(", ");
        }
        match dictionary.iter_mut().find(|e| e.term == term) {
            Some(entry) if entry.heard.trim().is_empty() => entry.heard = heard.clone(),
            Some(entry) => entry.heard = format!("{}, {heard}", entry.heard.trim()),
            None => dictionary.push(DictEntry { term: term.clone(), heard: heard.clone() }),
        }
        learned.push((heard, term));
    }
    learned
}

/// Mots du texte (position de début et de fin) ; une apostrophe ou un tiret entre deux lettres reste dans le mot.
fn spans(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < chars.len() {
        if !chars[k].1.is_alphanumeric() {
            k += 1;
            continue;
        }
        let mut last = k;
        while let Some(&(_, c)) = chars.get(last + 1) {
            let inner = matches!(c, '\'' | '’' | '-') && chars.get(last + 2).is_some_and(|n| n.1.is_alphanumeric());
            if !(c.is_alphanumeric() || inner) {
                break;
            }
            last += 1;
        }
        out.push((chars[k].0, chars[last].0 + chars[last].1.len_utf8()));
        k = last + 1;
    }
    out
}

/// Correction automatique : un mot (ou une suite de mots) dont Whisper doutait, et qui ressemble à un mot du
/// dictionnaire, est remplacé par ce mot. Whisper en doute justement quand il se trompe (noms propres, mots rares) ;
/// un mot dont il est sûr n'est jamais touché, ce qui évite de changer « route » en « Roue » sans raison.
/// Rend le texte corrigé et les remplacements faits (entendu, voulu).
pub fn auto_correct(text: &str, doubts: &[String], dictionary: &[DictEntry]) -> (String, Vec<(String, String)>) {
    let doubtful: HashSet<String> = doubts.iter().map(|d| d.to_lowercase()).collect();
    let terms: Vec<&str> =
        dictionary.iter().map(|e| e.term.trim()).filter(|t| compact(t).len() >= AUTO_MIN_LEN).collect();
    if doubtful.is_empty() || terms.is_empty() {
        return (text.to_string(), Vec::new());
    }
    let known: HashSet<String> = terms.iter().map(|t| t.to_lowercase()).collect();
    let spans = spans(text);
    let (mut out, mut fixes) = (String::new(), Vec::new());
    let (mut copied, mut w) = (0, 0);
    while w < spans.len() {
        let mut best: Option<(usize, &str, f64)> = None;
        for term in &terms {
            let n = term.split_whitespace().count();
            if n == 0 || n > MAX_WORDS || w + n > spans.len() {
                continue;
            }
            let window = &text[spans[w].0..spans[w + n - 1].1];
            let one_word = |k: usize| text[spans[k].0..spans[k].1].to_lowercase();
            let joined = (w..w + n - 1).all(|k| text[spans[k].1..spans[k + 1].0].chars().all(char::is_whitespace));
            let lower = window.to_lowercase();
            if !joined
                || window == *term
                || (known.contains(&lower) && lower != term.to_lowercase()) // autre mot du dictionnaire : intact
                || !(w..w + n).any(|k| doubtful.contains(&one_word(k)))
                || compact(window).len() < AUTO_MIN_LEN
            {
                continue;
            }
            let d = distance(window, term);
            if d <= AUTO_DISTANCE && best.is_none_or(|b| d < b.2) {
                best = Some((n, term, d));
            }
        }
        match best {
            Some((n, term, _)) => {
                let end = spans[w + n - 1].1;
                out.push_str(&text[copied..spans[w].0]);
                out.push_str(term);
                fixes.push((text[spans[w].0..end].to_string(), term.to_string()));
                copied = end;
                w += n;
            }
            None => w += 1,
        }
    }
    out.push_str(&text[copied..]);
    (out, fixes)
}

/// Apprend d'une correction : met le dictionnaire à jour et rend ce qui a été appris.
pub fn learn(original: &str, corrected: &str, dictionary: &mut Vec<DictEntry>) -> Vec<(String, String)> {
    merge(dictionary, extract(original, corrected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary;

    fn pairs(a: &str, b: &str) -> Vec<(String, String)> {
        extract(a, b)
    }

    #[test]
    fn proper_noun_is_learned_everywhere() {
        assert_eq!(
            pairs("Je lance Cloud Code ce soir.", "Je lance Claude Code ce soir."),
            vec![("Cloud".to_string(), "Claude".to_string())]
        );
        assert_eq!(pairs("On dit Reboot natin.", "On dit Reboot Nathan."), vec![("natin".into(), "Nathan".into())]);
    }

    #[test]
    fn ordinary_word_is_kept_in_context() {
        assert_eq!(
            pairs("la roux de la chance tourne", "la roue de la chance tourne"),
            vec![("roux de".to_string(), "roue de".to_string())]
        );
        // jamais « roux » → « roue » partout
        let mut d = Vec::new();
        learn("la roux de la chance", "la roue de la chance", &mut d);
        assert_eq!(dictionary::apply("des cheveux roux", &d), "des cheveux roux");
        assert_eq!(dictionary::apply("la roux de la chance", &d), "la roue de la chance");
    }

    #[test]
    fn rewrites_and_tiny_changes_teach_nothing() {
        assert!(pairs("le gros chat dort", "un petit chien mange vraiment bien").is_empty());
        assert!(pairs("il a mangé", "il a mangé beaucoup").is_empty()); // ajout seul
        assert!(pairs("bonjour à tous", "bonjour à tous").is_empty());
        assert!(pairs("a b", "à b").is_empty()); // mot de moins de 3 lettres
    }

    fn dict(terms: &[&str]) -> Vec<DictEntry> {
        terms.iter().map(|t| DictEntry { term: (*t).into(), heard: String::new() }).collect()
    }

    fn doubts(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_string()).collect()
    }

    #[test]
    fn doubtful_word_close_to_a_known_term_is_fixed() {
        let (text, fixes) = auto_correct("On lance Airbium ce soir.", &doubts(&["Airbium"]), &dict(&["Erbium"]));
        assert_eq!(text, "On lance Erbium ce soir.");
        assert_eq!(fixes, vec![("Airbium".to_string(), "Erbium".to_string())]);
        let (text, _) = auto_correct("Je lance Cloud Code.", &doubts(&["Cloud"]), &dict(&["Claude Code"]));
        assert_eq!(text, "Je lance Claude Code.");
        let (text, _) = auto_correct("sur zerotier", &doubts(&["zerotier"]), &dict(&["ZeroTier"]));
        assert_eq!(text, "sur ZeroTier"); // casse du dictionnaire
    }

    #[test]
    fn confident_words_are_never_touched() {
        // Whisper est sûr de « route » : même proche de « Roue », on n'y touche pas
        let (text, fixes) = auto_correct("la route est longue", &doubts(&["longue"]), &dict(&["Roue", "Routeur"]));
        assert_eq!(text, "la route est longue");
        assert!(fixes.is_empty());
        let (text, _) = auto_correct("on lance Airbium", &[], &dict(&["Erbium"]));
        assert_eq!(text, "on lance Airbium");
    }

    #[test]
    fn auto_correct_keeps_punctuation_and_other_words() {
        let (text, _) = auto_correct("Oui, « Airbium » !\nEt Airbium.", &doubts(&["airbium"]), &dict(&["Erbium"]));
        assert_eq!(text, "Oui, « Erbium » !\nEt Erbium.");
        let (text, fixes) = auto_correct("rien à changer ici", &doubts(&["changer"]), &dict(&["Erbium"]));
        assert_eq!(text, "rien à changer ici");
        assert!(fixes.is_empty());
    }

    #[test]
    fn dictionary_is_merged_without_duplicates() {
        let mut d = vec![DictEntry { term: "Claude".into(), heard: "Cloud".into() }];
        assert!(learn("Je lance Cloud Code", "Je lance Claude Code", &mut d).is_empty()); // déjà connu
        let learned = learn("Je lance Clode Code", "Je lance Claude Code", &mut d);
        assert_eq!(learned, vec![("Clode".to_string(), "Claude".to_string())]);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].heard, "Cloud, Clode");
        // une expression entendue ne pointe que vers un seul mot
        learn("un Cloud ici", "un Clod ici", &mut d);
        assert_eq!(d[0].heard, "Clode");
        assert_eq!(d.len(), 2);
    }
}
