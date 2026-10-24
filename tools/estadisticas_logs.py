"""Estadísticas de las partidas a partir de los registros (runtime/harness/harness.log de cada jugador).

Uso: python tools/estadisticas_logs.py <harness.log> [más logs...]

Saca:
- el daño de cada fuente (arma, estados, cada bendición) en todos los combates: parte del total, en
  cuántos combates sale, media por combate y la mayor parte que se ha llevado de uno;
- los combates en que una sola bendición hace más de la mitad del daño (candidatas a estar pasadas);
- los «FALLO ANOTADO» (opción de la pausa) con su contexto, para revisarlos uno a uno.

El 17-10, con los logs del jugador y su amigo (52 combates), salió Doble llama con el 20 % de todo el daño.
"""
import ast
import collections
import re
import sys

PATRON = re.compile(r"combate: (.+?) en (\d+) s; daño (\[.*\])")


def main(logs):
    total = collections.Counter()
    peleas = collections.Counter()
    maxparte = {}
    dominadas = []
    fallos = []
    n = 0
    for f in logs:
        lineas = open(f, encoding="utf8", errors="ignore").read().splitlines()
        for i, l in enumerate(lineas):
            if "=== FALLO ANOTADO" in l:
                bloque = [l]
                for x in lineas[i + 1:i + 8]:
                    if "=== FALLO" in x or not any(k in x for k in ("run:", "bendiciones", "rival:", "jugador:", "cooperativo:")):
                        break
                    bloque.append(x)
                fallos.append((f, bloque))
            m = PATRON.search(l)
            if not m:
                continue
            try:
                d = ast.literal_eval(m.group(3))
            except (ValueError, SyntaxError):
                continue
            suma = sum(v for _, v in d) or 1
            n += 1
            for k, v in d:
                total[k] += v
                peleas[k] += 1
                maxparte[k] = max(maxparte.get(k, 0), v / suma)
                if k not in ("arma", "estados") and v / suma > 0.5 and suma > 1000:
                    dominadas.append((m.group(1), m.group(2), k, v / suma))
    if not n:
        print("sin combates en esos registros")
    else:
        todo = sum(total.values())
        print(f"{n} combates\n")
        print(f"{'fuente':24s} {'del total':>9s} {'combates':>9s} {'media':>8s} {'máx. parte':>10s}")
        for k, v in total.most_common():
            print(f"{k:24s} {v / todo * 100:8.1f}% {peleas[k]:9d} {v / peleas[k]:8.0f} {maxparte[k] * 100:9.0f}%")
        if dominadas:
            print("\nCombates con una bendición por encima de la mitad del daño:")
            for nombre, s, k, p in dominadas:
                print(f"  {nombre} ({s} s): {k} {p * 100:.0f}%")
    if fallos:
        print(f"\n{len(fallos)} fallos anotados:")
        for f, bloque in fallos:
            print(f"--- {f}")
            for x in bloque:
                print("  " + x)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    main(sys.argv[1:])
