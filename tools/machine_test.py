#!/usr/bin/env python3
"""Checks the centrifuge in the game: hoppers feed it and empty it, coal from above is refused,
progress survives a reload, and its screen opens and takes a shift-click.

    python tools/machine_test.py GAME_FOLDER VERSION
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
WORLD = "bn-machine"
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
    """How many of ITEM the block's slots hold, by /execute if items."""
    said = chat(f"execute if items block {x} {y} {z} {slot} {item}")
    digits = "".join(c for c in said.split(":")[-1] if c.isdigit()) if ("passed" in said or "пройден" in said) else ""
    return int(digits) if digits else 0


def open_world():
    ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)
    for _ in range(60):
        player = ctl("state").get("player") or {}
        pos = player.get("pos") or player.get("position")
        if pos:
            return [int(v // 1) for v in pos]
        ctl("wait", 10)
    raise RuntimeError("no player")


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
y = cy + 6
chat(f"fill {cx - 8} {y - 3} {cz - 8} {cx + 8} {y + 3} {cz + 8} minecraft:air")
chat(f"fill {cx - 8} {y - 3} {cz - 8} {cx + 8} {y - 3} {cz + 8} minecraft:stone")

# A: a hopper above feeds two raw uranium, one at the side feeds coal, one below empties into a chest.
ax, az = cx + 4, cz
chat(f"setblock {ax} {y} {az} big_nukes:centrifuge")
chat(f"setblock {ax} {y + 1} {az} minecraft:hopper[facing=down]")
chat(f"setblock {ax - 1} {y} {az} minecraft:hopper[facing=east]")
chat(f"setblock {ax} {y - 1} {az} minecraft:hopper[facing=down]")
chat(f"setblock {ax} {y - 2} {az} minecraft:chest")
chat(f"item replace block {ax} {y + 1} {az} container.0 with big_nukes:raw_uranium 2")
chat(f"item replace block {ax - 1} {y} {az} container.0 with minecraft:coal 1")

# B: coal offered from above stays in the hopper.
bx, bz = cx - 4, cz
chat(f"setblock {bx} {y} {bz} big_nukes:centrifuge")
chat(f"setblock {bx} {y + 1} {bz} minecraft:hopper[facing=down]")
chat(f"item replace block {bx} {y + 1} {bz} container.0 with minecraft:coal 3")

# C: filled by command, for the reload.
qx, qz = cx, cz + 4
chat(f"setblock {qx} {y} {qz} big_nukes:centrifuge")
said = chat(f"item replace block {qx} {y} {qz} container.0 with big_nukes:raw_uranium 1")
check("the centrifuge's input slot takes an item by command", "Replaced" in said or "Заменён" in said or count(qx, y, qz, "big_nukes:raw_uranium") == 1, said)
chat(f"item replace block {qx} {y} {qz} container.1 with minecraft:coal 1")

ctl("wait", 200)
check("C: half spun, nothing out yet", count(qx, y, qz, "big_nukes:enriched_uranium") == 0)
ctl("quit")
time.sleep(8)

open_world()
ctl("wait", 260)
check("C: progress survived the reload (done 260 ticks after it)", count(qx, y, qz, "big_nukes:enriched_uranium", "container.2") == 1)
ctl("wait", 700)
check("A: two enriched uranium reached the chest below", count(ax, y - 2, az, "big_nukes:enriched_uranium") == 2,
      f"chest {count(ax, y - 2, az, 'big_nukes:enriched_uranium')}, centrifuge {count(ax, y, az, 'big_nukes:enriched_uranium')}")
check("A: no raw uranium left over", count(ax, y + 1, az, "big_nukes:raw_uranium") + count(ax, y, az, "big_nukes:raw_uranium") == 0)
check("B: coal from above stays in the hopper", count(bx, y + 1, bz, "minecraft:coal") == 3)

# The screen: open it and shift-click raw uranium and coal from the hotbar.
chat("clear @s")
chat("give @s big_nukes:raw_uranium 5")
chat("give @s minecraft:coal 7")
chat(f"tp @s {cx + 0.5} {y} {cz + 2.5} 180 30")
ctl("wait", 5)
opened = ctl("interact", qx, y, qz)
menu = ctl("menu")
print(" menu:", json.dumps(menu, ensure_ascii=False)[:700])
check("right-click opens the centrifuge's screen", "centrifuge" in json.dumps(menu, ensure_ascii=False).lower() or menu.get("slots"), str(opened)[:300])
ctl("screenshot", os.path.join(SHOTS, "centrifuge-screen.png"))
ctl("click-slot", 0, "shift")
ctl("click-slot", 1, "shift")
ctl("wait", 2)
inside_raw = count(qx, y, qz, "big_nukes:raw_uranium", "container.0")
inside_coal = count(qx, y, qz, "minecraft:coal", "container.1")
check("shift-click puts raw uranium in the input", inside_raw >= 5, f"input {inside_raw}")
check("shift-click puts coal in the fuel slot", inside_coal >= 7, f"fuel {inside_coal}")
ctl("close")
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
