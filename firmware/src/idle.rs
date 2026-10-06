//! Halts the CPU between frames instead of spinning in a delay loop: a
//! systimer alarm interrupt ends a `waiti`, so the spare part of every frame
//! costs no instruction fetches. DMA (display, speaker) keeps running.

use core::{
    arch::asm,
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};

use critical_section::Mutex;
use esp_hal::{
    Blocking, handler,
    time::Duration,
    timer::{OneShotTimer, systimer::Alarm},
};

static ALARM: Mutex<RefCell<Option<OneShotTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(None));
static RANG: AtomicBool = AtomicBool::new(false);

/// Halting for less than this isn't worth the interrupt round trip.
const SHORTEST: Duration = Duration::from_micros(100);

#[handler]
fn alarm_rang() {
    critical_section::with(|cs| {
        if let Some(alarm) = ALARM.borrow_ref_mut(cs).as_mut() {
            alarm.clear_interrupt();
        }
    });
    RANG.store(true, Ordering::Release);
}

/// Owns the systimer alarm the halts wake on.
pub struct Idle;

impl Idle {
    pub fn new(alarm: Alarm<'static>) -> Self {
        let mut timer = OneShotTimer::new(alarm);
        timer.set_interrupt_handler(alarm_rang);
        timer.listen();
        critical_section::with(|cs| ALARM.borrow_ref_mut(cs).replace(timer));
        Self
    }

    /// Halts the CPU for about `duration`; any other interrupt may end it early.
    pub fn wait(&self, duration: Duration) {
        if duration < SHORTEST {
            return;
        }
        RANG.store(false, Ordering::Release);
        let scheduled = critical_section::with(|cs| {
            ALARM
                .borrow_ref_mut(cs)
                .as_mut()
                .is_some_and(|alarm| alarm.schedule(duration).is_ok())
        });
        if !scheduled {
            return;
        }
        loop {
            let ps: u32;
            // Mask interrupts, then look: an alarm that rang before the look
            // is seen, and one that rings after it is taken by `waiti`, which
            // unmasks and halts in one instruction, so no wake-up is lost.
            unsafe { asm!("rsil {0}, 5", out(reg) ps) };
            let rang = RANG.load(Ordering::Acquire);
            if !rang {
                unsafe { asm!("waiti 0") };
            }
            unsafe { asm!("wsr.ps {0}", "rsync", in(reg) ps) };
            if rang {
                return;
            }
        }
    }
}
