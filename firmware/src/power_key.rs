//! The PWR key. Its live level is on TCA9554 pin P4 (SYS_OUT, high while
//! pressed). The AXP2101 power chip also watches it and switches the board
//! off after a long hold, so boot stretches that hold to the 10 s maximum.

use embedded_hal::i2c::I2c;

const TCA9554_ADDRESS: u8 = 0x20;
const TCA_INPUT: u8 = 0x00;
const TCA_CONFIG: u8 = 0x03;
const SYS_OUT: u8 = 1 << 4;

const AXP2101_ADDRESS: u8 = 0x34;
/// PWRON timing: bits 3:2 are the hold-to-power-off time (4/6/8/10 s).
const AXP_PWRON_LEVELS: u8 = 0x27;
const OFFLEVEL_MASK: u8 = 0b11 << 2;
const OFFLEVEL_10S: u8 = 0b11 << 2;

pub struct PwrKey<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> PwrKey<I2C> {
    /// Makes P4 an input without touching the other expander pins, and
    /// stretches the power-off hold (the PMIC forgets it whenever it loses
    /// power, so this runs every boot).
    pub fn new(mut i2c: I2C) -> Result<Self, I2C::Error> {
        let config = read(&mut i2c, TCA9554_ADDRESS, TCA_CONFIG)?;
        i2c.write(TCA9554_ADDRESS, &[TCA_CONFIG, config | SYS_OUT])?;
        let levels = read(&mut i2c, AXP2101_ADDRESS, AXP_PWRON_LEVELS)?;
        i2c.write(
            AXP2101_ADDRESS,
            &[AXP_PWRON_LEVELS, (levels & !OFFLEVEL_MASK) | OFFLEVEL_10S],
        )?;
        Ok(Self { i2c })
    }

    pub fn is_pressed(&mut self) -> Result<bool, I2C::Error> {
        Ok(read(&mut self.i2c, TCA9554_ADDRESS, TCA_INPUT)? & SYS_OUT != 0)
    }
}

fn read<I2C: I2c>(i2c: &mut I2C, address: u8, register: u8) -> Result<u8, I2C::Error> {
    let mut value = [0u8];
    i2c.write_read(address, &[register], &mut value)?;
    Ok(value[0])
}
