"""The trailer's edit: what plays when. `build.py` does the rest.

Times are seconds in the trailer. A shot's `takes` are clips filmed by `orbit_desktop footage`
(60 frames a second): `(take, from)` plays the take from `from` seconds in, and
`(take, from, length)` plays only that long before the next one. When the game changes,
`uv run build.py --refilm` films them again; check the cuts still land with
`uv run build.py --sheets`, which writes a contact sheet of every take.
"""

FPS = 30
SIZE = (1920, 1080)

# Happy Whistling Ukulele runs at 130 BPM with its first beat 0.135 s in. The cozy half cuts
# on its beats: eight for the title, then LOOP_BEATS for each gameplay shot.
BEAT = 60 / 130
LOOP_BEATS = 7


def on_beat(n):
    return 0.135 + n * BEAT


def loop_shot(n):
    """When gameplay shot n starts (FISH is 0, and 5 is when HATS ends)."""
    return on_beat(8 + n * LOOP_BEATS)


# After the loop each shot follows the last, (start, end), so changing one's length moves the
# rest.
SCRATCH = (loop_shot(5), loop_shot(5) + 0.7)
BEES = (SCRATCH[1], SCRATCH[1] + 4.0)
SKATE = (BEES[1], BEES[1] + 4.5)
SPIDERS = (SKATE[1], SKATE[1] + 3.5)
EAT = (SPIDERS[1], SPIDERS[1] + 8.5)
FISH_HOLD = (EAT[1], EAT[1] + 5.0)
LENGTH = FISH_HOLD[1]


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
        "end": loop_shot(0),
        "takes": [("opening", 0.0)],
        "title": ["FISHING", "MINIGAME"],
        # Then the camera dives into the player, blurring, until the first beat.
        "dive": loop_shot(0) - 1.15,
    },
    {
        "kind": "beat",
        "start": loop_shot(0),
        "end": loop_shot(1),
        "label": "FISH",
        "color": "#2b9cf2",
        "takes": [("fish_cast", 0.3, 1.3), ("fish_catch", 2.6)],
    },
    {
        "kind": "beat",
        "start": loop_shot(1),
        "end": loop_shot(2),
        "label": "TRAIN",
        "color": "#ff8a3d",
        "takes": [("train", 0.2)],
    },
    {
        "kind": "beat",
        "start": loop_shot(2),
        "end": loop_shot(3),
        "label": "WALK",
        "color": "#43b45a",
        "takes": [("walk", 1.8)],
    },
    {
        "kind": "beat",
        "start": loop_shot(3),
        "end": loop_shot(4),
        "label": "COMPETE",
        "color": "#c2457e",
        "takes": [("compete", 0.6, 2.0), ("compete_results", 0.0)],
        # The judges' ding rings on over the results, so the results' own is left out.
        "mute": ["compete_results"],
    },
    {
        "kind": "beat",
        "start": loop_shot(4),
        "end": loop_shot(5),
        "label": "HATS",
        "color": "#f2b01e",
        "takes": [("hats", 0.6)],
    },
    # The record scratches and HATS freezes, drains and goes dark.
    {"kind": "scratch", "start": SCRATCH[0], "end": SCRATCH[1]},
    {
        "kind": "chaos",
        "start": BEES[0],
        "end": BEES[1],
        "label": "BEESSSS????",
        # Honeycomb behind, the label in honey and pixel bees buzzing round it.
        "honey": True,
        "takes": [("bees", 1.6)],
    },
    {
        "kind": "chaos",
        "start": SKATE[0],
        "end": SKATE[1],
        "label": "SKATEBOARDS??",
        "color": "#4b2fbf",
        "takes": [("skate_grind", 0.3, 2.2), ("skate_halfpipe", 0.2)],
    },
    {
        "kind": "chaos",
        "start": SPIDERS[0],
        "end": SPIDERS[1],
        "label": "SPIDERS?!",
        "color": "#5e0a12",
        "takes": [("spider", 1.4)],
        # When the spider leaps, seconds into the shot: it comes at you.
        "pounce": 3.37 - 1.4,
    },
    {
        "kind": "eat",
        "start": EAT[0],
        "end": EAT[1],
        "screen": ("tank", 2.0),
        "bites": [1.5, 2.97],
        # Just after the second bite, so the words have time to land.
        "words_at": 3.3,
        "words": ["YOU", "CAN", "EAT", "IT"],
    },
    {
        "kind": "guten",
        "start": FISH_HOLD[0],
        "end": FISH_HOLD[1],
        "takes": [("guten", 0.0)],
        # The fish spins to the ukulele, not its own song.
        "mute": ["guten"],
        "tag": "FISHING MINIGAME",
        # Stacked beside the fish, a line at a time from this many seconds in; the last line
        # in gold.
        "message": ["HAPPY", "BIRTHDAY", "ETHAN"],
        "message_at": 0.4,
    },
]

# How loud the game's own sounds sit under the music.
GAME_SOUNDS_DB = -5.0

# Which music plays where. `offset` is seconds into the track (left out: where its sound
# starts), fades are in seconds, and `level` is (time, dB) points, straight lines between.
SCORE = [
    {"track": "cozy", "start": 0.0, "end": SCRATCH[0], "offset": 0.0},
    # The bumblebee crashes in after the scratch and fades away across the cut to the man.
    {"track": "bees", "start": BEES[0], "end": SPIDERS[1] + 0.6, "fade_out": 0.9},
    # The ukulele creeps back in under the chewing, from where the scratch stopped it, and
    # comes up for the fish.
    {
        "track": "cozy",
        "start": EAT[0],
        "end": FISH_HOLD[1],
        "offset": SCRATCH[0],
        "fade_in": 1.5,
        "fade_out": 1.5,
        "level": [(EAT[0], -11.0), (EAT[1] - 0.6, -11.0), (EAT[1], 0.0)],
    },
]
