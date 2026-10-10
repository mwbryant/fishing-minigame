# Fly-reel case

A 3D-printable case for the Waveshare ESP32-S3-Touch-AMOLED-1.75 that looks like a fly-fishing
reel. The screen is the spool, the rim is fishing line wound between two flanges, and the back is
the drag knob: a spouting whale in a speaker grille, with raised "HAPPY BIRTHDAY" and "ETHAN"
around the edge between the screw heads.

![front](previews/front.png)
![back, art in a second colour](previews/back.png)

PWR, BOOT, USB-C and the microSD slot all stay reachable. The case is parametric
[build123d](https://build123d.readthedocs.io) Python: every size lives in `params.py`, one command
rebuilds the STEP/STL/3MF files, and `check.py` fit-tests the parts against Waveshare's own STEP
model of the board (download it into `../docs/` first: see
[WAVESHARE_FILES.md](../docs/WAVESHARE_FILES.md)).

## How it goes together

```
            glass (0.5 mm below the rim, over a ledge)
   rim  ____________________________________  rim
  |  |  board, screwed down through its     |  |   USB-C opening (9 o'clock)
  |  |  three M2 standoffs onto three posts  |  |   PWR / BOOT button caps (2 and 4 o'clock)
  |  |     [ battery (battery case) ]        |  |   microSD slot (3 o'clock)
  |  |     [ GPS antenna, ceramic down ]     |  |
  |  |       [ speaker, face down ]          |  |
  |__|_______o o o grille o o o_____________|__|
       back: raised whale and text, 3 screw heads
```

- **One cup.** The board drops in glass-up, and three M2 screws through the back go into its own
  threaded standoffs. No clips, no glue: three screws open it.
- **Seat.** A chamfer round the rim guides the glass in, and a ledge under the glass rim (notched
  for USB-C) takes the load when you press the screen. The posts set the height; the ledge sits
  0.4 mm under the glass, so a print that comes out a little off can't hold the board up off its
  posts.
- **Posts.** Each post flares into the floor and a rib ties it to the wall, so screwing the board
  down doesn't snap it off at the base. Where a post passes the BAT/SPK sockets or the BOOT
  cap's flange it narrows for its last couple of mm, keeping at least 1.5 mm from the cap.
- **Buttons.** PWR and BOOT press through two printed caps, 8 x 4.6 mm rounded slots that stand
  1.4 mm out of the rim. Inside, a flange stops each cap falling out and a nub presses the board's
  side switch. The slot shape keys the cap so it can't turn.
- **Under the board,** from the floor up: the speaker face down behind the grille, the GPS antenna
  on it, and in the battery case the LiPo cell on top. `stack.py` lifts each layer as high as the
  parts under the board allow, and the floor drops to fit.

Both cases are 53.9 mm across. How deep they are depends on the board:

| Board | USB case | Battery case |
|---|---|---|
| -G board, with the speaker | 24.6 mm | 31.4 mm |
| no GPS (`GPS_ANTENNA = None`) | 18.2 mm | 23.6 mm |
| no GPS and no speaker (`SPEAKER = None`) | 16.3 mm | 18.5 mm |

## Files

`uv run build.py` writes these to `out/`:

| File | What it is |
|---|---|
| `reel_case.3mf` | Print plate: the USB-powered case plus three button caps (one spare) |
| `reel_case_battery.3mf` | The same for the battery case (deeper: the cell lies under the board) |
| `reel_case*.stl`, `button.stl` | The same parts as plain meshes |
| `reel_case*_two_colour.3mf` | The same plates with the raised art as its own part, for a second filament on an AMS |
| `*.step` | Solid models that open in Fusion 360, Onshape, FreeCAD or Bambu Studio |
| `fit_test_ring.stl` | The top 10 mm of the case: a short print to try the glass, USB-C and buttons first |

## Printing on the P2S

- **Material:** PLA is fine. PETG gives tougher button caps.
- **Profile:** 0.16 mm layers, 4 walls (the posts print solid), 15 % infill, no supports, no
  brim. The 3MF already lays
  every part the way it prints: the case on its back, the caps on the face you press.
- **Raised art, face down:** the background around the art is recessed 0.7 mm. The art and the
  outer rim are the first layers, so they take the plate's texture. The background prints as
  short bridges and comes out slightly striated, which makes the lettering stand out.
- **Two-tone rim (no AMS needed):** in the slicer's preview, add a filament change at the two
  heights `build.py` prints: 2.0 and 22.4 mm for the USB case, 2.0 and 29.2 mm for the battery
  case. The line between the flanges comes out in the second colour.
- **Two-colour back (AMS):** open `reel_case_two_colour.3mf` instead. The case is one object in
  two parts; give the "raised art" part your second filament.

## What you need

- 3x **M2x6** screws, pan or socket head. M2x5 also works. **Never longer than 6 mm:** an 8 mm
  screw reaches the display.
- The 8 ohm speaker from the box, plus a little double-sided foam tape.
- Battery case only: a single-cell 3.7 V LiPo pouch with a protection circuit (see
  [Battery](#battery)).

## Assembly

1. Optional: print `fit_test_ring.stl` and slide it over the board to check the glass fit and the
   hole positions.
2. From inside the case, push each button cap out through its slot (2 and 4 o'clock, either side
   of the microSD slot) until its flange stops it. A strip of masking tape over the caps outside
   keeps them from falling in during the next steps.
3. Lay the speaker face down between the two rails, wire end towards the lone post at 12 o'clock
   (a quarter turn clockwise from the USB-C opening, looking in). Plug it into the board's SPK
   socket, and tape it down if it rattles.
4. GPS board: lay the antenna on the speaker, ceramic side down. Plug its lead into the small U.FL
   socket on the back of the board nearer the USB-C port. The one by the side buttons is for
   Wi-Fi.
5. Battery case: stick the cell on top of the speaker (or the GPS antenna) with double-sided foam
   tape, wires towards 12 o'clock, and plug it into BAT. **Check the polarity first** (see
   [Battery](#battery)).
6. Line USB-C up with the big opening and lower the board straight in, glass up. Nothing should
   catch.
7. Turn it over and drive the three screws into the counterbores until they're snug, not tight.
   Then peel off the tape: each cap should click its switch.

To move a USB build into the battery case, print `reel_case_battery` and move the board and button
caps across.

## Battery

The BAT socket takes one 3.7 V lithium-polymer cell. The board's AXP2101 charges it from USB-C,
and PWR turns the device on and off. For cordless play now and then, a USB-C power bank on the
USB-C port works too, with no changes.

- **Plug:** MX1.25 2-pin (Molex PicoBlade 51021 style, 1.25 mm pitch). A JST-GH plug looks
  similar but won't fit. **Check the polarity against the + and - marks by the BAT socket before
  plugging in.** Sellers wire these leads both ways, and reversed it can damage the board.
- **Protection circuit:** buy a cell with a built-in protection board (PCM). Most hobby LiPos with
  wires have one.
- **Capacity: 300 mAh or more.** The AXP2101 charges at 300 mA to 4.2 V by default (register
  0x62), and the firmware leaves that alone. A cell should charge at no more than its own capacity
  per hour.
- **Size:** a 602030 cell, 300 to 400 mAh, measures about 20.5 x 32 x 6.7 mm with its protection
  board, which is what `BATTERY` is set to. Measure yours when it arrives, update `BATTERY` and
  run `check.py`. A thicker 802030 (about 500 mAh) also works: set the thickness to 8.7 and the
  case grows 2 mm.
- **Runtime:** roughly 2 to 3 hours from 300 to 400 mAh.

## GPS (the -G board)

- **Module:** the LC76G sits on the front of the board, under the display, so the case needs
  nothing for it.
- **Antenna:** 27.1 x 27.1 x 7.2 mm, measured. It lies on the speaker, ceramic side down, so it
  looks out through the plastic back plate. It's turned slightly to fit between the screw posts:
  8 deg in the USB case, 16 deg in the battery case.
- **Reception:** the speaker has the spot against the grille, so the antenna sees the sky through
  the speaker. It works best outdoors with the back towards open sky.
- **Settings:** `GPS_ANTENNA` takes your antenna's size and `GPS_PLACE` pins its position by hand.
  `GPS_ANTENNA = None` builds the case for a board without GPS.
- **Firmware:** `firmware/src/lc76g.rs` releases the module's reset (TCA9554 pin P7) and reads it
  over I2C, asking at 0x50 and reading at 0x54. Waveshare's
  [hardware reference](https://github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75/blob/main/HARDWARE_REFERENCE.md)
  also lists a UART on GPIO17 (TX) and GPIO18 (RX), but its resistors are marked NC/0R and are
  probably not fitted.

## Changing it

```sh
uv run build.py    # ~20 s: rebuild everything in out/
uv run check.py    # ~5 min: fit-test both cases against the board's STEP model
uv run render.py   # PNG previews in previews/
```

| File | Contents |
|---|---|
| `params.py` | Everything you might tune, with comments: text, font, fit gaps, wall and flange sizes, line pitch, button size, speaker and battery sizes. If a word no longer fits between its screws, `build.py` stops and says so |
| `board.py` | Board facts measured from the STEP model: positions, depths, keys, ports |
| `parts.py` | The geometry, one small function per feature: body, posts, rails, grille, cutouts, art, button caps |
| `stack.py` | What lies under the board (speaker, GPS antenna, battery): where each goes and how deep the case must be |
| `check.py` | Fit tests: no overlaps, the board lowers straight in, buttons hit their switches, the battery, speaker and plugs fit, safe screw lengths |

Every file uses one frame: looking at the screen with the game upright, +X is right, +Y is up,
+Z comes out of the glass, and Z = 0 is the glass face.

For a live 3D view while you edit, install the VS Code extension "OCP CAD Viewer", run
`uv add --dev ocp_vscode`, and `show(parts.case())`. You can edit the STEP files by hand in any
CAD program, but those edits don't flow back into the Python.

## If your parts or printer differ

- **Speaker:** `SPEAKER` is 20 x 30 x 4.5 mm, Waveshare's 2030 cavity speaker. If yours differs,
  set it: the floor drops to fit and `check.py` confirms the result. The rails are spaced for the
  speaker's width.
- **GPS antenna socket:** the board model doesn't label the two U.FL sockets. GPS is the one
  beside the LC76G, nearer USB-C.
- **Plug room:** `PLUG_ROOM` leaves 3.5 mm below the BAT/SPK sockets for a mated MX1.25 plug with
  its wires bent away.
- **Button feel:** if a button is pressed all the time, raise `BUTTON_ACTUATOR_GAP`. If it's
  mushy, lower it.
- **Orientation:** USB-C sits at 9 o'clock with the game upright, so the back art reads upright
  when the device is turned over left to right. `WHALE_ROTATION` turns the whale; the text stays
  laid out around the screw heads.
- **Tolerances:** they're tuned on a Bambu P2S, which prints a little fat: a 0.3 mm gap at the
  glass and 0.25 mm all round each button cap (`BUTTON_HOLE_GAP`). If the glass is tight in the
  fit ring, raise `GLASS_GAP`; if the caps don't slide freely, raise `BUTTON_HOLE_GAP`.
- **Screw holes:** printed holes come out small, and an M2 screw that has to cut its way in can
  crack a post. The shank hole is 2.7 mm (`SCREW_CLEARANCE_D`) and the head recess 4.4 mm
  (`SCREW_HEAD_D`). The screw should slide through the post and only bite in the board.
- **USB-C:** the opening is 11.5 x 6.0 mm (`USB_OPENING`), raised 0.9 mm above the modelled port
  (`USB_RAISE`) after two test prints. A cable's plug head has to fit through it to seat.
- **Buttons:** the caps sit 0.3 mm below the keys' modelled centre (`BUTTON_DROP`) and stand
  1.4 mm out of the rim (`BUTTON_PROUD`), both from test prints.
- **Glass seat:** `SEAT_GAP` is the gap under the glass, `SEAT_LEDGE` how far the ledge reaches in.
