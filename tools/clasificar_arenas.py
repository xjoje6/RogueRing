"""Coliseo o arena grande, según el tamaño (24-10: «debes ser más estricto con el tamaño... que la
clasificación fuera automática y que hubiera bastante número de combates en ambos sitios»).

Uso: python tools/clasificar_arenas.py [--aplicar]

Tamaño de un enemigo: lo mayor entre su radio y su altura / 2,5 (NpcParam hitRadius y hitHeight: Godfrey o
Margit son estrechos pero altos). Tamaño de un combate: la suma de los que salen a la vez (la primera
oleada), así que dos jefes juntos suman. Desde UMBRAL, arena grande (data/arenas_grandes.json); si no, el
coliseo. Los que tienen cambio de fase, renacer o animación de entrada se quedan donde estaban (puestos a
mano por su mecánica); los dúos de último en pie van por tamaño. Lo usa también tools/mezclas_coop.py con
las parejas posibles.
"""
import csv
import glob
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
UMBRAL = 2.8
ARENA = "grande"
_NPC = None


def npc():
    global _NPC
    if _NPC is None:
        _NPC = {int(r["ID"]): r for r in csv.DictReader(open(os.path.join(ROOT, "extract", "params", "NpcParam.csv"), encoding="utf8"))}
    return _NPC


def tamano_enemigo(fila):
    r = npc().get(int(fila))
    return max(float(r["hitRadius"]), float(r["hitHeight"]) / 2.5) if r else 0.0


def tamano(e):
    ens = e.get("enemigos") or ([{"npc": e["npc"]}] if e.get("npc") else [])
    ola = (e.get("oleadas") or [list(range(len(ens)))])[0]
    return sum(tamano_enemigo(ens[i]["npc"]) for i in ola if i < len(ens))


def a_mano(e):
    # Los dúos de «último en pie» sí van por tamaño (24-10: dos jefes que por separado caben en el
    # coliseo juntos no caben).
    return bool(e.get("fase_en") or e.get("renacer") or e.get("anim_inicio"))


def arena(e):
    """La arena que le toca: «grande» o None (el coliseo)."""
    if a_mano(e):
        return e.get("arena")
    return ARENA if tamano(e) >= UMBRAL else None


def main():
    aplicar = "--aplicar" in sys.argv
    cuenta = {}
    for f in sorted(glob.glob(os.path.join(ROOT, "data", "acts", "*.json")) + glob.glob(os.path.join(ROOT, "data", "coop", "*.json"))):
        d = json.load(open(f, encoding="utf8"))
        lista = "parejas sorteables" if "mezclas_" in f else ("cooperativo" if os.sep + "coop" + os.sep in f else "solitario")
        cambios = 0
        for e in d["encounters"]:
            nueva = arena(e)
            if nueva != e.get("arena"):
                cambios += 1
                if nueva:
                    e["arena"] = nueva
                else:
                    e.pop("arena", None)
            k = cuenta.setdefault((lista, e["kind"]), [0, 0])
            k[0] += bool(nueva)
            k[1] += 1
        if aplicar and cambios:
            open(f, "w", encoding="utf8").write(json.dumps(d, ensure_ascii=False, indent=1) + "\n")
        print(f"{os.path.relpath(f, ROOT)}: {cambios} cambios")
    for (lista, tipo), (g, n) in sorted(cuenta.items()):
        print(f"  {lista:20} {tipo:6} arena grande {g:4} de {n:4}")
    if aplicar:
        print("aplicado")


if __name__ == "__main__":
    main()
