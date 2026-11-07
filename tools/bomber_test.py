#!/usr/bin/env python3
"""Checks the bomber in the game: a right click on a strike beacon with a tactical warhead takes the bomb and calls
a bomber; 30 s later a plane flies over as a display (target/shots/bomber-*.png), drops its bomb, and the blast goes
off within 4 blocks of the beacon; a beacon broken before the pass drops its bomb.

    python tools/bomber_test.py GAME_FOLDER VERSION
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
WORLD = "bn-bomber"
SAVED = os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json")
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
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


def saved():
    ctl("save")
    try:
        with open(SAVED, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def displays():
    said = chat("execute if entity @e[type=minecraft:item_display]")
    digits = "".join(c for c in said.split(":")[-1] if c.isdigit())
    return int(digits) if "passed" in said.lower() and digits else 0


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


def call(bx, by, bz):
    chat("clear @s")
    chat("give @s big_nukes:tactical_warhead")
    ctl("select", 1)
    ctl("look-at", "block", bx, by, bz)
    ctl("interact", bx, by, bz)
    ctl("wait", 5)


os.makedirs(SHOTS, exist_ok=True)
shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
# The beacon on a glass floor high over the mountains, so the ground under it is the floor.
bx, by, bz = cx + 4, 200, cz
chat(f"tp @s {bx + 2.5} {by} {bz + 0.5}")
ctl("wait", 40)
chat(f"fill {bx - 6} {by - 1} {bz - 6} {bx + 6} {by - 1} {bz + 6} minecraft:glass")
chat(f"setblock {bx} {by} {bz} big_nukes:strike_beacon")
ctl("wait", 5)

# A beacon broken before the pass gives its bomb back.
call(bx, by, bz)
inv = ctl("inventory").get("counts", {})
check("the beacon takes the warhead", inv.get("big_nukes:tactical_warhead", 0) == 0, json.dumps(inv))
check("a bomber is called", len(saved().get("beacons", [])) == 1, json.dumps(saved().get("beacons"))[:300])
chat(f"setblock {bx} {by} {bz} minecraft:air destroy")
ctl("wait", 10)
items = chat(f"execute if entity @e[type=minecraft:item,distance=..8,x={bx},y={by},z={bz}]")
check("a broken beacon drops its bomb", "passed" in items.lower(), items)
ctl("wait", 20)
check("and the call is off", not saved().get("beacons"), json.dumps(saved().get("beacons"))[:300])
chat("kill @e[type=minecraft:item]")

# The pass.
chat(f"setblock {bx} {by} {bz} big_nukes:strike_beacon")
ctl("wait", 5)
call(bx, by, bz)
said = json.dumps(ctl("messages", 10), ensure_ascii=False)
check("the beacon says when the bomber comes", "Bomber called" in said or "бомбардировщик" in said.lower(), said[-300:])
# Watch from 60 blocks south, out of the blast, looking at the beacon.
chat(f"tp @s {bx + 0.5} {by + 20} {bz + 60.5} 180 15")
seen = 0
for _ in range(45):
    ctl("wait", 20)
    seen = max(seen, displays())
    if seen:
        break
ctl("screenshot", os.path.join(SHOTS, "bomber-pass.png"))
check("the bomber flies over as a display", seen >= 1, str(seen))
for _ in range(10):
    ctl("wait", 10)
    if displays() >= 2:
        ctl("screenshot", os.path.join(SHOTS, "bomber-drop.png"))
        break
for _ in range(40):
    ctl("wait", 20)
    if saved().get("zones"):
        break
ctl("screenshot", os.path.join(SHOTS, "bomber-blast.png"))
zones = saved().get("zones", [])
near = [z for z in zones if abs(z["center"][0] - bx) <= 4 and abs(z["center"][2] - bz) <= 4]
check("the bomb goes off within 4 blocks of the beacon", bool(near), json.dumps(zones)[:300])
ctl("wait", 300)
check("the plane and the bomb are gone", displays() == 0, str(displays()))
check("the beacon's chunk is let go", not saved().get("forced"), json.dumps(saved().get("forced")))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
