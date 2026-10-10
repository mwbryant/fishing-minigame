"""Case geometry, built from params.py (what you tune) and board.py (what was measured).

The case is one cup shaped like a fly reel: the glass is the spool face, the rim is line wound
between two flanges, and the back plate carries a drag-knob ring, a whale and raised text
around the screw heads. The board drops in glass-up and three M2 screws through the back pull
its own SMT standoffs down onto posts, so nothing clips or glues. Two printed button caps carry
PWR and BOOT through the rim. The battery variant is the same cup with the floor lowered to fit
the cell.
"""

import math

from build123d import (
    Align,
    Axis,
    Box,
    CenterArc,
    Circle,
    Cone,
    Curve,
    Cylinder,
    Ellipse,
    Part,
    Plane,
    Polygon,
    Polyline,
    Pos,
    Rectangle,
    RectangleRounded,
    Rot,
    Sketch,
    SlotOverall,
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
import stack

# Radii out from the centre: the glass bore, the line's trough and crest, then the flanges.
BORE_R = board.GLASS_R + p.GLASS_GAP
TROUGH_R = BORE_R + p.WALL
CREST_R = TROUGH_R + p.LINE_DEPTH
OUTER_R = CREST_R + p.FLANGE_PROUD
TOP_Z = p.RIM_PROUD
BOTTOM = (Align.CENTER, Align.CENTER, Align.MIN)
SEAT_Z = -board.GLASS_DEPTH - p.SEAT_GAP  # top of the ledge under the glass rim
SEAT_R = BORE_R - p.SEAT_LEDGE  # its inner edge


def floor_z(battery: bool) -> float:
    """Inside face of the floor."""
    return stack.stack(battery)[1]


def item_boxes(battery: bool) -> list[tuple[str, Part]]:
    """The speaker, GPS antenna and cell where they lie, for fit checks and previews."""
    return [
        (i.name, Pos(i.x, i.y, i.z) * Rot(0, 0, i.angle) * Box(*i.size, align=BOTTOM))
        for i in stack.stack(battery)[0]
    ]


def back_z(battery: bool) -> float:
    """Outside face of the back plate, which prints on the bed."""
    return floor_z(battery) - p.FLOOR


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
    """The revolved body: back flange, wound line, front flange and rim, hollow to the floor.

    Under the glass rim a ledge runs round the bore; its underside is a 45 deg slope so it prints.
    """
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
            (BORE_R, 0, SEAT_Z),
            (SEAT_R, 0, SEAT_Z),
            (SEAT_R, 0, SEAT_Z - 0.4),
            (BORE_R, 0, SEAT_Z - 0.4 - p.SEAT_LEDGE),
            (BORE_R, 0, zf),
            (0, 0, zf),
            (0, 0, zb),
        )
    )
    return revolve(make_face(outline), Axis.Z)


def screw_posts(battery: bool) -> Part:
    """The posts under the board's standoffs: wide, flared into the floor, ribbed to the wall.

    The top post narrows for its last few mm, beside the BAT/SPK sockets. The flares are cut
    back wherever the stack's bottom layer lies, so the speaker still drops in.
    """
    zf = floor_z(battery)
    wide = p.POST_WIDE_D / 2
    posts, flares = Part(), Part()
    for x, y in board.STANDOFFS:
        wide_top = stack.post_wide_top(x, y)
        posts += Pos(x, y, zf) * Cylinder(wide, wide_top - zf, align=BOTTOM)
        if wide_top < board.STANDOFF_END_Z:
            posts += Pos(x, y, zf) * Cylinder(p.POST_D / 2, board.STANDOFF_END_Z - zf, align=BOTTOM)
        if p.POST_FLARE:
            flare = Cone(wide + p.POST_FLARE, wide, p.POST_FLARE, align=BOTTOM)
            flares += Pos(x, y, zf) * flare
        if p.POST_RIB:
            rib = Polygon(*stack.rib(x, y), align=None)
            posts += Pos(0, 0, zf) * extrude(rib, amount=stack.rib_top() - zf)
    for _, box in item_boxes(battery):
        bb = box.bounding_box()
        if bb.min.Z < zf + p.POST_FLARE:
            keep_out = Box(
                bb.size.X + 2 * p.STACK_GAP, bb.size.Y + 2 * p.STACK_GAP, 20, align=BOTTOM
            )
            flares -= Pos(bb.center().X, bb.center().Y, zf - 1) * keep_out
    return posts + flares


def screw_holes(battery: bool) -> Part:
    """Clearance holes for the three board screws, counterbored from the back."""
    zb, zs = back_z(battery), stack.shelf_z()
    holes = Part()
    for x, y in board.STANDOFFS:
        holes += Pos(x, y, zb - 1) * Cylinder(
            p.SCREW_CLEARANCE_D / 2, board.STANDOFF_END_Z + 1 - zb, align=BOTTOM
        )
        holes += Pos(x, y, zb - 1) * Cylinder(p.SCREW_HEAD_D / 2, zs + 1 - zb, align=BOTTOM)
    return holes


def stack_rails(battery: bool) -> Part:
    """Two low walls either side of the speaker keep it over the grille."""
    if not p.SPEAKER:
        return Part()
    w, length, _ = p.SPEAKER
    x = w / 2 + p.STACK_GAP + p.RAIL_WALL / 2
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
    """The USB-C opening, both button holes with seats for their flanges, and the SD slot.

    The glass ledge is notched over USB-C, the one part wider than the ledge's opening.
    """
    w, h = p.USB_OPENING
    usb_z = board.USB_Z + p.USB_RAISE
    cuts = through_rim(board.USB_ANGLE, usb_z, RectangleRounded(w, h, p.USB_OPENING_ROUND))
    ledge_h = 0.4 + p.SEAT_LEDGE
    notch = Rectangle(board.USB_SIZE[0] + 1.6, ledge_h + 0.2)
    start, reach = SEAT_R - 1, board.USB_FACE_R + 0.4
    plane = radial_plane(board.USB_ANGLE, start, SEAT_Z - ledge_h / 2)
    cuts += extrude(plane * notch, amount=reach - start)
    w, h = p.BUTTON_SIZE
    g, f = p.BUTTON_HOLE_GAP, p.BUTTON_FLANGE
    hole = SlotOverall(w + 2 * g, h + 2 * g)
    seat = SlotOverall(w + 2 * f + 1, h + 2 * f + 1)
    for angle in (board.PWR_ANGLE, board.BOOT_ANGLE):
        cuts += through_rim(angle, stack.BUTTON_Z, hole)
        cuts += extrude(radial_plane(angle, BORE_R - 2, stack.BUTTON_Z) * seat, amount=2)
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
    """A ring `w` wide, centred on radius `r`."""
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


# --- Buttons ---------------------------------------------------------------------------------
# From the switch actuator out past the rim, a button is a nub, a flange, a 45 degree cone and the
# cap. The cone stops against the hole's edge on a flat seat cut into the bore, which also centres
# the cap. The rounded-slot shape keys it so it can't turn.


def button_stations() -> dict[str, float]:
    """Distances from the case centre along the button axis, with the button at rest."""
    cone_end = BORE_R + p.BUTTON_HOLE_GAP
    cone_start = cone_end - p.BUTTON_FLANGE
    return {
        "nub": board.KEY_TIP_R + p.BUTTON_ACTUATOR_GAP,
        "flange_in": cone_start - p.BUTTON_FLANGE_T,
        "cone_start": cone_start,
        "cone_end": cone_end,
        "tip": OUTER_R + p.BUTTON_PROUD,
    }


def button_axial() -> Part:
    """One button with its axis on +Z (Z is the distance from the case centre), long side on X."""
    s = button_stations()
    w, h = p.BUTTON_SIZE
    f, c = p.BUTTON_FLANGE, p.BUTTON_TIP_CHAMFER
    flange = SlotOverall(w + 2 * f, h + 2 * f)
    nub = Cylinder(p.BUTTON_NUB_D / 2, s["flange_in"] - s["nub"], align=BOTTOM)
    cap_length = s["tip"] - s["cone_end"]
    cap = extrude(SlotOverall(w, h), amount=cap_length - c)
    cap += Pos(0, 0, cap_length - c) * extrude(SlotOverall(w, h), amount=c, taper=45)
    return (
        Pos(0, 0, s["nub"]) * nub
        + Pos(0, 0, s["flange_in"]) * extrude(flange, amount=p.BUTTON_FLANGE_T)
        + Pos(0, 0, s["cone_start"]) * extrude(flange, amount=f, taper=45)
        + Pos(0, 0, s["cone_end"]) * cap
    )


def button(pressed: float = 0.0) -> Part:
    """Both buttons in place on the case (pressed = how far in, in mm)."""
    axial = Pos(0, 0, -pressed) * button_axial()
    both = Part()
    for angle in (board.PWR_ANGLE, board.BOOT_ANGLE):
        both += radial_plane(angle, 0, stack.BUTTON_Z).location * axial
    return both


def button_print() -> Part:
    """One button lying on the face you press, as it prints."""
    return Pos(0, 0, button_stations()["tip"]) * Rot(180, 0, 0) * button_axial()
