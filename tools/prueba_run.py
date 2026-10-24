"""Piloto automático de una run (pruebas en segundo plano). Recorre el mapa prefiriendo los tipos de
nodo pedidos, mata a los bosses con el banco de pruebas, elige en las pantallas y guarda una captura
de cada pantalla de cartas en runtime/prueba_run/.

Requisitos: juego abierto con arrancar.ps1. Uso: python tools/prueba_run.py SEMILLA [nodos] [Shop,Rest,Treasure]
(PERSONAJE=n elige personaje; NEOW, RELIQUIAS, PELEA y RECIBIR ajustan la run)
"""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from banco import orden

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
CAPS = os.path.join(ROOT, "runtime", "prueba_run")
CAPTURA = os.path.join(ROOT, "dll", "target", "release", "captura.exe")


def estado():
    r = orden("run", "estado")
    return json.loads(r) if r and r.startswith("{") else {}


def captura(nombre):
    os.makedirs(CAPS, exist_ok=True)
    subprocess.run([CAPTURA, os.path.join(CAPS, nombre + ".png")], capture_output=True, timeout=20)


def recorrer(seed, nodos, prefer, personaje=0, en_combate=None):
    """Una run de `nodos` pasos con el personaje `personaje`. Devuelve los tipos de nodo resueltos.
    `en_combate(entidad)`: se llama en cada combate del coliseo antes de rematar al rival."""
    hechos = []
    orden("dios", 1)
    print(orden("run", "nueva", seed))
    # Primera pantalla: el selector de personaje.
    print("personaje:", orden("run", "elegir", personaje))
    # Segunda pantalla: la bendición inicial (la primera opción, o la que diga NEOW).
    print("bendición inicial:", orden("run", "elegir", int(os.environ.get("NEOW", "0"))))
    time.sleep(0.5)
    if estado().get("fase", "").startswith("Aumento"):
        print("aumento:", orden("run", "elegir", 0))
    for r in [x for x in os.environ.get("RELIQUIAS", "").split(",") if x]:
        print("  reliquia:", orden("run", "reliquia", r))
    for paso in range(nodos):
        e = estado()
        disp = e.get("disponibles", [])
        if not disp:
            print("sin nodos disponibles:", e.get("fase"))
            break
        # Elegir el primer nodo de un tipo preferido; si no hay, el primero.
        idx = next((i for i, d in enumerate(disp) if d[2] in prefer), 0)
        c, r, kind = disp[idx]
        print(f"paso {paso}: {kind} ({c},{r}) ->", orden("run", "ir", c, r))
        hechos.append(kind)
        time.sleep(1)
        e = estado()
        fase = e.get("fase", "")
        if fase.startswith(("Combat", "Arena")):
            boss = fase.split('id: "')[1].split('"')[0]
            t0 = time.time()
            def listo():
                cola = "".join(open(os.path.join(ROOT, "runtime", "harness", "harness.log"), encoding="utf8", errors="ignore").readlines()[-3:])
                return "llegada" in cola or "listo" in cola
            while time.time() - t0 < 60 and not listo():
                time.sleep(1.5)
            time.sleep(3)
            captura(f"{paso:02d}_combate")
            # En el coliseo el boss tiene una entidad propia (la última "creado ... como N" del registro).
            log = open(os.path.join(ROOT, "runtime", "harness", "harness.log"), encoding="utf8", errors="ignore").read()
            if fase.startswith("Arena"):
                boss = log.split(" como ")[-1].split()[0]
            if en_combate and fase.startswith("Arena"):
                en_combate(boss)
            # Unos segundos de pelea de verdad (daño del arma y de las bendiciones) antes de rematar.
            t1 = time.time()
            if os.environ.get("RECIBIR"):
                orden("dios", 0)
            while time.time() - t1 < float(os.environ.get("PELEA", "6")):
                orden("pulsar", "RB", 120)
                time.sleep(0.5)
            orden("dios", 1)
            orden("matar", boss)
            # Esperar a la pantalla de recompensa.
            t0 = time.time()
            while time.time() - t0 < 40 and not estado().get("fase", "").startswith("Reward"):
                time.sleep(1.5)
            time.sleep(1.5)
            captura(f"{paso:02d}_recompensa")
            print("  ->", orden("run", "elegir", 0))
            time.sleep(1)
            if estado().get("fase", "").startswith("Aumento"):
                print("  aumento:", orden("run", "elegir", 0))
        elif fase.startswith("Event"):
            time.sleep(1.5)
            print("  -> evento", orden("run", "elegir", 0), "|", orden("run", "elegir", 0))
            time.sleep(1)
            e2 = estado().get("fase", "")
            if e2.startswith(("Combat", "Arena")):
                print("  (el evento acabó en pelea; se deja para el siguiente paso)")
        elif fase.startswith(("Treasure", "Shop", "Rest")):
            time.sleep(1.5)
            captura(f"{paso:02d}_{fase.split(' ')[0].split('{')[0].lower()}")
            if fase.startswith("Shop"):
                print("  ->", orden("run", "elegir", 0), "|", orden("run", "elegir", 99))
            else:
                print("  ->", orden("run", "elegir", 0))
        time.sleep(1)
    print(json.dumps(estado(), ensure_ascii=False))
    return hechos


if __name__ == "__main__":
    recorrer(
        sys.argv[1] if len(sys.argv) > 1 else "PRUEBA",
        int(sys.argv[2]) if len(sys.argv) > 2 else 8,
        (sys.argv[3] if len(sys.argv) > 3 else "Shop,Rest,Treasure").split(","),
        int(os.environ.get("PERSONAJE", "0")),
    )
