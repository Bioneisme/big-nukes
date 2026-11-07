#!/usr/bin/env python3
"""Draws the mod's 16x16 textures: original pixel art from a fixed seed.

    python tools/make_textures.py

Writes content/assets/big_nukes/textures/{block,item}/*.png, gui/centrifuge.png, the hazmat suit's
entity/equipment/{humanoid,humanoid_leggings}/hazmat.png, mob_effect/radiation.png and the guide's
screen, gui/guide.png.
"""
import math
import random
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
TEX = ROOT / "content" / "assets" / "big_nukes" / "textures"
rng = random.Random(263)

CLEAR = (0, 0, 0, 0)


def shade(c, k):
    return tuple(max(0, min(255, int(v * k))) for v in c[:3]) + (255,)


def noisy(base, spread=0.12):
    im = Image.new("RGBA", (16, 16))
    for y in range(16):
        for x in range(16):
            im.putpixel((x, y), shade(base, 1 + rng.uniform(-spread, spread)))
    return im


def frame(im, edge):
    for i in range(16):
        for p in ((i, 0), (i, 15), (0, i), (15, i)):
            im.putpixel(p, edge)
    return im


def speckle(im, colours, count, size=2):
    for _ in range(count):
        x, y = rng.randrange(1, 15 - size + 1), rng.randrange(1, 15 - size + 1)
        c = rng.choice(colours)
        for dx in range(size):
            for dy in range(size):
                if rng.random() < 0.8:
                    im.putpixel((x + dx, y + dy), c)
    return im


def save(im, kind, name):
    path = TEX / kind / f"{name}.png"
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path)


STONE = (125, 125, 125)
DEEPSLATE = (77, 77, 82)
URANIUM = [(96, 214, 60, 255), (150, 240, 90, 255), (60, 160, 40, 255)]
STEEL = (110, 116, 126)
EDGE = (40, 42, 48, 255)


def trefoil(im, cx, cy, fg, bg=None):
    """A three-bladed hazard sign, about 8 pixels across."""
    for y in range(16):
        for x in range(16):
            dx, dy = x - cx + 0.5, y - cy + 0.5
            r = (dx * dx + dy * dy) ** 0.5
            import math
            a = (math.degrees(math.atan2(dy, dx)) + 360) % 120
            if r < 1.3 or (2.2 < r < 4.6 and 30 < a < 90):
                im.putpixel((x, y), fg)
            elif bg and r < 5.2:
                im.putpixel((x, y), bg)
    return im


def stripes(im, a, b, rows):
    for y in rows:
        for x in range(16):
            im.putpixel((x, y), a if (x + y) // 2 % 2 == 0 else b)
    return im


# ---- blocks ----
save(speckle(noisy(STONE), URANIUM, 5), "block", "uranium_ore")
save(speckle(noisy(DEEPSLATE), URANIUM, 5), "block", "deepslate_uranium_ore")
save(speckle(noisy((58, 56, 54), 0.18), [(90, 86, 82, 255), (34, 33, 32, 255)], 9, 1), "block", "ash")

for part in ("top", "side", "front"):
    im = frame(noisy(STEEL, 0.06), EDGE)
    if part == "front":
        for y in range(3, 13):
            for x in range(3, 13):
                d = ((x - 7.5) ** 2 + (y - 7.5) ** 2) ** 0.5
                if d < 5:
                    im.putpixel((x, y), (30, 34, 40, 255) if d > 4 else shade((70, 200, 60), 0.6 + 0.08 * (5 - d)))
    if part == "top":
        for i in range(2, 14):
            im.putpixel((i, 7), EDGE)
            im.putpixel((7, i), EDGE)
    save(im, "block", f"centrifuge_{part}")

table_top = frame(noisy((150, 152, 158), 0.05), EDGE)
for i in range(1, 15):
    for j in (5, 10):
        table_top.putpixel((i, j), (90, 94, 100, 255))
        table_top.putpixel((j, i), (90, 94, 100, 255))
save(table_top, "block", "assembly_table_top")
save(frame(noisy((70, 72, 78), 0.06), EDGE), "block", "assembly_table_bottom")
side = frame(noisy((120, 124, 130), 0.06), EDGE)
save(stripes(side, (220, 180, 30, 255), (30, 30, 30, 255), [12, 13]), "block", "assembly_table_side")

BOMBS = {
    "demolition_charge": ((170, 40, 36), (230, 200, 40, 255)),
    "tactical_warhead": ((88, 98, 60), (230, 200, 40, 255)),
    "atomic_bomb": ((214, 178, 40), (24, 24, 24, 255)),
}
for name, (body, mark) in BOMBS.items():
    for part in ("top", "side", "front"):
        im = frame(noisy(body, 0.07), shade(body, 0.45))
        if part == "front":
            trefoil(im, 8, 8, mark, shade(body, 1.15) if name != "atomic_bomb" else None)
        elif part == "side":
            stripes(im, (230, 200, 40, 255), (24, 24, 24, 255), [2, 3])
            stripes(im, (230, 200, 40, 255), (24, 24, 24, 255), [12, 13])
        else:
            for x in range(6, 10):
                for y in range(6, 10):
                    im.putpixel((x, y), shade(body, 0.6))
        save(im, "block", f"{name}_{part}")


# ---- items ----
def item(pixels, palette):
    im = Image.new("RGBA", (16, 16), CLEAR)
    for y, row in enumerate(pixels):
        for x, ch in enumerate(row):
            if ch in palette:
                im.putpixel((x, y), palette[ch])
    return im


LUMP = [
    "................",
    "................",
    "......aaa.......",
    "....aabbbaa.....",
    "...abbcbbbba....",
    "...abbbbbcba....",
    "..abcbbbbbbba...",
    "..abbbbbcbbba...",
    "..abbbcbbbbba...",
    "...abbbbbbba....",
    "...aabbbcbaa....",
    ".....aaaaa......",
    "................",
    "................",
    "................",
    "................",
]
save(item(LUMP, {"a": (40, 70, 30, 255), "b": (90, 150, 60, 255), "c": (150, 230, 90, 255)}), "item", "raw_uranium")
save(item(LUMP, {"a": (30, 110, 30, 255), "b": (110, 240, 70, 255), "c": (220, 255, 170, 255)}), "item", "enriched_uranium")
ROD = [
    "................",
    "......aaaa......",
    ".....abbbba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abccba.....",
    ".....abbbba.....",
    "......aaaa......",
    "................",
    "................",
    "................",
]
save(item(ROD, {"a": (40, 40, 44, 255), "b": (210, 180, 40, 255), "c": (120, 240, 80, 255)}), "item", "nuclear_charge")
SHELL = [
    "................",
    "................",
    ".....aaaaaa.....",
    "....abbbbbba....",
    "...abbccbbbba...",
    "...abccbbbbba...",
    "...abbbbbbbba...",
    "...abbbbbbbba...",
    "...abbbbbbbba...",
    "...abbbbbbbba...",
    "...abbbbbbbba...",
    "....abbbbbba....",
    ".....aaaaaa.....",
    "................",
    "................",
    "................",
]
save(item(SHELL, {"a": (40, 42, 48, 255), "b": (120, 126, 136, 255), "c": (190, 196, 204, 255)}), "item", "bomb_casing")
BOX = [
    "................",
    "................",
    "................",
    "...aaaaaaaaaa...",
    "...abbbbbbbba...",
    "...abXXXXXXba...",
    "...abXXXXXXba...",
    "...abXXXXXXba...",
    "...abXXXXXXba...",
    "...abbbbbbbba...",
    "...aaaaaaaaaa...",
    "................",
    "................",
    "................",
    "................",
    "................",
]
base = {"a": (30, 30, 34, 255), "b": (80, 84, 92, 255)}
fuse = [r[:] for r in BOX]
fuse[0], fuse[1], fuse[2] = "........d.......", ".......dd.......", "......d........."
save(item(fuse, base | {"X": (150, 60, 40, 255), "d": (200, 180, 140, 255)}), "item", "fuse_detonator")
pad = item(BOX, base | {"X": (40, 44, 50, 255)})
for x in (6, 8, 10):
    for y in (5, 7):
        pad.putpixel((x, y), (200, 200, 200, 255))
pad.putpixel((8, 8), (230, 60, 50, 255))
save(pad, "item", "keypad_detonator")
recv = [r[:] for r in BOX]
recv[0], recv[1], recv[2] = "..........d.....", "..........d.....", "..........d....."
save(item(recv, base | {"X": (50, 90, 60, 255), "d": (180, 180, 190, 255)}), "item", "receiver_detonator")
REMOTE = [
    "...........d....",
    "...........d....",
    "...........d....",
    "....aaaaaaaa....",
    "....abbbbbba....",
    "....abeeeeba....",
    "....abeeeeba....",
    "....abbbbbba....",
    "....abbrrbba....",
    "....abrrrrba....",
    "....abbrrbba....",
    "....abbbbbba....",
    "....abcbcbba....",
    "....abbbbbba....",
    "....aaaaaaaa....",
    "................",
]
save(item(REMOTE, {"a": (30, 30, 34, 255), "b": (70, 74, 82, 255), "e": (90, 200, 120, 255),
                   "r": (220, 40, 40, 255), "c": (160, 160, 170, 255), "d": (150, 150, 160, 255)}), "item", "remote")


# ---- the centrifuge's screen ----
# 176x166 like a furnace's, and right of it the lit flame (176, 0) and the full
# arrow (176, 14) that glue.rs draws over the empty ones. No random here, so the
# textures above stay the same.
def rect(im, x, y, w, h, c):
    for yy in range(y, y + h):
        for xx in range(x, x + w):
            im.putpixel((xx, yy), c)


def slot(im, x, y, size=18):
    """A slot's frame, at the item's corner minus one (the output's is bigger)."""
    rect(im, x - 1, y - 1, size, size, (255, 255, 255, 255))
    rect(im, x - 1, y - 1, size - 1, size - 1, (55, 55, 55, 255))
    rect(im, x, y, size - 2, size - 2, (139, 139, 139, 255))


FLAME_SHAPE = [
    "......##......",
    "......##......",
    ".....###......",
    ".....####.....",
    "....#####.....",
    "....######....",
    "...#######....",
    "...########...",
    "..#########...",
    "..##########..",
    ".###########..",
    ".############.",
    "..##########..",
    "...########...",
]


def arrow_shape(x, y):
    if x < 14:
        return 6 <= y <= 10
    return abs(y - 8) <= 8 - (x - 14) * 0.85


def flame(im, ox, oy, lit):
    for y, row in enumerate(FLAME_SHAPE):
        for x, ch in enumerate(row):
            if ch != "#":
                continue
            if not lit:
                c = (139, 139, 139, 255)
            else:
                # Green-gold from the tip down, a paler core.
                core = row.count("#") > 4 and abs(x + 0.5 - (row.index("#") + row.count("#") / 2)) < row.count("#") / 4
                c = shade((230, 240, 90), 1.0 - y * 0.03) if y < 5 else shade((110, 220, 70), 1.0 - (y - 5) * 0.05)
                if core and y > 4:
                    c = (200, 255, 150, 255)
            im.putpixel((ox + x, oy + y), c)


def arrow(im, ox, oy, full):
    for y in range(17):
        for x in range(24):
            if arrow_shape(x, y):
                if not full:
                    c = (139, 139, 139, 255)
                else:
                    c = (150, 240, 100, 255) if y in (6, 0) or not arrow_shape(x, y - 1) else (80, 190, 60, 255)
                im.putpixel((ox + x, oy + y), c)


gui = Image.new("RGBA", (200, 166), CLEAR)
rect(gui, 0, 0, 176, 166, (198, 198, 198, 255))
rect(gui, 0, 0, 176, 2, (255, 255, 255, 255))
rect(gui, 0, 0, 2, 166, (255, 255, 255, 255))
rect(gui, 0, 164, 176, 2, (85, 85, 85, 255))
rect(gui, 174, 0, 2, 166, (85, 85, 85, 255))
# Where the slots are: glue.rs has the same numbers.
slot(gui, 56, 17)
slot(gui, 56, 53)
slot(gui, 112, 31, 26)
for row in range(3):
    for col in range(9):
        slot(gui, 8 + col * 18, 84 + row * 18)
for col in range(9):
    slot(gui, 8 + col * 18, 142)
flame(gui, 57, 37, False)
arrow(gui, 79, 34, False)
flame(gui, 176, 0, True)
arrow(gui, 176, 14, True)
save(gui, "gui", "centrifuge")


# ---- the radiation's items (plan 5) ----
# Drawn last and with their own random, so everything above stays the same.
suit_rng = random.Random(2605)
SUIT = (226, 190, 40)
SUIT_DARK = (170, 136, 22, 255)
RUBBER = (40, 40, 44, 255)
VISOR = (36, 62, 72, 255)
SHINE = (150, 205, 215, 255)


def sheet(pixels, palette):
    for row in pixels:
        assert len(row) == len(pixels[0]), row
    return item(pixels, palette)


SUIT_PALETTE = {"o": (64, 50, 10, 255), "y": (230, 196, 44, 255), "d": SUIT_DARK, "k": RUBBER,
                "v": VISOR, "w": SHINE}
HELMET = [
    "................",
    "................",
    ".....oooooo.....",
    "....oyyyyyyo....",
    "...oyyyyyyyyo...",
    "...oyooooooyo...",
    "...oyovwvvoyo...",
    "...oyovvvvoyo...",
    "...oyovvvvoyo...",
    "...oyooooooyo...",
    "...oyyyyyyyyo...",
    "...oyyyyyyyyo...",
    "....oooooooo....",
    "................",
    "................",
    "................",
]
JACKET = [
    "................",
    "................",
    "..oooo....oooo..",
    ".oyyyyooooyyyyo.",
    "oyyyyyyddyyyyyyo",
    "oyyyyyyddyyyyyyo",
    "okkoyyyddyyyokko",
    "ooooyyyddyyyoooo",
    "...oyyyddyyyo...",
    "...oyyyddyyyo...",
    "...oyyyddyyyo...",
    "...oyyyddyyyo...",
    "...oyyyyyyyyo...",
    "...oooooooooo...",
    "................",
    "................",
]
PANTS = [
    "................",
    "................",
    "...oooooooooo...",
    "...okkkkkkkko...",
    "...oyyyyyyyyo...",
    "...oyyyyyyyyo...",
    "...oyyyooyyyo...",
    "...oyyyooyyyo...",
    "...oyyyooyyyo...",
    "...oyyyooyyyo...",
    "...oyyyooyyyo...",
    "...oyyyooyyyo...",
    "...oyyyooyyyo...",
    "...oooooooooo...",
    "................",
    "................",
]
BOOTS = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "..oooo....oooo..",
    "..oyyo....oyyo..",
    "..oyyo....oyyo..",
    "..oyyo....oyyo..",
    ".oyyyo...oyyyo..",
    ".oyyyo...oyyyo..",
    ".okkko...okkko..",
    ".ooooo...ooooo..",
    "................",
]
for name, pixels in (("helmet", HELMET), ("chestplate", JACKET), ("leggings", PANTS), ("boots", BOOTS)):
    save(sheet(pixels, SUIT_PALETTE), "item", f"hazmat_{name}")

VIAL = [
    "................",
    "......cccc......",
    "......cccc......",
    "......gwwg......",
    "......gwwg......",
    ".....gwwwwg.....",
    "....gwwwwwwg....",
    "...gwwwwwwwwg...",
    "...gllllllllg...",
    "...glhllllllg...",
    "...glhllllllg...",
    "...gllllllllg...",
    "...gllllllllg...",
    "....gllllllg....",
    ".....gggggg.....",
    "................",
]
save(sheet(VIAL, {"c": (120, 82, 44, 255), "g": (160, 196, 208, 255), "w": (214, 234, 242, 120),
                  "l": (236, 132, 36, 255), "h": (255, 204, 120, 255)}), "item", "anti_rad")

GEIGER = [
    "...........dd...",
    "...........dd...",
    "..aaaaaaaaaaaa..",
    "..abbbbbbbbbba..",
    "..abwkwkwkwkba..",
    "..abwwwwwwrwba..",
    "..abwwwwwrwwba..",
    "..abwwwwrwwwba..",
    "..abbbbbbbbbba..",
    "..abbbbbbbgbba..",
    "..abbbbbbbbbba..",
    "..abkbkbkbkbba..",
    "..abbbbbbbbbba..",
    "..aaaaaaaaaaaa..",
    "................",
    "................",
]
save(sheet(GEIGER, {"a": (30, 30, 34, 255), "b": (196, 164, 48, 255), "w": (236, 236, 222, 255),
                    "k": (30, 30, 34, 255), "r": (220, 40, 40, 255), "g": (90, 230, 80, 255),
                    "d": (120, 120, 130, 255)}), "item", "geiger_counter")


# The suit on a player: 64x32 sheets in vanilla's armour layout. `humanoid` is the
# helmet, the jacket with its sleeves, and the boots on the legs' boxes;
# `humanoid_leggings` the legs and the waist.
def box(im, u, v, w, h, d, paint):
    """Paints a model box's six faces at (u, v): `paint(face, x, y, fw, fh)` gives a
    pixel's colour, or None to leave it clear."""
    faces = {
        "top": (u + d, v, w, d), "bottom": (u + d + w, v, w, d),
        "right": (u, v + d, d, h), "front": (u + d, v + d, w, h),
        "left": (u + d + w, v + d, d, h), "back": (u + 2 * d + w, v + d, w, h),
    }
    for face, (fx, fy, fw, fh) in faces.items():
        for y in range(fh):
            for x in range(fw):
                c = paint(face, x, y, fw, fh)
                if c is not None:
                    im.putpixel((fx + x, fy + y), c)


def cloth(x, y, fw, fh):
    """The suit's fabric, a shade darker at a face's edges, its seams."""
    edge = x in (0, fw - 1) or y in (0, fh - 1)
    return shade(SUIT, (0.86 if edge else 1.0) + suit_rng.uniform(-0.06, 0.06))


def head(face, x, y, fw, fh):
    if face == "front" and 1 <= x <= 6 and 2 <= y <= 5:
        return SHINE if (x, y) == (2, 2) else VISOR
    return cloth(x, y, fw, fh)


def jacket(face, x, y, fw, fh):
    if face == "front" and x in (3, 4):
        return SUIT_DARK
    return cloth(x, y, fw, fh)


def sleeve(face, x, y, fw, fh):
    if face == "bottom" or (face not in ("top", "bottom") and y >= fh - 3):
        return RUBBER
    return cloth(x, y, fw, fh)


def boot(face, x, y, fw, fh):
    if face == "top":
        return None
    if face == "bottom" or (face != "top" and y == fh - 1):
        return RUBBER
    return cloth(x, y, fw, fh) if y >= fh - 6 else None


def waist(face, x, y, fw, fh):
    if face == "top":
        return None
    if face == "bottom":
        return cloth(x, y, fw, fh)
    if y == fh - 5:
        return RUBBER
    return cloth(x, y, fw, fh) if y > fh - 5 else None


def leg(face, x, y, fw, fh):
    return cloth(x, y, fw, fh)


layer = Image.new("RGBA", (64, 32), CLEAR)
box(layer, 0, 0, 8, 8, 8, head)
box(layer, 16, 16, 8, 12, 4, jacket)
box(layer, 40, 16, 4, 12, 4, sleeve)
box(layer, 0, 16, 4, 12, 4, boot)
save(layer, "entity/equipment/humanoid", "hazmat")
layer = Image.new("RGBA", (64, 32), CLEAR)
box(layer, 0, 16, 4, 12, 4, leg)
box(layer, 16, 16, 8, 12, 4, waist)
save(layer, "entity/equipment/humanoid_leggings", "hazmat")

# The radiation's effect icon, 18x18: a green trefoil with a dark rim.
icon = Image.new("RGBA", (18, 18), CLEAR)


def blade(x, y):
    dx, dy = x - 9 + 0.5, y - 9 + 0.5
    r = math.hypot(dx, dy)
    a = (math.degrees(math.atan2(dy, dx)) + 360 + 30) % 120
    return r < 1.8 or (2.8 < r < 7.6 and 30 < a < 90)


for y in range(18):
    for x in range(18):
        if blade(x, y):
            icon.putpixel((x, y), (127, 178, 56, 255) if blade(x, y - 1) else (196, 236, 120, 255))
        elif any(blade(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
            icon.putpixel((x, y), (28, 44, 18, 255))
save(icon, "mob_effect", "radiation")


# ---- the silo and the bomber (plan 7) ----
# Drawn last and with their own random, so everything above stays the same.
silo_rng = random.Random(2607)
HAZARD = (230, 196, 40, 255)
NIGHT = (24, 24, 26, 255)
RIVET = (170, 176, 186, 255)


def plate(base, spread=0.06, edge=EDGE):
    im = Image.new("RGBA", (16, 16))
    for y in range(16):
        for x in range(16):
            im.putpixel((x, y), shade(base, 1 + silo_rng.uniform(-spread, spread)))
    return frame(im, edge)


def rivets(im, points, colour=RIVET):
    for p in points:
        im.putpixel(p, colour)
    return im


def ring(im, inner, outer, a, b):
    """A hazard band round the middle: yellow and black wedges between two radii."""
    for y in range(16):
        for x in range(16):
            dx, dy = x - 7.5, y - 7.5
            if inner <= math.hypot(dx, dy) < outer:
                wedge = int((math.degrees(math.atan2(dy, dx)) + 360) // 30) % 2
                im.putpixel((x, y), a if wedge else b)
    return im


# The launch pad: a blast plate with a grate in a hazard ring on top, ribbed steel on the sides.
top = plate((96, 100, 108))
ring(top, 4.5, 7.0, HAZARD, NIGHT)
for y in range(5, 11):
    for x in range(5, 11):
        top.putpixel((x, y), (40, 42, 46, 255) if (x + y) % 2 else (70, 74, 80, 255))
save(rivets(top, [(1, 1), (14, 1), (1, 14), (14, 14)]), "block", "launch_pad_top")
side = plate((84, 88, 96))
stripes(side, HAZARD, NIGHT, [1, 2])
for x in (4, 8, 12):
    for y in range(4, 15):
        side.putpixel((x, y), shade((84, 88, 96), 0.7))
save(side, "block", "launch_pad_side")
front = side.copy()
for y in range(6, 13):
    for x in range(5, 12):
        front.putpixel((x, y), (54, 58, 64, 255) if 5 < x < 11 and 6 < y < 12 else EDGE)
front.putpixel((8, 9), (220, 60, 40, 255))
save(front, "block", "launch_pad_front")

# The hatch: two doors with a seam down the middle, hazard edges and bolts.
hatch = plate((120, 124, 112), 0.05)
for x in range(1, 15):
    for y in (1, 14):
        hatch.putpixel((x, y), HAZARD if (x // 2) % 2 else NIGHT)
for y in range(1, 15):
    hatch.putpixel((7, y), NIGHT)
    hatch.putpixel((8, y), (60, 62, 58, 255))
for y in (4, 11):
    for x in (3, 4, 11, 12):
        hatch.putpixel((x, y), (150, 154, 140, 255))
save(rivets(hatch, [(2, 7), (13, 7), (2, 8), (13, 8)]), "block", "silo_hatch_top")
hatch_side = plate((100, 104, 94), 0.05)
stripes(hatch_side, HAZARD, NIGHT, [13, 14])
save(rivets(hatch_side, [(2, 4), (7, 4), (13, 4)]), "block", "silo_hatch_side")
save(plate((58, 60, 56), 0.05), "block", "silo_hatch_bottom")

# The console: a green screen, a red launch key under a flap, and a row of switches.
cons = plate((70, 76, 84))
for y in range(2, 8):
    for x in range(2, 14):
        cons.putpixel((x, y), (16, 34, 20, 255) if 2 < x < 13 and 2 < y < 7 else (30, 32, 36, 255))
for x, y in ((4, 4), (5, 4), (6, 4), (8, 4), (9, 4), (4, 5), (6, 5), (8, 5), (10, 5), (11, 5)):
    cons.putpixel((x, y), (90, 230, 110, 255))
for x in (3, 5, 7):
    for y in (10, 12):
        cons.putpixel((x, y), (190, 190, 196, 255))
for y in range(9, 14):
    for x in range(10, 14):
        cons.putpixel((x, y), NIGHT if x in (10, 13) or y in (9, 13) else (210, 40, 36, 255))
save(cons, "block", "launch_console_front")
save(stripes(plate((76, 82, 90)), HAZARD, NIGHT, [14]), "block", "launch_console_side")
cons_top = plate((86, 92, 100))
for x in range(3, 13):
    cons_top.putpixel((x, 12), (50, 54, 60, 255))
save(cons_top, "block", "launch_console_top")

# The strike beacon: a red lamp behind a grille, an antenna on top.
beacon = plate((92, 60, 56))
for y in range(3, 13):
    for x in range(3, 13):
        d = math.hypot(x - 7.5, y - 7.5)
        if d < 4.6:
            beacon.putpixel((x, y), shade((240, 50, 40), 1.25 - d / 6) if y % 2 else (40, 20, 18, 255))
save(beacon, "block", "strike_beacon_front")
beacon_side = plate((84, 56, 52))
stripes(beacon_side, HAZARD, NIGHT, [2, 3])
beacon_side.putpixel((7, 9), (240, 60, 40, 255))
beacon_side.putpixel((8, 9), (240, 60, 40, 255))
save(beacon_side, "block", "strike_beacon_side")
beacon_top = plate((80, 82, 88))
ring(beacon_top, 3.5, 6.5, (150, 154, 160, 255), (110, 114, 120, 255))
for p in ((7, 7), (8, 7), (7, 8), (8, 8)):
    beacon_top.putpixel(p, (240, 60, 40, 255))
save(beacon_top, "block", "strike_beacon_top")

# The missile's icon, nose up and to the right.
MISSILE = [
    "............rrr.",
    "...........rrrrr",
    "..........wrrrr.",
    ".........wwwrr..",
    "........wwwwws..",
    ".......wwwwws...",
    "......rrwwws....",
    ".....rrrrws.....",
    "....wwrrrs......",
    "...rwwwws.......",
    "..rrwwws........",
    ".rrrwws.........",
    "..rkkks.........",
    ".fokks..........",
    "fofss...........",
    "ff..............",
]
save(item(MISSILE, {"w": (236, 236, 232, 255), "s": (150, 154, 160, 255), "r": (210, 40, 36, 255),
                    "k": (50, 52, 56, 255), "f": (250, 170, 40, 255), "o": (255, 230, 120, 255)}), "item", "missile")


# The flying models' sheets: four flat regions, the models' faces map them by quarter.
def quarters(colours):
    im = Image.new("RGBA", (16, 16))
    for i, c in enumerate(colours):
        ox, oy = (i % 2) * 8, (i // 2) * 8
        for y in range(8):
            for x in range(8):
                im.putpixel((ox + x, oy + y), shade(c, 1 + silo_rng.uniform(-0.04, 0.04)))
    return im


save(quarters([(234, 234, 228), (200, 36, 32), (60, 62, 66), (22, 22, 24)]), "item", "missile_model")
save(quarters([(70, 76, 66), (128, 134, 126), (90, 140, 170), (20, 20, 20)]), "item", "bomber_model")
# ---- the guide (plan 6) ----
# Drawn last and with their own random, so everything above stays the same. The screen's
# numbers are runtime.rs's (BOOK_SIZE, PAGES, tab_src, arrow_src, SLOT_SRC, RECIPE_ARROW_SRC).
guide_rng = random.Random(2611)
LEATHER = (74, 44, 26)
LEATHER_EDGE = (36, 20, 12, 255)
LEATHER_LIGHT = (112, 72, 42, 255)
STITCH = (156, 116, 72, 255)
BRASS = (196, 158, 72, 255)
BRASS_DARK = (128, 98, 40, 255)
PARCHMENT = (233, 221, 184)
PAGE_EDGE = (186, 166, 126, 255)
INK = (96, 72, 44, 255)


def grain(base, spread):
    return shade(base, 1 + guide_rng.uniform(-spread, spread))


# The item: a green book with a gold-rimmed trefoil, its pages showing on the right.
book = Image.new("RGBA", (16, 16), CLEAR)
for y in range(1, 15):
    for x in range(2, 14):
        if x == 13:
            c = (236, 226, 196, 255) if 1 < y < 14 else (60, 40, 24, 255)
        elif x in (2, 3):
            c = (198, 160, 60, 255) if y in (3, 12) else shade((30, 48, 26), 1 + guide_rng.uniform(-0.08, 0.08))
        else:
            c = grain((48, 82, 40), 0.08)
        if y in (1, 14) and x < 13:
            c = (22, 30, 18, 255)
        book.putpixel((x, y), c)
for y in range(1, 15):
    book.putpixel((1, y), (22, 30, 18, 255))
    book.putpixel((14, y), (22, 30, 18, 255) if y in (1, 14) else (150, 136, 104, 255))
for y in range(3, 13):
    for x in range(4, 13):
        dx, dy = x - 8 + 0.5, y - 7.5 + 0.5
        r = math.hypot(dx, dy)
        a = (math.degrees(math.atan2(dy, dx)) + 360 + 30) % 120
        if r < 1.0 or (1.9 < r < 3.6 and 30 < a < 90):
            book.putpixel((x, y), (24, 22, 16, 255))
        elif r < 4.3:
            book.putpixel((x, y), (226, 190, 48, 255))
save(book, "item", "guide")

# The screen: the spread, the bookmarks, the arrows, a slot and a recipe's arrow.
W, H, BOOK_W, BOOK_H = 288, 264, 288, 192
gui = Image.new("RGBA", (W, H), CLEAR)
for y in range(BOOK_H):
    for x in range(BOOK_W):
        gui.putpixel((x, y), grain(LEATHER, 0.07))
for x in range(BOOK_W):
    gui.putpixel((x, 0), LEATHER_EDGE)
    gui.putpixel((x, BOOK_H - 1), LEATHER_EDGE)
    gui.putpixel((x, 1), LEATHER_LIGHT)
for y in range(BOOK_H):
    gui.putpixel((0, y), LEATHER_EDGE)
    gui.putpixel((BOOK_W - 1, y), LEATHER_EDGE)
    if 0 < y < BOOK_H - 1:
        gui.putpixel((1, y), LEATHER_LIGHT)
# Brass corners.
for i in range(7):
    for j in range(7 - i):
        for x, y in ((i, j), (BOOK_W - 1 - i, j), (i, BOOK_H - 1 - j), (BOOK_W - 1 - i, BOOK_H - 1 - j)):
            gui.putpixel((x, y), BRASS if i + j < 6 else BRASS_DARK)
# The pages, a stack of edges under each, and the gutter's shadow toward the spine.
PAGES = [(6, 6, 136, 164), (146, 6, 136, 164)]
for side, (px, py, pw, ph) in enumerate(PAGES):
    for y in range(py, py + ph):
        for x in range(px, px + pw):
            gutter = (px + pw - 1 - x) if side == 0 else (x - px)
            k = 1 - 0.16 * max(0, 1 - gutter / 10)
            outer = x - px if side == 0 else px + pw - 1 - x
            k -= 0.05 * max(0, 1 - outer / 4)
            gui.putpixel((x, y), shade(PARCHMENT, k * (1 + guide_rng.uniform(-0.025, 0.025))))
    for x in range(px, px + pw):
        gui.putpixel((x, py), PAGE_EDGE)
        for n, y in enumerate(range(py + ph, py + ph + 3)):
            gui.putpixel((x, y), shade(PARCHMENT, 0.86 - 0.1 * n) if (x + n) % 3 else PAGE_EDGE)
    for y in range(py, py + ph):
        gui.putpixel((px if side == 0 else px + pw - 1, y), PAGE_EDGE)
# The spine between them, down to the footer, which keeps its middle for the page number.
for y in range(3, 173):
    for x in range(142, 146):
        gui.putpixel((x, y), (54, 30, 16, 255) if x in (142, 145) else (92, 58, 32, 255))
# Stitches round the cover's inner edge, where no page lies.
for x in range(8, BOOK_W - 8, 4):
    gui.putpixel((x, 3), STITCH)
    gui.putpixel((x, BOOK_H - 4), STITCH)
for y in range(8, BOOK_H - 8, 4):
    gui.putpixel((3, y), STITCH)
    gui.putpixel((BOOK_W - 4, y), STITCH)
# A gold rule across the footer, between the arrows.
for x in range(40, BOOK_W - 40):
    if x < 120 or x > 168:
        gui.putpixel((x, 181), (150, 116, 60, 255))

# The bookmarks: a ribbon a chapter, its left end cut in a notch; the open ones lighter.
# Eight to a closed and an open row; the ninth on starts a pair of rows under the arrows.
TAB_W, TAB_H, TABS_A_ROW = 26, 18, 8
COLOURS = [(178, 46, 40), (214, 120, 36), (214, 182, 48), (74, 146, 58), (44, 136, 136), (56, 90, 170),
           (122, 70, 160), (112, 112, 122), (204, 78, 142)]
for i, colour in enumerate(COLOURS):
    top = 192 + i // TABS_A_ROW * 2 * TAB_H
    for row, open_ in ((top, False), (top + TAB_H, True)):
        base = shade(colour, 1.18) if open_ else colour
        for y in range(TAB_H):
            notch = abs(y - (TAB_H - 1) / 2)
            for x in range(TAB_W):
                if x < 4 - notch / 2.2:
                    continue
                if y == 0:
                    c = shade(base, 1.25)
                elif y == TAB_H - 1 or x < 5 - notch / 2.2:
                    c = shade(base, 0.6)
                else:
                    c = shade(base, 1 + guide_rng.uniform(-0.05, 0.05))
                gui.putpixel((i % TABS_A_ROW * TAB_W + x, row + y), c)
        if open_:
            for x in range(4, TAB_W):
                gui.putpixel((i % TABS_A_ROW * TAB_W + x, row + 1), (255, 248, 220, 255))


# The arrows, light on the leather; the lit ones brighter.
def page_arrow(ox, oy, right, colour):
    for y in range(10):
        for x in range(18):
            fx = 17 - x if right else x
            head = fx <= 6 and abs(y - 4.5) <= fx * 0.75 + 0.25
            shaft = 6 < fx < 17 and 3 <= y <= 6
            if head or shaft:
                gui.putpixel((ox + x, oy + y), colour if 0 < y < 9 and fx > 0 else shade(colour[:3], 0.7))


for right, x in ((False, 208), (True, 226)):
    page_arrow(x, 192, right, (204, 172, 110, 255))
    page_arrow(x, 202, right, (255, 232, 156, 255))
# A slot on the parchment, and a recipe's arrow.
for y in range(18):
    for x in range(18):
        if x == 17 or y == 17:
            c = (250, 242, 214, 255)
        elif x == 0 or y == 0:
            c = (120, 94, 60, 255)
        else:
            c = (206, 190, 150, 255)
        gui.putpixel((244 + x, 192 + y), c)
for y in range(10):
    for x in range(16):
        if (x <= 10 and 3 <= y <= 6) or (x > 10 and abs(y - 4.5) <= (15 - x) + 0.5):
            gui.putpixel((262 + x, 192 + y), INK)
save(gui, "gui", "guide")

# ---- the sunbreaker (plan 8) ----
# Drawn last and with their own random, so everything above stays the same.
sun_rng = random.Random(2613)
GLOW = (120, 255, 150, 255)
GLOW_DIM = (50, 150, 80, 255)


def sun_plate(base, spread=0.05, edge=EDGE):
    im = Image.new("RGBA", (16, 16))
    for y in range(16):
        for x in range(16):
            im.putpixel((x, y), shade(base, 1 + sun_rng.uniform(-spread, spread)))
    return frame(im, edge)


# The core: dark armour plates whose seams glow, a lit eye in front.
CORE = (34, 34, 40)
for part in ("top", "side", "front"):
    im = sun_plate(CORE, 0.08, (14, 14, 18, 255))
    for i in range(1, 15):
        for j in (5, 10):
            im.putpixel((i, j), GLOW_DIM if sun_rng.random() < 0.7 else GLOW)
            if part != "front":
                im.putpixel((j, i), GLOW_DIM if sun_rng.random() < 0.7 else GLOW)
    if part == "front":
        for y in range(16):
            for x in range(16):
                d = math.hypot(x - 7.5, y - 7.5)
                if d < 2.0:
                    im.putpixel((x, y), (230, 255, 220, 255))
                elif d < 3.4:
                    im.putpixel((x, y), GLOW)
                elif d < 4.4:
                    im.putpixel((x, y), (20, 22, 26, 255))
    elif part == "top":
        for x, y in ((7, 7), (8, 7), (7, 8), (8, 8)):
            im.putpixel((x, y), (230, 255, 220, 255))
    save(im, "block", f"sunbreaker_core_{part}")

# The implosion shell: a steel plate with two rows of shaped charges and their wire.
shell = sun_plate((104, 108, 116))
for row in (3, 9):
    for col in (2, 7, 12):
        for dy in range(4):
            for dx in range(2):
                c = (150, 54, 40, 255) if dx == 0 else (110, 36, 28, 255)
                shell.putpixel((col + dx, row + dy), shade(c, 1 + sun_rng.uniform(-0.08, 0.08)))
        shell.putpixel((col, row), (210, 120, 90, 255))
    for x in range(1, 15):
        if shell.getpixel((x, row + 4))[0] < 140:
            shell.putpixel((x, row + 4), (196, 170, 60, 255))
save(shell, "block", "implosion_shell")

# The neutron reflector: polished blue-grey metal with a bevel and a sheen across it.
refl = sun_plate((120, 138, 160), 0.03, (60, 70, 86, 255))
for i in range(1, 15):
    refl.putpixel((i, 1), (170, 186, 206, 255))
    refl.putpixel((1, i), (170, 186, 206, 255))
    refl.putpixel((i, 14), (88, 102, 122, 255))
    refl.putpixel((14, i), (88, 102, 122, 255))
for y in range(2, 14):
    for x in range(2, 14):
        if abs((x + y) - 12) <= 1:
            refl.putpixel((x, y), shade((150, 168, 190), 1.12))
        elif abs((x + y) - 17) == 0:
            refl.putpixel((x, y), shade((150, 168, 190), 1.05))
save(refl, "block", "neutron_reflector")

# Weapons-grade uranium: the lump, brighter than the enriched one, with a glow round it.
wg = item(LUMP, {"a": (40, 170, 40, 255), "b": (150, 255, 100, 255), "c": (240, 255, 215, 255)})
halo = wg.copy()
for y in range(16):
    for x in range(16):
        if wg.getpixel((x, y))[3] == 0 and any(
            0 <= x + dx < 16 and 0 <= y + dy < 16 and wg.getpixel((x + dx, y + dy))[3] == 255
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))
        ):
            halo.putpixel((x, y), (150, 255, 120, 110))
save(halo, "item", "weapons_grade_uranium")
print("textures written")
