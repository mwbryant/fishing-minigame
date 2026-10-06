#!/usr/bin/env python3
"""Synthesizes Bees???'s sounds into sfx/*.pcm (mono s16le, 16 kHz), with the
helpers from Angler's generator. Standard library only, seeded:

    python3 sfx.py
"""

import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "angler_game", "assets"))
from sfx import SOFT_SAW, glide, mix, noise, shape, tone, write  # noqa: E402


def drip():
    """A drop of honey leaving the jar: a thick, rising plink."""
    return tone(0.12, glide(380, 900, 0.08), decay=0.04)


def buzz():
    """A bee arriving: a saw buzz with a wingbeat tremolo."""
    wings = lambda x: 0.55 + 0.45 * math.sin(2 * math.pi * 32 * x * 0.45)
    hum = tone(0.45, glide(210, 250, 0.3), SOFT_SAW, attack=0.03, decay=0.3)
    return shape(hum, wings)


def empty():
    """The last of the honey: three slow glugs going down."""
    glug = lambda f: tone(0.22, glide(f, f * 0.55, 0.18), decay=0.08)
    return mix((0.0, glug(320), 1.0), (0.26, glug(260), 0.9), (0.52, glug(200), 0.8))


if __name__ == "__main__":
    os.chdir(HERE)
    write("drip", drip(), 0.45)
    write("buzz", buzz(), 0.4)
    write("empty", empty(), 0.6)
