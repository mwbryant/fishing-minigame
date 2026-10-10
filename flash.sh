#!/usr/bin/env bash
# Builds a game's firmware, flashes it to the board over USB-C and shows the
# serial log. Only the bootloader, partition table and app are written, so
# saves (0x9000/0xA000) survive. Every session's log is kept in logs/.
#
#   ./flash.sh [game]           build, flash, restart the board, then the log
#   ./flash.sh monitor [game]   restart the board and show its log
#
# game: pals (Fishing Minigame, the default), angler, spider, skate, bees, orbit
# Several boards plugged in: ESPFLASH_PORT=/dev/cu.usbmodemXXXX ./flash.sh
#
# The log is a separate non-interactive `espflash monitor` because only that
# path restarts the board with a watchdog reset when needed. On this board,
# entering download mode over USB leaves the ESP32-S3's boot latch reading
# BOOT (GPIO0) as held (GPIO_STRAP 0x23 with GPIO0 high), and espflash's normal
# reset keeps the latch, so a flash would end in "waiting for download" until
# the board loses power, which the battery prevents. The watchdog reset reads
# BOOT again. It drops the first 1.5 s of log; Ctrl+R then shows a full boot.
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
elif [ ! -f "$elf" ]; then
    echo "$game isn't built yet; run ./flash.sh $game first" >&2
    exit 1
fi

mkdir -p "$repo/logs"
log="logs/$game-$(date +%Y%m%d-%H%M%S).log"

failed() {
    cat >&2 <<EOF

espflash stopped with an error (log: $log). Things to try:
  - Use a USB-C cable that carries data; many charge-only cables don't.
  - Press PWR once in case the board is switched off.
  - Restart it fully: unplug USB, hold PWR for 10 s to turn it off, press PWR to
    turn it on, plug USB back in. With the battery connected, unplugging USB
    alone keeps it powered.
  - Force download mode: turn it off as above, plug USB in, then hold BOOT (the
    lower button) while pressing PWR, release both, and run this again.
  - Several boards connected: ESPFLASH_PORT=/dev/cu.usbmodemXXXX ./flash.sh
EOF
    exit 1
}

if [ "$mode" = flash ]; then
    echo "flashing $game; saves on the board are kept"
    # espflash warns that the ELF it passes itself is an unused monitor option.
    RUST_LOG="info,espflash::cli::monitor=error" \
        script -q "$repo/$log" espflash flash --chip esp32s3 "$elf" || failed
fi
echo "restarting the board; Ctrl+C quits, Ctrl+R restarts it with the full boot log"
script -aq "$repo/$log" espflash monitor --chip esp32s3 --non-interactive --elf "$elf" || failed
echo "log saved to $log; first time on this board? see README.md's bring-up checklist"
