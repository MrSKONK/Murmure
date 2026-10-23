import json
import os

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SETTINGS_PATH = os.path.join(BASE_DIR, "config", "settings.json")

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
    "llm_enabled": False,
    "llm_model": "llama3.2:3b",
    "llm_prompt": (
        "Tu es un assistant de correction orthographique. Corrige uniquement la ponctuation, "
        "les majuscules et les fautes évidentes de ce texte dicté, sans reformuler ni changer "
        "le sens. Réponds uniquement avec le texte corrigé, sans explication :"
    ),
    "ollama_url": "http://localhost:11434",
    "log_transcripts": False,
}


def load() -> dict:
    settings = dict(DEFAULTS)
    try:
        with open(SETTINGS_PATH, "r", encoding="utf-8-sig") as f:
            settings.update(json.load(f))
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
