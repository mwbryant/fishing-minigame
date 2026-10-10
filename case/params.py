"""Every number you're likely to tweak. Edit, then `uv run build.py` and `uv run check.py`.

Frame used by every file in this folder: hold the device with the game upright and look at
the screen. +X is right, +Y is up, +Z comes out of the glass, and Z = 0 is the glass front
face. Angles are counterclockwise from 3 o'clock, seen from the front. In this frame USB-C
is at 9 o'clock, PWR at 2 o'clock and BOOT at 4 o'clock (see board.py).
"""

# --- Fit: tune these after a test print -------------------------------------------------
GLASS_GAP = 0.30  # mm, glass edge to bore; raise it if the glass is tight in the fit ring
RIM_PROUD = 0.50  # mm the rim stands above the glass, so a face-down drop lands on plastic
FLOOR_GAP = 0.60  # mm under the deepest board part (8-pin header); sets depth if nothing's under
BUTTON_HOLE_GAP = 0.15  # mm all round a cap in its hole; raise it if the caps stick
BUTTON_ACTUATOR_GAP = 0.10  # mm from nub to switch at rest; raise if always pressed, lower if mushy

# --- Body ---------------------------------------------------------------------------------
WALL = 1.5  # mm, thinnest side wall, at the bottom of the line grooves; adds to the diameter
FLOOR = 2.5  # mm, back plate thickness; adds to the depth
FRONT_FLANGE = 2.2  # mm, height of the smooth flange around the glass; taller leaves less line
BACK_FLANGE = 2.0  # mm, height of the smooth flange at the back; taller leaves less line
FLANGE_PROUD = 0.30  # mm the flanges stand proud of the line; adds to the diameter
FRONT_ROUND = 1.0  # mm, radius of the front outer edge
BACK_CHAMFER = 0.8  # mm; a chamfer, not a round, so the first layers don't overhang
RIM_CHAMFER = 0.5  # mm, lead-in on the rim's inner edge that guides the glass into the bore

# Fishing line wound on the spool (the rim between the flanges)
LINE_PITCH = 1.2  # mm per turn, adjusted so a whole number of turns fit; smaller is finer
LINE_DEPTH = 0.35  # mm from groove to crest; deeper adds to the diameter

# --- Board screws (3x M2 into the board's own SMT standoffs) ------------------------------
SCREW_CLEARANCE_D = 2.7  # mm, hole for the M2 shank; printed holes come out small, and a tight
#                          one makes the screw cut its way in and crack the post
SCREW_HEAD_D = 4.4  # mm counterbore; M2 pan heads are 4.0, socket heads 3.8
SCREW_SHELF = 2.7  # mm of plastic under the head; 2.7 + 3.3 thread = M2x6 (check.py tests it)
POST_WIDE_D = 6.0  # mm, the posts the standoffs sit on; wider pushes the GPS antenna lower
POST_D = 4.6  # mm, the top post's last few mm, next to the BAT/SPK sockets
POST_FLARE = 1.5  # mm the foot of each post flares into the floor (45 deg), clear of the stack
POST_RIB = 1.6  # mm, thickness of the rib tying each post to the wall; 0 for none

# --- Buttons (two printed caps through the rim, pressing the side keys) --------------------
BUTTON_SIZE = (
    8.0,
    4.6,
)  # mm, the cap you press: along the rim x tall (a rounded slot, so it can't turn)
BUTTON_FLANGE = 0.9  # mm the flange inside overhangs the cap; a 45 deg cone, prints unsupported
BUTTON_FLANGE_T = 0.4  # mm, thickness of the flat flange behind the cone
BUTTON_NUB_D = 1.8  # mm, presses the switch actuator (2.0 x 1.0)
BUTTON_PROUD = 1.0  # mm the cap stands out past the flanges; more is easier to find and press
BUTTON_TIP_CHAMFER = 0.4  # mm, 45 deg chamfer round the cap's outer edge

# --- Glass seat ---------------------------------------------------------------------------
SEAT_LEDGE = 1.25  # mm a ledge reaches in from the bore under the glass rim (notched for USB-C)
SEAT_GAP = 0.10  # mm under the glass; the posts set the height, the ledge catches the glass when
#                  you press the screen and keeps the board level as it goes in

# --- Ports --------------------------------------------------------------------------------
USB_OPENING = (11.5, 6.0)  # mm, along the rim x tall; a cable's plug head must fit through
USB_RAISE = 0.5  # mm the opening sits above the port's modelled centre (from a test print)
USB_OPENING_ROUND = 2.5  # mm, corner radius
SD_SLOT = True  # cut the microSD slot; False closes it
SD_SLOT_SIZE = (13.0, 2.0)  # mm, along the rim x tall

# --- Back plate artwork ------------------------------------------------------------------
# The art is raised: the background around it is recessed by ART_DEPTH, so the case still
# prints face down without supports (the art is the first layers on the plate).
TEXT_TOP = ("HAPPY", "BIRTHDAY")  # either side of the top screw; "" leaves a side empty
TEXT_BOTTOM = "ETHAN"  # between the two lower screws
FONT = "Arial Rounded MT Bold"  # an installed font, by name
TEXT_SIZE = 7.0  # mm; build.py stops with a message if a word no longer fits between the screws
TEXT_RADIUS = 21.8  # mm, text centreline, on the same circle as the screw heads
SCREW_GAP = 1.0  # mm between a word and a screw's raised bezel; more leaves less room for text
BEZEL_W = 0.8  # mm, raised ring around each screw head
ART_DEPTH = 0.7  # mm, height of the raised art (depth of the recessed background)
FIELD_R = 24.9  # mm, where the recessed background ends; outside it is a flat rim
KNOB_RING_R = 12.2  # mm, the "drag knob" ring around the whale
KNOB_RING_W = 0.8  # mm, width of the knob ring and the border ring
KNOB_TICKS = 36  # raised ticks around the knob ring
TICK = (1.4, 0.7)  # mm, length x width of each raised tick outside the knob ring
BORDER_R = 16.6  # mm, ring between the knob and the text; 0 drops it
WHALE_SIZE = 13.0  # mm, nose to tail; must stay inside the grille ring (check.py)
WHALE_ROTATION = 0.0  # deg, turns the whale if the screen's "up" isn't +Y
GRILLE_R = 8.6  # mm, speaker grille: a ring of holes around the whale, like a reel's ported knob
GRILLE_HOLES = 24  # holes in the grille
GRILLE_HOLE_D = 1.6  # mm, grille hole diameter
GRILLE_CHAMBER = 0.5  # mm recess inside the floor so the speaker can breathe into the holes

# --- What lies under the board ----------------------------------------------------------
# From the floor up: the speaker (face down behind the grille), the GPS antenna (ceramic side
# down, looking out of the back), and in the battery case the cell. Sizes are mm, width x length
# x thickness. stack.py lifts each as high as the board allows and the floor drops to fit, so
# measure yours and set them here.
SPEAKER = (20.0, 30.0, 4.5)  # the 8 ohm speaker from the box; None: thinner, silent
GPS_ANTENNA = (27.1, 27.1, 7.2)  # the -G board's GNSS antenna; None for a board without GPS
GPS_PLACE = None  # (x, y, angle) to place the antenna by hand; None finds the highest spot
BATTERY = (20.5, 32.0, 6.7)  # battery case only: a real 602030 with its protection board
BAY_CENTER_Y = -1.0  # mm, y of the speaker, cell and rails; the antenna search centres here
STACK_GAP = 0.6  # mm around and above each item (LiPo pouches swell); adds to the depth
PLUG_ROOM = 3.5  # mm below the BAT/SPK sockets, for a mated plug and its wires bending away
RAIL_WALL = 1.0  # mm, thickness of the two low rails that keep the stack over the grille
RAIL_HEIGHT = 3.0  # mm, rail height
