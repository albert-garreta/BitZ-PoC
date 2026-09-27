//! The product-slot sumcheck of a linear relation whose rows also carry
//! product terms (SHA-256's `Ch`, one slot per round and bit).
//!
//! A row of such a relation reads `(C h)_j + Σ_s K[j, s] · h[l_s] ·
//! (h[p_s] − h[m_s]) = 0`. Once the local rows are collapsed by `ξ`
//! (`β = Cᵀ eq(·, ξ)`, `κ = Kᵀ eq(·, ξ)`) and the instances by `η`, the
//! batched residual is
//!
//! `μ = Σ_i eq(η, i) · [ D(i) + Σ_s Ê(i, s) · F(i, s) ]`,
//!
//! with `D(i) = ⟨β, h_i⟩` the collapsed linear part of instance `i`,
//! `Ê(i, s) = κ_s · h_i[l_s]` and `F(i, s) = h_i[p_s] − h_i[m_s]`. This
//! sumcheck runs over the instance variables and then the slot variables of
//!
//! `P(i, s) = eq(η, i) · [ D(i) · [s = 0] + Ê(i, s) · F(i, s) ]`
//!
//! (cubic in the instance variables, quadratic in the slot variables) and
//! ends at one point `(r_i, r_s)` with the three claims `D(r_i)`,
//! `Ê(r_i, r_s)` and `F(r_i, r_s)`. Each is a linear functional of `h` whose
//! instance factor is `eq(r_i, ·)`, so a random combination of them is again
//! one rank-one functional over the opened grid: the claim the openers
//! already discharge, now at instance point `r_i`.

use field::{RingOps, Uint};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{FieldConfig, ProtocolError, SpartanBitzField};
use crate::piop::spartan::{
    absorb_field_elements,
    grinding::GrindingDomain,
    matrix::eq_table,
    piop::SpartanError,
    squeeze_field,
    sumcheck::{RoundBoundaryPolicy, SumcheckProof, eq_eval, validate_field_elements},
};
use crate::transcript::traits::Transcript;

type F = SpartanBitzField;

/// Proof-of-work domain of the product sumcheck's round boundaries.
pub(crate) struct LinearProductGrinding;

impl GrindingDomain for LinearProductGrinding {
    const DOMAIN: &'static [u8] = b"bitz/spartan-linear/grinding/product-slots/v1";
}

/// The product-slot sumcheck: `instance_vars + slot_vars` round messages
/// (cubic coefficient form), their grinding nonces, and the three terminal
/// evaluations `[D(r_i), Ê(r_i, r_s), F(r_i, r_s)]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSumcheckProof {
    pub(crate) rounds: SumcheckProof<F, 4>,
    pub(crate) nonces: Vec<u64>,
    pub(crate) terminal: [F; 3],
}

impl ProductSumcheckProof {
    pub const fn rounds(&self) -> &SumcheckProof<F, 4> {
        &self.rounds
    }

    pub fn nonces(&self) -> &[u64] {
        &self.nonces
    }

    pub const fn terminal(&self) -> &[F; 3] {
        &self.terminal
    }

    pub fn rounds_mut(&mut self) -> &mut SumcheckProof<F, 4> {
        &mut self.rounds
    }

    pub fn terminal_mut(&mut self) -> &mut [F; 3] {
        &mut self.terminal
    }

    /// Serialized payload: 16-byte coefficients, 8-byte nonces.
    pub fn bytes(&self) -> usize {
        16 * (4 * self.rounds.round_polynomials.len() + 3) + 8 * self.nonces.len()
    }
}

/// The local product slots: slot `s` is `h[left_s] · (h[plus_s] − h[minus_s])`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductSlots<'a> {
    pub left: &'a [u32],
    pub right: &'a [(u32, u32)],
}

/// Per-instance read access to the opened assignment's bits.
pub(crate) trait InstanceBits: Sync {
    /// Instance slots of the batch (a power of two); instances past the
    /// live count read as zero.
    fn instance_capacity(&self) -> usize;

    /// Words per instance: local column `c` is bit `c % 64` of word `c / 64`.
    fn words_per_instance(&self) -> usize;

    /// Copies instance `i`'s local bits into `out` (`words_per_instance` words).
    fn read_local_words(&self, instance: usize, out: &mut [u64]);
}

#[inline]
fn bit(words: &[u64], column: u32) -> i32 {
    ((words[(column / 64) as usize] >> (column % 64)) & 1) as i32
}

/// Canonical residues of `values` as `u128` (the SHA field is below 2^115).
fn canonical_u128(config: &FieldConfig, values: &[F]) -> Vec<u128> {
    values
        .iter()
        .map(|value| u128::from(config.to_integer(value)))
        .collect()
}

fn from_u128(config: &FieldConfig, modulus: u128, value: u128) -> F {
    config.from_canonical_integer(&Uint::<2>::from(value % modulus))
}

fn from_i128(config: &FieldConfig, modulus: u128, value: i128) -> F {
    let magnitude = from_u128(config, modulus, value.unsigned_abs());
    if value < 0 {
        config.neg(&magnitude)
    } else {
        magnitude
    }
}

/// `D(i) = ⟨β, h_i⟩` for every instance slot, from nibble tables of `β`.
fn instance_linear_values(
    config: &FieldConfig,
    modulus: u128,
    local_coefficients: &[F],
    bits: &dyn InstanceBits,
) -> Vec<F> {
    let words = bits.words_per_instance();
    let beta = canonical_u128(config, local_coefficients);
    let nibbles = local_coefficients.len().div_ceil(4);
    assert!(nibbles <= 16 * words, "the local coefficients outrun the local bits");
    // table[4 n + v] for nibble n and value v: Σ of β over v's set bits,
    // each entry reduced, so 4096 of them still fit a u128 sum.
    let mut table = vec![0u128; 16 * nibbles];
    for nibble in 0..nibbles {
        for value in 1..16usize {
            let low = value.trailing_zeros() as usize;
            let column = 4 * nibble + low;
            let term = beta.get(column).copied().unwrap_or(0);
            let entry = (table[16 * nibble + (value & (value - 1))] + term) % modulus;
            table[16 * nibble + value] = entry;
        }
    }
    assert!(nibbles as u128 * modulus < u128::MAX / 2);
    let one_instance = |instance: usize| {
        let mut local = vec![0u64; words];
        bits.read_local_words(instance, &mut local);
        let mut sum = 0u128;
        for nibble in 0..nibbles {
            let value = (local[nibble / 16] >> (4 * (nibble % 16))) & 0xf;
            sum += table[16 * nibble + value as usize];
        }
        from_u128(config, modulus, sum)
    };
    #[cfg(feature = "parallel")]
    {
        (0..bits.instance_capacity())
            .into_par_iter()
            .map(one_instance)
            .collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        (0..bits.instance_capacity()).map(one_instance).collect()
    }
}

/// `scale · eq(η_j, X) · (l0 + (l1 − l0 − l∞) X + l∞ X²)` in coefficient form.
fn instance_round_polynomial(
    config: &FieldConfig,
    scale: &F,
    eta: &F,
    [l0, l1, linf]: [F; 3],
) -> [F; 4] {
    let one = config.one();
    let a = config.sub(&one, eta);
    let b = config.sub(&config.add(eta, eta), &one);
    let m1 = config.sub(&config.sub(&l1, &l0), &linf);
    let coefficients = [
        config.mul(&a, &l0),
        config.add(&config.mul(&a, &m1), &config.mul(&b, &l0)),
        config.add(&config.mul(&a, &linf), &config.mul(&b, &m1)),
        config.mul(&b, &linf),
    ];
    coefficients.map(|c| config.mul(scale, &c))
}

fn evaluate(config: &FieldConfig, coefficients: &[F; 4], x: &F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(config.zero(), |acc, c| config.add(&config.mul(&acc, x), c))
}

fn sum3(config: &FieldConfig, a: [F; 3], b: [F; 3]) -> [F; 3] {
    [
        config.add(&a[0], &b[0]),
        config.add(&a[1], &b[1]),
        config.add(&a[2], &b[2]),
    ]
}

/// Absorbs one round message, runs the boundary, draws the challenge.
fn round_challenge<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    coefficients: &[F; 4],
    round: usize,
    boundary: &mut B,
) -> Result<F, ProtocolError> {
    absorb_field_elements(transcript, coefficients, config);
    boundary
        .after_round(transcript, round)
        .map_err(SpartanError::from)?;
    Ok(squeeze_field(transcript, config).map_err(SpartanError::from)?)
}

/// What the prover returns: the round messages, the terminal evaluations,
/// and the point `(r_i, r_s)`.
pub(crate) struct ProductProverOutput {
    pub rounds: SumcheckProof<F, 4>,
    pub terminal: [F; 3],
    pub instance_point: Vec<F>,
    pub slot_point: Vec<F>,
}

/// Proves `μ = Σ_{i, s} P(i, s)` (see the module documentation).
///
/// `slot_weights` is `κ` padded to a power of two; slots past
/// `slots.left.len()` carry zero weight and zero factors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_product_sumcheck<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    instance_point: &[F],
    local_coefficients: &[F],
    slot_weights: &[F],
    slots: ProductSlots<'_>,
    bits: &dyn InstanceBits,
    claim: &F,
    boundary: &mut B,
) -> Result<ProductProverOutput, ProtocolError> {
    let instance_vars = instance_point.len();
    let slot_count = slot_weights.len();
    if !slot_count.is_power_of_two()
        || slots.left.len() != slots.right.len()
        || slots.left.len() > slot_count
        || bits.instance_capacity() != 1usize << instance_vars
        || instance_vars == 0
    {
        return Err(ProtocolError::InvalidGeometry);
    }
    let slot_vars = slot_count.ilog2() as usize;
    boundary
        .validate(instance_vars + slot_vars)
        .map_err(SpartanError::from)?;
    let modulus = u128::from(*config.modulus());
    // Round 0 sums `|κ_s · c| <= 2 q` over the slots in an i128, and the
    // nibble tables sum up to `2^12` residues in a u128.
    if modulus >> 115 != 0 || (2 * slots.left.len() as u128 + 1).saturating_mul(modulus) >> 126 != 0 {
        return Err(ProtocolError::InvalidGeometry);
    }
    let zero = config.zero();
    let one = config.one();
    let words = bits.words_per_instance();
    let live_slots = slots.left.len();

    let mut d = {
        let _scope = tracing::info_span!("product:instance_linear_values").entered();
        instance_linear_values(config, modulus, local_coefficients, bits)
    };
    let mut rounds = Vec::with_capacity(instance_vars + slot_vars);
    let mut point = Vec::with_capacity(instance_vars + slot_vars);
    let mut current = claim.clone();
    let mut scale = one.clone();

    // Round 0 reads the bits directly: `E ∈ {0, 1}` and `F ∈ {−1, 0, 1}`,
    // so every slot contributes a small multiple of κ, summed exactly.
    let kappa = canonical_u128(config, slot_weights);
    let (ehat, fvals) = {
        let _scope = tracing::info_span!("product:round0").entered();
        let rest = eq_table(&instance_point[1..], config).map_err(SpartanError::from)?;
        let half = 1usize << (instance_vars - 1);
        let pair_sums = |p: usize| -> [F; 3] {
            let mut w0 = vec![0u64; words];
            let mut w1 = vec![0u64; words];
            bits.read_local_words(2 * p, &mut w0);
            bits.read_local_words(2 * p + 1, &mut w1);
            let (mut a0, mut a1, mut ainf) = (0i128, 0i128, 0i128);
            for slot in 0..live_slots {
                let l = slots.left[slot];
                let (plus, minus) = slots.right[slot];
                let (e0, e1) = (bit(&w0, l), bit(&w1, l));
                let f0 = bit(&w0, plus) - bit(&w0, minus);
                let f1 = bit(&w1, plus) - bit(&w1, minus);
                let weight = kappa[slot] as i128;
                a0 += weight * i128::from(e0 * f0);
                a1 += weight * i128::from(e1 * f1);
                ainf += weight * i128::from((e1 - e0) * (f1 - f0));
            }
            let r = &rest[p];
            [
                config.mul(r, &config.add(&d[2 * p], &from_i128(config, modulus, a0))),
                config.mul(r, &config.add(&d[2 * p + 1], &from_i128(config, modulus, a1))),
                config.mul(r, &from_i128(config, modulus, ainf)),
            ]
        };
        #[cfg(feature = "parallel")]
        let sums = (0..half)
            .into_par_iter()
            .map(pair_sums)
            .reduce(|| [zero, zero, zero], |a, b| sum3(config, a, b));
        #[cfg(not(feature = "parallel"))]
        let sums = (0..half)
            .map(pair_sums)
            .fold([zero, zero, zero], |a, b| sum3(config, a, b));
        let coefficients = instance_round_polynomial(config, &scale, &instance_point[0], sums);
        debug_assert_eq!(
            config.add(&coefficients[0], &evaluate(config, &coefficients, &one)),
            current
        );
        let r = round_challenge(transcript, config, &coefficients, 0, boundary)?;
        current = evaluate(config, &coefficients, &r);
        let factor = config.add(
            &config.sub(&one, &instance_point[0]),
            &config.mul(&r, &config.sub(&config.add(&instance_point[0], &instance_point[0]), &one)),
        );
        scale = config.mul(&scale, &factor);
        rounds.push(coefficients);
        point.push(r);

        // Fold round 0: Ê' = κ (e0 + r (e1 − e0)) takes one of κ·{0, r,
        // 1 − r, 1}; F' = f0 + r (f1 − f0) one of fifteen values.
        let one_minus_r = config.sub(&one, &r);
        let scaled = slot_weights
            .iter()
            .map(|weight| {
                [
                    zero,
                    config.mul(weight, &r),
                    config.mul(weight, &one_minus_r),
                    *weight,
                ]
            })
            .collect::<Vec<_>>();
        let mut f_table = [zero; 15];
        for f0 in -1i32..=1 {
            for delta in -2i32..=2 {
                let base = from_i128(config, modulus, i128::from(f0));
                let step = from_i128(config, modulus, i128::from(delta));
                f_table[((f0 + 1) * 5 + delta + 2) as usize] =
                    config.add(&base, &config.mul(&step, &r));
            }
        }
        let mut ehat = vec![zero; half * slot_count];
        let mut fvals = vec![zero; half * slot_count];
        let fold_pair = |(p, (ehat_row, f_row)): (usize, (&mut [F], &mut [F]))| {
            let mut w0 = vec![0u64; words];
            let mut w1 = vec![0u64; words];
            bits.read_local_words(2 * p, &mut w0);
            bits.read_local_words(2 * p + 1, &mut w1);
            for slot in 0..live_slots {
                let l = slots.left[slot];
                let (plus, minus) = slots.right[slot];
                let code = (bit(&w0, l) | (bit(&w1, l) << 1)) as usize;
                // (e0, e1) = (0,0) → 0, (0,1) → r, (1,0) → 1 − r, (1,1) → 1.
                ehat_row[slot] = scaled[slot][[0, 2, 1, 3][code]];
                let f0 = bit(&w0, plus) - bit(&w0, minus);
                let f1 = bit(&w1, plus) - bit(&w1, minus);
                f_row[slot] = f_table[((f0 + 1) * 5 + (f1 - f0) + 2) as usize];
            }
        };
        #[cfg(feature = "parallel")]
        ehat.par_chunks_mut(slot_count)
            .zip(fvals.par_chunks_mut(slot_count))
            .enumerate()
            .for_each(fold_pair);
        #[cfg(not(feature = "parallel"))]
        ehat.chunks_mut(slot_count)
            .zip(fvals.chunks_mut(slot_count))
            .enumerate()
            .for_each(fold_pair);
        fold_in_place(config, &mut d, &point[0]);
        (ehat, fvals)
    };

    // Instance rounds 1.. over materialized field tables, one row per
    // remaining instance.
    let (mut ehat, mut fvals) = (ehat, fvals);
    for round in 1..instance_vars {
        let _scope = tracing::info_span!("product:instance_round").entered();
        let rest = eq_table(&instance_point[round + 1..], config).map_err(SpartanError::from)?;
        let half = d.len() / 2;
        let pair_sums = |p: usize| -> [F; 3] {
            let (e0, e1) = (
                &ehat[2 * p * slot_count..(2 * p + 1) * slot_count],
                &ehat[(2 * p + 1) * slot_count..(2 * p + 2) * slot_count],
            );
            let (f0, f1) = (
                &fvals[2 * p * slot_count..(2 * p + 1) * slot_count],
                &fvals[(2 * p + 1) * slot_count..(2 * p + 2) * slot_count],
            );
            let (mut a0, mut a1, mut ainf) = (zero, zero, zero);
            for slot in 0..live_slots {
                a0 = config.add(&a0, &config.mul(&e0[slot], &f0[slot]));
                a1 = config.add(&a1, &config.mul(&e1[slot], &f1[slot]));
                let de = config.sub(&e1[slot], &e0[slot]);
                let df = config.sub(&f1[slot], &f0[slot]);
                ainf = config.add(&ainf, &config.mul(&de, &df));
            }
            let r = &rest[p];
            [
                config.mul(r, &config.add(&d[2 * p], &a0)),
                config.mul(r, &config.add(&d[2 * p + 1], &a1)),
                config.mul(r, &ainf),
            ]
        };
        #[cfg(feature = "parallel")]
        let sums = (0..half)
            .into_par_iter()
            .map(pair_sums)
            .reduce(|| [zero, zero, zero], |a, b| sum3(config, a, b));
        #[cfg(not(feature = "parallel"))]
        let sums = (0..half)
            .map(pair_sums)
            .fold([zero, zero, zero], |a, b| sum3(config, a, b));
        let coefficients =
            instance_round_polynomial(config, &scale, &instance_point[round], sums);
        debug_assert_eq!(
            config.add(&coefficients[0], &evaluate(config, &coefficients, &one)),
            current
        );
        let r = round_challenge(transcript, config, &coefficients, round, boundary)?;
        current = evaluate(config, &coefficients, &r);
        let eta = &instance_point[round];
        let factor = config.add(
            &config.sub(&one, eta),
            &config.mul(&r, &config.sub(&config.add(eta, eta), &one)),
        );
        scale = config.mul(&scale, &factor);
        ehat = fold_rows(config, &ehat, slot_count, &r);
        fvals = fold_rows(config, &fvals, slot_count, &r);
        fold_in_place(config, &mut d, &r);
        rounds.push(coefficients);
        point.push(r);
    }

    // Slot rounds on the last row: `scale · [D · [s = 0] + Ê(s) F(s)]`.
    let d_star = d[0];
    let mut prefix = one;
    for round in 0..slot_vars {
        let half = ehat.len() / 2;
        let (mut c0, mut c1, mut c2) = (zero, zero, zero);
        for q in 0..half {
            let (e0, e1) = (&ehat[2 * q], &ehat[2 * q + 1]);
            let (f0, f1) = (&fvals[2 * q], &fvals[2 * q + 1]);
            let de = config.sub(e1, e0);
            let df = config.sub(f1, f0);
            c0 = config.add(&c0, &config.mul(e0, f0));
            c1 = config.add(
                &c1,
                &config.add(&config.mul(e0, &df), &config.mul(&de, f0)),
            );
            c2 = config.add(&c2, &config.mul(&de, &df));
        }
        let linear = config.mul(&d_star, &prefix);
        let coefficients = [
            config.mul(&scale, &config.add(&linear, &c0)),
            config.mul(&scale, &config.sub(&c1, &linear)),
            config.mul(&scale, &c2),
            zero,
        ];
        debug_assert_eq!(
            config.add(&coefficients[0], &evaluate(config, &coefficients, &one)),
            current
        );
        let r = round_challenge(
            transcript,
            config,
            &coefficients,
            instance_vars + round,
            boundary,
        )?;
        current = evaluate(config, &coefficients, &r);
        prefix = config.mul(&prefix, &config.sub(&one, &r));
        ehat = fold_rows(config, &ehat, 1, &r);
        fvals = fold_rows(config, &fvals, 1, &r);
        rounds.push(coefficients);
        point.push(r);
    }
    let terminal = [d_star, ehat[0], fvals[0]];
    debug_assert_eq!(
        config.mul(
            &scale,
            &config.add(&config.mul(&d_star, &prefix), &config.mul(&ehat[0], &fvals[0]))
        ),
        current
    );
    let slot_point = point.split_off(instance_vars);
    Ok(ProductProverOutput {
        rounds: SumcheckProof {
            round_polynomials: rounds,
        },
        terminal,
        instance_point: point,
        slot_point,
    })
}

/// Folds the lowest variable of a table of `rows` rows of `row_len`
/// entries: row `p` of the result is `row(2p) + r (row(2p+1) − row(2p))`.
fn fold_rows(config: &FieldConfig, table: &[F], row_len: usize, r: &F) -> Vec<F> {
    let half = table.len() / (2 * row_len);
    let mut out = vec![config.zero(); half * row_len];
    let fold = |(p, row): (usize, &mut [F])| {
        let (low, high) = (
            &table[2 * p * row_len..(2 * p + 1) * row_len],
            &table[(2 * p + 1) * row_len..(2 * p + 2) * row_len],
        );
        for ((out, low), high) in row.iter_mut().zip(low).zip(high) {
            *out = config.add(low, &config.mul(r, &config.sub(high, low)));
        }
    };
    #[cfg(feature = "parallel")]
    out.par_chunks_mut(row_len).enumerate().for_each(fold);
    #[cfg(not(feature = "parallel"))]
    out.chunks_mut(row_len).enumerate().for_each(fold);
    out
}

fn fold_in_place(config: &FieldConfig, values: &mut Vec<F>, r: &F) {
    let half = values.len() / 2;
    for p in 0..half {
        let (low, high) = (values[2 * p], values[2 * p + 1]);
        values[p] = config.add(&low, &config.mul(r, &config.sub(&high, &low)));
    }
    values.truncate(half);
}

/// Verifies the rounds and the terminal identity
/// `eq(η, r_i) · (D* · Π_l (1 − r_s,l) + Ê* · F*)`, returning `(r_i, r_s)`.
pub(crate) fn verify_product_sumcheck<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    proof: &ProductSumcheckProof,
    instance_point: &[F],
    slot_vars: usize,
    claim: &F,
    boundary: &mut B,
) -> Result<(Vec<F>, Vec<F>), ProtocolError> {
    let instance_vars = instance_point.len();
    if instance_vars == 0 {
        return Err(ProtocolError::InvalidGeometry);
    }
    validate_field_elements(&proof.terminal, config).map_err(SpartanError::from)?;
    let (mut point, last) = proof
        .rounds
        .verify_with_round_boundary(
            transcript,
            claim.clone(),
            instance_vars + slot_vars,
            config,
            boundary,
        )
        .map_err(SpartanError::from)?;
    let slot_point = point.split_off(instance_vars);
    let scale = eq_eval(instance_point, &point, config).map_err(SpartanError::from)?;
    let one = config.one();
    let prefix = slot_point
        .iter()
        .fold(one, |acc, r| config.mul(&acc, &config.sub(&one, r)));
    let [d, ehat, f] = &proof.terminal;
    let expected = config.mul(
        &scale,
        &config.add(&config.mul(d, &prefix), &config.mul(ehat, f)),
    );
    if expected != last {
        return Err(ProtocolError::InvalidProductTerminalClaim);
    }
    Ok((point, slot_point))
}

/// The local weights of the reduced claim and its value:
/// `w = β + γ₁ Σ_s eq(r_s, s) κ_s e_{l_s} + γ₂ Σ_s eq(r_s, s) (e_{p_s} − e_{m_s})`,
/// `v = D* + γ₁ Ê* + γ₂ F*`.
pub(crate) fn reduced_local_weights(
    config: &FieldConfig,
    local_coefficients: &[F],
    slot_weights: &[F],
    slots: ProductSlots<'_>,
    slot_point: &[F],
    terminal: &[F; 3],
    [g1, g2]: [F; 2],
) -> Result<(Vec<F>, F), ProtocolError> {
    let eq_slot = eq_table(slot_point, config).map_err(SpartanError::from)?;
    if eq_slot.len() != slot_weights.len() || slots.left.len() > slot_weights.len() {
        return Err(ProtocolError::InvalidGeometry);
    }
    let mut weights = local_coefficients.to_vec();
    let mut add = |column: u32, value: &F| -> Result<(), ProtocolError> {
        let entry = weights
            .get_mut(column as usize)
            .ok_or(ProtocolError::InvalidGeometry)?;
        *entry = config.add(entry, value);
        Ok(())
    };
    for slot in 0..slots.left.len() {
        let left = config.mul(&g1, &config.mul(&eq_slot[slot], &slot_weights[slot]));
        let right = config.mul(&g2, &eq_slot[slot]);
        let (plus, minus) = slots.right[slot];
        add(slots.left[slot], &left)?;
        add(plus, &right)?;
        add(minus, &config.neg(&right))?;
    }
    let [d, ehat, f] = terminal;
    let value = config.add(
        d,
        &config.add(&config.mul(&g1, ehat), &config.mul(&g2, f)),
    );
    Ok((weights, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcs::FQ_MOD;
    use crate::piop::spartan::SpartanField;
    use crate::sumcheck::UngrindedRoundBoundary;
    use crate::transcript::Blake3Transcript;

    struct Rows {
        words: usize,
        instances: Vec<Vec<u64>>,
    }

    impl InstanceBits for Rows {
        fn instance_capacity(&self) -> usize {
            self.instances.len()
        }

        fn words_per_instance(&self) -> usize {
            self.words
        }

        fn read_local_words(&self, instance: usize, out: &mut [u64]) {
            out.copy_from_slice(&self.instances[instance]);
        }
    }

    fn next(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state >> 11
    }

    fn element(config: &FieldConfig, value: u64) -> F {
        F::from_with_cfg(value, config)
    }

    /// `Σ_i eq(point, i) Σ_c weights[c] h_i[c]`, directly.
    fn functional(config: &FieldConfig, point: &[F], weights: &[F], rows: &Rows) -> F {
        let eq = eq_table(point, config).unwrap();
        let mut total = config.zero();
        for (instance, words) in rows.instances.iter().enumerate() {
            for (column, weight) in weights.iter().enumerate() {
                if bit(words, column as u32) == 1 {
                    total = config.add(&total, &config.mul(&eq[instance], weight));
                }
            }
        }
        total
    }

    #[test]
    fn product_sumcheck_roundtrips_and_reduces_to_one_rank_one_functional() {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let mut seed = 0x5eed_u64;
        let (instance_vars, columns, live_slots, slot_count) = (3usize, 128usize, 6usize, 8usize);
        let rows = Rows {
            words: columns / 64,
            instances: (0..1 << instance_vars)
                .map(|_| (0..columns / 64).map(|_| next(&mut seed) ^ (next(&mut seed) << 40)).collect())
                .collect(),
        };
        let eta = (0..instance_vars)
            .map(|_| element(&config, next(&mut seed)))
            .collect::<Vec<_>>();
        let beta = (0..columns - 3)
            .map(|_| element(&config, next(&mut seed)))
            .collect::<Vec<_>>();
        let kappa = (0..slot_count)
            .map(|_| element(&config, next(&mut seed)))
            .collect::<Vec<_>>();
        let left = (0..live_slots)
            .map(|_| (next(&mut seed) % (columns as u64 - 3)) as u32)
            .collect::<Vec<_>>();
        let right = (0..live_slots)
            .map(|s| (3 * s as u32 + 1, 3 * s as u32 + 2))
            .collect::<Vec<_>>();
        let slots = ProductSlots {
            left: &left,
            right: &right,
        };

        // μ = Σ_i eq(η, i) [⟨β, h_i⟩ + Σ_s κ_s h_i[l_s] (h_i[p_s] − h_i[m_s])].
        let eq_eta = eq_table(&eta, &config).unwrap();
        let mut claim = functional(&config, &eta, &beta, &rows);
        for (instance, words) in rows.instances.iter().enumerate() {
            for slot in 0..live_slots {
                let (plus, minus) = right[slot];
                let value = bit(words, left[slot]) * (bit(words, plus) - bit(words, minus));
                let term = config.mul(&eq_eta[instance], &kappa[slot]);
                claim = match value {
                    1 => config.add(&claim, &term),
                    -1 => config.sub(&claim, &term),
                    _ => claim,
                };
            }
        }

        let mut prover = Blake3Transcript::new();
        let output = prove_product_sumcheck(
            &mut prover,
            &config,
            &eta,
            &beta,
            &kappa,
            slots,
            &rows,
            &claim,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let proof = ProductSumcheckProof {
            rounds: output.rounds,
            nonces: Vec::new(),
            terminal: output.terminal,
        };
        let verify = |proof: &ProductSumcheckProof, claim: &F| {
            verify_product_sumcheck(
                &mut Blake3Transcript::new(),
                &config,
                proof,
                &eta,
                slot_count.ilog2() as usize,
                claim,
                &mut UngrindedRoundBoundary,
            )
        };
        let (instance_point, slot_point) = verify(&proof, &claim).unwrap();
        assert_eq!(instance_point, output.instance_point);
        assert_eq!(slot_point, output.slot_point);

        // The three terminal claims are the stated functionals of `h`, so
        // their combination is one functional at the instance point.
        let gamma = [element(&config, 7), element(&config, 11)];
        let (weights, value) = reduced_local_weights(
            &config,
            &beta,
            &kappa,
            slots,
            &slot_point,
            &proof.terminal,
            gamma,
        )
        .unwrap();
        assert_eq!(functional(&config, &instance_point, &weights, &rows), value);

        let one = config.one();
        let mut tampered = proof.clone();
        tampered.rounds.round_polynomials[2][1] =
            config.add(&tampered.rounds.round_polynomials[2][1], &one);
        assert!(verify(&tampered, &claim).is_err());
        let mut tampered = proof.clone();
        tampered.terminal[1] = config.add(&tampered.terminal[1], &one);
        assert!(matches!(
            verify(&tampered, &claim),
            Err(ProtocolError::InvalidProductTerminalClaim)
        ));
        assert!(verify(&proof, &config.add(&claim, &one)).is_err());
    }
}
