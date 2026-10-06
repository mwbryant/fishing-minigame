# Bevy on the Waveshare ESP32-S3-Touch-AMOLED-1.75: platform handbook

Everything learned bringing `no_std` Bevy games to this board, written so a
fresh session can keep going without re-deriving it. There are two games:
**Orbit** (circular pong, touch) and **Angler** (fishing: tilt-aimed cast,
hold-to-charge, lean-to-reel, persistent fish log, gold and a hat shop; see section 10.4).
Facts are marked
**[verified]** (built, tested, read in source, or run in the emulator) or
**[unverified]** (from vendor sources or inference; not yet seen on hardware).

Status as of 2026-10-06: the board has **arrived**. Since this section was written it boots
the **arcade** (section 10.6): a rotation-dial menu over Angler, Spider Roulette, Skate and
Bees???. The arcade and the three new games are built, tested on the host and in QEMU, but
not yet run on the board. The first Angler session
(`logs/angler-20261006-153858.log`) shows octal PSRAM (8 MB) up, `display ready`,
Bevy built entirely in internal SRAM (about 72 KB of internal heap free during play),
the ES8311 answering `0x83 0x11`, and several minutes of casts and fights played at
60 fps. Its frame stats exposed the arc render cost (section 6), since fixed. Saves
surviving a power cycle, and the speaker being audible, aren't confirmed from the logs yet.
Both games, the renderer, and the simulator are tested on the host and in Espressif's QEMU.

---

## 1. Quick commands

```sh
# host workspace (game + renderer + simulator)
cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt
cargo run -p orbit_desktop                           # the arcade menu (the default) ...
cargo run -p orbit_desktop -- bees                   # ... or one game: angler spider skate bees orbit
cargo run -p orbit_desktop -- skate snapshot screenshots   # headless PNGs of a game's screens
#   desktop controls: mouse = finger, Space = action (PWR), Z = zero (BOOT; hold 3 s
#   to sleep, press to wake: the window goes dark, then it boots afresh),
#   left/right = turn the virtual device (it stays turned), hold up/down = lean it
#   15 degrees (it springs back level when released), C = straighten, Esc = quit.
#   Desktop saves live in ~/.logic-orbit/<mode>.flash (device flash format; the arcade
#   and its games share arcade.flash, one section each, like the board).

# device firmware (separate crate, Xtensa toolchain; the arcade plus one binary per game)
./flash.sh [arcade|angler|spider|skate|bees|orbit]  # from the repo root (default arcade)
./flash.sh monitor [game]                            # reopen the serial log only
#   every session's log is saved in logs/<game>-<timestamp>.log
cd firmware && . ~/export-esp.sh                     # by hand:
cargo run --release --bin arcade                     # build, flash over USB-C, serial monitor
cargo clippy --release && cargo fmt

# verification without a board
tools/heap32/run.sh [orbit|angler|arcade|spider|skate|bees]   # 32-bit heap vs SRAM budget
tools/qemu-smoke/run.sh [secs] ["orbit angler spider skate bees arcade"]   # QEMU; PASS/FAIL
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
crates/orbit_audio   #![no_std] 16 kHz mono PCM clips in flash + a one-voice `Mixer`; both
                     platforms pull samples from it. No Bevy.
crates/orbit_core    #![no_std] shared Bevy plumbing. `CorePlugin`: Frame, Touch, Rng, SaveBlob
                     (one record, a section per game), `Launcher`, `testing` helpers.
                     `ControlsPlugin` (opt-in): Buttons {action, zero}, ImuFeed -> Tilt (zero
                     re-levels), `Power` (battery, idle, hold-BOOT sleep).
                     `SoundPlugin` (opt-in): `SoundQueue` (mutable), `preferences` (mute).
crates/orbit_ui      #![no_std] shared drawing: Button, battery level, sleep overlay, the
                     rotation dial, CLICK/THUD.
crates/arcade        #![no_std] the boot menu: a dial over the games and SETTINGS (mute).
crates/angler_game   #![no_std] Angler (fishing; one phase-machine resource, no Bevy states).
crates/spider_game   #![no_std] Spider Roulette (twelve eggs, one spider).
crates/skate_game    #![no_std] Skate (rotation pops and lands ollies).
crates/bees_game     #![no_std] Bees??? (the Moon's honey jar; bees wear Angler's hat).
crates/orbit_game    #![no_std] Orbit (circular pong, touch only; uses Bevy states).
desktop/             std simulator for the arcade or any one game (minifb window, RAM model
                     of the panel, virtual IMU, file-backed save flash, PNG snapshots).
firmware/            ESP32-S3: lib (`Board`: bring-up once, then `run` one app at a time,
                     + drivers), the arcade binary and one binary per game. Excluded from
                     the workspace.
tools/heap32/        wasm32 heap measurement of every app (excluded from workspace).
tools/qemu-smoke/    QEMU runtime smoke test of every game and the arcade (excluded).
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

4. If the game has `SoundPlugin`, it drains `SoundQueue` into its `orbit_audio::Mixer`
   and tops up the speaker:
   - device: I2S DMA stream into the ES8311, `firmware/src/speaker.rs`;
   - desktop: `tinyaudio` + `rtrb`, `desktop/src/speaker.rs`.
5. It writes `SaveBlob::take_pending()` (if any) to storage. At boot, it passes
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

**BOOT:** GPIO0, active low, with an external 10k pull-up and a 100 nF debounce cap. It is a
low-power (RTC) pad, so it is the deep-sleep wake button (section 10.5).

**PWR:**
- Its live level is TCA9554 (0x20) pin P4, `SYS_OUT`, **high while pressed**. Make P4 an input
  by setting bit 4 of register 0x03, then read bit 4 of register 0x00.
- The AXP2101 (0x34) powers the board off when PWR is held. Register 0x27 bits 3:2 set the
  hold time (4/6/8/10 s, **default 6 s**). The firmware sets 10 s every boot (the PMIC forgets
  it when it loses power).
- Angler also cancels any charge held past 4.4 s (`MAX_CHARGE_SECS`).
- 0x22 bit 1 would disable hold-to-off entirely; it isn't touched.
- Don't use XPowersLib's `setIrqLevel`/`setOffLevel`; their bit masks are wrong.
- **PWR can't wake the ESP32 from deep sleep** [verified in the schematic netlist]. The
  AXP2101's IRQ (pin 38, net `AXP_IRQ`) goes only to TCA9554 P5 (`EXIO5`), and the
  expander's INT output reaches no ESP32 GPIO.

**AXP2101 battery** [verified in datasheet v1.4 §6.13; readings unverified on hardware],
`firmware/src/axp2101.rs`:
- REG 00 bit 5 is VBUS good, bit 3 battery present.
- REG 01 bits 6:5 are the current direction: 01 charging, 10 discharging, 00 standby.
- REG 34/35 is the battery voltage in mV: bits 13:8 in the low six bits of 0x34.
- REG A4 is the fuel gauge's percentage. The gauge (REG 18 bit 3) and the battery-voltage
  ADC (REG 30 bit 0) are on from power-on.

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
- **Frame time on device** [verified, first Angler log; the firmware logs average and
  worst every 2 s]:
  - a frame with nothing to redraw: about 5.4 ms (input polling plus the Bevy update);
  - waiting for a bite or fighting a fish: 6-7 ms at about 11 KB/frame;
  - a full-screen change: 34-98 ms (the lake scene about 35 ms), a one-frame hitch;
  - charging a cast **was 0.3-0.5 s per frame** (2-3 fps) before arcs got their own
    rasterizer; now about 25 us on the host versus 2.7 ms before, same ratio on device.
- **Dirty-rect rules:**
  - Static content costs nothing after the first frame.
  - Anything that changes repaints its whole bounding box, plus every shape that overlaps it.
  - `Shape::bounds()` must be tight and conservative. Arc bounds are computed by hand,
    because embedded-graphics reports an arc's whole circle.
  - **A shape's draw cost must scale with the band, not with the shape.** The presenter
    draws every overlapping shape once per band (about 58 bands for a full screen).
    embedded-graphics' `Arc` tests every point of its whole circle's bounding box on every
    call (its own `PERF` note), about 190k points for the r=212 rim gauge; that was the
    charging lag. `orbit_gfx::arc` walks only the band's rows inside the arc's bounds and
    the annulus spans per row; whole circles match embedded-graphics pixel for pixel.
    Circles, rects, lines, text and sprites already scale with the band.
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
| Orbit | 205 KB | 213 KB | 265 KB | **12 KB** |
| Angler | 199 KB | 203 KB | 204 KB | **73 KB** |
| Bees??? | 188 KB | 192 KB | 192 KB | **84 KB** |
| Skate | 187 KB | 190 KB | 191 KB | **86 KB** |
| Spider Roulette | 182 KB | 184 KB | 185 KB | **91 KB** |
| Arcade menu | 179 KB | 181 KB | 182 KB | **95 KB** |

The arcade firmware drops each app before building the next (`Board::run` returns the app,
the caller drops it), so its worst case is its biggest game. The QEMU arcade smoke checks
the internal heap after every return to the menu doesn't creep. Not counted above: the
speaker's 8 KB stream buffer, taken from the heap the first time a game makes sound.

Angler is 67 KB leaner at peak because it uses **one phase-machine resource and four systems
instead of Bevy states**. The hat shop (one `Inventory` resource, a `Phase::Shop`, no new
systems) cost it 3 KB, `Power` (battery, idle, sleep) another 3 KB, and the shared
sleep/idle system plus the mute preference 3 KB more. `StatesPlugin`, the `StateTransition` schedule, and every
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

**Flash (storage) is not a constraint** [verified, `espflash save-image`, 2026-10-06]:
- The 16 MB chip's factory app partition is 16,384,000 B. The arcade image (every game) is
  1,767,824 B (**10.8%**), Angler alone 1,562,768 B (9.5%), Orbit 952,480 B (5.8%), so
  about 14.6 MB is free.
- The `.stack` is what DRAM is left: about 69 KB for the arcade, 70 KB for one game.
- Angler's image is about 800 KB of code (`.text`, mostly Bevy) and 696 KB of read-only
  data. That data is 485 KB for the GUTEN song, 117 KB of sound effects, and the rest
  fonts, sprites and strings.
- Anything that can be a `static` (sprites, sounds, level data, text) lives in flash at no
  RAM cost: about 7.7 minutes of 16 kHz audio would still fit. The limits for more game are
  internal SRAM (the heap headroom above) and frame time.

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
  - **What it runs:** one binary per game plus the arcade (`src/bin/*.rs`) over a shared
    `Smoke` harness (`src/lib.rs`): the firmware's heap layout, a byte-counting panel,
    scripted input, `relaunch` (drop the app, build the next, as the arcade does) and
    `wait(secs)`. The two `.stack` sizes are printed; keep them within about 1 KB by
    adjusting the harness's `INTERNAL_HEAP_BYTES` (237 KB since the CPU-halt handler moved
    about 5 KB into IRAM).
  - **Pass criteria:**
    - Orbit: two or more starts, one or more losses, and 0 B in PSRAM.
    - Angler: two fish landed by a scripted steady-hand player, their gold spent in the hat
      shop (MENU from aiming, then SHOP and BUY), the log, settings and inventory written
      through `esp-storage` into emulated flash at 0x9000 and read back equal, and 0 B in PSRAM.
    - Spider Roulette: rotation aims at each egg in turn and PWR cracks it until the hidden
      spider pounces exactly where it was hidden; the SPDR section reaches flash.
    - Skate: a rider who pops just before each obstacle survives 20 s of game time, a cone
      then bails it, and the SKAT section reaches flash.
    - Bees???: a steady hand keeps 90%+ of the honey for 20 s, tipping empties the jar, and
      the BEES section reaches flash.
    - Arcade: the menu launches each game by rotation and PWR, HOME returns, twice round;
      the internal heap after each return to the menu stays within 2 KB of the first.
  - **Scripting rule:** emulated frames are much shorter than real ones (about 170 fps of game
    time, and it varies from run to run), so drive scripts by **game time**, never frame
    counts. That covers charge-until-power, waiting out the result guard, and bounding fights
    by seconds. The Orbit script was frame-based until a run got 4.7 s of game time out of
    its 2,000 frames (26 s the time before) and the round never restarted.
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

First flash: `./flash.sh` from the repo root builds and flashes Angler, then shows the
serial log, which is also saved in `logs/`. `./flash.sh orbit` flashes the simpler game.
If espflash can't connect, force download mode: unplug, hold BOOT, plug in, release BOOT,
flash, then replug to boot. Watch the serial log:
1. **`display ready`, then the title on the glass.**
   - Black screen: try the Arduino init list (§3).
   - Shifted image: adjust `COLUMN_OFFSET`.
2. **`bevy app built entirely in internal SRAM`.**
3. **Touch:** in Angler, tapping LOG on the title opens the fish log. If SETTINGS opens
   instead, flip `TOUCH_MIRROR_X`; if nothing happens, flip `TOUCH_MIRROR_Y`. In Orbit, the
   paddle follows the finger. If the log shows `CST9217 touch init failed`, check that this
   isn't a 1.75C.
4. **The 2 s stats line:** frame time (average and worst), bytes per frame, heap.

For Angler (`./flash.sh`), also check:
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
9. **Saves:** catch a fish, power-cycle, and the title's "CAUGHT n" and "GOLD n" (and the
   angler's hat) should persist. The boot log prints `save storage: found a save`.
10. **Battery:** the 2 s stats line ends with `battery 76% 3.98 V, USB, charging` (or
    `no cell, USB`). The title shows the same level at the top.
11. **Idle:** leave it untouched for 30 s. The panel dims, and the stats line shows about
    40 frames per 2 s (20 Hz). Any touch, button or turn brings it back.
12. **Sleep:** hold BOOT. At 1.5 s "GOOD NIGHT" shows; let go and the panel goes dark, with
    `sleeping; press BOOT to wake` in the log. A BOOT press boots afresh and logs
    `woke from sleep`.
13. **Audio:** after a catch (its flash save stalls the loop), the stats line may show
    `audio restarted Nx`. Sound must keep working afterwards, and GUTEN must play.

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

### 10.2 Sound (built; the codec answers on the board, played on desktop)

What shipped:
- **Format:** `orbit_audio` clips are mono s16le PCM at `SAMPLE_RATE` = 16 kHz, embedded with
  `include_bytes!` (flash, no RAM). Both platforms run at that rate, so nothing resamples.
- **Mixer:** one voice; `SoundCommand::Play` restarts, `Stop` silences. A game therefore
  queues at most one sound per update that matters: the last one.
- **Queue bound:** `SoundQueue` keeps only its newest 16 commands when nothing drains it
  (the heap32 and QEMU harnesses have no speaker), so frequent effects can't grow the heap.
- **Restarting the stream:** when the DMA runs out of descriptors it stops for good (esp-hal:
  "the transfer must then be resumed or restarted"). So `AudioOut::pump` restarts whenever
  `is_done()`. The first version also required the whole buffer to be free. An underrun that
  landed mid-pump then left bytes in a dead transfer, and the speaker went silent until
  reboot with nothing in the log; that was the GUTEN that once didn't play. A start the
  driver refuses is kept and retried on the next pump. Restarts and failed starts are in the
  2 s stats line.
- **Firmware** (`firmware/src/speaker.rs`, adapted from the reference driver):
  - starts only when the game added `SoundPlugin`, after the first `app.update()`;
  - I2S0 with `DMA_CH1`, `Channels::STEREO` (the same sample in L and R), MCLK on GPIO42;
  - the codec over the shared I2C bus, GPIO46 driven high to enable the amp;
  - the 8 KB stream buffer and its 8 descriptors are **leaked from the internal heap**
    (word-aligned `Box<[u32]>`), not `dma_tx_stream_buffer!` statics. So the `.stack` doesn't
    shrink, and games without sound (Orbit) don't pay for it.
  - The boot log prints `ES8311 id 0x83 0x11` when the codec answers.
- **Desktop:** `tinyaudio` at 16 kHz mono, fed through an `rtrb` ring that keeps about 1/15 s
  queued. If no device opens, it plays silently.

Research notes from before it was built follow.

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
Title → Log | Shop | Settings | Aim → Flight → Waiting → Bite → Reel → Caught | Escaped, plus
Notice messages ("ON THE DOCK!", "NOTHING BITES", "REELED IN", "LET GO!"). Every result
returns to Aim; the MENU button on the aiming screen (shown until the charge starts) is the
only way back to the title without a reboot. Continuing after a fish (caught or got away)
re-levels the IMU quietly via `Tilt::request_zero()`, so each cast starts at the base angle
from however the device is held. `Update` handles input-driven steps; flight and reel
physics run in `FixedUpdate` at 60 Hz.

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

**Gold and hats** (`shop.rs`):
- **Earning:** every landed fish pays `Species::gold` (minnow 2, bluegill 3, perch 5, trout 8,
  bass 12, catfish 15, pike 30, golden koi 80), scaled up to double for a fish at the top of
  its length range (`Catch::gold`). The catch screen shows "+n GOLD"; the title shows the
  purse. A typical cast (accuracy 0.7, distance 0.6) averages about 11 gold.
- **Hats:** `HATS`, cheapest first: STRAW HAT (free, always owned), BEANIE 15, BALL CAP 30,
  BUCKET HAT 50, COWBOY HAT 90, TOP HAT 150, FISH HAT 250, CROWN 500. A test pins the pacing:
  the first hat within 3 typical catches, the crown 40 or more, the whole shelf within 150
  (about 100 today).
- **Art:** hats are text sprites in `sprites.rs` on the angler's 14-column grid (face in
  columns 4-8). `ANGLER` is bareheaded with a hair row on top, and `draw::angler` draws the
  hat with its bottom row over that hair row. Moving the body's top-left down by the two
  removed rows kept `HANDS`, and so the rod and every cast, at the same pixel.
- **Shop screen:** < > browse with the angler in a "mirror" trying the hat on. BUY (which also
  puts it on) and WEAR share one spot, so one tap does whichever applies; a hat you can't
  afford shows "NEED n MORE" instead. BUY and WEAR save, browsing doesn't. PWR or BACK leaves.
- **`Inventory`:** gold (u32, saturating), owned hats as a u32 bitmask (a const assert caps
  the shelf at 32), and the worn hat, which is always an owned one.

**Home screen and settings:**
- **Title buttons:** LOG, SHOP and SETTINGS are on-screen touch buttons (`ui.rs` holds the
  rectangles shared by drawing and hit tests). Taps elsewhere on the title do nothing;
  PWR still starts fishing.
- **Difficulty:** `settings.rs`, EASY / NORMAL / HARD, stepped with the < > buttons. Each
  level has a `Tuning`:

  | Level | Catch zone | Drain | Fish speed | PERFECT from |
  |---|---|---|---|---|
  | EASY | 124 px | ×0.7 | ×0.85 | 0.90 |
  | NORMAL | 96 px | ×1 | ×1 | 0.95 |
  | HARD | 76 px | ×1.3 | ×1.15 | 0.98 |

  - Fish speed is a time-warp: spring × s², top speed and dart rate × s, retarget pauses ÷ s.
  - NORMAL is the tuning the reel tests pin.
  - A test checks that the same hesitant player lands more on EASY and fewer on HARD.
- **Save format** (`save.rs`): Angler's `SaveBlob` section `FLOG` (the record's other
  sections belong to the other games). `FLOG` version 3 = the fish log, a difficulty byte, gold (u32),
  the owned-hat bitmask (u32) and the worn hat (u8). Version-1 saves load at NORMAL, and
  version-1 and -2 saves load with no gold and the straw hat. Owned bits for hats a build
  lacks are dropped, and a worn hat that isn't owned falls back to the straw hat. Every
  difficulty change, catch, purchase and hat change saves (never during a fight: a sector
  erase stalls the frame loop for tens of ms).

**Reset progress:** RESET in settings opens "RESET PROGRESS?" (`Phase::ConfirmReset`).
ERASE clears the fish log, gold and hats (back to the straw hat) and saves; difficulty is kept.
KEEP sits exactly where RESET was, so a reflexive double tap cancels; PWR cancels too.

**GUTEN easter egg** (a riff on the user's logic-fish toy):
- Lives in the settings menu.
- Plays "Guten Morgen, Sonnenschein" (`assets/guten_morgen.pcm`, converted from
  logic-fish's mp3; see `assets/README.md`) for its 15.15 s.
- Meanwhile the rainbow-trout sprite spins once per 2.3 s (logic-fish's speed). The spin uses
  `Shape::SpriteFit`: width × |cos θ|, mirrored past 90°.
- A tap or a PWR press stops it early. Either way it queues `Stop` and returns to settings.

**Sound effects** (`audio.rs`, synthesized by `assets/sfx.py`, 117 KB of flash):
- CAST whoosh, or PERFECT_CAST (whoosh plus chime) on release.
- PLOP on the water, or THUD on the dock.
- BITE, a double chirp.
- REEL, a click each time the catch meter rises another `REEL_CLICK_STEP` (0.03), so the
  reel goes quiet while the fish is out of the zone.
- CATCH jingle, or ESCAPE "wah-wah".
- In the shop: COIN for buying, CLICK for wearing, THUD for a BUY that can't do anything.
- CLICK for any tap that lands on a button (taps elsewhere are silent).
- With one voice, `play` picks one effect per update (the event's own sound first, else
  the click), and the catch jingle is queued after the final reel click.

**Not built yet:** a reel crank by circling a finger on the rim.

### 10.5 Power: battery, sleep, idle (built; compiled for the board, not yet run on it)

The contract is `orbit_core::Power`, added by `ControlsPlugin` (so Orbit doesn't pay):
- **Battery:** the platform writes `battery: Option<Battery {percent, external_power,
  charging}>`. The firmware reads the AXP2101 every 5 s; the desktop reports USB power with
  no cell.
- **Indicator:** `orbit_ui::battery_level`, an outline filled green/yellow/red, the
  percent (or USB) and a bolt while charging, at the top of every menu in every game. During
  play it shows only at 15% or less and not charging (`running_low`).
- **Sleep:** one `ControlsPlugin` system handles it for every game. Holding the zero
  button (BOOT) for half a second shows HOLD TO SLEEP with the rim filling
  (`orbit_ui::power_overlay`). At 3 s (`SLEEP_HOLD_SECS`) the speaker stops and GOOD NIGHT
  replaces the frame; games ignore input while `Power::saying_good_night()`. After
  `GOOD_NIGHT_SECS` (2.5 s) `Power::sleep_requested()` is set, and the firmware:
  - waits for BOOT to come up (it wakes on a low level);
  - drives the amp enable low;
  - sends touch to sleep and the IMU's sensors off (CTRL7 = 0);
  - darkens the panel and sends DISPLAY_OFF (0x28) and SLEEP_IN (0x10);
  - arms GPIO0 on its low-power wake path and enters deep sleep.

  A BOOT press resets the chip, so the board boots afresh into the arcade menu (saves are
  already in flash). PWR can't wake it (section 3).
- **Idle:** 30 s without a touch, a button or a 3-degree turn sets `idle`. The firmware dims
  the panel to 0x40 and paces the loop at 20 Hz; the desktop dims and drops to 20 fps.
- **CPU halt:** the spare part of every frame halts the CPU with `waiti` until a systimer
  alarm interrupt (`firmware/src/idle.rs`). It used to spin in `Delay`. The display and
  speaker DMA keep running, and frame pacing is unchanged.

**Bevy settings and power** (as of Bevy 0.19, no_std): Bevy's own power knob is
`WinitSettings` (reactive, low-power updates), which belongs to the winit/render stack this
build doesn't have. Nothing else in `default_no_std` affects power. The frame loop is ours,
so the levers are ours too:
- The loop itself: halting between frames, and the slower idle rate, both built.
- The panel: brightness, plus dark scenes, since a black AMOLED pixel is off. AMOLED draw
  scales with brightness times lit pixels, and the lake scene is bright sky at 0xFF.
- Peripherals: the sleep sequence above.

Not done yet, roughly by value:
- A brightness setting. The default is full; even 0xC0 should save noticeably.
- Switching the NS4150B amp off after a second of silence. It costs a few mA at idle, but
  waking it can clip a sound's start.
- A 160 MHz CPU clock. It saves some power, but every frame gets 1.5x longer.
- AXP2101 soft power-off (REG 10 bit 0) as a deeper "off": PWR would power it back on, BOOT
  could not.

### 10.6 The arcade and its games (built; compiled for the board, run in QEMU, not yet on it)

**Flow:** `firmware/src/bin/arcade.rs` loops. `Board::run` builds the menu app (`crates/arcade`)
and runs it until it sends Bevy's `AppExit`. The caller reads `Menu::chosen()` and drops
the menu, then builds the picked game with `orbit_core::Launcher` inserted. The game's title
then offers HOME, which sends `AppExit`, and the loop goes back to the menu. One app exists
at a time. The desktop's `play` does the same.

**Saves:** one `orbit_save` record holds every game's state as `SaveBlob` sections
(`ORBC | count | (key | len u16 | bytes)*`): `FLOG` Angler, `SPDR` Spider Roulette, `SKAT`
Skate, `BEES` Bees???, and `PREF` the shared preferences (mute). A record from before
sections loads as the section named by its first four bytes, so old Angler saves (which
start `FLOG`) carry over. A section can be read by another game: Bees??? reads `FLOG` for
the hat the angler wears.

**Rotation first:** turning the device selects and steers everywhere; PWR confirms; touch is
secondary.
- `orbit_ui::dial_choice(steer, n)` picks one of `n` options spaced round the glass from 12
  o'clock, with the pointer at 3x the turn (`DIAL_GAIN`), so +-60 degrees reaches all of
  them. The menu and Spider Roulette draw it as a gold arc on the rim.
- The menu and every game re-level (`Tilt::request_zero`) when they start a round, so
  however the device is held is "level".

**The menu** (`crates/arcade`): rings for ANGLER, SPIDER ROULETTE, SKATE, BEES??? and
SETTINGS, 72 degrees apart at radius 160. The pointed ring lights and its name fills the
middle. SETTINGS is a two-option dial, SOUND ON/OFF (`preferences::set_muted`, applied by
`SoundPlugin` at every game's startup) and BACK. The black background is free on an AMOLED.

**Spider Roulette** (`crates/spider_game`):
- **Board:** twelve eggs on a web at radius 160; one hides the spider, re-hidden each round.
  PWR or a tap cracks the aimed egg.
- **Tension:** safe eggs show a shell and shorten the odds in the middle ("1 IN 12" down to
  "1 IN 2"). A heartbeat speeds up from 1.25 s to 0.35 s between beats.
- **Endings:** the spider's egg pounces (three bounds over a red flash, a screech, then GOT
  YOU!). Eleven safe eggs trap it. The best run of safe picks persists.

**Skate** (`crates/skate_game`):
- **Controls:** a counterclockwise turn past `POP_DEG` (15) pops an ollie. The next pop
  needs the turn back past `REARM_DEG` (7). In the air the board follows the turn, and
  landing more than `LAND_DEG` (25) off level bails.
- **Physics:** gravity 1500 px/s², pop 560 px/s (about 0.75 s of air). Speed goes from 220
  to 420 px/s.
- **Obstacles:** cones 18x21, benches 120x36 (land level on top to grind, 25 points/s). They
  spawn at random gaps scaled by speed.
- **Score:** metres (30 px each), plus 10 per clean landing, plus grind time. The best
  persists.
- **Playability is pinned by a test:** a rider who pops 0-55 px before each obstacle
  survives 20 s.
- **Render cost:** only the rider, obstacles and lane dashes move over the static night city.

**Bees???** (`crates/bees_game`):
- **The jar:** the Moon is an open jar orbiting the Earth at radius 150, mouth outward,
  speeding from 45 to 100 degrees/s.
- **Wobble:** an unseen wobble eases toward random targets whose reach grows from 35 to 80
  degrees over 90 s. The device's turn counters it at `CONTROL_GAIN` 2.
- **Spilling:** honey pours once the tilt passes `spill_limit_deg(honey)`, 40 degrees full
  and 100 nearly empty, at up to 30%/s. The drops fall to Earth.
- **The bees:** three circle the jar, and one flies a 5% refill up every 3.5 s. They wear
  the angler's hat at scale 2, seated by the hat's head columns (4-8 on the angler's grid)
  over the bee's head, and mirrored when a bee flies left.
- **Score:** the game ends when the jar is empty; the best number of orbits persists.

**Sounds:** each game synthesizes its own with a small `assets/sfx.py`, importing Angler's
generator helpers (standard library only, seeded). That is 61 KB for Spider Roulette, 21 KB
for Skate and 42 KB for Bees???.

## 11. Gotchas (all hit or confirmed during bring-up)

- **The touch crate's example uses the 1.75C reset pin** (GPIO2). This board uses GPIO40.
- **Old I2S answers are wrong for esp-hal 1.2:** `write_dma_circular` was removed. See §10.2.
- **Button and touch edges (`pressed`, `released`, `just_pressed`) only exist during an update.**
  `Last` clears them. Tests must observe them from a system or through their effects, not by
  reading the resource after `app.update()`.
- **Bevy states are heap-expensive on this chip** (§7). Prefer a phase enum resource.
- **A dropped touch report looks like lift-then-touch, which is a new tap.** The firmware
  keeps the finger through a failed read and needs two empty reads to lift it. Screens that
  a tap dismisses should still ignore taps for a moment (GUTEN: 0.6 s).
- **An I2S stream that underran is dead until restarted** (§10.2). Restart on `is_done()`
  alone.
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
