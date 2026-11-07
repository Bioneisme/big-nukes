#!/usr/bin/env python3
"""Makes the mod's sounds from noise, so every one of them is original.

    python tools/make_sounds.py [name ...]

Writes content/assets/big_nukes/sounds/*.ogg (mono, 44.1 kHz, Vorbis) with numpy and ffmpeg, or only the
named ones. The same seed gives the same samples; Vorbis gives each file a new stream serial, so a sound
that is already there is best left out.
"""
import os
import subprocess
import sys
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


def loud(x, drive, peak=0.89):
    """A soft clip (tanh): the peaks round off and the body comes up, so a blast is as loud as
    vanilla's explosion on small speakers too, not only in the bass."""
    return normalize(np.tanh(drive * normalize(x, 1.0)), peak)


def near():
    n = seconds(7.0)
    noise = rng.standard_normal(n)
    crack = lowpass(noise, 7000) * envelope(n, 0.002, 0.18)
    boom = sweep_lowpass(noise, 2600, 80) * envelope(n, 0.01, 1.5) * 3.0
    sub = np.sin(2 * np.pi * np.cumsum(np.geomspace(55, 28, n)) / RATE) * envelope(n, 0.02, 1.2) * 0.5
    tail = lowpass(rng.standard_normal(n), 600) * envelope(n, 0.5, 2.6) * rumble(n) * 3.0
    return loud(crack * 1.4 + boom + sub + tail, 4.0)


def far():
    n = seconds(9.0)
    thud = sweep_lowpass(rng.standard_normal(n), 1800, 200) * envelope(n, 0.01, 0.5)
    body = sweep_lowpass(rng.standard_normal(n), 900, 120) * envelope(n, 0.25, 2.6) * rumble(n, 0.7, 1.8) * 2.0
    sub = np.sin(2 * np.pi * np.cumsum(np.geomspace(40, 24, n)) / RATE) * envelope(n, 0.3, 2.0) * 0.1
    return loud(thud + body + sub, 3.5, 0.85)


def wave_roar():
    n = seconds(4.5)
    wind = sweep_lowpass(rng.standard_normal(n), 4000, 500) * envelope(n, 0.15, 1.3) * rumble(n, 0.4, 7.0)
    grit = lowpass(rng.standard_normal(n) * (rng.random(n) < 0.004), 3000) * envelope(n, 0.05, 1.0) * 40
    return loud(wind + grit, 3.0, 0.85)


def geiger_click():
    """A Geiger counter's click: a dry tick of a few milliseconds, with its own seed."""
    own = np.random.default_rng(26_4)
    n = seconds(0.03)
    tick = own.standard_normal(n) * envelope(n, 0.0002, 0.0015)
    body = np.sin(2 * np.pi * 3200 * np.arange(n) / RATE) * envelope(n, 0.0001, 0.001) * 0.6
    return normalize(tick + body, 0.7)


def missile_launch():
    """A rocket motor's roar, 6 s, with its own seed: a deep rumble and a mid-range tearing crackle that
    swell as it lights, hold, and fade as it climbs away."""
    own = np.random.default_rng(26_7)
    n = seconds(6.0)
    t = np.arange(n) / RATE
    swell = np.clip(t / 0.8, 0, 1) ** 1.5 * np.clip((6.0 - t) / 2.5, 0, 1)
    rumble_ = lowpass(own.standard_normal(n), 220) * 6.0
    body = lowpass(own.standard_normal(n), 900) * 2.5
    crackle = own.standard_normal(n) * (own.random(n) < 0.02) * 6.0
    crackle = lowpass(crackle, 3500) - lowpass(crackle, 700)
    sub = np.sin(2 * np.pi * np.cumsum(48 + 6 * np.sin(2 * np.pi * 0.7 * t)) / RATE) * 0.6
    flutter = 1 + 0.15 * np.sin(2 * np.pi * 11 * t + own.uniform(0, 6.3))
    return loud((rumble_ + body + crackle * 2.0 + sub) * flutter * swell, 2.8, 0.88)


def bomber_flyby():
    """Propellers going over, 5 s, with their own seed: a buzz at the blades' rate with its harmonics and an
    engine's noise, rising, passing and falling in pitch as it goes."""
    own = np.random.default_rng(26_8)
    n = seconds(5.0)
    t = np.arange(n) / RATE
    # It passes overhead at 2.2 s: loudest there, a little higher before and lower after.
    near = 1 / (1 + ((t - 2.2) / 0.9) ** 2)
    pitch = 1 + 0.08 * np.tanh((2.2 - t) / 0.6)
    phase = 2 * np.pi * np.cumsum(68 * pitch) / RATE
    buzz = sum(np.sign(np.sin(k * phase)) / k ** 1.3 for k in (1, 2, 3, 4))
    buzz = lowpass(buzz, 1500)
    engine = lowpass(own.standard_normal(n), 500) * (1 + 0.3 * np.sin(phase / 2))
    return loud((buzz * 1.5 + engine * 2.0) * near, 2.2, 0.85)


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


SOUNDS = {
    "blast_near": near,
    "blast_far": far,
    "wave_roar": wave_roar,
    "geiger_click": geiger_click,
    "missile_launch": missile_launch,
    "bomber_flyby": bomber_flyby,
}

if __name__ == "__main__":
    wanted = sys.argv[1:] or list(SOUNDS)
    # The noise is drawn in this order whichever are written, so each sound stays the same.
    made = {name: make() for name, make in SOUNDS.items()}
    for name in wanted:
        write(name, made[name])
