#!/usr/bin/env python3
"""Checks the bombs in the game: detonators go in once, the demolition charge takes only a fuse,
flint and steel sets a fuse off after 4 s, a keypad takes its first code, refuses a wrong one,
locks after three and fires on the right one, and a broken bomb drops its detonator.

    python tools/bomb_test.py GAME_FOLDER VERSION

Blasts are counted by the world's horizon-big-nukes/waves.json (`next_id`).
"""
import json
import os
import shutil
import subprocess
import sys
import time

GAME, VERSION = sys.argv[1:3]
CTL = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", VERSION, "horizon-ctl.exe")
os.environ["HORIZON_HOME"] = GAME
os.environ["MSYS_NO_PATHCONV"] = "1"
sys.stdout.reconfigure(encoding="utf-8")
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
os.makedirs(SHOTS, exist_ok=True)
WORLD = "bn-bombs"
SAVED = os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json")
# The keypad's panel is 176x222 GUI pixels; key (row, col) is 32x16 at (36 + 36 col, 48 + 20 row).
PANEL = (176, 222)
KEY_AT = {1: (0, 0), 2: (0, 1), 3: (0, 2), 4: (1, 0), 5: (1, 1), 6: (1, 2), 7: (2, 0), 8: (2, 1), 9: (2, 2),
          "clear": (3, 0), 0: (3, 1), "enter": (3, 2)}
failed = []


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def chat(line):
    return " ".join(map(str, ctl("command", line).get("chat", [])))


def check(name, ok, detail=""):
    print(("ok  " if ok else "FAIL"), name, flush=True)
    if not ok:
        failed.append(f"{name}: {detail}")


def blasts():
    ctl("save")
    try:
        with open(SAVED, encoding="utf-8") as f:
            return json.load(f).get("next_id", 0)
    except (OSError, ValueError):
        return 0


def said_since(n):
    return " ".join(map(str, ctl("messages", n).get("chat", ctl("messages", n).get("messages", []))))


def hold(item):
    chat("clear @s")
    if item:
        chat(f"give @s {item}")
    ctl("select", 1)
    ctl("wait", 2)


def click(x, y, z):
    """Right-click the block with the held item, from beside it."""
    chat(f"tp @s {x + 0.5} {y} {z + 2.5} 180 35")
    ctl("wait", 3)
    return ctl("interact", x, y, z)


def gone(x, y, z):
    return "big_nukes" not in json.dumps(ctl("block", x, y, z))


def press_keys(keys):
    path = os.path.join(SHOTS, "keypad.png")
    ctl("screenshot", path)
    from PIL import Image
    w, h = Image.open(path).size
    ox, oy = (w // 2 - PANEL[0]) // 2, (h // 2 - PANEL[1]) // 2
    for k in keys:
        row, col = KEY_AT[k]
        ctl("click-at", ox + 36 + 36 * col + 16, oy + 48 + 20 * row + 8)
        ctl("wait", 2)


def keypad(x, y, z, keys):
    hold(None)
    click(x, y, z)
    ctl("wait", 3)
    # Without the screen the presses would be left clicks on the world, and in creative they break the bomb.
    screen = ctl("screen").get("screen")
    check("the keypad's screen opens", screen == "container", str(screen))
    if screen != "container":
        return
    press_keys(keys)
    ctl("close")
    ctl("wait", 2)


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
ctl("wait", 60)
for _ in range(60):
    player = ctl("state").get("player") or {}
    pos = player.get("pos") or player.get("position")
    if pos:
        break
    ctl("wait", 10)
cx, cy, cz = [int(v // 1) for v in pos]
y = 230  # above any terrain, so the bombs stand in the open and the keypad's screen isn't hidden
chat(f"fill {cx - 70} {y - 1} {cz - 4} {cx + 70} {y - 1} {cz + 4} minecraft:stone")

# The demolition charge: a fuse goes in once, a keypad never.
dx, dz = cx - 60, cz
chat(f"setblock {dx} {y} {dz} big_nukes:demolition_charge")
hold("big_nukes:keypad_detonator")
click(dx, y, dz)
hold("big_nukes:fuse_detonator 2")
r = click(dx, y, dz)
check("the furnace menu doesn't open on a bomb", "furnace" not in json.dumps(r).lower(), str(r)[:300])
click(dx, y, dz)
lines = json.dumps(ctl("messages", 10), ensure_ascii=False)
print(" chat:", lines[:600])
check("a second detonator is refused", "already" in lines or "уже" in lines, lines[:300])
check("the keypad was refused", "only a fuse" in lines or "только фитильный" in lines, lines[:300])

# Flint and steel: the fuse hisses 4 s, then the blast takes the bomb.
before = blasts()
hold("minecraft:flint_and_steel")
click(dx, y, dz)
click(dx, y, dz)
chat(f"tp @s {cx} {y + 20} {cz} 0 0")
ctl("wait", 60)
check("after 3 s the bomb still stands", not gone(dx, y, dz))
ctl("wait", 40)
check("after 5 s the fuse has set it off", gone(dx, y, dz), json.dumps(ctl("block", dx, y, dz))[:200])
check("lighting twice makes one blast", blasts() - before == 1, f"{blasts() - before}")

# The keypad: the first code is set, a wrong one is refused, the right one fires after 5 s.
kx, kz = cx + 60, cz
chat(f"setblock {kx} {y} {kz} big_nukes:tactical_warhead")
hold("big_nukes:keypad_detonator")
click(kx, y, kz)
said = json.dumps(ctl("messages", 4).get("chat", []), ensure_ascii=False)
check("the keypad goes into the tactical warhead", "installed" in said or "установлен" in said, said)
before = blasts()
keypad(kx, y, kz, [1, 2, 3, "enter"])
keypad(kx, y, kz, [1, 1, 1, "enter"])
ctl("wait", 120)
check("a wrong code sets nothing off", not gone(kx, y, kz) and blasts() == before)
keypad(kx, y, kz, [1, 1, 1, "enter", 1, 1, 1, "enter"])
keypad(kx, y, kz, [1, 2, 3, "enter"])
ctl("wait", 120)
check("after three wrong codes the pad is locked", not gone(kx, y, kz) and blasts() == before)
ctl("wait", 500)
keypad(kx, y, kz, [1, 2, 3, "enter"])
chat(f"tp @s {cx} {y + 20} {cz} 0 0")
ctl("wait", 80)
check("4 s after the right code the bomb stands", not gone(kx, y, kz))
ctl("wait", 40)
check("the right code sets it off after 5 s", gone(kx, y, kz) and blasts() == before + 1, f"{blasts() - before}")

# In survival, lighting a fuse wears the flint and steel by one use.
sx, sz = cx - 30, cz
chat(f"setblock {sx} {y} {sz} big_nukes:demolition_charge")
hold("big_nukes:fuse_detonator")
click(sx, y, sz)
chat("gamemode survival")
hold("minecraft:flint_and_steel")
click(sx, y, sz)
ctl("wait", 5)
inv = json.dumps(ctl("inventory"), ensure_ascii=False)
check("in survival the flint and steel loses a use", '"damage": 1' in inv, inv[:300])
chat("gamemode creative")
chat(f"tp @s {cx} {y + 20} {cz} 0 0")
ctl("wait", 120)

# A broken bomb drops its detonator.
bx, bz = cx, cz + 3
chat(f"setblock {bx} {y} {bz} big_nukes:atomic_bomb")
hold("big_nukes:receiver_detonator")
click(bx, y, bz)
chat("kill @e[type=item]")
chat(f"setblock {bx} {y} {bz} minecraft:air destroy")
ctl("wait", 5)
items = json.dumps(ctl("entities"), ensure_ascii=False)
check("a broken bomb drops its detonator", "receiver_detonator" in items, items[:300])
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
