# Devlog beats

Raw material for a devlog about building `no_std` Bevy games for the Waveshare
ESP32-S3-Touch-AMOLED-1.75: a 466x466 round AMOLED with touch, an IMU and two buttons.
Each beat is one story: what we wanted, what got in the way, and the numbers.
Images are in `img/`. Newest beats go at the bottom.

---

## 1. Gold and hats on a 284 KB heap (2026-10-05)

- **The ask:** catches earn gold that buys little hats, and it all persists.
- **The constraint:** Bevy must fit in about 284 KB of internal SRAM, because atomics are
  unreliable in PSRAM on the S3. Angler had 82 KB of headroom.
- **Design driven by the budget:**
  - No Bevy states: the shop is one more variant of the game's phase enum.
  - No new systems: one `Inventory` resource.
  - Hats are text-art sprites in flash, so they cost no RAM. Shapes compare sprites by
    pointer, so diffing a frame never touches the pixels.
- **Cost:** a peak of 195 to 198 KB, about 3 KB.
- **The neat trick:** the straw hat was baked into the angler sprite. It became a separate
  sprite over a new "hair row", with the body moved down two rows so the hands, and every
  cast arc, stayed on the same pixel. The regenerated aim, cast and waiting screenshots came
  out byte-identical, which proved the split changed nothing.
- **Economy:** about 11 gold per typical catch. The first hat costs about 1.4 catches, the
  crown about 46, and the whole shelf about 100. A test pins the pacing.
- **Images:** `01-title-before-hats.png`, `02-hat-mirror-sheet.png` (all eight hats in the
  shop's mirror), `02-hat-shop.png`, `02-dock-cowboy-hat.png`, `01-catch-before-gold.png`
  and then `02-catch-pays-gold.png`.

## 2. There was no way home

- While adding the shop: every result screen (caught, escaped, notice) went back to aiming.
  Nothing ever returned to the title.
- So LOG, SETTINGS and the new SHOP were reachable only by power-cycling the board.
- **Fix:** a MENU button on the aiming screen, shown only until you start charging a cast,
  so a steering-wheel grip on the glass can't hit it. Image: `03-menu-button.png`.

## 3. A reset button you can't hit by accident

- RESET in settings opens "RESET PROGRESS?". ERASE is framed in red.
- KEEP sits exactly where RESET was, so a nervous double-tap cancels instead of erasing.
- The difficulty survives; it's a preference, not progress. Image: `04-reset-confirm.png`.

## 4. The board arrives (2026-10-06)

- **First serial log:** octal PSRAM up, the display ready, Bevy built entirely in internal
  SRAM (about 72 KB free during play), and the speaker codec answering `0x83 0x11`.
- **Measured frame costs:**
  - an idle frame: about 5.4 ms (60 fps);
  - waiting or fighting: 6-7 ms;
  - a full-screen change: 34-98 ms against a 22 ms wire time, so rendering dominates.

## 5. "Lag on the reel power": a 190,000-point arc

- **The report:** heavy lag while the power increases. It turned out to be the cast power
  meter, not the fight.
- **The log** had 360-509 ms frames, which meant 2-3 fps while charging. That also made the
  0.1 s PERFECT window nearly impossible to hit.
- **A host profiler** with the real presenter put charging at 2,707 us/frame against at
  most 9 us for every other screen: 300-1000x more.
- **Root cause:** embedded-graphics 0.8 draws a stroked arc by testing every point of its
  whole circle's bounding box, and its own source says so (`PERF: The distance iterator
  should use the smaller arc bounding box`). The r=212 gauge is about 190k point tests per
  call. The presenter draws in 8-row bands, so it paid that per band, for three arcs.
- **Fix:** our own arc rasterizer walks only the band's rows inside the arc's bounds, with
  integer annulus spans per row and a fixed-point sector test.
  - Whole circles match embedded-graphics pixel for pixel.
  - Charging went from 2,707 to 25 us (108x), and the worst frame from 7,074 to 110 us.
  - Orbit's paddle got the same speedup for free.
- **The rule now in CLAUDE.md:** a shape's draw cost must scale with the band, not the shape.
- Image: `06-power-gauge-arc.png` (the gauge in question).

## 6. Re-level after every fish

- After a fight your hands end up tilted, so the next cast started at a weird angle.
- `Tilt::request_zero()` re-levels quietly when you continue past a catch or an escape,
  without the BOOT button's "LEVEL SET" toast.

## 7. Sound effects from a 150-line Python script

- **The clips:** ten effects (cast whoosh, PERFECT chime, plop, thud, bite, reel click,
  catch jingle, escape "wah-wah", coin, click), synthesized with Python's standard library
  only and a fixed seed. 117 KB of flash, no RAM.
- **One voice, so rules:** each update picks one sound. The event's own sound wins, a button
  click comes second, and the catch jingle is queued after the last reel click so it isn't
  cut off.
- **The reel clicks** each time the catch meter rises 3%, so it goes quiet while the fish is
  out of the zone. Feedback for free.
- **`SoundQueue` keeps only its newest 16 commands,** because the heap and QEMU harnesses
  never drain it.

## 8. How full is the board?

- **Flash:** Angler's image is 1.52 MB of a 16.4 MB app partition (9.3%). A third of it is
  the 15-second GUTEN song (485 KB).
- **The real limits** are internal RAM (about 76 KB of heap headroom) and frame time.
  Sprites, sounds and level data are free as long as they're `static`.

## 9. The time GUTEN didn't play

- **Bug 1 (the real one):** the I2S stream only restarted after an underrun if the whole
  buffer was free. Say a frame stall (a flash save, a 98 ms redraw) drained the DMA while
  `pump` was pushing. The pushed bytes sat in a dead transfer, the buffer never read as
  free, and the speaker stayed silent until reboot with nothing in the log.
  - esp-hal's docs say it plainly: "If all the descriptors run out ... the DMA stops
    writing. The transfer must then be resumed or restarted."
  - Fix: restart on `is_done()` alone, retry failed starts, and log restarts.
- **Bug 2:** one dropped touch read looks like lift-then-touch, which is a new tap, and any
  tap stops GUTEN. Fix: a failed read changes nothing, a release needs two empty reads, and
  GUTEN ignores taps for its first 0.6 s.

## 10. Battery, sleep and the PWR button that can't wake anything

- **Battery:** the AXP2101's registers came straight from its datasheet (v1.4 §6.13): VBUS
  and battery-present bits, charge direction, a 14-bit battery voltage and a fuel-gauge
  percentage. The menus show it, and play shows it only when low.
- **Schematic sleuthing:** the PMIC's IRQ pin (`AXP_IRQ`) goes only to I/O expander pin
  P5, and the expander's INT output reaches no ESP32 GPIO. So PWR physically can't wake the
  chip from deep sleep. BOOT (GPIO0, an RTC pad with an external pull-up) can.
- **Sleep:** hold BOOT, and the amp, touch, IMU and panel go off before deep sleep. Wake
  is a reset, so the game boots afresh, with saves already in flash.
- **"Use Bevy settings to minimize power":** in `no_std` there aren't any. Bevy's power
  knob is `WinitSettings`, which belongs to the windowing stack this build doesn't have.
  The loop is ours, so:
  - halt the CPU with `waiti` until a timer interrupt, instead of busy-waiting about 11 of
    every 16.7 ms;
  - after 30 s untouched, dim the panel to 25% and run at 20 Hz.
- Images: `10-battery-on-title.png`, `10-hold-to-sleep.png`.

## 11. Becoming an arcade (in progress)

- **The ask:** a boot menu choosing between three games (Angler, Spider Roulette, a
  skateboard side-scroller) plus global settings with mute. Device rotation is the primary
  control everywhere.
- **First problem:** Angler and any other game shared one flash record. Now the record
  holds a section per game plus a shared `PREF` section. Old Angler saves still load,
  because their magic (`FLOG`) doubles as the section name.
