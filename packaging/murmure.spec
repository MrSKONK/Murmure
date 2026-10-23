# Construction de Murmure.exe : python -m PyInstaller --noconfirm --clean packaging/murmure.spec
# Résultat : dist/Murmure/Murmure.exe (dossier autonome, Python inutile sur le PC).
import os
import sys

from PyInstaller.utils.hooks import collect_data_files, collect_dynamic_libs, collect_submodules

ROOT = os.path.abspath(os.path.join(SPECPATH, ".."))
sys.path.insert(0, ROOT)
from src.icon import save_ico  # noqa: E402

ICON = os.path.join(ROOT, "build", "murmure.ico")   # aussi utilisée par l'installateur (murmure.iss)
os.makedirs(os.path.dirname(ICON), exist_ok=True)
save_ico(ICON)

datas = [(os.path.join(ROOT, "ui"), "ui")]
datas += collect_data_files("faster_whisper")      # modèle du filtre de silences (silero_vad)
datas += collect_data_files("webview")             # pont .NET / WebView2 de la fenêtre
binaries = collect_dynamic_libs("ctranslate2") + collect_dynamic_libs("onnxruntime")
hiddenimports = collect_submodules("pystray") + collect_submodules("webview.platforms")

# Interface UI Automation (lecture du texte avant le curseur) : générée ici pour être embarquée
import comtypes.client  # noqa: E402
comtypes.client.GetModule("UIAutomationCore.dll")
hiddenimports += collect_submodules("comtypes.gen")

# Grammalecte est téléchargé à la demande (pur Python) : la bibliothèque standard dont il peut avoir besoin
# doit être embarquée même si Murmure ne l'importe pas lui-même.
hiddenimports += ["json", "re", "traceback", "itertools", "collections", "functools", "unicodedata", "copy",
                  "random", "math", "time", "datetime", "string", "textwrap", "html", "html.parser", "platform",
                  "getpass", "argparse", "importlib", "importlib.resources", "urllib.parse", "zipfile", "bisect",
                  "heapq", "struct", "array", "operator", "difflib"]

a = Analysis(
    [os.path.join(ROOT, "main.py")],
    pathex=[ROOT],
    datas=datas,
    binaries=binaries,
    hiddenimports=hiddenimports,
    excludes=["torch", "tensorflow", "matplotlib", "pandas", "IPython"],
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz, a.scripts, [],
    exclude_binaries=True,
    name="Murmure",
    icon=ICON,
    console=False,
    upx=False,
)
coll = COLLECT(exe, a.binaries, a.datas, name="Murmure", upx=False)
