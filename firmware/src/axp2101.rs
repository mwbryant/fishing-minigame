//! The AXP2101 power chip (I2C 0x34): the PWR key's hold-to-off time, the
//! battery's presence, charge state, voltage and fuel-gauge percentage, and
//! switching the whole board off. Register meanings are from the AXP2101
//! datasheet v1.4 (section 6.13).

use embedded_hal::i2c::I2c;
use orbit_core::Battery;

const ADDRESS: u8 = 0x34;
/// Bit 0 switches every output off, the ESP32's 3.3 V included.
const COMMON_CONFIG: u8 = 0x10;
const SOFT_POWER_OFF: u8 = 1 << 0;
const STATUS1: u8 = 0x00;
const VBUS_GOOD: u8 = 1 << 5;
const BATTERY_PRESENT: u8 = 1 << 3;
/// Bits 6:5 are the battery current direction: 00 standby, 01 charge, 10 discharge.
const STATUS2: u8 = 0x01;
const CHARGING: u8 = 0b01;
/// PWRON timing: bits 3:2 are the hold-to-power-off time (4/6/8/10 s).
const PWRON_LEVELS: u8 = 0x27;
const OFFLEVEL_MASK: u8 = 0b11 << 2;
const OFFLEVEL_10S: u8 = 0b11 << 2;
/// Battery voltage in mV: bits 13:8 in the low six bits of VBAT_H, then VBAT_L.
const VBAT_H: u8 = 0x34;
const VBAT_L: u8 = 0x35;
const BATTERY_PERCENT: u8 = 0xA4;

pub struct Axp2101<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Axp2101<I2C> {
    /// Stretches the PWR hold-to-off to its 10 s maximum (the chip forgets it
    /// whenever it loses power, so this runs every boot).
    pub fn new(mut i2c: I2C) -> Result<Self, I2C::Error> {
        let levels = read(&mut i2c, PWRON_LEVELS)?;
        i2c.write(
            ADDRESS,
            &[PWRON_LEVELS, (levels & !OFFLEVEL_MASK) | OFFLEVEL_10S],
        )?;
        Ok(Self { i2c })
    }

    /// The battery, and its voltage in mV (0 without a cell). The fuel gauge
    /// and the battery-voltage ADC are both on from power-on.
    pub fn battery(&mut self) -> Result<(Battery, u16), I2C::Error> {
        let status1 = read(&mut self.i2c, STATUS1)?;
        let status2 = read(&mut self.i2c, STATUS2)?;
        let present = status1 & BATTERY_PRESENT != 0;
        let (percent, millivolts) = if present {
            let high = read(&mut self.i2c, VBAT_H)? & 0x3F;
            let low = read(&mut self.i2c, VBAT_L)?;
            let percent = read(&mut self.i2c, BATTERY_PERCENT)?.min(100);
            (Some(percent), u16::from(high) << 8 | u16::from(low))
        } else {
            (None, 0)
        };
        let battery = Battery {
            percent,
            external_power: status1 & VBUS_GOOD != 0,
            charging: (status2 >> 5) & 0b11 == CHARGING,
        };
        Ok((battery, millivolts))
    }

    /// Switches the board off: every rail, so the display, codec, mics and
    /// GPS draw nothing either. Only PWR turns it back on, as a fresh boot.
    pub fn power_off(&mut self) -> Result<(), I2C::Error> {
        let config = read(&mut self.i2c, COMMON_CONFIG)?;
        self.i2c
            .write(ADDRESS, &[COMMON_CONFIG, config | SOFT_POWER_OFF])
    }
}

fn read<I2C: I2c>(i2c: &mut I2C, register: u8) -> Result<u8, I2C::Error> {
    let mut value = [0u8];
    i2c.write_read(ADDRESS, &[register], &mut value)?;
    Ok(value[0])
}
