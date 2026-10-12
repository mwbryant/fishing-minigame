"""PNG previews of the case, alone and with the board fitted: `uv run render.py`."""

from pathlib import Path

import pyvista as pv
from build123d import export_stl

import board
import params as p
import parts

OUT = Path(__file__).parent / "previews"
# The case as printed: brown flanges with white line wound between, changing filament at the
# flanges' heights. PRINTED in a shot cuts a case mesh into those three colours.
FLANGE_COLOR = "#6b4226"
LINE_COLOR = "#f2eee4"
PRINTED = "printed"
BUTTON_COLOR = "#ff7a1a"
BOARD_COLOR = "#3a3f45"
GLASS_COLOR = "#0b0d10"
ART_COLOR = "#efe6cf"
ITEM_COLORS = {"speaker": "#1d1d1f", "gps antenna": "#c9b37e", "battery": "#3d6fb6"}
SIZE = (1100, 900)


def mesh(shape, name: str) -> pv.PolyData:
    """`shape` as a PyVista mesh, by way of an STL in .cache/."""
    board.CACHE.mkdir(exist_ok=True)
    path = board.CACHE / f"{name}.stl"
    export_stl(shape, path, tolerance=0.01, angular_tolerance=0.15)
    return pv.read(path)


def board_mesh() -> pv.PolyData:
    """The board as a mesh, cached as .cache/board.stl."""
    path = board.CACHE / "board.stl"
    if not path.exists():
        board.CACHE.mkdir(exist_ok=True)
        export_stl(board.load(), path, tolerance=0.03, angular_tolerance=0.3)
    return pv.read(path)


def glass_mesh() -> pv.PolyData:
    """A dark disc standing in for the cover glass, its face 0.05 mm proud of the board model's
    so the two never fight over the same depth."""
    return pv.Cylinder(
        center=(0, 0, -0.55), direction=(0, 0, 1), radius=board.GLASS_R, height=1.2, resolution=180
    )


def printed(m: pv.PolyData) -> list:
    """`m` cut at the two filament changes into the back flange, the line and the front flange,
    each with its colour, so the colours meet exactly where the print's do."""
    back, front = m.bounds[4], m.bounds[5]
    below_line, above_line = back + p.BACK_FLANGE, front - p.FRONT_FLANGE
    up = (0, 0, 1)
    back_flange = m.clip(normal=up, origin=(0, 0, below_line), invert=True)
    rest = m.clip(normal=up, origin=(0, 0, below_line), invert=False)
    line = rest.clip(normal=up, origin=(0, 0, above_line), invert=True)
    front_flange = rest.clip(normal=up, origin=(0, 0, above_line), invert=False)
    return [(back_flange, FLANGE_COLOR), (line, LINE_COLOR), (front_flange, FLANGE_COLOR)]


def shot(name: str, actors, camera, section: bool = False, parallel: bool = False, key=None):
    """One preview PNG. `key` is the main light; by default it rakes in over the camera."""
    plot = pv.Plotter(off_screen=True, window_size=SIZE, lighting="none")
    plot.set_background("#f4f1ea")
    eye, focus, up = camera
    if key is None:
        key = tuple(e + 60 * u for e, u in zip(eye, up))
    plot.add_light(pv.Light(position=key, focal_point=focus, intensity=0.85))
    plot.add_light(pv.Light(position=eye, focal_point=focus, intensity=0.35))
    pieces = []
    for m, color in actors:
        pieces += printed(m) if color == PRINTED else [(m, color)]
    for m, color in pieces:
        if section:
            m = m.clip(normal=(0, -1, 0), origin=(0, 0.01, 0))
        m = m.compute_normals(split_vertices=True, feature_angle=35)
        plot.add_mesh(m, color=color, smooth_shading=True, specular=0.25, specular_power=20)
    plot.camera_position = camera
    if parallel:
        plot.enable_parallel_projection()
    plot.camera.zoom(1.25)
    path = OUT / f"{name}.png"
    plot.screenshot(path)
    plot.close()
    print(path.relative_to(Path.cwd()) if path.is_relative_to(Path.cwd()) else path)


def stack(battery: bool) -> list:
    """The items under the board, coloured, for the section views."""
    return [
        (mesh(box, f"{name}_{battery}"), ITEM_COLORS[name])
        for name, box in parts.item_boxes(battery)
    ]


def main():
    OUT.mkdir(exist_ok=True)
    case = mesh(parts.case(False), "case")
    case_bat = mesh(parts.case(True), "case_battery")
    buttons = mesh(parts.button(), "buttons")
    pcb = board_mesh()
    glass = glass_mesh()
    fitted = [(case, PRINTED), (buttons, BUTTON_COLOR), (pcb, BOARD_COLOR), (glass, GLASS_COLOR)]

    shot("front", fitted, [(40, -70, 140), (0, 0, -7), (0, 0, 1)])
    art = mesh(parts.art(False), "art")
    shot(
        "back",
        [(case, PRINTED), (art, ART_COLOR)],
        [(0, 0, -140), (0, 0, -8), (0, 1, 0)],
        parallel=True,
    )
    shot(
        "back_angle_single_colour",
        [(case, PRINTED)],
        [(-25, 40, -120), (0, 0, -8), (0, 1, 0)],
        key=(-80, 60, -40),
    )
    shot("usb_side", fitted, [(-140, -20, 10), (0, 0, -7), (0, 0, 1)])
    shot("button_side", fitted, [(140, 25, 10), (0, 0, -7), (0, 0, 1)])
    shot(
        "inside",
        [(case, PRINTED), (buttons, BUTTON_COLOR)],
        [(30, -60, 120), (0, 0, -8), (0, 0, 1)],
    )
    shot(
        "section",
        fitted + stack(False),
        [(0, -140, -7), (0, 0, -7), (0, 0, 1)],
        section=True,
        parallel=True,
    )
    bat = [
        (case_bat, PRINTED),
        (buttons, BUTTON_COLOR),
        (pcb, BOARD_COLOR),
        (glass, GLASS_COLOR),
    ]
    shot(
        "battery_section",
        bat + stack(True),
        [(0, -140, -10), (0, 0, -10), (0, 0, 1)],
        section=True,
        parallel=True,
    )


if __name__ == "__main__":
    main()
