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
