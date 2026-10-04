#!/usr/bin/env bash
# Heap use of the game on a 32-bit target vs the firmware's internal-SRAM budget.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release -q --target wasm32-unknown-unknown
node run.mjs
