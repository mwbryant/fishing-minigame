//! The browser as the board's platform, as the firmware is on the board: the
//! `#orbit` canvas is the panel, the mouse or a finger the touch, keys, the
//! `#pad` stick or a phone's own tilt the buttons and the device's motion,
//! WebAudio the speaker and the mic, and localStorage the save flash.
//! index.html resumes the audio on the first tap or key, as browsers ask.

use std::{cell::RefCell, mem, rc::Rc, time::Duration};

use bevy::math::Vec2;
use js_sys::{Function, Math, Object, Promise, Reflect};
use orbit_audio::SAMPLE_RATE;
use orbit_web::{
    board::{Board, Game},
    clock,
    controls::{Controls, Tilt, key_bit},
    mic::Downsampler,
    pad::Pad,
    panel::SIZE,
};
use wasm_bindgen::{
    Clamped, JsCast, JsValue,
    closure::{Closure, WasmClosure},
    convert::FromWasmAbi,
};
use web_sys::{
    AudioContext, AudioContextOptions, AudioContextState, AudioProcessingEvent,
    CanvasRenderingContext2d, DeviceOrientationEvent, Document, Event, EventTarget,
    HtmlCanvasElement, ImageData, KeyboardEvent, MediaStream, MediaStreamConstraints, PointerEvent,
    Storage, UrlSearchParams, Window, console,
};

pub fn run() {
    console_error_panic_hook::set_once();
    clock::start();
    let window = web_sys::window().expect("a window");
    let document = window.document().expect("a document");
    let (canvas, panel) = canvas_named(&document, "orbit");
    let (pad_canvas, pad_panel) = canvas_named(&document, "pad");
    let game = game_asked(&window).unwrap_or(Game::Town);
    let storage = window.local_storage().ok().flatten();
    let save_key = format!("logic-orbit/{}.flash", game.name());
    let flash = storage
        .as_ref()
        .and_then(|storage| load_flash(&window, storage, &save_key));
    let seed = (Math::random() * f64::from(u32::MAX)) as u32;
    let page = Rc::new(RefCell::new(Page {
        board: Board::boot(game, flash, seed),
        controls: Controls::default(),
        pad: Pad::default(),
        speaker: Speaker::open(),
        heard: Rc::default(),
        mic_opened: false,
        tilt_asked: false,
        last_frame: f64::NEG_INFINITY,
        brightness: 100,
        stats: Stats::default(),
        window: window.clone(),
        canvas: canvas.clone(),
        panel,
        pad_canvas: pad_canvas.clone(),
        pad_panel,
        storage,
        save_key,
    }));
    listen(&page, &window, &document, &canvas, &pad_canvas);
    start_frames(page, window);
}

/// index.html's `#id` canvas and its 2D context.
fn canvas_named(document: &Document, id: &str) -> (HtmlCanvasElement, CanvasRenderingContext2d) {
    let canvas: HtmlCanvasElement = document
        .get_element_by_id(id)
        .and_then(|element| element.dyn_into().ok())
        .unwrap_or_else(|| panic!("index.html has no #{id} canvas"));
    let context = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|context| context.dyn_into().ok())
        .expect("a 2d context");
    (canvas, context)
}

/// `?game=NAME` boots one app on its own, like the desktop simulator's
/// argument.
fn game_asked(window: &Window) -> Option<Game> {
    let search = window.location().search().ok()?;
    let name = UrlSearchParams::new_with_str(&search).ok()?.get("game")?;
    Game::named(&name)
}

struct Page {
    board: Board,
    controls: Controls,
    pad: Pad,
    speaker: Option<Speaker>,
    /// What the mic heard since the last frame, at 16 kHz.
    heard: Rc<RefCell<Vec<i16>>>,
    mic_opened: bool,
    tilt_asked: bool,
    last_frame: f64,
    brightness: u8,
    stats: Stats,
    window: Window,
    canvas: HtmlCanvasElement,
    panel: CanvasRenderingContext2d,
    pad_canvas: HtmlCanvasElement,
    pad_panel: CanvasRenderingContext2d,
    storage: Option<Storage>,
    save_key: String,
}

impl Page {
    fn frame(&mut self, now: f64) {
        if let Some(speaker) = self.speaker.as_mut() {
            self.stats.spoken += speaker.top_up(&mut self.board);
        }
        let interval = 1000.0 / f64::from(self.board.frame_rate());
        if now - self.last_frame < interval - 1.0 {
            return;
        }
        self.last_frame = (self.last_frame + interval).max(now - interval);

        let input = self.controls.frame();
        clock::set(Duration::from_secs_f64(now.max(0.0) / 1000.0));
        let started = self.now();
        self.board.frame(clock::read(), &input);
        self.stats.frame_took(self.now() - started);
        let heard = mem::take(&mut *self.heard.borrow_mut());
        self.stats.heard += heard.len();
        self.board.hear(&heard);

        if self.board.take_drawn() {
            put(&self.panel, self.board.pixels());
        }
        self.pad.show(input.stick, self.board.pose());
        if self.pad.take_drawn() {
            put(&self.pad_panel, self.pad.pixels());
        }
        self.show_brightness();
        if self.board.take_saved() {
            self.store_flash();
        }
        if self.board.listening() && !self.mic_opened {
            self.mic_opened = true;
            open_mic(&self.window, self.heard.clone());
        }
        self.stats.show(&self.canvas, &self.board);
    }

    fn now(&self) -> f64 {
        self.window.performance().map_or(0.0, |clock| clock.now())
    }

    /// The panel's brightness setting (and idle dimming) as a CSS filter.
    fn show_brightness(&mut self) {
        let percent = self.board.brightness();
        if percent == self.brightness {
            return;
        }
        self.brightness = percent;
        let filter = if percent < 100 {
            format!("brightness({percent}%)")
        } else {
            String::new()
        };
        self.canvas.style().set_property("filter", &filter).ok();
    }

    fn store_flash(&self) {
        let Some(storage) = &self.storage else {
            return;
        };
        let binary: String = self.board.flash().iter().map(|&b| char::from(b)).collect();
        let kept = self
            .window
            .btoa(&binary)
            .and_then(|text| storage.set_item(&self.save_key, &text));
        if let Err(err) = kept {
            warn("the save wasn't kept:", &err);
        }
    }

    fn screen_angle(&self) -> f32 {
        self.window
            .screen()
            .ok()
            .and_then(|screen| screen.orientation().angle().ok())
            .map_or(0.0, f32::from)
    }

    /// iPhones share the phone's tilt only when asked from a tap.
    fn ask_for_tilt(&mut self) {
        if mem::replace(&mut self.tilt_asked, true) {
            return;
        }
        let Ok(class) = Reflect::get(&js_sys::global(), &"DeviceOrientationEvent".into()) else {
            return;
        };
        let Ok(ask) = Reflect::get(&class, &"requestPermission".into()) else {
            return;
        };
        let Some(asked) = ask
            .dyn_ref::<Function>()
            .and_then(|ask| ask.call0(&class).ok())
            .and_then(|asked| asked.dyn_into::<Promise>().ok())
        else {
            return;
        };
        let refused = Closure::<dyn FnMut(JsValue)>::new(|_| {});
        let _ = asked.catch(&refused);
        refused.forget();
    }
}

/// 466 x 466 RGBA pixels onto a canvas.
fn put(panel: &CanvasRenderingContext2d, pixels: &[u8]) {
    let pixels = Clamped(pixels);
    if let Ok(image) = ImageData::new_with_u8_clamped_array_and_sh(pixels, SIZE as u32, SIZE as u32)
    {
        panel.put_image_data(&image, 0.0, 0.0).ok();
    }
}

/// The 466 x 466 panel pixel under the pointer on `canvas`.
fn panel_point(canvas: &HtmlCanvasElement, event: &PointerEvent) -> Vec2 {
    let rect = canvas.get_bounding_client_rect();
    let scale = SIZE as f64;
    Vec2::new(
        ((f64::from(event.client_x()) - rect.left()) * scale / rect.width()) as f32,
        ((f64::from(event.client_y()) - rect.top()) * scale / rect.height()) as f32,
    )
}

fn load_flash(window: &Window, storage: &Storage, key: &str) -> Option<Vec<u8>> {
    let text = storage.get_item(key).ok()??;
    let binary = window.atob(&text).ok()?;
    Some(binary.chars().map(|c| c as u8).collect())
}

fn listen(
    page: &Rc<RefCell<Page>>,
    window: &Window,
    document: &Document,
    canvas: &HtmlCanvasElement,
    pad: &HtmlCanvasElement,
) {
    on(canvas, "pointerdown", page, |page, event: PointerEvent| {
        if event.pointer_type() == "mouse" && event.button() != 0 {
            return;
        }
        page.ask_for_tilt();
        let at = panel_point(&page.canvas, &event);
        if page.controls.finger_down(event.pointer_id(), at) {
            event.prevent_default();
            page.canvas.set_pointer_capture(event.pointer_id()).ok();
        }
    });
    on(canvas, "pointermove", page, |page, event: PointerEvent| {
        let at = panel_point(&page.canvas, &event);
        page.controls.finger_moved(event.pointer_id(), at);
    });
    for lift in ["pointerup", "pointercancel"] {
        on(canvas, lift, page, |page, event: PointerEvent| {
            page.controls.finger_up(event.pointer_id());
        });
    }
    on(pad, "pointerdown", page, |page, event: PointerEvent| {
        if event.pointer_type() == "mouse" && event.button() != 0 {
            return;
        }
        let at = panel_point(&page.pad_canvas, &event);
        if page.controls.pad_down(event.pointer_id(), at) {
            event.prevent_default();
            page.pad_canvas.set_pointer_capture(event.pointer_id()).ok();
        }
    });
    on(pad, "pointermove", page, |page, event: PointerEvent| {
        let at = panel_point(&page.pad_canvas, &event);
        page.controls.pad_moved(event.pointer_id(), at);
    });
    for lift in ["pointerup", "pointercancel"] {
        on(pad, lift, page, |page, event: PointerEvent| {
            page.controls.pad_up(event.pointer_id());
        });
    }
    on(window, "keydown", page, |page, event: KeyboardEvent| {
        let Some(bit) = key_bit(&event.code()) else {
            return;
        };
        if event.meta_key() || event.ctrl_key() || event.alt_key() {
            return;
        }
        event.prevent_default();
        if !event.repeat() {
            page.controls.key_down(bit);
        }
    });
    on(window, "keyup", page, |page, event: KeyboardEvent| {
        if let Some(bit) = key_bit(&event.code()) {
            event.prevent_default();
            page.controls.key_up(bit);
        }
    });
    on(window, "blur", page, |page, _: Event| {
        page.controls.let_go()
    });
    on(document, "visibilitychange", page, |page, _: Event| {
        page.controls.let_go();
    });
    on(
        window,
        "deviceorientation",
        page,
        |page, event: DeviceOrientationEvent| {
            let (Some(beta), Some(gamma)) = (event.beta(), event.gamma()) else {
                return;
            };
            let tilt = Tilt {
                alpha: event.alpha().unwrap_or(0.0) as f32,
                beta: beta as f32,
                gamma: gamma as f32,
                screen_angle: page.screen_angle(),
            };
            page.controls.tilt(tilt);
        },
    );
}

/// Calls `handle` with the page for every `kind` event on `target`, for the
/// rest of the visit.
fn on<E>(
    target: &EventTarget,
    kind: &str,
    page: &Rc<RefCell<Page>>,
    handle: impl Fn(&mut Page, E) + 'static,
) where
    E: FromWasmAbi + 'static,
    dyn FnMut(E): WasmClosure,
{
    let page = page.clone();
    let closure =
        Closure::<dyn FnMut(E)>::new(move |event: E| handle(&mut page.borrow_mut(), event));
    target
        .add_event_listener_with_callback(kind, closure.as_ref().unchecked_ref())
        .expect("an event listener");
    closure.forget();
}

fn start_frames(page: Rc<RefCell<Page>>, window: Window) {
    let tick: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::default();
    let next = tick.clone();
    let again = window.clone();
    *tick.borrow_mut() = Some(Closure::new(move |now: f64| {
        page.borrow_mut().frame(now);
        if let Some(tick) = next.borrow().as_ref() {
            request_frame(&again, tick);
        }
    }));
    if let Some(tick) = tick.borrow().as_ref() {
        request_frame(&window, tick);
    }
}

fn request_frame(window: &Window, tick: &Closure<dyn FnMut(f64)>) {
    window
        .request_animation_frame(tick.as_ref().unchecked_ref())
        .expect("an animation frame");
}

/// The board's mixer through WebAudio, at the board's 16 kHz when the
/// browser allows it.
struct Speaker {
    context: AudioContext,
    next: f64,
}

impl Speaker {
    /// Starts suspended; index.html resumes it on the first tap or key.
    fn open() -> Option<Self> {
        let options = AudioContextOptions::new();
        options.set_sample_rate(SAMPLE_RATE as f32);
        let context = AudioContext::new_with_context_options(&options)
            .or_else(|_| AudioContext::new())
            .ok()?;
        Some(Self { context, next: 0.0 })
    }

    /// Keeps about 120 ms of sound queued in 50 ms buffers, and returns how
    /// many of the samples queued weren't silent.
    fn top_up(&mut self, board: &mut Board) -> usize {
        if self.context.state() != AudioContextState::Running {
            return 0;
        }
        let now = self.context.current_time();
        if self.next < now + 0.01 {
            self.next = now + 0.03;
        }
        let chunk = SAMPLE_RATE as usize / 20;
        let mut sounding = 0;
        while self.next < now + 0.12 {
            let samples = board.speak(chunk);
            if samples.iter().any(|&s| s != 0.0) {
                sounding += chunk;
            }
            if self.queue(samples).is_err() {
                break;
            }
            self.next += chunk as f64 / f64::from(SAMPLE_RATE);
        }
        sounding
    }

    fn queue(&self, samples: &[f32]) -> Result<(), JsValue> {
        let buffer = self
            .context
            .create_buffer(1, samples.len() as u32, SAMPLE_RATE as f32)?;
        buffer.copy_to_channel(samples, 0)?;
        let source = self.context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        source.connect_with_audio_node(&self.context.destination())?;
        source.start_with_when(self.next)
    }
}

/// Opened the first time an app listens (the house and the contest), so the
/// browser asks the player then. Raw, like the board's mics.
fn open_mic(window: &Window, heard: Rc<RefCell<Vec<i16>>>) {
    let Ok(devices) = window.navigator().media_devices() else {
        warn("no microphone, so no voice commands", &JsValue::UNDEFINED);
        return;
    };
    let audio = Object::new();
    for setting in ["echoCancellation", "noiseSuppression", "autoGainControl"] {
        Reflect::set(&audio, &setting.into(), &JsValue::FALSE).ok();
    }
    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&audio);
    let Ok(asked) = devices.get_user_media_with_constraints(&constraints) else {
        return;
    };
    let granted = Closure::once(move |stream: JsValue| {
        if let Err(err) = capture(stream.unchecked_into(), heard) {
            warn("the microphone didn't start:", &err);
        }
    });
    let refused = Closure::once(|err: JsValue| warn("no microphone, so no voice commands:", &err));
    let _ = asked.then2(&granted, &refused);
    granted.forget();
    refused.forget();
}

fn capture(stream: MediaStream, heard: Rc<RefCell<Vec<i16>>>) -> Result<(), JsValue> {
    let context = AudioContext::new()?;
    let source = context.create_media_stream_source(&stream)?;
    let node = context
        .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
            2048, 1, 1,
        )?;
    let mut downsampler = Downsampler::new(context.sample_rate());
    let process =
        Closure::<dyn FnMut(AudioProcessingEvent)>::new(move |event: AudioProcessingEvent| {
            let Ok(samples) = event
                .input_buffer()
                .and_then(|buffer| buffer.get_channel_data(0))
            else {
                return;
            };
            let mut heard = heard.borrow_mut();
            downsampler.push(&samples, &mut heard);
            let stale = heard.len().saturating_sub(SAMPLE_RATE as usize);
            heard.drain(..stale);
        });
    node.set_onaudioprocess(Some(process.as_ref().unchecked_ref()));
    process.forget();
    source.connect_with_audio_node(&node)?;
    // The processor writes silence; joining the graph keeps it running.
    node.connect_with_audio_node(&context.destination())?;
    let _ = context.resume();
    mem::forget((context, source, node));
    Ok(())
}

fn warn(message: &str, err: &JsValue) {
    console::warn_2(&message.into(), err);
}

/// What's running and how it runs, as `data-` attributes on the canvas, for
/// web/smoke.mjs.
#[derive(Default)]
struct Stats {
    frames: u32,
    frame_ms: f64,
    spoken: usize,
    heard: usize,
}

impl Stats {
    fn frame_took(&mut self, ms: f64) {
        self.frames += 1;
        self.frame_ms += (ms - self.frame_ms) / 30.0;
    }

    fn show(&self, canvas: &HtmlCanvasElement, board: &Board) {
        if !self.frames.is_multiple_of(10) {
            return;
        }
        let rate = f64::from(SAMPLE_RATE);
        let pose = board.pose().unwrap_or_default();
        for (name, value) in [
            ("data-playing", board.playing().name().to_string()),
            ("data-steer", format!("{:.1}", pose.steer_deg)),
            ("data-lean", format!("{:.1}", pose.lean_deg)),
            ("data-off", board.is_off().to_string()),
            ("data-frames", self.frames.to_string()),
            ("data-frame-ms", format!("{:.3}", self.frame_ms)),
            (
                "data-spoken-secs",
                format!("{:.1}", self.spoken as f64 / rate),
            ),
            (
                "data-heard-secs",
                format!("{:.1}", self.heard as f64 / rate),
            ),
        ] {
            canvas.set_attribute(name, &value).ok();
        }
    }
}
