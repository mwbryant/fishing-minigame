# logic-orbit

no_std Bevy 0.19 games for the Waveshare ESP32-S3-Touch-AMOLED-1.75 (466x466 round
AMOLED, touch, IMU, two buttons). The board boots the **arcade** (`crates/arcade`), a
rotation dial over **Angler** (fishing: tilt-aimed cast, lean-to-reel fight, fish log,
gold for a hat shop), **Spider Roulette** (PWR spins a pointer round twelve eggs, one hides a spider),
**Skate** (rotation pops and lands ollies; a halfpipe freestyle every 150 m) and **Bees???** (keep the Moon's honey jar from
spilling; the bees wear Angler's hat), plus SETTINGS (mute). **Orbit** (circular pong) is
a separate binary.
A desktop simulator shares the renderer and the motion/save code.

**Read `docs/ESP32_PLATFORM.md` before changing anything.** It has the hardware
pin/protocol reference, the toolchain and crate set, the no_std Bevy rules, the render
and memory budgets, the verification tools, and what is still unverified on real hardware.
`docs/reference/es8311_audio.rs` is the original researched speaker driver; the live
version is `firmware/src/speaker.rs`. Sound clips are 16 kHz mono s16le PCM
(`orbit_audio::SAMPLE_RATE`).
`case/` is the printable enclosure: parametric Python (build123d via uv), separate from the
Rust workspace. Tunables are in `case/params.py`; after changing them run `uv run check.py`,
which fit-tests the parts against Waveshare's STEP model of the board (`case/README.md`).

Rules that bite:
- Every crate under `crates/` stays `#![no_std]`. Use `bevy::math::ops`
  for trig and sqrt, since `f32::sin` is std-only. No bevy_render, bevy_asset, bevy_log or threads.
- `firmware/` and `tools/*` are excluded from the workspace. Build the firmware with
  `cd firmware && . ~/export-esp.sh && cargo build --release` (always release). Binaries:
  `--bin arcade` (the menu and every game) and one per game (`angler spider skate bees orbit`).
- The user flashes with `./flash.sh [game]` (default `arcade`), which saves every serial
  session to `logs/<game>-<timestamp>.log`. When they report a hardware problem, read the
  newest log.
- Turning the device (`Tilt::steer_deg`) is the primary control; PWR confirms, touch is
  secondary. Selections are `orbit_ui` dials (3x gain, so +-60 degrees reaches everything).
- New games: add `CorePlugin` and `ControlsPlugin` (buttons, tilt, `Power`: battery, idle and
  hold-BOOT sleep, all handled in core) plus `SoundPlugin` if it makes sound. Drive the flow
  with a phase enum resource rather than Bevy states (states cost device heap). Draw last
  with `orbit_ui::power_overlay`, and ignore input while `Power::saying_good_night()`.
  Save into your own `SaveBlob` section (four-byte key). Offer HOME (send `AppExit`) when
  `orbit_core::Launcher` is present. Then wire it in: `arcade::Game` and the menu dial,
  `firmware/src/bin/<game>.rs` plus the arcade binary's `launch`,
  `tools/qemu-smoke/src/bin/<game>.rs` (and the arcade smoke), a `measure_<game>` in
  `tools/heap32`, and a desktop `Game` with snapshots.
- The heap budget is tight: Bevy must fit in about 284 KB of internal SRAM, because
  PSRAM atomics are unreliable on the S3. Peak headroom: Orbit 12 KB, Angler 73, Skate 82,
  Bees??? 84, Spider Roulette 90, the menu 95. The arcade drops each app before building the
  next, so only one is ever in memory.
  After adding plugins, systems, resources or states, run `tools/heap32/run.sh`.
  `tools/qemu-smoke/run.sh` checks the full firmware runtime of every game and the arcade's
  app switching (a leak check) in QEMU. QEMU scripts run on game time, never frame counts.
- Shapes are diffed with `PartialEq` every frame and must report tight `bounds()`. The
  presenter draws every overlapping shape once per 8-row band, so a shape's draw cost must
  scale with the band: never draw embedded-graphics `Arc`s directly (they test their whole
  circle per call); `Shape::Arc` uses `orbit_gfx::arc`.
- Workflow: red-green TDD with behavioral test names, then `cargo clippy` + `cargo fmt`,
  and a commit per feature. UI strings are ASCII only.
