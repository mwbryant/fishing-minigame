#!/usr/bin/env bash
# Regenerates the spoken-word fixtures for the voice matcher tests (macOS only:
# `say` and `afconvert`). Two voices say each trick word at a normal and a fast
# rate (170 and 240 wpm), saved as 16 kHz mono s16le PCM, the board's mic format.
set -euo pipefail
cd "$(dirname "$0")"
tmp=$(mktemp -d)
for voice in Daniel Fred; do
  for word in spin jump splash bubbles flip; do
    for rate in 170 240; do
      name="$(echo "$voice" | tr 'A-Z' 'a-z')-$word-$rate"
      say -v "$voice" -r "$rate" -o "$tmp/$name.aiff" "$word"
      afconvert -f WAVE -d LEI16@16000 -c 1 "$tmp/$name.aiff" "$tmp/$name.wav"
      # Keep only the WAV's data chunk, walking past any others.
      python3 - "$tmp/$name.wav" "$name.pcm" <<'PY'
import struct, sys
data = open(sys.argv[1], 'rb').read()
at = 12
while at < len(data):
    tag, size = data[at:at + 4], struct.unpack('<I', data[at + 4:at + 8])[0]
    if tag == b'data':
        open(sys.argv[2], 'wb').write(data[at + 8:at + 8 + size])
        break
    at += 8 + size + (size & 1)
PY
    done
  done
done
rm -rf "$tmp"
