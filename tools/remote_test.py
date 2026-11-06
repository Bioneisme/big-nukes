#!/usr/bin/env python3
"""Checks the remote and the operator's limit in the game: linking, a click on the ground that
fires nothing, a shot from 700 blocks that leaves a crater found on return, a second shot and a
shot at a broken bomb that fire nothing, /bignukes maxpower, and links and the limit across a reload.

    python tools/remote_test.py GAME_FOLDER VERSION

Blasts are counted by the world's horizon-big-nukes/waves.json (`next_id`).
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
WORLD = "bn-remote"
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


def blasts():
    return saved().get("next_id", 0)


def last_lines(n=6):
    return json.dumps(ctl("messages", n).get("chat", []), ensure_ascii=False)


def gone(x, y, z):
    return "big_nukes" not in json.dumps(ctl("block", x, y, z))


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


def bomb_with_receiver(x, y, z, block="big_nukes:tactical_warhead"):
    chat(f"setblock {x} {y} {z} {block}")
    chat("clear @s")
    chat("give @s big_nukes:receiver_detonator")
    ctl("select", 1)
    chat(f"tp @s {x + 0.5} {y} {z + 2.5} 180 35")
    ctl("wait", 3)
    ctl("interact", x, y, z)


def link(x, y, z):
    chat("clear @s")
    chat("give @s big_nukes:remote")
    ctl("select", 1)
    chat(f"tp @s {x + 0.5} {y} {z + 2.5} 180 35")
    ctl("wait", 3)
    ctl("interact", x, y, z)
    ctl("wait", 2)
    return last_lines(3)


def fire():
    ctl("look", 0, -90)
    ctl("wait", 2)
    ctl("use-item")
    ctl("wait", 3)
    return last_lines(3)


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
y = cy + 40
chat(f"fill {cx - 6} {y - 1} {cz - 6} {cx + 6} {y - 1} {cz + 6} minecraft:stone")
bx, bz = cx, cz

bomb_with_receiver(bx, y, bz)
said = link(bx, y, bz)
check("the remote links to a bomb with a receiver", "linked" in said.lower() or "связан" in said.lower(), said)
check("the bomb is recorded", len(saved().get("bombs", [])) == 1, str(saved().get("bombs"))[:200])

# A click on the ground fires nothing.
before = blasts()
chat(f"tp @s {bx + 3.5} {y} {bz + 3.5} 0 90")
ctl("wait", 3)
ctl("interact", bx + 3, y - 1, bz + 3)
ctl("wait", 3)
said = last_lines(3)
check("a click on the ground fires nothing", blasts() == before and not gone(bx, y, bz), said)
check("it says to aim at the sky", "sky" in said or "небо" in said, said)

# The operator's limit.
said = chat("bignukes maxpower")
check("/bignukes maxpower tells 1024", "1024" in said, said)
said = chat("bignukes maxpower 100")
check("/bignukes maxpower 100 is refused", blasts() == before and "16" in said and "250" in said, said)
chat("bignukes maxpower 16")
said = fire()
check("a tactical warhead over the limit of 16 refuses", blasts() == before and not gone(bx, y, bz), said)
said = chat(f"bignukes blast 100 {cx + 200} {y} {cz}")
check("/bignukes blast 100 is refused at 16", blasts() == before, said)
chat("bignukes maxpower 1024")

# A shot from 700 blocks.
fx = cx + 700
chat(f"tp @s {fx} {y + 30} {cz}")
ctl("wait", 40)
said = fire()
check("the remote fires from 700 blocks", blasts() == before + 1, said)
check("the record is gone", len(saved().get("bombs", [])) == 0, str(saved().get("bombs"))[:200])
said = fire()
check("a second shot fires nothing", blasts() == before + 1 and ("gone" in said or "нет" in said.lower()), said)
chat(f"tp @s {bx} {y + 30} {bz}")
ctl("wait", 200)
check("back home the bomb is gone", gone(bx, y, bz), json.dumps(ctl("block", bx, y, bz))[:200])

# A broken bomb is forgotten.
qx, qz = cx + 300, cz
chat(f"tp @s {qx} {y + 2} {qz}")
ctl("wait", 40)
chat(f"fill {qx - 3} {y - 1} {qz - 3} {qx + 3} {y - 1} {qz + 3} minecraft:stone")
bomb_with_receiver(qx, y, qz, "big_nukes:atomic_bomb")
link(qx, y, qz)
chat(f"setblock {qx} {y} {qz} minecraft:air")
ctl("wait", 3)
before = blasts()
said = fire()
check("a broken bomb fires nothing", blasts() == before, said)
check("and its remote says it is gone", "gone" in said or "нет" in said.lower(), said)

# A bomb on the Nether's roof, set off from the Overworld: its crater stops below the roof, so the
# bomb must go when its wave works its chunk, not stay to be linked and fired again.
nx, ny, nz = 40, 129, 40
chat(f"execute in minecraft:the_nether run tp @s {nx + 0.5} {ny + 1} {nz + 2.5}")
ctl("wait", 60)
chat(f"fill {nx - 3} {ny - 1} {nz - 3} {nx + 3} {ny - 1} {nz + 3} minecraft:stone")
bomb_with_receiver(nx, ny, nz)
said = link(nx, ny, nz)
check("a bomb on the Nether's roof links", "linked" in said.lower() or "связан" in said.lower(), said)
chat(f"execute in minecraft:overworld run tp @s {cx} {y + 30} {cz}")
ctl("wait", 60)
before = blasts()
said = fire()
check("it fires from the Overworld", blasts() == before + 1, said)
chat(f"execute in minecraft:the_nether run tp @s {nx + 0.5} {ny + 1} {nz + 2.5}")
ctl("wait", 200)
check("back on the roof the bomb is gone", gone(nx, ny, nz), json.dumps(ctl("block", nx, ny, nz))[:200])
said = link(nx, ny, nz)
check("and nothing is left to link", "linked" not in said.lower(), said)
chat(f"execute in minecraft:overworld run tp @s {cx} {y + 2} {cz}")
ctl("wait", 40)

# Links and the limit across a reload.
rx, rz = cx - 300, cz
chat(f"tp @s {rx} {y + 2} {rz}")
ctl("wait", 40)
chat(f"fill {rx - 3} {y - 1} {rz - 3} {rx + 3} {y - 1} {rz + 3} minecraft:stone")
bomb_with_receiver(rx, y, rz, "big_nukes:demolition_charge")
said = last_lines(3)
check("a receiver doesn't go into a demolition charge", "only a fuse" in said or "фитиль" in said, said)
bomb_with_receiver(rx + 2, y, rz)
link(rx + 2, y, rz)
chat("bignukes maxpower 250")
ctl("quit")
time.sleep(8)
open_world()
said = chat("bignukes maxpower")
check("the limit survives a reload", "250" in said, said)
before = blasts()
chat(f"tp @s {rx} {y + 30} {rz + 200}")
ctl("wait", 20)
said = fire()
check("a link survives a reload", blasts() == before + 1, said)
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
