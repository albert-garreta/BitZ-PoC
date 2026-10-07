//! Optional benchmark-only placement of the main thread and Rayon workers.
//!
//! Both variables must be present: `BITZ_FALCON_WORKER_CPUS` is an ordered
//! comma-separated CPU list and `BITZ_FALCON_MAIN_CPU` selects the main CPU.
//! No affinity is changed when both are absent.

use serde::Serialize;
use std::{error::Error, io};

#[derive(Debug, Serialize)]
pub struct AffinityReport {
    policy: &'static str,
    requested_worker_cpus: Vec<usize>,
    requested_main_cpu: usize,
    requested_auxiliary_worker_cpus: Vec<usize>,
    observed_worker_cpus: Vec<Vec<usize>>,
    observed_auxiliary_worker_cpus: Vec<Vec<usize>>,
    observed_main_cpus: Vec<usize>,
    allowed_cpus: Vec<usize>,
    #[serde(skip)]
    verifier_tid: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct VerifierThreadAffinity {
    tid: i32,
    observed_cpus: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
struct Request {
    workers: Vec<usize>,
    main: usize,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

impl Request {
    fn parse(
        workers: Option<&str>,
        main: Option<&str>,
        threads: usize,
    ) -> io::Result<Option<Self>> {
        let (workers, main) = match (workers, main) {
            (None, None) => return Ok(None),
            (Some(workers), Some(main)) => (workers, main),
            _ => {
                return Err(invalid(
                    "set both BITZ_FALCON_WORKER_CPUS and BITZ_FALCON_MAIN_CPU",
                ));
            }
        };
        let parse_cpu = |value: &str| {
            value
                .trim()
                .parse::<usize>()
                .map_err(|_| invalid(format!("invalid benchmark CPU index: {value:?}")))
        };
        let workers = workers
            .split(',')
            .map(parse_cpu)
            .collect::<io::Result<Vec<_>>>()?;
        if workers.len() != threads || threads == 0 {
            return Err(invalid(
                "worker CPU count must equal the benchmark thread count",
            ));
        }
        for (index, cpu) in workers.iter().enumerate() {
            if workers[..index].contains(cpu) {
                return Err(invalid("worker CPU indices must be unique"));
            }
        }
        Ok(Some(Self {
            workers,
            main: parse_cpu(main)?,
        }))
    }

    #[cfg(any(target_os = "linux", test))]
    fn validate_allowed(&self, allowed: &[usize], mask_size: usize) -> io::Result<()> {
        for &cpu in self.workers.iter().chain(std::iter::once(&self.main)) {
            if cpu >= mask_size || !allowed.contains(&cpu) {
                return Err(invalid(format!(
                    "requested CPU {cpu} is outside the original allowed affinity mask"
                )));
            }
        }
        Ok(())
    }
}

fn optional_env(name: &str) -> Result<Option<String>, std::env::VarError> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn build_global_pool(threads: usize) -> Result<Option<AffinityReport>, Box<dyn Error>> {
    let workers = optional_env("BITZ_FALCON_WORKER_CPUS")?;
    let main = optional_env("BITZ_FALCON_MAIN_CPU")?;
    let request = Request::parse(workers.as_deref(), main.as_deref(), threads)?;
    #[cfg(feature = "parallel")]
    {
        if let Some(request) = request {
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            return linux::build_pinned_pool(request, threads).map(Some);
            #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
            {
                let _ = request;
                return Err(
                    invalid("Falcon benchmark affinity is audited only on Linux x86_64").into(),
                );
            }
        }
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()?;
    }
    #[cfg(not(feature = "parallel"))]
    {
        if request.is_some() {
            return Err(invalid("benchmark worker affinity requires the parallel feature").into());
        }
        if threads != 1 {
            return Err(invalid("multiple threads require the parallel feature").into());
        }
    }
    Ok(None)
}

/// Called after each successful verification, outside every benchmark timer.
pub fn verify_after_trial(
    report: &mut Option<AffinityReport>,
) -> Result<Option<VerifierThreadAffinity>, Box<dyn Error>> {
    let Some(report) = report else {
        return Ok(None);
    };
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return linux::verify_thread(report).map(Some).map_err(Into::into);
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = report;
        Err(invalid("Falcon benchmark affinity is audited only on Linux x86_64").into())
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::*;

    fn thread_affinity(tid: i32) -> io::Result<Vec<usize>> {
        // SAFETY: cpu_set_t is an integer bitmask; zero initializes all storage.
        // The kernel receives a writable, correctly sized mask. tid=0 denotes
        // this thread; other IDs are read from this process's /proc task list.
        let mut mask: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::sched_getaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &mut mask)
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((0..libc::CPU_SETSIZE as usize)
            // SAFETY: every index is within the initialized fixed-size mask.
            .filter(|&cpu| unsafe { libc::CPU_ISSET(cpu, &mask) })
            .collect())
    }

    fn current_affinity() -> io::Result<Vec<usize>> {
        thread_affinity(0)
    }

    fn require_singleton(observed: &[usize], cpu: usize) -> io::Result<()> {
        if observed != [cpu] {
            return Err(io::Error::other(format!(
                "CPU affinity readback differs: requested [{cpu}], observed {observed:?}"
            )));
        }
        Ok(())
    }

    fn pin_current(cpu: usize) -> io::Result<Vec<usize>> {
        if cpu >= libc::CPU_SETSIZE as usize {
            return Err(invalid("CPU index exceeds cpu_set_t capacity"));
        }
        // SAFETY: the zeroed integer mask has space for the checked CPU index;
        // sched_setaffinity targets only the calling thread (pid = 0).
        let mut mask: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::CPU_SET(cpu, &mut mask);
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mask)
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        let observed = current_affinity()?;
        require_singleton(&observed, cpu)?;
        Ok(observed)
    }

    #[cfg(feature = "parallel")]
    fn pin_pool(pool: &rayon::ThreadPool, cpus: &[usize]) -> io::Result<Vec<Vec<usize>>> {
        if pool.current_num_threads() != cpus.len() {
            return Err(invalid(
                "auxiliary Rayon pool count differs from requested thread count",
            ));
        }
        pool.broadcast(|context| pin_current(cpus[context.index()]))
            .into_iter()
            .collect()
    }

    #[cfg(feature = "parallel")]
    pub(super) fn build_pinned_pool(
        request: Request,
        threads: usize,
    ) -> Result<AffinityReport, Box<dyn Error>> {
        let allowed_cpus = current_affinity()?;
        request.validate_allowed(&allowed_cpus, libc::CPU_SETSIZE as usize)?;
        let worker_cpus = request.workers.clone();
        let (started, receiver) = std::sync::mpsc::channel();
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .start_handler(move |index| {
                let observed = pin_current(worker_cpus[index]);
                let _ = started.send((index, observed));
            })
            .build_global()?;
        let mut observed_worker_cpus = vec![Vec::new(); threads];
        // Startup errors fail the process before fixture preparation or timing.
        // All workers acknowledge pinning; no trial can race a late handler.
        for _ in 0..threads {
            let (index, observed) = receiver.recv_timeout(std::time::Duration::from_secs(30))?;
            observed_worker_cpus[index] = observed?;
        }
        // In these x86_64 Falcon paths this auxiliary pool runs only the serial
        // Keccak alpha fold reached from Flock's one-thread verifier pool.
        // Keep the original pool size and install dispatch, while fixing that
        // serial work to the main CPU regardless of which worker receives it.
        // Parallel Falcon work remains on the global pool above. This policy
        // must be re-audited if the auxiliary pool gains parallel Falcon work.
        let requested_auxiliary_worker_cpus = vec![request.main; threads];
        let observed_auxiliary_worker_cpus = pin_pool(
            flock_core::all_core_pool(),
            &requested_auxiliary_worker_cpus,
        )?;
        // Workers were created with the original allowed mask. Pin the main
        // thread afterwards so their startup cannot inherit a singleton mask.
        let observed_main_cpus = pin_current(request.main)?;
        Ok(AffinityReport {
            policy: "linux-x86_64-falcon-serial-aux-pinned-v1",
            requested_worker_cpus: request.workers,
            requested_main_cpu: request.main,
            requested_auxiliary_worker_cpus,
            observed_worker_cpus,
            observed_auxiliary_worker_cpus,
            observed_main_cpus,
            allowed_cpus,
            verifier_tid: None,
        })
    }

    fn unique_verifier_tid(tids: &[i32]) -> io::Result<i32> {
        match tids {
            &[tid] => Ok(tid),
            _ => Err(io::Error::other(format!(
                "expected one flock-verify thread, found {}",
                tids.len()
            ))),
        }
    }

    fn find_verifier_tid() -> io::Result<i32> {
        let mut tids = Vec::new();
        for entry in std::fs::read_dir("/proc/self/task")? {
            let entry = entry?;
            let name = match std::fs::read_to_string(entry.path().join("comm")) {
                Ok(name) => name,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if name.trim_end() == "flock-verify" {
                let tid = entry
                    .file_name()
                    .to_string_lossy()
                    .parse()
                    .map_err(|_| invalid("invalid verifier thread ID in /proc/self/task"))?;
                tids.push(tid);
            }
        }
        unique_verifier_tid(&tids)
    }

    pub(super) fn verify_thread(report: &mut AffinityReport) -> io::Result<VerifierThreadAffinity> {
        let tid = match report.verifier_tid {
            Some(tid) => tid,
            None => find_verifier_tid()?,
        };
        let name = std::fs::read_to_string(format!("/proc/self/task/{tid}/comm"))?;
        if name.trim_end() != "flock-verify" {
            return Err(io::Error::other(
                "cached verifier thread no longer has the expected name",
            ));
        }
        let observed_cpus = thread_affinity(tid)?;
        require_singleton(&observed_cpus, report.requested_main_cpu)?;
        report.verifier_tid = Some(tid);
        Ok(VerifierThreadAffinity { tid, observed_cpus })
    }

    #[cfg(test)]
    #[test]
    fn pinning_one_thread_leaves_callers_affinity_unchanged() {
        let original = current_affinity().unwrap();
        let cpu = original[0];
        let child = std::thread::spawn(move || pin_current(cpu).unwrap());
        assert_eq!(child.join().unwrap(), vec![cpu]);
        assert_eq!(current_affinity().unwrap(), original);
        assert!(pin_current(libc::CPU_SETSIZE as usize).is_err());
    }

    #[cfg(all(test, feature = "parallel"))]
    #[test]
    fn pool_acknowledges_each_pinned_worker_before_reporting() {
        let original = current_affinity().unwrap();
        let worker_count = flock_core::all_core_pool().current_num_threads();
        let workers: Vec<_> = original.iter().copied().take(worker_count).collect();
        assert_eq!(workers.len(), worker_count);
        let requested = workers.clone();
        let report = std::thread::spawn(move || {
            build_pinned_pool(
                Request {
                    main: workers[0],
                    workers: workers.clone(),
                },
                workers.len(),
            )
            .unwrap()
        })
        .join()
        .unwrap();
        assert_eq!(report.requested_worker_cpus, requested);
        assert_eq!(
            report.observed_worker_cpus,
            requested.iter().map(|&cpu| vec![cpu]).collect::<Vec<_>>()
        );
        assert_eq!(report.observed_main_cpus, [requested[0]]);
        assert_eq!(
            report.requested_auxiliary_worker_cpus,
            vec![requested[0]; worker_count]
        );
        assert_eq!(
            report.observed_auxiliary_worker_cpus,
            vec![vec![requested[0]]; worker_count]
        );
        assert_eq!(report.allowed_cpus, original);
        assert_eq!(current_affinity().unwrap(), original);
    }

    #[cfg(test)]
    #[test]
    fn verifier_metadata_rejects_missing_duplicate_or_wrong_masks() {
        assert!(unique_verifier_tid(&[]).is_err());
        assert!(unique_verifier_tid(&[1, 2]).is_err());
        assert_eq!(unique_verifier_tid(&[7]).unwrap(), 7);
        assert!(require_singleton(&[], 0).is_err());
        assert!(require_singleton(&[0, 1], 0).is_err());
        assert!(require_singleton(&[1], 0).is_err());
        require_singleton(&[0], 0).unwrap();
    }

    #[cfg(test)]
    #[test]
    fn verifier_thread_discovery_and_remote_readback_use_actual_mask() {
        let cpu = current_affinity().unwrap()[0];
        let (ready, started) = std::sync::mpsc::channel();
        let (finish, done) = std::sync::mpsc::channel();
        let child = std::thread::Builder::new()
            .name("flock-verify".to_owned())
            .spawn(move || {
                pin_current(cpu).unwrap();
                ready.send(()).unwrap();
                done.recv().unwrap();
            })
            .unwrap();
        started.recv().unwrap();
        let tid = find_verifier_tid().unwrap();
        require_singleton(&thread_affinity(tid).unwrap(), cpu).unwrap();
        finish.send(()).unwrap();
        child.join().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_affinity_preserves_default_and_explicit_order_is_retained() {
        assert_eq!(Request::parse(None, None, 16).unwrap(), None);
        let request = Request::parse(Some(" 8, 0, 9, 1 "), Some("8"), 4)
            .unwrap()
            .unwrap();
        assert_eq!(request.workers, [8, 0, 9, 1]);
        assert_eq!(request.main, 8);
        request.validate_allowed(&[0, 1, 8, 9], 1024).unwrap();
    }

    #[test]
    fn malformed_or_partial_requests_fail() {
        for (workers, main, threads) in [
            (Some("0,1"), None, 2),
            (None, Some("0"), 2),
            (Some("0,0"), Some("0"), 2),
            (Some("0,1"), Some("0"), 3),
            (Some("0,1,"), Some("0"), 3),
            (Some(""), Some("0"), 1),
            (Some("0,1"), Some("-1"), 2),
            (Some("0-3"), Some("0"), 4),
        ] {
            assert!(Request::parse(workers, main, threads).is_err());
        }
    }

    #[test]
    fn disallowed_or_unrepresentable_cpus_fail_before_pool_creation() {
        let request = Request::parse(Some("0,4"), Some("0"), 2).unwrap().unwrap();
        assert!(request.validate_allowed(&[0, 1], 1024).is_err());
        assert!(request.validate_allowed(&[0, 4], 4).is_err());
        let request = Request::parse(Some("0,1"), Some("4"), 2).unwrap().unwrap();
        assert!(request.validate_allowed(&[0, 1], 1024).is_err());
    }
}
