# logic-orbit

no_std Bevy 0.19 games for the Waveshare ESP32-S3-Touch-AMOLED-1.75: a 466x466 round AMOLED
with touch, an IMU, two buttons and two ES7210 mics. The user's board is the -G variant, with an
LC76G GPS.

The board boots **Fishing Minigame**, a Nintendogs-style loop. It was called Fish Pals, and the
code keeps that name (`pals`, `pals_game`, `--bin pals`, the `PALS` save key) so saves survive.
Only text people read says Fishing Minigame.

- **town** (`pals_game::town`): walk by touch; menus still turn.
- **pond**: Angler. Catch, then KEEP and name by turning, up to three.
- **house**: the tank, cards, and trick school by voice and touch strokes.
- **park**: GPS walks for stamina, or MOCK WALK.
- **contest hall**: live trick shows for gold.
- **hat shop**: hats for the fish.
- **den**: Spider Roulette with bets.
- **arcade cabinet** (Skate) and **humming tree** (Bees???).

Shared state and rules live in `crates/pals` (the `PALS` save section). **Orbit** (circular
pong) is a separate binary. A desktop simulator shares the renderer and the motion and save code.

## Where to look

- **`docs/ESP32_PLATFORM.md`: read it before changing anything.** Pins and protocols, the
  toolchain, the no_std Bevy rules, render and memory budgets, and the verification tools.
- `docs/reference/es8311_audio.rs`: the original speaker driver research. The live one is
  `firmware/src/speaker.rs`.
- `docs/reference/gnss_and_mics.md`: the research behind `firmware/src/mic.rs` (ES7210, on the
  speaker's I2S clocks) and `firmware/src/lc76g.rs` (GPS over I2C).
- Sound clips are 16 kHz mono s16le PCM (`orbit_audio::SAMPLE_RATE`).
- `case/`: the printable enclosure, parametric Python (build123d via uv), outside the Rust
  workspace. Tunables are in `case/params.py`. After changing them run `uv run check.py`, which
  fit-tests the parts against Waveshare's STEP model (`case/README.md`).
- `docs/architecture/build.py`: the 1080p slides for posts and devlogs (the PNGs beside it, and
  the same boards on a Design canvas). Their facts are typed in by hand, so when a change makes a
  slide wrong (a crate, a place, a chip, a number), fix it there and re-render.

## Rules that bite

- **no_std:** every crate under `crates/` stays `#![no_std]`. Use `bevy::math::ops` for trig and
  sqrt (`f32::sin` is std-only). No bevy_render, bevy_asset, bevy_log or threads.
- **Firmware:** `firmware/` and `tools/*` are outside the workspace. Build with
  `cd firmware && . ~/export-esp.sh && cargo build --release` (always release). Binaries:
  `--bin pals` (Fishing Minigame: the town and every place) and one per game
  (`angler spider skate bees orbit`).
- **Hardware problems:** the user flashes with `./flash.sh [game]` (default `pals`), which saves
  every serial session to `logs/<game>-<timestamp>.log`. When they report a problem, read the
  newest log.
- **Controls:** turning the device (`Tilt::steer_deg`) is the main control. The upper button (PWR
  on the board) confirms; touch is secondary. Selections are `orbit_ui` dials (3x gain, so ±60
  degrees reaches everything).
- **New games and places:**
  - **Plugins:** `CorePlugin` and `ControlsPlugin` (buttons, tilt, and `Power`: battery, idle and
    hold-BOOT sleep, all handled in core), plus `SoundPlugin` for sound, `MicPlugin` to listen and
    `GpsPlugin` to walk.
  - **Flow:** a phase enum resource, not Bevy states (states cost device heap).
  - **Drawing and input:** draw `orbit_ui::power_overlay` last, and ignore input while
    `Power::saying_good_night()`.
  - **Saves:** your own `SaveBlob` section (four-byte key). The fish, gold and hats are
    `pals::save` (`PALS`).
  - **Leaving:** offer OUT (send `AppExit`) when `orbit_core::Launcher` is present. A Fishing
    Minigame place exits on its own way out.
  - **Wiring it in:**
    - a `pals_game::Place`, with a door in `town::map` and the app in `firmware/src/bin/pals.rs`;
    - `firmware/src/bin/<game>.rs` if it's also standalone;
    - `tools/qemu-smoke` (its own bin, and the pals tour's `launch` and `leave`);
    - a `measure_<app>` in `tools/heap32`;
    - a desktop `Game` with snapshots.
- **Heap:** Bevy must fit in about 284 KB of internal SRAM, because PSRAM atomics are unreliable
  on the S3.
  - Peak headroom: Orbit 12 KB, Angler 67, the house 73, the contest 75, the park 80, Skate 82,
    Bees??? 84, the town 84, Spider Roulette 85, the shop 86.
  - Only one app is ever in memory: Fishing Minigame drops each one before building the next.
  - **A resource costs about 64 times its size in heap** (a 3 KB resource cost 204 KB). Keep
    resources under about 100 bytes and box anything bigger, phase enums included.
  - After adding plugins, systems, resources or states, run `tools/heap32/run.sh`.
    `tools/qemu-smoke/run.sh` runs every game's firmware and Fishing Minigame's app switching (a
    leak check) in QEMU. QEMU scripts run on game time, never frame counts.
- **Text never crowds:** no other text within 2 px, nothing over or half under it, nothing past
  the glass.
  - Every game test frame is checked (`orbit_gfx::crowding`, via `orbit_core`'s `crowding`
    feature in each game's dev-dependencies).
  - Fixes: put text over scenery on `orbit_ui::plated`/`plated_lines`, drop drifting decor with
    `clear_text_of`, and lay out moving things to stay clear.
  - New apps add a `tests/crowding.rs` that runs `testing::monkey` with the most crowded data.
- **Taps:** a `Button` takes taps within `TAP_SLOP` (16 px) of its box. A screen with several
  buttons picks with `orbit_ui::tapped(at, buttons)` (nearest wins), and dial spots with
  `orbit_ui::nearest`.
- **Shapes:** they're diffed with `PartialEq` every frame and must report tight `bounds()`. The
  presenter draws every overlapping shape once per 8-row band, so draw cost must scale with the
  band. Never draw embedded-graphics `Arc`s directly (they test their whole circle per call);
  `Shape::Arc` uses `orbit_gfx::arc`.
- **Workflow:** red-green TDD with behavioral test names, then `cargo clippy` and `cargo fmt`,
  and a commit per feature. UI strings are ASCII only, and screens say PRESS, never PWR
  (`crates/orbit_ui/tests/wording.rs` checks).
