"""Logo Murmure : bulle de dialogue contenant des lignes de texte et un curseur, colorée selon l'état."""
from PIL import Image, ImageDraw

_ICON_COLORS = {
    "loading": (130, 130, 140),
    "idle": (91, 91, 214),
    "recording": (230, 70, 70),
    "processing": (240, 160, 50),
    "error": (150, 0, 0),
}
_INK = (230, 228, 255)   # lignes et curseur, lavande très claire
_SUPERSAMPLE = 4         # dessin en grand puis réduction : bords lisses à toutes les tailles


def make_icon(state: str, size: int = 64) -> Image.Image:
    color = _ICON_COLORS.get(state, _ICON_COLORS["idle"])
    big = size * _SUPERSAMPLE
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    def box(x0, y0, x1, y1):   # coordonnées en fraction de la taille
        return [x0 * big, y0 * big, x1 * big, y1 * big]

    def bar(x0, y0, x1, y1):
        d.rounded_rectangle(box(x0, y0, x1, y1), radius=(y1 - y0 if y1 - y0 < x1 - x0 else x1 - x0) * big / 2, fill=_INK)

    # Bulle et sa pointe en bas à gauche
    d.ellipse(box(0.04, 0.08, 0.96, 0.80), fill=color)
    d.polygon([(0.17 * big, 0.62 * big), (0.42 * big, 0.74 * big), (0.12 * big, 0.95 * big)], fill=color)

    # Texte : trois lignes en grand, deux en petit pour rester lisible dans la barre des tâches
    t = 0.085 if size >= 40 else 0.11
    if size >= 40:
        rows = [(0.25, 0.73, 0.27), (0.25, 0.58, 0.40), (0.25, 0.58, 0.53)]
        cursor = (0.655, 0.385, 0.655 + t * 0.62, 0.53 + t)
    else:
        rows = [(0.22, 0.76, 0.30), (0.22, 0.56, 0.48)]
        cursor = (0.65, 0.42, 0.65 + t * 0.75, 0.48 + t + 0.06)
    for x0, x1, y in rows:
        bar(x0, y, x1, y + t)
    bar(*cursor)

    return img.resize((size, size), Image.LANCZOS)


def save_ico(path: str):
    """Icône du .exe (état « prêt ») : chaque taille dessinée séparément."""
    sizes = [16, 24, 32, 48, 64, 128, 256]
    images = [make_icon("idle", s) for s in sizes]
    images[-1].save(path, sizes=[(s, s) for s in sizes], append_images=images[:-1])
