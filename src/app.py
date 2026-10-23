"""Application : fenêtre de réglages (pywebview/WebView2), icône de zone de notification et pastille."""
import json
import os
import socket
import sys
import threading

import pyperclip
import pystray
import webview

from src import autostart, config, updater
from src.audio_capture import list_microphones
from src.engine import Engine, norm, normalize_hotkey
from src.grammar import Grammar
from src.history import History
from src.icon import make_icon
from src.overlay import Overlay
from src.version import __version__

import keyboard

UI_PATH = os.path.join(config.BASE_DIR, "ui", "index.html")
LOCK_PORT = 47653

_TOOLTIPS = {
    "loading": "Chargement du modèle…",
    "idle": "Prêt",
    "recording": "Enregistrement…",
    "processing": "Transcription…",
    "error": "Erreur — ouvrez la fenêtre pour les détails",
}
_VALID = {
    "hotkey_mode": ("hold", "toggle"),
    "whisper_model": ("tiny", "base", "small", "medium", "large-v3-turbo", "large-v3"),
    "whisper_language": ("auto", "fr", "en", "es", "de", "it", "pt", "nl"),
}


def acquire_single_instance():
    """Renvoie un socket verrou, ou None si l'application tourne déjà (on lui demande alors de s'afficher)."""
    s = socket.socket()
    try:
        s.bind(("127.0.0.1", LOCK_PORT))
        s.listen(1)
        return s
    except OSError:
        try:
            with socket.create_connection(("127.0.0.1", LOCK_PORT), timeout=1) as c:
                c.sendall(b"show")
        except OSError:
            pass
        return None


class Api:
    """Méthodes appelables depuis l'interface (window.pywebview.api.*)."""

    def __init__(self, app: "App"):
        self._app = app

    def get_settings(self):
        s = dict(self._app.settings)
        s["autostart"] = autostart.is_enabled()
        return s

    def save_settings(self, new: dict):
        return self._app.save_settings(new)

    def get_status(self):
        return self._app.engine.status()

    def list_microphones(self):
        return list_microphones()

    def capture_hotkey(self):
        return self._app.capture_hotkey()

    def get_system(self):
        return {"cpus": os.cpu_count() or 0, "history_path": config.HISTORY_PATH, "version": __version__}

    def get_grammar(self):
        return self._app.grammar.status()

    def get_update(self):
        return self._app.update_state()

    def check_update(self):
        self._app.check_update(manual=True)

    def install_update(self):
        self._app.install_update()

    def copy_last(self):
        text = self._app.engine.last_text
        if text:
            pyperclip.copy(text)
        return bool(text)

    def get_history(self):
        return self._app.history.entries()

    def copy_history(self, entry_id: str):
        entry = self._app.history.get(entry_id)
        if entry:
            pyperclip.copy(entry["text"])
        return bool(entry)

    def delete_history(self, entry_id: str):
        return self._app.history.delete(entry_id)

    def clear_history(self):
        self._app.history.clear()
        return True

    def quit_app(self):
        self._app.quit()


class App:
    def __init__(self, background: bool):
        self.settings = config.load()
        self.background = background
        self._quitting = False
        self.window = None
        self.icon = None
        self.overlay = Overlay(lambda: self.engine.audio.level, self.settings.get("show_overlay", True),
                               on_cancel=lambda: self.engine.cancel())
        self.history = History(config.HISTORY_PATH, int(self.settings.get("history_limit") or 0))
        self.grammar = Grammar(on_change=self._push_grammar)
        if self.settings.get("grammar_enabled"):
            self.grammar.install()   # charge, ou retélécharge s'il manque
        self.engine = Engine(self.settings, self._on_state, self._on_notice, self.history, self.grammar)
        self._update = {"state": "idle", "version": "", "notes": "", "progress": 0.0, "error": ""}
        self._release = None

    # ---------------------------------------------------------------- #

    def run(self, lock_socket):
        self.window = webview.create_window(
            config.APP_NAME, UI_PATH, js_api=Api(self),
            width=920, height=640, min_size=(780, 520), hidden=self.background,
            background_color="#f6f6f8",
        )
        self.window.events.closing += self._on_closing
        threading.Thread(target=self._listen_show, args=(lock_socket,), daemon=True).start()

        menu = pystray.Menu(
            pystray.MenuItem("Ouvrir", lambda: self.show_window(), default=True),
            pystray.MenuItem("Quitter", lambda: self.quit()),
        )
        self.icon = pystray.Icon(config.APP_NAME, make_icon("loading"), config.APP_NAME, menu)
        self.icon.run_detached()
        self._refresh_tray()
        threading.Thread(target=self._auto_update_loop, daemon=True).start()

        webview.start()

        self.engine.shutdown()
        self.overlay.stop()
        try:
            self.icon.stop()
        except Exception:
            pass

    # ---------------------------------------------------------------- #
    #  Fenêtre                                                          #
    # ---------------------------------------------------------------- #

    def show_window(self):
        try:
            self.window.show()
            self.window.restore()
        except Exception:
            pass

    def _on_closing(self):
        if self._quitting:
            return True
        self.window.hide()   # on reste en arrière-plan dans la zone de notification
        return False

    def quit(self):
        self._quitting = True
        try:
            self.window.destroy()
        except Exception:
            pass

    def _listen_show(self, sock):
        while True:
            try:
                conn, _ = sock.accept()
                with conn:
                    if conn.recv(16) == b"show":
                        self.show_window()
            except OSError:
                return

    # ---------------------------------------------------------------- #
    #  Retours du moteur                                                #
    # ---------------------------------------------------------------- #

    def _refresh_tray(self):
        if self.icon:
            state = self.engine.state if hasattr(self, "engine") else "loading"
            try:
                self.icon.icon = make_icon(state)
                self.icon.title = f"{config.APP_NAME} — " + _TOOLTIPS.get(state, "")
            except Exception:
                pass

    def _on_state(self):
        e = getattr(self, "engine", None)
        if e is None:   # le moteur signale son premier état pendant sa construction
            return
        self._refresh_tray()
        if e.state == "recording":
            self.overlay.show("recording")
        elif e.state == "processing":
            self.overlay.show("processing", e.message)
        else:
            self.overlay.show("hidden")
        self._push_status()

    def _on_notice(self, text: str, kind: str):
        self.overlay.show("error" if kind == "error" else "notice", text)
        if kind == "error" and self.icon:
            try:
                self.icon.notify(text[:200], config.APP_NAME)
            except Exception:
                pass
        self._push_status()

    def _push_status(self):
        if not self.window:
            return
        try:
            payload = json.dumps(self.engine.status())
            self.window.evaluate_js(f"window.__status && window.__status({payload})")
        except Exception:
            pass

    # ---------------------------------------------------------------- #
    #  Mises à jour                                                     #
    # ---------------------------------------------------------------- #

    def _push_grammar(self):
        if not self.window:
            return
        try:
            self.window.evaluate_js(f"window.__grammar && window.__grammar({json.dumps(self.grammar.status())})")
        except Exception:
            pass

    def update_state(self) -> dict:
        return dict(self._update, current=__version__, supported=config.FROZEN)

    def _set_update(self, **changes):
        self._update.update(changes)
        if not self.window:
            return
        try:
            self.window.evaluate_js(f"window.__update && window.__update({json.dumps(self.update_state())})")
        except Exception:
            pass

    def _auto_update_loop(self):
        if not config.FROZEN:
            return   # seule la version installée se met à jour
        delay = 20
        while not self._quitting:
            threading.Event().wait(delay)
            if self.settings.get("auto_update", True) and self._update["state"] not in ("downloading", "installing"):
                self.check_update(manual=False)
            delay = 12 * 3600

    def check_update(self, manual: bool):
        if not config.FROZEN:
            self._set_update(state="unsupported")
            return
        if self._update["state"] in ("checking", "downloading", "installing"):
            return
        self._set_update(state="checking", error="")
        try:
            release = updater.check()
        except Exception as e:
            self._set_update(state="error", error=f"Vérification impossible : {e}")
            return
        if not release:
            self._release = None
            self._set_update(state="uptodate", version="", notes="")
            return
        announce = self._update.get("version") != release["version"]
        self._release = release
        self._set_update(state="available", version=release["version"], notes=release["notes"][:2000])
        if announce and not manual and self.icon:
            try:
                self.icon.notify(f"Murmure {release['version']} est disponible. Ouvrez la fenêtre pour l'installer.",
                                 config.APP_NAME)
            except Exception:
                pass

    def install_update(self):
        if not self._release or self._update["state"] in ("downloading", "installing"):
            return
        threading.Thread(target=self._install_update, daemon=True).start()

    def _install_update(self):
        if self.engine.state in ("recording", "processing"):
            self._set_update(state="available", error="Terminez la dictée en cours avant d'installer.")
            return
        self._set_update(state="downloading", progress=0.0, error="")
        try:
            def progress(p):
                if p - self._update["progress"] >= 0.01 or p >= 1:
                    self._set_update(progress=round(p, 3))
            path = updater.download(self._release, progress)
            self._set_update(state="installing")
            updater.launch_installer(path)
        except Exception as e:
            self._set_update(state="available", error=f"Mise à jour impossible : {e}")
            return
        self.quit()   # l'installateur remplace les fichiers puis relance Murmure

    # ---------------------------------------------------------------- #
    #  Réglages                                                         #
    # ---------------------------------------------------------------- #

    def save_settings(self, new: dict):
        merged = dict(self.settings)
        for key, default in config.DEFAULTS.items():
            if key not in new:
                continue
            value = new[key]
            if key in _VALID and value not in _VALID[key]:
                continue
            if isinstance(default, bool):
                value = bool(value)
            elif isinstance(default, int):
                try:
                    value = max(0, int(value))
                except (TypeError, ValueError):
                    continue
            elif isinstance(default, str):
                value = str(value)
            elif key == "dictionary":
                value = [
                    {"term": str(e.get("term", "")).strip(), "heard": str(e.get("heard", "")).strip()}
                    for e in value if isinstance(e, dict) and str(e.get("term", "")).strip()
                ]
            merged[key] = value
        if not merged["hotkey"].strip():
            merged["hotkey"] = self.settings["hotkey"]

        if merged["autostart"] != self.settings.get("autostart"):
            try:
                autostart.set_enabled(merged["autostart"])
            except Exception as e:
                self._on_notice(f"Démarrage automatique impossible : {e}", "error")
                merged["autostart"] = self.settings.get("autostart", False)

        self.settings = merged
        config.save(merged)
        self.overlay.enabled = merged["show_overlay"]
        self.history.set_limit(merged["history_limit"])
        if merged["grammar_enabled"]:
            self.grammar.install()
        self.engine.apply_settings(merged)
        return self.get_settings_safe()

    def get_settings_safe(self):
        s = dict(self.settings)
        s["autostart"] = autostart.is_enabled()
        return s

    def capture_hotkey(self, timeout: float = 10.0) -> str:
        """Écoute la prochaine touche ou combinaison pressée puis relâchée."""
        self.engine.suspend_hotkey()
        pressed: list[str] = []
        down: set[str] = set()
        done = threading.Event()

        def handler(event):
            name = norm(event.name)
            if event.event_type == keyboard.KEY_DOWN:
                down.add(name)
                if name not in pressed:
                    pressed.append(name)
            else:
                down.discard(name)
                if not down and pressed:
                    done.set()

        hook = keyboard.hook(handler, suppress=False)
        try:
            done.wait(timeout)
        finally:
            keyboard.unhook(hook)
            self.engine.resume_hotkey()
        if pressed == ["esc"]:
            return ""
        return normalize_hotkey(pressed)
