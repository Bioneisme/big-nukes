#!/usr/bin/env python3
"""Takes the catalogue's screenshots offscreen, without the HUD (target/shots/catalog-*.png): a mushroom cloud at
sunset and one at noon, a tactical warhead's cloud rising over its crater and the crater after, the bombs and
machines in a row, the Sunbreaker cut open, a bunker in a hillside from outside and lit inside, and a missile
lifting off its silo.

    python tools/screenshots.py INSTANCE

It runs on its own control socket, so it doesn't take another game's commands, in its own world (BN Screenshots,
seed 1, made anew) and with its own player folder (target/shots-player: a copy of the instance's options with the
clouds off), so the instance's options stay as they are. The places are picked on seed 1's land: plains by a river,
a hollow under a cliff for the bunker, a coast for the crater. It takes about 15 minutes, most of it waiting for
the clouds to rise and go.
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
TARGET = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target")
SHOTS = os.path.join(TARGET, "shots")
PLAYER = os.path.abspath(os.path.join(TARGET, "shots-player"))
WORLD = "BN Screenshots"
os.makedirs(SHOTS, exist_ok=True)


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def chat(line):
    return " ".join(map(str, ctl("command", line).get("chat", [])))


def snap(name):
    ctl("screenshot", os.path.join(SHOTS, f"catalog-{name}.png"))
    print("shot", name, flush=True)


def look(name, x, y, z, yaw, pitch, wait=40, fov=70):
    ctl("options-set", "fov", fov)
    chat(f"tp @s {x} {y} {z} {yaw} {pitch}")
    ctl("wait", wait)
    snap(name)


def fill(a, b, block, mode=""):
    for _ in range(20):
        said = chat(f"fill {a[0]} {a[1]} {a[2]} {b[0]} {b[1]} {b[2]} {block} {mode}".strip())
        if "not loaded" not in said:
            return
        ctl("wait", 20)


def go(x, y, z, ticks=80):
    """Over a place, waiting for its chunks."""
    chat(f"tp @s {x} {y} {z}")
    ctl("wait", ticks)
    ctl("wait-ready", timeout=200)


def world():
    shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
    os.makedirs(PLAYER, exist_ok=True)
    for name in ("options.txt", "horizon-player.txt"):
        if os.path.exists(os.path.join(GAME, name)):
            shutil.copy(os.path.join(GAME, name), os.path.join(PLAYER, name))
    ctl("--timeout", "280", "launch", "--world", WORLD, "--seed", "1", "--creative", "--wait", "--offscreen",
        "--render-distance", "32", "--player-dir", PLAYER, timeout=290)
    ctl("wait", 60)
    # Minecraft's flat clouds would cross the mushroom clouds.
    ctl("options-set", "renderClouds", "false")
    for line in ("gamerule advance_time false", "gamerule advance_weather false", "time set 6000", "weather clear",
                 "gamemode spectator", "gamerule send_command_feedback false"):
        chat(line)
    ctl("key", "f1", "tap")


def workshop():
    """The bombs and machines in a row on a strip of stone in the plains, fronts to the camera."""
    X, F, Z = 658, 66, 430
    go(X, F + 10, Z + 10)
    fill([X - 7, F, Z - 4], [X + 7, F + 4, Z + 8], "minecraft:air")
    fill([X - 7, F, Z - 4], [X + 7, F, Z + 8], "minecraft:grass_block")
    fill([X - 6, F, Z - 1], [X + 6, F, Z + 1], "minecraft:polished_andesite")
    row = ["atomic_bomb", "tactical_warhead", "demolition_charge", "centrifuge", "assembly_table", "decontaminator"]
    for i, b in enumerate(row):
        state = "" if b == "assembly_table" else "[facing=south]"
        chat(f"setblock {X - 5 + 2 * i} {F + 1} {Z} big_nukes:{b}{state}")
    chat("kill @e[type=minecraft:item]")
    look("bombs", X + 0.5, F + 1.9, Z + 6.2, 180, 13, fov=60)


def sunbreaker():
    """The Sunbreaker on a plinth with a corner cut away: the core on its implosion shell inside the reflector."""
    X, B, Z = 681, 67, 455
    go(X, B + 10, Z + 10)
    fill([X - 5, B, Z - 5], [X + 5, B + 7, Z + 5], "minecraft:air")
    fill([X - 5, B - 1, Z - 5], [X + 5, B - 1, Z + 5], "minecraft:dirt")
    fill([X - 5, B, Z - 5], [X + 5, B, Z + 5], "minecraft:grass_block")
    fill([X - 3, B, Z - 3], [X + 3, B, Z + 3], "minecraft:polished_andesite")
    b = B + 1
    fill([X - 2, b, Z - 2], [X + 2, b + 4, Z + 2], "big_nukes:neutron_reflector")
    fill([X - 1, b + 1, Z - 1], [X + 1, b + 3, Z + 1], "big_nukes:implosion_shell")
    # The cut: the reflector's south-east quarter above its bottom row, the shell's above its bottom layer.
    fill([X, b + 1, Z], [X + 2, b + 4, Z + 2], "minecraft:air")
    fill([X - 1, b + 1, Z - 1], [X + 1, b + 1, Z + 1], "big_nukes:implosion_shell")
    chat(f"setblock {X} {b + 2} {Z} big_nukes:sunbreaker_core[facing=south]")
    chat("kill @e[type=minecraft:item]")
    look("sunbreaker", X + 5.8, b + 4.6, Z + 5.8, 135, 25)


def bunker():
    """A bunker of armored concrete dug into a cliff over a hollow, a gravel path to its hermetic door; inside lead
    shielding, bunks, stores, an air filter, a decontaminator and two generators burning coal, which light the lamps."""
    X, Y, Z = 631, 77, 303  # Y the floor, Z the front wall; the bunker runs south into the cliff.
    go(X, Y + 12, Z - 14)
    fill([X - 5, Y, Z], [X + 5, Y + 4, Z + 8], "big_nukes:armored_concrete", "hollow")
    # The yard in front, flattened, and earth over the roof.
    fill([X - 6, Y + 1, Z - 7], [X + 6, Y + 6, Z - 1], "minecraft:air")
    fill([X - 6, Y, Z - 7], [X + 6, Y, Z - 1], "minecraft:grass_block")
    fill([X - 1, Y, Z - 7], [X + 1, Y, Z - 1], "minecraft:gravel")
    fill([X - 6, Y + 5, Z], [X + 6, Y + 5, Z], "minecraft:grass_block", "replace minecraft:air")
    fill([X - 6, Y + 5, Z + 1], [X + 6, Y + 5, Z + 9], "minecraft:dirt", "replace minecraft:air")
    fill([X - 6, Y + 6, Z + 1], [X + 6, Y + 6, Z + 9], "minecraft:grass_block", "replace minecraft:air")
    # The front: the door, lamps over it, dampers beside it.
    chat(f"setblock {X} {Y + 1} {Z} big_nukes:hermetic_door[half=lower,facing=north]")
    chat(f"setblock {X} {Y + 2} {Z} big_nukes:hermetic_door[half=upper,facing=north]")
    for dx in (-2, 2):
        chat(f"setblock {X + dx} {Y + 3} {Z} big_nukes:bunker_lamp")
    for dx in (-4, 4):
        chat(f"setblock {X + dx} {Y + 2} {Z} big_nukes:blast_damper")
    # Inside.
    for dx, dz in ((-2, 3), (2, 3), (-2, 6), (2, 6)):
        chat(f"setblock {X + dx} {Y + 4} {Z + dz} big_nukes:bunker_lamp")
    fill([X - 5, Y + 1, Z + 1], [X - 5, Y + 3, Z + 7], "big_nukes:lead_shield")
    fill([X + 5, Y + 1, Z + 1], [X + 5, Y + 3, Z + 7], "big_nukes:lead_shield")
    for dx in (-3, -2):
        chat(f"setblock {X + dx} {Y + 1} {Z + 7} big_nukes:fuel_generator[facing=north]")
        chat(f"item replace block {X + dx} {Y + 1} {Z + 7} container.0 with minecraft:coal 64")
    chat(f"setblock {X} {Y + 1} {Z + 7} big_nukes:decontaminator[facing=north]")
    for dx in (2, 3):
        chat(f"setblock {X + dx} {Y + 1} {Z + 7} big_nukes:air_filter[facing=north]")
    for z in (Z + 2, Z + 5):
        chat(f"setblock {X - 4} {Y + 1} {z} minecraft:red_bed[facing=south,part=head]")
        chat(f"setblock {X - 4} {Y + 1} {z + 1} minecraft:red_bed[facing=south,part=foot]")
    for x, y, z in ((X + 4, Y + 1, Z + 2), (X + 4, Y + 1, Z + 3), (X + 4, Y + 2, Z + 2)):
        chat(f"setblock {x} {y} {z} minecraft:barrel[facing=up]")
    chat(f"setblock {X + 4} {Y + 1} {Z + 5} minecraft:chest[facing=west]")
    chat(f"setblock {X + 4} {Y + 1} {Z + 6} minecraft:crafting_table")
    chat("kill @e[type=minecraft:item]")
    ctl("wait", 80)
    look("bunker-outside", X - 5.5, Y + 3.6, Z - 10, -25, 9)
    look("bunker-inside", X + 0.5, Y + 1.1, Z + 1.6, 0, 7, fov=75)


def silo():
    """A missile lifting off its silo: a concrete apron, the hatch slid aside, the launch console, smoke and fire
    out of the shaft. The missile is the mod's own flight display, put where a launch has it a moment in."""
    X, G, Z = 697, 62, 392
    go(X, G + 10, Z + 12)
    fill([X - 4, G + 1, Z - 4], [X + 4, G + 6, Z + 4], "minecraft:air")
    fill([X - 4, G, Z - 4], [X + 4, G, Z + 4], "minecraft:grass_block")
    fill([X - 3, G, Z - 3], [X + 3, G, Z + 3], "big_nukes:reinforced_concrete")
    fill([X - 1, G - 9, Z - 1], [X + 1, G, Z + 1], "big_nukes:reinforced_concrete")
    fill([X, G - 8, Z], [X, G, Z], "minecraft:air")
    chat(f"setblock {X} {G - 8} {Z} big_nukes:launch_pad")
    chat(f"setblock {X - 1} {G} {Z} big_nukes:silo_hatch")
    chat(f"setblock {X + 3} {G + 1} {Z + 1} big_nukes:launch_console[facing=west]")
    chat("kill @e[type=minecraft:item]")
    up = G + 4.5
    display = ('{item:{id:"big_nukes:missile_model",count:1},item_display:"none",Rotation:[0f,-90f],'
               'brightness:{sky:15,block:15},view_range:12f,Tags:["bn_shot"],transformation:{left_rotation:[0f,0f,0f,1f],'
               'right_rotation:[0f,0f,0f,1f],translation:[0f,0f,3f],scale:[2f,2f,2f]}}')
    chat("kill @e[type=minecraft:item_display,tag=bn_shot]")
    chat(f"summon minecraft:item_display {X + 0.5} {up} {Z + 0.5} {display}")
    ctl("options-set", "fov", 70)
    chat(f"tp @s {X + 7.5} {G + 2.6} {Z + 8.5} 140 -14")
    ctl("wait", 40)
    for _ in range(6):
        chat(f"particle minecraft:campfire_cosy_smoke {X + 0.5} {G + 1} {Z + 0.5} 1.0 0.2 1.0 0.01 25 force")
        h = (up - G - 1) / 2
        chat(f"particle minecraft:cloud {X + 0.5} {G + 1 + h:.2f} {Z + 0.5} 0.2 {h / 2.5:.2f} 0.2 0.01 40 force")
        chat(f"particle minecraft:flame {X + 0.5} {up - 0.2} {Z + 0.5} 0.12 0.3 0.12 0.01 30 force")
        ctl("wait", 2)
    snap("silo")
    chat("kill @e[type=minecraft:item_display,tag=bn_shot]")


def crater_blast():
    """A tactical warhead on a point of the coast: its cloud rising over the fires, seen from over the bay."""
    x, y, z = 760, 64, -10
    ctl("options-set", "fov", 70)
    go(x, y + 45, z + 65, 120)
    chat(f"tp @s {x} {y + 45} {z + 65} 180 35")
    chat(f"bignukes blast 50 {x} {y} {z}")
    ctl("wait", 200)
    snap("crater-cloud")


def crater_after():
    """The same crater once its cloud has gone: the bowl, the ash and the fires on the rim."""
    look("crater", 760, 100, 35, 180, 42, 40)


def cloud_noon():
    """An atomic bomb 420 blocks off over the plains at noon, the river leading to it."""
    ctl("options-set", "fov", 50)
    spot = "636 86 256 -85 -14"
    go(636, 86, 256, 200)
    chat(f"tp @s {spot}")
    ctl("wait", 20)
    chat("bignukes blast 250 1050 70 320")
    ctl("wait", 700)
    chat(f"tp @s {spot}")
    ctl("wait", 2)
    snap("cloud-noon")


def cloud_sunset():
    """A bigger bomb 400 blocks off to the west at sunset, from the top of a hill, the sun beside the cloud."""
    chat("time set 12500")
    ctl("options-set", "fov", 50)
    spot = "576 113 300 82 -16"
    go(576, 113, 300, 200)
    chat(f"tp @s {spot}")
    ctl("wait", 20)
    chat("bignukes blast 300 180 65 350")
    ctl("wait", 900)
    chat(f"tp @s {spot}")
    ctl("wait", 2)
    snap("cloud-sunset")


world()
workshop()
sunbreaker()
bunker()
silo()
crater_blast()
cloud_noon()
ctl("wait", 5000)  # the crater's cloud gone: it fades from 3 minutes on and is out at 5
crater_after()
cloud_sunset()
ctl("quit")
