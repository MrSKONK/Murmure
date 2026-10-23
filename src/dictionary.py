import re


def _terms(entries: list[dict]) -> list[str]:
    return [e["term"].strip() for e in entries if e.get("term", "").strip()]


def initial_prompt(entries: list[dict]) -> str | None:
    """Vocabulaire passé à Whisper pour qu'il orthographie correctement ces mots."""
    terms = _terms(entries)
    return ("Vocabulaire : " + ", ".join(terms) + ".") if terms else None


def apply(text: str, entries: list[dict]) -> str:
    """Remplace ce que Whisper entend habituellement (champ « heard ») par le mot voulu."""
    for entry in entries:
        term = entry.get("term", "").strip()
        if not term:
            continue
        for heard in entry.get("heard", "").split(","):
            heard = heard.strip()
            if heard:
                pattern = r"(?<!\w)" + re.escape(heard) + r"(?!\w)"
                text = re.sub(pattern, lambda _m, t=term: t, text, flags=re.IGNORECASE)
    return text
