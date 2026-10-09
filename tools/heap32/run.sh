#!/usr/bin/env bash
# Measures each app's heap on a 32-bit target (wasm32) against the firmware's
# internal-SRAM budget. Exits nonzero if any app is over.
# Usage: tools/heap32/run.sh [app]   (default: all)
#   app: orbit angler town home park contest shop spider skate bees
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release -q --target wasm32-unknown-unknown
for game in ${1:-orbit angler town home park contest shop spider skate bees}; do node run.mjs "$game" || status=1; done
exit "${status:-0}"
