#!/usr/bin/env python3
"""Checks the chat warning when one of the mod's two parts is missing, and that none comes when both are there.

    python tools/halves_test.py GAME_FOLDER NATIVE_VERSION NIGHTLY_VERSION

GAME_FOLDER is a test instance's folder. Its horizon-mods.txt is put back afterwards.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

GAME, NATIVE, NIGHTLY = sys.argv[1:4]
os.environ["HORIZON_HOME"] = GAME
os.environ["MSYS_NO_PATHCONV"] = "1"
sys.stdout.reconfigure(encoding="utf-8")
LIST = os.path.join(GAME, "horizon-mods.txt")
failed = []


def run(version, world, setting):
    ctl_path = os.path.join(os.environ["APPDATA"], "horizon-launcher", "versions", version, "horizon-ctl.exe")

    def ctl(*args):
        out = subprocess.run([ctl_path, *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=280)
        try:
            return json.loads(out.stdout)
        except ValueError:
            return {"raw": (out.stdout + out.stderr)[:300]}

    with open(LIST, "w", encoding="utf-8", newline="\n") as f:
        f.write(setting)
    # A fresh world: the warning comes once a player, so an old world would stay quiet.
    shutil.rmtree(os.path.join(GAME, "saves", world), ignore_errors=True)
    r = ctl("launch", "--world", world, "--seed", "12345", "--creative", "--wait", "--offscreen")
    print(" launch:", str(r)[:70])
    ctl("wait", 60)
    lines = ctl("chat-log", 20).get("chat", [])
    drawn = json.dumps(ctl("messages", 20), ensure_ascii=False)
    ctl("screenshot", os.path.join(GAME, "bn-shots", f"{world}.png"))
    ctl("quit")
    time.sleep(8)
    print(" chat:", lines)
    print(" drawn:", drawn[:900])
    return " ".join(map(str, lines)), drawn


def links(drawn):
    """The pages of the catalog that a click on the chat's lines opens."""
    return set(re.findall(r'"open_url"[^{}]*?https://playhorizon\.dev/mods/(\w+)', drawn))


def check(name, ok, detail):
    print(("ok  " if ok else "FAIL"), name)
    if not ok:
        failed.append(f"{name}: {detail}")


saved = LIST + ".saved"
shutil.copyfile(LIST, saved)
try:
    both, _ = run(NATIVE, "bn-both", "mod.big_nukes:true\nmod.big_nukes_native:true\n")
    check("both parts: no warning", "Big Nukes:" not in both, both)
    code, drawn = run(NATIVE, "bn-code", "mod.big_nukes:false\nmod.big_nukes_native:true\n")
    check("code alone: the assets mod is named", "assets mod is missing" in code, code)
    check("code alone: a click opens the assets' page", links(drawn) == {"big_nukes"}, drawn)
    assets, drawn = run(NIGHTLY, "bn-assets", "mod.big_nukes:true\nmod.big_nukes_native:false\n")
    check("assets alone: the native mod is named", "native mod is missing" in assets or "нативный" in assets, assets)
    check("assets alone: a click opens the native part's page", links(drawn) == {"big_nukes_native"}, drawn)
finally:
    shutil.move(saved, LIST)
print(f"== failed: {failed}" if failed else "== all passed")
