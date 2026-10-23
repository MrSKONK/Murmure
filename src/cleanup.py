"""Correction locale du texte dicté, par règles : hésitations, ponctuation dictée, typographie française."""
import re

_NBSP = " "     # espace insécable (avant « : », dans les guillemets)
_NNBSP = " "    # espace fine insécable (avant ; ! ?)

# Hésitations retirées avec la virgule ou les points de suspension qui les suivent
_FILLERS = re.compile(
    r"(?<!\w)(?:eu+h+|heu+|euhm|hu+m+|hm+)(?!\w)\s*(?:,|\.\.\.|…)?",
    re.IGNORECASE,
)

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
    for i, ch in enumerate(text):
        if ch.isalpha():
            return text[:i] + ch.upper() + text[i + 1:]
        if not (ch.isspace() or ch in "«(\"'"):
            return text
    return text


def clean(text: str, fillers: bool = True, commands: bool = True,
          french: bool = False, nbsp: bool = True) -> str:
    if not text:
        return text
    if fillers:
        text = _FILLERS.sub(" ", text)
    if commands:
        text = _apply_commands(text)
    text = _tidy(text)
    if not any(ch.isalnum() for ch in text):
        return ""   # il ne restait que des hésitations ou de la ponctuation
    if french:
        text = _french_typography(text, nbsp)
    return _capitalize_first(text)
