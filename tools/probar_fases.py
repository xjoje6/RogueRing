"""Prueba en el banco los jefes de dos fases con nuestro sistema (24-10: «comprobar todos los bosses con
2 fases si funcionan bien con nuestro sistema»).

Uso: python tools/probar_fases.py [id ...]

Por cada jefe lo fuerza en una run limpia (como tools/validar_msb.py), espera a que salga y según cómo cambia
de fase:
- `fase_en` (un personaje por fase): le baja la vida por debajo del umbral y mira que avise, que se vaya la
  primera y que la segunda aparezca, se mueva y se pueda ganar.
- Oleadas sin `fase_en` (la Bestia de Elden): mata la primera y mira que salga la segunda.
- `renacer` (Malenia): le baja la vida, mira que pase a su segunda fase y que vuelva a vida llena.
- `ultimo_en_pie` (dúos de cooperativo): mata al otro y mira la segunda fase del que queda.
- El resto (la fase la lleva su propia IA): le deja la vida al 45 % y apunta las animaciones de 12 s, para
  ver si hace la de su cambio de fase (las 20000 del juego) o sigue como si nada.
Escribe runtime/probar_fases.json y lo resume al acabar.
Requisitos: juego abierto con lanzar.ps1 y la partida cargada (autocargar.py).
"""
import json
import os
import re
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from banco import ROOT, orden
from validar_msb import COPIA, HUECO, LOG, RT, esperar, fase, leer_desde, oleadas, pos, run_limpia

SALIDA = os.path.join(RT, "probar_fases.json")
# Jefes de una sola fila cuya segunda fase es de su IA (las que dice la wiki del juego).
PROPIAS = {"Putrescent Knight", "Scadutree Avatar", "Divine Beast Dancing Lion", "Rellana, Twin Moon Knight", "Romina, Saint of the Bud", "Rennala",
           "Borealis the Freezing Fog"}


def chr_(ent):
    try:
        return json.loads(orden("chr", ent))
    except Exception:
        return None


def movido(ent, segundos=4.0):
    a = pos(ent)
    time.sleep(segundos)
    b = pos(ent)
    return round(sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5, 2) if a and b else None


def bajar(ent, razon):
    c = chr_(ent)
    if not c:
        return None
    hp = max(1, int(c["hp_max"] * razon))
    orden("vida_chr", ent, hp)
    return hp


def anims(ent, segundos=12.0):
    vistas = []
    t0 = time.time()
    while time.time() - t0 < segundos:
        c = chr_(ent)
        if c and (not vistas or vistas[-1] != c["anim"]):
            vistas.append(c["anim"])
        time.sleep(0.3)
    return vistas


def tipo(e):
    if e.get("ultimo_en_pie"):
        return "ultimo"
    if e.get("renacer"):
        return "renacer"
    if e.get("fase_en"):
        return "fase_en"
    if len(oleadas(e)) > 1 and e["kind"] == "Boss":
        return "oleadas"
    return "propia"


def candidatos():
    out = []
    for carpeta in ("acts", "coop"):
        for a in (1, 2, 3):
            for e in json.load(open(os.path.join(ROOT, "data", carpeta, f"{a}.json"), encoding="utf8"))["encounters"]:
                if e.get("ultimo_en_pie") or e.get("renacer") or e.get("fase_en") or e.get("anim_inicio"):
                    out.append(e)
                elif carpeta == "acts" and e.get("chr") and not e.get("enemigos") and (e["kind"] == "Boss" or e["name"] in PROPIAS):
                    out.append(e)
                elif carpeta == "acts" and e["kind"] == "Boss" and len(oleadas(e)) > 1:
                    out.append(e)
    return out


def acercar(e, ent):
    # En las arenas grandes «ir» lo saca del mapa y muere al caer: allí el jefe ya viene.
    if not e.get("arena"):
        orden("ir", ent, 8)


def ganar(desde, ents):
    for x in ents:
        orden("matar", x)
        time.sleep(0.4)
    return esperar(desde, ["derrotado"], 30) is not None


def probar(e):
    r = {"nombre": e["name"], "tipo": tipo(e)}
    desde = os.path.getsize(LOG)
    orden("run", "forzar", "rival", e["id"])
    llego = esperar(desde, ["listo (tramo", "no aparece"], 75)
    r["listo"] = llego == "listo (tramo"
    if not r["listo"]:
        r["registro"] = [l for l in leer_desde(desde).splitlines() if "coliseo" in l][-4:]
        return r
    m = re.search(r" como (\d+) \((\d+) enemigos\)", leer_desde(desde))
    base = int(m.group(1)) if m else HUECO
    olas = oleadas(e)
    ent = base + olas[0][0]
    time.sleep(2)
    acercar(e, ent)
    r["se_mueve"] = movido(ent)
    t = r["tipo"]
    if t in ("fase_en", "oleadas"):
        if t == "fase_en":
            r["vida_puesta"] = bajar(ent, e["fase_en"] * 0.8)
        else:
            orden("matar", ent)
        claves = ["empieza a cambiar de fase", "cambia de fase", "aparece en su segunda fase"] if t == "fase_en" else ["oleada 2/2"]
        r["pasos"] = []
        for c in claves:
            if esperar(desde, [c], 30):
                r["pasos"].append(c)
        time.sleep(4)
        ent2 = base + olas[1][0]
        c2 = chr_(ent2)
        r["segunda_cargada"] = bool(c2)
        r["segunda_vida"] = [c2["hp"], c2["hp_max"]] if c2 else None
        acercar(e, ent2)
        r["segunda_se_mueve"] = movido(ent2, 5)
        r["derrotado"] = ganar(desde, [ent2])
        r["ok"] = len(r["pasos"]) == len(claves) and r["segunda_cargada"] and (r["segunda_se_mueve"] or 0) > 0.05 and r["derrotado"]
    elif t == "renacer":
        rn = e["renacer"]
        r["vida_puesta"] = bajar(ent, rn["vida"] * 0.5)
        r["pasa"] = esperar(desde, ["pasa a su segunda fase"], 15) is not None
        time.sleep(rn.get("segundos", 4) + 2)
        c = chr_(ent) or {}
        r["vida_tras"] = [c.get("hp"), c.get("hp_max")]
        r["vida_llena"] = bool(c) and c["hp"] >= c["hp_max"] * 0.95
        r["se_mueve_tras"] = movido(ent, 5)
        r["derrotado"] = ganar(desde, [ent])
        r["ok"] = r["pasa"] and r["vida_llena"] and (r["se_mueve_tras"] or 0) > 0.05 and r["derrotado"]
    elif t == "ultimo":
        ola = [base + k for k in olas[0]]
        con_fase = {u["enemigo"] for u in e["ultimo_en_pie"]}
        r["casos"] = []
        # Uno por superviviente con segunda fase; para el segundo hace falta otra run.
        for n, quien in enumerate(sorted(con_fase)):
            if n > 0:
                orden("run", "elegir", 99)
                time.sleep(3)
                run_limpia(forzar=True)
                desde = os.path.getsize(LOG)
                orden("run", "forzar", "rival", e["id"])
                if esperar(desde, ["listo (tramo"], 75) is None:
                    r["casos"].append({"superviviente": quien, "listo": False})
                    continue
                m = re.search(r" como (\d+) \((\d+) enemigos\)", leer_desde(desde))
                base = int(m.group(1)) if m else HUECO
                ola = [base + k for k in olas[0]]
                time.sleep(3)
            caso = {"superviviente": quien}
            sup = ola[quien]
            for k, x in enumerate(ola):
                if k != quien:
                    orden("matar", x)
            caso["queda"] = esperar(desde, ["último en pie: queda"], 15) is not None
            fase2 = next(u for u in e["ultimo_en_pie"] if u["enemigo"] == quien)
            if fase2.get("fase2"):
                caso["se_transforma"] = esperar(desde, ["se transforma"], 15) is not None
                mm = re.search(r"se transforma \((\d+) -> (\d+)\)", leer_desde(desde))
                nuevo = int(mm.group(2)) if mm else None
                time.sleep(8)
                c = chr_(nuevo) if nuevo else None
                caso["segunda_cargada"] = bool(c)
                caso["segunda_vida"] = [c["hp"], c["hp_max"]] if c else None
                acercar(e, nuevo or sup)
                caso["segunda_se_mueve"] = movido(nuevo, 5) if nuevo else None
                caso["derrotado"] = ganar(desde, [nuevo] if nuevo else [sup])
                caso["ok"] = caso["queda"] and caso["se_transforma"] and caso["segunda_cargada"] and (caso["segunda_se_mueve"] or 0) > 0.05 and caso["derrotado"]
            else:
                time.sleep(fase2.get("renacer", {}).get("segundos", 4) + 2)
                c = chr_(sup) or {}
                caso["vida_llena"] = bool(c) and c["hp"] >= c["hp_max"] * 0.95
                caso["se_mueve_tras"] = movido(sup, 5)
                caso["derrotado"] = ganar(desde, [sup])
                caso["ok"] = caso["queda"] and caso["vida_llena"] and (caso["se_mueve_tras"] or 0) > 0.05 and caso["derrotado"]
            r["casos"].append(caso)
        r["ok"] = all(c.get("ok") for c in r["casos"])
    else:
        r["vida_puesta"] = bajar(ent, 0.45)
        r["anims"] = anims(ent, 30)
        try:
            r["efectos_jugador"] = json.loads(orden("efectos", 0))
        except Exception:
            r["efectos_jugador"] = None
        r["anim_de_fase"] = [a for a in r["anims"] if 20000 <= a < 21000]
        try:
            r["efectos"] = json.loads(orden("efectos", ent))
        except Exception:
            r["efectos"] = orden("efectos", ent)
        c = chr_(ent) or {}
        r["vida_tras"] = [c.get("hp"), c.get("hp_max")]
        r["derrotado"] = ganar(desde, [base + k for ola in olas for k in ola])
        # Sin animación de fase no es un fallo seguro (algunos cambian sin ella); se mira a mano.
        r["ok"] = (r["se_mueve"] or 0) > 0.05 and r["derrotado"]
    r["murio"] = "run terminada: muerte" in leer_desde(desde)
    if r["murio"]:
        r["ok"] = False
    return r


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    todos = candidatos()
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
            if e["id"] in hechos and hechos[e["id"]].get("ok") and not args:
                continue
            run_limpia()
            try:
                r = probar(e)
            except Exception as ex:
                r = {"nombre": e["name"], "tipo": tipo(e), "error": repr(ex), "ok": False}
            t0 = time.time()
            while time.time() - t0 < 60:
                f = fase()
                if f.startswith(("Reward", "Replace")):
                    orden("run", "elegir", 99)
                elif f == "Map":
                    break
                elif f.startswith("Arena") and time.time() - t0 > 20:
                    # No se ha ganado: run nueva para seguir.
                    run_limpia(forzar=True)
                    break
                time.sleep(1.5)
            r.setdefault("ok", False)
            hechos[e["id"]] = r
            json.dump(hechos, open(SALIDA, "w", encoding="utf8"), indent=1, ensure_ascii=False)
            extra = r.get("pasos") or r.get("anim_de_fase") or [c.get("ok") for c in r.get("casos", [])] or r.get("vida_llena")
            print(f"{i + 1}/{len(todos)} {e['id']} {e['name']} [{r['tipo']}]: {'ok' if r['ok'] else 'FALLO'} {extra}", flush=True)
            if orden("ping", timeout=20) is None:
                print("el juego no responde; se para")
                break
    finally:
        orden("dios", 0)
        for f in ("run.json", "perfil.json"):
            if os.path.exists(os.path.join(COPIA, f)):
                shutil.copy2(os.path.join(COPIA, f), os.path.join(RT, f))
    malos = [k for k, v in hechos.items() if not v.get("ok")]
    print(f"probados {len(hechos)}; fallos {len(malos)}: {malos}")


if __name__ == "__main__":
    main()
