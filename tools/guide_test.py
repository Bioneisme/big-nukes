#!/usr/bin/env python3
"""Checks the guide in the game: a player joining a world for the first time gets the book, it
opens and reads its pages in the game's language, a second join gives no second book, and
/bignukes guide gives one more.

    python tools/guide_test.py GAME_FOLDER VERSION
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
WORLD = "bn-guide"
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


def books():
    inv = json.dumps(ctl("inventory"))
    return inv.count('"minecraft:written_book"')


def open_world():
    ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)


shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
open_world()
check("the first join gives the guide", books() == 1, json.dumps(ctl("inventory"))[:300])
ctl("select", "minecraft:written_book")
ctl("use-item")
ctl("wait", 5)
first = json.dumps(ctl("book"), ensure_ascii=False)
print(" page 1:", first[:300])
check("the book opens on its contents", "Big Nukes" in first and ("Uranium" in first or "Уран" in first), first[:300])
for _ in range(9):
    ctl("book", "next")
    ctl("wait", 2)
last = json.dumps(ctl("book"), ensure_ascii=False)
print(" page 10:", last[:300])
check("the last page lists the commands", "/bignukes guide" in last, last[:300])
check("no page shows a bare lang key", "big_nukes.guide." not in first + last)
ctl("book", "done")
ctl("quit")
time.sleep(8)
open_world()
check("a second join gives no second book", books() == 1, str(books()))
ctl("command", "bignukes guide")
ctl("wait", 5)
check("/bignukes guide gives one more", books() == 2, str(books()))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
