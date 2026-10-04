use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

use bevy::{math::ops, platform::time::Instant, prelude::*};
use orbit_game::{
    arena::SCREEN_CENTER,
    testing::{headless_app, press, tap, tick},
};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = CURRENT.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(now, Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

static CLOCK_US: AtomicU64 = AtomicU64::new(0);

fn fake_clock() -> Duration {
    Duration::from_micros(CLOCK_US.fetch_add(16_667, Relaxed))
}

static mut REPORT: [u32; 3] = [0; 3];

/// Builds the app, plays 20 s of scripted input, idles 10 s, and returns a
/// pointer to `[bytes after build, bytes at end, peak bytes]`.
#[unsafe(no_mangle)]
pub extern "C" fn measure() -> *const u32 {
    // SAFETY: called once, before anything reads Bevy's clock.
    unsafe { Instant::set_elapsed(fake_clock) };
    let mut app = headless_app();
    let built = CURRENT.load(Relaxed);
    tap(&mut app, SCREEN_CENTER);
    for frame in 0..1200 {
        let angle = frame as f32 * 0.05;
        let finger = SCREEN_CENTER + Vec2::new(ops::cos(angle), ops::sin(angle)) * 100.0;
        press(&mut app, Some(finger));
    }
    tick(&mut app, 600);
    // SAFETY: single-threaded wasm; the host reads REPORT after we return.
    unsafe {
        REPORT = [
            built as u32,
            CURRENT.load(Relaxed) as u32,
            PEAK.load(Relaxed) as u32,
        ];
        (&raw const REPORT).cast()
    }
}
