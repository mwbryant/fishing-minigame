//! Bees???'s section of the board's save record: `version | best orbits`.

use alloc::vec::Vec;

pub const KEY: [u8; 4] = *b"BEES";
const VERSION: u8 = 1;

pub fn encode(best: u32) -> Vec<u8> {
    let mut out = alloc::vec![VERSION];
    out.extend_from_slice(&best.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8]) -> Option<u32> {
    match bytes {
        [VERSION, a, b, c, d, ..] => Some(u32::from_le_bytes([*a, *b, *c, *d])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_run_round_trips_and_strangers_are_ignored() {
        assert_eq!(decode(&encode(70_000)), Some(70_000));
        assert_eq!(decode(&[9, 1, 0, 0, 0]), None);
        assert_eq!(decode(&[]), None);
    }
}
