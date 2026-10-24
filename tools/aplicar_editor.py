"""Aplica los cambios del editor de combates (revision_enemigos/editor.html) a data/acts y data/coop.

Uso: python tools/aplicar_editor.py <cambios.json> [--aplicar]

<cambios.json> es lo que da «Descargar mis cambios» en editor_para_compartir.html ({autor, fecha, cambios,
propuestas}) o lo mismo sacado de la base de datos del editor en claude.ai. Sin --aplicar solo dice lo que
haría.
- Mover (acto o tipo): el encuentro pasa al acto y tipo nuevos. Una pareja generada que se mueve se queda
  como pareja fija (sin la etiqueta «mezcla»): tools/mezclas_coop.py ya no la toca.
- No mezclar: el enemigo va a `no_mezclar` de data/mezclas_fallidas.json y no entra en las parejas que
  sortea el cooperativo.
- Quitar: fuera de su lista.
  - Una pareja generada va a `quitadas` de data/mezclas_fallidas.json y tools/mezclas_coop.py no la vuelve
    a sacar.
  - Un enemigo que deja de salir en las listas va a data/enemigos_fuera.json con el motivo, y
    tools/ampliar_enemigos.py no lo vuelve a meter.
- Propuestas de enemigos sin usar: solo se enseñan; las evalúa Claude antes de meterlas.
Después (siempre que se quite, se mueva o se marque «no mezclar»): tools/mezclas_coop.py, tools/equilibrar_vida.py y el editor (tools/enemigos.py --sin-red y
tools/editor_combates.py).
"""
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
TIPOS = {"Weak": "débil", "Normal": "normal", "Elite": "élite", "Boss": "jefe"}


def cargar(lista, acto):
    return json.load(open(os.path.join(ROOT, "data", lista, f"{acto}.json"), encoding="utf8"))


def guardar(lista, acto, d):
    open(os.path.join(ROOT, "data", lista, f"{acto}.json"), "w", encoding="utf8").write(json.dumps(d, ensure_ascii=False, indent=1) + "\n")


def modelos(e):
    return {int(x["chr"]) for x in (e.get("enemigos") or [e]) if x.get("chr") is not None}


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not args:
        sys.exit(__doc__)
    aplicar = "--aplicar" in sys.argv
    entrada = json.load(open(args[0], encoding="utf8"))
    autor, fecha = entrada.get("autor") or "el jugador", entrada.get("fecha") or ""
    print(f"Cambios de {autor} ({fecha or 'sin fecha'}): {len(entrada.get('cambios', {}))} cambios, {len(entrada.get('propuestas', {}))} propuestas")
    datos = {(l, a): cargar(l, a) for l in ("acts", "coop") for a in (1, 2, 3)}
    mezclas_f = os.path.join(ROOT, "data", "mezclas_fallidas.json")
    mezclas = json.load(open(mezclas_f, encoding="utf8"))
    mezclas.setdefault("quitadas", [])
    fuera_f = os.path.join(ROOT, "data", "enemigos_fuera.json")
    fuera = json.load(open(fuera_f, encoding="utf8"))
    quitados = set()
    for c in entrada.get("cambios", {}).values():
        lista, eid = c["lista"], c["id"]
        donde = next(((l, a) for (l, a), d in datos.items() if l == lista and any(e["id"] == eid for e in d["encounters"])), None)
        if not donde:
            print(f"  {c.get('nombre', eid)}: ya no está en las listas (se cambiaron después); se salta")
            continue
        e = next(x for x in datos[donde]["encounters"] if x["id"] == eid)
        nombre = e["name"]
        if c.get("quitar"):
            datos[donde]["encounters"].remove(e)
            if "mezcla" in e.get("tags", []):
                mezclas["quitadas"].append(eid)
            quitados |= modelos(e)
            print(f"  quitar {nombre} ({lista}, acto {donde[1]})")
            continue
        # «No mezclar»: el enemigo no entra en las parejas que sortea el cooperativo (tools/mezclas_coop.py).
        no_mezclar = mezclas.setdefault("no_mezclar", [])
        chr_ = int(e["chr"]) if e.get("chr") is not None and not e.get("enemigos") else None
        if chr_ is not None and bool(c.get("no_mezclar")) != (chr_ in no_mezclar):
            if c.get("no_mezclar"):
                no_mezclar.append(chr_)
            else:
                no_mezclar.remove(chr_)
            print(f"  {nombre}: {'no se mezcla' if c.get('no_mezclar') else 'vuelve a mezclarse'} en cooperativo")
        acto, tipo = int(c["acto"]), c["tipo"]
        if (acto, tipo) == (donde[1], e["kind"]):
            continue
        e["kind"] = tipo
        if "mezcla" in e.get("tags", []):
            e["tags"] = [t for t in e["tags"] if t != "mezcla"]
        if acto != donde[1]:
            datos[donde]["encounters"].remove(e)
            datos[(lista, acto)]["encounters"].append(e)
        print(f"  mover {nombre} ({lista}): acto {donde[1]} → {acto}, {TIPOS.get(tipo, tipo)}")
    # Modelos que ya no salen en ninguna lista: fuera con el motivo.
    usados = set()
    for d in datos.values():
        for e in d["encounters"]:
            usados |= modelos(e)
    for c in sorted(quitados - usados):
        fuera["fuera"][f"c{c:04d}"] = f"Quitado en el editor por {autor}" + (f" ({fecha})" if fecha else "") + "."
        print(f"  c{c:04d} ya no sale en ninguna lista: a enemigos_fuera.json")
    for p in entrada.get("propuestas", {}).values():
        print(f"  propuesta: {p.get('nombre')} ({p.get('chr')}) a {p.get('lista')} acto {p.get('acto')} {TIPOS.get(p.get('tipo'), p.get('tipo'))}: {p.get('nota') or 'sin nota'}")
    if aplicar:
        for (l, a), d in datos.items():
            guardar(l, a, d)
        json.dump(mezclas, open(mezclas_f, "w", encoding="utf8"), ensure_ascii=False, indent=1)
        json.dump(fuera, open(fuera_f, "w", encoding="utf8"), ensure_ascii=False, indent=1)
        print("aplicado")


if __name__ == "__main__":
    main()
