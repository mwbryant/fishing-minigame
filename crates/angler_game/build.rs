//! Sets `cfg(guten_pcm)` when assets/guten_morgen.pcm is present, to build the GUTEN clip in.

use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=assets");
    println!("cargo::rustc-check-cfg=cfg(guten_pcm)");
    if Path::new("assets/guten_morgen.pcm").exists() {
        println!("cargo::rustc-cfg=guten_pcm");
    }
}
