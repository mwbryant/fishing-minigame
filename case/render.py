"""PNG previews of the case, alone and with the board fitted: `uv run render.py`."""

from pathlib import Path

import pyvista as pv
from build123d import export_stl

import board
import parts

OUT = Path(__file__).parent / "previews"
CASE_COLOR = "#2e6b5a"
BUTTON_COLOR = "#ff7a1a"
BOARD_COLOR = "#3a3f45"
GLASS_COLOR = "#0b0d10"
ART_COLOR = "#efe6cf"
ITEM_COLORS = {"speaker": "#1d1d1f", "gps antenna": "#c9b37e", "battery": "#3d6fb6"}
SIZE = (1100, 900)


def mesh(shape, name: str) -> pv.PolyData:
    board.CACHE.mkdir(exist_ok=True)
    path = board.CACHE / f"{name}.stl"
    export_stl(shape, path, tolerance=0.01, angular_tolerance=0.15)
    return pv.read(path)


def board_mesh() -> pv.PolyData:
    path = board.CACHE / "board.stl"
    if not path.exists():
        board.CACHE.mkdir(exist_ok=True)
        export_stl(board.load(), path, tolerance=0.03, angular_tolerance=0.3)
    return pv.read(path)


def glass_mesh() -> pv.PolyData:
    return pv.Cylinder(
        center=(0, 0, -0.6), direction=(0, 0, 1), radius=board.GLASS_R, height=1.2, resolution=180
    )


def shot(name: str, actors, camera, section: bool = False, parallel: bool = False, key=None):
    """`key` is where the main light sits; by default it rakes in from over the camera's shoulder."""
    plot = pv.Plotter(off_screen=True, window_size=SIZE, lighting="none")
    plot.set_background("#f4f1ea")
    eye, focus, up = camera
    if key is None:
        key = tuple(e + 60 * u for e, u in zip(eye, up))
    plot.add_light(pv.Light(position=key, focal_point=focus, intensity=0.85))
    plot.add_light(pv.Light(position=eye, focal_point=focus, intensity=0.35))
    for m, color in actors:
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
    fitted = [(case, CASE_COLOR), (buttons, BUTTON_COLOR), (pcb, BOARD_COLOR), (glass, GLASS_COLOR)]

    shot("front", fitted, [(40, -70, 140), (0, 0, -7), (0, 0, 1)])
    art = mesh(parts.art(False), "art")
    shot(
        "back",
        [(case, CASE_COLOR), (art, ART_COLOR)],
        [(0, 0, -140), (0, 0, -8), (0, 1, 0)],
        parallel=True,
    )
    shot(
        "back_angle_single_colour",
        [(case, CASE_COLOR)],
        [(-25, 40, -120), (0, 0, -8), (0, 1, 0)],
        key=(-80, 60, -40),
    )
    shot("usb_side", fitted, [(-140, -20, 10), (0, 0, -7), (0, 0, 1)])
    shot("button_side", fitted, [(140, 25, 10), (0, 0, -7), (0, 0, 1)])
    shot(
        "inside",
        [(case, CASE_COLOR), (buttons, BUTTON_COLOR)],
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
        (case_bat, CASE_COLOR),
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
