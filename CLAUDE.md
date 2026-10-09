# logic-orbit

no_std Bevy 0.19 games for the Waveshare ESP32-S3-Touch-AMOLED-1.75 (466x466 round AMOLED,
touch, IMU, two buttons; the user's is the -G variant with an LC76G GPS, and it has two
ES7210 mics). The board boots **Fishing Minigame**, a Nintendogs-style loop. It was called Fish Pals,
and the code keeps that name (`pals`, `pals_game`, `--bin pals`, the `PALS` save key, so saves
survive); only what people read says Fishing Minigame:
- a **town** (`pals_game::town`) you walk around by touch (menus still turn);
- the **pond** (Angler: catch, then KEEP and name by turning, up to three);
- the **house** (the tank, cards, trick school by voice and touch strokes);
- the **park** (GPS walks for stamina, or MOCK WALK);
- the **contest hall** (live trick shows for gold);
- the **hat shop** (hats for the fish);
- the **den** (Spider Roulette with bets);
- an **arcade cabinet** (Skate) and a **humming tree** (Bees???).

The shared state and rules are in `crates/pals` (the `PALS` save section). **Orbit**
(circular pong) is a separate binary.
A desktop simulator shares the renderer and the motion/save code.

**Read `docs/ESP32_PLATFORM.md` before changing anything.** It has the hardware
pin/protocol reference, the toolchain and crate set, the no_std Bevy rules, the render
and memory budgets, the verification tools, and what is still unverified on real hardware.
`docs/reference/es8311_audio.rs` is the original researched speaker driver; the live
version is `firmware/src/speaker.rs`. `docs/reference/gnss_and_mics.md` is the research
behind `firmware/src/mic.rs` (ES7210, sharing the speaker's I2S clocks) and
`firmware/src/lc76g.rs` (the GPS, over I2C); both are unverified on the board. Sound clips are 16 kHz mono s16le PCM
(`orbit_audio::SAMPLE_RATE`).
`case/` is the printable enclosure: parametric Python (build123d via uv), separate from the
Rust workspace. Tunables are in `case/params.py`; after changing them run `uv run check.py`,
which fit-tests the parts against Waveshare's STEP model of the board (`case/README.md`).
`docs/architecture/build.py` draws the 1080p slides of all this for posts and devlogs (the PNGs
beside it, and the same boards on a Design canvas). Their facts are written in by hand, so when a
change makes a slide wrong (a crate, a place, a chip, a number), fix it there and re-render.

Rules that bite:
- Every crate under `crates/` stays `#![no_std]`. Use `bevy::math::ops`
  for trig and sqrt, since `f32::sin` is std-only. No bevy_render, bevy_asset, bevy_log or threads.
- `firmware/` and `tools/*` are excluded from the workspace. Build the firmware with
  `cd firmware && . ~/export-esp.sh && cargo build --release` (always release). Binaries:
  `--bin pals` (Fishing Minigame: the town and every place) and one per game
  (`angler spider skate bees orbit`).
- The user flashes with `./flash.sh [game]` (default `pals`), which saves every serial
  session to `logs/<game>-<timestamp>.log`. When they report a hardware problem, read the
  newest log.
- Turning the device (`Tilt::steer_deg`) is the primary control; the upper button (PWR on
  the board) confirms, touch is secondary. Selections are `orbit_ui` dials (3x gain, so
  +-60 degrees reaches everything).
- New games and places:
  - **Plugins:** add `CorePlugin` and `ControlsPlugin` (buttons, tilt, `Power`: battery, idle
    and hold-BOOT sleep, all handled in core), plus `SoundPlugin` if it makes sound,
    `MicPlugin` if it listens, `GpsPlugin` if it walks.
  - **Flow:** drive it with a phase enum resource rather than Bevy states (states cost
    device heap).
  - **Drawing and input:** draw last with `orbit_ui::power_overlay`, and ignore input while
    `Power::saying_good_night()`.
  - **Saves:** use your own `SaveBlob` section (four-byte key). The fish, gold and hats are
    `pals::save` (`PALS`).
  - **Leaving:** a game offers OUT (send `AppExit`) when `orbit_core::Launcher` is present; a
    Fishing Minigame place exits on its own way out.
  - **Wiring it in:**
    - a `pals_game::Place`, with a door in `town::map` and the app in `firmware/src/bin/pals.rs`;
    - `firmware/src/bin/<game>.rs` if it's also standalone;
    - `tools/qemu-smoke` (its own bin, and the pals tour's `launch` and `leave`);
    - a `measure_<app>` in `tools/heap32`;
    - a desktop `Game` with snapshots.
- The heap budget is tight: Bevy must fit in about 284 KB of internal SRAM, because PSRAM
  atomics are unreliable on the S3.
  - Peak headroom: Orbit 12 KB, Angler 67, the house 73, the contest 75, the park 80,
    Skate 82, Bees??? 84, the town 84, Spider Roulette 85, the shop 86.
  - Fishing Minigame drops each app before building the next, so only one is ever in memory.
  - **A resource costs about 64 times its size in heap**: a 3 KB resource cost 204 KB. Keep
    resources under about 100 bytes and box anything bigger, phase enums included.
  After adding plugins, systems, resources or states, run `tools/heap32/run.sh`.
  `tools/qemu-smoke/run.sh` checks the full firmware runtime of every game and Fishing Minigame's
  app switching (a leak check) in QEMU. QEMU scripts run on game time, never frame counts.
- No text ever crowds: no other text within 2 px, nothing over or half under it, nothing
  past the glass.
  - **Checked automatically:** every game test frame is checked (`orbit_gfx::crowding`, via
    `orbit_core`'s `crowding` feature in each game's dev-dependencies).
  - **Fixes:** put text over scenery on `orbit_ui::plated`/`plated_lines`, drop drifting
    decor with `clear_text_of`, and lay out moving things to stay clear.
  - **New apps:** add a `tests/crowding.rs` that runs `testing::monkey` with the most
    crowded data.
- Taps: a `Button` takes taps within `TAP_SLOP` (16 px) of its box; a screen with more than
  one button picks with `orbit_ui::tapped(at, buttons)` (nearest wins), and dial spots with
  `orbit_ui::nearest`.
- Shapes are diffed with `PartialEq` every frame and must report tight `bounds()`. The
  presenter draws every overlapping shape once per 8-row band, so a shape's draw cost must
  scale with the band: never draw embedded-graphics `Arc`s directly (they test their whole
  circle per call); `Shape::Arc` uses `orbit_gfx::arc`.
- Workflow: red-green TDD with behavioral test names, then `cargo clippy` + `cargo fmt`,
  and a commit per feature. UI strings are ASCII only, and the screens say PRESS, never PWR
  (`crates/orbit_ui/tests/wording.rs` checks).
