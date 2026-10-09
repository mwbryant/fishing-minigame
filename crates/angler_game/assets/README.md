# Angler assets

Where Angler's sounds come from and how to rebuild them. Every clip is raw mono
16-bit little-endian PCM at 16 kHz (`orbit_audio::SAMPLE_RATE`).

`sfx/*.pcm`: the sound effects, synthesized (no samples) by `sfx.py` with only
Python's standard library and a fixed seed. Tweak a recipe and run
`python3 sfx.py` to regenerate every clip. Spider Roulette, Skate and Bees???
import its helpers for their own clips.
