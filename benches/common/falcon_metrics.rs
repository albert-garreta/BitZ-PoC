use std::{fmt, fs};

/// Stream Debug into a hash for exact-build comparisons without allocating its
/// string. This is not a canonical proof encoding; call it outside timed phases.
pub fn debug_digest<T: fmt::Debug + ?Sized>(value: &T) -> Result<String, fmt::Error> {
    struct DebugHasher(blake3::Hasher);

    impl fmt::Write for DebugHasher {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.0.update(value.as_bytes());
            Ok(())
        }
    }

    let mut hasher = DebugHasher(blake3::Hasher::new());
    fmt::write(&mut hasher, format_args!("{value:?}"))?;
    Ok(hasher.0.finalize().to_hex().to_string())
}

/// Linux's process-lifetime high-water mark; this is not a per-trial delta.
pub fn process_peak_rss_kib() -> Option<u64> {
    process_status_kib("VmHWM:")
}

/// Current Linux RSS, including memory retained by the allocator and Prepared.
/// Unlike VmHWM, this can fall when an allocation is released to the OS.
pub fn process_current_rss_kib() -> Option<u64> {
    process_status_kib("VmRSS:")
}

fn process_status_kib(field: &str) -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

/// Process-wide minor faults (all threads), sampled outside measured phases.
/// Counts are reported directly: converting to bytes would require the host's
/// actual page size and still would not measure retained memory.
#[cfg(target_os = "linux")]
pub fn process_minor_faults() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage writes a complete rusage to this valid output pointer on
    // success. Do not read the output after an error.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: the successful call above initialized usage.
    u64::try_from(unsafe { usage.assume_init() }.ru_minflt).ok()
}

#[cfg(not(target_os = "linux"))]
pub fn process_minor_faults() -> Option<u64> {
    None
}
