"""Correcteur grammatical facultatif, avec Grammalecte (français, local, sans Java).

Grammalecte (GPL v3) n'est pas fourni avec Murmure : il est téléchargé à la demande depuis grammalecte.net,
vérifié par SHA-256, puis chargé depuis le dossier des réglages.

Seules les corrections sûres pour de la dictée sont appliquées. Whisper entend bien les sons mais se trompe
d'orthographe : on ne garde que les remplacements qui se prononcent comme le mot d'origine (« mange » ->
« mangent », « a » -> « à »), avec une seule suggestion, dans les catégories accords, conjugaisons et confusions.
Quand Grammalecte propose deux corrections alternatives (« Ils mange » : « mangent » ou « Il »), une seule est
retenue."""
import hashlib
import os
import re
import shutil
import sys
import tempfile
import threading
import urllib.request
import zipfile

from src import config

VERSION = "2.3.0"
URL = f"https://grammalecte.net/zip/Grammalecte-fr-v{VERSION}.zip"
SHA256 = "aaa4219704857778038ecc1db18f6a907994c0125b8e762dcecc69130280684f"
DIR = os.path.join(os.path.dirname(config.SETTINGS_PATH), "grammalecte", VERSION)

# Types d'erreurs Grammalecte pouvant être corrigés automatiquement
SAFE_TYPES = {"gn", "conj", "ppas", "infi", "vmode", "conf", "imp", "inte", "tu"}
# Options désactivées : la typographie est déjà gérée par src/cleanup.py, le reste est du style
OFF_OPTIONS = ("typo", "apos", "eepi", "esp", "tab", "nbsp", "maj", "minis", "num", "virg", "poncfin", "unit",
               "nf", "liga", "mapos", "chim", "ocr", "bs", "pleo", "neg", "redon1", "redon2", "date", "mc",
               "latex", "md", "html")


def _group(rule: str) -> str:
    """« g2__conj_ils__b1_a1_1 » et « g2__conj_ils__b1_a2_1 » sont deux alternatives de la même erreur."""
    return re.sub(r"_a\d+_\d+$", "", rule or "")


def select_corrections(text: str, errors: list[dict], is_homophone=None,
                       safe_types=SAFE_TYPES) -> list[tuple[int, int, str]]:
    """Choisit les remplacements à faire : liste de (début, fin, remplacement), sans chevauchement."""
    groups: dict[str, list[dict]] = {}
    for e in errors:
        groups.setdefault(_group(e.get("sRuleId")) or id(e), []).append(e)

    picked = []
    for alternatives in groups.values():
        for e in alternatives:
            start, end, sugg = e.get("nStart"), e.get("nEnd"), e.get("aSuggestions") or []
            if (e.get("sType") not in safe_types or len(sugg) != 1 or not isinstance(start, int)
                    or not isinstance(end, int) or not 0 <= start < end <= len(text)):
                continue
            repl = sugg[0] if "’" in text else sugg[0].replace("’", "'")
            if is_homophone is not None and not is_homophone(text[start:end], repl):
                continue
            if is_homophone is None and len(alternatives) > 1:
                break   # sans test phonétique, on ne tranche pas entre deux alternatives
            picked.append((start, end, repl))
            break       # une seule correction par erreur

    picked.sort()
    result, last_end = [], -1
    for start, end, repl in picked:
        if start >= last_end:
            result.append((start, end, repl))
            last_end = end
    return result


def apply_corrections(text: str, errors: list[dict], is_homophone=None) -> str:
    for start, end, repl in reversed(select_corrections(text, errors, is_homophone)):
        text = text[:start] + repl + text[end:]
    return text


class Grammar:
    """États : absent (à télécharger), idle (téléchargé, non chargé), downloading, loading, ready, error."""

    def __init__(self, on_change=None):
        self.state = "idle" if self.installed() else "absent"
        self.progress = 0.0
        self.error = ""
        self._checker = None
        self._homophone = None
        self._lock = threading.Lock()
        self._on_change = on_change or (lambda: None)

    @staticmethod
    def installed() -> bool:
        return os.path.isdir(os.path.join(DIR, "grammalecte"))

    def status(self) -> dict:
        return {"state": self.state, "progress": self.progress, "error": self.error, "version": VERSION}

    def _set(self, **kw):
        for k, v in kw.items():
            setattr(self, k, v)
        try:
            self._on_change()
        except Exception:
            pass

    # -- installation ------------------------------------------------------- #

    def install(self):
        if self.state in ("downloading", "loading", "ready"):
            return
        threading.Thread(target=self._install, daemon=True).start()

    def _install(self):
        if self.installed():
            self._load()
            return
        self._set(state="downloading", progress=0.0, error="")
        tmp = tempfile.mkdtemp(prefix="murmure-grammalecte-")
        try:
            archive = os.path.join(tmp, "g.zip")
            digest, done = hashlib.sha256(), 0
            req = urllib.request.Request(URL, headers={"User-Agent": "Murmure"})
            with urllib.request.urlopen(req, timeout=30) as r, open(archive, "wb") as f:
                total = int(r.headers.get("Content-Length") or 0)
                while chunk := r.read(1 << 16):
                    f.write(chunk)
                    digest.update(chunk)
                    done += len(chunk)
                    if total and done / total - self.progress >= 0.02:
                        self._set(progress=round(done / total, 2))
            if digest.hexdigest() != SHA256:
                raise ValueError("fichier téléchargé corrompu (empreinte différente)")
            extract = os.path.join(tmp, "x")
            with zipfile.ZipFile(archive) as z:
                for name in z.namelist():   # refuse les chemins qui sortiraient du dossier
                    if name.startswith(("/", "\\")) or ".." in name.replace("\\", "/").split("/"):
                        raise ValueError(f"archive invalide ({name})")
                z.extractall(extract)
            if not os.path.isdir(os.path.join(extract, "grammalecte")):
                raise ValueError("archive inattendue (dossier grammalecte absent)")
            shutil.rmtree(DIR, ignore_errors=True)
            os.makedirs(os.path.dirname(DIR), exist_ok=True)
            shutil.move(extract, DIR)
        except Exception as e:
            self._set(state="error", error=f"Téléchargement impossible : {e}")
            return
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
        self._set(progress=1.0)
        self._load()

    # -- correction --------------------------------------------------------- #

    def load(self):
        """Charge Grammalecte en arrière-plan s'il est téléchargé (moins d'une seconde)."""
        if self._checker is None and self.installed() and self.state not in ("downloading", "loading"):
            threading.Thread(target=self._load, daemon=True).start()

    def _load(self):
        self._set(state="loading", error="")
        try:
            with self._lock:
                if DIR not in sys.path:
                    sys.path.insert(0, DIR)
                import grammalecte
                checker = grammalecte.GrammarChecker("fr")
                gce = checker.getGCEngine()
                gce.setOptions({k: False for k in OFF_OPTIONS if k in gce.getOptions()})
                checker.getParagraphErrors("Chargement.")   # premier appel lent : fait d'avance
                self._homophone = _homophone_test(gce)
                self._checker = checker
        except Exception as e:
            self._set(state="error", error=f"Chargement impossible : {e}")
            return
        self._set(state="ready")

    def correct(self, text: str) -> str:
        """Texte corrigé, ou inchangé si le correcteur n'est pas prêt ou échoue."""
        if self._checker is None or not text.strip():
            return text
        try:
            with self._lock:
                gram, _spell = self._checker.getParagraphErrors(text, bSpellSugg=False)
            return apply_corrections(text, gram, self._homophone)
        except Exception as e:
            print(f"[Grammaire] ignorée : {e}")
            return text


def _homophone_test(gce):
    """Test « se prononce pareil » à partir du module phonétique de Grammalecte, ou None s'il manque."""
    phonet = getattr(gce, "phonet", None)
    simil = getattr(phonet, "isSimilAs", None)
    if simil is None:
        return None

    def same_sound(a: str, b: str) -> bool:
        if a.lower() == b.lower():
            return True
        try:
            return bool(simil(a, b) or simil(b, a) or simil(a.lower(), b.lower()) or simil(b.lower(), a.lower()))
        except Exception:
            return False
    return same_sound
