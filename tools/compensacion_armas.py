"""Compensación del daño de las armas que no son del estilo de cada personaje (data/compensacion_armas.json).

Uso: python tools/compensacion_armas.py

Desde que se quitaron los requisitos, cualquier personaje puede llevar cualquier arma, pero sus atributos
son fijos: un Espadón de luna (inteligencia) en el Samurái pega poco. Por cada personaje y cada arma que
no es de su estilo (y cada afinidad), con los datos del juego (extract/params: EquipParamWeapon,
ReinforceParamWeapon, CalcCorrectGraph, AttackElementCorrectParam) se calcula el ataque del arma al
máximo de mejora con sus atributos y con los mejores para ella (los mismos puntos repartidos a su favor).
La compensación lo deja en COMPENSACION del ideal: casi como si el personaje estuviera hecho para ella,
pero un poco por debajo de las de su estilo, y nunca por encima de lo que daría bien construida (el jugador,
24-10). La DLL la pone con un efecto de ataque mientras el arma está en la mano derecha.
"""
import csv
import itertools
import json
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
P = os.path.join(ROOT, "extract", "params")
COMPENSACION = 0.9
TIPOS = ["Physics", "Magic", "Fire", "Thunder", "Dark"]
# Atributos de ataque en el orden del juego y su índice en personajes.json («vig men agu fue des int fe arc»).
STATS = [("Strength", "correctStrength", 3), ("Dexterity", "correctAgility", 4), ("Magic", "correctMagic", 5), ("Faith", "correctFaith", 6), ("Luck", "correctLuck", 7)]
RATE = {"correctStrength": "correctStrengthRate", "correctAgility": "correctAgilityRate", "correctMagic": "correctMagicRate", "correctFaith": "correctFaithRate", "correctLuck": "correctLuckRate"}
ATK_RATE = {"Physics": "physicsAtkRate", "Magic": "magicAtkRate", "Fire": "fireAtkRate", "Thunder": "thunderAtkRate", "Dark": "darkAtkRate"}
BASE = {"Physics": "attackBasePhysics", "Magic": "attackBaseMagic", "Fire": "attackBaseFire", "Thunder": "attackBaseThunder", "Dark": "attackBaseDark"}
NOMBRE_ELEM = {"Physics": "Physics", "Magic": "Magic", "Fire": "Fire", "Thunder": "Thunder", "Dark": "Dark"}


def tabla(nombre):
    return {int(r["ID"]): r for r in csv.DictReader(open(os.path.join(P, nombre + ".csv"), encoding="utf8"))}


ARMAS = tabla("EquipParamWeapon")
MEJORA = tabla("ReinforceParamWeapon")
CURVAS = tabla("CalcCorrectGraph")
ELEMENTOS = tabla("AttackElementCorrectParam")


def f(r, k, d=0.0):
    try:
        return float(r.get(k, d))
    except (TypeError, ValueError):
        return d


def curva(id_, x):
    """CalcCorrectGraph: crecimiento (0..1) con el atributo x."""
    c = CURVAS[id_]
    mx = [f(c, f"stageMaxVal{i}") for i in range(5)]
    gr = [f(c, f"stageMaxGrowVal{i}") for i in range(5)]
    adj = [f(c, f"adjPt_maxGrowVal{i}") for i in range(5)]
    if x <= mx[0]:
        return gr[0] / 100
    for i in range(4):
        if x <= mx[i + 1]:
            r = (x - mx[i]) / max(mx[i + 1] - mx[i], 1e-6)
            a = adj[i]
            r = r ** a if a > 0 else 1 - (1 - r) ** (-a)
            return (gr[i] + (gr[i + 1] - gr[i]) * r) / 100
    return gr[4] / 100


def nivel_max(w):
    t = int(f(w, "reinforceTypeId"))
    return 25 if t + 25 in MEJORA else 10 if t + 10 in MEJORA else 0


def ataque(w, attrs):
    """Ataque total (los cinco tipos) del arma al máximo de mejora con estos atributos (fue, des, int, fe, arc)."""
    m = MEJORA[int(f(w, "reinforceTypeId")) + nivel_max(w)]
    el = ELEMENTOS.get(int(f(w, "attackElementCorrectId")), {})
    total = 0.0
    for t in TIPOS:
        base = f(w, BASE[t]) * f(m, ATK_RATE[t], 1.0)
        if base <= 0:
            continue
        extra = 0.0
        for (stat, campo, _), valor in zip(STATS, attrs):
            if f(el, f"is{stat if stat != 'Luck' else 'Luck'}Correct_by{t}") < 1:
                continue
            sobre = f(el, f"overwrite{stat}CorrectRate_by{t}", -1)
            esc = (sobre if sobre >= 0 else f(w, campo)) * f(m, RATE[campo], 1.0) / 100
            extra += base * esc * curva(int(f(w, f"correctType_{t}")), valor)
        total += base + extra
    return total


def main():
    pj = json.load(open(os.path.join(ROOT, "data", "personajes.json"), encoding="utf8"))["personajes"]
    pool = json.load(open(os.path.join(ROOT, "data", "objetos", "pool.json"), encoding="utf8"))
    pool = pool if isinstance(pool, list) else next(v for v in pool.values() if isinstance(v, list))
    armas = [i for i in pool if i["categoria"] == "Arma"]
    salida = {}
    for c in pj:
        attrs = [c["atributos"][i] for _, _, i in STATS]
        estilo = set(c.get("estilo", []))
        filas = {}
        for it in armas:
            if it["tipo"] in estilo:
                continue
            for afin in range(0, 1300, 100):
                w = ARMAS.get(it["id"] + afin)
                if not w:
                    continue
                real = ataque(w, attrs)
                ideal = max(ataque(w, p) for p in set(itertools.permutations(attrs)))
                if real <= 0:
                    continue
                filas[str(it["id"] + afin)] = round(COMPENSACION * ideal / real, 3)
        salida[c["id"]] = filas
        vals = sorted(filas.values())
        print(f"{c['id']}: {len(filas)} armas fuera de estilo; compensación mín {vals[0]}, mediana {vals[len(vals) // 2]}, máx {vals[-1]}")
    out = {
        "_nota": "Lo escribe tools/compensacion_armas.py. No editar a mano. Por personaje, armas que no son de su estilo (id con afinidad, sin mejora): multiplicador de su ataque para dejarlo en el 90 % del que daría con los atributos repartidos a su favor.",
        "compensacion": COMPENSACION,
        "personajes": salida,
    }
    open(os.path.join(ROOT, "data", "compensacion_armas.json"), "w", encoding="utf8").write(json.dumps(out, ensure_ascii=False, indent=1) + "\n")
    # Unas cuantas, para ver que tiene sentido.
    for c in pj:
        for n in ["Dark Moon Greatsword", "Moonveil", "Rivers of Blood", "Blasphemous Blade", "Dagger", "Zweihander"]:
            it = next((i for i in armas if i["nombre"] == n), None)
            if it and str(it["id"]) in salida[c["id"]]:
                print(f"  {c['id']:10} {n:22} x{salida[c['id']][str(it['id'])]}")


if __name__ == "__main__":
    main()
