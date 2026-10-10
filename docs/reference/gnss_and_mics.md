# GNSS (LC76G) and microphones (ES7210): research notes

The research behind `firmware/src/lc76g.rs` and `firmware/src/mic.rs`: the -G board's GNSS
module, the ES7210 mic ADC, and how esp-hal 1.2.2 runs the speaker and mics on one I2S port.
It comes from Waveshare's repo, Quectel's specs, Espressif's drivers and the esp-hal source
(listed at the end).

**On the board** both drivers work: voice commands run on the mics, and the GNSS driver reads
NMEA. No game uses GPS now, so the firmware sends the receiver to backup mode at boot; the boot
log shows `ES7210 mics listening`, `LC76G satellite receiver found; sent to backup mode` and
`LC76G asleep (backup mode)`.

## GNSS on the ESP32-S3-Touch-AMOLED-1.75-G (SKU 31264)

The module, its wiring, and how to read NMEA and send commands over I2C.

- **Module**: Quectel LC76G (AB), schematic part `LC76GABMD`, with a ceramic
  antenna on an IPEX Gen1 connector.
  - SKUs: 31261 is the plain board, 31262 the -B (with case), 31264 the -G.
  - To check at runtime, see whether I2C `0x50` answers.
- **Interface**: I2C on the shared bus (SDA GPIO15, SCL GPIO14), at three
  addresses: `0x50` takes commands, `0x54` is read from, `0x58` takes write data. None clash
  with the board's other devices.
- **UART:** TXD → R15 → GPIO18 and RXD → R16 → GPIO17, through resistors marked "NC/0R"
  that may not be fitted. The firmware uses I2C.
- **Reset**: module pin 9 is TCA9554 P7 (`EXIO7`/`GPS_RST`).
  - The expander boots with every pin as an input, so P7 floats until firmware sets it.
  - Active-low (Quectel calls it RESET_N). On the board, the module answers once the
    firmware drives P7 high.
- **Power**: VCC is VCC3V3 with no enable pin, so running it draws about 36 mA. The
  firmware puts it in backup mode at boot (`$PAIR650,0*25`, 13 uA; on the (AB) variant VCC
  may stay on, hardware design v1.3 3.3.4 note 3) because nothing uses it. V_BCKP runs from
  VCC-RTC, so hot starts survive. FORCE_ON (R18)
  isn't fitted, and 1PPS doesn't reach a GPIO.
- **Defaults** (Quectel spec v1.1 and protocol spec v1.4):
  - 1 Hz fix rate (up to 10 Hz), NMEA 0183 v4.10.
  - GGA, GLL, GSA, GSV, RMC and VTG are on, with talker `GN` (multi-GNSS).
  - Time to first fix: cold 28 s, warm 25 s, hot 1 s. A cold start with EASY prediction is
    15 s.
- **Commands** (the module acks with `$PAIR001,<cmd>,0`):
  - `$PAIR050,<ms>` sets the fix interval, 100–1000 ms.
  - `$PAIR062,<type>,<N>` sends a sentence every N fixes (0=GGA, 1=GLL, 2=GSA, 3=GSV, 4=RMC,
    5=VTG), for example `$PAIR062,0,3*3D`.
  - `$PAIR513*3D` saves settings to flash. `$PAIR004*3E` is a hot restart and `$PAIR006*3C` a
    cold restart.
  - `$PAIR650,<s>` enters backup mode for `<s>` seconds. Always pass a nonzero time.
- **Reading NMEA over I2C** (Waveshare's `09_LC76G_I2C.ino`, bus at 100 kHz):
  1. Write `08 00 51 AA 04 00 00 00` to `0x50`, then read 4 bytes from `0x54`: the number of
     bytes waiting (u32 LE).
  2. Write `00 20 51 AA <len u32 LE>` to `0x50`, then read `len` bytes of NMEA from `0x54`.
  3. Wait about 10 ms between steps. Read at most 4096 bytes at a time.
- **Sending a command over I2C:**
  1. Write `04 00 51 AA 04 00 00 00` to `0x50`, then read the free space from `0x54`.
  2. Write `00 10 53 AA <len>` to `0x50`, then send the data to `0x58`.
- **Gotcha** (Quectel forum): on a shared bus with reset on a TCA9554, `0x54` started NACKing
  after a few minutes, and only a long power-off cleared it. Quectel advises no other bus
  traffic during LC76G transactions (or using the UART). Touch and the IMU share this bus.

## ES7210 mic ADC (I2C `0x40`, SDOUT1 → GPIO10)

The register sequence that brings up two mics in slave mode, and how their data arrives.

Verified from esp-adf `esp_codec_dev/device/es7210/es7210.c`: slave mode, MIC1 and MIC2. The
I2S setup gives MCLK = 256 × fs, which is 4.096 MHz at 16 kHz. Start I2S (and so MCLK) before
the codec.

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

- **Channels** (BSP header): MIC1 and MIC2 are the two front mics. MIC3 is the
  ES8311 playback echo reference, and MIC4 isn't connected.
- **Slots**: with `12=00`, SDOUT1 carries MIC1 on the left slot and MIC2 on the
  right.

## esp-hal 1.2.2: speaker and mics at once (from the source)

How one I2S port plays and records at the same time, and what that costs.

- `I2s::new` gives both `i2s_tx` and `i2s_rx` on one DMA channel (DMA_CH1).
- `TdmConfig::with_signal_loopback(true)` makes RX share TX's BCLK/WS (`SIG_LOOPBACK`,
  `RX_SLAVE_MOD`).
- Wire TX as for the speaker (bclk 9, ws 45, dout 8), and give RX only `.with_din(GPIO10)`.
  Don't hand RX the real clock pins. `hil-test/src/bin/i2s.rs` `run_test_i2s_loopback` does
  this.
- **The mics need the speaker stream:**
  - RX's clocks come from TX, and `TX_STOP_EN` stops them when the TX FIFO empties.
  - So keep TX streaming (silence) while listening. A speaker underrun or restart stalls
    capture too.
- **RX stream:** like `AudioOut`, it stops when its descriptors fill and needs the same restart
  pattern. 4 KB of buffer is about 62 ms at 16 kHz stereo.
- **MCLK:** it comes from the RX clock (`RX_CLKM_CONF.MCLK_SEL=1`). It should keep working
  with RX in slave mode; check the speaker still plays.

## Sources

- Waveshare `HARDWARE_REFERENCE.md`, the schematic PDF, `examples/arduino/09_LC76G_I2C`, and
  the BSP in `firmware/brookesia/components/waveshare__esp32_s3_touch_amoled_1_75`
  (github.com/waveshareteam/ESP32-S3-Touch-AMOLED-1.75)
- Quectel LC76G GNSS Specification v1.1; LC26G/LC76G/LC86G GNSS Protocol Specification v1.4;
  LC26G(AB)/LC76G I2C Application Note v1.0; Quectel forum thread "LC76G(AB) over I2C 0x54
  permanently NACKs"
- esp-adf `components/esp_codec_dev/device/es7210/es7210.c` and `es7210_reg.h`; ES7210
  datasheet (files.waveshare.com)
- esp-hal v1.2.2 `src/i2s/master/{mod.rs,low_level/v2.rs}` and `hil-test/src/bin/i2s.rs`
