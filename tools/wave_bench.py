#!/usr/bin/env python3
"""Measures the frames and the server's ticks while a wave of `/bignukes blast` runs.

    python tools/wave_bench.py GAME_FOLDER VERSION [RADIUS ...]

For each radius (default 50 250 1000): a new world `bn-bench-<radius>` (seed 12345,
render distance 12), ten seconds of frames without a blast as the base, then the
blast at the player's feet and the frames until the loaded part of the wave is done
(the mod logs it), with `tick query` read meanwhile. Prints one line a radius.
"""
import json
import os
import shutil
import subprocess
import sys
import time

GAME, VERSION = sys.argv[1:3]
RADII = [int(r) for r in sys.argv[3:]] or [50, 250, 1000]
CTL = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", VERSION, "horizon-ctl.exe")
os.environ["HORIZON_HOME"] = GAME
os.environ["MSYS_NO_PATHCONV"] = "1"
sys.stdout.reconfigure(encoding="utf-8")
OPTIONS = os.path.join(GAME, "options.txt")
LOG = os.path.join(GAME, "logs", "latest.log")


def ctl(*args, timeout=300):
    out = subprocess.run([CTL, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {"raw": (out.stdout + out.stderr)[:400]}


def chat(line):
    return ctl("command", line).get("chat", [])


def log_lines():
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            return f.read().splitlines()
    except OSError:
        return []


def bench(radius):
    world = f"bn-bench-{radius}"
    shutil.rmtree(os.path.join(GAME, "saves", world), ignore_errors=True)
    ctl("launch", "--world", world, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    ctl("wait", 200)
    chat("gamerule advance_time false")
    chat("gamerule advance_weather false")
    base = ctl("bench", 10)
    seen = len(log_lines())
    ctl("stats", "reset")
    started = time.time()
    chat(f"bignukes blast {radius}")
    ticks = []
    done = None
    while time.time() - started < 180:
        ticks.append(chat("tick query"))
        lines = log_lines()[seen:]
        if any("the loaded part of every wave is done" in l for l in lines):
            done = time.time() - started
            break
        time.sleep(1)
    during = ctl("stats")
    slow = [l for l in log_lines()[seen:] if "slow" in l.lower() or "refused" in l]
    ctl("quit")
    time.sleep(8)
    return {"radius": radius, "base": base, "during": during, "done_s": done, "ticks": ticks[-3:], "slow": slow[:10]}


def main():
    with open(OPTIONS, encoding="utf-8") as f:
        saved = f.read()
    lines = [l for l in saved.splitlines() if not l.startswith(("renderDistance:", "simulationDistance:"))]
    with open(OPTIONS, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines + ["renderDistance:12", "simulationDistance:12"]) + "\n")
    try:
        for r in RADII:
            print(json.dumps(bench(r), ensure_ascii=False), flush=True)
    finally:
        with open(OPTIONS, "w", encoding="utf-8", newline="\n") as f:
            f.write(saved)


if __name__ == "__main__":
    main()
