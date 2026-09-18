//! Deterministic storage scheduling. Decisions use public geometry only.
use super::ForestSchedule;
use crate::pcs::IntegerMatrixLayout;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchedulePolicy {
    #[default]
    Auto,
    L2,
    L4,
    L8,
}
impl std::str::FromStr for SchedulePolicy {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "l2" => Ok(Self::L2),
            "l4" => Ok(Self::L4),
            "l8" => Ok(Self::L8),
            _ => Err(format!(
                "unknown GKR schedule {value:?}; expected auto, l2, l4, or l8"
            )),
        }
    }
}
impl SchedulePolicy {
    /// The shared environment policy used by the prover and benchmark metadata.
    pub fn from_env() -> Result<Self, String> {
        std::env::var("F2_FOREST_SCHEDULE")
            .unwrap_or_else(|_| "auto".into())
            .parse()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::L2 => "l2",
            Self::L4 => "l4",
            Self::L8 => "l8",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ForestPath {
    Single,
    Multi,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UnsupportedSchedule {
    #[error("GKR L2 is unsupported by the multi-claim forest; use auto, l4, or l8")]
    MultiL2,
    #[error("GKR L8 requires forest depth >= 5; got {depth}")]
    ShallowL8 { depth: usize },
}

/// A single shared policy for all forest entry points. Measurement coverage and
/// the full-product correction are documented in docs/gkr-full-product-regression.md.
pub fn resolve_schedule(
    policy: SchedulePolicy,
    layout: &IntegerMatrixLayout,
    path: ForestPath,
    threads: usize,
) -> Result<ForestSchedule, UnsupportedSchedule> {
    let depth = layout.row_vars + layout.word_bits.ilog2() as usize;
    let size = depth + layout.col_vars;
    if policy == SchedulePolicy::Auto
        && path == ForestPath::Single
        && cfg!(all(target_arch = "aarch64", target_os = "macos"))
    {
        if let Some(schedule) = apple_single_schedule(layout, depth, threads) {
            return Ok(schedule);
        }
    }
    match (policy, path) {
        (SchedulePolicy::L2, ForestPath::Multi) => Err(UnsupportedSchedule::MultiL2),
        (SchedulePolicy::L8, _) if depth < 5 => Err(UnsupportedSchedule::ShallowL8 { depth }),
        (SchedulePolicy::L2, _) => Ok(ForestSchedule::L2),
        (SchedulePolicy::L8, _) => Ok(ForestSchedule::L8),
        // Large, sufficiently deep forests benefit from the smaller stored chain.
        // With few workers, tall forests instead benefit from storing more levels.
        (SchedulePolicy::Auto, _)
            if depth >= 13 && size >= 25 && (threads > 4 || depth <= layout.col_vars + 1) =>
        {
            Ok(ForestSchedule::L8)
        }
        (SchedulePolicy::Auto, ForestPath::Single) if threads <= 4 && depth >= layout.col_vars => {
            Ok(ForestSchedule::L2)
        }
        (SchedulePolicy::Auto | SchedulePolicy::L4, _) => Ok(ForestSchedule::L4),
    }
}

/// Measured M1 Max crossovers, using only public geometry and worker count.
/// Retain the existing one-worker cases for wider words; the new cases were
/// measured with one-bit words. Other worker counts keep the shared fallback.
fn apple_single_schedule(
    layout: &IntegerMatrixLayout,
    depth: usize,
    threads: usize,
) -> Option<ForestSchedule> {
    // Full-product 2^20..=2^22, W=1, ten workers: L8 recomputation outweighs
    // its memory savings on M1 Max. Keep this exception to measured shapes.
    if threads == 10
        && layout.word_bits == 1
        && matches!((depth, layout.col_vars), (17, 10) | (18, 10 | 11))
    {
        return Some(ForestSchedule::L4);
    }
    if depth != 13 {
        return None;
    }
    match (threads, layout.word_bits, layout.col_vars) {
        (1, _, 12..=14) | (1, 1, 15..=16) | (10, 1, 12) => Some(ForestSchedule::L2),
        (10, 1, 13..=16) => Some(ForestSchedule::L4),
        _ => None,
    }
}

pub(super) fn configured(
    p: &IntegerMatrixLayout,
    path: ForestPath,
) -> Result<ForestSchedule, UnsupportedSchedule> {
    let policy = SchedulePolicy::from_env().expect("valid F2_FOREST_SCHEDULE");
    #[cfg(feature = "parallel")]
    let threads = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let threads = 1;
    let schedule = resolve_schedule(policy, p, path, threads)?;
    #[cfg(feature = "bench-internals")]
    {
        if let Some(records) = RECORDS.lock().expect("schedule recorder lock").as_mut() {
            let record = ScheduleRecord {
                path,
                row_vars: p.row_vars,
                col_vars: p.col_vars,
                word_bits: p.word_bits,
                threads,
                schedule,
            };
            if !records.contains(&record) {
                records.push(record);
            }
        }
    }
    Ok(schedule)
}
impl ForestSchedule {
    pub const fn name(self) -> &'static str {
        match self {
            Self::L2 => "l2",
            Self::L4 => "l4",
            Self::L8 => "l8",
        }
    }
}
#[cfg(feature = "bench-internals")]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ScheduleRecord {
    pub path: ForestPath,
    pub row_vars: usize,
    pub col_vars: usize,
    pub word_bits: usize,
    pub threads: usize,
    pub schedule: ForestSchedule,
}
#[cfg(feature = "bench-internals")]
static RECORDS: std::sync::Mutex<Option<Vec<ScheduleRecord>>> = std::sync::Mutex::new(None);
#[cfg(feature = "bench-internals")]
pub fn start_recording() {
    *RECORDS.lock().expect("schedule recorder lock") = Some(Vec::with_capacity(32));
}
#[cfg(feature = "bench-internals")]
pub fn take_records() -> Vec<ScheduleRecord> {
    let mut records = RECORDS
        .lock()
        .expect("schedule recorder lock")
        .take()
        .unwrap_or_default();
    records.sort();
    records
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_policy_covers_measured_crossovers_and_multi_eligibility() {
        let apple = cfg!(all(target_arch = "aarch64", target_os = "macos"));
        for (row_vars, col_vars, word_bits, threads, path, expected) in [
            (15, 7, 1, 1, ForestPath::Single, ForestSchedule::L2),
            (12, 7, 8, 1, ForestPath::Single, ForestSchedule::L2),
            (17, 9, 1, 4, ForestPath::Single, ForestSchedule::L2),
            (17, 9, 1, 10, ForestPath::Single, ForestSchedule::L8),
            (15, 7, 4, 8, ForestPath::Single, ForestSchedule::L4),
            (7, 15, 1, 1, ForestPath::Single, ForestSchedule::L4),
            (12, 15, 1, 10, ForestPath::Single, ForestSchedule::L4),
            (13, 12, 1, 8, ForestPath::Single, ForestSchedule::L8),
            (
                13,
                14,
                1,
                10,
                ForestPath::Single,
                if apple {
                    ForestSchedule::L4
                } else {
                    ForestSchedule::L8
                },
            ),
            (15, 7, 1, 1, ForestPath::Multi, ForestSchedule::L4),
        ] {
            let layout = IntegerMatrixLayout {
                row_vars,
                col_vars,
                word_bits,
            };
            assert_eq!(
                resolve_schedule(SchedulePolicy::Auto, &layout, path, threads),
                Ok(expected)
            );
            // An explicit supported request always overrides the automatic choice.
            assert_eq!(
                resolve_schedule(SchedulePolicy::L4, &layout, path, threads),
                Ok(ForestSchedule::L4)
            );
        }
    }

    #[test]
    fn apple_crossovers_are_limited_to_measured_geometry() {
        let apple = cfg!(all(target_arch = "aarch64", target_os = "macos"));
        for (rows, cols, bits, threads, path, apple_schedule) in [
            (13, 12, 1, 1, ForestPath::Single, ForestSchedule::L2),
            (13, 13, 1, 1, ForestPath::Single, ForestSchedule::L2),
            (13, 14, 1, 1, ForestPath::Single, ForestSchedule::L2),
            (10, 13, 8, 1, ForestPath::Single, ForestSchedule::L2),
            (13, 15, 1, 1, ForestPath::Single, ForestSchedule::L2),
            (13, 16, 1, 1, ForestPath::Single, ForestSchedule::L2),
            (13, 12, 1, 10, ForestPath::Single, ForestSchedule::L2),
            (13, 13, 1, 10, ForestPath::Single, ForestSchedule::L4),
            (13, 14, 1, 10, ForestPath::Single, ForestSchedule::L4),
            (13, 15, 1, 10, ForestPath::Single, ForestSchedule::L4),
            (13, 16, 1, 10, ForestPath::Single, ForestSchedule::L4),
            (13, 17, 1, 1, ForestPath::Single, ForestSchedule::L8),
            (13, 17, 1, 10, ForestPath::Single, ForestSchedule::L8),
            (10, 15, 8, 1, ForestPath::Single, ForestSchedule::L8),
            (10, 13, 8, 10, ForestPath::Single, ForestSchedule::L8),
            (14, 13, 1, 1, ForestPath::Single, ForestSchedule::L8),
            (14, 13, 1, 10, ForestPath::Single, ForestSchedule::L8),
            (13, 13, 1, 2, ForestPath::Single, ForestSchedule::L8),
            (13, 13, 1, 4, ForestPath::Single, ForestSchedule::L8),
            (13, 13, 1, 8, ForestPath::Single, ForestSchedule::L8),
            (13, 13, 1, 11, ForestPath::Single, ForestSchedule::L8),
            (13, 13, 1, 1, ForestPath::Multi, ForestSchedule::L8),
            (13, 13, 1, 10, ForestPath::Multi, ForestSchedule::L8),
        ] {
            let layout = IntegerMatrixLayout {
                row_vars: rows,
                col_vars: cols,
                word_bits: bits,
            };
            assert_eq!(
                resolve_schedule(SchedulePolicy::Auto, &layout, path, threads),
                Ok(if apple {
                    apple_schedule
                } else {
                    ForestSchedule::L8
                })
            );
            for (policy, expected) in [
                (SchedulePolicy::L4, ForestSchedule::L4),
                (SchedulePolicy::L8, ForestSchedule::L8),
            ] {
                assert_eq!(
                    resolve_schedule(policy, &layout, path, threads),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn explicit_multi_schedule_is_never_silently_substituted() {
        let p = IntegerMatrixLayout {
            row_vars: 15,
            col_vars: 2,
            word_bits: 1,
        };
        assert_eq!(
            resolve_schedule(SchedulePolicy::L2, &p, ForestPath::Multi, 8),
            Err(UnsupportedSchedule::MultiL2)
        );
        for policy in [SchedulePolicy::Auto, SchedulePolicy::L4, SchedulePolicy::L8] {
            assert!(resolve_schedule(policy, &p, ForestPath::Multi, 8).is_ok());
        }
        for (policy, expected) in [
            (SchedulePolicy::L2, ForestSchedule::L2),
            (SchedulePolicy::L4, ForestSchedule::L4),
            (SchedulePolicy::L8, ForestSchedule::L8),
        ] {
            assert_eq!(
                resolve_schedule(policy, &p, ForestPath::Single, 8),
                Ok(expected)
            );
        }
        let shallow = IntegerMatrixLayout { row_vars: 4, ..p };
        assert_eq!(
            resolve_schedule(SchedulePolicy::L8, &shallow, ForestPath::Single, 1),
            Err(UnsupportedSchedule::ShallowL8 { depth: 4 })
        );
        assert!("typo".parse::<SchedulePolicy>().is_err());
    }

    #[test]
    fn apple_full_product_exception_preserves_other_shapes_and_overrides() {
        // Exhaust the neighboring shapes, word widths, worker counts and paths:
        // the full-product exception must not become a general L8 replacement.
        for rows in 14..=19 {
            for cols in 9..=12 {
                for bits in [1, 8] {
                    for threads in [1, 4, 8, 10, 11] {
                        for path in [ForestPath::Single, ForestPath::Multi] {
                            let layout = IntegerMatrixLayout {
                                row_vars: rows,
                                col_vars: cols,
                                word_bits: bits,
                            };
                            let depth = rows + bits.ilog2() as usize;
                            let measured = cfg!(all(target_arch = "aarch64", target_os = "macos"))
                                && path == ForestPath::Single
                                && threads == 10
                                && bits == 1
                                && matches!((rows, cols), (17, 10) | (18, 10 | 11));
                            let expected = if measured {
                                ForestSchedule::L4
                            } else if depth + cols >= 25 && threads > 4 {
                                ForestSchedule::L8
                            } else if path == ForestPath::Single && threads <= 4 {
                                ForestSchedule::L2
                            } else {
                                ForestSchedule::L4
                            };
                            assert_eq!(
                                resolve_schedule(SchedulePolicy::Auto, &layout, path, threads),
                                Ok(expected),
                                "{layout:?}, {path:?}, threads={threads}"
                            );
                            assert_eq!(
                                resolve_schedule(SchedulePolicy::L8, &layout, path, threads),
                                Ok(ForestSchedule::L8)
                            );
                        }
                    }
                }
            }
        }
    }
}
