#!/usr/bin/env python3
"""Takes the catalogue's screenshots offscreen, without the HUD: the bombs and machines, the Sunbreaker,
a bunker lit inside, a mushroom cloud over the land and a crater after the wave (target/shots/catalog-*.png).

    python tools/screenshots.py INSTANCE

It runs on its own control socket, so it doesn't take another game's commands.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

INSTANCE = sys.argv[1]
with open(os.path.join(os.environ["APPDATA"], "horizon-launcher", "instances.json"), encoding="utf-8") as f:
    VERSION = json.load(f)["instances"][INSTANCE]["version"]
GAME = os.path.join(os.environ["APPDATA"], "horizon", "instances", INSTANCE)
CTL = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", VERSION, "horizon-ctl.exe")
os.environ["HORIZON_HOME"] = GAME
os.environ["HORIZON_CONTROL"] = os.path.join(tempfile.gettempdir(), "horizon-control-shots.json")
os.environ["MSYS_NO_PATHCONV"] = "1"
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
os.makedirs(SHOTS, exist_ok=True)


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def chat(line):
    return " ".join(map(str, ctl("command", line).get("chat", [])))


def look(name, x, y, z, yaw, pitch, wait=40):
    chat(f"tp @s {x} {y} {z} {yaw} {pitch}")
    ctl("wait", wait)
    ctl("screenshot", os.path.join(SHOTS, f"catalog-{name}.png"))
    print("shot", name, flush=True)


def world(name, time="1000"):
    shutil.rmtree(os.path.join(GAME, "saves", name), ignore_errors=True)
    ctl("launch", "--world", name, "--seed", "424242", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)
    for line in ("gamerule advance_time false", "gamerule advance_weather false", f"time set {time}", "weather clear",
                 "gamerule send_command_feedback false"):
        chat(line)
    ctl("key", "f1", "tap")
    for _ in range(60):
        player = ctl("state").get("player") or {}
        pos = player.get("pos") or player.get("position")
        if pos:
            return [int(v // 1) for v in pos]
        ctl("wait", 10)
    raise RuntimeError("no player")


def fill(a, b, block, mode=""):
    for _ in range(20):
        said = chat(f"fill {a[0]} {a[1]} {a[2]} {b[0]} {b[1]} {b[2]} {block} {mode}".strip())
        if "not loaded" not in said:
            return
        ctl("wait", 20)


def showroom(cx, cy, cz):
    """Bombs, machines and the bunker's blocks on a stone floor, then the Sunbreaker and a bunker."""
    X, Y, Z = cx + 40, cy + 30, cz
    chat(f"tp @s {X} {Y + 6} {Z - 10}")
    ctl("wait", 80)
    fill([X - 14, Y - 1, Z - 6], [X + 14, Y - 1, Z + 22], "minecraft:polished_andesite")
    bombs = ["demolition_charge", "tactical_warhead", "atomic_bomb", "centrifuge", "assembly_table", "decontaminator",
             "launch_console", "strike_beacon"]
    for i, b in enumerate(bombs):
        chat(f"setblock {X - 7 + 2 * i} {Y} {Z} big_nukes:{b}[facing=north]")
    look("bombs", X, Y + 2.2, Z - 6.5, 0, 22)
    # The Sunbreaker: its core in a 3x3x3 shell inside a 5x5x5 reflector.
    s = [X, Y, Z + 10]
    fill([s[0] - 2, s[1], s[2] - 2], [s[0] + 2, s[1] + 4, s[2] + 2], "big_nukes:neutron_reflector")
    fill([s[0] - 1, s[1] + 1, s[2] - 1], [s[0] + 1, s[1] + 3, s[2] + 1], "big_nukes:implosion_shell")
    chat(f"setblock {s[0]} {s[1] + 2} {s[2]} big_nukes:sunbreaker_core")
    # A cutaway: the front of the reflector and the shell open on the core.
    fill([s[0] - 1, s[1] + 1, s[2] - 2], [s[0] + 1, s[1] + 3, s[2] - 1], "minecraft:air")
    chat(f"setblock {s[0]} {s[1] + 2} {s[2] - 1} big_nukes:sunbreaker_core[facing=north]")
    chat(f"setblock {s[0]} {s[1] + 2} {s[2]} big_nukes:implosion_shell")
    look("sunbreaker", s[0] + 5.5, s[1] + 4, s[2] - 7.5, 35, 18)


def bunker(cx, cy, cz):
    """A bunker of armored concrete with its door, generator, filter and lamps, from inside and out."""
    X, Y, Z = cx - 40, cy + 30, cz
    chat(f"tp @s {X} {Y + 8} {Z - 12}")
    ctl("wait", 80)
    fill([X - 8, Y - 1, Z - 8], [X + 8, Y - 1, Z + 8], "minecraft:polished_andesite")
    fill([X - 4, Y, Z - 3], [X + 4, Y + 4, Z + 3], "big_nukes:armored_concrete", "hollow")
    fill([X - 4, Y + 1, Z - 1], [X - 4, Y + 3, Z + 1], "big_nukes:lead_shield")
    chat(f"setblock {X} {Y + 1} {Z - 3} big_nukes:hermetic_door[half=lower,facing=south]")
    chat(f"setblock {X} {Y + 2} {Z - 3} big_nukes:hermetic_door[half=upper,facing=south]")
    chat(f"setblock {X - 3} {Y + 1} {Z + 2} big_nukes:fuel_generator[facing=south]")
    chat(f"item replace block {X - 3} {Y + 1} {Z + 2} container.0 with minecraft:coal 64")
    chat(f"setblock {X + 3} {Y + 1} {Z + 2} big_nukes:air_filter[facing=south]")
    for lx in (X - 2, X + 2):
        chat(f"setblock {lx} {Y + 3} {Z} big_nukes:bunker_lamp")
    chat(f"setblock {X + 1} {Y + 1} {Z + 2} minecraft:barrel[facing=up]")
    chat(f"item replace block {X + 1} {Y + 1} {Z + 2} container.0 with big_nukes:ration 16")
    ctl("wait", 80)
    look("bunker-outside", X - 7.5, Y + 4, Z - 10.5, -30, 15)
    chat("time set 18000")
    look("bunker-inside", X + 0.5, Y + 1, Z - 2, 0, 12, 60)
    chat("time set 1000")


def cloud(cx, cy, cz):
    """An atomic bomb far off over the land: the flash, the cloud rising."""
    chat("time set 12500")
    chat(f"fill {cx - 2} {cy + 40} {cz - 2} {cx + 2} {cy + 40} {cz + 2} minecraft:glass")
    chat(f"tp @s {cx} {cy + 41} {cz} -90 -10")
    ctl("wait", 60)
    chat(f"bignukes blast 250 {cx + 450} {cy} {cz}")
    for t, name in ((120, "cloud-rising"), (380, "cloud")):
        ctl("wait", t)
        chat(f"tp @s {cx} {cy + 41} {cz} -90 -10")
        ctl("wait", 5)
        ctl("screenshot", os.path.join(SHOTS, f"catalog-{name}.png"))
        print("shot", name, flush=True)


def crater(cx, cy, cz):
    """A tactical warhead's crater with its scorched ring, seen from above once the wave is done."""
    chat("time set 1000")
    x = cx - 300
    chat(f"tp @s {x} {cy + 80} {cz + 60}")
    ctl("wait", 120)
    chat(f"bignukes blast 50 {x} {cy} {cz}")
    ctl("wait", 900)
    look("crater", x, cy + 70, cz + 75, 180, 45, 60)


cx, cy, cz = world("BN Screenshots")
showroom(cx, cy, cz)
bunker(cx, cy, cz)
cloud(cx, cy, cz)
crater(cx, cy, cz)
ctl("quit")
