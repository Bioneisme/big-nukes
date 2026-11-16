#!/usr/bin/env python3
"""Checks the consequences in the game: uranium carried in a survival player's inventory builds a dose, radiation
glass is a bunker wall, a tactical warhead's blast over sandstone leaves trinitite at its core's rim and a zone with a
wind, and a Sunbreaker-sized blast brings the winter.

    python tools/consequences_test.py GAME_FOLDER VERSION
"""
import json
import math
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
WORLD = "bn-consequences"
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

# Carried uranium: a stack of weapons-grade in a survival player's pocket, far from any zone.
chat(f"fill {cx - 3} {y - 1} {cz - 3} {cx + 3} {y - 1} {cz + 3} minecraft:stone")
chat(f"tp @s {cx + 0.5} {y} {cz + 0.5}")
chat("clear @s")
chat("give @s big_nukes:weapons_grade_uranium 64")
chat("gamemode survival")
ctl("wait", 240)
doses = saved().get("doses", {})
check("a stack of weapons-grade uranium builds a dose", any(d.get("rad", 0) > 0 for d in doses.values()), json.dumps(doses)[:300])
chat("clear @s")
chat("gamemode creative")

# Radiation glass is a bunker wall.
gx, gz = cx + 5, cz
chat(f"setblock {gx} {y} {gz} big_nukes:radiation_glass")
ctl("wait", 10)
walls = json.dumps(saved().get("walls", {}))
check("radiation glass is kept as a wall", '"Glass"' in walls, walls[:300])

# A tactical warhead over a sandstone plain, far off: trinitite at the rim, and a zone with a wind.
tx, tz = cx + 400, cz
chat(f"forceload add {tx - 40} {tz - 40} {tx + 40} {tz + 40}")
ctl("wait", 40)
chat(f"fill {tx - 36} {y - 1} {tz - 36} {tx + 36} {y - 1} {tz + 36} minecraft:sandstone")
chat(f"tp @s {tx} {y + 60} {tz + 120}")
ctl("wait", 20)
chat(f"bignukes blast 50 {tx} {y} {tz}")
ctl("wait", 200)
ring = []
for k in range(24):
    a = 2 * math.pi * k / 24
    for r in (18, 19, 20, 21):
        ring.append((round(tx + r * math.cos(a)), round(tz + r * math.sin(a))))
glass = sum(1 for x, z in ring if "trinitite" in block(x, y - 1, z))
check("trinitite at the core's rim", glass > 0, f"{glass} of {len(ring)}")
zones = saved().get("zones", [])
near = [z for z in zones if abs(z["center"][0] - tx) <= 2]
check("its zone has a wind", bool(near) and near[0].get("wind") is not None, json.dumps(near)[:300])
chat(f"forceload remove {tx - 40} {tz - 40} {tx + 40} {tz + 40}")

# A Sunbreaker-sized blast brings the winter.
data = saved()
chat(f"bignukes blast 600 {cx - 2000} 64 {cz}")
ctl("wait", 20)
data = saved()
check("a Sunbreaker-sized blast brings the winter", data.get("winter_until", 0) > data.get("tick", 0) + 60_000,
      f"{data.get('winter_until')} / {data.get('tick')}")
ctl("wait", 240)
said = chat("weather query") or ""
print(" weather:", said[:200])
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
