"""Lancement automatique avec Windows (clé Run de l'utilisateur, sans droits administrateur)."""
import os
import sys

from src.config import APP_NAME, FROZEN

_KEY = r"Software\Microsoft\Windows\CurrentVersion\Run"
_NAME = APP_NAME
_OLD_NAMES = ("SpeechToText",)   # entrées laissées par les versions précédentes


def _command() -> str:
    if FROZEN:
        return f'"{sys.executable}" --background'
    exe = sys.executable
    pythonw = os.path.join(os.path.dirname(exe), "pythonw.exe")
    if os.path.exists(pythonw):
        exe = pythonw
    main = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "main.py")
    return f'"{exe}" "{main}" --background'


def is_enabled() -> bool:
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, _KEY) as k:
            winreg.QueryValueEx(k, _NAME)
            return True
    except Exception:
        return False


def set_enabled(enabled: bool):
    import winreg
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, _KEY, 0, winreg.KEY_SET_VALUE) as k:
        for old in _OLD_NAMES:
            try:
                winreg.DeleteValue(k, old)
            except FileNotFoundError:
                pass
        if enabled:
            winreg.SetValueEx(k, _NAME, 0, winreg.REG_SZ, _command())
        else:
            try:
                winreg.DeleteValue(k, _NAME)
            except FileNotFoundError:
                pass
