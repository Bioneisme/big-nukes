#!/usr/bin/env python3
"""Checks the silo in the game: a shaft of a launch pad, air and a hatch under the open sky, a console beside it;
a loaded pad shows its missile standing in the shaft, with or without its warhead, and stops when it is taken out;
the console refuses a target too near, a wrong code and a shaft with no hatch, each with its reason; through the
countdown the one missile stands in the shaft, then a launch opens the hatch, the missile climbs out as a display
(target/shots/silo-*.png), the hatch shuts, and the missile comes down 1500 blocks off and goes off there, its
target's chunk let go after; a splitting missile parts into three warheads that go off round its target; a
launch survives the world closing mid-flight.

    python tools/silo_test.py GAME_FOLDER VERSION
"""
import json
import math
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
WORLD = "bn-silo"
SAVED = os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json")
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
# silo.rs: the console's screen, 176 x 240, and where its fields and buttons are: x, z, code, spread.
SCREEN_W, SCREEN_H = 176, 240
FIELDS = [(8, 28, 52, 14), (64, 28, 52, 14), (120, 28, 48, 14), (120, 128, 48, 14)]
KEYS_AT, KEY, KEY_GAP = (8, 66), (22, 13), 2
KEY_ROWS = ["123", "456", "789", "-0<"]
ERASE_AT, LAUNCH_AT = (86, 66, 82, 13), (86, 81, 82, 28)
CODE = "1234"
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


def messages():
    return json.dumps(ctl("messages", 20), ensure_ascii=False)


def displays(where=""):
    said = chat(f"execute if entity @e[type=minecraft:item_display{where}]")
    digits = "".join(c for c in said.split(":")[-1] if c.isdigit())
    return int(digits) if "passed" in said.lower() and digits else 0


def gametime():
    """The game's clock in ticks, from `/time query gametime`."""
    said = chat("time query gametime")
    digits = "".join(c for c in said.split(":")[-1] if c.isdigit()) or "".join(c for c in said if c.isdigit())
    return int(digits) if digits else 0


def at_pad(sx, sy, sz, tag=""):
    """The displays standing in the shaft over the pad, with `tag` if given."""
    return displays(f",x={sx + 0.5},y={sy + 1},z={sz + 0.5},distance=..2" + (f",tag={tag}" if tag else ""))


def open_world():
    play.launch(ctl, WORLD, "12345")
    ctl("wait", 60)
    for _ in range(60):
        player = ctl("state").get("player") or {}
        pos = player.get("pos") or player.get("position")
        if pos:
            return [int(v // 1) for v in pos]
        ctl("wait", 10)
    raise RuntimeError("no player")


def origin():
    w, h = ctl("ui").get("gui_size") or (0, 0)
    return math.floor((w - SCREEN_W) / 2), math.floor((h - SCREEN_H) / 2)


def press(rect):
    ox, oy = origin()
    x, y, w, h = rect
    ctl("click-at", ox + x + w / 2, oy + y + h / 2)
    ctl("wait", 1)


def key_rect(c):
    for row, keys in enumerate(KEY_ROWS):
        if c in keys:
            col = keys.index(c)
            return (KEYS_AT[0] + col * (KEY[0] + KEY_GAP), KEYS_AT[1] + row * (KEY[1] + KEY_GAP), *KEY)
    raise ValueError(c)


def fill(field, text, typed):
    """A field's text, typed on the keyboard or pressed on the console's keys."""
    press(FIELDS[field])
    if typed:
        ctl("type", text)
        ctl("wait", 2)
    else:
        for c in text:
            press(key_rect(c))


def console(at, x, z, code, typed=False, spread=""):
    """Opens the console at `at`, enters a target, a code and a spread, and presses Launch."""
    ctl("interact", *at)
    ctl("wait", 10)
    press(ERASE_AT)
    for field in range(len(FIELDS)):
        press(FIELDS[field])
        press(ERASE_AT)
    fill(0, str(x), typed)
    fill(1, str(z), typed)
    fill(2, code, typed)
    if spread:
        fill(3, spread, typed)
    press(LAUNCH_AT)
    ctl("wait", 10)


def build(sx, sy, sz):
    """The shaft: a pad at sy, 6 air, the hatch, open sky; the console 2 blocks east; glass to stand on."""
    chat(f"fill {sx - 3} {sy - 1} {sz - 3} {sx + 4} {sy - 1} {sz + 4} minecraft:glass")
    chat(f"fill {sx} {sy + 1} {sz} {sx} {sy + 6} {sz} minecraft:air")
    chat(f"setblock {sx} {sy} {sz} big_nukes:launch_pad")
    chat(f"setblock {sx} {sy + 7} {sz} big_nukes:silo_hatch")
    chat(f"setblock {sx + 2} {sy} {sz} big_nukes:launch_console")


def load(sx, sy, sz):
    chat(f"item replace block {sx} {sy} {sz} container.0 with big_nukes:missile")
    chat(f"item replace block {sx} {sy} {sz} container.1 with big_nukes:atomic_bomb")
    ctl("wait", 2)
    return json.dumps(ctl("container", sx, sy, sz), ensure_ascii=False)


os.makedirs(SHOTS, exist_ok=True)
shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
sx, sy, sz = cx + 6, 220, cz
# The floor first, then the player on it: a player put in the air falls before the glass is there.
build(sx, sy, sz)
chat(f"tp @s {sx + 2.5} {sy} {sz + 2.5}")
ctl("wait", 40)
at = (sx + 2, sy, sz)
ctl("look-at", "block", *at)
held = load(sx, sy, sz)
check("the pad takes a missile and an atomic bomb", "big_nukes:missile" in held and "atomic_bomb" in held, held[:300])
check("a loaded pad shows its missile in the shaft", at_pad(sx, sy, sz, "big_nukes_pad") == 1 and displays() == 1,
      chat("execute if entity @e[type=minecraft:item_display]"))
ctl("screenshot", os.path.join(SHOTS, "silo-loaded.png"))
chat(f"item replace block {sx} {sy} {sz} container.0 with minecraft:air")
ctl("wait", 2)
check("taking the missile out takes it away", displays() == 0, chat("execute if entity @e[type=minecraft:item_display]"))
chat(f"item replace block {sx} {sy} {sz} container.1 with minecraft:air")
chat(f"item replace block {sx} {sy} {sz} container.0 with big_nukes:missile")
ctl("wait", 2)
check("a missile without its warhead shows too", at_pad(sx, sy, sz, "big_nukes_pad") == 1 and displays() == 1,
      chat("execute if entity @e[type=minecraft:item_display]"))
held = load(sx, sy, sz)
ctl("wait", 40)
check("it stays one display a few seconds on", at_pad(sx, sy, sz) == 1 and displays() == 1,
      chat("execute if entity @e[type=minecraft:item_display]"))

# The console says why it won't launch.
console(at, sx + 50, sz, CODE, typed=True)
said = messages()
check("a target 50 blocks off is too near (typed on the keyboard)", "too near" in said or "50 blocks away" in said or "близко" in said, said[-400:])
chat(f"setblock {sx} {sy + 7} {sz} minecraft:air")
console(at, sx + 1500, sz, CODE)
said = messages()
check("a shaft without its hatch doesn't launch", "hatch" in said or "люк" in said, said[-400:])
chat(f"setblock {sx} {sy + 7} {sz} big_nukes:silo_hatch")
ctl("key", "esc", "tap")
ctl("wait", 5)

# The launch.
console(at, sx + 1500, sz, CODE)
launched = gametime()
ctl("key", "e", "tap")
ctl("wait", 2)
check("E doesn't close the console", ctl("screen").get("screen") == "container", str(ctl("screen"))[:100])
said = messages()
flights = saved().get("flights", [])
check("Launch starts a flight", len(flights) == 1, f"{flights} / {said[-300:]}")
check("in the countdown one missile stands in the shaft, the flight's",
      at_pad(sx, sy, sz, "big_nukes_flight") == 1 and displays() == 1,
      chat("execute if entity @e[type=minecraft:item_display]") + " / " + chat("execute if entity @e[type=minecraft:item_display,tag=big_nukes_pad]"))
ctl("key", "esc", "tap")
ctl("wait", 2)
check("Escape closes it", ctl("screen").get("screen") == "playing", str(ctl("screen"))[:100])
# Late in the countdown by the game's clock: each command to the game takes about a second of the countdown's 5.
# The launch was some 10 ticks before `launched` (the console's last wait); its liftoff 100 after it.
late = launched + 65 - gametime()
if late > 0:
    ctl("wait", late)
check("still one missile in the shaft late in the countdown", at_pad(sx, sy, sz, "big_nukes_flight") == 1 and displays() == 1,
      f"{late} ticks waited / " + chat("execute if entity @e[type=minecraft:item_display]"))
ctl("wait", 18)
check("the hatch opens in the countdown", "air" in json.dumps(ctl("block", sx, sy + 7, sz)), str(ctl("block", sx, sy + 7, sz)))
ctl("wait", 40)
ctl("screenshot", os.path.join(SHOTS, "silo-liftoff.png"))
check("the missile climbs out as a display", displays() >= 1, chat("execute if entity @e[type=minecraft:item_display]"))
ctl("wait", 40)
ctl("screenshot", os.path.join(SHOTS, "silo-climb.png"))
ctl("wait", 140)
check("the hatch shuts after the climb", "silo_hatch" in json.dumps(ctl("block", sx, sy + 7, sz)), str(ctl("block", sx, sy + 7, sz)))
# Down at the target: 1500 blocks east, watched from 300 blocks off it.
tx, tz = sx + 1500, sz
chat(f"tp @s {tx - 300} 260 {tz} -90 15")
for _ in range(60):
    ctl("wait", 20)
    if saved().get("zones"):
        break
ctl("screenshot", os.path.join(SHOTS, "silo-impact.png"))
zones = saved().get("zones", [])
near = [z for z in zones if abs(z["center"][0] - tx) <= 2 and abs(z["center"][2] - tz) <= 2] if zones else []
check("the missile goes off on its target", bool(near), json.dumps(zones)[:300])
for _ in range(30):
    ctl("wait", 20)
    if not saved().get("forced"):
        break
check("the target's chunk is let go after the wave", not saved().get("forced"), json.dumps(saved().get("forced")))
check("no flight is left", not saved().get("flights"), json.dumps(saved().get("flights"))[:300])

# The splitting missile: three tactical warheads round a target 1500 blocks south, on a triangle of side 100.
chat(f"tp @s {sx + 2.5} {sy} {sz + 2.5}")
ctl("wait", 40)
ctl("look-at", "block", *at)
chat(f"item replace block {sx} {sy} {sz} container.0 with big_nukes:splitting_missile")
chat(f"item replace block {sx} {sy} {sz} container.1 with minecraft:air")
ctl("wait", 2)
check("a pad takes a splitting missile alone", at_pad(sx, sy, sz, "big_nukes_pad") == 1, chat("execute if entity @e[type=minecraft:item_display]"))
zones_before = len(saved().get("zones", []))
tx, tz = sx, sz + 1500
siren = (sx - 3, sy, sz)
chat(f"setblock {siren[0]} {siren[1]} {siren[2]} big_nukes:siren")
console(at, tx, tz, CODE, spread="100")
ctl("key", "esc", "tap")
ctl("wait", 20)
lit = json.dumps(ctl("block", *siren))
check("a siren by the pad lights at the launch", re.search(r"lit\W+true", lit) is not None, lit[:200])
check("the world keeps the launch's alarms", len(saved().get("alarms", [])) >= 2, json.dumps(saved().get("alarms"))[:300])
flights = saved().get("flights", [])
check("a splitting missile is three flights", len(flights) == 3 and sum(1 for f in flights if f.get("rider")) == 2, json.dumps(flights)[:400])
check("one missile stands in the shaft", at_pad(sx, sy, sz, "big_nukes_flight") == 1 and displays() == 1,
      chat("execute if entity @e[type=minecraft:item_display]"))
chat(f"tp @s {tx} 300 {tz - 300} 0 25")
for _ in range(80):
    ctl("wait", 20)
    if len(saved().get("zones", [])) >= zones_before + 3:
        break
ctl("screenshot", os.path.join(SHOTS, "silo-split.png"))
zones = saved().get("zones", [])[zones_before:]
round_it = [z for z in zones if abs(z["center"][0] - tx) <= 70 and abs(z["center"][2] - tz) <= 70]
check("three warheads go off round the target", len(round_it) == 3, json.dumps(zones)[:400])
centers = sorted((z["center"][0], z["center"][2]) for z in round_it)
apart = [math.dist(a, b) for i, a in enumerate(centers) for b in centers[i + 1:]]
check("about 100 blocks apart", len(apart) == 3 and all(abs(d - 100) <= 3 for d in apart), str(apart))
for _ in range(30):
    ctl("wait", 20)
    if not saved().get("flights") and not saved().get("forced"):
        break
check("no flight or chunk is left after the split", not saved().get("flights") and not saved().get("forced"),
      json.dumps([saved().get("flights"), saved().get("forced")])[:300])

# A wrong code, then a launch the world closes on.
chat(f"tp @s {sx + 2.5} {sy} {sz + 2.5}")
ctl("wait", 40)
ctl("look-at", "block", *at)
load(sx, sy, sz)
console(at, sx + 1500, sz + 400, "9999")
said = messages()
check("a wrong code doesn't launch", "Wrong code" in said or "Неверный" in said or "неверный" in said, said[-400:])
console(at, sx + 1500, sz + 400, CODE)
ctl("key", "esc", "tap")
ctl("wait", 300)
check("the second flight is on its way", len(saved().get("flights", [])) == 1, json.dumps(saved().get("flights"))[:300])
ctl("quit")
time.sleep(8)
open_world()
for _ in range(60):
    ctl("wait", 20)
    if len(saved().get("zones", [])) >= 2:
        break
zones = saved().get("zones", [])
check("a flight goes on after the world reopens", len(zones) >= 2, json.dumps(zones)[:300])
ctl("wait", 220)
check("no display is left without its flight", displays() == 0, chat("execute if entity @e[type=minecraft:item_display]"))
# A loaded pad's display goes with the world too: after it reopens there is one, and none once the pad is broken.
load(sx, sy, sz)
ctl("quit")
time.sleep(8)
open_world()
ctl("wait", 40)
check("a loaded pad shows one missile after the world reopens", at_pad(sx, sy, sz, "big_nukes_pad") == 1 and displays() == 1,
      chat("execute if entity @e[type=minecraft:item_display]"))
chat(f"setblock {sx} {sy} {sz} minecraft:air destroy")
ctl("wait", 2)
check("a broken pad takes its missile's display away", displays() == 0, chat("execute if entity @e[type=minecraft:item_display]"))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
