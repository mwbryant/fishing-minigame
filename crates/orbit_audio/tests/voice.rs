use orbit_audio::{
    SAMPLE_RATE,
    voice::{Listener, VoicePrint, recognize},
};

const WORDS: [&str; 5] = ["spin", "jump", "splash", "bubbles", "flip"];
const SPEAKERS: [&str; 2] = ["daniel", "fred"];

fn clip(name: &str) -> Vec<i16> {
    let path = format!("{}/tests/voices/{name}.pcm", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path)
        .unwrap_or_else(|err| panic!("{path}: {err}"))
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

/// A quiet room: low, steady hiss.
struct Room(u32);

impl Room {
    fn hiss(&mut self, amplitude: f32) -> i16 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (((self.0 >> 16) as f32 / 32_768.0 - 1.0) * amplitude) as i16
    }

    fn silence(&mut self, secs: f32) -> Vec<i16> {
        (0..(secs * SAMPLE_RATE as f32) as usize)
            .map(|_| self.hiss(40.0))
            .collect()
    }
}

/// Plays `sound` at `volume` into the listener with room noise around it,
/// in the small chunks a mic driver delivers, and collects what it hears.
fn hear(listener: &mut Listener, sound: &[i16], volume: f32) -> Vec<VoicePrint> {
    let mut room = Room(7);
    let mut samples = room.silence(0.6);
    samples.extend(
        sound
            .iter()
            .map(|&s| (f32::from(s) * volume) as i16 + room.hiss(40.0)),
    );
    samples.extend(room.silence(0.8));
    let mut heard = Vec::new();
    for chunk in samples.chunks(160) {
        listener.push(chunk);
        heard.extend(listener.take());
    }
    heard
}

fn print_of(sound: &[i16], volume: f32) -> VoicePrint {
    let heard = hear(&mut Listener::new(), sound, volume);
    assert_eq!(heard.len(), 1, "one word, one print");
    heard[0]
}

fn recordings(speaker: &str) -> Vec<VoicePrint> {
    WORDS
        .iter()
        .map(|word| print_of(&clip(&format!("{speaker}-{word}-170")), 1.0))
        .collect()
}

#[test]
fn a_word_said_into_a_quiet_room_comes_out_as_one_print_and_quiet_as_none() {
    let mut listener = Listener::new();
    assert!(hear(&mut listener, &[], 1.0).is_empty(), "room noise alone");
    assert_eq!(hear(&mut listener, &clip("daniel-spin-170"), 1.0).len(), 1);
}

#[test]
fn a_print_survives_the_trip_through_bytes() {
    let print = print_of(&clip("fred-splash-170"), 1.0);
    assert_eq!(VoicePrint::from_bytes(&print.to_bytes()), Some(print));
    assert_eq!(VoicePrint::from_bytes(&[0; 3]), None);
}

#[test]
fn each_speaker_saying_a_word_faster_and_softer_is_recognized_as_that_word() {
    for speaker in SPEAKERS {
        let known = recordings(speaker);
        for (expected, word) in WORDS.iter().enumerate() {
            let command = print_of(&clip(&format!("{speaker}-{word}-240")), 0.35);
            let distances: Vec<f32> = known.iter().map(|k| k.distance(&command)).collect();
            assert_eq!(
                recognize(&command, known.iter().enumerate()),
                Some(expected),
                "{speaker} said {word}: distances {distances:?}"
            );
        }
    }
}

#[test]
fn a_clap_or_a_whistle_is_not_any_command() {
    let known = recordings("daniel");
    let rate = SAMPLE_RATE as f32;
    let clap: Vec<i16> = {
        let mut room = Room(99);
        (0..(0.12 * rate) as usize)
            .map(|i| room.hiss(9_000.0 * (-(i as f32) / (0.02 * rate)).exp()))
            .collect()
    };
    let whistle: Vec<i16> = (0..(0.6 * rate) as usize)
        .map(|i| (6_000.0 * (i as f32 * 2.0 * std::f32::consts::PI * 1_800.0 / rate).sin()) as i16)
        .collect();
    for (name, sound) in [("clap", clap), ("whistle", whistle)] {
        for print in hear(&mut Listener::new(), &sound, 1.0) {
            assert_eq!(
                recognize(&print, known.iter().enumerate()),
                None,
                "a {name} sounded like a command"
            );
        }
    }
}

/// Resamples by `factor` (above 1 is faster and higher), like a word said
/// quicker in a higher voice.
fn shifted(sound: &[i16], factor: f32) -> Vec<i16> {
    let length = (sound.len() as f32 / factor) as usize;
    (0..length)
        .map(|i| {
            let at = i as f32 * factor;
            let below = at as usize;
            let t = at - below as f32;
            let a = f32::from(sound[below.min(sound.len() - 1)]);
            let b = f32::from(sound[(below + 1).min(sound.len() - 1)]);
            (a + (b - a) * t) as i16
        })
        .collect()
}

/// A room's echo: the word again 40 ms later at 40%.
fn echoed(sound: &[i16]) -> Vec<i16> {
    let delay = (0.04 * SAMPLE_RATE as f32) as usize;
    let mut out: Vec<i16> = sound.to_vec();
    out.extend(std::iter::repeat_n(0, delay));
    for (sample, &early) in out[delay..].iter_mut().zip(sound) {
        let late = f32::from(early) * 0.4;
        *sample = (f32::from(*sample) + late).clamp(-32768.0, 32767.0) as i16;
    }
    out
}

/// Plays `sound` into a noisier room than `hear` does.
fn heard_in(sound: &[i16], volume: f32, hiss: f32) -> Vec<VoicePrint> {
    let mut listener = Listener::new();
    let mut room = Room(3);
    let mut samples: Vec<i16> = (0..9600).map(|_| room.hiss(hiss)).collect();
    samples.extend(
        sound
            .iter()
            .map(|&s| (f32::from(s) * volume) as i16 + room.hiss(hiss)),
    );
    samples.extend((0..12800).map(|_| room.hiss(hiss)));
    let mut heard = Vec::new();
    for chunk in samples.chunks(160) {
        listener.push(chunk);
        heard.extend(listener.take());
    }
    heard
}

/// A word heard the ways one speaker's word varies day to day: said faster
/// and higher, slower and lower, echoing, and quietly.
fn variations(fast: &[i16], slow: &[i16]) -> Vec<Vec<VoicePrint>> {
    [
        (fast.to_vec(), 0.35, 40.0),
        (shifted(fast, 1.12), 0.35, 40.0),
        (shifted(slow, 0.9), 0.5, 40.0),
        (echoed(fast), 0.35, 40.0),
        (fast.to_vec(), 0.12, 60.0),
    ]
    .into_iter()
    .map(|(sound, volume, hiss)| heard_in(&sound, volume, hiss))
    .collect()
}

#[test]
fn a_word_said_faster_slower_higher_echoing_or_quietly_is_still_recognized() {
    let (mut right, mut tried) = (0, 0);
    let mut misses = Vec::new();
    for speaker in SPEAKERS {
        let known = recordings(speaker);
        for (expected, word) in WORDS.iter().enumerate() {
            let fast = clip(&format!("{speaker}-{word}-240"));
            let slow = clip(&format!("{speaker}-{word}-170"));
            for (variant, heard) in variations(&fast, &slow).into_iter().enumerate() {
                tried += 1;
                let recognized = heard.len() == 1
                    && recognize(&heard[0], known.iter().enumerate()) == Some(expected);
                if recognized {
                    right += 1;
                } else {
                    misses.push(format!("{speaker} {word} #{variant}"));
                }
            }
        }
    }
    assert!(
        right * 10 >= tried * 9,
        "{right}/{tried}; missed {misses:?}"
    );
}

#[test]
fn a_word_nobody_taught_is_no_command() {
    let (mut ignored, mut tried) = (0, 0);
    let mut fooled = Vec::new();
    for speaker in SPEAKERS {
        let known = recordings(speaker);
        // Teach two words, then say each of the other three every way.
        for taught in [[0, 1], [2, 3], [1, 4], [0, 3]] {
            for (said, word) in WORDS.iter().enumerate() {
                if taught.contains(&said) {
                    continue;
                }
                let fast = clip(&format!("{speaker}-{word}-240"));
                let slow = clip(&format!("{speaker}-{word}-170"));
                for heard in variations(&fast, &slow).into_iter().flatten() {
                    tried += 1;
                    let answer = recognize(&heard, taught.iter().map(|&t| (t, &known[t])));
                    match answer {
                        None => ignored += 1,
                        Some(t) => fooled.push(format!("{speaker} {word} as {}", WORDS[t])),
                    }
                }
            }
        }
    }
    assert!(
        ignored * 10 >= tried * 9,
        "{ignored}/{tried}; fooled by {fooled:?}"
    );
}

#[test]
fn blending_two_takes_settles_between_them_and_both_still_match() {
    let first = print_of(&clip("fred-splash-170"), 1.0);
    let second = print_of(&clip("fred-splash-240"), 0.35);
    let blended = first.blend(&second, 0.5);
    assert!(blended.distance(&second) < first.distance(&second));
    assert!(blended.distance(&first) < second.distance(&first));
    for take in [first, second] {
        assert_eq!(recognize(&take, [((), &blended)]), Some(()));
    }
    assert_eq!(first.blend(&second, 0.0), first, "no weight, no change");
}

#[test]
fn two_takes_while_teaching_may_differ_a_little_more_than_a_command() {
    let first = print_of(&clip("daniel-splash-170"), 1.0);
    let again = print_of(&clip("daniel-splash-240"), 0.35);
    assert!(first.alike(&again));
    assert!(!first.alike(&print_of(&clip("daniel-bubbles-170"), 1.0)));
}
