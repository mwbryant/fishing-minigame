#!/usr/bin/env python3
"""Synthesizes Spider Roulette's sounds into sfx/*.pcm (mono s16le, 16 kHz),
with the helpers from Angler's generator. Standard library only, seeded:

    python3 sfx.py
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "angler_game", "assets"))
from sfx import BELL, SOFT_SAW, SOFT_SQUARE, glide, mix, noise, shape, tone, write  # noqa: E402


def crack():
    """An eggshell giving: a sharp crackle and a hollow tock."""
    return mix(
        (0.0, noise(0.03, 5000, high_pass=True, decay=0.008), 0.9),
        (0.012, noise(0.02, 2500, high_pass=True, decay=0.006), 0.6),
        (0.0, tone(0.08, glide(900, 500, 0.05), decay=0.02), 0.5),
    )


def tick():
    """The roulette pointer passing an egg: a short wooden ratchet click."""
    return mix(
        (0.0, noise(0.012, 3500, high_pass=True, decay=0.003), 0.8),
        (0.0, tone(0.03, 1300, decay=0.006), 0.6),
    )


def heartbeat():
    """Lub-dub, low and soft."""
    beat = lambda f: tone(0.12, glide(f, f * 0.6, 0.1), decay=0.04)
    return mix((0.0, beat(90), 1.0), (0.16, beat(75), 0.8))


def screech():
    """The spider: a rising hiss over a sour rising saw."""
    hiss = noise(0.7, lambda t: 1500 + 5000 * t, attack=0.01)
    hiss = shape(hiss, lambda x: (1 - x) ** 0.7)
    wail = tone(0.7, glide(300, 1300, 0.4), SOFT_SAW, attack=0.01, decay=0.5, vibrato=(25, 0.04))
    return mix((0.0, hiss, 0.6), (0.0, wail, 0.6))


def win():
    """Trapping the spider or cashing out: a rising arpeggio and a bell."""
    notes = [659.25, 783.99, 987.77, 1318.5]
    parts = [(i * 0.09, tone(0.16, f, SOFT_SQUARE, decay=0.1), 0.8) for i, f in enumerate(notes)]
    parts.append((0.36, tone(0.5, 1318.5, BELL, decay=0.25), 0.6))
    return mix(*parts)


if __name__ == "__main__":
    os.chdir(HERE)
    write("crack", crack(), 0.5)
    write("tick", tick(), 0.4)
    write("heartbeat", heartbeat(), 0.6)
    write("screech", screech(), 0.6)
    write("win", win(), 0.55)
