"""Moteur de dictée : raccourci -> micro -> Whisper -> dictionnaire -> correction -> collage."""
import gc
import threading
import time

import keyboard

from src import caret, cleanup, dictionary
from src.audio_capture import AudioCapture
from src.text_injector import TextInjector
from src.transcriber import Transcriber

_ALIASES = {
    "left ctrl": "ctrl", "right ctrl": "ctrl", "control": "ctrl",
    "left shift": "shift", "right shift": "shift",
    "left alt": "alt", "right alt": "alt",
    "left windows": "windows", "right windows": "windows",
}
_MODIFIERS = ("ctrl", "alt", "shift", "windows")
_MODEL_KEYS = ("whisper_model", "device", "compute_type", "cpu_threads")


def norm(name: str | None) -> str:
    n = (name or "").lower()
    return _ALIASES.get(n, n)


def normalize_hotkey(parts: list[str]) -> str:
    parts = [norm(p) for p in parts if p]
    mods = [m for m in _MODIFIERS if m in parts]
    others = [p for p in parts if p not in _MODIFIERS]
    return "+".join(mods + others)


class Engine:
    """États : loading, idle, recording, processing, error. Les erreurs ne bloquent jamais un nouvel essai."""

    def __init__(self, settings: dict, on_change, on_notice, history=None):
        self.settings = settings
        self._on_change = on_change      # appelé à chaque changement d'état
        self._on_notice = on_notice      # message court à afficher (erreur, info)
        self.state = "loading"
        self.message = ""
        self.last_text = ""
        self.dictations = 0              # compteur de dictées : l'interface recharge l'historique quand il change
        self.history = history
        self.transcriber: Transcriber | None = None
        self.audio = AudioCapture(settings.get("mic"))
        self.injector = TextInjector()

        self._lock = threading.RLock()
        self._load_lock = threading.Lock()
        self._hook = None
        self._down: set[str] = set()
        self._active = False             # le raccourci est actuellement tenu
        self._suspended = False
        self._last_use = time.time()
        self._closed = threading.Event()
        self._job = 0                    # numéro de la dictée en cours de traitement
        self._cancelled_job = -1         # dictée annulée pendant sa transcription : rien ne sera collé
        self._esc_hook = None

        self._install_hotkey()
        threading.Thread(target=self._ensure_model, daemon=True).start()
        threading.Thread(target=self._idle_watch, daemon=True).start()

    # ------------------------------------------------------------------ #
    #  Statut                                                              #
    # ------------------------------------------------------------------ #

    def status(self) -> dict:
        return {
            "state": self.state,
            "message": self.message,
            "model_loaded": self.transcriber is not None,
            "last_text": self.last_text,
            "dictations": self.dictations,
        }

    def _set(self, state: str, message: str = ""):
        self.state = state
        self.message = message
        try:
            self._on_change()
        except Exception:
            pass

    def _notify(self, text: str, kind: str = "info"):
        try:
            self._on_notice(text, kind)
        except Exception:
            pass

    # ------------------------------------------------------------------ #
    #  Modèle                                                              #
    # ------------------------------------------------------------------ #

    def _ensure_model(self, quiet: bool = False) -> bool:
        with self._load_lock:
            if self.transcriber is not None:
                if not quiet and self.state in ("loading", "error"):
                    self._set("idle")
                return True
            if not quiet:
                self._set("loading", "Chargement du modèle…")
            s = self.settings
            try:
                self.transcriber = Transcriber(
                    s["whisper_model"], s["device"], s["compute_type"], int(s.get("cpu_threads") or 0)
                )
            except Exception as e:
                self.transcriber = None
                msg = f"Chargement du modèle impossible : {e}"
                print(f"[Engine] {msg}")
                self._set("error", msg)
                self._notify(msg, "error")
                return False
            if not quiet:
                self._set("idle")
            self._last_use = time.time()
            return True

    def _unload(self):
        with self._load_lock:
            self.transcriber = None
        gc.collect()

    def _idle_watch(self):
        while not self._closed.wait(20):
            minutes = int(self.settings.get("unload_after_min") or 0)
            if (minutes > 0 and self.transcriber is not None and self.state in ("idle", "error")
                    and time.time() - self._last_use > minutes * 60):
                self._unload()
                self._on_change()

    # ------------------------------------------------------------------ #
    #  Réglages                                                            #
    # ------------------------------------------------------------------ #

    def apply_settings(self, new: dict):
        old = self.settings
        self.settings = new
        self.audio.device = new.get("mic") or None
        if (old["hotkey"], old["hotkey_mode"]) != (new["hotkey"], new["hotkey_mode"]):
            self._install_hotkey()
        if any(old.get(k) != new.get(k) for k in _MODEL_KEYS):
            self._unload()
            threading.Thread(target=self._ensure_model, daemon=True).start()
        self._on_change()

    # ------------------------------------------------------------------ #
    #  Raccourci push-to-talk                                              #
    # ------------------------------------------------------------------ #

    def cancel(self):
        """Croix de la pastille ou Échap : abandonne la dictée en cours sans rien coller."""
        with self._lock:
            if self.state == "recording":
                try:
                    self.audio.stop()      # l'audio est jeté
                except Exception:
                    pass
                self._set("idle")
                self._notify("Dictée annulée.")
            elif self.state == "processing" and self._cancelled_job != self._job:
                self._cancelled_job = self._job
                self._notify("Dictée annulée.")

    def _on_escape(self, _event):
        if self.state in ("recording", "processing") and not self._suspended:
            self.cancel()

    def _remove_hotkey(self):
        if self._hook is not None:
            try:
                keyboard.unhook(self._hook)
            except Exception:
                pass
            self._hook = None
        self._down.clear()
        self._active = False

    def _install_hotkey(self):
        self._remove_hotkey()
        if self._esc_hook is None:
            try:
                self._esc_hook = keyboard.on_press_key("esc", self._on_escape, suppress=False)
            except Exception:
                pass
        parts = [norm(p.strip()) for p in self.settings["hotkey"].split("+") if p.strip()]
        if not parts:
            return
        try:
            if len(parts) == 1:
                self._hook = keyboard.hook_key(parts[0], self._single_handler, suppress=True)
            else:
                self._parts = set(parts)
                self._hook = keyboard.hook(self._combo_handler)
        except Exception as e:
            self._notify(f"Raccourci « {self.settings['hotkey']} » inutilisable : {e}", "error")

    def suspend_hotkey(self):
        self._suspended = True
        self._remove_hotkey()

    def resume_hotkey(self):
        self._suspended = False
        self._install_hotkey()

    def _single_handler(self, event):
        if event.event_type == keyboard.KEY_DOWN:
            if not self._active:       # ignore la répétition automatique de la touche
                self._active = True
                self._press()
        else:
            self._active = False
            self._release()

    def _combo_handler(self, event):
        name = norm(event.name)
        if name not in self._parts:
            return
        if event.event_type == keyboard.KEY_DOWN:
            self._down.add(name)
            if self._parts <= self._down and not self._active:
                self._active = True
                self._press()
        else:
            self._down.discard(name)
            if self._active:
                self._active = False
                self._release()

    def _press(self):
        with self._lock:
            if self._suspended:
                return
            if self.state == "recording" and self.settings["hotkey_mode"] == "toggle":
                self._finish_recording()
                return
            if self.state == "loading":
                self._notify("Le modèle est en cours de chargement…")
                return
            if self.state not in ("idle", "error"):
                return
            try:
                self.audio.start()
            except Exception as e:
                msg = f"Microphone indisponible : {e}"
                print(f"[Engine] {msg}")
                self._set("idle")
                self._notify(msg, "error")
                return
            self._set("recording")

    def _release(self):
        with self._lock:
            if self.state == "recording" and self.settings["hotkey_mode"] == "hold":
                self._finish_recording()

    def _finish_recording(self):
        self._job += 1
        self._set("processing")
        threading.Thread(target=self._process, args=(self._job,), daemon=True).start()

    # ------------------------------------------------------------------ #
    #  Pipeline                                                            #
    # ------------------------------------------------------------------ #

    def _wait_hotkey_released(self, timeout: float = 1.0):
        """Évite que Ctrl+V parte alors que des touches du raccourci sont encore enfoncées."""
        parts = [p for p in self.settings["hotkey"].split("+") if p]
        if len(parts) < 2:
            return
        end = time.time() + timeout
        while time.time() < end:
            try:
                if not any(keyboard.is_pressed(p) for p in parts):
                    return
            except Exception:
                return
            time.sleep(0.02)

    def _process(self, job: int):
        model_failed = False
        try:
            audio = self.audio.stop()
            if audio is None:
                self._notify("Enregistrement trop court, ignoré.")
                return

            if self.transcriber is None:
                self.message = "Chargement du modèle…"
                self._on_change()
            if not self._ensure_model(quiet=True):
                model_failed = True
                return
            transcriber = self.transcriber
            if transcriber is None:
                return

            s = self.settings
            entries = s.get("dictionary", [])
            language = None if s["whisper_language"] == "auto" else s["whisper_language"]
            text, detected = transcriber.transcribe(audio, language=language,
                                                    initial_prompt=dictionary.initial_prompt(entries))
            if self._cancelled_job == job:
                return
            if s.get("log_transcripts"):
                print(f"[Whisper] {text!r}")
            if not text:
                self._notify("Aucune parole détectée.")
                return

            text = dictionary.apply(text, entries)
            text = cleanup.clean(text, fillers=s.get("remove_fillers", True),
                                 commands=s.get("spoken_punctuation", True),
                                 french=(language or detected) == "fr", nbsp=s.get("french_nbsp", True),
                                 addresses=s.get("spoken_addresses", True),
                                 short_no_period=s.get("short_no_period", True))
            if not text:
                self._notify("Aucune parole détectée.")
                return
            if s.get("fit_to_context", True):
                terms = tuple(e["term"] for e in entries if e.get("term"))
                text = cleanup.fit_to_context(text, caret.text_before_caret(), terms)
            if s.get("log_transcripts"):
                print(f"[Final]   {text!r}")

            if self._cancelled_job == job:
                return
            self.last_text = text.strip()
            self.dictations += 1
            if self.history is not None and s.get("history_enabled", True):
                try:
                    self.history.add(text.strip(), len(audio) / AudioCapture.SAMPLE_RATE, language or detected or "")
                except Exception as e:
                    print(f"[Engine] Historique non enregistré : {e}")
            self._wait_hotkey_released()
            self.injector.inject(text)
        except Exception as e:
            print(f"[Engine] Erreur pendant le traitement : {e}")
            self._notify(f"Erreur : {e}", "error")
        finally:
            self._last_use = time.time()
            if not model_failed:
                self._set("idle")

    # ------------------------------------------------------------------ #

    def shutdown(self):
        self._closed.set()
        self._remove_hotkey()
        if self._esc_hook is not None:
            try:
                keyboard.unhook(self._esc_hook)
            except Exception:
                pass
        try:
            self.audio.stop()
        except Exception:
            pass
