"""Vida real de cada combate, por acto y tipo, en solitario (data/acts) y en cooperativo (data/coop).

Vida de un combate = suma, enemigo a enemigo (todas las oleadas y fases), de
    vida de su NpcParam × maxHpRate de su tramo × vida del tipo en el acto (data/escalado.json) × `vida`
    del encuentro.
El tramo de un normal depende de la fila del mapa: se toma el de la fila del medio (el élite, un tramo
por encima). Los datos del juego (vida de cada NpcParam y maxHpRate de cada tramo) salen de la
regulation del mod con la herramienta paramdump (ver --params).

Uso: python tools/tabla_vida.py --params <params.json> [--csv docs/tabla_vida.csv]
"""
import argparse
import csv
import json
import os
import statistics as st

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
TIPOS = ["Weak", "Normal", "Elite", "Boss"]
CLAVE = {"Weak": "debil", "Normal": "normal", "Elite": "elite", "Boss": "jefe"}


def enemigos(e):
    """(npc, chr) de cada enemigo del encuentro, con todas sus oleadas."""
    if e.get("enemigos"):
        return [(x["npc"], x.get("chr", 0)) for x in e["enemigos"]]
    if e.get("npc"):
        return [(e["npc"], e.get("chr", 0))]
    return []


def tramo(esc, acto, tipo):
    t = esc["tramos"][str(acto)]
    medio = (t["normal"][0] + t["normal"][1]) // 2 // 10 * 10
    return {"Weak": t["debil"], "Normal": medio, "Elite": min(medio + t["elite"], t["jefe"]), "Boss": t["jefe"]}[tipo]


def tabla(params, esc, carpeta):
    filas = []
    for acto in (1, 2, 3):
        d = json.load(open(os.path.join(ROOT, "data", carpeta, f"{acto}.json"), encoding="utf8"))
        for e in d["encounters"]:
            tipo = e["kind"]
            tr = tramo(esc, acto, tipo)
            mult = params["tramo"].get(str(tr), [1, 1])[0] * esc["vida"][str(acto)][CLAVE[tipo]] * e.get("vida", 1.0)
            ens = enemigos(e)
            base = 0
            sin = []
            for npc, _ in ens:
                v = params["npc"].get(str(npc))
                if v is None:
                    sin.append(npc)
                    continue
                base += v[0]
            filas.append({
                "lista": carpeta, "acto": acto, "tipo": tipo, "id": e["id"], "nombre": e["name"],
                "enemigos": len(ens), "base": base, "tramo": tr, "mult": round(mult, 2),
                "vida": round(base * mult), "ajuste": e.get("vida", 1.0), "sin_npc": " ".join(map(str, sin)),
            })
    return filas


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--params", required=True)
    ap.add_argument("--csv", default=os.path.join(ROOT, "docs", "tabla_vida.csv"))
    a = ap.parse_args()
    params = json.load(open(a.params, encoding="utf8"))
    esc = json.load(open(os.path.join(ROOT, "data", "escalado.json"), encoding="utf8"))
    filas = tabla(params, esc, "acts") + tabla(params, esc, "coop")
    with open(a.csv, "w", newline="", encoding="utf8") as f:
        w = csv.DictWriter(f, fieldnames=list(filas[0].keys()))
        w.writeheader()
        w.writerows(sorted(filas, key=lambda r: (r["lista"], r["acto"], TIPOS.index(r["tipo"]), r["vida"])))
    for lista in ("acts", "coop"):
        print(f"== {lista}")
        for acto in (1, 2, 3):
            for tipo in TIPOS:
                v = [r["vida"] for r in filas if r["lista"] == lista and r["acto"] == acto and r["tipo"] == tipo and r["vida"] > 0]
                if v:
                    print(f"  acto {acto} {tipo:6} n={len(v):3}  min {min(v):6}  mediana {round(st.median(v)):6}  max {max(v):6}")


if __name__ == "__main__":
    main()
