"""Tras lanzar.ps1: mueve el juego al monitor virtual y pulsa A con el mando virtual hasta que la
partida del perfil esté cargada (jugador con vida > 0). Sin tocar teclado ni ratón del sistema."""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from banco import orden

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

t0 = time.time()
while orden("ping", timeout=3) is None:
    if time.time() - t0 > 120:
        sys.exit("el banco de pruebas no responde")
    time.sleep(0.5)
pos = open(os.path.join(ROOT, "runtime", "pantalla.pos")).read().split()
# En cuanto exista la ventana, al monitor virtual (la DLL la mantendrá ahí aunque el juego la mueva).
for _ in range(60):
    r = orden("ventana", int(pos[0]) + 40, int(pos[1]) + 40, 1296, 759)
    if r == "ok":
        break
    time.sleep(0.5)
print("ventana:", r)
# Antes de nada: que no le maten al aparecer y que el juego crea tener el foco (si no, ignora el mando).
orden("dios", 1)
print("foco:", orden("foco", 1))

t0 = time.time()
while time.time() - t0 < 150:
    e = orden("estado", timeout=5)
    try:
        j = json.loads(e) if e else {}
        # Cargada = jugador con vida y en un bloque de mapa real (durante la carga el bloque es 255).
        if j.get("hp", 0) > 0 and "area: 255" not in j.get("bloque_id", "area: 255"):
            print(f"partida cargada en {time.time() - t0:.0f} s")
            sys.exit(0)
    except json.JSONDecodeError:
        pass
    orden("pulsar", "A", 150)
    time.sleep(2.5)
# Captura para ver dónde se quedó.
subprocess.run([os.path.join(ROOT, "dll", "target", "release", "captura.exe"), os.path.join(ROOT, "runtime", "autocarga_fallo.png")], capture_output=True, timeout=20)
sys.exit("no se cargó la partida (captura en runtime/autocarga_fallo.png)")
