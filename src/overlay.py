"""Pastille d'état en bas de l'écran : onde vocale animée, minuteur et croix d'annulation.

Le dessin est fait par Pillow (bords lissés, dégradés) ; tkinter ne fait qu'afficher l'image.
La fenêtre ne prend jamais le focus : un clic sur la croix ne retire pas le curseur de l'application de saisie."""
import ctypes
import math
import os
import queue
import threading
import time

from PIL import Image, ImageDraw, ImageFont

_KEY = (1, 2, 3)                 # couleur rendue transparente par Windows
_W, _H = 300, 56                 # taille logique, multipliée par l'échelle d'affichage
_BG = (22, 22, 28)
_BORDER = (52, 52, 62)
_TEXT = (238, 238, 246)
_MUTED = (150, 150, 168)
_GRAD = ((139, 124, 255), (255, 111, 177))   # violet -> rose
_RED = (255, 84, 96)
_INFO = (110, 150, 255)
_BARS = 24
_SS = 3                          # suréchantillonnage des formes


def _mix(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def _font(size: int, bold: bool = False):
    names = (["seguisb.ttf", "segoeuib.ttf"] if bold else []) + ["segoeui.ttf", "DejaVuSans.ttf"]
    dirs = [os.path.join(os.environ.get("WINDIR", r"C:\Windows"), "Fonts"), "/usr/share/fonts/truetype/dejavu", ""]
    for name in names:
        for d in dirs:
            try:
                return ImageFont.truetype(os.path.join(d, name) if d else name, size)
            except OSError:
                continue
    return ImageFont.load_default()


class PillRenderer:
    """Dessine une image de la pastille ; aucune dépendance à tkinter (testable partout)."""

    def __init__(self, scale: float = 1.0):
        self.k = scale
        self.w, self.h = round(_W * scale), round(_H * scale)
        self.font = _font(round(13.5 * scale), bold=True)
        self.font_small = _font(round(12.5 * scale))

    # Zone cliquable de la croix, en pixels de la fenêtre
    def close_center(self):
        return self.w - 28 * self.k, self.h / 2, 15 * self.k

    def hit_close(self, x, y) -> bool:
        cx, cy, r = self.close_center()
        return (x - cx) ** 2 + (y - cy) ** 2 <= (r + 3 * self.k) ** 2

    def render(self, state: str, t: float, bars=None, elapsed: float = 0.0, text: str = "",
               hover: bool = False) -> Image.Image:
        k, s = self.k, self.k * _SS
        big = Image.new("RGBA", (self.w * _SS, self.h * _SS), (0, 0, 0, 0))
        d = ImageDraw.Draw(big)
        W, H = self.w * _SS, self.h * _SS
        d.rounded_rectangle([0, 0, W - 1, H - 1], radius=H // 2, fill=_BORDER)
        d.rounded_rectangle([s, s, W - 1 - s, H - 1 - s], radius=H // 2 - s, fill=_BG)
        cy = H / 2

        if state == "recording":
            # point rouge qui respire, avec halo
            pulse = (math.sin(t * 4) + 1) / 2
            halo = (5 + 5 * pulse) * s
            d.ellipse([26 * s - halo, cy - halo, 26 * s + halo, cy + halo], fill=_mix(_BG, _RED, 0.25 * (1 - pulse)))
            d.ellipse([26 * s - 5 * s, cy - 5 * s, 26 * s + 5 * s, cy + 5 * s], fill=_RED)
            # onde : barres symétriques en dégradé
            x0, x1, bw = 46 * s, 204 * s, 3.4 * s
            pitch = (x1 - x0) / _BARS
            for i, v in enumerate(bars or [0.0] * _BARS):
                hh = (4 + 28 * max(0.0, min(1.0, v))) * s / 2
                x = x0 + i * pitch + (pitch - bw) / 2
                d.rounded_rectangle([x, cy - hh, x + bw, cy + hh], radius=bw / 2,
                                    fill=_mix(_GRAD[0], _GRAD[1], i / (_BARS - 1)))
            self._close(d, s, cy, hover)
        elif state == "processing":
            for i in range(5):
                lift = max(0.0, math.sin(t * 6 - i * 0.75)) * 7 * s
                x = (26 + i * 10) * s
                r = 3.6 * s
                d.ellipse([x - r, cy - lift - r, x + r, cy - lift + r], fill=_mix(_GRAD[0], _GRAD[1], i / 4))
            self._close(d, s, cy, hover)
        else:
            color = _RED if state == "error" else _INFO
            d.ellipse([16 * s, cy - 9 * s, 34 * s, cy + 9 * s], fill=color)

        img = big.resize((self.w, self.h), Image.LANCZOS)
        d = ImageDraw.Draw(img)   # texte dessiné à la taille finale : plus net
        if state == "recording":
            m, sec = divmod(int(elapsed), 60)
            d.text((214 * k, self.h / 2), f"{m}:{sec:02d}", font=self.font_small, fill=_MUTED, anchor="lm")
        elif state == "processing":
            self._text(d, text or "Transcription…", 80 * k, self.w - 50 * k, self.font, _TEXT)
        else:
            d.text((25 * k, self.h / 2 - 0.5 * k), "!" if state == "error" else "i", font=self.font,
                   fill=(255, 255, 255), anchor="mm")
            self._text(d, text, 44 * k, self.w - 18 * k, self.font_small, _TEXT)
        return img

    def _close(self, d, s, cy, hover):
        cx, r = (_W - 28) * s, 15 * s
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=_mix(_BG, _RED, 0.85) if hover else _mix(_BG, (255, 255, 255), 0.09))
        a, lw = 5 * s, max(1, round(2 * s))
        col = (255, 255, 255) if hover else (205, 205, 218)
        d.line([cx - a, cy - a, cx + a, cy + a], fill=col, width=lw)
        d.line([cx - a, cy + a, cx + a, cy - a], fill=col, width=lw)

    def _text(self, d, text, x, x_max, font, fill):
        while text and d.textlength(text, font=font) > x_max - x:
            text = text[:-2] + "…"
        d.text((x, self.h / 2), text, font=font, fill=fill, anchor="lm")


class Overlay:
    def __init__(self, get_level, enabled: bool = True, on_cancel=None):
        self._get_level = get_level
        self._on_cancel = on_cancel
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
            import tkinter as tk
            from PIL import ImageTk
            root = tk.Tk()
        except Exception as e:
            print(f"[Overlay] indisponible : {e}")
            return
        scale = self._scale()
        pill = PillRenderer(scale)
        key_hex = "#%02x%02x%02x" % _KEY
        root.withdraw()
        root.overrideredirect(True)
        root.attributes("-topmost", True)
        root.attributes("-transparentcolor", key_hex)
        root.configure(bg=key_hex)
        canvas = tk.Canvas(root, width=pill.w, height=pill.h, bg=key_hex, highlightthickness=0, bd=0)
        canvas.pack()
        photo = ImageTk.PhotoImage(Image.new("RGB", (pill.w, pill.h), _KEY))
        canvas.create_image(0, 0, anchor="nw", image=photo)
        sw, sh = root.winfo_screenwidth(), root.winfo_screenheight()
        base_x, base_y = (sw - pill.w) // 2, sh - pill.h - round(84 * scale)
        root.geometry(f"{pill.w}x{pill.h}+{base_x}+{base_y}")
        root.update_idletasks()
        self._no_focus(root)

        st = {"state": "hidden", "text": "", "until": 0.0, "visible": False, "since": 0.0, "start": 0.0,
              "hiding": 0.0, "hover": False, "level": 0.0}

        def closable():
            return st["state"] in ("recording", "processing")

        def on_motion(e):
            hover = closable() and pill.hit_close(e.x, e.y)
            if hover != st["hover"]:
                st["hover"] = hover
                canvas.configure(cursor="hand2" if hover else "")

        def on_leave(_e):
            st["hover"] = False
            canvas.configure(cursor="")

        def on_click(e):
            if closable() and pill.hit_close(e.x, e.y) and self._on_cancel:
                try:
                    self._on_cancel()
                except Exception as ex:
                    print(f"[Overlay] annulation impossible : {ex}")

        canvas.bind("<Motion>", on_motion)
        canvas.bind("<Leave>", on_leave)
        canvas.bind("<Button-1>", on_click)

        def bars(t):
            raw = min(1.0, self._get_level() * 14)
            st["level"] = max(raw, st["level"] * 0.86)      # montée immédiate, descente douce
            lvl, out = st["level"], []
            for i in range(_BARS):
                env = math.exp(-((i - (_BARS - 1) / 2) / (_BARS * 0.32)) ** 2)
                wobble = 0.55 + 0.45 * abs(math.sin(t * 8.5 + i * 1.27) * math.cos(t * 3.1 + i * 0.6))
                idle = 0.05 * (1 + math.sin(t * 2.4 + i * 0.45)) / 2
                out.append(idle + lvl * env * wobble)
            return out

        def tick():
            now = time.time()
            try:
                while True:
                    state, text = self._q.get_nowait()
                    if state == "quit":
                        root.destroy()
                        return
                    st["until"] = now + 2.5 if state in ("notice", "error") else 0.0
                    if state == "recording" and st["state"] != "recording":
                        st["start"] = now
                    st["state"], st["text"] = state, text
            except queue.Empty:
                pass

            if st["state"] in ("notice", "error") and now > st["until"]:
                st["state"] = "hidden"
            want = st["state"] != "hidden" and (self.enabled or st["state"] in ("notice", "error"))

            if want:
                st["hiding"] = 0.0
                if not st["visible"]:
                    st["visible"], st["since"] = True, now
                    root.attributes("-alpha", 0.0)
                    root.deiconify()
                appear = min(1.0, (now - st["since"]) / 0.18)
                ease = 1 - (1 - appear) ** 3
                root.attributes("-alpha", ease)
                root.geometry(f"+{base_x}+{base_y + round((1 - ease) * 12 * scale)}")
                photo.paste(self._flatten(pill.render(st["state"], now, bars(now), now - st["start"],
                                                      st["text"], st["hover"])))
                root.after(16 if appear < 1 else 33, tick)
                return
            if st["visible"]:
                if not st["hiding"]:
                    st["hiding"] = now
                fade = (now - st["hiding"]) / 0.14
                if fade < 1:
                    root.attributes("-alpha", 1 - fade)
                    root.after(16, tick)
                    return
                root.withdraw()
                st["visible"], st["hover"] = False, False
                canvas.configure(cursor="")
            root.after(60, tick)

        root.after(60, tick)
        root.mainloop()

    @staticmethod
    def _flatten(img: Image.Image) -> Image.Image:
        base = Image.new("RGB", img.size, _KEY)
        base.paste(img, mask=img.getchannel("A"))
        return base

    @staticmethod
    def _scale() -> float:
        try:
            return max(1.0, ctypes.windll.user32.GetDpiForSystem() / 96)
        except Exception:
            return 1.0

    @staticmethod
    def _no_focus(root):
        """Fenêtre jamais activée (WS_EX_NOACTIVATE) et hors barre des tâches (WS_EX_TOOLWINDOW) :
        on peut cliquer la croix sans retirer le focus à l'application de saisie."""
        try:
            hwnd = ctypes.windll.user32.GetParent(root.winfo_id())
            style = ctypes.windll.user32.GetWindowLongW(hwnd, -20)
            ctypes.windll.user32.SetWindowLongW(hwnd, -20, (style | 0x08000000 | 0x00000080) & ~0x00000020)
        except Exception:
            pass
