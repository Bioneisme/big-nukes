"""Starts the game for a test through the launcher's Play: a build with native mods takes the launcher's cache
leases, so `horizon-ctl launch` can't start it. The instance comes from BN_INSTANCE (run_all.py sets it), the build
from the test's own VERSION argument, the launcher from HORIZON_LAUNCHER (default D:/Games/HorizonLauncher/
horizon-launcher.exe) with its real data folder in HORIZON_LAUNCHER_HOME. The game has no offscreen mode here: its
window shows while a test runs.
"""
import os
import subprocess
import sys
import time

LAUNCHER = os.environ.get("HORIZON_LAUNCHER", "D:/Games/HorizonLauncher/horizon-launcher.exe")


def mod_log():
    """The launcher's output of the game the last launch started: the game's standard error, where the mod's own
    log lines go (its `log` is not the game's: see big_nukes/src/logging.rs)."""
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target",
                        f"play-{os.environ['BN_INSTANCE']}.log")


def _position(ctl):
    player = ctl("state", timeout=60).get("player") or {}
    return player.get("pos") or player.get("position")


def launch(ctl, world, seed="12345"):
    """Opens (or creates, in creative) `world` and returns once the player stands in it."""
    # A game still closing from a test's quit: wait until its socket is gone.
    for _ in range(30):
        if "error" in ctl("state", timeout=30):
            break
        time.sleep(2)
    instance = os.environ["BN_INSTANCE"]
    version = sys.argv[2]
    args = [LAUNCHER, "play", "--instance", instance, "--version", version, "--world", world, "--creative",
            "--arg", "--control", "--arg", "--seed", "--arg", str(seed)]
    socket = os.environ.get("HORIZON_CONTROL")
    if socket:
        args += ["--arg", "--control-socket", "--arg", socket]
    env = {k: v for k, v in os.environ.items() if k != "HORIZON_HOME"}
    log = mod_log()
    os.makedirs(os.path.dirname(log), exist_ok=True)
    subprocess.Popen(args, env=env, stdout=open(log, "w"), stderr=subprocess.STDOUT)
    for _ in range(120):
        if _position(ctl):
            break
        time.sleep(3)
    return ctl("wait-ready", 60, timeout=120)
