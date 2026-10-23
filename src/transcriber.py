import os
import shutil

import numpy as np

# Erreurs qui ne viennent pas des fichiers du modèle : inutile de le retélécharger
_NOT_FILES = ("cuda", "cublas", "cudnn", "compute type", "compute_type", "device", "out of memory")


def cache_folder_name(repo_id: str) -> str:
    """Nom du dossier de cache Hugging Face d'un dépôt : « Systran/faster-whisper-medium »
    -> « models--Systran--faster-whisper-medium »."""
    return "models--" + repo_id.replace("/", "--")


def is_repairable(error: Exception) -> bool:
    """Échec probablement dû à des fichiers de modèle absents ou abîmés (et pas au matériel)."""
    msg = str(error).lower()
    return not any(word in msg for word in _NOT_FILES)


def model_cache_dir(model_size: str) -> str | None:
    """Dossier du modèle dans le cache Hugging Face, ou None (chemin local, nom inconnu)."""
    try:
        from faster_whisper.utils import _MODELS
        from huggingface_hub import constants
    except Exception:
        return None
    repo_id = _MODELS.get(model_size)
    if not repo_id:
        return None
    return os.path.join(constants.HF_HUB_CACHE, cache_folder_name(repo_id))


class Transcriber:
    def __init__(self, model_size: str = "medium", device: str = "cpu",
                 compute_type: str = "int8", cpu_threads: int = 0, on_repair=None):
        from faster_whisper import WhisperModel

        print(f"[Transcriber] Chargement du modèle {model_size} sur {device}...")

        def load():
            return WhisperModel(model_size, device=device, compute_type=compute_type, cpu_threads=cpu_threads)

        try:
            self.model = load()
        except Exception as e:
            folder = model_cache_dir(model_size)
            if not folder or not os.path.isdir(folder) or not is_repairable(e):
                raise
            # Fichiers du modèle incomplets ou abîmés (téléchargement interrompu, antivirus…) :
            # on vide son cache et on le retélécharge une fois.
            print(f"[Transcriber] Modèle endommagé ({e}) : nouveau téléchargement.")
            if on_repair:
                on_repair()
            shutil.rmtree(folder, ignore_errors=True)
            self.model = load()
        print("[Transcriber] Modèle prêt.")

    def transcribe(self, audio: np.ndarray, language: str | None = "fr",
                   initial_prompt: str | None = None) -> tuple[str, str]:
        """Renvoie (texte, langue détectée ou imposée)."""
        segments, info = self.model.transcribe(
            audio,
            language=language,
            beam_size=5,
            vad_filter=True,          # filtre les silences
            vad_parameters={"min_silence_duration_ms": 500},
            initial_prompt=initial_prompt,
            condition_on_previous_text=False,
        )
        text = " ".join(segment.text.strip() for segment in segments)
        return text.strip(), info.language
