use super::*;
use crate::poly::mle::DenseMultilinearExtension;
use crate::sumcheck::UngrindedRoundBoundary;
use crate::transcript::Blake3Transcript;
use field::{Fp, FpCtx, IntegerEmbedding, MergeAccumulator, Uint, Z};

pub(super) fn field() -> FpCtx<2> {
    field::create_prime_field(Uint::from_words([u64::MAX, (1 << 63) - 1]))
}

fn compare_native<T: Copy + Send + Sync>(values: Vec<T>)
where
    FpCtx<2>: PreparedLinearCombination<T> + IntegerEmbedding<T> + BatchMulAcc<Fp<2>, T>,
    FpCtx<2>: Reduce<<FpCtx<2> as BatchMulAcc<Fp<2>, T>>::Accumulator, Output = Fp<2>>,
{
    let f = field();
    let weights: Vec<_> = (0..values.len())
        .map(|i| <FpCtx<2> as IntegerEmbedding<u64>>::from_integer(&f, &(i as u64 * 13 + 7)))
        .collect();
    let projected: Vec<_> = values.iter().map(|v| f.from_integer(v)).collect();
    let claim = <FpCtx<2> as Reduce<field::FpProductAcc<2>>>::reduce(
        &f,
        <FpCtx<2> as BatchMulAcc<Fp<2>>>::batch_mul_acc(&f, &weights, &projected),
    );
    let mut actual_t = Blake3Transcript::new();
    let actual = prove_inner_sumcheck(
        &f,
        &mut actual_t,
        claim,
        values,
        weights.clone(),
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    let mut reference_t = Blake3Transcript::new();
    let expected = reference::prove_inner_sumcheck(
        &mut reference_t,
        claim,
        DenseMultilinearExtension::from_evaluations_vec(
            weights.len().ilog2() as usize,
            weights,
            f.zero(),
        ),
        DenseMultilinearExtension::from_evaluations_vec(
            projected.len().ilog2() as usize,
            projected,
            f.zero(),
        ),
        &f,
    )
    .unwrap();
    assert_eq!(actual.proof, expected.sumcheck.proof);
    assert_eq!(actual.point, expected.sumcheck.eval_points);
    assert_eq!(
        actual.terminal_evaluations,
        [
            expected.batched_matrix_evaluation,
            expected.witness_evaluation
        ]
    );
    assert_eq!(
        squeeze_field::<Fp<2>, _>(&mut actual_t, &f).unwrap(),
        squeeze_field::<Fp<2>, _>(&mut reference_t, &f).unwrap()
    );
    let mut verifier = Blake3Transcript::new();
    assert_eq!(
        actual
            .proof
            .verify(&mut verifier, claim, actual.point.len(), &f)
            .unwrap(),
        (actual.point, actual.final_claim)
    );
}

#[test]
fn native_inputs_match_independent_dense_rounds() {
    compare_native(vec![0u32, u32::MAX, 7, 0, 1, 2, 3, 4]);
    compare_native(vec![u64::MAX, 0, 0, u64::MAX]);
    compare_native(vec![0u128, u128::MAX, u128::MAX, 0, 1, 2, 3, 4]);
    compare_native(vec![
        Z::<2>::from(i128::MIN),
        Z::from(i128::MAX),
        Z::ZERO,
        Z::from(-1i128),
    ]);
    compare_native(vec![
        Uint::from_words([u64::MAX; 4]),
        Uint::ZERO,
        Uint::ONE,
        Uint::from_words([0, 0, 0, 1]),
    ]);
    compare_native(vec![
        Z::from_twos_complement_words([0, 0, 0, 1 << 63]),
        Z::<4>::from(-1i64),
    ]);
    compare_native(vec![9u64]);
    // Exercise Rayon thresholds and a full continuation, not just tiny scalar tables.
    compare_native(
        (0..(1 << 14))
            .map(|i| (i as u64).wrapping_mul(0x9e3779b97f4a7c15))
            .collect(),
    );
}

#[test]
fn shared_challenges_preserve_individual_claims() {
    let f = field();
    let values = [[2u64, 3, 5, 7], [11, 13, 17, 19], [23, 29, 31, 37]].map(Vec::from);
    let weights = core::array::from_fn::<_, 3, _>(|k| {
        (0..4)
            .map(|i| f.from_integer(&((k * 4 + i + 1) as u64)))
            .collect::<Vec<_>>()
    });
    let claims =
        core::array::from_fn(|i| Reduce::reduce(&f, f.batch_mul_acc(&weights[i], &values[i])));
    let out = prove_batched_inner_sumcheck(
        &f,
        &mut Blake3Transcript::new(),
        &claims,
        values,
        weights,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    let mut verifier = Blake3Transcript::new();
    let verified = SumcheckProof::verify_batch_with_round_boundary(
        core::array::from_fn(|i| &out.proofs[i]),
        &mut verifier,
        &claims,
        2,
        &f,
        &mut UngrindedRoundBoundary,
    )
    .unwrap();
    assert_eq!(verified, (out.point.clone(), out.final_claims));
    for (claim, [w, v]) in out.final_claims.iter().zip(out.terminal_evaluations) {
        assert_eq!(*claim, f.mul(&w, &v));
    }
    let mut corrupt = out.proofs.clone();
    corrupt[1].round_polynomials[0][0] = f.add(&corrupt[1].round_polynomials[0][0], &f.one());
    assert!(
        SumcheckProof::verify_batch_with_round_boundary(
            core::array::from_fn(|i| &corrupt[i]),
            &mut Blake3Transcript::new(),
            &claims,
            2,
            &f,
            &mut UngrindedRoundBoundary
        )
        .is_err()
    );
}

#[test]
fn invalid_shapes_do_not_change_the_transcript_and_singletons_check_the_claim() {
    let f = field();
    let mut transcript = Blake3Transcript::new();
    assert!(
        prove_inner_sumcheck(
            &f,
            &mut transcript,
            f.zero(),
            vec![1u64, 2, 3],
            vec![f.one(); 3],
            &mut UngrindedRoundBoundary
        )
        .is_err()
    );
    assert_eq!(
        squeeze_field::<Fp<2>, _>(&mut transcript, &f).unwrap(),
        squeeze_field::<Fp<2>, _>(&mut Blake3Transcript::new(), &f).unwrap()
    );
    assert_eq!(
        prove_inner_sumcheck(
            &f,
            &mut Blake3Transcript::new(),
            f.zero(),
            vec![7u64],
            vec![f.one()],
            &mut UngrindedRoundBoundary
        ),
        Err(SumcheckError::InvalidTerminalClaim)
    );
    assert!(
        prove_batched_inner_sumcheck::<_, u64, 0>(
            &f,
            &mut Blake3Transcript::new(),
            &[],
            [],
            [],
            &mut UngrindedRoundBoundary
        )
        .is_err()
    );
}

#[test]
fn streaming_prepared_reduction_matches_batch_and_worker_merges() {
    let f = field();
    let a: Vec<_> = (0..41u64).map(|i| f.from_integer(&i)).collect();
    let b: Vec<_> = (0..41u64).map(|i| f.from_integer(&(i * i + 3))).collect();
    let mut left = field::FpProductAcc::<2>::zero();
    let mut right = field::FpProductAcc::<2>::zero();
    for (i, (a, b)) in a.iter().zip(&b).enumerate() {
        f.mul_acc(if i % 2 == 0 { &mut left } else { &mut right }, a, b);
    }
    left.merge_assign(&right);
    let reduce = <FpCtx<2> as Reduce<field::FpProductAcc<2>>>::prepare_reduce(&f, a.len());
    assert_eq!(reduce(left), Reduce::reduce(&f, f.batch_mul_acc(&a, &b)));
    let gf = field::Gf128Ops;
    let values: Vec<_> = (0..17u64).map(|v| field::Gf128::from([v, 0])).collect();
    let fixed: Vec<_> = values
        .iter()
        .map(|v| field::PreparedGf128Mul::new(*v))
        .collect();
    let mut a = gf.batch_mul_acc(&values[..8], &fixed[..8]);
    let b = gf.batch_mul_acc(&values[8..], &fixed[8..]);
    a.merge_assign(&b);
    assert_eq!(
        Reduce::reduce(&gf, a),
        Reduce::reduce(&gf, gf.batch_mul_acc(&values, &values))
    );
}
