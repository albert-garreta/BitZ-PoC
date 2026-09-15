use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("worker lockfile");
    let revision = lock
        .split("[[package]]")
        .find(|p| p.lines().any(|l| l == "name = \"binius-circuits\""))
        .and_then(|p| p.lines().find(|l| l.starts_with("source = \"git+")))
        .and_then(|line| {
            line.trim_end_matches('"')
                .rsplit_once('#')
                .map(|(_, rev)| rev)
        })
        .unwrap_or("unpublished");
    let mut source = Sha256::new();
    for path in [
        "Cargo.toml",
        "rust-toolchain.toml",
        "build.rs",
        "build.py",
        "src/main.rs",
        "../../benches/support/sha256_ecdsa_fixture.rs",
        "../../benches/common/output.rs",
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
