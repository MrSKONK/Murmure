"""Benchmark de modèles Whisper : python bench/bench.py <modele> [<modele> ...]
Génère les WAV via la synthèse vocale Windows (une fois), puis mesure chargement, vitesse et texte obtenu."""
import glob
import json
import os
import subprocess
import sys
import time
import wave

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
WAV_DIR = os.path.join(HERE, "wav")


def generate_wavs():
    os.makedirs(WAV_DIR, exist_ok=True)
    phrases = json.load(open(os.path.join(HERE, "phrases.json"), encoding="utf-8"))
    ps = os.path.join(HERE, "tts.ps1")
    for i, (voice, text) in enumerate(phrases):
        out = os.path.join(WAV_DIR, f"p{i}.wav")
        if not os.path.exists(out):
            subprocess.run(["powershell", "-NoProfile", "-File", ps, "-Voice", voice, "-Out", out, "-Text", text], check=True)


def load_wav(path):
    w = wave.open(path)
    sr = w.getframerate()
    a = np.frombuffer(w.readframes(w.getnframes()), np.int16).astype(np.float32) / 32768
    if w.getnchannels() > 1:
        a = a[:: w.getnchannels()]
    idx = np.linspace(0, len(a) - 1, int(len(a) * 16000 / sr))
    return np.interp(idx, np.arange(len(a)), a).astype(np.float32)


def main(models):
    from faster_whisper import WhisperModel

    generate_wavs()
    phrases = json.load(open(os.path.join(HERE, "phrases.json"), encoding="utf-8"))
    for model in models:
        t = time.time()
        m = WhisperModel(model, device="cpu", compute_type="int8")
        print(f"\n[{model}] chargement {time.time() - t:.1f}s", flush=True)
        for i, (_, ref) in enumerate(phrases):
            a = load_wav(os.path.join(WAV_DIR, f"p{i}.wav"))
            t = time.time()
            segs, _ = m.transcribe(a, language="fr", beam_size=5, vad_filter=True,
                                   vad_parameters={"min_silence_duration_ms": 500})
            hyp = " ".join(s.text.strip() for s in segs).strip()
            print(f"[{model}] p{i} audio={len(a) / 16000:.1f}s calcul={time.time() - t:.1f}s\n  REF: {ref}\n  HYP: {hyp}", flush=True)


if __name__ == "__main__":
    main(sys.argv[1:])
