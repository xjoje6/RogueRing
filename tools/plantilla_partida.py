"""Partida plantilla del mod: un solo personaje, sin datos personales, para quien no tenga partida.

Parte de una COPIA de un guardado (nunca del original) y deja solo la ranura indicada:
- las otras nueve, vacías y desactivadas (bandera y resumen a cero);
- el personaje se llama «Tarnished»;
- fuera las contraseñas de grupo y de multijugador de la cabecera;
- el SteamID, en todas partes, pasa a ser el marcador (la DLL pone el del jugador al instalarla);
- MD5 de cada bloque recalculado.

Formato (BND4 de 12 bloques: USER_DATA000-009 son las ranuras, 010 la cabecera, 011 el reglamento).
Cada bloque empieza con el MD5 del resto. Cabecera: SteamID en 0x4, banderas de ranura activa en
0x1954 (10 bytes), resúmenes en 0x195E (cada 0x24C: nombre UTF-16 de 0x22 bytes, nivel, segundos),
contraseñas en 0x1464 (7 campos de 0x12).

Uso: python tools/plantilla_partida.py <copia.sl2> <ranura> [salida]
     (salida por defecto: data/partida/plantilla.sl2, fuera de git)
"""
import hashlib
import os
import struct
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
MARCADOR = struct.pack("<Q", 76561197960265728)  # SteamID «cero» (cuenta 0); la DLL lo cambia
NOMBRE = "Tarnished"
FLAGS, RESUMEN, PASO_RESUMEN, CLAVES = 0x1954, 0x195E, 0x24C, 0x1464


def bloques(b):
    n = struct.unpack_from("<I", b, 0x0C)[0]
    out = []
    for i in range(n):
        e = 0x40 + i * 0x20
        size = struct.unpack_from("<Q", b, e + 8)[0]
        doff = struct.unpack_from("<I", b, e + 0x10)[0]
        out.append((doff, size))
    return out


def nombre_utf16(texto):
    v = texto.encode("utf-16le")
    assert len(v) <= 0x20
    return v + b"\x00" * (0x22 - len(v))


def main():
    src, ranura = sys.argv[1], int(sys.argv[2])
    dst = sys.argv[3] if len(sys.argv) > 3 else os.path.join(ROOT, "data", "partida", "plantilla.sl2")
    b = bytearray(open(src, "rb").read())
    bl = bloques(b)
    assert len(bl) == 12 and b[:4] == b"BND4", "no es un guardado de Elden Ring"
    cab_off = bl[10][0] + 16
    steamid = bytes(b[cab_off + 4: cab_off + 12])
    # Nombre de la ranura que se queda (para renombrarlo también dentro de la ranura).
    res = cab_off + RESUMEN + ranura * PASO_RESUMEN
    viejo = bytes(b[res: res + 0x22]).decode("utf-16le").split("\x00")[0]
    print(f"ranura {ranura}: «{viejo}» -> «{NOMBRE}»")
    for i in range(10):
        doff, size = bl[i]
        if i != ranura:
            b[doff + 16: doff + size] = bytes(size - 16)
            b[cab_off + FLAGS + i] = 0
            r = cab_off + RESUMEN + i * PASO_RESUMEN
            b[r: r + PASO_RESUMEN] = bytes(PASO_RESUMEN)
        else:
            datos = bytes(b[doff + 16: doff + size])
            pat = viejo.encode("utf-16le") + b"\x00\x00"
            n = 0
            pos = datos.find(pat)
            while pos >= 0:
                if all(x == 0 for x in datos[pos + len(pat): pos + 0x22]):
                    b[doff + 16 + pos: doff + 16 + pos + 0x22] = nombre_utf16(NOMBRE)
                    n += 1
                pos = datos.find(pat, pos + 1)
            print(f"  nombre cambiado en la ranura: {n} vez/veces")
    b[res: res + 0x22] = nombre_utf16(NOMBRE)
    b[cab_off + CLAVES: cab_off + CLAVES + 7 * 0x12] = bytes(7 * 0x12)
    # SteamID -> marcador, y MD5 de cada bloque.
    total = 0
    for doff, size in bl:
        datos = bytes(b[doff + 16: doff + size])
        k = datos.count(steamid)
        total += k
        datos = datos.replace(steamid, MARCADOR)
        b[doff + 16: doff + size] = datos
        b[doff: doff + 16] = hashlib.md5(datos).digest()
    print(f"SteamID sustituido {total} veces")
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    open(dst, "wb").write(b)
    print(f"plantilla en {dst} ({len(b) / 1048576:.1f} MB)")


if __name__ == "__main__":
    main()
