"""Petite pastille d'état affichée en bas de l'écran (tkinter, sans prise de focus)."""
import ctypes
import math
import queue
import threading
import time
import tkinter as tk

_TRANSPARENT = "#010203"
_BG = "#1b1b1f"
_COLORS = {"recording": "#ff5c5c", "processing": "#ffb14a", "notice": "#8ab4ff", "error": "#ff7a7a"}
_W, _H = 250, 52


class Overlay:
    def __init__(self, get_level, enabled: bool = True):
        self._get_level = get_level
        self.enabled = enabled
        self._q: queue.Queue = queue.Queue()
        threading.Thread(target=self._run, daemon=True).start()

    def show(self, state: str, text: str = ""):
        """state : recording | processing | notice | error | hidden"""
        self._q.put((state, text))

    def stop(self):
        self._q.put(("quit", ""))

    # ------------------------------------------------------------------ #

    def _run(self):
        try:
            root = tk.Tk()
        except Exception as e:
            print(f"[Overlay] indisponible : {e}")
            return
        root.withdraw()
        root.overrideredirect(True)
        root.attributes("-topmost", True)
        root.attributes("-transparentcolor", _TRANSPARENT)
        root.configure(bg=_TRANSPARENT)
        canvas = tk.Canvas(root, width=_W, height=_H, bg=_TRANSPARENT, highlightthickness=0)
        canvas.pack()
        sw, sh = root.winfo_screenwidth(), root.winfo_screenheight()
        root.geometry(f"{_W}x{_H}+{(sw - _W) // 2}+{sh - _H - 90}")
        root.update_idletasks()
        self._no_focus(root)

        st = {"state": "hidden", "text": "", "until": 0.0, "visible": False, "levels": [0.0] * 9}

        def pill():
            r = _H // 2
            canvas.create_oval(0, 0, _H, _H, fill=_BG, outline=_BG)
            canvas.create_oval(_W - _H, 0, _W, _H, fill=_BG, outline=_BG)
            canvas.create_rectangle(r, 0, _W - r, _H, fill=_BG, outline=_BG)

        def draw():
            canvas.delete("all")
            pill()
            state, color = st["state"], _COLORS.get(st["state"], "#ffffff")
            t = time.time()
            if state == "recording":
                lvl = min(1.0, self._get_level() * 14)
                st["levels"] = st["levels"][1:] + [lvl]
                for i, v in enumerate(st["levels"]):
                    h = 6 + v * 26
                    x = 24 + i * 9
                    canvas.create_rectangle(x, _H / 2 - h / 2, x + 5, _H / 2 + h / 2, fill=color, outline=color)
                canvas.create_text(122, _H / 2, text="Écoute…", fill="#ffffff", anchor="w",
                                   font=("Segoe UI", 11))
            elif state == "processing":
                for i in range(3):
                    r = 4 + 2.5 * (0.5 + 0.5 * math.sin(t * 7 - i * 0.9))
                    x = 34 + i * 18
                    canvas.create_oval(x - r, _H / 2 - r, x + r, _H / 2 + r, fill=color, outline=color)
                canvas.create_text(92, _H / 2, text=st["text"] or "Transcription…", fill="#ffffff",
                                   anchor="w", font=("Segoe UI", 11))
            else:
                canvas.create_oval(20, _H / 2 - 5, 30, _H / 2 + 5, fill=color, outline=color)
                canvas.create_text(42, _H / 2, text=st["text"][:30], fill="#ffffff", anchor="w",
                                   font=("Segoe UI", 10))

        def tick():
            try:
                while True:
                    state, text = self._q.get_nowait()
                    if state == "quit":
                        root.destroy()
                        return
                    if state in ("notice", "error"):
                        st["until"] = time.time() + 2.5
                    else:
                        st["until"] = 0.0
                    if state == "processing" and not text:
                        text = "Transcription…"
                    st["state"], st["text"] = state, text
            except queue.Empty:
                pass

            if st["state"] in ("notice", "error") and time.time() > st["until"]:
                st["state"] = "hidden"

            show = self.enabled and st["state"] != "hidden"
            if st["state"] in ("notice", "error"):
                show = True   # les erreurs restent visibles même si la pastille est désactivée
            if show:
                if not st["visible"]:
                    root.deiconify()
                    st["visible"] = True
                draw()
                root.after(33, tick)
            else:
                if st["visible"]:
                    root.withdraw()
                    st["visible"] = False
                root.after(80, tick)

        root.after(80, tick)
        root.mainloop()

    @staticmethod
    def _no_focus(root):
        """Fenêtre non activable et traversée par les clics : ne vole jamais le focus à l'appli de saisie."""
        try:
            hwnd = ctypes.windll.user32.GetParent(root.winfo_id())
            style = ctypes.windll.user32.GetWindowLongW(hwnd, -20)
            ctypes.windll.user32.SetWindowLongW(hwnd, -20, style | 0x08000000 | 0x00000080 | 0x00000020)
        except Exception:
            pass
