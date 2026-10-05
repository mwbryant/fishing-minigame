# Bevy on the Waveshare ESP32-S3-Touch-AMOLED-1.75: platform handbook

Everything learned bringing `no_std` Bevy games to this board, written so a
fresh session can keep going without re-deriving it. There are two games:
**Orbit** (circular pong, touch) and **Angler** (fishing: tilt-aimed cast,
hold-to-charge, lean-to-reel, persistent fish log; see section 10.4). Facts are marked
**[verified]** (built, tested, read in source, or run in the emulator) or
**[unverified]** (from vendor sources or inference; not yet seen on hardware).

Status as of 2026-10-05: the board has **not arrived**. Both games, the
renderer, and the simulator are tested on the host. Both firmware binaries
build and link for the ESP32-S3, and both ran in Espressif's QEMU (Angler
including a save round-trip through emulated flash). The display, touch, IMU,
buttons, flash, and audio have never run on real hardware. Audio is researched
and compile-checked (`docs/reference/es8311_audio.rs`) but not wired in.

---

## 1. Quick commands

```sh
# host workspace (game + renderer + simulator)
cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt
cargo run -p orbit_desktop -- angler                 # play Angler (the default) ...
cargo run -p orbit_desktop -- orbit                  # ... or Orbit
cargo run -p orbit_desktop -- angler snapshot screenshots   # headless PNGs of each screen
#   desktop controls: mouse = finger, Space = action (PWR), Z = zero (BOOT),
#   left/right = turn the virtual device (it stays turned), hold up/down = lean it
#   15 degrees (it springs back level when released), C = straighten, Esc = quit.
#   Desktop saves live in ~/.logic-orbit/<game>.flash (device flash format).

# device firmware (separate crate, Xtensa toolchain; one binary per game)
cd firmware && . ~/export-esp.sh
cargo run --release --bin angler                     # build, flash over USB-C, serial monitor
cargo run --release --bin orbit
cargo clippy --release && cargo fmt

# verification without a board
tools/heap32/run.sh [orbit|angler]          # 32-bit heap vs the firmware's SRAM budget
tools/qemu-smoke/run.sh [secs] ["orbit angler"]   # runtime in Espressif QEMU; SMOKE PASS/FAIL
```

Installed on the dev Mac: `espup` 0.18, `espflash` 4.6, the `esp` Rust toolchain
(rustc 1.97.0.0, made by `espup install --targets esp32s3`, writes
`~/export-esp.sh`), and Homebrew `libgcrypt` (needed only by QEMU).

## 2. Repo map and data flow

```
crates/orbit_gfx     #![no_std] rasterizer (disc/ring/arc/label/rect/line/sprite), dirty-rect
                     diff, band Presenter, `Panel` trait. No Bevy.
crates/orbit_motion  #![no_std] Mahony IMU filter + zeroing -> steer/lean degrees. Bevy math only.
crates/orbit_save    #![no_std] power-fail-safe two-sector record store over embedded-storage NOR
                     flash, plus `RamFlash` (tests, desktop).
crates/orbit_core    #![no_std] shared Bevy plumbing. `CorePlugin`: Frame, Touch, Rng, SaveBlob.
                     `ControlsPlugin` (opt-in): Buttons {action, zero}, ImuFeed -> Tilt,
                     zero button re-levels the IMU from any screen.
crates/orbit_game    #![no_std] Orbit (circular pong, touch only; uses Bevy states).
crates/angler_game   #![no_std] Angler (fishing; one phase-machine resource, no Bevy states).
desktop/             std simulator for either game (minifb window, RAM model of the panel,
                     virtual IMU, file-backed save flash, PNG snapshots).
firmware/            ESP32-S3: lib (board bring-up + shared frame loop + drivers) and one
                     binary per game (src/bin/{orbit,angler}.rs). Excluded from the workspace.
tools/heap32/        wasm32 heap measurement of either game (excluded from workspace).
tools/qemu-smoke/    QEMU runtime smoke test of either game (excluded from workspace).
docs/reference/      researched, compile-checked code not wired in yet (ES8311 audio).
```

The frame loop is the same on both platforms:

1. The platform writes inputs:
   - touch via `Touch::set(Option<Vec2>)`, in screen pixels;
   - if the game added `ControlsPlugin`, the held state of each button via `Buttons.{action,zero}.set`;
   - IMU readings via `ImuFeed::push(ImuSample)`, in the screen frame.
2. It calls `app.update()`. Game systems run, and `draw_frame` (PostUpdate) refills `Frame { shapes }`.
3. It calls `Presenter::present(&shapes, &mut panel)`, which:
   - diffs the shapes against the previous frame (`FrameDiff`);
   - takes the bounds of each shape that appeared or vanished, even-aligned and merged;
   - renders each dirty rect in full-width bands into a small buffer (`render`);
   - calls `Panel::write(area, bytes)`. The bytes are big-endian RGB565, which is the panel's wire format.

4. It writes `SaveBlob::take_pending()` (if any) to storage. At boot, it passes
   storage's bytes in through `SaveBlob::from_storage`.

The desktop `Panel` copies into a 466x466 RAM model. The firmware `Panel` is the
CO5300 driver (QSPI + DMA). The simulator therefore shows exactly the pixels
the device would get, including any dirty-rect bugs.

Conventions, matching the user's other logic-* games:
- Red-green TDD with behavioral test names.
- `cargo clippy` + `cargo fmt`, and a commit per feature.
- The user's global style rules: no `else { None }` tails, no comments that just
  restate the code, `use` imports instead of full paths, and no `_`-prefixed
  unused variables (remove them instead).
- UI strings are ASCII only. The font is embedded-graphics' `FONT_10X20`
  (ASCII), scaled by an integer.

## 3. Hardware reference

Board: Waveshare ESP32-S3-Touch-AMOLED-1.75 (SKU 31261). The SoC is an
ESP32-S3R8: 2x LX7 at 240 MHz, 512 KB SRAM, 8 MB **octal** PSRAM, and 16 MB flash.
Waveshare's sources are github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75 (the
Arduino demos and the ESP-IDF BSP `waveshare__esp32_s3_touch_amoled_1_75`).

**Variant trap:** the **1.75C** is a different board with different reset pins
(LCD reset GPIO1, touch reset GPIO2). Everything here is for the plain 1.75.

### Pins [verified from Waveshare source/schematic]

| Function | GPIO |
|---|---|
| QSPI LCD: CS / SCLK / D0 / D1 / D2 / D3 | 12 / 38 / 4 / 5 / 6 / 7 |
| LCD reset / LCD TE (tearing) | 39 / 13 (TE unused so far) |
| I2C SDA / SCL (400 kHz, 2.2k pull-ups) | 15 / 14 |
| Touch INT / touch reset | 11 / 40 |
| I2S MCLK / BCLK / WS(LRCK) / DOUT (to ES8311) / DIN (from ES7210 mics) | 42 / 9 / 45 / 8 / 10 |
| Speaker amp (NS4150B) enable, active high, pulled down | 46 |
| SD card | 1, 2, 3 (+41) |
| IMU INT2 | 21 |
| BOOT button | 0 (active low) |
| Octal PSRAM (never assign) | 33-37 |

- **PWR button:** goes to the AXP2101 PWRON pin. Firmware can read it as TCA9554 P4 (high = pressed).
- **Display/touch power:** neither the AXP2101 PMIC nor the TCA9554 expander needs any setup before the display or touch work.
- **Brightness:** there is no backlight pin. Brightness is set with panel register `0x51`.
- **Stale MCLK define:** Waveshare's Arduino `pin_config.h` says `I2S_MCK_IO 16`. That is stale;
  the schematic routes GPIO16 to header pin 8, and MCLK is GPIO42.

### I2C addresses [verified]

ES8311 codec `0x18`, TCA9554 `0x20`, AXP2101 `0x34`, ES7210 mic ADC `0x40`,
PCF85063 RTC `0x51`, CST9217 touch `0x5A`, QMI8658 IMU `0x6B`.

### CO5300 display protocol [verified in vendor source; unverified on this board]

- **SPI setup:** mode 0, 40 MHz in every Waveshare demo. The datasheet maximum write clock is reportedly 50 MHz.
- **Register write:** opcode `0x02` plus 24-bit address `0x00RR00`. Parameters go out on one line.
- **Pixel write:** first a RAMWR (`0x2C`) register write, then opcode `0x32` plus address
  `0x003C00` (memory-write-continue) with the data on 4 lines. Pixels are RGB565, high byte first.
- **Window:** the 466 columns sit at RAM columns **6..=471** (x offset 6, y offset 0).
- **Alignment:** windows must start on even coordinates and have even width and height.
  `orbit_gfx::align_even` enforces this.
- **Init:** `firmware/src/co5300.rs` uses Waveshare's ESP-IDF BSP list. It sends `36 00`, `3A 55`,
  then page-`0x20` vendor writes (`FE 20`, `19 10`, `1C A0`), then `FE 00`, `C4 80`, `3A 55`, `35 00`,
  `53 20`, `51 FF`, `63 FF`, CASET `00 06 01 D7`, RASET `00 00 01 D1`, `11`, a 120 ms wait,
  and finally `29`.
- **Fallback init:** Waveshare's Arduino_GFX uses a shorter list: `11`, a 120 ms wait,
  `FE 00`, `C4 80`, `3A 55`, `53 20`, `63 FF`, `29`, `51 D0`, `58 00`, `20`, `36 00`.
- **Reset:** low for 10 ms, then high, then wait 150 ms.
- **Transport template:** github.com/okhsunrog/claudial (`firmware/src/co5300.rs`). It runs
  on the AMOLED-2.16, which has the same CO5300 and the same LCD/touch/I2C pins. It uses manual
  CS and 40 MHz with esp-hal 1.2 DMA. Its MADCTL `0xA0` is specific to the 2.16, so don't copy it.
- **No vsync/tearing sync yet:** claudial found the 2.16's TE line grounded. On the 1.75 it
  reaches GPIO13, so syncing to TE may be possible **[unverified]**.

### CST9217 touch [verified in vendor source; unverified on this board]

- **Crate:** `cst92xx` 0.2.4 with `default-features = false, features = ["blocking"]`.
  Use `CST92xx::new(i2c, delay).with_reset(gpio40).init()`, then `touches()`, which
  returns `[Option<Point>; N]` in raw panel coordinates.
- **Orientation:** Waveshare's BSP mirrors both axes for this board (`x = 465 - x`,
  `y = 465 - y`). The firmware's `TOUCH_MIRROR_X/Y` constants do this.
- **Pin caution:** the crate's own example uses TP_RST = GPIO2, which is the 1.75C pin.

### Buttons, IMU, PMIC, flash [verified in schematic/datasheets/vendor code; unverified on hardware]

**Physical layout** (front view, UI upright as drawn with MADCTL 0x00): USB-C is at 9 o'clock.
Both buttons are on the right edge, **PWR at about 2 o'clock and BOOT at about 4 o'clock**
(the lower one). The TF slot sits between them.

**Button roles:** the user's spec is "the bottom button re-zeroes the IMU", so **BOOT = zero**
and **PWR = action** (hold to charge a cast). They are swapped by the `ACTION_BUTTON` /
`ZERO_BUTTON` constants in `firmware/src/lib.rs`.

**BOOT:** GPIO0, active low, with an external 10k pull-up and a 100 nF debounce cap.

**PWR:**
- Its live level is TCA9554 (0x20) pin P4, `SYS_OUT`, **high while pressed**. Make P4 an input
  by setting bit 4 of register 0x03, then read bit 4 of register 0x00.
- The AXP2101 (0x34) powers the board off when PWR is held. Register 0x27 bits 3:2 set the
  hold time (4/6/8/10 s, **default 6 s**). The firmware sets 10 s every boot (the PMIC forgets
  it when it loses power).
- Angler also cancels any charge held past 4.4 s (`MAX_CHARGE_SECS`).
- 0x22 bit 1 would disable hold-to-off entirely; it isn't touched.
- Don't use XPowersLib's `setIrqLevel`/`setOffLevel`; their bit masks are wrong.

**QMI8658 IMU** (0x6B), driver at `firmware/src/qmi8658.rs`:
- **Boot and checks:** reset with `0x60 = 0xB0`, then poll `0x4D == 0x80`. WHO_AM_I (0x00)
  must read 0x05.
- **Configuration:**
  - CTRL1 = 0x60: auto-increment, little-endian decode as both vendor drivers do. If the data
    reads as garbage, try 0x40.
  - CTRL2 = 0x13: ±4 g, 8192 LSB/g.
  - CTRL3 = 0x53: ±512 dps, 64 LSB/dps.
  - CTRL5 = 0x33: both low-pass filters.
  - CTRL7 = 0x03: accel and gyro on.
- **Reading:** one 12-byte burst from 0x35.
- **Bug in Waveshare's ESP-IDF qmi8658 component:** its gyro range table is off by one.

**IMU axes in the screen frame** (x right, y down, z into the screen):
`screen = (imu.y, -imu.x, imu.z)`. Sources: the PCB axis marking next to U3 and three Waveshare
tilt demos. Expected at rest:
- face-up, the accel reads z ≈ −1 g;
- upright with USB on the left, x_imu ≈ +1;
- button edge up, y_imu ≈ +1.

**Flash** (`esp-storage` 0.10, features `["esp32s3", "embedded-storage"]`):
- `FlashStorage::new(peripherals.FLASH)` implements `NorFlash` (write 4, erase 4096) and
  `ReadNorFlash` (read 4).
- **Trap:** `FlashStorage` has *inherent* `read`/`write` methods (read-modify-write with an
  automatic erase) that shadow the trait methods. Generic code over `F: NorFlash` (like
  `orbit_save`) is fine; concrete calls must use `NorFlash::write(..)`.
- Saves live at **0x9000 and 0xA000**, in the default partition table's `nvs` region.
  `espflash flash` only writes 0x0, 0x8000 and 0x10000, so saves survive reflashing (but not
  `erase-flash` or `--erase-parts`).
- The factory image leaves old NVS bytes there; `orbit_save`'s magic and CRC ignore them.
- A sector erase blocks everything for tens of ms, so saves happen only on the catch screen.

## 4. Toolchain and crate set [verified: builds and links]

| Crate | Version / features |
|---|---|
| esp-hal | `~1.2.2`, `["esp32s3", "log-04", "unstable"]` (`unstable` gates DMA, PSRAM, `with_sio*`, half-duplex) |
| esp-bootloader-esp-idf | 0.6.0 |
| esp-alloc | 0.11.0 (the chip feature is mandatory on all of these) |
| esp-println | 0.18.0 |
| esp-backtrace | 0.20.0 `["panic-handler", "println"]` |
| bevy | `0.19`, `default-features = false, features = ["default_no_std"]` |

- Bevy 0.19.1 is the latest stable. 0.20 is at rc.2.
- The esp-hal 1.2 crate set must not be mixed with the older set (esp-alloc 0.10 and so on).
- esp-hal 1.2 has **no `psram` feature**. PSRAM is a runtime `PsramConfig`, and the default
  `Auto` mode tries octal, then quad. The firmware uses `Auto` so the same code runs in QEMU,
  which only emulates quad PSRAM.
- `esp_bootloader_esp_idf::esp_app_desc!()` is required. espflash 4.x refuses images without it.
- Project files: `rust-toolchain.toml` sets `channel = "esp"`. `.cargo/config.toml` sets target
  `xtensa-esp32s3-none-elf`, `build-std = ["alloc","core"]`, rustflag `-C link-arg=-nostartfiles`,
  and runner `espflash flash --monitor --chip esp32s3`. `build.rs` emits `-Tlinkall.x`.
- Always build `--release`. PSRAM needs it, and unoptimized Bevy is far too slow.
- Every link prints a harmless warning: "LOAD segment with RWX permissions".
- Logging needs `[env] ESP_LOG = "info"` in `.cargo/config.toml`. esp-println filters at
  compile time, so without it every `info!` silently disappears.

## 5. Bevy without std [verified]

- **What the umbrella crate gives you:** `bevy` with `default_no_std` (libm, critical-section,
  bevy_color, bevy_state) gives app, ecs, state, time, math, transform, input, reflect, and
  platform. Input kinds (touch, keyboard, gamepad) are opt-in features.
- **Unavailable:** `bevy_render`, `bevy_asset`, `bevy_log`, `bevy_audio`, `multi_threaded`,
  `async_executor`, `std`, and `default_app`. Everything is single-threaded.
- **The clock hook (mandatory):** the platform must call
  `unsafe { bevy::platform::time::Instant::set_elapsed(fn() -> Duration) }` **before**
  `TimePlugin` is added, because `Time<Real>::default()` calls `Instant::now()`. Without the hook
  on Xtensa, that call panics. The firmware feeds it esp-hal's microsecond timer. Tests and the
  desktop use std time; `testing::headless_app` uses `TimeUpdateStrategy::ManualDuration`, one
  60 Hz step per `update()`.
- **Float math:** `f32::sin`, `sqrt`, `atan2` and `rem_euclid` are std-only. Use
  `bevy::math::ops::*`. `to_degrees`, `abs`, `clamp`, `min` and `max` are fine in core.
  glam's `Vec2` methods work, backed by libm.
- **Atomics:** the S3 has 32-bit compare-and-swap. portable-atomic handles 64-bit by itself;
  nothing to configure.
- **Infallible draw results:** write `let Ok(()) = canvas.clear(c);`.
- **No `inventory`-style auto registration:** `.init_array` isn't run.
- **Prior art:** georgik/esp32-conways-game-of-life-rs (bevy_ecs on S3 boards, PSRAM heap),
  OS-Mote/Bevy-ESP32, and bushrat011899/bevy_mod_gba (custom runner and time pattern).
  Bevy 0.17.0–0.17.2 panicked on first spawn on 32-bit targets; this is fixed in 0.17.3+.

## 6. Rendering budget and rules

- **Link speed:** 40 MHz × 4 lines moves 20 MB/s. A full 466x466 frame is 434,312 B, so it takes
  **about 22 ms** on the wire. Full-screen animation therefore tops out around 30 fps before
  render time is counted.
- **Typical Orbit frame:** about 25 KB, roughly 1.3 ms of link time.
- **Render CPU time on device:** unmeasured. The firmware logs average and worst frame time
  every 2 s.
- **Dirty-rect rules:**
  - Static content costs nothing after the first frame.
  - Anything that changes repaints its whole bounding box, plus every shape that overlaps it.
  - `Shape::bounds()` must be tight and conservative. Arc bounds are computed by hand,
    because embedded-graphics reports an arc's whole circle.
  - Shapes are compared with `PartialEq` every frame. Keep shape payloads small: refer to big
    data by id, not by value.
- **Bands:** 8 rows × 466 px = 7,456 B, one DMA transfer each. The CO5300 driver keeps two
  7,680 B DMA buffers. It copies the next chunk while the previous one transmits, and returns
  with the last chunk still in flight, so the next band renders in parallel.
- **SPI transaction limit:** 32,736 B per DMA transaction (esp-hal `MAX_DMA_SIZE`).
- **Round glass:** pixels outside radius 233 are invisible. Pure black means the AMOLED pixel
  is off, which is free and power-saving.

## 7. Memory: the binding constraint

Measured **[verified]** with `tools/heap32/run.sh`:

| Game | After build | During play | Peak | Headroom in 284 KB (with the 7 KB band) |
|---|---|---|---|---|
| Orbit | 197 KB | 206 KB | 265 KB | **11 KB** |
| Angler | 185 KB | 189 KB | 189 KB | **87 KB** |

Angler is 76 KB leaner at peak because it uses **one phase-machine resource and four systems
instead of Bevy states**. `StatesPlugin`, the `StateTransition` schedule, and every
`OnEnter`/`OnExit` schedule cost KB each.

Opt-in plugins matter too. Moving buttons and IMU into `ControlsPlugin` saved Orbit 12 KB.
New games should follow Angler's pattern. The peak is transient, from schedule graph builds
during the first `update()`.
Most of it is Bevy itself:
- `App::new()` alone is about 100 KB (64-bit host).
- `TimePlugin` adds about 90 KB.
- Each schedule, resource table (`Column::with_capacity`), and message type adds KB.

Reflection is negligible (28 types).

**Why it has to fit in internal SRAM:** on the ESP32-S3, atomic read-modify-write
(`S32C1I`) doesn't work on PSRAM (esp-alloc's own warning). Bevy keeps `Arc` counts and
spinlocks on the heap.

Firmware layout **[verified in QEMU]**:

| Region | Size |
|---|---|
| reclaimed heap | 73,744 B |
| `INTERNAL_HEAP_BYTES` | 212 KB |
| internal heap total | **284 KB** |
| `.stack` | about 74 KB (whatever DRAM remains) |
| PSRAM | 8 MB, registered **last** as a safety net |

At boot the firmware logs whether Bevy spilled into PSRAM. In QEMU it doesn't (0 B).

If a game outgrows the budget, the options, in order of preference:
1. **Test PSRAM atomics on the real board first** (§9). georgik's Espressif demos run
   bevy_ecs entirely from a PSRAM heap on S3 hardware. If RMW atomics on PSRAM really work, or
   at least don't hang, the 8 MB is open and this whole section relaxes.
2. **Trade stack for heap.** Bevy ran fine in QEMU with about 74 KB of stack. Don't go below
   about 48 KB without testing in QEMU.
3. **Trim Bevy:**
   - fewer `OnEnter`/`OnExit` schedules (each schedule is several KB);
   - fewer message types and resources;
   - individual crates (`bevy_ecs`, `bevy_app`, `bevy_time`, `bevy_state`, `bevy_math`,
     `bevy_platform`) instead of the umbrella, which drops bevy_reflect.
4. **Big plain data never goes on the Bevy heap.** Sprites and audio live in flash
   (`include_bytes!`, zero RAM). Large scratch buffers go in PSRAM explicitly with esp-alloc's
   `ExternalMemory` allocator (needs `allocator-api2`).

Other memory facts:
- esp-alloc allows at most 3 regions and is first-fit in registration order.
- A large `static` (20 KB) in the LTO'd firmware broke linking with "dangerous relocation:
  l32r: literal target out of range". Size memory through `heap_allocator!`, not big statics.

## 8. Verifying without a board

- **Host tests:**
  - `crates/orbit_game/tests/orbit.rs`: gameplay, headless at a fixed 60 Hz.
  - `orbit_gfx` unit tests:
    - arc bounds contain every drawn pixel;
    - banded render equals full render;
    - **panel RAM always equals a full render of the latest frame** across 40 frames of motion;
    - writes are even-aligned.
- **Simulator:** the window title shows fps, KB per frame, and the estimated QSPI ms.
  `snapshot` mode writes PNGs headlessly. Use it to look at visuals; it works even while the
  Mac is locked, because it doesn't capture the screen.
- **`tools/heap32/run.sh`:**
  - builds the game for wasm32, which has 4-byte pointers like Xtensa;
  - counts allocations through build, 20 s of play, and 10 s idle;
  - compares the peak plus the render band against the budget parsed from `firmware/src/lib.rs`;
  - exits nonzero when over budget.
- **`tools/qemu-smoke/run.sh`:**
  - **Setup it handles:** downloads Espressif QEMU `esp-develop-9.2.2-20260417`
    (`-machine esp32s3`, needs `brew install libgcrypt`).
  - **What it runs:** one binary per game (`src/bin/{orbit,angler}.rs`) over a shared
    `Smoke` harness (`src/lib.rs`): the firmware's heap layout, a byte-counting panel, and
    scripted input. The two `.stack` sizes are printed; keep them within about 1 KB.
  - **Pass criteria:**
    - Orbit: two or more starts, one or more losses, and 0 B in PSRAM.
    - Angler: two fish landed by a scripted steady-hand player, the fish log written through
      `esp-storage` into emulated flash at 0x9000 and read back equal, and 0 B in PSRAM.
  - **Scripting rule:** emulated frames are much shorter than real ones (about 170 fps of game
    time), so drive scripts by **game time**, never frame counts. That covers charge-until-power,
    waiting out the result guard, and bounding fights by seconds.
  - **Emulator-only workarounds, all applied automatically:**
    1. esp-hal's `bbpll_cal_done` wait is patched out of a private esp-hal copy in
       `.patched/`, because QEMU never sets the bit.
    2. The FPU is enabled by hand, because QEMU boots with CPENABLE=0 (espressif/qemu#154,
       open). Without that, the first hardware-float instruction double-faults.
    3. esp-println uses the `uart` feature, because QEMU doesn't connect USB-JTAG.
  - **Debugging a hang in QEMU:** run with `-d in_asm,int -D tb.log`, then pass the last
    translated PCs to `xtensa-esp32s3-elf-addr2line -f -C -e <elf>`. QEMU's xtensa interrupt
    numbers are 11 = user exception and 12 = double exception.
- **What none of this proves:** the CO5300 init and offset, touch orientation, real timing,
  and PSRAM-atomics behaviour.

## 9. When the board arrives

First flash (`cd firmware && . ~/export-esp.sh && cargo run --release --bin orbit`, the simpler game, then `--bin angler`). Watch the serial log:
1. **`display ready`, then the title on the glass.**
   - Black screen: try the Arduino init list (§3).
   - Shifted image: adjust `COLUMN_OFFSET`.
2. **`bevy app built entirely in internal SRAM`.**
3. **Tap and the paddle follows the finger.** If it goes the wrong way, flip
   `TOUCH_MIRROR_X/Y`. If the log shows `CST9217 touch init failed`, check that this isn't a 1.75C.
4. **The 2 s stats line:** frame time (average and worst), bytes per frame, heap.

For Angler (`cargo run --release --bin angler`), also check:
5. **IMU:** the title screen has a bubble level (a dot in a ring). Turn the device like a
   wheel: the dot should move sideways. Tip the top away: it should move up. Press BOOT:
   "LEVEL SET" appears and the dot recentres. If the axes are wrong, fix `imu_to_screen` in
   `firmware/src/lib.rs`. If the log says `IMU answered WHO_AM_I`, the address or chip is wrong.
6. **Aim:** turning counterclockwise should raise the rod (the "ANGLE" readout goes up).
   If it's backwards, the steer sign is wrong; fix it in `imu_to_screen`, not in the game.
7. **PWR:** holding PWR fills the rim power gauge, and releasing it casts. A 2 s hold must not
   power off; the boot code sets the PMIC's hold-to-off to 10 s. If PWR does nothing, check
   the `PWR key unavailable` log (TCA9554 at 0x20).
8. **Reel:** leaning forward pushes the green zone up. Swap the sign in `imu_to_screen`'s y/z
   if it's inverted.
9. **Saves:** catch a fish, power-cycle, and the title's "CAUGHT n" should persist. The boot
   log prints `save storage: found a save`.

Then run these experiments, in this order:
- **PSRAM atomics:** allocate an `AtomicU32` (and an `Arc`) in PSRAM via `ExternalMemory`, run
  `fetch_add`/`compare_exchange` a million times, and check the results. That decides §7.1.
- **Real render cost:** frame-time stats for a full-screen redraw versus gameplay frames.
- **TE sync on GPIO13:** see whether waiting for the tearing pulse before large writes
  removes tearing.
- **Audio bring-up** (§10.2).

## 10. Extending: pixel art, sound, input

### 10.1 Pixel-art sprites (built: `orbit_gfx::Sprite`)

What shipped: sprites are text rows plus a `(u8, Rgb565)` palette, with `.` transparent
(`crates/angler_game/src/sprites.rs`). `Shape::Sprite { sprite: SpriteRef(&STATIC), top_left,
scale, flip_x }` compares by pointer, draws only the rows inside the band, and has a tested
tight `bounds()`. The original plan notes follow.
- **Assets:** PNG → (host build step) → raw big-endian RGB565 plus a transparent key color, or
  8-bit indexed plus palette → `include_bytes!`. Data stays in flash at zero RAM cost; there
  are 16 MB, read through the cache.
- **New shape:** add `Shape::Sprite { id: SpriteId, frame: u16, top_left: Point, scale: u8, flip_x: bool }`.
  - Look up `id` in a static atlas table. Comparing ids keeps the per-frame `PartialEq` diff cheap.
  - `bounds()` is `size × scale`.
  - `draw` writes the clipped rows straight into the `Canvas` byte buffer, skips the key
    color, and repeats each source pixel `scale` times. Don't push per-pixel
    embedded-graphics `Pixel`s.
  - The existing `Scaled` adapter in `shape.rs` shows how integer scaling was done for text.
- **Chunky look:** author at 155x155 and draw at ×3, or 233x233 at ×2. Output cost is per
  screen pixel either way.
- **Budget:**
  - A static full-screen background is free after the first frame.
  - A moving 32x32 sprite at ×3 repaints about 2×(96×96) px, about 37 KB, about 2 ms of link time.
  - Whole-screen animation (scrolling water) costs 22 ms+ per frame. Animate small regions
    (ripples, the bobber) instead.
- **Simulator:** shows sprites unchanged, because it uses the same renderer.

### 10.2 Sound [verified from schematic, BSP, and an esp-hal 1.2.2 compile; never run on hardware]

**Hardware path:**
- Chain: ESP32-S3 I2S0 → **ES8311** codec (I2C `0x18`, fixed in hardware) → **NS4150B** mono
  class-D amp (3.3 V supply, about 0.5 W into 8 Ω, estimated) → `SPK` MX1.25 2-pin connector.
- **The amp is off by default.** GPIO46 enables it: active high, with a 10k pull-down.
- **A speaker is in the box:** Waveshare's package contents list "8Ω 2W speaker ×1" for every variant.
- **Chip ID:** registers `0xFD`/`0xFE` read `0x83`/`0x11` on an ES8311. Use this as a first
  bring-up check.

**Driver code:**
- `docs/reference/es8311_audio.rs` holds the reference driver.
- The I2S setup below compiled in a scratch firmware against esp-hal 1.2.2.
- Everything I2S-related needs esp-hal's `unstable` feature.

```rust
// I2C is now shared: touch and codec each get a RefCellDevice (embedded-hal-bus 0.3).
let i2c_bus = RefCell::new(i2c);
let i2s = I2s::new(peripherals.I2S0, peripherals.DMA_CH1,          // CH0 is the display
    TdmConfig::new_tdm_philips()
        .with_sample_rate(Rate::from_hz(22_050))
        .with_data_format(DataFormat::Data16Channel16)
        .with_channels(Channels::STEREO))                        // same sample in L and R
    .expect("I2S config")
    .with_mclk(peripherals.GPIO42);                              // 256 x fs, what the ES8311 wants
let i2s_tx = i2s.i2s_tx.with_bclk(peripherals.GPIO9).with_ws(peripherals.GPIO45)
    .with_dout(peripherals.GPIO8).build();
let mut amp = Output::new(peripherals.GPIO46, Level::Low, OutputConfig::default());
let mut audio = AudioOut::start(i2s_tx, dma_tx_stream_buffer!(8 * 1024, 1024), &mut source);
let mut codec = Es8311::new(RefCellDevice::new(&i2c_bus));
codec.init(22_050, &mut amp)?;   // after I2S so MCLK is already running; raises GPIO46, unmutes, 0 dB
// every frame: audio.pump(&mut mixer);  tops up whatever the DMA has consumed
```

**esp-hal 1.2 API notes:**
- `write_dma_circular` and `write_dma` are **gone** (removed in 1.2.0-rc.0). Streaming is now
  `I2sTx::write(DmaTxStreamBuf)` → a transfer with `available_bytes()` / `push()` / `push_with()`.
- The stream buffer is **not a ring**. If the game stalls long enough for it to drain, the
  transfer stops; `AudioOut::pump` detects that with `is_done()` and restarts it.
- Pre-fill the buffer before `write`.
- 8 KB holds about 93 ms at 22.05 kHz stereo 16-bit.
- Start audio *after* the first `app.update()` (the slow schedule build), or the first pump underruns.
- Memory: the stream buffer is a static, so it comes out of the stack. With audio the stack is
  about **67 KB** (measured from section sizes).

**ES8311 register sequence:**
- Source: esp_codec_dev 1.6.2 `es8311.c`, as used by the BSP (slave, MCLK from the pin, DAC only).
  It is encoded in `init_sequence()` in the reference file.
- Every MCLK = 256 × fs rate from 8 to 48 kHz uses the same dividers. Only DAC OSR (reg `0x04`)
  changes: `0x20` for ≤16 kHz, `0x10` above.
- Volume is reg `0x32` in 0.5 dB steps: `0xBF` = 0 dB, `0x00` = −95.5 dB.
- Unmute by clearing bits 5–6 of reg `0x31`.
- Waveshare's Arduino demo runs 16 kHz and volume `0xE5`, about +19 dB.
- A crates.io alternative exists: `es8311` 0.1.1 (blocking, embedded-hal 1.0, untested). It
  doesn't unmute or set volume in `init`.

**Game-side design (not built):**
- Make a `no_std` mixer crate beside `orbit_gfx`. It needs a few voices, each playing either:
  - a PCM clip from flash (`include_bytes!`, i16 or i8 at 22.05 kHz; a 0.5 s 16-bit clip is
    about 22 KB of flash, no RAM), or
  - a tiny synth: phase-accumulator square, triangle, or noise with an envelope.
- The game emits `Sfx` messages, like logic_kit's message-driven SFX. The platform drains them
  into the mixer and pumps samples:
  - device → `AudioOut::pump`;
  - desktop → **tinyaudio 2.0 + rtrb 0.4**, which works on macOS at 22,050 Hz mono, so desktop
    and device sound the same.
- Don't add cpal 0.18 to the same dependency graph as tinyaudio: the two hit an `alsa-sys`
  `links` conflict.
- Mixing 22 k samples/s is negligible CPU.

### 10.3 Input beyond touch (built: buttons and IMU; see section 3 and `orbit_core::ControlsPlugin`)
- QMI8658 6-axis IMU at `0x6B` on the shared I2C bus, interrupt on GPIO21. Good for
  flick-to-cast or tilt.
  - The crate `ph-qmi8658` 0.1.1 is **async-only** (embedded-hal-async). Use
    `Qmi8658Address::Secondary` for 0x6B; its default is 0x6A.
  - The blocking `IniterWorker/qmi8658` is unpublished and reads the wrong register for accel Y-low.
  - A small hand-written blocking reader is probably simplest.
- The BOOT button (GPIO0) and PWR button (via TCA9554 P4) are usable as buttons.
- Put the I2C bus behind a shared-bus wrapper (e.g. `embedded-hal-bus` `RefCellDevice`) once
  more than one driver needs it. Touch currently owns the bus outright.

### 10.4 Angler, the fishing game (built)

**Flow:** `crates/angler_game/src/session.rs` holds one `Phase` enum resource:
Title → Log | Aim → Flight → Waiting → Bite → Reel → Caught | Escaped, plus Notice messages
("ON THE DOCK!", "NOTHING BITES", "REELED IN", "LET GO!"). `Update` handles input-driven steps;
flight and reel physics run in `FixedUpdate` at 60 Hz.

**Cast** (`cast.rs`, Angry Birds-style side view):
- **Aim:** `elevation = clamp(40 - steer, 10, 80)`. Counterclockwise raises the rod. While
  aiming (before and during the charge) a dotted path shows the **full-power** cast for the
  current angle, ending on the water exactly where it would land. Aiming means putting the end
  of the dots on the fishing spot; a high lob and a low line can both reach it.
- **Power** (the user's rule): power only decides whether the cast reaches the dots.
  - While the action button is held, the meter ping-pongs 0→1→0 over `CHARGE_SECS` (1.1 s)
    per sweep.
  - Releasing at or above `PERFECT_POWER` (0.95, about 0.1 s around the peak) throws at
    exactly full power, landing on the end of the dots and flashing "PERFECT!".
  - Anything lower comes up short.
  - The gauge marks the window with a gold tip, and its fill turns gold inside it.
  - Below 0.08 is a slip; past `MAX_CHARGE_SECS` (4.4 s) the charge cancels.
- **The dots disappear on release.** There is no trail behind the bobber either.
- **Physics:** an exact projectile with `GRAVITY` 420 px/s² and `MAX_CAST_SPEED` 310 px/s.
- **Tested guarantees:**
  - full power reaches about x 428;
  - every arc at any angle stays inside the round glass;
  - the dotted preview equals the stepped flight.
- **The fishing spot** (rippling marks) sits at `spot_x` in 180–410 and moves after each
  catch or escape. `accuracy` (landing distance to the spot) shortens the wait for a bite.
  `distance` (how far out) and accuracy together favor rarer tiers and longer fish.

**Fight** (`reel.rs`, Stardew-style):
- **Layout:** a vertical track (326 px) with a 96 px catch zone.
- **Control:** leaning forward pushes the zone up, with inertia: 2° dead zone, full push at
  22°, 1100 px/s², damping 2.5.
- **Fish movement:** each species' `Temper` (agility spring, max speed, retarget interval,
  darts per second, depth bias) drives it.
- **Hooking:** like Stardew, the fish starts *inside* the zone at the bottom of the bar and
  holds still for `HOOK_SETTLE_SECS` (1 s). Then it makes a run for the far half, and after
  that it moves by its temper.
- **Progress:** starts at 0.3, fills at 0.30/s inside the zone, and drains at
  0.15 + 0.05 × tier per second outside.
- **Pacing history:** the first tuning lost fish in 1.2–1.9 s untouched, and the user found
  that far too quick. It was retuned against simulated players (`reel.rs` tests):
  - **untouched:** no fish escapes in its first 3 s;
  - **hesitant** (reacting 0.25 s late with a soft hand): lands tier 0–1 fish ≥ 75% of the
    time, and trophies sometimes but not always;
  - **skilled:** lands every species ≥ 90%;
  - **idle:** lands < 15% overall;
  - **sluggish:** finds pike harder than minnows.
  Retune the constants at the top of `reel.rs` against these tests.

**Fish** (`species.rs`):
- Eight species in tiers 0–3: minnow, bluegill, perch, trout, bass, catfish, pike, golden koi.
- Length is uniform within the species range, skewed long by accuracy.
- Weight in grams = `girth × cm³`, the fisheries length-weight rule.
- `sprites.rs` has the art; the catch screen scales the fish to about 280 px wide.

**Log** (`log.rs`):
- `FishLog { total, species: [{caught, best_length_mm, best_weight_g}; 8] }`.
- Encoded as `FLOG | v1 | count | total | 8-byte records`. Saves from builds with fewer
  species still load.
- `Milestones` (new species, longest, heaviest) drive the "NEW SPECIES!" / "NEW RECORD!"
  banners.
- Saved through `SaveBlob` → `orbit_save` (device flash at 0x9000, desktop
  `~/.logic-orbit/angler.flash`).

**Tuning knobs:** all constants are at the top of `cast.rs`, `reel.rs` and `species.rs`. The
tests check behaviour, not values, so retune freely and rerun `cargo test -p angler_game`.

**Not built yet:** sound (section 10.2 has the full plan and a compiled driver), and a reel
crank by circling a finger on the rim (touch is wired and unused in fights).

## 11. Gotchas (all hit or confirmed during bring-up)

- **The touch crate's example uses the 1.75C reset pin** (GPIO2). This board uses GPIO40.
- **Old I2S answers are wrong for esp-hal 1.2:** `write_dma_circular` was removed. See §10.2.
- **Button and touch edges (`pressed`, `released`, `just_pressed`) only exist during an update.**
  `Last` clears them. Tests must observe them from a system or through their effects, not by
  reading the resource after `app.update()`.
- **Bevy states are heap-expensive on this chip** (§7). Prefer a phase enum resource.
- **Holding PWR long switches the board off** (the PMIC, not the firmware). Never design a
  mechanic that holds PWR for more than a few seconds.
- **In QEMU, script by game time, not frame count** (§8).
- **`FlashStorage`'s inherent `read`/`write` shadow the `NorFlash` trait methods** (§3).
- **embedded-graphics' `Arc::bounding_box()` is the whole circle.** Using it would redraw the
  full screen every frame.
- **`cargo new` inside the repo can create a nested `.git`.** Delete it before committing.
- **A device build that logs nothing may still be running.** Check `ESP_LOG` before
  suspecting a hang. QEMU's `-d in_asm` trace shows where execution actually is.
- **`cargo clippy` doesn't relink.** Run `cargo build` before reading section sizes with
  `xtensa-esp32s3-elf-size -A`.
- **`Rgb565::BLACK`/`WHITE` need the `RgbColor` trait in scope.** `orbit_gfx` re-exports it.
- **`Touch::just_pressed` is edge-detected** against the previous frame. A `Last` system copies
  current to previous. Platforms only call `set`.
- **Paddle reach is the half-span plus the ball's angular radius** (about 24.6°). Tests that
  probe the edge must stay inside it.
- **esp-generate templates and older answers predate esp-hal 1.1–1.2.** The `psram` feature,
  `ESP_HAL_CONFIG_PSRAM_MODE` and `SpiDmaBus` are gone, and the buffer-taking SPI methods are
  now `*_buffer`.

## 12. Sources

- Waveshare: github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75 (and `-1.75C`),
  docs.waveshare.com/ESP32-S3-Touch-AMOLED-1.75
- espressif/esp_lcd_co5300 2.2.0 (CO5300 opcodes); SensorLib `TouchDrvCST92xx`
- github.com/okhsunrog/claudial (esp-hal 1.2 CO5300 transport); crates `cst92xx` 0.2.4
- Audio:
  - Waveshare BSP `esp32_s3_touch_amoled_1_75.c`;
  - esp_codec_dev `es8311.c` (esp-adf);
  - esp-hal v1.2.2 `qa-test/src/bin/embassy_i2s_sound.rs` and `hil-test/src/bin/i2s.rs`;
  - waveshare.com/esp32-s3-touch-amoled-1.75.htm (package contents).
- esp-hal v1.2.2: CHANGELOG, MIGRATING-1.0.0/1.1.0, `qa-test/src/bin/qspi_flash.rs`, esp-alloc `macros.rs`
- Bevy v0.19.1 `bevy_platform/src/time/fallback.rs`, `examples/no_std`
- espressif/qemu releases and issue #154
