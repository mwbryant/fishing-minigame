# GNSS (LC76G) and microphones (ES7210): research notes

Gathered from Waveshare's repo, Quectel's specs, Espressif's drivers and the esp-hal 1.2.2
source. None of it has been tried on the board yet. Confidence is marked on each part.
Sources are listed at the end.

## GNSS on the ESP32-S3-Touch-AMOLED-1.75-G (SKU 31264)

- **Module** [verified]: Quectel LC76G (AB), schematic part `LC76GABMD`, with a ceramic
  antenna on an IPEX Gen1 connector. SKUs: 31261 is the plain board, 31262 is -B (with
  case), 31264 is -G. To check at runtime, see whether I2C `0x50` answers.
- **Interface** [verified]: I2C on the shared bus (SDA GPIO15, SCL GPIO14). Three addresses:
  `0x50` takes commands, `0x54` is read from, `0x58` takes write data. None clash with the
  board's other devices.
- **UART** [likely not connected]: TXD → R15 → GPIO18 and RXD → R16 → GPIO17. Both
  resistors are marked "NC/0R", so they are probably not fitted. Check R15/R16 before
  relying on UART.
- **Reset** [verified]: module pin 9 is TCA9554 P7 (`EXIO7`/`GPS_RST`).
  - The expander boots with every pin as an input, so P7 floats until firmware sets it.
  - Probably active-low (Quectel calls it RESET_N) [likely].
- **Power** [verified]: VCC is VCC3V3, with no enable pin, so the module draws about
  36 mA the whole time the board is on. V_BCKP runs from VCC-RTC, so hot starts survive.
  FORCE_ON (R18) is not fitted, and 1PPS doesn't reach a GPIO [likely].
- **Defaults** [verified, Quectel spec v1.1 and protocol spec v1.4]:
  - 1 Hz fix rate (up to 10 Hz), NMEA 0183 v4.10.
  - GGA, GLL, GSA, GSV, RMC and VTG are on by default, with talker `GN` (multi-GNSS).
  - Time to first fix: cold 28 s, warm 25 s, hot 1 s. A cold start with EASY prediction
    is 15 s.
- **Commands** (the module acks with `$PAIR001,<cmd>,0`):
  - `$PAIR050,<ms>` sets the fix interval, 100–1000 ms.
  - `$PAIR062,<type>,<N>` sends a sentence every N fixes (0=GGA, 1=GLL, 2=GSA, 3=GSV,
    4=RMC, 5=VTG). For example, `$PAIR062,0,3*3D`.
  - `$PAIR513*3D` saves settings to flash. `$PAIR004*3E` is a hot restart and `$PAIR006*3C`
    a cold restart.
  - `$PAIR650,<s>` enters backup mode. Never use 0, because there's no FORCE_ON to wake it.
- **Reading NMEA over I2C** (Waveshare's `09_LC76G_I2C.ino`, bus at 100 kHz):
  1. Write `08 00 51 AA 04 00 00 00` to `0x50`, then read 4 bytes from `0x54`. That's the
     number of bytes waiting (u32 LE).
  2. Write `00 20 51 AA <len u32 LE>` to `0x50`, then read `len` bytes of NMEA from `0x54`.
  3. Wait about 10 ms between steps. Read at most 4096 bytes at a time.
- **Sending a command** over I2C:
  1. Write `04 00 51 AA 04 00 00 00` to `0x50`, then read the free space from `0x54`.
  2. Write `00 10 53 AA <len>` to `0x50`, then send the data to `0x58`.
- **Gotcha** (Quectel forum): on a shared bus with reset on a TCA9554, `0x54` started NACKing
  after a few minutes and only a long power-off cleared it. Quectel's advice is no other
  bus traffic during LC76G transactions (or use the UART). Touch and the IMU share this bus.

## ES7210 mic ADC (I2C `0x40`, SDOUT1 → GPIO10)

Verified from esp-adf `esp_codec_dev/device/es7210/es7210.c`: slave mode, MIC1 and MIC2.
The current I2S setup gives MCLK = 256 × fs, which is 4.096 MHz at 16 kHz.

1. **Open:** `00=FF, 00=41, 01=3F, 09=30, 0A=30, 23=2A, 22=0A, 20=0A, 21=2A`. Clear bit 0 of
   `08` (slave). Then `40=43, 41=70, 42=70` (mic bias 2.87 V), `07=20, 02=C1`.
2. **Mic select:** clear bit 4 of `43`–`46`; `4B=FF, 4C=FF`; clear bits 0x0B of `01`;
   `4B=00`; set bit 4 of `43` and `44`; `12=00` (two mics, not TDM).
3. **Format:** `11 = (11 & 0x1C) | 0x60`, 16-bit Philips I2S.
4. **Start:** `01=34, 06=00, 40=43, 47..4A=08`, then step 2 again, then `40=43, 00=71, 00=41`.
5. **Gain, after start** (step 2 resets it to 0 dB): the low nibble of `43`/`44`.
   - Codes 0–10 are 0–30 dB in 3 dB steps; 11 = 33, 12 = 34.5, 13 = 36, 14 = 37.5 dB.
   - The driver's default is 30 dB: `43=1A, 44=1A`.
6. **Unmute:** clear bits 1:0 of `14` and `15`.

Start I2S (and so MCLK) before the codec.
- **Channels:** MIC1 and MIC2 are the two front mics [verified, BSP header]. MIC3 is the
  ES8311 playback echo reference, and MIC4 is not connected.
- **Slots:** with `12=00`, SDOUT1 should carry MIC1 on the left slot and MIC2 on the right
  [likely].

## esp-hal 1.2.2: speaker and mics at once [verified in source]

- `I2s::new` gives both `i2s_tx` and `i2s_rx` on one DMA channel (DMA_CH1).
- `TdmConfig::with_signal_loopback(true)` makes RX share TX's BCLK/WS (`SIG_LOOPBACK`,
  `RX_SLAVE_MOD`).
- Wire TX as today (bclk 9, ws 45, dout 8), and give RX only `.with_din(GPIO10)`. Don't
  hand RX the real clock pins. `hil-test/src/bin/i2s.rs` `run_test_i2s_loopback` does this.
- **The mics need the speaker stream:**
  - RX's clocks come from TX, and `TX_STOP_EN` stops them when the TX FIFO empties.
  - So keep TX streaming (silence) while listening. A speaker underrun or restart stalls
    capture too.
- **RX stream:** like `AudioOut`, it stops when its descriptors fill and needs the same
  restart pattern. 4 KB of buffer is about 62 ms at 16 kHz stereo.
- **MCLK:** it comes from the RX clock (`RX_CLKM_CONF.MCLK_SEL=1`). It should keep working
  with RX in slave mode [likely]; check the speaker still plays.

## Sources

- Waveshare `HARDWARE_REFERENCE.md`, the schematic PDF, `examples/arduino/09_LC76G_I2C`,
  and the BSP in `firmware/brookesia/components/waveshare__esp32_s3_touch_amoled_1_75`
  (github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75)
- Quectel LC76G GNSS Specification v1.1; LC26G/LC76G/LC86G GNSS Protocol Specification v1.4;
  LC26G(AB)/LC76G I2C Application Note v1.0; Quectel forum thread "LC76G(AB) over I2C 0x54
  permanently NACKs"
- esp-adf `components/esp_codec_dev/device/es7210/es7210.c` and `es7210_reg.h`; ES7210
  datasheet (files.waveshare.com)
- esp-hal v1.2.2 `src/i2s/master/{mod.rs,low_level/v2.rs}` and `hil-test/src/bin/i2s.rs`
