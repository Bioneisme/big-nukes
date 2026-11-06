#!/usr/bin/env python3
"""Draws the mod's 16x16 textures: original pixel art from a fixed seed.

    python tools/make_textures.py

Writes content/assets/big_nukes/textures/{block,item}/*.png.
"""
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
print("textures written")
