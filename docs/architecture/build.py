#!/usr/bin/env python3
"""The architecture slides for posts and devlogs: HTML placed in px, shot to 1920x1080 PNGs by
headless Chrome (Google Chrome in /Applications).

    python3 docs/architecture/build.py                      # a PNG per board in BOARDS
    python3 docs/architecture/build.py --canvas DIR BLOBS   # the same boards as .dc.html

Placing everything in px keeps the PNGs exact and repeatable. `--canvas` writes each board into
DIR as a Design canvas artboard, with images pointing at the uploaded copies named in BLOBS
(JSON: repo path -> /_blob/ url).
"""

import json
import subprocess
import sys
import tempfile
from html import escape
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"

W, H = 1920, 1080
BG = "#070a12"
INK = "#e9eef8"
SOFT = "#c9d3e3"
MUTED = "#9aa9c2"

YELLOW = (255, 212, 71)
PINK = (255, 140, 198)
TEAL = (95, 227, 200)
CYAN = (79, 200, 245)
GREEN = (142, 224, 138)
ORANGE = (255, 164, 92)
BLUE = (127, 166, 255)
VIOLET = (195, 166, 255)
SAND = (216, 192, 140)
WHITE = (255, 255, 255)

SANS = "'IBM Plex Sans', sans-serif"
MONO = "'JetBrains Mono', monospace"
PIXEL = "'Silkscreen', monospace"
FONTS = (
    "https://fonts.googleapis.com/css2?family=Silkscreen"
    "&family=JetBrains+Mono:wght@500;700"
    "&family=IBM+Plex+Sans:ital,wght@0,400;0,500;0,600;1,400&display=block"
)


def rgb(c, a=1.0):
    """CSS colour for an (r, g, b) tuple, rgba when `a` is below 1."""
    if a >= 1:
        return f"rgb({c[0]}, {c[1]}, {c[2]})"
    return f"rgba({c[0]}, {c[1]}, {c[2]}, {a})"


def css(**props):
    """Keyword arguments as inline CSS, underscores turned into hyphens."""
    return "; ".join(f"{k.replace('_', '-')}: {v}" for k, v in props.items())


def span(inner, **props):
    return f'<span style="{css(**props)}">{inner}</span>'


def mono(name, hue, size=18):
    """A name in bold monospace, coloured."""
    return span(name, font_family=MONO, font_weight=700, font_size=f"{size}px", color=rgb(hue))


def tag(name, hue):
    """A section tag in the pixel font."""
    return span(name, font_family=PIXEL, font_size="16px", color=rgb(hue))


class Board:
    """One diagram. Elements paint in layer order: bands, lines, cards, labels."""

    def __init__(self, title):
        self.title = title
        self.layers = {"bands": [], "lines": [], "cards": [], "labels": []}
        self.markers = 0

    def place(self, layer, x, y, inner="", **props):
        """An absolutely placed div. With x None, pass `right` instead of a left edge."""
        at = {"left": f"{x}px"} if x is not None else {}
        style = css(position="absolute", **at, top=f"{y}px", **props)
        self.layers[layer].append(f'<div style="{style}">{inner}</div>')

    def text(self, x, y, inner, size=16, color=INK, font=SANS, weight=400, line=None, **props):
        """A label. It wraps only when given a `width`."""
        self.place(
            "labels",
            x,
            y,
            inner,
            font_family=font,
            font_size=f"{size}px",
            line_height=f"{line or round(size * 1.3)}px",
            font_weight=weight,
            color=color,
            white_space="normal" if "width" in props else "nowrap",
            **props,
        )

    def card(self, x, y, w, h, inner, hue, fill=0.09, edge=0.55, radius=12, pad="0 18px", align="center", **props):
        """A rounded flex box, tinted and outlined in `hue`."""
        self.place(
            "cards",
            x,
            y,
            inner,
            width=f"{w}px",
            height=f"{h}px",
            box_sizing="border-box",
            padding=pad,
            display="flex",
            align_items=align,
            gap="12px",
            background=rgb(hue, fill),
            border=f"1.5px solid {rgb(hue, edge)}",
            border_radius=f"{radius}px",
            **props,
        )

    def band(self, x, y, w, h, hue=WHITE, fill=0.03, edge=0.12, radius=20):
        """A faint panel behind a group of cards."""
        self.place(
            "bands",
            x,
            y,
            width=f"{w}px",
            height=f"{h}px",
            box_sizing="border-box",
            background=rgb(hue, fill),
            border=f"1px solid {rgb(hue, edge)}",
            border_radius=f"{radius}px",
        )

    def image(self, x, y, w, h, src, alt, **props):
        """An image by repo path, left as `{{path}}` for `body` to turn into a URL."""
        style = css(position="absolute", left=f"{x}px", top=f"{y}px", width=f"{w}px", height=f"{h}px", **props)
        self.layers["cards"].append(f'<img src="{{{{{src}}}}}" alt="{alt}" style="{style}">')

    def marker(self, color, orient):
        """A filled arrowhead <marker> with an id unique on this board."""
        self.markers += 1
        mid = f"dc-arrow-head-filled-{self.markers}"
        head = (
            f'<marker id="{mid}" orient="{orient}" markerWidth="5" markerHeight="5" '
            f'refX="2" refY="2" overflow="visible"><path d="M0 0 L4 2 L0 4 Z" '
            f'fill="{color}" stroke="none" style="stroke: none; fill: {color}; '
            f'fill: context-stroke"/></marker>'
        )
        return mid, head

    def arrow(self, x1, y1, x2, y2, hue, heads="end", width=2, dash=None, alpha=0.85):
        """A straight horizontal or vertical segment from (x1, y1) to its tip (x2, y2)."""
        color = rgb(hue, alpha)
        flat = y1 == y2
        if flat:
            left, top, n = min(x1, x2), y1 - 4, abs(x2 - x1)
            sw, sh, forward = n, 8, x2 > x1
        else:
            left, top, n = x1 - 4, min(y1, y2), abs(y2 - y1)
            sw, sh, forward = 8, n, y2 > y1
        tail, tip = (0, n) if forward else (n, 0)
        step = 1 if forward else -1
        defs, ends = "", ""
        if heads in ("end", "both"):
            tip -= step * 2 * width
            mid, head = self.marker(color, "auto")
            defs += head
            ends += f' marker-end="url(#{mid})"'
        if heads == "both":
            tail += step * 2 * width
            mid, head = self.marker(color, "auto-start-reverse")
            defs += head
            ends += f' marker-start="url(#{mid})"'
        d = f"M {tail} 4 L {tip} 4" if flat else f"M 4 {tail} L 4 {tip}"
        style = css(
            position="absolute",
            left=f"{left}px",
            top=f"{top}px",
            width=f"{sw}px",
            height=f"{sh}px",
            overflow="visible",
            fill="none",
            stroke=color,
            stroke_width=width,
            stroke_linecap="round",
            stroke_linejoin="round",
            **({"stroke_dasharray": dash} if dash else {}),
        )
        self.layers["lines"].append(
            f'<svg width="{sw}" height="{sh}" viewBox="0 0 {sw} {sh}" '
            f'preserveAspectRatio="none" style="{style}">'
            + (f"<defs>{defs}</defs>" if defs else "")
            + f'<path d="{d}"{ends}></path></svg>'
        )

    def body(self, src):
        """The root element with every image path passed through `src`."""
        root = css(
            position="relative",
            width=f"{W}px",
            height=f"{H}px",
            overflow="hidden",
            background=BG,
            color=INK,
            font_family=SANS,
        )
        inner = "\n".join(
            self.layers["bands"] + self.layers["lines"] + self.layers["cards"] + self.layers["labels"]
        )
        for path in {p.split("}}")[0] for p in inner.split("{{")[1:]}:
            inner = inner.replace(f"{{{{{path}}}}}", src(path))
        return f'<div style="{root}">\n{inner}\n</div>'

    def page(self):
        """The board as a page for Chrome, its images as file:// URLs."""
        return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{escape(self.title)}</title>
<link rel="stylesheet" href="{escape(FONTS)}">
<style>body{{margin:0;background:{BG}}}</style>
</head>
<body>
{self.body(lambda path: (REPO / path).as_uri())}
</body>
</html>
"""

    def artboard(self, blobs):
        """The board as a Design canvas artboard, its images as the uploaded blob URLs."""
        props = json.dumps({"$preview": {"width": W, "height": H}})
        return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{escape(self.title)}</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="{escape(FONTS)}">
<style>body{{margin:0}}</style>
</helmet>
{self.body(lambda path: blobs[path])}
</x-dc>
<script type="text/x-dc" data-dc-script data-props='{props}'>
class Component extends DCLogic {{
renderVals() {{
return {{}};
}}
}}
</script>
</body>
</html>
"""


def header(b, title, subtitle, chips):
    """Every board's top: FISHING MINIGAME and its title, a subtitle, and chips at the right."""
    b.text(
        64,
        34,
        span("FISHING MINIGAME", font_family=PIXEL, font_size="52px", color=rgb(YELLOW))
        + span(title, font_size="38px", font_weight=500, margin_left="24px"),
        line=76,
    )
    b.text(64, 116, subtitle, size=20, color=MUTED, line=28)
    pills = "".join(
        span(chip, border=f"1px solid {rgb(WHITE, 0.18)}", border_radius="999px", padding="6px 14px")
        for chip in chips
    )
    b.text(None, 52, pills, size=15, color=SOFT, font=MONO, right="64px", display="flex", gap="10px")


# The places on the town's street, left to right: (centre x, screenshot, place, what, crate).
# The two games that don't use the pals crate (Skate, Orbit) sit at the ends.
PLACES = [
    (152, "skate_halfpipe_360", "arcade", "Skate", "skate_game"),
    (322, "bees_orbit", "humming tree", "Bees???", "bees_game"),
    (492, "spider_spin", "den", "Spider Roulette", "spider_game"),
    (662, "angler_caught", "pond", "Angler", "angler_game"),
    (832, "town_home", "town", "the hub", None),
    (1002, "home_tank", "house", "tank, tricks", None),
    (1172, "park_stepping", "park", "step walks", None),
    (1342, "contest_trick", "contest hall", "trick shows", None),
    (1512, "shop_crown", "hat shop", "12 hats", None),
    (1768, "orbit_playing", "Orbit", "own binary", "orbit_game"),
]

# Each plain library and what feeds it on the board and in the simulator:
# (library, what it does, board, desktop, which way data flows, what crosses).
BACKENDS = [
    ("orbit_motion", "IMU fusion (Mahony) to steer and lean", "QMI8658 IMU<br>I2C 0x6B", "arrow keys tip<br>a virtual IMU", "up", "IMU samples"),
    ("orbit_gfx", "rasterizer, dirty rects, 8-row bands", "CO5300 AMOLED<br>40 MHz QSPI", "minifb window<br>PNG snapshots", "down", "pixels"),
    ("orbit_audio", "16 kHz mixer and voice prints", "ES8311 speaker<br>ES7210 mics", "laptop speaker<br>and mic", "both", "PCM"),
    ("orbit_save", "power-fail-safe flash records", "SPI flash<br>two sectors", "~/.logic-orbit/<br>*.flash file", "both", "save records"),
    (None, None, "CST9217 touch<br>two buttons<br>AXP2101 battery<br>IMU pedometer", "mouse<br>Space and Z<br>W walks", "core", None),
]


def crate_label(b, x, w, crate, label_w):
    """A bracket under x..x+w with the crate's name centred below it."""
    b.place(
        "lines",
        x,
        430,
        width=f"{w}px",
        height="8px",
        box_sizing="border-box",
        border=f"1.5px solid {rgb(YELLOW, 0.5)}",
        border_top="none",
        border_radius="0 0 4px 4px",
    )
    left = x + (w - label_w) // 2
    b.text(left, 444, crate, size=15, color=rgb(YELLOW), font=MONO, weight=500, line=20, width=f"{label_w}px", text_align="center")


def overview():
    """How it's built: the town's places, the crates under them, and the no_std libraries."""
    b = Board("Fishing Minigame: how it's built")
    header(
        b,
        "how it's built",
        "One no_std Rust codebase on Bevy 0.19. It runs on a round ESP32-S3 board, and in a "
        "desktop simulator that draws the exact same pixels.",
        ["ESP32-S3", "466×466 round AMOLED", "≈25k lines of Rust", "337 tests"],
    )

    b.band(64, 168, 1792, 512)
    b.text(100, 186, tag("CRATES/", (184, 194, 214)) + "&nbsp; one Bevy 0.19 app · no_std, on one thread", size=15, color=MUTED, line=20)
    b.text(
        None,
        186,
        "each door swaps whole apps: only one is in memory at a time",
        size=15,
        color=MUTED,
        line=20,
        font_style="italic",
        right="100px",
    )

    b.arrow(152, 300, 1512, 300, SAND, heads=None, width=6, dash="0.1 14", alpha=0.6)
    b.arrow(1640, 248, 1640, 470, WHITE, heads=None, width=1.5, dash="4 6", alpha=0.18)
    for cx, shot, place, what, crate in PLACES:
        d = 140 if place == "town" else 104
        ring = (
            f"0 0 0 3px {rgb(YELLOW)}, 0 0 36px {rgb(YELLOW, 0.35)}"
            if place == "town"
            else f"0 0 0 2px {rgb(WHITE, 0.22)}"
        )
        b.image(cx - d // 2, 300 - d // 2, d, d, f"screenshots/{shot}.png", f"the {place}", border_radius="50%", box_shadow=ring)
        b.text(cx - 80, 378, place, size=17, weight=600, line=22, width="160px", text_align="center")
        b.text(cx - 80, 400, what, size=15, color=MUTED, line=20, width="160px", text_align="center")
        if crate:
            crate_label(b, cx - 52, 104, crate, 160)
    crate_label(b, 762, 802, "pals_game", 160)

    b.card(
        270,
        482,
        1294,
        48,
        span(mono("pals", PINK) + "&nbsp;&nbsp; the rules: pets, tricks, voice and stroke commands, walks, gold, hats")
        + span("save section PALS", font_family=MONO, font_size="14px", color=MUTED),
        PINK,
        justify_content="space-between",
        font_size="16px",
    )
    b.card(
        100,
        540,
        1464,
        48,
        span(mono("orbit_ui", TEAL) + "&nbsp;&nbsp; shared drawing: turn-to-pick dials, buttons, battery, typing by turning"),
        TEAL,
        fill=0.07,
        font_size="16px",
    )
    b.card(
        100,
        598,
        1720,
        64,
        span(mono("orbit_core", CYAN, 20) + "&nbsp;&nbsp; Bevy plumbing: the Frame, touch, buttons, tilt, power and sleep, sound, mic, steps, saves")
        + span("CorePlugin + ControlsPlugin", font_family=MONO, font_size="14px", color=MUTED),
        CYAN,
        fill=0.12,
        justify_content="space-between",
        font_size="16px",
    )

    b.text(100, 696, tag("NO_STD LIBRARIES", GREEN) + "&nbsp; plain Rust, fed by the board or the simulator", size=15, color=MUTED, line=20)
    for i, (lib, what, board, desk, flow, crossing) in enumerate(BACKENDS):
        left = 100 + i * 349
        if lib:
            b.card(
                left,
                728,
                324,
                80,
                span(mono(lib, GREEN) + "<br>" + what, line_height="21px"),
                GREEN,
                fill=0.07,
                pad="8px 18px",
                font_size="15px",
                color=SOFT,
            )
        for x, hue in ((left + 75, ORANGE), (left + 249, BLUE)):
            if flow == "up":
                b.arrow(x, 848, x, 809, hue)
            elif flow == "down":
                b.arrow(x, 808, x, 847, hue)
            elif flow == "both":
                b.arrow(x, 808, x, 847, hue, heads="both")
            else:
                b.arrow(x, 848, x, 663, hue)
        if crossing:
            b.text(left + 87, 820, crossing, size=13, color=MUTED, line=16, width="150px", text_align="center")
        else:
            b.text(
                left + 87,
                748,
                "straight into<br>" + span("orbit_core", font_family=MONO, color=rgb(CYAN)),
                size=15,
                color=MUTED,
                line=20,
                width="150px",
                text_align="center",
            )
        for x, text, hue in ((left, board, ORANGE), (left + 174, desk, BLUE)):
            b.card(
                x,
                848,
                150,
                84,
                span(text),
                hue,
                fill=0.10,
                radius=10,
                pad="0 8px",
                justify_content="center",
                text_align="center",
                font_size="14px",
                line_height="18px",
            )

    b.card(
        100,
        948,
        848,
        64,
        span(
            mono("firmware/", ORANGE, 17)
            + "&nbsp; the ESP32-S3 board: esp-hal drivers, release builds<br>"
            + span("--bin pals · angler · spider · skate · bees · orbit", font_family=MONO, font_size="13px", color=MUTED),
            line_height="22px",
        ),
        ORANGE,
        fill=0.08,
        font_size="16px",
    )
    b.card(
        972,
        948,
        848,
        64,
        span(
            mono("desktop/", BLUE, 17)
            + "&nbsp; the std simulator: same renderer, same pixels<br>"
            + span("cargo run -p orbit_desktop · snapshots · trailer footage", font_family=MONO, font_size="13px", color=MUTED),
            line_height="22px",
        ),
        BLUE,
        fill=0.08,
        font_size="16px",
    )
    return b


# The pipeline left to right: (left, name, what happens, colour of its crate, crate).
STAGES = [
    (64, "app.update()", "The platform writes touch, tilt and buttons into resources, then the game's systems run.", CYAN, "games + orbit_core"),
    (444, "Frame", "Systems push shapes: Disc, Ring, Arc, Label, Line, Rect, Sprite. Nothing is drawn yet.", CYAN, "orbit_core"),
    (824, "FrameDiff", "Shapes that appeared or vanished since the last frame. Their bounds, even-aligned and merged, are the dirty rects.", GREEN, "orbit_gfx"),
    (1204, "bands", "Each dirty rect is cut into 8-row bands, and every shape over a band is drawn into one 7.5 KB buffer.", GREEN, "orbit_gfx"),
]


def pipeline():
    """How a frame gets to the glass: four stages, dirty rects, bytes per frame, ping-pong DMA."""
    b = Board("Fishing Minigame: how a frame gets to the glass")
    header(
        b,
        "how a frame gets to the glass",
        "Each frame, Bevy hands over a list of shapes; only what changed is drawn, "
        "8 rows at a time, and sent while the next band draws.",
        ["40 MHz quad SPI", "RGB565", "466×466", "8-row bands"],
    )

    for i, (left, name, what, hue, crate) in enumerate(STAGES):
        b.card(
            left,
            200,
            272,
            152,
            span(mono(name, hue, 20) + "<br>" + what, line_height="21px"),
            hue,
            fill=0.08,
            pad="16px 18px",
            align="flex-start",
            font_size="15px",
            color=SOFT,
        )
        b.text(left + 2, 364, crate, size=13, color=rgb(hue), font=MONO, weight=500, line=18)
        if i < 3:
            b.arrow(left + 272, 276, left + 379, 276, WHITE, alpha=0.5)
            label = ["shapes", "this + last", "dirty rects"][i]
            b.text(left + 280, 248, label, size=13, color=MUTED, line=16, width="92px", text_align="center")

    b.arrow(1476, 276, 1530, 276, WHITE, heads=None, alpha=0.5)
    b.arrow(1530, 236, 1530, 316, WHITE, heads=None, alpha=0.5)
    b.arrow(1530, 236, 1583, 236, WHITE, alpha=0.5)
    b.arrow(1530, 316, 1583, 316, WHITE, alpha=0.5)
    b.text(1494, 208, "RGB565", size=13, color=MUTED, line=16, width="72px", text_align="center")
    b.card(1584, 200, 272, 72, span(mono("CO5300 panel", ORANGE) + "<br>40 MHz quad SPI, DMA", line_height="21px"), ORANGE, fill=0.10, font_size="15px", color=SOFT)
    b.card(1584, 280, 272, 72, span(mono("desktop window", BLUE) + "<br>a RAM model of the panel", line_height="21px"), BLUE, fill=0.10, font_size="15px", color=SOFT)
    b.text(1586, 364, "Panel: firmware/ or desktop/", size=13, color=MUTED, font=MONO, weight=500, line=18)

    # Dirty rects on a real frame. The Orbit screenshot is 466 px; it's shown at 320.
    b.band(64, 440, 560, 504)
    b.text(88, 460, tag("DIRTY RECTS", YELLOW), line=20)
    b.text(88, 484, "Only what moved is redrawn", size=22, weight=600, line=28)
    b.image(184, 530, 320, 320, "screenshots/orbit_playing.png", "Orbit mid-rally", border_radius="50%", box_shadow=f"0 0 0 2px {rgb(WHITE, 0.22)}")
    for x, y, w, h in ((369, 643, 24, 24), (214, 764, 107, 78)):
        b.place("cards", x, y, width=f"{w}px", height=f"{h}px", box_sizing="border-box", border=f"2px dashed {rgb(YELLOW)}", border_radius="3px")
    b.text(400, 645, "ball", size=13, color=rgb(YELLOW), line=16, font=MONO)
    b.text(331, 795, "paddle", size=13, color=rgb(YELLOW), line=16, font=MONO)
    b.text(
        88,
        868,
        "One frame later only Orbit's ball and paddle have moved, so only their boxes are drawn and sent. "
        "(Boxes drawn by hand.)",
        size=15,
        color=SOFT,
        line=20,
        width="512px",
    )

    b.band(672, 440, 560, 504)
    b.text(696, 460, tag("BYTES PER FRAME", GREEN), line=20)
    b.text(696, 484, "A normal frame is about 25 KB", size=22, weight=600, line=28)
    b.text(696, 540, "a gameplay frame", size=15, color=SOFT, line=20)
    b.place("cards", 696, 566, width="28px", height="32px", background=rgb(GREEN, 0.9), border_radius="4px")
    b.text(736, 571, "≈25 KB · 1.3 ms", size=15, font=MONO, weight=500, line=22)
    b.text(696, 622, "a full redraw, only when the screen changes", size=15, color=SOFT, line=20)
    b.place("cards", 696, 648, width="480px", height="32px", background=rgb(WHITE, 0.28), border_radius="4px")
    b.text(696, 688, "434 KB · 22 ms", size=15, font=MONO, weight=500, line=22)
    b.text(696, 736, "17×", size=72, color=rgb(YELLOW), font=PIXEL, line=80)
    b.text(696, 818, "less to send on a normal frame", size=18, line=24)
    b.text(696, 860, "40 MHz on four lines is about 20 MB a second.", size=15, color=SOFT, line=20, width="512px")

    b.band(1280, 440, 576, 504)
    b.text(1304, 460, tag("PING-PONG DMA", ORANGE), line=20)
    b.text(1304, 484, "The next band draws while this one sends", size=22, weight=600, line=28)
    b.text(1304, 550, "CPU draws", size=15, color=SOFT, line=20)
    b.text(1304, 610, "QSPI sends", size=15, color=SOFT, line=20)
    for i in range(5):
        for x, y, hue in ((1424 + i * 68, 540, GREEN), (1424 + (i + 1) * 68, 600, ORANGE)):
            b.card(x, y, 60, 40, span(f"{i + 1}"), hue, fill=0.18, edge=0.7, radius=8, pad="0", justify_content="center", font_family=MONO, font_size="15px")
    b.arrow(1424, 660, 1824, 660, WHITE, alpha=0.3)
    b.text(1424, 670, "time, one band at a time", size=13, color=MUTED, line=16)
    b.card(1304, 720, 176, 84, span(mono("band buffer", GREEN, 16) + "<br>7,456 B in SRAM", line_height="21px"), GREEN, fill=0.08, pad="0 14px", font_size="14px", color=SOFT)
    b.arrow(1480, 762, 1519, 762, WHITE, alpha=0.5)
    b.card(1520, 720, 176, 84, span(mono("DMA A, B", ORANGE, 16) + "<br>7,680 B each", line_height="21px"), ORANGE, fill=0.08, pad="0 14px", font_size="14px", color=SOFT)
    b.arrow(1696, 762, 1735, 762, WHITE, alpha=0.5)
    b.card(1736, 720, 96, 84, span(mono("CO5300", ORANGE, 16) + "<br>panel", line_height="21px"), ORANGE, fill=0.08, pad="0 14px", font_size="14px", color=SOFT)
    b.text(
        1304,
        832,
        "A band's bytes go into whichever DMA buffer is idle while the other is on the wire. "
        "(Timing is schematic.)",
        size=15,
        color=SOFT,
        line=20,
        width="528px",
    )

    b.card(
        64,
        960,
        1792,
        56,
        span(
            span("panel_ram_always_matches_a_full_render_of_the_latest_frame", font_family=MONO, font_weight=500, color=rgb(GREEN))
            + "&nbsp;&nbsp; plays 40 frames through this path and checks that the panel's RAM equals a full render, every time."
        ),
        WHITE,
        fill=0.03,
        edge=0.12,
        font_size="15px",
        color=SOFT,
    )
    return b


# Each line between the hardware and a game, through orbit_core: (the board's chip, how the
# board feeds or drains it, the resource, what it holds, its plugin, what a game calls).
INPUTS = [
    ("CST9217 touch · I2C 0x5A", "rotated 180°, lifted after 2 empty reads", "Touch", "the finger, in screen px", "CorePlugin", "touch.position()<br>touch.just_pressed()"),
    ("PWR key · TCA9554 at 0x20", "BOOT on GPIO0", "Buttons", "two switches, by role", "ControlsPlugin", "buttons.action.pressed()<br>buttons.zero.held()"),
    ("QMI8658 IMU · I2C 0x6B", "accel and gyro, read every frame", "ImuFeed → Tilt", "samples in, degrees out (Mahony)", "ControlsPlugin", "tilt.steer_deg<br>tilt.lean_deg"),
    ("AXP2101 gauge · I2C 0x34", "read every 5 s; idle dims, a doze darkens", "Power", "battery, idle, sleep, doze", "ControlsPlugin", "power.battery<br>power.set_dozing(true)"),
    ("ES7210 mics · I2C 0x40", "data on the speaker's I2S clocks", "Mic", "16 kHz samples in, words out", "MicPlugin", "mic.take() → VoicePrint<br>mic.level()"),
    ("QMI8658 pedometer · I2C 0x6B", "its count read every frame or doze wake", "Steps", "new steps since the last look", "StepsPlugin", "steps.take()"),
]
OUTPUTS = [
    ("CO5300 AMOLED · QSPI 40 MHz", "Presenter sends dirty rects, 2 DMA buffers", "Frame", "the display list, back to front", "CorePlugin", "frame.shapes.push(shape)"),
    ("ES8311 speaker · I2C 0x18, I2S", "one-voice Mixer, amp on GPIO46", "SoundQueue", "play and stop; PREF mutes it", "SoundPlugin", "sound.play(&amp;WIN)"),
    ("flash at 0x9000", "two sectors take turns, CRC32", "SaveBlob", "one record, a section per app", "CorePlugin", "blob.save_section(KEY, encode(pals))"),
]

# One pass of the firmware's frame loop: (step, width).
LOOP = [
    ("feed the inputs", 156),
    ("app.update()", 140),
    ("sounds to the mixer", 188),
    ("write the save if it changed", 262),
    ("present the Frame", 172),
    ("sleep if asked", 148),
    ("halt until the next 16.7 ms", 254),
]


def core():
    """orbit_core meets the hardware: a row per channel from chip to resource to game call."""
    b = Board("Fishing Minigame: orbit_core meets the hardware")
    header(
        b,
        "orbit_core meets the hardware",
        "Games only see Bevy resources. Each frame the platform writes the inputs into them, "
        "runs app.update(), then drains the outputs.",
        ["5 plugins, 4 opt-in", "7 chips on one I2C bus", "60 fps, 20 when idle"],
    )

    b.band(720, 168, 512, 708, CYAN, fill=0.05, edge=0.3)
    b.text(64, 184, tag("ON THE BOARD", ORANGE) + "&nbsp; firmware/", size=15, color=MUTED, line=20)
    b.text(744, 184, tag("ORBIT_CORE", CYAN) + "&nbsp; Bevy resources", size=15, color=MUTED, line=20)
    b.text(1288, 184, tag("YOUR GAME", YELLOW) + "&nbsp; calls", size=15, color=MUTED, line=20)
    b.text(64, 214, "INPUTS: the platform writes these, then calls app.update()", size=14, color=MUTED, line=18, font_style="italic")
    b.text(64, 644, "OUTPUTS: after app.update(), the platform drains these", size=14, color=MUTED, line=18, font_style="italic")

    rows = [(240 + i * 66, row, True) for i, row in enumerate(INPUTS)]
    rows += [(672 + i * 66, row, False) for i, row in enumerate(OUTPUTS)]
    for top, (chip, detail, resource, holds, plugin, call), incoming in rows:
        y = top + 28
        b.card(
            64,
            top,
            600,
            56,
            span(mono(chip, ORANGE, 17) + "<br>" + span(detail, font_size="14px", color=MUTED), line_height="21px"),
            ORANGE,
            fill=0.08,
            edge=0.45,
            font_size="17px",
        )
        b.card(
            736,
            top,
            480,
            56,
            span(mono(resource, CYAN, 19) + "<br>" + holds, line_height="21px")
            + span(plugin, font_family=MONO, font_size="13px", color=MUTED),
            CYAN,
            fill=0.12,
            justify_content="space-between",
            font_size="15px",
            color=SOFT,
        )
        b.card(1288, top, 568, 56, span(call), YELLOW, fill=0.06, edge=0.4, font_family=MONO, font_size="16px", line_height="21px")
        if incoming:
            b.arrow(664, y, 735, y, ORANGE)
            b.arrow(1216, y, 1287, y, WHITE, alpha=0.5)
        else:
            b.arrow(1288, y, 1217, y, WHITE, alpha=0.5)
            b.arrow(736, y, 665, y, ORANGE)

    b.band(64, 900, 1792, 116)
    b.text(88, 914, tag("EVERY FRAME ON THE BOARD", ORANGE) + "&nbsp; firmware/src/lib.rs", size=15, color=MUTED, line=20)
    left = 88
    for i, (step, w) in enumerate(LOOP):
        hue = YELLOW if step == "app.update()" else WHITE
        b.card(
            left,
            944,
            w,
            40,
            span(step),
            hue,
            fill=0.10 if hue == YELLOW else 0.05,
            edge=0.5 if hue == YELLOW else 0.25,
            radius=20,
            pad="0",
            justify_content="center",
            font_size="15px",
            **({"font_family": MONO} if hue == YELLOW else {}),
        )
        if i < len(LOOP) - 1:
            b.arrow(left + w, 964, left + w + 47, 964, WHITE, alpha=0.5)
        left += w + 48
    first, last = 88 + 156 // 2, left - 48 - 254 // 2
    b.arrow(last, 984, last, 1000, WHITE, heads=None, alpha=0.35)
    b.arrow(last, 1000, first, 1000, WHITE, heads=None, alpha=0.35)
    b.arrow(first, 1000, first, 985, WHITE, alpha=0.35)
    b.text(last + 12, 992, "next frame", size=13, color=MUTED, line=16)
    return b


# The trailer's shots, copied from trailer/edit.py: (start s, label, fill, text colour).
SHOTS = [
    (0.0, "TITLE", "#6fb7e8", "#0b1020"),
    (3.83, "FISH", "#2b9cf2", "#0b1020"),
    (7.06, "TRAIN", "#ff8a3d", "#0b1020"),
    (10.29, "WALK", "#43b45a", "#0b1020"),
    (13.52, "COMPETE", "#c2457e", "#ffffff"),
    (16.75, "HATS", "#f2b01e", "#0b1020"),
    (19.98, "", "#5b6274", "#ffffff"),
    (20.68, "BEESSSS????", "#ffd21f", "#0b1020"),
    (24.68, "SKATEBOARDS??", "#8a6cff", "#ffffff"),
    (29.18, "SPIDERS?!", "#d33a3a", "#ffffff"),
    (32.68, "YOU CAN EAT IT", "#ffd45c", "#0b1020"),
    (41.18, "HAPPY BIRTHDAY", "#6fb7e8", "#0b1020"),
]
TRAILER_END = 46.18


def at(t):
    """The x, in px, of trailer time t on the cut's timeline."""
    return round(200 + t * 1608 / TRAILER_END)


def trailer():
    """How the trailer was made: sources through to the MP4, and the cut on a timeline."""
    b = Board("Fishing Minigame: how the trailer was made")
    header(
        b,
        "how the trailer was made",
        "Filmed by the game itself: the desktop build plays scripted takes on game time and keeps every frame "
        "and sound; Python cuts them to the beat, and Blender eats the case.",
        ["14 takes in ~7 s", "130 BPM", "46 s at 1080p30", "~2 min to build"],
    )
    for x, name in ((64, "SOURCES"), (520, "PREPARED"), (1036, "CUT AND MIXED"), (1552, "ENCODED")):
        b.text(x, 180, tag(name, (184, 194, 214)), line=20)

    b.card(64, 216, 360, 104, span(mono("reel_case.stl", WHITE, 16) + "<br>the printable case, straight out of case/build.py", line_height="20px"), WHITE, fill=0.04, edge=0.25, font_size="15px", color=SOFT)
    b.card(
        520,
        216,
        420,
        104,
        span(mono("Blender · eat.py", ORANGE, 16) + "<br>headless EEVEE; MANIFOLD booleans take the bites in 0.2 s (EXACT took 30)", line_height="20px"),
        ORANGE,
        fill=0.08,
        pad="0 16px 0 164px",
        font_size="14px",
        color=SOFT,
    )
    b.image(536, 232, 128, 72, "devlog/img/37-you-can-eat-it.png", "the man eating the case", object_fit="cover", border_radius="8px")
    b.card(
        64,
        344,
        360,
        240,
        span(mono("orbit_desktop footage", BLUE, 16) + "<br>12 scripted takes. Each tick is 1/60 s of game time; every frame is kept as a PNG and every sound in a WAV.", line_height="20px"),
        BLUE,
        fill=0.08,
        pad="0 18px 14px",
        align="flex-end",
        font_size="15px",
        color=SOFT,
    )
    b.image(188, 360, 112, 112, "screenshots/contest_trick.png", "a game frame", border_radius="50%", box_shadow=f"0 0 0 2px {rgb(WHITE, 0.22)}")
    b.card(520, 432, 420, 72, span(mono("edit.py", YELLOW, 16) + "<br>the cut list: 130 BPM, 7 beats a shot", line_height="20px"), YELLOW, fill=0.08, font_size="15px", color=SOFT)
    b.card(64, 608, 360, 80, span(mono("music", WHITE, 16) + "<br>Happy Whistling Ukulele (CC0), Flight of the Bumblebee (public domain)", line_height="19px"), WHITE, fill=0.04, edge=0.25, font_size="14px", color=SOFT)
    b.card(
        1036,
        216,
        420,
        248,
        span(
            mono("build.py · Trailer.frame(t)", GREEN, 16)
            + "<br>Pillow and numpy draw every output frame: the reel case at 3× around the game, "
            "striped backdrops, tilts, zooms and shake, and letters that pop in one at a time.",
            line_height="21px",
        ),
        GREEN,
        fill=0.08,
        font_size="15px",
        color=SOFT,
    )
    b.card(
        1036,
        476,
        420,
        212,
        span(
            mono("mix() → mix.wav", GREEN, 16)
            + "<br>the takes' own sounds at −5 dB under the music, plus a record scratch, the dive's whoosh "
            "and the bites' crunches, all made in numpy. 48 kHz stereo.",
            line_height="21px",
        ),
        GREEN,
        fill=0.08,
        font_size="15px",
        color=SOFT,
    )
    b.card(
        1552,
        216,
        304,
        472,
        span(
            span("46.2 s", font_family=PIXEL, font_size="40px", line_height="48px", color=rgb(YELLOW))
            + "<br>1920×1080 · 30 fps · 28.5 MB<br>"
            + span("fishing_minigame_trailer.mp4", font_family=MONO, font_size="13px", color=INK)
            + "<br><br>"
            + mono("ffmpeg", WHITE, 16)
            + "<br>libx264 CRF 17, AAC 192k, loudness to −15 LUFS",
            line_height="20px",
        ),
        WHITE,
        fill=0.05,
        edge=0.3,
        pad="184px 16px 0",
        align="flex-start",
        font_size="14px",
        color=SOFT,
    )
    b.image(1568, 232, 272, 153, "devlog/img/37-trailer-fish.png", "a frame of the trailer", object_fit="cover", border_radius="8px")

    b.arrow(424, 268, 519, 268, WHITE, alpha=0.5)
    b.text(432, 240, "the STL", size=13, color=MUTED, line=16, width="80px", text_align="center")
    b.arrow(940, 268, 1035, 268, ORANGE)
    b.text(948, 240, "255 frames", size=13, color=MUTED, line=16, width="80px", text_align="center")
    b.arrow(424, 384, 1035, 384, BLUE)
    b.text(640, 356, "4,192 frames of 466², 60 fps", size=13, color=MUTED, line=16, width="180px", text_align="center")
    b.arrow(424, 544, 1035, 544, BLUE)
    b.text(640, 556, "a 16 kHz WAV per take", size=13, color=MUTED, line=16, width="180px", text_align="center")
    b.arrow(940, 452, 1035, 452, YELLOW, alpha=0.7)
    b.arrow(940, 488, 1035, 488, YELLOW, alpha=0.7)
    b.arrow(424, 648, 1035, 648, WHITE, alpha=0.5)
    b.text(620, 620, "two tracks, downloaded once", size=13, color=MUTED, line=16, width="220px", text_align="center")
    b.arrow(1456, 340, 1551, 340, GREEN)
    b.text(1464, 312, "raw RGB", size=13, color=MUTED, line=16, width="80px", text_align="center")
    b.arrow(1456, 582, 1551, 582, GREEN)
    b.text(1464, 554, "mix.wav", size=13, color=MUTED, line=16, width="80px", text_align="center")

    b.band(64, 712, 1792, 304)
    b.text(88, 728, tag("EDIT.PY · THE CUT", YELLOW) + "&nbsp; the cozy half cuts on the ukulele's beat: 8 beats for the title, then 7 a shot", size=15, color=MUTED, line=20)
    for y, lane in ((774, "shots"), (820, "beat"), (862, "music"), (912, "sounds")):
        b.text(88, y, lane, size=14, color=MUTED, line=18)
    ends = [start for start, *_ in SHOTS[1:]] + [TRAILER_END]
    for (start, label, fill, ink), end in zip(SHOTS, ends):
        b.place(
            "cards",
            at(start),
            760,
            f'<span style="{css(color=ink)}">{label}</span>',
            width=f"{at(end) - at(start) - 2}px",
            height="48px",
            box_sizing="border-box",
            display="flex",
            align_items="center",
            justify_content="center",
            background=fill,
            border_radius="6px",
            font_family=SANS,
            font_weight=600,
            font_size="13px",
        )
    beat = 60 / 130 * 1608 / TRAILER_END
    b.arrow(at(0.135), 828, at(19.98), 828, YELLOW, heads=None, width=10, dash=f"2 {beat - 2:.2f}", alpha=0.7)
    for start, end, y, label, alpha in (
        (0.0, 19.98, 846, "Happy Whistling Ukulele, −5 dB", 0.75),
        (20.68, 33.28, 872, "Flight of the Bumblebee, −4 dB", 0.75),
        (32.68, TRAILER_END, 846, "the ukulele again, from where it stopped", 0.4),
    ):
        b.place(
            "cards",
            at(start),
            y,
            span(label),
            width=f"{at(end) - at(start)}px",
            height="20px",
            box_sizing="border-box",
            padding="0 8px",
            display="flex",
            align_items="center",
            background=rgb(SAND if y == 846 else YELLOW, alpha * 0.45),
            border_radius="4px",
            font_size="12px",
            color=INK,
            white_space="nowrap",
            overflow="hidden",
        )
    for t, w, label in ((2.68, 72, "whoosh"), (20.28, 72, "scratch"), (34.9, 96, "2 crunches")):
        b.card(at(t) - w // 2, 906, w, 24, span(label), WHITE, fill=0.08, edge=0.35, radius=12, pad="0", justify_content="center", font_size="12px", color=SOFT)
    b.arrow(200, 948, 1808, 948, WHITE, heads=None, width=1, alpha=0.25)
    for t in (0, 10, 20, 30, 40):
        b.text(at(t) - 30, 956, f"{t} s", size=13, color=MUTED, line=16, width="60px", text_align="center")
    b.text(at(TRAILER_END) - 60, 956, "46.2 s", size=13, color=MUTED, line=16, width="60px", text_align="right")
    return b


# The case board's "worth a closer look" cards: (title, what).
TRICKS = [
    ("Line as stacked rings", "16 rings, the pitch stretched to fit a whole number of them"),
    ("Art that prints face down", "the background is recessed 0.7 mm and the art mirrored, to read from behind"),
    ("Words that fit", "each word's arc is measured against the gap between screws, or the build stops"),
    ("Buttons that stay put", "nub, flange, 45° cone, cap; the slot's shape keeps them from turning"),
]


def case():
    """How the case was made: the Python files and what flows between them, tricks, previews."""
    b = Board("Fishing Minigame: how the case was made")
    header(
        b,
        "how the case was made",
        "The case is code: build123d turns about 60 tunables into solids, and a fit test drops "
        "Waveshare's model of the board into them.",
        ["build123d 0.13", "53.9 mm across", "3 × M2×6 screws", "support-free"],
    )

    b.card(64, 184, 300, 120, span(mono("params.py", YELLOW, 16) + "<br>about 60 tunables in mm: glass gap 0.30, wall 1.5, line pitch 1.2", line_height="20px"), YELLOW, fill=0.07, font_size="15px", color=SOFT)
    b.card(
        64,
        352,
        300,
        160,
        span(mono("board.py", ORANGE, 16) + "<br>Waveshare's 18.7 MB STEP model, plus hand-measured spots: the glass, keys, USB-C and 3 standoffs", line_height="20px"),
        ORANGE,
        fill=0.08,
        font_size="15px",
        color=SOFT,
    )
    b.card(448, 184, 320, 120, span(mono("stack.py", TEAL, 16) + "<br>fits the speaker, GPS antenna and battery under the board, trying every 2°", line_height="20px"), TEAL, fill=0.07, font_size="15px", color=SOFT)
    b.card(
        448,
        352,
        320,
        248,
        span(
            mono("parts.py", GREEN, 16)
            + "<br>build123d solids: a revolved reel body wound with 'line', three screw posts, "
            "speaker rails and a 24-hole grille, raised art on the back, and two button caps",
            line_height="21px",
        ),
        GREEN,
        fill=0.08,
        font_size="15px",
        color=SOFT,
    )
    b.card(852, 184, 324, 120, span(mono("build.py", BLUE, 16) + "<br>STEP, STL and 3MF plates; the two-colour plate gives the art its own part", line_height="20px"), BLUE, fill=0.08, font_size="15px", color=SOFT)
    b.card(852, 328, 324, 104, span(mono("render.py", BLUE, 16) + "<br>PyVista renders the previews off-screen", line_height="20px"), BLUE, fill=0.08, font_size="15px", color=SOFT)
    b.card(
        852,
        456,
        324,
        184,
        span(
            mono("check.py", PINK, 16)
            + "<br>with the real board inside: every part clears it, it lowers in cleanly from 16 mm, "
            "a 0.35 mm press works each switch, and an M2×6 screw bites",
            line_height="20px",
        ),
        PINK,
        fill=0.08,
        font_size="15px",
        color=SOFT,
    )

    b.arrow(364, 244, 447, 244, WHITE, alpha=0.5)
    b.text(374, 216, "sizes", size=13, color=MUTED, line=16, width="64px", text_align="center")
    b.arrow(364, 432, 447, 432, ORANGE)
    b.text(370, 404, "keep-outs", size=13, color=MUTED, line=16, width="72px", text_align="center")
    b.arrow(608, 304, 608, 351, TEAL)
    b.text(620, 320, "floor depth", size=13, color=MUTED, line=16)
    b.arrow(768, 476, 810, 476, GREEN, heads=None)
    b.arrow(810, 244, 810, 548, GREEN, heads=None)
    for y in (244, 380, 548):
        b.arrow(810, y, 851, y, GREEN)
    b.arrow(214, 512, 214, 680, ORANGE, heads=None)
    b.arrow(214, 680, 1014, 680, ORANGE, heads=None)
    b.arrow(1014, 680, 1014, 641, ORANGE)
    b.text(474, 692, "the whole board, as solids", size=13, color=MUTED, line=16, width="200px", text_align="center")

    b.text(64, 740, tag("WORTH A CLOSER LOOK", (184, 194, 214)), line=20)
    for i, (title, what) in enumerate(TRICKS):
        b.card(
            64 + i * 284,
            772,
            260,
            128,
            span(span(title, font_weight=600, color=INK, font_size="16px") + "<br>" + what, line_height="21px"),
            WHITE,
            fill=0.04,
            edge=0.18,
            pad="0 16px",
            font_size="15px",
            color=SOFT,
        )
    b.card(
        64,
        924,
        1112,
        64,
        span(
            mono("out/", BLUE, 15)
            + "&nbsp; reel_case.3mf → a slicer: PLA or PETG, 0.16 mm layers, 15% infill&nbsp;&nbsp;·&nbsp;&nbsp;"
            "reel_case.stl → the trailer's Blender man"
        ),
        WHITE,
        fill=0.03,
        edge=0.12,
        font_size="15px",
        color=SOFT,
    )

    b.image(1224, 184, 632, 480, "case/previews/back.png", "the back of the case", object_fit="contain", background="#f4f1ea", border_radius="12px")
    b.text(1224, 672, "the back: raised art, printed face down", size=14, color=MUTED, line=18)
    b.image(1224, 712, 304, 248, "case/previews/battery_section.png", "a section through the battery case", object_fit="cover", border_radius="12px")
    b.image(1552, 712, 304, 248, "case/previews/button_side.png", "the side with the button caps", object_fit="cover", border_radius="12px")
    b.text(1224, 968, "section: speaker, antenna, battery", size=14, color=MUTED, line=18)
    b.text(1552, 968, "the line and the two caps", size=14, color=MUTED, line=18)
    return b


# Each board's file name, and the function that draws it.
BOARDS = {"overview": overview, "orbit-core": core, "frame-pipeline": pipeline, "trailer": trailer, "case": case}


def render(name, board):
    """Shoot one board to docs/architecture/<name>.png with headless Chrome."""
    with tempfile.TemporaryDirectory() as tmp:
        page = Path(tmp) / f"{name}.html"
        page.write_text(board.page())
        png = HERE / f"{name}.png"
        subprocess.run(
            [
                CHROME,
                "--headless=new",
                "--disable-gpu",
                "--hide-scrollbars",
                f"--window-size={W},{H}",
                "--force-device-scale-factor=1",
                "--virtual-time-budget=10000",
                f"--screenshot={png}",
                page.as_uri(),
            ],
            check=True,
            capture_output=True,
        )
    print(png.relative_to(REPO))


if __name__ == "__main__":
    if sys.argv[1:2] == ["--canvas"]:
        out, blobs = Path(sys.argv[2]), json.loads(Path(sys.argv[3]).read_text())
        out.mkdir(parents=True, exist_ok=True)
        for name, draw in BOARDS.items():
            (out / f"{name}.dc.html").write_text(draw().artboard(blobs))
            print(out / f"{name}.dc.html")
    else:
        for name, draw in BOARDS.items():
            render(name, draw())
