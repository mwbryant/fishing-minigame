"""The trailer's edit: what plays when. `build.py` does the rest.

Times are seconds in the trailer. A shot's `takes` are clips filmed by `orbit_desktop footage`
(60 frames a second): `(take, from)` plays the take from `from` seconds in, and
`(take, from, length)` plays only that long before the next one. When the game changes,
`uv run build.py --refilm` films them again; check the cuts still land with
`uv run build.py --sheets`, which writes a contact sheet of every take.
"""

FPS = 30
SIZE = (1920, 1080)
LENGTH = 45.0

# Happy Whistling Ukulele runs at 130 BPM with its first beat 0.135 s in; the cozy half cuts
# on its beats, six to a beat of the loop.
BEAT = 60 / 130


def on_beat(n):
    return 0.135 + n * BEAT


MUSIC = {
    "cozy": {
        "title": "Happy Whistling Ukulele",
        "credit": "FreePD.com (CC0)",
        "archive": "upbeat/Happy Whistling Ukulele.mp3",
        "gain_db": -5.0,
    },
    "bees": {
        "title": "Flight of the Bumblebee (Rimsky-Korsakov)",
        "credit": "US Army Band, public domain, via Wikimedia Commons",
        "url": "https://upload.wikimedia.org/wikipedia/commons/c/c0/"
        "Rimsky-Korsakov_-_flight_of_the_bumblebee.oga",
        "gain_db": -4.0,
    },
}

SHOTS = [
    {
        "kind": "opening",
        "start": 0.0,
        "end": on_beat(8),
        "takes": [("opening", 0.0)],
        "title": ["FISHING", "MINIGAME"],
        # Then the camera dives into the player, blurring, until the first beat.
        "dive": on_beat(8) - 1.15,
    },
    {
        "kind": "beat",
        "start": on_beat(8),
        "end": on_beat(14),
        "label": "FISH",
        "color": "#2b9cf2",
        "takes": [("fish_cast", 0.3, 1.1), ("fish_catch", 2.8)],
    },
    {
        "kind": "beat",
        "start": on_beat(14),
        "end": on_beat(20),
        "label": "TRAIN",
        "color": "#ff8a3d",
        "takes": [("train", 0.2)],
    },
    {
        "kind": "beat",
        "start": on_beat(20),
        "end": on_beat(26),
        "label": "WALK",
        "color": "#43b45a",
        "takes": [("walk", 1.8)],
    },
    {
        "kind": "beat",
        "start": on_beat(26),
        "end": on_beat(32),
        "label": "COMPETE",
        "color": "#c2457e",
        "takes": [("compete", 0.6, 1.9), ("compete_results", 0.0)],
    },
    {
        "kind": "beat",
        "start": on_beat(32),
        "end": on_beat(38),
        "label": "HATS",
        "color": "#f2b01e",
        "takes": [("hats", 0.6)],
    },
    # The record scratches and HATS freezes, drains and goes dark.
    {"kind": "scratch", "start": on_beat(38), "end": 18.4},
    {"kind": "card", "start": 18.4, "end": 20.0, "label": "BEESSSS????"},
    {
        "kind": "chaos",
        "start": 20.0,
        "end": 24.0,
        "color": "#2a1c05",
        "honey": True,
        "takes": [("bees", 1.6)],
    },
    {
        "kind": "chaos",
        "start": 24.0,
        "end": 28.5,
        "label": "SKATEBOARDS??",
        "color": "#4b2fbf",
        "takes": [("skate_grind", 0.3, 2.2), ("skate_halfpipe", 0.2)],
    },
    {
        "kind": "chaos",
        "start": 28.5,
        "end": 32.0,
        "label": "SPIDERS?!",
        "color": "#5e0a12",
        "takes": [("spider", 1.4)],
        # When the spider leaps, seconds into the shot: it comes at you.
        "pounce": 3.37 - 1.4,
    },
    {
        "kind": "eat",
        "start": 32.0,
        "end": 40.0,
        "screen": ("tank", 2.0),
        "bites": [1.5, 2.97],
        "words_at": 4.0,
        "words": ["YOU", "CAN", "EAT", "IT"],
    },
    {
        "kind": "guten",
        "start": 40.0,
        "end": LENGTH,
        "takes": [("guten", 0.0)],
        # The fish spins to the ukulele, not its own song.
        "game_sounds": False,
        "tag": "FISHING MINIGAME",
    },
]

# Which music plays where: (track, from, to, offset into the track, seconds of fade-out). An
# offset of None starts the track where its sound starts. The bumblebee crashes in on the
# BEES card, and the ukulele comes back for the fish where the scratch stopped it.
SCORE = [
    ("cozy", 0.0, on_beat(38), 0.0, 0.005),
    ("bees", 18.4, 32.0, None, 0.005),
    ("cozy", 40.0, LENGTH, on_beat(38), 1.5),
]
