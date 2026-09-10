//! Best-effort machine provenance. Missing probes are reported as unknown.
use serde_json::{Value, json};
use std::{fs, process::Command};

fn command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub fn cpu() -> String {
    let detected = if cfg!(target_os = "linux") {
        fs::read_to_string("/proc/cpuinfo").ok().and_then(|text| {
            text.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                matches!(key.trim(), "model name" | "Hardware").then(|| value.trim().to_owned())
            })
        })
    } else if cfg!(target_os = "macos") {
        command("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else {
        None
    };
    detected.unwrap_or_else(|| "unknown".into())
}

pub fn metadata(threads: usize) -> Value {
    let memory = if cfg!(target_os = "linux") {
        fs::read_to_string("/proc/meminfo").ok().and_then(|text| {
            text.lines().find_map(|line| {
                let kib = line
                    .strip_prefix("MemTotal:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()?;
                kib.checked_mul(1024)
            })
        })
    } else if cfg!(target_os = "macos") {
        command("sysctl", &["-n", "hw.memsize"]).and_then(|s| s.parse::<u64>().ok())
    } else {
        None
    };
    json!({"os": std::env::consts::OS, "arch": std::env::consts::ARCH, "cpu": cpu(),
        "os_version": command("uname", &["-r"]), "ram_bytes": memory,
        "available_parallelism": std::thread::available_parallelism().ok().map(|n| n.get()),
        "threads": threads, "requested_threads": std::env::var("RAYON_NUM_THREADS").ok(),
        "rustc": command("rustc", &["-Vv"]), "rustflags": std::env::var("RUSTFLAGS").ok(),
        "cargo_encoded_rustflags": std::env::var("CARGO_ENCODED_RUSTFLAGS").ok(),
        "git_revision": command("git", &["rev-parse", "HEAD"]),
        "git_status": command("git", &["status", "--porcelain", "--untracked-files=no"]),
        "cargo_lock_blake3": blake3::hash(include_bytes!("../../Cargo.lock")).to_hex().to_string()})
}
