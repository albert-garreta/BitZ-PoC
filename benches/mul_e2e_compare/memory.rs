//! Isolated whole-case peak RSS, with no allocator or polling overhead in
//! the latency trials. Each child builds exactly one backend and verifies one proof.

use super::{Context, Corpus, TraceCapture, Workload};
use serde::{Deserialize, Serialize};
use std::{
    process::{Command, Stdio},
    sync::Arc,
};

const RESULT_PREFIX: &str = "F2Z_MEMORY_RESULT ";
const BOUNDARY: &str = "fresh process: corpus generation, public setup, witness generation, commitment, proving, verification, and proof-size accounting; one verified proof, no warmup";

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Sample {
    pub backend: String,
    pub workload: String,
    pub log_multiplications: usize,
    pub corpus_digest: String,
    pub peak_rss_bytes: u64,
    pub proof_bytes: usize,
    pub proof_verified: bool,
    pub boundary: String,
    pub config: serde_json::Value,
}

pub(super) fn measure(
    backend: &str,
    workload: Workload,
    exponent: usize,
    seed: u64,
    threads: usize,
    expected_digest: &str,
    whir_params: Option<super::common::whir_tuning::Params>,
) -> Result<Sample, Box<dyn std::error::Error>> {
    let output = Command::new(std::env::current_exe()?)
        .args(["--measure-memory", backend, workload.slug()])
        .arg(exponent.to_string())
        .arg(seed.to_string())
        .arg(serde_json::to_string(&whir_params)?)
        .env("RAYON_NUM_THREADS", threads.to_string())
        .stderr(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{backend} {} memory pass failed: {}",
            workload.slug(),
            output.status
        )
        .into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let result = stdout
        .lines()
        .find_map(|line| line.strip_prefix(RESULT_PREFIX))
        .ok_or("memory child did not return a result")?;
    let sample: Sample = serde_json::from_str(result)?;
    if sample.backend != backend
        || sample.workload != workload.slug()
        || sample.log_multiplications != exponent
        || sample.corpus_digest != expected_digest
        || !sample.proof_verified
        || sample.proof_bytes == 0
        || sample.peak_rss_bytes == 0
        || sample.boundary != BOUNDARY
        || whir_params.is_some_and(|params| sample.config["params"] != serde_json::json!(params))
    {
        return Err("memory child returned an invalid or mismatched result".into());
    }
    Ok(sample)
}

pub(super) fn run_child(
    capture: &TraceCapture,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let [backend, workload, exponent, seed, params] = args else {
        return Err("memory child requires backend, workload, exponent, and seed".into());
    };
    let workload = match workload.as_str() {
        "u32" | "u32-mod32" => Workload::U32,
        "u64" => Workload::U64,
        "u128" => Workload::U128,
        _ => return Err("unknown memory workload".into()),
    };
    let exponent = exponent.parse()?;
    let corpus = Arc::new(Corpus::new(workload, exponent, seed.parse()?));
    let params = serde_json::from_str(params)?;
    let context = Context::setup_selected(backend, Arc::clone(&corpus), params);
    let proof_bytes = match &context {
        Context::BiniusLigerito(context) => context.prove_and_verify(),
        _ => {
            let timing = context.run(capture);
            timing.validate();
            timing.proof_bytes
        }
    };
    let sample = Sample {
        backend: backend.clone(),
        workload: workload.slug().into(),
        log_multiplications: exponent,
        corpus_digest: corpus.digest.clone(),
        peak_rss_bytes: peak_rss_bytes()?,
        proof_bytes,
        proof_verified: true,
        boundary: BOUNDARY.into(),
        config: context.config(),
    };
    println!("{RESULT_PREFIX}{}", serde_json::to_string(&sample)?);
    Ok(())
}

#[cfg(target_os = "macos")]
fn peak_rss_bytes() -> std::io::Result<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage writes a complete rusage on success; the return code
    // is checked before reading it. RUSAGE_SELF excludes the parent process.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let usage = unsafe { usage.assume_init() };
    // Darwin reports bytes and starts a fresh high-water mark after exec.
    u64::try_from(usage.ru_maxrss)
        .ok()
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| std::io::Error::other("invalid peak RSS"))
}

#[cfg(target_os = "linux")]
fn peak_rss_bytes() -> std::io::Result<u64> {
    // Unlike Linux getrusage, VmHWM belongs to the current address space and
    // cannot retain the parent's pre-exec peak after fork/exec.
    parse_linux_peak_rss(&std::fs::read_to_string("/proc/self/status")?)
}

#[cfg(any(target_os = "linux", test))]
#[allow(dead_code)] // Also compiled by the harness-free bench's test profile.
fn parse_linux_peak_rss(status: &str) -> std::io::Result<u64> {
    let peak = status.lines().find_map(|line| {
        let mut fields = line.strip_prefix("VmHWM:")?.split_whitespace();
        let kib = fields.next()?.parse::<u64>().ok()?;
        if fields.next()? != "kB" || fields.next().is_some() {
            return None;
        }
        kib.checked_mul(1024).filter(|bytes| *bytes > 0)
    });
    peak.ok_or_else(|| std::io::Error::other("missing or invalid VmHWM in /proc/self/status"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn linux_peak_uses_hwm_in_bytes_and_rejects_unavailable_values() {
        assert_eq!(
            super::parse_linux_peak_rss("Name:\tbench\nVmHWM:\t2048 kB\nVmRSS:\t1024 kB\n")
                .unwrap(),
            2 * 1024 * 1024
        );
        for status in [
            "VmRSS: 1024 kB",
            "VmHWM: 0 kB",
            "VmHWM: 2048 MB",
            "VmHWM: unknown kB",
            "VmHWM: 18446744073709551615 kB",
        ] {
            assert!(super::parse_linux_peak_rss(status).is_err());
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn peak_rss_bytes() -> std::io::Result<u64> {
    Err(std::io::Error::other(
        "peak RSS is supported on macOS and Linux; use F2Z_MUL_COMPARE_MEMORY=0 for latency-only runs",
    ))
}
