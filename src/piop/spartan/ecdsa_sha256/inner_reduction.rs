//! Reduce the outer matrix claims and linear/public equations to
//! `claimed_sum = Σ_i batched_matrix[i] · witness[i]` over the sampled field.
//! The prover prepares the matrix MLE; the verifier evaluates it directly.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    Config, Result, error, reduce_integer_mod_q,
    relation::{OuterMode, PreparedSha256Ecdsa, SHA_H, Sha256EcdsaStatement},
};
use crate::{
    piop::spartan::{
        f2z::SpartanF2zField as F,
        matrix::{eq_table, make_equality_factors},
        raw_monty::{Raw, RawMontyCtx},
        sumcheck::OuterSumcheckProof,
    },
    poly::mle::{CompositeMultilinearExtension, EqualityWeights, FactoredMultilinearExtension},
};

/// Compact description of the equation checked by the inner sumcheck.
pub(super) struct InnerSumcheckClaim {
    /// Final row-evaluation point returned by the outer sumcheck.
    outer_row_point: Vec<F>,
    /// Challenge `c` combining the A/B/C claims with weights `1, c, c²`.
    matrix_batch_challenge: F,
    /// Point defining equality weights for the linear and public-input equations.
    linear_row_point: Vec<F>,
    /// Scales the linear/public batch when adding it to the matrix batch.
    linear_batch_weight: F,
    /// Claimed `Σ_i batched_matrix[i] · witness[i]` over assignment indices.
    claimed_sum: F,
}

/// Distinct integer matrix coefficients reduced modulo the sampled prime.
pub(super) struct ModQCoefficients {
    /// Arithmetic modulo the transcript-sampled prime.
    ctx: RawMontyCtx,
    /// One Montgomery residue per distinct `LocalRelation::coefficients` entry.
    residues: Vec<Raw>,
}

/// Owned public matrix MLE and its Montgomery evaluation cache.
/// For `N` SHA instances and `H` local wires, its Boolean table is
/// `[sha_local_evaluations ⊗ instance_weights, p256_evaluations]`:
/// entry `i + N*j` is `instance_weights[i] * sha_local_evaluations[j]`,
/// with `constant_weight` added at index zero and implicit zero padding.
pub(super) struct BatchedMatrixMle {
    /// Arithmetic for the cached P-256 evaluations.
    ctx: RawMontyCtx,
    /// Assignment domain size is `2^num_vars`, including zero padding.
    num_vars: usize,
    /// `N = 2^log_compressions` instance weights.
    instance_weights: Vec<F>,
    /// `H = SHA_H = 20,457` local evaluations.
    sha_local_evaluations: Vec<F>,
    /// `P = 1,215,663` evaluations for the current P-256 circuit.
    p256_evaluations: Vec<F>,
    /// The same `P` evaluations in raw Montgomery form.
    p256_montgomery_evaluations: Vec<Raw>,
    /// Weight of the equation `witness[0] = 1`.
    constant_weight: F,
    /// `N · H`, the first P-256 assignment index.
    p256_assignment_offset: usize,
}

impl InnerSumcheckClaim {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_outer_claims(
        relation: &PreparedSha256Ecdsa,
        statement: &Sha256EcdsaStatement,
        outer: &OuterSumcheckProof<F>,
        outer_row_point: Vec<F>,
        matrix_batch_challenge: F,
        linear_row_point: Vec<F>,
        linear_batch_weight: F,
        cfg: &Config,
    ) -> Result<Self> {
        if outer_row_point.len() != relation.outer_sumcheck_num_vars()
            || linear_row_point.len() != relation.linear_vars()
        {
            return Err(error("inner batching point dimension mismatch"));
        }
        let squared_challenge = matrix_batch_challenge.clone() * &matrix_batch_challenge;
        let mut claimed_sum = outer.az_mle_claim.clone();
        claimed_sum += &(outer.bz_mle_claim.clone() * &matrix_batch_challenge);
        claimed_sum += &(outer.cz_mle_claim.clone() * &squared_challenge);
        let linear_weights = equality_weights(&linear_row_point, cfg)?;
        let public_start = public_row_start(relation);
        claimed_sum += &(linear_weights.at(public_start) * &linear_batch_weight);
        for bit in 0..1024 {
            if statement.bit(bit) {
                claimed_sum += &(linear_weights.at(public_start + 1 + bit) * &linear_batch_weight);
            }
        }
        Ok(Self {
            outer_row_point,
            matrix_batch_challenge,
            linear_row_point,
            linear_batch_weight,
            claimed_sum,
        })
    }

    pub(super) fn claimed_sum(&self) -> &F {
        &self.claimed_sum
    }
}

impl ModQCoefficients {
    pub(super) fn from_relation(
        relation: &PreparedSha256Ecdsa,
        modulus: u128,
        cfg: &Config,
    ) -> Self {
        let _scope = crate::utils::prof::scope("ecdsa:matrix_projection");
        let ctx = RawMontyCtx::new(cfg);
        let residues = relation
            .local
            .coefficients
            .iter()
            .map(|coefficient| ctx.raw(&reduce_integer_mod_q(coefficient, modulus, cfg)))
            .collect();
        Self { ctx, residues }
    }

    pub(super) fn build_batched_matrix_mle(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        cfg: &Config,
    ) -> Result<BatchedMatrixMle> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_combine");
        let weights = self.build_row_weights(relation, claim, cfg)?;
        let gather = |column| self.p256_column_weight(relation, &weights.matrix_rows, column);
        #[cfg(feature = "parallel")]
        let mut tail: Vec<Raw> = (0..relation.local.tail.columns())
            .into_par_iter()
            .with_min_len(TAIL_BLOCK)
            .map(gather)
            .collect();
        #[cfg(not(feature = "parallel"))]
        let mut tail: Vec<Raw> = (0..relation.local.tail.columns()).map(gather).collect();
        for (&cell, &weight) in relation.local.public_h.iter().zip(&weights.public_bits) {
            tail[cell] = self.ctx.add(tail[cell], weight);
        }
        #[cfg(feature = "parallel")]
        let p256_evaluations = tail
            .par_iter()
            .with_min_len(TAIL_BLOCK)
            .map(|&value| self.ctx.field(value))
            .collect();
        #[cfg(not(feature = "parallel"))]
        let p256_evaluations = tail.iter().map(|&value| self.ctx.field(value)).collect();
        let (instance_weights, sha_local_evaluations) =
            self.build_sha_factors(relation, claim, cfg)?;
        Ok(BatchedMatrixMle {
            ctx: self.ctx,
            num_vars: relation.p_h.t + relation.p_h.s,
            instance_weights,
            sha_local_evaluations,
            p256_evaluations,
            p256_montgomery_evaluations: tail,
            constant_weight: weights.constant,
            p256_assignment_offset: relation.map.h_offset,
        })
    }

    /// Evaluate the public matrix MLE without allocating either P-256 tail table.
    pub(super) fn evaluate_batched_matrix_mle(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        assignment_point: &[F],
        cfg: &Config,
    ) -> Result<F> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_evaluate");
        check_assignment_point(relation.p_h.t + relation.p_h.s, assignment_point)?;
        let weights = self.build_row_weights(relation, claim, cfg)?;
        let (instances, sha) = self.build_sha_factors(relation, claim, cfg)?;
        let equality = equality_weights(assignment_point, cfg)?;
        let mut value = evaluate_sha_factors(&instances, &sha, assignment_point, cfg)?;
        value += &(weights.constant * &equality.at(0));
        value += &evaluate_assignment_tail(
            self.ctx,
            relation.map.h_offset,
            relation.local.tail.columns(),
            &equality,
            |column| self.p256_column_weight(relation, &weights.matrix_rows, column),
        );
        for (&cell, &weight) in relation.local.public_h.iter().zip(&weights.public_bits) {
            value += &(self.ctx.field(weight) * &equality.at(relation.map.h_offset + cell));
        }
        Ok(value)
    }

    fn build_row_weights(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        cfg: &Config,
    ) -> Result<RowWeights> {
        let ctx = self.ctx;
        let linear = equality_weights(&claim.linear_row_point, cfg)?;
        let outer = equality_weights(&claim.outer_row_point, cfg)?;
        let batch = ctx.raw(&claim.matrix_batch_challenge);
        let batch_squared = ctx.mul(batch, batch);
        let mut matrix_rows = vec![0 as Raw; 3 * relation.local.rows()];
        let matrix_weights = |slots: &mut [Raw], weight| {
            slots[0] = weight;
            slots[1] = ctx.mul(weight, batch);
            slots[2] = ctx.mul(weight, batch_squared);
        };
        match relation.mode {
            OuterMode::Split => {
                for (index, &row) in relation.local.nonlinear.iter().enumerate() {
                    matrix_weights(
                        &mut matrix_rows[3 * row..3 * row + 3],
                        ctx.raw(&outer.at(index)),
                    );
                }
                for (index, &row) in relation.local.linear.iter().enumerate() {
                    let weight = linear.at(256 * relation.compressions() + index)
                        * &claim.linear_batch_weight;
                    matrix_rows[3 * row + 2] = ctx.raw(&weight);
                }
            }
            OuterMode::AllRows => {
                for row in 0..relation.local.rows() {
                    let weight = ctx.raw(&outer.at(256 * relation.compressions() + row));
                    matrix_weights(&mut matrix_rows[3 * row..3 * row + 3], weight);
                }
            }
        }
        let public_start = public_row_start(relation);
        let constant = linear.at(public_start) * &claim.linear_batch_weight;
        let public_bits = (0..1024)
            .map(|bit| ctx.raw(&(linear.at(public_start + 1 + bit) * &claim.linear_batch_weight)))
            .collect();
        Ok(RowWeights {
            matrix_rows,
            public_bits,
            constant,
        })
    }

    fn p256_column_weight(
        &self,
        relation: &PreparedSha256Ecdsa,
        row_weights: &[Raw],
        column: usize,
    ) -> Raw {
        relation
            .local
            .tail
            .column(column)
            .fold(0 as Raw, |sum, (slot, coefficient)| {
                self.ctx.add(
                    sum,
                    self.ctx.mul(row_weights[slot], self.residues[coefficient]),
                )
            })
    }

    fn build_sha_factors(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        cfg: &Config,
    ) -> Result<(Vec<F>, Vec<F>)> {
        let squared_challenge =
            claim.matrix_batch_challenge.clone() * &claim.matrix_batch_challenge;
        let (point, multiplier) = match relation.mode {
            OuterMode::Split => (&claim.linear_row_point, &claim.linear_batch_weight),
            OuterMode::AllRows => (&claim.outer_row_point, &squared_challenge),
        };
        let instances = eq_table(&point[..relation.log_n], cfg).map_err(error)?;
        let local_weights = equality_weights(&point[relation.log_n..], cfg)?;
        let multiplier = self.ctx.raw(multiplier);
        let mut sha = vec![0 as Raw; SHA_H];
        for row in 0..relation.local.sha_c.rows() {
            let weight = self
                .ctx
                .mul(self.ctx.raw(&local_weights.at(row)), multiplier);
            for (column, coefficient) in relation.local.sha_c.row(row) {
                sha[column] = self.ctx.add(
                    sha[column],
                    self.ctx.mul(weight, self.residues[coefficient]),
                );
            }
        }
        Ok((
            instances,
            sha.into_iter().map(|value| self.ctx.field(value)).collect(),
        ))
    }
}

impl BatchedMatrixMle {
    pub(super) fn as_mle(&self, cfg: &Config) -> Result<CompositeMultilinearExtension<'_, F>> {
        CompositeMultilinearExtension::from_parts(
            self.num_vars,
            &self.sha_local_evaluations,
            &self.instance_weights,
            &self.p256_evaluations,
            self.constant_weight.clone(),
            cfg,
        )
        .map_err(error)
    }

    pub(super) fn evaluate(&self, assignment_point: &[F], cfg: &Config) -> Result<F> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_evaluate");
        check_assignment_point(self.num_vars, assignment_point)?;
        let equality = equality_weights(assignment_point, cfg)?;
        let mut value = evaluate_sha_factors(
            &self.instance_weights,
            &self.sha_local_evaluations,
            assignment_point,
            cfg,
        )?;
        value += &(self.constant_weight.clone() * &equality.at(0));
        value += &evaluate_assignment_tail(
            self.ctx,
            self.p256_assignment_offset,
            self.p256_montgomery_evaluations.len(),
            &equality,
            |index| self.p256_montgomery_evaluations[index],
        );
        Ok(value)
    }
}

#[cfg(feature = "parallel")]
const TAIL_BLOCK: usize = 1 << 12;

struct RowWeights {
    /// Slot `3 * row + matrix` for A/B/C.
    matrix_rows: Vec<Raw>,
    public_bits: Vec<Raw>,
    constant: F,
}

fn equality_weights(point: &[F], cfg: &Config) -> Result<EqualityWeights<F>> {
    let (low, high) = make_equality_factors(point, cfg).map_err(error)?;
    Ok(EqualityWeights::from_tables(
        low.evaluations,
        high.evaluations,
    ))
}

fn public_row_start(relation: &PreparedSha256Ecdsa) -> usize {
    256 * relation.compressions() + relation.local.linear.len()
}

fn check_assignment_point(num_vars: usize, point: &[F]) -> Result<()> {
    if point.len() != num_vars {
        return Err(error("assignment point dimension mismatch"));
    }
    Ok(())
}

fn evaluate_sha_factors(instances: &[F], sha: &[F], point: &[F], cfg: &Config) -> Result<F> {
    FactoredMultilinearExtension::from_factors(point.len(), sha, instances, cfg)
        .and_then(|mle| mle.evaluate(point, cfg))
        .map_err(error)
}

fn evaluate_assignment_tail(
    ctx: RawMontyCtx,
    offset: usize,
    len: usize,
    equality: &EqualityWeights<F>,
    evaluation_at: impl Fn(usize) -> Raw + Sync,
) -> F {
    let (low, high) = (ctx.raw_vec(equality.low()), ctx.raw_vec(equality.high()));
    let term = |index| {
        let assignment_index = offset + index;
        let weight = ctx.mul(
            low[assignment_index & (low.len() - 1)],
            high[assignment_index >> low.len().ilog2()],
        );
        ctx.mul(weight, evaluation_at(index))
    };
    #[cfg(feature = "parallel")]
    let sum = (0..len)
        .into_par_iter()
        .with_min_len(TAIL_BLOCK)
        .fold(|| 0 as Raw, |sum, index| ctx.add(sum, term(index)))
        .reduce(|| 0 as Raw, |left, right| ctx.add(left, right));
    #[cfg(not(feature = "parallel"))]
    let sum = (0..len).fold(0 as Raw, |sum, index| ctx.add(sum, term(index)));
    ctx.field(sum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piop::spartan::{
        ecdsa_sha256::{prepare_sha256_ecdsa, tests::fixture},
        sumcheck::SumcheckProof,
    };
    use crypto_primitives::{FromWithConfig, PrimeField, crypto_bigint_uint::Uint};

    #[test]
    fn streamed_matrix_evaluation_matches_prepared_mle() {
        let modulus = 97u128;
        let cfg = F::make_cfg(&Uint::from(modulus)).unwrap();
        let f = |n| F::from_with_cfg(n, &cfg);
        let (statement, _) = fixture();
        for mode in [OuterMode::Split, OuterMode::AllRows] {
            let relation = prepare_sha256_ecdsa(3, 100, mode).unwrap();
            let outer = OuterSumcheckProof {
                sumcheck: SumcheckProof {
                    round_polynomials: Vec::new(),
                },
                az_mle_claim: f(5u64),
                bz_mle_claim: f(7),
                cz_mle_claim: f(11),
            };
            let claim = InnerSumcheckClaim::from_outer_claims(
                &relation,
                &statement,
                &outer,
                (0..relation.outer_sumcheck_num_vars())
                    .map(|i| f(i as u64 + 13))
                    .collect(),
                f(17),
                (0..relation.linear_vars())
                    .map(|i| f(i as u64 + 19))
                    .collect(),
                f(23),
                &cfg,
            )
            .unwrap();
            let coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
            let prepared = coefficients
                .build_batched_matrix_mle(&relation, &claim, &cfg)
                .unwrap();
            let mle = prepared.as_mle(&cfg).unwrap();
            let point: Vec<_> = (0..mle.num_vars()).map(|i| f(i as u64 + 29)).collect();
            let cached = prepared.evaluate(&point, &cfg).unwrap();
            assert_eq!(
                coefficients
                    .evaluate_batched_matrix_mle(&relation, &claim, &point, &cfg)
                    .unwrap(),
                cached
            );
            assert_eq!(mle.evaluate(&point, &cfg).unwrap(), cached);
            let tail_start = relation.map.h_offset;
            let tail_end = tail_start + relation.local.tail.columns();
            for index in [
                0,
                tail_start - 1,
                tail_start,
                tail_end - 1,
                tail_end,
                (1 << mle.num_vars()) - 1,
            ] {
                let point: Vec<_> = (0..mle.num_vars())
                    .map(|bit| f(((index >> bit) & 1) as u64))
                    .collect();
                let expected = mle.evaluation_at(index).unwrap();
                assert_eq!(
                    prepared.evaluate(&point, &cfg).unwrap(),
                    expected,
                    "{mode:?} index={index}"
                );
                assert_eq!(
                    coefficients
                        .evaluate_batched_matrix_mle(&relation, &claim, &point, &cfg)
                        .unwrap(),
                    expected,
                    "{mode:?} index={index}"
                );
            }
            assert!(prepared.evaluate(&point[..point.len() - 1], &cfg).is_err());
            assert!(
                coefficients
                    .evaluate_batched_matrix_mle(&relation, &claim, &point[..point.len() - 1], &cfg)
                    .is_err()
            );
        }
    }
}
