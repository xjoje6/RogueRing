"""Órdenes al banco de pruebas de la DLL (runtime/harness/cmd.txt) y sus respuestas (out.txt).

Uso: python tools/banco.py <orden> [args...]  (varias, separadas por ' ; '; «esperar <s>» espera)
"""
import json
import os
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
H = os.environ.get("ROGUE_HARNESS") or os.path.join(ROOT, "runtime", "harness")

_n = 0


def orden(*args, timeout=15.0):
    global _n
    _n += 1
    oid = f"b{int(time.time())}{_n}"
    with open(os.path.join(H, "cmd.txt"), "a", encoding="utf8") as f:
        f.write(f"{oid} {' '.join(str(a) for a in args)}\n")
    fin = time.time() + timeout
    while time.time() < fin:
        time.sleep(0.2)
        try:
            for linea in open(os.path.join(H, "out.txt"), encoding="utf8"):
                if linea.startswith(f"#{oid} "):
                    return linea[len(oid) + 2:].strip()
        except FileNotFoundError:
            pass
    return None


def json_de(*args, timeout=15.0):
    r = orden(*args, timeout=timeout)
    try:
        return json.loads(r) if r and r.startswith("{") else None
    except json.JSONDecodeError:
        return None


def ir_al_coliseo(espera=40):
    """Viaja al Coliseo de Limgrave (limpia cadáveres y deja al jugador en el centro)."""
    orden("flag", 1049300098, 1)
    time.sleep(4)
    t0 = time.time()
    while time.time() - t0 < espera:
        time.sleep(1.5)
        e = json_de("estado")
        fundido = orden("fundido")
        if e and "area: 45," in e.get("bloque_id", "") and e.get("hp", 0) > 0 and fundido and float(fundido) < 0.05:
            time.sleep(2)
            return True
    return False


if __name__ == "__main__":
    for c in " ".join(sys.argv[1:]).split(" ; "):
        if c.startswith("esperar "):
            time.sleep(float(c.split()[1]))
            continue
        print(c, "->", orden(*c.split()))
