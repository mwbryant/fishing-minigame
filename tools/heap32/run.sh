#!/usr/bin/env bash
# Heap use of a game on a 32-bit target vs the firmware's internal-SRAM budget.
# Usage: tools/heap32/run.sh [orbit|angler]   (default: both)
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release -q --target wasm32-unknown-unknown
for game in ${1:-orbit angler}; do node run.mjs "$game" || status=1; done
exit "${status:-0}"
