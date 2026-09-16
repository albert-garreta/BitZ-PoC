//! Adapter for a3450385. Calls historical kernels; does not alter arithmetic.
use super::super::{
    raw_monty as raw, univariate_skip as skip, univariate_skip_native as native_skip,
};
use super::*;
pub(super) const REVISION: &str = "a3450385e2a90115add370a3c05e5127db0b3a87";
pub(super) fn prepare(_: &Field, _: usize) {}
pub(super) fn generic(
    _: &Inputs,
    _: &Field,
    _: &mut Blake3Transcript,
    _: &[Elem],
    _: usize,
    _: &(),
) -> Proof {
    panic!("no generic API at this revision")
}
pub(super) fn production(
    input: &Inputs,
    f: &Field,
    t: &mut Blake3Transcript,
    tau: &[Elem],
    n: usize,
    k: usize,
) -> Proof {
    if k == 0 {
        let (lo, hi) = raw::make_equality_factors_raw(f, tau);
        macro_rules! prove {
            ($p:expr) => {
                Proof::Ordinary(
                    raw::prove_outer_native_raw(t, f, f, f.zero(), tau, lo, hi, $p)
                        .unwrap()
                        .proof,
                )
            };
        }
        match input {
            Inputs::U32 { a, b, c, .. } => prove!(NativeProducts {
                az: a,
                bz: b,
                cz: c
            }),
            Inputs::U64 {
                a,
                b,
                lo: cl,
                hi: ch,
                ..
            } => prove!(NativeWideProducts::new(a, b, cl, ch, 1 << n)),
            Inputs::U128 {
                a,
                b,
                lo: cl,
                hi: ch,
                ..
            } => prove!(NativeWideProducts::new(a, b, cl, ch, 1 << n)),
        }
    } else if let Inputs::U32 { a, b, c, .. } = input {
        let (lo, hi) = raw::make_equality_factors_raw(f, tau);
        let products = NativeProducts {
            az: a,
            bz: b,
            cz: c,
        };
        let message =
            native_skip::compute_u32_native_skip_message_raw(f, k, &lo, &hi, products, f, f)
                .unwrap();
        let prefix = skip::UnivariateSkipProof::from_ordered_message(k, message).unwrap();
        let reduction = prefix.verify_reduction(t, f).unwrap();
        let folded = native_skip::fold_u32_native_prefix_raw(k, products, &reduction.z, f).unwrap();
        let tail =
            raw::prove_outer_field_raw(t, f, f, reduction.q_at_z, tau, lo, hi, folded).unwrap();
        Proof::Skip(UnivariateSkipOuterSumcheckProof {
            skip: prefix,
            tail: tail.proof,
        })
    } else {
        let eq = super::super::matrix::make_equality_factors(tau, f).unwrap();
        Proof::Skip(
            skip::prove_univariate_skip_outer_sumcheck_with_reducer(
                t,
                k,
                tau,
                eq,
                input.project(f, n),
                f,
                f,
            )
            .unwrap()
            .proof,
        )
    }
}

pub(super) fn reset_measurements() {}
pub(super) fn take_measurements() -> Option<[u64;4]> { None }
