//! The microphone at the board's rate: browsers capture at 44.1 or 48 kHz,
//! and voice prints are made from 16 kHz, so each output sample averages the
//! input it covers, like the desktop simulator's.

use orbit_audio::SAMPLE_RATE;

pub struct Downsampler {
    from_rate: u32,
    /// Output progress counted in input samples times `SAMPLE_RATE`, so
    /// the steps add up exactly.
    phase: u32,
    sum: f32,
    count: u32,
}

impl Downsampler {
    pub fn new(from_rate: f32) -> Self {
        Self {
            from_rate: from_rate as u32,
            phase: 0,
            sum: 0.0,
            count: 0,
        }
    }

    /// Mono samples (-1..1) at the input rate, onto `out` at 16 kHz.
    pub fn push(&mut self, samples: &[f32], out: &mut Vec<i16>) {
        for &sample in samples {
            self.sum += sample;
            self.count += 1;
            self.phase += SAMPLE_RATE;
            if self.phase < self.from_rate {
                continue;
            }
            self.phase -= self.from_rate;
            let mean = (self.sum / self.count as f32).clamp(-1.0, 1.0);
            out.push((mean * 32767.0) as i16);
            self.sum = 0.0;
            self.count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_samples_at_48_khz_average_into_one_at_16() {
        let mut downsampler = Downsampler::new(48_000.0);
        let mut out = Vec::new();
        downsampler.push(&[0.1, 0.2, 0.3, -0.5, -0.5], &mut out);
        downsampler.push(&[-0.5, 2.0, 2.0, 2.0], &mut out);
        assert_eq!(out, [6553, -16383, 32767]);
    }
}
