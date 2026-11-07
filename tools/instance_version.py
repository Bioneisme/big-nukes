#!/usr/bin/env python3
"""Prints the game build an instance runs: horizon-ctl must come from that build's folder, or it starts an
older native half than the one just built.

    python tools/instance_version.py bn-auto-test
"""
import json
import os
import sys

with open(os.path.join(os.environ["APPDATA"], "horizon-launcher", "instances.json"), encoding="utf-8") as f:
    print(json.load(f)["instances"][sys.argv[1]]["version"])
