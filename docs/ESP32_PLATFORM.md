# Bevy on the Waveshare ESP32-S3-Touch-AMOLED-1.75: platform handbook

This handbook covers running the repo's `no_std` Bevy games on Waveshare's round AMOLED
board: the hardware, toolchain, Bevy without std, render and memory budgets, and verification
with and without the board. Sections 1-9 are the platform, section 10 the systems and games
built on it, and sections 11-12 the gotchas and sources.

The board boots **Fishing Minigame** (10.6), a Nintendogs-style loop. You walk a town by
touch, catch fish at the pond (**Angler**, 10.4), keep and name them, walk them, teach them
tricks by voice and touch, and win contest gold to buy them hats. Spider Roulette, Skate and
Bees??? live in the town. **Orbit** (circular pong, touch) is a separate binary.

**Contents:** 1 Quick commands · 2 Repo map and frame loop · 3 Hardware reference ·
4 Toolchain and crates · 5 Bevy without std · 6 Rendering · 7 Memory · 8 Verifying without a
board · 9 Bring-up checks on the board · 10 Systems and games (10.1 sprites, 10.2 sound,
10.3 input, 10.4 Angler, 10.5 power, 10.6 Fishing Minigame) · 11 Gotchas · 12 Sources

## Board status

The board is the -G variant, with the LC76G GNSS. Everything runs on it: every game and
place at 60 fps, touch, tilt, both buttons, voice commands, step-counted walks, sound, saves, idle
dimming, switching off and on (10.5), and battery charging. The display, touch, IMU and PWR
work with the original constants; no mirror, offset or axis needed changing. The serial logs in `logs/`
record sessions, for example:
- `angler-20261006-153858.log`: octal PSRAM (8 MB), `display ready`, Bevy built entirely in
  internal SRAM with about 72 KB of heap free in play, the ES8311 answering `0x83 0x11`, and
  minutes of casts and fights at 60 fps.
- `pals-20261007-*.log`: one session built and dropped 24 apps through the town's doors, all
  in internal SRAM.

Host tests and Espressif's QEMU cover every game, the renderer and the simulator.

---

## 1. Quick commands

The everyday commands; sections 8 and 9 have the details.

```sh
# host workspace (games, renderer, simulator)
cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt
cargo run -p orbit_desktop                           # Fishing Minigame's town (the default) ...
cargo run -p orbit_desktop -- home                   # ... or one app: town home park contest shop
                                                     #     angler spider skate bees orbit
cargo run -p orbit_desktop -- skate snapshot screenshots   # headless PNGs of an app's screens

# device firmware (separate crate, Xtensa toolchain; Fishing Minigame plus one binary per game)
./flash.sh [pals|angler|spider|skate|bees|orbit]    # from the repo root (default pals)
./flash.sh monitor [game]                            # reopen the serial log only
#   every session's log is saved in logs/<game>-<timestamp>.log
cd firmware && . ~/export-esp.sh                     # by hand:
cargo run --release --bin pals                       # build, flash over USB-C, serial monitor
cargo clippy --release && cargo fmt

# verification without a board
tools/heap32/run.sh [orbit|angler|town|home|park|contest|shop|spider|skate|bees]   # 32-bit heap vs SRAM budget
tools/qemu-smoke/run.sh [secs] ["orbit angler spider skate bees pals"]   # QEMU; PASS/FAIL
```

- **Desktop keys:** mouse = finger, Space = action (PWR), Z = zero (BOOT). Hold Z 3 s to
  switch off; Space turns it back on, and the app boots afresh. Left/right turns the virtual device (it
  stays turned), held up/down leans it 15 degrees (it springs back), C straightens, Esc quits.
- **Desktop saves:** `~/.logic-orbit/<mode>.flash`, in the device's flash format. The town and
  its places share `town.flash`, a section each, like the board.
- **Desktop mic and walks:** the laptop's mic stands in (macOS asks once), and walks use MOCK
  WALK from the town's settings.
- **Dev Mac:** `espup` 0.18, `espflash` 4.6, the `esp` Rust toolchain (rustc 1.97.0.0, from
  `espup install --targets esp32s3`, which writes `~/export-esp.sh`), and Homebrew
  `libgcrypt` (QEMU only).

## 2. Repo map and frame loop

Where the code lives, how a frame flows, and the coding conventions.

```
crates/orbit_gfx     #![no_std] rasterizer (disc/ring/arc/label/rect/line/sprite), dirty-rect
                     diff, band Presenter, `Panel` trait. No Bevy.
crates/orbit_motion  #![no_std] Mahony IMU filter + zeroing -> steer/lean degrees. Bevy math only.
                     `VirtualDevice`: the simulators' board, turned by keys or posed by a
                     phone, reporting IMU samples.
crates/orbit_save    #![no_std] power-fail-safe two-sector record store over embedded-storage NOR
                     flash, plus `RamFlash` (tests, desktop).
crates/orbit_audio   #![no_std] 16 kHz mono PCM clips in flash + a one-voice `Mixer`; both
                     platforms pull samples from it. `voice`: voice prints and matching.
                     No Bevy.
crates/orbit_core    #![no_std] shared Bevy plumbing. `CorePlugin`: Frame, Touch, Rng, SaveBlob
                     (one record, a section per game), `Launcher`, `testing` helpers.
                     `ControlsPlugin` (opt-in): Buttons {action, zero}, ImuFeed -> Tilt (zero
                     re-levels), `Power` (battery, idle, hold-BOOT sleep).
                     `SoundPlugin` (opt-in): `SoundQueue` (mutable), `preferences` (mute).
                     `MicPlugin` (opt-in): `Mic` (words heard). `StepsPlugin` (opt-in):
                     `Steps` (new steps counted). `GpsPlugin` (opt-in, unused): `Gps`
                     (NMEA in, fixes out).
crates/orbit_ui      #![no_std] shared drawing: Button, battery level, sleep overlay, the
                     rotation dial and list, typing by turning, the gold coin, CLICK/THUD.
crates/pals          #![no_std] Fishing Minigame's state and rules, no systems: pets, tricks,
                     gestures, walks (stride, mock walk), gold, the hat shelf, `PALS` save.
crates/pals_game     #![no_std] Fishing Minigame's places, one app each: the town (boot), the
                     house, the park, the contest hall, the hat shop.
crates/angler_game   #![no_std] Angler, the pond (fishing; keep and name; no Bevy states).
crates/spider_game   #![no_std] Spider Roulette, the den (twelve eggs, one spider, bets).
crates/skate_game    #![no_std] Skate, the arcade cabinet (ollies; halfpipe tricks).
crates/bees_game     #![no_std] Bees???, the humming tree (bees wear the lead fish's hat).
crates/orbit_game    #![no_std] Orbit (circular pong, touch only; uses Bevy states).
desktop/             std simulator for Fishing Minigame or any one app (minifb window, RAM model
                     of the panel, virtual IMU, the laptop's mic, file-backed save flash,
                     PNG snapshots).
web/                 the itch.io build (Trunk from the repo root's index.html): the same apps
                     in a browser. A canvas for the panel, WebAudio for the speaker and mic,
                     the simulator's keys, a touch pad (a stick around a mini board showing
                     `Tilt`, on its own canvas) or a phone's tilt, localStorage for the save
                     flash.
                     `web/smoke.mjs` plays the build in headless Chromium.
firmware/            ESP32-S3: lib (`Board`: bring-up once, then `run` one app at a time,
                     + drivers), the pals binary and one binary per game. Excluded from
                     the workspace.
tools/heap32/        wasm32 heap measurement of every app (excluded from the workspace).
tools/qemu-smoke/    QEMU runtime smoke test of every game and the Fishing Minigame tour
                     (excluded).
docs/reference/      research: the ES8311 speaker driver, and the LC76G and ES7210 notes.
```

**The frame loop**, the same on the board and the desktop:
1. The platform writes inputs:
   - touch: `Touch::set(Option<Vec2>)`, in screen pixels;
   - buttons (`ControlsPlugin`): held state via `Buttons.{action,zero}.set`;
   - IMU: `ImuFeed::push(ImuSample)`, in the screen frame;
   - mics (`MicPlugin`): `Mic::push(&[i16])`, 16 kHz mono;
   - steps (`StepsPlugin`): `Steps::add(count)`, new steps since the last frame;
   - GPS (`GpsPlugin`, unused): NMEA text via `Gps::push(&[u8])`.
2. `app.update()` runs the systems; `draw_frame` (PostUpdate) refills `Frame { shapes }`.
3. `Presenter::present(&shapes, &mut panel)` diffs against the last frame (`FrameDiff`), takes
   the bounds of every shape that appeared or vanished (even-aligned, merged), renders each
   dirty rect in full-width bands into a small buffer (`render`), and sends each band with
   `Panel::write(area, bytes)` as big-endian RGB565, the panel's wire format.
4. With `SoundPlugin`, it drains `SoundQueue` into an `orbit_audio::Mixer` and tops up the
   speaker: I2S DMA into the ES8311 on the board (`firmware/src/speaker.rs`), `tinyaudio` +
   `rtrb` on the desktop (`desktop/src/speaker.rs`).
5. It writes `SaveBlob::take_pending()` (if any) to storage. At boot, storage's bytes come in
   through `SaveBlob::from_storage`.

The desktop `Panel` copies into a 466x466 RAM model; the firmware's is the CO5300 driver
(QSPI + DMA). So the simulator shows exactly the device's pixels, dirty-rect bugs included.

**Conventions** (shared with the user's other logic-* games):
- Red-green TDD with behavioral test names, `cargo clippy` + `cargo fmt`, a commit per feature.
- The user's style rules: no `else { None }` tails, no comments that restate the code, `use`
  imports instead of full paths, no `_`-prefixed unused variables (remove them).
- UI strings are ASCII only. The font is embedded-graphics' `FONT_10X20`, scaled by an
  integer.

## 3. Hardware reference

The board, its pins and buses, and how to drive each chip.

- **Board:** Waveshare ESP32-S3-Touch-AMOLED-1.75 (SKU 31261; the user's is the -G, 31264).
- **SoC:** ESP32-S3R8: 2x LX7 at 240 MHz, 512 KB SRAM, 8 MB **octal** PSRAM, 16 MB flash.
- **Vendor sources:** github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75 (Arduino demos and
  the ESP-IDF BSP `waveshare__esp32_s3_touch_amoled_1_75`).
- **Variant trap:** the **1.75C** is a different board with different reset pins (LCD reset
  GPIO1, touch reset GPIO2). Everything here is for the plain 1.75.

### Pins (from Waveshare's source and schematic)

| Function | GPIO |
|---|---|
| QSPI LCD: CS / SCLK / D0 / D1 / D2 / D3 | 12 / 38 / 4 / 5 / 6 / 7 |
| LCD reset / LCD TE (tearing) | 39 / 13 (TE unused so far) |
| I2C SDA / SCL (400 kHz, 2.2k pull-ups) | 15 / 14 |
| Touch INT / touch reset | 11 / 40 |
| I2S MCLK / BCLK / WS(LRCK) / DOUT (to ES8311) / DIN (from ES7210 mics) | 42 / 9 / 45 / 8 / 10 |
| Speaker amp (NS4150B) enable, active high, pulled down | 46 |
| SD card | 1, 2, 3 (+41) |
| IMU INT2 (unused) | 21 |
| BOOT button | 0 (active low) |
| Octal PSRAM (never assign) | 33-37 |

- **PWR button:** goes to the AXP2101's PWRON pin; firmware reads it as TCA9554 P4
  (high = pressed).
- **No setup for display or touch:** neither the AXP2101 PMIC nor the TCA9554 expander needs
  configuring first.
- **Brightness:** no backlight pin; use panel register `0x51`.
- **Stale MCLK define:** Waveshare's Arduino `pin_config.h` says `I2S_MCK_IO 16`, but the
  schematic routes GPIO16 to header pin 8. MCLK is GPIO42.

### I2C addresses

All on one bus; each driver gets an `embedded-hal-bus` `RefCellDevice` on it.

| Chip | Address |
|---|---|
| ES8311 codec | `0x18` |
| TCA9554 expander | `0x20` |
| AXP2101 PMIC | `0x34` |
| ES7210 mic ADC | `0x40` |
| LC76G GNSS (-G board only) | `0x50` / `0x54` / `0x58` |
| PCF85063 RTC | `0x51` |
| CST9217 touch | `0x5A` |
| QMI8658 IMU | `0x6B` |

### Mics and GNSS

- **Mics** (`firmware/src/mic.rs`): the ES7210 runs esp-adf's slave-mode sequence, MIC1 and
  MIC2 at 30 dB.
  - Data comes in on I2S0's receive side (GPIO10), clocked by the speaker's transmit side via
    `TdmConfig::with_signal_loopback(true)`. So the speaker stream must run, which it does
    from the first app with sound.
  - The 4 KB receive stream starts the first time an app has `Mic`. The two mics are averaged.
- **GNSS** (`firmware/src/lc76g.rs`): the -G board's Quectel LC76G is on the shared I2C bus,
  not a UART (its UART resistors are marked NC).
  - At boot the firmware drives TCA9554 P7 (its reset) high and probes `0x50`.
  - While an app has `Gps`, it reads NMEA one I2C step per frame, about once a second. Each
    step wants 10 ms after the last, so it never blocks for 20 ms.
  - The module reports every second, fix or not. Silent for `GPS_SILENT_SECS` (5 s) while an
    app listens, it's restarted: P7 low for 20 ms, then high (`Lc76g::restart`). Its backup
    supply keeps the sky it knew, so a fix comes back in seconds.
  - The 2 s stats line adds `gps <n> B` (NMEA bytes since the last line) while an app
    listens, and `restarted <n>x` once it has been restarted.
  - **Asleep from boot:** it has no power enable and draws about 36 mA running, and nothing
    uses it, so the firmware sends `$PAIR650,0*25` at boot (backup mode, 13 uA, through the
    `0x50`/`0x58` write path) and 1.5 s later checks that it queues nothing (`LC76G asleep
    (backup mode)`). The LC76G (AB) may keep VCC on in backup mode; only the PA and PB need it
    cut (hardware design v1.3, 3.3.4, note 3). It wakes only when the board loses power, so an
    app that adds `GpsPlugin` again needs the boot command removed.
- Register sequences, protocol and sources: `docs/reference/gnss_and_mics.md`.

### CO5300 display protocol

- **SPI:** mode 0 at 40 MHz, as in every Waveshare demo. The datasheet maximum write clock is
  reportedly 50 MHz.
- **Register write:** opcode `0x02` + 24-bit address `0x00RR00`; parameters on one line.
- **Pixel write:** a RAMWR (`0x2C`) register write, then opcode `0x32` + address `0x003C00`
  (memory-write-continue) with the data on 4 lines. RGB565, high byte first.
- **Window:** the 466 columns are RAM columns **6..=471** (x offset 6, y offset 0).
- **Alignment:** windows start on even coordinates, with even width and height
  (`orbit_gfx::align_even`).
- **Init** (`firmware/src/co5300.rs`, Waveshare's ESP-IDF BSP list): `36 00`, `3A 55`, then
  page-`0x20` vendor writes (`FE 20`, `19 10`, `1C A0`), then `FE 00`, `C4 80`, `3A 55`,
  `35 00`, `53 20`, `51 FF`, `63 FF`, CASET `00 06 01 D7`, RASET `00 00 01 D1`, `11`, wait
  120 ms, `29`.
- **Fallback init** (Waveshare's Arduino_GFX): `11`, wait 120 ms, `FE 00`, `C4 80`, `3A 55`,
  `53 20`, `63 FF`, `29`, `51 D0`, `58 00`, `20`, `36 00`.
- **Reset:** low 10 ms, high, wait 150 ms.
- **Transport template:** github.com/okhsunrog/claudial (`firmware/src/co5300.rs`), on the
  AMOLED-2.16 (same CO5300, same LCD/touch/I2C pins): manual CS, 40 MHz, esp-hal 1.2 DMA. Its
  MADCTL `0xA0` is 2.16-specific; don't copy it.
- **No tearing sync yet:** claudial found the 2.16's TE line grounded. On the 1.75 it reaches
  GPIO13, so TE sync may be possible.

### CST9217 touch

- **Crate:** `cst92xx` 0.2.4, `default-features = false, features = ["blocking"]`.
  `CST92xx::new(i2c, delay).with_reset(gpio40).init()`, then `touches()` returns
  `[Option<Point>; N]` in raw panel coordinates.
- **Orientation:** Waveshare's BSP mirrors both axes (`x = 465 - x`, `y = 465 - y`), and so do
  the firmware's `TOUCH_MIRROR_X/Y`.
- **Pin caution:** the crate's own example uses TP_RST = GPIO2, the 1.75C pin.
- **Dropped reports:** the firmware keeps the finger through a failed read and lifts it only
  after two empty reads (`TOUCH_RELEASE_READS`), so a dropout can't fake a tap.

### Buttons

- **Layout** (front view, UI upright with MADCTL 0x00): USB-C at 9 o'clock. Both buttons on
  the right edge: **PWR at about 2 o'clock, BOOT at about 4 o'clock**, the TF slot between.
- **Roles:** the user's spec is "the bottom button re-zeroes the IMU", so **BOOT = zero** and
  **PWR = action** (hold to charge a cast). `ACTION_BUTTON` / `ZERO_BUTTON` in
  `firmware/src/lib.rs` swap them.
- **BOOT:** GPIO0, active low, external 10k pull-up and 100 nF debounce cap. A low-power (RTC)
  pad, so it can wake deep sleep (the fallback in 10.5).
  - It is also a strapping pin. On this board, entering download mode over USB leaves
    `GPIO_STRAP` (0x60004038) at 0x23, bit 3 clear ("GPIO0 low"), while `GPIO_IN` bit 0 reads
    BOOT high and `RTC_CNTL_OPTION1` (0x6000812C) is 0. A USB reset (`rst:0x15
    USB_UART_CHIP_RESET`, espflash's default after flashing and Ctrl+R) keeps the latch, so
    the ROM starts download mode again ("waiting for download"); a normal boot reads 0x2b.
    Only a power cycle or an RTC watchdog reset reads the pin again. `espflash monitor
    --non-interactive` (which `flash.sh` runs) checks bit 3 and uses the watchdog reset; it
    reopens the port after 1.5 s, so that boot's first lines are missing from the log.
- **PWR:**
  - Live level: TCA9554 (0x20) pin P4, `SYS_OUT`, **high while pressed**. Make P4 an input
    (set bit 4 of register 0x03), then read bit 4 of register 0x00.
  - The AXP2101 (0x34) powers off when PWR is held. Register 0x27 bits 3:2 set the hold time
    (4/6/8/10 s, **default 6 s**). The firmware sets 10 s every boot, since the PMIC forgets it
    without power. 0x22 bit 1 would disable hold-to-off; it isn't touched.
  - Angler cancels any charge held past 4.4 s (`MAX_CHARGE_SECS`).
  - Don't use XPowersLib's `setIrqLevel`/`setOffLevel`: their bit masks are wrong.
  - **PWR can't wake the ESP32 from deep sleep** (per the schematic netlist). The
    AXP2101's IRQ (pin 38, net `AXP_IRQ`) reaches only TCA9554 P5 (`EXIO5`), and the
    expander's INT output reaches no ESP32 GPIO. It does turn the AXP2101 back on after a
    power-off, which is how the board wakes (10.5).
  - **Power-off:** setting REG 0x10 bit 0 switches every AXP2101 output off. DCDC1 is the
    board's one 3.3 V rail (ESP32, display, touch, codec, mics, IMU, GPS), so nothing draws
    until PWR turns it back on as a power-on boot.

### AXP2101 battery (datasheet v1.4 §6.13)

`firmware/src/axp2101.rs`, read every 5 s:
- REG 00: bit 5 VBUS good, bit 3 battery present.
- REG 01 bits 6:5: current direction (01 charging, 10 discharging, 00 standby).
- REG 34/35: battery voltage in mV, bits 13:8 in the low six bits of 0x34.
- REG A4: fuel-gauge percentage. The gauge (REG 18 bit 3) and battery-voltage ADC (REG 30
  bit 0) are on from power-on.

### QMI8658 IMU

`firmware/src/qmi8658.rs`, a hand-written blocking driver at `0x6B`:
- **Boot:** reset with `0x60 = 0xB0`, poll until `0x4D == 0x80`. WHO_AM_I (0x00) must read
  0x05.
- **Configuration:**
  - CTRL1 = 0x60: auto-increment, little-endian, as both vendor drivers decode. If data reads
    as garbage, try 0x40.
  - CTRL2 = 0x13: ±4 g, 8192 LSB/g.
  - CTRL3 = 0x53: ±512 dps, 64 LSB/dps.
  - CTRL5 = 0x33: both low-pass filters.
  - CTRL7 = 0x03: accel and gyro on.
- **Reading:** one 12-byte burst from 0x35.
- **Steps** (while an app has `Steps`): the accelerometer reads above go to
  `orbit_motion::StepCounter`, every frame awake and once a doze wake (10 a second):
  - a step is the acceleration's size dipping more than `STEP_SWING_G` (15 mg) under its 1 s
    running average and coming back up more than 15 mg over it, at most one every
    `STEP_GAP_SECS` (0.2 s);
  - turning the board doesn't count (gravity's size stays 1 g), but any bounce or shake does,
    so it overcounts on purpose;
  - the chip's built-in pedometer (SensorLib's `configPedometer`, CTRL9 command 0x0D, count at
    0x5A-0x5C) counted no steps on the board even tuned down to 120 mg and 0.21 s steps, so
    it's unused;
  - while the walk dozes, CTRL7 = 0x01 turns the gyro off; steps need only the accelerometer.
- **Axes in the screen frame** (x right, y down, z into the screen):
  `screen = (imu.y, -imu.x, imu.z)`, from the PCB axis marking by U3 and three Waveshare tilt
  demos. At rest: face-up, accel z ≈ −1 g; upright with USB on the left, x_imu ≈ +1; button
  edge up, y_imu ≈ +1.
- **Other drivers:** `ph-qmi8658` 0.1.1 is **async-only** (embedded-hal-async; use
  `Qmi8658Address::Secondary` for 0x6B, its default is 0x6A). The blocking
  `IniterWorker/qmi8658` is unpublished and reads the wrong register for accel Y-low.
  Waveshare's ESP-IDF qmi8658 component has an off-by-one gyro range table.

### Flash storage

`esp-storage` 0.10, features `["esp32s3", "embedded-storage"]`:
- `FlashStorage::new(peripherals.FLASH)` implements `NorFlash` (write 4, erase 4096) and
  `ReadNorFlash` (read 4).
- **Trap:** `FlashStorage`'s *inherent* `read`/`write` (read-modify-write with automatic
  erase) shadow the trait methods. Generic code over `F: NorFlash` (like `orbit_save`) is
  fine; concrete calls must use `NorFlash::write(..)`.
- **Where:** saves live at **0x9000 and 0xA000**, in the default partition table's `nvs`
  region. `espflash flash` only writes 0x0, 0x8000 and 0x10000, so saves survive reflashing,
  but not `erase-flash` or `--erase-parts`. `orbit_save`'s magic and CRC ignore the old NVS
  bytes the factory image leaves there.
- **When:** a sector erase blocks everything for tens of ms, so apps save at rest points (a
  catch, a keep, a settings change, every 25 m of a walk), never during a fight.

## 4. Toolchain and crate set

The firmware's crate versions and the project settings they need.

| Crate | Version / features |
|---|---|
| esp-hal | `~1.2.2`, `["esp32s3", "log-04", "unstable"]` (`unstable` gates DMA, PSRAM, I2S, `with_sio*`, half-duplex) |
| esp-bootloader-esp-idf | 0.6.0 |
| esp-alloc | 0.11.0 (the chip feature is mandatory on all of these) |
| esp-println | 0.18.0 |
| esp-backtrace | 0.20.0 `["panic-handler", "println"]` |
| esp-storage | 0.10.0 `["esp32s3", "embedded-storage"]` |
| embedded-hal-bus | 0.3 (the shared I2C bus) |
| cst92xx | 0.2.4, `default-features = false, features = ["blocking"]` |
| bevy | `0.19`, `default-features = false, features = ["default_no_std"]` |

- Bevy 0.19.1 is the latest stable; 0.20 is at rc.2.
- Don't mix the esp-hal 1.2 crate set with the older one (esp-alloc 0.10 and so on).
- esp-hal 1.2 has **no `psram` feature**. PSRAM is a runtime `PsramConfig` whose default
  `Auto` tries octal, then quad. The firmware uses `Auto` so the same code runs in QEMU, which
  only emulates quad.
- `esp_bootloader_esp_idf::esp_app_desc!()` is required; espflash 4.x refuses images without
  it.
- **Project files:** `rust-toolchain.toml` sets `channel = "esp"`. `.cargo/config.toml` sets
  target `xtensa-esp32s3-none-elf`, `build-std = ["alloc","core"]`, rustflag
  `-C link-arg=-nostartfiles`, and runner `espflash flash --monitor --chip esp32s3`.
  `build.rs` emits `-Tlinkall.x`.
- Always build `--release`: PSRAM needs it, and unoptimized Bevy is far too slow.
- Every link prints a harmless "LOAD segment with RWX permissions" warning.
- Logging needs `[env] ESP_LOG = "info"` in `.cargo/config.toml`. esp-println filters at
  compile time, so without it every `info!` silently disappears.

## 5. Bevy without std

What the `no_std` Bevy build gives you and the rules it imposes.

- **What you get:** `bevy` with `default_no_std` (libm, critical-section, bevy_color,
  bevy_state) gives app, ecs, state, time, math, transform, input, reflect and platform. Input
  kinds (touch, keyboard, gamepad) are opt-in features.
- **Unavailable:** `bevy_render`, `bevy_asset`, `bevy_log`, `bevy_audio`, `multi_threaded`,
  `async_executor`, `std` and `default_app`. Everything is single-threaded.
- **The clock hook (mandatory):** call
  `unsafe { bevy::platform::time::Instant::set_elapsed(fn() -> Duration) }` **before** adding
  `TimePlugin`. `Time<Real>::default()` calls `Instant::now()`, which panics on Xtensa without
  the hook. The firmware feeds it esp-hal's microsecond timer. Tests and the desktop use std
  time; `testing::headless` uses `TimeUpdateStrategy::ManualDuration`, one 60 Hz step per
  `update()`.
- **Float math:** `f32::sin`, `sqrt`, `atan2` and `rem_euclid` are std-only; use
  `bevy::math::ops::*`. `to_degrees`, `abs`, `clamp`, `min` and `max` are fine in core, and
  glam's `Vec2` methods work, backed by libm.
- **Atomics:** the S3 has 32-bit compare-and-swap, and portable-atomic handles 64-bit itself.
  Nothing to configure.
- **Infallible draw results:** write `let Ok(()) = canvas.clear(c);`.
- **No `inventory`-style auto registration:** `.init_array` isn't run.
- **Prior art:** georgik/esp32-conways-game-of-life-rs (bevy_ecs on S3 boards, PSRAM heap),
  OS-Mote/Bevy-ESP32, and bushrat011899/bevy_mod_gba (custom runner and time pattern). Bevy
  0.17.0–0.17.2 panicked on first spawn on 32-bit targets; 0.17.3+ fixes it.

## 6. Rendering budget and rules

What a frame costs, the dirty-rect rules that keep it cheap, and the text-crowding check.

- **Link speed:** 40 MHz × 4 lines = 20 MB/s. A full 466x466 frame is 434,312 B, **about
  22 ms** on the wire, so full-screen animation tops out near 30 fps before render time.
- **A typical Orbit frame:** about 25 KB, roughly 1.3 ms of link time.
- **Frame time on the board** (the firmware logs average and worst every 2 s):
  - nothing to redraw: about 5.4 ms (input polling plus the Bevy update);
  - Angler waiting for a bite or fighting: 6-7 ms at about 11 KB/frame;
  - walking the town: 6-9 ms;
  - a full-screen change: 34-98 ms (the lake scene about 35 ms), a one-frame hitch. Against
    the 22 ms wire time, rendering dominates.
- **Dirty-rect rules:**
  - Static content costs nothing after the first frame.
  - A change repaints its whole bounding box, plus every shape overlapping it.
  - `Shape::bounds()` must be tight and conservative. Arc bounds are computed by hand, since
    embedded-graphics reports an arc's whole circle.
  - Shapes are compared with `PartialEq` every frame. Keep payloads small: refer to big data
    by id, not by value.
- **A shape's draw cost must scale with the band, not the shape.** The presenter draws every
  overlapping shape once per band (about 58 bands for a full screen).
  - embedded-graphics' `Arc` tests every point of its whole circle's bounding box per call (its
    own `PERF` note): about 190k points for the r=212 rim gauge. That made a charging cast
    take 0.3-0.5 s per frame (2-3 fps) on the board.
  - `orbit_gfx::arc` walks only the band's rows inside the arc's bounds, and the annulus spans
    per row: charging takes about 25 us per frame on the host against 2.7 ms, with the same
    ratio on the board. Whole circles match embedded-graphics pixel for pixel.
  - Circles, rects, lines, text and sprites already scale with the band.
- **Bands:** 8 rows × 466 px = 7,456 B, one DMA transfer each. The CO5300 driver keeps two
  7,680 B DMA buffers: it copies the next chunk while the last transmits, and returns with the
  final chunk in flight, so the next band renders in parallel.
- **SPI transaction limit:** 32,736 B per DMA transaction (esp-hal `MAX_DMA_SIZE`).
- **Round glass:** pixels outside radius 233 are invisible. Pure black turns an AMOLED pixel
  off, which is free and saves power.

**Text never crowds** (checked on every test frame):
- **The rule:** `orbit_gfx::crowding` measures the box each label's letters ink (not its font
  cells), then reports:
  - another label within 2 px;
  - any shape drawn over part of it;
  - any shape under part but not all of it, above the last shape that hides it all;
  - letters past the round glass.
- **Exempt:** bands across the whole screen (sky, water, ground) are backdrops, so their
  edges may cross text. So is a label hidden entirely, for example by the GOOD NIGHT screen.
- **Where it runs:** `orbit_core`'s `crowding` feature makes `testing::update` check every
  frame and panic with what crowds what. It's on in every game's dev-dependencies and the
  desktop's snapshots. It allocates, so the firmware, heap32 and QEMU builds leave it off.
  `CROWDING_LOG=<file>` logs findings instead of panicking, for a survey.
- **Scenery behind text** (`orbit_ui`):
  - `plated(shapes, label, backing)` puts a label on a plate of the background color, so a
    path, web, curtain fold or skyline stops short of the letters;
  - `plated_lines` puts lines that belong together on one plate, with nothing in the gaps;
  - `clear_text_of(shapes, margin, decor)` drops drifting decorations near text: bubbles, aim
    dots, Skate's windows;
  - `framed_label` boxes a pointed name where a ring would cut through it;
  - `notice(shapes, y, text, color)` puts a message on its own banner, and text the banner
    would half cover gives way (LEVEL SET, PERFECT!, HOLD TO SLEEP).
- **Moving things stay clear by layout:** tricks move 0.65 of their full size in the house and
  0.7 on the contest stage, so the tallest fish in the tallest hat (TOP HAT) never reaches the
  text. The tank's fish swim inside 150-316 x 170-290, clear of the names round the rim.
- **Coverage:**
  - `testing::monkey(app, secs, seed)` plays at random with the battery level at its widest
    (100%, charging): it turns, leans, presses, holds, taps all over and holds BOOT toward
    sleep.
  - The desktop's snapshots and trailer footage are checked too.
  - Each app's `tests/crowding.rs` runs it with the most crowded data: full tanks of 8-letter
    names, the tallest hat, 999,999 gold, the top stamina level, every trick on both stages
    and every count of stakes.

## 7. Memory: the binding constraint

How much internal SRAM each app takes, why Bevy stays out of PSRAM, and what to do when an
app outgrows the budget.

Measured with `tools/heap32/run.sh`:

| App | After build | During play | Peak | Headroom in 284 KB (with the 7 KB band) |
|---|---|---|---|---|
| Orbit | 205 KB | 213 KB | 265 KB | **12 KB** |
| Angler (the pond) | 200 KB | 208 KB | 209 KB | **67 KB** |
| House (tank, trick school) | 201 KB | 203 KB | 204 KB | **73 KB** |
| Contest hall | 198 KB | 201 KB | 202 KB | **75 KB** |
| Park (walks) | 194 KB | 196 KB | 197 KB | **80 KB** |
| Skate | 190 KB | 194 KB | 194 KB | **82 KB** |
| Bees??? | 189 KB | 192 KB | 193 KB | **84 KB** |
| Town | 190 KB | 192 KB | 193 KB | **84 KB** |
| Spider Roulette | 188 KB | 191 KB | 191 KB | **85 KB** |
| Hat shop | 188 KB | 190 KB | 191 KB | **86 KB** |

- **One app at a time:** Fishing Minigame drops each app before building the next
  (`Board::run` returns it and the caller drops it), so its worst case is its biggest app. The
  QEMU tour checks the internal heap doesn't creep across returns to the town.
- **Not in the table:** the speaker's 8 KB stream buffer, taken from the heap the first time an
  app makes sound, and the mics' 4 KB, the first time one listens.
- **The peak** is transient, from schedule graph builds during the first `update()`. Most of
  the heap is Bevy itself: `App::new()` alone is about 100 KB (64-bit host), `TimePlugin`
  adds about 90 KB, and each schedule, resource table (`Column::with_capacity`) and message
  type adds KB. Reflection is negligible (28 types).

**Keep resources small; box anything big** (measured with heap32):
- Bevy 0.19 keeps each resource in storage that reserves room for many. The arithmetic says
  64: a resource costs about 64 times its size in heap.
- `Mic` holding its 3,248-byte listener inline cost **204 KB**, and so did a plain resource
  holding `[u8; 3248]`. Boxed, `Mic` is 8 bytes plus one 3 KB allocation, and the mic app went
  from 376 KB to 177 KB. `Gps` boxes its sentence buffer the same way.
- Keep resources under about 100 bytes and put big state behind a `Box`, phase enums included.
  A variant holding a voice print made the house's `Home` 328 bytes. Boxed
  (`phase: Box<...Phase>`), the house went from 216 to 204 KB, the contest hall from 206 to
  201 and the park from 202 to 196.

**Phase enums beat Bevy states:**
- Angler is about 56 KB leaner than Orbit at peak because it uses **one phase-machine resource
  and a few systems instead of Bevy states**. New games should follow it.
- `StatesPlugin`, the `StateTransition` schedule and every `OnEnter`/`OnExit` schedule cost KB
  each.
- Small additions, measured: a resource with a phase and no new systems (Angler's old hat
  shop) 3 KB; `Power` 3 KB; the shared sleep/idle system plus the mute preference 3 KB;
  keeping fish (the `Pals` resource and naming phases) 3 KB.
- Opt-in plugins matter: moving buttons and IMU into `ControlsPlugin` saved Orbit 12 KB.

**Why internal SRAM:** on the ESP32-S3, atomic read-modify-write (`S32C1I`) doesn't work on
PSRAM (esp-alloc's own warning), and Bevy keeps `Arc` counts and spinlocks on the heap.

**Firmware layout** (section sizes of the 2026-10-07 builds):

| Region | Size |
|---|---|
| reclaimed heap | 73,744 B |
| `INTERNAL_HEAP_BYTES` | 212 KB |
| internal heap total | **284 KB** |
| `.stack` (whatever DRAM remains) | 67 KB for Fishing Minigame, 69-71 KB for one game |
| PSRAM | 8 MB, registered **last** as a safety net |

At boot the firmware logs whether Bevy spilled into PSRAM. It hasn't, in QEMU (0 B) or on the
board.

**If a game outgrows the budget**, in order of preference:
1. **Test PSRAM atomics on the real board first** (§9). georgik's Espressif demos run bevy_ecs
   entirely from a PSRAM heap on S3 hardware. If RMW atomics on PSRAM work, or at least don't
   hang, the 8 MB is open and this section relaxes.
2. **Trade stack for heap.** Bevy runs on the board with 67 KB of stack. Don't go below about
   48 KB without testing in QEMU.
3. **Trim Bevy:** fewer `OnEnter`/`OnExit` schedules (several KB each), fewer message types
   and resources, or individual crates (`bevy_ecs`, `bevy_app`, `bevy_time`, `bevy_state`,
   `bevy_math`, `bevy_platform`) instead of the umbrella, which drops bevy_reflect.
4. **Keep big plain data off the Bevy heap.** Sprites and audio live in flash
   (`include_bytes!`, zero RAM). Large scratch buffers go in PSRAM explicitly, with
   esp-alloc's `ExternalMemory` allocator (needs `allocator-api2`).

**Flash (storage) is not a constraint** (from `espflash save-image` and the flash logs):
- The factory app partition is 16,384,000 B. The Fishing Minigame image (every app) is
  1,956,544 B (**11.9%**, 2026-10-07), Angler alone 1,562,768 B (9.5%), Orbit 952,480 B
  (5.8%). About 14.4 MB is free.
- Angler's image is about 800 KB of code (`.text`, mostly Bevy) and 696 KB of read-only data:
  sound clips, fonts, sprites and strings.
- Anything that can be `static` (sprites, sounds, level data, text) lives in flash at no RAM
  cost; about 7.7 minutes of 16 kHz audio would still fit. The limits on more game are
  internal SRAM and frame time.

**Other memory facts:**
- esp-alloc allows at most 3 regions and is first-fit in registration order.
- A 20 KB `static` in the LTO'd firmware broke linking ("dangerous relocation: l32r: literal
  target out of range"). Size memory through `heap_allocator!`, not big statics.

## 8. Verifying without a board

The host tests, simulator, heap check and QEMU smoke test, and what each proves.

- **Host tests:**
  - `crates/orbit_game/tests/orbit.rs`: gameplay, headless at a fixed 60 Hz.
  - Every game's tests check crowded text on every frame, and each app's `tests/crowding.rs`
    plays it at random with the most crowded data (section 6).
  - `orbit_gfx` unit tests: arc bounds contain every drawn pixel; a banded render equals a
    full render; **panel RAM always equals a full render of the latest frame** across 40
    frames of motion; writes are even-aligned.
- **Simulator:** the window title shows fps, KB per frame and estimated QSPI ms. `snapshot`
  mode writes PNGs headlessly, even while the Mac is locked, since it doesn't capture the
  screen.
- **`tools/heap32/run.sh`:**
  - builds the game for wasm32, which has 4-byte pointers like Xtensa;
  - counts allocations through the build and a scripted session of each app (the house
    teaches a word through `testing::speak`, which streams like a mic driver and adds nothing
    of its own);
  - compares the peak plus the render band against the budget parsed from
    `firmware/src/lib.rs`, and exits nonzero when over.
- **`tools/qemu-smoke/run.sh`:**
  - **Setup:** downloads Espressif QEMU `esp-develop-9.2.2-20260417` (`-machine esp32s3`;
    needs `brew install libgcrypt`).
  - **Harness:** one binary per game plus the Fishing Minigame tour (`src/bin/*.rs`) over a
    shared `Smoke` harness (`src/lib.rs`): the firmware's heap layout, a byte-counting panel,
    scripted input, `relaunch` (drop the app and build the next, as Fishing Minigame does) and
    `wait(secs)`.
  - **Matching the stack:** it prints both `.stack` sizes. Keep them within about 1 KB by
    adjusting the harness's `INTERNAL_HEAP_BYTES` (238 KB today) when the firmware's DRAM use
    changes.
  - **Pass criteria:**
    - Orbit: two or more starts, one or more losses, 0 B in PSRAM.
    - Angler: a scripted steady hand lands two fish. The first is kept and named QEM by turning
      to each letter; the second is released. The log, settings and tank go through
      `esp-storage` into emulated flash at 0x9000 and read back equal, with 0 B in PSRAM.
    - Spider Roulette: PWR spins repeatedly (each spin rigged to stop on the next egg) until
      the spider pounces from exactly the egg it hid in; the SPDR section reaches flash.
    - Skate: a rider popping just before each obstacle survives 30 s of game time, through the
      halfpipe at 150 m and back out; a cone then bails it, and SKAT reaches flash.
    - Bees???: a steady hand keeps 90%+ of the honey for 20 s, tipping empties the jar, and
      BEES reaches flash.
    - Fishing Minigame: the town walks to each of the eight doors, PWR goes in, the place's way
      out ends it, and the town resumes at that door, twice round. The internal heap after each
      return to the town stays within 2 KB of the first (200 KB first, 202 KB worst over 16
      returns on 2026-10-07).
  - **Script by game time, never frame counts.** Emulated frames are much shorter than real
    ones (about 170 fps of game time, varying run to run). That covers charge-until-power,
    waiting out the result guard and bounding fights by seconds. A frame-based Orbit script
    once got 4.7 s of game time from 2,000 frames (26 s the run before), and the round never
    restarted.
  - **Emulator-only workarounds, applied automatically:**
    1. esp-hal's `bbpll_cal_done` wait is patched out of a private esp-hal copy in
       `.patched/`, because QEMU never sets the bit.
    2. The FPU is enabled by hand, because QEMU boots with CPENABLE=0 (espressif/qemu#154,
       open). Without that, the first hardware-float instruction double-faults.
    3. esp-println uses the `uart` feature, because QEMU doesn't connect USB-JTAG.
  - **Debugging a hang:** run with `-d in_asm,int -D tb.log`, then pass the last translated
    PCs to `xtensa-esp32s3-elf-addr2line -f -C -e <elf>`. QEMU's xtensa interrupt numbers are
    11 = user exception and 12 = double exception.
- **What none of this proves:** the CO5300 init and offset, touch orientation and real timing
  and PSRAM-atomics behaviour. The board covers those.

## 9. Bring-up checks on the board

What to watch in the serial log and on the glass after flashing each binary. Each check ends
with what the logs up to 2026-10-07 show.

**Flashing:** `./flash.sh` from the repo root builds and flashes Fishing Minigame, restarts the
board and shows the serial log (also saved in `logs/`). `./flash.sh angler` flashes Angler
alone, and `./flash.sh orbit` the simplest game. If espflash can't connect, force download
mode: unplug USB and hold PWR 10 s (off), plug USB in, hold BOOT while pressing PWR, flash. The
restart is a watchdog reset whenever the boot latch reads BOOT as held (section 3), so the game
boots.

**Every binary:**
1. **`display ready`, then the title on the glass.** Black screen: try the Arduino init list
   (§3). Shifted image: adjust `COLUMN_OFFSET`.
2. **`bevy app built entirely in internal SRAM`.**
3. **Touch:** in Angler, tapping LOG on the title opens the fish log. If SETTINGS opens, flip
   `TOUCH_MIRROR_X`; if nothing happens, flip `TOUCH_MIRROR_Y`. In Orbit the paddle follows
   the finger. `CST9217 touch init failed`: check this isn't a 1.75C.
4. **The 2 s stats line:** frame count, average and worst frame time, bytes per frame, audio
   restarts, battery, heap.

**Angler** (`./flash.sh angler`):

5. **IMU:** the title has a bubble level (a dot in a ring). Turn the device like a wheel and
   the dot moves sideways; tip the top away and it moves up. BOOT shows "LEVEL SET" and
   recentres it. Wrong axes: fix `imu_to_screen` in `firmware/src/lib.rs`.
   `IMU answered WHO_AM_I` means the wrong address or chip.
6. **Aim:** turning counterclockwise raises the rod (the ANGLE readout goes up). If it's
   backwards, fix the steer sign in `imu_to_screen`, not in the game.
7. **PWR:** holding PWR fills the rim gauge and releasing casts. A 2 s hold must not power
   off (the boot code sets hold-to-off to 10 s). If PWR does nothing, look for
   `PWR key unavailable` (TCA9554 at 0x20).
8. **Reel:** leaning forward pushes the green zone up. If inverted, swap the sign in
   `imu_to_screen`'s y/z.
9. **Saves:** catch and keep a fish, power-cycle, and the title's CAUGHT n and TANK n/3
   persist. The boot log prints `save storage: found a save`.
10. **Battery:** the stats line ends with `battery 76% 3.98 V, USB, charging` (or
    `no cell, USB`), and the title shows the level at the top.
11. **Idle:** untouched for 30 s, the panel dims and the stats line shows about 40 frames per
    2 s (20 Hz). Any touch, button or turn wakes it.
12. **Sleep:** hold BOOT. HOLD TO SLEEP shows at 0.5 s with the rim filling, and at 3 s GOOD
    NIGHT and PRESS TO WAKE replace the frame for 2.5 s. Let go: the log says `switching off;
    press PWR to turn on`, the panel goes dark and the USB serial port disappears. A PWR press
    boots afresh. If the log instead says `still powered a second after switching off` or
    `switching off failed`, the board fell back to deep sleep (`press BOOT to wake`). With
    USB plugged in it stays off and charges.

13. **Audio:** after a catch (its flash save stalls the loop), the stats line may show
    `audio restarted Nx`. Sound must keep working.

**Fishing Minigame** (`./flash.sh`):

14. **The speaker still plays.** I2S0 runs with signal loopback for the mics, which rewires its
    clocks. If the town is silent, turn loopback off in `start_speaker`; the mics go quiet
    instead.
15. **Mics:** in the house, teach a trick a word. The log says `ES7210 mics listening`, and the
    meter along the bottom of the rim rises as you speak. Words that never "take" may need a
    different gain (`GAIN_30_DB` in `firmware/src/mic.rs`) or the other mic slot.

16. **Steps:** the boot log says `LC76G satellite receiver found; sent to backup mode`, then
    `LC76G asleep (backup mode)`. In the park the stats line adds `steps <n>`, which moves with
    every step (any bounce or shake counts too). Five
    seconds into a real walk the log says `dozing: screen off until a button`, the stats line
    drops to about 20 frames per 2 s, and PWR or BOOT brings `awake` and the results.
17. **Saves across apps:** keep a fish at the pond, go home, and it's in the tank. Walk it, and
    the card's stamina grows.

**Experiments still to run**, in order:
- **PSRAM atomics:** allocate an `AtomicU32` (and an `Arc`) in PSRAM via `ExternalMemory`, run
  `fetch_add`/`compare_exchange` a million times, and check the results. That decides §7's
  option 1.
- **TE sync on GPIO13:** see whether waiting for the tearing pulse before large writes removes
  tearing.
- **Audio** (§10.2): the codec answers; confirm the speaker is audible.
- (Real render cost is measured; see section 6.)

## 10. Systems and games

How each system and game works, with the numbers to keep when changing them.

### 10.1 Pixel-art sprites (built: `orbit_gfx::Sprite`)

- **Format:** text rows plus a `(u8, Rgb565)` palette, `.` transparent
  (`crates/angler_game/src/sprites.rs`). Sprites are `static`, so they sit in flash at zero
  RAM, read through the cache.
- **Shapes:** `Shape::Sprite { sprite: SpriteRef(&STATIC), top_left, scale, flip_x }` compares
  by pointer, draws only the rows inside the band, and has a tested tight `bounds()`.
  `Shape::SpriteFit { sprite, area, flip_x }` stretches one to fill `area`, e.g. squashed to
  fake a 3D spin.
- **Bigger art:** a host build step could turn PNGs into raw big-endian RGB565 plus a
  transparent key color, or 8-bit indexed plus a palette, for `include_bytes!`.
- **Chunky look:** author at 155x155 and draw at ×3, or 233x233 at ×2. Output cost is per
  screen pixel either way.
- **Budget:** a static full-screen background is free after the first frame. A moving 32x32
  sprite at ×3 repaints about 2×(96×96) px, about 37 KB or 2 ms of link time. Whole-screen
  animation (scrolling water) costs 22 ms+ per frame, so animate small regions (ripples, the
  bobber) instead.
- **Simulator:** shows sprites exactly, since it uses the same renderer.

### 10.2 Sound

How game sounds reach the speaker, and the ES8311 and esp-hal facts behind it.

**What's built:**
- **Format:** `orbit_audio` clips are mono s16le PCM at `SAMPLE_RATE` = 16 kHz, embedded with
  `include_bytes!` (flash, no RAM). Both platforms run at that rate, so nothing resamples.
- **Mixer:** one voice. `SoundCommand::Play` restarts it and `Stop` silences it, so only the
  last sound queued in an update matters. Mixing costs negligible CPU.
- **Queue bound:** `SoundQueue` keeps only its newest 16 commands when nothing drains it (the
  heap32 and QEMU harnesses have no speaker), so frequent effects can't grow the heap.
- **Restarts:** when the DMA runs out of descriptors it stops for good (esp-hal: "the transfer
  must then be resumed or restarted"). `AudioOut::pump` restarts whenever `is_done()`, and a
  start the driver refuses is retried on the next pump. Restarts and failed starts are in the
  2 s stats line. Don't also require the whole buffer to be free: an underrun landing
  mid-pump then leaves bytes in a dead transfer, and the speaker is silent until reboot with
  nothing logged.
- **Firmware** (`firmware/src/speaker.rs`, adapted from the reference driver):
  - starts only for apps with `SoundPlugin`, after the first `app.update()` (the slow schedule
    build), or the first pump underruns;
  - I2S0 on `DMA_CH1` (CH0 is the display), `Channels::STEREO` (the same sample in L and R),
    MCLK on GPIO42 at 256 × fs, as the ES8311 wants;
  - the codec on the shared I2C bus, initialized after I2S so MCLK is running; GPIO46 high
    enables the amp;
  - the 8 KB stream buffer (128 ms at 16 kHz stereo 16-bit) and its 8 descriptors are
    **leaked from the internal heap** (word-aligned `Box<[u32]>`), not
    `dma_tx_stream_buffer!` statics, so the `.stack` doesn't shrink and games without sound
    (Orbit) don't pay;
  - the boot log prints `ES8311 id 0x83 0x11` when the codec answers.
- **Desktop:** `tinyaudio` 2.0 at 16 kHz mono, fed through an `rtrb` 0.4 ring holding about
  1/15 s. With no device it plays silently. The desktop mic uses `cpal` 0.16; cpal 0.18 and
  tinyaudio hit an `alsa-sys` `links` conflict.

**Hardware path:**
- I2S0 → **ES8311** codec (I2C `0x18`, fixed in hardware) → **NS4150B** mono class-D amp
  (3.3 V, about 0.5 W into 8 Ω, estimated) → `SPK` MX1.25 2-pin connector.
- **The amp is off by default.** GPIO46 enables it: active high, 10k pull-down.
- Waveshare's package contents list an "8Ω 2W speaker ×1" for every variant.
- **Chip ID:** registers `0xFD`/`0xFE` read `0x83`/`0x11`, the first bring-up check.

**ES8311 registers** (esp_codec_dev 1.6.2 `es8311.c` as the BSP uses it: slave, MCLK from the
pin, DAC only; encoded in `init_sequence()` in `docs/reference/es8311_audio.rs`):
- Every MCLK = 256 × fs rate from 8 to 48 kHz uses the same dividers. Only DAC OSR (reg
  `0x04`) changes: `0x20` for ≤16 kHz, `0x10` above.
- Volume: reg `0x32` in 0.5 dB steps, `0xBF` = 0 dB, `0x00` = −95.5 dB.
- Unmute: clear bits 5–6 of reg `0x31`.
- Waveshare's Arduino demo runs 16 kHz at volume `0xE5`, about +19 dB.
- crates.io's `es8311` 0.1.1 (blocking, embedded-hal 1.0) doesn't unmute or set
  volume in `init`.

**esp-hal 1.2 I2S** (from its source; builds against 1.2.2):
- Everything I2S needs esp-hal's `unstable` feature.
- `write_dma_circular` and `write_dma` are **gone** (removed in 1.2.0-rc.0). Streaming is
  `I2sTx::write(DmaTxStreamBuf)`, giving a transfer with `available_bytes()` / `push()` /
  `push_with()`.
- The stream buffer is **not a ring**: if the game stalls long enough to drain it, the transfer
  stops (see Restarts). Pre-fill it before `write`.
- The wiring, from `start_speaker` in `firmware/src/lib.rs` (the receive side is the mics):

```rust
let i2s = I2s::new(pins.i2s, pins.dma,                           // I2S0, DMA_CH1
    TdmConfig::new_tdm_philips()
        .with_sample_rate(Rate::from_hz(SAMPLE_RATE))             // 16 kHz
        .with_data_format(DataFormat::Data16Channel16)
        .with_channels(Channels::STEREO)                          // same sample in L and R
        .with_signal_loopback(true))                              // RX runs on TX's BCLK/WS
    .expect("I2S config")
    .with_mclk(pins.mclk);                                        // GPIO42, 256 x fs
let i2s_rx = i2s.i2s_rx.with_bclk(NoPin).with_ws(NoPin).with_din(pins.din).build(); // DIN 10
// TX pins: BCLK GPIO9, WS GPIO45, DOUT GPIO8
let i2s_tx = i2s.i2s_tx.with_bclk(pins.bclk).with_ws(pins.ws).with_dout(pins.dout).build();
let audio = AudioOut::start(i2s_tx, speaker::stream_buffer(), &mut mixer);
let mut codec = Es8311::new(RefCellDevice::new(bus));
codec.init(SAMPLE_RATE, &mut amp)?;  // after I2S so MCLK runs; raises GPIO46, unmutes, 0 dB
// every frame: audio.pump(&mut mixer), which tops up whatever the DMA has consumed
```

`docs/reference/es8311_audio.rs` is the original researched driver: written for 22.05 kHz
with a static `dma_tx_stream_buffer!` (8 KB out of the stack), and still carrying the restart
bug above.

### 10.3 Input beyond touch (built: buttons and IMU)

How buttons and tilt reach games through `orbit_core::ControlsPlugin`; the chips are in
section 3.
- **Buttons:** `Buttons { action, zero }` are PWR (TCA9554 P4) and BOOT (GPIO0). Their edges
  exist only during an update (§11).
- **Tilt:** `ImuFeed` takes QMI8658 samples in the screen frame; `orbit_motion`'s Mahony filter
  and zeroing turn them into `Tilt { steer_deg, lean_deg }`. The zero button re-levels, and
  `Tilt::request_zero()` re-levels quietly. Good for flick-to-cast or tilt.
- **Bus:** every I2C driver shares it through `embedded-hal-bus` `RefCellDevice`.

### 10.4 Angler, the fishing game and Fishing Minigame's pond (built)

Angler's flow, physics, fish, saves, settings and sounds.

**Flow:** one `Phase` enum resource in `crates/angler_game/src/session.rs`:
- Title → Log | Settings (→ Guten) | Aim → Flight → Waiting → Bite → Reel.
- A fight ends in Caught or Escaped. Caught leads to KEEP (→ Swap if the tank is full →
  Naming) or RELEASE.
- Notices: "ON THE DOCK!", "NOTHING BITES", "REELED IN", "LET GO!", "IN THE TANK!".
- Every result returns to Aim. The MENU button on the aiming screen (shown until the charge
  starts) goes back to the title.
- Landing, choosing and continuing re-level the IMU quietly (`Tilt::request_zero()`), so the
  catch screen starts pointed at KEEP and each cast starts at the base angle however the
  device is held.
- `Update` handles input-driven steps; flight and reel physics run in `FixedUpdate` at 60 Hz.

**Cast** (`cast.rs`, Angry Birds-style side view):
- **Aim:** `elevation = clamp(40 - steer, 10, 80)`; counterclockwise raises the rod. While
  aiming (before and during the charge) a dotted path shows the **full-power** cast for the
  angle, ending on the water exactly where it would land. You aim by putting the end of the
  dots on the fishing spot; a high lob and a low line can both reach it.
- **Power** (the user's rule) only decides whether the cast reaches the dots:
  - while the action button is held, the meter ping-pongs 0→1→0 over `CHARGE_SECS` (1.1 s)
    per sweep;
  - releasing at or above `PERFECT_POWER` (0.95, about 0.1 s around the peak) throws exactly
    full power, landing on the end of the dots and flashing "PERFECT!"; anything lower falls
    short;
  - the gauge marks the window with a gold tip, and its fill turns gold inside it;
  - below 0.08 is a slip; past `MAX_CHARGE_SECS` (4.4 s) the charge cancels.
- **The dots disappear on release,** and the bobber flies alone.
- **Physics:** an exact projectile, `GRAVITY` 420 px/s², `MAX_CAST_SPEED` 310 px/s.
- **Tested:** full power reaches about x 428, every arc at any angle stays inside the glass,
  and the dotted preview equals the stepped flight.
- **The fishing spot** (rippling marks) sits at `spot_x` in 180–410 and moves after each
  catch or escape. `accuracy` (landing distance to the spot) shortens the wait for a bite;
  `distance` (how far out) and accuracy together favor rarer tiers and longer fish.

**Fight** (`reel.rs`, Stardew-style):
- **Layout:** a vertical track (326 px) with a 96 px catch zone.
- **Control:** leaning forward pushes the zone up, with inertia: 2° dead zone, full push at
  22°, 1100 px/s², damping 2.5.
- **Fish movement:** each species' `Temper` (agility spring, max speed, retarget interval,
  darts per second, depth bias).
- **Hooking:** as in Stardew, the fish starts *inside* the zone at the bottom of the bar and
  holds still for `HOOK_SETTLE_SECS` (1 s), then runs for the far half, then moves by its
  temper.
- **Progress:** starts at 0.3, fills at 0.30/s inside the zone, drains at
  0.15 + 0.05 × tier per second outside.
- **Pacing** is pinned by simulated players in the `reel.rs` tests (the first tuning lost
  untouched fish in 1.2–1.9 s, which the user found far too quick):
  - **untouched:** no fish escapes in its first 3 s;
  - **hesitant** (0.25 s late, soft hand): lands tier 0–1 fish ≥ 75% of the time, and
    trophies sometimes but not always;
  - **skilled:** lands every species ≥ 90%;
  - **idle:** lands < 15% overall;
  - **sluggish:** finds pike harder than minnows.

  Retune the constants at the top of `reel.rs` against these tests.

**Fish** (`species.rs`):
- Eight species in tiers 0–3: minnow, bluegill, perch, trout, bass, catfish, pike, golden koi.
- Length is uniform in the species range, skewed long by accuracy. Weight in grams =
  `girth × cm³`, the fisheries length-weight rule.
- `sprites.rs` has the art; the catch screen scales the fish to about 280 px wide.

**Log** (`log.rs`):
- `FishLog { total, species: [{caught, best_length_mm, best_weight_g}; 8] }`, encoded as
  `FLOG | v1 | count | total | 8-byte records`. Saves from builds with fewer species load.
- `Milestones` (new species, longest, heaviest) drive the "NEW SPECIES!" / "NEW RECORD!"
  banners.
- Saved through `SaveBlob` → `orbit_save` (device flash at 0x9000, desktop
  `~/.logic-orbit/angler.flash`).

**Tuning knobs:** every constant is at the top of `cast.rs`, `reel.rs` and `species.rs`. The
tests check behaviour, not values, so retune freely and rerun `cargo test -p angler_game`.

**Keeping a catch** (for Fishing Minigame, 10.6):
- **Catch screen:** KEEP and RELEASE. Level points at KEEP, a turn right past 10 degrees at
  RELEASE; PWR picks, or tap. Below them: "TANK n/3", or "TANK FULL: KEEP SWAPS ONE OUT".
- **Swap** (full tank): the three kept fish and LET IT GO on a four-way dial.
- **Naming:** the letter ring (typing by turning, 10.6). OK with a name makes a `pals::Pet`
  with the catch's species, tier, length and `Catch::size_pct` (where its length falls in the
  species range), plus five tricks rolled by rarity, and saves `PALS`.
- **Catches pay no gold.** Gold and hats live in `pals`; a FLOG v3 save hands its old gold and
  hats over once (`save::legacy_purse`).
- **Art:** the angler is bareheaded; the fish wear the hats (`pals::hats`).

**Home screen and settings:**
- **Title:** LOG and SETTINGS are touch buttons (`ui.rs` holds the rectangles shared by drawing
  and hit tests), plus EXIT when the town launched it. Other taps do nothing; PWR starts
  fishing. It shows "CAUGHT n" and "TANK n/3".
- **Difficulty** (`settings.rs`): EASY / NORMAL / HARD, stepped with < >, each with a
  `Tuning`:

  | Level | Catch zone | Drain | Fish speed | PERFECT from |
  |---|---|---|---|---|
  | EASY | 124 px | ×0.7 | ×0.85 | 0.90 |
  | NORMAL | 96 px | ×1 | ×1 | 0.95 |
  | HARD | 76 px | ×1.3 | ×1.15 | 0.98 |

  Fish speed is a time-warp: spring × s², top speed and dart rate × s, retarget pauses ÷ s.
  NORMAL is what the reel tests pin, and a test checks the same hesitant player lands more on
  EASY and fewer on HARD.
- **Save format** (`save.rs`): Angler's `SaveBlob` section is `FLOG` (other sections belong to
  other apps). Version 4 is the fish log and a difficulty byte. Version 1 saves load at
  NORMAL; version 3 also carried gold and hats, which `legacy_purse` hands to the town. Every
  difficulty change, catch and keep saves, never during a fight.
- **Reset progress** is at the town's signpost (10.6); it erases everything but the
  preferences.

**GUTEN easter egg** (a riff on the user's logic-fish toy), in the settings menu:
- Runs for 15.15 s (`GUTEN_SECS`).
- The rainbow-trout sprite spins once per 2.3 s (logic-fish's speed) via `Shape::SpriteFit`:
  width × |cos θ|, mirrored past 90°.
- A tap (ignored for the first 0.6 s) or PWR stops it early. Either way it queues `Stop` and
  returns to settings.

**Sound effects** (`audio.rs`, synthesized by `assets/sfx.py`, 117 KB of flash):
- CAST whoosh, or PERFECT_CAST (whoosh plus chime), on release.
- PLOP on the water, or THUD on the dock.
- BITE, a double chirp.
- REEL, a click each time the catch meter rises another `REEL_CLICK_STEP` (0.03), so the reel
  is quiet while the fish is out of the zone.
- CATCH jingle, or ESCAPE "wah-wah".
- Typing a name: CLICK per key, THUD for one that can't be typed.
- CLICK for any tap on a button (taps elsewhere are silent).
- With one voice, `play` picks one effect per update (the event's own sound first, else the
  click), and the catch jingle is queued after the final reel click.

**Not built yet:** a reel crank by circling a finger on the rim.

### 10.5 Power: battery, sleep, idle (built; runs on the board)

How every game gets the battery level, idle dimming and hold-BOOT sleep, and what's left for
saving power. Section 9 (checks 10-12) has what the board has shown.

The contract is `orbit_core::Power`, added by `ControlsPlugin` (so Orbit doesn't pay):
- **Battery:** the platform writes `battery: Option<Battery {percent, external_power,
  charging}>`. The firmware reads the AXP2101 every 5 s; the desktop reports USB with no cell.
- **Indicator:** `orbit_ui::battery_level`: an outline filled green/yellow/red, the percent
  (or USB) and a bolt while charging, at the top of every menu. During play it shows only at
  15% or less and not charging (`running_low`).
- **Sleep**, one `ControlsPlugin` system for every game:
  - holding BOOT (the zero button) 0.5 s (`SLEEP_HINT_SECS`) shows HOLD TO SLEEP with the rim
    filling (`orbit_ui::power_overlay`);
  - at 3 s (`SLEEP_HOLD_SECS`) the speaker stops and GOOD NIGHT replaces the frame; games
    ignore input while `Power::saying_good_night()`;
  - after `GOOD_NIGHT_SECS` (2.5 s) `Power::sleep_requested()` is set, and the firmware
    1. waits for BOOT to come up (held at the next power-on, it would start download mode),
    2. drives the amp enable low,
    3. sends touch to sleep and the IMU's sensors off (CTRL7 = 0),
    4. darkens the panel and sends DISPLAY_OFF (0x28) and SLEEP_IN (0x10),
    5. switches the board off through the AXP2101 (REG 0x10 bit 0, section 3);
  - a PWR press powers it on, so it boots afresh into the town (saves are already in flash);
  - **fallback:** if the AXP2101 doesn't answer, or the board is still running a second
    later, it arms GPIO0 on its low-power wake path and enters ESP32 deep sleep, which BOOT
    wakes. Deep sleep alone keeps DCDC1 up, so the codec, mics, panel and GPS keep drawing
    and the battery runs down.
- **Idle:** 30 s (`IDLE_SECS`) without a touch, a button or a 3-degree turn sets `idle`. The
  firmware dims the panel to 0x40 and paces the loop at 20 Hz; the desktop dims and drops to
  20 fps.
- **Doze** (`Power::dozing`, set by a game; the park's real walks):
  - the firmware sends the panel to sleep, touch to sleep, the amp off and the gyro off, and
    stops drawing; leaving, it runs the panel's and touch's `init` again and redraws the frame;
  - it wakes every `DOZE_PERIOD_MS` (100 ms) to read the accelerometer for steps and the
    buttons (PWR has no wake line, so it's polled);
  - on battery it light-sleeps between wakes (`LowPower::sleep_light`, a timer deadline, and
    BOOT listening for a low level); on USB it only halts, so the serial log keeps working.
- **CPU halt:** the spare part of every frame halts the CPU with `waiti` until a systimer
  alarm interrupt (`firmware/src/idle.rs`), instead of spinning in `Delay`. The display and
  speaker DMA keep running. On the board the loop paces 117 frames per 2 s (58.5 fps), where
  the `Delay` spin paced 120.

**Bevy and power** (0.19, no_std): Bevy's power knob is `WinitSettings` (reactive, low-power
updates), part of the winit/render stack this build lacks, and nothing else in
`default_no_std` affects power. The frame loop is ours, so the levers are:
- the loop: halting between frames and the slower idle rate, both built;
- the panel: brightness and dark scenes, since a black AMOLED pixel is off. AMOLED draw scales
  with brightness times lit pixels, and the lake scene is bright sky at 0xFF;
- peripherals: the sleep sequence above, the doze, and the GPS asleep from boot.

**Not done yet**, roughly by value:
- Switching the NS4150B amp off after a second of silence: a few mA at idle, but waking it can
  clip a sound's start.
- A 160 MHz CPU clock: some power saved, but every frame 1.5x longer.

### 10.6 Fishing Minigame: the town and its places (built; runs on the board)

The town, its places, and the rules they share (the loop itself is in the introduction).

**Flow** (`firmware/src/bin/pals.rs` loops):
1. Build the town (`pals_game::town`) and run it until PWR at a door sends `AppExit`.
2. Read `Town::destination()`, drop the town, and build that place's app with
   `orbit_core::Launcher` inserted.
3. When the app sends `AppExit` (EXIT, or leaving an empty room), rebuild the town with
   `pals_game::Arrival(place)`, so you stand at the door you went in by.

One app exists at a time. The desktop's `play` does the same, with `town` the default mode.

| Door | App | Crate |
|---|---|---|
| HOME | the tank, cards, trick school | `pals_game::home` |
| THE POND | Angler (keep or release) | `angler_game` |
| THE PARK | walks | `pals_game::walk` |
| CONTEST HALL | shows | `pals_game::contest` |
| HAT SHOP | hats for the fish | `pals_game::shop` |
| THE DEN | Spider Roulette, with bets | `spider_game` |
| ARCADE | Skate | `skate_game` |
| A HUMMING TREE | Bees??? | `bees_game` |
| SETTINGS (the sign by the house) | inside the town | `pals_game::town` |

**Saves:** one `orbit_save` record holds every app's state as `SaveBlob` sections
(`ORBC | count | (key | len u16 | bytes)*`):
- `PALS`: the tank, gold, owned hats and the mock-walk setting;
- `FLOG`: Angler's log and difficulty;
- `SPDR`, `SKAT`, `BEES`: best scores;
- `PREF`: shared preferences (mute).

A record from before sections loads as the section named by its first four bytes, so old
Angler saves (starting `FLOG`) carry over. The town moves a pre-town save's gold and hats into
`PALS` once (`angler_game::save::legacy_purse` reads FLOG v3). Bees??? reads `PALS` for the
lead fish's hat.

**`PALS`** (`pals::save`, version 2): `version | gold u32 | owned hats u32 | flags | count |
pet*`.
- Flags: bit 0 mock walks, bits 1-3 their pace (an index into `pals::MOCK_PACES`).
- A pet is `species | tier | length u16 | size% | name (length + 8) | hat (0xFF none) |
  walked u32 | 5 x (trick | learned | gesture | has word | word print 193 B?)`.
- A full tank with every trick taught a word is about 3 KB of the 4 KB sector.
- Version 1 held an older 145-byte print that can't be compared with today's. It loads the
  fish, strokes and progress, but those tricks need their words again.

**Controls:** turning selects and steers and PWR confirms, but every PWR action also works by
touch, so a worn button never blocks play; only sleep (holding BOOT) needs a button.
- **Touch for PWR:** a tap where PWR would start (Angler's title, Spider Roulette, Skate,
  Bees???), a finger held down for a held PWR (Angler charges a cast while the finger is down
  and casts when it lifts), and buttons where a tap could mean something else: STOP on a walk,
  DONE/BACK while teaching, BACK beside NEW COMMAND in practice, the contest's tier rows.
- **Taps** (`orbit_ui`):
  - A `Button` takes taps up to `TAP_SLOP` (28 px, about 2.7 mm of glass) outside its box.
  - With more than one button, `tapped(at, buttons)` picks the one the tap lands on, else the
    nearest within reach, so a padded neighbor can't steal it. RENAME, TRICKS and BACK on a
    fish's card are 2 px apart.
- **Dial spots** are picked with `nearest(at, spots, reach)`:

  | Spots | Reach |
  |---|---|
  | Fish on a dial (their names included) | 64 px |
  | Swimming fish | 48 px |
  | Stakes | 48 px |
  | Hats on the ring | 40 px |
  | Keys | 30 px |

- **Dials:** `orbit_ui::dial_choice(steer, n)` picks one of `n` options round the glass from
  12 o'clock, the pointer at 3x the turn (`DIAL_GAIN`).
  `orbit_ui::list_choice(steer, n, start)` walks a list, one item per 12 degrees.
- **Two-way choices:** the pond's KEEP/RELEASE and the reset's KEEP/ERASE are "level or turned
  right".
- **Exceptions:** walking the town is touch. Rotation picks; it doesn't move you around a
  map.

**Typing by turning** (`orbit_ui::keys`):
- A to Z, SPACE, DEL and OK ring the glass, and the turn points at a key (29 keys at 3x gain
  is about 4 degrees of wrist each).
- The pointed key changes only once the pointer is 65% of a key past its middle, so a hand
  held on a boundary doesn't flicker.
- PWR types the key, or tap one. `key_turn(key)` and `key_center(key)` script it in tests.

**The town** (`pals_game::town`):
- **Layout:** six round clearings on a 3 x 2 grid. Top: pond, home, park. Bottom: contest
  hall, main street (hat shop, den), the grove (bee tree, arcade cabinet). Paths leave at the
  compass points where there's a neighbour.
- **Walking is touch** (the user's call; menus still turn):
  - a tap walks you there at 150 px/s;
  - a held finger is the target every frame, so you follow it as it drags;
  - a touch past 170 px out inside a path's gap means "down the path", and the target is
    pushed past the edge;
  - after a path, the same finger is spent until it lifts, or it would walk you straight back
    out.
- **Edges:** solid scenery slides you along it. The rim keeps you in, except down a path to
  the next clearing.
- **Doors:** within 40 px of a door, the plate at the bottom names the place and says PRESS
  TO GO IN (PRESS TO OPEN at the signpost), and PWR enters. Walking in from a landmark tap
  keeps the clearing's plate, so nothing flashes on the way in.
- **Coming out:** back from a place, or out of the settings, nothing goes in for
  `ENTRY_LOCK_SECS` (2 s), and a finger still down from the press that came out doesn't walk
  you until it lifts. So EXIT (whose spot is inside the lake's tap zone) and BACK can't send you
  straight back in.
- **Landmarks:** a tap on a place's landmark (`town::map::landmark`: the house, signpost,
  bench, hall, shop, den, bee tree or cabinet, and the pond's water and dock), give or take
  `TAP_SLOP`, walks you to its door and goes in. A tap at the end of a path still means the
  path. Every door sits below its landmark. A walk in that gets stuck (behind the building,
  say) goes round once: straight out past its nearer side, down that side, then to the door.
  The contest hall stands left of its clearing, off the path from the pond.
- **Settings** (the signpost by the house): SOUND, BRIGHT, MOCK WALK, RESET and BACK on a
  five-spot dial. BRIGHT steps 25, 50, 75 and 100% (`preferences::BRIGHTNESS_LEVELS`, saved in
  `PREF` version 2 as a third byte; version 1 reads as 100%). The platform reads it from the
  `SaveBlob` every frame and sets the panel's 0x51 register (idle dims to a quarter of it); the
  desktop scales its pixels. MOCK
  WALK steps OFF, x1, x2, x5, x10, x25 and back to OFF. RESET asks first; ERASE drops every
  section but `PREF` and keeps the mock-walk setting.

**The pond** is Angler (10.4): KEEP or RELEASE a landed fish; a full tank asks who goes back,
or lets the new catch go.

**Tricks** (`pals::tricks`): 15 in five rarities.

| Rarity | Tricks | Points | Stamina | Lesson |
|---|---|---|---|---|
| Common | SPLASH, BUBBLES, WIGGLE, DIVE | 10 | 5 | 25% |
| Uncommon | SPIN, JUMP, FLIP, FIN WAVE | 20 | 8 | 17% |
| Rare | LOOP, BUBBLE RING, TAIL WALK | 35 | 12 | 12% |
| Epic | CORKSCREW, MOONWALK | 60 | 18 | 9% |
| Legendary | RAINBOW LEAP, WATERSPOUT | 100 | 25 | 6% |

- Each fish rolls five different tricks. The odds blend from (60, 28, 9, 2.5, 0.5) for a
  tier-0 fish to (25, 30, 24, 14, 7) for a trophy.
- **How they look:** `fish::pose` moves and turns the fish over `TRICK_SECS` (1.6 s), and
  `fish::effects` adds the water: SPLASH throws drops and a ring, BUBBLES rise from the mouth,
  BUBBLE RING spreads a ring of bubbles. Both the house and the contest draw them, in the
  `fish::SPRAY` colour, which `clear_text_of` keeps off text. Every trick moves at least 5 px
  on a stage, turns, or throws water (`tests/tricks.rs`).
- A trick is taught when it has a word and/or a stroke.
- A try succeeds with chance 0.25 + 0.75 × learned, always once mastered. A praised success
  adds the lesson; an unpraised one half.

**Triggers:**
- **Strokes** (`pals::gesture`): TAP (still, under 15 px), HOLD (still for 0.6 s), SWIPE
  (60 px or more, any of four ways), CIRCLE (at least 270 degrees over 150 px, either way).
- **Words** (`orbit_audio::voice`): the mic's samples become a `VoicePrint`.
  - Every 16 ms, seven RBJ band-pass filters (300 Hz to 5 kHz, Q 1.4) give the spectrum's
    shape: each band's loudness minus the frame's mean. An eighth feature is the frame's
    loudness minus the word's mean, so the word's rise and fall counts but its volume doesn't.
  - Stretched to 24 frames of 8 bytes, plus the length: 193 bytes.
  - The listener finds speech against the room's floor and cuts the word's edges at 20 dB
    under its own peak (keeping 6 frames before speech for a soft "f"), so a soft word cuts
    like a loud one.
  - A command is the nearest taught print by DTW (a warp band of 9 frames), if it's under 34
    and under 0.85 x the runner-up. The margin stops a word halfway between two taught ones
    picking either.
  - Measured with the tests' perturbations (faster, slower, higher, echoing, soft, noisy): 47
    of 50 variations of a taught word are recognized, 114 of 120 untaught words ignored.
    Claps and whistles never match.
  - Tests use `say`-made speech in `crates/orbit_audio/tests/voices/` (`make.sh` regenerates
    it).

**The house** (`pals_game::home`, with `MicPlugin`):
- **Tank:** fish swim in their hats; the dial points at one, or EXIT.
- **Cards:** species, size, stamina, appeal and the five tricks by rarity, with RENAME, TRICKS
  and BACK on a row.
- **Teaching:** WATCH (the trick, twice), then say a word twice and/or draw a stroke twice. A
  word or stroke another trick answers to is refused. PWR saves the trigger.
  - The second take must sound like the first (`VoicePrint::alike`, 1.3 x the match
    distance), or it's THAT SOUNDED DIFFERENT and the first take stays.
  - The taught word is the two takes blended along their DTW path. A later alike take blends
    in too (EVEN BETTER).
  - Each confident command (under 0.7 x the match distance) moves the taught word 20% toward
    how it was just said (`Pet::hear`), so the fish gets used to your voice without a
    near-miss dragging it off.
- **Practice:** give the command and the fish tries. Pet it (tap, or PWR) within 2.5 s of a
  success to double the lesson. A command for another trick makes it do that one instead.
- **Echo:** words heard within 1 s of the speaker playing are ignored.

**Walks** (`pals_game::walk`, with `StepsPlugin`):
- Stamina level is 1 + √(metres / 100): levels at 100 m, 400 m, 900 m... A step is
  `pals::STRIDE_M` (0.75 m).
- **Counting:** `orbit_motion::StepCounter` counts from the accelerometer (section 3); the walk
  takes `Steps` each frame and throws away any counted before it started.
- **Dozing:** `DOZE_AFTER_SECS` (5 s) after a real walk starts or the glass was last touched,
  it sets `Power::dozing` and draws only a dark screen; the firmware turns the panel off. PWR
  or BOOT (the screen and touch are off) ends the walk; awake, STOP does too. The results show
  steps and metres, and a level-up during the doze shows there.
- **MOCK WALK** steps at 1.9 a second (about 1.4 m/s) times its pace (x1, x2, x5, x10 or x25)
  and never dozes, so - and + beside the distance can change the pace mid-walk; the new pace
  is saved. The desktop walks while W is held.
- Progress saves every 25 m and when the walk ends.

**Contests** (`pals_game::contest`, with `MicPlugin`):

| Tier | Needs | Prizes (1st, 2nd, 3rd) | Rivals score | Appeal |
|---|---|---|---|---|
| BEGINNER | LV 1 | 40, 20, 10 | 40-110 | x1 |
| OPEN | LV 3 | 100, 50, 20 | 130-280 | x1 |
| EXPERT | LV 6 | 220, 110, 45 | 420-800 | x2 |
| MASTER | LV 10 | 500, 250, 100 | 900-1500 | x3 |

- **The show** lasts 30 s:
  - a command makes the fish try (1.6 s);
  - a success scores points × (100 + learned)% × a combo (+10% per different trick in a row,
    up to +50%), doubled if a judge called for it;
  - judges call a taught trick every 4-7 s, for 4 s;
  - each try costs its stamina (from 30 + 10 × level); at 0 the show ends.
- **Final score:** the tricks plus appeal: tier (10, 20, 35, 60) + size / 5 + the hat's
  price / 25 (up to 20).
- Three rivals (SPLASHY, FINLEY, OLD BOOTS) decide the place; the top three pay.

**The hat shop** (`pals_game::shop`):
- NO HAT, the 12 hats and EXIT ring the glass; the mirror shows the fish in the pointed one.
- PWR buys (it goes straight on) or wears an owned hat. Tap the mirror for the next fish.
- `pals_game::fish::HAT_SEATS` seats hats on each species' head column and top row, at 3/4 of
  the fish's scale.
- **Pacing (a test):** the first hat costs a beginner win at most; the crown costs more than an
  expert win, and a master win buys it.

**The den** (Spider Roulette, `crates/spider_game`):
- **Board:** twelve eggs on a web at radius 160; one hides the spider, re-hidden each round. A
  purple spider dances in the middle, bobbing and swaying to the heartbeat and turning round
  every four beats.
- **The bet:** with gold in the purse, a round starts with a bet of 5, 10, 25, 50 or 100 (what
  the purse covers) or FREE, on a dial. Opened from the town, the dial's last spot is EXIT,
  back to the street.
- **The spin:** PWR or a tap spins a pointer `SPIN_TURNS` (3) turns plus the way to an egg
  picked uniformly among the whole ones, with a cubic ease-out over `SPIN_SECS` (2.4 s). A tick
  sounds at each egg it passes; the egg it stops on cracks.
- **Worth:** `cash_value(stake, safe) = stake × 12 × 90% / (12 - safe)`, fair odds less a
  tenth: 1.2x after three safe eggs, 10.8x for trapping the spider. CASH OUT (a tap) banks
  it; the spider takes the stake.
- **Tension:** safe eggs show a shell and shorten the odds at the top ("SAFE n | 1 IN 12" down
  to "1 IN 2"). A heartbeat speeds from 1.25 s to 0.35 s between beats.
- **Endings:** on the spider's egg the dancer pounces at the glass (three bounds over a red
  flash, a screech, then GOT YOU!). Eleven safe eggs trap it. The best run of safe eggs
  persists.

**The arcade cabinet** (Skate, `crates/skate_game`):
- **Controls:** a counterclockwise turn past `POP_DEG` (15) pops an ollie; the next pop needs
  the turn back past `REARM_DEG` (7). In the air the board follows the turn, and landing more
  than `LAND_DEG` (25) off level bails.
- **Physics:** gravity 1500 px/s², pop 560 px/s (about 0.75 s of air), speed 220 to 420 px/s.
- **Obstacles:** cones 18x21, benches 120x36 (land level on top to grind, 25 points/s),
  spawned at random gaps scaled by speed.
- **Score:** metres (30 px each), plus 10 per clean landing, plus grind time. The best
  persists.
- **Halfpipe:** every `HALFPIPE_EVERY_M` (150 m) the street empties (no spawns in the last
  20 m) and drops into a halfpipe, the bottom half of a 175 px circle.
  - The rider swings wall to wall like a pendulum and leaves each coping at `AIR_SPEED`
    (520 px/s, about 0.7 s of air).
  - In the air the spin is the device's turn since takeoff times `SPIN_GAIN` (6), so 60
    degrees of wrist is a 360.
  - Landing within `LAND_SPIN_DEG` (50) of a half turn scores by half turns: AIR 5, 180 15,
    360 35, 540 60, 720 90. Otherwise it's a SLAM: no score, but the run goes on.
  - After `HALFPIPE_SECS` (7) it rolls out onto the street. Distance and speed are frozen
    inside.
- **Playability (a test):** a rider who pops 0-55 px before each obstacle survives 30 s,
  through the halfpipe.
- **Render cost:** only the rider, obstacles and lane dashes move over the static night city.

**The humming tree** (Bees???, `crates/bees_game`):
- **The jar:** the Moon is an open jar orbiting the Earth at radius 150, mouth outward,
  speeding from 45 to 100 degrees/s.
- **Wobble:** an unseen wobble eases toward random targets whose reach grows from 35 to 80
  degrees over 90 s. The device's turn counters it at `CONTROL_GAIN` 2.
- **Spilling:** honey pours once the tilt passes `spill_limit_deg(honey)` (40 degrees full,
  100 nearly empty), at up to 30%/s, and the drops fall to Earth.
- **The bees:** three circle the jar, and one flies a 5% refill up every 3.5 s. They wear the
  lead fish's hat (or none) at scale 2, seated by the hat's head columns (4-8) over the bee's
  head, mirrored when a bee flies left.
- **Score:** the game ends when the jar is empty; the best number of orbits persists.

**Sounds:** each game synthesizes its own with a small `assets/sfx.py` importing Angler's
generator helpers (standard library only, seeded): 61 KB for Spider Roulette, 21 KB for Skate,
42 KB for Bees???. The pals places reuse Angler's and `orbit_ui`'s clips.

## 11. Gotchas

Traps hit or confirmed during bring-up, with where the detail lives.

- **The touch crate's example uses the 1.75C reset pin** (GPIO2); this board uses GPIO40 (§3).
- **"waiting for download" after every flash:** a USB download entry leaves the boot latch
  reading BOOT held, and a plain USB reset keeps it. Restart with a watchdog reset
  (`flash.sh` does); with the battery in, unplugging USB doesn't power-cycle the board (§3).
- **ESP32 deep sleep doesn't make this board sleep:** everything runs off the AXP2101's one
  3.3 V rail, so only the AXP2101 power-off stops the codec, mics, panel and GPS (§10.5).
- **Old I2S answers are wrong for esp-hal 1.2:** `write_dma_circular` is gone (§10.2).
- **esp-generate templates and older answers predate esp-hal 1.1–1.2.** The `psram` feature,
  `ESP_HAL_CONFIG_PSRAM_MODE` and `SpiDmaBus` are gone, and the buffer-taking SPI methods are
  now `*_buffer`.
- **Button and touch edges (`pressed`, `released`, `just_pressed`) exist only during an
  update.** A `Last` system clears them (for `Touch::just_pressed` it copies current to
  previous; platforms only call `set`). Tests must observe edges from a system or through
  their effects, not by reading the resource after `app.update()`.
- **A dropped touch report looks like a new tap.** The firmware handles it (§3), but screens a
  tap dismisses should still ignore taps for a moment (GUTEN: 0.6 s).
- **Bevy states are heap-expensive here.** Prefer a phase enum resource (§7).
- **A big resource costs about 64 times its size in heap.** Box anything over about 100 bytes
  (§7).
- **Measure what the game allocates, not the harness.** A test helper that built one big sample
  buffer (doubling as it grew) once added 90 KB to a heap peak. Stream input like the platform
  does.
- **An underrun I2S stream is dead until restarted.** Restart on `is_done()` alone (§10.2).
- **The mics only hear while the speaker streams:** the receiver runs on the transmitter's
  clocks (signal loopback), and esp-hal stops BCLK/WS when the TX FIFO empties.
- **The LC76G is on the shared I2C bus,** and Quectel reports its `0x54` side locking up on
  busy buses. Reads are spread one step per frame; if they fail, try a slower bus.
- **Holding PWR long switches the board off** (the PMIC, not the firmware). Never design a
  mechanic that holds PWR for more than a few seconds.
- **In QEMU, script by game time, not frame count** (§8).
- **`FlashStorage`'s inherent `read`/`write` shadow the `NorFlash` methods** (§3).
- **embedded-graphics' `Arc::bounding_box()` is the whole circle;** using it would redraw the
  full screen every frame (§6).
- **`cargo new` inside the repo can create a nested `.git`.** Delete it before committing.
- **A device build that logs nothing may still be running.** Check `ESP_LOG` (§4) before
  suspecting a hang; QEMU's `-d in_asm` trace shows where execution is.
- **`cargo clippy` doesn't relink.** Run `cargo build` before reading section sizes with
  `xtensa-esp32s3-elf-size -A`.
- **`Rgb565::BLACK`/`WHITE` need the `RgbColor` trait in scope;** `orbit_gfx` re-exports it.
- **Paddle reach is the half-span plus the ball's angular radius** (about 24.6°). Tests probing
  the edge must stay inside it.
- **Text over scenery needs a plate** (`plated`, or one `plated_lines` for lines that belong
  together; §6). A label on a path, web or skyline reads as overlapping even when nothing
  touches its letters.
- **Level 20 is the top stamina level:** `Pet::next_level_m` is `None` there.
  Subtracting from it underflowed; the random-play test found it on the park's done screen.
- **A clock in integer hundredths drifts:** a 1/60 s frame truncates to one hundredth, and a
  show ran at 60% speed. Keep game time in `f32` seconds.

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
- esp-hal v1.2.2: CHANGELOG, MIGRATING-1.0.0/1.1.0, `qa-test/src/bin/qspi_flash.rs`, esp-alloc
  `macros.rs`
- Bevy v0.19.1 `bevy_platform/src/time/fallback.rs`, `examples/no_std`
- espressif/qemu releases and issue #154
- Mics and GNSS: `docs/reference/gnss_and_mics.md`
