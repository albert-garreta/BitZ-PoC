//! Reduce the outer matrix claims and linear/public equations to
//! `claimed_sum = Σ_i batched_matrix[i] · witness[i]` over the sampled field.
//! The prover prepares the matrix MLE; the verifier evaluates it directly.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[cfg(test)]
use super::reduce_integer_mod_q;
use super::{
    Config, Result, error,
    relation::{OuterMode, PreparedSha256Ecdsa, SHA_H, Sha256EcdsaStatement},
};
use crate::{
    piop::spartan::{
        f2z::SpartanF2zField as F,
        matrix::{eq_table, make_equality_factors},
        raw_monty::{Raw, RawMontyCtx, make_equality_factors_raw},
        sumcheck::OuterSumcheckProof,
    },
    poly::mle::{CompositeMultilinearExtension, EqualityWeights, FactoredMultilinearExtension},
};
use circuit::{
    matrix_products::RuntimeModulus,
    matrix_wengert::{ForwardColumns, PreparedWengertEvaluator},
};
use crypto_primitives::FromWithConfig;
use num_bigint::BigUint;

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

/// Distinct integer matrix coefficients reduced modulo the sampled prime, and
/// the P-256 circuit's reverse-mode tape prepared for that prime.
pub(super) struct ModQCoefficients<'a> {
    /// Arithmetic modulo the transcript-sampled prime.
    ctx: RawMontyCtx,
    /// One Montgomery residue per distinct `LocalRelation::coefficients` entry.
    residues: Vec<Raw>,
    /// `r · (A + xB + x²C)` over the P-256 tail through the circuit's DAG. Its
    /// Montgomery form is the same two-limb crypto-bigint form as [`Raw`].
    tape: PreparedWengertEvaluator<'a>,
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
    /// Geometric runs of `p256_evaluations` (from the tape's power groups, split
    /// around the public-bit cells): `(start, len, base)` with
    /// `p256_evaluations[start + k] = base · 2^k`. Prover-side structure only.
    tail_runs: Vec<(usize, usize, F)>,
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

impl<'a> ModQCoefficients<'a> {
    pub(super) fn from_relation(
        relation: &'a PreparedSha256Ecdsa,
        modulus: u128,
        cfg: &Config,
    ) -> Self {
        let _scope = crate::utils::prof::scope("ecdsa:matrix_projection");
        let ctx = RawMontyCtx::new(cfg);
        // The distinct coefficients reduce natively from their two's-complement
        // words (Horner by the residue of 2^64, `signed_words_residue`), rows in
        // parallel; `bigint_residues` is the test oracle.
        let two_pow_64 = ctx.two_pow_64_residue();
        let max_words = relation
            .local
            .coefficient_words
            .iter()
            .map(|words| words.len())
            .max()
            .unwrap_or(0);
        let powers = ctx.two_pow_64_plain_powers(two_pow_64, max_words);
        let residue = |words: &[u64]| ctx.signed_words_residue(words, two_pow_64, &powers);
        #[cfg(feature = "parallel")]
        let residues = relation
            .local
            .coefficient_words
            .par_iter()
            .with_min_len(256)
            .map(|words| residue(words))
            .collect();
        #[cfg(not(feature = "parallel"))]
        let residues = relation
            .local
            .coefficient_words
            .iter()
            .map(|words| residue(words))
            .collect();
        // The sampled prime is odd and below 2^113, so both constructions succeed.
        let runtime_modulus = RuntimeModulus::<2>::new(BigUint::from(modulus))
            .expect("the sampled prime fits two limbs");
        let tape = relation
            .local
            .tape
            .prepare(&runtime_modulus)
            .expect("the sampled prime is odd");
        Self {
            ctx,
            residues,
            tape,
        }
    }

    /// `Σ_row Σ_m weight[3·row + m] · M_m[row]` over every P-256 tail column, by
    /// the reverse-mode tape. Both sides use crypto-bigint's two-limb Montgomery
    /// form with `R = 2^128`, so the words convert to [`Raw`] without arithmetic.
    fn tape_tail(&mut self, relation: &PreparedSha256Ecdsa, matrix_rows: &[Raw]) -> Result<Vec<Raw>> {
        let output = self
            .tape
            .apply_weighted(&row_triples(matrix_rows))
            .map_err(|e| error(format!("P-256 tape: {e}")))?;
        if output.len() != relation.local.tail.columns() {
            return Err(error("P-256 tape column count mismatch"));
        }
        Ok(output.iter().map(|words| words_raw(*words)).collect())
    }

    /// The geometric runs of the tape's last output, as field bases:
    /// `tail[start + k] = base · 2^k`. With `split_public`, every run is cut
    /// around the public-bit cells (whose values the prover adjusts afterwards).
    fn tail_runs(
        &self,
        relation: &PreparedSha256Ecdsa,
        cfg: &Config,
        split_public: bool,
    ) -> Vec<(usize, usize, F)> {
        let ctx = self.ctx;
        let two = F::from_with_cfg(2u64, cfg);
        let mut exceptions: Vec<usize> = if split_public {
            relation.local.public_h.to_vec()
        } else {
            Vec::new()
        };
        exceptions.sort_unstable();
        let mut out = Vec::new();
        for run in self.tape.power_runs() {
            let mut start = run.first_column;
            let end = run.first_column + run.len;
            let mut base_at_start = ctx.field(words_raw(run.base));
            let first = exceptions.partition_point(|&c| c < start);
            for &cell in &exceptions[first..] {
                if cell >= end {
                    break;
                }
                if cell > start {
                    out.push((start, cell - start, base_at_start.clone()));
                }
                // Skip the exceptional cell; the run continues after it with
                // its base advanced by 2^(cell + 1 - start).
                let mut advanced = base_at_start.clone();
                for _ in start..=cell {
                    advanced = advanced * &two;
                }
                base_at_start = advanced;
                start = cell + 1;
            }
            if start < end {
                out.push((start, end - start, base_at_start));
            }
        }
        out
    }

    pub(super) fn build_batched_matrix_mle(
        &mut self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        cfg: &Config,
    ) -> Result<BatchedMatrixMle> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_combine");
        let weights = self.build_row_weights(relation, claim, cfg)?;
        let mut tail = self.tape_tail(relation, &weights.matrix_rows)?;
        // The runs describe the tape's output; the public-bit cells are adjusted
        // below, so every run is split around them (the cells become unstructured).
        let tail_runs = self.tail_runs(relation, cfg, true);
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
        let (instance_weights, sha_local_evaluations) = self.build_sha_factors(relation, claim, cfg)?;
        Ok(BatchedMatrixMle {
            ctx: self.ctx,
            num_vars: relation.h_layout.row_vars + relation.h_layout.col_vars,
            instance_weights: instance_weights.iter().map(|&value| self.ctx.field(value)).collect(),
            sha_local_evaluations: sha_local_evaluations
                .iter()
                .map(|&value| self.ctx.field(value))
                .collect(),
            p256_evaluations,
            p256_montgomery_evaluations: tail,
            constant_weight: self.ctx.field(weights.constant),
            p256_assignment_offset: relation.map.h_offset,
            tail_runs,
        })
    }

    /// Evaluate the public matrix MLE at a point: the SHA part by its factored
    /// form, the P-256 tail by a FORWARD pass of the tape — the batched rows
    /// `Σ_m weight[3·row + m] · M_m[row]` evaluated at the tail cells' equality
    /// weights and closed with the row weights — and the public bits on top.
    /// The tail term is the bilinear form `⟨w, M·eq⟩ = ⟨Mᵀw, eq⟩` that the
    /// reverse pass ([`Self::evaluate_batched_matrix_mle_reverse`], the test
    /// oracle) computes as the tape's column vector dotted with the equality
    /// weights; the forward pass never materializes that vector.
    pub(super) fn evaluate_batched_matrix_mle(
        &mut self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        assignment_point: &[F],
        cfg: &Config,
    ) -> Result<F> {
        let _scope = crate::utils::prof::scope("ecdsa:coefficient_evaluate");
        check_assignment_point(
            relation.h_layout.row_vars + relation.h_layout.col_vars,
            assignment_point,
        )?;
        let ctx = self.ctx;
        let weights = {
            let _scope = crate::utils::prof::scope("ecdsa:ce_rows");
            self.build_row_weights(relation, claim, cfg)?
        };
        let (instances, sha) = {
            let _scope = crate::utils::prof::scope("ecdsa:ce_sha_factors");
            self.build_sha_factors(relation, claim, cfg)?
        };
        let columns = {
            let _scope = crate::utils::prof::scope("ecdsa:ce_columns");
            TailEqualityColumns::new(ctx, relation.map.h_offset, assignment_point)
        };
        // The SHA part `Σ_{i,j} instances[i]·sha[j]·eq(i + N·j)` factors over
        // the low `log N` and the remaining coordinates.
        let mut value = {
            let _scope = crate::utils::prof::scope("ecdsa:ce_sha_eval");
            let eq_instances = ctx.raw_vec(&eq_table(&assignment_point[..relation.log_n], cfg).map_err(error)?);
            let dot_instances = instances
                .iter()
                .zip(&eq_instances)
                .fold(0 as Raw, |sum, (&value, &weight)| ctx.add(sum, ctx.mul(value, weight)));
            let local = RawEqualityWeights::new(ctx, &assignment_point[relation.log_n..]);
            ctx.mul(dot_instances, local.dot(&sha))
        };
        value = ctx.add(value, ctx.mul(weights.constant, columns.eq_at(0)));
        let tail = {
            let _scope = crate::utils::prof::scope("ecdsa:ce_forward");
            self.tape
                .apply_forward_weighted(&row_triples(&weights.matrix_rows), &columns)
                .map_err(|e| error(format!("P-256 tape: {e}")))?
        };
        value = ctx.add(value, words_raw(tail));
        for (&cell, &weight) in relation.local.public_h.iter().zip(&weights.public_bits) {
            value = ctx.add(value, ctx.mul(weight, columns.eq_at(relation.map.h_offset + cell)));
        }
        Ok(ctx.field(value))
    }

    /// The reverse-mode evaluation the forward pass replaced: the tape's
    /// column vector (one reverse pass over the DAG), then the run-structured
    /// weighted sum over it. Kept as the test oracle.
    #[cfg(test)]
    pub(super) fn evaluate_batched_matrix_mle_reverse(
        &mut self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        assignment_point: &[F],
        cfg: &Config,
    ) -> Result<F> {
        check_assignment_point(
            relation.h_layout.row_vars + relation.h_layout.col_vars,
            assignment_point,
        )?;
        let weights = self.build_row_weights_field(relation, claim, cfg)?;
        let tail = self.tape_tail(relation, &weights.matrix_rows)?;
        // The verifier adds the public-bit weights separately below, so the
        // tape's runs describe `tail` unsplit.
        let runs = self.tail_runs(relation, cfg, false);
        let (instances, sha) = self.build_sha_factors_field(relation, claim, cfg)?;
        let equality = equality_weights(assignment_point, cfg)?;
        let mut value = evaluate_sha_factors(&instances, &sha, assignment_point, cfg)?;
        value += &(weights.constant * &equality.at(0));
        value += &evaluate_tail_by_runs(self.ctx, relation.map.h_offset, &tail, &runs, &equality);
        for (&cell, &weight) in relation.local.public_h.iter().zip(&weights.public_bits) {
            value += &(self.ctx.field(weight) * &equality.at(relation.map.h_offset + cell));
        }
        Ok(value)
    }

    /// The per-row `[A, B, C]` weights, the public-bit weights and the constant
    /// weight, in raw residues throughout (the equality weights as raw
    /// `low · high` products); `build_row_weights_field` is the test oracle.
    fn build_row_weights(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        _cfg: &Config,
    ) -> Result<RowWeights> {
        let ctx = self.ctx;
        let linear = RawEqualityWeights::new(ctx, &claim.linear_row_point);
        let outer = RawEqualityWeights::new(ctx, &claim.outer_row_point);
        let batch = ctx.raw(&claim.matrix_batch_challenge);
        let batch_squared = ctx.mul(batch, batch);
        let linear_batch = ctx.raw(&claim.linear_batch_weight);
        let mut matrix_rows = vec![0 as Raw; 3 * relation.local.rows()];
        let matrix_weights = |slots: &mut [Raw], weight| {
            slots[0] = weight;
            slots[1] = ctx.mul(weight, batch);
            slots[2] = ctx.mul(weight, batch_squared);
        };
        match relation.mode {
            OuterMode::Split => {
                for (index, &row) in relation.local.nonlinear.iter().enumerate() {
                    matrix_weights(&mut matrix_rows[3 * row..3 * row + 3], outer.at(index));
                }
                for (index, &row) in relation.local.linear.iter().enumerate() {
                    matrix_rows[3 * row + 2] =
                        ctx.mul(linear.at(256 * relation.compressions() + index), linear_batch);
                }
            }
            OuterMode::AllRows => {
                for row in 0..relation.local.rows() {
                    let weight = outer.at(256 * relation.compressions() + row);
                    matrix_weights(&mut matrix_rows[3 * row..3 * row + 3], weight);
                }
            }
        }
        let public_start = public_row_start(relation);
        let constant = ctx.mul(linear.at(public_start), linear_batch);
        let public_bits = (0..1024)
            .map(|bit| ctx.mul(linear.at(public_start + 1 + bit), linear_batch))
            .collect();
        Ok(RowWeights {
            matrix_rows,
            public_bits,
            constant,
        })
    }

    /// [`Self::build_row_weights`] with the equality weights and the products
    /// taken in the field domain; kept as the test oracle.
    #[cfg(test)]
    fn build_row_weights_field(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        cfg: &Config,
    ) -> Result<RowWeightsField> {
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
        Ok(RowWeightsField {
            matrix_rows,
            public_bits,
            constant,
        })
    }

    /// The arbitrary-precision projection of the distinct coefficients the
    /// native word reduction replaced; kept as the test oracle.
    #[cfg(test)]
    fn bigint_residues(relation: &PreparedSha256Ecdsa, modulus: u128, cfg: &Config) -> Vec<Raw> {
        let ctx = RawMontyCtx::new(cfg);
        relation
            .local
            .coefficients
            .iter()
            .map(|coefficient| ctx.raw(&reduce_integer_mod_q(coefficient, modulus, cfg)))
            .collect()
    }

    /// The expanded-entry gather the tape replaced; kept as the test oracle.
    #[cfg(test)]
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

    /// The SHA part's factors `(instances, sha)` in raw residues: the
    /// batched matrix MLE's entry `i + N·j` is `instances[i] · sha[j]`.
    /// `build_sha_factors_field` is the test oracle.
    fn build_sha_factors(
        &self,
        relation: &PreparedSha256Ecdsa,
        claim: &InnerSumcheckClaim,
        cfg: &Config,
    ) -> Result<(Vec<Raw>, Vec<Raw>)> {
        let ctx = self.ctx;
        let batch = ctx.raw(&claim.matrix_batch_challenge);
        let (point, multiplier) = match relation.mode {
            OuterMode::Split => (&claim.linear_row_point, ctx.raw(&claim.linear_batch_weight)),
            OuterMode::AllRows => (&claim.outer_row_point, ctx.mul(batch, batch)),
        };
        let instances = ctx.raw_vec(&eq_table(&point[..relation.log_n], cfg).map_err(error)?);
        let local_weights = RawEqualityWeights::new(ctx, &point[relation.log_n..]);
        let mut sha = vec![0 as Raw; SHA_H];
        for row in 0..relation.local.sha_c.rows() {
            let weight = ctx.mul(local_weights.at(row), multiplier);
            for (column, coefficient) in relation.local.sha_c.row(row) {
                sha[column] = ctx.add(sha[column], ctx.mul(weight, self.residues[coefficient]));
            }
        }
        Ok((instances, sha))
    }

    /// [`Self::build_sha_factors`] in the field domain; kept as the test oracle.
    #[cfg(test)]
    fn build_sha_factors_field(
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
        .and_then(|mle| mle.with_tail_runs(&self.tail_runs))
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
        value += &evaluate_tail_by_runs(
            self.ctx,
            self.p256_assignment_offset,
            &self.p256_montgomery_evaluations,
            &self.tail_runs,
            &equality,
        );
        Ok(value)
    }
}

#[cfg(feature = "parallel")]
const TAIL_BLOCK: usize = 1 << 12;

/// Little-endian words of a raw Montgomery residue, the tape's element form.
const fn raw_words(value: Raw) -> [u64; 2] {
    [value as u64, (value >> 64) as u64]
}

/// The per-row `[A, B, C]` weight triples of the tape, from the slot layout
/// `3 · row + matrix` of [`RowWeights::matrix_rows`].
fn row_triples(matrix_rows: &[Raw]) -> Vec<[[u64; 2]; 3]> {
    matrix_rows
        .chunks_exact(3)
        .map(|slots| [raw_words(slots[0]), raw_words(slots[1]), raw_words(slots[2])])
        .collect()
}

/// The tail cells' equality weights as forward-pass column values. A scalar
/// column `j` is `eq(offset + j) = low[(offset + j) mod L] · high[(offset + j) / L]`;
/// a power group's `Σ_{k<len} 2^k · eq(offset + first + k)` uses the backward
/// recurrence of [`evaluate_tail_by_runs`] (`Q[t] = Σ_{i ≥ t} 2^(i − t) · low[i]`,
/// no inverses): the group's intersection with one high block `[lo, hi)`
/// contributes `high[b] · (Q[lo] − 2^(hi − lo) · Q[hi])` times the running
/// power of two, so a group costs a few multiplications per block it spans.
struct TailEqualityColumns {
    ctx: RawMontyCtx,
    offset: usize,
    low: Vec<Raw>,
    high: Vec<Raw>,
    /// `Q[t] = low[t] + 2 · Q[t + 1]`, `Q[L] = 0`.
    suffix: Vec<Raw>,
    /// `2^k` in Montgomery form for `k ≤ L`.
    pow2: Vec<Raw>,
    shift: u32,
    mask: usize,
}

impl TailEqualityColumns {
    /// The equality tables of `point`, split at half the point (as
    /// `make_equality_factors` splits them).
    fn new(ctx: RawMontyCtx, offset: usize, point: &[F]) -> Self {
        let (low, high) = make_equality_factors_raw(&ctx, point);
        let block = low.len();
        let mut suffix = vec![0 as Raw; block + 1];
        for t in (0..block).rev() {
            let doubled = ctx.add(suffix[t + 1], suffix[t + 1]);
            suffix[t] = ctx.add(low[t], doubled);
        }
        let mut pow2 = Vec::with_capacity(block + 1);
        pow2.push(ctx.native_residue(1));
        for k in 0..block {
            pow2.push(ctx.add(pow2[k], pow2[k]));
        }
        Self {
            ctx,
            offset,
            low,
            high,
            suffix,
            pow2,
            shift: block.ilog2(),
            mask: block - 1,
        }
    }
}

impl TailEqualityColumns {
    /// `eq(point, index)` over the whole assignment domain.
    #[inline]
    fn eq_at(&self, index: usize) -> Raw {
        self.ctx
            .mul(self.low[index & self.mask], self.high[index >> self.shift])
    }
}

impl ForwardColumns for TailEqualityColumns {
    fn scalar(&self, column: usize) -> [u64; 2] {
        raw_words(self.eq_at(self.offset + column))
    }

    fn power_sum(&self, first: usize, len: usize) -> [u64; 2] {
        let ctx = self.ctx;
        let block = self.low.len();
        let mut sum = 0 as Raw;
        let mut base = self.pow2[0];
        let mut index = self.offset + first;
        let end = index + len;
        while index < end {
            let b = index >> self.shift;
            let lo = index & self.mask;
            let hi = (lo + (end - index)).min(block);
            let part = ctx.sub(self.suffix[lo], ctx.mul(self.pow2[hi - lo], self.suffix[hi]));
            sum = ctx.add(sum, ctx.mul(ctx.mul(base, part), self.high[b]));
            base = ctx.mul(base, self.pow2[hi - lo]);
            index += hi - lo;
        }
        raw_words(sum)
    }
}

/// The raw Montgomery residue of the tape's little-endian words.
const fn words_raw(words: [u64; 2]) -> Raw {
    (words[0] as Raw) | ((words[1] as Raw) << 64)
}

struct RowWeights {
    /// Slot `3 * row + matrix` for A/B/C.
    matrix_rows: Vec<Raw>,
    public_bits: Vec<Raw>,
    constant: Raw,
}

/// The field-domain oracle's form of [`RowWeights`].
#[cfg(test)]
struct RowWeightsField {
    matrix_rows: Vec<Raw>,
    public_bits: Vec<Raw>,
    constant: F,
}

/// [`EqualityWeights`] in raw residues: `eq(index) = low[index mod L] · high[index / L]`
/// with the tables split at half the point, as `make_equality_factors` splits
/// them, so every weight is the same residue one raw product later.
struct RawEqualityWeights {
    ctx: RawMontyCtx,
    low: Vec<Raw>,
    high: Vec<Raw>,
    shift: u32,
    mask: usize,
}

impl RawEqualityWeights {
    fn new(ctx: RawMontyCtx, point: &[F]) -> Self {
        let (low, high) = make_equality_factors_raw(&ctx, point);
        let block = low.len();
        Self {
            ctx,
            low,
            high,
            shift: block.ilog2(),
            mask: block - 1,
        }
    }

    #[inline]
    fn at(&self, index: usize) -> Raw {
        self.ctx
            .mul(self.low[index & self.mask], self.high[index >> self.shift])
    }

    /// `Σ_j values[j] · eq(j)`, with the high factor applied once per block:
    /// `Σ_h high[h] · Σ_l low[l] · values[h·L + l]`.
    fn dot(&self, values: &[Raw]) -> Raw {
        let ctx = self.ctx;
        let mut sum = 0 as Raw;
        for (h, block) in values.chunks(self.low.len()).enumerate() {
            let inner = block
                .iter()
                .zip(&self.low)
                .fold(0 as Raw, |acc, (&value, &low)| ctx.add(acc, ctx.mul(value, low)));
            sum = ctx.add(sum, ctx.mul(inner, self.high[h]));
        }
        sum
    }
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

/// `Σ_j tail[j] · eq(point, offset + j)` where `tail` is geometric on `runs`
/// (`tail[start + k] = base · 2^k`) and arbitrary elsewhere. With
/// `eq(offset + j) = low[(offset + j) mod L] · high[(offset + j) / L]`, a run's
/// intersection with one high block `[lo, hi)` contributes
/// `base_t · high[b] · (Q[lo] − 2^(hi − lo) · Q[hi])` where
/// `Q[t] = Σ_{i ≥ t} 2^(i − t) · low[i]` (a backward recurrence, no inverses),
/// so the runs cost a few multiplications each; columns outside every run pay
/// two multiplications.
fn evaluate_tail_by_runs(
    ctx: RawMontyCtx,
    offset: usize,
    tail: &[Raw],
    runs: &[(usize, usize, F)],
    equality: &EqualityWeights<F>,
) -> F {
    let (low, high) = (ctx.raw_vec(equality.low()), ctx.raw_vec(equality.high()));
    let block = low.len();
    let shift = block.ilog2();
    let mask = block - 1;
    // Q[t] = low[t] + 2 · Q[t + 1], Q[block] = 0.
    let mut suffix = vec![0 as Raw; block + 1];
    for t in (0..block).rev() {
        let doubled = ctx.add(suffix[t + 1], suffix[t + 1]);
        suffix[t] = ctx.add(low[t], doubled);
    }
    // 2^k in Montgomery form for k ≤ block.
    let mut pow2 = Vec::with_capacity(block + 1);
    pow2.push(ctx.native_residue(1));
    for k in 0..block {
        pow2.push(ctx.add(pow2[k], pow2[k]));
    }
    let mut sum = 0 as Raw;
    let mut cursor = 0usize;
    let scalar = |sum: &mut Raw, from: usize, to: usize| {
        for column in from..to {
            let index = offset + column;
            let weight = ctx.mul(low[index & mask], high[index >> shift]);
            *sum = ctx.add(*sum, ctx.mul(weight, tail[column]));
        }
    };
    for &(start, len, ref base) in runs {
        scalar(&mut sum, cursor, start);
        let mut base_t = ctx.raw(base);
        let mut index = offset + start;
        let end = offset + start + len;
        while index < end {
            let b = index >> shift;
            let lo = index & mask;
            let hi = (lo + (end - index)).min(block);
            let part = ctx.sub(suffix[lo], ctx.mul(pow2[hi - lo], suffix[hi]));
            sum = ctx.add(sum, ctx.mul(ctx.mul(base_t, part), high[b]));
            base_t = ctx.mul(base_t, pow2[hi - lo]);
            index += hi - lo;
        }
        cursor = start + len;
    }
    scalar(&mut sum, cursor, tail.len());
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

    /// A 113-bit prime drawn the way the protocol draws it (from a fresh
    /// transcript): the modulus class the verifier runs on, next to the wider
    /// Mersenne prime the other oracle tests use.
    pub(super) fn sampled_prime() -> u128 {
        crate::ext_proj::sample_prime_in_interval(
            &mut crate::transcript::Blake3Transcript::new(),
            1u128 << 112,
            (1u128 << 113) - 1,
        )
        .unwrap()
    }

    /// The native word reduction of the distinct coefficients equals the
    /// arbitrary-precision projection residue for residue, at a sampled
    /// 113-bit prime and at `2^127 − 1`; the stored words round-trip to the
    /// integers they encode.
    #[test]
    fn coefficient_residues_match_bigint_projection() {
        use num_bigint::BigInt;
        for modulus in [sampled_prime(), (1u128 << 127) - 1] {
            let cfg = F::make_cfg(&Uint::from(modulus)).unwrap();
            for mode in [OuterMode::Split, OuterMode::AllRows] {
                let relation = prepare_sha256_ecdsa(3, 100, mode).unwrap();
                for (value, words) in relation
                    .local
                    .coefficients
                    .iter()
                    .zip(&relation.local.coefficient_words)
                {
                    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                    assert_eq!(&BigInt::from_signed_bytes_le(&bytes), value);
                }
                let coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
                assert_eq!(
                    coefficients.residues,
                    ModQCoefficients::bigint_residues(&relation, modulus, &cfg),
                    "{mode:?} modulus {modulus}"
                );
            }
        }
    }

    /// The tape's tail must equal the expanded-entry gather element by element,
    /// for both outer modes (Split weights only the linear rows' `C` slot).
    #[test]
    fn tape_tail_matches_column_gather() {
        let modulus = (1u128 << 127) - 1; // the Mersenne prime M127: two limbs, odd, wider than the sampled primes
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
            let mut coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
            let weights = coefficients.build_row_weights(&relation, &claim, &cfg).unwrap();
            let tape = coefficients.tape_tail(&relation, &weights.matrix_rows).unwrap();
            assert_eq!(tape.len(), relation.local.tail.columns());
            let gather: Vec<Raw> = (0..relation.local.tail.columns())
                .map(|column| coefficients.p256_column_weight(&relation, &weights.matrix_rows, column))
                .collect();
            let first_mismatch = tape.iter().zip(&gather).position(|(a, b)| a != b);
            assert_eq!(first_mismatch, None, "{mode:?}");
        }
    }

    /// Every tail run must reproduce the materialized values it covers, and the
    /// runs must be sorted and disjoint (the prover kernel relies on both).
    #[test]
    fn tail_runs_describe_the_materialized_tail() {
        let modulus = (1u128 << 127) - 1;
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
            let mut coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
            let prepared = coefficients
                .build_batched_matrix_mle(&relation, &claim, &cfg)
                .unwrap();
            let two = f(2);
            let mut covered = 0usize;
            let mut end = 0usize;
            for &(start, len, ref base) in &prepared.tail_runs {
                assert!(start >= end && len > 0, "{mode:?}: runs overlap or are empty");
                end = start + len;
                let mut value = base.clone();
                for k in 0..len {
                    assert_eq!(
                        prepared.p256_evaluations[start + k],
                        value,
                        "{mode:?}: run at {start} disagrees at offset {k}"
                    );
                    value = value * &two;
                }
                covered += len;
            }
            assert!(end <= prepared.p256_evaluations.len());
            // The lifts cover most of the P-256 witness; the rest are scalar columns.
            assert!(covered * 10 > prepared.p256_evaluations.len() * 8, "{mode:?}: only {covered} of {} covered", prepared.p256_evaluations.len());
        }
    }

    /// The raw-residue row weights and SHA factors are the field-domain ones
    /// residue for residue (both outer modes, random claims, a sampled 113-bit
    /// prime and `2^127 − 1`).
    #[test]
    fn raw_factor_builders_match_field_builders() {
        let (statement, _) = fixture();
        for modulus in [sampled_prime(), (1u128 << 127) - 1] {
            let cfg = F::make_cfg(&Uint::from(modulus)).unwrap();
            let mut state = 0x1357_9BDF_2468_ACE0_u64 ^ modulus as u64;
            let mut random = move || {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };
            let mut element = || {
                let value = (u128::from(random()) << 64) | u128::from(random());
                F::from_with_cfg(value % modulus, &cfg)
            };
            for mode in [OuterMode::Split, OuterMode::AllRows] {
                let relation = prepare_sha256_ecdsa(3, 100, mode).unwrap();
                let outer = OuterSumcheckProof {
                    sumcheck: SumcheckProof {
                        round_polynomials: Vec::new(),
                    },
                    az_mle_claim: element(),
                    bz_mle_claim: element(),
                    cz_mle_claim: element(),
                };
                let claim = InnerSumcheckClaim::from_outer_claims(
                    &relation,
                    &statement,
                    &outer,
                    (0..relation.outer_sumcheck_num_vars()).map(|_| element()).collect(),
                    element(),
                    (0..relation.linear_vars()).map(|_| element()).collect(),
                    element(),
                    &cfg,
                )
                .unwrap();
                let coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
                let ctx = coefficients.ctx;
                let raw = coefficients.build_row_weights(&relation, &claim, &cfg).unwrap();
                let field = coefficients.build_row_weights_field(&relation, &claim, &cfg).unwrap();
                assert_eq!(raw.matrix_rows, field.matrix_rows, "{mode:?} rows");
                assert_eq!(raw.public_bits, field.public_bits, "{mode:?} public bits");
                assert_eq!(ctx.field(raw.constant), field.constant, "{mode:?} constant");
                let (instances, sha) = coefficients.build_sha_factors(&relation, &claim, &cfg).unwrap();
                let (instances_field, sha_field) =
                    coefficients.build_sha_factors_field(&relation, &claim, &cfg).unwrap();
                assert_eq!(ctx.raw_vec(&instances_field), instances, "{mode:?} instances");
                assert_eq!(ctx.raw_vec(&sha_field), sha, "{mode:?} sha");
                // The grouped raw dot is the factored MLE's evaluation.
                let vars = relation.h_layout.row_vars + relation.h_layout.col_vars;
                let point: Vec<F> = (0..vars).map(|_| element()).collect();
                let expected = evaluate_sha_factors(&instances_field, &sha_field, &point, &cfg).unwrap();
                let eq_instances = ctx.raw_vec(&eq_table(&point[..relation.log_n], &cfg).unwrap());
                let dot_instances = instances
                    .iter()
                    .zip(&eq_instances)
                    .fold(0 as Raw, |sum, (&v, &w)| ctx.add(sum, ctx.mul(v, w)));
                let local = RawEqualityWeights::new(ctx, &point[relation.log_n..]);
                assert_eq!(ctx.field(ctx.mul(dot_instances, local.dot(&sha))), expected, "{mode:?} sha dot");
            }
        }
    }

    /// The forward-pass evaluation is the reverse-pass evaluation (the
    /// tape's column vector dotted with the equality weights), for both outer
    /// modes, at random claims and points, at a sampled 113-bit prime and at
    /// `2^127 − 1`.
    #[test]
    fn forward_matrix_evaluation_matches_reverse() {
        let (statement, _) = fixture();
        for modulus in [sampled_prime(), (1u128 << 127) - 1] {
            let cfg = F::make_cfg(&Uint::from(modulus)).unwrap();
            let mut state = 0x5DEE_CE66_D1B4_E8A3_u64 ^ modulus as u64;
            let mut random = move || {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };
            let mut element = || {
                let value = (u128::from(random()) << 64) | u128::from(random());
                F::from_with_cfg(value % modulus, &cfg)
            };
            for mode in [OuterMode::Split, OuterMode::AllRows] {
                let relation = prepare_sha256_ecdsa(3, 100, mode).unwrap();
                for trial in 0..3 {
                    let outer = OuterSumcheckProof {
                        sumcheck: SumcheckProof {
                            round_polynomials: Vec::new(),
                        },
                        az_mle_claim: element(),
                        bz_mle_claim: element(),
                        cz_mle_claim: element(),
                    };
                    let claim = InnerSumcheckClaim::from_outer_claims(
                        &relation,
                        &statement,
                        &outer,
                        (0..relation.outer_sumcheck_num_vars()).map(|_| element()).collect(),
                        element(),
                        (0..relation.linear_vars()).map(|_| element()).collect(),
                        element(),
                        &cfg,
                    )
                    .unwrap();
                    let vars = relation.h_layout.row_vars + relation.h_layout.col_vars;
                    let point: Vec<F> = (0..vars).map(|_| element()).collect();
                    let mut coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
                    let forward = coefficients
                        .evaluate_batched_matrix_mle(&relation, &claim, &point, &cfg)
                        .unwrap();
                    let reverse = coefficients
                        .evaluate_batched_matrix_mle_reverse(&relation, &claim, &point, &cfg)
                        .unwrap();
                    assert_eq!(forward, reverse, "{mode:?} modulus {modulus} trial {trial}");
                }
            }
        }
    }

    #[test]
    fn streamed_matrix_evaluation_matches_prepared_mle() {
        // The verifier's modulus class (the native word kernels need q > 2^64).
        let modulus = sampled_prime();
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
            let mut coefficients = ModQCoefficients::from_relation(&relation, modulus, &cfg);
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
