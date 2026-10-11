//! Fishing Minigame for itch.io: `trunk build --release` from the repo root.

#[cfg(target_arch = "wasm32")]
mod browser;

fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::run();
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("orbit_web runs in a browser: `trunk serve` from the repo root");
}
