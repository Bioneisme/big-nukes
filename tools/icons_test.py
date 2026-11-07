#!/usr/bin/env python3
"""Shows every block and item of the mod for the eye: all of them in a survival inventory, then
the mod's creative tab. Frames go to target/shots/icons-*.png.

    python tools/icons_test.py GAME_FOLDER VERSION
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
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
TAB = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "content", "data", "big_nukes", "horizon", "creative_tab", "big_nukes.json")
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


with open(TAB, encoding="utf-8") as f:
    items = json.load(f)["items"]
shutil.rmtree(os.path.join(GAME, "saves", "bn-icons"), ignore_errors=True)
ctl("launch", "--world", "bn-icons", "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
ctl("wait", 60)
ctl("command", "gamemode survival")
ctl("command", "clear @s")
gave = [i for i in items if "Gave" in " ".join(map(str, ctl("command", f"give @s {i}").get("chat", [])))]
check("every item of the tab can be given", len(gave) == len(items), str(set(items) - set(gave)))
ctl("key", "e", "tap")
ctl("wait", 10)
ctl("screenshot", os.path.join(SHOTS, "icons-inventory.png"))
ctl("key", "esc", "tap")
ctl("command", "gamemode creative")
ctl("wait", 5)
ctl("key", "e", "tap")
ctl("wait", 10)
# The mod's tab comes after vanilla's, on the creative screen's second page: the arrow over the
# tabs turns it (GUI scale 2: 86 GUI pixels right of the middle, 71 down), then the first tab there.
from PIL import Image
probe = os.path.join(SHOTS, "icons-creative.png")
ctl("screenshot", probe)
w, h = Image.open(probe).size
ctl("click-at", w // 4 + 86, 71)
ctl("wait", 5)
ctl("click-at", w // 4 - 85, 100)
ctl("wait", 10)
ctl("screenshot", os.path.join(SHOTS, "icons-tab.png"))


def visible():
    return {w["stack"]["item"] for w in ctl("ui").get("widgets", []) if w.get("kind") == "slot" and w.get("id", "").startswith("slot-9") and w.get("stack")}


# More than five rows scroll: the wheel over the grid (GUI pixels) brings the rest up a row at a time.
shown = visible()
for _ in range(6):
    ctl("scroll-at", w // 4, 180, -1)
    ctl("wait", 3)
    shown |= visible()
ctl("screenshot", os.path.join(SHOTS, "icons-tab-scrolled.png"))
check("the Big Nukes tab shows every block and item", set(items) <= shown, str(set(items) - shown))
ctl("key", "esc", "tap")
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
