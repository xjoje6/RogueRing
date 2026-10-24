"""Prueba de humo en el juego: una run corta con cada personaje (prefiriendo élite, evento, tienda,
gracia y tesoro) y, en el primer combate de cada una, pausa, proyectil y rotura de postura. Falla si
el juego se cierra, si sale un PÁNICO en el registro o si un paso no responde.

Requisitos: juego abierto con lanzar.ps1 (mejor con -Savefile de pruebas) y la partida cargada
(autocargar.py). Copia runtime/run.json y runtime/perfil.json y los devuelve al acabar.
Uso: python tools/prueba_humo.py [nodos por run]
"""
import json
import os
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from banco import ROOT, json_de, orden
from prueba_run import recorrer

RT = os.path.join(ROOT, "runtime")
LOG = os.path.join(RT, "harness", "harness.log")
PREFIERE = ["Elite", "Unknown", "Shop", "Rest", "Treasure"]
GUIJARRO = 10683000  # proyectil visual de las bendiciones (data/reliquias.json)


class Fallo(Exception):
    pass


def juego_vivo():
    # En bytes: tasklist escribe en la codificación de la consola.
    r = subprocess.run(["tasklist", "/FI", "IMAGENAME eq eldenring.exe"], capture_output=True)
    return b"eldenring.exe" in r.stdout.lower()


def comprobar(desde, paso):
    if not juego_vivo():
        raise Fallo(f"el juego se ha cerrado ({paso})")
    with open(LOG, encoding="utf8", errors="ignore") as f:
        f.seek(desde)
        nuevo = f.read()
    if "PÁNICO" in nuevo:
        raise Fallo(f"pánico en el registro ({paso}): " + nuevo[nuevo.index("PÁNICO"):][:300])
    if orden("ping", timeout=20) is None:
        raise Fallo(f"el banco no responde ({paso})")


def en_combate(ent, hechos):
    if hechos.get("combate"):
        return
    orden("dios", 1)
    orden("pulsar", "START", 150)
    time.sleep(1.5)
    abierta = (json_de("run", "estado") or {}).get("pausa")
    orden("pulsar", "B", 150)
    time.sleep(1.0)
    cerrada = not (json_de("run", "estado") or {}).get("pausa")
    bala = orden("bala", GUIJARRO, ent)
    time.sleep(1.0)
    rotura = orden("romper", ent)
    hechos["combate"] = {"pausa": bool(abierta and cerrada), "proyectil": bala == "ok", "rotura": rotura == "true"}


def main():
    nodos = int(sys.argv[1]) if len(sys.argv) > 1 else 6
    copia = os.path.join(RT, "humo_copia")
    os.makedirs(copia, exist_ok=True)
    for f in ("run.json", "perfil.json"):
        if os.path.exists(os.path.join(RT, f)):
            shutil.copy2(os.path.join(RT, f), os.path.join(copia, f))
    desde = os.path.getsize(LOG) if os.path.exists(LOG) else 0
    informe, ok = {}, True
    try:
        if orden("ping", timeout=30) is None:
            raise Fallo("el juego no responde")
        orden("foco", 1)
        for p, nombre in enumerate(["monje", "samurai", "tanque"]):
            hechos = {}
            tipos = recorrer(f"HUMO{p}{int(time.time()) % 1000}", nodos, PREFIERE, p, lambda e: en_combate(e, hechos))
            comprobar(desde, nombre)
            informe[nombre] = {"nodos": tipos, **hechos}
            print(nombre, json.dumps(informe[nombre], ensure_ascii=False), flush=True)
            c = hechos.get("combate", {})
            if not c or not all(c.values()):
                ok = False
    except Fallo as e:
        ok = False
        informe["fallo"] = str(e)
        print("FALLO:", e)
    finally:
        orden("dios", 0)
        for f in ("run.json", "perfil.json"):
            if os.path.exists(os.path.join(copia, f)):
                shutil.copy2(os.path.join(copia, f), os.path.join(RT, f))
    vistos = {t for v in informe.values() if isinstance(v, dict) for t in v.get("nodos", [])}
    faltan = sorted(set(PREFIERE + ["Monster"]) - vistos)
    print(f"\nnodos vistos: {sorted(vistos)}; sin ver: {faltan}")
    print("PRUEBA DE HUMO:", "OK" if ok else "FALLA")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
