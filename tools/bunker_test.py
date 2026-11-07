#!/usr/bin/env python3
"""Checks the bunker in the game: a box of armored concrete 75 blocks from an atomic blast stands and its player
lives through the wave, a box of reinforced concrete beside it breaks; inside, a generator lights the lamp and powers
the air filter, the sealed room keeps most of the radiation out, and an open hermetic door lets it in
(target/shots/bunker-*.png).

    python tools/bunker_test.py GAME_FOLDER VERSION
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
WORLD = "bn-bunker"
SAVED = os.path.join(GAME, "saves", WORLD, "horizon-big-nukes", "waves.json")
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
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


def dose():
    doses = saved().get("doses", {})
    if not doses:
        return 0.0
    d = next(iter(doses.values()))
    return d.get("rad", 0.0) if isinstance(d, dict) else float(d)


def rate(ticks):
    before = dose()
    ctl("wait", ticks)
    return dose() - before + 0.5 * ticks / 20


def state(x, y, z):
    return ctl("block", x, y, z).get("state", "?")


def count(block, x1, y1, z1, x2, y2, z2):
    return ctl("blocks", x1, y1, z1, x2, y2, z2).get("counts", {}).get(block, 0)


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


def fill(line):
    """A fill once its chunks load."""
    for _ in range(20):
        said = chat(line)
        if "not loaded" not in said:
            return said
        ctl("wait", 20)
    return said


os.makedirs(SHOTS, exist_ok=True)
shutil.rmtree(os.path.join(GAME, "saves", WORLD), ignore_errors=True)
cx, cy, cz = open_world()
chat("gamerule advance_time false")
X, Y, Z = cx + 3000, 100, cz
ax, ay, az = X + 75, 150, Z          # box A, armored, the bunker
bx, by, bz = X + 75, 150, Z + 20     # box B, reinforced
chat(f"tp @s {ax + 0.5} {ay + 8} {az + 0.5}")
ctl("wait", 80)
fill(f"fill {ax - 2} {ay - 2} {az - 2} {ax + 2} {ay + 2} {az + 2} big_nukes:armored_concrete hollow")
fill(f"fill {bx - 2} {by - 2} {bz - 2} {bx + 2} {by + 2} {bz + 2} big_nukes:reinforced_concrete hollow")
# Inside A: the generator, the filter and a lamp; the door in its west wall.
chat(f"setblock {ax - 1} {ay - 1} {az - 1} big_nukes:fuel_generator")
chat(f"item replace block {ax - 1} {ay - 1} {az - 1} container.0 with minecraft:coal 16")
chat(f"setblock {ax + 1} {ay - 1} {az - 1} big_nukes:air_filter")
chat(f"setblock {ax + 1} {ay + 1} {az + 1} big_nukes:bunker_lamp")
chat(f"setblock {ax - 2} {ay} {az} big_nukes:hermetic_door[half=upper,facing=east]")
chat(f"setblock {ax - 2} {ay - 1} {az} big_nukes:hermetic_door[half=lower,facing=east]")
registry = [tuple(w["pos"]) for ws in saved().get("walls", {}).values() for w in ws]
check("the walls are in the registry", len(registry) > 0, json.dumps(saved().get("walls"))[:200])
check("both halves of the door are walls", {(ax - 2, ay - 1, az), (ax - 2, ay, az)} <= set(registry), str(len(registry)))

# The player in the bunker, in survival.
chat(f"tp @s {ax + 0.5} {ay - 1} {az + 1.5}")
chat("gamemode survival")
chat("effect clear @s")
ctl("wait", 160)
lamp = state(ax + 1, ay + 1, az + 1)
check("the generator lights the lamp", "lit=true" in lamp, lamp)
door = state(ax - 2, ay - 1, az)
check("the door stands shut", "open=false" in door, door)

# The blast.
chat(f"bignukes blast 250 {X} {Y} {Z}")
for _ in range(30):
    ctl("wait", 20)
    if not [w for w in saved().get("waves", {}).get("waves", []) if not w.get("deferred")]:
        break
ctl("wait", 40)
health = (ctl("state").get("player") or {}).get("health")
said = json.dumps(ctl("messages", 10).get("chat", []), ensure_ascii=False)
check("the player in the bunker lives through the wave", health is not None and health > 0 and "died" not in said and "погиб" not in said, f"{health} {said[-200:]}")
tags = " ".join(map(str, ctl("command", "tag @s list").get("chat", [])))
check("no shelter tag is left after the wave", "big_nukes_sheltered" not in tags, tags)
a_walls = count("big_nukes:armored_concrete", ax - 2, ay - 2, az - 2, ax + 2, ay + 2, az + 2) + count("big_nukes:cracked_armored_concrete", ax - 2, ay - 2, az - 2, ax + 2, ay + 2, az + 2)
b_walls = count("big_nukes:reinforced_concrete", bx - 2, by - 2, bz - 2, bx + 2, by + 2, bz + 2) + count("big_nukes:cracked_reinforced_concrete", bx - 2, by - 2, bz - 2, bx + 2, by + 2, bz + 2)
print(f" armored left {a_walls}/97, reinforced left {b_walls}/98")
check("armored concrete 75 blocks from an atomic stands", a_walls >= 95, str(a_walls))
check("reinforced concrete there breaks", b_walls <= 10, str(b_walls))

# Radiation: the sealed room, then the door open.
sealed = rate(400)
ctl("select", 9)  # an empty hand: the guide in hand would be used instead
ctl("look-at", "block", ax - 2, ay - 1, az)
ctl("interact", ax - 2, ay - 1, az)
ctl("wait", 120)
door = state(ax - 2, ay - 1, az)
check("a click opens the door", "open=true" in door, door)
opened = rate(400)
print(f" dose over 20 s: {sealed:.1f} sealed, {opened:.1f} with the door open")
check("the sealed room keeps most of the radiation out", opened > 5 and sealed < opened / 5, f"{sealed:.1f} / {opened:.1f}")
chat("gamemode spectator")
chat(f"tp @s {ax - 12} {ay + 6} {az + 10} -135 20")
ctl("wait", 40)
ctl("screenshot", os.path.join(SHOTS, "bunker-after.png"))
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
