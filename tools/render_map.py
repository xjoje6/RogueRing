"""Dibuja un mapa generado por rogue-core sobre el fondo gris de ER (vista previa sin abrir el juego).
Uso: cargo run --example mapa -- SEMILLA | python tools/render_map.py salida.png
Iconos provisionales (letras) hasta extraer los del juego.
"""
import sys, json, os, random
from PIL import Image, ImageDraw, ImageFont, ImageFilter

data = json.load(sys.stdin)
m = data["map"]; s = m["shape"]
cols, rooms = s["columns"], s["rooms"]
W, H = 900, 1400
root = os.path.join(os.path.dirname(__file__), "..")
bg = Image.open(os.path.join(root, "extract", "ui", "mapa_L2_gris.png")).convert("RGB")
# Encuadre vertical centrado en la masa de tierra.
bw, bh = bg.size; scale = max(W / bw, H / bh) * 1.05
bg = bg.resize((int(bw * scale), int(bh * scale)))
bg = bg.crop(((bg.width - W) // 2, (bg.height - H) // 2, (bg.width - W) // 2 + W, (bg.height - H) // 2 + H))
from PIL import ImageEnhance
img = ImageEnhance.Brightness(ImageEnhance.Contrast(bg).enhance(0.75)).enhance(0.62); d = ImageDraw.Draw(img)

rnd = random.Random(data["seed"])
top, bottom, left, right = 150, H - 150, 110, W - 110
def pos(c, r):
    x = left + (right - left) * c / (cols - 1)
    y = bottom - (bottom - top) * r / (rooms + 1)
    if 0 < r <= rooms:  # desplazamiento pequeño como en StS para que no parezca una rejilla
        jr = random.Random(f"{data['seed']}-{c}-{r}")
        x += jr.uniform(-18, 18); y += jr.uniform(-14, 14)
    return x, y

INK = (225, 218, 200); DIM = (196, 188, 170)
font = ImageFont.truetype("georgia.ttf", 22); small = ImageFont.truetype("georgia.ttf", 18); big = ImageFont.truetype("georgiab.ttf", 34)
def dotted(a, b, col, r0=24, r1=24):
    (x0, y0), (x1, y1) = a, b
    L = ((x1 - x0) ** 2 + (y1 - y0) ** 2) ** .5
    n = int((L - r0 - r1) // 12)
    for i in range(n + 1):
        t = (r0 + i * 12) / L
        cx, cy = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
        d.ellipse((cx - 2.6, cy - 2.6, cx + 2.6, cy + 2.6), fill=col)

nodes = {(n["col"], n["row"]): n for n in m["nodes"]}
start, boss = tuple(m["start"]), tuple(m["boss"])
for n in m["nodes"]:
    for ch in n["children"]:
        dotted(pos(n["col"], n["row"]), pos(*ch), DIM)
    if n["row"] == 1: dotted(pos(*start), pos(n["col"], 1), DIM, 34)
    if n["row"] == rooms: dotted(pos(n["col"], n["row"]), pos(*boss), DIM, 24, 60)

LET = {"Monster": ("C", INK), "Elite": ("E", (214, 120, 96)), "Rest": ("G", (222, 196, 120)), "Shop": ("T", INK),
       "Treasure": ("$", (222, 196, 120)), "Unknown": ("?", INK)}
for n in m["nodes"]:
    x, y = pos(n["col"], n["row"]); ch, col = LET[n["kind"]]
    d.ellipse((x - 20, y - 20, x + 20, y + 20), fill=(34, 32, 29), outline=col, width=2)
    d.text((x, y + 1), ch, font=font, fill=col, anchor="mm")
x, y = pos(*start); d.ellipse((x - 30, y - 30, x + 30, y + 30), fill=(34, 32, 29), outline=INK, width=2); d.text((x, y), "N", font=big, fill=INK, anchor="mm")
x, y = pos(*boss); d.ellipse((x - 56, y - 56, x + 56, y + 56), fill=(34, 32, 29), outline=(214, 120, 96), width=3); d.text((x, y), "BOSS", font=font, fill=(214, 120, 96), anchor="mm")

leg = [("C", "Combate"), ("E", "Élite"), ("G", "Gracia"), ("T", "Tienda"), ("$", "Tesoro"), ("?", "Evento")]
for i, (k, t) in enumerate(leg):
    d.text((24, 24 + i * 24), f"{k}  {t}", font=small, fill=INK)
d.text((W - 24, 24), f"Semilla {data['seed']} · Acto {data['act']}", font=small, fill=INK, anchor="ra")
img.save(sys.argv[1])
