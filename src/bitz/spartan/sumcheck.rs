//! Their `spartan/src/sumcheck.rs`: the reusable sumcheck verifier and
//! Spartan's two provers — the cubic equality-weighted outer sumcheck over
//! `eq(tau, x)·(Az(x)·Bz(x) − Cz(x))` and the quadratic inner sumcheck over
//! `D(y)·h(y)`. Rounds bind the lowest remaining index bit (adjacent pairs);
//! each round sends the full coefficient vector with `c1` reconstructed
//! from `g(0) + g(1) = claim`, as a public message, then squeezes one `Fq`.
//!
//! Every coefficient is an exact field sum, so the fused fold-and-next-round
//! passes here (theirs, without the low/high equality split — one dense
//! `eq` table is 8 MB at 2^19 rows) produce their bytes.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::super::fq::Fq;
use super::super::transcript::{ProverState, VerifierState};
use super::poly::eq_eval;
use crate::{cfg_chunks_mut, cfg_into_iter};

/// Pairs per parallel block.
const PAIRS_PER_BLOCK: usize = 1 << 12;

/// Failures produced while reducing or checking a sumcheck claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SumcheckError {
    EmptyRoundPolynomial,
    InvalidRoundCount { expected: usize, actual: usize },
    InvalidRoundClaim { round: usize },
    InvalidTerminalClaim,
    InvalidProductDimensions,
    InvalidEqualityDimensions,
}

/// Round polynomials in coefficient form; `COEFFS` is the degree plus one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SumcheckProof<const COEFFS: usize> {
    pub round_polynomials: Vec<[Fq; COEFFS]>,
}

impl<const COEFFS: usize> SumcheckProof<COEFFS> {
    /// Verifies the round reductions and returns `(r, final_claim)`; the
    /// caller checks the protocol-specific terminal identity.
    pub fn verify(
        &self,
        transcript: &mut VerifierState<'_>,
        initial_claim: Fq,
        expected_rounds: usize,
    ) -> Result<(Vec<Fq>, Fq), SumcheckError> {
        if COEFFS == 0 {
            return Err(SumcheckError::EmptyRoundPolynomial);
        }
        let actual_rounds = self.round_polynomials.len();
        if actual_rounds != expected_rounds {
            return Err(SumcheckError::InvalidRoundCount {
                expected: expected_rounds,
                actual: actual_rounds,
            });
        }
        let mut current_claim = initial_claim;
        let mut eval_points = Vec::with_capacity(expected_rounds);
        for (round, coefficients) in self.round_polynomials.iter().enumerate() {
            // The typed proof owns the message; the transcript absorbs its
            // canonical encoding as a public message.
            transcript.public_message(coefficients);
            let at_zero = coefficients[0];
            let at_one: Fq = coefficients.iter().copied().sum();
            if at_zero + at_one != current_claim {
                return Err(SumcheckError::InvalidRoundClaim { round });
            }
            let challenge = transcript.squeeze_fq();
            current_claim = evaluate_polynomial(coefficients, challenge);
            eval_points.push(challenge);
        }
        Ok((eval_points, current_claim))
    }
}

/// Dense Boolean-row tables for `Az`, `Bz` and `Cz` (their `R1csProductMles`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Products {
    pub az: Vec<Fq>,
    pub bz: Vec<Fq>,
    pub cz: Vec<Fq>,
}

/// Proof of the equality-weighted R1CS residual sum: the cubic rounds and
/// the terminal `[Az(r_x), Bz(r_x), Cz(r_x)]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuterSumcheckProof {
    pub sumcheck: SumcheckProof<4>,
    pub az_mle_claim: Fq,
    pub bz_mle_claim: Fq,
    pub cz_mle_claim: Fq,
}

/// Local result of the outer prover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuterSumcheckOutput {
    pub proof: OuterSumcheckProof,
    /// `r_x`, in round order.
    pub eval_points: Vec<Fq>,
    pub final_claim: Fq,
}

/// What the outer verifier returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuterSumcheckVerifierOutput {
    pub eval_points: Vec<Fq>,
    pub az_mle_claim: Fq,
    pub bz_mle_claim: Fq,
    pub cz_mle_claim: Fq,
}

/// Local result of the inner prover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerSumcheckOutput {
    pub proof: SumcheckProof<3>,
    /// `r_y`, in round order.
    pub eval_points: Vec<Fq>,
    pub final_claim: Fq,
    /// `D(r_y)`.
    pub batched_matrix_evaluation: Fq,
    /// `h(r_y)`.
    pub witness_evaluation: Fq,
}

impl OuterSumcheckProof {
    /// Verifies the outer reduction and its terminal R1CS identity.
    pub fn verify(
        &self,
        transcript: &mut VerifierState<'_>,
        initial_claim: Fq,
        tau: &[Fq],
    ) -> Result<OuterSumcheckVerifierOutput, SumcheckError> {
        let (eval_points, final_claim) = self.sumcheck.verify(transcript, initial_claim, tau.len())?;
        transcript.public_message(&[self.az_mle_claim, self.bz_mle_claim, self.cz_mle_claim]);
        let expected_claim = eq_eval(tau, &eval_points)
            * (self.az_mle_claim * self.bz_mle_claim - self.cz_mle_claim);
        if final_claim != expected_claim {
            return Err(SumcheckError::InvalidTerminalClaim);
        }
        Ok(OuterSumcheckVerifierOutput {
            eval_points,
            az_mle_claim: self.az_mle_claim,
            bz_mle_claim: self.bz_mle_claim,
            cz_mle_claim: self.cz_mle_claim,
        })
    }
}

/// Proves the outer sumcheck: `initial_claim = Σ_x eq[x]·(az[x]·bz[x] − cz[x])`.
/// `eq` is the dense equality table over the row domain.
pub fn prove_outer_sumcheck(
    transcript: &mut ProverState,
    initial_claim: Fq,
    eq: Vec<Fq>,
    products: &Products,
) -> Result<OuterSumcheckOutput, SumcheckError> {
    let len = products.az.len();
    if !len.is_power_of_two() || products.bz.len() != len || products.cz.len() != len {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    if eq.len() != len {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    let num_vars = len.ilog2() as usize;

    let mut eq = eq;
    let mut az = products.az.clone();
    let mut bz = products.bz.clone();
    let mut cz = products.cz.clone();
    // Each destination is allocated once at half the table size; after a
    // fold, swapping makes the old input the next scratch table.
    let mut eq_scratch = vec![Fq::ZERO; len / 2];
    let mut az_scratch = vec![Fq::ZERO; len / 2];
    let mut bz_scratch = vec![Fq::ZERO; len / 2];
    let mut cz_scratch = vec![Fq::ZERO; len / 2];

    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut without_linear = outer_coefficients(&eq, &az, &bz, &cz);

    for _ in 0..num_vars {
        let challenge = recover_and_sample(
            transcript,
            &mut current_claim,
            without_linear,
            &mut round_polynomials,
            &mut eval_points,
        );
        let next_len = az.len() / 2;
        eq_scratch.truncate(next_len);
        az_scratch.truncate(next_len);
        bz_scratch.truncate(next_len);
        cz_scratch.truncate(next_len);
        if next_len > 1 {
            without_linear = fold_outer_and_next(
                (&eq, &az, &bz, &cz),
                (&mut eq_scratch, &mut az_scratch, &mut bz_scratch, &mut cz_scratch),
                challenge,
            );
        } else {
            fold_table(&eq, &mut eq_scratch, challenge);
            fold_table(&az, &mut az_scratch, challenge);
            fold_table(&bz, &mut bz_scratch, challenge);
            fold_table(&cz, &mut cz_scratch, challenge);
        }
        std::mem::swap(&mut eq, &mut eq_scratch);
        std::mem::swap(&mut az, &mut az_scratch);
        std::mem::swap(&mut bz, &mut bz_scratch);
        std::mem::swap(&mut cz, &mut cz_scratch);
    }

    let (az_mle_claim, bz_mle_claim, cz_mle_claim) = (az[0], bz[0], cz[0]);
    debug_assert_eq!(current_claim, eq[0] * (az_mle_claim * bz_mle_claim - cz_mle_claim));
    transcript.public_message(&[az_mle_claim, bz_mle_claim, cz_mle_claim]);

    Ok(OuterSumcheckOutput {
        proof: OuterSumcheckProof {
            sumcheck: SumcheckProof { round_polynomials },
            az_mle_claim,
            bz_mle_claim,
            cz_mle_claim,
        },
        eval_points,
        final_claim: current_claim,
    })
}

/// Proves the inner sumcheck: `initial_claim = Σ_y matrix[y]·witness[y]`.
pub fn prove_inner_sumcheck(
    transcript: &mut ProverState,
    initial_claim: Fq,
    matrix: Vec<Fq>,
    witness: &[Fq],
) -> Result<InnerSumcheckOutput, SumcheckError> {
    let len = matrix.len();
    if !len.is_power_of_two() || witness.len() != len {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    let num_vars = len.ilog2() as usize;
    let mut matrix = matrix;
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    let witness_evaluation = if num_vars > 0 {
        let mut matrix_scratch = vec![Fq::ZERO; len / 2];
        // Round zero folds the witness straight out of the caller's table;
        // from then on two buffers alternate as fold input and output.
        let mut witness_in = vec![Fq::ZERO; len / 2];
        let mut witness_out = vec![Fq::ZERO; len / 4];
        let mut without_linear = inner_coefficients(&matrix, witness);

        for round in 0..num_vars {
            let challenge = recover_and_sample(
                transcript,
                &mut current_claim,
                without_linear,
                &mut round_polynomials,
                &mut eval_points,
            );
            let next_len = matrix.len() / 2;
            matrix_scratch.truncate(next_len);
            if round == 0 {
                debug_assert_eq!(witness_in.len(), next_len);
                if next_len == 1 {
                    matrix_scratch[0] = interpolate_pair(matrix[0], matrix[1], challenge);
                    witness_in[0] = interpolate_pair(witness[0], witness[1], challenge);
                } else {
                    without_linear = fold_inner_and_next(
                        &matrix,
                        witness,
                        &mut matrix_scratch,
                        &mut witness_in,
                        challenge,
                    );
                }
                std::mem::swap(&mut matrix, &mut matrix_scratch);
            } else {
                witness_out.truncate(next_len);
                if next_len == 1 {
                    matrix_scratch[0] = interpolate_pair(matrix[0], matrix[1], challenge);
                    witness_out[0] = interpolate_pair(witness_in[0], witness_in[1], challenge);
                } else {
                    without_linear = fold_inner_and_next(
                        &matrix,
                        &witness_in,
                        &mut matrix_scratch,
                        &mut witness_out,
                        challenge,
                    );
                }
                std::mem::swap(&mut matrix, &mut matrix_scratch);
                std::mem::swap(&mut witness_in, &mut witness_out);
            }
        }
        witness_in[0]
    } else {
        witness[0]
    };

    let batched_matrix_evaluation = matrix[0];
    debug_assert_eq!(current_claim, batched_matrix_evaluation * witness_evaluation);

    Ok(InnerSumcheckOutput {
        proof: SumcheckProof { round_polynomials },
        eval_points,
        final_claim: current_claim,
        batched_matrix_evaluation,
        witness_evaluation,
    })
}

#[inline]
fn add_coefficients<const N: usize>(left: [Fq; N], right: [Fq; N]) -> [Fq; N] {
    std::array::from_fn(|index| left[index] + right[index])
}

/// The contribution of one adjacent pair to `[c0, c2, c3]` of the cubic
/// round polynomial `eq(X)·(az(X)·bz(X) − cz(X))` (their `cubic_contribution`).
#[inline]
fn cubic_contribution(eq: [Fq; 2], az: [Fq; 2], bz: [Fq; 2], cz: [Fq; 2]) -> [Fq; 3] {
    let [eq_zero, eq_one] = eq;
    let [az_zero, az_one] = az;
    let [bz_zero, bz_one] = bz;
    let [cz_zero, cz_one] = cz;
    let eq_delta = eq_one - eq_zero;
    let az_delta = az_one - az_zero;
    let bz_delta = bz_one - bz_zero;
    let cz_delta = cz_one - cz_zero;
    let residual_zero = az_zero * bz_zero - cz_zero;
    let residual_linear = az_zero * bz_delta + az_delta * bz_zero - cz_delta;
    let residual_quadratic = az_delta * bz_delta;
    [
        eq_zero * residual_zero,
        eq_zero * residual_quadratic + eq_delta * residual_linear,
        eq_delta * residual_quadratic,
    ]
}

/// The contribution of one adjacent pair to `[c0, c2]` of the quadratic
/// round polynomial `matrix(X)·witness(X)`.
#[inline]
fn inner_pair_coefficients(matrix: [Fq; 2], witness: [Fq; 2]) -> [Fq; 2] {
    let [matrix_zero, matrix_one] = matrix;
    let [witness_zero, witness_one] = witness;
    [
        matrix_zero * witness_zero,
        (matrix_one - matrix_zero) * (witness_one - witness_zero),
    ]
}

/// Restores `c1` from `g(0) + g(1) = claim`: `c1 = claim − 2c0 − c2 − … − cD`.
#[inline]
fn reconstruct_round_coefficients<const IN: usize, const OUT: usize>(
    current_claim: Fq,
    without_linear: [Fq; IN],
) -> [Fq; OUT] {
    assert!(IN >= 1);
    assert_eq!(OUT, IN + 1);
    let mut coefficients = [Fq::ZERO; OUT];
    coefficients[0] = without_linear[0];
    coefficients[2..].copy_from_slice(&without_linear[1..]);
    let at_one_without_c1: Fq = coefficients.iter().copied().sum();
    coefficients[1] = current_claim - coefficients[0] - at_one_without_c1;
    coefficients
}

#[inline]
fn evaluate_polynomial<const COEFFS: usize>(coefficients: &[Fq; COEFFS], point: Fq) -> Fq {
    coefficients
        .iter()
        .rev()
        .copied()
        .fold(Fq::ZERO, |value, coefficient| value * point + coefficient)
}

/// Completes and records one round: reconstruct `c1`, absorb the polynomial
/// as a public message, squeeze `r_i`, update the claim to `g_i(r_i)`.
fn recover_and_sample<const IN: usize, const OUT: usize>(
    transcript: &mut ProverState,
    current_claim: &mut Fq,
    without_linear: [Fq; IN],
    round_polynomials: &mut Vec<[Fq; OUT]>,
    eval_points: &mut Vec<Fq>,
) -> Fq {
    let coefficients = reconstruct_round_coefficients::<IN, OUT>(*current_claim, without_linear);
    debug_assert_eq!(
        *current_claim,
        coefficients[0] + coefficients.iter().copied().sum::<Fq>()
    );
    transcript.public_message(&coefficients);
    let challenge = transcript.squeeze_fq();
    *current_claim = evaluate_polynomial(&coefficients, challenge);
    round_polynomials.push(coefficients);
    eval_points.push(challenge);
    challenge
}

#[inline]
fn interpolate_pair(zero: Fq, one: Fq, challenge: Fq) -> Fq {
    zero + challenge * (one - zero)
}

/// `[c0, c2, c3]` over the adjacent pairs of the current tables.
fn outer_coefficients(eq: &[Fq], az: &[Fq], bz: &[Fq], cz: &[Fq]) -> [Fq; 3] {
    let pairs = eq.len() / 2;
    let blocks = pairs.div_ceil(PAIRS_PER_BLOCK);
    let partials: Vec<[Fq; 3]> = cfg_into_iter!(0..blocks)
        .map(|block| {
            let start = block * PAIRS_PER_BLOCK;
            let end = (start + PAIRS_PER_BLOCK).min(pairs);
            (start..end).fold([Fq::ZERO; 3], |sum, pair| {
                let i = 2 * pair;
                add_coefficients(
                    sum,
                    cubic_contribution(
                        [eq[i], eq[i + 1]],
                        [az[i], az[i + 1]],
                        [bz[i], bz[i + 1]],
                        [cz[i], cz[i + 1]],
                    ),
                )
            })
        })
        .collect();
    partials.into_iter().fold([Fq::ZERO; 3], add_coefficients)
}

/// `[c0, c2]` over the adjacent pairs of the current tables.
fn inner_coefficients(matrix: &[Fq], witness: &[Fq]) -> [Fq; 2] {
    let pairs = matrix.len() / 2;
    let blocks = pairs.div_ceil(PAIRS_PER_BLOCK);
    let partials: Vec<[Fq; 2]> = cfg_into_iter!(0..blocks)
        .map(|block| {
            let start = block * PAIRS_PER_BLOCK;
            let end = (start + PAIRS_PER_BLOCK).min(pairs);
            (start..end).fold([Fq::ZERO; 2], |sum, pair| {
                let i = 2 * pair;
                add_coefficients(
                    sum,
                    inner_pair_coefficients([matrix[i], matrix[i + 1]], [witness[i], witness[i + 1]]),
                )
            })
        })
        .collect();
    partials.into_iter().fold([Fq::ZERO; 2], add_coefficients)
}

/// Folds one table into an already sized destination.
fn fold_table(input: &[Fq], output: &mut [Fq], challenge: Fq) {
    debug_assert_eq!(input.len(), 2 * output.len());
    cfg_chunks_mut!(output, PAIRS_PER_BLOCK)
        .enumerate()
        .for_each(|(block, out)| {
            let start = block * PAIRS_PER_BLOCK;
            for (j, value) in out.iter_mut().enumerate() {
                let i = 2 * (start + j);
                *value = interpolate_pair(input[i], input[i + 1], challenge);
            }
        });
}

/// Folds the four outer tables at `challenge` and accumulates the next
/// round's `[c0, c2, c3]` from the freshly folded pairs in the same pass.
fn fold_outer_and_next(
    (eq, az, bz, cz): (&[Fq], &[Fq], &[Fq], &[Fq]),
    (eq_out, az_out, bz_out, cz_out): (&mut [Fq], &mut [Fq], &mut [Fq], &mut [Fq]),
    challenge: Fq,
) -> [Fq; 3] {
    let out_block = 2 * PAIRS_PER_BLOCK;
    debug_assert_eq!(eq.len(), 2 * eq_out.len());
    let partials: Vec<[Fq; 3]> = cfg_chunks_mut!(eq_out, out_block)
        .zip(cfg_chunks_mut!(az_out, out_block))
        .zip(cfg_chunks_mut!(bz_out, out_block))
        .zip(cfg_chunks_mut!(cz_out, out_block))
        .enumerate()
        .map(|(block, (((eo, ao), bo), co))| {
            let start = block * 2 * out_block;
            for j in 0..eo.len() {
                let i = start + 2 * j;
                eo[j] = interpolate_pair(eq[i], eq[i + 1], challenge);
                ao[j] = interpolate_pair(az[i], az[i + 1], challenge);
                bo[j] = interpolate_pair(bz[i], bz[i + 1], challenge);
                co[j] = interpolate_pair(cz[i], cz[i + 1], challenge);
            }
            (0..eo.len() / 2).fold([Fq::ZERO; 3], |sum, pair| {
                let j = 2 * pair;
                add_coefficients(
                    sum,
                    cubic_contribution(
                        [eo[j], eo[j + 1]],
                        [ao[j], ao[j + 1]],
                        [bo[j], bo[j + 1]],
                        [co[j], co[j + 1]],
                    ),
                )
            })
        })
        .collect();
    partials.into_iter().fold([Fq::ZERO; 3], add_coefficients)
}

/// Folds both inner tables at `challenge` and accumulates the next round's
/// `[c0, c2]` from the freshly folded pairs in the same pass.
fn fold_inner_and_next(
    matrix: &[Fq],
    witness: &[Fq],
    matrix_out: &mut [Fq],
    witness_out: &mut [Fq],
    challenge: Fq,
) -> [Fq; 2] {
    let out_block = 2 * PAIRS_PER_BLOCK;
    debug_assert_eq!(matrix.len(), 2 * matrix_out.len());
    debug_assert_eq!(witness.len(), 2 * witness_out.len());
    let partials: Vec<[Fq; 2]> = cfg_chunks_mut!(matrix_out, out_block)
        .zip(cfg_chunks_mut!(witness_out, out_block))
        .enumerate()
        .map(|(block, (mo, wo))| {
            let start = block * 2 * out_block;
            for j in 0..mo.len() {
                let i = start + 2 * j;
                mo[j] = interpolate_pair(matrix[i], matrix[i + 1], challenge);
                wo[j] = interpolate_pair(witness[i], witness[i + 1], challenge);
            }
            (0..mo.len() / 2).fold([Fq::ZERO; 2], |sum, pair| {
                let j = 2 * pair;
                add_coefficients(
                    sum,
                    inner_pair_coefficients([mo[j], mo[j + 1]], [wo[j], wo[j + 1]]),
                )
            })
        })
        .collect();
    partials.into_iter().fold([Fq::ZERO; 2], add_coefficients)
}

/// The multilinear extension of `evaluations` at `point`, little-endian
/// (variable 0 = index bit 0), by folding adjacent pairs.
pub fn mle_evaluate(evaluations: &[Fq], point: &[Fq]) -> Fq {
    assert_eq!(evaluations.len(), 1usize << point.len());
    let mut table = evaluations.to_vec();
    for &r in point {
        let half = table.len() / 2;
        for j in 0..half {
            table[j] = interpolate_pair(table[2 * j], table[2 * j + 1], r);
        }
        table.truncate(half);
    }
    table[0]
}

#[cfg(test)]
mod tests {
    use super::super::super::transcript::{build_prover, build_verifier};
    use super::super::poly::eq_table;
    use super::*;

    const SESSION: &[u8] = b"spartan/outer-sumcheck/test";
    const INNER_SESSION: &[u8] = b"spartan/inner-sumcheck/test";

    fn fq(value: u128) -> Fq {
        Fq::new(value)
    }

    fn splitmix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn random(state: &mut u64) -> Fq {
        Fq::new((u128::from(splitmix(state)) << 64) | u128::from(splitmix(state)))
    }

    /// Their `full_coefficient_sumcheck_verifies_and_replays_challenges`.
    #[test]
    fn full_coefficient_sumcheck_verifies_and_replays_challenges() {
        let second_round = [fq(1), fq(2), fq(3), fq(3)];
        let sumcheck = SumcheckProof {
            round_polynomials: vec![[fq(10), fq(0), fq(0), fq(0)], second_round],
        };
        let instance = b"full-coefficient-sumcheck";
        let mut prover = build_prover(SESSION, instance);
        let prover_points: Vec<Fq> = sumcheck
            .round_polynomials
            .iter()
            .map(|round| {
                prover.public_message(round);
                prover.squeeze_fq()
            })
            .collect();
        let next_prover = prover.squeeze_fq();
        let proof = prover.finish();
        assert!(proof.narg_string.is_empty(), "public messages write nothing");

        let mut verifier = build_verifier(SESSION, instance, &proof);
        let (verifier_points, final_claim) = sumcheck.verify(&mut verifier, fq(20), 2).unwrap();
        let next_verifier = verifier.squeeze_fq();
        assert_eq!(verifier_points, prover_points);
        assert_eq!(final_claim, evaluate_polynomial(&second_round, verifier_points[1]));
        assert_eq!(next_verifier, next_prover);
        verifier.check_eof().unwrap();
    }

    #[test]
    fn sumcheck_rejects_an_explicit_bad_c1() {
        let sumcheck = SumcheckProof {
            round_polynomials: vec![[fq(10), fq(0), fq(0), fq(0)], [fq(1), fq(3), fq(3), fq(3)]],
        };
        let proof = super::super::super::transcript::Proof::default();
        let mut verifier = build_verifier(SESSION, b"bad-c1", &proof);
        assert_eq!(
            sumcheck.verify(&mut verifier, fq(20), 2),
            Err(SumcheckError::InvalidRoundClaim { round: 1 })
        );
    }

    fn check_outer(num_vars: usize, seed: u64) {
        let mut state = seed;
        let len = 1usize << num_vars;
        let az: Vec<Fq> = (0..len).map(|_| random(&mut state)).collect();
        let bz: Vec<Fq> = (0..len).map(|_| random(&mut state)).collect();
        let cz: Vec<Fq> = az.iter().zip(&bz).map(|(a, b)| *a * *b).collect();
        let products = Products { az, bz, cz };
        let tau: Vec<Fq> = (0..num_vars).map(|_| random(&mut state)).collect();
        let instance = (num_vars as u64).to_le_bytes();

        let mut prover = build_prover(SESSION, &instance);
        let output =
            prove_outer_sumcheck(&mut prover, Fq::ZERO, eq_table(&tau), &products).unwrap();
        let proof = prover.finish();

        let mut verifier = build_verifier(SESSION, &instance, &proof);
        let checked = output.proof.verify(&mut verifier, Fq::ZERO, &tau).unwrap();
        verifier.check_eof().unwrap();
        assert_eq!(checked.eval_points, output.eval_points);
        assert_eq!(output.proof.sumcheck.round_polynomials.len(), num_vars);
        assert_eq!(output.proof.az_mle_claim, mle_evaluate(&products.az, &output.eval_points));
        assert_eq!(output.proof.bz_mle_claim, mle_evaluate(&products.bz, &output.eval_points));
        assert_eq!(output.proof.cz_mle_claim, mle_evaluate(&products.cz, &output.eval_points));
    }

    #[test]
    fn outer_sumcheck_round_trips_at_every_small_width() {
        for num_vars in [0, 1, 2, 3, 5, 10, 14] {
            check_outer(num_vars, 0x5a17_a11c + num_vars as u64);
        }
    }

    #[test]
    fn outer_sumcheck_rejects_a_bad_terminal_evaluation() {
        let mut state = 9;
        let len = 8;
        let az: Vec<Fq> = (0..len).map(|_| random(&mut state)).collect();
        let bz: Vec<Fq> = (0..len).map(|_| random(&mut state)).collect();
        let cz: Vec<Fq> = az.iter().zip(&bz).map(|(a, b)| *a * *b).collect();
        let products = Products { az, bz, cz };
        let tau: Vec<Fq> = (0..3).map(|_| random(&mut state)).collect();
        let mut prover = build_prover(SESSION, b"bad-terminal");
        let mut output =
            prove_outer_sumcheck(&mut prover, Fq::ZERO, eq_table(&tau), &products).unwrap();
        let proof = prover.finish();
        output.proof.cz_mle_claim += Fq::ONE;
        let mut verifier = build_verifier(SESSION, b"bad-terminal", &proof);
        assert_eq!(
            output.proof.verify(&mut verifier, Fq::ZERO, &tau),
            Err(SumcheckError::InvalidTerminalClaim)
        );
    }

    fn check_inner(num_vars: usize, seed: u64) {
        let mut state = seed;
        let len = 1usize << num_vars;
        let matrix: Vec<Fq> = (0..len).map(|_| random(&mut state)).collect();
        let witness: Vec<Fq> = (0..len).map(|_| Fq::from(splitmix(&mut state) & 1 == 1)).collect();
        let claim: Fq = matrix.iter().zip(&witness).map(|(m, w)| *m * *w).sum();
        let instance = (num_vars as u64).to_le_bytes();

        let mut prover = build_prover(INNER_SESSION, &instance);
        let output = prove_inner_sumcheck(&mut prover, claim, matrix.clone(), &witness).unwrap();
        let next_prover = prover.squeeze_fq();
        let proof = prover.finish();

        let mut verifier = build_verifier(INNER_SESSION, &instance, &proof);
        let (points, final_claim) = output.proof.verify(&mut verifier, claim, num_vars).unwrap();
        let next_verifier = verifier.squeeze_fq();
        verifier.check_eof().unwrap();
        assert_eq!(points, output.eval_points);
        assert_eq!(final_claim, output.final_claim);
        assert_eq!(next_prover, next_verifier);
        assert_eq!(output.batched_matrix_evaluation, mle_evaluate(&matrix, &points));
        assert_eq!(output.witness_evaluation, mle_evaluate(&witness, &points));
        assert_eq!(final_claim, output.batched_matrix_evaluation * output.witness_evaluation);
    }

    #[test]
    fn inner_sumcheck_round_trips_at_every_small_width() {
        for num_vars in [0, 1, 2, 3, 5, 12, 15] {
            check_inner(num_vars, 0x1a2b + num_vars as u64);
        }
    }

    /// Their `inner_sumcheck_one_variable_has_expected_quadratic`: one pair
    /// gives `g(X) = (m0 + X(m1 − m0))(w0 + X(w1 − w0))`.
    #[test]
    fn inner_sumcheck_one_variable_has_expected_quadratic() {
        let matrix = vec![fq(3), fq(5)];
        let witness = vec![fq(7), fq(11)];
        let claim = fq(3 * 7 + 5 * 11);
        let mut prover = build_prover(INNER_SESSION, b"one-variable");
        let output = prove_inner_sumcheck(&mut prover, claim, matrix, &witness).unwrap();
        // c0 = 21, c2 = 2·4 = 8, c1 = claim − 2·21 − 8 = 76 − 50 = 26.
        assert_eq!(output.proof.round_polynomials, vec![[fq(21), fq(26), fq(8)]]);
    }
}
