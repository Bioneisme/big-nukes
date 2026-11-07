#!/usr/bin/env python3
"""Draws the catalogue's picture: a mushroom cloud with the radiation sign in its cap, on a hazard-yellow tile.

    python tools/make_icon.py

Original pixel art, the same every time: drawn on a 64x64 grid and written 8x as icon.png, 512x512. The design
is a silhouette first, so it still reads at the 48 px of the catalogue's lists: a black cloud on the yellow of
the mod's atomic bomb, the trefoil cut out of the cap, the flash at its foot throwing rays across the tile, and a
band of hazard stripes along the ground.
"""
import math
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
N = 64
SCALE = 8

INK = (22, 18, 26)
# The bomb's yellow, dark to light; the sky gets lighter toward the flash.
YELLOW = [(170, 120, 18), (226, 176, 32), (255, 214, 48), (255, 238, 130), (255, 252, 210)]
FLASH = (255, 255, 242)
FLASH_RING = (255, 200, 92)
FLASH_LINE = (250, 140, 52)
RIM = (150, 70, 26)
CLEAR = (0, 0, 0, 0)

FLASH_AT = (32.0, 52.0)  # the foot of the cloud, where the bomb went off
RING = (33, 11, 3)       # centre y, half width and half height of the ring round the stem
RAY = 30                 # degrees from one ray of light to the next
CORNER = 4               # radius of the tile's rounded corners
N4 = ((1, 0), (-1, 0), (0, 1), (0, -1))
N8 = N4 + ((1, 1), (1, -1), (-1, 1), (-1, -1))


def cells():
    for y in range(N):
        for x in range(N):
            yield x, y


def disc(cx, cy, r):
    return {(x, y) for x, y in cells() if (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= r * r}


def ellipse(cx, cy, rx, ry):
    return {(x, y) for x, y in cells() if ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1}


def outside(shape, nb=N4):
    return {(x + dx, y + dy) for x, y in shape for dx, dy in nb if (x + dx, y + dy) not in shape}


def sky(p):
    """Bands of yellow round the flash, darker toward the corners, every other ray a band lighter."""
    fx, fy = FLASH_AT
    for x, y in cells():
        dx, dy = x + 0.5 - fx, y + 0.5 - fy
        band = sum(math.hypot(dx / 1.2, dy) > b for b in (12, 22, 35, 50))
        if band >= 2 and math.degrees(math.atan2(dy, dx)) % RAY < RAY / 2:
            band -= 1
        p[x, y] = YELLOW[4 - band]


def cloud():
    """The cloud's silhouette: a lumpy cap, a ring of condensation round the stem below it, the stem widening to
    a skirt of dust at its foot. Returns the whole silhouette and the ring and the stem in it, as pixel sets."""
    cx = 32
    cap = set()
    for x, y, r in ((0, 15, 10), (-10, 16, 8), (10, 16, 8), (-18, 20, 6), (18, 20, 6), (-5, 11, 6), (5, 11, 6),
                    (-20.5, 24, 3.8), (20.5, 24, 3.8), (-14, 24.5, 4.6), (14, 24.5, 4.6), (-6, 24, 5.2),
                    (6, 24, 5.2), (0, 20, 9)):
        cap |= disc(cx + x, y, r)
    stem = set()
    for x, y in cells():
        if 26 <= y <= 50:
            t = (y + 0.5 - 26) / 24
            if abs(x + 0.5 - cx) <= 3.4 + 2.2 * t * t:
                stem.add((x, y))
    ring = ellipse(cx, *RING)
    skirt = disc(cx, 48, 6.5) | disc(cx - 7, 50, 4.6) | disc(cx + 7, 50, 4.6) | disc(cx - 13, 51.5, 3.4) \
        | disc(cx + 13, 51.5, 3.4)
    return cap | stem | ring | skirt, ring, stem


def trefoil(x, y, cx=32.0, cy=15.5, r=1.8):
    """The radiation sign: a dot and three blades 60 degrees wide, centred on 30, 150 and 270 degrees."""
    dx, dy = x + 0.5 - cx, cy - (y + 0.5)
    d = math.hypot(dx, dy)
    if d <= r:
        return True
    if not 1.6 * r <= d <= 5.2 * r:
        return False
    a = math.degrees(math.atan2(dy, dx)) % 360
    return any(abs((a - c + 180) % 360 - 180) <= 30 for c in (30, 150, 270))


def main():
    im = Image.new("RGBA", (N, N))
    p = im.load()
    sky(p)

    shape, ring, stem = cloud()
    # A line of sky above the ring either side of the stem, so it stands apart from the cap.
    gap = {(x, y) for x, y in outside(ring) - ring - stem if y < 32}
    shape -= gap
    for x, y in shape:
        p[x, y] = INK
    for x, y in gap:
        p[x, y] = YELLOW[3]
    # The flash lights the underside of the cap and the ring from below.
    for x, y in shape:
        if y >= 20 and (x, y + 1) not in shape:
            p[x, y] = RIM

    # The ground, the flash on it and the shock running out along it.
    fx, fy = FLASH_AT
    for x, y in cells():
        if y >= 53:
            p[x, y] = INK
        e = ((x + 0.5 - fx) / 8) ** 2 + ((y + 0.5 - (fy + 1)) / 3.2) ** 2
        if e <= 1:
            p[x, y] = FLASH if e < 0.35 else FLASH_RING
        elif y == 53 and 3 <= x <= 60:
            p[x, y] = FLASH_RING if abs(x + 0.5 - fx) <= 20 else FLASH_LINE

    for x, y in cells():
        if trefoil(x, y):
            p[x, y] = YELLOW[3]
    # Hazard stripes along the bottom.
    for x in range(N):
        for y in range(56, 61):
            if (x + y) // 4 % 2 == 0:
                p[x, y] = YELLOW[2]

    # Round the corners and ring the tile in ink.
    tile = set()
    for x, y in cells():
        cx = min(max(x + 0.5, CORNER), N - CORNER)
        cy = min(max(y + 0.5, CORNER), N - CORNER)
        if (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= CORNER * CORNER:
            tile.add((x, y))
    for x, y in cells():
        if (x, y) not in tile:
            p[x, y] = CLEAR
        elif any((x + dx, y + dy) not in tile for dx, dy in N8):
            p[x, y] = INK

    im.resize((N * SCALE, N * SCALE), Image.NEAREST).save(ROOT / "icon.png", optimize=True)


if __name__ == "__main__":
    main()
