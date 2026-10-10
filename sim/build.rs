//! Picks the data table the simulation is built with.
//!
//! `generated/extracted.rs` is written by `tools/setup.py` from the installed game.
//! Without it the build uses `src/extracted.example.rs` (placeholder
//! values marked ESTIMATE) and says so. Set REWRITE_REQUIRE_DATA=1 to make a missing table an error.

use std::{env, fs, path::Path};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let generated_dir = Path::new(&manifest).join("../generated");
    let generated = generated_dir.join("extracted.rs");
    let example = Path::new(&manifest).join("src/extracted.example.rs");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("extracted.rs");

    println!("cargo:rerun-if-changed={}", generated_dir.display());
    println!("cargo:rerun-if-changed={}", example.display());
    println!("cargo:rerun-if-env-changed=REWRITE_REQUIRE_DATA");

    if generated.exists() {
        fs::copy(&generated, &out).unwrap();
    } else if env::var_os("REWRITE_REQUIRE_DATA").is_some() {
        eprintln!("\ngenerated/extracted.rs is missing. Generate it from the installed game:");
        eprintln!("    python tools/setup.py\n");
        std::process::exit(1);
    } else {
        println!(
            "cargo:warning=generated/extracted.rs not found: building with src/extracted.example.rs \
             (ESTIMATE placeholders). Run `python tools/setup.py`."
        );
        fs::copy(&example, &out).unwrap();
    }
}
