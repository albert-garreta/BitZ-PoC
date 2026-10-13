//! Stable ownership of large rows across folds of their high index bits.

use std::iter::StepBy;
use std::ops::Range;
use std::sync::Mutex;

/// Bound fallback task coarsening without assuming persistent row ownership.
/// Two potential tasks per worker leave room for work stealing; small passes
/// and serial execution retain their existing scheduling.
fn bounded_grain(requested: usize, rows: usize, matrix_bytes: usize, workers: usize) -> usize {
    if workers <= 1 || matrix_bytes < 4 << 20 {
        1
    } else {
        // Rayon halves indexed ranges when splitting. A power-of-two cap
        // avoids rounding a five-row minimum up to eight-row leaves, for
        // example, and losing the intended two tasks per worker.
        let cap = (rows / workers.saturating_mul(2)).max(1);
        requested.min(1 << cap.ilog2())
    }
}

fn requested_grain() -> usize {
    #[cfg(test)]
    if let Some(requested) = TEST_GRAIN.get() {
        return requested;
    }
    #[cfg(feature = "bench-internals")]
    {
        static GRAIN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        *GRAIN.get_or_init(|| match std::env::var("BITZ_FOREST_ROW_GRAIN") {
            Err(std::env::VarError::NotPresent) => 1,
            Ok(value) => match value.as_str() {
                "1" => 1,
                "2" => 2,
                "4" => 4,
                "8" => 8,
                "16" => 16,
                _ => panic!("BITZ_FOREST_ROW_GRAIN must be 1, 2, 4, 8, or 16"),
            },
            Err(std::env::VarError::NotUnicode(_)) => {
                panic!("BITZ_FOREST_ROW_GRAIN must be valid Unicode")
            }
        })
    }
    #[cfg(not(feature = "bench-internals"))]
    1
}

pub(super) fn fallback_grain(rows: usize, matrix_bytes: usize) -> usize {
    let requested = requested_grain();
    if requested == 1 {
        return 1;
    }
    bounded_grain(requested, rows, matrix_bytes, rayon::current_num_threads())
}

#[cfg(test)]
thread_local! {
    static TEST_GRAIN: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Override only the calling proving thread; row jobs never consult this state.
/// Call inside the test pool's `install`, so unrelated tests and worker threads
/// retain their own settings, including when the test panics.
#[cfg(test)]
pub(super) fn with_grain<R>(requested: usize, run: impl FnOnce() -> R) -> R {
    assert!(matches!(requested, 1 | 2 | 4 | 8 | 16));
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_GRAIN.set(self.0);
        }
    }
    let _restore = Restore(TEST_GRAIN.replace(Some(requested)));
    run()
}

/// Row `y` stays on worker `y % workers`. High-bit folds preserve that
/// owner while the number of rows remains a multiple of the worker count.
/// This avoids moving dirty arena rows between workers at every fold.
#[derive(Clone, Copy)]
pub(super) struct CyclicRows {
    workers: usize,
}

impl CyclicRows {
    pub(super) fn new(rows: usize, matrix_bytes: usize) -> Option<Self> {
        let workers = rayon::current_num_threads();
        (workers > 8
            && workers.is_power_of_two()
            && rows >= workers
            && rows % workers == 0
            && matrix_bytes >= 4 << 20)
            .then_some(Self { workers })
    }

    /// Run one local reduction per worker over its row indexes.
    pub(super) fn map_indices<R: Send>(
        self,
        rows: usize,
        f: impl Fn(StepBy<Range<usize>>) -> R + Sync,
    ) -> Vec<R> {
        rayon::broadcast(|worker| {
            debug_assert_eq!(worker.num_threads(), self.workers);
            f((worker.index()..rows).step_by(self.workers))
        })
    }

    /// Transfer disjoint row jobs to their owners before running them.
    /// The mutex only transfers ownership; no lock is held during `f`.
    pub(super) fn map_jobs<J: Send, R: Send>(
        self,
        jobs: impl ExactSizeIterator<Item = J>,
        f: impl Fn(Vec<(usize, J)>) -> R + Sync,
    ) -> Vec<R> {
        let mut owned: Vec<Vec<(usize, J)>> = (0..self.workers)
            .map(|_| Vec::with_capacity(jobs.len().div_ceil(self.workers)))
            .collect();
        for (row, job) in jobs.enumerate() {
            owned[row % self.workers].push((row, job));
        }
        let owned: Vec<_> = owned.into_iter().map(Mutex::new).collect();
        rayon::broadcast(|worker| {
            debug_assert_eq!(worker.num_threads(), self.workers);
            let jobs = std::mem::take(&mut *owned[worker.index()].lock().unwrap());
            f(jobs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_grain_preserves_small_passes_and_parallelism() {
        for requested in [1, 2, 4, 8, 16] {
            for workers in [1, 8, 12, 16] {
                assert_eq!(bounded_grain(requested, 1024, (4 << 20) - 1, workers), 1);
                for rows in [0, 1, 7, 31, 64, 127, 128, 129, 256, 259, 1024] {
                    let grain = bounded_grain(requested, rows, 4 << 20, workers);
                    assert!(grain.is_power_of_two() && grain <= requested);
                    if workers == 1 || rows < 2 * workers {
                        assert_eq!(grain, 1);
                    } else {
                        assert!(rows / grain >= 2 * workers);
                    }
                }
            }
        }
        assert_eq!(bounded_grain(16, 128, 4 << 20, 12), 4);
        assert_eq!(bounded_grain(16, 256, 4 << 20, 12), 8);
    }

    #[test]
    fn fallback_row_grains_cover_disjoint_rows_and_tails() {
        use rayon::prelude::*;

        for workers in [1, 8, 12, 16] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            pool.install(|| {
                for requested in [1, 2, 4, 8, 16] {
                    with_grain(requested, || {
                        for rows in [0usize, 1, 127, 128, 129, 259] {
                            // Partial final row checks exact coverage even
                            // when neither rows nor elements divide the pool.
                            let len = (rows * 3).saturating_sub(1);
                            let mut values = vec![usize::MAX; len];
                            let grain = fallback_grain(rows, 4 << 20);
                            let visited: Vec<_> = values
                                .par_chunks_mut(3)
                                .enumerate()
                                .with_min_len(grain)
                                .map(|(row, values)| {
                                    values.fill(row);
                                    row
                                })
                                .collect();
                            assert_eq!(visited, (0..rows).collect::<Vec<_>>());
                            assert!(values.iter().enumerate().all(|(i, &value)| value == i / 3));
                        }
                        // The grain override cannot activate cyclic ownership
                        // at twelve workers or change its existing sixteen rule.
                        assert_eq!(CyclicRows::new(128, 4 << 20).is_some(), workers == 16);
                    });
                }
            });
        }
    }

    #[test]
    fn scoped_grain_override_restores_nested_and_panicking_calls() {
        with_grain(2, || {
            assert_eq!(requested_grain(), 2);
            with_grain(8, || assert_eq!(requested_grain(), 8));
            assert_eq!(requested_grain(), 2);
            assert!(std::panic::catch_unwind(|| with_grain(4, || panic!("test"))).is_err());
            assert_eq!(requested_grain(), 2);
        });
        assert_eq!(TEST_GRAIN.get(), None);
    }

    #[test]
    fn cyclic_rows_keep_disjoint_jobs_on_their_owner() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(16)
            .build()
            .unwrap();
        pool.install(|| {
            let dispatch = CyclicRows::new(48, 4 << 20).unwrap();
            let owners = dispatch.map_indices(48, |rows| {
                rows.map(|row| {
                    assert_eq!(rayon::current_thread_index(), Some(row % 16));
                    row
                })
                .collect::<Vec<_>>()
            });
            assert_eq!(owners.iter().map(Vec::len).sum::<usize>(), 48);

            // Unequal final chunks still belong to exactly one worker.
            let mut values = vec![usize::MAX; 48 * 3 - 1];
            let counts = dispatch.map_jobs(values.chunks_mut(3), |jobs| {
                let count = jobs.len();
                for (row, job) in jobs {
                    assert_eq!(rayon::current_thread_index(), Some(row % 16));
                    job.fill(row);
                }
                count
            });
            assert_eq!(counts.into_iter().sum::<usize>(), 48);
            assert!(values.iter().enumerate().all(|(i, &value)| value == i / 3));
        });
    }

    #[test]
    fn cyclic_rows_leave_small_pools_and_matrices_unchanged() {
        for workers in [1, 4, 8, 12, 16] {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| {
                    assert_eq!(CyclicRows::new(64, 4 << 20).is_some(), workers == 16);
                    assert!(CyclicRows::new(64, (4 << 20) - 1).is_none());
                    assert!(CyclicRows::new(8, 4 << 20).is_none());
                });
        }
    }
}
