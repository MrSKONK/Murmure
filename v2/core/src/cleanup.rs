//! Correction locale du texte dicté, par règles : hésitations, répétitions, ponctuation dictée,
//! adresses e-mail et web, typographie française, raccord avec le texte déjà présent.
//!
//! Portage de `src/cleanup.py` : les deux versions passent les mêmes cas (`tests/cleanup_cases.json`).
use std::sync::LazyLock;

use fancy_regex::{Captures, Regex};

const NBSP: &str = "\u{a0}"; // espace insécable (avant « : », dans les guillemets)
const NNBSP: &str = "\u{202f}"; // espace fine insécable (avant ; ! ?)

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|e| panic!("expression invalide {pattern:?} : {e}"))
}

/// `re.sub` de Python avec une fonction de remplacement.
fn sub(regex: &Regex, text: &str, f: impl Fn(&Captures<str>) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in regex.captures_iter(text) {
        let Ok(caps) = caps else { return text.to_string() };
        let m = caps.get(0).expect("groupe 0");
        out.push_str(&text[last..m.start()]);
        out.push_str(&f(&caps));
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

/// `re.sub` de Python avec un modèle de remplacement (`$1` pour les groupes).
fn sub_str(regex: &Regex, text: &str, replacement: &str) -> String {
    regex.replace_all(text, replacement).into_owned()
}

fn group<'t>(caps: &Captures<'t, str>, i: usize) -> &'t str {
    caps.get(i).map_or("", |m| m.as_str())
}

// Hésitations retirées avec la virgule ou les points de suspension qui les suivent
static FILLERS: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)(?<!\w)(?:eu+h+|heu+|euhm|hu+m+|hm+)(?!\w)\s*(?:,|\.\.\.|…)?"));

// Mot répété par hésitation (« le le chat »), sauf les répétitions correctes du français
static REPEAT: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)(?<!\w)([^\W\d_]+)(?:\s+\1(?!\w))+"));
const REPEAT_OK: &[&str] = &[
    "nous", "vous", "si", "non", "oui", "très", "trop", "plus", "bien", "tout", "encore", "vite", "ah", "ha", "oh",
    "eh", "hé", "bla", "toc", "chut", "bye", "ci", "là",
];

// Adresses dictées : « nathan point dupont arobase gmail point com », « www point exemple point fr »
const TLD: &str = r"(?:com|fr|net|org|io|dev|app|info)";
const DOT: &str = r"(?:\s+point\s+|\.)";
const LOCAL_SEP: &str = r"\s+(?:point|tiret\s+bas|underscore|tiret)\s+";
static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(r"(?i)(?<![\w@.])([\w+-]+(?:{LOCAL_SEP}[\w+-]+)*)\s*(?:arr?obase|@)\s*((?:[\w-]+{DOT})+{TLD})(?!\w)"))
});
static URL: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"(?i)(?<![\w@.])((?:[\w-]+{DOT})+{TLD})(?!\w)")));
// premiers mots qui ne peuvent pas commencer une adresse (« le point com » reste du texte)
const NOT_DOMAIN: &[&str] = &[
    "le",
    "la",
    "les",
    "un",
    "une",
    "du",
    "de",
    "des",
    "ce",
    "cet",
    "cette",
    "mon",
    "ton",
    "son",
    "ma",
    "ta",
    "sa",
    "au",
    "aux",
    "en",
    "sur",
    "quel",
    "quelle",
    "chaque",
    "premier",
    "deuxième",
    "dernier",
    "même",
    "autre",
    "bon",
    "seul",
    "notre",
    "votre",
    "leur",
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// ponctuation finale ou médiane ; absorbe celle que Whisper a mise autour
    End,
    /// saut de ligne ; garde la ponctuation qui précède
    Line,
    /// ouvrant ; garde ce qui précède
    Open,
    /// fermant ; absorbe la virgule qui précède
    Close,
}

const APOS: &str = r"['’]";
// (expression dictée, symbole, type) — les plus longues d'abord.
static COMMANDS: LazyLock<Vec<(Regex, &'static str, Kind)>> = LazyLock::new(|| {
    use Kind::*;
    [
        (format!(r"point d{APOS}interrogation"), "?", End),
        (format!(r"point d{APOS}exclamation"), "!", End),
        (r"points? de suspension".into(), "…", End),
        (r"point[\s-]+virgule".into(), ";", End),
        (r"deux-points".into(), ":", End),
        (r"point[\s,]+(?:à|a) la ligne".into(), ".\n", End),
        (r"point final".into(), ".", End),
        (r"nouveau paragraphe".into(), "\n\n", Line),
        (r"(?:retour )?(?:à|a) la ligne".into(), "\n", Line),
        (r"virgule".into(), ",", End),
        (r"(?:ouvre[zr]?|ouvrir) les guillemets".into(), "«", Open),
        (r"(?:ferme[zr]?|fermer) les guillemets".into(), "»", Close),
        (r"(?:ouvre[zr]?|ouvrir) (?:la )?parenthèse".into(), "(", Open),
        (r"(?:ferme[zr]?|fermer) (?:la )?parenthèse".into(), ")", Close),
    ]
    .into_iter()
    .map(|(phrase, symbol, kind)| (command_regex(&phrase, kind), symbol, kind))
    .collect()
});
const CAPITALIZE_AFTER: &[char] = &['.', '?', '!', '\n'];

fn command_regex(phrase: &str, kind: Kind) -> Regex {
    let before = match kind {
        Kind::End => r"[\s,.;:!?…]*",
        Kind::Line | Kind::Open => r"[ \t]*",
        Kind::Close => r"[\s,]*",
    };
    let after = match kind {
        Kind::End | Kind::Line => r"[\s,.;:!?…]*",
        Kind::Open => r"[ \t,]*",
        Kind::Close => "",
    };
    re(&format!(r"(?i){before}(?<!\w)(?:{phrase})(?!\w){after}(\w?)"))
}

fn apply_commands(text: &str) -> String {
    let mut text = text.to_string();
    for (regex, symbol, kind) in COMMANDS.iter() {
        text = sub(regex, &text, |caps| {
            let mut next = group(caps, 1).to_string();
            if symbol.ends_with(CAPITALIZE_AFTER) {
                next = next.to_uppercase();
            }
            if *kind == Kind::Open {
                format!(" {symbol}{next}")
            } else if *kind == Kind::Line || symbol.ends_with('\n') {
                format!("{symbol}{next}")
            } else if next.is_empty() {
                format!("{symbol} ")
            } else {
                format!("{symbol} {next}")
            }
        });
    }
    text
}

fn remove_repeats(text: &str) -> String {
    sub(&REPEAT, text, |caps| {
        let word = group(caps, 1);
        if REPEAT_OK.contains(&word.to_lowercase().as_str()) {
            group(caps, 0).to_string()
        } else {
            word.to_string()
        }
    })
}

static ADDR_DOT: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\s+point\s+|\s*\.\s*"));
static ADDR_UNDERSCORE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\s+(?:tiret\s+bas|underscore)\s+"));
static ADDR_DASH: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\s+tiret\s+"));
static FIRST_WORD: LazyLock<Regex> = LazyLock::new(|| re(r"\s|\."));

fn join_address(spoken: &str) -> String {
    let out = sub_str(&ADDR_DOT, spoken, ".");
    let out = sub_str(&ADDR_UNDERSCORE, &out, "_");
    sub_str(&ADDR_DASH, &out, "-").to_lowercase()
}

fn apply_addresses(text: &str) -> String {
    let text = sub(&EMAIL, text, |caps| format!("{}@{}", join_address(group(caps, 1)), join_address(group(caps, 2))));
    sub(&URL, &text, |caps| {
        let address = group(caps, 1);
        let end = FIRST_WORD.find(address).ok().flatten().map_or(address.len(), |m| m.start());
        if NOT_DOMAIN.contains(&address[..end].to_lowercase().as_str()) {
            group(caps, 0).to_string()
        } else {
            join_address(address)
        }
    })
}

static INNER_END: LazyLock<Regex> = LazyLock::new(|| re(r"[.!?…](?:\s|$)"));

/// Message d'une seule phrase courte : pas de point final (« ? » et « ! » sont gardés).
fn drop_short_final_period(text: &str, max_words: usize) -> String {
    if !text.ends_with('.') || text.ends_with("..") {
        return text.to_string();
    }
    let body = &text[..text.len() - 1];
    if body.contains('\n') || INNER_END.is_match(body).unwrap_or(true) || body.split_whitespace().count() > max_words {
        return text.to_string();
    }
    body.to_string()
}

static TIDY: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"[ \t]+", " "),
        (r" ?\n ?", "\n"),
        (r"\n{3,}", "\n\n"),
        (r" +([,.…)])", "$1"), // pas d'espace avant , . … )
        (r"\( +", "("),
        (r",(?:\s*,)+", ","),      // virgules en double
        (r",\s*([.;:!?…])", "$1"), // virgule suivie d'une autre ponctuation
        (r"(?m)^[ ,;:]+", ""),     // ponctuation orpheline en début de ligne
        (r",(?=[^\W\d_])", ", "),  // espace après une virgule suivie d'une lettre
    ]
    .into_iter()
    .map(|(p, r)| (re(p), r))
    .collect()
});

fn tidy(text: &str) -> String {
    let mut text = text.to_string();
    for (regex, replacement) in TIDY.iter() {
        text = sub_str(regex, &text, replacement);
    }
    text.trim_matches([' ', '\t', ',', ';', '\n']).to_string()
}

static TYPO: LazyLock<[Regex; 6]> = LazyLock::new(|| {
    let spaces = format!("[ {NBSP}{NNBSP}]*");
    [
        re(&format!(r"(?<=\S){spaces}([;!?]+)")),
        re(&format!(r"(?<=\S){spaces}:(?=\s|$)")), // pas les heures 14:30 ni http://
        re(&format!(r"«{spaces}")),
        re(&format!(r"{spaces}»")),
        re(r"(?<=[^\s(\[])«"),
        re(r"»(?=\w)"),
    ]
});

/// Espace avant ; : ! ? et dans les guillemets ; insécable si `nbsp`, ordinaire sinon.
fn french_typography(text: &str, nbsp: bool) -> String {
    let (thin, wide) = if nbsp { (NNBSP, NBSP) } else { (" ", " ") };
    let [double, colon, open, close, before_open, after_close] = &*TYPO;
    let text = sub(double, text, |caps| format!("{thin}{}", group(caps, 1)));
    let text = sub(colon, &text, |_| format!("{wide}:"));
    let text = sub(open, &text, |_| format!("«{wide}"));
    let text = sub(close, &text, |_| format!("{wide}»"));
    let text = sub_str(before_open, &text, " «");
    sub_str(after_close, &text, "» ")
}

static STARTS_WITH_ADDRESS: LazyLock<Regex> = LazyLock::new(|| re(r"^\S*[@.]\w"));

fn capitalize_first(text: &str) -> String {
    if STARTS_WITH_ADDRESS.is_match(text).unwrap_or(false) {
        return text.to_string(); // commence par une adresse : on n'y touche pas
    }
    for (i, ch) in text.char_indices() {
        if ch.is_alphabetic() {
            return format!("{}{}{}", &text[..i], ch.to_uppercase(), &text[i + ch.len_utf8()..]);
        }
        if !(ch.is_whitespace() || "«(\"'".contains(ch)) {
            break;
        }
    }
    text.to_string()
}

/// Options de [`clean`], avec les mêmes valeurs par défaut que la version Python.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub fillers: bool,
    pub commands: bool,
    pub french: bool,
    pub nbsp: bool,
    pub addresses: bool,
    pub short_no_period: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { fillers: true, commands: true, french: false, nbsp: true, addresses: true, short_no_period: false }
    }
}

pub fn clean(text: &str, o: Options) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut text = text.to_string();
    if o.fillers {
        text = sub_str(&FILLERS, &text, " ");
        text = remove_repeats(&text);
    }
    if o.addresses {
        text = apply_addresses(&text);
    }
    if o.commands {
        text = apply_commands(&text);
    }
    text = tidy(&text);
    if !text.chars().any(char::is_alphanumeric) {
        return String::new(); // il ne restait que des hésitations ou de la ponctuation
    }
    if o.short_no_period {
        text = drop_short_final_period(&text, 20);
    }
    if o.french {
        text = french_typography(&text, o.nbsp);
    }
    capitalize_first(&text)
}

static FIRST_TOKEN: LazyLock<Regex> = LazyLock::new(|| re(r"^[^\s,.;:!?…]+"));
static ADDRESS_LIKE: LazyLock<Regex> = LazyLock::new(|| re(r"[@.]\w"));

/// `str.isupper()` de Python : au moins une lettre, et toutes en majuscules.
fn is_upper(word: &str) -> bool {
    let mut cased = false;
    for ch in word.chars() {
        if ch.is_lowercase() {
            return false;
        }
        cased |= ch.is_uppercase();
    }
    cased
}

/// Raccorde la dictée au texte situé juste avant le curseur (`None` = inconnu : rien ne change).
///
/// En milieu de phrase : première lettre en minuscule et espace ajoutée si besoin.
/// Les mots du dictionnaire (`keep_case`), les sigles et les adresses gardent leur casse.
pub fn fit_to_context(text: &str, before: Option<&str>, keep_case: &[String]) -> String {
    let Some(before) = before else { return text.to_string() };
    if text.is_empty() {
        return String::new();
    }
    let before = before.replace('\r', "\n").replace('\u{fffc}', "");
    if before.trim_matches([' ', '\t']).is_empty() || before.ends_with('\n') {
        return text.to_string(); // début de champ ou de ligne
    }
    let (Some(last), Some(prev)) = (before.chars().last(), before.trim_end().chars().last()) else {
        return text.to_string();
    };
    let mut text = text.to_string();
    if !".!?…".contains(prev) {
        let word = FIRST_TOKEN.find(&text).ok().flatten().map_or("", |m| m.as_str()).to_string();
        let first = word.chars().next();
        if let Some(first) = first {
            if first.is_uppercase()
                && !(word.chars().count() > 1 && is_upper(&word))
                && !keep_case.contains(&word)
                && !ADDRESS_LIKE.is_match(&word).unwrap_or(false)
            {
                text = format!("{}{}", first.to_lowercase(), &text[first.len_utf8()..]);
            }
        }
    }
    let starts_with_closing = text.chars().next().is_some_and(|c| ",.;:)»]".contains(c));
    if !last.is_whitespace() && !"(«[\"'’-/".contains(last) && !starts_with_closing {
        text.insert(0, ' ');
    }
    text
}
