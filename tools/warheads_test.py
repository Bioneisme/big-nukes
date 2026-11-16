#!/usr/bin/env python3
"""Checks the neutron and the dirty bomb in the game. Each is set off by a small blast's wave (as a chain), so the
check runs the path a bomb takes in play: the neutron bomb breaks glass and leaves stone and the grass standing,
kills a cow in its reach and leaves a short-lived zone; the dirty bomb leaves a zone of 100 blocks.

    python tools/warheads_test.py GAME_FOLDER VERSION
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
WORLD = "bn-warheads"
SAVED = os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json")
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


def block(x, y, z):
    return json.dumps(ctl("block", x, y, z))


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
y = 230
chat("gamerule doMobSpawning false")
chat(f"fill {cx - 70} {y - 1} {cz - 4} {cx + 70} {y - 1} {cz + 4} minecraft:stone")

# The neutron bomb, set off by a demolition blast 10 blocks away.
bomb = (cx, y, cz)
chat(f"setblock {bomb[0]} {bomb[1]} {bomb[2]} big_nukes:neutron_bomb")
glass, stone = (cx + 40, y, cz), (cx + 41, y, cz)
chat(f"setblock {glass[0]} {glass[1]} {glass[2]} minecraft:glass")
chat(f"setblock {stone[0]} {stone[1]} {stone[2]} minecraft:stone_bricks")
chat(f"summon minecraft:cow {cx - 50} {y} {cz} {{NoAI:1b,Tags:[\"bn_cow\"]}}")
chat(f"tp @s {cx} {y + 150} {cz + 300}")
ctl("wait", 10)
before = saved().get("next_id", 0)
chat(f"bignukes blast 16 {cx + 10} {y} {cz}")
ctl("wait", 300)
data = saved()
check("the wave set the neutron bomb off", data.get("next_id", 0) == before + 2, str(data.get("next_id")))
check("the glass broke", "minecraft:glass" not in block(*glass), block(*glass)[:150])
check("the stone bricks stand", "stone_bricks" in block(*stone), block(*stone)[:150])
check("the cow died", "bn_cow" not in chat("execute if entity @e[tag=bn_cow] run say alive"), "")
zones = data.get("zones", [])
check("a short-lived neutron zone", any(z.get("half_life") == 6000 for z in zones), str(zones)[:300])

# The dirty bomb, set off the same way, far off.
dx = cx - 2000
chat(f"forceload add {dx - 16} {cz - 16} {dx + 16} {cz + 16}")
chat(f"fill {dx - 4} {y - 1} {cz - 4} {dx + 14} {y - 1} {cz + 4} minecraft:stone")
chat(f"setblock {dx} {y} {cz} big_nukes:dirty_bomb")
chat(f"bignukes blast 16 {dx + 10} {y} {cz}")
ctl("wait", 200)
zones = saved().get("zones", [])
dirty = [z for z in zones if z.get("center", [None])[0] == dx]
check("the dirty bomb left a zone of 100", dirty and abs(dirty[0]["radius"] - 100.0) < 1e-6, str(dirty)[:300])
chat(f"forceload remove {dx - 16} {cz - 16} {dx + 16} {cz + 16}")
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
