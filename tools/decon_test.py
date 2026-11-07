#!/usr/bin/env python3
"""Checks iodine and the decontaminator in the game: after a blast of 250, an iodine tablet halves how fast the dose
builds at a spot of the zone; a decontaminator with coal 120 blocks from the center grows its circle to 48, a player
standing in it gathers no dose, the ash in it turns to grass, and the circle is kept across a reload
(target/shots/decon-*.png).

    python tools/decon_test.py GAME_FOLDER VERSION
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
WORLD = "bn-decon"
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


def dose():
    doses = saved().get("doses", {})
    if not doses:
        return 0.0
    d = next(iter(doses.values()))
    return d.get("rad", 0.0) if isinstance(d, dict) else float(d)


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


def pad(x, y, z):
    """Glass under (x, y, z) once its chunk loads, the player then on it."""
    chat(f"tp @s {x + 0.5} {y + 2} {z + 0.5}")
    for _ in range(20):
        ctl("wait", 20)
        if "not loaded" not in chat(f"fill {x - 2} {y - 1} {z - 2} {x + 2} {y - 1} {z + 2} minecraft:glass"):
            break
    chat(f"tp @s {x + 0.5} {y} {z + 0.5}")
    ctl("wait", 5)


def rate(ticks):
    """The dose gathered over `ticks` (the mod writes it every 200 ticks), with the 0.5 rad/s healing put back."""
    before = dose()
    ctl("wait", ticks)
    return dose() - before + 0.5 * ticks / 20


def ash_near(radius):
    return ctl("find-block", "big_nukes:ash", radius).get("count", 0)


os.makedirs(SHOTS, exist_ok=True)
shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
chat("gamerule advance_time false")
bx, by, bz = cx + 3000, 250, cz
chat(f"tp @s {bx} {by + 2} {bz + 600}")
ctl("wait", 40)
chat(f"bignukes blast 250 {bx} {by} {bz}")
ctl("wait", 300)
check("the blast leaves a zone", len(saved().get("zones", [])) == 1, json.dumps(saved().get("zones"))[:200])

# Iodine: the dose's rate at a spot of the zone, without and with a tablet.
pad(bx + 200, by, bz)
chat("gamemode survival")
chat("effect clear @s")
ctl("wait", 220)
bare = rate(400)
chat("clear @s")
chat("give @s big_nukes:iodine_tablets")
ctl("select", 1)
ctl("use-item", "--hold", 2)
ctl("wait", 10)
effects = json.dumps(ctl("effects"), ensure_ascii=False)
check("a tablet gives the iodine effect", "iodine" in effects, effects[:300])
ctl("wait", 200)
shielded = rate(400)
print(f" dose over 20 s: {bare:.1f} bare, {shielded:.1f} with iodine")
check("iodine halves the dose's rate", bare > 5 and 0.35 <= shielded / bare <= 0.65, f"{bare:.1f} / {shielded:.1f}")
chat("gamemode creative")
chat("effect clear @s")

# The decontaminator, in the ash 120 blocks from the center.
# Low enough that find-block's cube (64 up and down) takes in the ground, which is near y 62 there.
dx, dy, dz = bx + 120, 75, bz
chat("gamemode spectator")
chat(f"tp @s {dx + 0.5} {dy} {dz + 0.5} 0 90")
ctl("wait", 100)
ash_before = ash_near(30)
print(f" ash within 30 before: {ash_before}")
ctl("screenshot", os.path.join(SHOTS, "decon-before.png"))
chat(f"fill {dx - 1} {dy - 5} {dz - 1} {dx + 1} {dy - 5} {dz + 1} minecraft:glass")
chat(f"setblock {dx} {dy - 4} {dz} big_nukes:decontaminator")
chat(f"item replace block {dx} {dy - 4} {dz} container.0 with minecraft:coal 8")
grown = 0
for _ in range(80):
    ctl("wait", 60)
    cleans = saved().get("cleans", [])
    grown = max([c.get("radius", 0) for c in cleans] or [0])
    if grown >= 48:
        break
check("its circle grows to 48", grown >= 48, json.dumps(saved().get("cleans"))[:300])
ctl("wait", 2400)
ash_after = ash_near(30)
print(f" ash within 30 after: {ash_after}")
ctl("screenshot", os.path.join(SHOTS, "decon-after.png"))
check("the ash in the circle turns to grass", ash_before > 20 and ash_after < ash_before / 10, f"{ash_before} -> {ash_after}")

# A player in the circle gathers no dose.
chat("gamemode creative")
pad(dx, dy + 30, dz + 5)
chat("gamemode survival")
ctl("wait", 220)
inside = rate(400)
print(f" dose over 20 s in the circle: {inside:.1f}")
check("in the circle the dose doesn't build", inside < 1.0, f"{inside:.1f}")
chat("gamemode creative")

# The circle across a reload.
kept = saved().get("cleans", [])
ctl("quit")
time.sleep(8)
open_world()
check("the circle is kept across a reload", saved().get("cleans") == kept and kept, json.dumps(saved().get("cleans"))[:200])
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
