"""Build every printable part into out/: `uv run build.py`.

out/ gets, for each case variant, a STEP (edit it in any CAD program) and an STL, plus a
3MF print plate with the case on its back and three button caps beside it, and a
_two_colour.3mf where the raised art is its own part, ready for a second filament on an AMS.
"""

from pathlib import Path

from build123d import Box, Mesher, Pos, export_step, export_stl

import params as p
import parts
import stack

OUT = Path(__file__).parent / "out"
VARIANTS = {"reel_case": False, "reel_case_battery": True}


def save(shape, name: str):
    export_step(shape, OUT / f"{name}.step")
    export_stl(shape, OUT / f"{name}.stl", tolerance=0.005, angular_tolerance=0.1)


def mesh_object(mesher: Mesher, shape, name: str):
    verts, tris = Mesher._mesh_shape(shape, 0.001, 0.1)
    mesh = mesher.model.AddMeshObject()
    mesh.SetGeometry(*Mesher._create_3mf_mesh(verts, tris))
    mesh.SetName(name)
    return mesh


def plate(name: str, case, two_colour: bool = False):
    """The case on its back, at the origin, with three button caps (one spare) beside it.

    With `two_colour` the case is one object in two parts, the raised art and the rest, so the
    slicer can give the art its own filament.
    """
    battery = VARIANTS[name]
    lift = Pos(0, 0, -parts.back_z(battery))
    mesher = Mesher()
    if two_colour:
        # Mesher only writes single-mesh objects, so the two-part object is built in lib3mf.
        identity = mesher.wrapper.GetIdentityTransform()
        art = parts.art(battery)
        group = mesher.model.AddComponentsObject()
        group.SetName("case")
        for label, shape in (("case", case - art), ("raised art", art)):
            group.AddComponent(mesh_object(mesher, lift * shape, label), identity)
        mesher.model.AddBuildItem(group, identity)
        name += "_two_colour"
    else:
        mesher.add_shape(lift * case, part_number="case")
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
    items = stack.stack(battery)[0]
    inside = ", ".join("{} {:g}x{:g}x{:g}".format(i.name, *i.size) for i in items) or "nothing"
    print(f"{name}: D{2 * parts.OUTER_R:.1f} x {height:.1f} mm, under the board: {inside}")
    band = p.BACK_FLANGE, height - p.FRONT_FLANGE
    print(f"    two-tone rim: change filament at {band[0]:.1f} mm and back at {band[1]:.1f} mm")


def main():
    OUT.mkdir(exist_ok=True)
    for name, battery in VARIANTS.items():
        case = parts.case(battery)
        save(case, name)
        plate(name, case)
        plate(name, case, two_colour=True)
        summary(name, battery)
    save(parts.button_print(), "button")
    save(fit_ring(), "fit_test_ring")
    w, h = p.BUTTON_SIZE
    print(f"buttons: {w:g} x {h:g} mm caps, print 2 (+1 spare) lying on the face you press")
    print("fit_test_ring: the top 10 mm of the case; print it as it lies, cut edge down")
    print("screws: 3x M2x6 (M2x5 also fits; never longer than 6), heads in the back counterbores")
    print(f"wrote {OUT.relative_to(Path.cwd()) if OUT.is_relative_to(Path.cwd()) else OUT}/")


if __name__ == "__main__":
    main()
