# Fly-reel case

A 3D-printable case for the Waveshare ESP32-S3-Touch-AMOLED-1.75, made to look like a fly-fishing
reel. The screen is the spool, the rim is fishing line wound between two flanges, and the back is
the drag knob: a spouting whale inside a speaker grille, with raised "HAPPY BIRTHDAY" and "ETHAN"
running around the outer edge between the screw heads. PWR, BOOT, USB-C and the microSD slot all
stay reachable.

![front](previews/front.png)
![back, art in a second colour](previews/back.png)

It is parametric [build123d](https://build123d.readthedocs.io) Python: every size lives in
`params.py`, and one command rebuilds the STEP/STL/3MF files. `check.py` tests the parts against
Waveshare's own STEP model of the board (in `../docs/`).

## How it goes together

```
            glass (flush, 0.5 mm below the rim)
   rim  ____________________________________  rim
  |  |  board, screwed down through its     |  |   USB-C opening (9 o'clock)
  |  |  three M2 standoffs onto three posts  |  |   PWR / BOOT button caps (2 and 4 o'clock)
  |  |       [ speaker, face down ]          |  |   microSD slot (3 o'clock)
  |__|_______o o o grille o o o_____________|__|
       back: raised whale and text, 3 screw heads
```

- The case is one cup. The board drops in glass-up and three M2 screws through the back go into
  the board's own threaded standoffs. There are no clips and no glue, and three screws open it.
- PWR and BOOT are pressed through two printed button caps, 8 x 4.6 mm rounded slots that stand
  1 mm out of the rim. Behind each cap a flange stops it falling out, and a nub presses the
  board's side switch. The slot shape keys the cap so it can't turn.
- The speaker lies face down on the floor behind the grille. In the battery case a LiPo cell lies on
  top of it.

## Files (`uv run build.py` writes them to `out/`)

| File | What it is |
|---|---|
| `reel_case.3mf` | Print plate: the USB-powered case plus three button caps (one spare) |
| `reel_case_battery.3mf` | The same for the battery case (thicker: the cell lies under the board) |
| `reel_case*.stl`, `button.stl` | The same parts as plain meshes |
| `reel_case*_two_colour.3mf` | The same plates with the raised art as its own part, for a second filament on an AMS |
| `*.step` | Solid models that open in Fusion 360, Onshape, FreeCAD or Bambu Studio |
| `fit_test_ring.stl` | Top 10 mm of the case: a short print to try the glass, USB-C and buttons first |

Sizes: the USB case is D53.9 x 18.2 mm; the battery case is 23.6 mm thick.
Without the speaker (`SPEAKER = None`) the case is 16.3 mm thick.

## Printing on the P2S

- **Material:** PLA is fine. PETG gives tougher button caps.
- **Profile:** 0.16 mm layers, 3 walls, 15 % infill, no supports, no brim. Open the 3MF and
  everything is already lying the way it should print: the case on its back and the button caps
  on the face you press.
- **Raised art, face down:** the background around the art is recessed 0.7 mm. The art and the
  outer rim are the first layers on the plate and take its texture. The background is printed as
  short bridges, so it comes out slightly striated, which makes the lettering stand out.
- **Two-tone rim (no AMS needed):** in the preview, add a filament change at the two heights that
  `build.py` prints (2.0 mm and 16.0 mm for the USB case). The line between the flanges then comes
  out in a second colour.
- **Two-colour back (AMS):** open `reel_case_two_colour.3mf` instead. The case comes in as one
  object in two parts; give the "raised art" part your second filament.

## What you need

- 3x **M2x6** screws, pan or socket head. M2x5 also works. Never use anything longer than 6 mm,
  because an 8 mm screw reaches the display.
- The 8 ohm speaker from the box, plus a little double-sided foam tape.
- Battery case only: a single-cell 3.7 V LiPo pouch with a protection circuit (see below).

## Assembly

1. Optional: print `fit_test_ring.stl` and slide it over the board to check the glass fit and
   the hole positions.
2. From inside the case, push each button cap out through its slot (2 and 4 o'clock, either side
   of the microSD slot) until its flange stops it. A strip of masking tape over the caps on the
   outside keeps them from falling in while you do the next steps.
3. Lay the speaker face down between the two rails, with its wire end towards the lone post at
   12 o'clock (a quarter turn clockwise from the USB-C opening, looking in). Plug it into the
   board's SPK socket, and tape it down if it rattles.
4. Line USB-C up with the big opening and lower the board straight in, glass up. Nothing should
   catch. Once the screws are in, peel off the tape: each cap should click its switch.
5. Turn it over and drive the three screws into the counterbores until they are snug, not tight.

## Battery

The BAT socket takes one 3.7 V lithium-polymer cell. The board's AXP2101 charges it from USB-C,
and PWR turns the device on and off.

- **Plug:** MX1.25 2-pin, which is Molex PicoBlade 51021 style at 1.25 mm pitch. A JST-GH plug
  looks similar but won't fit. **Check the polarity against the + and - marks by the BAT socket
  before plugging in**, because sellers wire these leads both ways. Reversed, it can damage the
  board.
- **Protection circuit:** buy a cell with a built-in protection board (PCM). Most hobby LiPos with
  wires have one.
- **Capacity: 300 mAh or more.** The AXP2101 charges at 300 mA unless firmware changes it
  (register 0x62, default 300 mA to 4.2 V). The firmware here leaves that alone, and a cell
  should charge at no more than its own capacity per hour.
- **Size for the battery case:** a 602030 cell, 300 to 400 mAh. With its protection board it
  measures about 20.5 x 32 x 6.7 mm, which is what `BATTERY` is set to. Measure yours when it
  arrives and update `BATTERY`, then run `check.py`. A thicker 802030 (about 500 mAh) also works:
  set the thickness to 8.7 and the case grows 2 mm.
- **Runtime:** expect roughly 2 to 3 hours from 300 to 400 mAh. That figure is an estimate from
  typical ESP32-S3 and AMOLED draw, not a measurement.

To fit it: print `reel_case_battery`, stick the cell on top of the speaker with double-sided foam
tape (wires towards 12 o'clock), plug it into BAT, and move the board and button caps across.

If you only want cordless play now and then, any USB-C power bank on the USB-C port also works,
with no changes.

## Changing it

```sh
uv run build.py    # ~20 s: rebuild everything in out/
uv run check.py    # ~5 min: fit-test both cases against the board's STEP model
uv run render.py   # PNG previews in previews/
```

| File | Contents |
|---|---|
| `params.py` | Everything you might tune, with comments: text, font, fit gaps, wall and flange sizes, line pitch, button size, speaker and battery sizes. If a word no longer fits between its screws, `build.py` stops and says so |
| `board.py` | Board facts measured from the STEP model (positions, depths, keys, ports) |
| `parts.py` | The geometry, one small function per feature (body, posts, rails, grille, cutouts, art, button caps) |
| `check.py` | Fit tests: no overlaps, board lowers straight in, buttons hit their switches, the battery/speaker/plugs fit, safe screw lengths |

Frame used everywhere: looking at the screen with the game upright, +X is right, +Y is up, +Z
comes out of the glass, and Z = 0 is the glass face.

For a live 3D view while you edit, install the VS Code extension "OCP CAD Viewer", run
`uv add --dev ocp_vscode`, and `show(parts.case())`. The STEP files can be edited by hand in any
CAD program, but those edits don't flow back into the Python.

## Not verified yet (the board hasn't been in hand)

- **Speaker size.** Waveshare doesn't publish it. 20 x 30 x 4.5 is a guess at their usual 2030
  cavity speaker. Measure it and set `SPEAKER`: the floor drops to fit, and `check.py` confirms
  the result. The speaker has to fit inside the 20 x 30 mm area between the rails.
- **Plug room.** `PLUG_ROOM` (3.5 mm below the BAT/SPK sockets) is a guess for a mated MX1.25 plug
  with its wires bent away.
- **Button feel.** If a button is pressed all the time, raise `BUTTON_ACTUATOR_GAP`. If it is
  mushy, lower it.
- **Which way is up.** The docs put USB-C at 9 o'clock when the game is upright, and the back art
  reads upright on that basis when the device is turned over left to right. The text is laid out
  around the screw heads. If the screen turns out rotated, `WHALE_ROTATION` turns the whale.
- **Tolerances.** Tolerances are tuned for a Bambu printer: a 0.3 mm gap at the glass and 0.15 mm
  around the button caps. If the glass is tight in the fit ring, raise `GLASS_GAP`.
