#!/usr/bin/env python3
"""Checks the chain in the game: a blast sets off the bombs its wave reaches, with or without a
detonator, and leaves those beyond its radius standing.

    python tools/chain_test.py GAME_FOLDER VERSION

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
WORLD = "bn-chain"
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


def gone(x, y, z):
    return "big_nukes" not in json.dumps(ctl("block", x, y, z))


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
ctl("wait", 60)
for _ in range(60):
    player = ctl("state").get("player") or {}
    pos = player.get("pos") or player.get("position")
    if pos:
        break
    ctl("wait", 10)
cx, cy, cz = [int(v // 1) for v in pos]
y = 230
chat(f"fill {cx - 4} {y - 1} {cz - 4} {cx + 95} {y - 1} {cz + 4} minecraft:stone")
near, far = (cx + 20, y, cz), (cx + 90, y, cz)
chat(f"setblock {near[0]} {near[1]} {near[2]} big_nukes:demolition_charge")
chat(f"setblock {far[0]} {far[1]} {far[2]} big_nukes:tactical_warhead")
chat(f"tp @s {cx + 40} {y + 30} {cz + 40}")
ctl("wait", 10)
placed = saved().get("placed", [])
check("the world knows both bombs", len(placed) == 2, str(placed)[:300])
before = saved().get("next_id", 0)
chat(f"bignukes blast 50 {cx} {y} {cz}")
ctl("wait", 200)
after = saved().get("next_id", 0)
check("the wave set the near bomb off", after == before + 2 and gone(*near), f"blasts {after - before}, {json.dumps(ctl('block', *near))[:150]}")
check("the far bomb stands", not gone(*far), json.dumps(ctl("block", *far))[:150])
left = [b["pos"] for b in saved().get("placed", [])]
check("only the far bomb is still known", left == [list(far)], str(left))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
