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

use field::{BatchMulAcc, MergeAccumulator, Reduce, RingOps, Uint};
use std::mem::MaybeUninit;
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
    let one_instance = |local: &mut Vec<u64>, instance: usize| {
        bits.read_local_words(instance, local);
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
            .map_init(|| vec![0u64; words], one_instance)
            .collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut local = vec![0u64; words];
        (0..bits.instance_capacity())
            .map(|instance| one_instance(&mut local, instance))
            .collect()
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

type Acc = <FieldConfig as BatchMulAcc<F>>::Accumulator;

/// The code of one slot after the first instance fold, from its two
/// instances' bits: `(e0 | e1 << 1) · 9 + (f0 + 1) + 3 (f1 + 1)`.
#[inline]
fn fold1_code(w0: &[u64], w1: &[u64], left: u32, (plus, minus): (u32, u32)) -> u8 {
    let (e0, e1) = (bit(w0, left), bit(w1, left));
    let f0 = bit(w0, plus) - bit(w0, minus);
    let f1 = bit(w1, plus) - bit(w1, minus);
    ((e0 | (e1 << 1)) * 9 + (f0 + 1) + 3 * (f1 + 1)) as u8
}

/// The code of an all-zero slot after one fold: `e = (0, 0)`, `f = (0, 0)`.
const ZERO_FOLD1: u8 = 4;

/// The code after the second fold, from two first-fold codes: four `e` bits
/// and four `f + 1` trits, `(ea | eb << 2) · 81 + (fa + 9 fb)`.
#[inline]
fn fold2_code(a: u8, b: u8) -> u16 {
    let e = u16::from(a / 9) | (u16::from(b / 9) << 2);
    e * 81 + u16::from(a % 9) + 9 * u16::from(b % 9)
}

/// The code of an all-zero slot after two folds.
const ZERO_FOLD2: u16 = 4 + 9 * 4;

/// The values the first two folds give a slot, per code: `E` scaled by the
/// slot weight `κ_s`, and `F`.
struct FoldValues {
    /// `κ_s · E1[c]` for the four first-fold `e` codes, slot-major.
    ke1: Vec<[F; 4]>,
    /// `F1[h]` for the nine first-fold `f` histories.
    f1: [F; 9],
    /// `κ_s · E2[c]` for the sixteen second-fold `e` codes, slot-major.
    ke2: Vec<[F; 16]>,
    /// `F2[h]` for the 81 second-fold `f` histories.
    f2: Vec<F>,
}

impl FoldValues {
    /// The first-fold tables at challenge `r0`.
    fn first(config: &FieldConfig, modulus: u128, slot_weights: &[F], r0: &F) -> Self {
        let one = config.one();
        // e0 + r (e1 − e0) at the codes e0 | e1 << 1.
        let e1 = [config.zero(), config.sub(&one, r0), *r0, one];
        let f1 = std::array::from_fn(|h| {
            let (f0, f1) = ((h % 3) as i128 - 1, (h / 3) as i128 - 1);
            let base = from_i128(config, modulus, f0);
            config.add(&base, &config.mul(r0, &from_i128(config, modulus, f1 - f0)))
        });
        let ke1 = slot_weights
            .iter()
            .map(|weight| e1.map(|e| config.mul(weight, &e)))
            .collect();
        Self {
            ke1,
            f1,
            ke2: Vec::new(),
            f2: Vec::new(),
        }
    }

    /// Adds the second-fold tables at challenge `r1`.
    fn second(&mut self, config: &FieldConfig, r1: &F) {
        let fold = |a: &F, b: &F| config.add(a, &config.mul(r1, &config.sub(b, a)));
        self.ke2 = self
            .ke1
            .iter()
            .map(|ke1| std::array::from_fn(|c| fold(&ke1[c & 3], &ke1[c >> 2])))
            .collect();
        self.f2 = (0..81)
            .map(|h| fold(&self.f1[h % 9], &self.f1[h / 9]))
            .collect();
    }

    #[inline]
    fn first_values(&self, slot: usize, code: u8) -> [F; 2] {
        [
            self.ke1[slot][usize::from(code / 9)],
            self.f1[usize::from(code % 9)],
        ]
    }

    #[inline]
    fn second_values(&self, slot: usize, code: u16) -> [F; 2] {
        [
            self.ke2[slot][usize::from(code / 81)],
            self.f2[usize::from(code % 81)],
        ]
    }
}

/// The instance tables between rounds: `[Ê, F]` of every remaining instance
/// row and slot, stored as codes while the entries still come from a few bits.
enum Tables {
    Fold1(Vec<u8>),
    Fold2(Vec<u16>),
    Field(Vec<[F; 2]>),
}

/// One row pair's round sums: `Σ_s Ê_a F_a`, `Σ_s Ê_b F_b` and
/// `Σ_s (Ê_b − Ê_a)(F_b − F_a)`, unreduced.
struct PairSums {
    a: Acc,
    b: Acc,
    inf: Acc,
}

impl PairSums {
    fn new() -> Self {
        Self {
            a: Acc::zero(),
            b: Acc::zero(),
            inf: Acc::zero(),
        }
    }

    #[inline]
    fn add(&mut self, config: &FieldConfig, [ea, fa]: [F; 2], [eb, fb]: [F; 2]) {
        config.mul_acc(&mut self.a, &ea, &fa);
        config.mul_acc(&mut self.b, &eb, &fb);
        config.mul_acc(&mut self.inf, &config.sub(&eb, &ea), &config.sub(&fb, &fa));
    }

    /// `eq · [D_a + Σ Ê_a F_a, D_b + Σ Ê_b F_b, Σ ΔÊ ΔF]`.
    fn weighted(self, config: &FieldConfig, eq: &F, da: &F, db: &F) -> [F; 3] {
        [
            config.mul(eq, &config.add(da, &config.reduce(self.a))),
            config.mul(eq, &config.add(db, &config.reduce(self.b))),
            config.mul(eq, &config.reduce(self.inf)),
        ]
    }
}

/// Runs `chunk(scratch, q, rows)` over the `chunk_len`-entry chunks of `out`
/// in parallel and sums what it returns.
fn over_chunks<T: Send, S>(
    config: &FieldConfig,
    out: &mut [T],
    chunk_len: usize,
    scratch: impl Fn() -> S + Sync + Send,
    chunk: impl Fn(&mut S, usize, &mut [T]) -> [F; 3] + Sync + Send,
) -> [F; 3] {
    let zero = config.zero();
    #[cfg(feature = "parallel")]
    {
        out.par_chunks_mut(chunk_len)
            .enumerate()
            .map_init(scratch, |scratch, (q, rows)| chunk(scratch, q, rows))
            .reduce(|| [zero, zero, zero], |a, b| sum3(config, a, b))
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut scratch = scratch();
        out.chunks_mut(chunk_len)
            .enumerate()
            .map(|(q, rows)| chunk(&mut scratch, q, rows))
            .fold([zero, zero, zero], |a, b| sum3(config, a, b))
    }
}

/// A table of `len` entries every one of which the caller writes.
fn uninit_table(len: usize) -> Vec<MaybeUninit<[F; 2]>> {
    let mut table = Vec::with_capacity(len);
    table.resize_with(len, MaybeUninit::uninit);
    table
}

/// # Safety
///
/// Every entry of `table` must have been written.
unsafe fn assume_init(table: Vec<MaybeUninit<[F; 2]>>) -> Vec<[F; 2]> {
    let mut table = std::mem::ManuallyDrop::new(table);
    let (pointer, length, capacity) = (
        table.as_mut_ptr().cast::<[F; 2]>(),
        table.len(),
        table.capacity(),
    );
    // SAFETY: the caller wrote every entry, and `MaybeUninit<[F; 2]>` has the
    // layout of `[F; 2]`.
    unsafe { Vec::from_raw_parts(pointer, length, capacity) }
}

#[inline]
fn fold2(config: &FieldConfig, r: &F, [e0, f0]: [F; 2], [e1, f1]: [F; 2]) -> [F; 2] {
    [
        config.add(&e0, &config.mul(r, &config.sub(&e1, &e0))),
        config.add(&f0, &config.mul(r, &config.sub(&f1, &f0))),
    ]
}

/// Proves `μ = Σ_{i, s} P(i, s)` (see the module documentation).
///
/// `slot_weights` is `κ` padded to a power of two; slots past
/// `slots.left.len()` carry zero weight and zero factors.
///
/// Round 0 reads the bits directly (`E ∈ {0, 1}`, `F ∈ {−1, 0, 1}`, so its
/// sums are exact small multiples of `κ`). After the first and second
/// instance folds the tables are one- and two-byte codes into small value
/// tables; from the third fold on they are field tables. Every fold pass also
/// accumulates the next round's sums, so each round reads its table once.
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
    if modulus >> 115 != 0
        || (2 * slots.left.len() as u128 + 1).saturating_mul(modulus) >> 126 != 0
    {
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
    let mut scale = one;

    // Round 0 reads the bits directly: `E ∈ {0, 1}` and `F ∈ {−1, 0, 1}`,
    // so every slot contributes a small multiple of κ, summed exactly.
    let mut sums = {
        let _scope = tracing::info_span!("product:round0").entered();
        let kappa = canonical_u128(config, slot_weights);
        let rest = eq_table(&instance_point[1..], config).map_err(SpartanError::from)?;
        let pair_sums = |(w0, w1): &mut (Vec<u64>, Vec<u64>), p: usize| -> [F; 3] {
            bits.read_local_words(2 * p, w0);
            bits.read_local_words(2 * p + 1, w1);
            let (mut a0, mut a1, mut ainf) = (0i128, 0i128, 0i128);
            for slot in 0..live_slots {
                let l = slots.left[slot];
                let (plus, minus) = slots.right[slot];
                let (e0, e1) = (bit(w0, l), bit(w1, l));
                let f0 = bit(w0, plus) - bit(w0, minus);
                let f1 = bit(w1, plus) - bit(w1, minus);
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
        let buffers = || (vec![0u64; words], vec![0u64; words]);
        let half = 1usize << (instance_vars - 1);
        #[cfg(feature = "parallel")]
        {
            (0..half)
                .into_par_iter()
                .map_init(buffers, pair_sums)
                .reduce(|| [zero, zero, zero], |a, b| sum3(config, a, b))
        }
        #[cfg(not(feature = "parallel"))]
        {
            let mut buffers = buffers();
            (0..half)
                .map(|p| pair_sums(&mut buffers, p))
                .fold([zero, zero, zero], |a, b| sum3(config, a, b))
        }
    };

    let mut tables: Option<Tables> = None;
    let mut values: Option<FoldValues> = None;
    for round in 0..instance_vars {
        let eta = &instance_point[round];
        let coefficients = instance_round_polynomial(config, &scale, eta, sums);
        debug_assert_eq!(
            config.add(&coefficients[0], &evaluate(config, &coefficients, &one)),
            current
        );
        let r = round_challenge(transcript, config, &coefficients, round, boundary)?;
        current = evaluate(config, &coefficients, &r);
        let factor = config.add(
            &config.sub(&one, eta),
            &config.mul(&r, &config.sub(&config.add(eta, eta), &one)),
        );
        scale = config.mul(&scale, &factor);
        rounds.push(coefficients);
        point.push(r);
        fold_in_place(config, &mut d, &r);

        // Fold every table with `r` into `rows` rows and, unless one row is
        // left, accumulate the next round's sums over its row pairs: output
        // rows (2q, 2q + 1) come from input rows 4q..4q + 3.
        let _scope = tracing::info_span!("product:fold").entered();
        let rows = d.len();
        let group = rows.min(2);
        let chunk_len = group * slot_count;
        let next = if rows >= 2 {
            Some(eq_table(&instance_point[round + 2..], config).map_err(SpartanError::from)?)
        } else {
            None
        };
        let weigh = |q: usize, pair: PairSums| match &next {
            Some(eq) => pair.weighted(config, &eq[q], &d[2 * q], &d[2 * q + 1]),
            None => [zero, zero, zero],
        };
        let (folded, next_sums) = match tables.take() {
            None => {
                // Instances → first-fold codes.
                let fv = FoldValues::first(config, modulus, slot_weights, &r);
                let mut codes = vec![ZERO_FOLD1; rows * slot_count];
                let scratch = || -> [Vec<u64>; 4] { std::array::from_fn(|_| vec![0u64; words]) };
                let sums = over_chunks(config, &mut codes, chunk_len, scratch, |w, q, out| {
                    for (k, buffer) in w.iter_mut().enumerate().take(2 * group) {
                        bits.read_local_words(2 * group * q + k, buffer);
                    }
                    let mut pair = PairSums::new();
                    for slot in 0..live_slots {
                        let (l, right) = (slots.left[slot], slots.right[slot]);
                        let a = fold1_code(&w[0], &w[1], l, right);
                        out[slot] = a;
                        if group == 2 {
                            let b = fold1_code(&w[2], &w[3], l, right);
                            out[slot_count + slot] = b;
                            pair.add(config, fv.first_values(slot, a), fv.first_values(slot, b));
                        }
                    }
                    weigh(q, pair)
                });
                values = Some(fv);
                (Tables::Fold1(codes), sums)
            }
            Some(Tables::Fold1(source)) => {
                // First-fold codes → second-fold codes.
                let fv = values.as_mut().ok_or(ProtocolError::InvalidGeometry)?;
                fv.second(config, &r);
                let fv = &*fv;
                let mut codes = vec![ZERO_FOLD2; rows * slot_count];
                let sums = over_chunks(config, &mut codes, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    let row = |k: usize| &source[base + k * slot_count..base + (k + 1) * slot_count];
                    let mut pair = PairSums::new();
                    for slot in 0..live_slots {
                        let a = fold2_code(row(0)[slot], row(1)[slot]);
                        out[slot] = a;
                        if group == 2 {
                            let b = fold2_code(row(2)[slot], row(3)[slot]);
                            out[slot_count + slot] = b;
                            pair.add(config, fv.second_values(slot, a), fv.second_values(slot, b));
                        }
                    }
                    weigh(q, pair)
                });
                (Tables::Fold2(codes), sums)
            }
            Some(Tables::Fold2(source)) => {
                // Second-fold codes → field entries.
                let fv = values.as_ref().ok_or(ProtocolError::InvalidGeometry)?;
                let mut out = uninit_table(rows * slot_count);
                let sums = over_chunks(config, &mut out, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    let value = |k: usize, slot: usize| {
                        fv.second_values(slot, source[base + k * slot_count + slot])
                    };
                    let mut pair = PairSums::new();
                    for slot in 0..slot_count {
                        let a = fold2(config, &r, value(0, slot), value(1, slot));
                        out[slot].write(a);
                        if group == 2 {
                            let b = fold2(config, &r, value(2, slot), value(3, slot));
                            out[slot_count + slot].write(b);
                            if slot < live_slots {
                                pair.add(config, a, b);
                            }
                        }
                    }
                    weigh(q, pair)
                });
                // SAFETY: every chunk wrote each of its entries.
                (Tables::Field(unsafe { assume_init(out) }), sums)
            }
            Some(Tables::Field(source)) => {
                let mut out = uninit_table(rows * slot_count);
                let sums = over_chunks(config, &mut out, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    let value = |k: usize, slot: usize| source[base + k * slot_count + slot];
                    let mut pair = PairSums::new();
                    for slot in 0..slot_count {
                        let a = fold2(config, &r, value(0, slot), value(1, slot));
                        out[slot].write(a);
                        if group == 2 {
                            let b = fold2(config, &r, value(2, slot), value(3, slot));
                            out[slot_count + slot].write(b);
                            if slot < live_slots {
                                pair.add(config, a, b);
                            }
                        }
                    }
                    weigh(q, pair)
                });
                // SAFETY: every chunk wrote each of its entries.
                (Tables::Field(unsafe { assume_init(out) }), sums)
            }
        };
        tables = Some(folded);
        sums = next_sums;
    }

    // One instance row is left: its slot entries as field values.
    let row: Vec<[F; 2]> = match (tables, values.as_ref()) {
        (Some(Tables::Fold1(codes)), Some(fv)) => {
            (0..slot_count).map(|slot| fv.first_values(slot, codes[slot])).collect()
        }
        (Some(Tables::Fold2(codes)), Some(fv)) => {
            (0..slot_count).map(|slot| fv.second_values(slot, codes[slot])).collect()
        }
        (Some(Tables::Field(table)), _) => table,
        _ => return Err(ProtocolError::InvalidGeometry),
    };
    let mut ehat = row.iter().map(|entry| entry[0]).collect::<Vec<_>>();
    let mut fvals = row.iter().map(|entry| entry[1]).collect::<Vec<_>>();

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
            c1 = config.add(&c1, &config.add(&config.mul(e0, &df), &config.mul(&de, f0)));
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
        let r = round_challenge(transcript, config, &coefficients, instance_vars + round, boundary)?;
        current = evaluate(config, &coefficients, &r);
        prefix = config.mul(&prefix, &config.sub(&one, &r));
        fold_in_place(config, &mut ehat, &r);
        fold_in_place(config, &mut fvals, &r);
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

    /// One to five instance variables: the last instance row comes out of
    /// the first-fold codes, the second-fold codes, the first field pass,
    /// and field-to-field passes.
    #[test]
    fn product_sumcheck_roundtrips_and_reduces_to_one_rank_one_functional() {
        for instance_vars in 1..=5 {
            roundtrip(instance_vars);
        }
    }

    fn roundtrip(instance_vars: usize) {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let mut seed = 0x5eed_u64 ^ instance_vars as u64;
        let (columns, live_slots, slot_count) = (128usize, 6usize, 8usize);
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
        tampered.rounds.round_polynomials[instance_vars][1] =
            config.add(&tampered.rounds.round_polynomials[instance_vars][1], &one);
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
