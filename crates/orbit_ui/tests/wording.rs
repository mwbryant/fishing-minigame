//! Screens say PRESS, never PWR. Checked in every crate's string literals.

use std::{
    fs,
    path::{Path, PathBuf},
};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_screen_calls_the_action_button_pwr() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    for entry in fs::read_dir(crates).unwrap() {
        let src = entry.unwrap().path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    let mut offenders = Vec::new();
    for file in &files {
        for (number, line) in fs::read_to_string(file).unwrap().lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let mut quoted = code.split('"').skip(1).step_by(2);
            if quoted.any(|text| text.contains("PWR")) {
                offenders.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "say PRESS instead:\n{}",
        offenders.join("\n")
    );
}
