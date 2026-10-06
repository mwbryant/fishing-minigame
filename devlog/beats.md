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

## 11. Becoming an arcade

- **The ask:** a boot menu choosing between three games (Angler, Spider Roulette, a
  skateboard side-scroller) plus global settings with mute. Device rotation is the primary
  control everywhere.
- **First problem:** Angler and any other game shared one flash record. Now the record
  holds a section per game plus a shared `PREF` section. Old Angler saves still load,
  because their magic (`FLOG`) doubles as the section name.
- (See beats 12-19.)

## 12. "The sleep is a bit too fast"

- A 1.5 s BOOT hold slept the board, and GOOD NIGHT flashed by before the panel went dark.
- **Now:** the hint appears at 0.5 s, sleep starts at 3 s, and GOOD NIGHT stays up 2.5 s.
- **The bigger change:** sleep and idle moved out of Angler into one core system. A shared
  `orbit_ui` crate (buttons, battery, sleep overlay) means every new game gets them free.

## 13. Rotation as the main control: a dial menu

- **The ask:** "Remember to use rotation as the primary control interface."
- **On a round screen, a dial is the obvious menu.** Turning the device sweeps a gold arc
  round the rim at 3x the turn, so about +-60 degrees of wrist reaches every option. The
  ring it points at lights up and its name fills the middle; PWR picks.
- **Picking a game ends the menu app with Bevy's own `AppExit`.** The firmware drops it and
  builds the game in its place, so only one Bevy app is ever in memory. That matters with
  284 KB of heap.
- **The menu re-levels the IMU when it appears,** so however you hold the device points at
  the top.
- **SETTINGS is a two-option dial** with a global mute, saved for every game.
- Images: `13-arcade-dial.png`, `13-arcade-dial-five.png` (after Bees??? joined),
  `13-arcade-settings-mute.png`.

## 14. Four games, one flash record

- **The problem:** the board keeps its save in two flash sectors, and every game wrote the
  whole record. Flash Angler, then any other game, and the fish log is gone.
- **Now:** the record holds named sections (`FLOG`, `SPDR`, `SKAT`, `BEES`, and `PREF` for
  shared preferences).
- **The trick:** Angler's old saves start with its magic `FLOG`, so a pre-sections record
  loads as the section named by its first four bytes. The fish log on the board survives
  the upgrade.

## 15. Spider Roulette

- **The game:** twelve eggs ring a web, and one hides a spider. Turn to aim, press PWR to
  crack.
- **Tension, cheaply:**
  - the odds in the middle tick down ("1 IN 12" ... "1 IN 2");
  - a synthesized heartbeat speeds up from one every 1.25 s to one every 0.35 s.
- **The pounce:** the spider jumps at the glass in three bounds over a red flash, with a
  screech (a rising hiss over a sour saw).
- Images: `15-spider-pick.png`, `15-spider-pounce.png`.

## 16. Skate: rotation is the board

- **The controls:** turn counterclockwise (nose up) to pop an ollie. In the air the board
  follows your turn, and you have to level out to land.
- **One pop per turn:** the device must come back toward level before the next one.
- **The obstacles:** cones and benches. Land level on a bench to grind.
- **Proof it's playable:** a test rider who pops just before each obstacle survives 20 s of
  a street speeding from 220 to 420 px/s. The same rider passes in QEMU.
- **Render budget:** the night city is static; only the rider, obstacles and lane dashes
  move, so frames stay small.
- Images: `16-skate-ollie.png`, `16-skate-grind.png`.

## 17. Bees???: the Moon is a jar of honey

- **The user's pitch:** "the moon being a hollow jar of honey orbiting the earth and you must
  prevent it from spilling by rotating the device."
- **The design:**
  - an unseen wobble keeps tipping the jar, and its reach grows over time;
  - your turn counters it at 2x;
  - the fuller the jar, the less tilt it takes to spill: 40 degrees full, 100 nearly empty.
- **Then: "the bees should be wearing the selected hat."**
  - Bees??? reads Angler's save section for the worn hat. Saves are sections now (beat 14),
    so another game's state is just a lookup.
  - The hats were drawn for the angler's head, so each is seated by its head columns
    (4-8 on the angler's 14-column grid) over the bee's head, and mirrored when a bee flies
    left.
- Images: `17-bees-orbit.png`, `17-bees-spilling.png`, `17-bees-in-cowboy-hats.png`.

## 18. Hats for the other games

- **The new hats:** a BEE HAT (stripes, antennae, little wings), a SPIDER HAT (legs draped
  over the head, a red eye) and a SKATE HAT (a deck balanced wheels-up).
- They slot into the shelf by price.
- Image: `12-bee-spider-skate-hats.png`.

## 19. What QEMU caught

- **A leak check:** the arcade smoke launches each game from the menu and sends it HOME,
  twice round. The internal heap back at the menu was 194 KB at the first return and 194 KB
  at the worst of eight. Dropping a Bevy app really gives its memory back.
- **A fragile script:**
  - the old Orbit smoke counted frames ("finger up for 60 of every 900");
  - one run's 2,000 emulated frames held only 4.7 s of game time, where the previous run
    had 26 s, and the round never restarted;
  - the handbook already said "script by game time, never frame counts", and now every
    script does.
- **Stack drift:**
  - the new CPU-halt interrupt handler lives in IRAM, which took about 5 KB of DRAM from
    the firmware's stack;
  - the emulator harness's heap grew to match, so its measurements stay honest.
- **Heap per app, at peak, out of 284 KB:**

  | App | Peak |
  |---|---|
  | Menu | 182 KB |
  | Spider Roulette | 185 KB |
  | Skate | 191 KB |
  | Bees??? | 192 KB |
  | Angler | 204 KB |
