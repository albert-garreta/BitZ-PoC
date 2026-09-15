//! Compact raw-residue kernels for the two-limb runtime-field Spartan prover.
//!
//! Dense tables retain reduced Montgomery residues as `u128` beside one shared
//! [`field::FpCtx<2>`]. Both this storage and the shared `Fp<2>` element occupy
//! two limbs; neither stores a per-element context. Native witness/product
//! segments remain borrowed until their first fold, whose output representation
//! matches the next round. Scalar arithmetic, exact accumulation and reduction
//! delegate to the shared provider with its prepared constants.
//!
//! Every kernel computes exactly the field elements the generic `sumcheck.rs`
//! and `matrix.rs` prover code computes: the same equality tables, the same
//! folded tables, the same round-polynomial coefficients, and the same
//! terminal claims. Exact modular arithmetic is independent of evaluation
//! order, so the sumcheck messages, transcript, and proof bytes are
//! bit-identical to the generic prover's. Verifier code never uses this
//! module.
//!
//! Two further prover-only shortcuts live here, both value-preserving: the
//! inner sumcheck touches only the live (validated-zero-padded) prefix of its
//! tables, and for block-selector relations (`BlockSelectorLayout`) it never
//! materializes the batched matrix — that matrix is a per-block scaled copy
//! of the row-weight vector, so one weight table is folded alongside the
//! witness blocks (`prove_inner_structured_raw`).

use crate::piop::spartan::SpartanField as _;
use crate::utils::delayed_reduction::EncodedMac;
#[cfg(test)]
use field::Uint;
use field::{Fp, RingOps};
use std::borrow::Cow;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::transcript::traits::Transcript;

use super::{
    absorb_field_elements,
    baby_bear_mul::{BABY_BEAR_MODULUS, BabyBearMulCoefficient},
    matrix::{
        BlockSelectorLayout, PrefixUnivariateRowFactors, PreparedConstraintMatrices,
        SpartanMatrixCoefficient,
    },
    sumcheck::{
        FactoredEndpoint, InnerSumcheckOutput, OuterSumcheckOutput, OuterSumcheckProof,
        R1csProductMles, RoundBoundaryPolicy, SumcheckError, SumcheckProof, SumcheckProverOutput,
        TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS, UngrindedRoundBoundary, batch_invert_nonzero,
        equality_coordinate_evaluation, reconstruct_eq_factored_cubic_without_linear,
        recover_full_round_polynomial_and_sample_next_challenge,
        recover_full_round_polynomial_and_sample_next_challenge_with_boundary,
    },
    u64_mul::{U64_MUL_LIMB_BASE, U64MulCoefficient},
};

mod folded;
mod native_products;
mod native_witness;
use folded::{FoldedValue, FoldedWitness};
pub(crate) use native_products::NativeOuterInput;
pub use native_products::NativeWideProducts;
pub use native_witness::{NativeLimbWitness, NativeU128Witness};

type Field = Fp<2>;
type FieldConfig = field::FpCtx<2>;

/// One canonical Montgomery residue of the runtime field: the same two limbs
/// `Fp<2>::as_montgomery` exposes, packed little-endian into a `u128`.
pub(crate) type Raw = u128;

/// Minimum work items before a kernel splits across the rayon pool. Matches
/// the generic prover's threshold; the split never changes any value.
#[cfg(feature = "parallel")]
const PARALLEL_MIN_ITEMS: usize = 1 << 12;

/// Output elements per parallel block in the fused fold kernels (even, so a
/// block always holds whole output pairs).
const FOLD_BLOCK: usize = 1 << 11;

/// Columns per parallel block in the matrix binding kernel.
const BIND_BLOCK: usize = 1 << 12;

#[cfg(feature = "parallel")]
#[inline]
fn parallel(work_items: usize) -> bool {
    work_items >= PARALLEL_MIN_ITEMS && rayon::current_num_threads() > 1
}

#[inline(always)]
const fn words_to_raw(words: &[u64; 2]) -> Raw {
    (words[0] as u128) | ((words[1] as u128) << 64)
}

#[inline(always)]
const fn raw_to_words(value: Raw) -> [u64; 2] {
    [value as u64, (value >> 64) as u64]
}

/// Prepares the shared arithmetic provider from the current protocol's config.
/// Configuration ownership stays with the prepared relation and round scalars.
pub(crate) fn field_context(config: &FieldConfig) -> field::FpCtx<2> {
    config.clone()
}

pub trait RawFieldStorage {
    fn one_raw(&self) -> Raw;
    fn raw(&self, value: &Field) -> Raw;
    fn native_residue(&self, value: u64) -> Raw;
    fn native_residue_u128(&self, value: u128) -> Raw;
    fn two_pow_128_residue(&self) -> Raw;
    fn native_residue_u256(&self, low: u128, high: u128, two_pow_128: Raw) -> Raw;
    fn raw_vec(&self, values: &[Field]) -> Vec<Raw>;
    fn add_raw(&self, lhs: Raw, rhs: Raw) -> Raw;
    fn sub_raw(&self, lhs: Raw, rhs: Raw) -> Raw;
    fn neg_raw(&self, value: Raw) -> Raw;
    fn mul_raw(&self, lhs: Raw, rhs: Raw) -> Raw;
    fn plain_to_raw(&self, plain: Raw) -> Raw;
    fn redc_linear(&self, accumulator: &field::FpLinearAcc<2, 1>) -> Raw;
    fn redc(&self, value: [u64; 4]) -> Raw;
    fn interpolate(&self, at_zero: Raw, at_one: Raw, point: Raw) -> Raw;
}

impl RawFieldStorage for field::FpCtx<2> {
    #[inline]
    fn one_raw(&self) -> Raw {
        raw_shared(field::RingOps::one(&self))
    }
    #[inline]
    fn raw(&self, value: &Field) -> Raw {
        words_to_raw(value.as_montgomery_integer().as_words())
    }
    #[inline]
    fn native_residue(&self, value: u64) -> Raw {
        raw_shared(field::IntegerEmbedding::from_integer(&self, &value))
    }
    #[inline]
    fn native_residue_u128(&self, value: u128) -> Raw {
        raw_shared(field::IntegerEmbedding::from_integer(&self, &value))
    }
    #[inline]
    fn two_pow_128_residue(&self) -> Raw {
        let half = self.native_residue_u128(1_u128 << 127);
        self.add_raw(half, half)
    }
    #[inline]
    fn native_residue_u256(&self, low: u128, high: u128, two_pow_128: Raw) -> Raw {
        let high = self.mul_raw(self.native_residue_u128(high), two_pow_128);
        self.add_raw(self.native_residue_u128(low), high)
    }
    #[inline]
    fn raw_vec(&self, values: &[Field]) -> Vec<Raw> {
        #[cfg(feature = "parallel")]
        if parallel(values.len()) {
            return values.par_iter().map(|value| self.raw(value)).collect();
        }
        values.iter().map(|value| self.raw(value)).collect()
    }
    #[inline]
    fn add_raw(&self, lhs: Raw, rhs: Raw) -> Raw {
        self.add_canonical_u128(lhs, rhs)
    }
    #[inline]
    fn sub_raw(&self, lhs: Raw, rhs: Raw) -> Raw {
        self.sub_canonical_u128(lhs, rhs)
    }
    #[inline]
    fn neg_raw(&self, value: Raw) -> Raw {
        self.sub_raw(0, value)
    }
    #[inline]
    fn mul_raw(&self, lhs: Raw, rhs: Raw) -> Raw {
        raw_shared(field::RingOps::mul(
            &self,
            &shared_raw(&self, lhs),
            &shared_raw(&self, rhs),
        ))
    }
    #[inline]
    fn plain_to_raw(&self, plain: Raw) -> Raw {
        raw_shared(self.from_canonical_integer(&field::Uint::from_words(raw_to_words(plain))))
    }
    #[inline]
    fn redc_linear(&self, accumulator: &field::FpLinearAcc<2, 1>) -> Raw {
        let (lo, hi, head) = accumulator.unreduced_integer().as_parts();
        let limbs = [lo[0], lo[1], hi[0], head, 0];
        debug_assert_eq!(limbs[4], 0);
        // Sufficient for `sum < q · R`: the top limb stays below q's top limb.
        debug_assert!(limbs[3] < (self.modulus_u128() >> 64) as u64);
        self.redc([limbs[0], limbs[1], limbs[2], limbs[3]])
    }
    #[inline]
    fn redc(&self, value: [u64; 4]) -> Raw {
        words_to_raw(
            self.reduce_montgomery_bounded(&field::Uint::from_words(value))
                .as_words(),
        )
    }
    #[inline]
    fn interpolate(&self, at_zero: Raw, at_one: Raw, point: Raw) -> Raw {
        self.add_raw(at_zero, self.mul_raw(point, self.sub_raw(at_one, at_zero)))
    }
}
#[inline(always)]
fn shared_raw(ctx: &field::FpCtx<2>, raw: Raw) -> field::Fp<2> {
    ctx.from_montgomery_integer(field::Uint::from_words(raw_to_words(raw)))
}
#[inline(always)]
fn raw_shared(value: field::Fp<2>) -> Raw {
    words_to_raw(value.as_montgomery_integer().as_words())
}

// ---------------------------------------------------------------------------
// Equality tables
// ---------------------------------------------------------------------------

/// `eq(boolean_index, point)` in little-endian index order, the raw twin of
/// `matrix::eq_table`: the same doubling recurrence, entry for entry.
pub(crate) fn eq_table_raw(ctx: &field::FpCtx<2>, point: &[Raw]) -> Vec<Raw> {
    let mut table = vec![0 as Raw; 1usize << point.len()];
    table[0] = ctx.one_raw();
    for (coordinate, &challenge) in point.iter().enumerate() {
        let half = 1usize << coordinate;
        let (zero_children, one_children) = table[..2 * half].split_at_mut(half);
        let expand = |zero_child: &mut Raw, one_child: &mut Raw| {
            let parent = *zero_child;
            *one_child = ctx.mul_raw(parent, challenge);
            *zero_child = ctx.sub_raw(parent, *one_child);
        };
        #[cfg(feature = "parallel")]
        if half >= (1 << 13) && rayon::current_num_threads() > 1 {
            zero_children
                .par_iter_mut()
                .zip(one_children.par_iter_mut())
                .for_each(|(zero_child, one_child)| expand(zero_child, one_child));
            continue;
        }
        for (zero_child, one_child) in zero_children.iter_mut().zip(one_children) {
            expand(zero_child, one_child);
        }
    }
    table
}

/// The low- and high-coordinate equality factors of `matrix::make_equality_factors`,
/// as raw tables: the first `len / 2` coordinates form the low table.
pub(crate) fn make_equality_factors_raw(
    ctx: &field::FpCtx<2>,
    point: &[Field],
) -> (Vec<Raw>, Vec<Raw>) {
    let split = point.len() / 2;
    let raw_point = ctx.raw_vec(point);
    (
        eq_table_raw(ctx, &raw_point[..split]),
        eq_table_raw(ctx, &raw_point[split..]),
    )
}

/// Sums adjacent pairs: the equality table with its active (lowest) coordinate
/// removed.
fn strip_coordinate_raw(ctx: &field::FpCtx<2>, input: &[Raw]) -> Vec<Raw> {
    debug_assert!(input.len() >= 2 && input.len().is_power_of_two());
    #[cfg(feature = "parallel")]
    if parallel(input.len() / 2) {
        return input
            .par_chunks_exact(2)
            .map(|pair| ctx.add_raw(pair[0], pair[1]))
            .collect();
    }
    input
        .chunks_exact(2)
        .map(|pair| ctx.add_raw(pair[0], pair[1]))
        .collect()
}

/// Equality weights with the active coordinate stripped: the pair weight is
/// `low[pair % low.len()] · high[pair / low.len()]`, or `high[pair]` once the
/// low coordinates are exhausted.
#[derive(Clone, Copy)]
struct RawEqWeights<'a> {
    low: Option<&'a [Raw]>,
    high: &'a [Raw],
}

impl<'a> RawEqWeights<'a> {
    #[inline(always)]
    fn pair_weight(&self, ctx: &field::FpCtx<2>, pair: usize) -> Raw {
        match self.low {
            Some(low) => ctx.mul_raw(low[pair % low.len()], self.high[pair / low.len()]),
            None => self.high[pair],
        }
    }

    /// The low table when the two-level bucket decomposition applies.
    #[inline]
    fn two_level_low(&self) -> Option<&'a [Raw]> {
        self.low
            .filter(|low| low.len() >= TWO_LEVEL_EQUALITY_MIN_LOW_PAIRS)
    }

    fn pair_count(&self) -> usize {
        self.low.map_or(1, <[Raw]>::len) * self.high.len()
    }
}

// ---------------------------------------------------------------------------
// Outer sumcheck
// ---------------------------------------------------------------------------

/// Borrowed exact native `Az`, `Bz`, `Cz` tables of one common power-of-two
/// length (the row domain), read by the first outer round and the prefix
/// skip without copying the relation's witness.
#[derive(Clone, Copy)]
pub(crate) struct NativeProducts<'a> {
    pub az: &'a [u64],
    pub bz: &'a [u64],
    pub cz: &'a [u64],
}

impl<'a> NativeProducts<'a> {
    pub(crate) fn from_mles(products: &'a R1csProductMles<u64>) -> Self {
        Self {
            az: &products.az.evaluations,
            bz: &products.bz.evaluations,
            cz: &products.cz.evaluations,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.az.len()
    }
}

/// Owned raw `Az`, `Bz`, `Cz` tables.
pub struct RawProducts {
    pub az: Vec<Raw>,
    pub bz: Vec<Raw>,
    pub cz: Vec<Raw>,
}

impl RawProducts {
    pub(crate) fn zeros(len: usize) -> Self {
        Self {
            az: vec![0; len],
            bz: vec![0; len],
            cz: vec![0; len],
        }
    }

    pub(crate) fn from_field(ctx: &field::FpCtx<2>, products: &R1csProductMles<Field>) -> Self {
        Self {
            az: ctx.raw_vec(&products.az.evaluations),
            bz: ctx.raw_vec(&products.bz.evaluations),
            cz: ctx.raw_vec(&products.cz.evaluations),
        }
    }

    pub(crate) fn len(&self) -> usize {
        debug_assert_eq!(self.az.len(), self.bz.len());
        debug_assert_eq!(self.az.len(), self.cz.len());
        self.az.len()
    }

    fn truncate(&mut self, len: usize) {
        self.az.truncate(len);
        self.bz.truncate(len);
        self.cz.truncate(len);
    }

    fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.az, &mut other.az);
        std::mem::swap(&mut self.bz, &mut other.bz);
        std::mem::swap(&mut self.cz, &mut other.cz);
    }
}

type ProductPair = [field::FpProductAcc<2>; 2];
type LinearPair = [field::FpLinearAcc<2, 1>; 2];

#[inline(always)]
fn product_pair() -> ProductPair {
    [
        field::FpProductAcc::<2>::default(),
        field::FpProductAcc::<2>::default(),
    ]
}

#[inline(always)]
fn linear_pair() -> LinearPair {
    [
        field::FpLinearAcc::<2, 1>::default(),
        field::FpLinearAcc::<2, 1>::default(),
    ]
}

#[inline(always)]
fn merge_product_pair(mut left: ProductPair, right: ProductPair) -> ProductPair {
    left[0] += right[0];
    left[1] += right[1];
    left
}

#[inline(always)]
fn reduce_product_pair(pair: ProductPair, reducer: &field::FpCtx<2>) -> [Raw; 2] {
    let [endpoint, infinity] = pair;
    [
        endpoint.reduce_encoded(reducer),
        infinity.reduce_encoded(reducer),
    ]
}

#[inline(always)]
fn reduce_linear_pair(pair: LinearPair, reducer: &field::FpCtx<2>) -> [Raw; 2] {
    let [endpoint, infinity] = pair;
    [
        endpoint.reduce_encoded(reducer),
        infinity.reduce_encoded(reducer),
    ]
}

/// Adds one pair's endpoint residual and leading coefficient, weighted.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn accumulate_cofactor_raw(
    ctx: &field::FpCtx<2>,
    accumulators: &mut ProductPair,
    weight: Raw,
    endpoint: FactoredEndpoint,
    az0: Raw,
    az1: Raw,
    bz0: Raw,
    bz1: Raw,
    cz0: Raw,
    cz1: Raw,
) {
    let residual = match endpoint {
        FactoredEndpoint::Zero => ctx.sub_raw(ctx.mul_raw(az0, bz0), cz0),
        FactoredEndpoint::One => ctx.sub_raw(ctx.mul_raw(az1, bz1), cz1),
    };
    accumulators[0].accumulate_encoded(ctx, weight, residual);
    let infinity = ctx.mul_raw(ctx.sub_raw(az1, az0), ctx.sub_raw(bz1, bz0));
    accumulators[1].accumulate_encoded(ctx, weight, infinity);
}

/// Branch-free `accumulator += (±weight) · |value|` for `|value| ≤ u64::MAX`.
#[inline(always)]
fn accumulate_signed_raw(
    ctx: &field::FpCtx<2>,
    accumulator: &mut field::FpLinearAcc<2, 1>,
    weight: Raw,
    negative_weight: Raw,
    value: i128,
) {
    let mask = (value >> 127) as u128;
    let magnitude = ((value as u128) ^ mask).wrapping_sub(mask);
    debug_assert!(magnitude <= u128::from(u64::MAX));
    let selected = (weight & !mask) | (negative_weight & mask);
    accumulator.accumulate_encoded(ctx, selected, magnitude as u64);
}

/// Adds one native pair's exact endpoint residual and leading coefficient.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn accumulate_native_cofactor_raw(
    ctx: &field::FpCtx<2>,
    accumulators: &mut LinearPair,
    weight: Raw,
    negative_weight: Raw,
    endpoint: FactoredEndpoint,
    az0: u64,
    az1: u64,
    bz0: u64,
    bz1: u64,
    cz0: u64,
    cz1: u64,
) {
    let (az_e, bz_e, cz_e) = match endpoint {
        FactoredEndpoint::Zero => (az0, bz0, cz0),
        FactoredEndpoint::One => (az1, bz1, cz1),
    };
    // Multiplicands are validated 32-bit wide, so the product is a u64 and the
    // residual magnitude is at most u64::MAX; each delta is below 2^32.
    let residual = i128::from(az_e * bz_e) - i128::from(cz_e);
    let infinity = (i128::from(az1) - i128::from(az0)) * (i128::from(bz1) - i128::from(bz0));
    accumulate_signed_raw(ctx, &mut accumulators[0], weight, negative_weight, residual);
    accumulate_signed_raw(ctx, &mut accumulators[1], weight, negative_weight, infinity);
}

/// `[endpoint evaluation, leading coefficient]` of the cofactor over the
/// current raw product tables.
fn cofactor_evaluations_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    products: &RawProducts,
    weights: RawEqWeights<'_>,
    endpoint: FactoredEndpoint,
) -> [Raw; 2] {
    let pair_count = products.len() / 2;
    debug_assert_eq!(pair_count, weights.pair_count());

    if let Some(low) = weights.two_level_low() {
        let low_pairs = low.len();
        let bucket = |high_index: usize| -> ProductPair {
            let mut inner = product_pair();
            let start = 2 * high_index * low_pairs;
            for (low_index, &weight) in low.iter().enumerate() {
                let index = start + 2 * low_index;
                accumulate_cofactor_raw(
                    ctx,
                    &mut inner,
                    weight,
                    endpoint,
                    products.az[index],
                    products.az[index + 1],
                    products.bz[index],
                    products.bz[index + 1],
                    products.cz[index],
                    products.cz[index + 1],
                );
            }
            let inner = reduce_product_pair(inner, reducer);
            let high_weight = weights.high[high_index];
            let mut outer = product_pair();
            outer[0].accumulate_encoded(ctx, high_weight, inner[0]);
            outer[1].accumulate_encoded(ctx, high_weight, inner[1]);
            outer
        };
        #[cfg(feature = "parallel")]
        if parallel(pair_count) {
            let total = (0..weights.high.len())
                .into_par_iter()
                .map(bucket)
                .reduce(product_pair, merge_product_pair);
            return reduce_product_pair(total, reducer);
        }
        let total = (0..weights.high.len())
            .map(bucket)
            .fold(product_pair(), merge_product_pair);
        return reduce_product_pair(total, reducer);
    }

    let block = |start: usize, end: usize| -> ProductPair {
        let mut accumulators = product_pair();
        for pair in start..end {
            let index = 2 * pair;
            let weight = weights.pair_weight(ctx, pair);
            accumulate_cofactor_raw(
                ctx,
                &mut accumulators,
                weight,
                endpoint,
                products.az[index],
                products.az[index + 1],
                products.bz[index],
                products.bz[index + 1],
                products.cz[index],
                products.cz[index + 1],
            );
        }
        accumulators
    };
    #[cfg(feature = "parallel")]
    if parallel(pair_count) {
        let blocks = pair_count.div_ceil(FOLD_BLOCK);
        let total = (0..blocks)
            .into_par_iter()
            .map(|b| block(b * FOLD_BLOCK, ((b + 1) * FOLD_BLOCK).min(pair_count)))
            .reduce(product_pair, merge_product_pair);
        return reduce_product_pair(total, reducer);
    }
    reduce_product_pair(block(0, pair_count), reducer)
}

/// The native (exact `u64`) twin of [`cofactor_evaluations_raw`] for the first
/// outer round: field × `u64` accumulation, one reduction per bucket.
fn native_cofactor_evaluations_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    products: NativeProducts<'_>,
    weights: RawEqWeights<'_>,
    endpoint: FactoredEndpoint,
) -> [Raw; 2] {
    let (az, bz, cz) = (products.az, products.bz, products.cz);
    let pair_count = az.len() / 2;
    debug_assert_eq!(pair_count, weights.pair_count());

    if let Some(low) = weights.two_level_low() {
        let negative_low: Vec<Raw> = low.iter().map(|&weight| ctx.neg_raw(weight)).collect();
        let low_pairs = low.len();
        let bucket = |high_index: usize| -> ProductPair {
            let mut inner = linear_pair();
            let start = 2 * high_index * low_pairs;
            for (low_index, (&weight, &negative_weight)) in
                low.iter().zip(&negative_low).enumerate()
            {
                let index = start + 2 * low_index;
                accumulate_native_cofactor_raw(
                    ctx,
                    &mut inner,
                    weight,
                    negative_weight,
                    endpoint,
                    az[index],
                    az[index + 1],
                    bz[index],
                    bz[index + 1],
                    cz[index],
                    cz[index + 1],
                );
            }
            let inner = reduce_linear_pair(inner, reducer);
            let high_weight = weights.high[high_index];
            let mut outer = product_pair();
            outer[0].accumulate_encoded(ctx, high_weight, inner[0]);
            outer[1].accumulate_encoded(ctx, high_weight, inner[1]);
            outer
        };
        #[cfg(feature = "parallel")]
        if parallel(pair_count) {
            let total = (0..weights.high.len())
                .into_par_iter()
                .map(bucket)
                .reduce(product_pair, merge_product_pair);
            return reduce_product_pair(total, reducer);
        }
        let total = (0..weights.high.len())
            .map(bucket)
            .fold(product_pair(), merge_product_pair);
        return reduce_product_pair(total, reducer);
    }

    let block = |start: usize, end: usize| -> LinearPair {
        let mut accumulators = linear_pair();
        for pair in start..end {
            let index = 2 * pair;
            let weight = weights.pair_weight(ctx, pair);
            accumulate_native_cofactor_raw(
                ctx,
                &mut accumulators,
                weight,
                ctx.neg_raw(weight),
                endpoint,
                az[index],
                az[index + 1],
                bz[index],
                bz[index + 1],
                cz[index],
                cz[index + 1],
            );
        }
        accumulators
    };
    let merge = |mut left: LinearPair, right: LinearPair| -> LinearPair {
        left[0] += right[0];
        left[1] += right[1];
        left
    };
    #[cfg(feature = "parallel")]
    if parallel(pair_count) {
        let blocks = pair_count.div_ceil(FOLD_BLOCK);
        let total = (0..blocks)
            .into_par_iter()
            .map(|b| block(b * FOLD_BLOCK, ((b + 1) * FOLD_BLOCK).min(pair_count)))
            .reduce(linear_pair, merge);
        return reduce_linear_pair(total, reducer);
    }
    reduce_linear_pair(block(0, pair_count), reducer)
}

/// Raw slices of one product table triple, for disjoint parallel blocks.
struct ProductSlices<'a> {
    az: &'a [Raw],
    bz: &'a [Raw],
    cz: &'a [Raw],
}

struct ProductSlicesMut<'a> {
    az: &'a mut [Raw],
    bz: &'a mut [Raw],
    cz: &'a mut [Raw],
}

/// Folds every table at `challenge` (no accumulation): the last round.
fn fold_products_raw(
    ctx: &field::FpCtx<2>,
    input: &RawProducts,
    output: &mut RawProducts,
    challenge: Raw,
) {
    debug_assert_eq!(input.len(), 2 * output.len());
    let fold = |table_in: &[Raw], table_out: &mut [Raw]| {
        #[cfg(feature = "parallel")]
        if parallel(table_out.len()) {
            table_in
                .par_chunks_exact(2)
                .zip(table_out.par_iter_mut())
                .for_each(|(pair, value)| *value = ctx.interpolate(pair[0], pair[1], challenge));
            return;
        }
        for (pair, value) in table_in.chunks_exact(2).zip(table_out.iter_mut()) {
            *value = ctx.interpolate(pair[0], pair[1], challenge);
        }
    };
    fold(&input.az, &mut output.az);
    fold(&input.bz, &mut output.bz);
    fold(&input.cz, &mut output.cz);
}

/// Folds the tables at `challenge` and accumulates the next round's cofactor
/// evaluations from the folded pairs in the same pass.
fn fold_products_and_cofactor_evaluations_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    input: &RawProducts,
    output: &mut RawProducts,
    challenge: Raw,
    weights: RawEqWeights<'_>,
    endpoint: FactoredEndpoint,
) -> [Raw; 2] {
    debug_assert_eq!(input.len(), 2 * output.len());
    let out_pairs = output.len() / 2;
    debug_assert_eq!(out_pairs, weights.pair_count());

    // One contiguous output pair range → its input window is four times as
    // long. `first_pair` indexes the equality weights.
    let process = |first_pair: usize,
                   input: ProductSlices<'_>,
                   output: ProductSlicesMut<'_>|
     -> ProductPair {
        let pairs = output.az.len() / 2;
        let mut accumulators = product_pair();
        if let Some(low) = weights.two_level_low() {
            // The block is a whole number of high buckets (see the callers).
            let low_pairs = low.len();
            debug_assert_eq!(pairs % low_pairs, 0);
            debug_assert_eq!(first_pair % low_pairs, 0);
            let mut high_index = first_pair / low_pairs;
            let mut base = 0;
            while base < pairs {
                let mut inner = product_pair();
                for (low_index, &weight) in low.iter().enumerate() {
                    let pair = base + low_index;
                    let (i, o) = (4 * pair, 2 * pair);
                    let az = [
                        ctx.interpolate(input.az[i], input.az[i + 1], challenge),
                        ctx.interpolate(input.az[i + 2], input.az[i + 3], challenge),
                    ];
                    let bz = [
                        ctx.interpolate(input.bz[i], input.bz[i + 1], challenge),
                        ctx.interpolate(input.bz[i + 2], input.bz[i + 3], challenge),
                    ];
                    let cz = [
                        ctx.interpolate(input.cz[i], input.cz[i + 1], challenge),
                        ctx.interpolate(input.cz[i + 2], input.cz[i + 3], challenge),
                    ];
                    output.az[o] = az[0];
                    output.az[o + 1] = az[1];
                    output.bz[o] = bz[0];
                    output.bz[o + 1] = bz[1];
                    output.cz[o] = cz[0];
                    output.cz[o + 1] = cz[1];
                    accumulate_cofactor_raw(
                        ctx, &mut inner, weight, endpoint, az[0], az[1], bz[0], bz[1], cz[0], cz[1],
                    );
                }
                let inner = reduce_product_pair(inner, reducer);
                let high_weight = weights.high[high_index];
                accumulators[0].accumulate_encoded(ctx, high_weight, inner[0]);
                accumulators[1].accumulate_encoded(ctx, high_weight, inner[1]);
                high_index += 1;
                base += low_pairs;
            }
        } else {
            for pair in 0..pairs {
                let (i, o) = (4 * pair, 2 * pair);
                let az = [
                    ctx.interpolate(input.az[i], input.az[i + 1], challenge),
                    ctx.interpolate(input.az[i + 2], input.az[i + 3], challenge),
                ];
                let bz = [
                    ctx.interpolate(input.bz[i], input.bz[i + 1], challenge),
                    ctx.interpolate(input.bz[i + 2], input.bz[i + 3], challenge),
                ];
                let cz = [
                    ctx.interpolate(input.cz[i], input.cz[i + 1], challenge),
                    ctx.interpolate(input.cz[i + 2], input.cz[i + 3], challenge),
                ];
                output.az[o] = az[0];
                output.az[o + 1] = az[1];
                output.bz[o] = bz[0];
                output.bz[o + 1] = bz[1];
                output.cz[o] = cz[0];
                output.cz[o + 1] = cz[1];
                let weight = weights.pair_weight(ctx, first_pair + pair);
                accumulate_cofactor_raw(
                    ctx,
                    &mut accumulators,
                    weight,
                    endpoint,
                    az[0],
                    az[1],
                    bz[0],
                    bz[1],
                    cz[0],
                    cz[1],
                );
            }
        }
        accumulators
    };

    // Block size in output pairs: a multiple of the low pair count so every
    // block covers whole high buckets.
    let block_pairs = match weights.two_level_low() {
        Some(low) => {
            let low_pairs = low.len();
            (FOLD_BLOCK / 2).div_ceil(low_pairs).max(1) * low_pairs
        }
        None => FOLD_BLOCK / 2,
    };

    #[cfg(feature = "parallel")]
    if parallel(out_pairs) {
        let out_block = 2 * block_pairs;
        let in_block = 4 * block_pairs;
        let total = (
            input.az.par_chunks(in_block),
            input.bz.par_chunks(in_block),
            input.cz.par_chunks(in_block),
            output.az.par_chunks_mut(out_block),
            output.bz.par_chunks_mut(out_block),
            output.cz.par_chunks_mut(out_block),
        )
            .into_par_iter()
            .enumerate()
            .map(|(block, (az, bz, cz, az_out, bz_out, cz_out))| {
                process(
                    block * block_pairs,
                    ProductSlices { az, bz, cz },
                    ProductSlicesMut {
                        az: az_out,
                        bz: bz_out,
                        cz: cz_out,
                    },
                )
            })
            .reduce(product_pair, merge_product_pair);
        return reduce_product_pair(total, reducer);
    }

    let total = process(
        0,
        ProductSlices {
            az: &input.az,
            bz: &input.bz,
            cz: &input.cz,
        },
        ProductSlicesMut {
            az: &mut output.az,
            bz: &mut output.bz,
            cz: &mut output.cz,
        },
    );
    reduce_product_pair(total, reducer)
}

/// Folds the exact native tables into the field at `challenge` — every output
/// is `(1 - challenge) · v0 + challenge · v1`, Montgomery-reduced once and
/// converted to raw form — and accumulates the next round's cofactor
/// evaluations in the same pass.
fn fold_native_products_and_cofactor_evaluations_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    products: NativeProducts<'_>,
    output: &mut RawProducts,
    challenge: Raw,
    weights: Option<(RawEqWeights<'_>, FactoredEndpoint)>,
) -> [Raw; 2] {
    let (az_in, bz_in, cz_in) = (products.az, products.bz, products.cz);
    debug_assert_eq!(az_in.len(), 2 * output.len());
    let one_minus_challenge = ctx.sub_raw(ctx.one_raw(), challenge);
    // Plain Montgomery reduction plus one conversion multiply: cheaper than
    // the Barrett remainder of the R-scaled sum, same canonical residue.
    let fold = |v0: u64, v1: u64| -> Raw {
        ctx.plain_to_raw(fold_native_pair_plain(
            ctx,
            one_minus_challenge,
            challenge,
            v0,
            v1,
        ))
    };

    let Some((weights, endpoint)) = weights else {
        // Final fold: a single output entry per table, no next round.
        debug_assert_eq!(output.len(), 1);
        output.az[0] = fold(az_in[0], az_in[1]);
        output.bz[0] = fold(bz_in[0], bz_in[1]);
        output.cz[0] = fold(cz_in[0], cz_in[1]);
        return [0, 0];
    };

    let out_pairs = output.len() / 2;
    debug_assert_eq!(out_pairs, weights.pair_count());
    let process = |first_pair: usize,
                   az_in: &[u64],
                   bz_in: &[u64],
                   cz_in: &[u64],
                   az_out: &mut [Raw],
                   bz_out: &mut [Raw],
                   cz_out: &mut [Raw]|
     -> ProductPair {
        let pairs = az_out.len() / 2;
        let mut accumulators = product_pair();
        let mut emit = |pair: usize| -> [[Raw; 2]; 3] {
            let (i, o) = (4 * pair, 2 * pair);
            let az = [
                fold(az_in[i], az_in[i + 1]),
                fold(az_in[i + 2], az_in[i + 3]),
            ];
            let bz = [
                fold(bz_in[i], bz_in[i + 1]),
                fold(bz_in[i + 2], bz_in[i + 3]),
            ];
            let cz = [
                fold(cz_in[i], cz_in[i + 1]),
                fold(cz_in[i + 2], cz_in[i + 3]),
            ];
            az_out[o] = az[0];
            az_out[o + 1] = az[1];
            bz_out[o] = bz[0];
            bz_out[o + 1] = bz[1];
            cz_out[o] = cz[0];
            cz_out[o + 1] = cz[1];
            [az, bz, cz]
        };
        if let Some(low) = weights.two_level_low() {
            let low_pairs = low.len();
            debug_assert_eq!(pairs % low_pairs, 0);
            debug_assert_eq!(first_pair % low_pairs, 0);
            let mut high_index = first_pair / low_pairs;
            let mut base = 0;
            while base < pairs {
                let mut inner = product_pair();
                for (low_index, &weight) in low.iter().enumerate() {
                    let [az, bz, cz] = emit(base + low_index);
                    accumulate_cofactor_raw(
                        ctx, &mut inner, weight, endpoint, az[0], az[1], bz[0], bz[1], cz[0], cz[1],
                    );
                }
                let inner = reduce_product_pair(inner, reducer);
                let high_weight = weights.high[high_index];
                accumulators[0].accumulate_encoded(ctx, high_weight, inner[0]);
                accumulators[1].accumulate_encoded(ctx, high_weight, inner[1]);
                high_index += 1;
                base += low_pairs;
            }
        } else {
            for pair in 0..pairs {
                let [az, bz, cz] = emit(pair);
                let weight = weights.pair_weight(ctx, first_pair + pair);
                accumulate_cofactor_raw(
                    ctx,
                    &mut accumulators,
                    weight,
                    endpoint,
                    az[0],
                    az[1],
                    bz[0],
                    bz[1],
                    cz[0],
                    cz[1],
                );
            }
        }
        accumulators
    };

    let block_pairs = match weights.two_level_low() {
        Some(low) => {
            let low_pairs = low.len();
            (FOLD_BLOCK / 2).div_ceil(low_pairs).max(1) * low_pairs
        }
        None => FOLD_BLOCK / 2,
    };

    #[cfg(feature = "parallel")]
    if parallel(out_pairs) {
        let out_block = 2 * block_pairs;
        let in_block = 4 * block_pairs;
        let total = (
            az_in.par_chunks(in_block),
            bz_in.par_chunks(in_block),
            cz_in.par_chunks(in_block),
            output.az.par_chunks_mut(out_block),
            output.bz.par_chunks_mut(out_block),
            output.cz.par_chunks_mut(out_block),
        )
            .into_par_iter()
            .enumerate()
            .map(|(block, (az, bz, cz, az_out, bz_out, cz_out))| {
                process(block * block_pairs, az, bz, cz, az_out, bz_out, cz_out)
            })
            .reduce(product_pair, merge_product_pair);
        return reduce_product_pair(total, reducer);
    }

    let total = process(
        0,
        az_in,
        bz_in,
        cz_in,
        &mut output.az,
        &mut output.bz,
        &mut output.cz,
    );
    reduce_product_pair(total, reducer)
}

/// Shared prover-side scalars of one outer sumcheck.
struct OuterScalars<'a> {
    config: FieldConfig,
    ctx: &'a field::FpCtx<2>,
    reducer: &'a field::FpCtx<2>,
    tau: &'a [Field],
    tau_inverses: Vec<Field>,
    zero: Field,
    one: Field,
}

impl OuterScalars<'_> {
    fn coefficients(
        &self,
        round: usize,
        current_claim: &Field,
        endpoint: FactoredEndpoint,
        evaluations: [Raw; 2],
        bound_equality: &Field,
    ) -> [Field; 3] {
        reconstruct_eq_factored_cubic_without_linear(
            current_claim,
            &self.tau[round],
            &self.tau_inverses[round],
            endpoint,
            [
                crate::utils::delayed_reduction::element(&self.config, evaluations[0]),
                crate::utils::delayed_reduction::element(&self.config, evaluations[1]),
            ],
            bound_equality,
            &self.one,
            &self.config,
        )
    }
}

/// Removes the active coordinate from the equality factors: the low table
/// while it has more than one entry, then the high table.
fn strip_active_coordinate(ctx: &field::FpCtx<2>, eq_low: &mut Vec<Raw>, eq_high: &mut Vec<Raw>) {
    if eq_low.len() > 1 {
        *eq_low = strip_coordinate_raw(ctx, eq_low);
    } else {
        *eq_high = strip_coordinate_raw(ctx, eq_high);
    }
}

fn weights_of<'a>(eq_low: &'a [Raw], eq_high: &'a [Raw]) -> RawEqWeights<'a> {
    RawEqWeights {
        low: (!eq_low.is_empty() && eq_low.len() > 1).then_some(eq_low),
        high: eq_high,
    }
}

/// Runs the remaining field-valued outer rounds from a prepared state: the
/// equality factors already stripped of the current coordinate (they are the
/// current round's weights) and the current round's coefficients computed.
/// `round_boundary` acts between each absorbed round polynomial and its
/// challenge ([`UngrindedRoundBoundary`] adds no transcript bytes).
#[allow(clippy::too_many_arguments)]
fn outer_rounds_raw<T: Transcript, P: RoundBoundaryPolicy>(
    transcript: &mut T,
    scalars: &OuterScalars<'_>,
    mut current_claim: Field,
    mut bound_equality: Field,
    mut eq_low: Vec<Raw>,
    mut eq_high: Vec<Raw>,
    mut products: RawProducts,
    mut coefficients_without_linear: [Field; 3],
    round_polynomials: &mut Vec<[Field; 4]>,
    eval_points: &mut Vec<Field>,
    round_boundary: &mut P,
) -> Result<(Field, Field, RawProducts), SumcheckError> {
    let ctx = scalars.ctx;
    let mut scratch = RawProducts::zeros(products.len() / 2);
    while products.len() > 1 {
        let round = eval_points.len();
        let challenge = recover_full_round_polynomial_and_sample_next_challenge_with_boundary(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            round_polynomials,
            eval_points,
            &scalars.zero,
            &scalars.config,
            round_boundary,
        )?;
        let bound_factor =
            equality_coordinate_evaluation(&scalars.tau[round], &challenge, &scalars.one, &ctx);
        bound_equality = ctx.mul(&(bound_equality), &(&bound_factor));
        let challenge_raw = ctx.raw(&challenge);

        let next_len = products.len() / 2;
        scratch.truncate(next_len);
        if next_len == 1 {
            fold_products_raw(ctx, &products, &mut scratch, challenge_raw);
            products.swap(&mut scratch);
            break;
        }

        strip_active_coordinate(ctx, &mut eq_low, &mut eq_high);
        let next_round = round + 1;
        let endpoint = FactoredEndpoint::for_tau(&scalars.tau[next_round]);
        let evaluations = fold_products_and_cofactor_evaluations_raw(
            ctx,
            scalars.reducer,
            &products,
            &mut scratch,
            challenge_raw,
            weights_of(&eq_low, &eq_high),
            endpoint,
        );
        coefficients_without_linear = scalars.coefficients(
            next_round,
            &current_claim,
            endpoint,
            evaluations,
            &bound_equality,
        );
        products.swap(&mut scratch);
    }
    Ok((current_claim, bound_equality, products))
}

/// Absorbs the terminal evaluations and assembles the outer output. The
/// field-valued prover rejects an inconsistent terminal claim (as the generic
/// field prover does); the native prover only debug-asserts it (as the generic
/// native prover does).
#[allow(clippy::too_many_arguments)]
fn finish_outer<T: Transcript>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    current_claim: Field,
    bound_equality: &Field,
    products: &RawProducts,
    round_polynomials: Vec<[Field; 4]>,
    eval_points: Vec<Field>,
    reject_inconsistent_terminal: bool,
) -> Result<OuterSumcheckOutput<Field>, SumcheckError> {
    let cfg = ctx.clone();
    debug_assert_eq!(products.len(), 1);
    let az_mle_claim = crate::utils::delayed_reduction::element(&cfg, products.az[0]);
    let bz_mle_claim = crate::utils::delayed_reduction::element(&cfg, products.bz[0]);
    let cz_mle_claim = crate::utils::delayed_reduction::element(&cfg, products.cz[0]);
    let expected = cfg.mul(
        &(bound_equality.clone()),
        &(&(cfg.sub(
            &(cfg.mul(&(az_mle_claim.clone()), &(&bz_mle_claim))),
            &(&cz_mle_claim),
        ))),
    );
    if reject_inconsistent_terminal {
        if current_claim != expected {
            return Err(SumcheckError::InvalidTerminalClaim);
        }
    } else {
        debug_assert_eq!(current_claim, expected);
    }
    absorb_field_elements(
        transcript,
        &[
            az_mle_claim.clone(),
            bz_mle_claim.clone(),
            cz_mle_claim.clone(),
        ],
        &cfg,
    );
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

fn outer_scalars<'a>(
    ctx: &'a field::FpCtx<2>,
    reducer: &'a field::FpCtx<2>,
    tau: &'a [Field],
    cfg: FieldConfig,
) -> OuterScalars<'a> {
    OuterScalars {
        config: cfg.clone(),
        ctx,
        reducer,
        tau,
        tau_inverses: batch_invert_nonzero(tau, &cfg),
        zero: Field::zero_with_cfg(&cfg),
        one: Field::one_with_cfg(&cfg),
    }
}

/// Proves the cubic outer sumcheck from field-valued raw product tables: the
/// raw twin of `prove_outer_sumcheck_with_reducer` with the delayed-Barrett
/// reducer. `eq_low`/`eq_high` are the full equality factors of `tau`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_outer_field_raw<T: Transcript>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    initial_claim: Field,
    tau: &[Field],
    eq_low: Vec<Raw>,
    eq_high: Vec<Raw>,
    products: RawProducts,
) -> Result<OuterSumcheckOutput<Field>, SumcheckError> {
    prove_outer_field_raw_with_boundary(
        transcript,
        ctx,
        reducer,
        initial_claim,
        tau,
        eq_low,
        eq_high,
        products,
        &mut UngrindedRoundBoundary,
    )
}

/// [`prove_outer_field_raw`] under an explicit message/challenge round
/// boundary policy: the raw twin of
/// `prove_outer_sumcheck_with_reducer_grinded`, transcript-identical to it
/// under the same policy.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_outer_field_raw_with_boundary<T: Transcript, P: RoundBoundaryPolicy>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    initial_claim: Field,
    tau: &[Field],
    mut eq_low: Vec<Raw>,
    mut eq_high: Vec<Raw>,
    products: RawProducts,
    round_boundary: &mut P,
) -> Result<OuterSumcheckOutput<Field>, SumcheckError> {
    let num_vars = tau.len();
    if products.len() != 1usize << num_vars || eq_low.len() * eq_high.len() != products.len() {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    round_boundary.validate(num_vars)?;
    let scalars = outer_scalars(ctx, reducer, tau, ctx.clone());
    let bound_equality = scalars.one.clone();
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut eval_points = Vec::with_capacity(num_vars);
    if num_vars == 0 {
        return finish_outer(
            transcript,
            ctx,
            initial_claim,
            &bound_equality,
            &products,
            round_polynomials,
            eval_points,
            true,
        );
    }

    strip_active_coordinate(ctx, &mut eq_low, &mut eq_high);
    let endpoint = FactoredEndpoint::for_tau(&tau[0]);
    let evaluations = {
        let _scope = tracing::info_span!("raw:outer_round0").entered();
        cofactor_evaluations_raw(
            ctx,
            reducer,
            &products,
            weights_of(&eq_low, &eq_high),
            endpoint,
        )
    };
    let coefficients =
        scalars.coefficients(0, &initial_claim, endpoint, evaluations, &bound_equality);
    let _rounds_scope = tracing::info_span!("raw:outer_rounds").entered();
    let (current_claim, bound_equality, products) = outer_rounds_raw(
        transcript,
        &scalars,
        initial_claim,
        bound_equality,
        eq_low,
        eq_high,
        products,
        coefficients,
        &mut round_polynomials,
        &mut eval_points,
        round_boundary,
    )?;
    finish_outer(
        transcript,
        ctx,
        current_claim,
        &bound_equality,
        &products,
        round_polynomials,
        eval_points,
        true,
    )
}

/// Proves the outer sumcheck whose first round runs on exact native `u64`
/// products: the raw twin of `prove_outer_sumcheck_u32_native_with_reducer`.
/// `Az`/`Bz` must already be validated 32-bit wide.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_outer_native_raw<T: Transcript, P: NativeOuterInput>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    initial_claim: Field,
    tau: &[Field],
    mut eq_low: Vec<Raw>,
    mut eq_high: Vec<Raw>,
    products: P,
) -> Result<OuterSumcheckOutput<Field>, SumcheckError> {
    let num_vars = tau.len();
    let len = products.len();
    if len != 1usize << num_vars || !products.valid_shape() || eq_low.len() * eq_high.len() != len {
        return Err(SumcheckError::InvalidEqualityDimensions);
    }
    let scalars = outer_scalars(ctx, reducer, tau, ctx.clone());
    let mut bound_equality = scalars.one.clone();
    let mut round_polynomials = Vec::with_capacity(num_vars);
    let mut eval_points = Vec::with_capacity(num_vars);
    if num_vars == 0 {
        let [a, b, c] = products.singleton(ctx);
        let single = RawProducts {
            az: vec![a],
            bz: vec![b],
            cz: vec![c],
        };
        return finish_outer(
            transcript,
            ctx,
            initial_claim,
            &bound_equality,
            &single,
            round_polynomials,
            eval_points,
            false,
        );
    }

    // Round zero on the exact tables.
    strip_active_coordinate(ctx, &mut eq_low, &mut eq_high);
    let endpoint = FactoredEndpoint::for_tau(&tau[0]);
    let evaluations = {
        let _scope = tracing::info_span!("raw:outer_native_round0").entered();
        products.round0(ctx, reducer, weights_of(&eq_low, &eq_high), endpoint)
    };
    let coefficients =
        scalars.coefficients(0, &initial_claim, endpoint, evaluations, &bound_equality);
    let mut current_claim = initial_claim;
    let challenge = recover_full_round_polynomial_and_sample_next_challenge(
        transcript,
        &mut current_claim,
        &coefficients,
        &mut round_polynomials,
        &mut eval_points,
        &scalars.zero,
        &scalars.config,
    )?;
    bound_equality = ctx.mul(
        &(bound_equality),
        &(&equality_coordinate_evaluation(&tau[0], &challenge, &scalars.one, ctx)),
    );
    let challenge_raw = ctx.raw(&challenge);

    // Fold into the field, preparing round one in the same pass.
    let mut folded = RawProducts::zeros(len / 2);
    if len == 2 {
        products.fold(ctx, reducer, &mut folded, challenge_raw, None);
        return finish_outer(
            transcript,
            ctx,
            current_claim,
            &bound_equality,
            &folded,
            round_polynomials,
            eval_points,
            false,
        );
    }
    strip_active_coordinate(ctx, &mut eq_low, &mut eq_high);
    let endpoint = FactoredEndpoint::for_tau(&tau[1]);
    let evaluations = {
        let _scope = tracing::info_span!("raw:outer_native_fold0").entered();
        products.fold(
            ctx,
            reducer,
            &mut folded,
            challenge_raw,
            Some((weights_of(&eq_low, &eq_high), endpoint)),
        )
    };
    let coefficients =
        scalars.coefficients(1, &current_claim, endpoint, evaluations, &bound_equality);
    let _rounds_scope = tracing::info_span!("raw:outer_rounds").entered();
    let (current_claim, bound_equality, products) = outer_rounds_raw(
        transcript,
        &scalars,
        current_claim,
        bound_equality,
        eq_low,
        eq_high,
        folded,
        coefficients,
        &mut round_polynomials,
        &mut eval_points,
        &mut UngrindedRoundBoundary,
    )?;
    finish_outer(
        transcript,
        ctx,
        current_claim,
        &bound_equality,
        &products,
        round_polynomials,
        eval_points,
        false,
    )
}

// ---------------------------------------------------------------------------
// Inner sumcheck
// ---------------------------------------------------------------------------

/// The assignment table entering the inner sumcheck.
pub enum RawWitness<'a> {
    /// Exact native values (the u32 and BabyBear relations): the first round
    /// accumulates field × `u64` products and its fold projects into the
    /// field. `values` holds the leading entries of a `domain`-length table
    /// whose remainder is zero, so a relation's logical assignment can be
    /// borrowed without padding it.
    Native {
        values: Cow<'a, [u64]>,
        domain: usize,
    },
    /// Borrowed declared-width x/y/u256 product segments.
    Wide(NativeU128Witness<'a>),
    /// Borrowed 32-limb witness and quotient blocks.
    Limbs(NativeLimbWitness<'a>),
    /// Raw field residues.
    Field(Vec<Raw>),
}

impl<'a> RawWitness<'a> {
    /// A complete native table.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn native_owned(values: Vec<u64>) -> Self {
        let domain = values.len();
        Self::Native {
            values: Cow::Owned(values),
            domain,
        }
    }

    /// The leading `values` of a `domain`-length native table (the rest is
    /// zero).
    pub(crate) fn native_borrowed(values: &'a [u64], domain: usize) -> Self {
        debug_assert!(values.len() <= domain);
        Self::Native {
            values: Cow::Borrowed(values),
            domain,
        }
    }

    /// The (padded) table length.
    fn len(&self) -> usize {
        match self {
            Self::Native { domain, .. } => *domain,
            Self::Field(values) => values.len(),
            Self::Wide(values) => values.len(),
            Self::Limbs(values) => values.len(),
        }
    }

    /// Pads a borrowed native table out to its domain (a copy only when the
    /// caller could not lend the whole table).
    fn materialize(self) -> Self {
        match self {
            Self::Native { mut values, domain } if values.len() < domain => {
                values.to_mut().resize(domain, 0);
                Self::Native { values, domain }
            }
            other => other,
        }
    }
}

#[inline(always)]
fn fold_native_pair(
    reducer: &field::FpCtx<2>,
    one_minus_challenge: Raw,
    challenge: Raw,
    at_zero: u64,
    at_one: u64,
) -> Raw {
    let mut accumulator = field::FpLinearAcc::<2, 1>::default();
    accumulator.accumulate_encoded(reducer, one_minus_challenge, at_zero);
    accumulator.accumulate_encoded(reducer, challenge, at_one);
    accumulator.reduce_encoded(reducer)
}

/// `(1 - c) · w0 + c · w1` as a PLAIN residue (scale one) from the raw
/// challenge and exact native values: the `R`-scaled sum
/// `(1-c)_raw · w0 + c_raw · w1 < 2 · 2^64 · q` is Montgomery-reduced once,
/// which is far cheaper than the Barrett remainder the raw form needs.
#[inline(always)]
fn fold_native_pair_plain(
    ctx: &field::FpCtx<2>,
    one_minus_challenge: Raw,
    challenge: Raw,
    at_zero: u64,
    at_one: u64,
) -> Raw {
    let coefficients = [
        crate::utils::delayed_reduction::element(ctx, one_minus_challenge),
        crate::utils::delayed_reduction::element(ctx, challenge),
    ];
    let values = [
        field::Uint::from_words([at_zero]),
        field::Uint::from_words([at_one]),
    ];
    u128::from(ctx.weighted_pair_to_integer(&coefficients, &values))
}

/// Round-zero `[c0, c2]` over a field witness: `Σ m0·w0` and
/// `Σ (m1 − m0)(w1 − w0)` over adjacent pairs.
fn inner_coefficients_field_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    matrix: &[Raw],
    witness: &[Raw],
) -> [Raw; 2] {
    debug_assert_eq!(matrix.len(), witness.len());
    debug_assert_eq!(matrix.len() % 2, 0);
    let block = |matrix: &[Raw], witness: &[Raw]| -> ProductPair {
        let mut accumulators = product_pair();
        for (m, w) in matrix.chunks_exact(2).zip(witness.chunks_exact(2)) {
            accumulators[0].accumulate_encoded(ctx, m[0], w[0]);
            accumulators[1].accumulate_encoded(
                ctx,
                ctx.sub_raw(m[1], m[0]),
                ctx.sub_raw(w[1], w[0]),
            );
        }
        accumulators
    };
    #[cfg(feature = "parallel")]
    if parallel(matrix.len() / 2) {
        let total = matrix
            .par_chunks(2 * FOLD_BLOCK)
            .zip(witness.par_chunks(2 * FOLD_BLOCK))
            .map(|(m, w)| block(m, w))
            .reduce(product_pair, merge_product_pair);
        return reduce_product_pair(total, reducer);
    }
    reduce_product_pair(block(matrix, witness), reducer)
}

/// Round-zero `[c0, c2]` over an exact native witness.
fn inner_coefficients_native_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    matrix: &[Raw],
    witness: &[u64],
) -> [Raw; 2] {
    debug_assert_eq!(matrix.len(), witness.len());
    debug_assert_eq!(matrix.len() % 2, 0);
    let block = |matrix: &[Raw], witness: &[u64]| -> LinearPair {
        let mut accumulators = linear_pair();
        for (m, w) in matrix.chunks_exact(2).zip(witness.chunks_exact(2)) {
            accumulators[0].accumulate_encoded(ctx, m[0], w[0]);
            // (m1 - m0)(w1 - w0) as (±(m1 - m0)) · |w1 - w0|: one product,
            // congruent modulo q to the two-product form.
            let delta = ctx.sub_raw(m[1], m[0]);
            let mask = 0u64.wrapping_sub(u64::from(w[1] < w[0]));
            let magnitude = (w[1].wrapping_sub(w[0]) ^ mask).wrapping_sub(mask);
            let mask128 = (mask as u128) | ((mask as u128) << 64);
            let signed = (delta & !mask128) | (ctx.neg_raw(delta) & mask128);
            accumulators[1].accumulate_encoded(ctx, signed, magnitude);
        }
        accumulators
    };
    let merge = |mut left: LinearPair, right: LinearPair| -> LinearPair {
        left[0] += right[0];
        left[1] += right[1];
        left
    };
    #[cfg(feature = "parallel")]
    if parallel(matrix.len() / 2) {
        let total = matrix
            .par_chunks(2 * FOLD_BLOCK)
            .zip(witness.par_chunks(2 * FOLD_BLOCK))
            .map(|(m, w)| block(m, w))
            .reduce(linear_pair, merge);
        return reduce_linear_pair(total, reducer);
    }
    reduce_linear_pair(block(matrix, witness), reducer)
}

/// First native fold with canonical integer output and typed linear MAC.
fn fold_inner_native_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    matrix_in: &[Raw],
    witness_in: &[u64],
    matrix_out: &mut [Raw],
    witness_out: &mut [field::Uint<2>],
    challenge: Raw,
) -> [Raw; 2] {
    debug_assert_eq!(matrix_in.len(), 2 * matrix_out.len());
    debug_assert_eq!(witness_in.len(), 2 * witness_out.len());
    debug_assert_eq!(matrix_out.len(), witness_out.len());
    debug_assert_eq!(matrix_out.len() % 2, 0);
    let one_minus_challenge = ctx.sub_raw(ctx.one_raw(), challenge);
    let block = |matrix_in: &[Raw],
                 witness_in: &[u64],
                 matrix_out: &mut [Raw],
                 witness_out: &mut [field::Uint<2>]|
     -> [field::FpLinearAcc<2, 2>; 2] {
        let mut accumulators = folded::pair::<field::Uint<2>>();
        for (((m, w), m_out), w_out) in matrix_in
            .chunks_exact(4)
            .zip(witness_in.chunks_exact(4))
            .zip(matrix_out.chunks_exact_mut(2))
            .zip(witness_out.chunks_exact_mut(2))
        {
            let m0 = ctx.interpolate(m[0], m[1], challenge);
            let m1 = ctx.interpolate(m[2], m[3], challenge);
            let w0 = fold_native_pair_plain(ctx, one_minus_challenge, challenge, w[0], w[1]);
            let w1 = fold_native_pair_plain(ctx, one_minus_challenge, challenge, w[2], w[3]);
            m_out[0] = m0;
            m_out[1] = m1;
            w_out[0] = field::Uint::from_words(raw_to_words(w0));
            w_out[1] = field::Uint::from_words(raw_to_words(w1));
            <field::Uint<2> as FoldedValue>::accumulate(
                ctx,
                &mut accumulators[0],
                m0,
                field::Uint::from_words(raw_to_words(w0)),
            );
            <field::Uint<2> as FoldedValue>::accumulate(
                ctx,
                &mut accumulators[1],
                ctx.sub_raw(m1, m0),
                field::Uint::from_words(raw_to_words(ctx.sub_raw(w1, w0))),
            );
        }
        accumulators
    };
    #[cfg(feature = "parallel")]
    if parallel(matrix_out.len() / 2) {
        let total = (
            matrix_in.par_chunks(2 * FOLD_BLOCK),
            witness_in.par_chunks(2 * FOLD_BLOCK),
            matrix_out.par_chunks_mut(FOLD_BLOCK),
            witness_out.par_chunks_mut(FOLD_BLOCK),
        )
            .into_par_iter()
            .map(|(m, w, m_out, w_out)| block(m, w, m_out, w_out))
            .reduce(
                folded::pair::<field::Uint<2>>,
                folded::merge::<field::Uint<2>>,
            );
        return folded::reduce::<field::Uint<2>>(ctx, total);
    }
    folded::reduce::<field::Uint<2>>(ctx, block(matrix_in, witness_in, matrix_out, witness_out))
}

/// Proves the quadratic inner sumcheck `initial_claim = Σ_y matrix(y) · witness(y)`
/// on raw tables: the raw twin of `prove_inner_sumcheck_with_reducer` (field
/// witness) and `prove_inner_sumcheck_u32_native_with_reducer` (native
/// witness) with the delayed-Barrett reducer.
///
/// `live` bounds the leading entries that may be nonzero in BOTH tables: the
/// batched matrix is zero beyond the logical column count by construction and
/// the assignment padding is validated zero before the transcript moves.
/// Rounds therefore touch only the live prefix; the values are exactly those
/// of the full-table prover, whose padded pairs contribute zero.
pub(crate) fn prove_inner_raw<T: Transcript>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    initial_claim: Field,
    matrix: Vec<Raw>,
    witness: RawWitness<'_>,
    live: usize,
) -> Result<InnerSumcheckOutput<Field>, SumcheckError> {
    let cfg = ctx.clone();
    let len = matrix.len();
    if !len.is_power_of_two() || witness.len() != len || live > len {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    if let RawWitness::Native { values, domain } = &witness
        && values.len() > *domain
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    let witness = witness.materialize();
    let num_vars = len.trailing_zeros() as usize;
    let zero = Field::zero_with_cfg(&cfg);
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_vars);
    let mut round_polynomials = Vec::with_capacity(num_vars);

    let finish = |matrix_evaluation: Raw,
                  witness_evaluation: Raw,
                  current_claim: Field,
                  round_polynomials: Vec<[Field; 3]>,
                  eval_points: Vec<Field>| {
        let batched_matrix_evaluation =
            crate::utils::delayed_reduction::element(&cfg, matrix_evaluation);
        let witness_evaluation = crate::utils::delayed_reduction::element(&cfg, witness_evaluation);
        debug_assert_eq!(
            current_claim,
            cfg.mul(&(batched_matrix_evaluation.clone()), &(&witness_evaluation))
        );
        Ok(InnerSumcheckOutput {
            sumcheck: SumcheckProverOutput {
                proof: SumcheckProof { round_polynomials },
                eval_points,
                final_claim: current_claim,
            },
            batched_matrix_evaluation,
            witness_evaluation,
        })
    };

    if num_vars == 0 {
        let witness_evaluation = match &witness {
            RawWitness::Native { values, .. } => ctx.native_residue(values[0]),
            RawWitness::Field(values) => values[0],
            RawWitness::Wide(values) => {
                raw_shared(field::IntegerEmbedding::from_integer(ctx, &values.read(0)))
            }
            RawWitness::Limbs(values) => {
                raw_shared(field::IntegerEmbedding::from_integer(ctx, &values.read(0)))
            }
        };
        return finish(
            matrix[0],
            witness_evaluation,
            current_claim,
            round_polynomials,
            eval_points,
        );
    }

    // Round zero over the live prefix, then the first fold (which is also the
    // native → field projection when the witness is exact).
    let pairs = live.div_ceil(2);
    let round0_scope = tracing::info_span!("raw:inner_round0").entered();
    let coefficients = match &witness {
        RawWitness::Native { values, .. } => {
            inner_coefficients_native_raw(ctx, reducer, &matrix[..2 * pairs], &values[..2 * pairs])
        }
        RawWitness::Wide(values) => {
            native_witness::wide_coefficients(ctx, &matrix[..2 * pairs], |i| values.read(i))
        }
        RawWitness::Limbs(values) => {
            native_witness::wide_coefficients(ctx, &matrix[..2 * pairs], |i| values.read(i))
        }
        RawWitness::Field(values) => {
            inner_coefficients_field_raw(ctx, reducer, &matrix[..2 * pairs], &values[..2 * pairs])
        }
    };
    drop(round0_scope);
    let mut coefficients_without_linear = [
        crate::utils::delayed_reduction::element(&cfg, coefficients[0]),
        crate::utils::delayed_reduction::element(&cfg, coefficients[1]),
    ];
    let challenge = recover_full_round_polynomial_and_sample_next_challenge(
        transcript,
        &mut current_claim,
        &coefficients_without_linear,
        &mut round_polynomials,
        &mut eval_points,
        &zero,
        &cfg,
    )?;
    let challenge_raw = ctx.raw(&challenge);
    let next_len = len / 2;
    let live = live.div_ceil(2);
    let mut matrix_next = vec![0 as Raw; next_len];
    if next_len == 1 {
        let matrix_evaluation = ctx.interpolate(matrix[0], matrix[1], challenge_raw);
        let witness_evaluation = match &witness {
            RawWitness::Native { values, .. } => fold_native_pair(
                reducer,
                ctx.sub_raw(ctx.one_raw(), challenge_raw),
                challenge_raw,
                values[0],
                values[1],
            ),
            RawWitness::Field(values) => ctx.interpolate(values[0], values[1], challenge_raw),
            RawWitness::Wide(values) => {
                let f = ctx;
                raw_shared(f.weighted_pair(
                    &[
                        shared_raw(f, ctx.sub_raw(ctx.one_raw(), challenge_raw)),
                        shared_raw(f, challenge_raw),
                    ],
                    &[values.read(0), values.read(1)],
                ))
            }
            RawWitness::Limbs(values) => {
                let f = ctx;
                raw_shared(f.weighted_pair(
                    &[
                        shared_raw(f, ctx.sub_raw(ctx.one_raw(), challenge_raw)),
                        shared_raw(f, challenge_raw),
                    ],
                    &[values.read(0), values.read(1)],
                ))
            }
        };
        return finish(
            matrix_evaluation,
            witness_evaluation,
            current_claim,
            round_polynomials,
            eval_points,
        );
    }
    let written = 2 * live.div_ceil(2);
    let fold0_scope = tracing::info_span!("raw:inner_fold0").entered();
    let (coefficients, witness_next) = match &witness {
        RawWitness::Native { values, .. } => {
            let mut out = vec![field::Uint::<2>::ZERO; next_len];
            let coefficients = fold_inner_native_raw(
                ctx,
                reducer,
                &matrix[..2 * written],
                &values[..2 * written],
                &mut matrix_next[..written],
                &mut out[..written],
                challenge_raw,
            );
            (coefficients, FoldedWitness::Integers(out))
        }
        RawWitness::Wide(values) => {
            let mut out = vec![field::Uint::<2>::ZERO; next_len];
            let coefficients = native_witness::wide_fold::<4, true>(
                ctx,
                &matrix[..2 * written],
                |i| values.read(i),
                &mut matrix_next[..written],
                &mut out[..written],
                challenge_raw,
            );
            (coefficients, FoldedWitness::Integers(out))
        }
        RawWitness::Limbs(values) => {
            let mut out = vec![field::Uint::<2>::ZERO; next_len];
            let coefficients = native_witness::wide_fold::<32, true>(
                ctx,
                &matrix[..2 * written],
                |i| values.read(i),
                &mut matrix_next[..written],
                &mut out[..written],
                challenge_raw,
            );
            (coefficients, FoldedWitness::Integers(out))
        }
        RawWitness::Field(values) => {
            let mut out = vec![shared_raw(ctx, 0); next_len];
            let coefficients = folded::fold_round::<field::Fp<2>, true>(
                ctx,
                &matrix[..2 * written],
                |i| shared_raw(ctx, values[i]),
                &mut matrix_next[..written],
                &mut out[..written],
                challenge_raw,
            );
            (coefficients, FoldedWitness::Field(out))
        }
    };
    coefficients_without_linear = [
        crate::utils::delayed_reduction::element(&cfg, coefficients[0]),
        crate::utils::delayed_reduction::element(&cfg, coefficients[1]),
    ];
    drop(matrix);
    drop(witness);
    drop(fold0_scope);

    let _rounds_scope = tracing::info_span!("raw:inner_rounds").entered();
    let (matrix_evaluation, witness_evaluation) = match witness_next {
        FoldedWitness::Integers(values) => inner_dense_rounds(
            transcript,
            ctx,
            reducer,
            &mut current_claim,
            matrix_next,
            values,
            live,
            coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
        )?,
        FoldedWitness::Field(values) => inner_dense_rounds(
            transcript,
            ctx,
            reducer,
            &mut current_claim,
            matrix_next,
            values,
            live,
            coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
        )?,
    };
    finish(
        matrix_evaluation,
        witness_evaluation,
        current_claim,
        round_polynomials,
        eval_points,
    )
}

/// Runs the remaining dense inner rounds from a prepared state: field-valued
/// `matrix` and `witness` tables (the latter in `scale` form), the leading
/// `live` entries possibly nonzero, and the current round's `[c0, c2]`
/// already computed. Returns the terminal `(matrix, witness)` values, both
/// as raw residues.
#[allow(clippy::too_many_arguments)]
fn inner_dense_rounds<T: Transcript, W: FoldedValue>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    current_claim: &mut Field,
    mut matrix: Vec<Raw>,
    mut witness: Vec<W>,
    mut live: usize,
    mut coefficients_without_linear: [Field; 2],
    round_polynomials: &mut Vec<[Field; 3]>,
    eval_points: &mut Vec<Field>,
) -> Result<(Raw, Raw), SumcheckError> {
    let cfg = ctx.clone();
    debug_assert_eq!(matrix.len(), witness.len());
    let zero = Field::zero_with_cfg(&cfg);
    let mut matrix_scratch = vec![0 as Raw; matrix.len() / 2];
    let mut witness_scratch = vec![W::zero(ctx); witness.len() / 2];
    while matrix.len() > 1 {
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            current_claim,
            &coefficients_without_linear,
            round_polynomials,
            eval_points,
            &zero,
            &cfg,
        )?;
        let challenge_raw = ctx.raw(&challenge);
        let next_len = matrix.len() / 2;
        let next_live = live.div_ceil(2);
        matrix_scratch.truncate(next_len);
        witness_scratch.truncate(next_len);
        if next_len == 1 {
            matrix_scratch[0] = ctx.interpolate(matrix[0], matrix[1], challenge_raw);
            witness_scratch[0] = W::from_encoding(
                ctx,
                ctx.interpolate(witness[0].encoding(), witness[1].encoding(), challenge_raw),
            );
        } else {
            let written = 2 * next_live.div_ceil(2);
            let coefficients = folded::fold_round::<W, true>(
                ctx,
                &matrix[..2 * written],
                |i| witness[i],
                &mut matrix_scratch[..written],
                &mut witness_scratch[..written],
                challenge_raw,
            );
            matrix_scratch[written..].fill(0);
            witness_scratch[written..].fill(W::zero(ctx));
            coefficients_without_linear = [
                crate::utils::delayed_reduction::element(&cfg, coefficients[0]),
                crate::utils::delayed_reduction::element(&cfg, coefficients[1]),
            ];
        }
        std::mem::swap(&mut matrix, &mut matrix_scratch);
        std::mem::swap(&mut witness, &mut witness_scratch);
        live = next_live;
    }
    Ok((matrix[0], witness[0].final_raw(ctx)))
}

// ---------------------------------------------------------------------------
// Structured inner sumcheck (block-selector relations)
// ---------------------------------------------------------------------------

/// The batched matrix of a block-selector relation, without materializing it:
/// `D(k · block_len + r) = scales[k] · weights[r]` for `r < rows`, zero for
/// blocks without a scale and for `r ≥ rows`.
pub(crate) struct BlockScales {
    pub block_len: usize,
    pub rows: usize,
    /// Per block: the summed `f · coefficient` of the runs starting there.
    pub scales: Vec<Option<Raw>>,
}

/// Aggregates a [`BlockSelectorLayout`] into per-block scales for `ρ`.
pub(crate) fn block_scales_raw<C: RawMontyCoefficient>(
    ctx: &field::FpCtx<2>,
    layout: &BlockSelectorLayout<C>,
    rho: Raw,
    num_column_vars: usize,
) -> BlockScales {
    let blocks = (1usize << num_column_vars) / layout.block_len;
    let prepared = C::prepare_raw(ctx);
    let mut scales = vec![None; blocks];
    let factors = [ctx.one_raw(), rho, ctx.mul_raw(rho, rho)];
    for (runs, factor) in [
        (&layout.a, factors[0]),
        (&layout.b, factors[1]),
        (&layout.c, factors[2]),
    ] {
        for run in runs {
            let scale = ctx.mul_raw(
                factor,
                run.coefficient.raw_scale(&prepared, ctx.one_raw(), ctx),
            );
            let slot = &mut scales[run.start / layout.block_len];
            *slot = Some(match *slot {
                Some(existing) => ctx.add_raw(existing, scale),
                None => scale,
            });
        }
    }
    BlockScales {
        block_len: layout.block_len,
        rows: layout.rows,
        scales,
    }
}

/// Folds one table at `challenge` (no accumulation).
fn fold_table_raw(ctx: &field::FpCtx<2>, input: &[Raw], output: &mut [Raw], challenge: Raw) {
    debug_assert_eq!(input.len(), 2 * output.len());
    #[cfg(feature = "parallel")]
    if parallel(output.len()) {
        input
            .par_chunks_exact(2)
            .zip(output.par_iter_mut())
            .for_each(|(pair, value)| *value = ctx.interpolate(pair[0], pair[1], challenge));
        return;
    }
    for (pair, value) in input.chunks_exact(2).zip(output.iter_mut()) {
        *value = ctx.interpolate(pair[0], pair[1], challenge);
    }
}

/// Folds one witness block at `challenge` against the ALREADY folded weight
/// vector and accumulates the block's next-round partial sums
/// First native block fold: the
/// folded block is emitted in plain form.
fn fold_block_native_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    weights_next: &[Raw],
    z_in: &[u64],
    z_out: &mut [field::Uint<2>],
    challenge: Raw,
) -> [Raw; 2] {
    debug_assert_eq!(z_in.len(), 2 * z_out.len());
    debug_assert_eq!(weights_next.len(), z_out.len());
    debug_assert_eq!(z_out.len() % 2, 0);
    let one_minus_challenge = ctx.sub_raw(ctx.one_raw(), challenge);
    let block = |weights: &[Raw],
                 z_in: &[u64],
                 z_out: &mut [field::Uint<2>]|
     -> [field::FpLinearAcc<2, 2>; 2] {
        let mut accumulators = folded::pair::<field::Uint<2>>();
        for ((w, z), out) in weights
            .chunks_exact(2)
            .zip(z_in.chunks_exact(4))
            .zip(z_out.chunks_exact_mut(2))
        {
            let z0 = fold_native_pair_plain(ctx, one_minus_challenge, challenge, z[0], z[1]);
            let z1 = fold_native_pair_plain(ctx, one_minus_challenge, challenge, z[2], z[3]);
            out[0] = field::Uint::from_words(raw_to_words(z0));
            out[1] = field::Uint::from_words(raw_to_words(z1));
            <field::Uint<2> as FoldedValue>::accumulate(
                ctx,
                &mut accumulators[0],
                w[0],
                field::Uint::from_words(raw_to_words(z0)),
            );
            <field::Uint<2> as FoldedValue>::accumulate(
                ctx,
                &mut accumulators[1],
                ctx.sub_raw(w[1], w[0]),
                field::Uint::from_words(raw_to_words(ctx.sub_raw(z1, z0))),
            );
        }
        accumulators
    };
    #[cfg(feature = "parallel")]
    if parallel(z_out.len() / 2) {
        let total = (
            weights_next.par_chunks(FOLD_BLOCK),
            z_in.par_chunks(2 * FOLD_BLOCK),
            z_out.par_chunks_mut(FOLD_BLOCK),
        )
            .into_par_iter()
            .map(|(w, z, out)| block(w, z, out))
            .reduce(
                folded::pair::<field::Uint<2>>,
                folded::merge::<field::Uint<2>>,
            );
        return folded::reduce::<field::Uint<2>>(ctx, total);
    }
    folded::reduce::<field::Uint<2>>(ctx, block(weights_next, z_in, z_out))
}

/// Terminal folds for structural constant/zero blocks. Private values never
/// determine whether a block receives the full weighted-sum kernel.
fn sparse_block_value_raw(eq_at_zero: Raw, block: BlockValues<'_>) -> Option<Raw> {
    match block {
        BlockValues::ConstantOne => Some(eq_at_zero),
        BlockValues::Zero => Some(0),
        BlockValues::Native([]) | BlockValues::Field([]) => Some(0),
        _ => None,
    }
}

/// `Σ_y eq[y] · z[y]` as a raw residue, for a block that carries no matrix
/// scale (its fold never enters a message before the block rounds).
fn weighted_block_sum_raw(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    eq: &[Raw],
    block: BlockValues<'_>,
) -> Raw {
    match block {
        BlockValues::U128(_)
        | BlockValues::U256(_, _)
        | BlockValues::ConstantOne
        | BlockValues::Limbs(_)
        | BlockValues::Zero => native_witness::weighted_wide_block(ctx, eq, block),
        BlockValues::Native(values) => {
            debug_assert!(values.len() <= eq.len());
            let block = |eq: &[Raw], values: &[u64]| -> field::FpLinearAcc<2, 1> {
                let mut accumulator = field::FpLinearAcc::<2, 1>::default();
                for (&weight, &value) in eq.iter().zip(values) {
                    accumulator.accumulate_encoded(ctx, weight, value);
                }
                accumulator
            };
            #[cfg(feature = "parallel")]
            if parallel(values.len()) {
                let total = eq[..values.len()]
                    .par_chunks(2 * FOLD_BLOCK)
                    .zip(values.par_chunks(2 * FOLD_BLOCK))
                    .map(|(eq, values)| block(eq, values))
                    .reduce(field::FpLinearAcc::<2, 1>::default, |mut left, right| {
                        left += right;
                        left
                    });
                return total.reduce_encoded(reducer);
            }
            block(&eq[..values.len()], values).reduce_encoded(reducer)
        }
        BlockValues::Field(values) => {
            debug_assert!(values.len() <= eq.len());
            let block = |eq: &[Raw], values: &[Raw]| -> field::FpProductAcc<2> {
                let mut accumulator = field::FpProductAcc::<2>::default();
                for (&weight, &value) in eq.iter().zip(values) {
                    accumulator.accumulate_encoded(ctx, weight, value);
                }
                accumulator
            };
            #[cfg(feature = "parallel")]
            if parallel(values.len()) {
                let total = eq[..values.len()]
                    .par_chunks(2 * FOLD_BLOCK)
                    .zip(values.par_chunks(2 * FOLD_BLOCK))
                    .map(|(eq, values)| block(eq, values))
                    .reduce(field::FpProductAcc::<2>::default, |mut left, right| {
                        left += right;
                        left
                    });
                return total.reduce_encoded(reducer);
            }
            block(&eq[..values.len()], values).reduce_encoded(reducer)
        }
    }
}

/// One whole witness block in the caller's representation (entries past the
/// live prefix are zero).
#[derive(Clone, Copy)]
enum BlockValues<'a> {
    Native(&'a [u64]),
    Field(&'a [Raw]),
    U128(&'a [u128]),
    U256(&'a [u128], &'a [u128]),
    ConstantOne,
    Zero,
    Limbs(&'a [field::Uint<32>]),
}

/// Proves the inner sumcheck of a block-selector relation without the dense
/// batched matrix: the raw twin of `prove_inner_raw` on
/// `D(k · block_len + r) = scales[k] · weights[r]`.
///
/// Rounds over the in-block coordinates fold ONE weight vector plus the
/// witness blocks that carry a scale, and each message is
/// `Σ_k scales[k] · (Σ_i w'(2i) z'_k(2i), Σ_i Δw' Δz'_k)`; blocks without a
/// scale contribute nothing until the block coordinates, where their fold is
/// one weighted sum against the equality table of the in-block challenges
/// (a single product for the constant block). The block rounds then run
/// densely on `blocks` entries. Every message equals the dense prover's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_inner_structured_raw<T: Transcript>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    initial_claim: Field,
    weights: &[Raw],
    scales: &BlockScales,
    witness: RawWitness<'_>,
    live: usize,
    num_column_vars: usize,
) -> Result<InnerSumcheckOutput<Field>, SumcheckError> {
    match witness {
        RawWitness::Field(_) => prove_inner_structured_typed::<T, field::Fp<2>>(
            transcript,
            ctx,
            reducer,
            initial_claim,
            weights,
            scales,
            witness,
            live,
            num_column_vars,
        ),
        RawWitness::Native { .. } | RawWitness::Wide(_) | RawWitness::Limbs(_) => {
            prove_inner_structured_typed::<T, field::Uint<2>>(
                transcript,
                ctx,
                reducer,
                initial_claim,
                weights,
                scales,
                witness,
                live,
                num_column_vars,
            )
        }
    }
}

fn prove_inner_structured_typed<T: Transcript, W: FoldedValue>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    initial_claim: Field,
    weights: &[Raw],
    scales: &BlockScales,
    witness: RawWitness<'_>,
    live: usize,
    num_column_vars: usize,
) -> Result<InnerSumcheckOutput<Field>, SumcheckError> {
    let cfg = ctx.clone();
    let block_len = scales.block_len;
    let blocks = scales.scales.len();
    let domain = 1usize << num_column_vars;
    if !block_len.is_power_of_two()
        || block_len < 2
        || block_len * blocks != domain
        || !blocks.is_power_of_two()
        || witness.len() != domain
        || live > domain
        || scales.rows > block_len
        || weights.len() < scales.rows
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    // A borrowed native table must cover every live entry in whole blocks;
    // otherwise pad it (the generated relations lend block-aligned tables).
    let witness = match witness {
        RawWitness::Native { values, domain }
            if values.len() < live || !values.len().is_multiple_of(block_len) =>
        {
            RawWitness::Native { values, domain }.materialize()
        }
        other => other,
    };
    let zero = Field::zero_with_cfg(&cfg);
    let mut current_claim = initial_claim;
    let mut eval_points = Vec::with_capacity(num_column_vars);
    let mut round_polynomials = Vec::with_capacity(num_column_vars);
    let live_of = |block: usize| live.saturating_sub(block * block_len).min(block_len);
    // Whole blocks: entries past a block's live prefix are validated zero,
    // so pair windows may straddle the live boundary exactly as in the dense
    // prover.
    let block_values = |block: usize| -> BlockValues<'_> {
        let start = block * block_len;
        let end = start + block_len;
        match &witness {
            RawWitness::Native { values, .. } => {
                BlockValues::Native(values.get(start..end).unwrap_or(&[]))
            }
            RawWitness::Field(values) => BlockValues::Field(&values[start..end]),
            RawWitness::Wide(values) => {
                assert_eq!(values.len() / 4, block_len);
                values.block(block)
            }
            RawWitness::Limbs(values) => {
                assert_eq!(values.len() / 4, block_len);
                values.block(block)
            }
        }
    };

    // The weight vector as one block: zero beyond the logical rows.
    let mut wz = vec![0 as Raw; block_len];
    wz[..scales.rows].copy_from_slice(&weights[..scales.rows]);

    // Blocks that carry a scale and live entries drive the in-block rounds.
    let scaled: Vec<(usize, Raw)> = (0..blocks)
        .filter_map(|block| scales.scales[block].map(|scale| (block, scale)))
        .filter(|(block, _)| live_of(*block) > 0)
        .collect();
    let combine = |partials: &[[Raw; 2]]| -> [Field; 2] {
        let mut c0 = 0 as Raw;
        let mut c2 = 0 as Raw;
        for ((_, scale), partial) in scaled.iter().zip(partials) {
            c0 = ctx.add_raw(c0, ctx.mul_raw(*scale, partial[0]));
            c2 = ctx.add_raw(c2, ctx.mul_raw(*scale, partial[1]));
        }
        [
            crate::utils::delayed_reduction::element(&cfg, c0),
            crate::utils::delayed_reduction::element(&cfg, c2),
        ]
    };

    // Round zero over the original witness.
    let round0_scope = tracing::info_span!("raw:inner_round0").entered();
    let partials: Vec<[Raw; 2]> = scaled
        .iter()
        .map(|&(block, _)| {
            let pairs = live_of(block).div_ceil(2);
            block_values(block).coefficients(ctx, reducer, &wz[..2 * pairs])
        })
        .collect();
    let mut coefficients_without_linear = combine(&partials);
    drop(round0_scope);

    // In-block rounds: fold the weights once per round, then every scaled
    // block against them. `tables[i]` holds scaled block `i` after its first
    // fold (plain for a native witness).
    let mut challenges_raw: Vec<Raw> = Vec::with_capacity(num_column_vars);
    let mut tables: Vec<Vec<W>> = Vec::new();
    let mut scratches: Vec<Vec<W>> = Vec::new();
    let mut lives: Vec<usize> = scaled.iter().map(|&(block, _)| live_of(block)).collect();
    let mut wz_scratch = vec![0 as Raw; block_len / 2];
    let mut len = block_len;
    let mut finals: Vec<Raw> = Vec::new(); // scaled blocks' terminal values (raw)
    let _low_scope = tracing::info_span!("raw:inner_block_rounds").entered();
    while len > 1 {
        let first_fold = tables.is_empty();
        let challenge = recover_full_round_polynomial_and_sample_next_challenge(
            transcript,
            &mut current_claim,
            &coefficients_without_linear,
            &mut round_polynomials,
            &mut eval_points,
            &zero,
            &cfg,
        )?;
        let challenge_raw = ctx.raw(&challenge);
        challenges_raw.push(challenge_raw);
        let next_len = len / 2;
        wz_scratch.truncate(next_len);
        fold_table_raw(ctx, &wz, &mut wz_scratch, challenge_raw);
        std::mem::swap(&mut wz, &mut wz_scratch);

        if next_len == 1 {
            // Final in-block fold: one value per scaled block.
            finals = scaled
                .iter()
                .enumerate()
                .map(|(index, &(block, _))| {
                    if first_fold {
                        block_values(block).folded_pair(ctx, reducer, challenge_raw)
                    } else {
                        let table = &tables[index];
                        W::from_encoding(
                            ctx,
                            ctx.interpolate(
                                table[0].encoding(),
                                table[1].encoding(),
                                challenge_raw,
                            ),
                        )
                        .final_raw(ctx)
                    }
                })
                .collect();
            len = next_len;
            break;
        }

        let mut partials = Vec::with_capacity(scaled.len());
        for (index, &(block, _)) in scaled.iter().enumerate() {
            let next_live = lives[index].div_ceil(2);
            let written = 2 * next_live.div_ceil(2);
            if first_fold {
                let mut out = vec![W::zero(ctx); next_len];
                let partial = W::fold_initial(
                    ctx,
                    reducer,
                    block_values(block),
                    &wz[..written],
                    &mut out[..written],
                    challenge_raw,
                );
                tables.push(out);
                scratches.push(vec![W::zero(ctx); next_len / 2]);
                partials.push(partial);
            } else {
                let scratch = &mut scratches[index];
                scratch.truncate(next_len);
                let partial = folded::fold_round::<W, false>(
                    ctx,
                    &wz[..written],
                    |i| tables[index][i],
                    &mut [],
                    &mut scratch[..written],
                    challenge_raw,
                );
                scratch[written..].fill(W::zero(ctx));
                std::mem::swap(&mut tables[index], scratch);
                partials.push(partial);
            }
            lives[index] = next_live;
        }
        coefficients_without_linear = combine(&partials);
        len = next_len;
    }
    debug_assert_eq!(len, 1);
    debug_assert_eq!(finals.len(), scaled.len());
    let weight_final = wz[0];

    // Block rounds on the dense `blocks`-entry tables. Unscaled blocks fold
    // to one weighted sum; the full equality table is built only if one of
    // them is not the sparse constant block.
    let eq_at_zero = challenges_raw
        .iter()
        .fold(ctx.one_raw(), |product, &challenge| {
            ctx.mul_raw(product, ctx.sub_raw(ctx.one_raw(), challenge))
        });
    let mut eq: Option<Vec<Raw>> = None;
    let mut matrix = vec![0 as Raw; blocks];
    let mut witness_final = vec![0 as Raw; blocks];
    for (index, &(block, scale_value)) in scaled.iter().enumerate() {
        matrix[block] = ctx.mul_raw(scale_value, weight_final);
        witness_final[block] = finals[index];
    }
    for (block, slot) in witness_final.iter_mut().enumerate() {
        if live_of(block) == 0 || scales.scales[block].is_some() {
            continue;
        }
        *slot = match sparse_block_value_raw(eq_at_zero, block_values(block)) {
            Some(value) => value,
            None => {
                let eq = eq.get_or_insert_with(|| eq_table_raw(ctx, &challenges_raw));
                weighted_block_sum_raw(ctx, reducer, eq, block_values(block))
            }
        };
    }
    drop(witness);

    let finish = |matrix_evaluation: Raw,
                  witness_evaluation: Raw,
                  current_claim: Field,
                  round_polynomials: Vec<[Field; 3]>,
                  eval_points: Vec<Field>| {
        let batched_matrix_evaluation =
            crate::utils::delayed_reduction::element(&cfg, matrix_evaluation);
        let witness_evaluation = crate::utils::delayed_reduction::element(&cfg, witness_evaluation);
        debug_assert_eq!(
            current_claim,
            cfg.mul(&(batched_matrix_evaluation.clone()), &(&witness_evaluation))
        );
        Ok(InnerSumcheckOutput {
            sumcheck: SumcheckProverOutput {
                proof: SumcheckProof { round_polynomials },
                eval_points,
                final_claim: current_claim,
            },
            batched_matrix_evaluation,
            witness_evaluation,
        })
    };
    if blocks == 1 {
        return finish(
            matrix[0],
            witness_final[0],
            current_claim,
            round_polynomials,
            eval_points,
        );
    }
    let coefficients = inner_coefficients_field_raw(ctx, reducer, &matrix, &witness_final);
    let coefficients_without_linear = [
        crate::utils::delayed_reduction::element(&cfg, coefficients[0]),
        crate::utils::delayed_reduction::element(&cfg, coefficients[1]),
    ];
    let (matrix_evaluation, witness_evaluation) = inner_dense_rounds(
        transcript,
        ctx,
        reducer,
        &mut current_claim,
        matrix,
        witness_final
            .into_iter()
            .map(|v| shared_raw(ctx, v))
            .collect::<Vec<_>>(),
        blocks,
        coefficients_without_linear,
        &mut round_polynomials,
        &mut eval_points,
    )?;
    finish(
        matrix_evaluation,
        witness_evaluation,
        current_claim,
        round_polynomials,
        eval_points,
    )
}

// ---------------------------------------------------------------------------
// Matrix binding
// ---------------------------------------------------------------------------

/// Coefficient scaling on raw residues: the prover-side twin of
/// [`SpartanMatrixCoefficient::scale`], with per-proof constants prepared once
/// instead of per matrix entry.
pub trait RawMontyCoefficient: Sync {
    /// Constants derived from the field context once per binding.
    type Prepared: Sync;

    fn prepare_raw(ctx: &field::FpCtx<2>) -> Self::Prepared;

    /// `self · value`.
    fn raw_scale(&self, prepared: &Self::Prepared, value: Raw, ctx: &field::FpCtx<2>) -> Raw;
}

impl RawMontyCoefficient for bool {
    type Prepared = ();

    fn prepare_raw(_ctx: &field::FpCtx<2>) -> Self::Prepared {}

    #[inline(always)]
    fn raw_scale(&self, _prepared: &(), value: Raw, _ctx: &field::FpCtx<2>) -> Raw {
        if *self { value } else { 0 }
    }
}

impl RawMontyCoefficient for Field {
    type Prepared = ();

    fn prepare_raw(_ctx: &field::FpCtx<2>) -> Self::Prepared {}

    #[inline(always)]
    fn raw_scale(&self, _prepared: &(), value: Raw, ctx: &field::FpCtx<2>) -> Raw {
        ctx.mul_raw(ctx.raw(self), value)
    }
}

impl RawMontyCoefficient for U64MulCoefficient {
    /// The public limb base `2^64` as a residue of the runtime field.
    type Prepared = Raw;

    fn prepare_raw(ctx: &field::FpCtx<2>) -> Self::Prepared {
        ctx.native_residue_u128(U64_MUL_LIMB_BASE)
    }

    #[inline(always)]
    fn raw_scale(&self, limb_base: &Raw, value: Raw, ctx: &field::FpCtx<2>) -> Raw {
        match self {
            Self::One => value,
            Self::LimbBase => ctx.mul_raw(*limb_base, value),
        }
    }
}

impl RawMontyCoefficient for BabyBearMulCoefficient {
    /// The embedded BabyBear modulus as a residue of the runtime field.
    type Prepared = Raw;

    fn prepare_raw(ctx: &field::FpCtx<2>) -> Self::Prepared {
        ctx.native_residue(BABY_BEAR_MODULUS)
    }

    #[inline(always)]
    fn raw_scale(&self, modulus: &Raw, value: Raw, ctx: &field::FpCtx<2>) -> Raw {
        match self {
            Self::One => value,
            Self::Modulus => ctx.mul_raw(*modulus, value),
        }
    }
}

/// `D(j) = Σ_i row_weights[i] (A[i,j] + ρ B[i,j] + ρ² C[i,j])` over the padded
/// column domain: the raw twin of
/// `PreparedConstraintMatrices::bind_and_batch_with_validated_row_weights`.
pub(crate) fn bind_and_batch_raw<C>(
    ctx: &field::FpCtx<2>,
    matrices: &PreparedConstraintMatrices<Field, C>,
    row_weights: &[Raw],
    rho: Raw,
) -> Vec<Raw>
where
    C: SpartanMatrixCoefficient<Field> + RawMontyCoefficient,
{
    debug_assert_eq!(row_weights.len(), 1usize << matrices.num_row_vars());
    let rho_squared = ctx.mul_raw(rho, rho);
    let prepared = C::prepare_raw(ctx);
    let m = matrices.matrices();
    let live_columns = m
        .a()
        .column_count()
        .min(m.b().column_count())
        .min(m.c().column_count());
    let (a_offsets, a_rows, a_coefficients) = (
        m.a().column_offsets(),
        m.a().row_indices(),
        m.a().coefficients(),
    );
    let (b_offsets, b_rows, b_coefficients) = (
        m.b().column_offsets(),
        m.b().row_indices(),
        m.b().coefficients(),
    );
    let (c_offsets, c_rows, c_coefficients) = (
        m.c().column_offsets(),
        m.c().row_indices(),
        m.c().coefficients(),
    );
    let dot = |offsets: &[usize], rows: &[usize], coefficients: &[C], column: usize| -> Raw {
        let mut evaluation = 0;
        for entry in offsets[column]..offsets[column + 1] {
            evaluation = ctx.add_raw(
                evaluation,
                coefficients[entry].raw_scale(&prepared, row_weights[rows[entry]], ctx),
            );
        }
        evaluation
    };
    // Whole column ranges per block: sequential walks over the three CSC
    // offset arrays instead of three bounds-checked column lookups per entry.
    let block = |first_column: usize, output: &mut [Raw]| {
        for (offset, slot) in output.iter_mut().enumerate() {
            let column = first_column + offset;
            let mut evaluation = dot(a_offsets, a_rows, a_coefficients, column);
            if b_offsets[column + 1] > b_offsets[column] {
                evaluation = ctx.add_raw(
                    evaluation,
                    ctx.mul_raw(rho, dot(b_offsets, b_rows, b_coefficients, column)),
                );
            }
            if c_offsets[column + 1] > c_offsets[column] {
                evaluation = ctx.add_raw(
                    evaluation,
                    ctx.mul_raw(rho_squared, dot(c_offsets, c_rows, c_coefficients, column)),
                );
            }
            *slot = evaluation;
        }
    };
    let mut evaluations = vec![0 as Raw; 1usize << matrices.num_column_vars()];
    let live = &mut evaluations[..live_columns];
    #[cfg(feature = "parallel")]
    if parallel(live_columns) {
        live.par_chunks_mut(BIND_BLOCK)
            .enumerate()
            .for_each(|(index, output)| block(index * BIND_BLOCK, output));
        return evaluations;
    }
    block(0, live);
    evaluations
}

/// The row functional binding the matrices for the inner sumcheck.
pub(crate) enum RowFunctional<'a> {
    /// `eq(·, point)` over the padded row domain (the standard outer).
    Point(&'a [Field]),
    /// The prefix-univariate factors (the univariate-skip outer).
    Prefix(&'a PrefixUnivariateRowFactors<Field>),
}

/// Binds the matrices with `functional` and proves the inner sumcheck on raw
/// tables: structured (no dense batched matrix) when the matrices are block
/// selectors, dense otherwise. Both routes emit identical messages.
#[allow(clippy::too_many_arguments)]
pub(crate) fn inner_sumcheck_raw<T, C>(
    transcript: &mut T,
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    matrices: &PreparedConstraintMatrices<Field, C>,
    initial_claim: Field,
    functional: RowFunctional<'_>,
    rho: Raw,
    witness: RawWitness<'_>,
) -> Result<InnerSumcheckOutput<Field>, SumcheckError>
where
    T: Transcript,
    C: SpartanMatrixCoefficient<Field> + RawMontyCoefficient,
{
    let num_column_vars = matrices.num_column_vars();
    let live = matrices.matrices().column_count();
    if let Some(layout) = matrices
        .block_selector()
        .filter(|layout| layout.block_len >= 2)
    {
        let (weights, scales) = {
            let _scope = tracing::info_span!("spartan:bind_and_batch").entered();
            let weights = match functional {
                RowFunctional::Point(point) => eq_table_raw(ctx, &ctx.raw_vec(point)),
                RowFunctional::Prefix(factors) => prefix_row_weights_raw(ctx, factors),
            };
            (weights, block_scales_raw(ctx, layout, rho, num_column_vars))
        };
        let _scope = tracing::info_span!("spartan:inner_sumcheck").entered();
        return prove_inner_structured_raw(
            transcript,
            ctx,
            reducer,
            initial_claim,
            &weights,
            &scales,
            witness,
            live,
            num_column_vars,
        );
    }
    let matrix = {
        let _scope = tracing::info_span!("spartan:bind_and_batch").entered();
        match functional {
            RowFunctional::Point(point) => {
                bind_and_batch_raw(ctx, matrices, &eq_table_raw(ctx, &ctx.raw_vec(point)), rho)
            }
            RowFunctional::Prefix(factors) => {
                bind_and_batch_prefix_raw(ctx, matrices, factors, rho)
            }
        }
    };
    let _scope = tracing::info_span!("spartan:inner_sumcheck").entered();
    prove_inner_raw(
        transcript,
        ctx,
        reducer,
        initial_claim,
        matrix,
        witness,
        live,
    )
}

/// Materializes the prefix-univariate row weights `prefix[s] · tail(x)` in
/// canonical row order (`s + 2^K x`), the raw twin of
/// `PrefixUnivariateRowFactors::materialize`.
pub(crate) fn prefix_row_weights_raw(
    ctx: &field::FpCtx<2>,
    factors: &PrefixUnivariateRowFactors<Field>,
) -> Vec<Raw> {
    let parts = factors.parts();
    let prefix = ctx.raw_vec(parts.prefix);
    let tail_low = ctx.raw_vec(parts.tail_low);
    let tail_high = ctx.raw_vec(parts.tail_high);
    let block_len = 1usize << parts.skip_vars;
    let low_mask = tail_low.len() - 1;
    let total_rows = 1usize << parts.num_row_vars;
    let mut weights = vec![0 as Raw; total_rows];
    let fill = |suffix: usize, block: &mut [Raw]| {
        let tail = ctx.mul_raw(
            tail_low[suffix & low_mask],
            tail_high[suffix >> parts.tail_low_vars],
        );
        for (weight, &prefix_weight) in block.iter_mut().zip(&prefix) {
            *weight = ctx.mul_raw(prefix_weight, tail);
        }
    };
    #[cfg(feature = "parallel")]
    if parallel(total_rows) {
        weights
            .par_chunks_mut(block_len)
            .enumerate()
            .for_each(|(suffix, block)| fill(suffix, block));
        return weights;
    }
    for (suffix, block) in weights.chunks_mut(block_len).enumerate() {
        fill(suffix, block);
    }
    weights
}

/// The raw twin of `bind_and_batch_with_prefix_univariate_factors`: disjoint
/// unit-selector matrices are filled one prefix block at a time, in parallel,
/// without materializing the row-weight tensor; other layouts materialize
/// the weights and bind generically.
pub(crate) fn bind_and_batch_prefix_raw<C>(
    ctx: &field::FpCtx<2>,
    matrices: &PreparedConstraintMatrices<Field, C>,
    factors: &PrefixUnivariateRowFactors<Field>,
    rho: Raw,
) -> Vec<Raw>
where
    C: SpartanMatrixCoefficient<Field> + RawMontyCoefficient,
{
    let parts = factors.parts();
    let prefix = ctx.raw_vec(parts.prefix);
    let tail_low = ctx.raw_vec(parts.tail_low);
    let tail_high = ctx.raw_vec(parts.tail_high);
    let block_len = 1usize << parts.skip_vars;
    let low_mask = tail_low.len() - 1;
    let tail_weight = |suffix: usize| -> Raw {
        ctx.mul_raw(
            tail_low[suffix & low_mask],
            tail_high[suffix >> parts.tail_low_vars],
        )
    };
    let domain = 1usize << matrices.num_column_vars();

    let layout = matrices.selector_layout().filter(|[rows, a, b, c]| {
        let mut offsets = [*a, *b, *c];
        offsets.sort_unstable();
        offsets[0] + rows <= offsets[1]
            && offsets[1] + rows <= offsets[2]
            && offsets[2] + rows <= domain
    });
    let Some([rows, a_offset, b_offset, c_offset]) = layout else {
        let weights = prefix_row_weights_raw(ctx, factors);
        return bind_and_batch_raw(ctx, matrices, &weights, rho);
    };

    let rho_squared = ctx.mul_raw(rho, rho);
    let mut evaluations = vec![0 as Raw; domain];
    // Carve the three disjoint selector regions out of the table.
    let mut order = [(a_offset, 0usize), (b_offset, 1), (c_offset, 2)];
    order.sort_unstable();
    let (first, rest) = evaluations[order[0].0..].split_at_mut(rows);
    let (second, rest) = rest[order[1].0 - order[0].0 - rows..].split_at_mut(rows);
    let third = &mut rest[order[2].0 - order[1].0 - rows..][..rows];
    let mut regions: [Option<&mut [Raw]>; 3] = [None, None, None];
    regions[order[0].1] = Some(first);
    regions[order[1].1] = Some(second);
    regions[order[2].1] = Some(third);
    let [Some(a_region), Some(b_region), Some(c_region)] = regions else {
        unreachable!("every selector region is assigned exactly once");
    };

    let fill = |suffix: usize, a: &mut [Raw], b: &mut [Raw], c: &mut [Raw]| {
        let tail = tail_weight(suffix);
        for (((a, b), c), &prefix_weight) in a.iter_mut().zip(b).zip(c).zip(&prefix) {
            let weight = ctx.mul_raw(prefix_weight, tail);
            *a = weight;
            *b = ctx.mul_raw(rho, weight);
            *c = ctx.mul_raw(rho_squared, weight);
        }
    };
    #[cfg(feature = "parallel")]
    if parallel(rows) {
        (
            a_region.par_chunks_mut(block_len),
            b_region.par_chunks_mut(block_len),
            c_region.par_chunks_mut(block_len),
        )
            .into_par_iter()
            .enumerate()
            .for_each(|(suffix, (a, b, c))| fill(suffix, a, b, c));
        return evaluations;
    }
    for (suffix, ((a, b), c)) in a_region
        .chunks_mut(block_len)
        .zip(b_region.chunks_mut(block_len))
        .zip(c_region.chunks_mut(block_len))
        .enumerate()
    {
        fill(suffix, a, b, c);
    }
    evaluations
}

#[cfg(test)]
mod tests {
    use rand::{RngExt, SeedableRng, rngs::StdRng};

    use super::super::{
        baby_bear_mul::{BabyBearMulLayout, baby_bear_mul_constraint_matrices},
        matrix::{
            ConstraintMatrices, ConstraintMatricesSkeleton, SparseMatrix, eq_table,
            make_equality_factors,
        },
        squeeze_field,
        sumcheck::{
            prove_inner_sumcheck_u32_native_with_reducer, prove_inner_sumcheck_with_reducer,
            prove_outer_sumcheck_u32_native_with_reducer, prove_outer_sumcheck_with_reducer,
        },
        u32_mul::{U32MulLayout, u32_mul_constraint_matrices},
        univariate_skip::PrefixUnivariateRowBinding,
        univariate_skip_native::{
            compute_u32_native_skip_message_raw, compute_u32_native_skip_message_validated,
            fold_u32_native_prefix_raw, fold_u32_native_prefix_validated,
        },
    };
    use super::*;
    use crate::{poly::mle::DenseMultilinearExtension, transcript::Blake3Transcript};

    const MODULI: [u128; 3] = [(1_u128 << 100) - 15, (1_u128 << 127) - 1, u128::MAX - 158];

    fn config(modulus: u128) -> FieldConfig {
        Fp::<2>::make_cfg(&Uint::from(modulus)).expect("odd test modulus")
    }

    fn random_field(rng: &mut StdRng, cfg: &FieldConfig) -> Field {
        Field::from_with_cfg(rng.random::<u128>(), cfg)
    }

    fn random_fields(rng: &mut StdRng, cfg: &FieldConfig, len: usize) -> Vec<Field> {
        (0..len).map(|_| random_field(rng, cfg)).collect()
    }

    fn dense<T>(evaluations: Vec<T>) -> DenseMultilinearExtension<T> {
        let num_vars = evaluations.len().trailing_zeros() as usize;
        assert_eq!(evaluations.len(), 1 << num_vars);
        DenseMultilinearExtension {
            evaluations,
            num_vars,
        }
    }

    fn next_challenge(transcript: &mut Blake3Transcript, cfg: &FieldConfig) -> Field {
        squeeze_field::<Field, _>(transcript, cfg).unwrap()
    }

    /// `signed_words_residue` against a BigInt reference: two's-complement
    /// words of every width up to nine (the P-256 row products), random and
    /// extreme values of both signs, zero, over every test modulus (all
    /// above `2^64`).
    #[test]
    fn signed_words_residue_matches_bigint() {
        use num_bigint::BigInt;
        use num_traits::ToPrimitive;
        let mut rng = StdRng::seed_from_u64(11);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let shared = field::create_prime_field(field::Uint::from_words([
                modulus as u64,
                (modulus >> 64) as u64,
            ]));
            let projection = field::PreparedSignedProjection::new(shared, 9);
            let q = BigInt::from(modulus);
            for len in 0..=9usize {
                let mut cases: Vec<Vec<u64>> = (0..16)
                    .map(|_| (0..len).map(|_| rng.random::<u64>()).collect())
                    .collect();
                cases.push(vec![u64::MAX; len]);
                if len > 0 {
                    let mut minimum = vec![0; len];
                    minimum[len - 1] = 1 << 63;
                    cases.push(minimum);
                }
                for words in cases {
                    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                    let value = BigInt::from_signed_bytes_le(&bytes);
                    let expected = ((value % &q) + &q) % &q;
                    let expected = Field::from_with_cfg(expected.to_u128().unwrap(), &cfg);
                    assert_eq!(
                        crate::utils::delayed_reduction::element(&cfg, {
                            let value = projection.project(&words);
                            let w = value.as_montgomery_integer().as_words();
                            Raw::from(w[0]) | (Raw::from(w[1]) << 64)
                        }),
                        expected,
                        "modulus {modulus} words {words:?}"
                    );
                }
            }
        }
    }

    /// Throughput microbenchmark of the raw kernels (run with `--ignored
    /// --nocapture`): dependent and independent multiplication chains, the
    /// two reductions, and a generic `Fp` multiplication for scale.
    #[test]
    #[ignore]
    fn raw_arithmetic_microbench() {
        use std::time::Instant;
        let cfg = config(MODULI[0]);
        let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
        let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
        let mut rng = StdRng::seed_from_u64(7);
        let n = 1usize << 22;
        let a: Vec<Raw> = (0..n)
            .map(|_| ctx.raw(&random_field(&mut rng, &cfg)))
            .collect();
        let b: Vec<Raw> = (0..n)
            .map(|_| ctx.raw(&random_field(&mut rng, &cfg)))
            .collect();
        let fa: Vec<Field> = a
            .iter()
            .map(|&v| crate::utils::delayed_reduction::element(&cfg, v))
            .collect();
        let fb: Vec<Field> = b
            .iter()
            .map(|&v| crate::utils::delayed_reduction::element(&cfg, v))
            .collect();
        let ns = |started: Instant| started.elapsed().as_secs_f64() * 1e9 / n as f64;

        // Dependent chain.
        let started = Instant::now();
        let mut acc = a[0];
        for &x in &b {
            acc = ctx.mul_raw(acc, x);
        }
        let dep = ns(started);
        std::hint::black_box(acc);

        // Independent products.
        let mut out = vec![0 as Raw; n];
        let started = Instant::now();
        for i in 0..n {
            out[i] = ctx.mul_raw(a[i], b[i]);
        }
        let indep = ns(started);
        std::hint::black_box(&out);

        // Interpolation.
        let c = ctx.raw(&random_field(&mut rng, &cfg));
        let started = Instant::now();
        for i in 0..n {
            out[i] = ctx.interpolate(a[i], b[i], c);
        }
        let interp = ns(started);
        std::hint::black_box(&out);

        // Product MAC + one reduction per 64 products.
        let started = Instant::now();
        let mut total = 0 as Raw;
        for chunk in a.chunks_exact(64).zip(b.chunks_exact(64)) {
            let mut acc = field::FpProductAcc::<2>::default();
            for (&x, &y) in chunk.0.iter().zip(chunk.1) {
                acc.accumulate_encoded(&ctx, x, y);
            }
            total ^= acc.reduce_encoded(&reducer);
        }
        let mac = ns(started);
        std::hint::black_box(total);

        // Linear reduce alone.
        let started = Instant::now();
        let mut total = 0 as Raw;
        for i in 0..n {
            let mut acc = field::FpLinearAcc::<2, 1>::default();
            acc.accumulate_encoded(&ctx, a[i], b[i] as u64);
            acc.accumulate_encoded(&ctx, b[i], a[i] as u64);
            total ^= acc.reduce_encoded(&reducer);
        }
        let linear = ns(started);
        std::hint::black_box(total);

        // Generic Fp multiplication.
        let mut fout = vec![Field::zero_with_cfg(&cfg); n];
        let started = Instant::now();
        for i in 0..n {
            fout[i] = cfg.mul(&(fa[i].clone()), &(&fb[i]));
        }
        let generic = ns(started);
        std::hint::black_box(&fout);

        eprintln!(
            "raw mul: dependent {dep:.2} ns, independent {indep:.2} ns, interpolate {interp:.2} ns, \
             product MAC+reduce/64 {mac:.2} ns, 2 linear MAC + reduce {linear:.2} ns; \
             Fp mul {generic:.2} ns"
        );
    }

    #[test]
    fn raw_arithmetic_matches_monty_field() {
        let mut rng = StdRng::seed_from_u64(0x5eed_a11c);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            assert_eq!(
                crate::utils::delayed_reduction::element(&cfg, ctx.one_raw()),
                Field::one_with_cfg(&cfg)
            );
            assert_eq!(
                crate::utils::delayed_reduction::element(&cfg, 0),
                Field::zero_with_cfg(&cfg)
            );
            let edge = [0_u128, 1, 2, modulus - 2, modulus - 1];
            let mut samples: Vec<(Field, Field, Field)> = edge
                .iter()
                .flat_map(|&a| edge.iter().map(move |&b| (a, b)))
                .map(|(a, b)| {
                    (
                        Field::from_with_cfg(a, &cfg),
                        Field::from_with_cfg(b, &cfg),
                        Field::from_with_cfg(a ^ b, &cfg),
                    )
                })
                .collect();
            for _ in 0..500 {
                samples.push((
                    random_field(&mut rng, &cfg),
                    random_field(&mut rng, &cfg),
                    random_field(&mut rng, &cfg),
                ));
            }
            for (a, b, c) in samples {
                let (ra, rb, rc) = (ctx.raw(&a), ctx.raw(&b), ctx.raw(&c));
                assert_eq!(crate::utils::delayed_reduction::element(&cfg, ra), a);
                assert_eq!(
                    crate::utils::delayed_reduction::element(&cfg, ctx.add_raw(ra, rb)),
                    cfg.add(&(a.clone()), &(&b))
                );
                assert_eq!(
                    crate::utils::delayed_reduction::element(&cfg, ctx.sub_raw(ra, rb)),
                    cfg.sub(&(a.clone()), &(&b))
                );
                assert_eq!(
                    crate::utils::delayed_reduction::element(&cfg, ctx.neg_raw(ra)),
                    cfg.neg(&a)
                );
                assert_eq!(
                    crate::utils::delayed_reduction::element(&cfg, ctx.mul_raw(ra, rb)),
                    cfg.mul(&(a.clone()), &(&b))
                );
                assert_eq!(
                    crate::utils::delayed_reduction::element(&cfg, ctx.interpolate(ra, rb, rc)),
                    cfg.add(
                        &(a.clone()),
                        &(&(cfg.mul(&(c.clone()), &(&(cfg.sub(&(b.clone()), &(&a)))))))
                    )
                );
            }
            for value in [0_u64, 1, 7, u32::MAX as u64, u64::MAX - 3, u64::MAX] {
                assert_eq!(
                    crate::utils::delayed_reduction::element(&cfg, ctx.native_residue(value)),
                    Field::from_with_cfg(value, &cfg)
                );
            }
        }
    }

    #[test]
    fn raw_equality_tables_match_generic() {
        let mut rng = StdRng::seed_from_u64(0xe9_7ab1e);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            for len in 0..=15 {
                let point = random_fields(&mut rng, &cfg, len);
                let expected = eq_table(&point, &cfg).unwrap();
                let raw = eq_table_raw(&ctx, &ctx.raw_vec(&point));
                assert_eq!(raw.len(), expected.len());
                assert!(
                    raw.iter()
                        .zip(&expected)
                        .all(|(r, e)| { crate::utils::delayed_reduction::element(&cfg, *r) == *e })
                );
                let (low, high) = make_equality_factors(&point, &cfg).unwrap();
                let (raw_low, raw_high) = make_equality_factors_raw(&ctx, &point);
                assert_eq!(ctx.raw_vec(&low.evaluations), raw_low);
                assert_eq!(ctx.raw_vec(&high.evaluations), raw_high);
            }
        }
    }

    fn outer_claim(cfg: &FieldConfig, tau: &[Field], products: &R1csProductMles<Field>) -> Field {
        let weights = eq_table(tau, cfg).unwrap();
        let mut claim = Field::zero_with_cfg(cfg);
        for (index, weight) in weights.iter().enumerate() {
            let residual = cfg.sub(
                &(cfg.mul(
                    &(products.az.evaluations[index].clone()),
                    &(&products.bz.evaluations[index]),
                )),
                &(&products.cz.evaluations[index]),
            );
            claim = cfg.add(&(claim), &(&(cfg.mul(&(weight.clone()), &(&residual)))));
        }
        claim
    }

    #[test]
    fn raw_field_outer_matches_generic() {
        let mut rng = StdRng::seed_from_u64(0x0a7e_0000);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            let generic_reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for num_vars in 0..=13 {
                let len = 1usize << num_vars;
                let tau = random_fields(&mut rng, &cfg, num_vars);
                let products = R1csProductMles {
                    az: dense(random_fields(&mut rng, &cfg, len)),
                    bz: dense(random_fields(&mut rng, &cfg, len)),
                    cz: dense(random_fields(&mut rng, &cfg, len)),
                };
                let claim = outer_claim(&cfg, &tau, &products);

                let mut generic_transcript = Blake3Transcript::new();
                let expected = prove_outer_sumcheck_with_reducer(
                    &mut generic_transcript,
                    claim.clone(),
                    &tau,
                    make_equality_factors(&tau, &cfg).unwrap(),
                    products.clone(),
                    &cfg,
                    &generic_reducer,
                )
                .unwrap();

                let mut raw_transcript = Blake3Transcript::new();
                let (eq_low, eq_high) = make_equality_factors_raw(&ctx, &tau);
                let actual = prove_outer_field_raw(
                    &mut raw_transcript,
                    &ctx,
                    &reducer,
                    claim,
                    &tau,
                    eq_low,
                    eq_high,
                    RawProducts::from_field(&ctx, &products),
                )
                .unwrap();
                assert_eq!(actual, expected, "num_vars={num_vars} modulus={modulus:#x}");
                assert_eq!(
                    next_challenge(&mut raw_transcript, &cfg),
                    next_challenge(&mut generic_transcript, &cfg)
                );
            }
        }
    }

    #[test]
    fn raw_native_outer_matches_generic() {
        let mut rng = StdRng::seed_from_u64(0x0a7e_4a71);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            let generic_reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for num_vars in 0..=13 {
                let len = 1usize << num_vars;
                let tau = random_fields(&mut rng, &cfg, num_vars);
                let az: Vec<u64> = (0..len).map(|_| u64::from(rng.random::<u32>())).collect();
                let bz: Vec<u64> = (0..len).map(|_| u64::from(rng.random::<u32>())).collect();
                let cz: Vec<u64> = (0..len)
                    .map(|index| {
                        // Mostly satisfied rows, with some arbitrary residuals.
                        if rng.random::<u8>() < 200 {
                            az[index] * bz[index]
                        } else {
                            rng.random::<u64>()
                        }
                    })
                    .collect();
                let products = R1csProductMles {
                    az: dense(az),
                    bz: dense(bz),
                    cz: dense(cz),
                };
                let field_products = R1csProductMles {
                    az: dense(
                        products
                            .az
                            .evaluations
                            .iter()
                            .map(|&v| Field::from_with_cfg(v, &cfg))
                            .collect(),
                    ),
                    bz: dense(
                        products
                            .bz
                            .evaluations
                            .iter()
                            .map(|&v| Field::from_with_cfg(v, &cfg))
                            .collect(),
                    ),
                    cz: dense(
                        products
                            .cz
                            .evaluations
                            .iter()
                            .map(|&v| Field::from_with_cfg(v, &cfg))
                            .collect(),
                    ),
                };
                let claim = outer_claim(&cfg, &tau, &field_products);

                let mut generic_transcript = Blake3Transcript::new();
                let expected = prove_outer_sumcheck_u32_native_with_reducer(
                    &mut generic_transcript,
                    claim.clone(),
                    &tau,
                    make_equality_factors(&tau, &cfg).unwrap(),
                    products.clone(),
                    &cfg,
                    &generic_reducer,
                )
                .unwrap();

                let mut raw_transcript = Blake3Transcript::new();
                let (eq_low, eq_high) = make_equality_factors_raw(&ctx, &tau);
                let actual = prove_outer_native_raw(
                    &mut raw_transcript,
                    &ctx,
                    &reducer,
                    claim,
                    &tau,
                    eq_low,
                    eq_high,
                    NativeProducts::from_mles(&products),
                )
                .unwrap();
                assert_eq!(actual, expected, "num_vars={num_vars} modulus={modulus:#x}");
                assert_eq!(
                    next_challenge(&mut raw_transcript, &cfg),
                    next_challenge(&mut generic_transcript, &cfg)
                );
            }
        }
    }

    #[test]
    fn raw_inner_matches_generic_with_live_prefix() {
        let mut rng = StdRng::seed_from_u64(0x1aa3_11fe);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            let generic_reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for num_vars in 0..=14 {
                let len = 1usize << num_vars;
                for live in [
                    len,
                    len.div_ceil(2),
                    (len * 5).div_ceil(8),
                    rng.random_range(1..=len),
                ] {
                    let mut matrix = random_fields(&mut rng, &cfg, len);
                    let mut witness = random_fields(&mut rng, &cfg, len);
                    let mut native: Vec<u64> = (0..len).map(|_| rng.random::<u64>()).collect();
                    for index in live..len {
                        matrix[index] = Field::zero_with_cfg(&cfg);
                        witness[index] = Field::zero_with_cfg(&cfg);
                        native[index] = 0;
                    }
                    let claim = |witness: &[Field]| {
                        matrix
                            .iter()
                            .zip(witness)
                            .fold(Field::zero_with_cfg(&cfg), |sum, (m, w)| {
                                cfg.add(&(sum), &(&(cfg.mul(&(m.clone()), &(w)))))
                            })
                    };

                    // Field witness.
                    let field_claim = claim(&witness);
                    let mut generic_transcript = Blake3Transcript::new();
                    let expected = prove_inner_sumcheck_with_reducer(
                        &mut generic_transcript,
                        field_claim.clone(),
                        dense(matrix.clone()),
                        dense(witness.clone()),
                        &cfg,
                        &generic_reducer,
                    )
                    .unwrap();
                    let mut raw_transcript = Blake3Transcript::new();
                    let actual = prove_inner_raw(
                        &mut raw_transcript,
                        &ctx,
                        &reducer,
                        field_claim,
                        ctx.raw_vec(&matrix),
                        RawWitness::Field(ctx.raw_vec(&witness)),
                        live,
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "field num_vars={num_vars} live={live}");
                    assert_eq!(
                        next_challenge(&mut raw_transcript, &cfg),
                        next_challenge(&mut generic_transcript, &cfg)
                    );

                    // Native witness.
                    let native_field: Vec<Field> = native
                        .iter()
                        .map(|&v| Field::from_with_cfg(v, &cfg))
                        .collect();
                    let native_claim = claim(&native_field);
                    let mut generic_transcript = Blake3Transcript::new();
                    let expected = prove_inner_sumcheck_u32_native_with_reducer(
                        &mut generic_transcript,
                        native_claim.clone(),
                        dense(matrix.clone()),
                        dense(native.clone()),
                        &cfg,
                        &generic_reducer,
                    )
                    .unwrap();
                    let mut raw_transcript = Blake3Transcript::new();
                    let actual = prove_inner_raw(
                        &mut raw_transcript,
                        &ctx,
                        &reducer,
                        native_claim,
                        ctx.raw_vec(&matrix),
                        RawWitness::native_owned(native.clone()),
                        live,
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "native num_vars={num_vars} live={live}");
                    assert_eq!(
                        next_challenge(&mut raw_transcript, &cfg),
                        next_challenge(&mut generic_transcript, &cfg)
                    );
                }
            }
        }
    }

    fn random_field_matrices(
        rng: &mut StdRng,
        cfg: &FieldConfig,
        rows: usize,
        columns: usize,
    ) -> ConstraintMatrices<Field> {
        let mut matrix = || {
            let entries = (0..rows)
                .map(|_| {
                    let mut row: Vec<(usize, Field)> = Vec::new();
                    for column in 0..columns {
                        if rng.random::<u8>() < 96 {
                            let mut value = random_field(rng, cfg);
                            if <Field as crate::piop::spartan::SpartanField>::is_zero(&value) {
                                value = Field::one_with_cfg(cfg);
                            }
                            row.push((column, value));
                        }
                    }
                    row
                })
                .collect();
            SparseMatrix::try_from_rows(columns, entries).unwrap()
        };
        ConstraintMatrices::new(matrix(), matrix(), matrix()).unwrap()
    }

    fn assert_binding_matches<C>(
        ctx: &field::FpCtx<2>,
        rng: &mut StdRng,
        cfg: &FieldConfig,
        matrices: &PreparedConstraintMatrices<Field, C>,
    ) where
        C: SpartanMatrixCoefficient<Field> + RawMontyCoefficient,
    {
        let row_point = random_fields(rng, cfg, matrices.num_row_vars());
        let rho = random_field(rng, cfg);
        let expected = matrices.bind_and_batch(&row_point, &rho).unwrap();
        let row_weights = eq_table_raw(ctx, &ctx.raw_vec(&row_point));
        let actual = bind_and_batch_raw(ctx, matrices, &row_weights, ctx.raw(&rho));
        assert_eq!(actual, ctx.raw_vec(&expected.evaluations));

        for skip_vars in 1..=4usize {
            if skip_vars > matrices.num_row_vars() {
                break;
            }
            let binding = PrefixUnivariateRowBinding {
                skip_vars: skip_vars as u8,
                z: random_field(rng, cfg),
                tail_point: random_fields(rng, cfg, matrices.num_row_vars() - skip_vars),
            };
            let factors = binding.row_factors(matrices.num_row_vars(), cfg).unwrap();
            let expected = matrices
                .bind_and_batch_with_prefix_univariate_factors(&factors, &rho)
                .unwrap();
            let actual = bind_and_batch_prefix_raw(ctx, matrices, &factors, ctx.raw(&rho));
            assert_eq!(
                actual,
                ctx.raw_vec(&expected.evaluations),
                "skip_vars={skip_vars}"
            );
        }
    }

    #[test]
    fn raw_binding_matches_generic() {
        let mut rng = StdRng::seed_from_u64(0xb1_4d00);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);

            for (rows, columns) in [(1, 1), (5, 9), (16, 16), (37, 70), (300, 129)] {
                let prepared = PreparedConstraintMatrices::<Field, Field>::new(
                    random_field_matrices(&mut rng, &cfg, rows, columns),
                    &cfg,
                )
                .unwrap();
                assert_binding_matches(&ctx, &mut rng, &cfg, &prepared);
            }

            for multiplications in [1usize, 3, 256, 257, 1000, 4096, 5000] {
                let layout = U32MulLayout::new(multiplications).unwrap();
                let prepared = PreparedConstraintMatrices::<Field, bool>::new(
                    u32_mul_constraint_matrices(&layout, true).unwrap(),
                    &cfg,
                )
                .unwrap();
                assert!(prepared.selector_layout().is_some());
                assert_binding_matches(&ctx, &mut rng, &cfg, &prepared);

                let layout = BabyBearMulLayout::new(multiplications).unwrap();
                let prepared = PreparedConstraintMatrices::<Field, BabyBearMulCoefficient>::new(
                    baby_bear_mul_constraint_matrices(&layout).unwrap(),
                    &cfg,
                )
                .unwrap();
                assert_binding_matches(&ctx, &mut rng, &cfg, &prepared);
            }
        }
    }

    #[test]
    fn structured_inner_matches_dense() {
        let mut rng = StdRng::seed_from_u64(0x57ac_7ed0);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for &(block_log, blocks_log) in &[
                (1usize, 0usize),
                (1, 1),
                (1, 3),
                (2, 2),
                (3, 1),
                (6, 2),
                (8, 3),
                (11, 2),
                (12, 0),
            ] {
                let block_len = 1usize << block_log;
                let blocks = 1usize << blocks_log;
                let domain = block_len * blocks;
                let num_column_vars = block_log + blocks_log;
                for trial in 0..4 {
                    let rows = if trial == 0 {
                        block_len
                    } else {
                        rng.random_range(1..=block_len)
                    };
                    let mut scales: Vec<Option<Raw>> = (0..blocks)
                        .map(|_| {
                            (rng.random::<u8>() < 160)
                                .then(|| ctx.raw(&random_field(&mut rng, &cfg)))
                        })
                        .collect();
                    if scales.iter().all(Option::is_none) {
                        scales[blocks - 1] = Some(ctx.raw(&random_field(&mut rng, &cfg)));
                    }
                    // Every scaled block's run must lie inside the live prefix, as
                    // the relation's column count guarantees.
                    let min_live = scales
                        .iter()
                        .enumerate()
                        .filter(|(_, scale)| scale.is_some())
                        .map(|(block, _)| block * block_len + rows)
                        .max()
                        .unwrap();
                    let live = match trial {
                        0 => domain,
                        1 => rng.random_range(min_live..=domain),
                        _ => (min_live + rng.random_range(0..=block_len)).min(domain),
                    };
                    let weights: Vec<Raw> = (0..block_len)
                        .map(|_| ctx.raw(&random_field(&mut rng, &cfg)))
                        .collect();
                    let mut dense = vec![0 as Raw; domain];
                    for (block, scale) in scales.iter().enumerate() {
                        if let Some(scale) = scale {
                            for (row, weight) in weights[..rows].iter().enumerate() {
                                dense[block * block_len + row] = ctx.mul_raw(*scale, *weight);
                            }
                        }
                    }
                    let mut native: Vec<u64> = (0..domain).map(|_| rng.random()).collect();
                    let mut field: Vec<Raw> = (0..domain)
                        .map(|_| ctx.raw(&random_field(&mut rng, &cfg)))
                        .collect();
                    for index in live..domain {
                        native[index] = 0;
                        field[index] = 0;
                    }
                    if trial % 2 == 0 {
                        // The constant block of the generated relations: zero
                        // beyond entry 0.
                        for index in 1..block_len.min(live) {
                            native[index] = 0;
                            field[index] = 0;
                        }
                    }
                    let scaled = BlockScales {
                        block_len,
                        rows,
                        scales: scales.clone(),
                    };
                    let field_claim = dense.iter().zip(&field).fold(0 as Raw, |sum, (&d, &w)| {
                        ctx.add_raw(sum, ctx.mul_raw(d, w))
                    });
                    let native_claim = dense.iter().zip(&native).fold(0 as Raw, |sum, (&d, &w)| {
                        ctx.add_raw(sum, ctx.mul_raw(d, ctx.native_residue(w)))
                    });

                    for kind in 0..2 {
                        let (claim, dense_witness, structured_witness) = if kind == 0 {
                            (
                                crate::utils::delayed_reduction::element(&cfg, native_claim),
                                RawWitness::native_owned(native.clone()),
                                RawWitness::native_owned(native.clone()),
                            )
                        } else {
                            (
                                crate::utils::delayed_reduction::element(&cfg, field_claim),
                                RawWitness::Field(field.clone()),
                                RawWitness::Field(field.clone()),
                            )
                        };
                        let mut dense_transcript = Blake3Transcript::new();
                        let expected = prove_inner_raw(
                            &mut dense_transcript,
                            &ctx,
                            &reducer,
                            claim.clone(),
                            dense.clone(),
                            dense_witness,
                            live,
                        )
                        .unwrap();
                        let mut structured_transcript = Blake3Transcript::new();
                        let actual = prove_inner_structured_raw(
                            &mut structured_transcript,
                            &ctx,
                            &reducer,
                            claim,
                            &weights,
                            &scaled,
                            structured_witness,
                            live,
                            num_column_vars,
                        )
                        .unwrap();
                        assert_eq!(
                            actual, expected,
                            "kind={kind} block_log={block_log} blocks_log={blocks_log} trial={trial} rows={rows} live={live}"
                        );
                        assert_eq!(
                            next_challenge(&mut structured_transcript, &cfg),
                            next_challenge(&mut dense_transcript, &cfg)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn block_selector_layouts_are_detected_for_the_generated_relations() {
        let cfg = config(MODULI[0]);
        for multiplications in [1usize, 3, 255, 256, 257, 1000, 4096, 5000] {
            let layout = U32MulLayout::new(multiplications).unwrap();
            let matrices = u32_mul_constraint_matrices(&layout, true).unwrap();
            let prepared =
                PreparedConstraintMatrices::<Field, bool>::new(matrices.clone(), &cfg).unwrap();
            let detected = prepared
                .block_selector()
                .expect("u32 selectors are block selectors");
            assert_eq!(detected.rows, multiplications);
            assert_eq!(detected.block_len, layout.capacity());
            assert_eq!(detected.a.len(), 1);
            assert_eq!(detected.a[0].start, layout.capacity());
            assert_eq!(detected.b[0].start, 2 * layout.capacity());
            assert_eq!(detected.c[0].start, 3 * layout.capacity());
            assert!(
                detected.a[0].coefficient && detected.b[0].coefficient && detected.c[0].coefficient
            );
            let skeleton = ConstraintMatricesSkeleton::<Field, bool>::new(matrices).unwrap();
            let from_skeleton = PreparedConstraintMatrices::from_skeleton(&skeleton, &cfg).unwrap();
            let replayed = from_skeleton.block_selector().unwrap();
            assert_eq!(replayed.rows, detected.rows);
            assert_eq!(replayed.block_len, detected.block_len);
            assert_eq!(replayed.c[0].start, detected.c[0].start);

            let layout = BabyBearMulLayout::new(multiplications).unwrap();
            let matrices = baby_bear_mul_constraint_matrices(&layout).unwrap();
            let prepared = PreparedConstraintMatrices::<Field, BabyBearMulCoefficient>::new(
                matrices.clone(),
                &cfg,
            )
            .unwrap();
            let detected = prepared
                .block_selector()
                .expect("BabyBear selectors are block selectors");
            assert_eq!(detected.rows, multiplications);
            assert_eq!(detected.block_len, layout.capacity());
            assert_eq!(detected.c.len(), 2);
            assert_eq!(detected.c[0].start, 3 * layout.capacity());
            assert_eq!(detected.c[0].coefficient, BabyBearMulCoefficient::One);
            assert_eq!(detected.c[1].start, 4 * layout.capacity());
            assert_eq!(detected.c[1].coefficient, BabyBearMulCoefficient::Modulus);
            let skeleton =
                ConstraintMatricesSkeleton::<Field, BabyBearMulCoefficient>::new(matrices).unwrap();
            let from_skeleton = PreparedConstraintMatrices::from_skeleton(&skeleton, &cfg).unwrap();
            let replayed = from_skeleton.block_selector().unwrap();
            assert_eq!(replayed.c[1].start, detected.c[1].start);
            assert_eq!(replayed.c[1].coefficient, BabyBearMulCoefficient::Modulus);
        }

        // General matrices are not block selectors.
        let mut rng = StdRng::seed_from_u64(0x0b10_c5e1);
        let prepared = PreparedConstraintMatrices::<Field, Field>::new(
            random_field_matrices(&mut rng, &cfg, 37, 70),
            &cfg,
        )
        .unwrap();
        assert!(prepared.block_selector().is_none());

        // A run starting at column zero: one block spanning the domain.
        let rows = 5;
        let identity = SparseMatrix::try_from_rows(
            8,
            (0..rows)
                .map(|row| vec![(row, Field::one_with_cfg(&cfg))])
                .collect(),
        )
        .unwrap();
        let empty = SparseMatrix::<Field>::try_from_rows(8, vec![Vec::new(); rows]).unwrap();
        let prepared = PreparedConstraintMatrices::<Field, Field>::new(
            ConstraintMatrices::new(identity, empty.clone(), empty).unwrap(),
            &cfg,
        )
        .unwrap();
        let detected = prepared.block_selector().unwrap();
        assert_eq!((detected.rows, detected.block_len), (rows, 8));
        assert_eq!(detected.a[0].start, 0);
        assert!(detected.b.is_empty() && detected.c.is_empty());
    }

    #[test]
    fn raw_skip_message_and_prefix_fold_match_generic() {
        let mut rng = StdRng::seed_from_u64(0x5c1b_0000);
        for modulus in MODULI {
            let cfg = config(modulus);
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            let generic_reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for num_vars in 1..=13 {
                let len = 1usize << num_vars;
                let az: Vec<u64> = (0..len).map(|_| u64::from(rng.random::<u32>())).collect();
                let bz: Vec<u64> = (0..len).map(|_| u64::from(rng.random::<u32>())).collect();
                let cz: Vec<u64> = (0..len)
                    .map(|index| {
                        if rng.random::<u8>() < 200 {
                            az[index] * bz[index]
                        } else {
                            rng.random::<u64>()
                        }
                    })
                    .collect();
                let mles = R1csProductMles {
                    az: dense(az.clone()),
                    bz: dense(bz.clone()),
                    cz: dense(cz.clone()),
                };
                let products = NativeProducts {
                    az: &az,
                    bz: &bz,
                    cz: &cz,
                };
                for skip_vars in 1..=4usize.min(num_vars) {
                    let tau_tail = random_fields(&mut rng, &cfg, num_vars - skip_vars);
                    let factors = make_equality_factors(&tau_tail, &cfg).unwrap();
                    let expected = compute_u32_native_skip_message_validated(
                        skip_vars,
                        &factors,
                        &mles,
                        &cfg,
                        &generic_reducer,
                    )
                    .unwrap();
                    let (eq_low, eq_high) = make_equality_factors_raw(&ctx, &tau_tail);
                    let actual = compute_u32_native_skip_message_raw(
                        &cfg, skip_vars, &eq_low, &eq_high, products, &ctx, &reducer,
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "skip message K={skip_vars} n={num_vars}");

                    let z = random_field(&mut rng, &cfg);
                    let expected = fold_u32_native_prefix_validated(
                        skip_vars,
                        mles.clone(),
                        &z,
                        &cfg,
                        &generic_reducer,
                    )
                    .unwrap();
                    let actual = fold_u32_native_prefix_raw(skip_vars, products, &z, &ctx).unwrap();
                    assert_eq!(ctx.raw_vec(&expected.az.evaluations), actual.az);
                    assert_eq!(ctx.raw_vec(&expected.bz.evaluations), actual.bz);
                    assert_eq!(ctx.raw_vec(&expected.cz.evaluations), actual.cz);
                }
            }
        }
    }
}
