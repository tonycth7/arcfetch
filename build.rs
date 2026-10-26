//! Embed the animation frames in `assets/animations/<name>/`.
//!
//! One directory per animation, one text file per frame, filename order. Add a
//! folder and rebuild; there is nothing to register by hand.

use std::{env, fs, path::Path};

fn main() {
    let root = Path::new("assets/animations");
    println!("cargo:rerun-if-changed=assets/animations");

    let mut dirs: Vec<_> = fs::read_dir(root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut out = String::from("const BUNDLED: &[(&str, &[&str])] = &[\n");
    for dir in dirs {
        let mut frames: Vec<_> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "txt"))
            .collect();
        frames.sort();
        if frames.is_empty() {
            continue;
        }

        let name = dir.file_name().unwrap().to_string_lossy();
        out.push_str(&format!("    ({name:?}, &[\n"));
        for frame in frames {
            println!("cargo:rerun-if-changed={}", frame.display());
            let path = frame.to_string_lossy().replace('\\', "/");
            out.push_str(&format!(
                "        include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{path}\")),\n"
            ));
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");

    let dest = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("bundled_anims.rs");
    fs::write(&dest, out).unwrap_or_else(|e| panic!("cannot write {}: {e}", dest.display()));
}
