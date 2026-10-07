"""The trailer's ending, rendered in Blender: a crude man takes two bites out of the printed case.

    blender -b --factory-startup -P eat.py -- config.json

`build.py` writes the config: where the frames go, the case STL, the game frame shown on the
screen, the frames the bites land on, and the mouth's openness for every frame (the bites and
the spoken line, worked out from the voice clip).
"""

import json
import math
import random
import sys
from pathlib import Path

import bpy
from mathutils import Euler, Matrix, Vector

CONFIG = json.loads(Path(sys.argv[sys.argv.index("--") + 1]).read_text())
FRAMES = CONFIG["frames"]
BITES = CONFIG["bites"]
MOUTH = CONFIG["mouth"]

SKIN = "#e9b48c"
SHIRT = "#d8452f"
HAT = "#6f7d3c"
CASE = "#2e6b5a"
CASE_INSIDE = "#24584a"
BUTTON = "#ff7a1a"
WALL = "#ffd45c"

# Where the mouth is, in metres: the bites happen here.
MOUTH_AT = Vector((0.0, -0.098, 1.535))
# The case is modelled in mm with its screen facing +Z; this stands it up facing the camera, a
# little bigger than life so it reads on screen.
CASE_TURN = Euler((math.radians(90), 0, 0)).to_matrix().to_4x4() @ Matrix.Scale(0.0014, 4)
# The case's centre above the hand holding it.
IN_HAND = Vector((0.0, 0.0, 0.074))
# Each bite: the rim angle it takes (degrees from the top, in the case's own frame) and how far
# the case is rolled in the hand so that spot is on top.
BITE_SPOTS = [0.0, 50.0]


def hex_color(text, alpha=1.0):
    text = text.lstrip("#")
    srgb = [int(text[i : i + 2], 16) / 255 for i in (0, 2, 4)]
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in srgb]
    return (*linear, alpha)


def material(name, color, roughness=0.55, emission=0.0):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = hex_color(color)
    bsdf.inputs["Roughness"].default_value = roughness
    if emission:
        bsdf.inputs["Emission Color"].default_value = hex_color(color)
        bsdf.inputs["Emission Strength"].default_value = emission
    return mat


def give(obj, mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    return obj


def sphere(name, at, radius, scale=(1, 1, 1), mat=None, parent=None, segments=10, rings=7):
    bpy.ops.mesh.primitive_uv_sphere_add(
        segments=segments, ring_count=rings, radius=radius, location=at
    )
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = scale
    if mat:
        give(obj, mat)
    if parent:
        obj.parent = parent
        obj.matrix_parent_inverse = parent.matrix_world.inverted()
    return obj


def cylinder(name, at, radius, depth, rotation=(0, 0, 0), scale=(1, 1, 1), mat=None, parent=None):
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=12, radius=radius, depth=depth, location=at, rotation=rotation
    )
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = scale
    if mat:
        give(obj, mat)
    if parent:
        obj.parent = parent
        obj.matrix_parent_inverse = parent.matrix_world.inverted()
    return obj


def cube(name, at, size, mat=None, parent=None, rotation=(0, 0, 0)):
    bpy.ops.mesh.primitive_cube_add(size=1, location=at, rotation=rotation)
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = size
    if mat:
        give(obj, mat)
    if parent:
        obj.parent = parent
        obj.matrix_parent_inverse = parent.matrix_world.inverted()
    return obj


def empty(name, at=(0, 0, 0), parent=None):
    """With `parent`, `at` is relative to it."""
    obj = bpy.data.objects.new(name, None)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = at
    obj.parent = parent
    bpy.context.view_layer.update()
    return obj


def attached(name, parent):
    """An empty that follows `parent` while its own location stays a plain world offset."""
    obj = empty(name)
    obj.parent = parent
    obj.matrix_parent_inverse = parent.matrix_world.inverted()
    return obj


def key(obj, frame, **props):
    for prop, value in props.items():
        setattr(obj, prop, value)
        obj.keyframe_insert(prop, frame=frame)


def scene_setup():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.eevee.taa_render_samples = CONFIG.get("samples", 16)
    scene.render.resolution_x, scene.render.resolution_y = CONFIG["size"]
    scene.render.fps = CONFIG["fps"]
    scene.frame_start, scene.frame_end = 0, FRAMES - 1
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = str(Path(CONFIG["out"]) / "#####")
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("wall")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = hex_color(WALL)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.55
    scene.world = world
    return scene


def lights():
    for name, at, energy, size in [
        ("key", (-1.2, -1.6, 2.4), 110, 1.2),
        ("fill", (1.4, -1.2, 1.6), 35, 1.5),
        ("rim", (0.6, 1.2, 2.2), 120, 0.6),
    ]:
        data = bpy.data.lights.new(name, "AREA")
        data.energy = energy
        data.size = size
        obj = bpy.data.objects.new(name, data)
        bpy.context.scene.collection.objects.link(obj)
        obj.location = at
        obj.rotation_euler = (Vector((0, 0, 1.5)) - Vector(at)).to_track_quat("-Z", "Y").to_euler()
    bpy.ops.mesh.primitive_plane_add(
        size=8, location=(0, 1.2, 1.2), rotation=(math.radians(90), 0, 0)
    )
    give(bpy.context.active_object, material("backdrop", WALL, roughness=0.9))


def man():
    skin = material("skin", SKIN)
    head = empty("head", (0, 0, 1.6))
    sphere("skull", (0, 0, 1.6), 0.11, (1, 0.95, 1.15), skin, head)
    for side in (-1, 1):
        sphere(f"ear{side}", (side * 0.108, 0.0, 1.6), 0.026, (0.5, 1, 1.3), skin, head)
    sphere("nose", (0, -0.112, 1.585), 0.026, (1, 1.2, 0.9), skin, head)
    white = material("eye white", "#ffffff", 0.3)
    black = material("pupil", "#111111", 0.2)
    eyes = attached("eyes", head)
    for side in (-1, 1):
        sphere(f"eye{side}", (side * 0.042, -0.094, 1.645), 0.024, (1, 0.7, 1.15), white, head)
        sphere(f"pupil{side}", (side * 0.042, -0.112, 1.64), 0.011, (1, 0.6, 1), black, eyes)
    brows = attached("brows", head)
    for side in (-1, 1):
        cube(
            f"brow{side}",
            (side * 0.042, -0.102, 1.69),
            (0.045, 0.012, 0.01),
            black,
            brows,
            rotation=(0, side * math.radians(-12), 0),
        )
    # The mouth: a dark hole whose height is the openness, with a row of teeth top and bottom.
    hole = sphere(
        "mouth", MOUTH_AT, 1.0, (0.048, 0.02, 0.01), material("mouth", "#3a0d12", 0.8), head, 12, 8
    )
    top = cube("top teeth", MOUTH_AT + Vector((0, -0.012, 0.01)), (0.06, 0.012, 0.012), white, head)
    bottom = cube(
        "bottom teeth", MOUTH_AT + Vector((0, -0.012, -0.01)), (0.052, 0.012, 0.01), white, head
    )
    tongue = sphere(
        "tongue",
        MOUTH_AT + Vector((0, -0.006, -0.008)),
        1.0,
        (0.03, 0.012, 0.008),
        material("tongue", "#d0566b", 0.5),
        head,
    )
    # A bucket hat with a lure in its band.
    hat = material("hat", HAT, 0.9)
    cylinder("hat crown", (0, 0, 1.745), 0.104, 0.085, mat=hat, parent=head)
    bpy.ops.mesh.primitive_cone_add(
        vertices=12, radius1=0.17, radius2=0.108, depth=0.045, location=(0, 0, 1.705)
    )
    brim = give(bpy.context.active_object, hat)
    brim.parent = head
    brim.matrix_parent_inverse = head.matrix_world.inverted()
    sphere("lure", (0.07, -0.08, 1.735), 0.014, (1, 1, 1.4), material("lure", BUTTON, 0.3), head)
    shirt = material("shirt", SHIRT, 0.8)
    cylinder("neck", (0, 0.0, 1.46), 0.048, 0.1, mat=skin)
    cylinder("body", (0, 0.03, 1.15), 0.2, 0.55, scale=(1, 0.62, 1), mat=shirt)
    for side in (-1, 1):
        sphere(f"shoulder{side}", (side * 0.19, 0.03, 1.38), 0.075, (1, 0.8, 0.8), shirt)
    return head, eyes, brows, (hole, top, bottom, tongue)


def case_object(mat_case, mat_inside, screen_mat, button):
    bpy.ops.wm.stl_import(filepath=CONFIG["stl"])
    shell = bpy.context.selected_objects[0]
    shell.name = "case"
    give(shell, mat_case)
    bpy.ops.mesh.primitive_cylinder_add(vertices=96, radius=24.48, depth=0.8, location=(0, 0, -0.4))
    glass = give(bpy.context.active_object, screen_mat)
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=64, radius=23.6, depth=22.6, location=(0, 0, -12.0)
    )
    filling = give(bpy.context.active_object, mat_inside)
    nubs = []
    for deg in (60.0, 120.0):
        a = math.radians(90.0 - deg)
        bpy.ops.mesh.primitive_cube_add(
            size=1, location=(27.3 * math.cos(a), 27.3 * math.sin(a), -8.0), rotation=(0, 0, a)
        )
        nub = give(bpy.context.active_object, button)
        nub.scale = (2.4, 4.6, 8.0)
        nubs.append(nub)
    for obj in [glass, filling, *nubs]:
        obj.select_set(True)
    shell.select_set(True)
    bpy.context.view_layer.objects.active = shell
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.ops.object.join()
    return shell


def bite_cutters(spot_deg, name):
    """Three overlapping rounds through the whole case at the rim's `spot_deg`: a bite with
    tooth scallops, front to back like a cookie's."""
    cutters = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(cutters)
    for i, spread in enumerate((-17.0, 0.0, 17.0)):
        a = math.radians(spot_deg + spread)
        radius = 11.0 if spread == 0.0 else 9.0
        reach = 30.0 if spread == 0.0 else 30.5
        bpy.ops.mesh.primitive_cylinder_add(
            vertices=40,
            radius=radius,
            depth=40.0,
            location=(reach * math.sin(a), reach * math.cos(a), -12.0),
        )
        ball = bpy.context.active_object
        ball.name = f"{name}{i}"
        for coll in ball.users_collection:
            coll.objects.unlink(ball)
        cutters.objects.link(ball)
    return cutters


def bitten(whole, cutters, name):
    copy = whole.copy()
    copy.data = whole.data.copy()
    copy.name = name
    bpy.context.scene.collection.objects.link(copy)
    for coll in cutters:
        mod = copy.modifiers.new(f"bite {coll.name}", "BOOLEAN")
        mod.operation = "DIFFERENCE"
        mod.operand_type = "COLLECTION"
        mod.collection = coll
        mod.solver = "MANIFOLD"
    bpy.ops.object.select_all(action="DESELECT")
    copy.select_set(True)
    bpy.context.view_layer.objects.active = copy
    for mod in list(copy.modifiers):
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return copy


def screen_material():
    mat = bpy.data.materials.new("screen")
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (0.0, 0.0, 0.0, 1)
    bsdf.inputs["Roughness"].default_value = 0.08
    coords = nodes.new("ShaderNodeTexCoord")
    mapping = nodes.new("ShaderNodeMapping")
    mapping.inputs["Location"].default_value = (0.5, 0.5, 0)
    mapping.inputs["Scale"].default_value = (1 / 48.96, 1 / 48.96, 1)
    image = nodes.new("ShaderNodeTexImage")
    image.image = bpy.data.images.load(CONFIG["screen"])
    image.interpolation = "Closest"
    image.extension = "CLIP"
    links.new(coords.outputs["Object"], mapping.inputs["Vector"])
    links.new(mapping.outputs["Vector"], image.inputs["Vector"])
    links.new(image.outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = 1.4
    return mat


def show_between(obj, first, last):
    """Visible for frames first..last inclusive."""
    for frame, hidden in [(0, first > 0), (first, False), (last + 1, True)]:
        if frame < 0 or frame > FRAMES:
            continue
        obj.hide_render = hidden
        obj.hide_viewport = hidden
        obj.keyframe_insert("hide_render", frame=frame)
        obj.keyframe_insert("hide_viewport", frame=frame)


def hand_rig(versions):
    skin = bpy.data.materials["skin"]
    shirt = bpy.data.materials["shirt"]
    grip = empty("grip")
    roll = empty("roll", parent=grip)
    holder = empty("holder", IN_HAND, roll)
    for version in versions:
        version.parent = holder
        version.matrix_parent_inverse = Matrix.Identity(4)
        version.matrix_basis = CASE_TURN
    sphere("palm", (0, 0.012, 0.0), 0.042, (1.1, 0.8, 0.85), skin, grip)
    sphere("thumb", (-0.032, -0.012, 0.042), 0.015, (1, 1, 1.7), skin, grip)
    for i in range(3):
        sphere(f"finger{i}", (-0.006 + i * 0.019, 0.04, 0.042), 0.014, (1, 1, 1.6), skin, grip)
    arm = cylinder(
        "forearm", (0.1, 0.03, -0.17), 0.036, 0.36, rotation=(0, math.radians(-30), 0), mat=skin
    )
    arm.parent = grip
    sleeve = cylinder(
        "sleeve", (0.19, 0.03, -0.33), 0.055, 0.2, rotation=(0, math.radians(-30), 0), mat=shirt
    )
    sleeve.parent = grip
    return grip, roll


def grip_for_bite(roll_deg):
    """Where the hand must be for the top of the case (rolled by `roll_deg`) to be in the mouth."""
    rim = CASE_TURN @ Vector((0.0, 27.0, -12.0))
    rolled = Euler((0, math.radians(roll_deg), 0)).to_matrix() @ (IN_HAND + rim)
    return MOUTH_AT + Vector((0, -0.004, -0.004)) - rolled


def animate_hand(grip, roll):
    rest = Vector((0.05, -0.25, 1.33))
    show = Vector((0.17, -0.21, 1.47))
    first, second = BITES
    poses = [
        (0, rest, 0.0, -8.0),
        (first - 22, rest + Vector((0, 0, 0.02)), 0.0, -8.0),
        (first - 4, grip_for_bite(0.0) + Vector((0, -0.03, -0.02)), 0.0, 0.0),
        (first, grip_for_bite(0.0), 0.0, 0.0),
        (first + 3, grip_for_bite(0.0), 0.0, 0.0),
        (first + 16, rest + Vector((-0.02, 0.0, 0.06)), -15.0, -10.0),
        (second - 18, rest + Vector((-0.02, 0.0, 0.07)), -BITE_SPOTS[1], -10.0),
        (
            second - 4,
            grip_for_bite(-BITE_SPOTS[1]) + Vector((0, -0.03, -0.02)),
            -BITE_SPOTS[1],
            0.0,
        ),
        (second, grip_for_bite(-BITE_SPOTS[1]), -BITE_SPOTS[1], 0.0),
        (second + 3, grip_for_bite(-BITE_SPOTS[1]), -BITE_SPOTS[1], 0.0),
        (second + 22, show, -20.0, -22.0),
        (FRAMES - 1, show + Vector((0, 0, 0.012)), -24.0, -22.0),
    ]
    for frame, at, roll_deg, face_deg in poses:
        key(grip, frame, location=at, rotation_euler=Euler((0, 0, math.radians(face_deg))))
        key(roll, frame, rotation_euler=Euler((0, math.radians(roll_deg), 0)))


def animate_face(head, eyes, brows, mouth):
    hole, top, bottom, tongue = mouth
    for frame, openness in enumerate(MOUTH):
        gap = 0.002 + openness * 0.034
        key(hole, frame, scale=Vector((0.046 + openness * 0.012, 0.02, gap * 0.62 + 0.004)))
        key(top, frame, location=MOUTH_AT + Vector((0, -0.013, gap * 0.45 + 0.004)))
        key(bottom, frame, location=MOUTH_AT + Vector((0, -0.013, -gap * 0.55 - 0.004)))
        key(tongue, frame, location=MOUTH_AT + Vector((0, -0.006, -gap * 0.35)))
    first, second = BITES
    look_down = Vector((0.004, 0, -0.008))
    for frame, look in [
        (0, look_down),
        (first - 24, look_down),
        (first - 12, Vector((0, 0, 0))),
        (first + 18, Vector((0.004, 0, -0.006))),
        (second - 10, Vector((0, 0, 0))),
        (second + 20, Vector((-0.006, 0, 0.0))),
        (FRAMES - 1, Vector((-0.006, 0, 0.0))),
    ]:
        key(eyes, frame, location=look)
    for frame, lift in [
        (0, 0.0),
        (first - 16, 0.012),
        (first, -0.004),
        (first + 10, 0.0),
        (second - 10, 0.014),
        (second, -0.004),
        (second + 30, 0.0),
    ]:
        key(brows, frame, location=Vector((0, 0, lift)))
    wiggle = second + 40
    while wiggle < FRAMES - 6:
        key(brows, wiggle, location=Vector((0, 0, 0.012)))
        key(brows, wiggle + 5, location=Vector((0, 0, 0.0)))
        wiggle += 12
    for frame in range(0, FRAMES, 6):
        nod = 0.012 * math.sin(frame / FRAMES * math.tau * 3)
        chew = (
            0.02 * max(0.0, math.sin((frame - first) * 0.6)) if first < frame < second - 10 else 0.0
        )
        key(
            head,
            frame,
            location=Vector((0, 0, 1.6 + nod * 0.2)),
            rotation_euler=Euler((chew, 0, nod * 2)),
        )


def crumbs(at_frames, mats):
    rng = random.Random(7)
    for b, frame in enumerate(at_frames):
        for i in range(26):
            size = rng.uniform(0.003, 0.008)
            crumb = cube(f"crumb{b}.{i}", MOUTH_AT, (size, size, size), rng.choice(mats))
            velocity = Vector(
                (rng.uniform(-0.9, 0.9), rng.uniform(-1.1, -0.2), rng.uniform(0.2, 1.3))
            )
            spin = Vector((rng.uniform(-12, 12) for _ in range(3)))
            key(crumb, 0, scale=Vector((0, 0, 0)))
            key(crumb, frame - 1, scale=Vector((0, 0, 0)))
            for step in range(0, 40, 2):
                t = step / CONFIG["fps"]
                at = MOUTH_AT + velocity * t + Vector((0, 0, -4.9 * t * t))
                key(
                    crumb,
                    frame + step,
                    location=at,
                    scale=Vector((size, size, size)),
                    rotation_euler=Euler(spin * t),
                )


def camera():
    data = bpy.data.cameras.new("camera")
    data.lens = 50
    cam = bpy.data.objects.new("camera", data)
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    target = Vector((0.0, -0.05, 1.565))
    rng = random.Random(3)
    for frame in range(FRAMES):
        push = frame / FRAMES
        at = Vector((-0.48, -1.34, 1.68)).lerp(Vector((-0.4, -1.12, 1.66)), push)
        shake = Vector((0, 0, 0))
        for bite in BITES:
            if bite <= frame < bite + 10:
                fade = 1 - (frame - bite) / 10
                shake = Vector((rng.uniform(-1, 1), 0, rng.uniform(-1, 1))) * 0.012 * fade
        cam.location = at + shake
        cam.rotation_euler = (target - at).to_track_quat("-Z", "Y").to_euler()
        cam.keyframe_insert("location", frame=frame)
        cam.keyframe_insert("rotation_euler", frame=frame)


def main():
    scene = scene_setup()
    lights()
    head, eyes, brows, mouth = man()
    case_mat, inside = material("case", CASE, 0.45), material("inside", CASE_INSIDE, 0.8)
    whole = case_object(case_mat, inside, screen_material(), material("button", BUTTON, 0.4))
    cutters = [bite_cutters(spot, f"bite{i}") for i, spot in enumerate(BITE_SPOTS)]
    once = bitten(whole, cutters[:1], "case bitten once")
    twice = bitten(whole, cutters, "case bitten twice")
    for coll in cutters:
        for ball in list(coll.objects):
            bpy.data.objects.remove(ball)
    grip, roll = hand_rig([whole, once, twice])
    first, second = BITES
    show_between(whole, 0, first - 1)
    show_between(once, first, second - 1)
    show_between(twice, second, FRAMES)
    animate_hand(grip, roll)
    animate_face(head, eyes, brows, mouth)
    crumbs(BITES, [case_mat, inside])
    camera()
    if CONFIG.get("stills"):
        for frame in CONFIG["stills"]:
            scene.frame_set(frame)
            scene.render.filepath = str(Path(CONFIG["out"]) / f"still_{frame:05d}.png")
            bpy.ops.render.render(write_still=True)
        return
    bpy.ops.render.render(animation=True)


main()
