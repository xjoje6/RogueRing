"""Fondos del mapa: el mapa del juego descubierto entero, sin costuras.

Requiere extract/maptiles_todas/ (Extractor maptile <raíz>): de cada tesela, solo la variante cuya
máscara es la de 71_maptile.mtmskbnd (todos sus fragmentos). Mezclar otras combinaciones deja
costuras y trozos repetidos.
Uso: python tools/map_background.py
Salida (fuera de git, son recursos del juego): extract/ui/fondos/acto{1,2,3}.png (1920x1080), un
tramo del mapa por acto.
"""
import glob, os, re
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SRC = os.path.join(ROOT, "extract", "maptiles_todas")
DST = os.path.join(ROOT, "extract", "ui", "fondos")
T = 256


def montar(mapa, nivel):
    fs = glob.glob(os.path.join(SRC, f"MENU_MapTile_{mapa}_L{nivel}_*.dds"))
    idx = {tuple(map(int, re.search(r"_(\d\d)_(\d\d)_[0-9a-f]{8}\.dds", f).groups())): f for f in fs}
    n = max(max(k) for k in idx) + 1
    img = Image.new("RGB", (n * T, n * T))
    for (a, b), f in idx.items():
        img.paste(Image.open(f).convert("RGB"), (a * T, (n - 1 - b) * T))  # a = columna, b = fila desde abajo
    return img


# Tramo de cada acto en el mapa de nivel 1 (31x31 teselas, 7936 px): (mapa, x, y, ancho), 16:9.
ACTOS = {
    1: ("M00", 1510, 3970, 3650),  # Necrolimbo, la península y Caelid: donde empieza todo
    2: ("M00", 1980, 640, 3890),   # Altiplano de Altus, Leyndell y las cumbres de los Gigantes
    3: ("M10", 2200, 2880, 3560),  # la Tierra Sombría (mapa del DLC)
}

if __name__ == "__main__":
    os.makedirs(DST, exist_ok=True)
    mapas = {}
    for acto, (mapa, x, y, w) in ACTOS.items():
        if mapa not in mapas:
            mapas[mapa] = montar(mapa, 1)
        h = w * 9 // 16
        mapas[mapa].crop((x, y, x + w, y + h)).resize((1920, 1080), Image.LANCZOS).save(os.path.join(DST, f"acto{acto}.png"))
        print("acto", acto, mapa, (x, y, w, h))
