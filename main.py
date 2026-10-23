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


def self_check(report_path: str) -> int:
    """Vérifie que tous les composants sont présents (sert à valider l'exécutable construit)."""
    lines, ok = [], True
    try:
        import glob

        import ctranslate2
        import faster_whisper
        import onnxruntime  # noqa: F401  (filtre des silences de Faster-Whisper)

        from src import app  # noqa: F401  (fenêtre, icône, raccourci, micro, pastille)
        import webview.platforms.winforms  # noqa: F401  (pont .NET et WebView2 utilisés par la fenêtre)
        from src.config import BASE_DIR

        vad = glob.glob(os.path.join(os.path.dirname(faster_whisper.__file__), "assets", "*.onnx"))
        from src import caret, updater
        from src.version import __version__
        uia = caret.self_test()
        lines.append(f"Murmure {__version__}, mises à jour depuis {updater.RELEASES_REPO}")
        ui = os.path.join(BASE_DIR, "ui", "index.html")
        lines.append(f"ctranslate2 {ctranslate2.__version__}, faster-whisper {faster_whisper.__version__}")
        lines.append(f"modele VAD : {vad or 'ABSENT'}")
        lines.append(f"interface : {ui} {'ok' if os.path.exists(ui) else 'ABSENTE'}")
        lines.append(f"UI Automation (texte avant le curseur) : {uia}")
        ok = bool(vad) and os.path.exists(ui)
    except Exception as e:
        import traceback
        lines.append(traceback.format_exc())
        ok = False
    lines.append("OK" if ok else "ECHEC")
    with open(report_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    return 0 if ok else 1


def main():
    if "--check" in sys.argv:
        sys.exit(self_check(sys.argv[sys.argv.index("--check") + 1]))

    from src.app import App, acquire_single_instance

    lock = acquire_single_instance()
    if lock is None:
        return  # déjà lancée : l'instance existante affiche sa fenêtre
    App(background="--background" in sys.argv).run(lock)


if __name__ == "__main__":
    main()
