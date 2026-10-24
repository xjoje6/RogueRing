"""Mide los encuentros del acto 1 en el Coliseo de Limgrave, con cada personaje.

Por encuentro: lo crea con su tramo de escalado, deja 25 s en los que el jugador ataca con R1 y los dos
aguantan (banco: `medir`), y apunta la vida del enemigo, cada golpe que recibe el jugador (fracción de
su vida) y cada golpe que da. Se retoma donde se quedó.

Requisito: juego abierto con lanzar.ps1 y una partida de pruebas cargada.
Uso: python tools/medir_balance.py [monje|samurai|tanque ...] [--muestra N]
  --muestra N: solo N encuentros repartidos por tipo (para comparar personajes sin medirlo todo).
Resultado: runtime/balance/medidas.json. La tabla sale con tools/tabla_balance.py.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from banco import ROOT, ir_al_coliseo, json_de, orden

OUT = os.path.join(ROOT, "runtime", "balance", "medidas.json")
PERSONAJES = ["monje", "samurai", "tanque"]
SEGUNDOS = 25
ENT0 = 79_790_500


def tramo(kind, t):
    """Tramo con el que sale cada tipo de encuentro (el normal, a media altura del mapa)."""
    normal = (t["normal"][0] + t["normal"][1]) // 2 // 10 * 10
    return {"Weak": t["debil"], "Normal": normal, "Elite": min(normal + t["elite"], t["jefe"]), "Boss": t["jefe"]}[kind]


def preparar(personaje):
    print(orden("run", "nueva", f"MEDIR{personaje.upper()}"))
    print(orden("run", "elegir", PERSONAJES.index(personaje), timeout=30))
    time.sleep(4)
    print(orden("run", "soltar"))


def muestra(encuentros, n):
    """n encuentros repartidos: los mismos tipos en la misma proporción, a saltos regulares."""
    out = []
    for kind in ("Weak", "Normal", "Elite", "Boss"):
        de_tipo = [e for e in encuentros if e["kind"] == kind]
        k = max(1, round(n * len(de_tipo) / len(encuentros))) if de_tipo else 0
        out += de_tipo[:: max(1, len(de_tipo) // k)][:k] if k else []
    return out


def main():
    args = sys.argv[1:]
    n_muestra = None
    if "--muestra" in args:
        i = args.index("--muestra")
        n_muestra = int(args[i + 1])
        del args[i:i + 2]
    pedidos = args or PERSONAJES
    acto = json.load(open(os.path.join(ROOT, "data", "acts", "1.json"), encoding="utf8"))
    esc = json.load(open(os.path.join(ROOT, "data", "escalado.json"), encoding="utf8"))
    tramos = esc["tramos"]["1"]
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    res = json.load(open(OUT, encoding="utf8")) if os.path.exists(OUT) else {}
    if orden("ping", timeout=30) is None:
        sys.exit("el juego no responde")
    orden("foco", 1)
    orden("dios", 0)
    ent = ENT0 + len(res)
    for p in pedidos:
        lista = muestra(acto["encounters"], n_muestra) if n_muestra else acto["encounters"]
        pendientes = [e for e in lista if f"{p}/{e['id']}" not in res]
        if not pendientes:
            continue
        preparar(p)
        for e in pendientes:
            if orden("ping", timeout=20) is None:
                sys.exit(f"el juego dejó de responder antes de {e['name']}")
            if not ir_al_coliseo():
                print("no llega al coliseo; se reintenta una vez")
                if not ir_al_coliseo():
                    sys.exit("sin coliseo")
            ent += 1
            t = tramo(e["kind"], tramos)
            orden("invocar", e["chr"], e["npc"], e["think"], ent, 8)
            info = None
            t0 = time.time()
            while time.time() - t0 < 15 and not info:
                time.sleep(1)
                info = json_de("chr", ent)
            r = {"personaje": p, "id": e["id"], "nombre": e["name"], "tipo": e["kind"], "tramo": t}
            if not info:
                r["error"] = "no aparece"
            else:
                orden("efecto_a", ent, t)
                time.sleep(1)
                orden("medir", ent, "atacar")
                time.sleep(SEGUNDOS)
                m = json_de("medida") or {}
                orden("medir", 0)
                orden("matar", ent)
                r.update(m)
            res[f"{p}/{e['id']}"] = r
            json.dump(res, open(OUT, "w", encoding="utf8"), ensure_ascii=False, indent=1)
            rec = r.get("recibidos", [])
            dad = r.get("dados", [])
            print(f"{p} {e['name']} ({e['kind']}, {t}): vida {r.get('vida_enemigo')}, "
                  f"recibe {len(rec)} golpes (máx {max(rec, default=0):.0%}), da {len(dad)}", flush=True)
    orden("foco", 0)


if __name__ == "__main__":
    main()
