//! Voice commands without speech recognition: the mic's samples become a
//! short print of how the loudness in three frequency bands, and the hiss,
//! move through a word; a command is whichever recorded print it's nearest.
//! Like Nintendogs, it only has to tell apart the words one player taught.

use libm::{expf, fabsf, logf};

use crate::SAMPLE_RATE;

/// Samples per analysis frame: 16 ms.
pub const FRAME: usize = 256;
/// Every print is stretched or squeezed to this many frames.
pub const PRINT_FRAMES: usize = 24;
/// Loudness bands, split at these frequencies (Hz).
const SPLITS: [f32; BANDS - 1] = [400.0, 1_000.0, 2_000.0, 4_000.0];
const BANDS: usize = 5;
/// Each band's loudness, then the zero-crossing rate.
pub const FEATURES: usize = BANDS + 1;
/// The longest command: about 1.6 s.
pub const MAX_FRAMES: usize = 100;
/// Prints farther than this from every recording aren't a command.
pub const MATCH_DISTANCE: f32 = 60.0;

/// Loudness (natural log of energy) this far over the room's floor is speech...
const SPEECH_OVER_FLOOR: f32 = 2.5;
/// ...and it stays speech until it falls this close to the floor...
const QUIET_OVER_FLOOR: f32 = 1.5;
/// ...for this many frames (about 0.2 s), so the gap before a "p" doesn't
/// split a word.
const END_FRAMES: usize = 12;
/// Shorter than this (about 80 ms) is a click, not a word.
const MIN_FRAMES: usize = 5;
/// Frames kept from before speech is detected, for a soft onset like "f".
const PRE_ROLL: usize = 6;
/// A word's edges are where it rises out of the room to within this of its
/// own peak (natural log of energy: about 20 dB), so a word said softly is
/// cut at the same sounds as the same word said loudly...
const EDGE_UNDER_PEAK: f32 = 4.5;
/// ...but never below this over the floor, which is the room's own hiss.
const EDGE_OVER_FLOOR: f32 = 1.0;
/// How fast the floor creeps up toward a steadily louder room, per frame.
const FLOOR_RISE: f32 = 0.02;
/// Quantization: one natural-log unit of loudness is this many steps.
const STEPS_PER_LOG: f32 = 16.0;
/// How much a gap of one in the zero-crossing rate counts against a frame,
/// against one step of loudness.
const ZCR_WEIGHT: f32 = 0.5;
/// How far apart in time DTW may pair frames.
const WARP_BAND: usize = 6;
/// Distance added per doubling of length between two words.
const LENGTH_PENALTY: f32 = 6.0;

/// One word, `PRINT_FRAMES` frames of `FEATURES`, and how long it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoicePrint {
    frames: [[u8; FEATURES]; PRINT_FRAMES],
    length: u8,
}

impl VoicePrint {
    pub const BYTES: usize = PRINT_FRAMES * FEATURES + 1;

    pub fn to_bytes(&self) -> [u8; Self::BYTES] {
        let mut out = [0; Self::BYTES];
        for (chunk, frame) in out.chunks_exact_mut(FEATURES).zip(&self.frames) {
            chunk.copy_from_slice(frame);
        }
        out[Self::BYTES - 1] = self.length;
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::BYTES || bytes[Self::BYTES - 1] == 0 {
            return None;
        }
        let mut frames = [[0; FEATURES]; PRINT_FRAMES];
        for (frame, chunk) in frames.iter_mut().zip(bytes.chunks_exact(FEATURES)) {
            frame.copy_from_slice(chunk);
        }
        Some(Self {
            frames,
            length: bytes[Self::BYTES - 1],
        })
    }

    /// How unalike two words are: dynamic time warping over the frames, so
    /// a word said faster still lines up, plus a little for length.
    pub fn distance(&self, other: &VoicePrint) -> f32 {
        let mut cost = [[f32::INFINITY; PRINT_FRAMES + 1]; PRINT_FRAMES + 1];
        cost[0][0] = 0.0;
        for i in 1..=PRINT_FRAMES {
            let from = i.saturating_sub(WARP_BAND).max(1);
            let to = (i + WARP_BAND).min(PRINT_FRAMES);
            for j in from..=to {
                let step = frame_distance(&self.frames[i - 1], &other.frames[j - 1]);
                let best = cost[i - 1][j - 1].min(cost[i - 1][j]).min(cost[i][j - 1]);
                cost[i][j] = step + best;
            }
        }
        let warped = cost[PRINT_FRAMES][PRINT_FRAMES] / (2 * PRINT_FRAMES) as f32;
        let ratio = f32::from(self.length) / f32::from(other.length);
        warped + LENGTH_PENALTY * fabsf(logf(ratio)) / core::f32::consts::LN_2
    }
}

fn frame_distance(a: &[u8; FEATURES], b: &[u8; FEATURES]) -> f32 {
    let bands: f32 = (0..FEATURES - 1)
        .map(|k| fabsf(f32::from(a[k]) - f32::from(b[k])))
        .sum();
    let zcr = fabsf(f32::from(a[FEATURES - 1]) - f32::from(b[FEATURES - 1]));
    bands + zcr * ZCR_WEIGHT
}

/// Which of the `known` prints `heard` is, if it's near enough to one.
pub fn recognize<'a, K>(
    heard: &VoicePrint,
    known: impl IntoIterator<Item = (K, &'a VoicePrint)>,
) -> Option<K> {
    known
        .into_iter()
        .map(|(key, print)| (key, print.distance(heard)))
        .filter(|(_, distance)| *distance < MATCH_DISTANCE)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(key, _)| key)
}

/// A one-pole low-pass filter.
#[derive(Clone, Copy, Debug)]
struct LowPass {
    alpha: f32,
    out: f32,
}

impl LowPass {
    fn new(cutoff_hz: f32) -> Self {
        let rate = SAMPLE_RATE as f32;
        Self {
            alpha: 1.0 - expf(-2.0 * core::f32::consts::PI * cutoff_hz / rate),
            out: 0.0,
        }
    }

    fn step(&mut self, x: f32) -> f32 {
        self.out += self.alpha * (x - self.out);
        self.out
    }
}

/// Splits samples into `BANDS` bands at `SPLITS` with two-pole slopes,
/// after taking out any DC offset.
#[derive(Clone, Copy, Debug)]
struct Bands {
    dc: LowPass,
    below: [[LowPass; 2]; BANDS - 1],
}

impl Bands {
    fn new() -> Self {
        Self {
            dc: LowPass::new(30.0),
            below: SPLITS.map(|hz| [LowPass::new(hz); 2]),
        }
    }

    /// The sample with DC removed, and its part in each band.
    fn step(&mut self, sample: i16) -> (f32, [f32; BANDS]) {
        let x = f32::from(sample);
        let x = x - self.dc.step(x);
        let mut parts = [0.0; BANDS];
        let mut lower = 0.0;
        for (part, filters) in parts.iter_mut().zip(&mut self.below) {
            let below = cascade(filters, x);
            *part = below - lower;
            lower = below;
        }
        parts[BANDS - 1] = x - lower;
        (x, parts)
    }
}

fn cascade(filters: &mut [LowPass; 2], x: f32) -> f32 {
    let once = filters[0].step(x);
    filters[1].step(once)
}

/// Listens to the mic: finds where speech starts and stops against the
/// room's noise floor and turns each word into a [`VoicePrint`].
#[derive(Clone, Debug)]
pub struct Listener {
    bands: Bands,
    energy: [f32; BANDS],
    crossings: u32,
    last_sign: bool,
    filled: usize,
    floor: Option<f32>,
    loudness: f32,
    /// Recent frames before speech, oldest first.
    pre_roll: [Frame; PRE_ROLL],
    frames: [Frame; MAX_FRAMES],
    length: usize,
    speaking: bool,
    quiet_run: usize,
    ready: Option<VoicePrint>,
}

impl Default for Listener {
    fn default() -> Self {
        Self::new()
    }
}

impl Listener {
    pub fn new() -> Self {
        Self {
            bands: Bands::new(),
            energy: [0.0; BANDS],
            crossings: 0,
            last_sign: false,
            filled: 0,
            floor: None,
            loudness: 0.0,
            pre_roll: [Frame::QUIET; PRE_ROLL],
            frames: [Frame::QUIET; MAX_FRAMES],
            length: 0,
            speaking: false,
            quiet_run: 0,
            ready: None,
        }
    }

    pub fn push(&mut self, samples: &[i16]) {
        for &sample in samples {
            let (x, parts) = self.bands.step(sample);
            for (energy, part) in self.energy.iter_mut().zip(parts) {
                *energy += part * part;
            }
            let sign = x >= 0.0;
            self.crossings += u32::from(sign != self.last_sign);
            self.last_sign = sign;
            self.filled += 1;
            if self.filled == FRAME {
                self.end_frame();
            }
        }
    }

    /// The last word heard, once.
    pub fn take(&mut self) -> Option<VoicePrint> {
        self.ready.take()
    }

    pub fn speaking(&self) -> bool {
        self.speaking
    }

    /// How loud the room is over its floor, 0 to 1, for a level meter.
    pub fn level(&self) -> f32 {
        let over = self.loudness - self.floor.unwrap_or(self.loudness);
        (over / (2.0 * SPEECH_OVER_FLOOR)).clamp(0.0, 1.0)
    }

    fn end_frame(&mut self) {
        let mut features = [0.0; FEATURES];
        for (feature, energy) in features.iter_mut().zip(self.energy) {
            *feature = logf(energy / FRAME as f32 + 1.0);
        }
        features[FEATURES - 1] = self.crossings as f32 / FRAME as f32;
        let total: f32 = self.energy.iter().sum();
        self.loudness = logf(total / FRAME as f32 + 1.0);
        let frame = Frame {
            features,
            loudness: self.loudness,
        };
        self.energy = [0.0; BANDS];
        self.crossings = 0;
        self.filled = 0;
        let floor = *self.floor.get_or_insert(self.loudness);
        let over = self.loudness - floor;

        if !self.speaking {
            self.floor = Some((floor + FLOOR_RISE).min(self.loudness));
            if over > SPEECH_OVER_FLOOR {
                self.speaking = true;
                self.quiet_run = 0;
                self.frames[..PRE_ROLL].copy_from_slice(&self.pre_roll);
                self.length = PRE_ROLL;
                self.record(frame);
            } else {
                self.pre_roll.rotate_left(1);
                self.pre_roll[PRE_ROLL - 1] = frame;
            }
            return;
        }
        self.record(frame);
        self.quiet_run = if over < QUIET_OVER_FLOOR {
            self.quiet_run + 1
        } else {
            0
        };
        if self.quiet_run >= END_FRAMES || self.length == MAX_FRAMES {
            self.finish();
        }
    }

    fn record(&mut self, frame: Frame) {
        if self.length < MAX_FRAMES {
            self.frames[self.length] = frame;
            self.length += 1;
        }
    }

    fn finish(&mut self) {
        self.speaking = false;
        let heard = &self.frames[..self.length];
        self.length = 0;
        self.quiet_run = 0;
        let peak = heard.iter().map(|f| f.loudness).fold(f32::MIN, f32::max);
        let floor = self.floor.unwrap_or(peak);
        let edge = (peak - EDGE_UNDER_PEAK).max(floor + EDGE_OVER_FLOOR);
        let first = heard.iter().position(|f| f.loudness >= edge);
        let last = heard.iter().rposition(|f| f.loudness >= edge);
        if let (Some(first), Some(last)) = (first, last)
            && last + 1 - first >= MIN_FRAMES
        {
            self.ready = Some(print(&heard[first..=last]));
        }
    }
}

/// One analysis frame: its features, and its overall loudness.
#[derive(Clone, Copy, Debug)]
struct Frame {
    features: [f32; FEATURES],
    loudness: f32,
}

impl Frame {
    const QUIET: Frame = Frame {
        features: [0.0; FEATURES],
        loudness: 0.0,
    };
}

/// Stretches `frames` to `PRINT_FRAMES`, takes out the overall loudness so a
/// word said softly matches the same word said loudly, and quantizes.
fn print(frames: &[Frame]) -> VoicePrint {
    let bands = BANDS as f32;
    let mean = frames
        .iter()
        .map(|frame| frame.features[..BANDS].iter().sum::<f32>() / bands)
        .sum::<f32>()
        / frames.len() as f32;
    let mut out = [[0; FEATURES]; PRINT_FRAMES];
    let last = (frames.len() - 1) as f32;
    for (i, slot) in out.iter_mut().enumerate() {
        let at = last * i as f32 / (PRINT_FRAMES - 1) as f32;
        let (below, t) = (at as usize, at - (at as usize) as f32);
        let above = (below + 1).min(frames.len() - 1);
        for (k, out) in slot.iter_mut().enumerate() {
            let value = frames[below].features[k] * (1.0 - t) + frames[above].features[k] * t;
            *out = if k == FEATURES - 1 {
                quantize(value * 255.0)
            } else {
                quantize((value - mean) * STEPS_PER_LOG + 128.0)
            };
        }
    }
    VoicePrint {
        frames: out,
        length: frames.len().min(usize::from(u8::MAX)) as u8,
    }
}

fn quantize(value: f32) -> u8 {
    value.clamp(0.0, 255.0) as u8
}
