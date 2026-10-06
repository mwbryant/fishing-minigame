//! Blocking QMI8658 6-axis IMU driver. Register values follow the QMI8658A
//! datasheet and Waveshare's qmi8658 component / SensorLib.

use embedded_hal::{delay::DelayNs, i2c::I2c};

/// SA0 is strapped high on this board.
pub const ADDRESS: u8 = 0x6B;

const WHO_AM_I: u8 = 0x00;
const CTRL1: u8 = 0x02;
const CTRL2: u8 = 0x03;
const CTRL3: u8 = 0x04;
const CTRL5: u8 = 0x06;
const CTRL7: u8 = 0x08;
const AX_L: u8 = 0x35;
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

const ACCEL_LSB_PER_G: f32 = 8192.0;
const GYRO_LSB_PER_DPS: f32 = 64.0;

#[derive(Debug)]
pub enum Error<E> {
    I2c(E),
    WrongChip(u8),
    ResetTimeout,
}

pub struct Qmi8658<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Qmi8658<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

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

    /// Turns both sensors off (CTRL7 = 0), a few microamps until `init`.
    pub fn power_down(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(CTRL7, 0)
    }

    /// One burst read of AX_L..GZ_H (12 bytes, little-endian i16 pairs).
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
