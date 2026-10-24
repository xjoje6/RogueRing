"""Pasa a PNG las imágenes que saca `Extractor ui` (fuera de git, son del juego):
- iconos de objeto (extract/ui/iconos_dds, 1024 px) a extract/ui/iconos/<id>.png de 256 px;
- pantallas de carga (extract/ui/carga_dds, 4K) a extract/ui/carga/<nombre>.png de 1280x720.
"""
import glob
import os

from PIL import Image

UI = os.path.join(os.path.abspath(os.path.join(os.path.dirname(__file__), "..")), "extract", "ui")


def pasar(origen, destino, nombre, tam, modo):
    os.makedirs(os.path.join(UI, destino), exist_ok=True)
    n = 0
    for f in glob.glob(os.path.join(UI, origen, "*.dds")):
        Image.open(f).convert(modo).resize(tam, Image.LANCZOS).save(os.path.join(UI, destino, nombre(f) + ".png"))
        n += 1
    print(f"{n} imágenes en extract/ui/{destino}")


pasar("iconos_dds", "iconos", lambda f: str(int(os.path.basename(f).split("_")[-1].split(".")[0])), (256, 256), "RGBA")
pasar("carga_dds", "carga", lambda f: os.path.basename(f)[:-4], (1280, 720), "RGB")
