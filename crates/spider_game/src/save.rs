//! Spider Roulette's section of the board's save record: `version | best`.

use alloc::vec::Vec;

pub const KEY: [u8; 4] = *b"SPDR";
const VERSION: u8 = 1;

pub fn encode(best: u16) -> Vec<u8> {
    let [lo, hi] = best.to_le_bytes();
    alloc::vec![VERSION, lo, hi]
}

pub fn decode(bytes: &[u8]) -> Option<u16> {
    match bytes {
        [VERSION, lo, hi, ..] => Some(u16::from_le_bytes([*lo, *hi])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_best_run_round_trips_and_strangers_are_ignored() {
        assert_eq!(decode(&encode(11)), Some(11));
        assert_eq!(decode(&[9, 1, 0]), None);
        assert_eq!(decode(&[]), None);
    }
}
