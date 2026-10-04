# logic-orbit

no_std Bevy 0.19 game for the Waveshare ESP32-S3-Touch-AMOLED-1.75 (466x466 round
AMOLED + touch), with a desktop simulator that shares the renderer.

**Read `docs/ESP32_PLATFORM.md` before changing anything.** It has the hardware
pin/protocol reference, the toolchain and crate set, the no_std Bevy rules, the render
and memory budgets, the verification tools, and what is still unverified on real hardware.
`docs/reference/` holds researched, compile-checked driver code that is not yet wired in
(ES8311 speaker audio).

Rules that bite:
- `crates/orbit_game` and `crates/orbit_gfx` stay `#![no_std]`. Use `bevy::math::ops`
  for trig and sqrt, since `f32::sin` is std-only. No bevy_render, bevy_asset, bevy_log or threads.
- `firmware/` and `tools/*` are excluded from the workspace. Build the firmware with
  `cd firmware && . ~/export-esp.sh && cargo build --release` (always release).
- The heap budget is tight: Bevy must fit in about 284 KB of internal SRAM, because
  PSRAM atomics are unreliable on the S3. After adding plugins, systems, resources or
  states, run `tools/heap32/run.sh`; `tools/qemu-smoke/run.sh` checks the full firmware
  runtime in QEMU.
- Shapes are diffed with `PartialEq` every frame and must report tight `bounds()`.
- Workflow: red-green TDD with behavioral test names, then `cargo clippy` + `cargo fmt`,
  and a commit per feature. UI strings are ASCII only.
