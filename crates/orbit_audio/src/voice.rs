//! Voice commands without speech recognition: the mic's samples become a
//! short print of how a word's spectrum (seven bands, each against the
//! frame's own loudness) and its loudness move through the word; a command
//! is whichever recorded print it's nearest. Like Nintendogs, it only has to
//! tell apart the words one player taught.

use libm::{cosf, expf, fabsf, logf, sinf};

use crate::SAMPLE_RATE;

/// Samples per analysis frame: 16 ms.
pub const FRAME: usize = 256;
/// Every print is stretched or squeezed to this many frames.
pub const PRINT_FRAMES: usize = 24;
/// Band-pass filters at these centers (Hz), spaced as hearing spaces pitch.
const CENTERS: [f32; BANDS] = [300.0, 500.0, 800.0, 1_250.0, 2_000.0, 3_150.0, 5_000.0];
const BANDS: usize = 7;
/// How narrow each band is.
const BAND_Q: f32 = 1.4;
/// Each band against the frame's loudness, then the loudness against the
/// word's.
pub const FEATURES: usize = BANDS + 1;
/// The longest command: about 1.6 s.
pub const MAX_FRAMES: usize = 100;
/// Prints farther than this from every recording aren't a command. Measured
/// on spoken words with the perturbations in this crate's tests: the same
/// word said faster, slower, higher, echoing or softly comes in under about
/// 29 nine times in ten; a different word rarely under 38.
pub const MATCH_DISTANCE: f32 = 34.0;
/// ...and the nearest has to beat the next nearest by this much, so a word
/// that's about as like two taught words as each other is neither.
pub const MATCH_MARGIN: f32 = 0.85;

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
/// How much a step of the loudness contour counts against a frame, against
/// one step of a band's shape.
const LOUDNESS_WEIGHT: f32 = 1.0;
/// Two takes of a word while teaching it may be this much farther apart
/// than a command may be from it.
const TAKE_SLACK: f32 = 1.3;
/// How far apart in time DTW may pair frames.
const WARP_BAND: usize = 9;

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
    /// a word said faster or slower still lines up.
    pub fn distance(&self, other: &VoicePrint) -> f32 {
        warp(self, other)[PRINT_FRAMES][PRINT_FRAMES] / (2 * PRINT_FRAMES) as f32
    }

    /// Whether `other` is another take of the same word, as teaching asks.
    pub fn alike(&self, other: &VoicePrint) -> bool {
        self.distance(other) < MATCH_DISTANCE * TAKE_SLACK
    }

    /// This print moved `weight` (0 to 1) of the way toward `other`, frame
    /// by frame along their warp, so it settles between how a word is said
    /// on different days.
    pub fn blend(&self, other: &VoicePrint, weight: f32) -> VoicePrint {
        let cost = warp(self, other);
        let (mut i, mut j) = (PRINT_FRAMES, PRINT_FRAMES);
        let mut sums = [[0.0f32; FEATURES]; PRINT_FRAMES];
        let mut counts = [0u32; PRINT_FRAMES];
        while i > 0 && j > 0 {
            for (sum, value) in sums[i - 1].iter_mut().zip(&other.frames[j - 1]) {
                *sum += f32::from(*value);
            }
            counts[i - 1] += 1;
            let diagonal = cost[i - 1][j - 1];
            let up = cost[i - 1][j];
            let left = cost[i][j - 1];
            if diagonal <= up && diagonal <= left {
                (i, j) = (i - 1, j - 1);
            } else if up <= left {
                i -= 1;
            } else {
                j -= 1;
            }
        }
        let mut blended = *self;
        for ((frame, sum), count) in blended.frames.iter_mut().zip(&sums).zip(counts) {
            if count == 0 {
                continue;
            }
            for (value, total) in frame.iter_mut().zip(sum) {
                let toward = total / count as f32;
                let mine = f32::from(*value);
                *value = quantize(mine + (toward - mine) * weight.clamp(0.0, 1.0));
            }
        }
        blended
    }
}

/// The DTW cost table between two prints.
fn warp(a: &VoicePrint, b: &VoicePrint) -> [[f32; PRINT_FRAMES + 1]; PRINT_FRAMES + 1] {
    const N: usize = PRINT_FRAMES;
    let mut cost = [[f32::INFINITY; N + 1]; N + 1];
    cost[0][0] = 0.0;
    for i in 1..=N {
        let from = i.saturating_sub(WARP_BAND).max(1);
        let to = (i + WARP_BAND).min(N);
        for j in from..=to {
            let step = frame_distance(&a.frames[i - 1], &b.frames[j - 1]);
            let best = cost[i - 1][j - 1].min(cost[i - 1][j]).min(cost[i][j - 1]);
            cost[i][j] = step + best;
        }
    }
    cost
}

fn frame_distance(a: &[u8; FEATURES], b: &[u8; FEATURES]) -> f32 {
    let shape: f32 = (0..BANDS)
        .map(|k| fabsf(f32::from(a[k]) - f32::from(b[k])))
        .sum();
    shape + fabsf(f32::from(a[BANDS]) - f32::from(b[BANDS])) * LOUDNESS_WEIGHT
}

/// Which of the `known` prints `heard` is: the nearest, if it's near
/// enough and clearly nearer than the next one.
pub fn recognize<'a, K>(
    heard: &VoicePrint,
    known: impl IntoIterator<Item = (K, &'a VoicePrint)>,
) -> Option<K> {
    let mut best: Option<(K, f32)> = None;
    let mut runner_up = f32::INFINITY;
    for (key, print) in known {
        let distance = print.distance(heard);
        match &best {
            Some((_, nearest)) if distance >= *nearest => runner_up = runner_up.min(distance),
            _ => {
                if let Some((_, nearest)) = best.replace((key, distance)) {
                    runner_up = runner_up.min(nearest);
                }
            }
        }
    }
    let (key, nearest) = best?;
    (nearest < MATCH_DISTANCE && nearest < runner_up * MATCH_MARGIN).then_some(key)
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

/// A band-pass biquad (the RBJ cookbook's, 0 dB at its center).
#[derive(Clone, Copy, Debug)]
struct BandPass {
    b0: f32,
    a1: f32,
    a2: f32,
    x: [f32; 2],
    y: [f32; 2],
}

impl BandPass {
    fn new(center_hz: f32) -> Self {
        let w0 = 2.0 * core::f32::consts::PI * center_hz / SAMPLE_RATE as f32;
        let alpha = sinf(w0) / (2.0 * BAND_Q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            a1: -2.0 * cosf(w0) / a0,
            a2: (1.0 - alpha) / a0,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn step(&mut self, x: f32) -> f32 {
        let y = self.b0 * (x - self.x[1]) - self.a1 * self.y[0] - self.a2 * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// Splits samples into the `CENTERS` bands, after taking out any DC offset.
#[derive(Clone, Copy, Debug)]
struct Bands {
    dc: LowPass,
    filters: [BandPass; BANDS],
}

impl Bands {
    fn new() -> Self {
        Self {
            dc: LowPass::new(30.0),
            filters: CENTERS.map(BandPass::new),
        }
    }

    /// The sample's part in each band.
    fn step(&mut self, sample: i16) -> [f32; BANDS] {
        let x = f32::from(sample);
        let x = x - self.dc.step(x);
        let mut parts = [0.0; BANDS];
        for (part, filter) in parts.iter_mut().zip(&mut self.filters) {
            *part = filter.step(x);
        }
        parts
    }
}

/// Listens to the mic: finds where speech starts and stops against the
/// room's noise floor and turns each word into a [`VoicePrint`].
#[derive(Clone, Debug)]
pub struct Listener {
    bands: Bands,
    energy: [f32; BANDS],
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
            let parts = self.bands.step(sample);
            for (energy, part) in self.energy.iter_mut().zip(parts) {
                *energy += part * part;
            }
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
        let total: f32 = self.energy.iter().sum();
        self.loudness = logf(total / FRAME as f32 + 1.0);
        let frame = Frame {
            features,
            loudness: self.loudness,
        };
        self.energy = [0.0; BANDS];
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

/// Stretches `frames` to `PRINT_FRAMES` as each frame's spectral shape (each
/// band against the frame's own loudness) and the loudness contour (each
/// frame against the word's), so a word said softly, or farther off, matches
/// the same word said loudly and close; then quantizes.
fn print(frames: &[Frame]) -> VoicePrint {
    let word = frames.iter().map(|frame| frame.loudness).sum::<f32>() / frames.len() as f32;
    let described = |frame: &Frame| {
        let bands = &frame.features[..BANDS];
        let level = bands.iter().sum::<f32>() / BANDS as f32;
        let mut out = [0.0; FEATURES];
        for (shape, band) in out.iter_mut().zip(bands) {
            *shape = band - level;
        }
        out[BANDS] = frame.loudness - word;
        out
    };
    let mut out = [[0; FEATURES]; PRINT_FRAMES];
    let last = (frames.len() - 1) as f32;
    for (i, slot) in out.iter_mut().enumerate() {
        let at = last * i as f32 / (PRINT_FRAMES - 1) as f32;
        let (below, t) = (at as usize, at - (at as usize) as f32);
        let above = (below + 1).min(frames.len() - 1);
        let (low, high) = (described(&frames[below]), described(&frames[above]));
        for (k, value) in slot.iter_mut().enumerate() {
            let blended = low[k] * (1.0 - t) + high[k] * t;
            *value = quantize(blended * STEPS_PER_LOG + 128.0);
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
