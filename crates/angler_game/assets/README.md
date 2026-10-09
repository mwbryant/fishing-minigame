# Angler assets

Where Angler's sounds come from and how to rebuild them. Every clip is raw mono
16-bit little-endian PCM at 16 kHz (`orbit_audio::SAMPLE_RATE`).

`guten_morgen.pcm`: "Guten Morgen, Sonnenschein" from the "Guten Morgen"
spinning-fish meme ([TikTok @liquidsquident](https://www.tiktok.com/@liquidsquident)),
played by the GUTEN easter egg. It isn't in git (`.gitignore` skips it): without it
the game builds and GUTEN spins in silence for the same 15.15 s. To add it, save
the meme's audio as an MP3 and convert it here:

    ffmpeg -i guten_morgen.mp3 -ac 1 -ar 16000 -f s16le guten_morgen.pcm

`sfx/*.pcm`: the sound effects, synthesized (no samples) by `sfx.py` with only
Python's standard library and a fixed seed. Tweak a recipe and run
`python3 sfx.py` to regenerate every clip. Spider Roulette, Skate and Bees???
import its helpers for their own clips.
