"""Case geometry, built from params.py (what you tune) and board.py (what was measured).

The case is one cup shaped like a fly reel: the glass is the spool face, the rim is line wound
between two flanges, and the back plate carries a drag-knob ring, a whale and a line of
text. The board drops in glass-up and three M2 screws through the back pull its own SMT
standoffs down onto posts, so nothing clips or glues. Two printed plungers carry PWR and BOOT
through the rim. The battery variant is the same cup with the floor lowered by one cell.
"""

import math

from build123d import (
    Align,
    Axis,
    Box,
    CenterArc,
    Circle,
    Curve,
    Cylinder,
    Ellipse,
    Part,
    Plane,
    Polyline,
    Pos,
    Rectangle,
    RectangleRounded,
    Rot,
    Sketch,
    Spline,
    Text,
    ThreePointArc,
    extrude,
    make_face,
    mirror,
    revolve,
)

import board
import params as p

BORE_R = board.GLASS_R + p.GLASS_GAP
TROUGH_R = BORE_R + p.WALL
CREST_R = TROUGH_R + p.LINE_DEPTH
OUTER_R = CREST_R + p.FLANGE_PROUD
TOP_Z = p.RIM_PROUD
BOTTOM = (Align.CENTER, Align.CENTER, Align.MIN)


def contents(battery: bool) -> list[tuple[str, tuple[float, float, float]]]:
    """What lies under the board, from the floor up."""
    items = []
    if p.SPEAKER:
        items.append(("speaker", p.SPEAKER))
    if battery:
        items.append(("battery", p.BATTERY))
    return items


def floor_z(battery: bool) -> float:
    """Inside face of the floor: below the header, the stack under the board, and any plugs."""
    z = board.DEEPEST_Z - p.FLOOR_GAP
    items = contents(battery)
    if items:
        stack = sum(t + p.STACK_GAP for _, (_, _, t) in items)
        z = min(z, board.UNDER_BOARD_Z - stack, board.SOCKETS_Z - p.PLUG_ROOM)
    return z


def item_boxes(battery: bool) -> list[tuple[str, Part]]:
    """The speaker and cell where they lie, for fit checks and previews."""
    boxes, z = [], floor_z(battery)
    for name, (w, length, t) in contents(battery):
        boxes.append((name, Pos(0, p.BAY_CENTER_Y, z) * Box(w, length, t, align=BOTTOM)))
        z += t + p.STACK_GAP
    return boxes


def back_z(battery: bool) -> float:
    return floor_z(battery) - p.FLOOR


def shelf_z() -> float:
    """Where each board screw's head bears; its counterbore runs from the back up to here."""
    return board.STANDOFF_END_Z - p.SCREW_SHELF


def radial_plane(angle: float, r: float, z: float, offset: float = 0.0) -> Plane:
    """A plane facing straight out of the rim: local x runs along the rim, local y is +Z."""
    a = math.radians(angle)
    out = (math.cos(a), math.sin(a), 0.0)
    along = (-math.sin(a), math.cos(a), 0.0)
    origin = (r * out[0] + offset * along[0], r * out[1] + offset * along[1], z)
    return Plane(origin=origin, x_dir=along, z_dir=out)


def through_rim(angle: float, z: float, profile: Sketch, offset: float = 0.0) -> Part:
    """A cutter that pushes `profile` through the side wall."""
    start = BORE_R - 1.5
    return extrude(radial_plane(angle, start, z, offset) * profile, amount=OUTER_R + 3 - start)


def spool(battery: bool) -> Part:
    zb, zf = back_z(battery), floor_z(battery)
    step = OUTER_R - TROUGH_R
    band_lo = zb + p.BACK_FLANGE + step
    band_hi = TOP_Z - p.FRONT_FLANGE - step
    ridges = max(1, round((band_hi - band_lo) / p.LINE_PITCH))
    pitch = (band_hi - band_lo) / ridges
    line = Curve()
    for i in range(ridges):
        z0 = band_lo + i * pitch
        line += ThreePointArc(
            (TROUGH_R, 0, z0), (CREST_R, 0, z0 + pitch / 2), (TROUGH_R, 0, z0 + pitch)
        )
    r = p.FRONT_ROUND
    outline = (
        Polyline(
            (0, 0, zb),
            (OUTER_R - p.BACK_CHAMFER, 0, zb),
            (OUTER_R, 0, zb + p.BACK_CHAMFER),
            (OUTER_R, 0, zb + p.BACK_FLANGE),
            (TROUGH_R, 0, band_lo),
        )
        + line
        + Polyline((TROUGH_R, 0, band_hi), (OUTER_R, 0, band_hi + step), (OUTER_R, 0, TOP_Z - r))
        + ThreePointArc(
            (OUTER_R, 0, TOP_Z - r),
            (OUTER_R - r + r * math.sqrt(0.5), 0, TOP_Z - r + r * math.sqrt(0.5)),
            (OUTER_R - r, 0, TOP_Z),
        )
        + Polyline(
            (OUTER_R - r, 0, TOP_Z),
            (BORE_R + p.RIM_CHAMFER, 0, TOP_Z),
            (BORE_R, 0, TOP_Z - p.RIM_CHAMFER),
            (BORE_R, 0, zf),
            (0, 0, zf),
            (0, 0, zb),
        )
    )
    return revolve(make_face(outline), Axis.Z)


def screw_posts(battery: bool) -> Part:
    zf, zs = floor_z(battery), shelf_z()
    posts = Part()
    for x, y in board.STANDOFFS:
        posts += Pos(x, y, zf) * Cylinder(p.POST_D / 2, board.STANDOFF_END_Z - zf, align=BOTTOM)
        wide_top = zs + 0.6
        if wide_top > zf:
            posts += Pos(x, y, zf) * Cylinder(p.POST_WIDE_D / 2, wide_top - zf, align=BOTTOM)
    return posts


def screw_holes(battery: bool) -> Part:
    zb, zs = back_z(battery), shelf_z()
    holes = Part()
    for x, y in board.STANDOFFS:
        holes += Pos(x, y, zb - 1) * Cylinder(
            p.SCREW_CLEARANCE_D / 2, board.STANDOFF_END_Z + 1 - zb, align=BOTTOM
        )
        holes += Pos(x, y, zb - 1) * Cylinder(p.SCREW_HEAD_D / 2, zs + 1 - zb, align=BOTTOM)
    return holes


def stack_rails(battery: bool) -> Part:
    """Two low walls either side of the stack keep the speaker over the grille."""
    items = contents(battery)
    if not items:
        return Part()
    w = max(i[1][0] for i in items) + 2 * p.STACK_GAP
    length = max(i[1][1] for i in items)
    x = w / 2 + p.RAIL_WALL / 2
    rail = Box(p.RAIL_WALL, length, p.RAIL_HEIGHT, align=BOTTOM)
    z = floor_z(battery)
    return Pos(-x, p.BAY_CENTER_Y, z) * rail + Pos(x, p.BAY_CENTER_Y, z) * rail


def speaker_grille(battery: bool) -> Part:
    """Holes through the floor, plus a shallow chamber so the speaker can breathe into them."""
    if not p.SPEAKER:
        return Part()
    zb, zf = back_z(battery), floor_z(battery)
    holes = Part()
    for i in range(p.GRILLE_HOLES):
        a = 2 * math.pi * i / p.GRILLE_HOLES
        holes += Pos(p.GRILLE_R * math.cos(a), p.GRILLE_R * math.sin(a), zb - 1) * Cylinder(
            p.GRILLE_HOLE_D / 2, zf - zb + 2, align=BOTTOM
        )
    chamber_r = p.GRILLE_R + p.GRILLE_HOLE_D / 2 + 0.6
    chamber = Cylinder(chamber_r, p.GRILLE_CHAMBER + 0.4, align=BOTTOM)
    return holes + Pos(0, 0, zf - p.GRILLE_CHAMBER) * chamber


def port_cutouts() -> Part:
    w, h = p.USB_OPENING
    cuts = through_rim(board.USB_ANGLE, board.USB_Z, RectangleRounded(w, h, p.USB_OPENING_ROUND))
    hole = Circle(p.BUTTON_D / 2 + p.BUTTON_HOLE_GAP)
    for angle in (board.PWR_ANGLE, board.BOOT_ANGLE):
        cuts += through_rim(angle, board.KEY_Z, hole)
    if p.SD_SLOT:
        sw, sh = p.SD_SLOT_SIZE
        cuts += through_rim(
            board.SD_ANGLE, board.SD_Z, RectangleRounded(sw, sh, sh / 2 - 0.05), board.SD_OFFSET
        )
    return cuts


def whale(n: float) -> Sketch:
    """A cartoon whale about `n` long: nose left, tail raised, spouting. Centred on the origin."""

    def at(x, y):
        return Pos(x * n, y * n)

    outline = Spline(
        *[
            (x * n, y * n)
            for x, y in (
                (-0.50, -0.02),
                (-0.46, 0.12),
                (-0.34, 0.21),
                (-0.14, 0.23),
                (0.04, 0.17),
                (0.18, 0.08),
                (0.28, 0.06),
                (0.34, 0.12),
                (0.37, 0.17),
                (0.41, 0.15),
                (0.39, 0.06),
                (0.31, -0.07),
                (0.16, -0.16),
                (-0.06, -0.21),
                (-0.28, -0.20),
                (-0.44, -0.13),
            )
        ],
        periodic=True,
    )
    shape = make_face(outline)
    for a in (150, 35):
        r = math.radians(a)
        fluke = Rot(0, 0, a) * Ellipse(0.1 * n, 0.05 * n)
        shape += at(0.39 + 0.08 * math.cos(r), 0.17 + 0.08 * math.sin(r)) * fluke
    shape += at(-0.06, -0.2) * Rot(0, 0, -35) * Ellipse(0.1 * n, 0.045 * n)
    w = 0.05
    shape += at(-0.2, 0.3) * Rectangle(w * n, 0.12 * n)
    for cx, side in ((-0.28, 1), (-0.12, -1)):
        arm = at(cx, 0.36) * (Circle((0.08 + w / 2) * n) - Circle((0.08 - w / 2) * n))
        shape += arm & at(cx, 0.46) * Rectangle(0.3 * n, 0.2 * n)
        shape += at(cx - side * 0.08, 0.34) * Circle(0.035 * n)
    shape -= at(-0.33, 0.05) * Circle(0.035 * n)
    mouth = at(-0.3, 0.25) * Circle(0.33 * n) - at(-0.3, 0.3) * Circle(0.33 * n)
    shape -= mouth & at(-0.34, -0.07) * Ellipse(0.1 * n, 0.08 * n)
    box = shape.bounding_box()
    return Pos(-(box.min.X + box.max.X) / 2, -(box.min.Y + box.max.Y) / 2) * shape


def ring(r: float, w: float) -> Sketch:
    return Circle(r + w / 2) - Circle(r - w / 2)


def text_span(txt: str) -> float:
    """Degrees of the text circle that `txt` covers."""
    width = Text(txt, p.TEXT_SIZE, font=p.FONT).bounding_box().size.X
    return math.degrees(width / p.TEXT_RADIUS)


def arc_text(txt: str, centre: float, top: bool) -> Sketch:
    """`txt` centred on `centre` degrees, upright: clockwise over the top, anticlockwise below."""
    span = text_span(txt) + 20
    if top:
        path = CenterArc((0, 0), p.TEXT_RADIUS, centre + span / 2, -span)
    else:
        path = CenterArc((0, 0), p.TEXT_RADIUS, centre - span / 2, span)
    return Text(txt, p.TEXT_SIZE, font=p.FONT, path=path, position_on_path=0.5)


def screws_from_behind() -> list[tuple[float, float, float]]:
    """(x, y, angle) of each screw as seen on the back, which mirrors left and right."""
    return [(-x, y, math.degrees(math.atan2(y, -x)) % 360) for x, y in board.STANDOFFS]


def screw_clearance(x: float, y: float) -> float:
    """Degrees of the text circle to keep free either side of a screw head's bezel."""
    keep = p.SCREW_HEAD_D / 2 + p.BEZEL_W + p.SCREW_GAP
    return math.degrees(2 * math.asin(keep / (2 * math.hypot(x, y))))


def text_layout() -> list[tuple[str, float, bool]]:
    """(word, centre angle, over the top) for each word, fitted between the screw heads."""
    lower_left, lower_right, top = sorted(screws_from_behind(), key=lambda s: (s[1], s[0]))
    gap = {s: screw_clearance(s[0], s[1]) for s in (lower_left, lower_right, top)}
    left, right = p.TEXT_TOP
    words = []
    if left:
        start = top[2] + gap[top]
        room = lower_left[2] - gap[lower_left] - start
        words.append((left, start + text_span(left) / 2, True, room))
    if right:
        start = top[2] - gap[top]
        room = start - (lower_right[2] - 360 + gap[lower_right])
        words.append((right, start - text_span(right) / 2, True, room))
    if p.TEXT_BOTTOM:
        room = lower_right[2] - gap[lower_right] - (lower_left[2] + gap[lower_left])
        words.append((p.TEXT_BOTTOM, (lower_left[2] + lower_right[2]) / 2, False, room))
    for word, _, _, room in words:
        if text_span(word) > room:
            raise ValueError(
                f"{word!r} needs {text_span(word):.0f} deg of the text circle but only "
                f"{room:.0f} deg fit between its screws: shorten it or lower TEXT_SIZE"
            )
    return [(word, centre, over) for word, centre, over, _ in words]


def back_art_sketch() -> Sketch:
    """The raised art on the back, as seen from behind (+x right, +y up)."""
    r, w = p.KNOB_RING_R, p.KNOB_RING_W
    art = ring(r, w) + Rot(0, 0, -p.WHALE_ROTATION) * whale(p.WHALE_SIZE)
    length, width = p.TICK
    tick = Pos(r + w / 2 + 0.4 + length / 2, 0) * RectangleRounded(length, width, width / 2 - 0.05)
    for i in range(p.KNOB_TICKS):
        art += Rot(0, 0, 360 * i / p.KNOB_TICKS) * tick
    if p.BORDER_R:
        art += ring(p.BORDER_R, w)
    for x, y, _ in screws_from_behind():
        art += Pos(x, y) * ring(p.SCREW_HEAD_D / 2 + p.BEZEL_W / 2, p.BEZEL_W)
    for word, centre, over in text_layout():
        art += arc_text(word, centre, over)
    return art


def art(battery: bool = False) -> Part:
    """The raised art as a solid: what stands above the recessed background."""
    solid = extrude(back_art_sketch(), amount=p.ART_DEPTH)
    return Pos(0, 0, back_z(battery)) * mirror(solid, about=Plane.YZ)


def background(battery: bool) -> Part:
    """The recessed field around the raised art."""
    field = Cylinder(p.FIELD_R, p.ART_DEPTH + 0.1, align=BOTTOM)
    return Pos(0, 0, back_z(battery) - 0.1) * field - art(battery)


def case(battery: bool = False) -> Part:
    """The cup, in the case frame. Its back face is down, which is also how it prints."""
    body = spool(battery) + screw_posts(battery) + stack_rails(battery)
    cuts = screw_holes(battery) + port_cutouts() + speaker_grille(battery) + background(battery)
    return body - cuts


# --- Button plungers -------------------------------------------------------------------------
# Along its own axis a plunger runs from the switch actuator (inside) to past the rim. Its flange
# is a 45 degree cone that stops against the edge of the hole, which also centres it.


def button_stations() -> dict[str, float]:
    """Distances from the case centre along the button axis, with the plunger at rest."""
    hole_r = p.BUTTON_D / 2 + p.BUTTON_HOLE_GAP
    stop = math.sqrt(BORE_R**2 - hole_r**2)
    flange = stop - (p.BUTTON_FLANGE_D / 2 - hole_r)
    return {
        "nub": board.KEY_TIP_R + p.BUTTON_ACTUATOR_GAP,
        "flange_in": flange - p.BUTTON_FLANGE_T,
        "flange": flange,
        "shaft": stop + (hole_r - p.BUTTON_D / 2),
        "tip": OUTER_R + p.BUTTON_PROUD,
    }


def button_axial() -> Part:
    """The plunger with its axis on +Z, where Z is the distance from the case centre."""
    s = button_stations()
    rb, rf, rn, c = p.BUTTON_D / 2, p.BUTTON_FLANGE_D / 2, p.BUTTON_NUB_D / 2, p.BUTTON_TIP_CHAMFER
    outline = Polyline(
        (0, 0, s["nub"]),
        (rn, 0, s["nub"]),
        (rn, 0, s["flange_in"]),
        (rf, 0, s["flange_in"]),
        (rf, 0, s["flange"]),
        (rb, 0, s["shaft"]),
        (rb, 0, s["tip"] - c),
        (rb - c, 0, s["tip"]),
        (0, 0, s["tip"]),
        close=True,
    )
    return revolve(make_face(outline), Axis.Z)


def button(pressed: float = 0.0) -> Part:
    """Both plungers in place on the case (pressed = how far in, in mm)."""
    axial = Pos(0, 0, -pressed) * button_axial()
    both = Part()
    for angle in (board.PWR_ANGLE, board.BOOT_ANGLE):
        both += Pos(0, 0, board.KEY_Z) * Rot(0, 0, angle) * Rot(0, 90, 0) * axial
    return both


def button_print() -> Part:
    """One plunger standing on the face you press, as it prints."""
    return Pos(0, 0, button_stations()["tip"]) * Rot(180, 0, 0) * button_axial()
