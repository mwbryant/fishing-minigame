//! Blocking QMI8658 6-axis IMU driver, with its built-in pedometer. Register
//! values follow the QMI8658A datasheet and Waveshare's qmi8658 component /
//! SensorLib (`SensorQMI8658.hpp`'s `configPedometer`, `writeCommand`).

use embedded_hal::{delay::DelayNs, i2c::I2c};

/// SA0 is strapped high on this board.
pub const ADDRESS: u8 = 0x6B;

const WHO_AM_I: u8 = 0x00;
const CTRL1: u8 = 0x02;
const CTRL2: u8 = 0x03;
const CTRL3: u8 = 0x04;
const CTRL5: u8 = 0x06;
const CTRL7: u8 = 0x08;
const CTRL8: u8 = 0x09;
const CTRL9: u8 = 0x0A;
const CAL1_L: u8 = 0x0B;
const STATUS_INT: u8 = 0x2D;
const AX_L: u8 = 0x35;
const STEP_CNT_L: u8 = 0x5A;
const RESET_RESULT: u8 = 0x4D;
const RESET: u8 = 0x60;

const CHIP_ID: u8 = 0x05;
const RESET_COMMAND: u8 = 0xB0;
const RESET_DONE: u8 = 0x80;
/// ADDR_AI (bit 6) plus the reset-default bit 5, exactly what both vendor
/// drivers write; they then decode the data registers little-endian.
const CTRL1_AUTO_INCREMENT: u8 = 0x60;
/// aFS = 001 (+-4 g), aODR = 0011 (896.8 Hz with the gyro on).
const CTRL2_ACCEL_4G: u8 = 0x13;
/// gFS = 101 (+-512 dps), gODR = 0011 (896.8 Hz).
const CTRL3_GYRO_512DPS: u8 = 0x53;
/// Both low-pass filters on, mode 01 (3.63 % of ODR, about 33 Hz).
const CTRL5_LPF: u8 = 0x33;
const CTRL7_ACCEL_GYRO: u8 = 0x03;
const CTRL7_ACCEL: u8 = 0x01;
/// For the pedometer: aODR/gODR 0111, 56 Hz with the gyro on and 62.5 Hz
/// with it off, the rate SensorLib's pedometer settings are tuned for.
const CTRL2_ACCEL_4G_STEPS: u8 = 0x17;
const CTRL3_GYRO_512DPS_STEPS: u8 = 0x57;
const CTRL8_PEDOMETER: u8 = 1 << 4;
const CMD_ACK: u8 = 0x00;
const CMD_CONFIGURE_PEDOMETER: u8 = 0x0D;
const CMD_RESET_PEDOMETER: u8 = 0x0F;
const CMD_DONE: u8 = 1 << 7;
/// SensorLib's example settings, written to CAL1_L..CAL4_H in two pages
/// (CAL4_H = page, CAL4_L = 0x02): a 50-sample window, 200 mg peak to peak,
/// 100 mg over the average; a 200-sample timeout, 20-sample minimum step,
/// counting once 10 steps come in a row (then all of them), and the count
/// updated every 4 steps.
const PEDOMETER_PAGES: [[u8; 8]; 2] = [
    [50, 0, 200, 0, 100, 0, 0x02, 0x01],
    [200, 0, 20, 10, 0, 4, 0x02, 0x02],
];

const ACCEL_LSB_PER_G: f32 = 8192.0;
const GYRO_LSB_PER_DPS: f32 = 64.0;

#[derive(Debug)]
pub enum Error<E> {
    I2c(E),
    WrongChip(u8),
    ResetTimeout,
    CommandTimeout,
}

pub struct Qmi8658<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Qmi8658<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Soft-resets the chip, checks WHO_AM_I, then runs both sensors at
    /// 896.8 Hz (+-4 g, +-512 dps, ~33 Hz low-pass).
    pub fn init(&mut self, delay: &mut impl DelayNs) -> Result<(), Error<I2C::Error>> {
        self.write(RESET, RESET_COMMAND)?;
        self.wait_for_reset(delay)?;
        let id = self.read_register(WHO_AM_I)?;
        if id != CHIP_ID {
            return Err(Error::WrongChip(id));
        }
        self.write(CTRL1, CTRL1_AUTO_INCREMENT)?;
        self.write(CTRL2, CTRL2_ACCEL_4G)?;
        self.write(CTRL3, CTRL3_GYRO_512DPS)?;
        self.write(CTRL5, CTRL5_LPF)?;
        self.write(CTRL7, CTRL7_ACCEL_GYRO)
    }

    /// Counts steps: both sensors at the pedometer's rate, the pedometer set
    /// up and enabled, and its count zeroed. `init` undoes it.
    pub fn start_pedometer(&mut self, delay: &mut impl DelayNs) -> Result<(), Error<I2C::Error>> {
        self.write(CTRL7, 0)?;
        self.write(CTRL2, CTRL2_ACCEL_4G_STEPS)?;
        self.write(CTRL3, CTRL3_GYRO_512DPS_STEPS)?;
        for page in PEDOMETER_PAGES {
            // In SensorLib's order: CAL1_L..CAL3_H, then CAL4_H before CAL4_L.
            for (offset, value) in page.iter().enumerate().take(6) {
                self.write(CAL1_L + offset as u8, *value)?;
            }
            self.write(CAL1_L + 7, page[7])?;
            self.write(CAL1_L + 6, page[6])?;
            self.command(CMD_CONFIGURE_PEDOMETER, delay)?;
        }
        let ctrl8 = self.read_register(CTRL8)?;
        self.write(CTRL8, ctrl8 | CTRL8_PEDOMETER)?;
        self.command(CMD_RESET_PEDOMETER, delay)?;
        self.write(CTRL7, CTRL7_ACCEL_GYRO)
    }

    /// The pedometer's running count (it updates every few steps).
    pub fn steps(&mut self) -> Result<u32, Error<I2C::Error>> {
        let mut count = [0u8; 3];
        self.i2c
            .write_read(ADDRESS, &[STEP_CNT_L], &mut count)
            .map_err(Error::I2c)?;
        Ok(u32::from_le_bytes([count[0], count[1], count[2], 0]))
    }

    /// Gyro off (the pedometer needs only the accelerometer), or back on.
    pub fn gyro(&mut self, on: bool) -> Result<(), Error<I2C::Error>> {
        self.write(CTRL7, if on { CTRL7_ACCEL_GYRO } else { CTRL7_ACCEL })
    }

    /// Runs a CTRL9 command: write it, wait for CmdDone (STATUS_INT bit 7),
    /// acknowledge, and wait for the bit to clear.
    fn command(&mut self, command: u8, delay: &mut impl DelayNs) -> Result<(), Error<I2C::Error>> {
        self.write(CTRL9, command)?;
        self.wait_for_done(true, delay)?;
        self.write(CTRL9, CMD_ACK)?;
        self.wait_for_done(false, delay)
    }

    fn wait_for_done(
        &mut self,
        done: bool,
        delay: &mut impl DelayNs,
    ) -> Result<(), Error<I2C::Error>> {
        for _ in 0..100 {
            if (self.read_register(STATUS_INT)? & CMD_DONE != 0) == done {
                return Ok(());
            }
            delay.delay_ms(1);
        }
        Err(Error::CommandTimeout)
    }

    /// Turns both sensors off (CTRL7 = 0), a few microamps until `init`.
    pub fn power_down(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(CTRL7, 0)
    }

    /// Accel in g and gyro in dps, from one 12-byte burst read of AX_L..GZ_H.
    pub fn read(&mut self) -> Result<([f32; 3], [f32; 3]), Error<I2C::Error>> {
        let mut raw = [0u8; 12];
        self.i2c
            .write_read(ADDRESS, &[AX_L], &mut raw)
            .map_err(Error::I2c)?;
        let axis = |i: usize| f32::from(i16::from_le_bytes([raw[2 * i], raw[2 * i + 1]]));
        let accel_g = [0, 1, 2].map(|i| axis(i) / ACCEL_LSB_PER_G);
        let gyro_dps = [3, 4, 5].map(|i| axis(i) / GYRO_LSB_PER_DPS);
        Ok((accel_g, gyro_dps))
    }

    fn wait_for_reset(&mut self, delay: &mut impl DelayNs) -> Result<(), Error<I2C::Error>> {
        for _ in 0..20 {
            delay.delay_ms(5);
            if let Ok(RESET_DONE) = self.read_register(RESET_RESULT) {
                return Ok(());
            }
        }
        Err(Error::ResetTimeout)
    }

    fn read_register(&mut self, register: u8) -> Result<u8, Error<I2C::Error>> {
        let mut value = [0u8];
        self.i2c
            .write_read(ADDRESS, &[register], &mut value)
            .map_err(Error::I2c)?;
        Ok(value[0])
    }

    fn write(&mut self, register: u8, value: u8) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(ADDRESS, &[register, value])
            .map_err(Error::I2c)
    }
}
