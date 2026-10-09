//! Scripted, headless takes for the trailer: every update of a take is saved
//! as a PNG (60 a second of game time) next to a WAV of what the board's
//! speaker would have played. `trailer/build.py` cuts them together.

use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use angler_game::{
    Phase, Session,
    cast::{self, CHARGE_SECS, Flight},
    testing as angler, ui,
};
use bevy::prelude::*;
use orbit_audio::{Mixer, SAMPLE_RATE};
use orbit_core::{Frame, SaveBlob, SoundQueue, Touch, testing as core_testing};
use orbit_gfx::Presenter;

use crate::{
    BAND_BYTES, Game,
    panel::RamPanel,
    snapshots::{battery, pet, reel},
};

/// Each take's name and the script that films it.
const TAKES: [(&str, fn(&Path)); 12] = [
    ("opening", opening),
    ("fish_cast", fish),
    ("tank", tank),
    ("train", train),
    ("walk", walk),
    ("compete", compete),
    ("hats", hats),
    ("bees", bees),
    ("skate_grind", skate_grind),
    ("skate_halfpipe", skate_halfpipe),
    ("spider", spider),
    ("guten", guten),
];

/// Films every take into `dir/<take>/`, or only the takes named in `only`.
/// `fish_cast` also films `fish_catch`, and `compete` films
/// `compete_results`.
pub fn run(dir: &Path, only: &[String]) {
    for (name, film) in TAKES {
        if only.is_empty() || only.iter().any(|wanted| wanted == name) {
            film(dir);
        }
    }
}

/// The camera running inside the app: `roll` saves each frame after the
/// game has drawn it.
#[derive(Resource)]
struct Film {
    dir: PathBuf,
    panel: RamPanel,
    presenter: Presenter<Vec<u8>>,
    mixer: Mixer,
    audio: Vec<i16>,
    frames: u64,
}

fn roll(film: Option<ResMut<Film>>, frame: Res<Frame>, sounds: Option<ResMut<SoundQueue>>) {
    let Some(mut film) = film else {
        return;
    };
    let film = &mut *film;
    if let Some(mut sounds) = sounds {
        for command in sounds.drain() {
            film.mixer.apply(command);
        }
    }
    let samples = per_frame(film.frames);
    film.presenter.present(&frame.shapes, &mut film.panel);
    film.panel
        .save_png(&film.dir.join(format!("{:05}.png", film.frames)));
    film.audio
        .extend((0..samples).map(|_| film.mixer.next_sample()));
    film.frames += 1;
}

/// Speaker samples in the `frame`th 60 Hz update, so the audio never drifts.
fn per_frame(frame: u64) -> usize {
    let rate = u64::from(SAMPLE_RATE);
    ((frame + 1) * rate / 60 - frame * rate / 60) as usize
}

/// Lets `app` be filmed; call once, before its first take.
fn rig(app: &mut App) {
    app.add_systems(Last, roll);
}

/// Drops the sounds asked for since the last take, so a take doesn't open
/// on one played long before it.
fn hush(app: &mut App) {
    if let Some(mut sounds) = app.world_mut().get_resource_mut::<SoundQueue>() {
        sounds.drain().for_each(drop);
    }
}

/// Starts filming `name` from the next update. Sounds still queued play as
/// it starts.
fn action(app: &mut App, dir: &Path, name: &str, game: Game) {
    let dir = dir.join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear old take");
    }
    fs::create_dir_all(&dir).expect("create take dir");
    let mut presenter = Presenter::new(vec![0u8; BAND_BYTES], game.background());
    presenter.invalidate();
    app.insert_resource(Film {
        dir,
        panel: RamPanel::new(),
        presenter,
        mixer: Mixer::default(),
        audio: Vec::new(),
        frames: 0,
    });
}

/// Ends the take and writes its sound.
fn cut(app: &mut App) {
    let film = app
        .world_mut()
        .remove_resource::<Film>()
        .expect("a take is rolling");
    write_wav(&film.dir.join("audio.wav"), &film.audio);
    println!(
        "filmed {} ({:.1} s)",
        film.dir.display(),
        film.frames as f32 / 60.0
    );
}

/// Writes 16-bit mono PCM at `SAMPLE_RATE` as a WAV file.
fn write_wav(path: &Path, samples: &[i16]) {
    let data = (samples.len() * 2) as u32;
    let mut out = BufWriter::new(File::create(path).expect("create wav"));
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    header.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data.to_le_bytes());
    out.write_all(&header).expect("wav header");
    for sample in samples {
        out.write_all(&sample.to_le_bytes()).expect("wav data");
    }
}

/// Holds a finger at `at` for `frames` updates.
fn hold_touch(app: &mut App, at: Vec2, frames: u32) {
    app.world_mut().resource_mut::<Touch>().set(Some(at));
    core_testing::tick(app, frames);
    app.world_mut().resource_mut::<Touch>().set(None);
}

/// A finger dragged up the glass, the stroke tricks are taught with.
fn swipe_up(app: &mut App) {
    for i in 0..=10 {
        app.world_mut()
            .resource_mut::<Touch>()
            .set(Some(Vec2::new(233.0, 330.0 - i as f32 * 12.0)));
        app.update();
    }
    app.world_mut().resource_mut::<Touch>().set(None);
}

/// `tank` as the record a save would put in storage.
fn saved(tank: &pals::Pals) -> Option<Vec<u8>> {
    let mut blob = SaveBlob::from_storage(None);
    pals::save::store(&mut blob, tank);
    blob.take_pending()
}

/// The player walks up to their house and stands at the door. Also writes
/// `focus.txt`: where they stand, for the trailer's zoom.
fn opening(dir: &Path) {
    use pals_game::{
        Place,
        town::{Town, TownPlugin},
    };
    let mut tank = pals::Pals::default();
    tank.earn(135);
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(TownPlugin);
        },
        saved(&tank),
    );
    rig(&mut app);
    battery(&mut app, 76);
    hold_touch(&mut app, Vec2::new(320.0, 300.0), 90);
    action(&mut app, dir, "opening", Game::Town);
    core_testing::tick(&mut app, 20);
    let (_, door) = pals_game::town::map::door(Place::Home);
    core_testing::tap(&mut app, door);
    core_testing::tick(&mut app, 60 * 5);
    let at = app.world().resource::<Town>().at();
    cut(&mut app);
    fs::write(
        dir.join("opening/focus.txt"),
        format!("{} {}\n", at.x, at.y),
    )
    .expect("write focus");
}

/// A cast, then (skipping the wait) the bite, the fight and the catch.
fn fish(dir: &Path) {
    let mut app = angler::headless_app();
    rig(&mut app);
    battery(&mut app, 76);
    action(&mut app, dir, "fish_cast", Game::Angler);
    angler::tick(&mut app, 12);
    angler::click_action(&mut app);
    angler::set_tilt(&mut app, -12.0, 0.0);
    let dots_end = Flight::launch(cast::elevation_deg(-12.0), 1.0).landing_x();
    app.world_mut().resource_mut::<Session>().spot_x = dots_end;
    angler::hold_action(&mut app, (CHARGE_SECS * 60.0) as u32 + 1);
    angler::release_action(&mut app);
    angler::run_until(&mut app, 600, |p| matches!(p, Phase::Waiting { .. }));
    angler::tick(&mut app, 30);
    cut(&mut app);
    angler::run_until(&mut app, 60 * 15, |p| matches!(p, Phase::Bite { .. }));
    action(&mut app, dir, "fish_catch", Game::Angler);
    angler::run_until(&mut app, 120, |p| matches!(p, Phase::Reel { .. }));
    reel(&mut app, 60 * 60);
    angler::tick(&mut app, 150);
    cut(&mut app);
}

fn trained_tank() -> pals::Pals {
    let mut tank = pals::Pals::default();
    tank.earn(500);
    for hat in [2, 6] {
        tank.buy_hat(hat);
    }
    for (name, species, hat, walked) in [
        ("GILLY", 3, Some(6), 520.0),
        ("BUBBA", 1, None, 80.0),
        ("KOI", 7, Some(2), 1500.0),
    ] {
        let mut fish = pet(name, species);
        fish.hat = hat;
        fish.walk(walked);
        fish.teach(
            0,
            pals::Trigger {
                gesture: Some(pals::Gesture::SwipeUp),
                voice: None,
            },
        );
        fish.tricks[0].learned = 45;
        tank.keep(fish).ok();
    }
    tank
}

/// The three fish swimming in the tank at home.
fn tank(dir: &Path) {
    use pals_game::home::HomePlugin;
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(HomePlugin);
        },
        saved(&trained_tank()),
    );
    rig(&mut app);
    battery(&mut app, 76);
    action(&mut app, dir, "tank", Game::Home);
    core_testing::tick(&mut app, 60 * 4);
    cut(&mut app);
}

/// Practising a trick: the stroke, the fish trying it, the praise.
fn train(dir: &Path) {
    use pals_game::home::{Home, HomePlugin};
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(HomePlugin);
        },
        saved(&trained_tank()),
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::tick(&mut app, 40);
    core_testing::click_action(&mut app);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 12.0, 0.0);
    core_testing::tick(&mut app, 2);
    core_testing::click_action(&mut app);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 250);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 2);
    core_testing::click_action(&mut app);
    hush(&mut app);
    action(&mut app, dir, "train", Game::Home);
    core_testing::tick(&mut app, 20);
    app.world_mut().resource_mut::<Home>().rig_next_try(true);
    swipe_up(&mut app);
    core_testing::tick(&mut app, 110);
    core_testing::tap(&mut app, Vec2::new(233.0, 230.0));
    core_testing::tick(&mut app, 80);
    cut(&mut app);
}

/// A mock walk in the park, sped up to x10 partway.
fn walk(dir: &Path) {
    use pals_game::walk::{WalkPlugin, ui};
    let mut tank = pals::Pals::default();
    tank.mock_walk = true;
    tank.keep({
        let mut fish = pet("GILLY", 3);
        fish.walk(380.0);
        fish
    })
    .ok();
    tank.keep(pet("BUBBA", 1)).ok();
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(WalkPlugin);
        },
        saved(&tank),
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    action(&mut app, dir, "walk", Game::Park);
    core_testing::tick(&mut app, 60);
    for _ in 0..3 {
        core_testing::tap(&mut app, ui::FASTER.center());
        core_testing::tick(&mut app, 20);
    }
    core_testing::tick(&mut app, 60 * 8);
    cut(&mut app);
}

/// A show in the contest hall: the trick the judge asked for, then
/// (skipping the rest of the show) the results.
fn compete(dir: &Path) {
    use pals_game::contest::{Contest, ContestPlugin, SHOW_SECS};
    let mut star = pet("STAR", 7);
    star.walk(450.0);
    star.hat = Some(11);
    for (slot, gesture) in [(0, pals::Gesture::SwipeUp), (1, pals::Gesture::SwipeDown)] {
        star.teach(
            slot,
            pals::Trigger {
                gesture: Some(gesture),
                voice: None,
            },
        );
        star.tricks[slot].learned = 80;
    }
    let mut tank = pals::Pals::default();
    tank.earn(1000);
    tank.buy_hat(11);
    tank.keep(star).ok();
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(ContestPlugin);
        },
        saved(&tank),
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    core_testing::click_action(&mut app);
    {
        let mut contest = app.world_mut().resource_mut::<Contest>();
        contest.rig_rivals([60, 45, 30]);
        contest.rig_request(0);
        contest.rig_next_try(true);
    }
    action(&mut app, dir, "compete", Game::Contest);
    core_testing::tick(&mut app, 30);
    swipe_up(&mut app);
    core_testing::tick(&mut app, 150);
    cut(&mut app);
    core_testing::tick(&mut app, (SHOW_SECS * 60.0) as u32);
    action(&mut app, dir, "compete_results", Game::Contest);
    core_testing::tick(&mut app, 60 * 3);
    cut(&mut app);
}

/// Turning round the hat shop's ring, buying the clown hat, turning on.
fn hats(dir: &Path) {
    use pals_game::shop::{ShopPlugin, hat_turn};
    let mut tank = pals::Pals::default();
    tank.earn(260);
    tank.keep(pet("GILLY", 3)).ok();
    tank.keep(pet("KOI", 7)).ok();
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(ShopPlugin);
        },
        saved(&tank),
    );
    rig(&mut app);
    battery(&mut app, 76);
    let sweep = |app: &mut App, from: f32, to: f32, frames: u32| {
        for i in 1..=frames {
            let t = i as f32 / frames as f32;
            core_testing::set_tilt(app, from + (to - from) * t, 0.0);
            app.update();
        }
    };
    core_testing::set_tilt(&mut app, hat_turn(0), 0.0);
    core_testing::tick(&mut app, 2);
    action(&mut app, dir, "hats", Game::Shop);
    core_testing::tick(&mut app, 10);
    sweep(&mut app, hat_turn(0), hat_turn(7), 100);
    core_testing::tick(&mut app, 20);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 50);
    let clown_from_the_left = hat_turn(7) - 360.0 / orbit_ui::DIAL_GAIN;
    sweep(&mut app, clown_from_the_left, hat_turn(12), 70);
    core_testing::tick(&mut app, 60);
    cut(&mut app);
}

/// Bees??? with the lead fish's cowboy hat on every bee: orbiting, then
/// tipped too far so the honey spills.
fn bees(dir: &Path) {
    use bees_game::{CONTROL_GAIN, Orbit};
    let mut tank = pals::Pals::default();
    tank.earn(1000);
    let cowboy = pals::HATS
        .iter()
        .position(|hat| hat.name == "COWBOY HAT")
        .unwrap_or(0);
    tank.buy_hat(cowboy);
    tank.keep(pet("COWBOY", 4)).ok();
    tank.wear(0, Some(cowboy));
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(bees_game::BeesPlugin);
        },
        saved(&tank),
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    action(&mut app, dir, "bees", Game::Bees);
    for _ in 0..60 * 4 {
        let steer = app.world().resource::<Orbit>().drift_deg / CONTROL_GAIN;
        core_testing::set_tilt(&mut app, steer, 0.0);
        app.update();
    }
    core_testing::set_tilt(&mut app, 60.0, 0.0);
    core_testing::tick(&mut app, 60 * 3);
    cut(&mut app);
}

/// An ollie onto a bench, the grind along it, the landing.
fn skate_grind(dir: &Path) {
    use skate_game::{Kind, POP_DEG, Ride, SKATER_X};
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(skate_game::SkatePlugin);
        },
        None,
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 20);
    hush(&mut app);
    action(&mut app, dir, "skate_grind", Game::Skate);
    core_testing::tick(&mut app, 20);
    app.world_mut()
        .resource_mut::<Ride>()
        .place(Kind::Bench, SKATER_X + 90.0);
    core_testing::set_tilt(&mut app, -(POP_DEG + 8.0), 0.0);
    core_testing::tick(&mut app, 14);
    core_testing::set_tilt(&mut app, 0.0, 0.0);
    core_testing::tick(&mut app, 120);
    cut(&mut app);
}

/// Dropping into the halfpipe, a spin in the air, the landing.
fn skate_halfpipe(dir: &Path) {
    use skate_game::Ride;
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(skate_game::SkatePlugin);
        },
        None,
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::click_action(&mut app);
    app.world_mut().resource_mut::<Ride>().approach_halfpipe();
    action(&mut app, dir, "skate_halfpipe", Game::Skate);
    let mut frames = 0;
    while !app.world().resource::<Ride>().halfpipe_air() && frames < 60 * 10 {
        app.update();
        frames += 1;
    }
    core_testing::set_tilt(&mut app, 30.0, 0.0);
    core_testing::tick(&mut app, 18);
    core_testing::set_tilt(&mut app, 60.0, 0.0);
    while app.world().resource::<Ride>().halfpipe_air() && frames < 60 * 20 {
        app.update();
        frames += 1;
    }
    core_testing::tick(&mut app, 60);
    cut(&mut app);
}

/// Four safe eggs cracked, then the spin that finds the spider, which
/// leaps at the glass.
fn spider(dir: &Path) {
    use spider_game::Table;
    let mut app = core_testing::headless(
        |app| {
            app.add_plugins(spider_game::SpiderPlugin);
        },
        None,
    );
    rig(&mut app);
    battery(&mut app, 76);
    core_testing::tick(&mut app, 7);
    core_testing::click_action(&mut app);
    app.world_mut().resource_mut::<Table>().hide_spider(6);
    for egg in [0, 3, 8, 10] {
        app.world_mut().resource_mut::<Table>().rig_next_spin(egg);
        core_testing::click_action(&mut app);
        core_testing::tick(&mut app, 60 * 4);
    }
    hush(&mut app);
    action(&mut app, dir, "spider", Game::Spider);
    core_testing::tick(&mut app, 10);
    app.world_mut().resource_mut::<Table>().rig_next_spin(6);
    core_testing::click_action(&mut app);
    core_testing::tick(&mut app, 60 * 6);
    cut(&mut app);
}

/// The GUTEN easter egg: the spinning fish and its song.
fn guten(dir: &Path) {
    let mut app = angler::headless_app();
    rig(&mut app);
    battery(&mut app, 76);
    angler::tap(&mut app, ui::SETTINGS.center());
    action(&mut app, dir, "guten", Game::Angler);
    angler::tap(&mut app, ui::GUTEN.center());
    angler::tick(&mut app, 60 * 8);
    cut(&mut app);
}
