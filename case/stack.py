"""What lies under the board, and how deep that makes the case.

From the floor up: the speaker (face down behind the grille), the GPS antenna (ceramic side
down, so it looks out of the back), and in the battery case the cell. Each one goes as high as
the parts hanging under the board allow (board.LOW_PARTS), then the floor goes under the lowest
and the items rest on it, one on top of the other. Plain 2D geometry with no CAD kernel, so it's
fast.
"""

import math
from dataclasses import dataclass
from functools import cache

import board
import params as p

Poly = list[tuple[float, float]]
BORE_R = board.GLASS_R + p.GLASS_GAP


@dataclass(frozen=True)
class Item:
    """One thing under the board, placed: centre (x, y), turned `angle` deg, bottom face at z."""

    name: str
    size: tuple[float, float, float]  # width x length x thickness
    x: float
    y: float
    angle: float
    z: float  # bottom face


def shelf_z() -> float:
    """Where each board screw's head bears; its counterbore runs from the back up to here."""
    return board.STANDOFF_END_Z - p.SCREW_SHELF


def post_wide_top(x: float, y: float) -> float:
    """How high the post at (x, y) stays wide: up to its standoff, unless a socket is close."""
    keep = p.POST_WIDE_D / 2 + 0.3
    top = board.STANDOFF_END_Z
    for x0, y0, x1, y1, low in board.LOW_PARTS:
        if math.hypot(max(x0 - x, 0, x - x1), max(y0 - y, 0, y - y1)) < keep:
            top = min(top, low - 0.5)
    return top


def rib_top() -> float:
    """The ribs stop below the button flanges."""
    return board.KEY_Z - p.BUTTON_SIZE[1] / 2 - p.BUTTON_FLANGE - 0.5


def footprint(w: float, length: float, x: float, y: float, angle: float) -> Poly:
    """Corners of a `w` x `length` rectangle centred on (x, y), turned `angle` deg."""
    a = math.radians(angle)
    ca, sa = math.cos(a), math.sin(a)
    corners = ((-1, -1), (1, -1), (1, 1), (-1, 1))
    return [
        (x + sx * w / 2 * ca - sy * length / 2 * sa, y + sx * w / 2 * sa + sy * length / 2 * ca)
        for sx, sy in corners
    ]


def overlap(a: Poly, b: Poly) -> bool:
    """Whether two convex outlines overlap (separating axis test)."""
    for poly in (a, b):
        for (x1, y1), (x2, y2) in zip(poly, poly[1:] + poly[:1]):
            nx, ny = y2 - y1, x1 - x2
            pa = [nx * x + ny * y for x, y in a]
            pb = [nx * x + ny * y for x, y in b]
            if max(pa) <= min(pb) or max(pb) <= min(pa):
                return False
    return True


def box(x0: float, y0: float, x1: float, y1: float) -> Poly:
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


def rib(x: float, y: float, grow: float = 0.0) -> Poly:
    """The rib from the post at (x, y) straight out into the wall, `grow` bigger all round."""
    r = math.hypot(x, y)
    a = math.atan2(y, x)
    length = BORE_R + 0.5 - r
    mid = r + length / 2
    return footprint(
        length + 2 * grow,
        p.POST_RIB + 2 * grow,
        mid * math.cos(a),
        mid * math.sin(a),
        math.degrees(a),
    )


def ceiling(outline: Poly, plugs: bool) -> float:
    """The lowest thing over `outline`: board parts, the GPS plug and, with `plugs`, BAT/SPK."""
    zones = list(board.LOW_PARTS)
    if p.GPS_ANTENNA:
        x0, y0, x1, y1 = board.GPS_SOCKET
        zones.append((x0 - 1, y0 - 1, x1 + 1, y1 + 1, board.GPS_PLUG_Z))
    if plugs:
        m = 0.5
        zones += [
            (x0 - m, y0 - m, x1 + m, y1 + m, board.SOCKETS_Z - p.PLUG_ROOM)
            for x0, y0, x1, y1 in board.PLUG_ENTRIES
        ]
    z = board.UNDER_BOARD_Z
    for x0, y0, x1, y1, low in zones:
        if overlap(outline, box(x0, y0, x1, y1)):
            z = min(z, low)
    return z


def clear_of_posts(w: float, length: float, x: float, y: float, angle: float) -> bool:
    """Whether the rectangle keeps 0.3 mm clear of the bore, the screw posts and their ribs."""
    outline = footprint(w, length, x, y, angle)
    if any(math.hypot(cx, cy) > BORE_R - 0.3 for cx, cy in outline):
        return False
    if p.POST_RIB and any(overlap(outline, rib(px, py, 0.3)) for px, py in board.STANDOFFS):
        return False
    a = math.radians(angle)
    ca, sa = math.cos(a), math.sin(a)
    for px, py in board.STANDOFFS:
        dx, dy = px - x, py - y
        u, v = dx * ca + dy * sa, -dx * sa + dy * ca
        gap = math.hypot(max(abs(u) - w / 2, 0), max(abs(v) - length / 2, 0))
        if gap < p.POST_WIDE_D / 2 + 0.3:
            return False
    return True


@cache
def place_antenna(limit: float, plugs: bool) -> tuple[float, float, float, float]:
    """(x, y, angle, top) of the highest spot for the GPS antenna below `limit`.

    Tries every 2 deg and every 0.5 mm within 6 mm of the bay's centre. Ties go to the spot
    nearest the middle, over the speaker.
    """
    w, length, _ = p.GPS_ANTENNA
    best = None
    for angle in range(0, 90, 2):
        for i in range(-12, 13):
            for j in range(-12, 13):
                x, y = i / 2, p.BAY_CENTER_Y + j / 2
                if not clear_of_posts(w, length, x, y, angle):
                    continue
                top = min(ceiling(footprint(w, length, x, y, angle), plugs), limit) - p.STACK_GAP
                key = (round(top, 2), -math.hypot(x, y - p.BAY_CENTER_Y))
                if best is None or key > best[0]:
                    best = (key, x, y, angle, top)
    if best is None:
        raise ValueError(f"a {w} x {length} GPS antenna doesn't fit between the screw posts")
    _, x, y, angle, top = best
    return x, y, float(angle), top


@cache
def stack(battery: bool) -> tuple[tuple[Item, ...], float]:
    """Everything under the board, from the floor up, and the floor's inside face."""
    wanted = []
    if p.SPEAKER:
        wanted.append(("speaker", p.SPEAKER))
    if p.GPS_ANTENNA:
        wanted.append(("gps antenna", p.GPS_ANTENNA))
    if battery:
        wanted.append(("battery", p.BATTERY))
    plugs = bool(wanted)

    limit = board.UNDER_BOARD_Z
    placed = []
    for name, (w, length, t) in reversed(wanted):
        if name == "gps antenna" and p.GPS_PLACE:
            x, y, angle = p.GPS_PLACE
            top = min(ceiling(footprint(w, length, x, y, angle), plugs), limit) - p.STACK_GAP
        elif name == "gps antenna":
            x, y, angle, top = place_antenna(limit, plugs)
        else:
            x, y, angle = 0.0, p.BAY_CENTER_Y, 0.0
            top = min(ceiling(footprint(w, length, x, y, angle), plugs), limit) - p.STACK_GAP
        placed.append((name, (w, length, t), x, y, angle))
        limit = top - t

    floor = board.DEEPEST_Z - p.FLOOR_GAP
    if wanted:
        floor = min(floor, limit, board.SOCKETS_Z - p.PLUG_ROOM)
    items, z = [], floor
    for name, size, x, y, angle in reversed(placed):
        items.append(Item(name, size, x, y, angle, z))
        z += size[2] + p.STACK_GAP
    return tuple(items), floor
