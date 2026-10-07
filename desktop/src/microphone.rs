//! The desktop's stand-in for the board's mics: the default input device,
//! mixed to mono and resampled to the board's 16 kHz, for apps that listen.
//! On macOS the terminal asks for microphone access the first time.

use cpal::{
    SampleFormat, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use orbit_audio::SAMPLE_RATE;
use orbit_core::Mic;
use rtrb::{Consumer, Producer, RingBuffer};

pub struct Microphone {
    /// Capture stops when the stream is dropped.
    #[expect(dead_code, reason = "held so the capture keeps running")]
    stream: Stream,
    heard: Consumer<i16>,
}

/// Turns the device's frames into the board's: mono, at `SAMPLE_RATE`.
struct Resampler {
    channels: usize,
    step: f64,
    phase: f64,
    sum: f32,
    count: u32,
    out: Producer<i16>,
}

impl Resampler {
    fn push(&mut self, frame: &[f32]) {
        let mono = frame.iter().sum::<f32>() / self.channels as f32;
        self.sum += mono;
        self.count += 1;
        self.phase += self.step;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            let sample = (self.sum / self.count as f32).clamp(-1.0, 1.0) * 32767.0;
            self.out.push(sample as i16).ok();
            self.sum = 0.0;
            self.count = 0;
        }
    }
}

impl Microphone {
    /// The default input, or `None` (with a note) if there isn't one.
    pub fn open() -> Option<Self> {
        let device = cpal::default_host().default_input_device()?;
        let config = device
            .default_input_config()
            .inspect_err(|err| eprintln!("no microphone config ({err}); no voice commands"))
            .ok()?;
        let (producer, heard) = RingBuffer::<i16>::new(SAMPLE_RATE as usize / 2);
        let channels = usize::from(config.channels());
        let mut resampler = Resampler {
            channels,
            step: f64::from(SAMPLE_RATE) / f64::from(config.sample_rate().0),
            phase: 0.0,
            sum: 0.0,
            count: 0,
            out: producer,
        };
        let failed = |err| eprintln!("microphone error: {err}");
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    data.chunks_exact(channels)
                        .for_each(|frame| resampler.push(frame));
                },
                failed,
                None,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let mut frame = [0.0f32; 8];
                    for raw in data.chunks_exact(channels) {
                        for (out, sample) in frame.iter_mut().zip(raw) {
                            *out = f32::from(*sample) / 32768.0;
                        }
                        resampler.push(&frame[..channels.min(8)]);
                    }
                },
                failed,
                None,
            ),
            other => {
                eprintln!("microphone sample format {other:?} isn't handled; no voice commands");
                return None;
            }
        }
        .inspect_err(|err| eprintln!("microphone didn't open ({err}); no voice commands"))
        .ok()?;
        stream
            .play()
            .inspect_err(|err| eprintln!("microphone didn't start ({err})"))
            .ok()?;
        println!("listening on the default microphone");
        Some(Self { stream, heard })
    }

    /// Everything heard since the last call, into `mic` (or dropped, for an
    /// app that isn't listening).
    pub fn drain(&mut self, mut mic: Option<&mut Mic>) {
        let mut chunk = [0i16; 512];
        loop {
            let count = self.heard.slots().min(chunk.len());
            if count == 0 {
                return;
            }
            for slot in chunk.iter_mut().take(count) {
                *slot = self.heard.pop().unwrap_or(0);
            }
            if let Some(mic) = mic.as_deref_mut() {
                mic.push(&chunk[..count]);
            }
        }
    }
}
