"""Historique des dictées, enregistré en JSON à côté des réglages (jamais envoyé ailleurs)."""
import json
import os
import threading
import time
import uuid


class History:
    def __init__(self, path: str, limit: int = 1000):
        self.path = path
        self.limit = limit
        self._lock = threading.Lock()
        self._entries = self._load()

    def _load(self) -> list[dict]:
        try:
            with open(self.path, "r", encoding="utf-8") as f:
                data = json.load(f)
            return [e for e in data if isinstance(e, dict) and isinstance(e.get("text"), str)]
        except FileNotFoundError:
            return []
        except (OSError, ValueError) as e:
            print(f"[Historique] illisible ({e}) — historique vide.")
            return []

    def _save(self):
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        tmp = self.path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(self._entries, f, ensure_ascii=False, indent=1)
        os.replace(tmp, self.path)

    def _trim(self):
        if self.limit > 0 and len(self._entries) > self.limit:
            del self._entries[: len(self._entries) - self.limit]

    def add(self, text: str, duration: float = 0.0, language: str = "") -> dict:
        entry = {"id": uuid.uuid4().hex, "time": time.time(), "text": text,
                 "duration": round(duration, 1), "language": language}
        with self._lock:
            self._entries.append(entry)
            self._trim()
            self._save()
        return entry

    def entries(self) -> list[dict]:
        """Du plus récent au plus ancien."""
        with self._lock:
            return list(reversed(self._entries))

    def get(self, entry_id: str) -> dict | None:
        with self._lock:
            return next((e for e in self._entries if e.get("id") == entry_id), None)

    def delete(self, entry_id: str) -> bool:
        with self._lock:
            before = len(self._entries)
            self._entries = [e for e in self._entries if e.get("id") != entry_id]
            if len(self._entries) != before:
                self._save()
                return True
            return False

    def clear(self):
        with self._lock:
            self._entries = []
            self._save()

    def set_limit(self, limit: int):
        with self._lock:
            self.limit = limit
            before = len(self._entries)
            self._trim()
            if len(self._entries) != before:
                self._save()
