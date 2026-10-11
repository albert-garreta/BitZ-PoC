//! Stable ownership of large rows across folds of their high index bits.

use std::iter::StepBy;
use std::ops::Range;
use std::sync::Mutex;

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
