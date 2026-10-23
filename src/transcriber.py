import numpy as np
from faster_whisper import WhisperModel


class Transcriber:
    def __init__(self, model_size: str = "medium", device: str = "cpu",
                 compute_type: str = "int8", cpu_threads: int = 0):
        print(f"[Transcriber] Chargement du modèle {model_size} sur {device}...")
        self.model = WhisperModel(model_size, device=device, compute_type=compute_type,
                                  cpu_threads=cpu_threads)
        print("[Transcriber] Modèle prêt.")

    def transcribe(self, audio: np.ndarray, language: str | None = "fr",
                   initial_prompt: str | None = None) -> str:
        segments, _info = self.model.transcribe(
            audio,
            language=language,
            beam_size=5,
            vad_filter=True,          # filtre les silences
            vad_parameters={"min_silence_duration_ms": 500},
            initial_prompt=initial_prompt,
            condition_on_previous_text=False,
        )
        text = " ".join(segment.text.strip() for segment in segments)
        return text.strip()
