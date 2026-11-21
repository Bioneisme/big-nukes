#!/usr/bin/env python3
"""Checks uranium ore in a fresh world: some below y=16, none above.

    python tools/ore_test.py GAME_FOLDER VERSION
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time
import play

GAME, VERSION = sys.argv[1:3]
CTL = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", VERSION, "horizon-ctl.exe")
os.environ["HORIZON_HOME"] = GAME
os.environ["MSYS_NO_PATHCONV"] = "1"
sys.stdout.reconfigure(encoding="utf-8")
failed = []


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def check(name, ok, detail=""):
    print(("ok  " if ok else "FAIL"), name, flush=True)
    if not ok:
        failed.append(f"{name}: {detail}")


def count(x, z, y0, y1, block):
    """How many of `block` a 16x16 column from y0 to y1 holds (fill counts what it replaces)."""
    said = " ".join(ctl("command", f"fill {x} {y0} {z} {x + 15} {y1} {z + 15} minecraft:stone replace {block}").get("chat", []))
    m = re.search(r"(\d+)", said)
    return int(m.group(1)) if m and "filled" in said.lower() else 0


def world(name):
    shutil.rmtree(os.path.join(GAME, "saves", name), ignore_errors=True)
    r = play.launch(ctl, name, "4242")
    ctl("wait", 60)
    return r


world("bn-ore")
player = ctl("state").get("player") or {}
pos = player.get("pos") or player.get("position") or [0, 0, 0]
bx, bz = int(pos[0]) // 16 * 16, int(pos[2]) // 16 * 16
low = high = 0
for i in range(6):
    x, z = bx + (i % 3) * 16, bz + (i // 3) * 16
    low += count(x, z, -63, -24, "big_nukes:deepslate_uranium_ore") + count(x, z, -23, 16, "big_nukes:deepslate_uranium_ore")
    low += count(x, z, -63, -24, "big_nukes:uranium_ore") + count(x, z, -23, 16, "big_nukes:uranium_ore")
    high += count(x, z, 17, 80, "big_nukes:uranium_ore")
print(f" ore below y=16: {low}, above: {high}")
check("uranium ore is generated below y=16", low > 0, str(low))
check("no uranium ore above y=16", high == 0, str(high))
ctl("quit")
time.sleep(8)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
