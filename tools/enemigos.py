"""Enemigos del mod y del juego, con su imagen, para el editor de combates (revision_enemigos/).

Uso: python tools/enemigos.py [--sin-red]

- Usados: cada encuentro de data/acts y data/coop con sus enemigos (modelo cXXXX y su nombre de los alias
  de Smithbox; los humanos c0000, con el nombre del encuentro).
- Sin usar: los modelos del juego (alias de Smithbox) que no sale en ningún encuentro, con el motivo
  conocido (docs/LISTAS_Y_COMBATES.md) o «sin revisar».
- Imagen de cada nombre: la de la Elden Ring Fan API (eldenring.fanapis.com, jefes, criaturas y PNJ) y,
  si no está, la de su ficha en la wiki de Fextralife. Se guarda en miniatura (revision_enemigos/img/) y
  el índice en revision_enemigos/enemigos.json. Las que ya están no se vuelven a bajar.
"""
import glob
import io
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT = os.path.join(ROOT, "revision_enemigos")
IMG = os.path.join(OUT, "img")
ALIAS = os.path.join(ROOT, "tools", "vendor", "Smithbox", "Smithbox.Release", "Output", "Assets", "Aliases", "ER", "Characters.json")
UA = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) RogueRing-mod (uso personal)"}
LADO = 112

# Por qué no se usa cada modelo: data/enemigos_fuera.json (tools/ampliar_enemigos.py y la validación del banco).
FUERA = os.path.join(ROOT, "data", "enemigos_fuera.json")


def motivos():
    return json.load(open(FUERA, encoding="utf8")).get("fuera", {}) if os.path.exists(FUERA) else {}


def json_url(url):
    return json.loads(urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read())


def texto_url(url):
    return urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read().decode("utf8", "replace")


def norm(n):
    return re.sub(r"[^a-z0-9]", "", n.lower())


def fan_api():
    """Nombre normalizado -> imagen, de jefes, criaturas y PNJ de la Fan API."""
    m = {}
    for tipo in ("bosses", "creatures", "npcs"):
        pag = 0
        while True:
            d = json_url(f"https://eldenring.fanapis.com/api/{tipo}?limit=100&page={pag}")
            for e in d.get("data", []):
                if e.get("image"):
                    m.setdefault(norm(e["name"]), e["image"])
            if len(d.get("data", [])) < 100:
                break
            pag += 1
    return m


def limpio(nombre):
    """Nombre de la ficha: lo de antes de «/» y sin paréntesis («Stray (Large)» -> «Stray»)."""
    return re.sub(r"\s*\(.*?\)", "", nombre.split("/")[0]).strip()


def wiki(nombre):
    """Imagen principal de la ficha de la wiki de Fextralife (la primera imagen del artículo). Despacio y con
    reintentos: con muchas seguidas corta la conexión."""
    url = "https://eldenring.wiki.fextralife.com/" + urllib.parse.quote(limpio(nombre).replace(" ", "_"), safe="_")
    html = None
    for intento in range(3):
        try:
            html = texto_url(url)
            break
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
        except Exception:
            pass
        time.sleep(10 * (intento + 1))
    if html is None:
        return None
    for m in re.finditer(r"https://static0\.fextralifeimages\.com/file/eldenring/[0-9a-f]/[0-9a-f]{2}/[^\"'?\s]+", html):
        return m.group(0)
    return None


def miniatura(url, destino):
    datos = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read()
    im = Image.open(io.BytesIO(datos)).convert("RGB")
    im.thumbnail((LADO * 2, LADO * 2))
    # Recorte cuadrado centrado y a LADO.
    w, h = im.size
    s = min(w, h)
    im = im.crop(((w - s) // 2, (h - s) // 2, (w - s) // 2 + s, (h - s) // 2 + s)).resize((LADO, LADO))
    im.save(destino, "JPEG", quality=82)


def main():
    os.makedirs(IMG, exist_ok=True)
    alias = {a["ID"]: a for a in json.load(open(ALIAS, encoding="utf8"))}
    es = json.load(open(os.path.join(ROOT, "data", "i18n", "es.json"), encoding="utf8"))
    nombres_es = es.get("enemigos", {})
    usados, chrs = [], set()
    # Parejas posibles del cooperativo (las sortea el mod): no van como cartas, solo cuántas hay y quién
    # puede entrar en ellas.
    mezclas, en_mezclas = {}, set()
    for f in sorted(glob.glob(os.path.join(ROOT, "data", "coop", "mezclas_*.json"))):
        acto = os.path.splitext(os.path.basename(f))[0].split("_")[1]
        for e in json.load(open(f, encoding="utf8"))["encounters"]:
            mezclas.setdefault(acto, {}).setdefault(e["kind"], 0)
            mezclas[acto][e["kind"]] += 1
            en_mezclas |= {x["npc"] for x in e["enemigos"]}
    for carpeta in ("acts", "coop"):
        for f in sorted(glob.glob(os.path.join(ROOT, "data", carpeta, "*.json"))):
            acto = os.path.splitext(os.path.basename(f))[0]
            if acto.startswith("mezclas_"):
                continue
            for e in json.load(open(f, encoding="utf8"))["encounters"]:
                ens = e.get("enemigos") or [{"chr": e.get("chr"), "npc": e.get("npc"), "think": e.get("think")}]
                enemigos = []
                for x in ens:
                    c = f"c{int(x.get('chr') or 0):04d}"
                    chrs.add(c)
                    nombre = e["name"] if c == "c0000" or c not in alias else alias[c]["Name"]
                    enemigos.append({"chr": c, "npc": x.get("npc"), "nombre": nombre, "nombre_es": nombres_es.get(nombre, "")})
                usados.append({"lista": carpeta, "acto": acto, "id": e["id"], "nombre": e["name"], "nombre_es": nombres_es.get(e["name"], ""),
                               "tipo": e["kind"], "formato": e.get("formato", "solo"), "vida": e.get("vida"), "enemigos": enemigos,
                               "mezcla": "mezcla" in e.get("tags", []), "nuevo": "nuevo_24_10" in e.get("tags", []),
                               "mezclable": carpeta == "acts" and e.get("npc") in en_mezclas, "arena": e.get("arena")})
    sin_usar, por_que = [], motivos()
    for c, a in alias.items():
        tags = set(a.get("Tags", []))
        if c in chrs or not ({"creature", "boss"} & tags) or {"horse", "trap", "debug", "interactable_placeholder", "player"} & tags:
            continue
        sin_usar.append({"chr": c, "nombre": a["Name"], "nombre_es": nombres_es.get(a["Name"], ""), "dlc": "sote" in tags, "jefe": "boss" in tags, "motivo": por_que.get(c, "Sin revisar todavía.")})
    print(f"{len(usados)} encuentros, {len(chrs)} modelos usados; {len(sin_usar)} modelos sin usar")

    indice_f = os.path.join(OUT, "imagenes.json")
    indice = json.load(open(indice_f, encoding="utf8")) if os.path.exists(indice_f) else {}
    nombres = sorted({x["nombre"] for u in usados for x in u["enemigos"]} | {s["nombre"] for s in sin_usar})
    # Otros nombres con los que buscar su ficha: el del encuentro cuando todos sus enemigos son ese modelo
    # (el alias «Rennala» es «Rennala, Queen of the Full Moon» en la wiki).
    otros = {}
    for u in usados:
        if len({x["nombre"] for x in u["enemigos"]}) == 1:
            otros.setdefault(u["enemigos"][0]["nombre"], set()).add(u["nombre"])
    falta = [n for n in nombres if not indice.get(n, {}).get("archivo")]
    # --sin-red: solo rehace el índice con las imágenes que ya hay.
    if falta and "--sin-red" not in sys.argv:
        api = fan_api()
        print(f"Fan API: {len(api)} imágenes; faltan {len(falta)} nombres")
        for n in falta:
            url = api.get(norm(n)) or api.get(norm(limpio(n))) or next((api[norm(o)] for o in otros.get(n, ()) if norm(o) in api), None) or wiki(n)
            for o in sorted(otros.get(n, ())):
                if url:
                    break
                time.sleep(4)
                url = wiki(o)
            archivo = None
            if url:
                archivo = re.sub(r"[^a-z0-9]+", "_", n.lower()).strip("_") + ".jpg"
                try:
                    miniatura(url, os.path.join(IMG, archivo))
                except Exception as e:
                    print(f"  {n}: {e}")
                    archivo = None
            indice[n] = {"url": url, "archivo": archivo}
            print(f"  {n}: {'ok' if archivo else 'sin imagen'}")
            time.sleep(4)
        json.dump(indice, open(indice_f, "w", encoding="utf8"), ensure_ascii=False, indent=1)
    con = sum(1 for n in nombres if indice.get(n, {}).get("archivo"))
    print(f"imágenes: {con} de {len(nombres)}")
    json.dump({"usados": usados, "sin_usar": sin_usar, "mezclas": mezclas, "imagenes": {n: indice.get(n, {}).get("archivo") for n in nombres}},
              open(os.path.join(OUT, "enemigos.json"), "w", encoding="utf8"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
