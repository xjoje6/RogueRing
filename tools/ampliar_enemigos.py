"""Amplía las listas con los modelos del juego que no se usan (24-10: «usando todo lo que esté fuera
que puedas y dejando solo fuera lo justificado»).

Uso: python tools/ampliar_enemigos.py [--aplicar]

Por cada modelo de los alias de Smithbox (criaturas y jefes) que no sale en ningún encuentro:
- Fuera, con su motivo, los que se dejaron fuera por decisión (FUERA) y los que no tienen fila de NpcParam
  o de IA con la que crearlos.
- El resto, un encuentro `coliseo_<modelo>` como los del acto 1 (su fila base de NpcParam e IA) en el acto
  y tipo cuya mediana de vida base se le parece más (los del DLC, solo en los actos 2 y 3; los jefes, solo
  élite o jefe). Su `vida` la ajusta después tools/equilibrar_vida.py, y tools/validar_msb.py comprueba en
  el banco que sale, se mueve y se gana; los que no, a data/enemigos_fuera.json con lo que pasó.

Escribe data/enemigos_fuera.json (modelo -> motivo; lo usa tools/enemigos.py para el editor) y, con
--aplicar, los encuentros nuevos en data/acts (con la etiqueta «nuevo_24_10») y su lista en
runtime/ampliacion.json.
"""
import csv
import json
import math
import os
import re
import statistics as st
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
ALIAS = os.path.join(ROOT, "tools", "vendor", "Smithbox", "Smithbox.Release", "Output", "Assets", "Aliases", "ER", "Characters.json")
ETIQUETA = "nuevo_24_10"
# Fuera por decisión (docs/LISTAS_Y_COMBATES.md), por parte del nombre en inglés.
FUERA = [
    ("Dragon", "Dragón: los voladores quedaron fuera por decisión (vuelan por encima del coliseo)."),
    ("Rykard", "Fuera por decisión: su combate depende de su propio mapa."),
    ("Mimic", "Fuera por decisión: el Mímico copia al jugador."),
    ("Giant", "Gigante: no cabe en el coliseo."),
    ("Radahn", "Radahn de Caelid: gigante, no cabe en el coliseo."),
    ("Abductor Virgin", "En la validación de octubre no se movía nunca."),
    ("Astel", "En la validación de octubre se quedaba quieto."),
    ("Fortissax", "En la validación de octubre se quedaba quieto."),
    ("Corpse", "Es un cadáver o un elemento del escenario, no un enemigo."),
    ("Watcher Stones", "Es un elemento del escenario, no un enemigo."),
    ("Ball", "Es un proyectil o un objeto, no un enemigo."),
    ("Tibia", "Fuera por decisión: la Tibia (docs/LISTAS_Y_COMBATES.md)."),
    ("Chariot", "Va sobre raíles por su mapa: en el coliseo no tiene por dónde ir."),
]
AMISTOSO = "PNJ amistoso (equipo 26 del juego): no pelea."
# Los de muy poca vida (cangrejos, babosas...) van en manada de MANADA: uno solo no es un combate.
VIDA_MANADA = 400
MANADA = 3
SIN_DATOS = "No tiene fila de datos (NpcParam o IA) con la que crearlo en el coliseo."


def tabla(nombre):
    return {int(r["ID"]): r for r in csv.DictReader(open(os.path.join(ROOT, "extract", "params", nombre + ".csv"), encoding="utf8"))}


def main():
    aplicar = "--aplicar" in sys.argv
    alias = json.load(open(ALIAS, encoding="utf8"))
    npc, think = tabla("NpcParam"), tabla("NpcThinkParam")
    actos = {a: json.load(open(os.path.join(ROOT, "data", "acts", f"{a}.json"), encoding="utf8")) for a in (1, 2, 3)}
    usados, ids = set(), set()
    for carpeta in ("acts", "coop"):
        for a in (1, 2, 3):
            d = actos[a] if carpeta == "acts" else json.load(open(os.path.join(ROOT, "data", carpeta, f"{a}.json"), encoding="utf8"))
            for e in d["encounters"]:
                ids.add(e["id"])
                for x in e.get("enemigos") or [{"chr": e.get("chr")}]:
                    if x.get("chr"):
                        usados.add(int(x["chr"]))
    # Vida base de los combates de un enemigo que ya hay, por acto y tipo (medianas).
    vidas = {}
    for a, d in actos.items():
        for e in d["encounters"]:
            if e.get("enemigos") or not e.get("npc") or e["npc"] not in npc:
                continue
            hp = float(npc[e["npc"]]["hp"])
            if hp > 0:
                vidas.setdefault((a, e["kind"]), []).append(math.log(hp))
    medianas = {k: st.median(v) for k, v in vidas.items() if len(v) >= 3}

    previo = os.path.join(ROOT, "data", "enemigos_fuera.json")
    viejo = json.load(open(previo, encoding="utf8")).get("fuera", {}) if os.path.exists(previo) else {}
    fuera, nuevos = {}, []
    for al in alias:
        tags = set(al.get("Tags", []))
        if not ({"creature", "boss"} & tags) or {"horse", "trap", "debug", "interactable_placeholder", "player"} & tags:
            continue
        c = int(al["ID"][1:])
        if c in usados or c == 0:
            continue
        # Lo que ya se dejó fuera (falló en el banco o se quitó en el editor) no vuelve.
        if al["ID"] in viejo:
            continue
        nombre = al["Name"]
        motivo = next((m for k, m in FUERA if k.lower() in nombre.lower()), None)
        if motivo:
            fuera[al["ID"]] = motivo
            continue
        filas = sorted(i for i in npc if c * 10000 <= i < (c + 1) * 10000 and float(npc[i]["hp"] or 0) > 0)
        n = c * 10000 if c * 10000 in filas else (filas[0] if filas else None)
        t = n if n in think else (c * 10000 if c * 10000 in think else next((i for i in sorted(think) if c * 10000 <= i < (c + 1) * 10000), None))
        if n is None or t is None:
            fuera[al["ID"]] = SIN_DATOS
            continue
        if npc[n]["teamType"] == "26":
            fuera[al["ID"]] = AMISTOSO
            continue
        hp = math.log(float(npc[n]["hp"]))
        dlc = "sote" in tags
        jefe = "boss" in tags
        opciones = [(k, m) for k, m in medianas.items()
                    if (not dlc or k[0] >= 2) and (k[1] in ("Elite", "Boss") if jefe else k[1] != "Boss")]
        (acto, tipo), _ = min(opciones, key=lambda km: abs(km[1] - hp))
        eid = f"coliseo_{c}"
        if eid in ids:
            continue
        slug = re.sub(r"[^a-z0-9]+", " ", nombre.lower()).strip()
        e = {"id": eid, "name": nombre, "kind": tipo, "warp_flag": 0, "defeat_flag": 0, "llegada": None, "tags": [slug, ETIQUETA]}
        if float(npc[n]["hp"]) < VIDA_MANADA and not jefe:
            e.update({"name": f"{nombre} ×{MANADA}", "formato": "manada", "enemigos": [{"chr": c, "npc": n, "think": t}] * MANADA})
        else:
            e.update({"chr": c, "npc": n, "think": t})
        nuevos.append((acto, e))
    print(f"{len(nuevos)} encuentros nuevos; fuera {len(fuera)}")
    for acto, e in sorted(nuevos, key=lambda x: (x[0], x[1]["kind"])):
        print(f"  acto {acto} {e['kind']:6} {e['name']}")
    for k, m in sorted(fuera.items()):
        print(f"  fuera {k}: {m}")
    # Lo que ya apuntó la validación del banco se queda.
    for k, m in viejo.items():
        fuera.setdefault(k, m)
    json.dump({"_nota": "Modelos del juego que no están en las listas y por qué (tools/ampliar_enemigos.py y tools/validar_msb.py). Lo enseña el editor de combates.", "fuera": fuera},
              open(previo, "w", encoding="utf8"), ensure_ascii=False, indent=1)
    if aplicar:
        for acto, e in nuevos:
            actos[acto]["encounters"].append(e)
        for a, d in actos.items():
            open(os.path.join(ROOT, "data", "acts", f"{a}.json"), "w", encoding="utf8").write(json.dumps(d, ensure_ascii=False, indent=1) + "\n")
        json.dump([e["id"] for _, e in nuevos], open(os.path.join(ROOT, "runtime", "ampliacion.json"), "w", encoding="utf8"))
        print("aplicado")


if __name__ == "__main__":
    main()
