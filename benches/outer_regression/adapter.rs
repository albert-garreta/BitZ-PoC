//! Current production entrypoints and the public generic API, without arithmetic copies.
use super::*;
use crate::sumcheck::{
    UngrindedRoundBoundary,
    outer::{self, arithmetic},
};
pub(super) const REVISION: &str = "current";
pub(super) fn prepare(f: &Field, k: usize) -> Option<outer::PreparedUnivariateSkip<Elem>> {
    (k > 0).then(|| outer::prepare_univariate_skip(f, k as u8).unwrap())
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
        let (lo, hi) = super::super::raw_monty::make_equality_factors_raw(f, tau);
        macro_rules! prove {
            ($p:expr) => {
                Proof::Ordinary(
                    arithmetic::prove_native_zerocheck(t, f, tau, lo, hi, $p)
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
        let (lo, hi) = super::super::raw_monty::make_equality_factors_raw(f, tau);
        Proof::Skip(
            outer::native_skip::prove_native_skip(
                t,
                f,
                k as u8,
                tau,
                lo,
                hi,
                NativeProducts {
                    az: a,
                    bz: b,
                    cz: c,
                },
            )
            .unwrap()
            .proof,
        )
    } else {
        let eq = super::super::matrix::make_equality_factors(tau, f).unwrap();
        Proof::Skip(
            outer::univariate::prove_field_skip_with_factors(
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
pub(super) fn generic(
    input: &Inputs,
    f: &Field,
    t: &mut Blake3Transcript,
    tau: &[Elem],
    k: usize,
    prepared: &Option<outer::PreparedUnivariateSkip<Elem>>,
) -> Proof {
    let wrap = |o: outer::OuterOutput<Elem>| OuterSumcheckProof {
        sumcheck: o.proof,
        az_mle_claim: o.evaluations.ax,
        bz_mle_claim: o.evaluations.bx,
        cz_mle_claim: o.evaluations.cx,
    };
    macro_rules! prove {
        ($a:expr,$b:expr,$c:expr) => {
            if k == 0 {
                Proof::Ordinary(wrap(
                    outer::prove_outer_zerocheck_from_slices(
                        f,
                        t,
                        tau,
                        $a,
                        $b,
                        $c,
                        &mut UngrindedRoundBoundary,
                    )
                    .unwrap(),
                ))
            } else {
                let o = outer::prove_outer_zerocheck_with_skip_from_slices(
                    f,
                    t,
                    prepared.as_ref().unwrap(),
                    tau,
                    $a,
                    $b,
                    $c,
                    &mut UngrindedRoundBoundary,
                )
                .unwrap();
                Proof::Skip(UnivariateSkipOuterSumcheckProof {
                    skip: o.prefix,
                    tail: wrap(o.tail),
                })
            }
        };
    }
    match input {
        Inputs::U32 {
            narrow_a,
            narrow_b,
            c,
            ..
        } => prove!(narrow_a, narrow_b, c),
        Inputs::U64 { a, b, c, .. } => prove!(a, b, c),
        Inputs::U128 { a, b, c, .. } => prove!(a, b, c),
    }
}
