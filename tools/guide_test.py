#!/usr/bin/env python3
"""Checks the guide in the game: a player joining a world for the first time gets big_nukes:guide, a right click
opens the mod's screen, the arrow keys, the wheel, the arrows and the bookmarks turn its pages (target/shots/guide-*.png
show them), Escape closes it and it opens again where it was left, a second join gives no second guide, and
/bignukes guide gives one more.

    python tools/guide_test.py GAME_FOLDER VERSION
"""
import json
import math
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
GUIDE = "big_nukes:guide"
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
# runtime.rs: BOOK_SIZE, TAB_OPEN_X, TAB_TOP, TAB_STEP, TAB_SIZE, NEXT_AT, PREV_AT.
BOOK_W, BOOK_H, TAB_OPEN_X, TAB_TOP, TAB_STEP, TAB_H = 288, 192, -26, 14, 20, 18
NEXT_AT, PREV_AT = (258, 176, 18, 10), (12, 176, 18, 10)
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


def guides():
    return ctl("inventory").get("counts", {}).get(GUIDE, 0)


def screen():
    return ctl("screen").get("screen")


def shot(name):
    ctl("wait", 2)
    ctl("screenshot", os.path.join(SHOTS, f"guide-{name}.png"))


def origin():
    """Where the book's top left corner is, as guide_origin reckons it."""
    w, h = ctl("ui").get("gui_size") or (0, 0)
    return math.floor((w - (BOOK_W - TAB_OPEN_X)) / 2) - TAB_OPEN_X, math.floor((h - BOOK_H) / 2)


def click(x, y):
    ox, oy = origin()
    ctl("click-at", ox + x, oy + y)
    ctl("wait", 2)


def open_world():
    ctl("launch", "--world", WORLD, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)


os.makedirs(SHOTS, exist_ok=True)
shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
open_world()
check("the first join gives the guide", guides() == 1, json.dumps(ctl("inventory"))[:300])
check("no written book any more", ctl("inventory").get("counts", {}).get("minecraft:written_book", 0) == 0)
ctl("select", GUIDE)
ctl("use-item")
ctl("wait", 5)
check("a right click opens the guide's screen", screen() == "mod", json.dumps(ctl("screen"))[:300])
shot("1-uranium")
ctl("key", "right", "tap")
shot("2-centrifuge")
ctl("key", "left", "tap")
ox, oy = origin()
ctl("scroll-at", ox + 140, oy + 90, -1)
shot("3-wheel")
# The bookmarks, top to bottom: the assembly chapter, the radiation chapter.
click(TAB_OPEN_X + 13, TAB_TOP + 2 * TAB_STEP + TAB_H // 2)
shot("4-assembly")
click(NEXT_AT[0] + 9, NEXT_AT[1] + 5)
shot("5-assembly-recipes")
click(TAB_OPEN_X + 13, TAB_TOP + 6 * TAB_STEP + TAB_H // 2)
shot("6-radiation")
check("clicks keep the screen open", screen() == "mod", str(screen()))
ctl("key", "esc", "tap")
ctl("wait", 3)
check("Escape closes it", screen() != "mod", str(screen()))
ctl("use-item")
ctl("wait", 5)
shot("7-reopened")
check("it opens again", screen() == "mod", str(screen()))
ctl("key", "esc", "tap")
ctl("quit")
time.sleep(8)
open_world()
check("a second join gives no second guide", guides() == 1, str(guides()))
ctl("command", "bignukes guide")
ctl("wait", 5)
check("/bignukes guide gives one more", guides() == 2, str(guides()))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
