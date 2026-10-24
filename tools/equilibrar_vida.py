"""Lleva la vida de cada combate a una curva que siempre sube (23-10: «hay combates en acto 3 que son
un paseo y otros que tienen más vida»). Objetivo por acto y tipo (OBJETIVO; en cooperativo, ×COOP) y,
alrededor, un margen de ±MARGEN: el combate que se sale se lleva al borde más cercano cambiando su
`vida` en data/acts y data/coop. Los de «aguante» no se tocan (se gana aguantando); en los de «escolta»
cuenta el líder. La vida de cada combate, como en tools/tabla_vida.py.

Uso: python tools/equilibrar_vida.py --params <params.json> [--aplicar]
"""
import argparse
import json
import os
import statistics as st

from tabla_vida import CLAVE, TIPOS, tramo

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OBJETIVO = {
    1: {"Weak": 3500, "Normal": 5500, "Elite": 8500, "Boss": 17000},
    2: {"Weak": 6000, "Normal": 10000, "Elite": 16000, "Boss": 30000},
    3: {"Weak": 11000, "Normal": 18000, "Elite": 27000, "Boss": 45000},
}
# Cooperativo: la misma curva; la vida extra por cada jugador además del primero la pone el juego
# (data/escalado.json, coop_extra) desde el 24-10.
COOP = 1.0
MARGEN = 0.2
AJUSTE = (0.3, 5.0)


def vida_base(e, params):
    if e.get("enemigos"):
        ens = e["enemigos"]
        if e.get("formato") == "escolta" and e.get("lider") is not None:
            ens = [ens[e["lider"]]]
        return sum(params["npc"].get(str(x["npc"]), [0])[0] for x in ens)
    return params["npc"].get(str(e.get("npc")), [0])[0] if e.get("npc") else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--params", required=True)
    ap.add_argument("--aplicar", action="store_true")
    a = ap.parse_args()
    params = json.load(open(a.params, encoding="utf8"))
    esc = json.load(open(os.path.join(ROOT, "data", "escalado.json"), encoding="utf8"))
    # En cooperativo también las parejas posibles que sortea el mod (tools/mezclas_coop.py).
    archivos = [("acts", 1.0, a, f"{a}.json") for a in (1, 2, 3)] + [("coop", COOP, a, f"{a}.json") for a in (1, 2, 3)]         + [("coop", COOP, a, f"mezclas_{a}.json") for a in (1, 2, 3)]
    for carpeta, factor, acto, archivo in archivos:
        if True:
            ruta = os.path.join(ROOT, "data", carpeta, archivo)
            if not os.path.exists(ruta):
                continue
            texto = open(ruta, encoding="utf8", newline="").read()
            d = json.loads(texto)
            cambios, antes, despues = 0, {}, {}
            for e in d["encounters"]:
                if e.get("formato") == "aguante":
                    continue
                tipo = e["kind"]
                base = vida_base(e, params)
                if base <= 0:
                    continue
                mult = params["tramo"].get(str(tramo(esc, acto, tipo)), [1, 1])[0] * esc["vida"][str(acto)][CLAVE[tipo]]
                ajuste = e.get("vida", 1.0)
                vida = base * mult * ajuste
                obj = OBJETIVO[acto][tipo] * factor
                lo, hi = obj * (1 - MARGEN), obj * (1 + MARGEN)
                antes.setdefault(tipo, []).append(vida)
                if lo <= vida <= hi:
                    despues.setdefault(tipo, []).append(vida)
                    continue
                nuevo = ajuste * ((lo if vida < lo else hi) / vida)
                nuevo = round(min(max(nuevo, AJUSTE[0]), AJUSTE[1]), 2)
                if abs(nuevo - 1.0) < 0.005:
                    e.pop("vida", None)
                else:
                    e["vida"] = nuevo
                cambios += 1
                despues.setdefault(tipo, []).append(base * mult * nuevo)
                if nuevo in AJUSTE:
                    print(f"  AVISO {carpeta} {acto} {e['id']} ({e['name']}): ajuste al tope {nuevo} (vida {round(vida)} → {round(base * mult * nuevo)}, objetivo {round(obj)})")
            print(f"{carpeta} {archivo}: {cambios} cambios")
            for tipo in TIPOS:
                if tipo in antes:
                    print(f"   {tipo:6} antes {round(min(antes[tipo])):6}-{round(max(antes[tipo])):6} (mediana {round(st.median(antes[tipo])):6})"
                          f"  después {round(min(despues[tipo])):6}-{round(max(despues[tipo])):6} (mediana {round(st.median(despues[tipo])):6})")
            if a.aplicar:
                nuevo_txt = json.dumps(d, ensure_ascii=False, indent=1) + ("\n" if texto.endswith("\n") else "")
                open(ruta, "w", encoding="utf8", newline="").write(nuevo_txt)


if __name__ == "__main__":
    main()
