"""Mises à jour depuis GitHub Releases : vérification, téléchargement vérifié (SHA-256), installation.

Les releases sont publiées dans un dépôt public dédié (le code reste privé) : aucune clé n'est embarquée."""
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.request

from src.version import __version__

RELEASES_REPO = "MrSKONK/Murmure"
_API = f"https://api.github.com/repos/{RELEASES_REPO}/releases/latest"
_SETUP = re.compile(r"^Murmure-Setup-(\d+(?:\.\d+){1,3})\.exe$")
_UA = {"User-Agent": f"Murmure/{__version__}", "Accept": "application/vnd.github+json"}


def parse_version(text: str) -> tuple[int, ...]:
    nums = re.findall(r"\d+", text or "")
    return tuple(int(n) for n in nums[:4]) or (0,)


def is_newer(candidate: str, current: str = __version__) -> bool:
    return parse_version(candidate) > parse_version(current)


def pick_release(data: dict) -> dict | None:
    """Extrait d'une réponse « releases/latest » l'installateur et son empreinte, ou None."""
    if not isinstance(data, dict) or data.get("draft") or data.get("prerelease"):
        return None
    assets = {a.get("name"): a for a in data.get("assets") or [] if isinstance(a, dict)}
    for name, asset in assets.items():
        m = _SETUP.match(name or "")
        if not m or not str(asset.get("browser_download_url", "")).startswith("https://github.com/"):
            continue
        digest = assets.get(name + ".sha256")
        if not digest:
            return None   # sans empreinte, on n'installe rien
        return {"version": m.group(1), "notes": (data.get("body") or "").strip(),
                "url": asset["browser_download_url"], "size": int(asset.get("size") or 0),
                "sha256_url": digest["browser_download_url"]}
    return None


def check(timeout: float = 10) -> dict | None:
    """Renvoie la release plus récente que la version installée, sinon None."""
    req = urllib.request.Request(_API, headers=_UA)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        release = pick_release(json.load(r))
    return release if release and is_newer(release["version"]) else None


def download(release: dict, progress=None) -> str:
    """Télécharge l'installateur dans le dossier temporaire et vérifie son empreinte SHA-256."""
    with urllib.request.urlopen(urllib.request.Request(release["sha256_url"], headers=_UA), timeout=20) as r:
        expected = r.read().decode("ascii", "replace").split()[0].strip().lower()
    if not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError("empreinte de la mise à jour illisible")

    path = os.path.join(tempfile.gettempdir(), f"Murmure-Setup-{release['version']}.exe")
    digest, done = hashlib.sha256(), 0
    req = urllib.request.Request(release["url"], headers={"User-Agent": _UA["User-Agent"]})
    with urllib.request.urlopen(req, timeout=30) as r, open(path + ".part", "wb") as f:
        total = int(r.headers.get("Content-Length") or release.get("size") or 0)
        while chunk := r.read(1 << 16):
            f.write(chunk)
            digest.update(chunk)
            done += len(chunk)
            if progress and total:
                progress(done / total)
    if digest.hexdigest() != expected:
        os.remove(path + ".part")
        raise ValueError("le fichier téléchargé est corrompu (empreinte différente)")
    os.replace(path + ".part", path)
    return path


def launch_installer(path: str):
    """Installe en silence ; l'installateur ferme Murmure puis le relance une fois terminé."""
    flags = 0x00000008 | 0x00000200 if sys.platform == "win32" else 0   # DETACHED_PROCESS | NEW_PROCESS_GROUP
    subprocess.Popen([path, "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"],
                     creationflags=flags, close_fds=True)
