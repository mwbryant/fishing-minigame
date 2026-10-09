//! Satellite position from the NMEA 0183 text the -G board's LC76G sends.
//! Platforms push the bytes and games take each new fix. Only GGA (satellites,
//! HDOP) and RMC (position, speed) sentences are read.

use alloc::boxed::Box;

use bevy::{math::ops, prelude::*};

/// Mean Earth radius, for distances between nearby fixes.
const EARTH_RADIUS_M: f32 = 6_371_000.0;
/// NMEA's 82-character sentence limit, plus slack.
const MAX_SENTENCE: usize = 96;

/// Where the receiver was, how sure it is, and how fast it was moving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fix {
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Horizontal dilution of precision: under about 2 is good, over 5 poor.
    pub hdop: f32,
    pub satellites: u8,
    pub speed_mps: f32,
}

impl Fix {
    /// Metres between two fixes, flat-Earth: exact enough for a walk's
    /// steps, which are metres apart.
    pub fn meters_to(&self, other: &Fix) -> f32 {
        let north = (other.lat_deg - self.lat_deg) as f32;
        let east = (other.lon_deg - self.lon_deg) as f32;
        let mid_lat = ((self.lat_deg + other.lat_deg) / 2.0) as f32;
        let east = east * ops::cos(mid_lat.to_radians());
        ops::sqrt(north * north + east * east).to_radians() * EARTH_RADIUS_M
    }
}

/// What the receiver reports. Only apps that add [`GpsPlugin`] have one, and
/// platforms run the receiver only while it exists. The sentence buffer is
/// boxed to keep the resource small (see `Mic`).
#[derive(Resource, Debug)]
pub struct Gps {
    sentence: Box<[u8; MAX_SENTENCE]>,
    length: usize,
    /// HDOP and satellites from the last GGA, to complete RMC's fixes.
    quality: Option<(f32, u8)>,
    latest: Option<Fix>,
    fresh: bool,
    /// Satellites in the last GGA, for "SEARCHING" screens.
    pub satellites: u8,
}

impl Default for Gps {
    fn default() -> Self {
        Self {
            sentence: Box::new([0; MAX_SENTENCE]),
            length: 0,
            quality: None,
            latest: None,
            fresh: false,
            satellites: 0,
        }
    }
}

impl Gps {
    /// NMEA text as it arrives, in any size of chunk.
    pub fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match byte {
                b'$' => {
                    self.sentence[0] = byte;
                    self.length = 1;
                }
                b'\r' | b'\n' if self.length > 0 => {
                    let length = core::mem::take(&mut self.length);
                    self.parse(length);
                }
                _ if self.length > 0 && self.length < MAX_SENTENCE => {
                    self.sentence[self.length] = byte;
                    self.length += 1;
                }
                _ => self.length = 0,
            }
        }
    }

    /// The newest fix, once.
    pub fn take(&mut self) -> Option<Fix> {
        core::mem::take(&mut self.fresh)
            .then_some(self.latest)
            .flatten()
    }

    /// The newest fix, however old.
    pub fn latest(&self) -> Option<Fix> {
        self.latest
    }

    fn parse(&mut self, length: usize) {
        let Ok(text) = core::str::from_utf8(&self.sentence[..length]) else {
            return;
        };
        let Some(body) = checked_body(text) else {
            return;
        };
        let mut fields = body.split(',');
        let Some(kind) = fields.next().and_then(|tag| tag.get(2..)) else {
            return;
        };
        let fields: Fields = fields.collect();
        match kind {
            "GGA" => {
                let satellites = fields.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                self.satellites = satellites;
                let quality = fields
                    .get(5)
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                let hdop = fields.get(7).and_then(|s| s.parse().ok());
                self.quality = hdop.filter(|_| quality > 0).map(|hdop| (hdop, satellites));
            }
            "RMC" => {
                if fields.get(1) != Some("A") {
                    return;
                }
                let lat = coordinate(fields.get(2), fields.get(3), 2);
                let lon = coordinate(fields.get(4), fields.get(5), 3);
                let (Some(lat_deg), Some(lon_deg)) = (lat, lon) else {
                    return;
                };
                let knots: f32 = fields.get(6).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let (hdop, satellites) = self.quality.unwrap_or((99.0, 0));
                self.latest = Some(Fix {
                    lat_deg,
                    lon_deg,
                    hdop,
                    satellites,
                    speed_mps: knots * 0.514_444,
                });
                self.fresh = true;
            }
            _ => {}
        }
    }
}

/// The text between `$` and `*` if its checksum is right.
fn checked_body(text: &str) -> Option<&str> {
    let (body, sum) = text.strip_prefix('$')?.split_once('*')?;
    let expected = u8::from_str_radix(sum.get(..2)?, 16).ok()?;
    (body.bytes().fold(0, |sum, byte| sum ^ byte) == expected).then_some(body)
}

/// `ddmm.mmmm` (or `dddmm.mmmm`, `degree_digits` 3) with its hemisphere, as
/// signed degrees.
fn coordinate(value: Option<&str>, hemisphere: Option<&str>, degree_digits: usize) -> Option<f64> {
    let value = value.filter(|v| v.len() > degree_digits)?;
    let degrees: f64 = value[..degree_digits].parse().ok()?;
    let minutes: f64 = value[degree_digits..].parse().ok()?;
    let unsigned = degrees + minutes / 60.0;
    match hemisphere? {
        "N" | "E" => Some(unsigned),
        "S" | "W" => Some(-unsigned),
        _ => None,
    }
}

/// A sentence's fields without allocating: NMEA sentences have fewer than
/// twenty.
#[derive(Default)]
struct Fields<'a> {
    fields: [&'a str; MAX_FIELDS],
    count: usize,
}

const MAX_FIELDS: usize = 20;

impl<'a> Fields<'a> {
    fn get(&self, index: usize) -> Option<&'a str> {
        (index < self.count).then(|| self.fields[index])
    }
}

impl<'a> FromIterator<&'a str> for Fields<'a> {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        let mut out = Fields::default();
        for field in iter.into_iter().take(MAX_FIELDS) {
            out.fields[out.count] = field;
            out.count += 1;
        }
        out
    }
}

/// Satellite position for walks. Opt-in like `ControlsPlugin`. It adds no
/// systems, since sentences are parsed as platforms push them.
pub struct GpsPlugin;

impl Plugin for GpsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Gps>();
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::String};

    use super::*;

    /// A sentence with its checksum and line ending, as the receiver sends it.
    fn nmea(body: &str) -> String {
        let sum = body.bytes().fold(0u8, |sum, byte| sum ^ byte);
        format!("${body}*{sum:02X}\r\n")
    }

    fn gga(satellites: u8, hdop: f32) -> String {
        nmea(&format!(
            "GNGGA,123519.00,4807.0380,N,01131.0000,E,1,{satellites:02},{hdop:.1},545.4,M,46.9,M,,"
        ))
    }

    fn rmc(lat: &str, lon: &str) -> String {
        nmea(&format!(
            "GNRMC,123519.00,A,{lat},N,{lon},E,2.5,84.4,230394,,,A,V"
        ))
    }

    #[test]
    fn an_rmc_after_a_gga_is_a_fix_with_its_precision_and_speed() {
        let mut gps = Gps::default();
        gps.push(gga(9, 1.2).as_bytes());
        assert_eq!(gps.take(), None, "GGA alone isn't a position");
        gps.push(rmc("4807.0380", "01131.0000").as_bytes());
        let fix = gps.take().expect("a fix");
        assert!((fix.lat_deg - (48.0 + 7.038 / 60.0)).abs() < 1e-9);
        assert!((fix.lon_deg - (11.0 + 31.0 / 60.0)).abs() < 1e-9);
        assert_eq!((fix.hdop, fix.satellites), (1.2, 9));
        assert!((fix.speed_mps - 1.286).abs() < 0.01);
        assert_eq!(gps.take(), None, "each fix once");
        assert_eq!(gps.latest(), Some(fix));
    }

    #[test]
    fn sentences_split_across_pushes_and_southern_or_western_fixes_parse() {
        let mut gps = Gps::default();
        let text = nmea("GPRMC,000000.00,A,3352.1200,S,15112.6000,W,0.0,0.0,010125,,,A");
        let (a, b) = text.as_bytes().split_at(17);
        gps.push(a);
        gps.push(b);
        let fix = gps.take().expect("a fix");
        assert!(fix.lat_deg < -33.0 && fix.lon_deg < -151.0);
    }

    #[test]
    fn no_fix_bad_checksums_and_garbage_are_ignored() {
        let mut gps = Gps::default();
        gps.push(nmea("GNRMC,123519.00,V,,,,,,,230394,,,N,V").as_bytes());
        gps.push(b"$GNRMC,123519.00,A,4807.0380,N,01131.0000,E,2.5,84.4,230394,,,A,V*00\r\n");
        gps.push(b"\x00\xff$$$,,,\r\n$GNGSV,1,1,00*79\r\n");
        assert_eq!(gps.take(), None);
    }

    #[test]
    fn a_thousandth_of_a_minute_north_is_under_two_metres() {
        let mut gps = Gps::default();
        gps.push(rmc("4807.0000", "01131.0000").as_bytes());
        let a = gps.take().unwrap();
        gps.push(rmc("4807.0010", "01131.0000").as_bytes());
        let b = gps.take().unwrap();
        assert!(
            (a.meters_to(&b) - 1.853).abs() < 0.01,
            "{}",
            a.meters_to(&b)
        );
        gps.push(rmc("4807.0000", "01131.0010").as_bytes());
        let c = gps.take().unwrap();
        let east = a.meters_to(&c);
        assert!(
            (east - 1.853 * ops::cos(48.117_f32.to_radians())).abs() < 0.01,
            "{east}"
        );
    }
}
