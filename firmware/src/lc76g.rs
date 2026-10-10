//! The -G board's Quectel LC76G satellite receiver on the shared I2C bus.
//!
//! A read asks `0x50` how many NMEA bytes are waiting and reads the count from
//! `0x54`, then asks `0x50` for that many and reads them from `0x54`. Each step
//! wants about 10 ms after the last, so `poll` runs one step per frame instead
//! of blocking for 20 ms. The module's reset is TCA9554 P7, released on
//! startup; boards without the module don't answer `0x50`. From Waveshare's
//! `09_LC76G_I2C` demo and Quectel's I2C note (`docs/reference/gnss_and_mics.md`).

use embedded_hal::{delay::DelayNs, i2c::I2c};
use esp_hal::time::Instant;

const COMMAND: u8 = 0x50;
const READ: u8 = 0x54;
const ASK_LENGTH: [u8; 8] = [0x08, 0x00, 0x51, 0xAA, 0x04, 0x00, 0x00, 0x00];
const ASK_DATA: [u8; 4] = [0x00, 0x20, 0x51, 0xAA];
/// The receiver wants this long between the steps of a read.
const STEP_MS: u64 = 10;
/// It reports once a second, so it's asked once a second.
const POLL_MS: u64 = 1000;
const TCA9554_ADDRESS: u8 = 0x20;
const TCA_OUTPUT: u8 = 0x01;
const TCA_CONFIG: u8 = 0x03;
const GPS_RESET: u8 = 1 << 7;
/// How long a restart holds the module's reset low.
const RESET_MS: u32 = 20;

#[derive(Clone, Copy, Debug)]
enum Step {
    Idle,
    AskedLength,
    AskedData(usize),
}

pub struct Lc76g<I2C> {
    i2c: I2C,
    step: Step,
    since: Instant,
}

impl<I2C: I2c> Lc76g<I2C> {
    /// Releases the module's reset (P7 driven high) and asks for the first
    /// length; `None` on a board without the module.
    pub fn new(mut i2c: I2C) -> Option<Self> {
        let mut value = [0u8];
        i2c.write_read(TCA9554_ADDRESS, &[TCA_OUTPUT], &mut value)
            .ok()?;
        i2c.write(TCA9554_ADDRESS, &[TCA_OUTPUT, value[0] | GPS_RESET])
            .ok()?;
        i2c.write_read(TCA9554_ADDRESS, &[TCA_CONFIG], &mut value)
            .ok()?;
        i2c.write(TCA9554_ADDRESS, &[TCA_CONFIG, value[0] & !GPS_RESET])
            .ok()?;
        i2c.write(COMMAND, &ASK_LENGTH).ok()?;
        Some(Self {
            i2c,
            step: Step::AskedLength,
            since: Instant::now(),
        })
    }

    /// Runs the next read step if it's due. Returns how many NMEA bytes it put
    /// in `out`: 0 until a read's last step.
    pub fn poll(&mut self, out: &mut [u8]) -> Result<usize, I2C::Error> {
        let waited = self.since.elapsed().as_millis();
        let due = match self.step {
            Step::Idle => waited >= POLL_MS,
            _ => waited >= STEP_MS,
        };
        if !due {
            return Ok(0);
        }
        self.since = Instant::now();
        let (next, read) = match self.step {
            Step::Idle => {
                self.i2c.write(COMMAND, &ASK_LENGTH)?;
                (Step::AskedLength, 0)
            }
            Step::AskedLength => {
                let mut length = [0u8; 4];
                self.i2c.read(READ, &mut length)?;
                let waiting = (u32::from_le_bytes(length) as usize).min(out.len());
                if waiting == 0 {
                    (Step::Idle, 0)
                } else {
                    let mut ask = [0u8; 8];
                    ask[..4].copy_from_slice(&ASK_DATA);
                    ask[4..].copy_from_slice(&(waiting as u32).to_le_bytes());
                    self.i2c.write(COMMAND, &ask)?;
                    (Step::AskedData(waiting), 0)
                }
            }
            Step::AskedData(waiting) => {
                self.i2c.read(READ, &mut out[..waiting])?;
                (Step::Idle, waiting)
            }
        };
        self.step = next;
        Ok(read)
    }

    /// After a failed transaction, start over at the next poll.
    pub fn reset(&mut self) {
        self.step = Step::Idle;
        self.since = Instant::now();
    }

    /// Holds the module in reset for a moment and lets it start again, for
    /// when it stops answering. Its backup supply keeps what it knows of the
    /// sky, so it picks a fix back up quickly.
    pub fn restart(&mut self, delay: &mut impl DelayNs) -> Result<(), I2C::Error> {
        let mut value = [0u8];
        self.i2c
            .write_read(TCA9554_ADDRESS, &[TCA_OUTPUT], &mut value)?;
        self.i2c
            .write(TCA9554_ADDRESS, &[TCA_OUTPUT, value[0] & !GPS_RESET])?;
        delay.delay_ms(RESET_MS);
        self.i2c
            .write(TCA9554_ADDRESS, &[TCA_OUTPUT, value[0] | GPS_RESET])?;
        self.reset();
        Ok(())
    }
}
