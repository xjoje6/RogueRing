"""Audio de un banco Wwise de Nightreign a PCM, para que suene en Elden Ring.

Uso: python tools/bnk_pcm.py <banco.bnk> <salida.bnk> <vgmstream-cli.exe>

El Vorbis de los bancos nuevos de Nightreign va con otros libros de códigos (sello eeeb161c y
c9012706; ER usa 30d92159 y e6e0c53f) y el Wwise de ER no los decodifica: el jefe se queda mudo
aunque el banco cargue (18-10: el Fundidor sin un sonido; con el banco del dragón de ER, sí
sonaba). Cambiar el sello no basta. vgmstream decodifica cada audio y se guarda en PCM de 16 bits
(códec de Wwise 0x00010001, el formato base de su motor). Los audios que ya son de ER (sello de ER)
no se tocan.
"""
import os
import struct
import subprocess
import sys
import tempfile
import wave

VORBIS = 0x00040001
PCM = 0x00010001
SELLOS_ER = {bytes.fromhex("30d92159"), bytes.fromhex("e6e0c53f")}


def secciones(b):
    r, i = [], 0
    while i + 8 <= len(b):
        n = struct.unpack_from("<I", b, i + 4)[0]
        r.append([bytes(b[i:i + 4]), bytes(b[i + 8:i + 8 + n])])
        i += 8 + n
    return r


def sello(wem):
    """Sello de los libros de códigos del Vorbis de Wwise (None si no es Vorbis)."""
    j = 12
    while j + 8 <= len(wem):
        t, m = wem[j:j + 4], struct.unpack_from("<I", wem, j + 4)[0]
        if t == b"fmt ":
            if struct.unpack_from("<H", wem, j + 8)[0] != 0xFFFF:
                return None
            return wem[j + 8 + m - 6:j + 8 + m - 2]
        j += 8 + m + (m & 1)
    return None


def wem_pcm(canales, hz, muestras):
    """WEM de PCM como los de Wwise: WAVEFORMATEXTENSIBLE corto (0x18) y los datos."""
    mascara = {1: 0x4, 2: 0x3, 4: 0x33, 6: 0x3F}.get(canales, 0)
    bloque = canales * 2
    fmt = struct.pack("<HHIIHHHHI", 0xFFFE, canales, hz, hz * bloque, bloque, 16, 6, 16, mascara)
    cuerpo = b"WAVE" + b"fmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(muestras)) + muestras
    if len(muestras) & 1:
        cuerpo += b"\0"
    return b"RIFF" + struct.pack("<I", len(cuerpo)) + cuerpo


def main(entrada, salida, vgm):
    b = open(entrada, "rb").read()
    secs = secciones(b)
    sec = {t: d for t, d in secs}
    didx = [struct.unpack_from("<III", sec[b"DIDX"], k * 12) for k in range(len(sec[b"DIDX"]) // 12)]
    data = sec[b"DATA"]
    nuevos, convertidos = {}, set()
    with tempfile.TemporaryDirectory() as tmp:
        for sid, off, ln in didx:
            wem = data[off:off + ln]
            s = sello(wem)
            if s is None or s in SELLOS_ER:
                nuevos[sid] = wem
                continue
            fw, fo = os.path.join(tmp, f"{sid}.wem"), os.path.join(tmp, f"{sid}.wav")
            open(fw, "wb").write(wem)
            r = subprocess.run([vgm, "-i", "-o", fo, fw], capture_output=True, text=True)
            if r.returncode != 0 or not os.path.exists(fo):
                print(f"no se decodifica {sid}: {r.stderr.strip()[:120]}")
                nuevos[sid] = wem
                continue
            with wave.open(fo) as w:
                nuevos[sid] = wem_pcm(w.getnchannels(), w.getframerate(), w.readframes(w.getnframes()))
            convertidos.add(sid)
    # DATA y DIDX nuevos, alineados a 16 como los de Wwise.
    datos, indice = bytearray(), bytearray()
    for sid, _, _ in didx:
        while len(datos) % 16:
            datos.append(0)
        indice += struct.pack("<III", sid, len(datos), len(nuevos[sid]))
        datos += nuevos[sid]
    # Sonidos (HIRC tipo 2): códec PCM y tamaño en memoria del audio nuevo.
    h = bytearray(sec[b"HIRC"])
    c, j, cambiados = struct.unpack_from("<I", h, 0)[0], 4, 0
    for _ in range(c):
        ty, ln = h[j], struct.unpack_from("<I", h, j + 1)[0]
        if ty == 2:
            plugin, sid = struct.unpack_from("<I", h, j + 9)[0], struct.unpack_from("<I", h, j + 14)[0]
            if plugin == VORBIS and sid in convertidos:
                struct.pack_into("<I", h, j + 9, PCM)
                struct.pack_into("<I", h, j + 18, len(nuevos[sid]))
                cambiados += 1
        j += 5 + ln
    out = bytearray()
    for t, d in secs:
        d = {b"DIDX": bytes(indice), b"DATA": bytes(datos), b"HIRC": bytes(h)}.get(t, d)
        out += t + struct.pack("<I", len(d)) + d
    open(salida, "wb").write(out)
    print(f"{os.path.basename(entrada)}: {len(convertidos)} audios a PCM, {cambiados} sonidos, {len(b) // 1024} KB -> {len(out) // 1024} KB")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    main(*sys.argv[1:])
