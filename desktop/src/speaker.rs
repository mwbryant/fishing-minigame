//! The desktop's stand-in for the board's speaker: the same `orbit_audio`
//! mixer, played through the default output device at the board's rate.

use orbit_audio::{Mixer, SAMPLE_RATE, SoundCommand};
use rtrb::{Producer, RingBuffer};
use tinyaudio::prelude::*;

/// About 1/15 s of audio kept queued ahead of the sound card.
const TARGET_FILL: usize = SAMPLE_RATE as usize / 15;

pub struct Speaker {
    mixer: Mixer,
    output: Option<(OutputDevice, Producer<i16>)>,
    mix: Vec<i16>,
}

impl Speaker {
    /// Plays silently (with a warning) if no output device opens.
    pub fn open() -> Self {
        let (producer, mut consumer) = RingBuffer::<i16>::new(SAMPLE_RATE as usize / 4);
        let params = OutputDeviceParameters {
            channels_count: 1,
            sample_rate: SAMPLE_RATE as usize,
            channel_sample_count: 512,
        };
        let device = run_output_device(params, move |data: &mut [f32]| {
            for out in data.iter_mut() {
                *out = consumer.pop().map_or(0.0, |s| f32::from(s) / 32768.0);
            }
        });
        let output = match device {
            Ok(device) => Some((device, producer)),
            Err(err) => {
                eprintln!("no audio output ({err}); playing silently");
                None
            }
        };
        Self {
            mixer: Mixer::default(),
            output,
            mix: Vec::new(),
        }
    }

    pub fn apply(&mut self, command: SoundCommand) {
        self.mixer.apply(command);
    }

    /// Tops the device's queue back up; call once per frame.
    pub fn pump(&mut self) {
        let Some((_, producer)) = self.output.as_mut() else {
            return;
        };
        let queued = producer.buffer().capacity() - producer.slots();
        self.mix.resize(TARGET_FILL.saturating_sub(queued), 0);
        self.mixer.fill(&mut self.mix);
        for &sample in &self.mix {
            if producer.push(sample).is_err() {
                break;
            }
        }
    }
}
