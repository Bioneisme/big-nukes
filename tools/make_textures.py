#!/usr/bin/env python3
"""Draws the mod's textures: original pixel art from fixed seeds.

    python tools/make_textures.py

Every block and item texture is drawn by a function of its size: a design is laid out in pixels of its own
texture (or on the 16 grid with `Tex.g`), and its details (outlines, bevels, rivets, seams, stencils) are drawn a
pixel at a time at the size it gets. Each `add(...)` gives a texture its size: 64x64 for the bombs, the
Sunbreaker's and the silo's parts and the key items, 32x32 for every other block and item. Each texture's random
comes from its own name, so redrawing one leaves the others the same, and a cracked wall is its wall's drawing
with cracks over it.

The materials are ramps of 5 or 6 tones lit from the top left, shadows a little cooler and lights a little
warmer; noise is soft clusters of a ramp's neighbouring tones with an ordered dither where they meet, and wear
(chipped paint, scratches, grime, soot) is drawn on top.

The screens (gui/*), the suit on a player (entity/equipment/*) and the effect icons (mob_effect/*) keep their
fixed sizes, since the code draws them by pixel coordinates: they are drawn last, pixel for pixel as before.
"""
import math
import random
import zlib
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
TEX = ROOT / "content" / "assets" / "big_nukes" / "textures"
CLEAR = (0, 0, 0, 0)


# ---------------------------------------------------------------- colours
def rgba(c):
    return tuple(int(v) for v in c[:3]) + ((int(c[3]),) if len(c) > 3 else (255,))


def shade(c, k):
    return tuple(max(0, min(255, int(v * k))) for v in c[:3]) + (255,)


def mix(a, b, t):
    return tuple(max(0, min(255, int(round(a[i] + (b[i] - a[i]) * t)))) for i in range(3)) + (255,)


COOL, WARM = (24, 28, 56), (255, 248, 226)


def ramp(base, steps=5, dark=0.5, light=0.42):
    """Dark to light round `base`, the middle tone: the shadows darker and a little cooler, the lights paler and
    warmer, the way pixel art shifts its hues."""
    mid = (steps - 1) / 2
    out = []
    for i in range(steps):
        t = (i - mid) / mid
        if t < 0:
            out.append(mix(shade(base, 1 + dark * t), COOL, -t * 0.14))
        elif t > 0:
            out.append(mix(base, WARM, light * t))
        else:
            out.append(rgba(base))
    return out


# ---------------------------------------------------------------- noise and dither
BAYER = ((0, 8, 2, 10), (12, 4, 14, 6), (3, 11, 1, 9), (15, 7, 13, 5))


def bayer(x, y):
    return (BAYER[y & 3][x & 3] + 0.5) / 16


def pick(R, i, x, y, dither=0.5):
    """The ramp's tone at index `i` (a float), dithered in an ordered pattern near the steps between tones."""
    k = int(math.floor(i + 0.5 + (bayer(x, y) - 0.5) * dither))
    return R[max(0, min(len(R) - 1, k))]


def vnoise(n, rng, cells=4, cells_y=None, octaves=2):
    """Smooth value noise, 0..1, that tiles across the texture's edges."""
    field = [[0.0] * n for _ in range(n)]
    amp, cx, cy = 1.0, cells, cells_y or cells
    for _ in range(octaves):
        grid = [[rng.random() for _ in range(cx)] for _ in range(cy)]
        for y in range(n):
            fy = y * cy / n
            y0 = int(fy)
            ty = fy - y0
            ty = ty * ty * (3 - 2 * ty)
            r0, r1 = grid[y0 % cy], grid[(y0 + 1) % cy]
            for x in range(n):
                fx = x * cx / n
                x0 = int(fx)
                tx = fx - x0
                tx = tx * tx * (3 - 2 * tx)
                a = r0[x0 % cx] + (r0[(x0 + 1) % cx] - r0[x0 % cx]) * tx
                b = r1[x0 % cx] + (r1[(x0 + 1) % cx] - r1[x0 % cx]) * tx
                field[y][x] += (a + (b - a) * ty) * amp
        amp, cx, cy = amp * 0.5, cx * 2, cy * 2
    lo = min(map(min, field))
    hi = max(map(max, field))
    return [[(v - lo) / (hi - lo) for v in row] for row in field]


# ---------------------------------------------------------------- canvas and masks
class Tex:
    def __init__(self, n, fill=None):
        self.n = n
        self.im = Image.new("RGBA", (n, n), rgba(fill) if fill else CLEAR)
        self.px = self.im.load()

    def g(self, v):
        """A length on the 16 grid, in this texture's pixels."""
        return int(round(v * self.n / 16))

    def ok(self, x, y):
        return 0 <= x < self.n and 0 <= y < self.n

    def set(self, x, y, c):
        if self.ok(x, y):
            self.px[x, y] = rgba(c)

    def get(self, x, y):
        return self.px[x % self.n, y % self.n]

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1):
            for x in range(x0, x1):
                self.set(x, y, c)

    def paint(self, mask, c):
        for x, y in mask:
            self.set(x, y, c)

    def mul(self, x, y, k):
        if self.ok(x, y):
            c = self.px[x, y]
            if c[3]:
                self.px[x, y] = shade(c, k)[:3] + (c[3],)

    def tint(self, x, y, c, a):
        if self.ok(x, y):
            o = self.px[x, y]
            if o[3]:
                self.px[x, y] = mix(o, c, a)[:3] + (o[3],)

    def solid(self):
        return {(x, y) for y in range(self.n) for x in range(self.n) if self.px[x, y][3] >= 128}

    def save(self, kind, name):
        path = TEX / kind / f"{name}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        self.im.save(path)


N4 = ((1, 0), (-1, 0), (0, 1), (0, -1))


def box(x0, y0, x1, y1):
    return {(x, y) for y in range(y0, y1) for x in range(x0, x1)}


def disc(cx, cy, r, r0=0.0):
    """Pixels whose centres lie between r0 and r of (cx, cy), in pixel-edge coordinates."""
    out = set()
    for y in range(int(math.floor(cy - r)) - 1, int(math.ceil(cy + r)) + 1):
        for x in range(int(math.floor(cx - r)) - 1, int(math.ceil(cx + r)) + 1):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            if r0 <= d < r:
                out.add((x, y))
    return out


def ellipse(cx, cy, rx, ry):
    return {(x, y) for y in range(int(cy - ry) - 1, int(cy + ry) + 2) for x in range(int(cx - rx) - 1, int(cx + rx) + 2)
            if ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 < 1}


def rrect(x0, y0, x1, y1, r):
    """A box with its corners rounded by r."""
    out = set()
    for y in range(y0, y1):
        for x in range(x0, x1):
            qx = min(x + 0.5 - x0, x1 - x - 0.5)
            qy = min(y + 0.5 - y0, y1 - y - 0.5)
            if qx < r and qy < r and math.hypot(r - qx, r - qy) > r:
                continue
            out.add((x, y))
    return out


def poly(pts):
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    out = set()
    for y in range(int(min(ys)) - 1, int(max(ys)) + 2):
        for x in range(int(min(xs)) - 1, int(max(xs)) + 2):
            px, py = x + 0.5, y + 0.5
            inside = False
            j = len(pts) - 1
            for i in range(len(pts)):
                xi, yi = pts[i]
                xj, yj = pts[j]
                if (yi > py) != (yj > py) and px < (xj - xi) * (py - yi) / (yj - yi) + xi:
                    inside = not inside
                j = i
            if inside:
                out.add((x, y))
    return out


def border(mask):
    """The pixels just outside a mask, side by side with it."""
    return {(x + dx, y + dy) for x, y in mask for dx, dy in N4 if (x + dx, y + dy) not in mask}


def trefoil(cx, cy, R):
    """The radiation sign: a hub and three blades, one down and two up, R its outer radius."""
    out = set()
    for y in range(int(cy - R) - 1, int(cy + R) + 2):
        for x in range(int(cx - R) - 1, int(cx + R) + 2):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            r = math.hypot(dx, dy)
            a = (math.degrees(math.atan2(dy, dx)) - 60) % 120
            if r < 0.21 * R or (0.32 * R < r < R and a < 60):
                out.add((x, y))
    return out


# ---------------------------------------------------------------- shading
LIGHT = (-0.52, -0.6, 0.6)
_l = math.sqrt(sum(v * v for v in LIGHT))
LX, LY, LZ = (v / _l for v in LIGHT)
LIGHT_A = math.atan2(-1, -1)  # the light's direction from a shape's middle: up and left


def emboss(t, mask, R, mid=2.0, depth=1, sunk=False, field=None, spread=0.0, dither=0.0):
    """Fills a mask from ramp R: a tone lighter along its top-left edge, a tone darker along its bottom-right
    (the other way round when it is sunk into the surface)."""
    s = -1 if sunk else 1
    for x, y in mask:
        v = mid
        if field is not None:
            v += (field[y % len(field)][x % len(field)] - 0.5) * spread
        lit = any((x - k, y) not in mask or (x, y - k) not in mask for k in range(1, depth + 1))
        dim = any((x + k, y) not in mask or (x, y + k) not in mask for k in range(1, depth + 1))
        if lit and not dim:
            v += s
        elif dim and not lit:
            v -= s
        t.set(x, y, pick(R, v, x, y, dither))


def sphere(t, mask, cx, cy, r, R, lo=0.0, hi=None, dither=0.3, spec=True):
    hi = len(R) - 1 if hi is None else hi
    for x, y in mask:
        nx, ny = (x + 0.5 - cx) / r, (y + 0.5 - cy) / r
        nz = math.sqrt(max(0.0, 1 - nx * nx - ny * ny))
        l = nx * LX + ny * LY + nz * LZ
        v = lo + (hi - lo) * (l * 0.5 + 0.5)
        c = pick(R, v, x, y, dither)
        if spec and l > 0.965:
            c = R[-1]
        t.set(x, y, c)


def cylinder(t, mask, x0, x1, R, lo=0.0, hi=None, dither=0.2):
    """A vertical cylinder's shading across x0..x1."""
    hi = len(R) - 1 if hi is None else hi
    w = (x1 - x0) / 2
    for x, y in mask:
        nx = (x + 0.5 - x0 - w) / w
        nz = math.sqrt(max(0.0, 1 - nx * nx))
        l = nx * LX + nz * LZ
        t.set(x, y, pick(R, lo + (hi - lo) * (l * 0.55 + 0.45), x, y, dither))


def ring(t, cx, cy, r0, r1, R, mid=2.0, amp=1.3, dither=0.5, raised=True):
    """A round rim between two radii: its outer slope lit at the top left, its inner slope at the bottom right."""
    rm = (r0 + r1) / 2
    for x, y in disc(cx, cy, r1, r0):
        dx, dy = x + 0.5 - cx, y + 0.5 - cy
        facing = math.cos(math.atan2(dy, dx) - LIGHT_A)
        slope = 1 if math.hypot(dx, dy) >= rm else -1
        if not raised:
            slope = -slope
        t.set(x, y, pick(R, mid + amp * facing * slope, x, y, dither))


def outline(t, mask, c):
    for x, y in border(mask):
        t.set(x, y, c)


def drop(t, mask, k=0.7, dx=1, dy=1):
    """A shadow a pixel down and right of a raised shape."""
    for x, y in mask:
        if (x + dx, y + dy) not in mask:
            t.mul(x + dx, y + dy, k)


def lines_box(t, x0, y0, x1, y1, hi, lo, c_tr=None):
    """A box's 1-pixel edges: top and left in `hi`, bottom and right in `lo` (x1, y1 exclusive)."""
    for x in range(x0, x1):
        t.set(x, y0, hi)
        t.set(x, y1 - 1, lo)
    for y in range(y0, y1):
        t.set(x0, y, hi)
        t.set(x1 - 1, y, lo)
    if c_tr:
        t.set(x1 - 1, y0, c_tr)
        t.set(x0, y1 - 1, c_tr)


def fill(t, mask, R, rng, mid=2.0, spread=1.0, cells=4, grain=0.25, dither=0.15, cells_y=None):
    """A material: soft clusters of the ramp's neighbouring tones round `mid`, and a fine grain."""
    n = t.n
    f = vnoise(n, rng, cells, cells_y)
    g = vnoise(n, rng, max(2, n // 4), octaves=1)
    for x, y in mask:
        v = mid + (f[y % n][x % n] - 0.5) * spread * 2 + (g[y % n][x % n] - 0.5) * grain
        t.set(x, y, pick(R, v, x, y, dither))
    return f


def casing(t, rng, base, edge=None, region=None, R=None, spread=0.8, cells=4, bevel=True, grain=0.2, dark=0.5,
           light=0.42):
    """A machine's plate: a material, its rim's dark edge and a bevel lit from the top left."""
    n = t.n
    x0, y0, x1, y1 = region or (0, 0, n, n)
    R = R or ramp(base, dark=dark, light=light)
    fill(t, box(x0, y0, x1, y1), R, rng, spread=spread, cells=cells, grain=grain)
    if edge:
        lines_box(t, x0, y0, x1, y1, edge, edge)
        x0, y0, x1, y1 = x0 + 1, y0 + 1, x1 - 1, y1 - 1
    if bevel:
        lines_box(t, x0, y0, x1, y1, R[3], R[1], R[2])
    return R


def groove(t, x0, y0, x1, y1, R, vertical=None):
    """A seam between plates: a dark line with a lit lip after it."""
    if vertical is None:
        vertical = (x1 - x0) < (y1 - y0)
    if vertical:
        for y in range(y0, y1):
            t.set(x0, y, R[0])
            t.set(x0 + 1, y, R[3])
    else:
        for x in range(x0, x1):
            t.set(x, y0, R[0])
            t.set(x, y0 + 1, R[3])


def rivet(t, x, y, R, size=None):
    """A domed rivet with its top-left corner at (x, y), and its shadow."""
    size = size or (2 if t.n <= 32 else 3)
    if size == 2:
        t.set(x, y, R[4])
        t.set(x + 1, y, R[3])
        t.set(x, y + 1, R[3])
        t.set(x + 1, y + 1, R[1])
        t.mul(x + 2, y + 1, 0.8)
        t.mul(x + 1, y + 2, 0.8)
        t.mul(x + 2, y + 2, 0.75)
    else:
        pat = ((None, 4, 3, None), (4, 3, 2, 1), (3, 2, 1, 0), (None, 1, 0, None))
        for dy, row in enumerate(pat[:size + 1] if size == 3 else pat):
            for dx, i in enumerate(row):
                if i is not None:
                    t.set(x + dx, y + dy, R[i])
        for dx, dy in ((4, 2), (4, 3), (3, 4), (2, 4)):
            t.mul(x + dx, y + dy, 0.75)


def screw(t, x, y, R):
    """A slotted screw head, 4 pixels."""
    m = disc(x + 2, y + 2, 2.1)
    sphere(t, m, x + 2, y + 2, 2.1, R, lo=1, spec=False)
    t.set(x + 1, y + 2, R[0])
    t.set(x + 2, y + 1, R[0])
    drop(t, m, 0.75)


def hazard(t, x0, y0, x1, y1, period=None, rng=None, chips=0, rims=True, flip=False):
    """Yellow and black stripes at 45 degrees, a lit top row and a shaded bottom one, paint chipped off here
    and there."""
    period = period or max(4, 2 * (y1 - y0))
    for y in range(y0, y1):
        for x in range(x0, x1):
            d = ((x + y) if flip else (x - y)) % period
            R = HAZ if d < period // 2 else INK
            i = 2
            if y == y0:
                i = 3
            elif y == y1 - 1:
                i = 1
            t.set(x, y, R[i])
    if rims:
        for x in range(x0, x1):
            t.mul(x, y0 - 1, 0.55)
            t.mul(x, y1, 0.7)
    if rng and chips:
        wear_chips(t, rng, chips, x0, y0, x1, y1)


def wear_chips(t, rng, count, x0, y0, x1, y1, metal=None):
    """Chips of paint gone down to bare steel, with the paint's lip shading them."""
    metal = metal or STEEL
    for _ in range(count):
        x, y = rng.randrange(x0, x1), rng.randrange(y0, y1)
        blob = {(x, y)}
        for _ in range(rng.randrange(1, 4 if t.n <= 32 else 6)):
            bx, by = rng.choice(sorted(blob))
            dx, dy = rng.choice(N4)
            if x0 <= bx + dx < x1 and y0 <= by + dy < y1:
                blob.add((bx + dx, by + dy))
        for bx, by in blob:
            t.set(bx, by, metal[3] if (bx + by) % 3 else metal[2])
        for bx, by in blob:
            if (bx, by - 1) not in blob:
                t.set(bx, by, metal[1])
            if (bx + 1, by + 1) not in blob:
                t.mul(bx + 1, by + 1, 1.12)


def scratches(t, rng, count, x0, y0, x1, y1, k=1.22, length=(2, 5)):
    """Thin bright scratches, each with a darker pixel under its start."""
    for _ in range(count):
        x, y = rng.randrange(x0, x1), rng.randrange(y0, y1)
        dx, dy = rng.choice(((1, 0), (1, 1), (1, -1), (2, 1), (2, -1)))
        steps = rng.randrange(*length)
        t.mul(x, y + 1, 0.85)
        for _ in range(steps):
            if not (x0 <= x < x1 and y0 <= y < y1):
                break
            t.mul(x, y, k)
            x += 1 if dx else 0
            if dx == 2:
                if rng.random() < 0.5:
                    y += dy
            else:
                y += dy


def grime(t, rng, x0, y0, x1, y1, k=0.88, strength=0.6, power=1.8, up=False):
    """Dirt gathering toward the bottom (or top), laid down as a broken dither."""
    h = max(1, y1 - y0)
    f = vnoise(t.n, rng, max(2, t.n // 6), octaves=2)
    for y in range(y0, y1):
        frac = (y - y0 + 1) / h if not up else (y1 - y) / h
        p = strength * frac ** power
        for x in range(x0, x1):
            if bayer(x, y) * 0.3 + f[y % t.n][x % t.n] * 0.7 < p:
                t.mul(x, y, k)


def soot(t, rng, cx, cy, r, k=0.7, strength=0.9):
    """A scorch: darkening that thins out from (cx, cy) as a broken dither."""
    f = vnoise(t.n, rng, max(2, t.n // 8), octaves=1)
    for x, y in disc(cx, cy, r):
        p = strength * (1 - math.hypot(x + 0.5 - cx, y + 0.5 - cy) / r) ** 1.2
        if bayer(x, y) * 0.3 + f[y % t.n][x % t.n] * 0.7 < p:
            t.mul(x, y, k)


def item_outline(t, k=0.32):
    """Vanilla items' 1-pixel dark outline, in a darker tone of whatever it borders."""
    solid = t.solid()
    for x, y in sorted(border(solid)):
        if not t.ok(x, y):
            continue
        near = [t.px[x + dx, y + dy] for dx, dy in N4 if (x + dx, y + dy) in solid]
        c = min(near, key=lambda c: sum(c[:3]))
        t.set(x, y, mix(shade(c, k), (16, 14, 20), 0.35))


def halo(t, colour, alphas):
    """A soft glow round an item: rings of a colour, fading."""
    for a in alphas:
        solid = {(x, y) for y in range(t.n) for x in range(t.n) if t.px[x, y][3] > 0}
        for x, y in border(solid):
            if t.ok(x, y):
                t.px[x, y] = rgba(colour)[:3] + (a,)


# ---------------------------------------------------------------- stencils
_GLYPHS = """
A .#. #.# ### #.# #.#   B ##. #.# ##. #.# ##.   C .## #.. #.. #.. .##   D ##. #.# #.# #.# ##.
E ### #.. ##. #.. ###   F ### #.. ##. #.. #..   G .## #.. #.# #.# .##   H #.# #.# ### #.# #.#
I ### .#. .#. .#. ###   J ..# ..# ..# #.# .#.   K #.# #.# ##. #.# #.#   L #.. #.. #.. #.. ###
M #.# ### ### #.# #.#   N ##. #.# #.# #.# #.#   O .#. #.# #.# #.# .#.   P ##. #.# ##. #.. #..
Q .#. #.# #.# ##. .##   R ##. #.# ##. #.# #.#   S .## #.. .#. ..# ##.   T ### .#. .#. .#. .#.
U #.# #.# #.# #.# ###   V #.# #.# #.# #.# .#.   W #.# #.# ### ### #.#   X #.# #.# .#. #.# #.#
Y #.# #.# .#. .#. .#.   Z ### ..# .#. #.. ###   0 ### #.# #.# #.# ###   1 .#. ##. .#. .#. ###
2 ##. ..# .#. #.. ###   3 ##. ..# .#. ..# ##.   4 #.# #.# ### ..# ..#   5 ### #.. ##. ..# ##.
6 .## #.. ### #.# ###   7 ### ..# .#. .#. .#.   8 ### #.# ### #.# ###   9 ### #.# ### ..# ##.
- ... ... ### ... ...   . ... ... ... ... .#.   / ..# ..# .#. #.. #..   : ... .#. ... .#. ...
"""
FONT = {" ": "." * 15}
for _row in _GLYPHS.strip().splitlines():
    _f = _row.split()
    for _i in range(0, len(_f), 6):
        FONT[_f[_i]] = "".join(_f[_i + 1:_i + 6])
assert all(len(v) == 15 for v in FONT.values())


def text(t, x, y, s, c, scale=1, rng=None, worn=0.0):
    """Stencils `s` in the 3x5 font; `worn` knocks out a share of its paint."""
    for ch in s:
        glyph = FONT[ch]
        for i, b in enumerate(glyph):
            if b != "#":
                continue
            gx, gy = i % 3, i // 3
            for sy in range(scale):
                for sx in range(scale):
                    if rng is None or rng.random() >= worn:
                        t.set(x + gx * scale + sx, y + gy * scale + sy, c)
        x += 4 * scale


def text_w(s, scale=1):
    return (4 * len(s) - 1) * scale


# ---------------------------------------------------------------- palettes
EDGE = (40, 42, 48, 255)
STEEL = ramp((110, 116, 126))
DARK_STEEL = ramp((66, 70, 78), dark=0.45)
INK = [(12, 12, 15, 255), (20, 20, 24, 255), (30, 30, 34, 255), (46, 46, 52, 255), (66, 66, 74, 255)]
HAZ = [(122, 86, 14, 255), (186, 140, 22, 255), (226, 186, 40, 255), (242, 212, 84, 255), (252, 236, 150, 255)]
URAN = [(14, 46, 20, 255), (30, 92, 30, 255), (56, 148, 40, 255), (96, 204, 58, 255), (156, 240, 96, 255),
        (226, 255, 186, 255)]
GLOW = [(18, 54, 32, 255), (34, 110, 62, 255), (60, 176, 96, 255), (120, 255, 150, 255), (210, 255, 220, 255)]
RED = ramp((200, 40, 34), dark=0.55, light=0.45)
REDGLOW = [(70, 12, 10, 255), (140, 24, 18, 255), (210, 44, 32, 255), (250, 92, 64, 255), (255, 176, 140, 255),
           (255, 236, 220, 255)]
GLASS = [(18, 34, 42, 255), (30, 56, 68, 255), (46, 86, 100, 255), (84, 136, 150, 255), (150, 200, 210, 255),
         (226, 246, 250, 255)]
BRASS = ramp((190, 150, 64), dark=0.5, light=0.5)


# ---------------------------------------------------------------- registry
TEXTURES = []


def add(kind, name, size, fn, *args):
    TEXTURES.append((kind, name, size, fn, args))


def seeded(name):
    return random.Random(zlib.crc32(name.encode()))


# ================================================================ blocks: the ores and ash
def stone(t, rng, base, streaky=False):
    n = t.n
    R = ramp(base, dark=0.48, light=0.3)
    f = vnoise(n, rng, 3, 9) if streaky else vnoise(n, rng, 4)
    g = vnoise(n, rng, n // 4, n // 8 if streaky else None, octaves=1)
    for y in range(n):
        for x in range(n):
            v = 2 + (f[y][x] - 0.5) * 2.4 + (g[y][x] - 0.5) * 1.0
            t.set(x, y, pick(R, v, x, y, 0.55))
    # Dark pits and short fractures, the way stone shows its grain.
    for _ in range(n // 3):
        x, y = rng.randrange(n), rng.randrange(n)
        t.set(x, y, R[0] if rng.random() < 0.5 else R[1])
        t.mul((x + 1) % n, (y + 1) % n, 1.12)
    for _ in range(3):
        x, y = rng.randrange(n), rng.randrange(n)
        for _ in range(rng.randrange(3, 6)):
            t.set(x % n, y % n, R[1])
            x += 1
            y += rng.choice((0, 0, 1, -1)) if not streaky else rng.choice((0, 0, 0, 1))
    if streaky:
        for _ in range(4):
            y = rng.randrange(n)
            x = rng.randrange(n)
            for _ in range(rng.randrange(6, 14)):
                t.set(x % n, y, R[0] if rng.random() < 0.6 else R[1])
                x += 1
    return R


def uranium_grains(t, rng, clusters):
    """Clusters of faceted green grains, lit top left, sunk in the stone with a glow round them."""
    n = t.n
    centres = []
    while len(centres) < clusters:
        c = (rng.uniform(4, n - 5), rng.uniform(4, n - 5))
        if all(math.hypot(c[0] - a, c[1] - b) > n / 3.2 for a, b in centres):
            centres.append(c)
    allm = set()
    grains = []
    for cx, cy in centres:
        for _ in range(rng.randrange(2, 4)):
            gx, gy = cx + rng.uniform(-2.5, 2.5), cy + rng.uniform(-2.5, 2.5)
            r = rng.uniform(1.4, 2.5)
            k = rng.randrange(5, 7)
            pts = [(gx + math.cos(2 * math.pi * i / k + rng.uniform(-0.3, 0.3)) * r * rng.uniform(0.8, 1.25),
                    gy + math.sin(2 * math.pi * i / k + rng.uniform(-0.3, 0.3)) * r * rng.uniform(0.8, 1.25))
                   for i in range(k)]
            m = poly(pts) | {(int(gx), int(gy))}
            grains.append((gx, gy, r, m))
            allm |= m
    for x, y in border(allm):
        t.tint(x % n, y % n, URAN[1], 0.45)
    for x, y in border(allm):
        if (x - 1, y - 1) in allm:
            t.set(x % n, y % n, URAN[0])
    for gx, gy, r, m in grains:
        for x, y in m:
            v = -((x + 0.5 - gx) + (y + 0.5 - gy)) / (r * 1.4)
            t.set(x % n, y % n, pick(URAN, 2.8 + v * 1.6, x, y, 0.4))
        tl = min(m, key=lambda p: p[0] + p[1])
        t.set(tl[0] % n, tl[1] % n, URAN[5])
    for _ in range(clusters):
        cx, cy = rng.choice(centres)
        x, y = int(cx + rng.uniform(-5, 5)), int(cy + rng.uniform(-5, 5))
        if (x, y) not in allm:
            t.set(x % n, y % n, URAN[4])


def uranium_ore(t, rng):
    stone(t, rng, (125, 125, 125))
    uranium_grains(t, rng, 5)


def deepslate_uranium_ore(t, rng):
    stone(t, rng, (78, 78, 86), streaky=True)
    uranium_grains(t, rng, 5)


def ash(t, rng):
    n = t.n
    R = ramp((58, 58, 58), dark=0.5, light=0.22)
    f = vnoise(n, rng, 5)
    g = vnoise(n, rng, n // 2, octaves=1)
    for y in range(n):
        for x in range(n):
            t.set(x, y, pick(R, 2 + (f[y][x] - 0.5) * 1.5 + (g[y][x] - 0.5) * 1.3, x, y, 0.3))
    for _ in range(n):
        x, y = rng.randrange(n), rng.randrange(n)
        if rng.random() < 0.55:
            t.set(x, y, (100, 99, 97, 255))
            t.set(x, (y + 1) % n, R[1])
        else:
            t.set(x, y, R[0])
    # Pale flakes of burnt-out stuff and a few still-dark cinders.
    for _ in range(n // 4):
        x, y = rng.randrange(n), rng.randrange(n)
        t.set(x, y, (128, 126, 122, 255))
        t.set((x + 1) % n, y, (100, 99, 97, 255))
        t.set(x, (y + 1) % n, R[1])
        t.set((x + 1) % n, (y + 1) % n, R[0])


add("block", "uranium_ore", 32, uranium_ore)
add("block", "deepslate_uranium_ore", 32, deepslate_uranium_ore)
add("block", "ash", 32, ash)


# ================================================================ blocks: the machines
def label_plate(t, x0, y0, x1, y1, R=None):
    """A small enamel plate held on by its corners."""
    R = R or HAZ
    t.rect(x0, y0, x1, y1, R[2])
    lines_box(t, x0, y0, x1, y1, R[3], R[1])
    for x in range(x0 - 1, x1 + 1):
        t.mul(x, y0 - 1, 0.7)
        t.mul(x, y1, 0.6)
    for y in range(y0, y1):
        t.mul(x0 - 1, y, 0.7)
        t.mul(x1, y, 0.6)


def centrifuge(t, rng, part):
    n = t.n
    R = casing(t, rng, (110, 116, 126), EDGE)
    m = n // 2
    if part == "side":
        groove(t, m - 1, 2, m, n - 2, R, vertical=True)
        for x, y in ((3, 3), (m - 5, 3), (3, n - 6), (m - 5, n - 6), (m + 3, 3), (n - 6, 3), (m + 3, n - 6), (n - 6, n - 6)):
            rivet(t, x, y, R)
        # Vents on the right plate.
        for k in range(5):
            y = 8 + k * 3
            for x in range(m + 4, n - 5):
                t.set(x, y, INK[1])
                t.set(x, y + 1, R[3])
            t.set(m + 3, y, R[1])
        # A warning plate on the left one.
        label_plate(t, 4, 6, m - 4, 18)
        t.paint(trefoil(m / 2, 12, 5.2), INK[2])
        text(t, 4, 21, "UF6", R[0])
        grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.5)
        scratches(t, rng, 5, 2, 2, n - 2, n - 2)
    elif part == "front":
        cx = cy = n / 2
        rg, rr = n * 0.29, n * 0.41
        ring(t, cx, cy, rg, rr, R, mid=2.2)
        for i in range(8):
            a = 2 * math.pi * i / 8 + math.pi / 8
            rivet(t, int(cx + math.cos(a) * (rr - 2.4) - 1), int(cy + math.sin(a) * (rr - 2.4) - 1), R, 2)
        for x, y in disc(cx, cy, rg):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            d = math.hypot(dx, dy)
            a = math.atan2(dy, dx)
            swirl = math.sin(a * 2 + d * 0.65)
            v = 1.6 + 0.8 * swirl + (1 - d / rg) * 1.9
            c = pick(URAN, v, x, y, 0.5)
            if d > rg - 1.2:
                c = INK[1] if dy < 0.8 * -dx + 2 else URAN[1]
            t.set(x, y, c)
        for x, y in disc(cx, cy, rg * 0.78, rg * 0.58):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            a = math.degrees(math.atan2(dy, dx)) % 360
            if 200 < a < 245:
                t.set(x, y, URAN[5])
        # A lamp and a tag under the glass.
        t.set(3, n - 5, URAN[4])
        t.set(4, n - 5, URAN[3])
        t.set(3, n - 4, URAN[3])
        t.set(4, n - 4, URAN[1])
        grime(t, rng, 1, n * 3 // 4, n - 1, n - 1, strength=0.45)
    else:
        cx = cy = n / 2
        ring(t, cx, cy, n * 0.36, n * 0.44, R, mid=2.2)
        emboss(t, disc(cx, cy, n * 0.36), R, 1.6, sunk=True)
        # A cross brace over the lid, and the rotor's hub.
        bar = box(2, m - 2, n - 2, m + 2) | box(m - 2, 2, m + 2, n - 2)
        drop(t, bar, 0.6)
        emboss(t, bar, R, 2.2)
        hub = disc(cx, cy, 4.2)
        drop(t, hub, 0.6)
        sphere(t, hub, cx, cy, 4.2, R, lo=0.6, spec=True)
        for x, y in ((m - 1, 3), (m - 1, n - 5), (3, m - 1), (n - 5, m - 1)):
            rivet(t, x, y, R, 2)
        for x, y in ((3, 3), (n - 5, 3), (3, n - 5), (n - 5, n - 5)):
            rivet(t, x, y, R, 2)


for part in ("top", "side", "front"):
    add("block", f"centrifuge_{part}", 32, centrifuge, part)


def assembly_table(t, rng, part):
    n = t.n
    if part == "top":
        R = casing(t, rng, (150, 152, 158), EDGE, spread=0.55)
        a, b = n * 5 // 16, n * 10 // 16 + 1
        for c in (a, b):
            groove(t, c, 1, c + 1, n - 1, R, vertical=True)
            groove(t, 1, c, n - 1, c + 1, R, vertical=False)
        for cx in (a, b):
            for cy in (a, b):
                t.set(cx, cy, INK[1])
                t.set(cx + 1, cy, INK[2])
                t.set(cx, cy + 1, INK[2])
        # Oil, a burn and the marks of tools on the plates.
        soot(t, rng, n * 0.74, n * 0.28, 4.5, 0.82, 0.7)
        scratches(t, rng, 12, 2, 2, n - 2, n - 2, k=1.15)
        for _ in range(4):
            x, y = rng.randrange(3, n - 3), rng.randrange(3, n - 3)
            t.set(x, y, R[0])
    elif part == "side":
        R = casing(t, rng, (120, 124, 130), EDGE)
        # A drawer with its handle.
        x0, y0, x1, y1 = 4, 4, n - 4, n * 15 // 32
        lines_box(t, x0, y0, x1, y1, R[0], R[4])
        lines_box(t, x0 + 1, y0 + 1, x1 - 1, y1 - 1, R[3], R[1])
        hy = (y0 + y1) // 2 - 1
        handle = box(n // 2 - 5, hy, n // 2 + 5, hy + 2)
        drop(t, handle, 0.55)
        emboss(t, handle, STEEL, 3)
        t.set(n // 2 - 6, hy, R[0])
        t.set(n // 2 + 5, hy, R[0])
        hazard(t, 1, n * 3 // 4, n - 1, n * 3 // 4 + 4, period=8, rng=rng, chips=2)
        for x in (3, n - 5):
            rivet(t, x, n * 3 // 4 - 4, R, 2)
        grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.4)
        scratches(t, rng, 4, 2, 2, n - 2, n - 2)
    else:
        R = casing(t, rng, (70, 72, 78), EDGE)
        for i in range(2, n - 2):
            for x, y in ((i, i), (n - 1 - i, i)):
                t.set(x, y, R[3])
                t.set(x + 1, y, R[1])
        for x, y in ((3, 3), (n - 5, 3), (3, n - 5), (n - 5, n - 5)):
            rivet(t, x, y, R, 2)


for part in ("top", "side", "bottom"):
    add("block", f"assembly_table_{part}", 32, assembly_table, part)


# ================================================================ blocks: the bombs (64)
BOMBS = {
    "demolition_charge": ((168, 40, 34), HAZ, "DEMO", "HIGH EXPL"),
    "tactical_warhead": ((88, 98, 60), HAZ, "TAC-2", "FIELD MK-3"),
    "atomic_bomb": ((214, 176, 40), INK, "U-235", "MK-1 BOMB"),
}


def bomb_body(t, rng, body):
    n = t.n
    R = ramp(body, dark=0.55, light=0.4)
    fill(t, box(0, 0, n, n), R, rng, spread=0.45, cells=3, grain=0.35)
    edge = mix(shade(body, 0.32), COOL, 0.2)
    lines_box(t, 0, 0, n, n, edge, edge)
    lines_box(t, 1, 1, n - 1, n - 1, R[4], R[0], R[2])
    lines_box(t, 2, 2, n - 2, n - 2, R[3], R[1], R[2])
    return R, edge


def bomb(t, rng, name, part):
    n = t.n
    body, mark, label, small = BOMBS[name]
    R, edge = bomb_body(t, rng, body)
    if part == "side":
        # Two steel collars with hazard bands, riveted to the casing.
        for y0 in (5, n - 17):
            collar = box(3, y0, n - 3, y0 + 12)
            emboss(t, collar, DARK_STEEL, 2.2)
            hazard(t, 3, y0 + 3, n - 3, y0 + 9, period=12, rng=rng, chips=4)
            for x in range(6, n - 6, 9):
                t.set(x, y0 + 1, DARK_STEEL[4])
                t.set(x + 1, y0 + 1, DARK_STEEL[3])
                t.set(x, y0 + 10, DARK_STEEL[3])
                t.set(x + 1, y0 + 10, DARK_STEEL[1])
            for x in range(3, n - 3):
                t.mul(x, y0 + 12, 0.62)
        # Seams down the casing between the collars, riveted.
        for sx in (9, n - 11):
            groove(t, sx, 17, sx + 1, n - 17, R, vertical=True)
            for y in range(21, n - 20, 7):
                rivet(t, sx - 4 if sx < n // 2 else sx + 3, y, R, 3)
        # The stencil.
        w = text_w(label, 2)
        text(t, (n - w) // 2 + 1, 23, label, mark[2], 2, rng, 0.07)
        w2 = text_w(small)
        text(t, (n - w2) // 2 + 1, 35, small, shade(mark[2], 0.95) if mark is HAZ else mark[3], 1, rng, 0.05)
        serial = "LOT " + str(zlib.crc32(name.encode()) % 9000 + 1000)
        text(t, (n - text_w(serial)) // 2 + 1, 41, serial, shade(mark[2], 0.8) if mark is HAZ else mark[3], 1, rng, 0.1)
        wear_chips(t, rng, 7, 3, 17, n - 3, n - 17)
        scratches(t, rng, 10, 4, 18, n - 4, n - 18)
        grime(t, rng, 3, n // 2, n - 3, n - 17, strength=0.45)
    elif part == "front":
        cx = cy = n / 2
        disc_bg = R if mark is INK else ramp(shade(body, 1.18)[:3], dark=0.55, light=0.4)
        plate = disc(cx, cy, 20.5)
        fill(t, plate, disc_bg, rng, mid=2.2, spread=0.35, cells=3, grain=0.3)
        for x, y in disc(cx, cy, 20.5, 19.5):
            if (x + 0.5 - cx) + (y + 0.5 - cy) < 0:
                t.set(x, y, disc_bg[0])
        ring(t, cx, cy, 20.5, 26, STEEL, mid=2.0)
        for i in range(10):
            a = 2 * math.pi * i / 10 - math.pi / 2
            rivet(t, int(round(cx + math.cos(a) * 23.2 - 1.5)), int(round(cy + math.sin(a) * 23.2 - 1.5)), STEEL, 3)
        tm = trefoil(cx, cy, 16.5)
        drop(t, tm, 0.72)
        emboss(t, tm, mark, 2)
        for x, y in ((5, 5), (n - 8, 5), (5, n - 8), (n - 8, n - 8)):
            rivet(t, x, y, R, 3)
        wear_chips(t, rng, 6, 3, 3, n - 3, n - 3)
        scratches(t, rng, 8, 3, 3, n - 3, n - 3)
        grime(t, rng, 3, n * 2 // 3, n - 3, n - 3, strength=0.4)
    else:
        cx = cy = n / 2
        # A seam round the fuze housing, and lugs to lift it by.
        for sx in range(3, n - 3):
            t.set(sx, n // 2 - 1, R[0])
            t.set(sx, n // 2, R[3])
        well = box(21, 21, n - 21, n - 21)
        t.rect(19, 19, n - 19, n - 19, DARK_STEEL[2])
        lines_box(t, 19, 19, n - 19, n - 19, DARK_STEEL[4], DARK_STEEL[0])
        lines_box(t, 20, 20, n - 20, n - 20, DARK_STEEL[3], DARK_STEEL[1])
        emboss(t, well, INK, 2, sunk=True)
        ring(t, cx, cy, 6.5, 8.5, RED, mid=2)
        sphere(t, disc(cx, cy, 6.5), cx, cy, 6.5, STEEL, lo=0.5)
        t.paint(disc(cx, cy, 1.5), INK[1])
        for x, y in ((22, 22), (n - 25, 22), (22, n - 25), (n - 25, n - 25)):
            rivet(t, x, y, STEEL, 3)
        for ly in (9, n - 10):
            lug = disc(cx, ly, 5.5, 2.6)
            drop(t, lug, 0.55)
            ring(t, cx, ly, 2.6, 5.5, STEEL, mid=2.2)
            t.paint(disc(cx, ly, 2.6), shade(R[1], 0.7))
        for x, y in ((6, 6), (n - 9, 6), (6, n - 9), (n - 9, n - 9)):
            rivet(t, x, y, R, 3)
        wear_chips(t, rng, 6, 3, 3, n - 3, n - 3)
        scratches(t, rng, 10, 3, 3, n - 3, n - 3)


for name in BOMBS:
    for part in ("top", "side", "front"):
        add("block", f"{name}_{part}", 64, bomb, name, part)


# ================================================================ blocks: the Sunbreaker (64)
CORE = ramp((36, 36, 44), dark=0.55, light=0.3)


def core(t, rng, part):
    n = t.n
    edge = (12, 12, 16, 255)
    fill(t, box(0, 0, n, n), CORE, rng, spread=0.4, cells=3, grain=0.4, dither=0.4)
    cuts = [20, 42]
    spans = [(1, 20), (22, 42), (44, n - 1)]
    rows = spans if part != "front" else [(1, 20), (22, 42), (44, n - 1)]
    cols = spans if part != "front" else [(1, n - 1)]
    # Each armour plate: bevelled, bolted, some with vents.
    for px0, px1 in cols:
        for py0, py1 in rows:
            lines_box(t, px0, py0, px1, py1, CORE[4], CORE[0], CORE[2])
            lines_box(t, px0 + 1, py0 + 1, px1 - 1, py1 - 1, CORE[3], CORE[1], CORE[2])
            for bx, by in ((px0 + 3, py0 + 3), (px1 - 6, py0 + 3), (px0 + 3, py1 - 6), (px1 - 6, py1 - 6)):
                rivet(t, bx, by, CORE, 3)
            kind = rng.random()
            if kind < 0.35 and part != "front":
                for k in range(3):
                    y = py0 + 7 + k * 3
                    for x in range(px0 + 7, px1 - 7):
                        t.set(x, y, INK[0])
                        t.set(x, y + 1, CORE[3])
            elif kind < 0.7:
                scratches(t, rng, 3, px0 + 2, py0 + 2, px1 - 2, py1 - 2, k=1.3)
    lines_box(t, 0, 0, n, n, edge, edge)

    # The seams between them, glowing in runs of brighter and dimmer light.
    def seam(c, vertical):
        i = 0
        while i < n:
            run = rng.randrange(3, 10)
            lvl = 3 if rng.random() < 0.35 else 2
            for j in range(i, min(n, i + run)):
                if vertical:
                    t.set(c, j, GLOW[lvl])
                    t.set(c + 1, j, GLOW[lvl - 1])
                else:
                    t.set(j, c, GLOW[lvl])
                    t.set(j, c + 1, GLOW[lvl - 1])
            i += run
        for j in range(n):
            for o in (-1, 2):
                if vertical:
                    t.tint(c + o, j, GLOW[2], 0.3)
                else:
                    t.tint(j, c + o, GLOW[2], 0.3)

    for c in cuts:
        seam(c, False)
        if part != "front":
            seam(c, True)
    if part != "front":
        for cx in cuts:
            for cy in cuts:
                t.rect(cx - 1, cy - 1, cx + 3, cy + 3, GLOW[3])
                t.rect(cx, cy, cx + 2, cy + 2, GLOW[4])
    cx = cy = n / 2
    if part == "front":
        # The eye: a lit lens behind an iris of shutter blades in a bolted rim.
        for x, y in disc(cx, cy, 26):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            if bayer(x, y) < 0.6 * (1 - (d - 21) / 5):
                t.tint(x, y, GLOW[2], 0.35)
        ring(t, cx, cy, 17.5, 21.5, STEEL, mid=1.4)
        for i in range(8):
            a = 2 * math.pi * i / 8 + math.pi / 8
            rivet(t, int(round(cx + math.cos(a) * 19.5 - 1.5)), int(round(cy + math.sin(a) * 19.5 - 1.5)), STEEL, 3)
        for x, y in disc(cx, cy, 17.5, 12.5):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            a = (math.degrees(math.atan2(dy, dx)) + 360) % 45
            facing = math.cos(math.atan2(dy, dx) - LIGHT_A)
            c = pick(DARK_STEEL, 1.4 + facing * 0.7 + (a / 45) * 1.4, x, y, 0.4)
            if a < 2.6:
                c = INK[0]
            t.set(x, y, c)
        for x, y in disc(cx, cy, 12.5):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            if d > 11.5:
                c = GLOW[1]
            elif d > 9:
                c = pick(GLOW, 2.4 - (d - 9) / 2.5, x, y)
            elif d > 5.5:
                c = pick(GLOW, 3.4 - (d - 5.5) / 3.5, x, y)
            else:
                c = GLOW[4] if d > 2.5 else (240, 255, 236, 255)
            t.set(x, y, c)
        for x, y in disc(cx - 4, cy - 4, 2.2):
            t.set(x, y, (250, 255, 248, 255))
    elif part == "top":
        ring(t, cx, cy, 6, 9, STEEL, mid=1.6)
        for x, y in disc(cx, cy, 6):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            t.set(x, y, (240, 255, 236, 255) if d < 2.5 else GLOW[4] if d < 4.2 else GLOW[3])


for part in ("top", "side", "front"):
    add("block", f"sunbreaker_core_{part}", 64, core, part)


def implosion_shell(t, rng):
    n = t.n
    R = casing(t, rng, (104, 108, 116), EDGE, spread=0.55)
    groove(t, 2, n // 2 - 1, n - 2, n // 2, R, vertical=False)
    CH = ramp((150, 54, 40), dark=0.55, light=0.35)
    WIRE = ramp((200, 172, 60), dark=0.5, light=0.4)
    for row in (6, 37):
        # The wire's bus under the row, clipped to the plate.
        by = row + 21
        for x in range(3, n - 3):
            t.set(x, by, WIRE[3])
            t.set(x, by + 1, WIRE[1])
            t.mul(x, by + 2, 0.65)
        for x in range(8, n - 6, 14):
            emboss(t, box(x, by - 1, x + 3, by + 3), STEEL, 2)
        for col in (6, 26, 46):
            # A shaped charge: a cylinder, its strap, its stencil and its detonator.
            x0, x1, y0, y1 = col, col + 12, row + 2, row + 18
            body = rrect(x0, y0, x1, y1, 2)
            drop(t, body, 0.55)
            drop(t, {(x + 1, y + 1) for x, y in body}, 0.8)
            cylinder(t, body, x0, x1, CH, lo=0.3)
            cylinder(t, box(x0, y0 + 3, x1, y0 + 6), x0, x1, STEEL)
            for x in range(x0, x1):
                t.mul(x, y0 + 6, 0.7)
            text(t, x0 + 3, y0 + 9, "HE", CH[4])
            cylinder(t, box(x0 + 4, row - 2, x0 + 8, y0 + 1), x0 + 4, x0 + 8, STEEL)
            t.set(x0 + 5, row - 1, (240, 80, 50, 255))
            for y in range(y1, by):
                t.set(x0 + 9, y, WIRE[3])
                t.set(x0 + 10, y, WIRE[1])
    for x, y in ((3, 3), (n - 6, 3), (3, n - 6), (n - 6, n - 6)):
        rivet(t, x, y, R, 3)
    scratches(t, rng, 8, 2, 2, n - 2, n - 2)
    grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.35)


add("block", "implosion_shell", 64, implosion_shell)


def neutron_reflector(t, rng):
    n = t.n
    R = [(56, 66, 84, 255), (86, 100, 122, 255), (118, 136, 158, 255), (146, 164, 186, 255), (178, 194, 214, 255),
         (222, 232, 244, 255)]
    f = vnoise(n, rng, 3)
    w = 5
    lit_steps = [R[5], R[4], R[4], R[3], R[3]]
    dim_steps = [R[0], R[1], R[1], R[1], R[2]]
    for y in range(n):
        for x in range(n):
            e = min(x, y, n - 1 - x, n - 1 - y)
            if e < w:
                # A wide bevel: lit on the top and left, in shade on the bottom and right.
                top_left = (y == e and x <= n - 1 - e) or (x == e and y <= n - 1 - e)
                if y == e and x > n - 1 - e - 0:
                    top_left = x + y < n - 1
                t.set(x, y, lit_steps[e] if top_left and not (x == n - 1 - e or y == n - 1 - e) else dim_steps[e])
            else:
                v = 2 + (f[y][x] - 0.5) * 0.7
                s = x + y
                if abs(s - 1.24 * n) < 4:
                    v = 3.6 if abs(s - 1.24 * n) < 2 else 3
                elif abs(s - 1.72 * n) < 1.5:
                    v = 3
                elif abs(s - 0.6 * n) < 1:
                    v = 2.6
                t.set(x, y, pick(R, v, x, y, 0.45))
    lines_box(t, 0, 0, n, n, (40, 48, 62, 255), (40, 48, 62, 255))
    # Brushed: faint streaks along x.
    for _ in range(40):
        x, y = rng.randrange(w, n - w - 8), rng.randrange(w, n - w)
        k = 1.07 if rng.random() < 0.5 else 0.94
        for i in range(rng.randrange(5, 18)):
            if x + i < n - w:
                t.mul(x + i, y, k)
    for x, y in ((w + 1, w + 1), (n - w - 5, w + 1), (w + 1, n - w - 5), (n - w - 5, n - w - 5)):
        screw(t, x, y, R)
    text(t, n - w - 24, n - w - 8, "BERYL", R[1])


add("block", "neutron_reflector", 64, neutron_reflector)


# ================================================================ blocks: the silo and its pad (64)
def silo_hatch(t, rng, part):
    n = t.n
    if part == "top":
        R = casing(t, rng, (120, 124, 112), EDGE, spread=0.5)
        m = n // 2
        # Two leaves: hazard edges, ribs across, hinges on the outer edges, a lock bar over the seam.
        hazard(t, 2, 2, n - 2, 8, period=12, rng=rng, chips=3, rims=False)
        hazard(t, 2, n - 8, n - 2, n - 2, period=12, rng=rng, chips=3, rims=False)
        for x in range(2, n - 2):
            t.set(x, 8, R[0])
            t.set(x, n - 9, R[0])
        for x0, x1 in ((2, m - 1), (m + 1, n - 2)):
            for ry in (17, n - 22):
                rib = box(x0 + 3, ry, x1 - 3, ry + 5)
                drop(t, rib, 0.55)
                emboss(t, rib, R, 2.4)
                for bx in range(x0 + 5, x1 - 6, 7):
                    rivet(t, bx, ry + 1, R, 3)
        for y in range(9, n - 9):
            t.set(m - 1, y, INK[0])
            t.set(m, y, INK[2])
            t.set(m - 2, y, R[1])
            t.set(m + 1, y, R[3])
        for x, y in ((4, m - 6), (4, m + 2), (n - 8, m - 6), (n - 8, m + 2)):
            emboss(t, box(x, y, x + 4, y + 5), DARK_STEEL, 2)
        lock = box(m - 7, m - 3, m + 7, m + 3)
        drop(t, lock, 0.5)
        emboss(t, lock, STEEL, 2.2)
        for bx in (m - 5, m + 2):
            rivet(t, bx, m - 2, STEEL, 3)
        # Scorched where the missile leaves.
        soot(t, rng, m, m, 18, 0.78, 0.95)
        soot(t, rng, m, m, 10, 0.85, 0.9)
        scratches(t, rng, 10, 3, 10, n - 3, n - 10)
    elif part == "side":
        R = casing(t, rng, (100, 104, 94), EDGE, spread=0.5)
        hazard(t, 2, n - 12, n - 2, n - 4, period=16, rng=rng, chips=4)
        for sx in (n // 3, 2 * n // 3):
            groove(t, sx, 2, sx + 1, n - 13, R, vertical=True)
        for x in range(5, n - 5, 10):
            rivet(t, x, 4, R, 3)
            rivet(t, x, n - 18, R, 3)
        text(t, 6, 22, "SILO", INK[3], 2, rng, 0.08)
        text(t, 6, 34, "KEEP", INK[3], 1, rng, 0.08)
        text(t, 6, 40, "CLEAR", INK[3], 1, rng, 0.08)
        grime(t, rng, 2, n // 2, n - 2, n - 12, strength=0.5)
        scratches(t, rng, 8, 2, 2, n - 2, n - 12)
    else:
        R = casing(t, rng, (58, 60, 56), EDGE, spread=0.5)
        for i in range(3, n - 3):
            for x, y in ((i, i), (n - 1 - i, i)):
                t.set(x, y, R[3])
                t.set(x + 1, y, R[1])
        for x, y in ((4, 4), (n - 7, 4), (4, n - 7), (n - 7, n - 7), (n // 2 - 1, 4), (n // 2 - 1, n - 7)):
            rivet(t, x, y, R, 3)


for part in ("top", "side", "bottom"):
    add("block", f"silo_hatch_{part}", 64, silo_hatch, part)


def launch_pad(t, rng, part):
    n = t.n
    if part == "top":
        R = casing(t, rng, (96, 100, 108), EDGE, spread=0.5)
        cx = cy = n / 2
        c = n // 2 - 1
        groove(t, c, 1, c + 1, 16, R, vertical=True)
        groove(t, c, n - 16, c + 1, n - 1, R, vertical=True)
        groove(t, 1, c, 16, c + 1, R, vertical=False)
        groove(t, n - 16, c, n - 1, c + 1, R, vertical=False)
        for x, y in disc(cx, cy, 28.5, 18):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            wedge = int((math.degrees(math.atan2(dy, dx)) + 360) // 30) % 2
            d = math.hypot(dx, dy)
            t.set(x, y, (HAZ if wedge else INK)[1 if d > 27.5 else 3 if d < 19 else 2])
        wear_chips(t, rng, 10, 6, 6, n - 6, n - 6)
        for x, y in disc(cx, cy, 29.5, 28.5) | disc(cx, cy, 18, 17):
            t.set(x, y, INK[1])
        # The flame grate.
        g0, g1 = 21, n - 21
        t.rect(g0, g0, g1, g1, INK[0])
        for y in range(g0 + 1, g1 - 1, 4):
            emboss(t, box(g0 + 1, y, g1 - 1, y + 2), DARK_STEEL, 2.2)
        lines_box(t, g0 - 1, g0 - 1, g1 + 1, g1 + 1, R[0], R[3])
        soot(t, rng, cx, cy, 26, 0.72, 1.0)
        soot(t, rng, cx, cy, 14, 0.8, 1.0)
        for x, y in ((4, 4), (n - 7, 4), (4, n - 7), (n - 7, n - 7)):
            rivet(t, x, y, R, 3)
    else:
        R = casing(t, rng, (84, 88, 96), EDGE, spread=0.5)
        hazard(t, 2, 4, n - 2, 12, period=16, rng=rng, chips=4)
        # Ribs down the side.
        for rx in ((10, 30, 50) if part == "side" else (5, 54)):
            rib = box(rx, 16, rx + 5, n - 3)
            drop(t, rib, 0.55)
            emboss(t, rib, R, 2.4)
            rivet(t, rx + 1, 18, R, 3)
            rivet(t, rx + 1, n - 8, R, 3)
        if part == "front":
            # The service hatch: a red lamp and a keyed switch.
            x0, y0, x1, y1 = 16, 18, n - 16, n - 6
            t.rect(x0, y0, x1, y1, DARK_STEEL[2])
            lines_box(t, x0, y0, x1, y1, R[0], R[4])
            lines_box(t, x0 + 1, y0 + 1, x1 - 1, y1 - 1, DARK_STEEL[1], DARK_STEEL[3])
            text(t, (n - text_w("PAD-1")) // 2, y0 + 4, "PAD-1", HAZ[2])
            ly = y0 + 18
            ring(t, n / 2, ly, 4.5, 6.5, STEEL, mid=2)
            sphere(t, disc(n / 2, ly, 4.5), n / 2, ly, 4.5, REDGLOW, lo=1)
            ky = y1 - 9
            sphere(t, disc(n / 2, ky, 3.5), n / 2, ky, 3.5, STEEL, lo=0.5)
            t.set(n // 2 - 1, ky - 1, INK[0])
            t.set(n // 2, ky - 1, INK[0])
            for x, y in ((x0 + 2, y0 + 2), (x1 - 5, y0 + 2), (x0 + 2, y1 - 5), (x1 - 5, y1 - 5)):
                rivet(t, x, y, DARK_STEEL, 2)
        grime(t, rng, 2, n // 2, n - 2, n - 2, strength=0.5)
        scratches(t, rng, 8, 2, 12, n - 2, n - 2)


for part in ("top", "side", "front"):
    add("block", f"launch_pad_{part}", 64, launch_pad, part)


def launch_console(t, rng, part):
    n = t.n
    if part == "front":
        R = casing(t, rng, (70, 76, 84), EDGE, spread=0.5)
        # The screen: a bezel, scanlines, a trajectory to its target.
        x0, y0, x1, y1 = 6, 6, n - 6, 32
        t.rect(x0, y0, x1, y1, INK[2])
        lines_box(t, x0, y0, x1, y1, R[0], R[4])
        lines_box(t, x0 + 1, y0 + 1, x1 - 1, y1 - 1, INK[1], INK[4])
        SCR = [(10, 26, 14, 255), (16, 40, 22, 255), (40, 120, 60, 255), (90, 230, 110, 255), (190, 255, 200, 255)]
        for y in range(y0 + 3, y1 - 3):
            for x in range(x0 + 3, x1 - 3):
                t.set(x, y, SCR[1] if y % 2 else SCR[0])
        text(t, x0 + 5, y0 + 5, "TARGET", SCR[3])
        text(t, x0 + 5, y0 + 12, "LOCKED", SCR[2])
        for i in range(22):
            x = x0 + 28 + i
            y = int(y1 - 6 - 14 * math.sin(math.pi * i / 22))
            t.set(x, y, SCR[3] if i % 3 else SCR[2])
        tx, ty = x0 + 49, y1 - 6
        for d in range(-3, 4):
            t.set(tx + d, ty, SCR[4] if d == 0 else SCR[3])
            t.set(tx, ty + d, SCR[4] if d == 0 else SCR[3])
        # Toggle switches with their lamps.
        for i, lamp in enumerate((GLOW[3], (250, 190, 60, 255), REDGLOW[3])):
            sx = 8 + i * 9
            for dx in (1, 2):
                t.set(sx + dx, 37, lamp)
                t.set(sx + dx, 38, shade(lamp, 0.7))
            emboss(t, box(sx, 42, sx + 4, 50), INK, 2, sunk=True)
            up = i != 2
            for k in range(4):
                yy = 45 - k if up else 46 + k
                t.set(sx + 1, yy, STEEL[4])
                t.set(sx + 2, yy, STEEL[2])
            text(t, sx + 1, 53, str(i + 1), R[4])
        # The launch button under a striped guard.
        bx0, by0, bx1, by1 = 36, 36, n - 6, n - 9
        hazard(t, bx0, by0, bx1, by1, period=8, rims=False)
        t.rect(bx0 + 3, by0 + 3, bx1 - 3, by1 - 3, INK[1])
        lines_box(t, bx0 - 1, by0 - 1, bx1 + 1, by1 + 1, INK[0], INK[0])
        bcx, bcy = (bx0 + bx1) / 2, (by0 + by1) / 2
        btn = disc(bcx, bcy, 6)
        drop(t, btn, 0.6)
        sphere(t, btn, bcx, bcy, 6, RED, lo=0.4)
        text(t, (bx0 + bx1 - text_w("FIRE")) // 2, n - 7, "FIRE", HAZ[2])
        for x, y in ((2, 2), (n - 6, 2), (2, n - 6), (n - 6, n - 6)):
            screw(t, x, y, R)
        scratches(t, rng, 5, 2, 34, 34, n - 2)
    elif part == "side":
        R = casing(t, rng, (76, 82, 90), EDGE, spread=0.5)
        for k in range(6):
            y = 12 + k * 5
            for x in range(14, n - 14):
                t.set(x, y, INK[0])
                t.set(x, y + 1, INK[2])
                t.set(x, y + 2, R[3])
        hazard(t, 2, n - 10, n - 2, n - 3, period=14, rng=rng, chips=3)
        for x, y in ((4, 4), (n - 7, 4), (4, n - 15), (n - 7, n - 15)):
            rivet(t, x, y, R, 3)
        grime(t, rng, 2, n // 2, n - 2, n - 10, strength=0.45)
        scratches(t, rng, 6, 2, 2, n - 2, n - 10)
    else:
        R = casing(t, rng, (86, 92, 100), EDGE, spread=0.5)
        groove(t, 2, n // 3, n - 2, n // 3 + 1, R, vertical=False)
        # The key slot and a row of vents.
        emboss(t, box(10, 46, n - 10, 51), INK, 1.4, sunk=True)
        for x in range(12, n - 12):
            t.set(x, 48, INK[0])
        for k in range(8):
            x = 10 + k * 6
            for y in range(6, 17):
                t.set(x, y, INK[1])
                t.set(x + 1, y, R[3])
        for x, y in ((4, 4), (n - 7, 4), (4, n - 7), (n - 7, n - 7)):
            rivet(t, x, y, R, 3)
        scratches(t, rng, 6, 2, 2, n - 2, n - 2)


for part in ("top", "side", "front"):
    add("block", f"launch_console_{part}", 64, launch_console, part)


def strike_beacon(t, rng, part):
    n = t.n
    if part == "front":
        R = casing(t, rng, (92, 60, 56), EDGE, spread=0.5)
        cx = cy = n / 2
        ring(t, cx, cy, 19, 24, STEEL, mid=2)
        for i in range(8):
            a = 2 * math.pi * i / 8
            rivet(t, int(round(cx + math.cos(a) * 21.5 - 1.5)), int(round(cy + math.sin(a) * 21.5 - 1.5)), STEEL, 3)
        lens = disc(cx, cy, 19)
        sphere(t, lens, cx, cy, 19, REDGLOW, lo=0.8, spec=False)
        for x, y in disc(cx, cy, 6):
            t.set(x, y, REDGLOW[5] if math.hypot(x + 0.5 - cx, y + 0.5 - cy) < 3.5 else REDGLOW[4])
        # The grille's bars over the lens.
        for x, y in sorted(lens):
            if (y - int(cy)) % 5 == 0:
                t.set(x, y, STEEL[1])
                t.set(x, y - 1, STEEL[3])
                t.mul(x, y + 1, 0.7)
        for x, y in ((4, 4), (n - 7, 4), (4, n - 7), (n - 7, n - 7)):
            rivet(t, x, y, R, 3)
        grime(t, rng, 2, n * 2 // 3, n - 2, n - 2, strength=0.4)
    elif part == "side":
        R = casing(t, rng, (84, 56, 52), EDGE, spread=0.5)
        hazard(t, 2, 6, n - 2, 14, period=16, rng=rng, chips=3)
        label_plate(t, 12, 22, n - 12, 32, ramp((214, 210, 196)))
        text(t, (n - text_w("BEACON")) // 2, 25, "BEACON", REDGLOW[1])
        for x in range(n // 2 - 8, n // 2 + 8, 4):
            sphere(t, disc(x + 2, 42, 1.8), x + 2, 42, 1.8, REDGLOW, lo=2)
        for k in range(4):
            y = 48 + k * 3
            for x in range(14, n - 14):
                t.set(x, y, INK[0])
                t.set(x, y + 1, R[3])
        for x, y in ((4, 18), (n - 7, 18), (4, n - 7), (n - 7, n - 7)):
            rivet(t, x, y, R, 3)
        grime(t, rng, 2, n // 2, n - 2, n - 2, strength=0.45)
        scratches(t, rng, 6, 2, 14, n - 2, n - 2)
    else:
        R = casing(t, rng, (80, 82, 88), EDGE, spread=0.5)
        cx = cy = n / 2
        for x, y in disc(cx, cy, 26, 14):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            wedge = int((math.degrees(math.atan2(dy, dx)) + 360) // 30) % 2
            facing = math.cos(math.atan2(dy, dx) - LIGHT_A)
            t.set(x, y, pick(STEEL, (3 if wedge else 1.8) + facing * 0.6, x, y, 0.4))
        outline(t, disc(cx, cy, 26), INK[1])
        ring(t, cx, cy, 9, 14, DARK_STEEL, mid=2)
        sphere(t, disc(cx, cy, 9), cx, cy, 9, REDGLOW, lo=0.8)
        for x, y in ((4, 4), (n - 7, 4), (4, n - 7), (n - 7, n - 7)):
            rivet(t, x, y, R, 3)
        scratches(t, rng, 6, 2, 2, n - 2, n - 2)


for part in ("top", "side", "front"):
    add("block", f"strike_beacon_{part}", 64, strike_beacon, part)


# ================================================================ blocks: the bunker's walls and their cracks
def reinforced_concrete(t, rng):
    n = t.n
    R = ramp((150, 150, 146), dark=0.45, light=0.3)
    fill(t, box(0, 0, n, n), R, rng, spread=0.8, cells=4, grain=0.5, dither=0.25)
    # Pores: small pits, dark with a lit lower lip.
    for _ in range(n):
        x, y = rng.randrange(n), rng.randrange(n)
        t.set(x, y, R[0] if rng.random() < 0.4 else R[1])
        t.set(x, (y + 1) % n, R[3])
    # A faint seam from the formwork across the middle.
    for x in range(n):
        if rng.random() < 0.8:
            t.mul(x, n // 2, 0.9)
    # The form ties' holes, rust running down from each.
    q = n // 4
    for cx, cy in ((q, q), (n - q, q), (q, n - q), (n - q, n - q)):
        hole = disc(cx, cy, 2.2)
        emboss(t, hole, R, 0.6, sunk=True)
        t.set(cx - 1, cy - 1, (110, 72, 48, 255))
        t.set(cx, cy - 1, (132, 88, 56, 255))
        t.set(cx - 1, cy, (96, 62, 42, 255))
        t.set(cx, cy, (120, 80, 52, 255))
        for k in range(1, 6):
            if rng.random() < 0.85 - k * 0.12:
                t.tint(cx - 1 + (k > 3), cy + 1 + k, (140, 96, 64), 0.4 - k * 0.05)


def armored_concrete(t, rng):
    n = t.n
    R = ramp((104, 108, 114), dark=0.5, light=0.3)
    fill(t, box(0, 0, n, n), R, rng, spread=0.7, cells=4, grain=0.4, dither=0.25)
    for _ in range(n // 2):
        x, y = rng.randrange(n), rng.randrange(n)
        t.set(x, y, R[0])
    # The steel mesh cast in it: bars with a lit top edge, studs where they cross.
    M = ramp((88, 92, 102), dark=0.5, light=0.4)
    a, b = n * 5 // 16, n * 11 // 16 - 1
    for c in (a, b):
        for i in range(n):
            t.set(i, c, M[3])
            t.set(i, c + 1, M[1])
            t.mul(i, c + 2, 0.75)
            t.set(c, i, M[3])
            t.set(c + 1, i, M[1])
            t.mul(c + 2, i, 0.75)
    for cx in (a, b):
        for cy in (a, b):
            stud = box(cx - 1, cy - 1, cx + 3, cy + 3)
            emboss(t, stud, ramp((150, 156, 166)), 2)
    lines_box(t, 0, 0, n, n, (54, 56, 62, 255), (54, 56, 62, 255))
    lines_box(t, 1, 1, n - 1, n - 1, R[3], R[1])


def lead_shield(t, rng):
    n = t.n
    R = ramp((82, 88, 104), dark=0.5, light=0.35)
    fill(t, box(0, 0, n, n), R, rng, spread=0.55, cells=3, grain=0.1, dither=0.1)
    # Soft dents and the white bloom lead gets.
    for _ in range(4):
        cx, cy = rng.uniform(3, n - 3), rng.uniform(3, n - 3)
        for x, y in disc(cx, cy, 2.2):
            t.mul(x % n, y % n, 0.9 if (x + 0.5 - cx) + (y + 0.5 - cy) < 0 else 1.1)
    for _ in range(n // 6):
        t.set(rng.randrange(n), rng.randrange(n), (136, 142, 158, 255))
    # Two plates, seamed, riveted.
    for y in (0, n // 2):
        for x in range(n):
            t.set(x, y, R[0])
            t.set(x, y + 1, R[4])
            t.mul(x, y + 2, 1.05)
    for py in (0, n // 2):
        for x in (3, n - 6, n // 2 - 1):
            rivet(t, x, py + 7, R, 2)
    scratches(t, rng, 6, 0, 0, n, n, k=1.18)


def blast_damper(t, rng):
    n = t.n
    D = ramp((196, 176, 116), dark=0.5, light=0.35)
    p = n // 2
    h = p // 2
    for y in range(n):
        for x in range(n):
            # Which pillow: diamonds between stitch lines x+y and x-y every p pixels.
            u = (x + y) % p
            v = (x - y) % p
            fu = (u - h + 0.5) / h
            fv = (v - h + 0.5) / h
            # Light from the top left falls on a pillow's upper-left slopes.
            lit = -(fu + fv) * 0.5
            dome = 1 - max(abs(fu), abs(fv))
            val = 2 + lit * 1.4 + dome * 0.4
            t.set(x, y, pick(D, val, x, y, 0.4))
            if u == 0 or v == 0:
                t.set(x, y, D[0] if (x + y) % 3 else D[1])
    # A button where the stitches cross.
    for y in range(n):
        for x in range(n):
            if (x + y) % p == 0 and (x - y) % p == 0:
                for dx, dy, c in ((0, 0, (120, 98, 52)), (1, 0, (90, 72, 38)), (0, 1, (90, 72, 38)), (1, 1, (60, 48, 26)),
                                  (-1, 0, (150, 124, 70)), (0, -1, (150, 124, 70))):
                    t.set((x + dx) % n, (y + dy) % n, c)
    for _ in range(n // 2):
        x, y = rng.randrange(n), rng.randrange(n)
        t.mul(x, y, 0.9)
    edge = shade((196, 176, 116), 0.5)
    lines_box(t, 0, 0, n, n, edge, edge)


WALLS = {
    "reinforced_concrete": reinforced_concrete,
    "armored_concrete": armored_concrete,
    "lead_shield": lead_shield,
    "blast_damper": blast_damper,
}


def cracks(t, rng):
    """Cracks run in from two edges and fork; each is a dark line with a lit lip and a little spalling."""
    n = t.n
    mid = t.get(n // 2 - 3, n // 2 - 3)
    dark, deep, lip = shade(mid, 0.38), shade(mid, 0.22), shade(mid, 1.22)
    paths = set()

    def run(x, y, dx, dy, steps, fork):
        for step in range(steps):
            x += dx + (rng.choice((-1, 0, 0, 1)) if dy else 0)
            y += dy + (rng.choice((-1, 0, 0, 1)) if dx else 0)
            if not (0 <= x < n and 0 <= y < n):
                return
            paths.add((x, y))
            if fork and step in fork:
                fdx, fdy = (dx, rng.choice((-1, 1))) if dx else (rng.choice((-1, 1)), dy)
                run(x, y, fdx, fdy, steps // 3, None)

    run(-1, rng.randrange(n // 5, 4 * n // 5), 1, 0, n * 3 // 4, (n // 4, n // 2))
    run(rng.randrange(n // 5, 4 * n // 5), n, 0, -1, n * 2 // 3, (n // 3,))
    run(n, rng.randrange(n // 4, 3 * n // 4), -1, 0, n // 3, None)
    for x, y in paths:
        t.set(x, y, dark)
        if (x + 1, y) not in paths and (x, y + 1) not in paths:
            t.set(x + 1, y + 1, lip)
    for x, y in paths:
        if sum((x + dx, y + dy) in paths for dx, dy in N4) >= 2 and rng.random() < 0.3:
            t.set(x, y, deep)
    for _ in range(len(paths) // 5):
        x, y = rng.choice(sorted(paths))
        ox, oy = x + rng.choice((-1, 1)), y + rng.choice((-1, 1))
        if (ox, oy) not in paths:
            t.mul(ox, oy, 0.8)


def wall(t, rng, name):
    WALLS[name](t, rng)


def cracked_wall(t, rng, name):
    WALLS[name](t, seeded(name))
    cracks(t, rng)


for name in WALLS:
    add("block", name, 32, wall, name)
    add("block", f"cracked_{name}", 32, cracked_wall, name)


# ================================================================ blocks: the bunker's door and machines
DOOR = ramp((118, 124, 130), dark=0.5, light=0.4)


def door_leaf(t, rng):
    n = t.n
    R = casing(t, rng, None, (52, 56, 62, 255), R=DOOR, spread=0.7)
    # A hinge-side stile.
    for y in range(1, n - 1):
        t.set(5, y, R[1])
        t.set(6, y, R[3])
    return R


def hermetic_door(t, rng, part):
    n = t.n
    R = door_leaf(t, rng)
    if part == "top":
        cx, cy = n / 2 + 2, n / 2 + 1
        ring(t, cx, cy, 6.4, 9.2, STEEL, mid=2.2)
        glass = disc(cx, cy, 6.4)
        for x, y in glass:
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            v = 2.2 - (dx + dy) / 10
            if -2 < dx - dy < 1 and dx + dy < 0:
                v = 4.3
            t.set(x, y, pick(GLASS, v, x, y, 0.4))
        for x, y in disc(cx, cy, 6.4, 5.4):
            if (x + 0.5 - cx) + (y + 0.5 - cy) < 0:
                t.set(x, y, GLASS[0])
        for i in range(6):
            a = 2 * math.pi * i / 6 + 0.3
            rivet(t, int(cx + math.cos(a) * 7.9 - 1), int(cy + math.sin(a) * 7.9 - 1), STEEL, 2)
        for x in range(1, n - 1):
            t.set(x, n - 1, R[1])
            t.set(x, n - 2, R[3])
        for y in (5, n - 7):
            emboss(t, box(1, y, 4, y + 4), DARK_STEEL, 2)
        grime(t, rng, 1, 1, n - 1, n // 3, strength=0.25, up=True)
    else:
        cx, cy = n / 2 + 2, n / 2 - 3
        # The locking dogs out to the edges.
        for x in range(7, n - 1):
            if abs(x + 0.5 - cx) > 8:
                t.set(x, int(cy) - 1, STEEL[3])
                t.set(x, int(cy), STEEL[2])
                t.set(x, int(cy) + 1, STEEL[0])
        wheel = disc(cx, cy, 8.6, 6.0)
        drop(t, wheel, 0.55)
        ring(t, cx, cy, 6.0, 8.6, RED, mid=2)
        for i in range(4):
            a = math.pi / 4 + i * math.pi / 2
            for k in range(2, 7):
                x, y = int(cx + math.cos(a) * k), int(cy + math.sin(a) * k)
                t.set(x, y, RED[3] if i < 2 else RED[1])
        hub = disc(cx, cy, 2.4)
        sphere(t, hub, cx, cy, 2.4, STEEL, lo=0.5)
        for x in range(1, n - 1):
            t.set(x, 0, R[3])
            t.set(x, 1, R[2])
        hazard(t, 7, n - 8, n - 1, n - 3, period=8, rng=rng, chips=1)
        for y in (3, n - 13):
            emboss(t, box(1, y, 4, y + 4), DARK_STEEL, 2)
        grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.5)
    scratches(t, rng, 4, 7, 2, n - 2, n - 2)


for part in ("top", "bottom"):
    add("block", f"hermetic_door_{part}", 32, hermetic_door, part)

ENGINE = (96, 104, 76)
ENGINE_EDGE = (40, 44, 34, 255)


def fuel_generator(t, rng, part):
    n = t.n
    R = casing(t, rng, ENGINE, ENGINE_EDGE)
    if part.startswith("front"):
        lit = part.endswith("on")
        # Louvres over the engine.
        for k in range(4):
            y = 4 + k * 3
            for x in range(4, 19):
                t.set(x, y, INK[1])
                t.set(x, y + 1, R[4])
        # A gauge.
        ring(t, 24.5, 8.5, 3, 4.2, STEEL, mid=2)
        t.paint(disc(24.5, 8.5, 3), (226, 222, 200, 255))
        for x, y in ((24, 8), (25, 7), (26, 6)):
            t.set(x, y, RED[2])
        t.set(24, 9, INK[1])
        # The firebox under its grate.
        x0, y0, x1, y1 = 6, 17, n - 6, n - 4
        lines_box(t, x0 - 1, y0 - 1, x1 + 1, y1 + 1, R[0], R[4])
        t.rect(x0, y0, x1, y1, INK[1])
        if lit:
            FIRE = [(110, 30, 10, 255), (190, 60, 16, 255), (240, 120, 30, 255), (255, 186, 60, 255), (255, 236, 150, 255)]
            f = vnoise(n, rng, 6, 3)
            for y in range(y0, y1):
                for x in range(x0, x1):
                    v = (y - y0) / (y1 - y0) * 3.4 + (f[y][x] - 0.5) * 2.2 + 0.4
                    t.set(x, y, pick(FIRE, v, x, y, 0.5))
            for x in range(x0, x1):
                t.set(x, y1 - 1, FIRE[4] if x % 3 else FIRE[3])
        else:
            for _ in range(10):
                t.set(rng.randrange(x0, x1), rng.randrange(y1 - 3, y1), (70, 66, 60, 255))
        for x in range(x0 + 2, x1, 3):
            for y in range(y0, y1):
                t.set(x, y, INK[3] if y != y0 else INK[4])
        for x, y in ((2, 2), (n - 4, 2), (2, n - 4), (n - 4, n - 4)):
            rivet(t, x, y, R, 2)
        grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.45)
    elif part == "side":
        # Cooling fins.
        for x in range(4, n - 4, 5):
            fin = box(x, 4, x + 2, n - 7)
            drop(t, fin, 0.6)
            emboss(t, fin, R, 2.2)
        hazard(t, 1, n - 5, n - 1, n - 2, period=8, rng=rng, chips=2)
        grime(t, rng, 1, n // 2, n - 1, n - 5, strength=0.5)
    else:
        # The filler cap, the exhaust, a seam.
        groove(t, 2, n // 2 + 3, n - 2, n // 2 + 4, R, vertical=False)
        cap = disc(21, 11, 6.4)
        drop(t, cap, 0.55)
        for x, y in cap:
            a = math.atan2(y + 0.5 - 11, x + 0.5 - 21)
            d = math.hypot(x + 0.5 - 21, y + 0.5 - 11)
            if d > 4.6:
                t.set(x, y, STEEL[3] if int(math.degrees(a) // 20) % 2 else STEEL[1])
            else:
                t.set(x, y, pick(STEEL, 2 - (x + y - 32) / 6, x, y, 0.3))
        t.paint(disc(21, 11, 1.6), INK[2])
        ring(t, 8, 8, 2.6, 4.2, DARK_STEEL, mid=2)
        t.paint(disc(8, 8, 2.6), INK[0])
        soot(t, rng, 8, 8, 8, 0.75, 0.8)
        text(t, 5, n - 9, "FUEL", HAZ[2])
        for x, y in ((n - 5, n - 5), (n - 5, n // 2 + 6)):
            rivet(t, x, y, R, 2)


for part in ("top", "side", "front", "front_on"):
    add("block", f"fuel_generator_{part}", 32, fuel_generator, part)

FILTER = (150, 172, 150)
FILTER_EDGE = (56, 70, 60, 255)


def air_filter(t, rng, part):
    n = t.n
    R = casing(t, rng, FILTER, FILTER_EDGE)
    cx = cy = n / 2
    if part == "front":
        ring(t, cx, cy, 11.2, 13.2, ramp((90, 104, 94)), mid=2)
        BLADE = ramp((150, 160, 154), dark=0.5, light=0.35)
        for x, y in disc(cx, cy, 11.2):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            d = math.hypot(dx, dy)
            a = (math.degrees(math.atan2(dy, dx)) + 360 - d * 4) % 120
            if a < 62:
                c = BLADE[3] if a < 8 else BLADE[2] if a < 40 else BLADE[1]
            else:
                c = INK[1] if d > 3 else INK[2]
            t.set(x, y, c)
        # The guard: two rings over the fan.
        for r in (6.0, 10.0):
            for x, y in disc(cx, cy, r + 0.6, r - 0.5):
                t.set(x, y, STEEL[3] if (x + 0.5 - cx) + (y + 0.5 - cy) < 0 else STEEL[1])
        hub = disc(cx, cy, 2.6)
        sphere(t, hub, cx, cy, 2.6, STEEL, lo=1)
        for x, y in ((2, 2), (n - 4, 2), (2, n - 4), (n - 4, n - 4)):
            rivet(t, x, y, R, 2)
    elif part == "side":
        PAPER = [(150, 140, 112, 255), (190, 182, 156, 255), (222, 216, 194, 255), (240, 236, 220, 255)]
        for x0 in (4, n // 2 + 2):
            x1 = x0 + n // 2 - 6
            lines_box(t, x0 - 1, 3, x1 + 1, n - 3, R[0], R[4])
            for y in range(4, n - 4):
                for x in range(x0, x1):
                    k = y % 3
                    t.set(x, y, PAPER[[3, 1, 0][k]] if x > x0 else PAPER[[2, 0, 0][k]])
            for y in (4, n - 6):
                emboss(t, box(x0 - 1, y, x1 + 1, y + 2), ramp((90, 104, 94)), 2)
        grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.35)
    else:
        for y in range(5, n - 5, 4):
            for x in range(5, n - 5, 4):
                hole = box(x, y, x + 3, y + 3)
                emboss(t, hole, INK, 1.6, sunk=True)
        lines_box(t, 4, 4, n - 4, n - 4, R[0], R[4])
        for x, y in ((1, 1), (n - 3, 1), (1, n - 3), (n - 3, n - 3)):
            t.set(x, y, R[4])


for part in ("top", "side", "front"):
    add("block", f"air_filter_{part}", 32, air_filter, part)


def bunker_lamp(t, rng, lit):
    n = t.n
    G = ([(130, 96, 40, 255), (200, 156, 70, 255), (240, 200, 110, 255), (255, 232, 160, 255), (255, 246, 210, 255),
          (255, 255, 240, 255)] if lit else
         [(52, 50, 42, 255), (72, 70, 58, 255), (94, 90, 74, 255), (118, 114, 94, 255), (140, 136, 114, 255),
          (166, 162, 140, 255)])
    f = vnoise(n, rng, 3)
    for y in range(n):
        for x in range(n):
            d = math.hypot(x + 0.5 - n / 2, y + 0.5 - n / 2)
            v = 4.2 - d / (n / 2) * 3.2 + (f[y][x] - 0.5) * 0.6
            t.set(x, y, pick(G, v, x, y, 0.5))
    # Each pane's own glint and edge shadow, in a steel cage of 2-pixel bars.
    for x0, x1 in ((2, 10), (12, 20), (22, 30)):
        for y0, y1 in ((2, 10), (12, 20), (22, 30)):
            for i in range(x0, x1):
                t.mul(i, y1 - 1, 0.86)
                t.mul(x1 - 1, y0 + i - x0, 0.86)
            t.set(x0 + 1, y0 + 1, G[5])
            t.set(x0 + 2, y0 + 1, G[4])
            t.set(x0 + 1, y0 + 2, G[4])
    CAGE = ramp((64, 66, 72), dark=0.5, light=0.35)
    for c in (0, 10, 20, 30):
        for i in range(n):
            t.set(i, c, CAGE[3])
            t.set(i, c + 1, CAGE[1])
        for i in range(n):
            if i % 10 not in (0, 1):
                t.set(c, i, CAGE[3])
                t.set(c + 1, i, CAGE[1])
    for cx in (0, 10, 20, 30):
        for cy in (0, 10, 20, 30):
            t.set(cx, cy, CAGE[4])
            t.set(cx + 1, cy, CAGE[3])
            t.set(cx, cy + 1, CAGE[2])
            t.set(cx + 1, cy + 1, CAGE[0])


add("block", "bunker_lamp", 32, bunker_lamp, False)
add("block", "bunker_lamp_lit", 32, bunker_lamp, True)

CABINET = (196, 200, 204)
CABINET_EDGE = (70, 76, 84, 255)
WATER = ramp((60, 150, 210), dark=0.55, light=0.5)


def decontaminator(t, rng, part):
    n = t.n
    if part == "top":
        R = casing(t, rng, (150, 156, 162), CABINET_EDGE)
        cx = cy = n / 2
        ring(t, cx, cy, 11, 13, R, mid=1.6)
        for x, y in disc(cx, cy, 11):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            a = (math.degrees(math.atan2(dy, dx)) + 360) % 90
            t.set(x, y, pick(STEEL, 3.2 - a / 40, x, y, 0.3) if 10 < a < 55 else INK[2])
        sphere(t, disc(cx, cy, 3.2), cx, cy, 3.2, STEEL, lo=1)
        for x, y in ((2, 2), (n - 4, 2), (2, n - 4), (n - 4, n - 4)):
            rivet(t, x, y, R, 2)
        return
    R = casing(t, rng, CABINET, CABINET_EDGE, spread=0.6)
    if part == "front":
        cx, cy = n / 2, n / 2 - 2
        ring(t, cx, cy, 7.4, 9.6, ramp((86, 92, 102)), mid=2)
        for x, y in disc(cx, cy, 7.4):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            streak = (x * 2 + y) % 5 == 0
            v = 2.6 - (dy + 7) / 8 + (0.8 if streak else 0)
            t.set(x, y, pick(WATER, v, x, y, 0.4))
        for _ in range(7):
            x, y = int(cx + rng.uniform(-5, 5)), int(cy + rng.uniform(-5, 4))
            if math.hypot(x + 0.5 - cx, y + 0.5 - cy) < 6:
                t.set(x, y, WATER[4])
                t.set(x, y + 1, WATER[3])
        for x, y in disc(cx, cy, 6.4, 4.6):
            a = math.degrees(math.atan2(y + 0.5 - cy, x + 0.5 - cx)) % 360
            if 205 < a < 250:
                t.set(x, y, (232, 246, 255, 255))
        for i in range(6):
            a = 2 * math.pi * i / 6
            t.set(int(cx + math.cos(a) * 8.5), int(cy + math.sin(a) * 8.5), STEEL[4])
        for x in (4, 5):
            t.set(x, n - 6, GLOW[3])
            t.set(x, n - 5, GLOW[2])
        t.set(7, n - 6, (240, 190, 60, 255))
        t.set(7, n - 5, (200, 150, 40, 255))
        text(t, n - 4 - text_w("DECON"), n - 8, "DECON", CABINET_EDGE)
        grime(t, rng, 1, n // 2, n - 1, n - 1, strength=0.35)
    else:
        hazard(t, 1, n - 8, n - 1, n - 3, period=8, rng=rng, chips=2)
        pipe = box(6, 1, 10, n - 9)
        cylinder(t, pipe, 6, 10, WATER, lo=0.4)
        for y in (6, 16):
            clamp = box(4, y, 12, y + 2)
            drop(t, clamp, 0.6)
            emboss(t, clamp, STEEL, 2)
        groove(t, n // 2 + 3, 2, n // 2 + 4, n - 9, R, vertical=True)
        for x, y in ((n - 5, 3), (n - 5, n - 12), (14, 3), (14, n - 12)):
            rivet(t, x, y, R, 2)
        grime(t, rng, 1, n // 2, n - 1, n - 8, strength=0.35)


for part in ("top", "side", "front"):
    add("block", f"decontaminator_{part}", 32, decontaminator, part)


# ================================================================ items: uranium
def lump(t, rng, R, facets, specks=None):
    """A rough lump of ore: facets that catch the light at their own angles, crisp edges between them."""
    n = t.n
    cx, cy, base = n * 0.5, n * 0.53, n * 0.37
    k = 11
    pts = []
    for i in range(k):
        a = 2 * math.pi * i / k + rng.uniform(-0.15, 0.15)
        r = base * rng.uniform(0.78, 1.0)
        pts.append((cx + math.cos(a) * r, cy + math.sin(a) * r * 0.86))
    mask = poly(pts)
    cells = sorted(mask)
    seeds = []
    for _ in range(facets):
        sx, sy = rng.choice(cells)
        ox, oy = (sx + 0.5 - cx) / base, (sy + 0.5 - cy) / base
        nx, ny = ox * 0.7 + rng.uniform(-0.55, 0.55), oy * 0.7 + rng.uniform(-0.55, 0.55)
        nz = math.sqrt(max(0.08, 1 - nx * nx - ny * ny))
        ln = math.sqrt(nx * nx + ny * ny + nz * nz)
        seeds.append((sx + 0.5, sy + 0.5, (nx * LX + ny * LY + nz * LZ) / ln))
    owner = {}
    for x, y in mask:
        owner[(x, y)] = min(range(len(seeds)), key=lambda i: (seeds[i][0] - x - 0.5) ** 2 + (seeds[i][1] - y - 0.5) ** 2)
    for x, y in mask:
        # The facet's own light, the lump's roundness, a lit rim on its upper left and a shaded one below right.
        g = -((x + 0.5 - cx) + (y + 0.5 - cy)) / (2 * base)
        v = 1.0 + 2.4 * (seeds[owner[(x, y)]][2] * 0.5 + 0.5) + g * 1.6
        if (x - 1, y) not in mask or (x, y - 1) not in mask:
            v += 0.9 if g > -0.1 else 0
        if (x + 1, y) not in mask or (x, y + 1) not in mask:
            v -= 0.9
        right, down = owner.get((x + 1, y)), owner.get((x, y + 1))
        if any(o is not None and o != owner[(x, y)] and seeds[o][2] > seeds[owner[(x, y)]][2] for o in (right, down)):
            v -= 0.5
        t.set(x, y, pick(R, min(v, len(R) - 1.6), x, y, 0.0))
    for _ in range(specks or 0):
        x, y = rng.choice(cells)
        t.set(x, y, R[-1])
    item_outline(t)
    return mask


RAW = [(24, 34, 24, 255), (40, 58, 36, 255), (60, 84, 48, 255), (84, 114, 62, 255), (114, 150, 80, 255),
       (160, 200, 110, 255)]
ENRICHED = [(18, 80, 28, 255), (32, 128, 40, 255), (62, 184, 56, 255), (106, 230, 80, 255), (168, 255, 128, 255),
            (232, 255, 200, 255)]
WEAPONS = [(16, 92, 34, 255), (40, 154, 50, 255), (92, 216, 74, 255), (156, 252, 120, 255), (216, 255, 192, 255),
           (255, 255, 244, 255)]


def raw_uranium(t, rng):
    mask = lump(t, rng, RAW, 6)
    # Grains of the ore showing through the rock.
    for _ in range(7):
        x, y = rng.choice(sorted(mask))
        if all(p in mask for p in ((x + 1, y), (x, y + 1), (x + 1, y + 1))):
            t.set(x, y, URAN[5])
            t.set(x + 1, y, URAN[4])
            t.set(x, y + 1, URAN[3])
            t.set(x + 1, y + 1, URAN[2])


def enriched_uranium(t, rng):
    lump(t, rng, ENRICHED, 13, specks=8)


def weapons_grade_uranium(t, rng):
    lump(t, rng, WEAPONS, 7, specks=3)
    halo(t, (150, 255, 120), (130, 50))


add("item", "raw_uranium", 32, raw_uranium)
add("item", "enriched_uranium", 64, enriched_uranium)
add("item", "weapons_grade_uranium", 32, weapons_grade_uranium)


# ================================================================ items: the bomb's parts
def nuclear_charge(t, rng):
    GOLD = ramp((210, 180, 40), dark=0.5, light=0.45)
    CAP = ramp((70, 72, 80), dark=0.5, light=0.4)
    for y0, y1 in ((2, 8), (24, 30)):
        cylinder(t, rrect(10, y0, 22, y1, 2), 10, 22, CAP, lo=0.3)
    cylinder(t, box(11, 8, 21, 24), 11, 21, GOLD, lo=0.3)
    for y in (9, 22):
        for x in range(11, 21):
            t.mul(x, y, 0.7)
    for y in range(11, 21):
        t.set(13, y, INK[2])
        t.set(18, y, INK[2])
    cylinder(t, box(14, 11, 18, 21), 14, 18, URAN[1:], lo=0.8)
    for y in range(12, 20):
        t.set(15, y, URAN[5] if y % 4 else URAN[4])
    for x in range(13, 19):
        t.set(x, 10, INK[2])
        t.set(x, 21, INK[2])
    item_outline(t)


def bomb_casing(t, rng):
    n = t.n
    S = [(40, 44, 52, 255), (66, 72, 82, 255), (98, 104, 116, 255), (134, 140, 152, 255), (174, 180, 192, 255),
         (226, 230, 236, 255)]
    cx, cy, r = n / 2, n / 2 + 2, 25.5
    m = disc(cx, cy, r)
    sphere(t, m, cx, cy, r, S, lo=0.2, dither=0.3)
    # Seams: the equator and three meridians, seen from a little above.
    seams = set()
    for x in range(int(cx - r), int(cx + r) + 1):
        q = 1 - ((x + 0.5 - cx) / r) ** 2
        if q > 0:
            seams.add((x, int(cy + 5 * math.sqrt(q))))
    for th in (-0.62, 0.0, 0.62):
        for y in range(int(cy - r), int(cy + r) + 1):
            q = r * r - (y + 0.5 - cy) ** 2
            if q > 0:
                seams.add((int(cx + math.sin(th) * math.sqrt(q)), y))
    for x, y in seams:
        if (x, y) in m:
            t.mul(x, y, 0.55)
            if (x + 1, y + 1) in m and (x + 1, y + 1) not in seams:
                t.mul(x + 1, y + 1, 1.18)
    for x in range(int(cx - r) + 5, int(cx + r) - 4, 6):
        q = 1 - ((x + 0.5 - cx) / r) ** 2
        y = int(cy + 5 * math.sqrt(q)) - 2
        rivet(t, x, y, S, 2)
    # The filling port on top, a lug through it.
    port = ellipse(cx, cy - r * 0.64, 6, 2.6)
    for x, y in port:
        t.set(x, y, S[0] if y + 0.5 < cy - r * 0.64 else S[1])
    for x, y in border(port):
        if (x, y) in m:
            t.set(x, y, S[3] if y + 0.5 > cy - r * 0.64 else S[1])
    ring(t, cx, cy - r + 1, 2.2, 4.2, S, mid=2.5)
    item_outline(t)


add("item", "nuclear_charge", 32, nuclear_charge)
add("item", "bomb_casing", 64, bomb_casing)


# ================================================================ items: the detonators
def line(t, x0, y0, x1, y1, c):
    steps = max(abs(x1 - x0), abs(y1 - y0), 1)
    for i in range(steps + 1):
        t.set(int(round(x0 + (x1 - x0) * i / steps)), int(round(y0 + (y1 - y0) * i / steps)), c)


def fuse_detonator(t, rng):
    B = ramp((150, 60, 40), dark=0.55, light=0.4)
    front = box(10, 30, 46, 56)
    topf = poly([(10, 30), (18, 22), (54, 22), (46, 30)])
    side = poly([(46, 30), (54, 22), (54, 48), (46, 56)])
    t.paint(topf, B[3])
    t.paint(side, B[1])
    emboss(t, front, B, 2)
    for x in range(10, 46):
        t.set(x, 30, B[4])
    for y in range(30, 56):
        t.set(46, y, B[3])
    # Steel corner brackets.
    for x0, y0 in ((10, 30), (42, 30), (10, 52), (42, 52)):
        emboss(t, box(x0, y0, x0 + 4, y0 + 4), STEEL, 2)
    # The label.
    label = box(16, 37, 40, 49)
    t.paint(label, (222, 212, 182, 255))
    lines_box(t, 16, 37, 40, 49, (240, 232, 208, 255), (170, 160, 130, 255))
    text(t, 28 - text_w("FUSE") // 2, 41, "FUSE", (60, 26, 18, 255))
    # The brass igniter, and the cord out of it, burning.
    cylinder(t, box(28, 16, 36, 26), 28, 36, BRASS)
    t.paint(ellipse(32, 16, 4, 1.6), BRASS[4])
    CORD = ramp((200, 180, 140), dark=0.5, light=0.35)
    path = []
    for i in range(70):
        s = i / 69
        path.append((32 - 22 * s + 5 * math.sin(s * math.pi * 1.6), 15 - 9 * s - 3 * math.sin(s * math.pi)))
    for i, (x, y) in enumerate(path):
        for dx, dy, k in ((0, 0, 2), (1, 0, 1), (0, 1, 1), (-1, 0, 3), (0, -1, 3)):
            t.set(int(x) + dx, int(y) + dy, CORD[min(4, k + (i // 5) % 2)])
    item_outline(t)
    ex, ey = int(path[-1][0]), int(path[-1][1])
    for dx, dy, c in ((0, 0, (255, 255, 230)), (1, 0, (255, 220, 90)), (-1, 0, (255, 220, 90)), (0, 1, (255, 220, 90)),
                      (0, -1, (255, 220, 90)), (2, 0, (255, 150, 40)), (-2, 0, (255, 150, 40)), (0, -2, (255, 150, 40)),
                      (0, 2, (255, 150, 40)), (-2, -2, (255, 190, 60)), (2, -2, (255, 190, 60)), (-2, 2, (255, 190, 60)),
                      (3, -3, (255, 120, 30)), (-3, -1, (255, 120, 30))):
        t.set(ex + dx, ey + dy, c)


def remote(t, rng):
    PL = ramp((70, 74, 82), dark=0.5, light=0.35)
    cylinder(t, box(40, 6, 43, 18), 40, 43, STEEL)
    tip = disc(41.5, 5, 3)
    sphere(t, tip, 41.5, 5, 3, RED, lo=0.5)
    body = rrect(16, 16, 48, 60, 5)
    emboss(t, body, PL, 2, depth=2)
    for y in range(36, 56, 3):
        t.set(17, y, PL[0])
        t.set(46, y, PL[0])
    # The screen.
    LCD = [(28, 64, 38, 255), (48, 112, 66, 255), (90, 196, 118, 255), (150, 236, 168, 255)]
    t.rect(20, 20, 44, 33, INK[2])
    lines_box(t, 20, 20, 44, 33, INK[1], PL[3])
    for y in range(22, 31):
        for x in range(22, 42):
            t.set(x, y, LCD[2] if y % 2 else LCD[1] if x > 40 else LCD[2])
    text(t, 23, 24, "ARM", LCD[0])
    for i in range(4):
        for y in range(29 - 2 * i, 30):
            t.set(36 + i * 2, y, LCD[0])
    t.set(22, 22, LCD[3])
    t.set(23, 22, LCD[3])
    t.set(22, 23, LCD[3])
    # The big red button in a striped guard.
    cx, cy = 32, 44.5
    for x, y in disc(cx, cy, 9.5, 6.5):
        wedge = int((math.degrees(math.atan2(y + 0.5 - cy, x + 0.5 - cx)) + 360) // 30) % 2
        t.set(x, y, (HAZ if wedge else INK)[2 if (x + 0.5 - cx) + (y + 0.5 - cy) > 0 else 3])
    btn = disc(cx, cy, 6.5)
    sphere(t, btn, cx, cy, 6.5, RED, lo=0.4)
    for x, y in ((21, 55), (40, 55)):
        emboss(t, rrect(x, y, x + 4, y + 3, 1), STEEL, 2.4)
    for x in range(28, 37, 2):
        for y in (55, 57):
            t.set(x, y, PL[0])
    item_outline(t)


def keypad_detonator(t, rng):
    PL = ramp((80, 84, 92), dark=0.5, light=0.35)
    emboss(t, rrect(5, 6, 27, 28, 2), PL, 2)
    t.rect(8, 8, 24, 14, INK[1])
    lines_box(t, 8, 8, 24, 14, INK[0], PL[3])
    text(t, 9, 9, "0417", (240, 70, 50, 255))
    for row in range(3):
        for col in range(3):
            x, y = 8 + col * 4, 16 + row * 4
            emboss(t, box(x, y, x + 3, y + 3), STEEL, 2.6)
    emboss(t, box(20, 16, 24, 27), RED, 2)
    item_outline(t)


def receiver_detonator(t, rng):
    PL = ramp((80, 84, 92), dark=0.5, light=0.35)
    cylinder(t, box(21, 2, 23, 12), 21, 23, STEEL)
    sphere(t, disc(22, 2.5, 1.9), 22, 2.5, 1.9, STEEL, lo=1)
    emboss(t, rrect(5, 11, 27, 29, 2), PL, 2)
    G = ramp((50, 90, 60), dark=0.5, light=0.4)
    emboss(t, box(8, 14, 24, 26), G, 2, sunk=True)
    for y in range(17, 25, 3):
        for x in range(10, 18, 3):
            t.set(x, y, G[0])
            t.set(x + 1, y + 1, G[3])
    t.set(21, 17, GLOW[3])
    t.set(21, 18, GLOW[2])
    t.set(21, 21, REDGLOW[3])
    t.set(21, 22, REDGLOW[2])
    item_outline(t)


add("item", "fuse_detonator", 64, fuse_detonator)
add("item", "remote", 64, remote)
add("item", "keypad_detonator", 32, keypad_detonator)
add("item", "receiver_detonator", 32, receiver_detonator)


# ================================================================ items: the suit and the medicine
SUIT = [(112, 84, 14, 255), (172, 138, 24, 255), (220, 184, 38, 255), (240, 208, 64, 255), (252, 232, 128, 255)]
RUBBER = ramp((44, 44, 48), dark=0.5, light=0.35)
TAPE = ramp((176, 180, 186), dark=0.5, light=0.4)


def folds(t, rng, mask, count):
    cells = sorted(mask)
    for _ in range(count):
        x, y = rng.choice(cells)
        for k in range(rng.randrange(2, 5)):
            if (x, y + k) in mask:
                t.set(x, y + k, SUIT[1])
                t.mul(x + 1, y + k, 1.08)


def hazmat_helmet(t, rng):
    hood = ellipse(16, 14, 11, 11) | rrect(5, 14, 27, 29, 4)
    emboss(t, hood, SUIT, 2)
    folds(t, rng, box(6, 18, 26, 27), 5)
    visor = rrect(9, 8, 23, 20, 3)
    for x, y in visor:
        v = 2.4 - ((x - 9) + (y - 8)) / 9
        if -1 <= (x - 9) - (y - 8) * 1.2 <= 1 and y < 16:
            v = 4.6
        t.set(x, y, pick(GLASS, v, x, y, 0.3))
    outline(t, visor, RUBBER[1])
    mask = disc(16, 24, 3.6)
    sphere(t, mask, 16, 24, 3.6, RUBBER, lo=0.5)
    for x, y in ((15, 23), (17, 23), (16, 25)):
        t.set(x, y, RUBBER[0])
    item_outline(t)


def hazmat_chestplate(t, rng):
    torso = box(8, 7, 24, 29) | rrect(3, 6, 29, 15, 3) | box(2, 10, 7, 24) | box(25, 10, 30, 24)
    emboss(t, torso, SUIT, 2)
    folds(t, rng, box(9, 12, 23, 27), 5)
    for x0 in (2, 25):
        emboss(t, box(x0, 20, x0 + 5, 24), RUBBER, 2)
        for y in range(11, 20):
            t.set(x0 + 4 if x0 == 2 else x0, y, SUIT[1])
    emboss(t, box(12, 6, 20, 9), RUBBER, 2)
    for y in range(9, 29):
        t.set(15, y, SUIT[1])
        t.set(16, y, SUIT[0] if y % 2 else TAPE[3])
    for x in range(8, 24):
        if x not in (15, 16):
            t.set(x, 21, TAPE[4])
            t.set(x, 22, TAPE[2])
    item_outline(t)


def hazmat_leggings(t, rng):
    legs = box(8, 6, 24, 12) | box(8, 6, 15, 29) | box(17, 6, 24, 29)
    emboss(t, legs, SUIT, 2)
    folds(t, rng, legs - box(8, 4, 24, 9), 6)
    emboss(t, box(8, 4, 24, 8), RUBBER, 2)
    t.set(15, 5, TAPE[4])
    t.set(16, 5, TAPE[2])
    for x0 in (9, 18):
        emboss(t, box(x0, 15, x0 + 5, 19), SUIT, 1.2)
        for x in range(x0 - 1, x0 + 6):
            t.set(x, 24, TAPE[4])
            t.set(x, 25, TAPE[2])
    item_outline(t)


def hazmat_boots(t, rng):
    for sx, fx0, fx1 in ((5, 2, 13), (20, 18, 29)):
        shaft = box(sx, 9, sx + 7, 22)
        foot = rrect(fx0, 19, fx1, 27, 2)
        emboss(t, shaft | foot, SUIT, 2)
        emboss(t, box(fx0, 25, fx1, 28), RUBBER, 2)
        toe = rrect(fx0, 21, fx0 + 4, 26, 1) if fx0 < 10 else rrect(fx1 - 4, 21, fx1, 26, 1)
        emboss(t, toe, RUBBER, 2)
        for x in range(sx, sx + 7):
            t.set(x, 14, RUBBER[2])
            t.set(x, 15, RUBBER[1])
        t.set(sx + 5, 14, TAPE[4])
        emboss(t, box(sx, 9, sx + 7, 11), RUBBER, 2)
    item_outline(t)


def anti_rad(t, rng):
    CORK = ramp((120, 82, 44), dark=0.5, light=0.35)
    emboss(t, rrect(12, 2, 20, 8, 1), CORK, 2)
    for x in (14, 17):
        t.set(x, 4, CORK[1])
    GL = (206, 230, 240, 150)
    flask = ellipse(16, 21, 10, 9) | box(13, 8, 19, 14)
    for x, y in flask:
        t.set(x, y, GL)
    LIQ = ramp((236, 132, 36), dark=0.5, light=0.45)
    liquid = {(x, y) for x, y in flask if y >= 17}
    sphere(t, liquid, 16, 21, 10, LIQ, lo=0.5)
    for x, y in liquid:
        if y == 17:
            t.set(x, y, LIQ[4])
    for x, y in ((12, 24), (19, 22), (15, 26), (20, 25)):
        t.set(x, y, LIQ[4])
    for y in range(14, 25):
        t.set(9 if y > 16 else 13, y, (250, 254, 255, 210))
    for x, y in border(flask):
        if y > 7:
            t.set(x, y, (150, 186, 198, 255))
    for x, y in disc(16, 21.5, 3.4):
        t.set(x, y, (246, 244, 236, 255))
    for x, y in ((15, 20), (16, 20), (15, 21), (16, 21), (14, 21), (17, 21), (15, 22), (16, 22), (15, 19), (16, 19),
                 (15, 23), (16, 23)):
        t.set(x, y, RED[2])
    item_outline(t)


def iodine_tablets(t, rng):
    CARD = ramp((206, 210, 216), dark=0.45, light=0.4)
    TAB = ramp((184, 104, 46), dark=0.5, light=0.45)
    card = rrect(3, 5, 29, 28, 1)
    emboss(t, card, CARD, 2)
    for row, ty in enumerate((10.5, 17.5)):
        for col, tx in enumerate((9.5, 16.5, 23.5)):
            taken = row == 1 and col == 2
            bub = disc(tx, ty, 3.2)
            if taken:
                # Pushed through: an empty dimple, the foil torn round its rim.
                for x, y in bub:
                    t.set(x, y, CARD[1] if (x + 0.5 - tx) + (y + 0.5 - ty) < 0 else CARD[3])
                for x, y in disc(tx, ty, 2.2):
                    t.set(x, y, (92, 96, 104, 255))
                for x, y in ((int(tx) - 1, int(ty) - 2), (int(tx) + 1, int(ty) - 1), (int(tx) - 2, int(ty) + 1)):
                    t.set(x, y, CARD[4])
                continue
            for x, y in bub:
                t.set(x, y, CARD[4])
            drop(t, bub, 0.75)
            pill = disc(tx, ty, 2.4)
            sphere(t, pill, tx, ty, 2.4, TAB, lo=0.6)
            for x in range(int(tx) - 1, int(tx) + 2):
                t.set(x, int(ty), TAB[0] if x != int(tx) - 1 else TAB[1])
    text(t, 5, 22, "KI-130", (70, 74, 96, 255))
    item_outline(t)


for piece in ("helmet", "chestplate", "leggings", "boots"):
    add("item", f"hazmat_{piece}", 32, globals()[f"hazmat_{piece}"])
add("item", "anti_rad", 32, anti_rad)
add("item", "iodine_tablets", 32, iodine_tablets)


# ================================================================ items: tools and the rest
def geiger_counter(t, rng):
    Y = ramp((196, 164, 48), dark=0.5, light=0.4)
    PL = ramp((54, 56, 62), dark=0.5, light=0.4)
    handle = rrect(12, 5, 52, 20, 6) - box(18, 11, 46, 20)
    emboss(t, handle, PL, 2)
    body = rrect(8, 16, 56, 59, 4)
    emboss(t, body, Y, 2, depth=2)
    # The dial: a cream face, its scale running into red, the needle up in it.
    t.rect(13, 21, 51, 41, INK[2])
    lines_box(t, 13, 21, 51, 41, INK[1], Y[3])
    face = rrect(15, 23, 49, 39, 2)
    for x, y in face:
        t.set(x, y, (236, 234, 216, 255) if y > 25 else (246, 244, 232, 255))
    pcx, pcy = 32, 40
    for i in range(11):
        a = math.radians(205 + i * 13)
        red = i >= 8
        for r in (12, 13, 14) if i % 2 == 0 else (13, 14):
            t.set(int(pcx + math.cos(a) * r), int(pcy + math.sin(a) * r), RED[2] if red else INK[2])
    na = math.radians(312)
    line(t, pcx, pcy - 1, int(pcx + math.cos(na) * 14), int(pcy - 1 + math.sin(na) * 14), RED[1])
    t.paint(disc(pcx, pcy - 1, 2), INK[1])
    text(t, 17, 33, "CPM", INK[3])
    for i in range(4):
        t.set(16 + i, 24 + i, (255, 255, 255, 255))
    # A knob, a grille, a lamp and the buttons.
    knob = disc(18.5, 48.5, 4.2)
    drop(t, knob, 0.65)
    sphere(t, knob, 18.5, 48.5, 4.2, PL, lo=0.6)
    line(t, 18, 48, 16, 46, Y[4])
    for y in range(45, 54, 3):
        for x in range(26, 40, 3):
            t.set(x, y, INK[1])
            t.set(x + 1, y + 1, Y[4])
    sphere(t, disc(47, 46.5, 2.6), 47, 46.5, 2.6, GLOW, lo=1.5)
    for x in (43, 48):
        emboss(t, box(x, 52, x + 4, 55), PL, 2)
    item_outline(t)


def guide(t, rng):
    G = ramp((48, 82, 40), dark=0.5, light=0.35)
    PAGES = [(170, 156, 120, 255), (214, 202, 168, 255), (236, 226, 196, 255)]
    for y in range(4, 28):
        t.set(26, y, PAGES[2] if y % 2 else PAGES[1])
        t.set(27, y, PAGES[1] if y % 2 else PAGES[0])
    for x in range(8, 27):
        t.set(x, 28, PAGES[1] if x % 2 else PAGES[0])
    cover = rrect(5, 3, 26, 28, 1)
    emboss(t, cover, G, 2)
    spine = box(4, 3, 8, 29)
    emboss(t, spine, ramp((32, 56, 28)), 2)
    for y in (6, 24):
        emboss(t, box(4, y, 8, y + 2), BRASS, 2.5)
    for x, y in ((24, 3), (25, 3), (25, 4), (24, 27), (25, 27), (25, 26)):
        t.set(x, y, BRASS[3])
    cx, cy = 16.5, 15.5
    ring(t, cx, cy, 5.2, 7, BRASS, mid=2.4)
    t.paint(disc(cx, cy, 5.2), (226, 190, 48, 255))
    t.paint(trefoil(cx, cy, 5.2), (24, 22, 16, 255))
    item_outline(t)


def hermetic_door_item(t, rng):
    leaf = rrect(8, 2, 24, 30, 1)
    emboss(t, leaf, DOOR, 2)
    for y in range(3, 29):
        t.set(10, y, DOOR[1])
    ring(t, 17, 9, 2.6, 4.2, STEEL, mid=2.2)
    for x, y in disc(17, 9, 2.6):
        t.set(x, y, GLASS[4] if (x - 17) + (y - 9) < -1 else GLASS[2])
    ring(t, 17, 18.5, 2.4, 4.4, RED, mid=2)
    t.set(16, 18, STEEL[4])
    t.set(17, 18, STEEL[3])
    t.set(16, 19, STEEL[3])
    t.set(17, 19, STEEL[1])
    hazard(t, 11, 25, 23, 28, period=6, rims=False)
    for y in (5, 24):
        emboss(t, box(6, y, 9, y + 3), DARK_STEEL, 2)
    item_outline(t)


def ration(t, rng):
    TIN = ramp((170, 174, 180), dark=0.5, light=0.45)
    LABEL = ramp((92, 104, 56), dark=0.5, light=0.35)
    body = box(7, 9, 25, 27) | ellipse(16, 27, 9, 2.5)
    cylinder(t, body, 7, 25, TIN, lo=0.3)
    cylinder(t, box(7, 13, 25, 24), 7, 25, LABEL, lo=0.3)
    for y in (11, 25):
        for x in range(7, 25):
            t.mul(x, y, 0.75)
            t.mul(x, y - 1, 1.15)
    lid = ellipse(16, 8.5, 9, 3)
    for x, y in lid:
        t.set(x, y, TIN[3] if y < 8 else TIN[2])
    for x, y in ellipse(16, 8.5, 7, 2):
        t.set(x, y, TIN[2] if y < 8 else TIN[1])
    t.set(19, 8, TIN[4])
    t.set(20, 8, TIN[4])
    t.set(20, 9, TIN[0])
    text(t, 16 - text_w("FOOD") // 2, 15, "FOOD", (214, 196, 104, 255))
    for x in range(9, 23):
        t.set(x, 21, (196, 180, 96, 255) if x % 3 else LABEL[3])
    item_outline(t)


add("item", "geiger_counter", 64, geiger_counter)
add("item", "guide", 32, guide)
add("item", "hermetic_door", 32, hermetic_door_item)
add("item", "ration", 32, ration)


# ================================================================ items: the missile and the flying models' sheets
def missile(t, rng):
    WHITE = [(112, 116, 126, 255), (160, 164, 170, 255), (204, 206, 204, 255), (234, 234, 230, 255),
             (250, 250, 246, 255)]
    ax, ay = 1 / math.sqrt(2), -1 / math.sqrt(2)
    qx, qy = 1 / math.sqrt(2), 1 / math.sqrt(2)
    ox, oy = 15.0, 49.0
    L, rb = 56.0, 6.5
    lp = LX * qx + LY * qy

    def radius(s):
        if s < 4:
            return 4.6 + (4 - s) * 0.25
        if s > L - 15:
            q = 1 - ((s - (L - 15)) / 15) ** 2
            return rb * math.sqrt(max(0.0, q))
        return rb

    flame = []
    for y in range(t.n):
        for x in range(t.n):
            dx, dy = x + 0.5 - ox, y + 0.5 - oy
            s, q = dx * ax + dy * ay, dx * qx + dy * qy
            r = radius(s)
            if 0 <= s <= L and abs(q) < r:
                nx = q / r
                l = nx * lp + math.sqrt(max(0.0, 1 - nx * nx)) * 0.75
                v = 0.4 + 3.6 * (l * 0.5 + 0.5)
                if s < 4:
                    R = DARK_STEEL
                elif s > L - 15 or L - 26 < s < L - 22 or 4 <= s < 7:
                    R = RED
                else:
                    R = WHITE
                c = pick(R, v, x, y, 0.4)
                if abs(s - 18) < 0.5 or abs(s - 32) < 0.5:
                    c = shade(c, 0.72)
                t.set(x, y, c)
            elif 6 < s < 22 and r <= abs(q) < r + 8.5 * (1 - (s - 6) / 16):
                t.set(x, y, RED[3] if q < 0 else RED[1])
                if abs(s - 6.5) < 0.7:
                    t.set(x, y, RED[0])
            elif -15 < s < 0 and abs(q) < 4.2 * (1 + s / 15) + 0.4:
                flame.append((x, y, -s / 15, abs(q) / (4.2 * (1 + s / 15) + 0.4)))
    item_outline(t)
    FIRE = [(220, 70, 20, 255), (250, 140, 30, 255), (255, 200, 70, 255), (255, 240, 170, 255), (255, 255, 236, 255)]
    for x, y, along, across in flame:
        if t.px[x, y][3] == 0:
            v = 4.2 - along * 3 - across * 1.8 + rng.uniform(-0.4, 0.4)
            t.set(x, y, pick(FIRE, v, x, y, 0.5))


def sheet_quarter(t, rng, ox, oy, base, kind):
    q = t.n // 2
    R = ramp(base, dark=0.45, light=0.3)
    f = vnoise(t.n, rng, 3)
    for y in range(oy, oy + q):
        for x in range(ox, ox + q):
            t.set(x, y, pick(R, 2 + (f[y][x] - 0.5) * 0.6, x, y, 0.2))
    if kind == "paint":
        for x in range(ox, ox + q):
            t.set(x, oy + q // 2, R[1])
            t.set(x, oy + q // 2 + 1, R[3])
        for x in range(ox + 3, ox + q - 2, 6):
            for y in (oy + 3, oy + q - 4):
                t.set(x, y, R[3])
                t.set(x + 1, y + 1, R[1])
    elif kind == "metal":
        for x in range(ox, ox + q, q // 2):
            for y in range(oy, oy + q):
                t.set(x, y, R[1])
        for y in range(oy + 3, oy + q, 6):
            t.set(ox + q // 4, y, R[3])
            t.set(ox + 3 * q // 4, y, R[3])
    elif kind == "glass":
        for x, y in box(ox, oy, ox + q, oy + q):
            if (x - ox) % 8 == 0 or (y - oy) % 8 == 0:
                t.set(x, y, shade(base, 0.45))
            elif 6 < (x - ox) % 8 + (y - oy) % 8 < 9:
                t.set(x, y, R[4])
    elif kind == "nozzle":
        cx, cy = ox + q / 2, oy + q / 2
        for r in range(3, q // 2, 4):
            for x, y in disc(cx, cy, r + 1, r):
                t.set(x, y, R[1] if (x - cx) + (y - cy) < 0 else R[3])
    elif kind == "prop":
        cx, cy = ox + q / 2, oy + q / 2
        for x, y in disc(cx, cy, q / 2 - 1, q / 2 - 4):
            t.set(x, y, R[3] if bayer(x, y) < 0.5 else R[2])
        t.paint(disc(cx, cy, 3), R[4])


def flying_sheet(t, rng, quarters):
    for i, (base, kind) in enumerate(quarters):
        sheet_quarter(t, rng, (i % 2) * t.n // 2, (i // 2) * t.n // 2, base, kind)


add("item", "missile", 64, missile)
add("item", "missile_model", 64, flying_sheet,
    [((234, 234, 228), "paint"), ((200, 36, 32), "paint"), ((60, 62, 66), "nozzle"), ((22, 22, 24), "flat")])
add("item", "bomber_model", 64, flying_sheet,
    [((70, 76, 66), "metal"), ((128, 134, 126), "metal"), ((90, 140, 170), "glass"), ((26, 26, 28), "prop")])


# ================================================================ write them all
for kind, name, size, fn, args in TEXTURES:
    tex = Tex(size)
    fn(tex, seeded(name), *args)
    tex.save(kind, name)
print(f"{len(TEXTURES)} block and item textures written")


# ================================================================ fixed sizes: the screens, the suit on a player, the effect icons
# Drawn the same as before, pixel for pixel: their layouts are the code's pixel coordinates.
def save(im, kind, name):
    path = TEX / kind / f"{name}.png"
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path)


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


# ---- the suit on a player and the radiation's icon (plan 5) ----
suit_rng = random.Random(2605)
SUIT_CLOTH = (226, 190, 40)
SUIT_DARK = (170, 136, 22, 255)
SUIT_RUBBER = (40, 40, 44, 255)
VISOR = (36, 62, 72, 255)
SHINE = (150, 205, 215, 255)



# The suit on a player: 64x32 sheets in vanilla's armour layout. `humanoid` is the
# helmet, the jacket with its sleeves, and the boots on the legs' boxes;
# `humanoid_leggings` the legs and the waist.
def uv_box(im, u, v, w, h, d, paint):
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
    return shade(SUIT_CLOTH, (0.86 if edge else 1.0) + suit_rng.uniform(-0.06, 0.06))


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
        return SUIT_RUBBER
    return cloth(x, y, fw, fh)


def boot(face, x, y, fw, fh):
    if face == "top":
        return None
    if face == "bottom" or (face != "top" and y == fh - 1):
        return SUIT_RUBBER
    return cloth(x, y, fw, fh) if y >= fh - 6 else None


def waist(face, x, y, fw, fh):
    if face == "top":
        return None
    if face == "bottom":
        return cloth(x, y, fw, fh)
    if y == fh - 5:
        return SUIT_RUBBER
    return cloth(x, y, fw, fh) if y > fh - 5 else None


def leg(face, x, y, fw, fh):
    return cloth(x, y, fw, fh)


layer = Image.new("RGBA", (64, 32), CLEAR)
uv_box(layer, 0, 0, 8, 8, 8, head)
uv_box(layer, 16, 16, 8, 12, 4, jacket)
uv_box(layer, 40, 16, 4, 12, 4, sleeve)
uv_box(layer, 0, 16, 4, 12, 4, boot)
save(layer, "entity/equipment/humanoid", "hazmat")
layer = Image.new("RGBA", (64, 32), CLEAR)
uv_box(layer, 0, 16, 4, 12, 4, leg)
uv_box(layer, 16, 16, 8, 12, 4, waist)
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


# ---- the guide's screen (plan 6) ----
guide_rng = random.Random(2611)
LEATHER = (74, 44, 26)
LEATHER_EDGE = (36, 20, 12, 255)
LEATHER_LIGHT = (112, 72, 42, 255)
STITCH = (156, 116, 72, 255)
GUIDE_BRASS = (196, 158, 72, 255)
BRASS_DARK = (128, 98, 40, 255)
PARCHMENT = (233, 221, 184)
PAGE_EDGE = (186, 166, 126, 255)
GUIDE_INK = (96, 72, 44, 255)


def guide_grain(base, spread):
    return shade(base, 1 + guide_rng.uniform(-spread, spread))


# The guide's old 16x16 item took 150 draws of this random before the screen; they are still taken, so the
# screen stays the same.
for _ in range(150):
    guide_rng.random()


# The screen: the spread, the bookmarks, the arrows, a slot and a recipe's arrow.
W, H, BOOK_W, BOOK_H = 288, 264, 288, 192
gui = Image.new("RGBA", (W, H), CLEAR)
for y in range(BOOK_H):
    for x in range(BOOK_W):
        gui.putpixel((x, y), guide_grain(LEATHER, 0.07))
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
            gui.putpixel((x, y), GUIDE_BRASS if i + j < 6 else BRASS_DARK)
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
            gui.putpixel((262 + x, 192 + y), GUIDE_INK)
save(gui, "gui", "guide")


# ---- the iodine's icon and the decontaminator's screen (plan 9) ----
TABLET = (184, 104, 46)
WATER_RGB = (60, 150, 210)

# The iodine's effect icon, 18x18: one tablet, scored across, with a dark rim.
icon = Image.new("RGBA", (18, 18), CLEAR)
for y in range(18):
    for x in range(18):
        d = math.hypot(x - 8.5, y - 8.5)
        if d < 6.2:
            c = shade(TABLET, 1.15 - 0.05 * (y - 2.5) / 2)
            if y in (8, 9) and abs(x - 8.5) < 5:
                c = shade(TABLET, 0.65)
            icon.putpixel((x, y), c)
        elif d < 7.4:
            icon.putpixel((x, y), (70, 36, 14, 255))
for x, y in ((6, 5), (7, 4), (7, 5)):
    icon.putpixel((x, y), (250, 214, 160, 255))
save(icon, "mob_effect", "iodine")

# The decontaminator's screen: a furnace's panel with a radius bar's trough over the fuel slot, the flame left of
# the slot; right of the panel the lit flame (176, 0), under it the full bar (0, 166).
gui = Image.new("RGBA", (192, 176), CLEAR)
rect(gui, 0, 0, 176, 166, (198, 198, 198, 255))
rect(gui, 0, 0, 176, 2, (255, 255, 255, 255))
rect(gui, 0, 0, 2, 166, (255, 255, 255, 255))
rect(gui, 0, 164, 176, 2, (85, 85, 85, 255))
rect(gui, 174, 0, 2, 166, (85, 85, 85, 255))
rect(gui, 33, 17, 110, 10, (255, 255, 255, 255))
rect(gui, 33, 17, 109, 9, (55, 55, 55, 255))
rect(gui, 34, 18, 108, 8, (139, 139, 139, 255))
slot(gui, 80, 53)
for row in range(3):
    for col in range(9):
        slot(gui, 8 + col * 18, 84 + row * 18)
for col in range(9):
    slot(gui, 8 + col * 18, 142)
flame(gui, 62, 54, False)
flame(gui, 176, 0, True)
for x in range(108):
    t = x / 107
    base = tuple(int(WATER_RGB[i] + ((110, 220, 120)[i] - WATER_RGB[i]) * t) for i in range(3))
    for y in range(8):
        k = 1.25 if y == 0 else 0.7 if y == 7 else 1.0 - 0.03 * y
        gui.putpixel((x, 166 + y), shade(base, k))
save(gui, "gui", "decontaminator")
print("screens, suit and effect icons written")
