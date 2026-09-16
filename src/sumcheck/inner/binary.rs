//! The post-GKR quadratic batch, retaining its evaluation-form wire codec.
use crate::piop::sumcheck::{
    multi_degree::MultiDegreeSumcheckProof,
    prover::{NatEvaluatedPolyWithoutConstant, ProverMsg},
};
use crate::poly::coefficient::PolynomialField;
use crate::transcript::traits::Transcript;
use field::{Gf128 as F, Gf128Ops, Gf128Product, PreparedGf128Mul};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

// The first pass also obtains the claims. Subsequent passes need only c0,c2.
fn first_round([a, b]: &[Vec<F>; 2]) -> [F; 3] {
    let zero = || [Gf128Product::zero(); 3];
    let accumulate = |mut acc: [Gf128Product; 3], (a, b): (&[F], &[F])| {
        acc[0] ^= a[0].mul_unreduced(b[0]);
        acc[1] ^= (a[0] + a[1]).mul_unreduced(b[0] + b[1]);
        acc[2] ^= a[1].mul_unreduced(b[1]);
        acc
    };
    #[cfg(feature = "parallel")]
    let acc = if crate::sumcheck::arithmetic::should_parallelize(a.len() / 2) {
        a.par_chunks_exact(2)
            .zip(b.par_chunks_exact(2))
            .fold(zero, accumulate)
            .reduce(zero, |mut a, b| {
                for i in 0..3 {
                    a[i] ^= b[i];
                }
                a
            })
    } else {
        a.chunks_exact(2)
            .zip(b.chunks_exact(2))
            .fold(zero(), accumulate)
    };
    #[cfg(not(feature = "parallel"))]
    let acc = a
        .chunks_exact(2)
        .zip(b.chunks_exact(2))
        .fold(zero(), accumulate);
    acc.map(Gf128Product::reduce)
}

/// In-place low-coordinate fold and next-round coefficients. Reading each four
/// source entries before writing the two outputs keeps the original allocation.
fn fold_round([a, b]: &mut [Vec<F>; 2], challenge: F) -> [F; 2] {
    let n = a.len() / 2;
    let prepared = PreparedGf128Mul::new(challenge);
    if n == 1 {
        a[0] = a[0] + prepared.mul(&(a[0] + a[1]));
        b[0] = b[0] + prepared.mul(&(b[0] + b[1]));
        a.truncate(1);
        b.truncate(1);
        return [F::ZERO; 2];
    }
    let mut acc = [Gf128Product::zero(); 2];
    for i in 0..n / 2 {
        let j = 4 * i;
        let a0 = a[j] + prepared.mul(&(a[j] + a[j + 1]));
        let a1 = a[j + 2] + prepared.mul(&(a[j + 2] + a[j + 3]));
        let b0 = b[j] + prepared.mul(&(b[j] + b[j + 1]));
        let b1 = b[j + 2] + prepared.mul(&(b[j + 2] + b[j + 3]));
        a[2 * i] = a0;
        a[2 * i + 1] = a1;
        b[2 * i] = b0;
        b[2 * i + 1] = b1;
        acc[0] ^= a0.mul_unreduced(b0);
        acc[1] ^= (a0 + a1).mul_unreduced(b0 + b1);
    }
    a.truncate(n);
    b.truncate(n);
    acc.map(Gf128Product::reduce)
}

/// Shared ordinary rounds, with the historical post-GKR header, interpolation
/// nodes, and challenge reabsorption. The enclosing protocol binds the claims.
pub(crate) fn prove_batch(
    transcript: &mut impl Transcript,
    mut pairs: Vec<[Vec<F>; 2]>,
    num_vars: usize,
) -> (MultiDegreeSumcheckProof<F>, Vec<F>) {
    assert!(num_vars > 0 && !pairs.is_empty());
    assert!(
        pairs
            .iter()
            .all(|[a, b]| a.len() == 1usize << num_vars && b.len() == a.len())
    );
    let mut buf = [0; 16];
    for n in [num_vars, pairs.len()]
        .into_iter()
        .chain(core::iter::repeat_n(2, pairs.len()))
    {
        transcript.absorb_random_field(&F::interpolation_node(n as u64, &()), &mut buf);
    }
    let first: Vec<_> = pairs.iter().map(first_round).collect();
    let claimed_sums: Vec<_> = first.iter().map(|[even, _, odd]| *even + *odd).collect();
    let mut claims = claimed_sums.clone();
    let mut coefficients: Vec<_> = first.iter().map(|[c0, c2, _]| [*c0, *c2]).collect();
    let mut messages = vec![[F::ZERO; 3]; pairs.len()];
    let mut group_messages: Vec<Vec<ProverMsg<F>>> = (0..pairs.len())
        .map(|_| Vec::with_capacity(num_vars))
        .collect();
    let mut point = Vec::with_capacity(num_vars);
    let node = F::interpolation_node(2, &());
    super::engine::drive(
        &Gf128Ops,
        transcript,
        num_vars,
        &mut point,
        &mut claims,
        &mut coefficients,
        &mut messages,
        |transcript, _, messages| {
            for (group, &[c0, c1, c2]) in group_messages.iter_mut().zip(messages) {
                let tail_evaluations = vec![c0 + c1 + c2, c0 + node * (c1 + node * c2)];
                transcript.absorb_random_field_slice(&tail_evaluations, &mut buf);
                group.push(ProverMsg(NatEvaluatedPolyWithoutConstant {
                    tail_evaluations,
                }));
            }
            let r = transcript.get_field_challenge(&());
            transcript.absorb_random_field(&r, &mut buf);
            Ok::<_, core::convert::Infallible>(r)
        },
        |_, r, next| {
            #[cfg(feature = "parallel")]
            {
                pairs
                    .par_iter_mut()
                    .zip(next.par_iter_mut())
                    .for_each(|(pair, out)| *out = fold_round(pair, *r));
            }
            #[cfg(not(feature = "parallel"))]
            {
                for (pair, out) in pairs.iter_mut().zip(next) {
                    *out = fold_round(pair, *r);
                }
            }
            Ok(())
        },
    )
    .unwrap();
    for (claim, [a, b]) in claims.iter().zip(&pairs) {
        assert_eq!(*claim, a[0] * b[0], "inner terminal claim");
    }
    (
        MultiDegreeSumcheckProof::quadratic(group_messages, claimed_sums),
        point,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piop::sumcheck::multi_degree::{MultiDegreeSumcheck, MultiDegreeSumcheckGroup};
    use crate::poly::mle::DenseMultilinearExtension;
    use crate::transcript::Blake3Transcript;
    #[test]
    fn evaluation_codec_and_shared_challenges_match_legacy() {
        for (vars, k) in [(1, 1), (3, 3), (14, 2)] {
            let pairs: Vec<[Vec<F>; 2]> = (0..k)
                .map(|k| {
                    core::array::from_fn(|j| {
                        (0..1 << vars)
                            .map(|i| {
                                F::new(
                                    (i as u64).wrapping_mul(0x9e3779b97f4a7c15) + (k + j) as u64,
                                    i as u64,
                                )
                            })
                            .collect()
                    })
                })
                .collect();
            let groups = pairs
                .iter()
                .map(|pair| {
                    MultiDegreeSumcheckGroup::new(
                        2,
                        pair.iter()
                            .map(|v| {
                                DenseMultilinearExtension::from_evaluations_vec(
                                    vars,
                                    v.iter()
                                        .map(|f| field::Uint::from_words([f.lo, f.hi]))
                                        .collect(),
                                    field::Uint::ZERO,
                                )
                            })
                            .collect(),
                        Box::new(|v: &[F]| v[0] * v[1]),
                    )
                })
                .collect();
            let mut old = Blake3Transcript::new();
            let (expected, states) =
                MultiDegreeSumcheck::prove_as_subprotocol(&mut old, groups, vars, &());
            let mut new = Blake3Transcript::new();
            let (actual, point) = prove_batch(&mut new, pairs, vars);
            assert_eq!(actual, expected);
            assert_eq!(point, states[0].randomness);
            assert_eq!(
                new.get_field_challenge::<F>(&()),
                old.get_field_challenge::<F>(&())
            );
            let verified = actual
                .verify_as_subprotocol(&mut Blake3Transcript::new(), vars, &vec![2; k], &())
                .unwrap();
            assert_eq!(point, verified.point());
        }
    }
}
