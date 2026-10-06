"""Every number you are likely to tweak, in millimetres. Edit, then `uv run build.py`.

Frame used by every file in this folder: hold the device with the game upright and look at
the screen. +X is right, +Y is up, +Z comes out of the glass, and Z = 0 is the glass front
face. Angles are counterclockwise from 3 o'clock, seen from the front. In this frame USB-C
is at 9 o'clock, PWR at 2 o'clock and BOOT at 4 o'clock (see board.py).
"""

# --- Fit: tune these after a test print -------------------------------------------------
GLASS_GAP = 0.30  # radial gap between the glass edge and the case bore
RIM_PROUD = 0.50  # rim height above the glass, so a face-down drop lands on plastic
FLOOR_GAP = 0.60  # gap between the deepest board part (8-pin header) and the floor
BUTTON_HOLE_GAP = 0.15  # gap all round between a button cap and its hole
BUTTON_ACTUATOR_GAP = 0.10  # gap between a button and the switch at rest

# --- Body ---------------------------------------------------------------------------------
WALL = 1.5  # thinnest side wall, at the bottom of the line grooves
FLOOR = 2.5  # back plate thickness
FRONT_FLANGE = 2.2  # smooth reel flange around the glass (height)
BACK_FLANGE = 2.0  # smooth reel flange at the back (height)
FLANGE_PROUD = 0.30  # flanges stand this far proud of the line
FRONT_ROUND = 1.0
BACK_CHAMFER = 0.8  # a chamfer, not a round, so the first layers don't overhang
RIM_CHAMFER = 0.3  # inner edge of the rim, around the glass

# Fishing line wound on the spool (the rim between the flanges)
LINE_PITCH = 1.2
LINE_DEPTH = 0.35

# --- Board screws (3x M2 into the board's own SMT standoffs) ------------------------------
SCREW_CLEARANCE_D = 2.4
SCREW_HEAD_D = 4.2  # counterbore; M2 pan and socket heads are 3.8
SCREW_SHELF = 2.7  # plastic the screw head clamps; 2.7 + 3.3 thread = M2x6
POST_D = 4.6  # posts the standoffs sit on (narrow: the BAT/SPK sockets are close)
POST_WIDE_D = 6.0  # lower part of the posts in the battery case, around the counterbore

# --- Buttons (two printed caps through the rim, pressing the side keys) --------------------
BUTTON_SIZE = (
    8.0,
    4.6,
)  # the cap you press: along the rim x tall (a rounded slot, so it can't turn)
BUTTON_FLANGE = (
    0.9  # how far the flange inside overhangs the cap; a 45 deg cone, prints unsupported
)
BUTTON_FLANGE_T = 0.4
BUTTON_NUB_D = 1.8  # presses the switch actuator (2.0 x 1.0)
BUTTON_PROUD = 1.0  # how far the cap stands out past the flanges
BUTTON_TIP_CHAMFER = 0.4

# --- Ports --------------------------------------------------------------------------------
USB_OPENING = (13.0, 7.4)  # along the rim x tall; USB-C plug overmolds are at most 12.35 x 6.5
USB_OPENING_ROUND = 2.5
SD_SLOT = True
SD_SLOT_SIZE = (13.0, 2.0)

# --- Back plate artwork ------------------------------------------------------------------
# The art is raised: the background around it is recessed by ART_DEPTH, so the case still
# prints face down without supports (the art is the first layers on the plate).
TEXT_TOP = ("HAPPY", "BIRTHDAY")  # either side of the top screw; "" leaves a side empty
TEXT_BOTTOM = "ETHAN"  # between the two lower screws
FONT = "Arial Rounded MT Bold"
TEXT_SIZE = 7.0  # build.py stops with a message if a word no longer fits between the screws
TEXT_RADIUS = 21.8  # text centreline, on the same circle as the screw heads
SCREW_GAP = 1.0  # between a word and a screw's raised bezel
BEZEL_W = 0.8  # raised ring around each screw head
ART_DEPTH = 0.7  # height of the raised art (depth of the recessed background)
FIELD_R = 24.9  # the recessed background ends here; outside it is a flat rim
KNOB_RING_R = 12.2  # the "drag knob" ring around the whale
KNOB_RING_W = 0.8
KNOB_TICKS = 36
TICK = (1.4, 0.7)  # length x width of each raised tick outside the knob ring
BORDER_R = 16.6  # ring between the knob and the text; 0 to drop it
WHALE_SIZE = 13.0  # nose to tail; must stay inside the grille ring (check.py)
WHALE_ROTATION = 0.0  # turn the whale (degrees) if the screen's "up" isn't +Y
GRILLE_R = 8.6  # speaker grille: a ring of holes around the whale, like a reel's ported knob
GRILLE_HOLES = 24
GRILLE_HOLE_D = 1.6
GRILLE_CHAMBER = 0.5  # recess inside the floor so the speaker can breathe into the holes

# --- What lies under the board ----------------------------------------------------------
# Both lie flat in the middle, wires out of the top end towards the BAT and SPK sockets at
# 12 o'clock. The speaker lies face down on the floor behind the grille; in the battery case
# the cell lies on top of it. Measure yours: the floor drops to fit whatever you set here.
SPEAKER = (20.0, 30.0, 4.5)  # the 8 ohm speaker from the box; None for a thinner, silent case
BATTERY = (20.0, 30.0, 6.0)  # battery case only; 20 x 30 LiPo cells (402030 ... 802030) are common
BAY_CENTER_Y = -1.8
STACK_GAP = 0.6  # around and above each item (LiPo pouches swell)
PLUG_ROOM = 3.5  # below the BAT/SPK sockets, for a mated plug and its wires bending away
RAIL_WALL = 1.0  # two low rails keep the stack over the grille
RAIL_HEIGHT = 3.0
