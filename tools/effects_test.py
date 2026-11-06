#!/usr/bin/env python3
"""Drives the game offscreen with horizon-ctl and checks how a blast feels: the flash, the news
the window hears, the debris and the damage.

    python tools/effects_test.py GAME_FOLDER VERSION [SCENARIO ...]

Scenarios: near far. Without any, both.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

from PIL import Image, ImageStat

GAME, VERSION = sys.argv[1:3]
ONLY = set(sys.argv[3:])
CTL = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", VERSION, "horizon-ctl.exe")
os.environ["HORIZON_HOME"] = GAME
os.environ["MSYS_NO_PATHCONV"] = "1"
sys.stdout.reconfigure(encoding="utf-8")
SHOTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "shots")
os.makedirs(SHOTS, exist_ok=True)
LOG = os.path.join(GAME, "logs", "latest.log")
failed = []


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def chat(line):
    return ctl("command", line).get("chat", [])


def check(name, ok, detail=""):
    print(("ok  " if ok else "FAIL"), name, flush=True)
    if not ok:
        failed.append(f"{name}: {detail}")


def here():
    for _ in range(20):
        player = ctl("state").get("player") or {}
        pos = player.get("pos") or player.get("position") or player.get("feet")
        if pos:
            break
        ctl("wait", 10)
    if isinstance(pos, dict):
        pos = [pos["x"], pos["y"], pos["z"]]
    return [int(v // 1) for v in pos]


def brightness(name):
    path = os.path.join(SHOTS, f"{name}.png")
    ctl("screenshot", path)
    try:
        return ImageStat.Stat(Image.open(path).convert("L")).mean[0]
    except OSError as e:
        return -1.0


def log_since(n):
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            return [l for l in f.read().splitlines()[n:] if "[big_nukes]" in l]
    except OSError:
        return []


def log_len():
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            return len(f.read().splitlines())
    except OSError:
        return 0


def chat_log_since(n):
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            return [l for l in f.read().splitlines()[n:] if "[CHAT]" in l]
    except OSError:
        return []


def launch(world):
    shutil.rmtree(os.path.join(GAME, "saves", world), ignore_errors=True)
    ctl("launch", "--world", world, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)
    chat("gamerule advance_time false")
    chat("gamerule advance_weather false")
    chat("time set noon")
    ctl("key", "f1", "tap")
    ctl("wait", 20)


def flash_peak(name):
    """The brightest of three frames taken at once: the flash fades within a second."""
    return max(brightness(f"{name}-{i}") for i in range(3))


def passed(lines):
    return any("passed" in str(l).lower() for l in lines)


def s_near():
    launch("bn-effects")
    cx, cy, cz = here()
    for tag, dx in (("bn_core", 12), ("bn_edge", 56)):
        chat(f"fill {cx + dx - 1} {cy + 2} {cz - 1} {cx + dx + 1} {cy + 5} {cz + 1} minecraft:air")
        chat(f'summon minecraft:cow {cx + dx} {cy + 3} {cz} {{NoAI:1b,NoGravity:1b,Tags:["{tag}"]}}')
    ctl("wait", 10)
    base = brightness("near-base")
    seen = log_len()
    chat(f"bignukes blast 60 {cx} {cy} {cz}")
    lit = flash_peak("near-flash")
    print(f" brightness: {base:.0f} -> {lit:.0f}")
    check("the flash whitens the screen", lit > base + 60, f"{base:.0f} -> {lit:.0f}")
    ctl("wait", 4)
    check("debris flies", passed(chat("execute if entity @e[type=minecraft:falling_block]")), "no falling block")
    ctl("wait", 100)
    after = brightness("near-after")
    ctl("wait", 20)
    later = brightness("near-later")
    check("the flash is gone after a few seconds", abs(after - later) < 15, f"{after:.0f} then {later:.0f}")
    lines = log_since(seen)
    check("the window hears of the blast", any("blast 1 heard" in l for l in lines), str(lines)[:300])
    check("no edit is refused", not [l for l in lines if "refused" in l], str(lines)[:400])
    errors = [l for l in chat_log_since(seen) if any(w in l for w in ("Can't find", "Unknown", "Incorrect", "Expected", "Invalid"))]
    check("no command of the blast is refused", not errors, str(errors)[:400])
    check("a cow in the core dies", not passed(chat("execute if entity @e[tag=bn_core]")))
    health = " ".join(chat("data get entity @e[tag=bn_edge,limit=1] Health"))
    m = re.search(r"([0-9.]+)f", health)
    check("a cow at the edge lives, hurt", m is not None and 0 < float(m.group(1)) < 10, health)
    ctl("quit")
    time.sleep(8)


def s_far():
    launch("bn-effects-far")
    cx, cy, cz = here()
    base = brightness("far-base")
    chat(f"bignukes blast 200 {cx + 600} {cy} {cz}")
    lit = flash_peak("far-flash")
    print(f" brightness: {base:.0f} -> {lit:.0f}")
    check("a far flash lightens the screen, not to white", base + 10 < lit < 235, f"{base:.0f} -> {lit:.0f}")
    ctl("quit")
    time.sleep(8)


SCENARIOS = {"near": s_near, "far": s_far}


def main():
    for name, run in SCENARIOS.items():
        if not ONLY or name in ONLY:
            print("=====", name, flush=True)
            try:
                run()
            except Exception as e:
                check(f"{name} ran", False, repr(e)[:300])
                ctl("quit")
    print(f"== failed: {failed}" if failed else "== all passed", flush=True)


if __name__ == "__main__":
    main()
