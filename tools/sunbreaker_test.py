#!/usr/bin/env python3
"""Checks the Sunbreaker in the game: the centrifuge makes weapons-grade uranium of four enriched; a click with an
empty hand on the structure tells its state and radius, which falls as reflectors go; a keypad put in through a shell
block opens from the shell; the operator's limit holds a fuse back; a full structure lit by its fuse goes off with
radius 1000, the frames are measured, the center is open down to the void and bedrock stays further out; a second,
shell-only Sunbreaker in its reach goes off too, with radius 600 (target/shots/sunbreaker-*.png).

    python tools/sunbreaker_test.py GAME_FOLDER VERSION
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
WORLD = "bn-sun"
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


def said():
    return json.dumps(ctl("messages", 8).get("chat", []), ensure_ascii=False)


def block(x, y, z):
    return ctl("block", x, y, z).get("block", "?")


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


os.makedirs(SHOTS, exist_ok=True)
shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
chat("gamerule advance_time false")
sx, sy, sz = cx + 8, 200, cz
# The floor first, then the player on it.
chat(f"fill {sx - 4} {sy - 3} {sz - 4} {sx + 14} {sy - 3} {sz + 4} minecraft:glass")
chat(f"tp @s {sx + 3.5} {sy - 2} {sz + 0.5} 90 0")
ctl("wait", 40)

# The centrifuge's second step.
chat(f"setblock {sx + 12} {sy - 2} {sz + 3} big_nukes:centrifuge")
chat(f"item replace block {sx + 12} {sy - 2} {sz + 3} container.0 with big_nukes:enriched_uranium 4")
chat(f"item replace block {sx + 12} {sy - 2} {sz + 3} container.1 with minecraft:coal 8")

# The state, and the radius falling as reflectors go.
structure(sx, sy, sz)
face = (sx + 2, sy - 1, sz)
holding(None)
ctl("look-at", "block", *face)
ctl("interact", *face)
ctl("wait", 5)
line = said()
check("a click tells a full structure's state", "26/26" in line and "98/98" in line and "1000" in line, line[-300:])
chat(f"fill {sx - 2} {sy + 2} {sz - 2} {sx + 2} {sy + 2} {sz + 2} minecraft:air")
ctl("interact", *face)
ctl("wait", 5)
line = said()
check("25 reflectors gone take the radius to 897", "73/98" in line and "897" in line, line[-300:])
chat(f"fill {sx - 1} {sy + 2} {sz - 1} {sx + 1} {sy + 2} {sz + 1} minecraft:air")
chat(f"fill {sx - 2} {sy - 2} {sz - 2} {sx + 2} {sy + 2} {sz + 2} big_nukes:neutron_reflector outline")

# A second Sunbreaker, shell only, 40 blocks east: a keypad through its shell, then the first one's wave sets it off.
bx = sx + 40
chat(f"fill {bx - 3} {sy - 3} {sz - 3} {bx + 3} {sy - 3} {sz + 3} minecraft:glass")
structure(bx, sy, sz, reflector=False)
chat(f"tp @s {bx + 2.5} {sy - 2} {sz + 0.5} 90 0")
ctl("wait", 20)
shell = (bx + 1, sy - 1, sz)
holding("big_nukes:keypad_detonator")
ctl("look-at", "block", *shell)
ctl("interact", *shell)
ctl("wait", 5)
check("a keypad goes in through a shell block", "keypad" in said().lower() or "замок" in said().lower(), said()[-300:])
holding(None)
ctl("interact", *shell)
ctl("wait", 10)
screen = ctl("screen")
check("a click on the shell opens the core's keypad", screen.get("screen") == "container", str(screen)[:200])
# Escape only over a screen: over the world it opens the pause menu, which stops the game.
if screen.get("screen") == "container":
    ctl("key", "esc", "tap")
ctl("wait", 5)

# The operator's limit holds the fuse back.
chat(f"tp @s {sx + 3.5} {sy - 2} {sz + 0.5} 90 0")
ctl("wait", 20)
holding("big_nukes:fuse_detonator")
ctl("look-at", "block", *face)
ctl("interact", *face)
ctl("wait", 5)
chat("bignukes maxpower 250")
holding("minecraft:flint_and_steel")
ctl("interact", *face)
ctl("wait", 10)
line = said()
check("maxpower 250 holds the Sunbreaker back", "limit" in line or "лимит" in line, line[-300:])
check("and nothing burns", not saved().get("waves", {}).get("waves"), json.dumps(saved().get("waves"))[:200])
chat("bignukes maxpower 1024")

# The centrifuge has had its minute.
for _ in range(40):
    menu = json.dumps(ctl("container", sx + 12, sy - 2, sz + 3), ensure_ascii=False)
    if "weapons_grade_uranium" in menu:
        break
    ctl("wait", 40)
check("four enriched make one weapons-grade", "weapons_grade_uranium" in menu, menu[:300])

# The blast.
ctl("interact", *face)
ctl("wait", 5)
line = said()
# A fuse burns only while its chunk is loaded: wait 150 blocks off, then watch from 1300.
chat("gamemode spectator")
chat(f"tp @s {sx - 150} 260 {sz} -90 10")
ctl("stats", "reset")
started = time.time()
for _ in range(40):
    ctl("wait", 20)
    if saved().get("zones"):
        break
lit = time.time() - started
ctl("screenshot", os.path.join(SHOTS, "sunbreaker-flash.png"))
chat(f"tp @s {sx - 1300} 320 {sz} -90 5")
ctl("wait", 200)
ctl("screenshot", os.path.join(SHOTS, "sunbreaker-cloud.png"))
zones = saved().get("zones", [])
main = [z for z in zones if z["center"][0] == sx and z["center"][2] == sz]
print(f" zones: {json.dumps(zones)[:400]}")
check("the fuse lights and the blast goes off", bool(main), f"{line[-200:]} / {json.dumps(zones)[:200]}")
waves = saved().get("waves", {}).get("waves", [])
radii = [w["blast"]["radius"] for w in waves]
print(f" lit after {lit:.0f} s; waves now {radii}")
check("its radius is 1000", 1000 in radii or (main and abs(main[0]["radius"] - 1200) < 1), f"{radii} {main}")
for _ in range(90):
    ctl("wait", 40)
    if not [w for w in saved().get("waves", {}).get("waves", []) if not w.get("deferred")] or time.time() - started > 300:
        break
stats = ctl("stats")
print(" frames during the wave:", json.dumps(stats)[:600])
second = [z for z in saved().get("zones", []) if z["center"][0] == bx]
check("the shell-only Sunbreaker in reach goes off too, at 600", bool(second) and abs(second[0]["radius"] - 720) < 1, json.dumps(saved().get("zones"))[:300])

# Down the hole.
floor = -64
chat(f"tp @s {sx + 0.5} {floor + 20} {sz + 0.5} 0 90")
ctl("wait", 100)
ctl("screenshot", os.path.join(SHOTS, "sunbreaker-hole.png"))
center = [block(sx, floor + dy, sz) for dy in range(0, 5)]
check("the center is open to the void", all(b == "minecraft:air" for b in center), str(center))
chat(f"tp @s {sx + 60.5} {floor + 20} {sz + 0.5} 0 90")
ctl("wait", 100)
check("bedrock stays 60 blocks out", block(sx + 60, floor, sz) == "minecraft:bedrock", block(sx + 60, floor, sz))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
