# FISHING MINIGAME trailer

This folder builds a 46-second trailer cut from the game's own frames and sounds:

1. **FISHING MINIGAME**, with the player at their house, and the camera dives into them.
2. **FISH, TRAIN, WALK, COMPETE, HATS:** one shot each, tilted, zooming in and out, cut on the
   ukulele's beat.
3. A record scratch, then the bees with **BEESSSS????** beside them as "Flight of the
   Bumblebee" crashes in.
4. **SKATEBOARDS??** (a grind, then the halfpipe) and **SPIDERS?!** (the spider leaps at
   you).
5. A crude man takes two bites out of the printed reel case: **YOU CAN EAT IT**.
6. The GUTEN MORGEN spinning fish, held for 5 seconds as the ukulele comes back, beside
   **HAPPY BIRTHDAY ETHAN**.

```sh
cd trailer
uv run build.py              # out/fishing_minigame_trailer.mp4 (about 2 minutes)
uv run build.py --preview    # half size, 15 fps
uv run build.py --still 36.5 # one frame, as out/still-36.50.png
```

You need `cargo`, `uv`, `ffmpeg` and Blender. `build.py` looks for Blender at `$BLENDER`, on
`PATH`, in `/Applications`, then in Steam.

## When the game changes

```sh
uv run build.py --refilm     # film every take again from the current game
uv run build.py --sheets     # cache/sheets/<take>.png: 24 timestamped frames of each take
```

Then check that each cut in `edit.py` still lands. A shot's takes are `(take, from)` or
`(take, from, length)`, in seconds into the take: FISH plays `fish_cast` from 0.3 s for 1.3 s,
then `fish_catch` from 2.6 s to the end of the beat. After the gameplay loop each shot starts
where the one before ends, so lengthening one (the tuples at the top of `edit.py`) moves
everything after it, and the trailer grows to fit.

The takes are scripted in `desktop/src/footage.rs` with the snapshots' test helpers. To film them
yourself, optionally naming takes:

```sh
cargo run --release -p orbit_desktop -- footage <dir> [take...]
```

Each take is a folder of PNGs, one per 60 Hz update of game time, plus `audio.wav`, which is what
the board's speaker would have played. The opening also writes `focus.txt`, where the player
stands, for the dive. If a game change stops a script reaching its moment (a renamed `rig_*`, a
moved button), fix the take in `footage.rs`.

## How it's built

`build.py` runs these steps. Each caches in `cache/`; delete a folder there to redo that step.

| Step | Cached in | Redone when |
|---|---|---|
| Music and the font, downloaded | `cache/assets/` | the file is missing |
| The takes, filmed headless | `cache/footage/` | a take is missing, or `--refilm` |
| The ending, rendered in Blender (`eat.py`) | `cache/eat/<hash>/` | `eat.py`, the edit's bites, the screen frame or the case STL changes |
| Frames and the mix, encoded with ffmpeg | `out/` | every run |

- **The ending:** Blender loads `case/out/reel_case.stl`, shows a game frame on the glass and cuts
  the bites out with booleans. The man's mouth follows a curve that `build.py` works out: wide
  open for each bite, then chewing, with a crunch each time it shuts. YOU CAN EAT IT is on screen
  only.
- **Sounds:** the game's own sounds play under the music. A cut lets a sound finish (up to a
  second) instead of chopping it. The record scratch is the ukulele track dragged back and
  forth, and the dive's whoosh and the bites' crunches are synthesized.
- **Muted takes:** a shot's `mute` drops a take's sounds. The results after COMPETE are muted so
  the judges' ding plays alone, and the GUTEN fish spins to the ukulele.
- **Music** (`SCORE` in `edit.py`): the bumblebee fades across the cut to the man. The ukulele
  creeps back in under the chewing, from where the scratch stopped it, and comes up to full for
  the fish.

## Credits

- **Music:**
  - "Happy Whistling Ukulele", from FreePD.com (CC0), fetched from the Internet Archive's
    copy of the FreePD library.
  - "Flight of the Bumblebee" (Rimsky-Korsakov), played by the US Army Band. It is public
    domain as a work of the US government, from Wikimedia Commons.
- **Font:** Luckiest Guy by Astigmatic, under the Apache License 2.0.
