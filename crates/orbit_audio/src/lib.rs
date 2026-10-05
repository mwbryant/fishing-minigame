//! Sound for the round board: PCM clips that live in flash and a one-voice
//! mixer that both platforms pull samples from (the ESP32's I2S DMA stream,
//! the desktop's audio callback), so they play identical audio.

#![no_std]

use core::fmt;

/// Every clip and both platforms' outputs run at this rate; no resampling.
pub const SAMPLE_RATE: u32 = 16_000;

/// Mono, 16-bit little-endian PCM at [`SAMPLE_RATE`], usually `include_bytes!`.
pub struct Clip {
    pub pcm: &'static [u8],
}

impl Clip {
    pub fn samples(&self) -> usize {
        self.pcm.len() / 2
    }

    pub fn duration_secs(&self) -> f32 {
        self.samples() as f32 / SAMPLE_RATE as f32
    }

    fn sample(&self, index: usize) -> Option<i16> {
        let bytes = self.pcm.get(index * 2..index * 2 + 2)?;
        Some(i16::from_le_bytes([bytes[0], bytes[1]]))
    }
}

/// A clip compared by identity, never by its (large) contents.
#[derive(Clone, Copy)]
pub struct ClipRef(pub &'static Clip);

impl PartialEq for ClipRef {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}

impl fmt::Debug for ClipRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Clip({:p}, {} samples)", self.0, self.0.samples())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoundCommand {
    /// Starts the clip from the beginning, replacing whatever was playing.
    Play(ClipRef),
    Stop,
}

/// Plays one clip at a time; silence otherwise.
#[derive(Default)]
pub struct Mixer {
    playing: Option<(ClipRef, usize)>,
}

impl Mixer {
    pub fn apply(&mut self, command: SoundCommand) {
        self.playing = match command {
            SoundCommand::Play(clip) => Some((clip, 0)),
            SoundCommand::Stop => None,
        };
    }

    pub fn is_playing(&self) -> bool {
        self.playing.is_some()
    }

    pub fn next_sample(&mut self) -> i16 {
        let Some((clip, position)) = self.playing.as_mut() else {
            return 0;
        };
        let Some(sample) = clip.0.sample(*position) else {
            self.playing = None;
            return 0;
        };
        *position += 1;
        sample
    }

    pub fn fill(&mut self, out: &mut [i16]) {
        for sample in out {
            *sample = self.next_sample();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static RAMP: Clip = Clip {
        pcm: &[1, 0, 2, 0, 3, 0, 0xFF, 0xFF],
    };

    fn played(mixer: &mut Mixer, n: usize) -> [i16; 8] {
        let mut out = [9; 8];
        mixer.fill(&mut out[..n]);
        out
    }

    #[test]
    fn a_playing_clip_streams_its_samples_then_falls_silent() {
        let mut mixer = Mixer::default();
        mixer.apply(SoundCommand::Play(ClipRef(&RAMP)));
        assert_eq!(played(&mut mixer, 6), [1, 2, 3, -1, 0, 0, 9, 9]);
        assert!(!mixer.is_playing());
    }

    #[test]
    fn stopping_silences_immediately() {
        let mut mixer = Mixer::default();
        mixer.apply(SoundCommand::Play(ClipRef(&RAMP)));
        mixer.next_sample();
        mixer.apply(SoundCommand::Stop);
        assert_eq!(played(&mut mixer, 3), [0, 0, 0, 9, 9, 9, 9, 9]);
    }

    #[test]
    fn playing_again_restarts_from_the_top() {
        let mut mixer = Mixer::default();
        mixer.apply(SoundCommand::Play(ClipRef(&RAMP)));
        mixer.next_sample();
        mixer.next_sample();
        mixer.apply(SoundCommand::Play(ClipRef(&RAMP)));
        assert_eq!(mixer.next_sample(), 1);
    }

    #[test]
    fn an_idle_mixer_is_silent() {
        assert_eq!(Mixer::default().next_sample(), 0);
    }

    #[test]
    fn durations_come_from_the_shared_sample_rate() {
        assert_eq!(RAMP.samples(), 4);
        assert_eq!(RAMP.duration_secs(), 4.0 / 16_000.0);
    }
}
