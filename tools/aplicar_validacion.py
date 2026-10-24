"""Tras validar en el banco los combates nuevos (tools/validar_msb.py), quita los que fallan.

Uso: python tools/aplicar_validacion.py [--aplicar]

- Combates nuevos de un modelo sin usar (etiqueta «nuevo_24_10») que fallan: fuera de data/acts y su modelo
  a data/enemigos_fuera.json con lo que pasó en el banco (lo enseña el editor).
- Parejas de mezcla que fallan: fuera de data/coop y a data/mezclas_fallidas.json (tools/mezclas_coop.py
  no las vuelve a sacar).
"""
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))


def motivo(r):
    if not r.get("listo"):
        return "En el banco (24-10) no aparece en el coliseo."
    if not r.get("cargados"):
        return "En el banco (24-10) no termina de cargar como enemigo."
    if (r.get("se_mueve") or 0) <= 0.05:
        return "En el banco (24-10) se queda quieto: no pelea."
    if not r.get("derrotado"):
        return "En el banco (24-10) no se puede ganar el combate."
    return "En el banco (24-10) falló."


def main():
    aplicar = "--aplicar" in sys.argv
    res = json.load(open(os.path.join(ROOT, "runtime", "validar_msb.json"), encoding="utf8"))
    ids = []
    for nombre in ("validar_ids.txt", "revalidar_ids.txt"):
        ruta = os.path.join(ROOT, "runtime", nombre)
        if os.path.exists(ruta):
            ids += [i for i in open(ruta, encoding="utf8").read().split() if i not in ids]
    malos = {i: res[i] for i in ids if i in res and not res[i].get("ok")}
    sin = [i for i in ids if i not in res]
    print(f"{len(ids)} por validar: {len(ids) - len(sin) - len(malos)} bien, {len(malos)} mal, {len(sin)} sin resultado")
    fuera_f = os.path.join(ROOT, "data", "enemigos_fuera.json")
    fuera = json.load(open(fuera_f, encoding="utf8"))
    fallidas_f = os.path.join(ROOT, "data", "mezclas_fallidas.json")
    fallidas = json.load(open(fallidas_f, encoding="utf8")) if os.path.exists(fallidas_f) else {"_nota": "Parejas de mezcla que fallaron en el banco (tools/aplicar_validacion.py); tools/mezclas_coop.py no las vuelve a sacar.", "fallidas": []}
    for carpeta in ("acts", "coop"):
        for acto in (1, 2, 3):
            ruta = os.path.join(ROOT, "data", carpeta, f"{acto}.json")
            d = json.load(open(ruta, encoding="utf8"))
            antes = len(d["encounters"])
            quedan = []
            for e in d["encounters"]:
                r = malos.get(e["id"])
                if r is None:
                    quedan.append(e)
                    continue
                print(f"  fuera {carpeta} {acto} {e['id']} ({e['name']}): {motivo(r)}")
                if "mezcla" in e.get("tags", []):
                    fallidas["fallidas"].append(e["id"])
                else:
                    c = e.get("chr") or (e.get("enemigos") or [{}])[0].get("chr")
                    fuera["fuera"][f"c{int(c):04d}"] = motivo(r)
            d["encounters"] = quedan
            if aplicar and len(quedan) != antes:
                open(ruta, "w", encoding="utf8").write(json.dumps(d, ensure_ascii=False, indent=1) + "\n")
    if aplicar:
        json.dump(fuera, open(fuera_f, "w", encoding="utf8"), ensure_ascii=False, indent=1)
        json.dump(fallidas, open(fallidas_f, "w", encoding="utf8"), ensure_ascii=False, indent=1)
        print("aplicado")


if __name__ == "__main__":
    main()
