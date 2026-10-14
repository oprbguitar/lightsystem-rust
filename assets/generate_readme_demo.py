"""Create the animated MiniPOS workflow illustration used in README.md.

Optional authoring dependency: Pillow (python -m pip install Pillow).
"""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parent
OUT = ROOT / "minipos-flujo.gif"
W, H = 960, 570
BG = (7, 10, 9)
GREEN = (37, 230, 76)
DIM = (17, 110, 46)
CYAN = (76, 220, 224)
WHITE = (230, 245, 230)
BLUE = (55, 111, 244)
YELLOW = (255, 211, 60)

font_path = Path("C:/Windows/Fonts/consola.ttf")
if not font_path.exists():
    font_path = Path("C:/Windows/Fonts/cour.ttf")
FONT = ImageFont.truetype(str(font_path), 18)
SMALL = ImageFont.truetype(str(font_path), 15)
TITLE = ImageFont.truetype(str(font_path), 22)


def line(draw, xy, text, fill=GREEN, font=FONT):
    draw.text(xy, text, fill=fill, font=font)


def base(step, title, hint):
    image = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(image)
    draw.rectangle((34, 28, W - 34, 68), fill=BLUE)
    line(draw, (52, 36), "[ MINIPOS ]  TIENDA DEMO  |  CAJA01  |  " + title, WHITE, TITLE)
    draw.rectangle((34, 84, W - 34, 492), outline=DIM, width=2)
    line(draw, (54, 510), hint, YELLOW, SMALL)
    draw.rectangle((54, 540, W - 54, 550), outline=DIM, width=1)
    draw.rectangle((54, 540, 54 + int((W - 108) * step / 4), 550), fill=GREEN)
    line(draw, (W - 185, 510), f"PASO {step}/4", CYAN, SMALL)
    return image, draw


def make_frames():
    frames = []

    image, draw = base(1, "CAJA · BÚSQUEDA", "Escribe para filtrar · ↑/↓ elige · ENTER agrega")
    line(draw, (56, 101), "PRODUCTOS DEMO · busca por código, nombre, marca o categoría", CYAN)
    line(draw, (56, 135), "BUSCAR > teclado", WHITE)
    line(draw, (56, 171), "> DEMO-014  Teclado mecánico RGB     STOCK 18    S/ 219.90", YELLOW)
    line(draw, (56, 205), "  DEMO-027  Mouse óptico            STOCK 24    S/  49.90")
    line(draw, (56, 239), "  DEMO-083  Audífonos USB           STOCK 12    S/ 129.00")
    line(draw, (56, 273), "  DEMO-091  Monitor 24 pulgadas     STOCK  8    S/ 699.00")
    line(draw, (56, 325), "CARRITO · 0 productos", CYAN)
    line(draw, (56, 393), "BASE S/ 0.00  ·  IGV DEMO S/ 0.00  ·  TOTAL S/ 0.00", GREEN)
    frames.append(image)

    image, draw = base(2, "CAJA · CARRITO", "Agrega más de un producto · TAB selecciona líneas")
    draw.line((492, 104, 492, 470), fill=DIM, width=2)
    line(draw, (56, 105), "CATÁLOGO · 1,040 DEMO", CYAN)
    line(draw, (56, 151), "> DEMO-014  Teclado RGB", YELLOW)
    line(draw, (56, 187), "  DEMO-027  Mouse óptico")
    line(draw, (56, 223), "  DEMO-083  Audífonos USB")
    line(draw, (56, 280), " ↑/↓ selecciona · ENTER agrega", GREEN, SMALL)
    line(draw, (520, 105), "VENTA · 3 LÍNEAS", CYAN)
    line(draw, (520, 151), "> Teclado mecánico RGB ×1   S/ 219.90", WHITE, SMALL)
    line(draw, (520, 183), "  Mouse óptico ×2           S/  99.80", WHITE, SMALL)
    line(draw, (520, 215), "  Audífonos USB ×1           S/ 129.00", WHITE, SMALL)
    line(draw, (520, 286), "TAB · F9/F10 cantidad · SUPR quitar", GREEN, SMALL)
    line(draw, (56, 408), "BASE S/ 380.25  ·  IGV DEMO S/ 68.45  ·  TOTAL S/ 448.70", GREEN)
    frames.append(image)

    image, draw = base(3, "COBRO · DATOS", "F8 abre el cobro · la segunda pulsación confirma la demo")
    line(draw, (56, 110), "COBRO · SIMULACIÓN LOCAL · SIN CONEXIÓN SUNAT", CYAN)
    line(draw, (74, 159), "TIPO DE DOCUMENTO:  BOLETA DEMO", WHITE)
    line(draw, (74, 198), "CLIENTE:              DNI  12345678", WHITE)
    line(draw, (74, 237), "NOMBRE:               CLIENTE DE PRUEBA", WHITE)
    line(draw, (74, 276), "PAGO:                 EFECTIVO", WHITE)
    line(draw, (74, 315), "RECIBIDO PEN:         500.00", WHITE)
    line(draw, (74, 354), "TIPO DE CAMBIO:       S/ 3.448 por USD", WHITE)
    line(draw, (74, 418), "TOTAL S/ 448.70   ·   VUELTO S/ 51.30", GREEN)
    frames.append(image)

    image, draw = base(4, "COMPROBANTE · PDF", "PDF imprimible guardado en datos/caja01/comprobantes")
    draw.rectangle((285, 103, 675, 468), outline=CYAN, width=2)
    line(draw, (325, 121), "[ MINIPOS ]", YELLOW)
    line(draw, (325, 154), "TIENDA DEMO · CAJA01", WHITE, SMALL)
    line(draw, (325, 190), "BOLETA DEMO · 13/10/2026 14:30", CYAN, SMALL)
    line(draw, (325, 223), "Teclado mecánico RGB   ×1", WHITE, SMALL)
    line(draw, (325, 251), "Mouse óptico           ×2", WHITE, SMALL)
    line(draw, (325, 279), "Audífonos USB          ×1", WHITE, SMALL)
    draw.line((325, 311, 635, 311), fill=DIM, width=1)
    line(draw, (325, 329), "BASE                  S/ 380.25", WHITE, SMALL)
    line(draw, (325, 357), "IGV DEMO              S/  68.45", WHITE, SMALL)
    line(draw, (325, 389), "TOTAL                 S/ 448.70", GREEN)
    line(draw, (325, 430), "SIN VALIDEZ TRIBUTARIA", YELLOW, SMALL)
    frames.append(image)
    return frames


frames = make_frames()
frames[0].save(ROOT / "minipos-flujo.png", optimize=True)
frames[0].save(
    OUT,
    save_all=True,
    append_images=frames[1:],
    duration=[1050, 1050, 1050, 1500],
    loop=0,
    optimize=True,
    disposal=2,
)
print(OUT)
