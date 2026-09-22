//! Peak resident memory of the current worker's address space.

#[cfg(target_os = "linux")]
pub(super) fn peak_rss_bytes() -> std::io::Result<u64> {
    // Linux ru_maxrss can retain the parent's pre-exec peak. VmHWM is reset
    // when exec replaces the address space, so it measures this worker alone.
    parse_linux_peak_rss(&std::fs::read_to_string("/proc/self/status")?)
}

#[cfg(target_os = "linux")]
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

#[cfg(not(target_os = "linux"))]
pub(super) fn peak_rss_bytes() -> std::io::Result<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes this valid output pointer on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let rss = u64::try_from(unsafe { usage.assume_init() }.ru_maxrss)
        .map_err(|_| std::io::Error::other("invalid peak RSS"))?;
    if cfg!(target_os = "macos") {
        Ok(rss)
    } else {
        rss.checked_mul(1024)
            .ok_or_else(|| std::io::Error::other("peak RSS overflow"))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn linux_peak_uses_hwm_in_bytes() {
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
            "VmHWM: 2048 kB extra",
            "VmHWM: 18446744073709551615 kB",
        ] {
            assert!(super::parse_linux_peak_rss(status).is_err(), "{status}");
        }
    }

    #[test]
    fn linux_peak_excludes_pre_exec_parent_memory() {
        const CHILD: &str = "BITZ_TEST_RSS_EXEC_CHILD";
        const MIB: u64 = 1024 * 1024;
        if std::env::var_os(CHILD).is_some() {
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
            // SAFETY: getrusage initializes usage when it returns zero.
            assert_eq!(
                unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
                0
            );
            let inherited = unsafe { usage.assume_init() }.ru_maxrss as u64 * 1024;
            let worker = super::peak_rss_bytes().unwrap();
            assert!(
                inherited > worker + 32 * MIB,
                "expected inherited parent peak; ru_maxrss={inherited}, worker={worker}"
            );
            return;
        }

        // Keep touched pages live across exec. Replacing VmHWM with getrusage
        // makes the child assertion fail, even though its own workload is tiny.
        let allocation = vec![0xa5_u8; 64 * MIB as usize];
        std::hint::black_box(&allocation);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["linux_peak_excludes_pre_exec_parent_memory", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        std::hint::black_box(&allocation);
        assert!(
            output.status.success(),
            "child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
