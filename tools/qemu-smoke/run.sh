#!/usr/bin/env bash
# Boots the firmware's runtime (heap layout, Bevy, game, presenter) in
# Espressif's QEMU and waits for SMOKE PASS / SMOKE FAIL. No board needed.
# Usage: tools/qemu-smoke/run.sh [timeout_secs]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
firmware="$here/../../firmware"
timeout_secs="${1:-180}"
qemu_tag="esp-develop-9.2.2-20260417"
qemu_tar="qemu-xtensa-softmmu-esp_develop_9.2.2_20260417-aarch64-apple-darwin.tar.xz"
qemu="$here/.qemu/qemu/bin/qemu-system-xtensa"

# shellcheck disable=SC1090
. ~/export-esp.sh

if [ ! -x "$qemu" ]; then
    echo "downloading Espressif QEMU $qemu_tag"
    mkdir -p "$here/.qemu"
    curl -fsSL "https://github.com/espressif/qemu/releases/download/$qemu_tag/$qemu_tar" \
        | tar -xJ -C "$here/.qemu"
fi
if ! "$qemu" --version > /dev/null 2>&1; then
    echo "QEMU won't start; it needs Homebrew's libgcrypt: brew install libgcrypt" >&2
    exit 1
fi

# Copy the esp-hal the firmware resolves to and delete QEMU's missing
# BBPLL-calibration wait.
(cd "$firmware" && cargo fetch -q)
hal_version="$(grep -A1 '^name = "esp-hal"$' "$firmware/Cargo.lock" | sed -n 's/^version = "\(.*\)"$/\1/p')"
if [ ! -f "$here/.patched/esp-hal/.version-$hal_version" ]; then
    hal_src="$(ls -d ~/.cargo/registry/src/*/esp-hal-"$hal_version" | head -1)"
    rm -rf "$here/.patched/esp-hal"
    mkdir -p "$here/.patched"
    cp -R "$hal_src" "$here/.patched/esp-hal"
    python3 - "$here/.patched/esp-hal/src/soc/esp32s3/clocks.rs" <<'EOF'
import re, sys
path = sys.argv[1]
src = open(path).read()
patched, n = re.subn(r"while I2C_ANA_MST::regs\(\)\s*\.ana_conf0\(\)\s*\.read\(\)\s*\.bbpll_cal_done\(\)\s*\.bit_is_clear\(\)\s*\{\}", "", src)
if n != 1:
    sys.exit("could not find the bbpll_cal_done wait loop to patch in " + path)
open(path, "w").write(patched)
EOF
    touch "$here/.patched/esp-hal/.version-$hal_version"
fi

cd "$here"
cargo build --release -q
elf="target/xtensa-esp32s3-none-elf/release/orbit_qemu_smoke"
fw_elf="$firmware/target/xtensa-esp32s3-none-elf/release/orbit_firmware"
if [ -f "$fw_elf" ]; then
    echo "stack: firmware $(xtensa-esp32s3-elf-size -A "$fw_elf" | awk '/^\.stack/ {print $2}') B, smoke $(xtensa-esp32s3-elf-size -A "$elf" | awk '/^\.stack/ {print $2}') B"
fi
espflash save-image --chip esp32s3 --merge --flash-size 16mb "$elf" flash.bin > /dev/null

rm -f smoke.log
"$qemu" -nographic -machine esp32s3 -m 8M \
    -drive file=flash.bin,if=mtd,format=raw -serial file:smoke.log > /dev/null 2>&1 &
qemu_pid=$!
trap 'kill -9 "$qemu_pid" 2> /dev/null || true' EXIT

for _ in $(seq "$timeout_secs"); do
    if grep -aq "SMOKE PASS\|SMOKE FAIL\|PANIC\|Exception" smoke.log 2> /dev/null; then
        break
    fi
    sleep 1
done
grep -aE "INFO|WARN|ERROR|PANIC|panicked|Exception|^(Internal|External) " smoke.log \
    | grep -avE "INFO - (PSRAM|vendor id|dev id|density|good-die|Latency|Readlatency|VCC|SRF|Burst|DriveStrength|chip id|size is)" || true
grep -aq "SMOKE PASS" smoke.log || { echo "smoke test did not pass (log: $here/smoke.log)" >&2; exit 1; }
