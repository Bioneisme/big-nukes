#!/usr/bin/env python3
"""Checks the assembly table in the game: its screen opens, Assemble takes exactly a recipe's
parts, foreign items and a full output refuse, a hopper below pulls the result, and only the
table, the centrifuge, the detonators and the remote have recipes for the crafting table.

    python tools/assembly_test.py GAME_FOLDER VERSION
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
WORLD = "bn-assembly"
# The panel is 176x180 GUI pixels, centred; the button is [96, 47, 72, 18] inside it.
PANEL = (176, 180)
BUTTON = (96 + 36, 47 + 9)
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


def count(x, y, z, item, slot="container.*"):
    said = chat(f"execute if items block {x} {y} {z} {slot} {item}")
    digits = "".join(c for c in said.split(":")[-1] if c.isdigit()) if ("passed" in said or "пройден" in said) else ""
    return int(digits) if digits else 0


def fill(x, y, z, slots):
    for i in range(10):
        chat(f"item replace block {x} {y} {z} container.{i} with minecraft:air")
    for i, (item, n) in slots.items():
        chat(f"item replace block {x} {y} {z} container.{i} with {item} {n}")


def press(x, y, z, shot=None):
    ctl("interact", x, y, z)
    ctl("wait", 3)
    path = os.path.join(SHOTS, f"assembly-{shot or 'press'}.png")
    ctl("screenshot", path)
    from PIL import Image
    w, h = Image.open(path).size
    scale = 2
    gx, gy = (w // scale - PANEL[0]) // 2 + BUTTON[0], (h // scale - PANEL[1]) // 2 + BUTTON[1]
    ctl("click-at", gx, gy)
    ctl("wait", 3)
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
y = cy + 6
chat(f"fill {cx - 6} {y - 3} {cz - 6} {cx + 6} {y + 3} {cz + 6} minecraft:air")
chat(f"fill {cx - 6} {y - 3} {cz - 6} {cx + 6} {y - 3} {cz + 6} minecraft:stone")
tx, tz = cx, cz + 3
chat(f"setblock {tx} {y} {tz} big_nukes:assembly_table")
chat(f"tp @s {cx + 0.5} {y} {cz + 0.5} 0 30")
ctl("wait", 5)

opened = ctl("interact", tx, y, tz)
menu = ctl("menu")
slots = {s.get("slot") for s in menu.get("slots", [])}
print(" menu:", json.dumps(menu, ensure_ascii=False)[:400])
ctl("screenshot", os.path.join(SHOTS, "assembly-empty.png"))
ctl("close")
check("right-click opens the table's screen", menu.get("screen") not in (None, "none") and "not available" not in json.dumps(opened),
      str(opened)[:300])

# 1 enriched uranium and 6 TNT over three slots make one charge and leave 2 TNT.
fill(tx, y, tz, {0: ("big_nukes:enriched_uranium", 1), 1: ("minecraft:tnt", 3), 4: ("minecraft:tnt", 3)})
press(tx, y, tz, "charge")
check("Assemble makes one nuclear charge", count(tx, y, tz, "big_nukes:nuclear_charge", "container.9") == 1,
      f"output {count(tx, y, tz, 'big_nukes:nuclear_charge')}")
check("it takes exactly 1 uranium and 4 TNT", count(tx, y, tz, "minecraft:tnt") == 2 and count(tx, y, tz, "big_nukes:enriched_uranium") == 0,
      f"tnt {count(tx, y, tz, 'minecraft:tnt')}")

# A stick in the grid refuses.
fill(tx, y, tz, {0: ("big_nukes:enriched_uranium", 1), 1: ("minecraft:tnt", 4), 2: ("minecraft:stick", 1)})
press(tx, y, tz, "foreign")
check("a foreign item assembles nothing", count(tx, y, tz, "big_nukes:nuclear_charge") == 0 and count(tx, y, tz, "minecraft:tnt") == 4)

# A different item in the output refuses.
fill(tx, y, tz, {0: ("big_nukes:enriched_uranium", 1), 1: ("minecraft:tnt", 4), 9: ("minecraft:dirt", 1)})
press(tx, y, tz, "full")
check("an occupied output refuses", count(tx, y, tz, "big_nukes:nuclear_charge") == 0 and count(tx, y, tz, "minecraft:tnt") == 4)

# The atomic bomb, then a hopper below pulls it out.
fill(tx, y, tz, {0: ("big_nukes:nuclear_charge", 4), 1: ("big_nukes:bomb_casing", 4), 2: ("minecraft:redstone_block", 1)})
press(tx, y, tz, "atomic")
check("4 charges, 4 casings and a redstone block make an atomic bomb", count(tx, y, tz, "big_nukes:atomic_bomb", "container.9") == 1)
chat(f"setblock {tx} {y - 1} {tz} minecraft:hopper")
ctl("wait", 20)
check("a hopper below pulls the result", count(tx, y - 1, tz, "big_nukes:atomic_bomb") == 1)

# Crafting-table recipes: only the table, the centrifuge, the detonators and the remote.
for name in ("assembly_table", "centrifuge", "fuse_detonator", "keypad_detonator", "receiver_detonator", "remote"):
    said = chat(f"recipe give @s big_nukes:{name}")
    check(f"a recipe for {name}", "nlocked" in said or "Открыт" in said or "рецепт" in said.lower(), said[:200])
for name in ("atomic_bomb", "tactical_warhead", "demolition_charge", "nuclear_charge", "bomb_casing"):
    said = chat(f"recipe give @s big_nukes:{name}")
    check(f"no crafting-table recipe for {name}", "nlocked" not in said, said[:200])
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
