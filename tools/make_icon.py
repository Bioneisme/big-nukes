#!/usr/bin/env python3
"""Draws the catalog's picture: a yellow bomb with a black trefoil, a mushroom cloud behind it.

    python tools/make_icon.py

Original pixel art, the same every time: drawn on a 64x64 grid and written 4x as icon.png, 256x256.
"""
import math
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
N = 64
SCALE = 4

SKY = [(46, 28, 46), (70, 34, 50), (104, 42, 50), (146, 60, 48), (194, 94, 52), (232, 146, 70)]
GROUND = (50, 42, 38)
GROUND_DARK = (36, 30, 28)
ASH = (112, 104, 96)
CLOUD_LIGHT = (250, 222, 170)
CLOUD = (230, 170, 104)
CLOUD_SHADE = (176, 102, 62)
STEM = (204, 132, 80)
STEM_SHADE = (150, 84, 54)
FIREBALL = (255, 206, 92)
OUTLINE = (30, 24, 24)
YELLOW = (240, 200, 40)
YELLOW_LIGHT = (255, 238, 128)
YELLOW_SHADE = (198, 150, 22)
YELLOW_DEEP = (150, 106, 12)
STEEL = (76, 80, 90)
STEEL_LIGHT = (122, 128, 140)
TREFOIL = (24, 20, 20)


def ellipse(x, y, cx, cy, rx, ry):
    return ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0


def sky(p):
    band = (N - 14) / len(SKY)
    for y in range(N):
        for x in range(N):
            p[x, y] = SKY[min(len(SKY) - 1, int(y / band))]


def cloud(p):
    """A mushroom cloud far off on the right: a stem from the ground, a cap with a darker underside."""
    for y in range(N):
        for x in range(N):
            c = None
            # The stem widens at its foot, where the fireball still glows.
            half = 3 + max(0, y - 40) * 0.35
            if 20 <= y <= 50 and abs(x - 48 + 0.5) <= half:
                c = STEM_SHADE if x >= 49 else STEM
                if y >= 46:
                    c = FIREBALL if abs(x - 48 + 0.5) <= half - 2 else STEM
            # The ring round the stem, under the cap.
            if ellipse(x, y, 48, 25, 9, 2.5):
                c = CLOUD_SHADE
            # The cap: lit on top, shaded below.
            if ellipse(x, y, 48, 17, 13, 7.5):
                c = CLOUD_SHADE if y >= 20 else CLOUD
                if ellipse(x, y, 45, 14, 8, 4):
                    c = CLOUD_LIGHT
            if c is not None:
                p[x, y] = c


def ground(p):
    for y in range(N - 14, N):
        for x in range(N):
            p[x, y] = GROUND_DARK if (x * 7 + y * 3) % 11 == 0 else GROUND
    for x in range(N):
        p[x, N - 14] = ASH


def in_body(x, y):
    """The bomb's body: a round nose on the left, a cylinder, a taper to the tail."""
    cy = 39.5
    if ellipse(x, y, 17, cy, 11, 10):
        return True
    if 17 <= x <= 38 and abs(y - cy) <= 10:
        return True
    if 38 < x <= 46:
        half = 10 - (x - 38) * 0.75
        return abs(y - cy) <= half
    return False


def in_fins(x, y):
    """Two fins and the box round them, at the tail."""
    cy = 39.5
    if 44 <= x <= 54 and abs(y - cy) <= 2.5:
        return True
    for sign in (-1, 1):
        d = (y - cy) * sign
        if 46 <= x <= 54 and 2 <= d <= 4 + (x - 46) * 0.9:
            return True
    return False


def bomb(p):
    body = {(x, y) for y in range(N) for x in range(N) if in_body(x, y)}
    fins = {(x, y) for y in range(N) for x in range(N) if in_fins(x, y)} - body
    for x, y in fins:
        p[x, y] = STEEL_LIGHT if y < 39.5 else STEEL
    for x, y in body:
        top = y - 29.5
        if top < 4:
            c = YELLOW_LIGHT
        elif top < 13:
            c = YELLOW
        elif top < 17:
            c = YELLOW_SHADE
        else:
            c = YELLOW_DEEP
        p[x, y] = c
    # The seams between the nose, the body and the tail.
    for y in range(N):
        for x in (20, 42):
            if (x, y) in body and (x, y - 1) in body and (x, y + 1) in body:
                p[x, y] = YELLOW_SHADE
    # A dark outline round the whole bomb.
    shape = body | fins
    for x, y in shape:
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            q = (x + dx, y + dy)
            if q not in shape and 0 <= q[0] < N and 0 <= q[1] < N:
                p[q] = OUTLINE


def trefoil(p, cx=31.0, cy=39.5, inner=2.8, outer=8.4):
    """The radiation sign: three blades 60 degrees wide round a dot."""
    for y in range(N):
        for x in range(N):
            dx, dy = x + 0.5 - cx - 0.5, cy - (y + 0.5) + 0.5
            r = math.hypot(dx, dy)
            if r <= 1.8:
                p[x, y] = TREFOIL
                continue
            if not inner <= r <= outer:
                continue
            a = math.degrees(math.atan2(dy, dx)) % 360
            # Blades centred on 30, 150 and 270 degrees: up-right, up-left, down.
            if any(abs((a - c + 180) % 360 - 180) <= 30 for c in (30, 150, 270)):
                p[x, y] = TREFOIL


def main():
    im = Image.new("RGB", (N, N))
    p = im.load()
    sky(p)
    cloud(p)
    ground(p)
    bomb(p)
    trefoil(p)
    im.convert("RGBA").resize((N * SCALE, N * SCALE), Image.NEAREST).save(ROOT / "icon.png", optimize=True)


if __name__ == "__main__":
    main()
