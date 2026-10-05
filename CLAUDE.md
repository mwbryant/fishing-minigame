# logic-orbit

no_std Bevy 0.19 games for the Waveshare ESP32-S3-Touch-AMOLED-1.75 (466x466 round
AMOLED, touch, IMU, two buttons): **Orbit** (circular pong) and **Angler** (fishing:
tilt-aimed hold-to-charge cast, lean-to-reel Stardew-style fight, persistent fish log).
A desktop simulator shares the renderer and the motion/save code.

**Read `docs/ESP32_PLATFORM.md` before changing anything.** It has the hardware
pin/protocol reference, the toolchain and crate set, the no_std Bevy rules, the render
and memory budgets, the verification tools, and what is still unverified on real hardware.
`docs/reference/es8311_audio.rs` is the original researched speaker driver; the live
version is `firmware/src/speaker.rs`. Sound clips are 16 kHz mono s16le PCM
(`orbit_audio::SAMPLE_RATE`).

Rules that bite:
- Every crate under `crates/` stays `#![no_std]`. Use `bevy::math::ops`
  for trig and sqrt, since `f32::sin` is std-only. No bevy_render, bevy_asset, bevy_log or threads.
- `firmware/` and `tools/*` are excluded from the workspace. Build the firmware with
  `cd firmware && . ~/export-esp.sh && cargo build --release` (always release). There is one
  binary per game: `--bin angler` or `--bin orbit`.
- New games: add `CorePlugin` (plus `ControlsPlugin` for buttons/tilt and `SoundPlugin` for
  the speaker, only if used), drive
  the flow with a phase enum resource rather than Bevy states (states cost device heap), and
  add a `firmware/src/bin/<game>.rs` plus a `tools/qemu-smoke/src/bin/<game>.rs`.
- The heap budget is tight: Bevy must fit in about 284 KB of internal SRAM, because
  PSRAM atomics are unreliable on the S3. Peak headroom is Orbit 11 KB and Angler 87 KB.
  After adding plugins, systems, resources or states, run `tools/heap32/run.sh`.
  `tools/qemu-smoke/run.sh` checks the full firmware runtime of both games in QEMU.
- Shapes are diffed with `PartialEq` every frame and must report tight `bounds()`.
- Workflow: red-green TDD with behavioral test names, then `cargo clippy` + `cargo fmt`,
  and a commit per feature. UI strings are ASCII only.
