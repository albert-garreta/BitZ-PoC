//! Reduce expected PCS work without increasing its existing summed error.
use crate::ligerito_flock::grinding_plan::GrindingPlan;

/// The old equal-per-block allocation supplies the budget. Greedily buy the
/// largest error reduction per extra expected hash, starting at native minima.
/// Accept only a strictly cheaper plan whose upward-rounded error sum is below
/// the old plan's downward-rounded sum. The choice depends only on public
/// geometry/error bounds, never on a statement, nonce, or transcript seed.
pub(super) fn rebalance(mut plan: GrindingPlan) -> GrindingPlan {
    let original: Vec<_> = plan.blocks.iter().map(|b| b.bits).collect();
    let mut bits: Vec<_> = plan
        .blocks
        .iter()
        .map(|b| b.native_bits.unwrap_or(0))
        .collect();
    if bits == original {
        return plan;
    }
    let budget = error_bound(&plan, &original, false);
    if budget <= 0. || !budget.is_finite() {
        return plan;
    }
    while error_bound(&plan, &bits, true) > budget {
        let mut best = None;
        let mut best_gain = -1.;
        for (index, (block, &g)) in plan.blocks.iter().zip(&bits).enumerate() {
            if g < 32 {
                // A zero-bit boundary performs no nonce search: enabling it
                // costs two expected hashes. Later increments cost 2^g.
                let exponent = if g == 0 { -2 } else { -2 * g as i32 - 1 };
                let gain = block.raw_error * 2f64.powi(exponent);
                if gain > best_gain {
                    best = Some(index);
                    best_gain = gain;
                }
            }
        }
        let Some(index) = best else { return plan };
        bits[index] += 1;
    }
    if expected_work(&bits) < expected_work(&original) {
        for (block, bits) in plan.blocks.iter_mut().zip(bits) {
            block.bits = bits;
        }
    }
    plan
}

fn expected_work(bits: &[u32]) -> u128 {
    bits.iter()
        .map(|&g| if g == 0 { 0 } else { 1u128 << g })
        .sum()
}

fn error_bound(plan: &GrindingPlan, bits: &[u32], upper: bool) -> f64 {
    plan.blocks.iter().zip(bits).fold(0., |sum, (block, &g)| {
        let term = block.raw_error * 2f64.powi(-(g as i32));
        if upper {
            (sum + term.next_up()).next_up()
        } else {
            (sum + term.next_down().max(0.)).next_down().max(0.)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ligerito_flock::{LigeritoSelection, grinding_plan::ChallengeBlock};
    use num_bigint::BigUint;

    // Exact dyadic interpretation of the existing f64 raw bounds, independent
    // of the optimizer's directed-rounding test. SCALE also covers subnormals.
    fn exact_error(plan: &GrindingPlan) -> BigUint {
        plan.blocks
            .iter()
            .map(|block| {
                let raw = block.raw_error.to_bits();
                let exponent = ((raw >> 52) & 0x7ff) as i32;
                let fraction = raw & ((1u64 << 52) - 1);
                let (mantissa, power) = if exponent == 0 {
                    (fraction, -1074)
                } else {
                    ((1u64 << 52) | fraction, exponent - 1023 - 52)
                };
                BigUint::from(mantissa) << (1106 + power - block.bits as i32) as usize
            })
            .sum()
    }

    fn check(plan: GrindingPlan) -> bool {
        let optimized = rebalance(plan.clone());
        assert_eq!(optimized.blocks.len(), plan.blocks.len());
        for (a, b) in plan.blocks.iter().zip(&optimized.blocks) {
            assert_eq!(
                (&a.label, a.native_bits, a.raw_error),
                (&b.label, b.native_bits, b.raw_error)
            );
            assert!(b.bits >= b.native_bits.unwrap_or(0) && b.bits <= 32);
        }
        assert!(exact_error(&optimized) <= exact_error(&plan));
        let old: Vec<_> = plan.blocks.iter().map(|b| b.bits).collect();
        let new: Vec<_> = optimized.blocks.iter().map(|b| b.bits).collect();
        assert!(expected_work(&new) <= expected_work(&old));
        let repeated = rebalance(plan);
        assert_eq!(
            new,
            repeated.blocks.iter().map(|b| b.bits).collect::<Vec<_>>()
        );
        expected_work(&new) < expected_work(&old)
    }

    #[test]
    fn public_pcs_shapes_preserve_exact_error_and_native_minima() {
        let mut improved = 0;
        for log in 13..=28 {
            for target in [100, 128] {
                let resolved = LigeritoSelection::MATCHED_UDR.resolve(log, target).unwrap();
                let initial = GrindingPlan::resolve(resolved.security(), target as u32).unwrap();
                let target = target + 2 + initial.blocks.len().next_power_of_two().ilog2() as usize;
                // Larger general-purpose PCS shapes can exceed the existing
                // 32-bit work cap; no Falcon configuration uses such a plan.
                let Ok(plan) = GrindingPlan::resolve(resolved.security(), target as u32) else {
                    continue;
                };
                assert!(rebalance(plan.clone()).matches(resolved.security()));
                improved += usize::from(check(plan));
            }
        }
        assert!(improved > 0);
    }

    #[test]
    fn heterogeneous_bounds_and_saturated_minima_preserve_exact_error() {
        for seed in 0..64u32 {
            let blocks = (0..32u32)
                .map(|i| {
                    let exponent = 105 + (i * 13 + seed * 7) % 24;
                    let native = (i + seed) % 8;
                    ChallengeBlock {
                        label: format!("test/{i}"),
                        native_bits: Some(native),
                        raw_error: 2f64.powi(-(exponent as i32)) + 2f64.powi(-128),
                        bits: (136 - exponent).max(native),
                    }
                })
                .collect();
            assert!(check(GrindingPlan::scripted(blocks)));
        }
        for bits in [0, 32] {
            let plan = GrindingPlan::scripted(vec![ChallengeBlock {
                label: "fixed".into(),
                native_bits: Some(bits),
                raw_error: 2f64.powi(-128),
                bits,
            }]);
            assert!(!check(plan));
        }
    }

    #[test]
    fn prepared_falcon_keeps_its_full_security_bound() {
        use super::super::{N, PreparedFalconHybrid};
        for batch in [1, 3, 16, 1024] {
            for target in [100, 128] {
                // Not every ring-extension profile supports both targets.
                let Ok(prepared) = PreparedFalconHybrid::prepare(batch, target, 1024) else {
                    continue;
                };
                let config = prepared.ligerito.security();
                let initial = GrindingPlan::resolve(config, target as u32).unwrap();
                let pcs_target =
                    target + 2 + initial.blocks.len().next_power_of_two().ilog2() as usize;
                let original = GrindingPlan::resolve(config, pcs_target as u32).unwrap();
                let old: Vec<_> = original.blocks.iter().map(|b| b.bits).collect();
                let new: Vec<_> = prepared
                    .pcs_grinding
                    .blocks
                    .iter()
                    .map(|b| b.bits)
                    .collect();
                assert!(exact_error(&prepared.pcs_grinding) <= exact_error(&original));
                assert!(prepared.security().algebraic_bits >= target as f64);
                if target == 100 {
                    assert_eq!(new, old);
                }
                if batch == 1024 {
                    eprintln!(
                        "PCS expected work: N={N} target={target} old={} new={}",
                        expected_work(&old),
                        expected_work(&new)
                    );
                }
            }
        }
    }
}
