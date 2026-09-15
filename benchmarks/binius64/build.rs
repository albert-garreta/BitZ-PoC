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
    // The F2Z opener path dependency: record the parent repository's revision
    // and whether its tracked tree is dirty. Cargo rebuilds the path dep on
    // source change by itself; the parent campaign manifest records the exact
    // tracked diff, and the .build.json sidecar pins the binary hash.
    let f2z_root = root.join("../..");
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&f2z_root)
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
    };
    let f2z_revision = git(&["rev-parse", "HEAD"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let f2z_dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .map(|s| if s.trim().is_empty() { "clean" } else { "dirty" })
        .unwrap_or("unknown");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rustc-env=F2Z_REVISION={f2z_revision}");
    println!("cargo:rustc-env=F2Z_DIRTY={f2z_dirty}");
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
