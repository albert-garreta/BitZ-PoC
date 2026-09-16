use super::ordinary::prove_outer_sumcheck_direct_reference;
use super::*;
use crate::piop::spartan::{R1csProductMles, SpartanField, matrix::eq_table};
use crate::poly::mle::DenseMultilinearExtension;
use crate::sumcheck::{SumcheckError, UngrindedRoundBoundary};
use crate::transcript::{
    Blake3Transcript,
    traits::{ConstTranscribable, Transcript},
};
use field::{Fp, FpCtx, IntegerEmbedding, RingOps, Uint, Z};

fn field() -> FpCtx<2> {
    Fp::<2>::make_cfg(&Uint::from((1u128 << 100) - 15)).unwrap()
}
fn fe(f: &FpCtx<2>, v: u64) -> Fp<2> {
    f.from_integer(&v)
}
fn tables(f: &FpCtx<2>, a: &[u64], b: &[u64], c: &[u128]) -> R1csProductMles<Fp<2>> {
    let n = a.len().ilog2() as usize;
    let mle = |v| DenseMultilinearExtension {
        num_vars: n,
        evaluations: v,
    };
    R1csProductMles {
        az: mle(a.iter().map(|x| f.from_integer(x)).collect()),
        bz: mle(b.iter().map(|x| f.from_integer(x)).collect()),
        cz: mle(c.iter().map(|x| f.from_integer(x)).collect()),
    }
}
#[test]
fn mixed_inputs_match_direct_cubic_and_transcript() {
    let f = field();
    for n in 0..=7 {
        let a: Vec<_> = (0..1 << n).map(|i| i as u64 * 3 + 7).collect();
        let b: Vec<_> = a.iter().map(|x| x + 11).collect();
        let c: Vec<_> = a.iter().map(|x| u128::from(*x) * 17).collect();
        let tau: Vec<_> = (0..n)
            .map(|i| {
                fe(
                    &f,
                    if i % 3 == 0 {
                        0
                    } else if i % 3 == 1 {
                        1
                    } else {
                        9
                    },
                )
            })
            .collect();
        let products = tables(&f, &a, &b, &c);
        let eq = eq_table(&tau, &f).unwrap();
        let claim = (0..a.len()).fold(f.zero(), |sum, i| {
            f.add(
                &sum,
                &f.mul(
                    &eq[i],
                    &f.sub(
                        &f.mul(&products.az.evaluations[i], &products.bz.evaluations[i]),
                        &products.cz.evaluations[i],
                    ),
                ),
            )
        });
        let mut reference = Blake3Transcript::new();
        let old = prove_outer_sumcheck_direct_reference(&mut reference, claim, &tau, products, &f)
            .unwrap();
        let mut prover = Blake3Transcript::new();
        let out = prove_outer_sumcheck_from_slices(
            &f,
            &mut prover,
            claim,
            &tau,
            &a,
            &b,
            &c,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(out.proof, old.proof.sumcheck);
        assert_eq!(out.point, old.eval_points);
        assert_eq!(prover.state_digest(), reference.state_digest());
        let mut verifier = Blake3Transcript::new();
        let verified = verify_outer_sumcheck(
            &f,
            &mut verifier,
            claim,
            &tau,
            &out.proof,
            out.evaluations,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(verified.final_claim, out.final_claim);
        assert_eq!(prover.state_digest(), verifier.state_digest());
    }
}
#[test]
fn zero_prefix_and_owned_field_inputs_match_ordinary() {
    let f = field();
    for n in 0..=7 {
        let a: Vec<_> = (0..1 << n).map(|i| u64::MAX - i as u64).collect();
        let b: Vec<_> = a.iter().rev().copied().collect();
        let c: Vec<_> = a
            .iter()
            .zip(&b)
            .map(|(a, b)| u128::from(*a) * u128::from(*b))
            .collect();
        let tau = vec![fe(&f, 7); n];
        let mut ordinary = Blake3Transcript::new();
        let expected = prove_outer_sumcheck_from_slices(
            &f,
            &mut ordinary,
            f.zero(),
            &tau,
            &a,
            &b,
            &c,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let mut zero = Blake3Transcript::new();
        let got = prove_outer_zerocheck_from_slices(
            &f,
            &mut zero,
            &tau,
            &a,
            &b,
            &c,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        assert_eq!(got, expected);
        assert_eq!(ordinary.state_digest(), zero.state_digest());
        let projected = tables(&f, &a, &b, &c);
        let inputs = OuterInputs {
            ax: projected.az.evaluations,
            bx: projected.bz.evaluations,
            cx: projected.cz.evaluations,
        };
        let mut owned = Blake3Transcript::new();
        assert_eq!(
            prove_outer_zerocheck(&f, &mut owned, &tau, inputs, &mut UngrindedRoundBoundary)
                .unwrap(),
            expected
        );
        assert_eq!(owned.state_digest(), ordinary.state_digest());
    }
}
#[test]
fn skip_all_widths_matches_preserved_native_kernel() {
    let f = field();
    for k in 1..=4u8 {
        for n in usize::from(k)..=usize::from(k) + 2 {
            let a: Vec<_> = (0..1 << n).map(|i| i as u64 + 3).collect();
            let b: Vec<_> = a.iter().map(|x| x + 11).collect();
            let c: Vec<_> = a.iter().zip(&b).map(|(a, b)| a * b).collect();
            let tau = vec![fe(&f, 7); n - usize::from(k)];
            let prepared = prepare_univariate_skip(&f, k).unwrap();
            let mut prover = Blake3Transcript::new();
            let got = prove_outer_zerocheck_with_skip_from_slices(
                &f,
                &mut prover,
                &prepared,
                &tau,
                &a,
                &b,
                &c,
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            let (lo, hi) = crate::piop::spartan::raw_monty::make_equality_factors_raw(&f, &tau);
            let native = arithmetic::NativeProducts {
                az: &a,
                bz: &b,
                cz: &c,
            };
            let message =
                native_skip::encoded_native_message(&f, usize::from(k), &lo, &hi, native, &f, &f)
                    .unwrap();
            let prefix =
                UnivariateSkipProof::from_ordered_message(usize::from(k), message).unwrap();
            assert_eq!(prefix, got.prefix);
            let mut expected_transcript = Blake3Transcript::new();
            let reduction = prefix
                .verify_reduction(&mut expected_transcript, &f)
                .unwrap();
            let folded =
                native_skip::fold_encoded_lagrange(usize::from(k), native, &reduction.z, &f)
                    .unwrap();
            let tail = arithmetic::prove_encoded(
                &mut expected_transcript,
                &f,
                &f,
                reduction.q_at_z,
                &tau,
                lo,
                hi,
                folded,
            )
            .unwrap();
            assert_eq!(got.tail, tail.into());
            assert_eq!(prover.state_digest(), expected_transcript.state_digest());
            let mut verifier = Blake3Transcript::new();
            let verified = verify_outer_zerocheck_with_skip(
                &f,
                &mut verifier,
                &prepared,
                &tau,
                &got.prefix,
                &got.tail.proof,
                got.tail.evaluations,
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            assert_eq!(verified.prefix_challenge, got.prefix_challenge);
            assert_eq!(prover.state_digest(), verifier.state_digest());
        }
    }
}
#[test]
fn signed_and_four_limb_inputs() {
    let f = field();
    let tau = [fe(&f, 7)];
    let a = [
        Z::<2>::from_twos_complement_words([u64::MAX, u64::MAX]),
        Z::from_twos_complement_words([2, 0]),
    ];
    let c = [
        Z::<4>::from_twos_complement_words([1, 0, 0, 0]),
        Z::from_twos_complement_words([4, 0, 0, 0]),
    ];
    let mut transcript = Blake3Transcript::new();
    prove_outer_zerocheck_from_slices(
        &f,
        &mut transcript,
        &tau,
        &a,
        &a,
        &c,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    let a = [u128::MAX, u128::MAX - 1];
    let c = a.map(|a| {
        let p =
            field::WideMul::mul_wide(&field::IntegerOps, &Uint::<2>::from(a), &Uint::<2>::from(a));
        *p.checked_resize_ct::<4>().value()
    });
    prove_outer_zerocheck_from_slices(
        &f,
        &mut Blake3Transcript::new(),
        &tau,
        &a,
        &a,
        &c,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
}
#[test]
fn invalid_shape_and_singleton_claim_rejected() {
    let f = field();
    for len in [0, 3, 5] {
        let mut t = Blake3Transcript::new();
        let before = t.state_digest();
        let input = vec![1u64; len];
        assert_eq!(
            prove_outer_sumcheck_from_slices(
                &f,
                &mut t,
                f.zero(),
                &[],
                &input,
                &input,
                &input,
                &mut UngrindedRoundBoundary
            )
            .unwrap_err(),
            SumcheckError::InvalidProductDimensions
        );
        assert_eq!(before, t.state_digest());
    }
    assert_eq!(
        prove_outer_sumcheck_from_slices(
            &f,
            &mut Blake3Transcript::new(),
            f.zero(),
            &[],
            &[2u64],
            &[3u64],
            &[7u64],
            &mut UngrindedRoundBoundary
        )
        .unwrap_err(),
        SumcheckError::InvalidTerminalClaim
    );
    assert!(prepare_univariate_skip(&f, 0).is_err());
    assert!(prepare_univariate_skip(&f, 5).is_err());
}
struct ZeroChallenges;
impl Transcript for ZeroChallenges {
    fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
        T::read_transcription_bytes_exact(&vec![0; T::NUM_BYTES])
    }
    fn fill_sampling_bytes(&mut self, out: &mut [u8]) {
        out.fill(0);
    }
    fn absorb_inner(&mut self, _: &[u8]) {}
}
#[test]
fn vanishing_equality_scale_is_carried_without_division() {
    let f = field();
    let tau = [f.one(), f.zero()];
    let output = prove_outer_zerocheck_from_slices(
        &f,
        &mut ZeroChallenges,
        &tau,
        &[2u64, 3, 4, 5],
        &[3u64, 4, 5, 6],
        &[6u64, 12, 20, 30],
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    assert_eq!(output.point, vec![f.zero(); 2]);
    assert_eq!(output.final_claim, f.zero());
    verify_outer_sumcheck(
        &f,
        &mut ZeroChallenges,
        f.zero(),
        &tau,
        &output.proof,
        output.evaluations,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
}

#[test]
fn zero_weighted_claim_does_not_imply_rowwise_zerocheck() {
    let f = field();
    let tau = [f.zero()];
    let a = [2u64, 3];
    let b = [3u64, 4];
    let c = [6u64, 13];
    let out = prove_outer_sumcheck_from_slices(
        &f,
        &mut Blake3Transcript::new(),
        f.zero(),
        &tau,
        &a,
        &b,
        &c,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    verify_outer_sumcheck(
        &f,
        &mut Blake3Transcript::new(),
        f.zero(),
        &tau,
        &out.proof,
        out.evaluations,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    assert_eq!(
        prove_outer_zerocheck_from_slices(
            &f,
            &mut Blake3Transcript::new(),
            &tau,
            &a,
            &b,
            &c,
            &mut UngrindedRoundBoundary
        )
        .unwrap_err(),
        SumcheckError::InvalidTerminalClaim
    );
}

#[test]
fn encoded_zero_prefix_preserves_grinding_rounds_and_transcript() {
    use crate::sumcheck::boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary};
    struct Domain;
    impl crate::piop::spartan::grinding::GrindingDomain for Domain {
        const DOMAIN: &'static [u8] = b"outer-refactor-test/v1";
    }
    let f = field();
    let a = [2u64, 3, 4, 5, 6, 7, 8, 9];
    let b = [3u64, 4, 5, 6, 7, 8, 9, 10];
    let c: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(a, b)| u128::from(*a) * u128::from(b))
        .collect();
    let products = tables(&f, &a, &b, &c);
    let tau = vec![fe(&f, 3); 3];
    let eq = crate::piop::spartan::matrix::make_equality_factors(&tau, &f).unwrap();
    let mut reference = Blake3Transcript::new();
    let mut reference_boundary = ProverGrindingRoundBoundary::<Domain>::with_round_offset(3, 0);
    let expected = ordinary::prove_field_with_boundary(
        &mut reference,
        f.zero(),
        &tau,
        eq,
        products.clone(),
        &f,
        &f,
        &mut reference_boundary,
    )
    .unwrap();
    let (low, high) = crate::piop::spartan::raw_monty::make_equality_factors_raw(&f, &tau);
    let encoded = arithmetic::RawProducts::from_field(&f, &products);
    let mut prover = Blake3Transcript::new();
    let mut boundary = ProverGrindingRoundBoundary::<Domain>::with_round_offset(3, 0);
    let got = arithmetic::prove_encoded_zerocheck(
        &mut prover,
        &f,
        &tau,
        low,
        high,
        encoded,
        &mut boundary,
    )
    .unwrap();
    assert_eq!(expected, got);
    assert_eq!(prover.state_digest(), reference.state_digest());
    let nonces = boundary.into_nonces();
    assert_eq!(nonces, reference_boundary.into_nonces());
    assert_eq!(nonces.len(), 3);
    let mut verifier = Blake3Transcript::new();
    let mut boundary = VerifierGrindingRoundBoundary::<Domain>::new(3, &nonces);
    let got: OuterOutput<_> = got.into();
    verify_outer_sumcheck(
        &f,
        &mut verifier,
        f.zero(),
        &tau,
        &got.proof,
        got.evaluations,
        &mut boundary,
    )
    .unwrap();
    assert_eq!(prover.state_digest(), verifier.state_digest());
}
