#!/usr/bin/env python3
"""Checks in the game that every block and item of the mod exists: /give of each, /setblock of
each block, and no load error about the mod's content in the log.

    python tools/content_test.py GAME_FOLDER VERSION
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
BLOCKS = ["centrifuge", "assembly_table", "demolition_charge", "tactical_warhead", "atomic_bomb",
          "uranium_ore", "deepslate_uranium_ore", "ash"]
ITEMS = ["raw_uranium", "enriched_uranium", "nuclear_charge", "bomb_casing", "fuse_detonator",
         "keypad_detonator", "receiver_detonator", "remote"]
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


shutil.rmtree(os.path.join(GAME, "saves", "bn-content"), ignore_errors=True)
ctl("launch", "--world", "bn-content", "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
ctl("wait", 60)
for _ in range(60):
    player = ctl("state").get("player") or {}
    pos = player.get("pos") or player.get("position")
    if pos:
        break
    ctl("wait", 10)
cx, cy, cz = [int(v // 1) for v in pos]
for i, name in enumerate(BLOCKS + ITEMS):
    said = chat(f"give @s big_nukes:{name}")
    check(f"/give big_nukes:{name}", "Gave" in said or "Выдано" in said, said[:200])
for i, name in enumerate(BLOCKS):
    x = cx + 2 + i
    chat(f"setblock {x} {cy + 5} {cz} big_nukes:{name}")
    got = str(ctl("block", x, cy + 5, cz).get("block"))
    check(f"/setblock big_nukes:{name}", f"big_nukes:{name}" in got, got)
ctl("quit")
time.sleep(6)
with open(LOG, encoding="utf-8", errors="replace") as f:
    bad = [l for l in f if "big_nukes" in l and any(w in l.lower() for w in ("error", "warn", "unknown", "missing", "refus"))]
check("no load warning about the mod's content", not bad, "".join(bad)[:600])
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
