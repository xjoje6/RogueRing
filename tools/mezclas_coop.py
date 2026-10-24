"""Parejas posibles del cooperativo (24-10: «un algoritmo vivo en el mod, que mezcle y genere parejas
distintas cada vez que alguien juega una run nueva en coop», y «que funcione siempre»).

Uso: python tools/mezclas_coop.py

Escribe data/coop/mezclas_<acto>.json con TODAS las parejas que cumplen las reglas (no un sorteo): el mod
sortea de ahí al empezar cada acto en cooperativo, con la semilla de la run (igual en los dos PCs), y
ModBuilder hace el coliseo de cada una (los enemigos colocados en el mapa son lo que sincroniza Seamless).

Reglas, para que cualquier pareja funcione:
- Dos combates de un enemigo del mismo acto y tipo, del coliseo (sin arena propia, cambios de fase ni
  animación de entrada), de modelo y nombre distintos.
- Vida base parecida: la mayor, como mucho RAZON_VIDA veces la menor (que ninguno se coma al otro).
- La arena según el tamaño de los dos juntos (tools/clasificar_arenas.py): dos grandes, a la arena grande.
  De esas, como mucho MAX_GRANDE por acto y tipo (cada una es una copia del mapa de Placidusax).
- Del mismo bando (teamType de NpcParam, con los de data/bandos.json ya pasados a enemigo corriente,
  como hace ModBuilder): los de bandos distintos se pegan entre ellos (tools/probar_mezclas.py).
- Fuera: las de data/mezclas_fallidas.json («fallidas» en el banco, «quitadas» en el editor), el enemigo
  que falla en dos parejas o más, los de NO_MEZCLAR y los marcados «No mezclar» en el editor
  (`no_mezclar` de data/mezclas_fallidas.json, por tools/aplicar_editor.py).
Nombre en español de cada pareja a data/i18n/es.json; su vida la ajusta tools/equilibrar_vida.py.
"""
import csv
import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
import clasificar_arenas

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
RAZON_VIDA = 3.0
# Parejas de la arena grande por acto y tipo, y veces que sale cada enemigo en ellas.
MAX_GRANDE = 30
USOS_GRANDE = 3
BANDOS = json.load(open(os.path.join(ROOT, "data", "bandos.json"), encoding="utf8"))


def bando(fila):
    t = int(fila["teamType"])
    return BANDOS["enemigo"] if t in BANDOS["a_enemigo"] else t
# Enemigos (modelo) que nunca van en pareja, con el motivo.
# El Perro guardián se queda de estatua hasta que te acercas: en el banco «fallaba» porque la prueba no se
# acercaba, pero jugando se despierta (24-10). Los golpes de rebote entre compañeros (los hechizos de
# las reinas semihumanas) valen: lo que no vale es que se ataquen a propósito, y eso lo quita data/bandos.json.
NO_MEZCLAR = {}


def candidatos(acto, npc):
    solos = json.load(open(os.path.join(ROOT, "data", "acts", f"{acto}.json"), encoding="utf8"))["encounters"]
    return [e for e in solos if e.get("chr") and not e.get("enemigos") and not e.get("fase_en")
            and not e.get("renacer") and not e.get("anim_inicio") and e.get("formato", "solo") == "solo" and e.get("npc") in npc]


def compatibles(a, b, npc):
    na, nb = npc[a["npc"]], npc[b["npc"]]
    if a["chr"] == b["chr"] or a["name"] == b["name"]:
        return False
    va, vb = float(na["hp"]), float(nb["hp"])
    if max(va, vb) > RAZON_VIDA * max(1.0, min(va, vb)):
        return False
    return bando(na) == bando(nb)


def main():
    npc = {int(r["ID"]): r for r in csv.DictReader(open(os.path.join(ROOT, "extract", "params", "NpcParam.csv"), encoding="utf8"))}
    fallidas_f = os.path.join(ROOT, "data", "mezclas_fallidas.json")
    previas = json.load(open(fallidas_f, encoding="utf8")) if os.path.exists(fallidas_f) else {}
    fuera = set(previas.get("fallidas", [])) | set(previas.get("quitadas", []))
    veces = {}
    for f in previas.get("fallidas", []):
        for n in f.split("_")[1:]:
            veces[int(n) // 10000] = veces.get(int(n) // 10000, 0) + 1
    vetados = {c for c, v in veces.items() if v >= 2} | set(NO_MEZCLAR) | set(previas.get("no_mezclar", []))
    es_f = os.path.join(ROOT, "data", "i18n", "es.json")
    es = json.load(open(es_f, encoding="utf8"))
    nombres_es = es.setdefault("enemigos", {})
    total = 0
    for acto in (1, 2, 3):
        # Las parejas de antes (con la etiqueta «mezcla») salen de la lista fija: ahora las sortea el mod.
        ruta = os.path.join(ROOT, "data", "coop", f"{acto}.json")
        coop = json.load(open(ruta, encoding="utf8"))
        coop["encounters"] = [e for e in coop["encounters"] if "mezcla" not in e.get("tags", [])]
        open(ruta, "w", encoding="utf8").write(json.dumps(coop, ensure_ascii=False, indent=1) + "\n")
        fijas = {e["id"] for e in coop["encounters"]}
        de_acto = [e for e in candidatos(acto, npc) if int(e["chr"]) not in vetados]
        # La vida que ya tenían ajustada se conserva (tools/equilibrar_vida.py la corrige si hace falta).
        mf = os.path.join(ROOT, "data", "coop", f"mezclas_{acto}.json")
        vidas = {e["id"]: e.get("vida") for e in json.load(open(mf, encoding="utf8"))["encounters"]} if os.path.exists(mf) else {}
        parejas = []
        for tipo in ("Weak", "Normal", "Elite", "Boss"):
            de = [e for e in de_acto if e["kind"] == tipo]
            del_tipo = []
            for i, a in enumerate(de):
                for b in de[i + 1:]:
                    eid = f"mezcla{acto}_{a['npc']}_{b['npc']}"
                    if eid in fuera or eid in fijas or not compatibles(a, b, npc):
                        continue
                    nombre = f"{a['name']} and {b['name']}"
                    p = {"id": eid, "name": nombre, "kind": tipo, "warp_flag": 0, "defeat_flag": 0, "tags": [eid, "mezcla"], "llegada": None,
                         "formato": "duo", "enemigos": [{k: x[k] for k in ("chr", "npc", "think")} for x in (a, b)]}
                    if vidas.get(eid):
                        p["vida"] = vidas[eid]
                    if clasificar_arenas.arena(p):
                        p["arena"] = clasificar_arenas.arena(p)
                    del_tipo.append((p, a, b))
            # Las de la arena grande, como mucho MAX_GRANDE por acto y tipo (las que el sorteo usa como
            # mucho): cada variante de ese mapa ocupa unos 300 KB. Variadas: cada enemigo en USOS_GRANDE.
            grandes = [x for x in del_tipo if x[0].get("arena")]
            random.Random(f"grande-{acto}-{tipo}").shuffle(grandes)
            usos, elegidas = {}, []
            for x in grandes:
                if len(elegidas) >= MAX_GRANDE:
                    break
                if any(usos.get(m["chr"], 0) >= USOS_GRANDE for m in (x[1], x[2])):
                    continue
                for m in (x[1], x[2]):
                    usos[m["chr"]] = usos.get(m["chr"], 0) + 1
                elegidas.append(x)
            quedan = [x for x in del_tipo if not x[0].get("arena")] + elegidas
            for p, a, b in quedan:
                parejas.append(p)
                ea, eb = nombres_es.get(a["name"]), nombres_es.get(b["name"])
                if ea and eb:
                    nombres_es[p["name"]] = f"{ea} y {eb}"
            n = len(quedan)
            print(f"acto {acto} {tipo:6}: {len(de)} de uno, {n} parejas posibles ({len(elegidas)} en la arena grande de {len(grandes)})")
            total += n
        json.dump({"_nota": "Parejas posibles del cooperativo (tools/mezclas_coop.py): el mod sortea de aquí al empezar cada acto. No editar a mano.",
                   "encounters": parejas}, open(mf, "w", encoding="utf8"), ensure_ascii=False, indent=1)
    open(es_f, "w", encoding="utf8", newline="").write(json.dumps(es, ensure_ascii=False, indent=1) + "\n")
    print(f"{total} parejas posibles")


if __name__ == "__main__":
    main()
