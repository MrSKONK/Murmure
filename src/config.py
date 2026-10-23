import json
import os
import sys

APP_NAME = "Murmure"
FROZEN = getattr(sys, "frozen", False)   # True dans l'exécutable construit par PyInstaller

# Fichiers fournis avec l'application (interface) : à côté du code, ou dans le dossier de l'exécutable
BASE_DIR = getattr(sys, "_MEIPASS", os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# Réglages : dans le projet en développement, dans %APPDATA%\Murmure pour l'exécutable
if FROZEN:
    SETTINGS_PATH = os.path.join(os.environ.get("APPDATA") or os.path.expanduser("~"), APP_NAME, "settings.json")
else:
    SETTINGS_PATH = os.path.join(BASE_DIR, "config", "settings.json")
HISTORY_PATH = os.path.join(os.path.dirname(SETTINGS_PATH), "history.json")

DEFAULTS = {
    "hotkey": "²",
    "hotkey_mode": "hold",          # hold = maintenir, toggle = appuyer pour démarrer / arrêter
    "whisper_model": "medium",
    "whisper_language": "fr",       # "auto" = détection automatique
    "device": "cpu",
    "compute_type": "int8",
    "cpu_threads": 0,               # 0 = tous les cœurs ; 4 laisse de la marge au reste du PC
    "mic": "",                      # "" = micro par défaut de Windows
    "unload_after_min": 0,          # 0 = garder le modèle en mémoire
    "show_overlay": True,
    "autostart": False,
    "dictionary": [],               # [{"term": "Kubernetes", "heard": "cuber nette, cuber nétesse"}]
    "remove_fillers": True,         # retire « euh », « heu », « hum »… et les mots répétés (« le le »)
    "spoken_addresses": True,       # « nathan arobase gmail point com » -> nathan@gmail.com
    "short_no_period": True,        # pas de point final sur un message d'une seule phrase courte
    "fit_to_context": True,         # majuscule et espace selon le texte déjà présent avant le curseur
    "spoken_punctuation": True,     # « virgule », « point d'interrogation », « à la ligne »…
    "french_nbsp": True,            # espaces insécables avant ; : ! ? en français
    "history_enabled": True,        # garde chaque dictée dans history.json, à côté des réglages
    "history_limit": 1000,          # nombre de dictées conservées (0 = toutes)
    "log_transcripts": False,
}


def load() -> dict:
    settings = dict(DEFAULTS)
    try:
        with open(SETTINGS_PATH, "r", encoding="utf-8-sig") as f:
            stored = json.load(f)
        # les clés inconnues (anciens réglages, ex. Ollama) sont abandonnées
        settings.update({k: v for k, v in stored.items() if k in DEFAULTS})
    except FileNotFoundError:
        pass
    except json.JSONDecodeError as e:
        print(f"[Config] settings.json illisible ({e}) — valeurs par défaut utilisées.")
    return settings


def save(settings: dict):
    os.makedirs(os.path.dirname(SETTINGS_PATH), exist_ok=True)
    tmp = SETTINGS_PATH + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(settings, f, indent=2, ensure_ascii=False)
    os.replace(tmp, SETTINGS_PATH)
