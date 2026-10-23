import ctypes
import os
import sys

# Ajoute le dossier racine au path pour les imports src.*
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# Sous pythonw.exe (lancement sans console) stdout/stderr n'existent pas
if sys.stdout is None:
    sys.stdout = open(os.devnull, "w")
if sys.stderr is None:
    sys.stderr = open(os.devnull, "w")

try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)  # rendu net sur écrans à forte densité
except Exception:
    pass


def main():
    from src.app import App, acquire_single_instance

    lock = acquire_single_instance()
    if lock is None:
        return  # déjà lancée : l'instance existante affiche sa fenêtre
    App(background="--background" in sys.argv).run(lock)


if __name__ == "__main__":
    main()
