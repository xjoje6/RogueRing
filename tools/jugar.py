"""Parte de jugar.ps1: espera al juego, pulsa Continuar hasta cargar el perfil y empieza la run."""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(__file__))
from banco import orden

seed = sys.argv[1] if len(sys.argv) > 1 else "ELDEN"
t0 = time.time()
while orden("ping", timeout=3) is None:
    if time.time() - t0 > 120:
        sys.exit("el juego no responde")
    time.sleep(0.5)
t0 = time.time()
while time.time() - t0 < 150:
    e = orden("estado", timeout=5)
    j = json.loads(e) if e and e.startswith("{") else {}
    if j.get("hp", 0) > 0 and "area: 255" not in j.get("bloque_id", "area: 255"):
        break
    orden("pulsar", "A", 150)
    time.sleep(2.5)
time.sleep(2)
print(orden("run", "nueva", seed))
print(f"Run con semilla {seed}. Cruceta para elegir, A para confirmar.")
