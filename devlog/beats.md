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

## 23. Fish Pals: the arcade becomes a town

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
- **What QEMU checked:** the Fish Pals tour, every door twice round. The heap back in town
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
