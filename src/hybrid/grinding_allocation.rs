//! Work-optimal split of a composed error budget across grinding groups.
//!
//! Every group is a set of challenge boundaries ground at one common
//! difficulty `bits`. Its union-bound error contribution is
//! `raw_error * 2^-bits`, and the prover's expected work is `sites * 2^bits`
//! BLAKE3 compressions (one per attempted nonce). Under the repository's
//! computational grinding model only the composed sum matters, so the split
//! is free as long as it stays under the budget. Allocating it uniformly per
//! group (each group to `2^-(target + margin)`) overpays for the groups whose
//! raw error is largest; the optimum gives each group a share proportional
//! to `sqrt(raw_error * sites)`.
//!
//! The allocator walks the integer optimum by marginal analysis: starting
//! at every group's minimum, it adds one bit where the halved error buys the
//! most per added work, until the total meets the budget, then removes any
//! bit that the budget no longer needs. All arithmetic is IEEE-754 sums and
//! products by exact powers of two in a fixed order, so the prover and the
//! verifier derive identical difficulties from identical inputs.

/// Challenge boundaries that share one difficulty.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GrindingGroup {
    /// Union of the group's raw (unground) challenge errors.
    pub raw_error: f64,
    /// Grinding sites; only weights the work objective.
    pub sites: f64,
    /// Smallest admissible difficulty.
    pub min_bits: u32,
}

/// Largest difficulty any group may receive.
pub(crate) const MAX_BITS: u32 = 32;

/// Difficulties minimizing total expected work subject to
/// `sum(raw_error * 2^-bits) <= budget`.
pub(crate) fn allocate(groups: &[GrindingGroup], budget: f64) -> Result<Vec<u32>, String> {
    if !budget.is_finite() || budget <= 0.0 {
        return Err("grinding budget must be positive".into());
    }
    if groups.iter().any(|g| {
        !g.raw_error.is_finite() || g.raw_error < 0.0 || !(g.sites > 0.0) || g.min_bits > MAX_BITS
    }) {
        return Err("invalid grinding group".into());
    }
    let mut bits: Vec<u32> = groups.iter().map(|g| g.min_bits).collect();
    let error = |bits: &[u32]| -> f64 {
        groups
            .iter()
            .zip(bits)
            .map(|(g, &b)| g.raw_error * 2f64.powi(-(b as i32)))
            .sum()
    };
    while error(&bits) > budget {
        // Halving group i's error buys raw*2^-b/2 for sites*2^b more work.
        let mut best: Option<(usize, f64)> = None;
        for (i, g) in groups.iter().enumerate() {
            if bits[i] >= MAX_BITS || g.raw_error == 0.0 {
                continue;
            }
            let b = bits[i] as i32;
            let ratio = g.raw_error * 2f64.powi(-b) / (g.sites * 2f64.powi(b));
            if best.is_none_or(|(_, r)| ratio > r) {
                best = Some((i, ratio));
            }
        }
        let (i, _) = best.ok_or("grinding budget is unreachable within the difficulty cap")?;
        bits[i] += 1;
    }
    // The last increment may overshoot; return bits the budget does not need,
    // most expensive first.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| {
        let cost = |i: usize| groups[i].sites * 2f64.powi(bits[i] as i32);
        cost(b).total_cmp(&cost(a)).then(a.cmp(&b))
    });
    for i in order {
        while bits[i] > groups[i].min_bits {
            bits[i] -= 1;
            if error(&bits) > budget {
                bits[i] += 1;
                break;
            }
        }
    }
    Ok(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(raw_log2: i32, sites: f64, min_bits: u32) -> GrindingGroup {
        GrindingGroup {
            raw_error: 2f64.powi(raw_log2),
            sites,
            min_bits,
        }
    }

    #[test]
    fn meets_budget_and_respects_minimums() {
        let groups = [group(-110, 1.0, 3), group(-120, 300.0, 0), group(-128, 1.0, 9)];
        let budget = 2f64.powi(-128);
        let bits = allocate(&groups, budget).unwrap();
        let error: f64 = groups
            .iter()
            .zip(&bits)
            .map(|(g, &b)| g.raw_error * 2f64.powi(-(b as i32)))
            .sum();
        assert!(error <= budget);
        assert!(bits.iter().zip(&groups).all(|(&b, g)| b >= g.min_bits));
        // Dropping any single bit breaks the budget (no slack bits remain).
        for i in 0..groups.len() {
            if bits[i] == groups[i].min_bits {
                continue;
            }
            let mut fewer = bits.clone();
            fewer[i] -= 1;
            let error: f64 = groups
                .iter()
                .zip(&fewer)
                .map(|(g, &b)| g.raw_error * 2f64.powi(-(b as i32)))
                .sum();
            assert!(error > budget, "group {i} kept an unneeded bit");
        }
    }

    #[test]
    fn beats_per_group_margins_and_is_deterministic() {
        // Falcon-512-like groups: dominant UDR folds, a 273-round GKR and a
        // 106-block Keccak prefix (raw errors over GF(2^128)).
        let gf = |numerator: f64| numerator * 2f64.powi(-128);
        let groups = [
            group(-110, 4.0, 0),
            GrindingGroup {
                raw_error: gf(807.0),
                sites: 273.0,
                min_bits: 0,
            },
            GrindingGroup {
                raw_error: gf(1096.0),
                sites: 106.0,
                min_bits: 0,
            },
        ];
        let budget = 2f64.powi(-128);
        let bits = allocate(&groups, budget).unwrap();
        assert_eq!(bits, allocate(&groups, budget).unwrap());
        let work = |bits: &[u32]| -> f64 {
            groups
                .iter()
                .zip(bits)
                .map(|(g, &b)| g.sites * 2f64.powi(b as i32))
                .sum()
        };
        let error = |bits: &[u32]| -> f64 {
            groups
                .iter()
                .zip(bits)
                .map(|(g, &b)| g.raw_error * 2f64.powi(-(b as i32)))
                .sum()
        };
        // The per-group margin rule: every group to 2^-(target + 8).
        let margins: Vec<u32> = groups
            .iter()
            .map(|g| (g.raw_error.log2() + 136.0).ceil().max(0.0) as u32)
            .collect();
        assert!(error(&margins) <= budget && error(&bits) <= budget);
        assert!(work(&bits) * 4.0 < work(&margins));
    }

    #[test]
    fn rejects_unreachable_budgets() {
        assert!(allocate(&[group(-10, 1.0, 0)], 2f64.powi(-128)).is_err());
        assert!(allocate(&[group(-110, 1.0, 0)], 0.0).is_err());
    }
}
