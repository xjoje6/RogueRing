"""Prueba en el banco una muestra de las parejas posibles del cooperativo (data/coop/mezclas_<acto>.json):
que salgan las dos, se muevan, NO se peguen entre ellas y se puedan ganar (24-10: «que funcione
siempre... que no se rompan al estar juntos»).

Uso: python tools/probar_mezclas.py [N] [id ...]

La muestra cubre cada combinación de bandos (teamType de NpcParam) y de tamaño (grande = radio >= 3 m), y
N parejas más al azar (fijo). Con el jugador quieto y sin atacar, si uno de los dos pierde vida es que
el otro le pega: esa combinación de bandos no se puede juntar (BANDOS_AMIGOS de tools/mezclas_coop.py).
Escribe runtime/probar_mezclas.json. Requisitos: juego abierto con lanzar.ps1 y la partida cargada.
"""
import csv
import json
import os
import random
import re
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from banco import ROOT, orden
from validar_msb import COPIA, HUECO, LOG, RT, esperar, fase, leer_desde, pos, run_limpia

SALIDA = os.path.join(RT, "probar_mezclas.json")
QUIETO = 20
ROCE = 50


def chr_(ent):
    try:
        return json.loads(orden("chr", ent))
    except Exception:
        return None


def muestra(n_azar):
    npc = {int(r["ID"]): r for r in csv.DictReader(open(os.path.join(ROOT, "extract", "params", "NpcParam.csv"), encoding="utf8"))}
    todas = []
    for a in (1, 2, 3):
        todas += json.load(open(os.path.join(ROOT, "data", "coop", f"mezclas_{a}.json"), encoding="utf8"))["encounters"]
    clase = {}
    for e in todas:
        x, y = (npc[m["npc"]] for m in e["enemigos"])
        k = (tuple(sorted((x["teamType"], y["teamType"]))), sum(float(z["hitRadius"]) >= 3.0 for z in (x, y)))
        clase.setdefault(k, []).append(e)
    rng = random.Random("mezclas-banco")
    elegidas = []
    for k in sorted(clase):
        elegidas += rng.sample(clase[k], min(3, len(clase[k])))
    resto = [e for e in todas if e not in elegidas]
    elegidas += rng.sample(resto, min(n_azar, len(resto)))
    return elegidas, {e["id"]: k for k, v in clase.items() for e in v}


def probar(e):
    r = {"nombre": e["name"]}
    desde = os.path.getsize(LOG)
    orden("run", "forzar", "rival", e["id"])
    r["listo"] = esperar(desde, ["listo (tramo", "no aparece"], 75) == "listo (tramo"
    if not r["listo"]:
        r["registro"] = [l for l in leer_desde(desde).splitlines() if "coliseo" in l][-3:]
        return r
    m = re.search(r" como (\d+) \((\d+) enemigos\)", leer_desde(desde))
    base = int(m.group(1)) if m else HUECO
    ents = [base, base + 1]
    time.sleep(2)
    antes = [chr_(x) for x in ents]
    r["cargados"] = all(antes)
    if not r["cargados"]:
        return r
    # El jugador al lado, como jugando: los que esperan quietos hasta verte (el Perro guardián) se despiertan.
    # (En la arena grande, «ir» lo saca del mapa: allí los enemigos ya vienen.)
    if not e.get("arena"):
        orden("ir", ents[0], 5)
        time.sleep(1)
    p0 = [pos(x) for x in ents]
    # El jugador quieto y sin atacar: lo que pierdan es del otro.
    time.sleep(QUIETO)
    despues = [chr_(x) for x in ents]
    p1 = [pos(x) for x in ents]
    r["se_mueven"] = [round(sum((a - b) ** 2 for a, b in zip(u, v)) ** 0.5, 1) if u and v else None for u, v in zip(p0, p1)]
    r["vida_perdida"] = [(a["hp"] - b["hp"]) if a and b else None for a, b in zip(antes, despues)]
    # Un roce de unos puntos (un sangrado que salpica) no es pelearse.
    r["se_pegan"] = any((v or 0) > ROCE for v in r["vida_perdida"])
    for x in ents:
        orden("matar", x)
        time.sleep(0.4)
    r["derrotado"] = esperar(desde, ["derrotado"], 30) is not None
    # Que se den de rebote vale (24-10: «que se peleen deliberadamente no, daños accidentales no me
    # importa»): la vida perdida queda como aviso. Lo de pelearse a propósito lo quita data/bandos.json.
    r["ok"] = bool(r["cargados"] and all((v or 0) > 0.5 for v in r["se_mueven"]) and r["derrotado"])
    return r


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    n = int(args[0]) if args and args[0].isdigit() else 30
    ids = [a for a in args if not a.isdigit()]
    elegidas, clase = muestra(n)
    if ids:
        todas = []
        for a in (1, 2, 3):
            todas += json.load(open(os.path.join(ROOT, "data", "coop", f"mezclas_{a}.json"), encoding="utf8"))["encounters"]
        elegidas = [e for e in todas if e["id"] in ids]
    hechos = json.load(open(SALIDA, encoding="utf8")) if os.path.exists(SALIDA) else {}
    os.makedirs(COPIA, exist_ok=True)
    for f in ("run.json", "perfil.json"):
        if os.path.exists(os.path.join(RT, f)):
            shutil.copy2(os.path.join(RT, f), os.path.join(COPIA, f))
    try:
        orden("dios", 1)
        orden("foco", 1)
        run_limpia(forzar=fase() != "Map")
        for i, e in enumerate(elegidas):
            if e["id"] in hechos and hechos[e["id"]].get("ok") and not ids:
                continue
            run_limpia()
            try:
                r = probar(e)
            except Exception as ex:
                r = {"nombre": e["name"], "error": repr(ex)}
            r.setdefault("ok", False)
            k = clase.get(e["id"])
            r["bandos"], r["grandes"] = (list(k[0]), k[1]) if k else (None, None)
            t0 = time.time()
            while time.time() - t0 < 60:
                f = fase()
                if f.startswith(("Reward", "Replace")):
                    orden("run", "elegir", 99)
                elif f == "Map":
                    break
                elif f.startswith("Arena") and time.time() - t0 > 20:
                    run_limpia(forzar=True)
                    break
                time.sleep(1.5)
            hechos[e["id"]] = r
            json.dump(hechos, open(SALIDA, "w", encoding="utf8"), indent=1, ensure_ascii=False)
            print(f"{i + 1}/{len(elegidas)} {e['id']} {e['name']} bandos {r['bandos']} grandes {r['grandes']}: {'ok' if r['ok'] else 'FALLO'}"
                  f" mueven {r.get('se_mueven')} pierden {r.get('vida_perdida')}", flush=True)
            if orden("ping", timeout=20) is None:
                print("el juego no responde; se para")
                break
    finally:
        orden("dios", 0)
        for f in ("run.json", "perfil.json"):
            if os.path.exists(os.path.join(COPIA, f)):
                shutil.copy2(os.path.join(COPIA, f), os.path.join(RT, f))
    malos = [k for k, v in hechos.items() if not v.get("ok")]
    print(f"probadas {len(hechos)}; fallos {len(malos)}: {malos}")


if __name__ == "__main__":
    main()
