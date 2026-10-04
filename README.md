# Orbit

Circular pong for the Waveshare **ESP32-S3-Touch-AMOLED-1.75** (466x466 round
AMOLED, CO5300 over QSPI, CST9217 touch), written as a `no_std` **Bevy 0.19**
app. Drag anywhere to swing the paddle around the rim and keep the ball inside.
Every return speeds it up.

The same Bevy app runs in a desktop simulator and on the board, and both draw
through the same software renderer, so the simulator shows the exact pixels the
panel will get.

```
crates/orbit_game   no_std Bevy plugin: states, touch, FixedUpdate physics, emits a Frame shape list
crates/orbit_gfx    no_std rasterizer (embedded-graphics), dirty-rect diffing, band Presenter
desktop/            minifb simulator + headless PNG snapshots (std)
firmware/           ESP32-S3 binary: CO5300 QSPI/DMA driver, CST9217 touch, heap/clock glue
```

## How a frame gets to the glass

1. The platform writes the finger position into the `Touch` resource and calls `app.update()`.
2. Game systems produce `Frame { shapes }`: rim ring, labels, paddle arc, ball disc.
3. The `Presenter` diffs the shapes against the previous frame. Only the bounds of
   shapes that changed are redrawn, aligned to even coordinates as the CO5300 requires.
4. Each dirty rect is rendered in 8-row bands into a small internal-SRAM buffer.
   The firmware DMAs each band over quad SPI while the next band renders.

A normal gameplay frame sends about 25 KB, about 1.3 ms of the 40 MHz QSPI link.
A full-screen redraw is 434 KB (about 22 ms) and only happens on screen changes.
`panel_ram_always_matches_a_full_render_of_the_latest_frame` pins that the
dirty-rect path never leaves stale pixels.

## Desktop

```sh
cargo run -p orbit_desktop                         # play: hold left mouse = finger
cargo run -p orbit_desktop -- snapshot screenshots # title/playing/game_over PNGs, headless
cargo test --workspace
cargo clippy --workspace --all-targets && cargo fmt
```

The window title shows fps, KB pushed per frame, and the estimated time that
traffic would take on the real link.

## Firmware

One-time setup (already done on this machine): `cargo install espup espflash`
then `espup install --targets esp32s3`.

```sh
cd firmware
. ~/export-esp.sh            # every new shell: puts the Xtensa linker on PATH
cargo run --release          # build, flash over USB-C, open the serial monitor
cargo clippy --release && cargo fmt
```

`firmware/` is its own crate with its own `rust-toolchain.toml` (`esp`), outside the
host workspace. It pulls the game crates in by path. Always build it in release
mode; PSRAM needs it, and unoptimized Bevy is far too slow.

### Bring-up checklist (first flash)

The board itself hasn't been run yet; everything below is from Waveshare's
sources plus an emulator run. Watch the serial log:

1. **`display ready`, then the ORBIT title on the panel.** If the panel stays
   black, the init list in `firmware/src/co5300.rs` is the place to look. It is
   Waveshare's ESP-IDF BSP sequence for this board. Their Arduino demo uses a
   shorter one (`11`, wait 120 ms, `FE 00`, `C4 80`, `3A 55`, `53 20`, `63 FF`,
   `29`, `51 D0`) that is worth trying.
2. **Image shifted or wrapped horizontally:** adjust `COLUMN_OFFSET` (6) in `co5300.rs`.
3. **`bevy app built entirely in internal SRAM`.** If it instead warns that Bevy
   spilled into PSRAM, see Memory below.
4. **Tap the screen and the paddle jumps to your finger.** If it jumps to the
   opposite side or a mirrored spot, flip `TOUCH_MIRROR_X` / `TOUCH_MIRROR_Y` in
   `firmware/src/main.rs`. If the log says `CST9217 touch init failed`, check
   the touch reset pin (below).
5. Every 2 s the log prints frame count, average and worst frame time, bytes
   per frame, and heap use.

**1.75 vs 1.75C:** this targets the plain 1.75 (LCD reset GPIO39, touch reset
GPIO40). The 1.75C variant uses GPIO1 and GPIO2 for those two resets.

### Memory

Bevy's own bookkeeping (schedules, per-resource tables) dominates: about
216 KB steady and 270 KB at peak while schedules build on the first update.
Those figures were measured with a counting allocator on a 32-bit target.
The firmware gives Bevy about 284 KB of internal SRAM (`INTERNAL_HEAP_BYTES` plus
the 72 KB reclaimed region) and keeps about 74 KB of stack. PSRAM is
registered last, only as a safety net. On the ESP32-S3, atomic
read-modify-write does not work on PSRAM, and Bevy keeps `Arc` counts and
spinlocks on the heap. So keep Bevy's working set internal. Put big plain
buffers (sprites, framebuffers) in PSRAM explicitly with `esp_alloc::ExternalMemory`.

### Checking a change without the board

- `tools/heap32/run.sh` measures the game's heap on a 32-bit target and checks it
  against the firmware's internal-SRAM budget. Run it after adding Bevy plugins,
  systems or resources.
- `tools/qemu-smoke/run.sh` boots the firmware runtime in Espressif's QEMU and prints
  `SMOKE PASS`/`SMOKE FAIL`.

The full platform handbook is `docs/ESP32_PLATFORM.md`.

### Emulator smoke test notes

Espressif's QEMU (`esp-develop-9.2.2-20260417`, `-machine esp32s3`) was used to boot this
firmware's memory layout, with a stub panel and scripted touch, through
3,000+ frames: title, play, a scored hit, game over, retry. Peak heap stayed
internal (0 B in PSRAM). Three emulator-only workarounds were needed:

- esp-hal's wait on `bbpll_cal_done` has to be patched out, because QEMU never sets it.
- The FPU has to be enabled by hand (espressif/qemu#154: QEMU boots with `CPENABLE = 0`).
- Logging has to go over UART rather than USB-JTAG.

The display and touch protocols can only be proven on the real board.

## Making your own game on this

- Bevy here is `default-features = false, features = ["default_no_std"]`:
  app, ECS, states, time, math, and transform. There is no `bevy_render`,
  `bevy_asset`, or `bevy_log`, and no threads.
- Draw by pushing `orbit_gfx::Shape`s into the `Frame` resource. Add new
  shape kinds in `orbit_gfx/src/shape.rs`, and make `bounds()` tight so moving
  things stay cheap.
- Keep `orbit_game` `#![no_std]`. Use `bevy::math::ops` for trig, because
  `f32::sin` is std-only.
- Watch the heap. Every plugin, schedule, and resource costs a few KB on-device;
  the boot log tells you where you stand.
