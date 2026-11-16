#!/usr/bin/env python3
"""Takes the screenshots of 0.4.0's events (target/shots/ev-*.png): a radar by its generator, a storm's warning toast,
the storm's green dust, glowing mutants, a satellite's trail down the sky and its crater with debris.

    python tools/screenshots_events.py INSTANCE

Like screenshots.py: its own control socket, its own world (BN Screenshots Events, seed 1, made anew) and player folder.
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
os.environ["HORIZON_CONTROL"] = os.path.join(tempfile.gettempdir(), "horizon-control-shots-ev.json")
os.environ["MSYS_NO_PATHCONV"] = "1"
TARGET = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target")
SHOTS = os.path.join(TARGET, "shots")
PLAYER = os.path.abspath(os.path.join(TARGET, "shots-player"))
WORLD = "BN Screenshots Events"
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
    ctl("screenshot", os.path.join(SHOTS, f"ev-{name}.png"))
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




def events_file():
    ctl("save")
    try:
        with open(os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json"), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def base():
    """A radar and a generator burning coal on a concrete pad in the plains; a blast far off starts the events."""
    X, F, Z = 658, 66, 430
    go(X, F + 10, Z + 10)
    fill([X - 6, F + 1, Z - 4], [X + 6, F + 6, Z + 8], "minecraft:air")
    fill([X - 6, F, Z - 4], [X + 6, F, Z + 8], "minecraft:grass_block")
    fill([X - 2, F, Z - 1], [X + 2, F, Z + 1], "big_nukes:reinforced_concrete")
    chat(f"setblock {X - 1} {F + 1} {Z} big_nukes:fuel_generator[facing=south]")
    chat(f"item replace block {X - 1} {F + 1} {Z} container.0 with minecraft:coal 64")
    chat(f"setblock {X + 1} {F + 1} {Z} big_nukes:radar[facing=south]")
    chat(f"setblock {X + 2} {F + 1} {Z - 1} big_nukes:siren")
    chat("kill @e[type=minecraft:item]")
    # A blast 140 blocks off, out of sight: the events start, and its zone reaches the base.
    chat(f"bignukes blast 120 {X + 140} {F} {Z}")
    ctl("wait", 60)
    ctl("options-set", "fov", 60)
    chat(f"tp @s {X + 0.5} {F + 2.2} {Z + 5.5} 180 12")
    ctl("wait", 40)
    snap("radar")
    return X, F, Z


def storm(X, F, Z):
    """The storm's warning toast by the radar, then the storm itself: green dust and ash over the plains."""
    chat("gamemode survival")
    chat("effect give @s minecraft:resistance 900 4 true")
    hud(True)
    chat("bignukes events trigger storm")
    ctl("wait", 40)
    snap("storm-toast")
    hud(False)
    ev = events_file().get("events", {})
    st = ev.get("storm")
    left = (st["from"] - events_file().get("tick", 0)) if st else 2400
    ctl("wait", max(left, 0) + 120)
    chat(f"tp @s {X + 0.5} {F + 2.2} {Z + 7.5} 160 2")
    ctl("wait", 60)
    snap("storm")
    chat("gamemode creative")


def mutants(X, F, Z):
    """Three zombies by the base turned mutants: they glow green."""
    zx, zz = X - 4, Z + 4
    for k in range(3):
        chat(f"summon minecraft:zombie {zx + 2 * k} {F + 1} {zz} {{NoAI:1b,Rotation:[180f,0f],Tags:[\"bn_shot\"]}}")
    for _ in range(40):
        ctl("wait", 20)
        healths = [chat(f"attribute @e[tag=bn_shot,limit=1,sort=nearest,x={zx + 2 * k},y={F + 1},z={zz},distance=..1] minecraft:max_health get") for k in range(3)]
        if sum("40.25" in h for h in healths) >= 2:
            break
    chat(f"tp @s {zx + 2.5} {F + 2} {zz - 5.5} 0 8")
    ctl("wait", 40)
    snap("mutants")
    chat("kill @e[tag=bn_shot]")


def satellite():
    """A satellite: its trail down the sky, then its crater with debris."""
    chat("bignukes events trigger satellite")
    ctl("wait", 220)
    sat = events_file().get("events", {}).get("satellite")
    if not sat:
        print("no satellite", flush=True)
        return
    tx, tz = sat["to"]
    view = (tx + 140, 140, tz + 40)
    go(*view, ticks=100)
    yaw = math.degrees(math.atan2(-(tx - view[0]), tz - view[2]))
    chat(f"tp @s {view[0]} {view[1]} {view[2]} {yaw:.1f} -10")
    for _ in range(80):
        data = events_file()
        s = data.get("events", {}).get("satellite")
        if not s or data.get("tick", 0) >= s["at"] - 120:
            break
        ctl("wait", 20)
    ctl("wait", 60)
    snap("satellite-trail")
    for _ in range(40):
        ctl("wait", 20)
        if not events_file().get("events", {}).get("satellite"):
            break
    ctl("wait", 200)
    go(tx + 0.5, 120, tz + 14.5, 80)
    chat(f"tp @s {tx + 0.5} {events_file().get('tick', 0) * 0 + 0} {tz + 14.5}")
    s = [z for z in events_file().get("zones", []) if abs(z.get("radius", 0) - 24.0) < 1e-6]
    gy = s[0]["center"][1] if s else 70
    chat(f"tp @s {tx + 0.5} {gy + 12} {tz + 14.5} 180 40")
    ctl("wait", 60)
    snap("satellite-crater")


world()
X, F, Z = base()
storm(X, F, Z)
mutants(X, F, Z)
satellite()
ctl("quit")
