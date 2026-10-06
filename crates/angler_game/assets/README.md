# Angler assets

`guten_morgen.pcm`: "Guten Morgen, Sonnenschein" as heard in the "Guten Morgen"
spinning-fish meme (TikTok @liquidsquident), reused from the logic-fish project
for the GUTEN easter egg. Raw mono 16-bit little-endian PCM at 16 kHz
(`orbit_audio::SAMPLE_RATE`), made with:

    ffmpeg -i ../../../../logic-fish/assets/guten_morgen.mp3 -ac 1 -ar 16000 -f s16le guten_morgen.pcm

`sfx/*.pcm`: the game's sound effects, synthesized (no samples) by `sfx.py`, which
uses only Python's standard library and a fixed seed. Tweak a recipe and rerun
`python3 sfx.py` to regenerate every clip in the same format.
