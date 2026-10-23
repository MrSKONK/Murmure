import numpy as np
import sounddevice as sd


class AudioCapture:
    SAMPLE_RATE = 16000  # Whisper attend du 16 kHz

    def __init__(self, device: str | None = None):
        self.device = device or None
        self._chunks: list[np.ndarray] = []
        self._stream: sd.InputStream | None = None
        self.recording = False
        self.level = 0.0  # volume instantané (0..1) pour la pastille

    def _open(self, device):
        return sd.InputStream(
            samplerate=self.SAMPLE_RATE,
            channels=1,
            dtype="float32",
            device=device,
            callback=self._callback,
        )

    def start(self):
        self._chunks = []
        self.level = 0.0
        try:
            stream = self._open(self.device)
        except Exception:
            if self.device is None:
                raise
            stream = self._open(None)  # micro choisi introuvable : micro par défaut
        self.recording = True
        try:
            stream.start()
        except Exception:
            self.recording = False
            stream.close()
            raise
        self._stream = stream

    def _callback(self, indata: np.ndarray, frames: int, time, status):
        if self.recording:
            self._chunks.append(indata.copy())
            self.level = float(np.sqrt(np.mean(np.square(indata))))

    def stop(self) -> np.ndarray | None:
        self.recording = False
        self.level = 0.0
        if self._stream:
            try:
                self._stream.stop()
                self._stream.close()
            finally:
                self._stream = None

        if not self._chunks:
            return None

        audio = np.concatenate(self._chunks, axis=0).flatten()

        # Ignore les enregistrements trop courts (< 0.5 s)
        if len(audio) < self.SAMPLE_RATE * 0.5:
            return None

        return audio


def list_microphones() -> list[str]:
    names = []
    try:
        for d in sd.query_devices():
            if d["max_input_channels"] > 0 and d["name"] not in names:
                names.append(d["name"])
    except Exception:
        pass
    return names
