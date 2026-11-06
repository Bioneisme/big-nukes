#!/usr/bin/env python3
"""Makes the mod's sounds from noise, so every one of them is original.

    python tools/make_sounds.py

Writes content/assets/big_nukes/sounds/*.ogg (mono, 44.1 kHz, Vorbis) with numpy and ffmpeg.
The same seed gives the same files.
"""
import os
import subprocess
import tempfile
import wave

import numpy as np

RATE = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "content", "assets", "big_nukes", "sounds")
rng = np.random.default_rng(26_3)


def seconds(s):
    return int(s * RATE)


def lowpass(x, cutoff):
    """A one-pole low-pass, run twice for a softer slope."""
    a = np.exp(-2 * np.pi * cutoff / RATE)
    for _ in range(2):
        y = np.empty_like(x)
        acc = 0.0
        for i, v in enumerate(x):
            acc = (1 - a) * v + a * acc
            y[i] = acc
        x = y
    return x


def sweep_lowpass(x, start, end):
    """A low-pass whose cutoff glides from `start` to `end` Hz: the boom darkening as it rolls."""
    cut = np.geomspace(start, end, len(x))
    a = np.exp(-2 * np.pi * cut / RATE)
    y = np.empty_like(x)
    acc = 0.0
    for i, v in enumerate(x):
        acc = (1 - a[i]) * v + a[i] * acc
        y[i] = acc
    return y


def envelope(n, attack, decay):
    t = np.arange(n) / RATE
    rise = np.clip(t / max(attack, 1e-4), 0, 1)
    return rise * np.exp(-t / decay)


def rumble(n, depth=0.5, rate=3.0):
    """Slow, uneven swells, as a roll of thunder has."""
    t = np.arange(n) / RATE
    s = sum(np.sin(2 * np.pi * (rate * k) * t + rng.uniform(0, 6.3)) / k for k in (1, 1.7, 2.9))
    return 1 + depth * s / 2


def normalize(x, peak=0.89):
    return x / np.max(np.abs(x)) * peak


def near():
    n = seconds(7.0)
    noise = rng.standard_normal(n)
    crack = lowpass(noise, 6000) * envelope(n, 0.002, 0.08)
    boom = sweep_lowpass(noise, 900, 60) * envelope(n, 0.01, 1.6) * 3.0
    sub = np.sin(2 * np.pi * np.cumsum(np.geomspace(55, 28, n)) / RATE) * envelope(n, 0.02, 1.2) * 0.8
    tail = lowpass(rng.standard_normal(n), 220) * envelope(n, 0.6, 2.8) * rumble(n) * 4.0
    return normalize(crack * 0.6 + boom + sub + tail)


def far():
    n = seconds(9.0)
    body = lowpass(rng.standard_normal(n), 140) * envelope(n, 0.25, 2.6) * rumble(n, 0.7, 1.8)
    sub = np.sin(2 * np.pi * np.cumsum(np.geomspace(40, 24, n)) / RATE) * envelope(n, 0.3, 2.0) * 0.15
    return normalize(body + sub, 0.8)


def wave_roar():
    n = seconds(4.5)
    wind = sweep_lowpass(rng.standard_normal(n), 2500, 300) * envelope(n, 0.15, 1.3) * rumble(n, 0.4, 7.0)
    grit = lowpass(rng.standard_normal(n) * (rng.random(n) < 0.004), 3000) * envelope(n, 0.05, 1.0) * 40
    return normalize(wind + grit, 0.85)


def write(name, x):
    os.makedirs(OUT, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        wav = os.path.join(tmp, f"{name}.wav")
        with wave.open(wav, "wb") as f:
            f.setnchannels(1)
            f.setsampwidth(2)
            f.setframerate(RATE)
            f.writeframes((x * 32767).astype("<i2").tobytes())
        ogg = os.path.join(OUT, f"{name}.ogg")
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", wav, "-c:a", "libvorbis", "-q:a", "4", ogg], check=True)
        print(name, os.path.getsize(ogg), "bytes")


if __name__ == "__main__":
    write("blast_near", near())
    write("blast_far", far())
    write("wave_roar", wave_roar())
