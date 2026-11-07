#!/usr/bin/env python3
"""Draws the catalogue's picture: a mushroom cloud with the radiation sign in its cap, on a hazard-yellow tile.

    python tools/make_icon.py

Original pixel art, the same every time: drawn on a 128x128 grid and written 4x as icon.png, 512x512. The design
is a dark silhouette first, so it still reads at the 48 px of the catalogue's lists: a black cloud on the yellow
of the mod's atomic bomb, the trefoil cut out of its cap, the flash at its foot throwing rays and bands of light
across the tile, and a band of hazard stripes along the ground. Up close the cloud is billows of smoke lit from
the top left, their undersides glowing orange from the flash.

The layout is in units of a 64 grid (UNIT pixels each), so the shapes keep their proportions at any grid.
"""
import math
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
N = 128          # pixels on a side
SCALE = 4        # written N * SCALE = 512 px
UNIT = N / 64    # pixels per layout unit

INK = (22, 18, 26)
SMOKE = [INK, (32, 25, 33), (46, 35, 42), (64, 48, 52)]           # the cloud, shadow to light
GLOW = [(84, 38, 30), (128, 56, 28), (178, 84, 32), (226, 122, 44)]  # the cloud's undersides, lit by the flash
SKY = [(255, 252, 215), (255, 242, 160), (255, 230, 100), (255, 214, 48), (245, 198, 40), (226, 176, 32),
       (196, 144, 24), (160, 108, 16)]                                # near the flash to the corners
CUT = (255, 238, 130)
CUT_SHADE = (232, 186, 40)
FLASH = [(255, 255, 242), (255, 240, 180), (255, 214, 120), (255, 176, 72)]
SHOCK = (250, 140, 52)
STRIPE = [(255, 236, 120), (255, 214, 48), (214, 160, 26)]           # a bar's lit edge, face, shaded edge
CLEAR = (0, 0, 0, 0)

LIGHT = (-0.55, -0.7, 0.6)
CX = 32.0                # the cloud's axis
FLASH_AT = (32.0, 53.0)  # the foot of the cloud, where the bomb went off
GROUND = 53.0
RAY = 15                 # degrees from one ray of light to the next
CORNER = 4.0             # radius of the tile's rounded corners
N4 = ((1, 0), (-1, 0), (0, 1), (0, -1))


def norm(*v):
    l = math.sqrt(sum(a * a for a in v))
    return tuple(a / l for a in v)


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


L = norm(*LIGHT)


def cells():
    for y in range(N):
        for x in range(N):
            yield x, y


def unit(x, y):
    """A pixel's centre in layout units."""
    return (x + 0.5) / UNIT, (y + 0.5) / UNIT


# ---------------------------------------------------------------- the cloud's surfaces, in layout units
# Each surface answers (height toward the viewer, normal) at a point it covers, or None; f.box bounds it.
def sphere(cx, cy, r, cz=0.0):
    def f(u, v):
        dx, dy = u - cx, v - cy
        d2 = dx * dx + dy * dy
        if d2 > r * r:
            return None
        z = math.sqrt(r * r - d2)
        return z + cz, (dx / r, dy / r, z / r)
    f.box = (cx - r, cy - r, cx + r, cy + r)
    return f


def column():
    """The stem: a cylinder widening toward its foot."""
    def f(u, v):
        if not 25 <= v <= 51:
            return None
        t = (v - 26) / 24
        half = 3.3 + 2.4 * t * t
        dx = u - CX
        if abs(dx) > half:
            return None
        nx = dx / half
        nz = math.sqrt(max(0.0, 1 - nx * nx))
        return nz * half, (nx, 0.0, nz)
    f.box = (CX - 6, 25, CX + 6, 51)
    return f


def band(cy=33.0, rx=10.4, ry=2.1, z=20.0):
    """The ring of condensation round the stem: a flat band, its top lit, its underside toward the flash."""
    def f(u, v):
        ex, ey = (u - CX) / rx, (v - cy) / ry
        if ex * ex + ey * ey > 1:
            return None
        nz = math.sqrt(max(0.0, 1 - ex * ex - ey * ey))
        return z + nz, norm(ex * 0.5, ey, nz + 0.2)
    f.box = (CX - rx, cy - ry, CX + rx, cy + ry)
    return f


def layers():
    """The cloud's parts, back to front: the stem, the cap, the skirt of dust at the foot, the ring."""
    stem = [column()] + [sphere(CX + x, y, r, 1.5) for x, y, r in
                         ((-3.2, 37, 2.2), (3.4, 41, 2.4), (-3.6, 44, 2.4), (2.8, 30, 1.8))]
    cap = [sphere(CX + x, y, r) for x, y, r in (
        (0, 15, 10), (-10, 16, 8), (10, 16, 8), (-18, 20, 6), (18, 20, 6), (-5, 11, 6), (5, 11, 6),
        (-20.5, 24, 3.8), (20.5, 24, 3.8), (-14, 24.5, 4.6), (14, 24.5, 4.6), (-6, 24, 5.2), (6, 24, 5.2),
        (0, 20, 9))]
    # billows round the top, bigger toward the crown
    for k, (a, r) in enumerate(((196, 3.6), (214, 4.4), (236, 4.8), (258, 4.4), (282, 4.4), (304, 4.8),
                                (326, 4.4), (344, 3.6))):
        t = math.radians(a)
        cap.append(sphere(CX + math.cos(t) * 18.0, 18.0 + math.sin(t) * 9.6, r, 4 + (k % 2) * 1.2))
    # folds inside the cap and its rolled underside
    cap += [sphere(CX + x, y, r, z) for x, y, r, z in (
        (-13, 15, 5.0, 7), (13, 15, 5.0, 7), (-7, 20.5, 4.6, 8.5), (7, 20.5, 4.6, 8.5),
        (-19, 21.5, 3.6, 6), (19, 21.5, 3.6, 6), (-15, 25.6, 3.2, 7), (15, 25.6, 3.2, 7),
        (-5, 25.8, 3.4, 8), (5, 25.8, 3.4, 8))]
    skirt = [sphere(CX + x, y, r, z) for x, y, r, z in (
        (0, 48, 6.5, 2), (-7, 50, 4.6, 1), (7, 50, 4.6, 1), (-13, 51.5, 3.4, 0), (13, 51.5, 3.4, 0),
        (-18, 52.4, 2.2, 0), (18, 52.4, 2.2, 0), (-4, 45.5, 3.2, 4), (4, 45.5, 3.2, 4),
        (-10, 48.3, 2.8, 3), (10, 48.3, 2.8, 3))]
    # the ring and the puffs rolled along its front
    ring = [band()]
    for k in range(13):
        t = math.radians(-6 + k * 192 / 12)
        ring.append(sphere(CX + math.cos(t) * 8.8, 33.0 + math.sin(t) * 1.3, 2.3 + (k % 3 == 1) * 0.4, 21))
    return [stem, cap, skirt, ring]


def cloud():
    """{pixel: normal}: each layer over the ones behind it, the frontmost surface within a layer."""
    out = {}
    for layer in layers():
        best = {}
        for f in layer:
            x0, y0, x1, y1 = f.box
            for y in range(max(0, int(y0 * UNIT) - 1), min(N, int(y1 * UNIT) + 2)):
                for x in range(max(0, int(x0 * UNIT) - 1), min(N, int(x1 * UNIT) + 2)):
                    hit = f(*unit(x, y))
                    if hit and ((x, y) not in best or hit[0] > best[(x, y)][0]):
                        best[(x, y)] = hit
        out.update({k: v[1] for k, v in best.items()})
    return out


def trefoil(u, v, cx=32.0, cy=15.5, r=1.8):
    """The radiation sign: a dot and three blades 60 degrees wide, centred on 30, 150 and 270 degrees."""
    dx, dy = u - cx, cy - v
    d = math.hypot(dx, dy)
    if d <= r:
        return True
    if not 1.6 * r <= d <= 5.2 * r:
        return False
    a = math.degrees(math.atan2(dy, dx)) % 360
    return any(abs((a - c + 180) % 360 - 180) <= 30 for c in (30, 150, 270))


# ---------------------------------------------------------------- painting
def sky(p):
    """Bands of yellow round the flash, darker toward the corners, every other ray a band lighter."""
    fx, fy = FLASH_AT
    for x, y in cells():
        u, v = unit(x, y)
        dx, dy = u - fx, v - fy
        b = sum(math.hypot(dx / 1.2, dy) > r for r in (11, 16, 22, 29, 37, 46, 56))
        if b >= 2 and math.degrees(math.atan2(dy, dx)) % RAY < RAY / 2:
            b -= 1
        p[x, y] = SKY[b]


def smoke(p):
    """The cloud: smoke lit from the top left, its undersides near the flash glowing, an ink edge where it's lit
    against the sky so the silhouette stays crisp."""
    fx, fy = FLASH_AT
    puffs = cloud()
    for (x, y), n in puffs.items():
        u, v = unit(x, y)
        towards = norm(fx - u, fy - v, 5)
        glow = dot(n, towards) * max(0.0, 1.25 - math.hypot(fx - u, fy - v) / 40)
        if n[1] > 0.4 and glow > 0.42:
            p[x, y] = GLOW[min(3, int((glow - 0.42) / 0.14))]
        else:
            p[x, y] = SMOKE[sum(dot(n, L) > t for t in (0.35, 0.62, 0.84))]
    shape = set(puffs)
    for x, y in shape:
        if any((x + dx, y + dy) not in shape for dx, dy in N4) and p[x, y][:3] in SMOKE[1:]:
            p[x, y] = INK


def sign(p):
    """The trefoil cut through the cap, the top edges of its blades in the cap's shadow."""
    cut = {(x, y) for x, y in cells() if trefoil(*unit(x, y))}
    depth = max(1, int(UNIT // 2))
    for x, y in cut:
        u, v = unit(x, y)
        in_dot = math.hypot(u - 32.0, v - 15.5) <= 1.8 * 1.3
        shaded = not in_dot and any((x, y - k) not in cut for k in range(1, depth + 1))
        p[x, y] = CUT_SHADE if shaded else CUT


def ground(p):
    """The ground, the flash on it, the shock running out along it and the hazard stripes."""
    fx, fy = FLASH_AT
    for x, y in cells():
        u, v = unit(x, y)
        if v >= GROUND:
            p[x, y] = INK
            ring = ((u - fx) / 24) ** 2 + ((v - GROUND - 0.4) / 2.0) ** 2
            if abs(ring - 1) < 0.16:
                p[x, y] = SHOCK if abs(u - fx) > 14 else FLASH[3]
            if v < GROUND + 1 / UNIT and 3 <= u <= 61:
                p[x, y] = FLASH[3] if abs(u - fx) <= 20 else SHOCK
        for i, (rx, ry) in enumerate(((4.5, 1.6), (6.5, 2.3), (8.2, 3.0), (9.5, 3.5))):
            if ((u - fx) / rx) ** 2 + ((v - fy) / ry) ** 2 <= 1:
                p[x, y] = FLASH[i]
                break

    def bar(x, y):
        u, v = unit(x, y)
        return 56 <= v < 61 and int((u + v) // 4) % 2 == 0

    for x, y in cells():
        if bar(x, y):
            if not bar(x - 1, y) or not bar(x, y - 1):
                p[x, y] = STRIPE[0]
            elif not bar(x + 1, y) or not bar(x, y + 1):
                p[x, y] = STRIPE[2]
            else:
                p[x, y] = STRIPE[1]


def tile(p):
    """Round the corners and ring the tile in ink."""
    edge = max(1, round(UNIT))
    for x, y in cells():
        u, v = unit(x, y)
        cu = min(max(u, CORNER), 64 - CORNER)
        cv = min(max(v, CORNER), 64 - CORNER)
        if (u, v) == (cu, cv):
            inside = CORNER + min(u, v, 64 - u, 64 - v)
        else:
            inside = CORNER - math.hypot(u - cu, v - cv)
        if inside < 0:
            p[x, y] = CLEAR
        elif inside * UNIT < edge:
            p[x, y] = INK


def main():
    im = Image.new("RGBA", (N, N))
    p = im.load()
    sky(p)
    smoke(p)
    sign(p)
    ground(p)
    tile(p)
    im.resize((N * SCALE, N * SCALE), Image.NEAREST).save(ROOT / "icon.png", optimize=True)


if __name__ == "__main__":
    main()
