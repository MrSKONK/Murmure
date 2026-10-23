# Construction de Murmure.exe : python -m PyInstaller --noconfirm --clean packaging/murmure.spec
# Résultat : dist/Murmure/Murmure.exe (dossier autonome, Python inutile sur le PC).
import os
import sys

from PyInstaller.utils.hooks import collect_data_files, collect_dynamic_libs, collect_submodules

ROOT = os.path.abspath(os.path.join(SPECPATH, ".."))
sys.path.insert(0, ROOT)
from src.icon import save_ico  # noqa: E402

ICON = os.path.join(workpath, "murmure.ico")
os.makedirs(workpath, exist_ok=True)
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
