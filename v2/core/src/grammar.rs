//! Choix des corrections de Grammalecte, mêmes règles que la version 1 (`src/grammar.py`).
//!
//! Whisper entend bien les sons mais se trompe d'orthographe : on ne garde que les remplacements qui se prononcent
//! comme le mot d'origine (« mange » -> « mangent », « a » -> « à »), avec une seule suggestion, dans les catégories
//! accords, conjugaisons et confusions. Quand Grammalecte propose deux corrections alternatives (« Ils mange » :
//! « mangent » ou « Il »), une seule est retenue ; quand il se contredit, on n'y touche pas.
use std::collections::HashSet;
use std::sync::LazyLock;

use fancy_regex::Regex;
use serde::Deserialize;

/// Une erreur relevée par Grammalecte (positions en caractères).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarError {
    #[serde(rename = "nStart")]
    pub start: i64,
    #[serde(rename = "nEnd")]
    pub end: i64,
    #[serde(rename = "aSuggestions", default)]
    pub suggestions: Vec<String>,
    #[serde(rename = "sType", default)]
    pub kind: String,
    #[serde(rename = "sRuleId", default)]
    pub rule: String,
    #[serde(rename = "sMessage", default)]
    pub message: String,
    /// pour chaque suggestion : homophone selon les données phonétiques de Grammalecte
    #[serde(rename = "aSimilar", default)]
    pub similar: Vec<bool>,
}

/// Types d'erreurs pouvant être corrigés automatiquement.
pub const SAFE_TYPES: &[&str] = &["gn", "conj", "ppas", "infi", "vmode", "conf", "imp", "inte", "tu"];

fn re(p: &str) -> Regex {
    Regex::new(p).expect("expression valide")
}

static ALT_SUFFIX: LazyLock<Regex> = LazyLock::new(|| re(r"_a\d+_\d+$"));
static CITED: LazyLock<Regex> = LazyLock::new(|| re(r"“([^”]+)”"));

/// « g2__conj_ils__b1_a1_1 » et « g2__conj_ils__b1_a2_1 » sont deux alternatives de la même erreur.
fn group(rule: &str) -> String {
    ALT_SUFFIX.replace(rule, "").into_owned()
}

/// Remplacements à faire : (début, fin, remplacement) en caractères, sans chevauchement.
/// `phonetics` : faux pour ignorer le test « se prononce pareil » (on ne tranche alors pas entre alternatives).
pub fn select_corrections(text: &str, errors: &[GrammarError], phonetics: bool) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut groups: Vec<(String, Vec<&GrammarError>)> = Vec::new();
    for (i, e) in errors.iter().enumerate() {
        let key = match group(&e.rule) {
            k if k.is_empty() => format!("#{i}"),
            k => k,
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, alts)) => alts.push(e),
            None => groups.push((key, vec![e])),
        }
    }
    // Mots sur lesquels s'appuient les autres erreurs (« Accord avec la forme verbale “sont” ») : si une règle veut
    // changer ce mot alors qu'une autre le tient pour juste, Grammalecte se contredit, on n'y touche pas.
    let cited: Vec<HashSet<String>> = groups
        .iter()
        .map(|(_, alts)| {
            alts.iter()
                .flat_map(|e| CITED.captures_iter(&e.message).filter_map(Result::ok).collect::<Vec<_>>())
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_lowercase()))
                .collect()
        })
        .collect();

    let curly = text.contains('’');
    let mut picked = Vec::new();
    for (gi, (_, alternatives)) in groups.iter().enumerate() {
        let others: HashSet<&String> =
            cited.iter().enumerate().filter(|(i, _)| *i != gi).flat_map(|(_, w)| w).collect();
        for e in alternatives {
            let ok_offsets = 0 <= e.start && e.start < e.end && e.end as usize <= chars.len();
            if !SAFE_TYPES.contains(&e.kind.as_str()) || e.suggestions.len() != 1 || !ok_offsets {
                continue;
            }
            let (start, end) = (e.start as usize, e.end as usize);
            let word: String = chars[start..end].iter().collect();
            if others.contains(&word.to_lowercase()) {
                continue;
            }
            let repl = if curly { e.suggestions[0].clone() } else { e.suggestions[0].replace('’', "'") };
            if phonetics {
                let similar = e.similar.first().copied().unwrap_or(false);
                if !(similar || sound_key(&word) == sound_key(&repl)) {
                    continue;
                }
            } else if alternatives.len() > 1 {
                break; // sans test phonétique, on ne tranche pas entre deux alternatives
            }
            picked.push((start, end, repl));
            break; // une seule correction par erreur
        }
    }
    picked.sort();
    let mut result: Vec<(usize, usize, String)> = Vec::new();
    for p in picked {
        if result.last().is_none_or(|last| p.0 >= last.1) {
            result.push(p);
        }
    }
    result
}

pub fn apply_corrections(text: &str, errors: &[GrammarError], phonetics: bool) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    for (start, end, repl) in select_corrections(text, errors, phonetics).into_iter().rev() {
        chars.splice(start..end, repl.chars());
    }
    chars.into_iter().collect()
}

const ACCENTS_FROM: &str = "àâäéèêëîïôöûùüÿç";
const ACCENTS_TO: &str = "aaaeeeeiioouuuys";
static E_SOUND: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?:aient|ais|ait|ai|ées|ée|és|é|er|ez)$|(?<=^[a-z])es$|(?<=^[cs])est$|^(?:es|est|et)$"));
static SOFT_C: LazyLock<Regex> = LazyLock::new(|| re(r"c(?=[eiyÉ])"));
static ENT: LazyLock<Regex> = LazyLock::new(|| re(r"ent$"));
static SILENT: LazyLock<Regex> = LazyLock::new(|| re(r"[sxtdz]+$"));
static MUTE_E: LazyLock<Regex> = LazyLock::new(|| re(r"(?<=.)e$"));
static DOUBLE: LazyLock<Regex> = LazyLock::new(|| re(r"(.)\1+"));

/// Prononciation approximative d'un mot français, pour comparer des formes fléchies :
/// « mange » = « mangent », « rouge » = « rouges », « manger » = « mangé », « Ces » = « C'est ».
pub fn sound_key(word: &str) -> String {
    let w = word.to_lowercase().replace(['’', '\'', '-'], "");
    let w = E_SOUND.replace_all(&w, "É").into_owned(); // terminaisons prononcées « é » / « è »
    let w: String = w
        .chars()
        .map(|c| ACCENTS_FROM.chars().position(|a| a == c).map_or(c, |i| ACCENTS_TO.chars().nth(i).unwrap()))
        .collect();
    let mut w = SOFT_C.replace_all(&w, "s").into_owned(); // « ce » / « se », « ça » / « sa »
    if w.chars().count() > 3 {
        w = ENT.replace(&w, "").into_owned(); // « mangent » -> « mang »
    }
    let w = SILENT.replace(&w, "").into_owned(); // consonnes finales muettes
    let w = MUTE_E.replace(&w, "").into_owned(); // e muet final
    DOUBLE.replace_all(&w, "$1").into_owned() // consonnes doublées
}

#[cfg(test)]
mod tests {
    //! Erreurs réelles de Grammalecte 2.3.0, mêmes cas que `tests/test_grammar.py`.
    use super::*;

    fn err(text: &str, span: &str, sugg: &[&str], kind: &str, rule: &str, nth: usize, msg: &str) -> GrammarError {
        let chars: Vec<char> = text.chars().collect();
        let span_chars: Vec<char> = span.chars().collect();
        let starts: Vec<usize> = (0..=chars.len().saturating_sub(span_chars.len()))
            .filter(|&i| chars[i..i + span_chars.len()] == span_chars[..])
            .collect();
        let start = starts[nth];
        GrammarError {
            start: start as i64,
            end: (start + span_chars.len()) as i64,
            suggestions: sugg.iter().map(|s| s.to_string()).collect(),
            kind: kind.into(),
            rule: rule.into(),
            message: msg.into(),
            similar: vec![false; sugg.len()], // clé phonétique seule, comme les tests Python
        }
    }

    #[test]
    fn homophones() {
        for (a, b) in [
            ("mange", "mangent"),
            ("rouge", "rouges"),
            ("enfant", "enfants"),
            ("garé", "garés"),
            ("manger", "mangé"),
            ("Elle", "Elles"),
            ("Ils", "Il"),
            ("a", "as"),
            ("a", "à"),
            ("Ou", "Où"),
            ("Ces", "C’est"),
            ("sa", "ça"),
            ("c'est", "s'est"),
            ("dise", "dises"),
            ("vert", "verre"),
            ("les", "lé"),
            ("ses", "ces"),
        ] {
            assert_eq!(sound_key(a), sound_key(b), "{a} / {b}");
        }
    }

    #[test]
    fn different_sounds() {
        for (a, b) in [("sont", "est"), ("de", "des"), ("le", "les"), ("elle", "est"), ("test", "tes")] {
            assert_ne!(sound_key(a), sound_key(b), "{a} / {b}");
        }
    }

    #[test]
    fn simple_homophone_fixes() {
        let t = "Les voitures rouge sont garé devant chez moi.";
        let errors = [
            err(t, "rouge", &["rouges"], "gn", "g3__gn_les_2m__b1_a4_1", 0, ""),
            err(t, "garé", &["garés"], "ppas", "gv1__ppas_être_accord_plur__b1_a1_1", 0, ""),
        ];
        assert_eq!(apply_corrections(t, &errors, true), "Les voitures rouges sont garés devant chez moi.");
    }

    #[test]
    fn alternatives_only_one_applied() {
        let t = "Ils mange des pommes.";
        let errors = [
            err(t, "mange", &["mangent"], "conj", "g2__conj_ils__b1_a1_1", 0, ""),
            err(t, "Ils", &["Il"], "conj", "g2__conj_ils__b1_a2_1", 0, ""),
        ];
        assert_eq!(apply_corrections(t, &errors, true), "Ils mangent des pommes.");
    }

    #[test]
    fn alternative_chosen_by_sound() {
        let t = "Elle se sont trompés de route.";
        let errors = [
            err(t, "sont", &["est"], "conj", "gv2__conj_elle__b1_a1_1", 0, ""),
            err(t, "Elle", &["Elles"], "conj", "gv2__conj_elle__b1_a2_1", 0, ""),
        ];
        assert_eq!(apply_corrections(t, &errors, true), "Elles se sont trompés de route.");
    }

    #[test]
    fn refused_cases() {
        let t = "Elle se sont vues.";
        assert_eq!(apply_corrections(t, &[err(t, "sont", &["est"], "conj", "r", 0, "")], true), t);
        let t = "Je suis aller au marché.";
        assert_eq!(apply_corrections(t, &[err(t, "aller", &["allé", "allée"], "ppas", "r", 0, "")], true), t);
        let t = "C'est quoi ?";
        let errors = [
            err(t, "C'", &["C’"], "apos", "apostrophe_typographique", 0, ""),
            err(t, " ?", &[" ?"], "nbsp", "nbsp_avant_double_ponctuation", 0, ""),
        ];
        assert_eq!(apply_corrections(t, &errors, true), t);
    }

    #[test]
    fn straight_apostrophe_kept() {
        let t = "Ces un bon moyen.";
        assert_eq!(apply_corrections(t, &[err(t, "Ces", &["C’est"], "conf", "r", 0, "")], true), "C'est un bon moyen.");
    }

    #[test]
    fn grammalecte_phonetics_accepted() {
        // « a » -> « à » : homophone selon Grammalecte même si la clé phonétique ne suffisait pas
        let t = "il va a Paris";
        let mut e = err(t, "a", &["à"], "conf", "g3__conf_à_a__b3_a1_1", 1, "");
        e.similar = vec![true];
        assert_eq!(apply_corrections(t, &[e], true), "il va à Paris");
    }

    #[test]
    fn without_phonetics_alternatives_skipped() {
        let t = "Ils mange des pommes et il va a Paris.";
        let errors = [
            err(t, "mange", &["mangent"], "conj", "g2__conj_ils__b1_a1_1", 0, ""),
            err(t, "Ils", &["Il"], "conj", "g2__conj_ils__b1_a2_1", 0, ""),
            err(t, "a Paris", &["à Paris"], "conf", "g3__conf_à_a__b3_a1_1", 0, ""),
        ];
        assert_eq!(apply_corrections(t, &errors, false), "Ils mange des pommes et il va à Paris.");
    }

    #[test]
    fn contradiction_left_alone() {
        let t = "Les résultats que j'ai obtenu sont bon.";
        let errors = [
            err(
                t,
                "sont",
                &["son"],
                "conf",
                "g2__conf_son_sont__b4_a1_1",
                0,
                "Confusion probable : “sont” est la forme verbale de “être” à la 3ᵉ personne du pluriel.",
            ),
            err(
                t,
                "obtenu",
                &["obtenus"],
                "ppas",
                "gv1__ppas_det_plur_COD_que_avoir__b1_a2_1",
                0,
                "Si “Les résultats…” est bien le COD du participe passé “obtenu”, celui-ci devrait être au pluriel.",
            ),
            err(
                t,
                "bon",
                &["bons"],
                "ppas",
                "gv1__ppas_être_accord_plur__b1_a1_1",
                0,
                "Accord avec la forme verbale “sont” : “bon” devrait être au pluriel.",
            ),
        ];
        assert_eq!(apply_corrections(t, &errors, true), "Les résultats que j'ai obtenus sont bons.");
    }

    #[test]
    fn alternatives_citing_each_other_still_fixed() {
        let t = "Ils mange des pommes.";
        let errors = [
            err(t, "mange", &["mangent"], "conj", "g2__conj_ils__b1_a1_1", 0, "Accord avec “ils”."),
            err(t, "Ils", &["Il"], "conj", "g2__conj_ils__b1_a2_1", 0, "Accord avec le terme “mange”."),
        ];
        assert_eq!(apply_corrections(t, &errors, true), "Ils mangent des pommes.");
    }

    #[test]
    fn overlaps_bad_offsets_and_accents() {
        let e = |s, n, sugg: &str, rule: &str| GrammarError {
            start: s,
            end: n,
            suggestions: vec![sugg.into()],
            kind: "gn".into(),
            rule: rule.into(),
            similar: vec![true],
            ..Default::default()
        };
        let t = "abc def";
        let errors = [e(0, 3, "x", "a"), e(2, 5, "y", "b"), e(5, 99, "z", "c"), e(-1, 2, "w", "d")];
        assert_eq!(apply_corrections(t, &errors, true), "x def");
        // positions en caractères : les lettres accentuées comptent pour un
        let t = "été garé";
        assert_eq!(apply_corrections(t, &[e(4, 8, "garés", "r")], true), "été garés");
    }

    #[test]
    fn reads_the_page_output() {
        let json = r#"[{"nStart": 4, "nEnd": 9, "aSuggestions": ["mangent"], "sType": "conj",
            "sRuleId": "g2__conj_ils__b1_a1_1", "sMessage": "", "aSimilar": [false]}]"#;
        let errors: Vec<GrammarError> = serde_json::from_str(json).unwrap();
        assert_eq!(apply_corrections("Ils mange des pommes.", &errors, true), "Ils mangent des pommes.");
    }
}
