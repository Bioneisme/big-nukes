#!/usr/bin/env python3
"""Runs the in-game checks one after another in a test instance, on the build that instance runs, and
sums them up. Each check's output goes to target/all-tests/NAME.log.

    python tools/run_all.py INSTANCE [NAME ...]

INSTANCE is a launcher instance id (bn-auto-test); NAMEs pick checks (silo bunker ...), all by default.
Each check starts the game through the launcher's Play (tools/play.py), with the launcher's real data folder in
HORIZON_LAUNCHER_HOME (default D:/Games/horizon-launcher).
"""
import json
import os
import subprocess
import sys
import tempfile
import time

CHECKS = [
    "content", "ore", "machine", "assembly", "bomb", "chain", "remote", "wave", "effects", "sky",
    "rad", "decon", "guide", "icons", "silo", "bomber", "sunbreaker", "sunbreaker_remote", "bunker", "warheads", "consequences", "events",
]
TOOLS = os.path.dirname(os.path.abspath(__file__))
LOGS = os.path.join(TOOLS, "..", "target", "all-tests")


def simulate(game, chunks=16):
    """The checks wait for fuses and waves from up to 150 blocks off: the instance must tick that far (a new one
    simulates 6 chunks, 96 blocks)."""
    path = os.path.join(game, "options.txt")
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except OSError:
        return
    lines = [l for l in lines if not l.startswith("simulationDistance:")] + [f"simulationDistance:{chunks}"]
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


def version(instance):
    """The build the instance runs: horizon-ctl must come from its folder, or it starts an older native half."""
    with open(os.path.join(os.environ["APPDATA"], "horizon-launcher", "instances.json"), encoding="utf-8") as f:
        return json.load(f)["instances"][instance]["version"]


def main():
    instance, names = sys.argv[1], sys.argv[2:] or CHECKS
    unknown = [n for n in names if n not in CHECKS]
    if unknown:
        sys.exit(f"unknown checks: {unknown}; known: {CHECKS}")
    game = os.path.join(os.environ["APPDATA"], "horizon", "instances", instance)
    build = version(instance)
    env = {k: v for k, v in os.environ.items() if k != "HORIZON_HOME"}
    env["BN_INSTANCE"] = instance
    env.setdefault("HORIZON_LAUNCHER_HOME", "D:/Games/horizon-launcher")
    # A control socket of the instance's own, so a game the player has open never takes the checks' commands.
    env["HORIZON_CONTROL"] = os.path.join(tempfile.gettempdir(), f"horizon-control-{instance}.json")
    os.makedirs(LOGS, exist_ok=True)
    simulate(game)
    print(f"{instance} on {build}", flush=True)
    results = []
    for name in names:
        started = time.time()
        log = os.path.join(LOGS, f"{name}.log")
        with open(log, "w", encoding="utf-8") as out:
            try:
                subprocess.run([sys.executable, os.path.join(TOOLS, f"{name}_test.py"), game, build],
                               stdout=out, stderr=subprocess.STDOUT, env=env, timeout=1800)
            except subprocess.TimeoutExpired:
                out.write("\n== failed: timed out\n")
        with open(log, encoding="utf-8") as f:
            lines = f.read().splitlines()
        last = next((l for l in reversed(lines) if l.startswith("== ")), "== no verdict")
        fails = [l for l in lines if l.startswith("FAIL")]
        ok = last == "== all passed"
        results.append((name, ok, fails or ([] if ok else [last])))
        print(f"{'ok  ' if ok else 'FAIL'} {name} ({time.time() - started:.0f} s)", flush=True)
        for line in fails:
            print(f"     {line}", flush=True)
    bad = [r for r in results if not r[1]]
    print(f"== {len(results) - len(bad)}/{len(results)} passed" + (f"; failed: {[r[0] for r in bad]}" if bad else ""), flush=True)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
