#!/usr/bin/env python3
"""Synthesizes Angler's sound effects into sfx/*.pcm (mono s16le, 16 kHz).

Standard library only, seeded, so the clips are reproducible:

    python3 sfx.py
"""

import math
import os
import random
import struct

RATE = 16_000
random.seed(7)


def silence(secs):
    return [0.0] * int(secs * RATE)


def tone(secs, freq, harmonics=(1.0,), attack=0.003, decay=None, vibrato=(0.0, 0.0)):
    """A note whose pitch is `freq` (Hz, or a function of time) built from
    `harmonics` (amplitude of the 1st, 2nd, ... partial), with an exponential
    `decay` time constant or a flat sustain when None. `vibrato` is (rate in
    Hz, depth as a fraction of the pitch)."""
    out, phase = [], 0.0
    rate, depth = vibrato
    for i in range(int(secs * RATE)):
        t = i / RATE
        f = freq(t) if callable(freq) else freq
        f *= 1.0 + depth * math.sin(2 * math.pi * rate * t)
        phase += 2 * math.pi * f / RATE
        wave = sum(a * math.sin(n * phase) for n, a in enumerate(harmonics, 1))
        env = min(1.0, t / attack) if attack else 1.0
        if decay:
            env *= math.exp(-t / decay)
        out.append(wave * env)
    return out


def noise(secs, cutoff, decay=None, attack=0.0, high_pass=False):
    """White noise through a one-pole low-pass at `cutoff` (Hz, or a function
    of time); `high_pass` keeps what the low-pass removes instead."""
    out, low = [], 0.0
    for i in range(int(secs * RATE)):
        t = i / RATE
        c = cutoff(t) if callable(cutoff) else cutoff
        k = 1.0 - math.exp(-2 * math.pi * c / RATE)
        white = random.uniform(-1.0, 1.0)
        low += k * (white - low)
        sample = white - low if high_pass else low
        env = min(1.0, t / attack) if attack else 1.0
        if decay:
            env *= math.exp(-t / decay)
        out.append(sample * env)
    return out


def mix(*parts):
    """Each part is (offset_secs, samples, gain)."""
    length = max(int(at * RATE) + len(s) for at, s, _ in parts)
    out = [0.0] * length
    for at, samples, gain in parts:
        start = int(at * RATE)
        for i, x in enumerate(samples):
            out[start + i] += x * gain
    return out


def glide(start, end, secs):
    """A pitch that slides exponentially from `start` to `end` Hz over `secs`."""
    return lambda t: start * (end / start) ** min(1.0, t / secs)


def shape(samples, envelope):
    """Scales `samples` by `envelope(x)`, with x running 0 to 1 over the clip."""
    n = len(samples)
    return [x * envelope(i / n) for i, x in enumerate(samples)]


def write(name, samples, peak):
    """Fades out the last 2 ms, normalizes to `peak` (a fraction of full scale)
    and writes sfx/<name>.pcm."""
    fade = int(0.002 * RATE)
    for i in range(min(fade, len(samples))):
        samples[-1 - i] *= i / fade
    top = max(abs(x) for x in samples) or 1.0
    pcm = b"".join(struct.pack("<h", round(x / top * peak * 32767)) for x in samples)
    os.makedirs("sfx", exist_ok=True)
    with open(os.path.join("sfx", name + ".pcm"), "wb") as f:
        f.write(pcm)
    print(f"sfx/{name}.pcm  {len(samples) / RATE:.2f} s  {len(pcm)} B")


SOFT_SQUARE = (1.0, 0.0, 0.33, 0.0, 0.2)
SOFT_SAW = (1.0, 0.5, 0.33, 0.25, 0.2, 0.16)
BELL = (1.0, 0.0, 0.25, 0.0, 0.08)


def whoosh():
    sweep = lambda t: 500 + 2600 * math.sin(math.pi * min(1.0, t / 0.32))
    air = noise(0.32, sweep, attack=0.06)
    return shape(air, lambda x: (1 - x) ** 1.5)


def cast():
    return whoosh()


def perfect():
    return mix(
        (0.0, whoosh(), 0.6),
        (0.0, tone(0.45, 1319, BELL, decay=0.12), 0.5),
        (0.08, tone(0.4, 1976, BELL, decay=0.12), 0.45),
        (0.16, tone(0.3, 2637, BELL, decay=0.08), 0.3),
    )


def plop():
    return mix(
        (0.0, tone(0.18, glide(1100, 250, 0.07), decay=0.04), 1.0),
        (0.015, noise(0.2, 2500, decay=0.05, attack=0.005), 0.35),
    )


def thud():
    return mix(
        (0.0, tone(0.16, glide(150, 70, 0.1), decay=0.05), 1.0),
        (0.0, noise(0.03, 800, decay=0.008), 0.5),
    )


def bite():
    chirp = lambda: tone(0.09, glide(480, 950, 0.07), SOFT_SQUARE, decay=0.05)
    return mix((0.0, chirp(), 1.0), (0.13, chirp(), 1.0))


def reel():
    return mix(
        (0.0, noise(0.006, 3000, high_pass=True, decay=0.002), 0.6),
        (0.0, tone(0.035, 2400, decay=0.007), 0.7),
    )


def catch():
    notes = [523.25, 659.25, 783.99, 1046.5]
    parts = [(i * 0.085, tone(0.14, f, SOFT_SQUARE, decay=0.09), 0.8) for i, f in enumerate(notes)]
    held = 0.36
    parts.append((0.34, tone(held, 1046.5, SOFT_SQUARE, decay=0.22, vibrato=(6, 0.006)), 0.8))
    parts.append((0.34, tone(held, 783.99, SOFT_SQUARE, decay=0.22, vibrato=(6, 0.006)), 0.5))
    return mix(*parts)


def escape():
    wah = lambda x: 0.35 + 0.65 * math.sin(math.pi * min(1.0, x * 1.15))
    parts = []
    for i, f in enumerate([392.0, 369.99, 349.23]):
        parts.append((i * 0.21, shape(tone(0.2, f, SOFT_SAW), wah), 0.7))
    last = tone(0.45, glide(329.63, 290.0, 0.45), SOFT_SAW, decay=0.35, vibrato=(5, 0.012))
    parts.append((0.63, shape(last, wah), 0.7))
    return mix(*parts)


def coin():
    return mix(
        (0.0, tone(0.08, 987.77, SOFT_SQUARE), 0.8),
        (0.07, tone(0.35, 1318.5, SOFT_SQUARE, decay=0.12), 0.8),
    )


def click():
    return mix(
        (0.0, tone(0.018, 1800, decay=0.004), 1.0),
        (0.0, noise(0.004, 4000, high_pass=True), 0.3),
    )


if __name__ == "__main__":
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    write("cast", cast(), 0.45)
    write("perfect", perfect(), 0.6)
    write("plop", plop(), 0.6)
    write("thud", thud(), 0.65)
    write("bite", bite(), 0.55)
    write("reel", reel(), 0.35)
    write("catch", catch(), 0.55)
    write("escape", escape(), 0.5)
    write("coin", coin(), 0.5)
    write("click", click(), 0.35)
