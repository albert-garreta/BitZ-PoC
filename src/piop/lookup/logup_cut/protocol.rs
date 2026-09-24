use flock_core::{
    merkle::Hash,
    pcs::{
        commit::Commitment,
        ligerito::{
            LigeritoProof, ProverConfig as LigProverConfig,
            VerifierConfig as LigVerifierConfig,
        },
    },
};

use std::time::{Duration, Instant};

use crate::{
    binary_pcs::{BinaryPcs, MIN_PACKED_LOG},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, LigOpenProof, OodRound,
        prove_rs_open_ligerito_combined, verify_rs_open_ligerito_combined,
    },
    merged_forest::{absorb_gfs, mle_at},
    pcs::{
        GF128_MULT_ORDER, IntegerMatrixLayout, fill_chunk_pow2_flat, is_generator,
        max_fold_magnitude,
    },
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    transcript::{Blake3Transcript, traits::Transcript},
    utils::{cfg_iter, cfg_iter_mut, wide_mul::WideMulAcc},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    ChunkProductTable, ChunkSpec, DyadicPlan, DyadicUpperProof, FractionTreeWitness,
    InnerProductProof, InnerProductWorkspace, LogupCutEstimate, PackedLayout, ProductWorkspace,
    RationalProof, StructuredSumcheckProof, chunk_specs,
    derive_source_index_claim, encode_table_row, eval_table_encoding, fill_pushforward,
    DyadicUpperProfile, merge_cut_claims, prove_dyadic_upper, prove_inner_product, prove_rational,
    prove_structured_sumcheck, source_layout, verify_dyadic_upper, verify_inner_product,
    verify_rational, verify_structured_sumcheck,
};
use super::upper::DyadicUpperScratch;

const DOMAIN: &[u8] = b"bitz/logup-dyadic-cut/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogupCutConfig {
    pub chunk_bits: usize,
    pub aux_component_bits: usize,
    pub memory_limit_bytes: u64,
}

impl Default for LogupCutConfig {
    fn default() -> Self {
        Self {
            chunk_bits: 8,
            aux_component_bits: 100,
            memory_limit_bytes: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogupCutError {
    InvalidParams,
    ScratchMismatch,
    MemoryLimit,
    NegligibleEvent,
    InvalidProof,
    UpperGkr,
    Rational,
    TableInnerProduct,
    AuxiliaryOpening,
    StructuredClaim,
    MainRingSwitch,
    MainLigerito,
    AuxiliaryPcs,
}

#[derive(Debug)]
pub struct LogupCutProof {
    pub v: Vec<u128>,
    upper: DyadicUpperProof,
    aux_root: Hash,
    aux_round0: OodRound,
    rational: RationalProof,
    table_inner: InnerProductProof,
    aux_open: LigeritoProof,
    structured: StructuredSumcheckProof,
    main_open: Option<LigOpenProof>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LogupCutProofSize {
    pub public_values: usize,
    pub root_merge: usize,
    pub block_forest: usize,
    pub auxiliary_commitment: usize,
    pub auxiliary_round0: usize,
    pub rational: usize,
    pub table_inner_product: usize,
    pub auxiliary_opening: usize,
    pub structured_adapter: usize,
    pub main_ring_switch: usize,
    pub main_ligerito: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LogupCutProfile {
    pub common_setup: Duration,
    pub public_table: Duration,
    pub patterns: Duration,
    pub roots: Duration,
    pub upper_gkr: Duration,
    pub upper_forest_build: Duration,
    pub upper_root_merge: Duration,
    pub upper_forest_sumcheck: Duration,
    pub pushforward: Duration,
    pub auxiliary_commit: Duration,
    pub denominators: Duration,
    pub fraction_witness: Duration,
    pub fraction_prove: Duration,
    pub table_inner_product: Duration,
    pub auxiliary_open_prepare: Duration,
    pub auxiliary_open: Duration,
    pub structured_adapter: Duration,
    pub main_opening: Duration,
    pub total: Duration,
}

impl LogupCutProofSize {
    pub fn total(self) -> usize {
        self.public_values
            + self.root_merge
            + self.block_forest
            + self.auxiliary_commitment
            + self.auxiliary_round0
            + self.rational
            + self.table_inner_product
            + self.auxiliary_opening
            + self.structured_adapter
            + self.main_ring_switch
            + self.main_ligerito
    }
}

pub fn logup_cut_proof_size(proof: &LogupCutProof) -> LogupCutProofSize {
    let (root_merge, block_forest) = proof.upper.proof_size_bytes();
    let (main_ring_switch, main_ligerito) = proof.main_open.as_ref().map_or((0, 0), |opening| {
        (
            opening.ring.s_v.len() * 16,
            opening.lig.size_bytes(),
        )
    });
    LogupCutProofSize {
        public_values: proof.v.len() * 16,
        root_merge,
        block_forest,
        auxiliary_commitment: 32,
        auxiliary_round0: 16 + usize::from(proof.aux_round0.nonce.is_some()) * 8,
        rational: proof.rational.proof_size_bytes(),
        table_inner_product: proof.table_inner.proof_size_bytes(),
        auxiliary_opening: proof.aux_open.size_bytes(),
        structured_adapter: proof.structured.proof_size_bytes(),
        main_ring_switch,
        main_ligerito,
    }
}

pub struct LogupCutScratch {
    layout: IntegerMatrixLayout,
    config: LogupCutConfig,
    chunks: Vec<ChunkSpec>,
    plan: DyadicPlan,
    table_layout: PackedLayout,
    source_layout: PackedLayout,
    aux_pcs: BinaryPcs,
    factors: Vec<Gf>,
    patterns: Vec<u16>,
    table_values: Vec<Gf>,
    source_weights: Vec<Gf>,
    pushforward: Vec<Gf>,
    source_den: Vec<Gf>,
    table_den: Vec<Gf>,
    left_tree: FractionTreeWitness,
    right_tree: FractionTreeWitness,
    upper_forest: DyadicUpperScratch,
    upper_products: ProductWorkspace,
    fraction_products: ProductWorkspace,
    inner_product: InnerProductWorkspace,
}

impl LogupCutScratch {
    pub fn new(
        layout: IntegerMatrixLayout,
        config: LogupCutConfig,
    ) -> Result<Self, LogupCutError> {
        let ell1 = layout
            .rows()
            .checked_mul(layout.word_bits)
            .ok_or(LogupCutError::InvalidParams)?;
        if ell1 == 0
            || !ell1.is_power_of_two()
            || layout.cols() == 0
            || config.chunk_bits == 0
            || config.chunk_bits > ell1.min(16)
            || config.aux_component_bits == 0
        {
            return Err(LogupCutError::InvalidParams);
        }
        let estimate = LogupCutEstimate::new(ell1, layout.cols(), config.chunk_bits);
        if config.memory_limit_bytes != 0
            && estimate.estimated_scratch_bytes > config.memory_limit_bytes
        {
            return Err(LogupCutError::MemoryLimit);
        }
        let chunks = chunk_specs(ell1, config.chunk_bits);
        let plan = DyadicPlan::new(chunks.len());
        let table_layout = PackedLayout::new(chunks.iter().map(|chunk| chunk.width));
        let source_layout = source_layout(&plan, layout.col_vars);
        let aux_log = table_layout.dim.max(MIN_PACKED_LOG);
        let aux_pcs = BinaryPcs::new(aux_log, config.aux_component_bits)
            .map_err(|_| LogupCutError::AuxiliaryPcs)?;
        let aux_len = 1usize << aux_log;
        let mut upper_products = ProductWorkspace::default();
        if plan.blocks.len() > 1 {
            upper_products.reserve(
                (plan.blocks.len().next_power_of_two() / 2).max(1),
                layout.cols(),
                layout.col_vars,
            );
        }
        let mut fraction_products = ProductWorkspace::default();
        let fraction_dimension = source_layout.dim.max(table_layout.dim);
        let fraction_table_len = if fraction_dimension == 0 {
            1
        } else {
            1usize << (fraction_dimension - 1)
        };
        fraction_products.reserve(1, fraction_table_len, fraction_dimension);
        let mut inner_product = InnerProductWorkspace::default();
        inner_product.reserve(table_layout.padded_len);
        let upper_forest = DyadicUpperScratch::new(&plan, layout.col_vars);

        let mut scratch = Self {
            layout,
            config,
            factors: vec![Gf::ZERO; ell1],
            patterns: vec![0; chunks.len() * layout.cols()],
            table_values: vec![Gf::ZERO; table_layout.padded_len],
            source_weights: vec![Gf::ZERO; source_layout.real_len],
            pushforward: vec![Gf::ZERO; aux_len],
            source_den: vec![Gf::ONE; source_layout.real_len],
            table_den: vec![Gf::ONE; table_layout.real_len],
            left_tree: FractionTreeWitness::default(),
            right_tree: FractionTreeWitness::default(),
            upper_forest,
            upper_products,
            fraction_products,
            inner_product,
            chunks,
            plan,
            table_layout,
            source_layout,
            aux_pcs,
        };
        scratch.left_tree.rebuild_swapped(
            &mut scratch.source_weights,
            &mut scratch.source_den,
            scratch.source_layout.dim,
            Gf::ZERO,
            Gf::ONE,
        );
        scratch
            .left_tree
            .release_leaves(&mut scratch.source_weights, &mut scratch.source_den);
        scratch.right_tree.rebuild(
            &scratch.pushforward[..scratch.table_layout.real_len],
            &scratch.table_den,
            scratch.table_layout.dim,
            Gf::ZERO,
            Gf::ONE,
        );
        scratch.source_den.fill(Gf::ZERO);
        scratch.table_den.fill(Gf::ZERO);
        if config.memory_limit_bytes != 0
            && scratch.retained_bytes() as u64 > config.memory_limit_bytes
        {
            return Err(LogupCutError::MemoryLimit);
        }
        Ok(scratch)
    }

    pub fn retained_bytes(&self) -> usize {
        let fields = self.factors.capacity()
            + self.table_values.capacity()
            + self.source_weights.capacity()
            + self.pushforward.capacity()
            + self.source_den.capacity()
            + self.table_den.capacity();
        fields * core::mem::size_of::<Gf>()
            + self.patterns.capacity() * core::mem::size_of::<u16>()
            + self.chunks.capacity() * core::mem::size_of::<ChunkSpec>()
            + self.plan.blocks.capacity() * core::mem::size_of::<super::DyadicBlock>()
            + self.table_layout.blocks.capacity() * core::mem::size_of::<super::PackedBlock>()
            + self.source_layout.blocks.capacity() * core::mem::size_of::<super::PackedBlock>()
            + self.left_tree.retained_bytes()
            + self.right_tree.retained_bytes()
            + self.upper_forest.retained_bytes()
            + self.upper_products.retained_bytes()
            + self.fraction_products.retained_bytes()
            + self.inner_product.retained_bytes()
    }
}

/// `alpha^v` for every column fold `v` by windowed fixed-base
/// exponentiation (the crate's `FixedBasePow`, window 8): 16 table products
/// per 128-bit exponent. The verifier's sequential version.
fn column_roots(alpha: Gf, folds: &[u128]) -> Vec<Gf> {
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, alpha.into(), 8);
    folds
        .iter()
        .map(|&value| {
            Gf::from(comb.pow_public(&field::Uint::from_words([value as u64, (value >> 64) as u64])))
        })
        .collect()
}

/// [`column_roots`] with the columns spread over the thread pool.
fn column_roots_parallel(alpha: Gf, folds: &[u128]) -> Vec<Gf> {
    let comb = field::FixedBasePow::<_, 2>::new_public(field::Gf128Ops, alpha.into(), 8);
    cfg_iter!(folds)
        .map(|&value| {
            Gf::from(comb.pow_public(&field::Uint::from_words([value as u64, (value >> 64) as u64])))
        })
        .collect()
}

pub fn prove_logup_cut(
    transcript: &mut Blake3Transcript,
    hint: &FlockCommitHint,
    layout: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    ligerito: &LigProverConfig,
    config: LogupCutConfig,
    scratch: &mut LogupCutScratch,
) -> Result<LogupCutProof, LogupCutError> {
    prove_logup_cut_profiled(
        transcript,
        hint,
        layout,
        row_weights,
        alpha,
        ligerito,
        config,
        scratch,
    )
    .map(|(proof, _)| proof)
}

#[allow(clippy::too_many_arguments)]
pub fn prove_logup_cut_profiled(
    transcript: &mut Blake3Transcript,
    hint: &FlockCommitHint,
    layout: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    ligerito: &LigProverConfig,
    config: LogupCutConfig,
    scratch: &mut LogupCutScratch,
) -> Result<(LogupCutProof, LogupCutProfile), LogupCutError> {
    let total_start = Instant::now();
    let mut profile = LogupCutProfile::default();
    let phase_start = Instant::now();
    check_prover_shape(hint, layout, row_weights, alpha, config, scratch)?;
    bind_statement(
        transcript,
        &hint.commitment,
        layout,
        row_weights,
        alpha,
        config,
    );
    profile.common_setup = phase_start.elapsed();

    let phase_start = Instant::now();
    fill_chunk_pow2_flat(layout, row_weights, alpha, &mut scratch.factors);
    let table = ChunkProductTable {
        y: &scratch.factors,
        chunks: &scratch.chunks,
        layout: &scratch.table_layout,
    };
    table.materialize(&mut scratch.table_values);
    profile.public_table = phase_start.elapsed();

    let phase_start = Instant::now();
    let n_chunks = scratch.chunks.len();
    let chunks = &scratch.chunks;
    let table_blocks = &scratch.table_layout.blocks;
    let table_values = &scratch.table_values;
    cfg_iter_mut!(&mut scratch.patterns)
        .enumerate()
        .for_each(|(index, pattern_slot)| {
            let column = index / n_chunks;
            let chunk_index = index % n_chunks;
            let chunk = chunks[chunk_index];
            let row = &hint.rows()[column];
            debug_assert!(chunk.width <= 16);
            let word = chunk.factor_start >> 6;
            let shift = chunk.factor_start & 63;
            let mut pattern = row[word] >> shift;
            if shift + chunk.width > 64 {
                pattern |= row[word + 1] << (64 - shift);
            }
            let mask = (1u64 << chunk.width) - 1;
            let pattern = (pattern & mask) as u16;
            *pattern_slot = pattern;
        });
    profile.patterns = phase_start.elapsed();

    let phase_start = Instant::now();
    let v = crate::ligerito::fold_values_bits(layout, hint.rows(), row_weights);
    if max_fold_magnitude(&v) >= GF128_MULT_ORDER {
        return Err(LogupCutError::InvalidParams);
    }
    // Windowed fixed-base exponentiation (what the crate's forest and the
    // wfbitz scheme use): 16 table products per exponent instead of
    // square-and-multiply over its ~100 bits, one column per task.
    let roots = column_roots_parallel(alpha, &v);
    absorb_gfs(transcript, 0x30, &roots);
    let root_point = transcript.get_field_challenges(layout.col_vars, &());
    let root_value = mle_at(&roots, &root_point);
    profile.roots = phase_start.elapsed();

    let phase_start = Instant::now();
    let mut upper_profile = DyadicUpperProfile::default();
    let patterns = &scratch.patterns;
    let cut_value = |column: usize, chunk: usize| {
        table_values[
            table_blocks[chunk].offset | patterns[column * n_chunks + chunk] as usize
        ]
    };
    let (upper, cut_claims) = prove_dyadic_upper(
        transcript,
        &scratch.plan,
        layout.col_vars,
        &root_point,
        root_value,
        &cut_value,
        &mut scratch.upper_products,
        &mut scratch.upper_forest,
        &mut upper_profile,
    );
    let merged = merge_cut_claims(transcript, &scratch.plan, layout.col_vars, &cut_claims)
        .ok_or(LogupCutError::NegligibleEvent)?;
    profile.upper_gkr = phase_start.elapsed();
    profile.upper_forest_build = upper_profile.forest_build;
    profile.upper_root_merge = upper_profile.root_merge;
    profile.upper_forest_sumcheck = upper_profile.forest_sumcheck;

    let phase_start = Instant::now();
    fill_pushforward(
        &merged,
        &scratch.plan,
        &scratch.chunks,
        &scratch.table_layout,
        layout.col_vars,
        &cut_claims,
        &scratch.patterns,
        &mut scratch.source_weights,
        &mut scratch.pushforward[..scratch.table_layout.padded_len],
    );
    scratch.pushforward[scratch.table_layout.padded_len..].fill(Gf::ZERO);
    profile.pushforward = phase_start.elapsed();

    let phase_start = Instant::now();
    let (aux_commitment, aux_data) = scratch
        .aux_pcs
        .commit(&scratch.pushforward)
        .map_err(|_| LogupCutError::AuxiliaryPcs)?;
    transcript.absorb_slice(&aux_commitment.root);
    let aux_round0 = scratch.aux_pcs.prove_round0(transcript, &scratch.pushforward);
    let tau = transcript.get_field_challenge::<Gf>(&());
    if tau == Gf::ZERO {
        return Err(LogupCutError::NegligibleEvent);
    }
    let tau_words = tau.as_words();
    if tau_words[1] == 0 && tau_words[0] < scratch.table_layout.real_len as u64 {
        return Err(LogupCutError::NegligibleEvent);
    }
    profile.auxiliary_commit = phase_start.elapsed();

    let phase_start = Instant::now();
    for (block, packed) in scratch.plan.blocks.iter().zip(&merged.source_layout.blocks) {
        cfg_iter_mut!(&mut scratch.source_den[packed.offset..packed.offset + packed.len()])
            .enumerate()
            .for_each(|(local, denominator)| {
                let local_chunk = local & (block.n_chunks - 1);
                let column = local >> block.depth;
                let chunk = block.chunk_start + local_chunk;
                *denominator = tau
                    + encode_table_row(
                        scratch.table_layout.blocks[chunk].offset
                            | scratch.patterns[column * n_chunks + chunk] as usize,
                    );
            });
    }
    for (row, denominator) in scratch.table_den.iter_mut().enumerate() {
        *denominator = tau + encode_table_row(row);
    }
    profile.denominators = phase_start.elapsed();

    let phase_start = Instant::now();
    scratch.left_tree.rebuild_swapped(
        &mut scratch.source_weights,
        &mut scratch.source_den,
        scratch.source_layout.dim,
        Gf::ZERO,
        tau,
    );
    scratch.right_tree.rebuild(
        &scratch.pushforward[..scratch.table_layout.real_len],
        &scratch.table_den,
        scratch.table_layout.dim,
        Gf::ZERO,
        tau,
    );
    profile.fraction_witness = phase_start.elapsed();

    let phase_start = Instant::now();
    let (rational, rational_claims) =
        prove_rational(
            transcript,
            &mut scratch.left_tree,
            &mut scratch.right_tree,
            &mut scratch.fraction_products,
        );
    debug_assert_eq!(
        rational_claims.right.den,
        tau + eval_table_encoding(&scratch.table_layout, &rational_claims.right.point),
    );
    profile.fraction_prove = phase_start.elapsed();

    let phase_start = Instant::now();
    scratch
        .left_tree
        .release_leaves(&mut scratch.source_weights, &mut scratch.source_den);
    profile.fraction_witness += phase_start.elapsed();

    let phase_start = Instant::now();
    let (table_inner, table_claim) = prove_inner_product(
        transcript,
        &scratch.pushforward[..scratch.table_layout.padded_len],
        &scratch.table_values,
        merged.claim,
        &mut scratch.inner_product,
    );
    profile.table_inner_product = phase_start.elapsed();

    let phase_start = Instant::now();
    let eta = transcript.get_field_challenge::<Gf>(&());
    let table_point = extend_point(&table_claim.point, scratch.aux_pcs.packed_log());
    let fraction_point = extend_point(
        &rational_claims.right.point,
        scratch.aux_pcs.packed_log(),
    );
    let basis = combined_equality_basis(&table_point, &fraction_point, eta);
    let aux_target = table_claim.value + eta * rational_claims.right.num;
    crate::ligerito::absorb_ood_value(transcript, aux_target);
    profile.auxiliary_open_prepare = phase_start.elapsed();

    let phase_start = Instant::now();
    let aux_open = scratch.aux_pcs.open_basis(
        transcript,
        &scratch.pushforward,
        &aux_data,
        &aux_round0,
        basis,
        aux_target,
    );
    profile.auxiliary_open = phase_start.elapsed();

    let phase_start = Instant::now();
    let structured_claim = derive_source_index_claim(
        &merged,
        &scratch.plan,
        &scratch.chunks,
        &scratch.table_layout,
        layout.col_vars,
        &cut_claims,
        tau,
        &rational_claims.left.point,
        rational_claims.left.num,
        rational_claims.left.den,
    )
    .ok_or(LogupCutError::StructuredClaim)?;
    let (structured, bit_claims) = prove_structured_sumcheck(
        transcript,
        layout,
        hint.packed_cols(),
        &structured_claim,
    );
    debug_assert!(bit_claims.iter().all(|claim| {
        direct_bit_mle(layout, hint.rows(), &claim.point) == claim.value
    }));
    profile.structured_adapter = phase_start.elapsed();

    let phase_start = Instant::now();
    let main_open = (!bit_claims.is_empty()).then(|| {
        prove_rs_open_ligerito_combined(
            transcript,
            hint,
            &bit_claims.iter().map(|claim| claim.point.clone()).collect::<Vec<_>>(),
            ligerito,
        )
    });
    profile.main_opening = phase_start.elapsed();

    let proof = LogupCutProof {
        v,
        upper,
        aux_root: aux_commitment.root,
        aux_round0: aux_round0.round(),
        rational,
        table_inner,
        aux_open,
        structured,
        main_open,
    };
    profile.total = total_start.elapsed();
    Ok((proof, profile))
}

pub fn verify_logup_cut(
    transcript: &mut Blake3Transcript,
    commitment: &Commitment,
    proof: &LogupCutProof,
    layout: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    ligerito: &LigVerifierConfig,
    config: LogupCutConfig,
) -> Result<(), LogupCutError> {
    check_public_shape(commitment, layout, row_weights, alpha, config)?;
    if proof.v.len() != layout.cols() || max_fold_magnitude(&proof.v) >= GF128_MULT_ORDER {
        return Err(LogupCutError::InvalidProof);
    }
    bind_statement(
        transcript,
        commitment,
        layout,
        row_weights,
        alpha,
        config,
    );
    let ell1 = layout.rows() * layout.word_bits;
    let chunks = chunk_specs(ell1, config.chunk_bits);
    let plan = DyadicPlan::new(chunks.len());
    let table_layout = PackedLayout::new(chunks.iter().map(|chunk| chunk.width));
    let source_layout = source_layout(&plan, layout.col_vars);
    let aux_log = table_layout.dim.max(MIN_PACKED_LOG);
    let aux_pcs = BinaryPcs::new(aux_log, config.aux_component_bits)
        .map_err(|_| LogupCutError::AuxiliaryPcs)?;
    let mut factors = vec![Gf::ZERO; ell1];
    fill_chunk_pow2_flat(layout, row_weights, alpha, &mut factors);
    let public_table = ChunkProductTable {
        y: &factors,
        chunks: &chunks,
        layout: &table_layout,
    };

    let roots = column_roots(alpha, &proof.v);
    absorb_gfs(transcript, 0x30, &roots);
    let root_point = transcript.get_field_challenges(layout.col_vars, &());
    let root_value = mle_at(&roots, &root_point);
    let cut_claims = verify_dyadic_upper(
        transcript,
        &proof.upper,
        &plan,
        layout.col_vars,
        &root_point,
        root_value,
    )
    .ok_or(LogupCutError::UpperGkr)?;
    let merged = merge_cut_claims(transcript, &plan, layout.col_vars, &cut_claims)
        .ok_or(LogupCutError::NegligibleEvent)?;

    transcript.absorb_slice(&proof.aux_root);
    let aux_round0 = aux_pcs
        .verify_round0(transcript, &proof.aux_round0)
        .map_err(|_| LogupCutError::AuxiliaryOpening)?;
    let tau = transcript.get_field_challenge::<Gf>(&());
    if tau == Gf::ZERO {
        return Err(LogupCutError::NegligibleEvent);
    }
    let rational = verify_rational(
        transcript,
        &proof.rational,
        source_layout.dim,
        table_layout.dim,
    )
    .ok_or(LogupCutError::Rational)?;
    if rational.right.den != tau + eval_table_encoding(&table_layout, &rational.right.point) {
        return Err(LogupCutError::Rational);
    }
    let table_claim = verify_inner_product(
        transcript,
        &proof.table_inner,
        table_layout.dim,
        merged.claim,
        |point| public_table.eval(point),
    )
    .ok_or(LogupCutError::TableInnerProduct)?;

    let eta = transcript.get_field_challenge::<Gf>(&());
    let table_point = extend_point(&table_claim.point, aux_log);
    let fraction_point = extend_point(&rational.right.point, aux_log);
    let aux_target = table_claim.value + eta * rational.right.num;
    crate::ligerito::absorb_ood_value(transcript, aux_target);
    aux_pcs
        .verify_basis(
            transcript,
            &proof.aux_root,
            &aux_round0,
            aux_target,
            |prefix, log_y| {
                (0..1usize << log_y)
                    .map(|tail| {
                        equality_tail(&table_point, prefix, tail, log_y)
                            + eta * equality_tail(&fraction_point, prefix, tail, log_y)
                    })
                    .collect()
            },
            &proof.aux_open,
        )
        .map_err(|_| LogupCutError::AuxiliaryOpening)?;

    let structured_claim = derive_source_index_claim(
        &merged,
        &plan,
        &chunks,
        &table_layout,
        layout.col_vars,
        &cut_claims,
        tau,
        &rational.left.point,
        rational.left.num,
        rational.left.den,
    )
    .ok_or(LogupCutError::StructuredClaim)?;
    let bit_claims = verify_structured_sumcheck(
        transcript,
        &proof.structured,
        layout,
        &structured_claim,
    )
    .ok_or(LogupCutError::StructuredClaim)?;
    match (&proof.main_open, bit_claims.is_empty()) {
        (None, true) => {}
        (Some(main_open), false) => verify_rs_open_ligerito_combined(
            transcript,
            commitment,
            &bit_claims.iter().map(|claim| claim.point.clone()).collect::<Vec<_>>(),
            &bit_claims.iter().map(|claim| claim.value).collect::<Vec<_>>(),
            main_open,
            ligerito,
        )
        .map_err(|error| match error {
            FlockRsError::RingSwitch(_) => LogupCutError::MainRingSwitch,
            _ => LogupCutError::MainLigerito,
        })?,
        _ => return Err(LogupCutError::MainRingSwitch),
    }
    Ok(())
}

fn check_prover_shape(
    hint: &FlockCommitHint,
    layout: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    config: LogupCutConfig,
    scratch: &LogupCutScratch,
) -> Result<(), LogupCutError> {
    check_public_shape(&hint.commitment, layout, row_weights, alpha, config)?;
    if scratch.layout != *layout || scratch.config != config {
        return Err(LogupCutError::ScratchMismatch);
    }
    let row_len = layout.rows() * layout.word_bits;
    if hint.rows().len() != layout.cols()
        || hint
            .rows()
            .iter()
            .any(|row| row.len() < row_len.div_ceil(64))
    {
        return Err(LogupCutError::InvalidParams);
    }
    Ok(())
}

fn check_public_shape(
    commitment: &Commitment,
    layout: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    config: LogupCutConfig,
) -> Result<(), LogupCutError> {
    let row_len = layout
        .rows()
        .checked_mul(layout.word_bits)
        .ok_or(LogupCutError::InvalidParams)?;
    if row_weights.len() != layout.rows()
        || !row_len.is_power_of_two()
        || commitment.params.m != row_len.ilog2() as usize + layout.col_vars
        || config.chunk_bits == 0
        || config.chunk_bits > row_len.min(16)
        || config.aux_component_bits == 0
        || !is_generator(alpha)
    {
        return Err(LogupCutError::InvalidParams);
    }
    Ok(())
}

fn bind_statement(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    layout: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    config: LogupCutConfig,
) {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&commitment.root);
    for value in [
        layout.row_vars,
        layout.col_vars,
        layout.word_bits,
        config.chunk_bits,
        config.aux_component_bits,
    ] {
        transcript.absorb_slice(&(value as u64).to_le_bytes());
    }
    for &weight in row_weights {
        transcript.absorb_slice(&weight.to_le_bytes());
    }
    for word in alpha.as_words() {
        transcript.absorb_slice(&word.to_le_bytes());
    }
}

fn extend_point(point: &[Gf], dimension: usize) -> Vec<Gf> {
    assert!(point.len() <= dimension);
    let mut extended = Vec::with_capacity(dimension);
    extended.extend_from_slice(point);
    extended.resize(dimension, Gf::ZERO);
    extended
}

fn combined_equality_basis(left: &[Gf], right: &[Gf], right_scale: Gf) -> Vec<Gf> {
    assert_eq!(left.len(), right.len());
    let split = left.len() / 2;
    let left_low = build_eq_x_r_vec(&left[..split], &()).expect("nonempty low point");
    let left_high = build_eq_x_r_vec(&left[split..], &()).expect("nonempty high point");
    let right_low = build_eq_x_r_vec(&right[..split], &()).expect("nonempty low point");
    let mut right_high = build_eq_x_r_vec(&right[split..], &()).expect("nonempty high point");
    cfg_iter_mut!(&mut right_high).for_each(|value| *value *= right_scale);

    let block = left_low.len();
    let mut basis = vec![Gf::ZERO; block * left_high.len()];
    crate::cfg_chunks_mut!(&mut basis, block)
        .enumerate()
        .for_each(|(high, output)| {
            let left_high = left_high[high];
            let right_high = right_high[high];
            for (index, output) in output.iter_mut().enumerate() {
                let mut sum = <Gf as WideMulAcc>::mul_wide(&left_low[index], &left_high);
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut sum,
                    &<Gf as WideMulAcc>::mul_wide(&right_low[index], &right_high),
                );
                *output = <Gf as WideMulAcc>::from_wide(sum);
            }
        });
    basis
}

fn equality_tail(point: &[Gf], prefix: &[Gf], tail: usize, log_tail: usize) -> Gf {
    debug_assert_eq!(point.len(), prefix.len() + log_tail);
    let mut value = Gf::ONE;
    for (&left, &right) in point.iter().zip(prefix) {
        value *= Gf::ONE + left + right;
    }
    for bit in 0..log_tail {
        let coordinate = point[prefix.len() + bit];
        value *= if tail >> bit & 1 == 0 {
            Gf::ONE + coordinate
        } else {
            coordinate
        };
    }
    value
}

fn direct_bit_mle(layout: &IntegerMatrixLayout, rows: &[Vec<u64>], point: &[Gf]) -> Gf {
    let row_len = layout.rows() * layout.word_bits;
    let row_vars = row_len.ilog2() as usize;
    debug_assert_eq!(point.len(), row_vars + layout.col_vars);
    let row_eq = build_eq_x_r_vec(&point[..row_vars], &()).expect("row point");
    let column_eq = build_eq_x_r_vec(&point[row_vars..], &()).unwrap_or(vec![Gf::ONE]);
    let mut value = Gf::ZERO;
    for (column, &column_weight) in column_eq.iter().enumerate() {
        for (row, &row_weight) in row_eq.iter().enumerate() {
            if rows[column][row >> 6] >> (row & 63) & 1 == 1 {
                value += column_weight * row_weight;
            }
        }
    }
    value
}
