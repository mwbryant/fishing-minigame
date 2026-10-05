"""Build every printable part into out/: `uv run build.py`.

out/ gets, for each case variant, a STEP (edit it in any CAD program) and an STL, plus a
3MF print plate with the case on its back and three button plungers beside it. The
artwork STLs are optional: load one together with its case as a single multi-part object
to print the back in two colours on an AMS.
"""

from pathlib import Path

from build123d import Box, Mesher, Pos, export_step, export_stl

import params as p
import parts

OUT = Path(__file__).parent / "out"
VARIANTS = {"reel_case": False, "reel_case_battery": True}


def save(shape, name: str):
    export_step(shape, OUT / f"{name}.step")
    export_stl(shape, OUT / f"{name}.stl", tolerance=0.005, angular_tolerance=0.1)


def plate(name: str, case):
    """The case on its back, at the origin, with three plungers (one spare) beside it."""
    mesher = Mesher()
    mesher.add_shape(Pos(0, 0, -parts.back_z(VARIANTS[name])) * case, part_number="case")
    for i in range(3):
        x = parts.OUTER_R + 6
        mesher.add_shape(
            Pos(x, (i - 1) * 8, 0) * parts.button_print(), part_number=f"button {i + 1}"
        )
    mesher.write(OUT / f"{name}.3mf")


def fit_ring():
    """The top 10 mm of the case: a short print to try the glass, USB-C and button holes first."""
    slab = Pos(0, 0, -10) * Box(80, 80, 20, align=parts.BOTTOM)
    return parts.case(False) & slab


def summary(name: str, battery: bool):
    zb = parts.back_z(battery)
    height = parts.TOP_Z - zb
    inside = (
        ", ".join(f"{n} {w:g}x{l:g}x{t:g}" for n, (w, l, t) in parts.contents(battery)) or "nothing"
    )
    print(f"{name}: D{2 * parts.OUTER_R:.1f} x {height:.1f} mm, under the board: {inside}")
    band = p.BACK_FLANGE, height - p.FRONT_FLANGE
    print(f"    two-tone rim: change filament at {band[0]:.1f} mm and back at {band[1]:.1f} mm")


def main():
    OUT.mkdir(exist_ok=True)
    for name, battery in VARIANTS.items():
        case = parts.case(battery)
        save(case, name)
        export_stl(parts.art(battery), OUT / f"{name}_art.stl", tolerance=0.005)
        plate(name, case)
        summary(name, battery)
    save(parts.button_print(), "button")
    save(fit_ring(), "fit_test_ring")
    print(f"buttons: print 2 (+1 spare) standing on the round end, {p.BUTTON_D} mm across")
    print("fit_test_ring: the top 10 mm of the case; print it as it lies, cut edge down")
    print("screws: 3x M2x6 (M2x5 also fits; never longer than 6), heads in the back counterbores")
    print(f"wrote {OUT.relative_to(Path.cwd()) if OUT.is_relative_to(Path.cwd()) else OUT}/")


if __name__ == "__main__":
    main()
