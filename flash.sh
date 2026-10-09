#!/usr/bin/env bash
# Builds a game's firmware, flashes it to the board over USB-C and shows the
# board's serial log. Only the bootloader, partition table and app are
# written, so saves (0x9000/0xA000) survive. Every session's log is kept in logs/.
#
#   ./flash.sh [game]     build + flash (Fishing Minigame by default; or angler, spider, skate, bees, orbit), then the log
#   ./flash.sh monitor [game]     only reopen the log (Ctrl+R restarts the board)
#
# Several boards plugged in: ESPFLASH_PORT=/dev/cu.usbmodemXXXX ./flash.sh
set -euo pipefail

repo="$(cd "$(dirname "$0")" && pwd)"

mode=flash
if [ "${1:-}" = monitor ]; then
    mode=monitor
    shift
fi
game="${1:-pals}"
if [ ! -f "$repo/firmware/src/bin/$game.rs" ]; then
    games="$(cd "$repo/firmware/src/bin" && ls -- *.rs | sed 's/\.rs$//' | tr '\n' ' ')"
    echo "unknown game '$game'; pick one of: $games" >&2
    exit 2
fi

if [ ! -f ~/export-esp.sh ] || ! command -v espflash > /dev/null; then
    echo "The ESP32 toolchain isn't installed. One-time setup:" >&2
    echo "  cargo install espup espflash && espup install --targets esp32s3" >&2
    exit 1
fi
# shellcheck disable=SC1090
. ~/export-esp.sh

elf="$repo/firmware/target/xtensa-esp32s3-none-elf/release/$game"
if [ "$mode" = flash ]; then
    (cd "$repo/firmware" && cargo build --release --bin "$game")
    echo "flashing $game; saves on the board are kept"
    espflash=(espflash flash --chip esp32s3 --monitor "$elf")
elif [ -f "$elf" ]; then
    espflash=(espflash monitor --chip esp32s3 --elf "$elf")
else
    echo "$game isn't built yet; run ./flash.sh $game first" >&2
    exit 1
fi

mkdir -p "$repo/logs"
log="logs/$game-$(date +%Y%m%d-%H%M%S).log"
if ! script -q "$repo/$log" "${espflash[@]}"; then
    cat >&2 <<EOF

espflash stopped with an error (log: $log). Things to try:
  - Use a USB-C cable that carries data; many charge-only cables don't.
  - Press PWR once in case the board is switched off.
  - Force download mode: unplug, hold BOOT (the lower button), plug back in,
    release BOOT, then run this again. Afterwards unplug and replug so it boots.
  - Several boards connected: ESPFLASH_PORT=/dev/cu.usbmodemXXXX ./flash.sh
EOF
    exit 1
fi
echo "log saved to $log; first time on this board? see README.md's bring-up checklist"
