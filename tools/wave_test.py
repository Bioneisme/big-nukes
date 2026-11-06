#!/usr/bin/env python3
"""Drives the game offscreen with horizon-ctl and checks the wave of `/bignukes blast`.

    python tools/wave_test.py GAME_FOLDER VERSION [SCENARIO ...]

GAME_FOLDER is a test instance's folder, VERSION the installed version it plays
(native-...). Scenarios: small negative deferred restart two. Without any, all of them.
Each scenario plays a new world `bn-<scenario>`; markers (floating blocks) are placed
where the zones are, so the checks don't hang on the terrain.
"""
import json
import os
import shutil
import subprocess
import sys
import time

GAME, VERSION = sys.argv[1:3]
ONLY = set(sys.argv[3:])
CTL = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", VERSION, "horizon-ctl.exe")
os.environ["HORIZON_HOME"] = GAME
os.environ["MSYS_NO_PATHCONV"] = "1"
sys.stdout.reconfigure(encoding="utf-8")
SHOTS = os.path.join(GAME, "bn-shots")
try:
    os.makedirs(SHOTS, exist_ok=True)
except OSError:  # the Store's Python keeps its own copy of new folders in AppData
    pass
OPTIONS = os.path.join(GAME, "options.txt")
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


def block(x, y, z):
    r = ctl("block", x, y, z)
    return str(r.get("block", r))


def launch(world):
    r = ctl("launch", "--world", world, "--seed", "12345", "--creative", "--wait", "--offscreen", timeout=280)
    print(" launch:", str(r)[:100], flush=True)
    ctl("wait", 40)
    chat("gamerule advance_time false")
    chat("gamerule advance_weather false")


def quit_game():
    ctl("quit")
    time.sleep(8)


def here():
    state = ctl("state")
    player = state.get("player", {})
    pos = player.get("pos") or player.get("position") or player.get("feet")
    if not pos:
        raise RuntimeError(f"no position in state: {str(state)[:300]}")
    if isinstance(pos, dict):
        pos = [pos["x"], pos["y"], pos["z"]]
    return [int(v // 1) for v in pos]


def mark(x, y, z, what):
    chat(f"setblock {x} {y} {z} {what}")


def refused():
    try:
        with open(LOG, encoding="utf-8", errors="replace") as f:
            return [line.strip() for line in f if "[big_nukes]" in line and "refused" in line]
    except OSError:
        return ["no log"]


def waves_file(world):
    path = os.path.join(GAME, "saves", world, "horizon-big-nukes", "waves.json")
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError) as e:
        return {"error": repr(e)}


LEAVES = "minecraft:oak_leaves[persistent=true]"


def s_small():
    launch("bn-small")
    cx, cy, cz = here()
    mark(cx + 25, cy + 10, cz, LEAVES)
    mark(cx + 25, cy + 20, cz, "minecraft:grass_block")
    mark(cx + 43, cy + 10, cz, "minecraft:glass")
    mark(cx + 70, cy + 10, cz, "minecraft:glass")
    print(" blast:", chat("bignukes blast 50"))
    ctl("wait", 80)
    ctl("screenshot", os.path.join(SHOTS, "small.png"))
    check("the center is a crater", "air" in block(cx, cy - 5, cz), block(cx, cy - 5, cz))
    check("bedrock stays under the center", "bedrock" in block(cx, -64, cz), block(cx, -64, cz))
    check("the middle loses its leaves", "air" in block(cx + 25, cy + 10, cz), block(cx + 25, cy + 10, cz))
    check("the middle's grass is scorched", "coarse_dirt" in block(cx + 25, cy + 20, cz), block(cx + 25, cy + 20, cz))
    check("the outer ring loses its glass", "air" in block(cx + 43, cy + 10, cz), block(cx + 43, cy + 10, cz))
    check("beyond the border nothing changes", "glass" in block(cx + 70, cy + 10, cz), block(cx + 70, cy + 10, cz))
    check("no edit was refused", not refused(), str(refused())[:300])
    quit_game()


def s_negative():
    launch("bn-negative")
    _, cy, _ = here()
    mark(4, cy - 1, -1, "minecraft:stone")
    mark(-6, cy - 1, -1, "minecraft:stone")
    print(" blast:", chat(f"bignukes blast 24 -1 {cy} -1"))
    ctl("wait", 60)
    check("the crater reaches both sides of -1 alike",
          "air" in block(4, cy - 1, -1) and "air" in block(-6, cy - 1, -1),
          f"{block(4, cy - 1, -1)} {block(-6, cy - 1, -1)}")
    quit_game()


def s_deferred():
    with open(OPTIONS, encoding="utf-8") as f:
        saved = f.read()
    lines = [l for l in saved.splitlines() if not l.startswith(("renderDistance:", "simulationDistance:"))]
    with open(OPTIONS, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines + ["renderDistance:4", "simulationDistance:5"]) + "\n")
    try:
        launch("bn-deferred")
        cx, cy, cz = here()
        far = cx + 200
        chat(f"tp @s {far} {cy + 80} {cz}")
        ctl("wait", 60)
        mark(far, cy + 60, cz, LEAVES)
        mark(far, cy + 70, cz, "minecraft:grass_block")
        chat(f"tp @s {cx} {cy + 2} {cz}")
        ctl("wait", 200)
        print(" far column before the blast (unloaded?):", block(far, cy + 60, cz))
        print(" blast:", chat(f"bignukes blast 400 {cx} {cy} {cz}"))
        ctl("wait", 300)
        chat(f"tp @s {far} {cy + 90} {cz}")
        ctl("wait", 100)
        ctl("screenshot", os.path.join(SHOTS, "deferred.png"))
        check("a chunk loaded after the wave loses its leaves", "air" in block(far, cy + 60, cz), block(far, cy + 60, cz))
        check("a chunk loaded after the wave is scorched", "coarse_dirt" in block(far, cy + 70, cz), block(far, cy + 70, cz))
        check("no edit was refused", not refused(), str(refused())[:300])
        quit_game()
    finally:
        with open(OPTIONS, "w", encoding="utf-8", newline="\n") as f:
            f.write(saved)


def s_restart():
    launch("bn-restart")
    cx, cy, cz = here()
    mark(cx + 75, cy + 10, cz, LEAVES)
    mark(cx + 75, cy + 20, cz, "minecraft:grass_block")
    print(" blast:", chat("bignukes blast 150"))
    ctl("wait", 20)
    quit_game()
    print(" mid-wave:", str(waves_file("bn-restart"))[:200])
    launch("bn-restart")
    ctl("wait", 300)
    check("after a restart the wave goes on", "air" in block(cx + 75, cy + 10, cz), block(cx + 75, cy + 10, cz))
    check("after a restart the middle is scorched", "coarse_dirt" in block(cx + 75, cy + 20, cz), block(cx + 75, cy + 20, cz))
    quit_game()
    check("an ended wave leaves nothing to do", waves_file("bn-restart").get("waves", {}).get("waves") == [], str(waves_file("bn-restart"))[:300])


def s_two():
    launch("bn-two")
    cx, cy, cz = here()
    mark(cx - 40, cy - 3, cz, "minecraft:stone")
    mark(cx + 40, cy - 3, cz, "minecraft:stone")
    print(" blasts:", chat(f"bignukes blast 60 {cx - 40} {cy} {cz}"), chat(f"bignukes blast 60 {cx + 40} {cy} {cz}"))
    ctl("wait", 100)
    check("both craters are there",
          "air" in block(cx - 40, cy - 3, cz) and "air" in block(cx + 40, cy - 3, cz),
          f"{block(cx - 40, cy - 3, cz)} {block(cx + 40, cy - 3, cz)}")
    quit_game()


SCENARIOS = {"small": s_small, "negative": s_negative, "deferred": s_deferred, "restart": s_restart, "two": s_two}


def main():
    for name, run in SCENARIOS.items():
        if not ONLY or name in ONLY:
            print("=====", name, flush=True)
            shutil.rmtree(os.path.join(GAME, "saves", f"bn-{name}"), ignore_errors=True)
            try:
                run()
            except Exception as e:  # keep going: the other scenarios still tell something
                check(f"{name} ran", False, repr(e)[:300])
                ctl("quit")
    print(f"== failed: {failed}" if failed else "== all passed", flush=True)


if __name__ == "__main__":
    main()
