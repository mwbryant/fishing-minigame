//! The PWR key. Its live level is on TCA9554 pin P4 (SYS_OUT, high while
//! pressed). The AXP2101 power chip also watches it and switches the board
//! off after a long hold; `axp2101` stretches that hold to the 10 s maximum.

use embedded_hal::i2c::I2c;

const TCA9554_ADDRESS: u8 = 0x20;
const TCA_INPUT: u8 = 0x00;
const TCA_CONFIG: u8 = 0x03;
const SYS_OUT: u8 = 1 << 4;

pub struct PwrKey<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> PwrKey<I2C> {
    /// Makes P4 an input without touching the other expander pins.
    pub fn new(mut i2c: I2C) -> Result<Self, I2C::Error> {
        let config = read(&mut i2c, TCA_CONFIG)?;
        i2c.write(TCA9554_ADDRESS, &[TCA_CONFIG, config | SYS_OUT])?;
        Ok(Self { i2c })
    }

    pub fn is_pressed(&mut self) -> Result<bool, I2C::Error> {
        Ok(read(&mut self.i2c, TCA_INPUT)? & SYS_OUT != 0)
    }
}

fn read<I2C: I2c>(i2c: &mut I2C, register: u8) -> Result<u8, I2C::Error> {
    let mut value = [0u8];
    i2c.write_read(TCA9554_ADDRESS, &[register], &mut value)?;
    Ok(value[0])
}
