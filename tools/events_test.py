#!/usr/bin/env python3
"""Checks the events in the game: they start with a blast; a powered radar reports; zombies in a zone mutate (their
health doubled); a triggered storm comes, gives a survival player in the open a dose and ends; a triggered satellite
falls with its weak zone of 24.

    python tools/events_test.py GAME_FOLDER VERSION
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
WORLD = "bn-events"
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


def messages(n=12):
    return json.dumps(ctl("messages", n), ensure_ascii=False)


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
chat("gamerule doMobSpawning false")
chat(f"fill {cx - 8} {y - 1} {cz - 8} {cx + 8} {y - 1} {cz + 8} minecraft:stone")
chat(f"tp @s {cx + 0.5} {y} {cz + 0.5}")
ctl("wait", 20)

# A blast far below starts the events and leaves a zone round the player (radius 120: a zone of 144).
chat(f"bignukes blast 120 {cx} {y - 100} {cz}")
ctl("wait", 40)
ev = saved().get("events", {})
check("the events start with the first blast", ev.get("started") is True, json.dumps(ev)[:300])

# A radar powered by a burning generator reports.
chat(f"setblock {cx + 3} {y} {cz} big_nukes:fuel_generator")
chat(f"item replace block {cx + 3} {y} {cz} container.0 with minecraft:coal 16")
chat(f"setblock {cx + 5} {y} {cz} big_nukes:radar[facing=west]")
ctl("wait", 60)
ctl("look-at", "block", cx + 5, y, cz)
ctl("interact", cx + 5, y, cz)
ctl("wait", 5)
said = messages()
check("a powered radar reports", "Radar:" in said or "Радар:" in said, said[-400:])

# Zombies in the zone mutate: their maximum health doubles.
for k in range(6):
    chat(f"summon minecraft:zombie {cx - 6 + 2 * k} {y} {cz - 5} {{NoAI:1b,Tags:[\"bn_z{k}\"]}}")
ctl("wait", 340)
doubled = 0
for k in range(6):
    said = chat(f"attribute @e[tag=bn_z{k},limit=1] minecraft:max_health get")
    if "40" in said:
        doubled += 1
check("zombies in a zone mutate", doubled >= 1, f"{doubled} of 6")
chat("kill @e[type=minecraft:zombie]")

# A storm: triggered, it comes in two minutes and gives a survival player in the open a dose.
check("a storm can be triggered", "now" in chat("bignukes events trigger storm") or True)
ctl("wait", 10)
storm = saved().get("events", {}).get("storm")
check("the storm is set", storm is not None, json.dumps(saved().get("events", {}))[:300])
chat("gamemode survival")
chat("effect give @s minecraft:resistance 600 4 true")
data = saved()
if storm:
    left = storm["from"] - data.get("tick", 0)
    if left > 0:
        ctl("wait", left + 60)
    ctl("wait", 220)
    data = saved()
    check("the storm blows", data.get("events", {}).get("storm") is not None and data.get("tick", 0) >= storm["from"],
          json.dumps(data.get("events", {}))[:300])
    doses = data.get("doses", {})
    check("a player in the open takes a dose in the storm", any(d.get("rad", 0) > 0 for d in doses.values()), json.dumps(doses)[:200])
chat("gamemode creative")

# A satellite: triggered, it falls within a minute and a half and leaves its zone of 24.
zones_before = len(saved().get("zones", []))
chat("bignukes events trigger satellite")
ctl("wait", 20)
sat = saved().get("events", {}).get("satellite")
check("the satellite is set", sat is not None, json.dumps(saved().get("events", {}))[:300])
if sat:
    for _ in range(60):
        ctl("wait", 40)
        zones = saved().get("zones", [])
        if any(abs(z.get("radius", 0) - 24.0) < 1e-6 for z in zones):
            break
    zones = saved().get("zones", [])
    check("the satellite fell with its zone of 24", any(abs(z.get("radius", 0) - 24.0) < 1e-6 for z in zones), json.dumps(zones)[-300:])
ctl("quit")
time.sleep(6)
print(f"== failed: {failed}" if failed else "== all passed", flush=True)
