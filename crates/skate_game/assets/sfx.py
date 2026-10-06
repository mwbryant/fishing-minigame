#!/usr/bin/env python3
"""Synthesizes Skate's sounds into sfx/*.pcm (mono s16le, 16 kHz), with the
helpers from Angler's generator. Standard library only, seeded:

    python3 sfx.py
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "angler_game", "assets"))
from sfx import glide, mix, noise, shape, tone, write  # noqa: E402


def pop():
    """The tail snapping the street: a crack and a woody knock."""
    return mix(
        (0.0, noise(0.02, 6000, high_pass=True, decay=0.005), 1.0),
        (0.0, tone(0.09, glide(420, 180, 0.06), decay=0.025), 0.7),
    )


def land():
    """Four wheels at once: a low clack with a short rattle."""
    return mix(
        (0.0, tone(0.12, glide(160, 70, 0.08), decay=0.035), 1.0),
        (0.0, noise(0.08, 2500, decay=0.02), 0.5),
    )


def grind():
    """One scrape of truck on wood, repeated while grinding."""
    scrape = noise(0.11, 3500, high_pass=True, attack=0.01)
    return shape(scrape, lambda x: 1.0 - x)


def bail():
    """Board clattering away and a thud."""
    return mix(
        (0.0, noise(0.35, 1800, decay=0.12), 0.8),
        (0.0, tone(0.25, glide(140, 50, 0.2), decay=0.08), 1.0),
        (0.12, noise(0.05, 4000, high_pass=True, decay=0.015), 0.5),
        (0.22, noise(0.05, 4000, high_pass=True, decay=0.015), 0.4),
    )


if __name__ == "__main__":
    os.chdir(HERE)
    write("pop", pop(), 0.6)
    write("land", land(), 0.6)
    write("grind", grind(), 0.35)
    write("bail", bail(), 0.65)
