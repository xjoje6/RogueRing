"""Repasa los ficheros de texto del repositorio antes de publicar y lista lo que hay que mirar:
guiones largos, emojis, comentarios con fecha, con nombres o que cuentan historia, palabras de
relleno, rutas personales, SteamIDs y correos, y ficheros con demasiados comentarios.

Uso: python tools/auditoria.py [--todo]   (sin --todo, solo el resumen y los 40 primeros avisos)
Sale con código 1 si encuentra algo.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
TEXTO = {".rs", ".py", ".cs", ".ps1", ".hks", ".json", ".toml", ".md", ".txt", ".csproj", ".gitignore"}
# Datos del juego o de terceros: no son texto nuestro.
FUERA = ("mod/", "tools/Extractor/EldenRingDictionary.txt", "tools/auditoria.py")
COMENTARIO = {".rs": "//", ".cs": "//", ".py": "#", ".ps1": "#", ".hks": "--", ".toml": "#"}

PALABRAS = [
    # inglés
    "delve", "tapestry", "seamless", "leverage", "robust", "comprehensive", "crucial", "furthermore",
    "moreover", "notably", "elevate", "streamline", "cutting-edge", "game-changer", "utilize", "embark",
    "meticulous", "vibrant", "testament", "unleash", "in summary", "it's worth noting",
    # castellano
    "cabe destacar", "en resumen", "sin duda", "exhaustivo", "crucial", "aprovechar al máximo",
    "potenciar", "de manera integral", "es importante señalar", "en definitiva",
]
NOMBRES = r"\b(el jugador|el jugador|Claude|Codex|ChatGPT|GPT|Opus|Sonnet|Peleón|Ratilla|Hijo de la Sangre|Golpetón|Festín|Enjambre)\b"
# 14-10, 26-09-2026 o 2026-10-14. Un rango como 2-3 o 10-12 no lleva el mes con dos cifras.
FECHA = r"(?<![\d/,.])(?:[1-9]|[12]\d|3[01])-(?:0[1-9]|1[0-2])(?:-20\d\d)?(?![\d/]|[.,]\d)|\b20\d\d-\d\d-\d\d\b"
# Comentarios que cuentan la historia del código en vez del porqué.
HISTORIA = r"\b(antes(?:,| se| era| iba| había| salía| estaba| solo)|ya no (?:se|hay|va|sale)|ahora (?:se|va|es|sale|el|la)|por ahora|se quitó|esta sesión|sin probar|provisional)\b"
PERSONAL = [
    (r"[A-Za-z]:[\\/]+Users[\\/]+\w+", "ruta personal"),
    # La base de los SteamID64 (cuenta 0) no es de nadie.
    (r"\b7656119(?!7960265728)\d{10}\b", "SteamID"),
    (r"[\w.+-]+@(?!users\.noreply\.github\.com)[\w-]+\.[a-z]{2,}\b", "correo"),
]
EMOJI = re.compile("[\U0001F300-\U0001FAFF☀-➿]")
DENSIDAD = 0.3


def ficheros():
    r = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, encoding="utf8", check=True)
    for p in r.stdout.splitlines():
        ext = os.path.splitext(p)[1] or os.path.basename(p)
        if ext in TEXTO and not p.startswith(FUERA) and os.path.exists(os.path.join(ROOT, p)):
            yield p


def revisar(p):
    avisos = []
    try:
        lineas = open(os.path.join(ROOT, p), encoding="utf-8-sig").read().splitlines()
    except UnicodeDecodeError:
        return avisos, 0.0, 0
    ext = os.path.splitext(p)[1]
    marca = COMENTARIO.get(ext)
    comentarios = codigo = 0
    for n, l in enumerate(lineas, 1):
        s = l.strip()
        es_coment = bool(marca) and s.startswith(marca)
        if marca and s:
            comentarios += es_coment
            codigo += not es_coment
        # Parte comentada de la línea (también la que va detrás del código, salvo dentro de una URL).
        coment = ""
        if marca and marca in l and not l.split(marca, 1)[0].endswith(":"):
            coment = l.split(marca, 1)[1]
        prosa = coment or (l if ext in (".md", ".txt") or (ext == ".json" and '"id"' not in l) else "")
        if "—" in l:
            avisos.append((n, "guion largo", s))
        if EMOJI.search(l):
            avisos.append((n, "emoji", s))
        for rx, que in PERSONAL:
            if re.search(rx, l):
                avisos.append((n, que, s))
        for w in PALABRAS:
            if re.search(r"\b" + re.escape(w) + r"\b", prosa.lower()) and not (w == "seamless" and "Seamless" in prosa):
                avisos.append((n, f"palabra «{w}»", s))
        # El crédito a las herramientas de IA es a propósito (la etiqueta de Nexus se mantiene).
        if re.search(NOMBRES, prosa) and "AI tools" not in prosa:
            avisos.append((n, "nombre", s))
        if re.search(FECHA, prosa) and not (p == "CHANGELOG.md" and s.startswith("## ")):
            avisos.append((n, "fecha", s))
        if re.search(HISTORIA, coment, re.I):
            avisos.append((n, "historia", s))
    return avisos, comentarios / max(1, comentarios + codigo), len(lineas)


def main():
    todo = "--todo" in sys.argv
    total = 0
    densos = []
    for p in ficheros():
        avisos, dens, n_lineas = revisar(p)
        if dens > DENSIDAD and n_lineas > 40:
            densos.append((dens, p))
        for n, que, s in avisos:
            total += 1
            if todo or total <= 40:
                print(f"{p}:{n}: {que}: {s[:110]}")
    for d, p in sorted(densos, reverse=True):
        print(f"{p}: {d:.0%} de líneas de comentario")
    print(f"\n{total} avisos, {len(densos)} ficheros con más del {DENSIDAD:.0%} de comentarios")
    sys.exit(1 if total or densos else 0)


if __name__ == "__main__":
    main()
