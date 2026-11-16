#!/usr/bin/env python3
"""Takes the screenshots of what 0.4.0 adds (target/shots/v04-*.png): a bunker with a radiation-glass window, the
trinitite a tactical warhead leaves at its crater's rim on a sandstone plain, the fallout's ash falling in its plume
down the wind, and the grey rain of a nuclear winter.

    python tools/screenshots_04.py INSTANCE

Like screenshots.py: its own control socket, its own world (BN Screenshots 04, seed 1, made anew) and player folder.
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
os.environ["HORIZON_CONTROL"] = os.path.join(tempfile.gettempdir(), "horizon-control-shots04.json")
os.environ["MSYS_NO_PATHCONV"] = "1"
TARGET = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target")
SHOTS = os.path.join(TARGET, "shots")
PLAYER = os.path.abspath(os.path.join(TARGET, "shots-player"))
WORLD = "BN Screenshots 04"
os.makedirs(SHOTS, exist_ok=True)
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
    ctl("screenshot", os.path.join(SHOTS, f"v04-{name}.png"))
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



def window():
    """A small bunker of armored concrete on the plains with a radiation-glass window, lamps inside."""
    X, F, Z = 658, 66, 430
    go(X, F + 10, Z + 12)
    fill([X - 6, F + 1, Z - 4], [X + 6, F + 6, Z + 8], "minecraft:air")
    fill([X - 6, F, Z - 4], [X + 6, F, Z + 8], "minecraft:grass_block")
    fill([X - 3, F + 1, Z - 2], [X + 3, F + 4, Z + 2], "big_nukes:armored_concrete", "hollow")
    fill([X - 1, F + 2, Z + 2], [X + 1, F + 3, Z + 2], "big_nukes:radiation_glass")
    chat(f"setblock {X} {F + 3} {Z} big_nukes:bunker_lamp[lit=true]")
    chat("kill @e[type=minecraft:item]")
    ctl("options-set", "fov", 60)
    chat(f"tp @s {X + 3.5} {F + 3} {Z + 7.5} 155 8")
    ctl("wait", 40)
    snap("glass-window")


def zone_wind(x, z):
    ctl("save")
    try:
        with open(os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json"), encoding="utf-8") as f:
            zones = json.load(f).get("zones", [])
    except (OSError, ValueError):
        return None
    for zn in zones:
        if abs(zn["center"][0] - x) <= 2 and abs(zn["center"][2] - z) <= 2:
            return zn
    return None


def trinitite():
    """A tactical warhead over a sandstone plain: the rim of its crater glassed green; then ash down its plume."""
    X, F, Z = 720, 80, 520
    go(X, F + 30, Z + 60, 120)
    fill([X - 40, F - 3, Z - 40], [X + 40, F + 25, Z + 40], "minecraft:air")
    fill([X - 40, F - 6, Z - 40], [X + 40, F - 4, Z + 40], "minecraft:sandstone")
    fill([X - 40, F - 3, Z - 40], [X + 40, F - 3, Z + 40], "minecraft:sand")
    chat(f"tp @s {X} {F + 60} {Z + 110} 180 25")
    ctl("wait", 20)
    chat(f"bignukes blast 50 {X} {F - 2} {Z}")
    ctl("wait", 300)
    ctl("options-set", "fov", 60)
    chat(f"tp @s {X + 0.5} {F + 22} {Z + 32.5} 180 40")
    ctl("wait", 60)
    snap("trinitite-rim")
    zn = zone_wind(X, Z)
    if zn and zn.get("wind") is not None:
        a = math.radians(zn["wind"])
        w = (math.cos(a), math.sin(a))
        d = 1.15 * zn["radius"]
        px, pz = X + w[0] * d, Z + w[1] * d
        yaw = math.degrees(math.atan2(-w[0], w[1])) + 180  # looking back up the wind, toward the blast
        go(px, F + 20, pz, 80)
        chat(f"tp @s {px:.1f} {F + 4} {pz:.1f} {yaw:.1f} -5")
        ctl("wait", 40)
        snap("fallout-ash")


def winter():
    """A Sunbreaker-sized blast far away brings the winter: the plains under grey rain."""
    chat("bignukes blast 600 -2500 64 -2500")
    ctl("wait", 260)
    ctl("options-set", "fov", 70)
    go(640, 90, 300, 100)
    chat("tp @s 640 90 300 60 -2")
    ctl("wait", 120)
    snap("winter")


world()
window()
trinitite()
winter()
ctl("quit")
