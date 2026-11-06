#!/usr/bin/env python3
"""Checks that radiation stays quiet with only the native half: with no damage type, effect,
suit or anti-rad in the game, a player at a blast's center isn't sickened or killed.

    python tools/rad_halves_test.py GAME_FOLDER VERSION

The instance's horizon-mods.txt is put back afterwards.
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
LIST = os.path.join(GAME, "horizon-mods.txt")
WORLD = "bn-rad-halves"
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


saved = LIST + ".saved"
shutil.copyfile(LIST, saved)
try:
    with open(LIST, "w", encoding="utf-8", newline="\n") as f:
        f.write("mod.big_nukes:false\nmod.big_nukes_native:true\n")
    shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
    ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)
    player = ctl("state").get("player") or {}
    cx, cy, cz = [int(v // 1) for v in (player.get("pos") or player.get("position"))]
    bx, by, bz = cx + 3000, 250, cz
    chat(f"tp @s {bx} {by + 2} {bz + 600}")
    ctl("wait", 40)
    chat(f"bignukes blast 250 {bx} {by} {bz}")
    ctl("wait", 300)
    chat(f"tp @s {bx + 0.5} {by + 2} {bz + 0.5}")
    for _ in range(20):
        ctl("wait", 20)
        if "not loaded" not in chat(f"fill {bx - 3} {by - 1} {bz - 3} {bx + 3} {by - 1} {bz + 3} minecraft:glass"):
            break
    chat(f"tp @s {bx + 0.5} {by} {bz + 0.5}")
    ctl("wait", 5)
    chat("gamemode survival")
    ctl("wait", 1200)
    state = json.dumps(ctl("state"), ensure_ascii=False)
    lines = json.dumps(ctl("messages", 10), ensure_ascii=False)
    effects = state[state.find('"effects"'):state.find('"effects"') + 160]
    print(" effects:", effects)
    check("no sickness without the assets half", "weakness" not in effects and "nausea" not in effects, effects)
    check("no death without the assets half", "died" not in lines and "confines" not in lines and "left" not in lines, lines[:300])
    ctl("quit")
    time.sleep(6)
finally:
    shutil.move(saved, LIST)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
