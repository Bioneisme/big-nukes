#!/usr/bin/env python3
"""Takes the screenshots of what 0.3.0 adds (target/shots/v03-*.png): the new blocks in a row, a neutron blast's pale
cloud over a cottage it leaves standing with its windows gone, the console with its Spread field, a launch with its
toast and the siren lit beside the silo, and a splitting missile's three clouds round its target.

    python tools/screenshots_03.py INSTANCE

Like screenshots.py: its own control socket, its own world (BN Screenshots 03, seed 1, made anew) and player folder.
"""
import json
import math
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
os.environ["HORIZON_CONTROL"] = os.path.join(tempfile.gettempdir(), "horizon-control-shots03.json")
os.environ["MSYS_NO_PATHCONV"] = "1"
TARGET = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target")
SHOTS = os.path.join(TARGET, "shots")
PLAYER = os.path.abspath(os.path.join(TARGET, "shots-player"))
WORLD = "BN Screenshots 03"
os.makedirs(SHOTS, exist_ok=True)
# silo.rs: the console's screen and its fields (x, z, code, spread) and the Launch button.
SCREEN_W, SCREEN_H = 176, 240
FIELDS = [(8, 28, 52, 14), (64, 28, 52, 14), (120, 28, 48, 14), (120, 128, 48, 14)]
ERASE_AT, LAUNCH_AT = (86, 66, 82, 13), (86, 81, 82, 28)
HUD = [True]


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def chat(line):
    return " ".join(map(str, ctl("command", line).get("chat", [])))


def snap(name):
    ctl("screenshot", os.path.join(SHOTS, f"v03-{name}.png"))
    print("shot", name, flush=True)


def fill(a, b, block, mode=""):
    for _ in range(20):
        said = chat(f"fill {a[0]} {a[1]} {a[2]} {b[0]} {b[1]} {b[2]} {block} {mode}".strip())
        if "not loaded" not in said:
            return
        ctl("wait", 20)


def go(x, y, z, ticks=80):
    chat(f"tp @s {x} {y} {z}")
    ctl("wait", ticks)
    ctl("wait-ready", timeout=200)


def hud(on):
    """F1 toggles the HUD."""
    if on != HUD[0]:
        ctl("key", "f1", "tap")
        HUD[0] = on


def world():
    shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
    os.makedirs(PLAYER, exist_ok=True)
    for name in ("options.txt", "horizon-player.txt"):
        if os.path.exists(os.path.join(GAME, name)):
            shutil.copy(os.path.join(GAME, name), os.path.join(PLAYER, name))
    ctl("--timeout", "280", "launch", "--world", WORLD, "--seed", "1", "--creative", "--wait", "--offscreen",
        "--render-distance", "32", "--player-dir", PLAYER, timeout=290)
    ctl("wait", 60)
    ctl("options-set", "renderClouds", "false")
    for line in ("gamerule advance_time false", "gamerule advance_weather false", "time set 6000", "weather clear",
                 "gamemode spectator", "gamerule send_command_feedback false", "gamerule doMobSpawning false"):
        chat(line)
    hud(False)


def blocks():
    """The new blocks in a row on a strip of stone in the plains."""
    X, F, Z = 658, 66, 430
    go(X, F + 10, Z + 10)
    fill([X - 7, F, Z - 4], [X + 7, F + 4, Z + 8], "minecraft:air")
    fill([X - 7, F, Z - 4], [X + 7, F, Z + 8], "minecraft:grass_block")
    fill([X - 5, F, Z - 1], [X + 5, F, Z + 1], "minecraft:polished_andesite")
    row = ["atomic_bomb[facing=south]", "neutron_bomb[facing=south]", "dirty_bomb[facing=south]", "siren"]
    for i, b in enumerate(row):
        chat(f"setblock {X - 3 + 2 * i} {F + 1} {Z} big_nukes:{b}")
    chat("kill @e[type=minecraft:item]")
    ctl("options-set", "fov", 55)
    chat(f"tp @s {X + 0.5} {F + 1.9} {Z + 5.6} 180 14")
    ctl("wait", 40)
    snap("blocks")


def neutron():
    """A cottage with glass windows and cows in a pen, a neutron bomb 30 blocks off: its pale cloud, then the
    cottage standing with its windows gone and the pen empty."""
    X, F, Z = 600, 66, 470  # the cottage's north-west corner on the floor
    go(X, F + 15, Z - 20)
    fill([X - 4, F + 1, Z - 4], [X + 12, F + 8, Z + 12], "minecraft:air")
    fill([X - 4, F, Z - 4], [X + 12, F, Z + 12], "minecraft:grass_block")
    fill([X, F + 1, Z], [X + 6, F + 4, Z + 6], "minecraft:oak_planks", "hollow")
    fill([X, F + 5, Z], [X + 6, F + 5, Z + 6], "minecraft:spruce_planks")
    for x in (X + 2, X + 4):
        fill([x, F + 2, Z], [x, F + 3, Z], "minecraft:glass")
    fill([X, F + 2, Z + 2], [X, F + 3, Z + 4], "minecraft:glass")
    chat(f"setblock {X + 3} {F + 1} {Z} minecraft:oak_door[half=lower,facing=north]")
    chat(f"setblock {X + 3} {F + 2} {Z} minecraft:oak_door[half=upper,facing=north]")
    fill([X + 8, F + 1, Z], [X + 12, F + 1, Z + 4], "minecraft:oak_fence", "hollow")
    for dz in (1, 3):
        chat(f"summon minecraft:cow {X + 10} {F + 1} {Z + dz} {{NoAI:1b}}")
    chat("kill @e[type=minecraft:item]")
    ctl("options-set", "fov", 60)
    chat(f"tp @s {X - 6.5} {F + 3.5} {Z - 10.5} -25 6")
    ctl("wait", 40)
    snap("cottage-before")
    # The bomb 30 blocks west, set off by a small blast 10 blocks further west.
    bx = X - 30
    fill([bx - 14, F, Z - 2], [bx + 2, F, Z + 2], "minecraft:stone")
    chat(f"setblock {bx} {F + 1} {Z} big_nukes:neutron_bomb[facing=east]")
    # Outside its zone (144 blocks), whose green fog would hide it.
    chat(f"tp @s {X + 120} {F + 40} {Z - 160} 40 2")
    ctl("wait", 20)
    chat(f"bignukes blast 16 {bx - 10} {F + 1} {Z}")
    ctl("wait", 220)
    snap("neutron-cloud")
    ctl("wait", 200)
    chat(f"tp @s {X - 6.5} {F + 3.5} {Z - 10.5} -25 6")
    ctl("wait", 60)
    snap("cottage-after")


def press(rect):
    w, h = ctl("ui").get("gui_size") or (0, 0)
    ox, oy = math.floor((w - SCREEN_W) / 2), math.floor((h - SCREEN_H) / 2)
    x, y, rw, rh = rect
    ctl("click-at", ox + x + rw / 2, oy + y + rh / 2)
    ctl("wait", 1)


def split():
    """A silo with a siren beside it launches a splitting missile 700 blocks north: the console with its Spread, the
    launch with its toast and the siren lit, then the three clouds round the target, seen from the side."""
    X, G, Z = 697, 62, 392
    go(X, G + 10, Z + 12)
    fill([X - 4, G + 1, Z - 4], [X + 4, G + 6, Z + 4], "minecraft:air")
    fill([X - 4, G, Z - 4], [X + 4, G, Z + 4], "minecraft:grass_block")
    fill([X - 3, G, Z - 3], [X + 3, G, Z + 3], "big_nukes:reinforced_concrete")
    fill([X - 1, G - 9, Z - 1], [X + 1, G, Z + 1], "big_nukes:reinforced_concrete")
    fill([X, G - 7, Z], [X, G - 1, Z], "minecraft:air")
    chat(f"setblock {X} {G - 7} {Z} big_nukes:launch_pad")
    chat(f"setblock {X} {G} {Z} big_nukes:silo_hatch")
    # Set into the apron: within 8 blocks of the pad 7 down.
    console = (X + 3, G, Z)
    chat(f"setblock {console[0]} {console[1]} {console[2]} big_nukes:launch_console[facing=west]")
    chat(f"setblock {X - 3} {G + 1} {Z + 2} big_nukes:siren")
    chat(f"item replace block {X} {G - 7} {Z} container.0 with big_nukes:splitting_missile")
    chat("kill @e[type=minecraft:item]")
    chat("gamemode creative")
    chat(f"tp @s {X + 5.5} {G + 1} {Z + 3.5} 120 20")
    ctl("wait", 40)
    hud(True)
    ctl("look-at", "block", *console)
    ctl("interact", *console)
    ctl("wait", 10)
    tx, tz = X, Z - 700
    for field, text in enumerate([str(tx), str(tz), "4242", "120"]):
        press(FIELDS[field])
        press(ERASE_AT)
        ctl("type", text)
        ctl("wait", 2)
    snap("console")
    press(LAUNCH_AT)
    ctl("wait", 4)
    ctl("key", "esc", "tap")
    ctl("options-set", "fov", 70)
    chat(f"tp @s {X + 7.5} {G + 2.2} {Z + 7.5} 135 -4")
    ctl("wait", 30)
    snap("launch-toast")
    hud(False)
    chat("gamemode spectator")
    ctl("wait", 100)
    chat(f"tp @s {X + 6.5} {G + 2.6} {Z + 7.5} 140 -30")
    ctl("wait", 2)
    snap("liftoff")
    # The target 700 north; watched from 380 blocks east of it, high up.
    view = f"{tx + 380} {G + 80} {tz} 90 -6"
    go(tx + 380, G + 80, tz, 100)
    chat(f"tp @s {view}")
    saved = os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json")
    for _ in range(40):
        ctl("wait", 20)
        ctl("save")
        try:
            with open(saved, encoding="utf-8") as f:
                # The neutron blast's zone, and one for each of the three warheads.
                if len(json.load(f).get("zones", [])) >= 4:
                    break
        except (OSError, ValueError):
            pass
    ctl("wait", 260)
    chat(f"tp @s {view}")
    ctl("wait", 2)
    snap("split-clouds")


world()
blocks()
neutron()
split()
ctl("quit")
