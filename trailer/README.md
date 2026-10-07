# FISHING MINIGAME trailer

A 45 second trailer cut from the game itself:

1. **FISHING MINIGAME**, with the player at their house, and the camera dives into them.
2. **FISH, TRAIN, WALK, COMPETE, HATS:** one beat each, tilted, zooming in and out, cut on
   the ukulele's beat.
3. A record scratch, then **BEESSSS????** as "Flight of the Bumblebee" crashes in, and the
   bees.
4. **SKATEBOARDS??** (a grind, then the halfpipe) and **SPIDERS?!** (the spider leaps at
   you).
5. A crude man takes two bites out of the printed reel case: **YOU CAN EAT IT**.
6. The GUTEN MORGEN spinning fish, held for 5 seconds as the ukulele comes back.

```sh
cd trailer
uv run build.py              # out/fishing_minigame_trailer.mp4 (about 2 minutes)
uv run build.py --preview    # half size, 15 fps
uv run build.py --still 36.5 # one frame, as out/still-36.50.png
```

You need `cargo`, `uv`, `ffmpeg` and Blender. `build.py` finds Blender in
`/Applications`, in Steam, on `PATH`, or at `$BLENDER`.

## When the footage is final

```sh
uv run build.py --refilm     # film every take again from the current game
uv run build.py --sheets     # cache/sheets/<take>.png: 24 frames of each take, timestamped
```

Then check that each cut in `edit.py` still lands where it should. A shot's takes are
`(take, from)` or `(take, from, length)`, in seconds into the take. For example, FISH plays
`fish_cast` from 0.3 s for 1.1 s, then `fish_catch` from 2.8 s to the end of the beat.

The takes are scripted in `desktop/src/footage.rs` with the same test helpers as the
snapshots. To film them yourself, with an optional list of take names:

```sh
cargo run --release -p orbit_desktop -- footage <dir> [take...]
```

Each take is a folder of PNGs (one per 60 Hz update of game time) plus `audio.wav`, which is
what the board's speaker would have played. The trailer mixes those sounds in under the
music. If a game changes so a script no longer reaches its moment (a renamed `rig_*`, a
moved button), fix the take in `footage.rs`.

## How it's built

`build.py` runs these steps. Each one caches in `cache/`; delete a folder there to redo
that step.

| Step | Cached in | Redone when |
|---|---|---|
| Music and the font, downloaded | `cache/assets/` | the file is missing |
| The takes, filmed headless | `cache/footage/` | a take is missing, or `--refilm` |
| The ending, rendered in Blender (`eat.py`) | `cache/eat/<hash>/` | `eat.py`, the edit's bites, the screen frame or the case STL changes |
| Frames and the mix, encoded with ffmpeg | `out/` | every run |

- **The ending:**
  - Blender loads `case/out/reel_case.stl`, shows a game frame on the glass and cuts the
    bites out with booleans.
  - The man's mouth follows a curve that `build.py` works out: wide open for each bite,
    then chewing. A crunch plays each time it shuts.
- **Sound effects:**
  - the record scratch is the ukulele track dragged back and forth;
  - the dive's whoosh and the bites' crunches are synthesized;
  - everything else is the game's own sounds, except on the GUTEN fish.
- **No voice lines and no guten song:** YOU CAN EAT IT is only on screen. The GUTEN fish
  spins to the ukulele, because its own song is the "Guten Morgen, Sonnenschein" meme
  audio, which isn't ours (`"game_sounds": False` in `edit.py`).

## Credits

- **Music:**
  - "Happy Whistling Ukulele", from FreePD.com (CC0), fetched from the Internet Archive's
    copy of the FreePD library.
  - "Flight of the Bumblebee" (Rimsky-Korsakov), played by the US Army Band. It is public
    domain as a work of the US government, from Wikimedia Commons.
- **Font:** Luckiest Guy by Astigmatic, under the Apache License 2.0.
