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

- **Flash:** Angler's image is 1.52 MB of a 16.4 MB app partition (9.3%).
- **The real limits** are internal RAM (about 76 KB of heap headroom) and frame time.
  Sprites, sounds and level data are free as long as they're `static`.

## 9. The time the speaker went silent

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

## 20. Spider Roulette becomes a real roulette

- **The feedback:** "it doesn't need orbit select and the button should start the spin,
  there should just be a dancing spider in the middle."
- **The lesson:** rotation-first is a default, not a rule. Aiming at an egg made a chance
  game feel like a skill game, and it was fiddly.
- **The spin:**
  - PWR whirls a pointer round the ring: three turns plus the way to a random whole egg,
    easing out over 2.4 s;
  - a tick at every egg it passes, so the slowdown is audible;
  - wherever it stops cracks.
- **The dancer:** a purple spider in the middle bobs and sways to the heartbeat and turns
  round every four beats. When its egg comes up, it's the one that pounces at the glass.
- **Testable chance:** `rig_next_spin(egg)` lets tests and the QEMU smoke choose where a
  spin stops, so "the spider was where it was hidden" is still checked end to end.
- Images: `20-spider-dancer.png`, `20-spider-spin.png`.

## 21. Skate: a halfpipe every 150 m

- **The ask:** "every 150 units or so in skateboard there should be a drop into a halfpipe
  freestyle section for like 7s."
- **The drop:** the street stops spawning 20 m early so the rider never drops in
  mid-ollie, then the scene swaps to a halfpipe carved into concrete under the night sky.
- **The ride:** a pendulum on a 175 px circle. Each coping launches the same 0.7 s air, so
  every wall is a fair chance at a trick.
- **Rotation again:** in the air, the skater spins 6x the device's turn since takeoff, so
  about 60 degrees of wrist is a 360. Land near a whole half turn for AIR, 180, 360, 540 or
  720 (5 to 90 points). Land crooked and it's a SLAM: no points, but the run goes on.
- **A rotatable skater:** the street skater is a sprite, which can't turn. In the pipe it's
  lines and discs instead, built around a "board up" vector, and airs spin about the hips
  so a 180 doesn't swing the head off the round glass.
- **Cost:** no new systems or resources, just fields on `Ride`. Peak heap went from 191 to
  194 KB.
- Images: `21-skate-drop-in.png`, `21-skate-air.png`, `21-skate-360.png`.

## 22. The clown hat

- **The ask:** "clown hat".
- **Three drafts at 14 columns:**
  - a two-row pom-pom read as a red medical cross, so the pom is one pixel;
  - an orange wig across the top of the head read as a sombrero brim;
  - what worked: a polka-dot cone keeps the top of the head and orange tufts sweep up either
    side, Bozo style.
- **Free on the heap:** a hat is a static text sprite in flash, so the twelfth costs nothing
  at runtime. It sits at 100 gold, between the cowboy and spider hats.
- **The bees wear it too**, since Bees??? reads Angler's worn hat.
- Images: `22-clown-hat-shop.png`, `22-clown-bees.png`.

## 23. Fishing Minigame: the arcade becomes a town

- **The ask:** "reconfigure into more of a game loop like Nintendogs":
  - catch fish and keep three, named;
  - walk them for stamina;
  - teach them five tricks of varying rarity with the mics and touch;
  - win gold in contests;
  - Spider Roulette as the gambling den, Skate as an arcade cabinet and Bees??? as a tree,
    all found on an overworld map.
- **The architecture is the heap budget:**
  - every place is its own Bevy app, built when you walk in and dropped when you leave;
  - the town is the hub the board boots into;
  - the kept fish, gold and hats pass between apps in one `PALS` save section, about 2.3 KB
    for a full tank of taught fish.
- **Walking around:** six round clearings joined by paths at the compass points.
  - Turning the device walks you left and right; leaning walks you up and down; or tap
    where to go.
  - PWR at a door goes in, and you come back out at the same door.
- Images: `23-town-home.png`, `23-town-main-street.png`.

## 24. The pond: keep, name, release

- **Catches pay no gold now:** the pond is where pets come from.
  - A landed fish is KEEP (held level) or RELEASE (turned right).
  - A full tank asks who goes back.
- **Typing by turning:** A to Z, SPACE, DEL and OK ring the glass, and the turn points at
  one, as the menus do.
  - 29 keys at 3x gain is about 4 degrees of wrist per letter.
  - A little hysteresis stops a hand held on a boundary flickering between two letters.
  - DEL and OK sit just counterclockwise of A, a small turn left from level.
- **Old saves:** the gold and hats from Angler's shop carry over into the tank, once.
- Images: `24-keep-or-release.png`, `24-naming-by-turning.png`, `24-tank-full.png`.

## 25. Voice commands without speech recognition

- **The idea (Nintendogs' trick):** don't recognize words; recognize *your* words.
  - Each word becomes a 145-byte print: five frequency bands' loudness plus the hiss, every
    16 ms, stretched to 24 frames.
  - A command is the nearest taught print by dynamic time warping, if it's near enough.
- **Real speech for the tests:** macOS `say` made 20 clips (two voices, five trick words,
  normal and fast). A test command is the fast version at 35% volume in room hiss, matched
  against that speaker's normal recordings.
- **First try: 9 of 10.** Fred's fast "flip" came out nearer his "jump".
- **The cause was the soft "f":** at 35% volume it fell under the speech threshold, so the
  listener heard "lip".
  - Fred's slow "flip" lost 4 of its 32 frames; Daniel's fast one lost 3 of 12.
  - Fix: cut each word's edges against its *own* peak (about -20 dB), with a longer
    pre-roll, instead of a fixed threshold.
  - Lengths came out identical at full and 35% volume, and all 10 matched.
- **The threshold:** correct matches came out at 12 to 45, other words at 33 and up, a
  whistle at 94 or more, so the cutoff is 60. A clap no longer even registers as a word.
- Image: `25-say-a-word.png`.

## 26. The GPS is on I2C

- **The board:** it's the -G variant (SKU 31264), with a Quectel LC76G and a ceramic antenna.
- **Research said:**
  - The module talks over the shared I2C bus (0x50, 0x54, 0x58). The UART pads are
    probably unpopulated (resistors marked NC).
  - It has no power enable, so it draws about 36 mA whenever the board is on.
  - A Quectel forum thread reports the I2C side locking up on a busy shared bus. Touch and
    the IMU share this one.
- **The game doesn't care where the bytes come from:** an NMEA parser (RMC and GGA, any
  talker, checksummed) feeds a trail that:
  - only counts once you're 8 m from the last counted spot, so jitter doesn't count;
  - ignores fixes with an HDOP over 5;
  - skips anything over 4.5 m/s (a car).
- **MOCK WALK** in the town's settings strolls at 1.4 m/s through the same trail, for
  indoors and the desktop.
- **The mics share the speaker's clocks:** esp-hal's I2S signal loopback lets RX share TX's
  BCLK/WS. So capture needs the speaker stream running, even if it's only silence.

## 27. The house: a tank, cards and trick school

- **The tank:** your fish swim in it wearing their hats. Turn to point at one; its card shows
  species, size, stamina, appeal and the five tricks, coloured by rarity.
- **Teaching:** watch the trick, then say a word twice into the mics or draw a stroke twice
  (tap, hold, swipe four ways, or a circle either way). A word or stroke another trick
  already uses is refused.
- **Practice:** give the command and the fish tries, by how well it knows the trick. Pet it
  right after a success to double the lesson.
- **A bug the tests caught:** the petting tap also lifted as a Tap command, and its "LOOKS
  PUZZLED" overwrote "GOOD FISH!".
  - Fix: strokes are only read on screens waiting for a command.
  - A press that starts anywhere else, petting included, is never one.
- Images: `27-the-tank.png`, `27-a-card.png`, `27-pet-it.png`.

## 28. Walks build stamina

- **Levels:** stamina level is 1 + √(metres / 100), so levels come at 100 m, 400 m and 900 m.
- **Saving:** progress saves every 25 m, so a flat battery loses little of a walk.
- **On screen:** the fish rides in a bowl on wheels, the rim fills toward the next level,
  and a level chimes as it comes.
- Image: `28-a-mock-walk.png`.

## 29. Contests, and a clock that ran at 60%

- **The show:** 30 seconds of live commands.
  - Each trick scores its rarity's points, raised by how well it's known, with a combo for
    changing it up.
  - Every few seconds a judge calls a trick, worth double if done in time.
  - Each try costs stamina, and a spent fish ends its show.
  - Looks (rarity, size, hat) add their appeal; three rivals decide the place.
- **The bug:**
  - the show kept time in hundredths of a second so it could compare exactly;
  - each 1/60 s frame truncated to a single hundredth, so the show ran at 60% speed;
  - five timing tests failed at once. Plain seconds fixed all five.
- Images: `29-a-judge-wants.png`, `29-first-place.png`.

## 30. The hat shop goes round

- **Rotation first:** every hat rings the glass, turning points at one, and the mirror in
  the middle shows your fish wearing it.
- **Pacing, pinned by a test:** the first hat is one beginner win, and the crown costs more
  than an expert win but one master win buys it.
- Image: `30-hat-shop.png`.

## 31. The den's odds

- **Fair odds less a tenth:** with k safe eggs out of 12, the odds of having got that far
  are (12 - k) in 12, so a bet is worth `stake x 12 x 90% / (12 - k)`.
- **What that means:** cashing out after one egg loses money, 3 safe eggs pay 1.2x, and
  trapping the spider pays 10.8x.
- **Controls:** PWR still just spins (beat 20's lesson); CASH OUT is a tap.
- Images: `31-place-your-bet.png`, `31-cash-out.png`.

## 32. A 3 KB resource that cost 204 KB of heap

- **heap32 flagged two apps:** the house peaked at 518 KB and the contest at 403 KB, against
  a 284 KB budget. Both already cost about 400 KB just to *build*.
- **Narrowing it down:** the one thing they shared was `MicPlugin`.
  - A bare app was 172 KB; the same app plus `MicPlugin` was 376 KB.
  - `MicPlugin` adds no systems, just the `Mic` resource, and the listener inside is only
    3,248 bytes.
- **The experiment:** a plain resource holding `[u8; 3248]` also cost 204 KB.
- **The arithmetic:** 64 x 3,248 = 203 KB. Bevy 0.19 seems to keep resources in storage that
  reserves room for about 64 of each, so a resource costs roughly 64 times its size in heap.
- **The fix:** box the big part. `Mic` is now 8 bytes, the listener's 3 KB is a single heap
  allocation, and the mic app went from 376 KB to 177 KB. The GPS buffer got the same
  treatment.
- **A second false alarm:** the house still spiked to 319 KB. That was the measurement's own
  fault.
  - The test helper that "says" a word built one 45 KB buffer of samples, which doubled
    while it grew, and the harness held a 17 KB copy of the fixture.
  - Streaming the samples (as a mic driver does) and a compile-time `static` fixture brought
    the house to 211 KB.
- **The rule now:** keep resources small (under about 100 bytes) and box anything bigger.
- **Heap at peak, of 284 KB:**

  | App | Peak |
  |---|---|
  | Shop | 189 KB |
  | Spider Roulette | 191 KB |
  | Town | 192 KB |
  | Bees??? | 193 KB |
  | Skate | 194 KB |
  | Park | 202 KB |
  | Contest | 204 KB |
  | Angler (the pond) | 207 KB |
  | House | 211 KB |
  | Orbit | 265 KB |

## 33. Drivers from research, and what QEMU can and can't say

- **The mics:**
  - the ES7210 needs no clock of its own; esp-hal's I2S *signal loopback* runs the
    receiver on the speaker's BCLK and WS;
  - so the mics only hear while the speaker streams. The speaker already streams silence
    from the first app with sound, so that's the town;
  - the ADC init is esp-adf's slave-mode register sequence, about 40 writes.
- **The GPS:**
  - Waveshare's demo reads the LC76G with 10 ms waits between I2C steps, and two of those
    would eat a whole frame;
  - the firmware runs it as a state machine, one I2C step per frame, about once a second.
- **What QEMU checked:** the Fishing Minigame tour, every door twice round. The heap back in town
  was 200 KB at the first return and 202 KB at the worst of 16, so dropping each app gives
  its memory back.
- **What it can't check:** neither driver runs in QEMU; there's no ES7210 or LC76G in it.
  Both are compiled and wired, and unverified on the board.
- **A small calibration:**
  - the drivers took about 1 KB of the firmware's DRAM, so its stack shrank;
  - the emulator harness keeps its stack within 1 KB of the firmware's so stack overflows
    show up in both;
  - its heap went from 237 to 238 KB to match.

## 34. Walking the town is touch

- **The feedback:** "movement on the overworld should be touch, not rotate." Turning and
  leaning to walk was rotation-first taken one step too far; rotation picks things, it
  doesn't steer you around a map.
- **Now:** tap where to go, or hold a finger down and you follow it as it drags. Tapping the
  end of a path means "down the path", so the target is pushed past the edge of the glass.
- **The detail a test caught first:**
  - hold a finger at the east end of a path and you walk into the next clearing;
  - you come out at its *west* edge, with the finger still down at the *east* edge;
  - so it would walk you across and straight back out.
  - The fix: a finger that took you down a path is spent until it lifts.
- **Rotation stays for the menus:** the settings dial, the house's lists, the shop's ring of
  hats.
- Image: `34-tap-where-to-go.png`.

## 35. Voice commands: too hard, too easy, then the middle

- **The first matcher:** five band loudnesses and the zero-crossing rate every 16 ms, DTW
  to the nearest taught word, accepted under 60. It was tuned on `say` speech: two voices,
  each word at two speeds.
- **The feedback:** "the word matching seems a bit too hard and finicky." A synthesizer
  says a word the same way every time; a person doesn't.
- **Measuring it:** a diagnostic said every fixture higher, lower, muffled, echoing, in a
  noisy room and quietly, and printed how far each landed from its own taught word.
  - "flip" stayed under 30 every way.
  - "spin" landed 141 muffled and 140 noisy, more than twice the cutoff.
  - **Why:** the "s" is a soft hiss. Muffled or in noise it falls under the edge cut
    (20 dB below the word's peak), so the print starts at "pin".
- **The first fix:**
  - DTW was allowed to skip a word's edge frames cheaply, and muffled "spin" went from 141
    to 49;
  - seven band-pass filters replaced the five bands, each against the frame's mean, so the
    print is the spectrum's shape, not how loud it was;
  - a sweep scored the settings over all 70 cases (2 voices x 5 words x 7 ways).
  - Edge skipping also made the wrong word nearest more often, and the cutoff stayed at 60
    to favour hearing commands.
- **The feedback again:** "now it seems a bit too over fit and easy."
  - A cutoff wide enough for a muffled "spin" takes in other words too.
  - The fish also adapted the taught word toward each command, which compounds it: one
    false match pulls the word toward the impostor and makes the next one likelier.
- **The middle:**
  - plain DTW again, no edge skipping;
  - a cutoff of 34;
  - the nearest word has to beat the runner-up by 15%, so a word halfway between two taught
    words is neither;
  - the fish only adapts on a confident match (under 70% of the cutoff);
  - teaching's second take must sound like the first, or the screen says THAT SOUNDED
    DIFFERENT.
- **What it gave up:** muffled words and loud rooms are no longer promised. 54 of the 70
  harsh cases pass.
- **The tests now measure both sides:**
  - 47 of 50 mild variations are recognized (faster, higher, slower and lower, echoing,
    quiet in a louder room);
  - 114 of 120 untaught words are ignored;
  - claps and whistles never match.
  - **The lesson:** a test that only checks "recognized" tunes the cutoff open. Measure the
    impostors in the same sweep.
- **Side costs:**
  - A print grew from 145 to 193 bytes. The save went to `PALS` v2, which loads a v1 tank
    with its fish and strokes, but the words need teaching again.
  - The 64x resource rule (beat 32) struck again: a phase variant holding a print made the
    house's `Home` resource 328 bytes. Boxing the phases took the house from 216 to
    204 KB, the contest from 206 to 201 and the park from 202 to 196.

## 36. Mock walks fast-forward

- **The ask:** "mock walk should let the user change walk speed."
- **Why it matters:** stamina level n comes at 100 x (n - 1)^2 metres, and MASTER contests
  want level 10: 8,100 m, or about 96 minutes of mock walking at 1.4 m/s. At x25 it's under
  4 minutes.
- **The catch:** at x25 the mock walker moves 35 m/s, and the trail rejects anything over
  4.5 m/s as a bike or a car.
  - So a mock fix covers fast-forwarded distance but still reports walking speed. The
    trail's filters stay as they are for real walks, and a mock walk still runs through the
    same code.
- **The controls:**
  - the town's MOCK WALK steps OFF, x1, x2, x5, x10, x25 and back to OFF;
  - on a mock walk, - and + beside the distance change the pace mid-walk;
  - the pace fits in three spare bits of the `PALS` flags byte, so the save version
    didn't change.
- Image: `36-fast-forward.png`.

## 37. A trailer, filmed by the game

- **The ask:** a whimsical trailer for the FISHING MINIGAME:
  - FISH, TRAIN, WALK, COMPETE, HATS at slight angles, zooming in and out;
  - then BEESSSS????, skateboarding and spiders;
  - a crude 3D man eating the printed case: YOU CAN EAT IT;
  - the guten morgen fish, held for 5 seconds, then HAPPY BIRTHDAY ETHAN beside it.
- **The camera is the game:**
  - `orbit_desktop footage` runs scripted takes headless, with the same test helpers and
    `rig_*` hooks as the snapshots.
  - A `Last` system saves every frame the presenter draws, and mixes the `SoundQueue`
    through the real one-voice `Mixer` into a WAV. Every take carries its own game sounds.
  - All 14 takes film in about 7 seconds.
- **The cut:**
  - `trailer/build.py` composites the takes into the reel case's green rim on striped
    backdrops.
  - The cozy half cuts on the beat of "Happy Whistling Ukulele" (130 BPM, seven beats to a
    shot). A record scratch dragged out of the same track hands over to the US Army Band's
    "Flight of the Bumblebee".
- **The man:**
  - Blender (`trailer/eat.py`) loads `case/out/reel_case.stl` and shows a frame of the
    tank on its glass.
  - Bites go front to back through the case as boolean cylinders. Balls only notched the
    middle of the side wall, which can't be seen from the front.
  - The MANIFOLD solver cuts both bites in 0.2 s against EXACT's 30.
  - His mouth opens wide for each bite and chews after it, and a synthesized crunch lands
    each time it shuts.
- **What it leaves out:**
  - no voice lines: YOU CAN EAT IT is only on screen;
  - no BEES card: BEESSSS???? sits beside the bees, like the skate and spider labels. Its
    1.6 s went to the gameplay shots (six beats each became seven) and to the YOU CAN EAT IT
    words, which now come in just after the second bite and hold for 4 s.
- **Regenerating:** `uv run build.py --refilm` once the footage is final. `--sheets` shows
  where each take's moments fall, for the cuts in `trailer/edit.py`.
- Images: `37-trailer-fish.png`, `37-you-can-eat-it.png`.

## 38. No text gets in anything's way

- **The ask:** "Ensure that no where in the game there is text overlapping."
- **Eyeballing doesn't scale**, so the renderer got a checker: `orbit_gfx::crowding`.
  - It measures the box a label's letters actually ink, not its 10x20 font cells. A
    capital uses 13 of the cell's 20 rows, so cells would flag lines that never touch.
  - Then it renders every other shape alone into that box, twice on two backgrounds (so a
    pixel the color of one still shows on the other), and reports:
    - text within 2 px of text;
    - anything drawn over part of a label;
    - anything under part of it but not all of it;
    - letters past the round glass.
  - Bands across the whole screen (sky, water, ground) count as backdrops.
- **The first survey:** with `CROWDING_LOG`, every frame of the whole test suite.
  - 275,888 findings, 820 kinds once numbers were folded together.
  - Only one was text on text: with five stakes on the bet dial, PLACE YOUR BET ran into
    FREE and 10.
  - Everything else was text half on scenery: the park's path, the contest curtain's folds,
    Spider Roulette's web, Skate's windows, stars, bubbles, picker rings through names.
- **Two refinements before the rule was right:**
  - a door sign hides the path behind it, so only what's above the last shape covering the
    whole label counts;
  - a label the GOOD NIGHT screen hides entirely doesn't count either.
- **The fixes:**
  - **Plates:** text over scenery sits on a plate of the background's color. Lines that
    belong together share one, or slivers of path and windows show between them.
  - **Decor that gives way:** bubbles, aim dots and windows near text are dropped.
  - **Rings:** picker rings go round the fish, with the name below. A pointed name gets a
    frame sized to it.
  - **Leaps:** RAINBOW LEAP rises 140 px, and the tallest fish in a TOP HAT already stands
    47 px above the house's stage. So tricks move 0.65 of their size in the house and 0.7 on
    the contest stage, which was laid out in zones: score, judge's banner, fish, news,
    stamina.
  - **Orbit:** the faint score behind play steps aside while the ball crosses it.
- **Random play found what the scripts didn't.** `testing::monkey` turns, leans, presses,
  holds, taps anywhere and holds BOOT toward sleep. Each app got a run with the most crowded
  data: full tanks of 8-letter names, the tallest hat, 999,999 gold, the top stamina level,
  every trick on both stages. It found:
  - **A crash:** at the top stamina level, "metres to the next level" underflowed. A debug
    build panics; a release build would have shown about 4 billion metres.
  - **Banners:** HOLD TO SLEEP half covered whatever text was behind it, and LEVEL SET
    landed on REELED IN. Both are now notices whose banner clears the text it would cut.
  - **Flight paths:** the bobber flew through PERFECT!, and the aim dots ran through HOLD
    BUTTON TO CAST.
  - **Too wide:** RAINBOW LEAP! was wider than the judge's banner, A HUMMING TREE wider
    than its door sign, and TURN RIGHT TO RELEASE, PRESS ran off the glass.
  - **The shop:** its last line sat under the hats at the bottom of the ring.
- **The trailer's footage found one more.** The tests had never shown a battery. In the hat
  shop, the gold highlight round the hat at 12 o'clock covered the battery level. The ring
  came in from 186 to 176 px, and the random player now plays with the battery showing.
- **Speed:** checking every frame made the full-tank run take 172 s on its own. Two changes
  brought the whole crowding file to 38 s:
  - skipping frames identical to the last one checked;
  - working out a rectangle's coverage arithmetically instead of rendering it.
- Image: `38-before-after.png` (before on the left: the web, the judge's banner over GO!,
  the ring through GILLY, the windows in the score).

## 39. Buttons a bit easier to tap

- **The ask:** "make the buttons a bit easier to tap".
- **The numbers:** 466 px over 44.5 mm of glass is about 10.5 px per mm. A 32 px button is
  3 mm tall, and a fingertip's contact patch is closer to 8 mm.
- **The change:** a button now takes taps up to 16 px (1.5 mm) outside its box.
- **What the test caught first:** on a fish's card, RENAME, TRICKS and BACK are 2 px apart.
  With the slop alone, RENAME, checked first, stole a tap that landed on TRICKS.
  - So a screen picks between its buttons by distance (`orbit_ui::tapped`): the one under
    the finger, else the nearest within reach.
- **Dial spots work the same way** (`orbit_ui::nearest`):
  - fish on a dial reach 64 px, so tapping a fish's name picks it;
  - hats 40, stakes 48, keys 30.
- **One place the slop was wrong:** at a door, GO IN's padding caught taps on the end of
  the path just below the sign, which should walk you down it. Now the whole sign is the
  target, and below it you walk.

## 40. The whole thing on one slide (2026-10-09)

- **The ask:** a diagram of the project's high-level structure, good-looking enough for
  blog posts and YouTube devlogs, saved as PNGs.
- **Two 1080p slides**, drawn by `docs/architecture/build.py` (HTML placed in px, shot by
  headless Chrome, so they re-render exactly):
  - **How it's built:** the town's street of places as real round screenshots, grouped by
    crate. Below them `pals`, `orbit_ui`, then `orbit_core`, and four plain `no_std`
    libraries. Each library has an arrow to what feeds it on the board (orange) and in
    the simulator (blue): QMI8658 or arrow keys, CO5300 or a minifb window, and so on.
    `tools/`, `case/` and `trailer/` run down the side.
  - **How a frame gets to the glass:** `app.update()` → `Frame` → `FrameDiff` → 8-row
    bands → the CO5300 or the desktop window. Then the dirty rects on a real Orbit frame,
    25 KB against 434 KB a frame, and the ping-pong DMA.
- **The layers tell the truth:** `pals` stops short of the arcade, and `orbit_ui` stops
  short of Orbit, because Skate and Orbit don't use them. Touch, buttons and GPS bypass
  the libraries and go straight into `orbit_core`.
- **The numbers:** 337 tests passing, about 25k lines of Rust, 7,456 B per band, about 25 KB
  (1.3 ms) for a normal frame against 434 KB (22 ms) for a full redraw.
- **Honest labels:** the dashed dirty-rect boxes on the Orbit frame are drawn by hand, and
  the DMA timeline is schematic. Both slides say so.
- **Hook:** "Here's the whole game on one slide: a round watch-sized screen, a fish, and
  twenty-five thousand lines of no_std Rust."
- Images: `40-architecture.png`, `40-frame-pipeline.png`.

## 41. Three more slides: the hardware layer, the trailer, the case (2026-10-09)

- **The ask:** a diagram of just orbit_core and how it connects the games to the hardware.
  Also, take the trailer and CAD panels off the overview and give each its own slide,
  "because each are built in cool ways worth discussing."
- **The overview:** the stack now runs the full width, with bigger screenshots and backend
  cards (before: `40-architecture.png`, after: `41-architecture.png`).
- **orbit_core meets the hardware:** one row per channel, from what a game calls, through
  the Bevy resource, to the chip on the board or its stand-in on the desktop.
  - **Inputs:** Touch, Buttons, ImuFeed → Tilt, Power, Mic and Gps are written before
    `app.update()`.
  - **Outputs:** Frame, SoundQueue and SaveBlob are drained after it.
  - **The board side:** seven chips share one 400 kHz I2C bus (touch 0x5A, the TCA9554
    expander 0x20, IMU 0x6B, AXP2101 0x34, ES7210 0x40, LC76G 0x50 and 0x54, ES8311 0x18).
    The panel has its own 40 MHz QSPI link.
  - **The frame loop:** a strip along the bottom shows the firmware's loop at 60 fps, 20
    when idle.
  - **What stood out:** the opt-in plugins work as power switches too. The firmware only
    starts the speaker if the app has a SoundQueue, and only powers the mics if it has a
    Mic.
- **How the trailer was made:** picture and sound are built separately and meet in ffmpeg.
  - **Picture:** 12 scripts film 14 takes (4,192 frames at 466², 60 fps) in about 7 s.
    Blender bites the real `reel_case.stl` (255 frames in about 91 s). `Trailer.frame(t)`
    composites everything in Pillow and numpy.
  - **Sound:** the takes' WAVs go under the two tracks with a synthesized scratch, whoosh
    and crunches.
  - **The cut:** a timeline of all 12 shots shows the 130 BPM beat ticks under the cozy
    half and where each track and sound lands.
  - **Result:** 46.2 s, 1080p30, 28.5 MB.
- **How the case was made:** `params.py` and `board.py` feed `stack.py` and `parts.py`
  (build123d), and the parts fan out to `build.py`, `render.py` and `check.py`.
  - **The fit test:** check.py gets the whole Waveshare STEP as solids. It lowers the board
    in from 16 mm, presses each switch 0.35 mm, and checks that the M2×6 screw bites.
  - **Four tricks get cards:** line rings instead of a helix, art that prints face down,
    words fitted between the screws, and buttons kept from turning.
- **Found on the way, and fixed:**
  - case/README.md said to change filament at 16.0 mm; the 24.6 mm case wants 22.4 (and the
    battery case 29.2), as build.py prints.
  - `previews/button_cap.png` showed the caps before their 45° cone, and nothing made or used
    it any more, so it's gone.
  - The README called the trailer 45 s (it's 46.2), and build.py's help named its stills
    `out/still.png` (they're `out/still-09.50.png`).
- **Hook:** "The game never talks to a single chip. Here's the one layer that does."
- Images: `41-architecture.png`, `41-orbit-core.png`, `41-trailer-pipeline.png`,
  `41-case-pipeline.png`.

## 42. The hardware slide reads left to right (2026-10-09)

- **The ask:** "remove the simulator and reverse the direction flow on the hardware
  diagram."
- **Before:** the game sat on the left and the chips on the right, so inputs ran right to
  left, against reading order. A simulator column took the last quarter of the width.
- **After:** board, then orbit_core, then the game. A touch, a tilt or a word now runs left
  to right into the game, and the frame, sounds and saves run back left to their chips.
  The width the simulator gave up went to bigger type. The desktop stand-ins are still on
  the overview slide.
- Images: `41-orbit-core.png` (before), `42-orbit-core.png` (after).

## 43. Fish Pals is now Fishing Minigame (2026-10-09)

- **The ask:** "Instead of fish pals call it fishing minigame and update that branding
  everywhere." The trailer already called it the FISHING MINIGAME.
- **What changed:** 59 mentions across 17 files: the README, CLAUDE.md and AGENTS.md, the
  platform handbook, code comments, `flash.sh` and the desktop's help, all five slides, and
  the Design canvas (now "Fishing Minigame Architecture").
- **What didn't:** the code's names. `pals`, `pals_game`, `--bin pals` and the `PALS` save
  key stay, because renaming the save key would orphan the fish already saved on a board.
  CLAUDE.md now says so, so nobody "finishes" the rename later.
- **The slides:** FISHING MINIGAME is 7 letters longer than FISH PALS in a wide pixel font,
  so the title went from 64 to 52 px. The longest header, orbit_core's, still clears its
  chips by about 27 px.
- Images: `41-architecture.png` (before), `43-architecture.png` (after).

## 44. The README becomes a pitch (2026-10-09)

- **The ask:** make the top-level README a marketing pitch, then building and running every
  part, then sourcing and printing your own, then the architecture with the new slides. Say
  it was made AI-first, but in paragraph 2, not as a selling point.
- **The order now:**
  - what it is and what makes it cool, with the trailer still on top and a 3-up grid of
    screenshots;
  - build and run it: the simulator, the tests, heap32, qemu-smoke, flashing, the case, the
    trailer and the slides;
  - build your own: parts, printing, assembly, first flash;
  - how it's built: the five slides, then the crates, memory and the emulator notes.
- **What the logs said:** the README and the platform handbook both said the town had never
  run on the board. The logs say otherwise: one session from 2026-10-07 built and dropped 24
  apps going through the town's doors, at 60 fps with 6 to 9 ms of work a frame, the mics
  listening and the GPS answering. Both docs now say what's confirmed and what isn't yet: a
  voice command, a GPS fix, waking from sleep, and battery charging.
- **Cut for honesty:** a line saying the board "costs less than a game cartridge", since
  nothing in the repo backs up a price.

## 45. Cleaning house: license, plain words, and a lighter history (2026-10-09)

- **The ask:** add a license, do "a deep docs cleanup pass over the entire codebase" so a human
  gets each file or system in the fewest words, drop every "untested" note ("It is all tested
  and working"), and take anything downloaded from outside out of git history, with links in its
  place.
- **License:** MIT OR Apache-2.0, as `Cargo.toml` already declared.
- **The docs pass:**
  - six agents took a slice each (core libraries, the pals crates, the games, firmware and
    tools, case and trailer, the handbook) with one style guide;
  - a comment-stripping diff proved 128 code files changed only in comments, and clippy,
    rustdoc with warnings as errors, and all 337 tests passed after;
  - Markdown went from 21,585 words to 21,106, and the handbook from 12,871 to 12,278;
  - Rust comments grew from 17,366 words to 19,140, because files and items that had no doc
    got one line.
- **What the pass caught:** stale comments that said the opposite of the code.
  - PWR is the action button and BOOT the zero button, not the other way round.
  - Standalone games never offer OUT.
  - Fish and bees wear the hats, not the angler.
  - The shop's hats sit 78 px apart, not 83.
  - The case README told you to peel the tape before driving the screws.
- **Untested notes:** gone. The handbook drops its [verified]/[unverified] flags, and the case
  README's "Not verified yet" list became tuning advice.
- **History:**
  - Waveshare's two 3D-model zips and the schematic PDF came out of all 100 commits, along with
    one local-only asset.
  - `docs/WAVESHARE_FILES.md` links the originals, which match byte for byte.
  - The repo went from a 16.4 MB bundle to a 9.8 MiB pack. `HEAD`'s tree is unchanged.
- **A slip worth telling:** a commit with an unchained second step swept in three `.pyc` files.
  It was undone before anything else happened, and `__pycache__/` is now ignored.

## 46. The first print: two screw posts snapped (2026-10-10)

- **The ask:** "the screwposts broke 2/3 on first test, lets add a bit more attachment at the
  base and more tolerance on their screw diameter (I think the 3d print expanded just a tiny
  bit)". Also something so the screen seats more easily, and a smaller USB-C opening sitting
  slightly higher. The buttons wait for another test print.
- **What got in the way:** each post was a plain 6 mm cylinder standing 11 mm off the floor of
  the 24.6 mm GPS case, with a 2.4 mm hole for a 2.0 mm M2 shank. Printed holes come out small,
  so the screw had to cut its way in, and the twist snapped the posts at the floor.
- **The fix:**
  - shank hole 2.4 -> 2.7 mm and head recess 4.2 -> 4.4 mm, so the screw only bites in the board;
  - each post flares into the floor (45 deg, 1.5 mm), and a 1.6 mm rib ties it to the wall
    1.65 mm away (+95 mm3 over the three posts);
  - a ledge 1.25 mm wide runs under the glass rim, 0.1 mm below the glass, notched for USB-C; the
    rim's lead-in chamfer went 0.3 -> 0.5 mm;
  - USB-C opening 13 x 7.4 -> 11.5 x 6.0 mm, raised 0.5 mm.
- **Dead end:** widening the posts to 6.6 mm pushed the GPS antenna's spot 4 mm lower and the
  USB case from 24.6 to 28.6 mm. The posts stayed 6.0 mm and the flare and rib do the work, so
  both cases are still 24.6 and 31.4 mm.
- **Checks:** `check.py` passes both cases. The board still lowers straight in past the new
  ledge, which ends 0.17 mm from the glass and 0.53 mm from the spacer under it.
- **Hook:** "Two of three screw posts snapped on the first print. The fix wasn't a fatter post;
  it was a bigger hole."
- **Media:** `img/46-before-post.png`, `img/46-after-post.png`, `img/46-before-seat.png`,
  `img/46-after-seat.png` (the rim cut through at 10:30), `img/46-before-usb.png`,
  `img/46-after-usb.png`.
  - TODO(user): a photo of the two snapped posts on the first print, and of the second print
    with the board screwed down.

## 47. Every flash ended in "waiting for download" (2026-10-10)

- **The ask:** "flash seems hung on waiting for download. The battery and pc are both
  currently connected."
- **What got in the way:** since 2026-10-07 12:43, every flash finished, verified, and then
  rebooted straight into the ROM's download mode:
  ```
  rst:0x15 (USB_UART_CHIP_RESET),boot:0x23 (DOWNLOAD(USB/UART0))
  waiting for download
  ```
  The workaround had been unplug and replug. With the battery in the case, unplugging USB no
  longer cut the power, so the board just sat there with a black screen, and then stopped
  answering at all until it was held off with PWR for 10 s.
- **The numbers:** read straight off the chip in download mode with esptool:
  `GPIO_STRAP` = 0x23 (bit 3 clear, "BOOT held"), `GPIO_IN` bit 0 = 1 (BOOT actually up),
  `RTC_CNTL_OPTION1` = 0 (no forced download). After a watchdog reset the game booted; one
  more USB download entry and `GPIO_STRAP` read 0x23 again. The last good flash, 12:35 on
  Oct 7, booted with 0x2b.
- **The fix:** a USB reset keeps the boot latch and a watchdog reset re-reads the pin.
  `espflash monitor --non-interactive` already knows this (it checks bit 3 and logs
  `Resetting with RTC WDT`), so `flash.sh` now flashes, then restarts the board through it.
  The board was never broken and BOOT was never stuck.
- **Hook:** "The screen was black, the flash had verified, and the chip insisted I was holding
  a button I wasn't touching."
- **Media:** the serial logs `logs/pals-20261010-113222.log` (stuck) and
  `logs/pals-20261010-115653.log` (the fixed script: flashed, watchdog reset, game running).

## 48. Sleep that wasn't: the battery hit 1% (2026-10-10)

- **The ask:** "The low power sleep does not seem to work as the battery dropped to 1%."
- **What got in the way:** hold-BOOT sleep put only the ESP32 into deep sleep. On this board
  the AXP2101's DCDC1 is the one 3.3 V rail for everything, so the codec, both mics, the
  panel's controller and the LC76G GPS stayed powered.
- **The fix:** after the goodbye, the firmware quiets the peripherals as before, then sets
  AXP2101 REG 0x10 bit 0, which switches every rail off. PWR, the upper button, turns it back
  on as a fresh boot, so the goodbye now says PRESS TO WAKE. Deep sleep stays only as a
  fallback if the power chip doesn't answer. The simulator matches: Z held switches off,
  Space turns it on.
- **The numbers:** after the recovery, on USB, the gauge climbed 1% -> 10% in about 4 minutes
  while the game ran.
- **On the board:** the log said `switching off; press PWR to turn on` at 12:03:23, the USB
  serial port vanished, and it stayed gone for the whole 3-minute watch with USB plugged in
  (no `still powered` line, so the deep-sleep fallback never ran). A PWR press brought the
  game back.
- **Hook:** "Sleep mode is pointless if the GPS doesn't know the rest of the board went to bed."
- **Media:** `img/48-before-good-night.png`, `img/48-after-good-night.png`.
  - TODO(user): a phone clip of holding BOOT through GOOD NIGHT to a dark screen, then PWR
    bringing it back.

## 49. Tap the house to go in (2026-10-10)

- **The ask:** "the buttons on the touch screen are a bit hard to click, can you give them a bit
  more forgiveness. I was struggling to enter the house. also the pond starting hitbox is way
  too small on the overworld map, it should be very easy to enter the points of interest"
- **What got in the way:** going in took two steps. You had to end a walk within 22 px of an
  invisible door spot (about 2 mm of glass) before the sign appeared, then tap the sign. Tapping
  the house itself walked you into its wall, and tapping the lake walked you to the shore,
  nowhere near the dock's door spot.
- **The fix:**
  - a tap on a place's landmark (the house, signpost, bench, hall, shop, den, bee tree, cabinet,
    or the pond's water and dock), give or take 28 px, walks you to its door and goes in;
  - the door counts from 40 px instead of 22, so a walk that stops close enough shows the sign;
  - every touch button takes taps up to 28 px off its edge instead of 16 (about 2.7 mm);
  - taps at the end of a path still walk down the path first.
- **The numbers:** 6 new town and tap tests; 344 tests pass.
- **Hook:** "My own house had a 4 mm door, and I was tapping the roof."
- **Media:** `img/49-before-home-taps.png`, `img/49-after-home-taps.png`,
  `img/49-before-pond-taps.png`, `img/49-after-pond-taps.png`. These are the real town
  screenshots with the tap zones drawn over them in pink, so they're annotated, not footage.
  - TODO(user): a phone clip of tapping the lake from across the clearing and walking straight
    into the pond.

## 50. A way out of the den's bet (2026-10-10)

- **The ask:** "the spider game should also have an out option on the bet screen"
- **What got in the way:** walk into the den with gold and the first screen was the bet dial.
  OUT only showed on the title and results, so the only way back to the street was to bet,
  play FREE, or lose.
- **The fix:** opened from the town, the bet dial gets one more spot, OUT, at 11 o'clock.
  Turn to it and press, or tap it; pointed at, it says BACK TO TOWN. Leaving costs nothing.
  Spider Roulette on its own (`--bin spider`) has no town to go back to, so its dial is
  unchanged.
- **The numbers:** 3 new tests; spider's 60 s random-play crowding run covers the 7-spot dial.
- **Media:** `img/50-before-spider-bet.png`, `img/50-after-spider-bet.png`.

## 51. GO IN, gone (2026-10-10)

- **The ask:** "remove the go in button on the overworld can be completely removed now, it only
  appears for a frame and seems buggy"
- **What got in the way:** beat 49 made a landmark tap walk you to the door and straight in. The
  door's big sign with its GO IN button popped up on the frame you arrived and vanished the
  next, which read as a glitch.
- **The fix:** the sign and its button are gone, along with the tap target behind them. At a
  door, the small plate that already names the clearing names the place instead and says PRESS
  TO GO IN (PRESS TO OPEN at the signpost). Walking in from a landmark keeps the clearing's
  plate, so nothing flashes. A tap where GO IN used to be now just walks down the south path.
- **The numbers:** 3 new town tests, including one that watches every frame of a walk-in for
  the door plate.
- **Media:** `img/51-before-town-door.png`, `img/51-after-town-door.png`.

## 52. OUT, and straight back in (2026-10-10)

- **The ask:** "pressing out on the pond also has a bit of flicker, maybe we should do a debounce
  where once you press out you cannot re-enter anything for 2s. I've also seen an issue like
  this on settings", then "after pressing an exit button my press shouldn't instantly be counted
  as a movement command".
- **What got in the way:** two bugs from beat 49's tap-to-enter.
  - The pond: OUT sits at the top of the glass, and that spot is inside the lake's tap zone. The
    town starts with the finger (or PWR) still down from OUT, sees a fresh press at the pond's
    door, and goes straight back in.
  - The settings: tapping the signpost remembered "heading in" and nothing cleared it, so BACK
    landed you at the signpost still heading in, and the settings opened again a frame later.
- **The fix:** coming out of a place or the settings starts a 2 s lock on going in, and the
  finger still on the glass is spent until it lifts, so it doesn't walk you either. Going in
  forgets the landmark you tapped.
- **The numbers:** 5 new tests reproduce each case first; 355 tests pass.
- **Hook:** "The OUT button was inside the pond."

## 53. BUBBLE RING didn't blow a ring (2026-10-10)

- **The ask:** "bubble ring has no visual effect", then "splash also has no visuals".
- **What got in the way:** a trick was only ever a path for the fish sprite. BUBBLE RING's path
  was "stay still", SPLASH was a 25 px hop scaled down to 16 px in the tank, BUBBLES bobbed 3 px
  and FIN WAVE 5 px. There was no water at all.
- **The fix:** `fish::effects` draws the water a trick throws about, in the house and the
  contest. BUBBLE RING blows twelve bubbles that spread out round the fish; SPLASH throws nine
  drops up and out with a ring of spray; BUBBLES rise and wobble from the mouth. FIN WAVE
  bobs 10 px. A new test makes every one of the 15 tricks move at least 5 px on a stage, turn
  the fish, or throw water, so none can be invisible again.
- **The numbers:** 5 new tests; the contest's and house's random-play crowding runs keep the
  spray off every label; 360 tests pass.
- **Media:** `img/53-before-bubble-ring.png`, `img/53-after-bubble-ring.png`,
  `img/53-before-splash.png`, `img/53-after-splash.png` (the "before" shots are the same
  moments rendered with the effects switched off).
  - TODO(user): a phone clip of a fish doing BUBBLE RING in the tank.

## 54. Nothing needs the button (2026-10-10)

- **The ask:** "the fishing game should also treat a tap and hold as a valid way to cast.
  Generally nothing should require the button as it may become jank to press. Just sleep is the
  only real button only thing I think"
- **What got in the way:** an audit of every place a game reads PWR found six with no touch
  way in: Angler's title (a stray tap did nothing, on purpose), the cast itself, reeling in
  early, stopping a walk ("PRESS TO STOP"), finishing a word while teaching, leaving practice,
  and picking a contest tier.
- **The fix:**
  - Angler: a tap off the buttons starts; a finger held anywhere charges the cast exactly like
    holding PWR, and lifting it casts; a tap reels in. The title says PRESS OR TAP TO FISH.
  - Buttons where a stray tap could mean something else: STOP on a walk, DONE/BACK while
    teaching, BACK beside NEW COMMAND in practice. The contest's tier rows and BACK are tap
    targets, drawn from the same rectangles.
  - CLAUDE.md now says nothing may need PWR; only sleep (holding BOOT) is button-only.
- **The numbers:** 8 new tests; 367 tests pass; the tier screen re-renders pixel-identical.
- **Media:** `img/54-before-angler_title.png` / `img/54-after-angler_title.png`,
  `img/54-before-home_practice.png` / `img/54-after-home_practice.png`,
  `img/54-before-park_walking.png` / `img/54-after-park_walking.png`.

## 55. NO SKY? (2026-10-10)

- **The ask:** "the gps game always says looking for the sky, how long should a lock take.
  Update it says No Sky?, i am inside is that the problem? Can we get a retry built in?"
- **What got in the way:**
  - Indoors, mostly. The LC76G needs open sky: about 28 s for a cold first fix, 1 s hot,
    indoors usually never. In the case its antenna lies ceramic-down on the speaker, looking
    out through the back.
  - The park never stopped listening, but after 60 s its one status line said NO SKY? TRY MOCK
    WALK, which reads like it gave up. Lose the sky mid-walk and it kept showing the last fix's
    satellite count forever.
  - A receiver that stops answering (Oct 7's logs show its I2C reads failing over and over)
    was never restarted, only re-polled.
- **The fix:**
  - The walk tracks time since its last fix: five seconds without one says LOOKING FOR THE SKY
    again, and after a minute it takes turns between NO SKY YET, STILL LOOKING and INDOORS? TRY
    MOCK WALK. A fix still counts whenever it comes.
  - The firmware restarts a receiver that's silent for 5 s while the park listens (its reset
    line low for 20 ms; the backup supply keeps what it knew of the sky).
  - The 2 s stats line now says `gps <n> B`, so a log shows whether it's talking at all.
- **The numbers:** 3 new walk tests (one already passed: a late fix always counted); 369 tests
  pass.
- **Hook:** "It wasn't broken. It was inside."
  - TODO(user): a photo of SATELLITES on the walk screen outdoors.

## 56. Into the back of the contest hall (2026-10-10)

- **The ask:** "i should be able to enter it from any side, when I enter from above currently
  and tap I just walk into the back of the building. Maybe it shouldn't just be in the middle of
  the path and be off to the left of the scene"
- **What got in the way:** the hall stood at the top of its clearing, right where the path from
  the pond comes in, so arriving from the pond put you behind it. Walking only slides along
  walls: pressed straight down into the hall's back, the slide wobbled by a hair each frame, so
  it never even counted as stuck. Going north from the hall's own door had the same problem in
  reverse.
- **The fix:**
  - the hall moved to the left of its clearing, off the path;
  - a walk in that stops closing on the door (under 5% of walking speed) goes round the
    landmark once, well clear of it: out past its nearer side, down that side, then the door;
  - "stuck" is now "not getting closer", which catches the wobble.
- **Dead ends:** a detour to the building's lower corner slid along its roof too slowly and
  tripped the stuck check; going sideways first then caught on the rounded corner (the walker is
  a circle against a box). A route that never touches the building fixed both. A first stuck
  threshold of 20% also stopped ordinary walks that slide along a shop at a shallow angle; 5%
  doesn't.
- **The numbers:** a test walks into every place's clearing down every path and taps its
  landmark (all 9 places), plus one from behind the house; 372 tests pass.
- **Media:** `img/56-before-town-hall.png`, `img/56-after-town-hall.png`.

## 47. Second print: the board floats until you screw it down (2026-10-10)

- **The ask:** buttons "just slightly longer", one button "gets stuck at an angle" when the
  screws are tight, and USB-C up "a little bit" more. Then: "maybe it's printer issues again.
  should just move the button hole down very slightly".
- **What the print showed:** the board sat high before the screws went in and only came down as
  they tightened. Our suspect is the glass ledge from beat 46: at 0.1 mm under the glass, a print
  a fraction off lets the glass land on the ledge before the standoffs reach their posts, so
  tightening pulls the board down and drags the key under its cap.
- **The fix:**
  - the ledge sits 0.4 mm under the glass instead of 0.1, so the posts set the height again;
  - the button holes and caps move 0.3 mm down (`BUTTON_DROP`), and the caps stand 1.4 mm out
    of the rim instead of 1.0;
  - the USB-C opening goes up 0.4 mm more, 0.9 mm above the modelled port in all.
- **Checks:** `check.py` passes both cases. The lowered caps still land on both switches, and
  the closest the glass comes to the case is now 0.26 mm (was 0.17).
- **Hook:** "The board wouldn't sit down. The culprit was the ledge I'd added to help it sit
  down."
- **Media:** `img/47-before-button.png`, `img/47-after-button.png`, `img/47-before-usb.png`,
  `img/47-after-usb.png`, `img/47-before-seat.png`, `img/47-after-seat.png` (the rim cut
  through at 10:30: after, there's daylight between the glass and the ledge).
  - TODO(user): a photo of the stuck button, and of the third print with the board screwed
    down.

## 57. A brightness setting (2026-10-10)

- **The ask:** "add brightness to the settings menu"
- **The fix:** the signpost's settings dial goes from four spots to five: SOUND, BRIGHT, MOCK
  WALK, RESET, BACK, with the title in the middle. BRIGHT steps 25, 50, 75, 100% and is saved
  with the other shared preferences, so every game shows at it. The board sets the panel's
  brightness register from it, and idle still dims to a quarter of whatever you chose. Old saves
  read as 100%.
- **Why it's cheap:** no new Bevy resource or system (each costs heap, and Orbit has 12 KB to
  spare). The platform reads the preference straight from the save data every frame.
- **The numbers:** a full-screen AMOLED's draw scales with brightness, so 50% should roughly
  halve the panel's share of the battery.
- **Media:** `img/57-before-settings.png`, `img/57-after-settings.png`.

## 58. OUT is EXIT (2026-10-10)

- **The ask:** "make the out buttons say exit instead"
- **The fix:** every way back to the town that said OUT says EXIT: the house's tank dial and
  button, the hat shop's ring, the den's bet dial, and the top button on the pond, the den, the
  arcade and the humming tree. The park's and contest's BACK spots, which also leave, are
  unchanged.
- **Media:** `img/58-before-home-tank.png`, `img/58-after-home-tank.png`.

## 59. The GPS never found the sky, so walks count steps (2026-10-10)

- **The ask:** "I still have never gotten a lock on the gps. Can we change that game to be a
  step counter system instead. the game should be as power efficent as possible in that mode
  and it should be ended by a button press."
- **What got in the way:** the receiver never got a fix, and it draws about 36 mA with no
  power switch, more than any other part of the board when the screen is dim.
- **The fix:**
  - **Steps from the IMU:** the QMI8658 has a pedometer built in. SensorLib's settings went in
    as two pages of CAL registers through its CTRL9 command handshake, and the firmware reads
    its 24-bit count every frame. Walks count steps at 0.75 m each, so stamina and saves carry
    on.
  - **The doze:** five seconds into a real walk the game sets `Power::dozing`. The board puts
    the panel and touch to sleep, turns the amp and gyro off, stops drawing, and wakes every
    100 ms only to read the step count and the buttons. On battery it light-sleeps in between;
    BOOT can wake it directly, and PWR is polled. PWR or BOOT ends the walk and shows the
    steps.
  - **The GPS asleep:** at boot the firmware sends `$PAIR650,0*25`, backup mode at 13 uA.
    Quectel's hardware design (v1.3, 3.3.4, note 3) says only the PA and PB variants must
    also cut VCC; this board's (AB) can keep it.
  - **Dead end:** a research subagent was meant to find the pedometer registers while the game
    side was built, but it stopped silently when a command was interrupted. The registers came
    from SensorLib's source and the receiver's sleep from Quectel's PDF, read directly.
- **The numbers:** the park's peak heap headroom went from 80 to 84 KB (`Steps` is smaller than
  the GPS sentence buffer); the QEMU tour passes; 378 tests pass.
- **Media:** `img/59-before-park.png` (LOOKING FOR THE SKY), `img/59-after-park.png`,
  `img/59-after-park-dozing.png`.
  - TODO(user): a phone clip of the screen going dark mid-walk and a button bringing back the
    step count.
