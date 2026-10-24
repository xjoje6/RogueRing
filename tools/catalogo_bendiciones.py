"""Catálogo de revisión de las bendiciones: una página con todas, para repasarlas de una vez buscando
incoherencias (QA, 17-10).

Uso: python tools/catalogo_bendiciones.py [harness.log ...]

Escribe revision_bendiciones/catalogo.html (un solo archivo, iconos dentro; se abre con doble clic o en
el móvil). Por bendición: icono, nombre y texto en los dos idiomas, lo que hace según los datos, el
proyectil que lanza y, si se le pasan registros de partidas, el daño medido (como
tools/estadisticas_logs.py).

Avisos automáticos: falta la traducción o no cuadran los niveles, guion largo en un texto, icono que
no está, compartido, diminuto o muy oscuro, números distintos entre el texto en inglés y el español, números del texto que
no aparecen en los datos (pueden estar fijos en la DLL), proyectil sin copia (pegaría de verdad),
elemento del texto distinto del del proyectil y combates en que se lleva más de la mitad del daño.
"""
import ast
import base64
import collections
import html
import io
import json
import os
import re
import sys

from PIL import Image, ImageStat

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONOS = os.path.join(RAIZ, "extract", "ui", "iconos")
SALIDA = os.path.join(RAIZ, "revision_bendiciones", "catalogo.html")
COMBATE = re.compile(r"combate: (.+?) en (\d+) s; daño (\[.*\])")
RAREZAS = {"Comun": "Común", "PocoComun": "Poco común", "Rara": "Rara", "Mitica": "Mítica"}
ORDEN_RAREZA = list(RAREZAS)
PERSONAJES = {"monje": "Monje", "samurai": "Samurái", "tanque": "Tanque", "invocador": "Invocador"}

# Elemento de cada proyectil (por el hechizo del que es copia) y palabras que lo nombran en el texto.
ELEMENTO_BALA = {
    79600020: "fuego", 79600022: "magia", 79600024: "magia", 79600026: "hielo", 79600028: "bestia",
    79600030: "gravedad", 79600032: "hielo", 79600034: "rayo", 79600036: "rayo", 79600038: "fuego",
    79600040: "magia", 79600042: "magia", 79600044: "gravedad", 79600046: "sagrado", 79600050: "magia",
}
PALABRAS = {
    "fuego": r"fire|flame|burn|blaze",
    "hielo": r"frost|ice|frozen|cold",
    "rayo": r"lightning|thunder|bolt|storm",
    "sagrado": r"holy|light of|disc of light|golden",
    "sangre": r"blood|bleed",
    "magia": r"glintstone|glintblade|pebble|sorcer|magic|spectral|stars?\b",
    "gravedad": r"rock|stone|gravit|meteor",
    "bestia": r"claw|beast|gurranq",
}


def leer(p):
    return json.load(open(os.path.join(RAIZ, p), encoding="utf8"))


def icono_b64(n, cache={}):
    if n in cache:
        return cache[n]
    f = os.path.join(ICONOS, f"{n}.png")
    if not os.path.exists(f):
        cache[n] = None
        return None
    im = Image.open(f).convert("RGBA")
    im.thumbnail((64, 64), Image.LANCZOS)
    b = io.BytesIO()
    im.save(b, "PNG", optimize=True)
    cache[n] = base64.b64encode(b.getvalue()).decode()
    return cache[n]


def legible(n):
    """Lado del dibujo frente a la casilla y brillo medio: en el juego el PNG ocupa la casilla entera,
    así que un dibujo pequeño u oscuro casi no se ve (17-10: Fuegos fatuos y ¡BONK!)."""
    f = os.path.join(ICONOS, f"{n}.png")
    if not os.path.exists(f):
        return None
    im = Image.open(f).convert("RGBA")
    a = im.split()[3].point(lambda v: 255 if v > 40 else 0)
    bb = a.getbbox()
    if not bb:
        return 0.0, 0.0
    lado = max(bb[2] - bb[0], bb[3] - bb[1]) / max(im.size)
    fondo = Image.new("RGB", im.size)
    fondo.paste(im, mask=im.split()[3])
    return lado, ImageStat.Stat(fondo.convert("L"), mask=a).mean[0]


def numeros(texto):
    return [float(x) for x in re.findall(r"(?<![\w.])(\d+(?:[.,]\d+)?)", texto.replace(",", "."))]


def hojas(o):
    if isinstance(o, dict):
        for v in o.values():
            yield from hojas(v)
    elif isinstance(o, list):
        for v in o:
            yield from hojas(v)
    elif isinstance(o, (int, float)) and not isinstance(o, bool):
        yield float(o)


def cuadra(t, datos):
    # También el producto de dos datos: las pilas (máximo × lo de cada una, «hasta un 30 %»).
    datos = datos + [a * b for i, a in enumerate(datos) for b in datos[i + 1:]]
    cands = (t, t / 100, 1 + t / 100, 1 - t / 100, t * 100)
    return any(abs(v - c) < 1e-6 * max(1, abs(c)) for v in datos for c in cands)


# Números del texto que están fijos en la DLL y no en los datos (comprobados el 17-10 en
# director/combate.rs, director/mod.rs y motor.rs). Si se cambian allí, cambiarlos aquí y en el texto.
FIJOS = {
    "reloj": [20], "venganza": [30], "pacto_vida": [50], "forja": [5], "espatula": [800],
    "raton_perez": [2.5], "ultima_voluntad": [1],
}


def balas(o):
    # "balas" es cuántas salen, no un proyectil.
    return [int(v) for v in re.findall(r'"bala": (\d+)', json.dumps(o))]


def resumir_campos(c):
    grupos = collections.defaultdict(list)
    sueltos = []
    for k, v in c.items():
        if k.endswith("AttackPowerRate"):
            grupos[("daño hecho", v)].append(k)
        elif k.endswith("DamageCutRate"):
            grupos[("daño recibido", v)].append(k)
        else:
            sueltos.append(f"{k} {v}")
    partes = [f"{n} ×{v} ({len(ks)} tipos)" for (n, v), ks in grupos.items()]
    return partes + sueltos


def valor(v, proy):
    if isinstance(v, int) and v in proy:
        return f"{v} «{proy[v]['nombre']}»"
    if isinstance(v, (dict, list)):
        return json.dumps(v, ensure_ascii=False)
    return str(v)


def mecanica(nivel, proy):
    lineas = []
    if nivel.get("campos"):
        lineas.append(("efecto del juego", "; ".join(resumir_campos(nivel["campos"]))))
    for clave, nombre in (("dll", "DLL"), ("pasivo", "pasivo"), ("al_coger", "al cogerla")):
        if nivel.get(clave):
            d = nivel[clave]
            texto = ", ".join(f"{k} {valor(v, proy)}" for k, v in d.items()) if isinstance(d, dict) else valor(d, proy)
            lineas.append((nombre, texto))
    for e in nivel.get("efectos", []):
        cond = [f"{e['cuando']}" + (f" {e['valor']}" if "valor" in e else "")]
        cond += [f"{k} {valor(v, proy)}" for k, v in e.items() if k not in ("cuando", "valor", "hacer")]
        hacer = ["; ".join(f"{k} {valor(v, proy)}" for k, v in h.items()) for h in e.get("hacer", [])]
        lineas.append(("cuando " + ", ".join(cond), " · ".join(hacer)))
    return lineas


def danios(logs):
    total = collections.Counter()
    peleas = collections.Counter()
    maxparte = {}
    dominadas = collections.defaultdict(list)
    n = 0
    for f in logs:
        for l in open(f, encoding="utf8", errors="ignore"):
            m = COMBATE.search(l)
            if not m:
                continue
            try:
                d = ast.literal_eval(m.group(3))
            except (ValueError, SyntaxError):
                continue
            suma = sum(v for _, v in d) or 1
            n += 1
            for k, v in d:
                total[k] += v
                peleas[k] += 1
                maxparte[k] = max(maxparte.get(k, 0), v / suma)
                if k not in ("arma", "estados") and v / suma > 0.5 and suma > 1000:
                    dominadas[k].append(f"{m.group(1)} ({v / suma * 100:.0f} %)")
    return n, total, peleas, maxparte, dominadas


def main(logs):
    rel = leer("data/reliquias.json")["reliquias"]
    es = leer("data/i18n/es.json")["bendiciones"]
    proy = {p["id"]: p for p in leer("data/proyectiles.json")["proyectiles"]}
    n_comb, total, peleas, maxparte, dominadas = danios(logs)
    todo = sum(total.values()) or 1
    por_icono = collections.defaultdict(list)
    for r in rel:
        por_icono[r["icono"]].append(r["id"])
    uso_bala = collections.Counter(b for r in rel for b in set(balas(r)))

    fichas = []
    for r in rel:
        t = es.get(r["id"], {})
        avisos = []
        niv_es = t.get("niveles", [])
        if not t:
            avisos.append("sin traducción al español")
        elif len(niv_es) != len(r["niveles"]):
            avisos.append(f"niveles: {len(r['niveles'])} en inglés, {len(niv_es)} en español")
        textos = [r["nombre"], t.get("nombre", ""), r.get("contra") or "", t.get("contra") or ""]
        textos += [n["texto"] for n in r["niveles"]] + list(niv_es)
        if any("—" in x for x in textos):
            avisos.append("guion largo en un texto")
        ico = icono_b64(r["icono"])
        if ico is None:
            avisos.append(f"no está el icono {r['icono']}")
        vista = legible(r["icono"])
        if vista and vista[0] < 0.5:
            avisos.append(f"icono diminuto: el dibujo ocupa el {vista[0] * 100:.0f} % de la casilla")
        if vista and vista[1] < 30:
            avisos.append("icono muy oscuro")
        otros = [x for x in por_icono[r["icono"]] if x != r["id"]]
        if otros:
            avisos.append("icono compartido con " + ", ".join(otros))
        for i, n in enumerate(r["niveles"]):
            ne = numeros(n["texto"])
            if i < len(niv_es):
                nes = numeros(niv_es[i])
                if sorted(ne) != sorted(nes):
                    avisos.append(f"nivel {i + 1}: números del inglés {ne} y del español {nes}")
            datos = list(hojas({k: v for k, v in n.items() if k != "texto"}))
            sueltos = [x for x in ne if not cuadra(x, datos) and x not in FIJOS.get(r["id"], [])]
            if sueltos:
                fmt = ", ".join(f"{x:g}" for x in sueltos)
                avisos.append(f"nivel {i + 1}: {fmt} del texto no está en los datos (¿fijo en la DLL?)")
        bs = sorted(set(balas(r)))
        for b in bs:
            if b not in proy:
                avisos.append(f"proyectil {b} sin copia: pega de verdad y cuenta como golpe tuyo")
        texto_en = " ".join([r["nombre"]] + [n["texto"] for n in r["niveles"]]).lower()
        dicho = {e for e, p in PALABRAS.items() if re.search(p, texto_en)}
        for b in bs:
            el = ELEMENTO_BALA.get(b)
            if el and dicho and el not in dicho:
                avisos.append(f"el texto habla de {', '.join(sorted(dicho))} y el proyectil es de {el}")
        if r["id"] in dominadas:
            avisos.append("se lleva más de la mitad del daño en: " + "; ".join(dominadas[r["id"]]))
        fichas.append((r, t, ico, avisos, bs))

    def seccion(r):
        if r.get("maldita"):
            return (3, "Maldiciones")
        if r.get("personaje"):
            return (1, "Solo " + PERSONAJES.get(r["personaje"], r["personaje"]))
        if r.get("contra"):
            return (2, "Míticas con contra")
        return (0, "Para todos")

    fichas.sort(key=lambda f: (seccion(f[0]), ORDEN_RAREZA.index(f[0]["rareza"]) if f[0]["rareza"] in ORDEN_RAREZA else 9, (f[1].get("nombre") or f[0]["nombre"]).lower()))
    n_avisos = sum(1 for f in fichas if f[3])
    e = html.escape
    out = []
    actual = None
    for r, t, ico, avisos, bs in fichas:
        s = seccion(r)
        if s != actual:
            if actual is not None:
                out.append("</section>")
            cuantas = sum(1 for f in fichas if seccion(f[0]) == s)
            out.append(f'<section><h2>{e(s[1])} <span class="n">{cuantas}</span></h2>')
            actual = s
        niv = []
        for i, n in enumerate(r["niveles"]):
            es_txt = t.get("niveles", [])[i] if i < len(t.get("niveles", [])) else ""
            mec = "".join(f"<dt>{e(a)}</dt><dd>{e(b)}</dd>" for a, b in mecanica(n, proy))
            marca = f'<span class="nv">{i + 1}</span>' if len(r["niveles"]) > 1 else ""
            niv.append(
                f'<div class="nivel">{marca}<p class="es">{e(es_txt) or "<em>sin traducir</em>"}</p>'
                f'<p class="en">{e(n["texto"])}</p><dl>{mec}</dl></div>'
            )
        contra = ""
        if r.get("contra"):
            contra = f'<p class="contra">Contra: {e(t.get("contra") or "")} <span class="en">{e(r["contra"])}</span></p>'
        img = f'<img src="data:image/png;base64,{ico}" alt="">' if ico else '<div class="sin">?</div>'
        tiro = ""
        if bs:
            tiro = "<p class='bala'>" + "<br>".join(
                f"{e(proy[b]['nombre']) if b in proy else 'ORIGINAL ' + str(b)}"
                + (f" <span class='en'>copia de {proy[b]['de']}{', sobre el objetivo' if proy[b].get('en_objetivo') else ''}; la usan {uso_bala[b]}</span>" if b in proy else "")
                for b in bs
            ) + "</p>"
        medido = ""
        if r["id"] in total:
            k = r["id"]
            medido = (
                f"<p class='medido'>{total[k] / todo * 100:.1f} % del daño de todo · {peleas[k]} combates · "
                f"media {total[k] / peleas[k]:.0f} · máx. {maxparte[k] * 100:.0f} % de un combate</p>"
            )
        av = "".join(f"<li>{e(a)}</li>" for a in avisos)
        out.append(
            f'<article data-q="{e((r["id"] + " " + r["nombre"] + " " + t.get("nombre", "")).lower())}" '
            f'data-avisos="{len(avisos)}">'
            f'<div class="ico">{img}</div>'
            f'<div class="cab"><h3>{e(t.get("nombre") or r["nombre"])}</h3>'
            f'<p class="meta">{e(r["nombre"])} · <code>{e(r["id"])}</code> · {RAREZAS.get(r["rareza"], r["rareza"])} · icono {r["icono"]}</p>'
            f"{contra}{tiro}{medido}</div>"
            f'<div class="niveles">{"".join(niv)}</div>'
            f'<div class="avisos">{"<ul>" + av + "</ul>" if av else ""}</div>'
            "</article>"
        )
    out.append("</section>")
    medida = f"Daño medido en {n_comb} combates de {len(logs)} registros." if n_comb else "Sin registros de partidas: no hay daño medido."
    pagina = PLANTILLA.replace("{{CUANTAS}}", str(len(fichas))).replace("{{AVISOS}}", str(n_avisos))
    pagina = pagina.replace("{{MEDIDA}}", medida).replace("{{FICHAS}}", "\n".join(out))
    os.makedirs(os.path.dirname(SALIDA), exist_ok=True)
    open(SALIDA, "w", encoding="utf8", newline="\n").write(pagina)
    print(f"{SALIDA}: {len(fichas)} bendiciones, {n_avisos} con avisos, {n_comb} combates medidos")


PLANTILLA = """<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Bendiciones de RogueRing</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Cormorant+Garamond:ital,wght@0,500;0,600;1,500&family=EB+Garamond:ital@0;1&display=swap" rel="stylesheet">
<style>
:root{--fondo:#1a1814;--tinta:#ddd3c0;--apagado:#8e8676;--oro:#c9aa66;--aviso:#d0704f;--regla:#3a352c}
*{box-sizing:border-box}
body{margin:0;background:var(--fondo);color:var(--tinta);font:16px/1.45 "EB Garamond",Georgia,serif}
header{display:grid;grid-template-columns:1.6fr 1fr;gap:24px;padding:28px 32px 18px;border-bottom:1px solid var(--regla)}
h1{font:italic 500 44px/1 "Cormorant Garamond",Georgia,serif;margin:0;color:var(--oro)}
header p{margin:6px 0 0;color:var(--apagado)}
.filtro{align-self:end;display:flex;gap:16px;align-items:center;justify-content:flex-end;flex-wrap:wrap}
.filtro input[type=search]{background:transparent;border:0;border-bottom:1px solid var(--regla);color:var(--tinta);font:inherit;padding:4px 0;width:220px}
.filtro label{color:var(--apagado)}
section{padding:0 32px}
h2{font:600 26px/1.2 "Cormorant Garamond",Georgia,serif;color:var(--oro);margin:34px 0 4px;padding-bottom:6px;border-bottom:1px solid var(--oro)}
h2 .n{color:var(--apagado);font-weight:500}
article{display:grid;grid-template-columns:64px 1fr 1.7fr 1fr;gap:18px;padding:16px 0;border-bottom:1px solid var(--regla)}
.ico img,.sin{width:64px;height:64px;display:block}
.sin{border:1px solid var(--aviso);color:var(--aviso);text-align:center;line-height:64px}
h3{font:600 21px/1.15 "Cormorant Garamond",Georgia,serif;margin:0}
.meta,.en,.bala .en{color:var(--apagado);font-size:14px}
.meta{margin:2px 0 6px}
code{font:13px Consolas,monospace}
.contra{margin:4px 0;color:var(--aviso)}
.bala{margin:6px 0 0;font-style:italic}
.medido{margin:6px 0 0;color:var(--oro);font-size:14px}
.nivel{position:relative;padding-left:22px;margin-bottom:10px}
.nivel:last-child{margin-bottom:0}
.nv{position:absolute;left:0;top:1px;color:var(--oro);font:italic 500 18px "Cormorant Garamond",Georgia,serif}
.nivel p{margin:0}
.nivel .es{font-size:17px}
dl{margin:6px 0 0;display:grid;grid-template-columns:max-content 1fr;column-gap:12px;font-size:13.5px}
dt{color:var(--apagado);font-style:italic}
dd{margin:0;word-break:break-word}
.avisos ul{margin:0;padding-left:16px;color:var(--aviso);font-size:14.5px}
.avisos li{margin-bottom:4px}
article.oculto,section.oculto{display:none}
@media (max-width:820px){
 header{grid-template-columns:1fr;padding:20px 16px 14px}
 .filtro{justify-content:flex-start}
 section{padding:0 16px}
 article{grid-template-columns:48px 1fr}
 .ico img,.sin{width:48px;height:48px;line-height:48px}
 .niveles,.avisos{grid-column:1/-1}
}
</style>
</head>
<body>
<header>
 <div><h1>Bendiciones</h1><p>{{CUANTAS}} bendiciones, {{AVISOS}} con avisos. {{MEDIDA}}</p></div>
 <div class="filtro"><input type="search" id="q" placeholder="Buscar"><label><input type="checkbox" id="solo"> Solo con avisos</label></div>
</header>
{{FICHAS}}
<script>
const q=document.getElementById('q'),solo=document.getElementById('solo');
function filtrar(){const t=q.value.trim().toLowerCase();
 document.querySelectorAll('section').forEach(s=>{let n=0;
  s.querySelectorAll('article').forEach(a=>{const ok=(!t||a.dataset.q.includes(t))&&(!solo.checked||a.dataset.avisos!=='0');a.classList.toggle('oculto',!ok);if(ok)n++});
  s.classList.toggle('oculto',n===0)})}
q.addEventListener('input',filtrar);solo.addEventListener('change',filtrar);
</script>
</body>
</html>
"""

if __name__ == "__main__":
    main(sys.argv[1:])
