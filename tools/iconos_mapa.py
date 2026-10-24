"""Recorta los iconos del mapa de ER (atlas SB_MapCursor* de menu/hi/01_common) para los nodos del
mapa de la run. Requiere extract/ui/sb/ (Extractor ui). Salida: extract/ui/mapa/<nodo>.png"""
import os, re
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SB = os.path.join(ROOT, "extract", "ui", "sb")
OUT = os.path.join(ROOT, "extract", "ui", "mapa")
NODOS = {
    "combate": "MENU_MAP_memo_21",   # marcador de espada
    "elite": "MENU_MAP_memo_20",     # marcador de calavera
    "gracia": "MENU_MAP_01_Bonfire", # gracia
    "mercader": "MENU_MAP_80",       # nómada encapuchado
    "tesoro": "MENU_MAP_memo_26",    # marcador de cofre
    "evento": "MENU_MAP_Church",     # capilla
    "jefe": "MENU_MAP_Enemy_02",     # espadas cruzadas en rojo
    "inicio": "MENU_MAP_Bearing",    # rosa de los vientos
}
subs = {}
for atlas in ["SB_MapCursor", "SB_MapCursor_02", "SB_MapCursor_03_dlc"]:
    im = Image.open(os.path.join(SB, atlas + ".dds")).convert("RGBA")
    for m in re.finditer(r'name="([^"]+)\.png" x="(\d+)" y="(\d+)" width="(\d+)" height="(\d+)"', open(os.path.join(SB, atlas + ".layout"), encoding="utf8").read()):
        x, y, w, h = map(int, m.groups()[1:])
        subs.setdefault(m.group(1), im.crop((x, y, x + w, y + h)))
os.makedirs(OUT, exist_ok=True)
for nodo, name in NODOS.items():
    img = subs[name]
    side = max(img.size)
    sq = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    sq.alpha_composite(img, ((side - img.size[0]) // 2, (side - img.size[1]) // 2))
    sq.resize((128, 128), Image.LANCZOS).save(os.path.join(OUT, nodo + ".png"))
print("ok", OUT)
