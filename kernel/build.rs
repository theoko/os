use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("none") {
        return;
    }

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linker = manifest_dir.join(format!("linker-{arch}.ld"));

    println!("cargo:rustc-link-arg=-T{}", linker.display());
    println!("cargo:rerun-if-changed={}", linker.display());
}
