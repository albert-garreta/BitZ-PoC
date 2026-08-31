//! Small-value prefix prover for SHA-256's linear assignment sumcheck.
//!
//! The verifier sees an ordinary degree-two sumcheck. The prefix length is a
//! prover-only implementation choice: every round still absorbs the same three
//! field coefficients, performs the same optional grinding step, and samples
//! the same challenge as the field-only prover.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crypto_bigint::{Choice, CtSelect};
use crypto_primitives::{
    PrimeField, crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint as FieldUint,
};

use crate::transcript::traits::Transcript;

use super::super::{
    SpartanField, absorb_field_elements,
    f2z::SpartanF2zField,
    grinding::{GrindingDomain, GrindingError, GrindingRound, MAX_GRINDING_BITS, grind_and_absorb},
    squeeze_field,
    sumcheck::{
        OptimizedSumcheckReducer, SumcheckError, SumcheckLinearReducer, SumcheckProductReducer,
        SumcheckProof,
    },
};

type Field = SpartanF2zField;
type FieldConfig = crypto_bigint::modular::FixedMontyParams<2>;
type LinearAccumulator = <OptimizedSumcheckReducer as SumcheckLinearReducer>::Accumulator;
type ProductAccumulator = <OptimizedSumcheckReducer as SumcheckProductReducer<Field>>::Accumulator;
type RawMontgomery = [u64; 2];

/// Lazy Boolean source for the flat assignment table.
///
/// A packed word slice retains strict canonical-padding validation. A callback
/// source is useful when the flat bit table is itself spread across packed
/// column rows; it is queried only at indices below `live_len`.
pub(crate) trait Sha256InnerBitSource: Sync {
    fn bit_at(&self, index: usize) -> Result<u64, SumcheckError>;

    fn validate_shape(&self, _live_len: usize, _table_len: usize) -> Result<(), SumcheckError> {
        Ok(())
    }
}

impl Sha256InnerBitSource for [u64] {
    #[inline]
    fn bit_at(&self, index: usize) -> Result<u64, SumcheckError> {
        Ok((self[index / u64::BITS as usize] >> (index % u64::BITS as usize)) & 1)
    }

    fn validate_shape(&self, live_len: usize, table_len: usize) -> Result<(), SumcheckError> {
        let live_words = live_len.div_ceil(u64::BITS as usize);
        let domain_words = table_len.div_ceil(u64::BITS as usize);
        if self.len() != live_words && self.len() != domain_words {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        let used_live_bits = live_len % u64::BITS as usize;
        if (used_live_bits != 0 && self[live_words - 1] >> used_live_bits != 0)
            || self[live_words..].iter().any(|word| *word != 0)
        {
            return Err(SumcheckError::InvalidProductDimensions);
        }
        Ok(())
    }
}

impl Sha256InnerBitSource for Vec<u64> {
    #[inline]
    fn bit_at(&self, index: usize) -> Result<u64, SumcheckError> {
        self.as_slice().bit_at(index)
    }

    fn validate_shape(&self, live_len: usize, table_len: usize) -> Result<(), SumcheckError> {
        self.as_slice().validate_shape(live_len, table_len)
    }
}

impl<F> Sha256InnerBitSource for F
where
    F: Fn(usize) -> Result<u64, SumcheckError> + Sync,
{
    #[inline]
    fn bit_at(&self, index: usize) -> Result<u64, SumcheckError> {
        self(index)
    }
}

/// Largest supported number of native-small prefix rounds.
pub const SHA256_INNER_PREFIX_MAX_VARS: usize = 4;

/// The small-prefix prover reports the same failures as the ordinary sumcheck.
pub(crate) type Sha256InnerSumcheckError = SumcheckError;

/// Prover output for the transcript-identical SHA-256 inner sumcheck.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sha256InnerSumcheckOutput {
    /// Ordinary degree-two sumcheck proof; `prefix_vars` is intentionally absent.
    pub sumcheck_proof: SumcheckProof<Field, 3>,
    /// One nonce per round when grinding is enabled, otherwise empty.
    pub round_nonces: Vec<u64>,
    /// Fiat--Shamir point in low-coordinate-first order.
    pub eval_points: Vec<Field>,
    /// Terminal product `V(eval_points) * H(eval_points)`.
    pub final_claim: Field,
    /// Terminal evaluation of the field-valued `V` table.
    pub v_evaluation: Field,
    /// Terminal evaluation of the packed Boolean `H` table.
    pub h_evaluation: Field,
}

/// Typed proof-of-work domain for SHA-256 inner-sumcheck rounds.
pub(crate) enum Sha256InnerGrinding {}

impl GrindingDomain for Sha256InnerGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-sha256/grinding/inner/v1";
}

/// Proves `claim = sum_y V(y) H(y)` with a native-small prefix.
///
/// `V` and the Boolean `H` table are supplied lazily. A packed `[u64]` is also
/// a valid `H` source, with table entry `i` at bit `i % 64` of word `i / 64`.
/// Coordinates and challenges are low-coordinate first. `prefix_vars` is
/// dispatched to separately monomorphized kernels for `K = 0, ..., 4`.
///
/// `live_len` identifies the non-padding prefix of the logical `2^num_vars`
/// domain. The oracle is never queried outside that prefix, and the compact
/// tail stores one raw Montgomery residue per live suffix (plus at most one),
/// then reuses that allocation for interleaved folded `V`/`H` cells. The
/// omitted suffix is folded as implicit zeros.
///
/// The oracle must be deterministic: the prefix pass and the fold pass can
/// query an entry independently. Every returned element is checked against the
/// one shared runtime-field configuration before it is used.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_sha256_inner_sumcheck<T, V, H>(
    transcript: &mut T,
    initial_claim: Field,
    num_vars: usize,
    live_len: usize,
    v_at: &V,
    h_source: &H,
    prefix_vars: usize,
    field_cfg: &FieldConfig,
    reducer: &OptimizedSumcheckReducer,
    grinding_bits: u32,
) -> Result<Sha256InnerSumcheckOutput, Sha256InnerSumcheckError>
where
    T: Transcript,
    V: Fn(usize) -> Result<Field, SumcheckError> + Sync,
    H: Sha256InnerBitSource + ?Sized,
{
    match prefix_vars {
        0 => prove_with_prefix::<0, _, _, _>(
            transcript,
            initial_claim,
            num_vars,
            live_len,
            v_at,
            h_source,
            field_cfg,
            reducer,
            grinding_bits,
        ),
        1 => prove_with_prefix::<1, _, _, _>(
            transcript,
            initial_claim,
            num_vars,
            live_len,
            v_at,
            h_source,
            field_cfg,
            reducer,
            grinding_bits,
        ),
        2 => prove_with_prefix::<2, _, _, _>(
            transcript,
            initial_claim,
            num_vars,
            live_len,
            v_at,
            h_source,
            field_cfg,
            reducer,
            grinding_bits,
        ),
        3 => prove_with_prefix::<3, _, _, _>(
            transcript,
            initial_claim,
            num_vars,
            live_len,
            v_at,
            h_source,
            field_cfg,
            reducer,
            grinding_bits,
        ),
        4 => prove_with_prefix::<4, _, _, _>(
            transcript,
            initial_claim,
            num_vars,
            live_len,
            v_at,
            h_source,
            field_cfg,
            reducer,
            grinding_bits,
        ),
        _ => Err(SumcheckError::InvalidProductDimensions),
    }
}

/// Replays the ordinary degree-two verifier with SHA-256 inner-round grinding.
///
/// No prefix length is accepted: it is not part of the proof or transcript.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_sha256_inner_sumcheck<T: Transcript>(
    transcript: &mut T,
    initial_claim: Field,
    sumcheck_proof: &SumcheckProof<Field, 3>,
    round_nonces: &[u64],
    expected_rounds: usize,
    field_cfg: &FieldConfig,
    grinding_bits: u32,
) -> Result<(Vec<Field>, Field), Sha256InnerSumcheckError> {
    sumcheck_proof.verify_grinded::<Sha256InnerGrinding>(
        transcript,
        initial_claim,
        expected_rounds,
        field_cfg,
        round_nonces,
        grinding_bits,
    )
}

#[allow(clippy::too_many_arguments)]
fn prove_with_prefix<const K: usize, T, V, H>(
    transcript: &mut T,
    initial_claim: Field,
    num_vars: usize,
    live_len: usize,
    v_at: &V,
    h_source: &H,
    field_cfg: &FieldConfig,
    reducer: &OptimizedSumcheckReducer,
    grinding_bits: u32,
) -> Result<Sha256InnerSumcheckOutput, Sha256InnerSumcheckError>
where
    T: Transcript,
    V: Fn(usize) -> Result<Field, SumcheckError> + Sync,
    H: Sha256InnerBitSource + ?Sized,
{
    validate_inputs::<K, _>(
        &initial_claim,
        num_vars,
        live_len,
        h_source,
        field_cfg,
        grinding_bits,
    )?;

    let zero = Field::zero_with_cfg(field_cfg);
    let one = Field::one_with_cfg(field_cfg);
    let mut current_claim = initial_claim;
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_nonces = Vec::with_capacity(if grinding_bits == 0 { 0 } else { num_vars });

    if K > 0 {
        let accumulators = build_prefix_accumulators::<K, _, _>(
            num_vars, live_len, v_at, h_source, field_cfg, &zero, reducer,
        )?;
        let mut lagrange_coefficients = vec![one.clone()];

        for round in 0..K {
            let [at_infinity, at_zero] =
                accumulators.evaluate_round(round, &lagrange_coefficients, reducer)?;
            let coefficients = quadratic_coefficients(&current_claim, &at_zero, &at_infinity);

            absorb_field_elements(transcript, &coefficients);
            if grinding_bits != 0 {
                let round_index =
                    u64::try_from(round).expect("an in-memory sumcheck round index fits in u64");
                round_nonces.push(grind_and_absorb::<Sha256InnerGrinding, _>(
                    transcript,
                    GrindingRound::new(round_index),
                    grinding_bits,
                )?);
            }
            let challenge = squeeze_field(transcript, field_cfg);
            current_claim = evaluate_quadratic(&coefficients, &challenge, &zero);
            extend_lagrange_coefficients(&mut lagrange_coefficients, &challenge, &one, &zero);
            round_polynomials.push(coefficients);
            eval_points.push(challenge);
        }
    }

    let prefix_v = fold_prefix_v_table::<K, _>(
        num_vars,
        live_len,
        v_at,
        &eval_points,
        field_cfg,
        &zero,
        &one,
        reducer,
    )?;
    let tail = prove_compact_tail::<K, _, _>(
        transcript,
        current_claim,
        prefix_v,
        live_len,
        h_source,
        &eval_points,
        num_vars - K,
        field_cfg,
        reducer,
        grinding_bits,
        K,
    )?;

    round_polynomials.extend(tail.round_polynomials);
    eval_points.extend(tail.eval_points);
    round_nonces.extend(tail.round_nonces);

    debug_assert_eq!(round_polynomials.len(), num_vars);
    debug_assert_eq!(eval_points.len(), num_vars);
    debug_assert_eq!(
        round_nonces.len(),
        if grinding_bits == 0 { 0 } else { num_vars }
    );

    Ok(Sha256InnerSumcheckOutput {
        sumcheck_proof: SumcheckProof { round_polynomials },
        round_nonces,
        eval_points,
        final_claim: tail.final_claim,
        v_evaluation: tail.v_evaluation,
        h_evaluation: tail.h_evaluation,
    })
}

fn validate_inputs<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    initial_claim: &Field,
    num_vars: usize,
    live_len: usize,
    h_source: &H,
    field_cfg: &FieldConfig,
    grinding_bits: u32,
) -> Result<(), Sha256InnerSumcheckError> {
    if K > SHA256_INNER_PREFIX_MAX_VARS
        || K > num_vars
        || num_vars >= usize::BITS as usize
        || live_len == 0
        || live_len > (1usize << num_vars)
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }

    let table_len = 1usize << num_vars;
    h_source.validate_shape(live_len, table_len)?;

    validate_field_value(initial_claim, field_cfg)?;

    if grinding_bits > MAX_GRINDING_BITS {
        return Err(GrindingError::InvalidDifficulty {
            bits: grinding_bits,
        }
        .into());
    }
    Ok(())
}

#[inline]
fn validate_field_value(value: &Field, field_cfg: &FieldConfig) -> Result<(), SumcheckError> {
    // `MontyField` embeds a copied `FixedMontyParams`; compare the configs by
    // value rather than by address.
    if value.cfg() != field_cfg {
        return Err(SumcheckError::FieldConfigurationMismatch);
    }
    value
        .validate_element()
        .map_err(|_| SumcheckError::NonCanonicalFieldElement)
}

struct PrefixBuildState {
    partial_sums: Vec<LinearAccumulator>,
    v_values: Vec<Field>,
    v_scratch: Vec<Field>,
    h_values: Vec<i64>,
    h_scratch: Vec<i64>,
}

impl PrefixBuildState {
    fn new<const K: usize>(zero: &Field, reducer: &OptimizedSumcheckReducer) -> Self {
        let prefix_size = 1usize << K;
        let extension_size = pow3(K);
        Self {
            partial_sums: (0..extension_size)
                .map(|_| linear_accumulator_zero(reducer))
                .collect(),
            v_values: vec![zero.clone(); prefix_size],
            v_scratch: vec![zero.clone(); extension_size],
            h_values: vec![0; prefix_size],
            h_scratch: vec![0; extension_size],
        }
    }
}

struct PrefixAccumulators {
    rounds: Vec<Vec<[Field; 2]>>,
}

impl PrefixAccumulators {
    fn new<const K: usize>(zero: &Field) -> Self {
        let rounds = (0..K)
            .map(|round| vec![[zero.clone(), zero.clone()]; pow3(round)])
            .collect();
        Self { rounds }
    }

    fn evaluate_round(
        &self,
        round: usize,
        coefficients: &[Field],
        reducer: &OptimizedSumcheckReducer,
    ) -> Result<[Field; 2], SumcheckError> {
        let buckets = &self.rounds[round];
        debug_assert_eq!(buckets.len(), coefficients.len());
        let mut at_infinity = product_accumulator_zero(reducer);
        let mut at_zero = product_accumulator_zero(reducer);
        for (coefficient, bucket) in coefficients.iter().zip(buckets) {
            product_multiply_accumulate(reducer, &mut at_infinity, coefficient, &bucket[0]);
            product_multiply_accumulate(reducer, &mut at_zero, coefficient, &bucket[1]);
        }
        Ok([
            product_reduce(reducer, at_infinity)?,
            product_reduce(reducer, at_zero)?,
        ])
    }
}

fn build_prefix_accumulators<const K: usize, V, H>(
    num_vars: usize,
    live_len: usize,
    v_at: &V,
    h_source: &H,
    field_cfg: &FieldConfig,
    zero: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<PrefixAccumulators, SumcheckError>
where
    V: Fn(usize) -> Result<Field, SumcheckError> + Sync,
    H: Sha256InnerBitSource + ?Sized,
{
    debug_assert!(K > 0);
    debug_assert!(live_len <= 1usize << num_vars);
    let suffix_count = live_len.div_ceil(1usize << K);

    #[cfg(feature = "parallel")]
    let state = if suffix_count >= 1 << 10 && rayon::current_num_threads() > 1 {
        (0..suffix_count)
            .into_par_iter()
            .try_fold(
                || PrefixBuildState::new::<K>(zero, reducer),
                |mut state, suffix| -> Result<_, SumcheckError> {
                    accumulate_suffix::<K, _, _>(
                        &mut state, field_cfg, live_len, suffix, v_at, h_source, zero, reducer,
                    )?;
                    Ok(state)
                },
            )
            .try_reduce(
                || PrefixBuildState::new::<K>(zero, reducer),
                |left, right| Ok(merge_prefix_states(left, right, reducer)),
            )?
    } else {
        accumulate_suffixes_sequential::<K, _, _>(
            suffix_count,
            live_len,
            v_at,
            h_source,
            field_cfg,
            zero,
            reducer,
        )?
    };

    #[cfg(not(feature = "parallel"))]
    let state = accumulate_suffixes_sequential::<K, _, _>(
        suffix_count,
        live_len,
        v_at,
        h_source,
        field_cfg,
        zero,
        reducer,
    )?;

    let beta_values = state
        .partial_sums
        .into_iter()
        .map(|accumulator| linear_reduce(reducer, accumulator))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(scatter_beta_values::<K>(&beta_values, zero))
}

fn accumulate_suffixes_sequential<const K: usize, V, H>(
    suffix_count: usize,
    live_len: usize,
    v_at: &V,
    h_source: &H,
    field_cfg: &FieldConfig,
    zero: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<PrefixBuildState, SumcheckError>
where
    V: Fn(usize) -> Result<Field, SumcheckError> + Sync,
    H: Sha256InnerBitSource + ?Sized,
{
    let mut state = PrefixBuildState::new::<K>(zero, reducer);
    for suffix in 0..suffix_count {
        accumulate_suffix::<K, _, _>(
            &mut state, field_cfg, live_len, suffix, v_at, h_source, zero, reducer,
        )?;
    }
    Ok(state)
}

fn accumulate_suffix<const K: usize, V, H>(
    state: &mut PrefixBuildState,
    field_cfg: &FieldConfig,
    live_len: usize,
    suffix: usize,
    v_at: &V,
    h_source: &H,
    zero: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<(), SumcheckError>
where
    V: Fn(usize) -> Result<Field, SumcheckError> + Sync,
    H: Sha256InnerBitSource + ?Sized,
{
    let prefix_size = 1usize << K;
    let base = suffix << K;
    state.v_values.resize(prefix_size, zero.clone());
    state.h_values.resize(prefix_size, 0);
    state.v_values.fill(zero.clone());
    state.h_values.fill(0);
    let active_prefixes = prefix_size.min(live_len - base);
    for prefix in 0..active_prefixes {
        let index = base | prefix;
        let value = v_at(index)?;
        validate_field_value(&value, field_cfg)?;
        state.v_values[prefix] = value;
        state.h_values[prefix] = source_bit(h_source, index)? as i64;
    }

    extend_lsb::<Field, K, _>(
        &mut state.v_values,
        &mut state.v_scratch,
        zero,
        |high, low| high.clone() - low,
    );
    extend_lsb::<i64, K, _>(
        &mut state.h_values,
        &mut state.h_scratch,
        &0,
        |high, low| *high - *low,
    );

    for beta in 0..pow3(K) {
        linear_multiply_accumulate_signed(
            reducer,
            &mut state.partial_sums[beta],
            &state.v_values[beta],
            state.h_values[beta],
            zero,
        );
    }
    Ok(())
}

#[cfg(feature = "parallel")]
fn merge_prefix_states(
    mut left: PrefixBuildState,
    right: PrefixBuildState,
    reducer: &OptimizedSumcheckReducer,
) -> PrefixBuildState {
    for (left, right) in left.partial_sums.iter_mut().zip(right.partial_sums) {
        linear_merge(reducer, left, right);
    }
    left
}

fn scatter_beta_values<const K: usize>(beta_values: &[Field], zero: &Field) -> PrefixAccumulators {
    let mut accumulators = PrefixAccumulators::new::<K>(zero);
    for (beta, value) in beta_values.iter().enumerate() {
        for round in 0..K {
            let coordinate = (beta / pow3(round)) % 3;
            if coordinate == 2 || !ternary_suffix_is_binary(beta, round + 1, K) {
                continue;
            }
            let prefix = beta % pow3(round);
            let endpoint = usize::from(coordinate == 1);
            accumulators.rounds[round][prefix][endpoint] += value;
        }
    }
    accumulators
}

fn ternary_suffix_is_binary(beta: usize, start: usize, variables: usize) -> bool {
    let mut remaining = beta / pow3(start);
    for _ in start..variables {
        if remaining % 3 == 0 {
            return false;
        }
        remaining /= 3;
    }
    true
}

fn extend_lsb<T, const K: usize, S>(
    values: &mut Vec<T>,
    scratch: &mut Vec<T>,
    zero: &T,
    subtract: S,
) where
    T: Clone,
    S: Fn(&T, &T) -> T,
{
    debug_assert_eq!(values.len(), 1usize << K);
    let mut current_len = 1usize << K;

    for coordinate in 0..K {
        let low_stride = pow3(coordinate);
        let high_groups = 1usize << (K - coordinate - 1);
        let next_len = current_len / 2 * 3;
        if coordinate % 2 == 0 {
            scratch.resize(next_len, zero.clone());
            extend_lsb_axis(
                &values[..current_len],
                &mut scratch[..next_len],
                low_stride,
                high_groups,
                &subtract,
            );
        } else {
            values.resize(next_len, zero.clone());
            extend_lsb_axis(
                &scratch[..current_len],
                &mut values[..next_len],
                low_stride,
                high_groups,
                &subtract,
            );
        }
        current_len = next_len;
    }

    if K % 2 == 1 {
        values.clear();
        values.extend_from_slice(&scratch[..current_len]);
    } else {
        values.truncate(current_len);
    }
}

fn extend_lsb_axis<T, S>(
    input: &[T],
    output: &mut [T],
    low_stride: usize,
    high_groups: usize,
    subtract: &S,
) where
    T: Clone,
    S: Fn(&T, &T) -> T,
{
    for high_group in 0..high_groups {
        let input_base = high_group * 2 * low_stride;
        let output_base = high_group * 3 * low_stride;
        for low_index in 0..low_stride {
            let low = &input[input_base + low_index];
            let high = &input[input_base + low_stride + low_index];
            output[output_base + low_index] = subtract(high, low);
            output[output_base + low_stride + low_index] = low.clone();
            output[output_base + 2 * low_stride + low_index] = high.clone();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fold_prefix_v_table<const K: usize, V>(
    num_vars: usize,
    live_len: usize,
    v_at: &V,
    challenges: &[Field],
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<CompactPrefixVTable, SumcheckError>
where
    V: Fn(usize) -> Result<Field, SumcheckError> + Sync,
{
    debug_assert_eq!(challenges.len(), K);
    debug_assert!(live_len <= 1usize << num_vars);
    let prefix_size = 1usize << K;
    let suffix_count = live_len.div_ceil(prefix_size);
    let weights = equality_weights_lsb(challenges, zero, one);
    let fold_suffix = |suffix: usize| -> Result<RawMontgomery, SumcheckError> {
        let base = suffix << K;

        if K == 0 {
            let value = v_at(base)?;
            validate_field_value(&value, field_cfg)?;
            return Ok(raw_montgomery(&value));
        }

        let mut v_accumulator = product_accumulator_zero(reducer);
        let active_prefixes = prefix_size.min(live_len - base);
        for (prefix, weight) in weights.iter().take(active_prefixes).enumerate() {
            let index = base | prefix;
            let value = v_at(index)?;
            validate_field_value(&value, field_cfg)?;
            product_multiply_accumulate(reducer, &mut v_accumulator, weight, &value);
        }
        let v = product_reduce(reducer, v_accumulator)?;
        Ok(raw_montgomery(&v))
    };

    let zero_raw = raw_montgomery(zero);
    // Round up by one raw residue so the first tail fold can overwrite every
    // adjacent V pair with one interleaved [V, H] cell. This is at most 16
    // bytes beyond the exact one-residue-per-suffix representation.
    let storage_len = suffix_count
        .checked_add(1)
        .ok_or(SumcheckError::InvalidProductDimensions)?
        & !1;
    let mut table = CompactPrefixVTable {
        values: vec![zero_raw; storage_len],
        suffix_count,
    };

    #[cfg(feature = "parallel")]
    if suffix_count >= 1 << 10 && rayon::current_num_threads() > 1 {
        table.values[..suffix_count]
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(suffix, v_out)| -> Result<(), SumcheckError> {
                *v_out = fold_suffix(suffix)?;
                Ok(())
            })?;
    } else {
        for (suffix, v_out) in table.values[..suffix_count].iter_mut().enumerate() {
            *v_out = fold_suffix(suffix)?;
        }
    }

    #[cfg(not(feature = "parallel"))]
    for (suffix, v_out) in table.values[..suffix_count].iter_mut().enumerate() {
        *v_out = fold_suffix(suffix)?;
    }

    Ok(table)
}

struct CompactPrefixVTable {
    /// Before the first tail round, one V residue per suffix plus at most one
    /// zero placeholder. Afterwards, interleaved [V, H] cells in the same
    /// allocation.
    values: Vec<RawMontgomery>,
    suffix_count: usize,
}

struct CompactTailOutput {
    round_polynomials: Vec<[Field; 3]>,
    round_nonces: Vec<u64>,
    eval_points: Vec<Field>,
    final_claim: Field,
    v_evaluation: Field,
    h_evaluation: Field,
}

#[allow(clippy::too_many_arguments)]
fn prove_compact_tail<const K: usize, T: Transcript, H: Sha256InnerBitSource + ?Sized>(
    transcript: &mut T,
    mut current_claim: Field,
    mut table: CompactPrefixVTable,
    live_len: usize,
    h_source: &H,
    prefix_challenges: &[Field],
    num_vars: usize,
    field_cfg: &FieldConfig,
    reducer: &OptimizedSumcheckReducer,
    grinding_bits: u32,
    round_offset: usize,
) -> Result<CompactTailOutput, SumcheckError> {
    debug_assert_eq!(prefix_challenges.len(), K);
    debug_assert!(!table.values.is_empty());
    debug_assert!(table.suffix_count <= 1usize << num_vars);
    debug_assert_eq!(table.values.len(), (table.suffix_count + 1) & !1);

    let zero = Field::zero_with_cfg(field_cfg);
    let one = Field::one_with_cfg(field_cfg);
    let prefix_weights = equality_weights_lsb(prefix_challenges, &zero, &one);
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_nonces = Vec::with_capacity(if grinding_bits == 0 { 0 } else { num_vars });

    if num_vars == 0 {
        debug_assert_eq!(table.suffix_count, 1);
        let v_evaluation = field_from_raw(&table.values[0], field_cfg);
        let h_evaluation = folded_packed_h::<K, _>(
            0,
            live_len,
            h_source,
            &prefix_weights,
            field_cfg,
            &zero,
            &one,
            reducer,
        )?;
        return Ok(CompactTailOutput {
            round_polynomials,
            round_nonces,
            eval_points,
            final_claim: current_claim,
            v_evaluation,
            h_evaluation,
        });
    }

    // H starts as packed bits, not a second dense field table. Stream its K
    // already-fixed coordinates for the first tail message, then overwrite
    // each adjacent V pair with the folded interleaved [V, H] cell. All later
    // rounds use that single allocation.
    let [at_zero, leading] = sum_first_tail_round::<K, _>(
        &table,
        live_len,
        h_source,
        &prefix_weights,
        field_cfg,
        &zero,
        &one,
        reducer,
    )?;
    let coefficients = quadratic_coefficients(&current_claim, &at_zero, &leading);
    absorb_field_elements(transcript, &coefficients);
    if grinding_bits != 0 {
        let round_index =
            u64::try_from(round_offset).expect("an in-memory sumcheck round index fits in u64");
        round_nonces.push(grind_and_absorb::<Sha256InnerGrinding, _>(
            transcript,
            GrindingRound::new(round_index),
            grinding_bits,
        )?);
    }
    let challenge = squeeze_field(transcript, field_cfg);
    current_claim = evaluate_quadratic(&coefficients, &challenge, &zero);
    round_polynomials.push(coefficients);
    eval_points.push(challenge.clone());
    let mut stride = 1usize;
    // After each nonterminal challenge, the fold pass also prepares the next
    // round's [c0, c2]. The transcript still receives one ordinary quadratic
    // per round; only the table traversal that computes it moves earlier.
    let mut prepared_round = if num_vars > 1 {
        Some(fold_first_tail_round_and_prepare_next_in_place::<K, _>(
            &mut table,
            live_len,
            h_source,
            &prefix_weights,
            &challenge,
            field_cfg,
            &zero,
            &one,
            reducer,
        )?)
    } else {
        fold_first_tail_round_in_place::<K, _>(
            &mut table,
            live_len,
            h_source,
            &prefix_weights,
            &challenge,
            field_cfg,
            &zero,
            &one,
            reducer,
        )?;
        None
    };

    for tail_round in 1..num_vars {
        let [at_zero, leading] = prepared_round
            .take()
            .expect("every non-initial tail round has prepared coefficients");
        let coefficients = quadratic_coefficients(&current_claim, &at_zero, &leading);
        absorb_field_elements(transcript, &coefficients);
        if grinding_bits != 0 {
            let round = round_offset + tail_round;
            let round_index =
                u64::try_from(round).expect("an in-memory sumcheck round index fits in u64");
            round_nonces.push(grind_and_absorb::<Sha256InnerGrinding, _>(
                transcript,
                GrindingRound::new(round_index),
                grinding_bits,
            )?);
        }
        let challenge = squeeze_field(transcript, field_cfg);
        current_claim = evaluate_quadratic(&coefficients, &challenge, &zero);
        round_polynomials.push(coefficients);
        eval_points.push(challenge.clone());

        if tail_round + 1 < num_vars {
            prepared_round = Some(fold_interleaved_and_prepare_next_round_in_place(
                &mut table.values,
                stride,
                &challenge,
                field_cfg,
                &zero,
                reducer,
            )?);
        } else {
            fold_interleaved_in_place(&mut table.values, stride, &challenge, field_cfg, &zero);
        }
        stride *= 2;
    }

    let v_evaluation = field_from_raw(&table.values[0], field_cfg);
    let h_evaluation = field_from_raw(&table.values[1], field_cfg);
    // Do not assert terminal consistency here: malformed witnesses are valid
    // untrusted prover inputs. The caller compares this final claim with
    // V(r)·H(r) and returns `InvalidInnerTerminalClaim` without panicking.

    Ok(CompactTailOutput {
        round_polynomials,
        round_nonces,
        eval_points,
        final_claim: current_claim,
        v_evaluation,
        h_evaluation,
    })
}

#[allow(clippy::too_many_arguments)]
fn folded_packed_h<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    suffix: usize,
    live_len: usize,
    h_source: &H,
    prefix_weights: &[Field],
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<Field, SumcheckError> {
    let base = suffix << K;
    debug_assert!(base < live_len);
    if K == 0 {
        return Ok(select_field_by_bit(zero, one, source_bit(h_source, base)?));
    }

    let active_prefixes = (1usize << K).min(live_len - base);
    let mut accumulator = linear_accumulator_zero(reducer);
    for (prefix, weight) in prefix_weights.iter().take(active_prefixes).enumerate() {
        linear_multiply_accumulate(
            reducer,
            &mut accumulator,
            weight,
            &source_bit(h_source, base | prefix)?,
        );
    }
    let value = linear_reduce(reducer, accumulator)?;
    validate_field_value(&value, field_cfg)?;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
fn accumulate_first_tail_pair<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    accumulators: &mut [ProductAccumulator; 2],
    table: &CompactPrefixVTable,
    pair: usize,
    live_len: usize,
    h_source: &H,
    prefix_weights: &[Field],
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<(), SumcheckError> {
    let low_suffix = 2 * pair;
    let high_suffix = low_suffix + 1;
    let v_zero = field_from_raw(&table.values[low_suffix], field_cfg);
    let h_zero = folded_packed_h::<K, _>(
        low_suffix,
        live_len,
        h_source,
        prefix_weights,
        field_cfg,
        zero,
        one,
        reducer,
    )?;
    let (v_one, h_one) = if high_suffix < table.suffix_count {
        (
            field_from_raw(&table.values[high_suffix], field_cfg),
            folded_packed_h::<K, _>(
                high_suffix,
                live_len,
                h_source,
                prefix_weights,
                field_cfg,
                zero,
                one,
                reducer,
            )?,
        )
    } else {
        (zero.clone(), zero.clone())
    };

    product_multiply_accumulate(reducer, &mut accumulators[0], &v_zero, &h_zero);
    product_multiply_accumulate(
        reducer,
        &mut accumulators[1],
        &(v_one - &v_zero),
        &(h_one - &h_zero),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn sum_first_tail_round<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    table: &CompactPrefixVTable,
    live_len: usize,
    h_source: &H,
    prefix_weights: &[Field],
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<[Field; 2], SumcheckError> {
    let pair_count = table.suffix_count.div_ceil(2);

    #[cfg(feature = "parallel")]
    if pair_count >= 1 << 10 && rayon::current_num_threads() > 1 {
        let accumulators = (0..pair_count)
            .into_par_iter()
            .try_fold(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |mut accumulators, pair| -> Result<_, SumcheckError> {
                    accumulate_first_tail_pair::<K, _>(
                        &mut accumulators,
                        table,
                        pair,
                        live_len,
                        h_source,
                        prefix_weights,
                        field_cfg,
                        zero,
                        one,
                        reducer,
                    )?;
                    Ok(accumulators)
                },
            )
            .try_reduce(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |left, right| Ok(merge_product_accumulators(left, right, reducer)),
            )?;
        return reduce_product_accumulators(accumulators, reducer);
    }

    let mut accumulators = std::array::from_fn(|_| product_accumulator_zero(reducer));
    for pair in 0..pair_count {
        accumulate_first_tail_pair::<K, _>(
            &mut accumulators,
            table,
            pair,
            live_len,
            h_source,
            prefix_weights,
            field_cfg,
            zero,
            one,
            reducer,
        )?;
    }
    reduce_product_accumulators(accumulators, reducer)
}

#[allow(clippy::too_many_arguments)]
fn fold_first_tail_pair_in_place<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    pair: usize,
    values: &mut [RawMontgomery],
    suffix_count: usize,
    live_len: usize,
    h_source: &H,
    prefix_weights: &[Field],
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<[Field; 2], SumcheckError> {
    debug_assert_eq!(values.len(), 2);
    let low_suffix = 2 * pair;
    let high_suffix = low_suffix + 1;
    let h_zero = folded_packed_h::<K, _>(
        low_suffix,
        live_len,
        h_source,
        prefix_weights,
        field_cfg,
        zero,
        one,
        reducer,
    )?;
    let h_one = if high_suffix < suffix_count {
        folded_packed_h::<K, _>(
            high_suffix,
            live_len,
            h_source,
            prefix_weights,
            field_cfg,
            zero,
            one,
            reducer,
        )?
    } else {
        zero.clone()
    };
    let folded_v = interpolate_raw_pair(
        &values[0],
        (high_suffix < suffix_count).then_some(&values[1]),
        challenge,
        field_cfg,
        zero,
    );
    let folded_h = h_zero.clone() + challenge * &(h_one - &h_zero);
    values[0] = raw_montgomery(&folded_v);
    values[1] = raw_montgomery(&folded_h);
    Ok([folded_v, folded_h])
}

#[allow(clippy::too_many_arguments)]
fn fold_first_tail_round_in_place<const K: usize, H: Sha256InnerBitSource + ?Sized>(
    table: &mut CompactPrefixVTable,
    live_len: usize,
    h_source: &H,
    prefix_weights: &[Field],
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<(), SumcheckError> {
    let suffix_count = table.suffix_count;
    let fold_pair = |pair: usize, values: &mut [RawMontgomery]| {
        fold_first_tail_pair_in_place::<K, _>(
            pair,
            values,
            suffix_count,
            live_len,
            h_source,
            prefix_weights,
            challenge,
            field_cfg,
            zero,
            one,
            reducer,
        )
        .map(|_| ())
    };

    #[cfg(feature = "parallel")]
    if table.values.len() / 2 >= 1 << 10 && rayon::current_num_threads() > 1 {
        table
            .values
            .par_chunks_mut(2)
            .enumerate()
            .try_for_each(|(pair, values)| fold_pair(pair, values))?;
        table.suffix_count = table.values.len() / 2;
        return Ok(());
    }

    for (pair, values) in table.values.chunks_mut(2).enumerate() {
        fold_pair(pair, values)?;
    }
    table.suffix_count = table.values.len() / 2;
    Ok(())
}

/// Folds the first tail coordinate and prepares the following round's
/// `[c0, c2]` buckets while the new interleaved `[V, H]` cells are hot.
#[allow(clippy::too_many_arguments)]
fn fold_first_tail_round_and_prepare_next_in_place<
    const K: usize,
    H: Sha256InnerBitSource + ?Sized,
>(
    table: &mut CompactPrefixVTable,
    live_len: usize,
    h_source: &H,
    prefix_weights: &[Field],
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
    one: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<[Field; 2], SumcheckError> {
    let suffix_count = table.suffix_count;
    let fold_and_accumulate = |mut accumulators: [ProductAccumulator; 2],
                               superchunk: usize,
                               values: &mut [RawMontgomery]|
     -> Result<[ProductAccumulator; 2], SumcheckError> {
        debug_assert!(values.len() == 2 || values.len() == 4);
        let first_pair = 2 * superchunk;
        let folded_zero = fold_first_tail_pair_in_place::<K, _>(
            first_pair,
            &mut values[..2],
            suffix_count,
            live_len,
            h_source,
            prefix_weights,
            challenge,
            field_cfg,
            zero,
            one,
            reducer,
        )?;
        let folded_one = if values.len() == 4 {
            fold_first_tail_pair_in_place::<K, _>(
                first_pair + 1,
                &mut values[2..],
                suffix_count,
                live_len,
                h_source,
                prefix_weights,
                challenge,
                field_cfg,
                zero,
                one,
                reducer,
            )?
        } else {
            [zero.clone(), zero.clone()]
        };

        product_multiply_accumulate(
            reducer,
            &mut accumulators[0],
            &folded_zero[0],
            &folded_zero[1],
        );
        product_multiply_accumulate(
            reducer,
            &mut accumulators[1],
            &(folded_one[0].clone() - &folded_zero[0]),
            &(folded_one[1].clone() - &folded_zero[1]),
        );
        Ok(accumulators)
    };

    #[cfg(feature = "parallel")]
    if table.values.len() / 2 >= 1 << 10 && rayon::current_num_threads() > 1 {
        let accumulators = table
            .values
            .par_chunks_mut(4)
            .enumerate()
            .try_fold(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |accumulators, (superchunk, values)| {
                    fold_and_accumulate(accumulators, superchunk, values)
                },
            )
            .try_reduce(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |left, right| Ok(merge_product_accumulators(left, right, reducer)),
            )?;
        table.suffix_count = table.values.len() / 2;
        return reduce_product_accumulators(accumulators, reducer);
    }

    let mut accumulators = std::array::from_fn(|_| product_accumulator_zero(reducer));
    for (superchunk, values) in table.values.chunks_mut(4).enumerate() {
        accumulators = fold_and_accumulate(accumulators, superchunk, values)?;
    }
    table.suffix_count = table.values.len() / 2;
    reduce_product_accumulators(accumulators, reducer)
}

#[cfg(test)]
fn sum_interleaved_round_coefficients(
    values: &[RawMontgomery],
    stride: usize,
    field_cfg: &FieldConfig,
    reducer: &OptimizedSumcheckReducer,
) -> Result<[Field; 2], SumcheckError> {
    debug_assert!(!values.is_empty());
    debug_assert_eq!(values.len() % 2, 0);
    let chunk_len = 4 * stride;
    let zero = Field::zero_with_cfg(field_cfg);

    #[cfg(feature = "parallel")]
    if values.len().div_ceil(chunk_len) >= 1 << 10 && rayon::current_num_threads() > 1 {
        let accumulators = values
            .par_chunks(chunk_len)
            .fold(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |mut accumulators, values| {
                    accumulate_interleaved_chunk(
                        &mut accumulators,
                        values,
                        stride,
                        field_cfg,
                        &zero,
                        reducer,
                    );
                    accumulators
                },
            )
            .reduce(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |left, right| merge_product_accumulators(left, right, reducer),
            );
        return reduce_product_accumulators(accumulators, reducer);
    }

    let accumulators = values.chunks(chunk_len).fold(
        std::array::from_fn(|_| product_accumulator_zero(reducer)),
        |mut accumulators, values| {
            accumulate_interleaved_chunk(
                &mut accumulators,
                values,
                stride,
                field_cfg,
                &zero,
                reducer,
            );
            accumulators
        },
    );
    reduce_product_accumulators(accumulators, reducer)
}

#[cfg(test)]
fn accumulate_interleaved_chunk(
    accumulators: &mut [ProductAccumulator; 2],
    values: &[RawMontgomery],
    stride: usize,
    field_cfg: &FieldConfig,
    zero: &Field,
    reducer: &OptimizedSumcheckReducer,
) {
    debug_assert!(values.len() >= 2);
    debug_assert!(values.len() <= 4 * stride);
    let high_offset = 2 * stride;
    let v_zero = field_from_raw(&values[0], field_cfg);
    let h_zero = field_from_raw(&values[1], field_cfg);
    let v_one = values
        .get(high_offset)
        .map_or_else(|| zero.clone(), |raw| field_from_raw(raw, field_cfg));
    let h_one = values
        .get(high_offset + 1)
        .map_or_else(|| zero.clone(), |raw| field_from_raw(raw, field_cfg));

    product_multiply_accumulate(reducer, &mut accumulators[0], &v_zero, &h_zero);
    product_multiply_accumulate(
        reducer,
        &mut accumulators[1],
        &(v_one - &v_zero),
        &(h_one - &h_zero),
    );
}

fn fold_interleaved_chunk_in_place(
    values: &mut [RawMontgomery],
    stride: usize,
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
) -> [Field; 2] {
    debug_assert!(values.len() >= 2);
    debug_assert!(values.len() <= 4 * stride);
    let high_offset = 2 * stride;
    let folded_v = interpolate_raw_pair(
        &values[0],
        values.get(high_offset),
        challenge,
        field_cfg,
        zero,
    );
    let folded_h = interpolate_raw_pair(
        &values[1],
        values.get(high_offset + 1),
        challenge,
        field_cfg,
        zero,
    );
    values[0] = raw_montgomery(&folded_v);
    values[1] = raw_montgomery(&folded_h);
    [folded_v, folded_h]
}

fn fold_interleaved_in_place(
    values: &mut [RawMontgomery],
    stride: usize,
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
) {
    debug_assert!(!values.is_empty());
    debug_assert_eq!(values.len() % 2, 0);
    let chunk_len = 4 * stride;

    #[cfg(feature = "parallel")]
    if values.len().div_ceil(chunk_len) >= 1 << 10 && rayon::current_num_threads() > 1 {
        values.par_chunks_mut(chunk_len).for_each(|values| {
            fold_interleaved_chunk_in_place(values, stride, challenge, field_cfg, zero);
        });
        return;
    }

    for chunk in values.chunks_mut(chunk_len) {
        fold_interleaved_chunk_in_place(chunk, stride, challenge, field_cfg, zero);
    }
}

/// Folds one interleaved tail coordinate and prepares the following round's
/// `[c0, c2]` buckets from adjacent pairs of newly folded cells.
fn fold_interleaved_and_prepare_next_round_in_place(
    values: &mut [RawMontgomery],
    stride: usize,
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
    reducer: &OptimizedSumcheckReducer,
) -> Result<[Field; 2], SumcheckError> {
    debug_assert!(!values.is_empty());
    debug_assert_eq!(values.len() % 2, 0);
    let fold_chunk_len = 4 * stride;
    let superchunk_len = 2 * fold_chunk_len;
    let fold_and_accumulate = |mut accumulators: [ProductAccumulator; 2],
                               values: &mut [RawMontgomery]| {
        debug_assert!(values.len() >= 2);
        debug_assert!(values.len() <= superchunk_len);
        let first_len = values.len().min(fold_chunk_len);
        let (first, second) = values.split_at_mut(first_len);
        let folded_zero =
            fold_interleaved_chunk_in_place(first, stride, challenge, field_cfg, zero);
        let folded_one = if second.is_empty() {
            [zero.clone(), zero.clone()]
        } else {
            fold_interleaved_chunk_in_place(second, stride, challenge, field_cfg, zero)
        };

        product_multiply_accumulate(
            reducer,
            &mut accumulators[0],
            &folded_zero[0],
            &folded_zero[1],
        );
        product_multiply_accumulate(
            reducer,
            &mut accumulators[1],
            &(folded_one[0].clone() - &folded_zero[0]),
            &(folded_one[1].clone() - &folded_zero[1]),
        );
        accumulators
    };

    #[cfg(feature = "parallel")]
    if values.len().div_ceil(fold_chunk_len) >= 1 << 10 && rayon::current_num_threads() > 1 {
        let accumulators = values
            .par_chunks_mut(superchunk_len)
            .fold(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                fold_and_accumulate,
            )
            .reduce(
                || std::array::from_fn(|_| product_accumulator_zero(reducer)),
                |left, right| merge_product_accumulators(left, right, reducer),
            );
        return reduce_product_accumulators(accumulators, reducer);
    }

    let accumulators = values.chunks_mut(superchunk_len).fold(
        std::array::from_fn(|_| product_accumulator_zero(reducer)),
        fold_and_accumulate,
    );
    reduce_product_accumulators(accumulators, reducer)
}

#[inline]
fn interpolate_raw_pair(
    low: &RawMontgomery,
    high: Option<&RawMontgomery>,
    challenge: &Field,
    field_cfg: &FieldConfig,
    zero: &Field,
) -> Field {
    let low = field_from_raw(low, field_cfg);
    let high = high.map_or_else(|| zero.clone(), |raw| field_from_raw(raw, field_cfg));
    low.clone() + challenge * &(high - &low)
}

#[inline]
fn raw_montgomery(value: &Field) -> RawMontgomery {
    let words = value.as_montgomery().as_words();
    [words[0], words[1]]
}

#[inline]
fn field_from_raw(raw: &RawMontgomery, field_cfg: &FieldConfig) -> Field {
    MontyField::from_montgomery(FieldUint::from_words(*raw), field_cfg)
}

#[inline]
fn merge_product_accumulators(
    mut left: [ProductAccumulator; 2],
    right: [ProductAccumulator; 2],
    reducer: &OptimizedSumcheckReducer,
) -> [ProductAccumulator; 2] {
    product_merge(reducer, &mut left[0], right[0]);
    product_merge(reducer, &mut left[1], right[1]);
    left
}

#[inline]
fn reduce_product_accumulators(
    accumulators: [ProductAccumulator; 2],
    reducer: &OptimizedSumcheckReducer,
) -> Result<[Field; 2], SumcheckError> {
    let [at_zero, leading] = accumulators;
    Ok([
        product_reduce(reducer, at_zero)?,
        product_reduce(reducer, leading)?,
    ])
}

fn equality_weights_lsb(challenges: &[Field], zero: &Field, one: &Field) -> Vec<Field> {
    let mut weights = vec![one.clone()];
    for challenge in challenges {
        let old_len = weights.len();
        weights.resize(2 * old_len, zero.clone());
        let one_minus_challenge = one.clone() - challenge;
        for index in 0..old_len {
            let parent = weights[index].clone();
            weights[index + old_len] = parent.clone() * challenge;
            weights[index] = parent * &one_minus_challenge;
        }
    }
    weights
}

fn quadratic_coefficients(claim: &Field, at_zero: &Field, leading: &Field) -> [Field; 3] {
    let mut linear = claim.clone();
    linear -= at_zero;
    linear -= at_zero;
    linear -= leading;
    [at_zero.clone(), linear, leading.clone()]
}

fn evaluate_quadratic(coefficients: &[Field; 3], point: &Field, zero: &Field) -> Field {
    coefficients
        .iter()
        .rev()
        .fold(zero.clone(), |value, coefficient| {
            value * point + coefficient
        })
}

fn extend_lagrange_coefficients(
    coefficients: &mut Vec<Field>,
    challenge: &Field,
    one: &Field,
    zero: &Field,
) {
    // For U_2 = {infinity, 0, 1}, infinity denotes the quadratic leading
    // coefficient. Thus p(r) = L_inf(r)p_inf + L_0(r)p(0) + L_1(r)p(1).
    let at_one = challenge.clone();
    let at_zero = one.clone() - challenge;
    let at_infinity = challenge.clone() * (challenge.clone() - one);
    let old_len = coefficients.len();
    let mut next = vec![zero.clone(); 3 * old_len];
    for (prefix, coefficient) in coefficients.iter().enumerate() {
        next[prefix] = coefficient.clone() * &at_infinity;
        next[old_len + prefix] = coefficient.clone() * &at_zero;
        next[2 * old_len + prefix] = coefficient.clone() * &at_one;
    }
    *coefficients = next;
}

#[inline]
fn source_bit<H: Sha256InnerBitSource + ?Sized>(
    source: &H,
    index: usize,
) -> Result<u64, SumcheckError> {
    let bit = source.bit_at(index)?;
    if bit > 1 {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    Ok(bit)
}

#[cfg(test)]
#[inline]
fn packed_bit(words: &[u64], index: usize) -> u64 {
    (words[index / u64::BITS as usize] >> (index % u64::BITS as usize)) & 1
}

#[inline]
fn select_field_by_bit(zero: &Field, one: &Field, bit: u64) -> Field {
    MontyField::from_montgomery(
        CtSelect::ct_select(
            zero.as_montgomery(),
            one.as_montgomery(),
            Choice::from(bit as u8),
        ),
        zero.cfg(),
    )
}

#[inline]
fn linear_multiply_accumulate_signed(
    reducer: &OptimizedSumcheckReducer,
    accumulator: &mut LinearAccumulator,
    value: &Field,
    signed_coefficient: i64,
    zero: &Field,
) {
    // A Boolean K<=4 extension has magnitude at most 2^(K-1), but perform the
    // sign extraction in i128 so the signed-magnitude conversion is total.
    let signed_coefficient = i128::from(signed_coefficient);
    let sign_mask = (signed_coefficient >> 127) as u128;
    let magnitude = ((signed_coefficient as u128) ^ sign_mask).wrapping_sub(sign_mask) as u64;
    let negative_value = zero.clone() - value;
    let selected_value = MontyField::from_montgomery(
        CtSelect::ct_select(
            value.as_montgomery(),
            negative_value.as_montgomery(),
            Choice::from((sign_mask & 1) as u8),
        ),
        value.cfg(),
    );
    linear_multiply_accumulate(reducer, accumulator, &selected_value, &magnitude);
}

#[inline]
fn linear_accumulator_zero(reducer: &OptimizedSumcheckReducer) -> LinearAccumulator {
    <OptimizedSumcheckReducer as SumcheckLinearReducer>::accumulator_zero(reducer)
}

#[inline]
fn linear_multiply_accumulate(
    reducer: &OptimizedSumcheckReducer,
    accumulator: &mut LinearAccumulator,
    lhs: &Field,
    rhs: &u64,
) {
    <OptimizedSumcheckReducer as SumcheckLinearReducer>::multiply_accumulate(
        reducer,
        accumulator,
        lhs,
        rhs,
    );
}

#[cfg(feature = "parallel")]
#[inline]
fn linear_merge(
    reducer: &OptimizedSumcheckReducer,
    accumulator: &mut LinearAccumulator,
    other: LinearAccumulator,
) {
    <OptimizedSumcheckReducer as SumcheckLinearReducer>::merge(reducer, accumulator, other);
}

#[inline]
fn linear_reduce(
    reducer: &OptimizedSumcheckReducer,
    accumulator: LinearAccumulator,
) -> Result<Field, SumcheckError> {
    <OptimizedSumcheckReducer as SumcheckLinearReducer>::reduce(reducer, accumulator)
}

#[inline]
fn product_accumulator_zero(reducer: &OptimizedSumcheckReducer) -> ProductAccumulator {
    <OptimizedSumcheckReducer as SumcheckProductReducer<Field>>::accumulator_zero(reducer)
}

#[inline]
fn product_multiply_accumulate(
    reducer: &OptimizedSumcheckReducer,
    accumulator: &mut ProductAccumulator,
    lhs: &Field,
    rhs: &Field,
) {
    <OptimizedSumcheckReducer as SumcheckProductReducer<Field>>::multiply_accumulate(
        reducer,
        accumulator,
        lhs,
        rhs,
    );
}

#[inline]
fn product_merge(
    reducer: &OptimizedSumcheckReducer,
    accumulator: &mut ProductAccumulator,
    other: ProductAccumulator,
) {
    <OptimizedSumcheckReducer as SumcheckProductReducer<Field>>::merge(reducer, accumulator, other);
}

#[inline]
fn product_reduce(
    reducer: &OptimizedSumcheckReducer,
    accumulator: ProductAccumulator,
) -> Result<Field, SumcheckError> {
    <OptimizedSumcheckReducer as SumcheckProductReducer<Field>>::reduce(reducer, accumulator)
}

const fn pow3(exponent: usize) -> usize {
    let mut result = 1usize;
    let mut index = 0usize;
    while index < exponent {
        result *= 3;
        index += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crypto_primitives::{FromWithConfig, crypto_bigint_uint::Uint};

    use crate::{
        piop::spartan::f2z::spartan_f2z_field_config,
        piop::spartan::sumcheck::{
            prove_inner_sumcheck_with_reducer, prove_inner_sumcheck_with_reducer_grinded,
        },
        poly::mle::DenseMultilinearExtension,
        transcript::{Blake3Transcript, traits::Transcript},
    };

    use super::*;

    fn field(value: u64, field_cfg: &FieldConfig) -> Field {
        Field::from_with_cfg(value, field_cfg)
    }

    fn fixture(
        num_vars: usize,
        field_cfg: &FieldConfig,
    ) -> (DenseMultilinearExtension<Field>, Vec<u64>, Field) {
        let table_len = 1usize << num_vars;
        let zero = Field::zero_with_cfg(field_cfg);
        let v = (0..table_len)
            .map(|index| field((17 * index as u64 + 3) % 251, field_cfg))
            .collect::<Vec<_>>();
        let mut h_words = vec![0u64; table_len.div_ceil(u64::BITS as usize)];
        let mut claim = zero.clone();
        for (index, value) in v.iter().enumerate() {
            let bit = (((index as u64).wrapping_mul(0x9e37_79b9) ^ (index as u64 >> 1))
                .count_ones()
                & 1) as u64;
            h_words[index / u64::BITS as usize] |= bit << (index % u64::BITS as usize);
            if bit != 0 {
                claim += value;
            }
        }
        (
            DenseMultilinearExtension::from_evaluations_vec(num_vars, v, zero),
            h_words,
            claim,
        )
    }

    fn assert_first_tail_fused_matches_reference<const K: usize>() {
        const NUM_VARS: usize = 8;
        const LIVE_LEN: usize = 13;

        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let one = Field::one_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let values = (0..LIVE_LEN)
            .map(|index| field(19 * index as u64 + 5, &field_cfg))
            .collect::<Vec<_>>();
        let mut h_words = vec![0u64; LIVE_LEN.div_ceil(64)];
        for index in 0..LIVE_LEN {
            let bit = ((index * 7 + 3).count_ones() & 1) as u64;
            h_words[index / 64] |= bit << (index % 64);
        }
        let prefix_challenges = (0..K)
            .map(|index| field(11 * index as u64 + 7, &field_cfg))
            .collect::<Vec<_>>();
        let prefix_weights = equality_weights_lsb(&prefix_challenges, &zero, &one);
        let table = fold_prefix_v_table::<K, _>(
            NUM_VARS,
            LIVE_LEN,
            &|index| Ok(values[index].clone()),
            &prefix_challenges,
            &field_cfg,
            &zero,
            &one,
            &reducer,
        )
        .unwrap();
        let mut reference = CompactPrefixVTable {
            values: table.values.clone(),
            suffix_count: table.suffix_count,
        };
        let mut fused = table;
        let challenge = field(113, &field_cfg);

        fold_first_tail_round_in_place::<K, _>(
            &mut reference,
            LIVE_LEN,
            &h_words,
            &prefix_weights,
            &challenge,
            &field_cfg,
            &zero,
            &one,
            &reducer,
        )
        .unwrap();
        let expected =
            sum_interleaved_round_coefficients(&reference.values, 1, &field_cfg, &reducer).unwrap();
        let actual = fold_first_tail_round_and_prepare_next_in_place::<K, _>(
            &mut fused,
            LIVE_LEN,
            &h_words,
            &prefix_weights,
            &challenge,
            &field_cfg,
            &zero,
            &one,
            &reducer,
        )
        .unwrap();

        assert_eq!(fused.values, reference.values, "K={K}");
        assert_eq!(fused.suffix_count, reference.suffix_count, "K={K}");
        assert_eq!(actual, expected, "K={K}");
    }

    #[test]
    fn fused_first_tail_fold_and_prepare_matches_separate_passes() {
        assert_first_tail_fused_matches_reference::<0>();
        assert_first_tail_fused_matches_reference::<1>();
        assert_first_tail_fused_matches_reference::<2>();
        assert_first_tail_fused_matches_reference::<3>();
        assert_first_tail_fused_matches_reference::<4>();
    }

    #[test]
    fn fused_interleaved_fold_and_prepare_matches_separate_passes() {
        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();

        for stride in [1usize, 2, 4] {
            let fold_chunk_len = 4 * stride;
            let lengths = [
                2,
                2 * stride + 2,
                fold_chunk_len,
                fold_chunk_len + 2,
                2 * fold_chunk_len,
                3 * fold_chunk_len + 2,
                fold_chunk_len * (1 << 10),
            ];
            for length in lengths {
                for challenge in [
                    zero.clone(),
                    Field::one_with_cfg(&field_cfg),
                    field(211, &field_cfg),
                ] {
                    let values = (0..length)
                        .map(|index| raw_montgomery(&field(13 * index as u64 + 17, &field_cfg)))
                        .collect::<Vec<_>>();
                    let mut reference = values.clone();
                    let mut fused = values;

                    fold_interleaved_in_place(
                        &mut reference,
                        stride,
                        &challenge,
                        &field_cfg,
                        &zero,
                    );
                    let expected = sum_interleaved_round_coefficients(
                        &reference,
                        2 * stride,
                        &field_cfg,
                        &reducer,
                    )
                    .unwrap();
                    let actual = fold_interleaved_and_prepare_next_round_in_place(
                        &mut fused, stride, &challenge, &field_cfg, &zero, &reducer,
                    )
                    .unwrap();

                    assert_eq!(fused, reference, "stride={stride}, length={length}");
                    assert_eq!(actual, expected, "stride={stride}, length={length}");
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn prove_dense_test<T: Transcript>(
        transcript: &mut T,
        initial_claim: Field,
        v_mle: DenseMultilinearExtension<Field>,
        h_words: &[u64],
        prefix_vars: usize,
        field_cfg: &FieldConfig,
        reducer: &OptimizedSumcheckReducer,
        grinding_bits: u32,
    ) -> Result<Sha256InnerSumcheckOutput, SumcheckError> {
        let num_vars = v_mle.num_vars;
        let live_len = v_mle.evaluations.len();
        prove_sha256_inner_sumcheck(
            transcript,
            initial_claim,
            num_vars,
            live_len,
            &|index| Ok(v_mle.evaluations[index].clone()),
            h_words,
            prefix_vars,
            field_cfg,
            reducer,
            grinding_bits,
        )
    }

    #[test]
    fn every_prefix_width_matches_ordinary_field_prover_with_and_without_grinding() {
        const NUM_VARS: usize = 7;

        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let one = Field::one_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let (v_mle, h_words, initial_claim) = fixture(NUM_VARS, &field_cfg);
        let h_mle = DenseMultilinearExtension::from_evaluations_vec(
            NUM_VARS,
            (0..v_mle.evaluations.len())
                .map(|index| select_field_by_bit(&zero, &one, packed_bit(&h_words, index)))
                .collect(),
            zero,
        );

        for grinding_bits in [0, 3] {
            let mut reference_transcript = Blake3Transcript::new();
            let (ordinary, reference_nonces) =
                prove_inner_sumcheck_with_reducer_grinded::<Sha256InnerGrinding, _, _>(
                    &mut reference_transcript,
                    initial_claim.clone(),
                    v_mle.clone(),
                    h_mle.clone(),
                    &field_cfg,
                    &reducer,
                    grinding_bits,
                    0,
                )
                .unwrap();
            let reference = Sha256InnerSumcheckOutput {
                sumcheck_proof: ordinary.sumcheck.proof,
                round_nonces: reference_nonces,
                eval_points: ordinary.sumcheck.eval_points,
                final_claim: ordinary.sumcheck.final_claim,
                v_evaluation: ordinary.batched_matrix_evaluation,
                h_evaluation: ordinary.witness_evaluation,
            };
            let reference_continuation = reference_transcript.get_challenge::<u128>();

            for prefix_vars in 0..=SHA256_INNER_PREFIX_MAX_VARS {
                let mut prover_transcript = Blake3Transcript::new();
                let output = prove_dense_test(
                    &mut prover_transcript,
                    initial_claim.clone(),
                    v_mle.clone(),
                    &h_words,
                    prefix_vars,
                    &field_cfg,
                    &reducer,
                    grinding_bits,
                )
                .unwrap();
                let prover_continuation = prover_transcript.get_challenge::<u128>();

                assert_eq!(output, reference, "K={prefix_vars}, bits={grinding_bits}");
                assert_eq!(prover_continuation, reference_continuation);
                assert_eq!(
                    output.round_nonces.len(),
                    if grinding_bits == 0 { 0 } else { NUM_VARS }
                );

                let mut verifier_transcript = Blake3Transcript::new();
                let (eval_points, final_claim) = verify_sha256_inner_sumcheck(
                    &mut verifier_transcript,
                    initial_claim.clone(),
                    &output.sumcheck_proof,
                    &output.round_nonces,
                    NUM_VARS,
                    &field_cfg,
                    grinding_bits,
                )
                .unwrap();
                assert_eq!(eval_points, output.eval_points);
                assert_eq!(final_claim, output.final_claim);
                assert_eq!(
                    final_claim,
                    output.v_evaluation.clone() * &output.h_evaluation
                );
                assert_eq!(
                    verifier_transcript.get_challenge::<u128>(),
                    reference_continuation
                );
            }
        }
    }

    #[test]
    fn live_prefix_storage_omits_the_global_zero_suffix_and_matches_dense_reference() {
        const NUM_VARS: usize = 7;
        const LIVE_LEN: usize = 61;

        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let one = Field::one_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let table_len = 1usize << NUM_VARS;
        let mut v = vec![zero.clone(); table_len];
        let mut h_padded = vec![0u64; table_len.div_ceil(u64::BITS as usize)];
        let mut initial_claim = zero.clone();
        for index in 0..LIVE_LEN {
            v[index] = field((29 * index as u64 + 11) % 251, &field_cfg);
            let bit = ((index * 13 + 5).count_ones() & 1) as u64;
            h_padded[index / 64] |= bit << (index % 64);
            if bit != 0 {
                initial_claim += &v[index];
            }
        }
        let h_live = h_padded[..LIVE_LEN.div_ceil(64)].to_vec();
        let v_mle =
            DenseMultilinearExtension::from_evaluations_vec(NUM_VARS, v.clone(), zero.clone());
        let h_mle = DenseMultilinearExtension::from_evaluations_vec(
            NUM_VARS,
            (0..table_len)
                .map(|index| select_field_by_bit(&zero, &one, packed_bit(&h_padded, index)))
                .collect(),
            zero.clone(),
        );
        let mut reference_transcript = Blake3Transcript::new();
        let reference = prove_inner_sumcheck_with_reducer(
            &mut reference_transcript,
            initial_claim.clone(),
            v_mle,
            h_mle,
            &field_cfg,
            &reducer,
        )
        .unwrap();
        let reference_continuation = reference_transcript.get_challenge::<u128>();

        for prefix_vars in 0..=SHA256_INNER_PREFIX_MAX_VARS {
            for h_words in [&h_live[..], &h_padded[..]] {
                let largest_query = AtomicUsize::new(0);
                let oracle = |index: usize| {
                    if index >= LIVE_LEN {
                        return Err(SumcheckError::InvalidProductDimensions);
                    }
                    largest_query.fetch_max(index, Ordering::Relaxed);
                    Ok(v[index].clone())
                };
                let mut transcript = Blake3Transcript::new();
                let output = prove_sha256_inner_sumcheck(
                    &mut transcript,
                    initial_claim.clone(),
                    NUM_VARS,
                    LIVE_LEN,
                    &oracle,
                    h_words,
                    prefix_vars,
                    &field_cfg,
                    &reducer,
                    0,
                )
                .unwrap();

                assert_eq!(largest_query.load(Ordering::Relaxed), LIVE_LEN - 1);
                assert_eq!(output.sumcheck_proof, reference.sumcheck.proof);
                assert_eq!(output.eval_points, reference.sumcheck.eval_points);
                assert_eq!(output.final_claim, reference.sumcheck.final_claim);
                assert_eq!(output.v_evaluation, reference.batched_matrix_evaluation);
                assert_eq!(output.h_evaluation, reference.witness_evaluation);
                assert_eq!(transcript.get_challenge::<u128>(), reference_continuation);
            }
        }

        assert_eq!(core::mem::size_of::<RawMontgomery>(), 16);
        assert!(core::mem::size_of::<Field>() > core::mem::size_of::<RawMontgomery>());
        let challenges = [field(7, &field_cfg), field(19, &field_cfg)];
        let table = fold_prefix_v_table::<2, _>(
            NUM_VARS,
            LIVE_LEN,
            &|index| Ok(v[index].clone()),
            &challenges,
            &field_cfg,
            &zero,
            &one,
            &reducer,
        )
        .unwrap();
        let suffix_count = LIVE_LEN.div_ceil(1 << 2);
        assert_eq!(table.suffix_count, suffix_count);
        assert_eq!(table.values.len(), (suffix_count + 1) & !1);
        assert!(table.values.len() < 2 * suffix_count);
        assert!(
            table
                .values
                .iter()
                .all(|raw| field_from_raw(raw, &field_cfg).cfg() == &field_cfg)
        );
    }

    #[test]
    fn lazy_bit_source_spanning_packed_rows_matches_contiguous_words() {
        const NUM_VARS: usize = 7;
        const LIVE_LEN: usize = 93;
        const ROW_BITS: usize = 19;

        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let table_len = 1usize << NUM_VARS;
        let mut v = vec![zero.clone(); table_len];
        let mut h_words = vec![0u64; LIVE_LEN.div_ceil(64)];
        let mut h_rows = vec![0u64; LIVE_LEN.div_ceil(ROW_BITS)];
        let mut initial_claim = zero;
        for index in 0..LIVE_LEN {
            v[index] = field((41 * index as u64 + 7) % 251, &field_cfg);
            let bit = ((index * 23 + 9).count_ones() & 1) as u64;
            h_words[index / 64] |= bit << (index % 64);
            h_rows[index / ROW_BITS] |= bit << (index % ROW_BITS);
            if bit != 0 {
                initial_claim += &v[index];
            }
        }

        for prefix_vars in 0..=SHA256_INNER_PREFIX_MAX_VARS {
            let mut packed_transcript = Blake3Transcript::new();
            let packed = prove_sha256_inner_sumcheck(
                &mut packed_transcript,
                initial_claim.clone(),
                NUM_VARS,
                LIVE_LEN,
                &|index| Ok(v[index].clone()),
                &h_words,
                prefix_vars,
                &field_cfg,
                &reducer,
                0,
            )
            .unwrap();

            let largest_query = AtomicUsize::new(0);
            let row_source = |index: usize| {
                if index >= LIVE_LEN {
                    return Err(SumcheckError::InvalidProductDimensions);
                }
                largest_query.fetch_max(index, Ordering::Relaxed);
                Ok((h_rows[index / ROW_BITS] >> (index % ROW_BITS)) & 1)
            };
            let mut lazy_transcript = Blake3Transcript::new();
            let lazy = prove_sha256_inner_sumcheck(
                &mut lazy_transcript,
                initial_claim.clone(),
                NUM_VARS,
                LIVE_LEN,
                &|index| Ok(v[index].clone()),
                &row_source,
                prefix_vars,
                &field_cfg,
                &reducer,
                0,
            )
            .unwrap();

            assert_eq!(largest_query.load(Ordering::Relaxed), LIVE_LEN - 1);
            assert_eq!(lazy, packed);
            assert_eq!(
                lazy_transcript.get_challenge::<u128>(),
                packed_transcript.get_challenge::<u128>()
            );
        }
    }

    #[test]
    fn oracle_field_configuration_is_checked_before_the_transcript() {
        let field_cfg = spartan_f2z_field_config();
        let foreign_cfg = Field::make_cfg(&Uint::from((1_u128 << 127) - 1)).unwrap();
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let (_, h_words, initial_claim) = fixture(3, &field_cfg);
        let mut transcript = Blake3Transcript::new();
        let mut untouched = transcript.clone();
        let result = prove_sha256_inner_sumcheck(
            &mut transcript,
            initial_claim,
            3,
            8,
            &|_| Ok(field(1, &foreign_cfg)),
            &h_words,
            2,
            &field_cfg,
            &reducer,
            0,
        );
        assert_eq!(result, Err(SumcheckError::FieldConfigurationMismatch));
        assert_eq!(
            transcript.get_challenge::<u128>(),
            untouched.get_challenge::<u128>()
        );
    }

    #[test]
    fn k0_is_identical_to_the_existing_field_quadratic_prover() {
        const NUM_VARS: usize = 7;

        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let one = Field::one_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let (v_mle, h_words, initial_claim) = fixture(NUM_VARS, &field_cfg);
        let h_mle = DenseMultilinearExtension::from_evaluations_vec(
            NUM_VARS,
            (0..v_mle.evaluations.len())
                .map(|index| select_field_by_bit(&zero, &one, packed_bit(&h_words, index)))
                .collect(),
            zero,
        );

        let mut ordinary_transcript = Blake3Transcript::new();
        let ordinary = prove_inner_sumcheck_with_reducer(
            &mut ordinary_transcript,
            initial_claim.clone(),
            v_mle.clone(),
            h_mle,
            &field_cfg,
            &reducer,
        )
        .unwrap();
        let ordinary_continuation = ordinary_transcript.get_challenge::<u128>();

        let mut k0_transcript = Blake3Transcript::new();
        let k0 = prove_dense_test(
            &mut k0_transcript,
            initial_claim,
            v_mle,
            &h_words,
            0,
            &field_cfg,
            &reducer,
            0,
        )
        .unwrap();

        assert_eq!(k0.sumcheck_proof, ordinary.sumcheck.proof);
        assert_eq!(k0.eval_points, ordinary.sumcheck.eval_points);
        assert_eq!(k0.final_claim, ordinary.sumcheck.final_claim);
        assert_eq!(k0.v_evaluation, ordinary.batched_matrix_evaluation);
        assert_eq!(k0.h_evaluation, ordinary.witness_evaluation);
        assert!(k0.round_nonces.is_empty());
        assert_eq!(k0_transcript.get_challenge::<u128>(), ordinary_continuation);
    }

    #[test]
    fn extension_uses_low_coordinate_first_u2_layout() {
        // f(x0, x1) = x0 XOR x1 in binary table order with x0 as the low bit.
        let mut values = vec![0i64, 1, 1, 0];
        let mut scratch = vec![0i64; pow3(2)];
        extend_lsb::<i64, 2, _>(&mut values, &mut scratch, &0, |high, low| *high - *low);

        // Ternary digit order per coordinate is [infinity, 0, 1], with x0
        // the least-significant digit.
        assert_eq!(values, vec![-2, 1, -1, 1, 0, 1, -1, 1, 0]);
    }

    #[test]
    fn first_round_is_pinned_to_the_word_lsb() {
        let field_cfg = spartan_f2z_field_config();
        let zero = Field::zero_with_cfg(&field_cfg);
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let v_mle = DenseMultilinearExtension::from_evaluations_vec(
            2,
            [2, 5, 11, 17]
                .into_iter()
                .map(|value| field(value, &field_cfg))
                .collect(),
            zero.clone(),
        );
        // h = [1, 0, 1, 1], so the exact first polynomial is
        // 13 + 7 X - 3 X^2 when coordinate zero is the word's low bit.
        let initial_claim = field(30, &field_cfg);

        for prefix_vars in [0, 1, 2] {
            let output = prove_dense_test(
                &mut Blake3Transcript::new(),
                initial_claim.clone(),
                v_mle.clone(),
                &[0b1101],
                prefix_vars,
                &field_cfg,
                &reducer,
                0,
            )
            .unwrap();
            assert_eq!(
                output.sumcheck_proof.round_polynomials[0],
                [
                    field(13, &field_cfg),
                    field(7, &field_cfg),
                    zero.clone() - field(3, &field_cfg),
                ]
            );
        }
    }

    #[test]
    fn zero_variable_and_fully_native_prefixes_match_k0() {
        let field_cfg = spartan_f2z_field_config();
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();

        let zero_var = prove_dense_test(
            &mut Blake3Transcript::new(),
            field(9, &field_cfg),
            DenseMultilinearExtension::zero_vars(field(9, &field_cfg)),
            &[1],
            0,
            &field_cfg,
            &reducer,
            0,
        )
        .unwrap();
        assert!(zero_var.sumcheck_proof.round_polynomials.is_empty());
        assert!(zero_var.eval_points.is_empty());
        assert_eq!(zero_var.v_evaluation, field(9, &field_cfg));
        assert_eq!(zero_var.h_evaluation, field(1, &field_cfg));

        let (v_mle, h_words, initial_claim) = fixture(4, &field_cfg);
        let reference = prove_dense_test(
            &mut Blake3Transcript::new(),
            initial_claim.clone(),
            v_mle.clone(),
            &h_words,
            0,
            &field_cfg,
            &reducer,
            0,
        )
        .unwrap();
        let fully_native = prove_dense_test(
            &mut Blake3Transcript::new(),
            initial_claim,
            v_mle,
            &h_words,
            4,
            &field_cfg,
            &reducer,
            0,
        )
        .unwrap();
        assert_eq!(fully_native, reference);
        assert_eq!(
            fully_native.final_claim,
            fully_native.v_evaluation.clone() * &fully_native.h_evaluation
        );
    }

    #[test]
    fn invalid_prefix_or_noncanonical_padding_does_not_touch_transcript() {
        let field_cfg = spartan_f2z_field_config();
        let reducer = OptimizedSumcheckReducer::new(&field_cfg).unwrap();
        let (v_mle, h_words, initial_claim) = fixture(3, &field_cfg);

        let mut invalid_k_transcript = Blake3Transcript::new();
        let untouched_k = invalid_k_transcript.clone();
        assert_eq!(
            prove_dense_test(
                &mut invalid_k_transcript,
                initial_claim.clone(),
                v_mle.clone(),
                &h_words,
                SHA256_INNER_PREFIX_MAX_VARS + 1,
                &field_cfg,
                &reducer,
                0,
            ),
            Err(SumcheckError::InvalidProductDimensions)
        );
        assert_eq!(
            invalid_k_transcript.get_challenge::<u128>(),
            untouched_k.clone().get_challenge::<u128>()
        );

        let mut too_wide_transcript = Blake3Transcript::new();
        let mut untouched_width = too_wide_transcript.clone();
        assert_eq!(
            prove_dense_test(
                &mut too_wide_transcript,
                initial_claim.clone(),
                v_mle.clone(),
                &h_words,
                4,
                &field_cfg,
                &reducer,
                0,
            ),
            Err(SumcheckError::InvalidProductDimensions)
        );
        assert_eq!(
            too_wide_transcript.get_challenge::<u128>(),
            untouched_width.get_challenge::<u128>()
        );

        for prefix_vars in [0, 2] {
            let mut invalid_bit_transcript = Blake3Transcript::new();
            let mut untouched_bit = invalid_bit_transcript.clone();
            assert_eq!(
                prove_sha256_inner_sumcheck(
                    &mut invalid_bit_transcript,
                    initial_claim.clone(),
                    v_mle.num_vars,
                    v_mle.evaluations.len(),
                    &|index| Ok(v_mle.evaluations[index].clone()),
                    &|_| Ok(2),
                    prefix_vars,
                    &field_cfg,
                    &reducer,
                    0,
                ),
                Err(SumcheckError::InvalidProductDimensions)
            );
            assert_eq!(
                invalid_bit_transcript.get_challenge::<u128>(),
                untouched_bit.get_challenge::<u128>()
            );
        }

        let mut padded_words = h_words;
        padded_words[0] |= 1 << 63;
        let mut padded_transcript = Blake3Transcript::new();
        let mut untouched_padding = padded_transcript.clone();
        assert_eq!(
            prove_dense_test(
                &mut padded_transcript,
                initial_claim,
                v_mle,
                &padded_words,
                2,
                &field_cfg,
                &reducer,
                0,
            ),
            Err(SumcheckError::InvalidProductDimensions)
        );
        assert_eq!(
            padded_transcript.get_challenge::<u128>(),
            untouched_padding.get_challenge::<u128>()
        );
    }
}
