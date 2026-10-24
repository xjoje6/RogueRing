"""Traducción del mod (data/i18n/<idioma>.json): rellena lo que sale del juego y avisa de lo que falta.

- «ui»: todos los textos de pantalla del código (lo que pasa por t(), tf() y las ayudas de botones),
  más tipos de arma, nombres de acto, estados y Ascensiones. Los que falten se añaden vacíos para
  traducirlos a mano.
- «objetos» y «enemigos»: los nombres los pone el propio juego (extract/fmg/<idioma del juego>,
  sacado con el Extractor), por id de arma o ceniza y por el NpcParam de cada encuentro.
- «bendiciones», «eventos», «personajes», «mutadores»: a mano; aquí solo se listan los que faltan.

Uso: python tools/i18n.py [es]
"""
import csv
import glob
import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
IDIOMA = sys.argv[1] if len(sys.argv) > 1 else "es"
FMG_JUEGO = {"es": "spaes"}[IDIOMA]
SALIDA = os.path.join(ROOT, "data", "i18n", f"{IDIOMA}.json")


def leer(p):
    return json.load(open(os.path.join(ROOT, p), encoding="utf8"))


def fmg(idioma, nombre):
    """Textos del juego de un archivo FMG (con los del DLC): id → texto."""
    out = {}
    for f in sorted(glob.glob(os.path.join(ROOT, "extract", "fmg", idioma, f"{nombre}*.json"))):
        if re.search(rf"{nombre}(_dlc\d+)?\.json$", f):
            for k, v in json.load(open(f, encoding="utf8")).items():
                # Los del DLC traen huecos de relleno («DLC dummy») con ids del juego base.
                if v and "dummy" not in v.lower() and int(k) not in out:
                    out[int(k)] = v
    return out


def textos_codigo():
    """Textos de pantalla del código, en inglés."""
    claves = set()
    lit = r'"((?:[^"\\]|\\.)*)"'
    for f in glob.glob(os.path.join(ROOT, "dll", "*", "src", "**", "*.rs"), recursive=True):
        s = open(f, encoding="utf8").read()
        # El texto puede ir en la línea siguiente si rustfmt parte la llamada.
        for m in re.finditer(r"\b(?:t|tf)\(\s*" + lit, s):
            claves.add(m.group(1))
        # Ayudas de botones: ("A", "Choose"), ("<>", "Character")...
        for m in re.finditer(r'\("[^"]{1,6}",\s*' + lit + r"\)", s):
            claves.add(m.group(1))
    # Estados del rival y Ascensiones (se traducen al enseñarlos).
    motor = open(os.path.join(ROOT, "dll", "harness", "src", "motor.rs"), encoding="utf8").read()
    bloque = motor[motor.index("const ESTADOS"):]
    bloque = bloque[:bloque.index("];")]
    claves |= set(re.findall(r'\("([^"]+)", \[', bloque))
    perfil = open(os.path.join(ROOT, "dll", "core", "src", "perfil.rs"), encoding="utf8").read()
    bloque = perfil[perfil.index("ASCENSIONES"):]
    bloque = bloque[:bloque.index("];")]
    claves |= set(re.findall(lit, bloque))
    return {c.replace('\\"', '"') for c in claves}


def main():
    t = json.load(open(SALIDA, encoding="utf8")) if os.path.exists(SALIDA) else {}
    for k in ("ui", "bendiciones", "eventos", "personajes", "mutadores", "objetos", "enemigos"):
        t.setdefault(k, {})

    # Textos de pantalla, tipos de arma y nombres de acto.
    ui = textos_codigo()
    pool = leer("data/objetos/pool.json")
    ui |= {i["tipo"] for i in pool}
    # Todos los actos y las listas del cooperativo (data/coop).
    actos = [json.load(open(f, encoding="utf8")) for f in sorted(glob.glob(os.path.join(ROOT, "data", "acts", "*.json")))]
    actos += [dict(json.load(open(f, encoding="utf8")), name=None) for f in sorted(glob.glob(os.path.join(ROOT, "data", "coop", "*.json")))]
    ui |= {a["name"] for a in actos if a.get("name")}
    # Tipos de arma: el juego los tiene en sus textos de menú (mismo id en los dos idiomas).
    en_menu, es_menu = fmg("engus", "GR_MenuText"), fmg(FMG_JUEGO, "GR_MenuText")
    por_texto = {}
    for i, v in en_menu.items():
        por_texto.setdefault(v, i)
    for k in sorted(ui):
        if k not in t["ui"] or not t["ui"][k]:
            i = por_texto.get(k)
            t["ui"][k] = es_menu.get(i, "") if i is not None else t["ui"].get(k, "")
    for k in list(t["ui"]):
        if k not in ui:
            print("sobra en ui:", k)

    # Objetos: armas por WeaponName, cenizas por GemName.
    armas, cenizas = fmg(FMG_JUEGO, "WeaponName"), fmg(FMG_JUEGO, "GemName")
    for it in pool:
        # Las cenizas de espíritu se traducen a mano: su id del mod no es el del juego y en GemName
        # coincide con otras cosas (19-10: Tiche, 200000, salía como «C. G.: Torb. hojaseca»).
        if it["categoria"] == "Invocacion":
            if str(it["id"]) not in t["objetos"]:
                print("sin traducir (invocación):", it["id"], it["nombre"])
            continue
        nombre = (armas if it["categoria"] == "Arma" else cenizas).get(it["id"])
        # Las cenizas del juego en español llevan delante «Ceniza de Guerra: »; las del mod, no.
        if nombre and it["categoria"] == "Ceniza":
            nombre = re.sub(r"^Ceniza de [Gg]uerra:\s*", "", nombre)
        if nombre:
            t["objetos"][str(it["id"])] = nombre
        elif str(it["id"]) not in t["objetos"]:
            print("sin nombre del juego:", it["id"], it["nombre"])

    # Enemigos: por su nombre en inglés en los nombres de NPC del juego (mismo id en los dos idiomas);
    # el NpcParam casi nunca trae nameId. Las variantes entre paréntesis («(Spear)») van aparte.
    en_npc, es_npc = fmg("engus", "NpcName"), fmg(FMG_JUEGO, "NpcName")
    por_nombre = {}
    for i, v in en_npc.items():
        if i in es_npc:
            por_nombre.setdefault(v, es_npc[i])
    variantes = {"Spear": "lanza", "Sword": "espada", "Cleaver": "cuchilla", "Ringblade": "hoja circular", "Swinging Sickle": "hoz"}
    for a in actos:
        for e in a["encounters"]:
            n = e["name"]
            m = re.fullmatch(r"(.+?) \((.+)\)", n)
            if n in por_nombre:
                t["enemigos"][n] = por_nombre[n]
            elif m and m.group(1) in por_nombre and m.group(2) in variantes:
                t["enemigos"][n] = f"{por_nombre[m.group(1)]} ({variantes[m.group(2)]})"
            elif n not in t["enemigos"]:
                print("enemigo sin nombre del juego:", n)

    # Lo que va a mano: avisar de lo que falta.
    faltan = [k for k, v in t["ui"].items() if not v]
    rel = leer("data/reliquias.json")["reliquias"]
    ev = leer("data/eventos.json")["eventos"]
    pj = leer("data/personajes.json")["personajes"]
    mut = leer("data/mutadores.json")["mutadores"]
    print(f"ui: {len(t['ui'])} ({len(faltan)} sin traducir)")
    print("bendiciones sin traducir:", [x["id"] for x in rel if x["id"] not in t["bendiciones"]][:200])
    print("eventos sin traducir:", [x["id"] for x in ev if x["id"] not in t["eventos"]])
    print("personajes sin traducir:", [x["id"] for x in pj if x["id"] not in t["personajes"]])
    print("mutadores sin traducir:", [x["id"] for x in mut if x["id"] not in t["mutadores"]])
    os.makedirs(os.path.dirname(SALIDA), exist_ok=True)
    orden = ["_nota", "ui", "bendiciones", "eventos", "personajes", "mutadores", "objetos", "enemigos"]
    t.setdefault("_nota", "Traducción al español (rogue_core::idioma). ui: textos de pantalla por su texto en inglés; el resto, por id. objetos y enemigos los rellena tools/i18n.py con los textos del juego.")
    salida = {k: t[k] for k in orden}
    salida["ui"] = dict(sorted(salida["ui"].items(), key=lambda kv: kv[0].lower()))
    open(SALIDA, "w", encoding="utf8", newline="").write(json.dumps(salida, ensure_ascii=False, indent=1) + "\n")


if __name__ == "__main__":
    main()
