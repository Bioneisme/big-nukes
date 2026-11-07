#!/usr/bin/env python3
"""Checks the Sunbreaker from afar in the game: a core without a whole shell that another blast's wave reaches
stays where it is; a full Sunbreaker with a receiver, linked to a remote, goes off with radius 1000 when the remote
is pressed 2500 blocks away, its chunk unloaded.

    python tools/sunbreaker_remote_test.py GAME_FOLDER VERSION
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
WORLD = "bn-sun-remote"
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


def said():
    return json.dumps(ctl("messages", 6).get("chat", []), ensure_ascii=False)


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


def holding(item):
    chat("clear @s")
    if item:
        chat(f"give @s {item}")
        ctl("select", 1)
    else:
        ctl("select", 9)


def structure(x, y, z, reflector=True):
    if reflector:
        chat(f"fill {x - 2} {y - 2} {z - 2} {x + 2} {y + 2} {z + 2} big_nukes:neutron_reflector outline")
    chat(f"fill {x - 1} {y - 1} {z - 1} {x + 1} {y + 1} {z + 1} big_nukes:implosion_shell")
    chat(f"setblock {x} {y} {z} big_nukes:sunbreaker_core")


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
chat("gamerule advance_time false")
y = 200

# An unfinished core 300 blocks west, and a blast of 250 another 200 blocks west of it: its outer ring reaches.
ux = cx - 300
chat(f"tp @s {ux + 0.5} {y + 3} {cz + 0.5}")
ctl("wait", 60)
chat(f"fill {ux - 1} {y - 1} {cz - 1} {ux + 1} {y + 1} {cz + 1} big_nukes:implosion_shell")
chat(f"fill {ux - 1} {y + 1} {cz - 1} {ux + 1} {y + 1} {cz + 1} minecraft:air")
chat(f"setblock {ux} {y} {cz} big_nukes:sunbreaker_core")
ctl("wait", 40)
chat(f"bignukes blast 250 {ux - 200} {y} {cz}")
for _ in range(30):
    ctl("wait", 20)
    if not [w for w in saved().get("waves", {}).get("waves", []) if not w.get("deferred")]:
        break
ctl("wait", 100)
core = ctl("block", ux, y, cz).get("block")
check("an unfinished core the wave reaches stays", core == "big_nukes:sunbreaker_core", str(core))

# The full one, its receiver and the remote.
sx = cx + 8
chat(f"fill {sx - 3} {y - 3} {cz - 3} {sx + 6} {y - 3} {cz + 3} minecraft:glass")
structure(sx, y, cz)
chat(f"tp @s {sx + 3.5} {y - 2} {cz + 0.5} 90 0")
ctl("wait", 40)
face = (sx + 2, y - 1, cz)
holding("big_nukes:receiver_detonator")
ctl("look-at", "block", *face)
ctl("interact", *face)
ctl("wait", 5)
holding("big_nukes:remote")
ctl("interact", *face)
ctl("wait", 40)
line = said()
check("the remote links through a reflector", "linked" in line.lower() or "привязан" in line.lower(), line[-300:])
bombs = saved().get("bombs", [])
check("the core has read its structure", any(b.get("power") == {"Ok": 1000} for b in bombs), json.dumps(bombs)[:300])

# 2500 blocks off, the chunk unloaded: press.
chat("gamemode spectator")
chat(f"tp @s {sx - 2500} 300 {cz} 90 -90")
ctl("wait", 300)
# A spectator uses no items: creative for the press.
chat("gamemode creative")
ctl("look", 0, -90)
ctl("use-item")
ctl("wait", 20)
line = said()
zones = saved().get("zones", [])
main = [z for z in zones if z["center"][0] == sx and z["center"][2] == cz]
check("the remote sets it off from 2500 blocks, radius 1000", bool(main) and abs(main[0]["radius"] - 1200) < 1, f"{line[-300:]} / {json.dumps(zones)[:300]}")
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
