"""Icône de l'application : pastille colorée selon l'état, avec un micro blanc."""
from PIL import Image, ImageDraw

_ICON_COLORS = {
    "loading": (130, 130, 140),
    "idle": (91, 91, 214),
    "recording": (230, 70, 70),
    "processing": (240, 160, 50),
    "error": (150, 0, 0),
}


def make_icon(state: str, size: int = 64) -> Image.Image:
    color = _ICON_COLORS.get(state, _ICON_COLORS["idle"])
    k = size / 64   # coordonnées pensées pour 64 px, mises à l'échelle
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.ellipse([2 * k, 2 * k, 62 * k, 62 * k], fill=color)
    white = (255, 255, 255, 255)
    w = max(1, round(4 * k))
    d.rounded_rectangle([24 * k, 12 * k, 40 * k, 38 * k], radius=8 * k, fill=white)   # capsule du micro
    d.arc([17 * k, 24 * k, 47 * k, 48 * k], 0, 180, fill=white, width=w)            # arceau
    d.line([32 * k, 48 * k, 32 * k, 54 * k], fill=white, width=w)
    d.line([24 * k, 54 * k, 40 * k, 54 * k], fill=white, width=w)
    return img


def save_ico(path: str):
    """Icône du .exe (état « prêt »), en plusieurs tailles."""
    make_icon("idle", 256).save(
        path, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
