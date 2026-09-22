use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};

/// The pinned Binius64 commit, read from the `binius-core` git source in the worker lockfile.
fn binius_revision(lock: &str) -> String {
    let mut in_core = false;
    for line in lock.lines() {
        if let Some(name) = line.strip_prefix("name = ") {
            in_core = name == "\"binius-core\"";
        } else if in_core {
            if let Some(source) = line.strip_prefix("source = \"git+") {
                return source.trim_end_matches('"').rsplit('#').next().unwrap().to_owned();
            }
        }
    }
    panic!("binius-core is not a git dependency in the worker lockfile");
}

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("worker lockfile");
    let revision = binius_revision(&lock);
    let mut source = Sha256::new();
    for path in [
        "Cargo.toml",
        "rust-toolchain.toml",
        "build.rs",
        "build.py",
        "src/main.rs",
        "../../benches/support/sha256_ecdsa_fixture.rs",
        "../../benches/common/output.rs",
        "../../benches/common/trace_capture.rs",
        "../../src/observability.rs",
        "../../src/observability/memory.rs",
    ] {
        let path = root.join(path);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(path).unwrap();
        source.update((bytes.len() as u64).to_le_bytes());
        source.update(bytes);
    }
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    // The BitZ opener path dependency: record the parent repository's revision
    // and whether its tracked tree is dirty. Cargo rebuilds the path dep on
    // source change by itself; the parent campaign manifest records the exact
    // tracked diff, and the .build.json sidecar pins the binary hash.
    let rustc = Command::new(env::var_os("RUSTC").unwrap())
        .arg("-Vv")
        .output()
        .unwrap();
    let rustc = String::from_utf8(rustc.stdout)
        .unwrap()
        .replace('\n', " | ");
    println!("cargo:rustc-env=BINIUS_REVISION={revision}");
    println!(
        "cargo:rustc-env=LOCK_SHA256={:x}",
        Sha256::digest(lock.as_bytes())
    );
    println!("cargo:rustc-env=SOURCE_SHA256={:x}", source.finalize());
    println!("cargo:rustc-env=BUILD_RUSTC={rustc}");
    println!(
        "cargo:rustc-env=BUILD_RUSTFLAGS={}",
        env::var("CARGO_ENCODED_RUSTFLAGS")
            .unwrap_or_default()
            .replace('\u{1f}', " ")
    );
}
