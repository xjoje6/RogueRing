"""Valida los combates del coliseo (un coliseo por combate, docs/COOP_SALAS.md y
docs/LISTAS_Y_COMBATES.md): por cada encuentro de data/acts y data/coop lo fuerza en una run, mira que
la primera oleada salga como enemigos del mapa (grupo con red) y se mueva, y lo gana como toca a su
formato: oleada a oleada, al líder de la escolta, o esperando el aguante.

Requisitos: juego abierto con lanzar.ps1 y la partida cargada (autocargar.py). Copia
runtime/run.json y runtime/perfil.json y los devuelve al acabar. Retoma por donde iba si se corta.
Uso: python tools/validar_msb.py [--cerca] [id ...]
--cerca: antes de mirar si se mueve, pone al jugador a 6 m del primero y mira 5 s (los que esperan dormidos
o quietos hasta ver al jugador, como el Oso rojo o los invasores).
"""
import glob
import json
import os
import re
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from banco import ROOT, orden

RT = os.path.join(ROOT, "runtime")
LOG = os.path.join(RT, "harness", "harness.log")
SALIDA = os.path.join(RT, "validar_msb.json")
COPIA = os.path.join(RT, "validar_msb_copia")
HUECO = 45000990


def leer_desde(desde):
    with open(LOG, encoding="utf8", errors="ignore") as f:
        f.seek(desde)
        return f.read()


def esperar(desde, textos, limite):
    t0 = time.time()
    while time.time() - t0 < limite:
        nuevo = leer_desde(desde)
        for t in textos:
            if t in nuevo:
                return t
        time.sleep(0.5)
    return None


def fase():
    r = orden("run", "estado")
    try:
        return json.loads(r).get("fase", "")
    except Exception:
        return ""


def pos(ent):
    try:
        return json.loads(orden("chr", ent))["pos"]
    except Exception:
        return None


def encuentros():
    out = []
    for f in sorted(glob.glob(os.path.join(ROOT, "data", "acts", "*.json"))) + sorted(glob.glob(os.path.join(ROOT, "data", "coop", "*.json"))):
        for e in json.load(open(f, encoding="utf8"))["encounters"]:
            if e.get("enemigos") or e.get("chr") is not None:
                out.append(e)
    return out


def oleadas(e):
    n = len(e.get("enemigos") or [0])
    o = [w for w in e.get("oleadas", []) if w]
    return o or [list(range(n))]


def run_limpia(forzar=False):
    """Run nueva sin bendiciones: las que se van ganando (efectos que paran o congelan al rival) daban por
    quietos a todos los siguientes (24-10, desde el combate 26 de una tanda)."""
    try:
        est = json.loads(orden("run", "estado"))
    except Exception:
        est = {}
    if not forzar and est.get("fase") == "Map" and not est.get("reliquias"):
        return
    orden("run", "nueva", "VALIDAMSB")
    time.sleep(12)
    orden("run", "elegir", 1)
    time.sleep(1.5)
    orden("run", "elegir", 1)
    time.sleep(3)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    cerca = "--cerca" in sys.argv
    todos = encuentros()
    if args:
        todos = [e for e in todos if e["id"] in args]
    hechos = json.load(open(SALIDA, encoding="utf8")) if os.path.exists(SALIDA) else {}
    os.makedirs(COPIA, exist_ok=True)
    for f in ("run.json", "perfil.json"):
        if os.path.exists(os.path.join(RT, f)):
            shutil.copy2(os.path.join(RT, f), os.path.join(COPIA, f))
    try:
        orden("dios", 1)
        orden("foco", 1)
        run_limpia(forzar=fase() != "Map")
        for i, e in enumerate(todos):
            eid = e["id"]
            if eid in hechos and hechos[eid].get("ok") and not args:
                continue
            run_limpia()
            r = {"nombre": e["name"], "formato": e.get("formato", "solo")}
            desde = os.path.getsize(LOG)
            orden("run", "forzar", "rival", eid)
            llego = esperar(desde, ["listo (tramo", "no aparece"], 75)
            log = leer_desde(desde)
            r["jefe_msb"] = "colocado en el MSB" in log
            r["listo"] = llego == "listo (tramo"
            m = re.search(r" como (\d+) \((\d+) enemigos\)", log)
            base = int(m.group(1)) if m else HUECO
            olas = oleadas(e)
            if r["listo"]:
                time.sleep(2)
                grupos = set()
                for k in olas[0]:
                    try:
                        grupos.add((json.loads(orden("red", base + k))["chr"] or {}).get("set_index"))
                    except Exception:
                        grupos.add(None)
                r["grupos"] = sorted(g for g in grupos if g is not None)
                r["cargados"] = None not in grupos
                if cerca:
                    orden("ir", base + olas[0][0], 6)
                    time.sleep(1)
                a = pos(base + olas[0][0])
                time.sleep(5 if cerca else 2.5)
                b = pos(base + olas[0][0])
                r["se_mueve"] = round(sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5, 2) if a and b else None
                formato = e.get("formato", "solo")
                if formato == "aguante":
                    r["derrotado"] = esperar(desde, ["derrotado"], e.get("segundos", 60) + 15) is not None
                elif formato == "escolta":
                    orden("matar", base + e.get("lider", 0))
                    r["derrotado"] = esperar(desde, ["derrotado"], 30) is not None
                else:
                    r["oleadas_vistas"] = 1
                    for w, ola in enumerate(olas):
                        if w > 0:
                            if not esperar(desde, [f"oleada {w + 1}/{len(olas)}"], 20):
                                break
                            r["oleadas_vistas"] = w + 1
                            time.sleep(4)
                        for k in ola:
                            orden("matar", base + k)
                            time.sleep(0.3)
                    r["derrotado"] = esperar(desde, ["derrotado"], 30) is not None
            else:
                r["registro"] = [l for l in leer_desde(desde).splitlines() if "coliseo" in l][-4:]
            # De vuelta al mapa (recompensa saltada).
            t0 = time.time()
            while time.time() - t0 < 60:
                f = fase()
                if f.startswith(("Reward", "Replace")):
                    orden("run", "elegir", 99)
                elif f == "Map":
                    break
                time.sleep(1.5)
            msb_ok = not r["jefe_msb"] or r.get("grupos") == [107]
            r["ok"] = bool(r.get("listo") and r.get("cargados") and msb_ok and r.get("derrotado") and (r.get("se_mueve") or 0) > 0.05)
            hechos[eid] = r
            json.dump(hechos, open(SALIDA, "w", encoding="utf8"), indent=1, ensure_ascii=False)
            print(f"{i + 1}/{len(todos)} {eid} {e['name']} [{r['formato']}]: {'ok' if r['ok'] else 'FALLO'} grupos {r.get('grupos')} mueve {r.get('se_mueve')}", flush=True)
            if orden("ping", timeout=20) is None:
                print("el juego no responde; se para")
                break
    finally:
        orden("dios", 0)
        for f in ("run.json", "perfil.json"):
            if os.path.exists(os.path.join(COPIA, f)):
                shutil.copy2(os.path.join(COPIA, f), os.path.join(RT, f))
    malos = {k: v for k, v in hechos.items() if not v.get("ok")}
    print(f"validados {len(hechos)}; fallos {len(malos)}: {list(malos)}")


if __name__ == "__main__":
    main()
