#!/usr/bin/env python3
"""Checks radiation in the game: a blast of 250 leaves a zone, a player at its center builds a
dose, falls sick and dies of it with our message, comes back clean; a full hazmat suit keeps
most of it out and wears; the anti-rad takes dose off; a cow and a zombie in the zone are hurt;
the dose is kept across a reload.

    python tools/rad_test.py GAME_FOLDER VERSION
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
WORLD = "bn-rad"
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


def dose():
    doses = saved().get("doses", {})
    if not doses:
        return 0.0
    d = next(iter(doses.values()))
    return d.get("rad", 0.0) if isinstance(d, dict) else float(d)


def entity_health(selector):
    said = chat(f"data get entity {selector} Health")
    digits = "".join(c for c in said.split(":")[-1] if c.isdigit() or c == ".")
    try:
        return float(digits)
    except ValueError:
        return None


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


def stand(x, y, z):
    chat(f"tp @s {x + 0.5} {y} {z + 0.5}")


def pad(x, y, z):
    """Glass under (x, y, z) once its chunk loads, so a survival player there doesn't fall."""
    stand(x, y + 2, z)
    for _ in range(20):
        ctl("wait", 20)
        if "not loaded" not in chat(f"fill {x - 2} {y - 1} {z - 2} {x + 2} {y - 1} {z + 2} minecraft:glass"):
            break
    stand(x, y, z)
    ctl("wait", 5)


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
# The blast far off, so the wave's damage doesn't reach the player; the zone is what matters.
bx, by, bz = cx + 3000, 250, cz
chat(f"tp @s {bx} {by + 2} {bz + 600}")
ctl("wait", 40)
chat(f"bignukes blast 250 {bx} {by} {bz}")
ctl("wait", 300)
zones = saved().get("zones", [])
print(" zones:", json.dumps(zones)[:300])
check("a blast of 250 leaves one zone", len(zones) == 1 and abs(zones[0].get("radius", 0) - 300) < 1, str(zones)[:200])
# Wait for the center's chunk before laying the glass under the player: a fill there fails until it loads.
stand(bx, by + 2, bz)
for _ in range(20):
    ctl("wait", 20)
    if "not loaded" not in chat(f"fill {bx - 3} {by - 1} {bz - 3} {bx + 3} {by - 1} {bz + 3} minecraft:glass"):
        break
stand(bx, by, bz)
ctl("wait", 5)
chat("gamemode survival")
chat("effect clear @s")
ctl("wait", 200)
d = dose()
state = json.dumps(ctl("state"), ensure_ascii=False)
print(f" dose after 10 s at the center: {d:.0f}; state effects: {state[state.find('effect'):state.find('effect') + 200] if 'effect' in state else '-'}")
check("10 s at the center build a dose over 100", d > 100, f"{d}")
check("the player is weakened", "weakness" in state, state[:300])
ctl("wait", 1000)
lines = json.dumps(ctl("messages", 10), ensure_ascii=False)
check("the dose kills, with our message", "radiation sickness" in lines or "лучевой" in lines, lines[:300])
ctl("click-button", "Respawn")
ctl("wait", 40)
check("after respawning the dose is gone", dose() == 0.0, f"{dose()}")

# The suit keeps most of it out and wears.
for slot, piece in (("head", "helmet"), ("chest", "chestplate"), ("legs", "leggings"), ("feet", "boots")):
    chat(f"item replace entity @s armor.{slot} with big_nukes:hazmat_{piece}")
stand(bx, by, bz)
ctl("wait", 200)
d = dose()
inv = json.dumps(ctl("inventory"), ensure_ascii=False)
check("in a full suit 10 s at the center build little", d < 30, f"{d}")
check("the suit wears", '"damage": 0' not in inv[inv.find("armor"):inv.find("armor") + 600] if "armor" in inv else False, inv[:300])

# The anti-rad takes dose off.
chat("clear @s")
ctl("wait", 300)
# Out of the zone, so the dose only heals slowly while the file catches up (the mod writes it every 10 s).
pad(bx, by + 40, bz + 900)
ctl("wait", 220)
before = dose()
chat("give @s big_nukes:anti_rad")
ctl("select", 1)
ctl("use-item", "--hold", 2)
ctl("wait", 220)
after = dose()
inv = json.dumps(ctl("inventory"), ensure_ascii=False)
held = sorted(set(__import__("re").findall(r'"item": "([a-z_:]+)"', inv)))
check("drinking leaves a glass bottle", "minecraft:glass_bottle" in held, str(held))
stand(bx, by, bz)
check("the anti-rad takes about 300 off", before > 200 and after < before - 200, f"{before:.0f} -> {after:.0f}")

# Mobs in the zone are hurt.
chat("gamemode creative")
chat(f"summon minecraft:cow {bx + 2} {by} {bz} {{NoAI:1b,Tags:[\"bn\"]}}")
chat(f"summon minecraft:zombie {bx - 2} {by} {bz} {{NoAI:1b,PersistenceRequired:1b,Tags:[\"bn\"]}}")
ctl("wait", 5)
cow0, zom0 = entity_health("@e[type=cow,tag=bn,limit=1]"), entity_health("@e[type=zombie,tag=bn,limit=1]")
ctl("wait", 60)
cow1, zom1 = entity_health("@e[type=cow,tag=bn,limit=1]"), entity_health("@e[type=zombie,tag=bn,limit=1]")
check("a cow in the zone is hurt", cow0 is not None and (cow1 is None or cow1 < cow0), f"{cow0} -> {cow1}")
check("a zombie in the zone is hurt", zom0 is not None and (zom1 is None or zom1 < zom0), f"{zom0} -> {zom1}")

# The Geiger counter's line and the zone's fog, for the eye: target/shots.
shots = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
os.makedirs(shots, exist_ok=True)
chat("clear @s")
chat("give @s big_nukes:geiger_counter")
ctl("select", 1)
chat(f"tp @s {bx + 0.5} {by} {bz + 0.5} 0 0")
ctl("wait", 60)
ctl("screenshot", os.path.join(shots, "geiger-center.png"))
chat(f"tp @s {bx + 0.5} {by + 40} {bz + 900.5} 180 10")
ctl("wait", 80)
ctl("screenshot", os.path.join(shots, "geiger-outside.png"))
chat(f"tp @s {bx + 0.5} {by} {bz + 0.5} 0 0")
ctl("wait", 20)

# The dose across a reload.
# A few seconds of dose at the center, then creative so it stands still (and the player doesn't die of
# what is left from before), then wait for the mod's file, which it writes every 10 s.
stand(bx, by, bz)
chat("gamemode survival")
ctl("wait", 100)
chat("gamemode creative")
ctl("wait", 220)
kept = dose()
ctl("quit")
time.sleep(8)
open_world()
check("the dose survives a reload", kept > 0 and abs(dose() - kept) < 60, f"{kept:.0f} -> {dose():.0f}")
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
