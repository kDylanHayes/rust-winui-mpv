//! Points the linker at the local mpv dev files and copies the mpv DLL next to
//! the built executable so `cargo run` works without touching PATH.
//!
//! Expects the files described in README.md under `./mpv`, or wherever the
//! `MPV_DIR` environment variable points.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=MPV_DIR");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mpv_dir = env::var_os("MPV_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("mpv"));
    println!("cargo:rerun-if-changed={}", mpv_dir.display());
    println!("cargo:rustc-link-search=native={}", mpv_dir.display());

    // OUT_DIR is target/<profile>/build/<pkg>/out; the exe lives in target/<profile>.
    let dll = mpv_dir.join("libmpv-2.dll");
    if dll.exists() {
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        if let Some(profile_dir) = out.ancestors().nth(3) {
            let _ = fs::copy(&dll, profile_dir.join("libmpv-2.dll"));
        }
    } else {
        println!(
            "cargo:warning=libmpv-2.dll not found in {}; see README.md",
            mpv_dir.display()
        );
    }
}
