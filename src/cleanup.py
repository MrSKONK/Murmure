"""Correction locale du texte dicté, par règles : hésitations, répétitions, ponctuation dictée,
adresses e-mail et web, typographie française, raccord avec le texte déjà présent."""
import re

_NBSP = " "     # espace insécable (avant « : », dans les guillemets)
_NNBSP = " "    # espace fine insécable (avant ; ! ?)

# Hésitations retirées avec la virgule ou les points de suspension qui les suivent
_FILLERS = re.compile(
    r"(?<!\w)(?:eu+h+|heu+|euhm|hu+m+|hm+)(?!\w)\s*(?:,|\.\.\.|…)?",
    re.IGNORECASE,
)

# Mot répété par hésitation (« le le chat »), sauf les répétitions correctes du français
_REPEAT = re.compile(r"(?<!\w)([^\W\d_]+)(?:\s+\1(?!\w))+", re.IGNORECASE)
_REPEAT_OK = {"nous", "vous", "si", "non", "oui", "très", "trop", "plus", "bien", "tout", "encore", "vite",
              "ah", "ha", "oh", "eh", "hé", "bla", "toc", "chut", "bye", "ci", "là"}

# Adresses dictées : « nathan point dupont arobase gmail point com », « www point exemple point fr »
_TLD = r"(?:com|fr|net|org|io|dev|app|info)"
_DOT = r"(?:\s+point\s+|\.)"
_LOCAL_SEP = r"\s+(?:point|tiret\s+bas|underscore|tiret)\s+"
_EMAIL = re.compile(
    rf"(?<![\w@.])([\w+-]+(?:{_LOCAL_SEP}[\w+-]+)*)\s*(?:arr?obase|@)\s*((?:[\w-]+{_DOT})+{_TLD})(?!\w)",
    re.IGNORECASE)
_URL = re.compile(rf"(?<![\w@.])((?:[\w-]+{_DOT})+{_TLD})(?!\w)", re.IGNORECASE)
# premiers mots qui ne peuvent pas commencer une adresse (« le point com » reste du texte)
_NOT_DOMAIN = {"le", "la", "les", "un", "une", "du", "de", "des", "ce", "cet", "cette", "mon", "ton", "son",
               "ma", "ta", "sa", "au", "aux", "en", "sur", "ce", "quel", "quelle", "chaque", "premier",
               "deuxième", "dernier", "même", "autre", "bon", "seul", "notre", "votre", "leur"}

# (expression dictée, symbole, type) — les plus longues d'abord.
#   end   : ponctuation finale ou médiane ; absorbe celle que Whisper a mise autour
#   line  : saut de ligne ; garde la ponctuation qui précède
#   open  : ouvrant ; garde ce qui précède
#   close : fermant ; absorbe la virgule qui précède
_APOS = r"['’]"
_COMMANDS = [
    (rf"point d{_APOS}interrogation", "?", "end"),
    (rf"point d{_APOS}exclamation", "!", "end"),
    (r"points? de suspension", "…", "end"),
    (r"point[\s-]+virgule", ";", "end"),
    (r"deux-points", ":", "end"),
    (r"point[\s,]+(?:à|a) la ligne", ".\n", "end"),
    (r"point final", ".", "end"),
    (r"nouveau paragraphe", "\n\n", "line"),
    (r"(?:retour )?(?:à|a) la ligne", "\n", "line"),
    (r"virgule", ",", "end"),
    (r"(?:ouvre[zr]?|ouvrir) les guillemets", "«", "open"),
    (r"(?:ferme[zr]?|fermer) les guillemets", "»", "close"),
    (r"(?:ouvre[zr]?|ouvrir) (?:la )?parenthèse", "(", "open"),
    (r"(?:ferme[zr]?|fermer) (?:la )?parenthèse", ")", "close"),
]
_CAPITALIZE_AFTER = (".", "?", "!", "\n")


def _command_regex(phrase: str, kind: str) -> re.Pattern:
    before = {"end": rf"[\s,.;:!?…]*", "line": r"[ \t]*", "open": r"[ \t]*", "close": r"[\s,]*"}[kind]
    after = r"[\s,.;:!?…]*" if kind in ("end", "line") else r"[ \t,]*" if kind == "open" else ""
    return re.compile(rf"{before}(?<!\w)(?:{phrase})(?!\w){after}(\w?)", re.IGNORECASE)


_COMPILED = [(_command_regex(p, k), sym, k) for p, sym, k in _COMMANDS]


def _apply_commands(text: str) -> str:
    for regex, symbol, kind in _COMPILED:
        def repl(m, symbol=symbol, kind=kind):
            nxt = m.group(1)
            if symbol.endswith(_CAPITALIZE_AFTER):
                nxt = nxt.upper()
            if kind == "open":
                return " " + symbol + nxt
            if kind == "line" or symbol.endswith("\n"):
                return symbol + nxt
            return symbol + (" " + nxt if nxt else " ")
        text = regex.sub(repl, text)
    return text


def _remove_repeats(text: str) -> str:
    return _REPEAT.sub(lambda m: m.group(0) if m.group(1).lower() in _REPEAT_OK else m.group(1), text)


def _join_address(spoken: str) -> str:
    out = re.sub(r"\s+point\s+|\s*\.\s*", ".", spoken, flags=re.IGNORECASE)
    out = re.sub(r"\s+(?:tiret\s+bas|underscore)\s+", "_", out, flags=re.IGNORECASE)
    out = re.sub(r"\s+tiret\s+", "-", out, flags=re.IGNORECASE)
    return out.lower()


def _apply_addresses(text: str) -> str:
    text = _EMAIL.sub(lambda m: _join_address(m.group(1)) + "@" + _join_address(m.group(2)), text)

    def url(m):
        first = re.split(r"\s|\.", m.group(1), maxsplit=1)[0].lower()
        return m.group(0) if first in _NOT_DOMAIN else _join_address(m.group(1))
    return _URL.sub(url, text)


def _drop_short_final_period(text: str, max_words: int = 20) -> str:
    """Message d'une seule phrase courte : pas de point final (« ? » et « ! » sont gardés)."""
    if not text.endswith(".") or text.endswith(".."):
        return text
    body = text[:-1]
    if "\n" in body or re.search(r"[.!?…](?:\s|$)", body) or len(body.split()) > max_words:
        return text
    return body


def _tidy(text: str) -> str:
    text = re.sub(r"[ \t]+", " ", text)
    text = re.sub(r" ?\n ?", "\n", text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    text = re.sub(r" +([,.…)])", r"\1", text)                  # pas d'espace avant , . … )
    text = re.sub(r"\( +", "(", text)
    text = re.sub(r",(?:\s*,)+", ",", text)                     # virgules en double
    text = re.sub(r",\s*([.;:!?…])", r"\1", text)               # virgule suivie d'une autre ponctuation
    text = re.sub(r"(?m)^[ ,;:]+", "", text)                    # ponctuation orpheline en début de ligne
    text = re.sub(r",(?=[^\W\d_])", ", ", text)                 # espace après une virgule suivie d'une lettre
    return text.strip(" \t,;\n")


def _french_typography(text: str, nbsp: bool) -> str:
    """Espace avant ; : ! ? et dans les guillemets ; insécable si nbsp, ordinaire sinon."""
    spaces = rf"[ {_NBSP}{_NNBSP}]*"
    thin, wide = (_NNBSP, _NBSP) if nbsp else (" ", " ")
    text = re.sub(rf"(?<=\S){spaces}([;!?]+)", lambda m: thin + m.group(1), text)
    text = re.sub(rf"(?<=\S){spaces}:(?=\s|$)", wide + ":", text)   # pas les heures 14:30 ni http://
    text = re.sub(rf"«{spaces}", "«" + wide, text)
    text = re.sub(rf"{spaces}»", wide + "»", text)
    text = re.sub(r"(?<=[^\s(\[])«", " «", text)
    text = re.sub(r"»(?=\w)", "» ", text)
    return text


def _capitalize_first(text: str) -> str:
    if re.match(r"\S*[@.]\w", text):
        return text   # commence par une adresse : on n'y touche pas
    for i, ch in enumerate(text):
        if ch.isalpha():
            return text[:i] + ch.upper() + text[i + 1:]
        if not (ch.isspace() or ch in "«(\"'"):
            return text
    return text


def clean(text: str, fillers: bool = True, commands: bool = True, french: bool = False,
          nbsp: bool = True, addresses: bool = True, short_no_period: bool = False) -> str:
    if not text:
        return text
    if fillers:
        text = _FILLERS.sub(" ", text)
        text = _remove_repeats(text)
    if addresses:
        text = _apply_addresses(text)
    if commands:
        text = _apply_commands(text)
    text = _tidy(text)
    if not any(ch.isalnum() for ch in text):
        return ""   # il ne restait que des hésitations ou de la ponctuation
    if short_no_period:
        text = _drop_short_final_period(text)
    if french:
        text = _french_typography(text, nbsp)
    return _capitalize_first(text)


def fit_to_context(text: str, before: str | None, keep_case: tuple[str, ...] = ()) -> str:
    """Raccorde la dictée au texte situé juste avant le curseur (None = inconnu : rien ne change).

    En milieu de phrase : première lettre en minuscule et espace ajoutée si besoin.
    Les mots du dictionnaire (keep_case), les sigles et les adresses gardent leur casse."""
    if before is None or not text:
        return text
    before = before.replace("\r", "\n").replace("\ufffc", "")
    if not before.strip(" \t") or before.endswith("\n"):
        return text                                   # début de champ ou de ligne
    last = before[-1]
    prev = before.rstrip()[-1]
    if prev not in ".!?…":
        word = re.match(r"[^\s,.;:!?…]+", text)
        w = word.group(0) if word else ""
        if (w and w[0].isupper() and not (len(w) > 1 and w.isupper()) and w not in keep_case
                and not re.search(r"[@.]\w", w)):
            text = w[0].lower() + text[1:]
    if not last.isspace() and last not in "(«[\"'’-/" and not text[0] in ",.;:)»]":
        text = " " + text
    return text
