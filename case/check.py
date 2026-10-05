"""Fit check against Waveshare's STEP model of the board: `uv run check.py`.

Exits non-zero if a printed part overlaps the board, the board can't be lowered in, a button
misses its switch, the battery collides, or the recommended screws would reach the display.
"""

import math
import sys

from build123d import Box, Pos

import board
import params as p
import parts

OVERLAP = 0.01  # mm^3 of overlap that still counts as touching
PCB_FRONT_Z = board.PCB_BACK_Z + 1.2
SWITCH_TRAVEL = 0.25

failures = []


def report(ok: bool, what: str):
    print(("  ok    " if ok else "  FAIL  ") + what)
    if not ok:
        failures.append(what)


def where(shape) -> str:
    c = shape.bounding_box().center()
    angle = math.degrees(math.atan2(c.Y, c.X))
    return f"{angle:5.0f} deg, r {math.hypot(c.X, c.Y):4.1f}, z {c.Z:5.1f}"


def overlaps(cutter, solids) -> list:
    hits = []
    box = cutter.bounding_box()
    for s in solids:
        if not box.overlaps(s.bounding_box()):
            continue
        common = cutter & s
        if common.volume > OVERLAP:
            hits.append((s, common.volume))
    return hits


def edge_solids(solids) -> list:
    """Board parts that come anywhere near the case: the rim, the floor or the posts."""
    near = []
    for s in solids:
        b = s.bounding_box()
        r = max(math.hypot(x, y) for x in (b.min.X, b.max.X) for y in (b.min.Y, b.max.Y))
        if r > 21.5 or b.min.Z < board.STANDOFF_END_Z + 0.4:
            near.append(s)
    return near


def check_variant(battery: bool, solids: list):
    name = "battery case" if battery else "USB case"
    print(f"\n{name}")
    case = parts.case(battery)
    near = edge_solids(solids)

    hits = overlaps(case, near)
    report(
        not hits,
        "case clears the board"
        + "".join(f"\n        hits {where(s)} ({v:.2f} mm3)" for s, v in hits),
    )

    rest = parts.button()
    report(not overlaps(rest, near), "buttons at rest clear the board")
    report(
        (rest & case).volume < OVERLAP,
        "buttons at rest sit in their holes without overlapping the case",
    )

    lifted_ok = True
    for dz in (0.3, 0.8, 1.5, 3, 5, 8, 12, 16):
        lifted = [Pos(0, 0, dz) * s for s in near]
        if overlaps(case, lifted) or overlaps(rest, lifted):
            lifted_ok = False
            print(f"        board catches when {dz} mm above its seat")
    report(lifted_ok, "board lowers straight in, past the buttons")

    touch = parts.button(pressed=p.BUTTON_ACTUATOR_GAP + 0.1)
    keys = [s for s, _ in overlaps(touch, near)]
    report(len(keys) >= 2, f"each button lands on its switch ({len(keys)} parts touched)")
    full = parts.button(pressed=p.BUTTON_ACTUATOR_GAP + SWITCH_TRAVEL)
    others = [s for s, _ in overlaps(full, near) if s not in keys]
    report(not others, "a full press touches nothing but the switches")
    report((full & case).volume < OVERLAP, "a full press doesn't bind in the case")

    for item, box in parts.item_boxes(battery):
        size = box.bounding_box().size
        dims = f"{size.X:g} x {size.Y:g} x {size.Z:g}"
        report((box & case).volume < OVERLAP, f"{item} ({dims}) clears the posts and rails")
        report(not overlaps(box, solids), f"{item} ({dims}) clears the board")

    if parts.contents(battery):
        m = 0.5  # the two wires splay a little as they leave the plug
        plugs = [
            Pos((x0 + x1) / 2, (y0 + y1) / 2, board.SOCKETS_Z - p.PLUG_ROOM)
            * Box(x1 - x0 + 2 * m, y1 - y0 + 2 * m, p.PLUG_ROOM, align=parts.BOTTOM)
            for x0, y0, x1, y1 in board.PLUG_ENTRIES
        ]
        report(all((plug & case).volume < OVERLAP for plug in plugs), "room for the BAT/SPK plugs")

    if not battery:  # the rim is the same in both cases, and distances are slow
        gaps = sorted(((case.distance_to(s), s) for s in near), key=lambda g: g[0])
        print("  closest board parts (gap mm, where; the standoffs sit on the posts):")
        for gap, s in gaps[:6]:
            print(f"        {gap:5.2f}  {where(s)}")


def check_screws():
    print("\nboard screws (M2, head in the counterbore)")
    for length in (4, 5, 6, 8):
        thread = length - p.SCREW_SHELF
        tip = board.STANDOFF_END_Z + thread
        ok = thread >= 2.0 and tip <= PCB_FRONT_Z - 0.3
        why = f"{thread:.1f} mm in the standoff, tip {PCB_FRONT_Z - tip:+.1f} mm from the PCB front"
        print(f"  {'ok  ' if ok else 'no  '}  M2x{length}: {why}")
    m2x6 = 6 - p.SCREW_SHELF
    report(2.0 <= m2x6 and board.STANDOFF_END_Z + m2x6 <= PCB_FRONT_Z - 0.3, "M2x6 is safe")


def main():
    print("loading the board STEP ...")
    solids = board.load().solids()
    for battery in (False, True):
        check_variant(battery, solids)
    check_screws()
    print()
    if failures:
        print(f"{len(failures)} check(s) failed")
        sys.exit(1)
    print("all checks passed")


if __name__ == "__main__":
    main()
