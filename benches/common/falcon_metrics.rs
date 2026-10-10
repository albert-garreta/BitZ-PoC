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
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}
