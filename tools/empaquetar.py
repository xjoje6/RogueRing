"""Empaqueta RogueRing para publicarlo: dist/RogueRing/ y dist/RogueRing-<versión>.zip.

Incluye solo lo que el mod usa en el juego: paquete de me3 (mod/), la DLL de jugador, los datos
(data/) y las imágenes de interfaz (extract/ui/, solo los iconos y artes referenciados). Después
comprueba el paquete y falla si algo no cuadra: falta una imagen, la DLL lleva el banco de pruebas,
sobra algún fichero o hay datos personales.

Uso: python tools/empaquetar.py [versión] [--privada] [--seamless=<carpeta SeamlessCoop>]

--privada: build de pruebas para pasarla a mano. Lleva LEEME.txt (docs/publicar/PRUEBAS.txt, cómo
instalarla) y no exige LICENSE, porque no se publica.
--seamless: solo con --privada. Mete Seamless Co-op en el paquete (para descargar y jugar con amigos
sin instalar nada más). Su autor solo permite distribuirlo desde su página: nunca en una build pública.
Va su ersc.dll con sus textos y un INI con la contraseña vacía (la pide el lanzador la primera vez).
"""
import glob
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import zipfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DIST = os.path.join(ROOT, "dist")
OUT = os.path.join(DIST, "RogueRing")
UI = os.path.join(ROOT, "extract", "ui")
VERSION = next((a for a in sys.argv[1:] if not a.startswith("--")), "0.1.0-prealpha")
PRIVADA = "--privada" in sys.argv
SEAMLESS = next((a.split("=", 1)[1] for a in sys.argv[1:] if a.startswith("--seamless=")), None)
if SEAMLESS and not PRIVADA:
    sys.exit("--seamless solo con --privada: Seamless Co-op no se puede publicar dentro del mod")

PERFIL_ME3 = '''profileVersion = "v1"
savefile = "RogueRing.sl2"

[[supports]]
game = "eldenring"

[[packages]]
path = "mod"

[[natives]]
path = "runtime/rogue_harness.dll"
'''

LANZADOR = (
    '@echo off\r\ncd /d "%~dp0"\r\n'
    'where me3 >nul 2>nul || (echo me3 was not found. Install it from https://github.com/garyttierney/me3/releases & pause & exit /b 1)\r\n'
    'me3 launch --profile "%~dp0RogueRing.me3"\r\n'
)

# Cooperativo (docs/COOP_SALAS.md): el mismo perfil con Seamless Co-op delante. Seamless no viene con
# el mod: cada jugador pone su carpeta SeamlessCoop (2.0.1) junto al lanzador, con la misma contraseña.
PERFIL_SEAMLESS = PERFIL_ME3.replace(
    '[[natives]]\npath = "runtime/rogue_harness.dll"',
    '[[natives]]\npath = "SeamlessCoop/ersc.dll"\n\n[[natives]]\npath = "runtime/rogue_harness.dll"',
)

LANZADOR_SEAMLESS = (
    '@echo off\r\ncd /d "%~dp0"\r\n'
    'where me3 >nul 2>nul || (echo me3 was not found. Install it from https://github.com/garyttierney/me3/releases & pause & exit /b 1)\r\n'
    # runtime\seamless.ps1 (tools/seamless.ps1): contraseña de la sesión (se pide una vez), partida aparte
    # (rogue; con co2 sería tu partida de Seamless), sin invasores y escalado de Seamless a cero (lo pone el mod).
    'powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0runtime\\seamless.ps1" || (pause & exit /b 1)\r\n'
    'me3 launch --profile "%~dp0RogueRing (Seamless).me3"\r\n'
)

# Lo único que puede haber en la raíz del paquete.
RAIZ = {"mod", "runtime", "data", "extract", "RogueRing.me3", "Play RogueRing.bat", "RogueRing (Seamless).me3",
        "Play RogueRing (Seamless).bat", "README.md", "LICENSE", "CREDITS.md"}
if PRIVADA:
    RAIZ.add("LEEME.txt")
if SEAMLESS:
    RAIZ.add("SeamlessCoop")


def copy(src, dst):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copy2(src, dst)


def codigo():
    for f in glob.glob(os.path.join(ROOT, "dll", "harness", "src", "**", "*.rs"), recursive=True):
        yield open(f, encoding="utf8").read()


def iconos_usados():
    """Todo "icono" de data/ y los iconos fijos del código (constantes ICON_ e `icono: N`)."""
    ids = set()
    for f in glob.glob(os.path.join(ROOT, "data", "**", "*.json"), recursive=True):
        ids |= {int(m) for m in re.findall(r'"icono":\s*(\d+)', open(f, encoding="utf8").read())}
    for s in codigo():
        for a, b in re.findall(r"ICON_\w+: i32 = ([\d_]+)|icono: ([1-9]\d*)", s):
            ids.add(int((a or b).replace("_", "")))
    return ids - {0}


def artes_usados(eventos):
    """Artes de los eventos y de las pantallas del mod (MENU_Load_* en el código)."""
    nombres = {e["imagen"] for e in eventos}
    for s in codigo():
        nombres |= set(re.findall(r"MENU_Load_\d+(?:_PS5)?", s))
    return nombres


def construir():
    if os.path.exists(OUT):
        shutil.rmtree(OUT)
    os.makedirs(OUT)
    falta = []
    # Paquete de me3 (sin la receta de caché).
    for f in glob.glob(os.path.join(ROOT, "mod", "**", "*"), recursive=True):
        if os.path.isfile(f) and not f.endswith(".receta"):
            copy(f, os.path.join(OUT, os.path.relpath(f, ROOT)))
    # El coliseo lo cambia el director antes de cada combate: el paquete lleva el normal.
    copy(os.path.join(ROOT, "mod", "map", "rogue_coliseo", "base.msb.dcx"), os.path.join(OUT, "mod", "map", "mapstudio", "m45_00_00_00.msb.dcx"))
    # DLL de jugador: sin banco de pruebas. Se compila aparte para no pisar la de pruebas.
    dll = os.path.join(ROOT, "dll")
    subprocess.run(["cargo", "build", "--release", "--no-default-features", "-p", "rogue-harness",
                    "--target-dir", os.path.join("target", "jugador")], cwd=dll, check=True)
    copy(os.path.join(dll, "target", "jugador", "release", "rogue_harness.dll"), os.path.join(OUT, "runtime", "rogue_harness.dll"))
    os.makedirs(os.path.join(OUT, "runtime", "harness"), exist_ok=True)
    # Datos (las arenas de los jefes y las de prueba solo las usa tools/ModBuilder; las grandes, también
    # la DLL).
    for f in glob.glob(os.path.join(ROOT, "data", "**", "*.json"), recursive=True):
        rel = os.path.relpath(f, ROOT)
        if os.path.basename(rel) not in ("arenas_jefes.json", "arenas_prueba.json"):
            copy(f, os.path.join(OUT, rel))
    # Versión de pruebas (paquetes privados): menú de pruebas en la pausa del mapa (director/pruebas.rs).
    if PRIVADA:
        open(os.path.join(OUT, "data", "modo_pruebas.txt"), "w", encoding="utf8").write("Versión de pruebas: menú de pruebas en la pausa del mapa.\n")
        # Todo desbloqueado (personajes, armas, apariencias) para revisarlo sin ganar runs (23-10).
        open(os.path.join(OUT, "data", "desbloqueado.txt"), "w", encoding="utf8").write("Versión de pruebas: todo desbloqueado.\n")
    # Jefes de Nightreign (data/nightreign.json): sin sus archivos en mod/ (ModBuilder con extract_nr) el
    # combate saldría sin jefe.
    nr = os.path.join(ROOT, "data", "nightreign.json")
    if os.path.exists(nr):
        for j in json.load(open(nr, encoding="utf8"))["jefes"]:
            if not os.path.exists(os.path.join(ROOT, "mod", "chr", f"c{j['chr']:04d}.chrbnd.dcx")):
                falta.append(f"jefe de Nightreign c{j['chr']:04d} ({j['nombre']}): Extractor sobre Nightreign y ModBuilder")
    # Partida plantilla (tools/plantilla_partida.py): la DLL la instala a quien no tenga partida del mod.
    plantilla = os.path.join(ROOT, "data", "partida", "plantilla.sl2")
    if os.path.exists(plantilla):
        copy(plantilla, os.path.join(OUT, "data", "partida", "plantilla.sl2"))
    else:
        falta.append("data/partida/plantilla.sl2 (python tools/plantilla_partida.py <copia.sl2> <ranura>)")
    # Interfaz: fondo, piezas de menú (con el logo), iconos del mapa, el tramo de mapa de cada acto
    # (tools/map_background.py), iconos usados y artes.
    copy(os.path.join(UI, "mapa_L2_gris.png"), os.path.join(OUT, "extract", "ui", "mapa_L2_gris.png"))
    for n in (1, 2, 3):
        if not os.path.exists(os.path.join(UI, "fondos", f"acto{n}.png")):
            falta.append(f"extract/ui/fondos/acto{n}.png (Extractor maptile y tools/map_background.py)")
    if not os.path.exists(os.path.join(UI, "menu", "logo.png")):
        falta.append("extract/ui/menu/logo.png (tools/recursos_menu.py con runtime/logo)")
    # Letras libres de la capa (OFL) con sus licencias.
    for f in glob.glob(os.path.join(ROOT, "data", "fuentes", "*")):
        copy(f, os.path.join(OUT, "data", "fuentes", os.path.basename(f)))
    for sub in ("menu", "mapa", "fondos"):
        for f in glob.glob(os.path.join(UI, sub, "*.png")):
            if not os.path.basename(f).startswith("_"):
                copy(f, os.path.join(OUT, "extract", "ui", sub, os.path.basename(f)))
    for i in sorted(iconos_usados()):
        f = os.path.join(UI, "iconos", f"{i}.png")
        if os.path.exists(f):
            copy(f, os.path.join(OUT, "extract", "ui", "iconos", f"{i}.png"))
        else:
            falta.append(f"icono {i} (Extractor ui y tools/imagenes.py)")
    eventos = json.load(open(os.path.join(ROOT, "data", "eventos.json"), encoding="utf8"))["eventos"]
    for imagen in sorted(artes_usados(eventos)):
        f = os.path.join(UI, "carga", imagen + ".png")
        if os.path.exists(f):
            copy(f, os.path.join(OUT, "extract", "ui", "carga", imagen + ".png"))
        else:
            falta.append(f"arte {imagen}")
    # Perfil de me3, lanzador, léame, licencia y créditos.
    open(os.path.join(OUT, "RogueRing.me3"), "w", encoding="utf8").write(PERFIL_ME3)
    open(os.path.join(OUT, "Play RogueRing.bat"), "w", encoding="utf8").write(LANZADOR)
    open(os.path.join(OUT, "RogueRing (Seamless).me3"), "w", encoding="utf8").write(PERFIL_SEAMLESS)
    open(os.path.join(OUT, "Play RogueRing (Seamless).bat"), "w", encoding="utf8").write(LANZADOR_SEAMLESS)
    copy(os.path.join(ROOT, "tools", "seamless.ps1"), os.path.join(OUT, "runtime", "seamless.ps1"))
    if SEAMLESS:
        meter_seamless(SEAMLESS, falta)
    for src, dst in (("docs/publicar/README.md", "README.md"), ("LICENSE", "LICENSE"), ("CREDITS.md", "CREDITS.md")):
        if os.path.exists(os.path.join(ROOT, src)):
            copy(os.path.join(ROOT, src), os.path.join(OUT, dst))
        elif not (PRIVADA and src == "LICENSE"):
            falta.append(src)
    if PRIVADA:
        # Con saltos de Windows para que el Bloc de notas lo abra bien en cualquier versión.
        leeme = open(os.path.join(ROOT, "docs", "publicar", "PRUEBAS.txt"), encoding="utf8").read()
        open(os.path.join(OUT, "LEEME.txt"), "w", encoding="utf-8-sig", newline="\r\n").write(leeme.replace("{version}", VERSION))
    return falta


def meter_seamless(origen, falta):
    """Seamless Co-op dentro del paquete privado: su DLL, sus textos y su crashpad, y el INI sin
    contraseña (tools/seamless.ps1 la pide la primera vez y fija el resto)."""
    if not os.path.exists(os.path.join(origen, "ersc.dll")):
        falta.append(f"Seamless Co-op: no está {os.path.join(origen, 'ersc.dll')}")
        return
    destino = os.path.join(OUT, "SeamlessCoop")
    for f in glob.glob(os.path.join(origen, "**", "*"), recursive=True):
        if os.path.isfile(f) and os.path.basename(f) != "ersc_settings.ini":
            copy(f, os.path.join(destino, os.path.relpath(f, origen)))
    ini = open(os.path.join(origen, "ersc_settings.ini"), encoding="utf8", errors="ignore").read()
    # Al morir, mirar a los compañeros hasta que caigan todos (docs/COOP_PROTOCOLO.md).
    fijos = {"cooppassword": "", "save_file_extension": "rogue", "allow_invaders": "0", "death_debuffs": "0", "always_spectate_on_death": "0"}
    for k in ("enemy_health_scaling", "enemy_damage_scaling", "enemy_posture_scaling",
              "boss_health_scaling", "boss_damage_scaling", "boss_posture_scaling"):
        fijos[k] = "0"
    lineas = []
    for l in ini.splitlines():
        clave = l.split("=", 1)[0].strip()
        lineas.append(f"{clave} = {fijos[clave]}" if "=" in l and clave in fijos else l)
    open(os.path.join(destino, "ersc_settings.ini"), "w", encoding="utf8", newline="\r\n").write("\n".join(lineas) + "\n")


def steam_id_activo():
    """SteamID64 del usuario de Steam con sesión abierta (para buscarlo en el paquete)."""
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Valve\Steam\ActiveProcess") as k:
            cuenta = winreg.QueryValueEx(k, "ActiveUser")[0]
        return 76561197960265728 + cuenta if cuenta else None
    except OSError:
        return None


_OODLE = None


def contenido(f):
    """Bytes de un archivo tal como los lee el juego: los .dcx (Kraken) descomprimidos con el
    oo2core del Extractor. En los datos comprimidos una ruta no se ve y una palabra corta aparece por
    azar (19-10: «el jugador» en sfxbnd_commoneffects); se busca en lo de dentro."""
    global _OODLE
    b = open(f, "rb").read()
    if not (f.endswith(".dcx") and b[:4] == b"DCX\0" and b[0x28:0x2C] == b"KRAK"):
        return b
    if _OODLE is None:
        import ctypes
        dll = os.path.join(ROOT, "tools", "Extractor", "bin", "Release", "net10.0", "oo2core_6_win64.dll")
        _OODLE = ctypes.WinDLL(dll) if os.path.exists(dll) else False
    if not _OODLE:
        raise SystemExit("falta oo2core_6_win64.dll (compila tools/Extractor) para comprobar los .dcx")
    import ctypes
    tam = struct.unpack(">I", b[0x1C:0x20])[0]
    comp = struct.unpack(">I", b[0x20:0x24])[0]
    src = b[0x4C:0x4C + comp]
    dst = ctypes.create_string_buffer(tam)
    n = _OODLE.OodleLZ_Decompress(src, len(src), dst, tam, 0, 0, 0, None, 0, None, None, None, 0, 3)
    if n != tam:
        raise SystemExit(f"no se pudo descomprimir {f}")
    return dst.raw


def comprobar():
    problemas = []
    sobra = set(os.listdir(OUT)) - RAIZ
    if sobra:
        problemas.append(f"sobra en la raíz: {sorted(sobra)}")
    rt = sorted(os.path.relpath(f, OUT) for f in glob.glob(os.path.join(OUT, "runtime", "**", "*"), recursive=True) if os.path.isfile(f))
    if rt != [os.path.join("runtime", "rogue_harness.dll"), os.path.join("runtime", "seamless.ps1")]:
        problemas.append(f"runtime/ debe llevar solo la DLL y seamless.ps1: {rt}")
    # Un archivo vacío es un mod a medias (16-10: ModBuilder cortado dejó c0000.anibnd.dcx en 0 bytes).
    for f in glob.glob(os.path.join(OUT, "**", "*"), recursive=True):
        if os.path.isfile(f) and os.path.getsize(f) == 0:
            problemas.append(f"{os.path.relpath(f, OUT)} está vacío")
    # Seamless en el paquete: nunca con una contraseña puesta.
    ini = os.path.join(OUT, "SeamlessCoop", "ersc_settings.ini")
    if os.path.exists(ini):
        for l in open(ini, encoding="utf8", errors="ignore").read().splitlines():
            if l.strip().startswith("cooppassword") and l.split("=", 1)[1].strip():
                problemas.append("SeamlessCoop/ersc_settings.ini lleva una contraseña")
    dll = open(os.path.join(OUT, "runtime", "rogue_harness.dll"), "rb").read()
    for marca in (b"cmd.txt", b"out.txt", b"orden #"):
        if marca in dll:
            problemas.append(f"la DLL lleva el banco de pruebas ({marca.decode()})")
    # Datos personales: rutas de usuario, el SteamID de quien empaqueta y su nombre de usuario.
    usuario = os.environ.get("USERNAME", "")
    sid = steam_id_activo()
    marcas = [b"C:\\Users\\", b"C:/Users/"]
    if sid:
        marcas += [str(sid).encode(), struct.pack("<Q", sid)]
    if len(usuario) > 2:
        marcas += [usuario.encode(), usuario.encode("utf-16-le")]
    for f in glob.glob(os.path.join(OUT, "**", "*"), recursive=True):
        if not os.path.isfile(f) or f.endswith(".dll"):
            continue
        b = contenido(f)
        # Los binarios de Seamless van tal cual los publica su autor y llevan sus rutas de compilación
        # (C:\Users\Luke\crashpad...): en ellos solo se busca lo de quien empaqueta.
        de_seamless = os.path.relpath(f, OUT).startswith("SeamlessCoop") and not f.endswith(".ini")
        for m in marcas[2:] if de_seamless else marcas:
            if m in b:
                problemas.append(f"{os.path.relpath(f, OUT)}: contiene {m!r}")
    return problemas


def main():
    falta = construir()
    problemas = [f"falta {x}" for x in falta] + comprobar()
    if problemas:
        sys.exit("paquete NO válido:\n  " + "\n  ".join(problemas))
    zpath = os.path.join(DIST, f"RogueRing-{VERSION}.zip")
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED) as z:
        for f in glob.glob(os.path.join(OUT, "**", "*"), recursive=True):
            z.write(f, os.path.join("RogueRing", os.path.relpath(f, OUT)))
    n = sum(1 for f in glob.glob(os.path.join(OUT, "**", "*"), recursive=True) if os.path.isfile(f))
    print(f"{zpath}: {n} ficheros, {os.path.getsize(zpath) / 1048576:.0f} MB. Paquete comprobado.")


if __name__ == "__main__":
    main()
