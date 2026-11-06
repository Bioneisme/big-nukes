#!/usr/bin/env python3
"""Drives the game offscreen with horizon-ctl and checks the mushroom cloud drawn over the world.

    python tools/sky_test.py GAME_FOLDER VERSION [SCENARIO ...]

Scenarios: near far behind inside. Without any, all of them. Each stands the player on a
glass platform 60 blocks up, looking east (towards the blast) or west (away from it), and
compares the middle of the frame before and after. Frames go to target/shots.
"""
import json
import os
import shutil
import subprocess
import sys
import time

from PIL import Image, ImageChops, ImageStat

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


def shot(name):
    path = os.path.join(SHOTS, f"sky-{name}.png")
    ctl("screenshot", path)
    return Image.open(path).convert("RGB")


def band(im):
    """The middle third of the frame, where a blast ahead stands."""
    w, h = im.size
    return im.crop((2 * w // 5, 3 * h // 10, 3 * w // 5, 3 * h // 5))


def change(a, b):
    """The percentage of the band's pixels that changed clearly (by more than 15 of 255)."""
    diff = ImageChops.difference(band(a), band(b)).convert("L")
    pixels = list(diff.getdata())
    return 100.0 * sum(1 for v in pixels if v > 15) / len(pixels)


def stand(cx, cy, cz, yaw):
    chat(f"tp @s {cx} {cy + 61} {cz} {yaw} -8")
    ctl("wait", 5)


def world(name):
    shutil.rmtree(os.path.join(GAME, "saves", name), ignore_errors=True)
    ctl("launch", "--world", name, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 60)
    for line in ("gamerule advance_time false", "gamerule advance_weather false", "time set noon", "weather clear"):
        chat(line)
    ctl("key", "f1", "tap")
    for _ in range(60):
        player = ctl("state").get("player") or {}
        pos = player.get("pos") or player.get("position")
        if pos:
            break
        ctl("wait", 10)
    cx, cy, cz = [int(v // 1) for v in pos]
    chat(f"fill {cx - 2} {cy + 60} {cz - 2} {cx + 2} {cy + 60} {cz + 2} minecraft:glass")
    return cx, cy, cz


def errors(since):
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            lines = f.read().splitlines()[since:]
    except OSError:
        return ["no log"]
    return [l for l in lines if "pipeline failed" in l or "wgpu" in l.lower() and "error" in l.lower() or "panicked" in l]


def log_len():
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            return len(f.read().splitlines())
    except OSError:
        return 0


def ahead(name, radius, dx, wait, least):
    cx, cy, cz = world(f"bn-sky-{name}")
    stand(cx, cy, cz, -90)
    base = shot(f"{name}-base")
    seen = log_len()
    chat(f"bignukes blast {radius} {cx + dx} {cy} {cz}")
    ctl("wait", wait)
    stand(cx, cy, cz, -90)
    after = shot(f"{name}-after")
    c = change(base, after)
    print(f" changed in the middle: {c:.2f}%")
    check(f"the cloud shows {dx} blocks away", c > least, f"{c:.1f}")
    check("the renderer reports no error", not errors(seen), str(errors(seen))[:300])
    ctl("quit")


def s_near():
    ahead("near", 150, 300, 400, 5.0)


def s_far():
    ahead("far", 250, 1500, 1000, 1.0)


def s_behind():
    cx, cy, cz = world("bn-sky-behind")
    stand(cx, cy, cz, 90)
    base = shot("behind-base")
    chat(f"bignukes blast 150 {cx + 300} {cy} {cz}")
    ctl("wait", 400)
    stand(cx, cy, cz, 90)
    after = shot("behind-after")
    c = change(base, after)
    print(f" changed in the middle: {c:.2f}%")
    check("nothing is drawn looking away", c < 0.3, f"{c:.1f}")
    ctl("quit")


def s_inside():
    cx, cy, cz = world("bn-sky-inside")
    seen = log_len()
    chat(f"bignukes blast 100 {cx} {cy - 50} {cz}")
    for pitch in (-90, 0, 90):
        ctl("wait", 100)
        chat(f"tp @s {cx} {cy + 61} {cz} 0 {pitch}")
        shot(f"inside-{pitch}")
    check("standing in the cloud brings no error", not errors(seen), str(errors(seen))[:300])
    ctl("quit")


SCENARIOS = {"near": s_near, "far": s_far, "behind": s_behind, "inside": s_inside}


def main():
    for name, run in SCENARIOS.items():
        if not ONLY or name in ONLY:
            print("=====", name, flush=True)
            try:
                run()
            except Exception as e:
                check(f"{name} ran", False, repr(e)[:300])
                ctl("quit")
            time.sleep(8)
    print(f"== failed: {failed}" if failed else "== all passed", flush=True)


if __name__ == "__main__":
    main()
