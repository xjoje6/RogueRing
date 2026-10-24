"""Tabla del balance del acto 1: las medidas de runtime/balance/medidas.json (tools/medir_balance.py)
llevadas a la configuración actual. Cada encuentro toma su tipo de data/acts/1.json y su tramo y su
vida de data/escalado.json (el normal y el élite, a media altura del mapa); el cambio de tramo se
aplica con los multiplicadores reales del juego (runtime/balance/tramos.json, orden `tramos`).

Por encuentro y personaje: vida, golpe recibido (mediana sin los tics de estado y máximo, en % de la
vida del personaje), presión (vida que quita en 10 s si no esquivas), golpe del arma de salida, golpes
para matarlo y duración estimada (un golpe cada SEG_GOLPE segundos). Aplica los ajustes `danio` y
`recibido` de data/personajes.json.

Uso: python tools/tabla_balance.py   -> docs/balance_acto1.csv y un resumen por tipo
"""
import csv
import json
import os
import statistics as st

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BAL = os.path.join(ROOT, "runtime", "balance")
SALIDA = os.path.join(ROOT, "docs", "balance_acto1.csv")
SEGUNDOS = 25
# Segundos entre golpe y golpe de un jugador de verdad, según el arma (runs registradas: artes
# marciales 1,5; katana, la mediana de todas, 2,5; martillo pesado, unos 4).
SEG_GOLPE = {"monje": 1.5, "samurai": 2.5, "tanque": 4.0}
TIPOS = ["Weak", "Normal", "Elite", "Boss"]
CLAVE = {"Weak": "debil", "Normal": "normal", "Elite": "elite", "Boss": "jefe"}


def tramo_actual(kind, t):
    normal = (t["normal"][0] + t["normal"][1]) // 2 // 10 * 10
    return {"Weak": t["debil"], "Normal": normal, "Elite": min(normal + t["elite"], t["jefe"]), "Boss": t["jefe"]}[kind]


def filas():
    medidas = json.load(open(os.path.join(BAL, "medidas.json"), encoding="utf8"))
    mult = {r["id"]: r for r in json.load(open(os.path.join(BAL, "tramos.json"), encoding="utf8"))}
    esc = json.load(open(os.path.join(ROOT, "data", "escalado.json"), encoding="utf8"))
    tipos = {e["id"]: e["kind"] for e in json.load(open(os.path.join(ROOT, "data", "acts", "1.json"), encoding="utf8"))["encounters"]}
    pjs = {c["id"]: c for c in json.load(open(os.path.join(ROOT, "data", "personajes.json"), encoding="utf8"))["personajes"]}
    for m in medidas.values():
        kind = tipos.get(m["id"])
        if not kind or not m.get("vida_enemigo"):
            continue
        antes = mult[m["tramo"]]
        tramo = tramo_actual(kind, esc["tramos"]["1"])
        ahora = mult[tramo]
        pj = pjs.get(m["personaje"], {})
        ataque = ahora["ataque"] / antes["ataque"] * pj.get("recibido", 1.0)
        rec = [x * ataque for x in m.get("recibidos", [])]
        golpes_fuertes = [x for x in rec if x >= 0.03]
        dad = m.get("dados", [])
        golpe = st.median(dad) * antes["defensa"] / ahora["defensa"] * pj.get("danio", 1.0) if dad else None
        vida = m["vida_enemigo"] / antes["vida"] * ahora["vida"] * esc["vida"]["1"][CLAVE[kind]]
        yield {
            "personaje": m["personaje"],
            "id": m["id"],
            "enemigo": m["nombre"],
            "tipo": kind,
            "tramo": tramo,
            "vida": round(vida),
            "golpe_rec_mediana": round(100 * st.median(golpes_fuertes), 1) if golpes_fuertes else "",
            "golpe_rec_max": round(100 * max(rec), 1) if rec else "",
            "presion_10s": round(100 * sum(rec) / SEGUNDOS * 10, 1),
            "golpe_dado": round(golpe) if golpe else "",
            "golpes_matar": round(vida / golpe) if golpe else "",
            "duracion_s": round(vida / golpe * SEG_GOLPE[m["personaje"]]) if golpe else "",
        }


def main():
    tabla = list(filas())
    with open(SALIDA, "w", encoding="utf8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(tabla[0]))
        w.writeheader()
        w.writerows(sorted(tabla, key=lambda r: (r["personaje"], TIPOS.index(r["tipo"]), r["enemigo"])))
    print(f"{SALIDA}: {len(tabla)} filas\n")
    print(f"{'personaje':9} {'tipo':7} {'n':>3} {'golpe rec.':>10} {'máx':>6} {'presión':>8} {'golpes':>7} {'duración':>9}")
    for p in sorted({r["personaje"] for r in tabla}):
        for t in TIPOS:
            rs = [r for r in tabla if r["personaje"] == p and r["tipo"] == t]
            if not rs:
                continue

            def med(k):
                return st.median([r[k] for r in rs if r[k] != ""] or [0])

            print(f"{p:9} {t:7} {len(rs):>3} {med('golpe_rec_mediana'):>9.1f}% {med('golpe_rec_max'):>5.1f}% {med('presion_10s'):>7.1f}% "
                  f"{med('golpes_matar'):>7.0f} {med('duracion_s'):>8.0f}s")


if __name__ == "__main__":
    main()
