"""What lies under the board, and how deep that makes the case.

From the floor up: the speaker (face down behind the grille), the GPS antenna (ceramic side
down, so it looks out of the back), and in the battery case the cell. Each one goes as high as
the parts hanging under the board allow (board.LOW_PARTS), then the floor goes under the lowest
and everything rests on it, one on top of the other. Plain geometry, no CAD kernel, so it's fast.
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
    name: str
    size: tuple[float, float, float]  # width x length x thickness
    x: float
    y: float
    angle: float
    z: float  # bottom face


def shelf_z() -> float:
    """Where each board screw's head bears; its counterbore runs from the back up to here."""
    return board.STANDOFF_END_Z - p.SCREW_SHELF


def wide_post_top() -> float:
    """Below this the screw posts are wide, around the screw heads."""
    return shelf_z() + 0.6


def footprint(w: float, length: float, x: float, y: float, angle: float) -> Poly:
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


def ceiling(outline: Poly, plugs: bool) -> float:
    """The lowest board part over `outline`."""
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
    """Clear of the (wide) screw posts and inside the bore."""
    if any(math.hypot(cx, cy) > BORE_R - 0.3 for cx, cy in footprint(w, length, x, y, angle)):
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

    Ties go to the spot nearest the middle, over the speaker.
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
