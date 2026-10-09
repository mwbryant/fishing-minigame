//! Builds the GUTEN song in only when its gitignored PCM is there (see assets/README.md).

use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=assets");
    println!("cargo::rustc-check-cfg=cfg(guten_song)");
    if Path::new("assets/guten_morgen.pcm").exists() {
        println!("cargo::rustc-cfg=guten_song");
    }
}
