#!/usr/bin/env python3
"""Checks in the game what radiation stands on: /damage takes big_nukes:radiation, a death by it
reads as ours, and the game ticks the player in single player (the log's "player tick" lines).

    python tools/rad_probe.py GAME_FOLDER VERSION
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
LOG = os.path.join(GAME, "logs", "latest.log")
WORLD = "bn-rad-probe"
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


def health():
    p = ctl("state").get("player") or {}
    return p.get("health")


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
ctl("wait", 80)
chat("gamemode survival")
ctl("wait", 5)
before = health()
said = chat("damage @s 4 big_nukes:radiation")
ctl("wait", 5)
after = health()
print(f" health {before} -> {after}: {said}")
check("/damage takes big_nukes:radiation", after is not None and before is not None and after < before, said)
said = chat("damage @s 1000 big_nukes:radiation")
ctl("wait", 10)
lines = json.dumps(ctl("messages", 8), ensure_ascii=False)
print(" chat:", lines[:500])
check("the death reads as ours", "radiation sickness" in lines or "лучевой" in lines, lines[:300])
ctl("quit")
time.sleep(6)
with open(LOG, encoding="utf-8", errors="replace") as f:
    log = f.read()
ticks = [l for l in log.splitlines() if "player tick" in l]
print(" log:", ticks[:3], [l for l in log.splitlines() if "known to /damage" in l][:1])
check("the game ticks the player in single player", bool(ticks), "no line")
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
