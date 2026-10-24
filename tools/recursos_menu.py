"""Recorta piezas de los menús de ER (atlas SB_* de menu/hi/01_common) para las pantallas del mod.
Requiere extract/ui/sb/ (Extractor ui). Salida: extract/ui/menu/<nombre>.png"""
import os, re
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SB = os.path.join(ROOT, "extract", "ui", "sb")
OUT = os.path.join(ROOT, "extract", "ui", "menu")
PIEZAS = {
    "panel": "MENU_FL_Panel01",      # panel oscuro con esquinas doradas (cartas)
    "cursor": "MENU_FL_Tab_Cursor",  # resalte dorado con chispas de la opción elegida
    "subrayado": "MENU_Title_Cursor", # brillo naranja de la pantalla de título
    "brillo": "MENU_Cursor_05",      # brillo alrededor de la carta elegida
    "runa": "MENU_FL_RuneDeco",      # runa decorativa de fondo
    "barra_jefe": "MENU_FL_BarDeco_Boss",
    # Piezas de las barras de vida del juego (barra del jefe y barras de carga del mod).
    "barra_base": "MENU_FL_HP_Base",     # fondo oscuro translúcido
    "barra_relleno": "MENU_HP_Bar",      # relleno gris: se tiñe (rojo de vida, oro de carga)
    "barra_filete": "MENU_FL_BarDeco",   # filete dorado con su remate a la derecha
    "barra_borde": "MENU_FL_BarEdge",    # tope del relleno
    "carga_icono": "MENU_Loading_02",    # icono de la pantalla de carga del juego
    # Iconos de los estados (SB_Condition), en el orden del módulo de resistencias: bajo la barra del rival.
    "estado_veneno": "MENU_RE_Poison",
    "estado_podredumbre": "MENU_RE_Plague",
    "estado_sangrado": "MENU_RE_bleed",
    "estado_plaga": "MENU_RE_Death",
    "estado_congelacion": "MENU_RE_Cold",
    "estado_sueno": "MENU_RE_Sleep",
    "estado_locura": "MENU_RE_Mad",
    # Menús nuevos (19-10): las piezas de la tienda y el inventario del juego.
    "marco_objeto": "MENU_Item_waku",          # marco de casilla de objeto (listas)
    "marco_ficha": "MENU_FL_Arts_wakuDeco",    # marco octogonal del icono grande (ficha)
    "filete": "MENU_FL_Equip_wakuDeco",        # filete ornamental bajo el nombre
    "casilla": "MENU_SlotBase",                # casilla vacía del inventario
    "mapa_cursor": "MENU_MAP_Cursor_01",       # cursor del mapa del mundo
    "mapa_coop": "MENU_MAP_Coop_01",           # marcador de jugador del mapa (compañeros)
    "mapa_amigo": "MENU_MAP_Friend_01",        # emblema azul de jugador (destino elegido)
}
# Texturas sueltas (no van en un atlas): banda negra del «HAS MUERTO».
SUELTAS = {"banda": "MENU_KUROOBI"}
subs = {}
for lay in os.listdir(SB):
    if not lay.endswith(".layout"):
        continue
    dds = os.path.join(SB, lay[:-7] + ".dds")
    if not os.path.exists(dds):
        continue
    txt = open(os.path.join(SB, lay), encoding="utf8").read()
    want = [n for n in PIEZAS.values() if f'"{n}.png"' in txt]
    if not want:
        continue
    im = Image.open(dds).convert("RGBA")
    for m in re.finditer(r'name="([^"]+)\.png" x="(\d+)" y="(\d+)" width="(\d+)" height="(\d+)"', txt):
        if m.group(1) in want:
            x, y, w, h = map(int, m.groups()[1:])
            subs[m.group(1)] = im.crop((x, y, x + w, y + h))
os.makedirs(OUT, exist_ok=True)
ADITIVAS = {"subrayado"}  # fondo negro opaco (se suma): pasar a transparencia
for nombre, sub in PIEZAS.items():
    img = subs[sub]
    if nombre in ADITIVAS:
        px = img.load()
        for yy in range(img.size[1]):
            for xx in range(img.size[0]):
                r, g, b, _ = px[xx, yy]
                a = max(r, g, b)
                px[xx, yy] = (min(255, r * 255 // a), min(255, g * 255 // a), min(255, b * 255 // a), a) if a else (0, 0, 0, 0)
    if nombre == "barra_relleno":
        # El relleno es gris oscuro (el juego lo tiñe con más de 1): se aclara para que el tinte
        # dé el rojo de la vida y no un granate apagado, conservando su textura.
        px = img.load()
        for yy in range(img.size[1]):
            for xx in range(img.size[0]):
                r, g, b, a = px[xx, yy]
                px[xx, yy] = (min(255, r * 23 // 10), min(255, g * 23 // 10), min(255, b * 23 // 10), a)
    img.save(os.path.join(OUT, nombre + ".png"))
    print(nombre, subs[sub].size)
for nombre, sub in SUELTAS.items():
    img = Image.open(os.path.join(SB, sub + ".dds")).convert("RGBA")
    img.save(os.path.join(OUT, nombre + ".png"))
    print(nombre, img.size)
# Logo del mod (hecho por Codex desde el título del juego, runtime/logo; fuera de git): a la mitad,
# de sobra para pantalla completa.
LOGO = os.path.join(ROOT, "runtime", "logo", "rogue_ring_titulo.png")
if os.path.exists(LOGO):
    img = Image.open(LOGO).convert("RGBA")
    img.resize((img.width // 2, img.height // 2), Image.LANCZOS).save(os.path.join(OUT, "logo.png"))
    print("logo", (img.width // 2, img.height // 2))
else:
    print("falta el logo:", LOGO)
