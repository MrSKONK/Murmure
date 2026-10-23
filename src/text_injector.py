import time
import pyperclip
import pyautogui

pyautogui.FAILSAFE = False  # un coin de l'écran ne doit pas faire échouer le collage
pyautogui.PAUSE = 0


class TextInjector:
    def inject(self, text: str):
        if not text:
            return

        # None = lecture impossible : on ne touchera pas au presse-papiers après coup
        try:
            previous_clipboard = pyperclip.paste()
        except Exception:
            previous_clipboard = None

        try:
            pyperclip.copy(text)
            time.sleep(0.05)  # laisser le temps au presse-papiers d'être mis à jour
            pyautogui.hotkey("ctrl", "v")
            time.sleep(0.2)   # laisser l'application lire le presse-papiers avant restauration
        finally:
            # Restaure l'ancien contenu, y compris vide
            if previous_clipboard is not None:
                try:
                    pyperclip.copy(previous_clipboard)
                except Exception:
                    pass
