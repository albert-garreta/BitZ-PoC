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

/// The round messages of a product sumcheck: cubic for two-factor slots,
/// quartic for three-factor slots (coefficient form).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductRounds {
    Quadratic(SumcheckProof<F, 4>),
    Cubic(SumcheckProof<F, 5>),
}

/// The product-slot sumcheck: `instance_vars + slot_vars` round messages,
/// their grinding nonces, and the terminal evaluations
/// `[D(r_i), Ê(r_i, r_s), B(r_i, r_s)(, C(r_i, r_s))]` (`B` is `F` for
/// two-factor slots).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSumcheckProof {
    pub(crate) rounds: ProductRounds,
    pub(crate) nonces: Vec<u64>,
    pub(crate) terminal: Vec<F>,
}

impl ProductSumcheckProof {
    pub const fn rounds(&self) -> &ProductRounds {
        &self.rounds
    }

    pub fn nonces(&self) -> &[u64] {
        &self.nonces
    }

    pub fn terminal(&self) -> &[F] {
        &self.terminal
    }

    pub fn rounds_mut(&mut self) -> &mut ProductRounds {
        &mut self.rounds
    }

    pub fn terminal_mut(&mut self) -> &mut [F] {
        &mut self.terminal
    }

    /// Number of round messages.
    pub fn round_count(&self) -> usize {
        match &self.rounds {
            ProductRounds::Quadratic(rounds) => rounds.round_polynomials.len(),
            ProductRounds::Cubic(rounds) => rounds.round_polynomials.len(),
        }
    }

    /// Field elements per round message.
    pub const fn coefficients_per_round(&self) -> usize {
        match &self.rounds {
            ProductRounds::Quadratic(_) => 4,
            ProductRounds::Cubic(_) => 5,
        }
    }

    /// Serialized payload: 16-byte coefficients, 8-byte nonces.
    pub fn bytes(&self) -> usize {
        16 * (self.coefficients_per_round() * self.round_count() + self.terminal.len())
            + 8 * self.nonces.len()
    }
}

/// Two-factor product slots: slot `s` is `h[left_s] · (h[plus_s] − h[minus_s])`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuadraticSlots<'a> {
    pub left: &'a [u32],
    pub right: &'a [(u32, u32)],
}

/// Marks an absent side of a three-factor slot's factor.
pub(crate) const NO_COLUMN: u32 = u32::MAX;

/// A relation's local product slots.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ProductSlots<'a> {
    Quadratic(QuadraticSlots<'a>),
    /// Slot `s` is `A_s · B_s · C_s`, each factor `h[plus] − h[minus]` with
    /// either side possibly [`NO_COLUMN`] (the constant one is `h[0]`).
    Cubic(&'a [[(u32, u32); 3]]),
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

/// `D(i) = ⟨β, h_i⟩` for every instance slot, from nibble tables of `β`
/// (nibbles where `β` vanishes are skipped).
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
    let active = (0..nibbles)
        .filter(|nibble| beta[4 * nibble..(4 * nibble + 4).min(beta.len())].iter().any(|b| *b != 0))
        .collect::<Vec<_>>();
    // table[16 k + v] for the k-th active nibble and value v: Σ of β over
    // v's set bits, each entry reduced, so 4096 of them still fit a u128 sum.
    let mut table = vec![0u128; 16 * active.len()];
    for (index, nibble) in active.iter().enumerate() {
        for value in 1..16usize {
            let low = value.trailing_zeros() as usize;
            let column = 4 * nibble + low;
            let term = beta.get(column).copied().unwrap_or(0);
            let entry = (table[16 * index + (value & (value - 1))] + term) % modulus;
            table[16 * index + value] = entry;
        }
    }
    assert!(active.len() as u128 * modulus < u128::MAX / 2);
    let one_instance = |local: &mut Vec<u64>, instance: usize| {
        bits.read_local_words(instance, local);
        let mut sum = 0u128;
        for (index, nibble) in active.iter().enumerate() {
            let value = (local[nibble / 16] >> (4 * (nibble % 16))) & 0xf;
            sum += table[16 * index + value as usize];
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
/// row and slot, stored as codes while the entries still come from a few bits
/// (or, on the word path, not stored at all: its cross terms).
enum Tables {
    Words(Vec<F>),
    Fold1(Vec<u8>),
    Fold2(Vec<u16>),
    Field(Vec<[F; 2]>),
}

/// One row pair's round sums: `Σ_s Ê_a F_a`, `Σ_s Ê_b F_b` and
/// `Σ_s (Ê_b − Ê_a)(F_b − F_a)`, unreduced. Without `one` the middle sum is
/// skipped: the round's claim gives it.
struct PairSums {
    a: Acc,
    b: Acc,
    inf: Acc,
    one: bool,
}

impl PairSums {
    fn new(one: bool) -> Self {
        Self {
            a: Acc::zero(),
            b: Acc::zero(),
            inf: Acc::zero(),
            one,
        }
    }

    #[inline]
    fn add(&mut self, config: &FieldConfig, [ea, fa]: [F; 2], [eb, fb]: [F; 2]) {
        config.mul_acc(&mut self.a, &ea, &fa);
        if self.one {
            config.mul_acc(&mut self.b, &eb, &fb);
        }
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

/// Instance rounds the word path takes from cross terms (blocks of
/// `2^WORD_ROUNDS` instances) before it materializes the field tables.
const WORD_ROUNDS: usize = 4;

/// [`WORD_ROUNDS`], or `BITZ_PRODUCT_WORD_ROUNDS` (0 to 4; 0 is the per-slot
/// path). The choice only moves prover work: the transcript is the same.
fn word_rounds() -> usize {
    static ROUNDS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *ROUNDS.get_or_init(|| {
        std::env::var("BITZ_PRODUCT_WORD_ROUNDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .map_or(WORD_ROUNDS, |rounds: usize| rounds.min(WORD_ROUNDS))
    })
}

/// Quadratic slots in groups of 32 that read bit `i` of three words:
/// slot `32 g + i` is `h[e_g + i] · (h[p_g + i] − h[m_g + i])` with weight
/// `κ_{32 g} · 2^i` (SHA-256's `Ch`, one group per round).
struct QuadraticWords {
    /// `[e_g, p_g, m_g]`.
    columns: Vec<[u32; 3]>,
    /// `κ_{32 g}`, canonical, as its low and high 64-bit halves.
    weights: Vec<(u64, u64)>,
}

impl QuadraticWords {
    /// The groups, when every live slot belongs to one.
    fn detect(
        config: &FieldConfig,
        slots: QuadraticSlots<'_>,
        slot_weights: &[F],
        words: usize,
    ) -> Option<Self> {
        let live = slots.left.len();
        if live == 0 || live % 32 != 0 {
            return None;
        }
        let limit = 64 * words as u64;
        let mut columns = Vec::with_capacity(live / 32);
        let mut weights = Vec::with_capacity(live / 32);
        for first in (0..live).step_by(32) {
            let (e, (p, m)) = (slots.left[first], slots.right[first]);
            if [e, p, m].iter().any(|column| u64::from(*column) + 32 > limit) {
                return None;
            }
            let mut weight = slot_weights[first];
            for i in 0..32 {
                let slot = first + i as usize;
                if slots.left[slot] != e + i
                    || slots.right[slot] != (p + i, m + i)
                    || slot_weights[slot] != weight
                {
                    return None;
                }
                weight = config.add(&weight, &weight);
            }
            let weight = u128::from(config.to_integer(&slot_weights[first]));
            columns.push([e, p, m]);
            weights.push((weight as u64, (weight >> 64) as u64));
        }
        Some(Self { columns, weights })
    }
}

/// The transpose of the 8 × 8 bit matrix whose row `r` is byte `r`
/// (bit `8 r + c` goes to bit `8 c + r`).
#[inline]
fn transpose8(mut x: u64) -> u64 {
    let t = (x ^ (x >> 7)) & 0x00aa_00aa_00aa_00aa;
    x ^= t ^ (t << 7);
    let t = (x ^ (x >> 14)) & 0x0000_cccc_0000_cccc;
    x ^= t ^ (t << 14);
    let t = (x ^ (x >> 28)) & 0x0000_0000_f0f0_f0f0;
    x ^= t ^ (t << 28);
    x
}

/// Bits `column..column + 32` of an instance's local words.
#[inline]
fn word32(words: &[u64], column: u32) -> u32 {
    let (index, shift) = ((column / 64) as usize, column % 64);
    let low = words[index] >> shift;
    if shift > 32 {
        (low | (words[index + 1] << (64 - shift))) as u32
    } else {
        low as u32
    }
}

/// Reads the `(e, p, m)` words of every group for the instances
/// `first..first + count` into `out[g · count + b]`.
fn read_group_words(
    words: &QuadraticWords,
    bits: &dyn InstanceBits,
    first: usize,
    count: usize,
    local: &mut [u64],
    out: &mut [[u32; 3]],
) {
    for b in 0..count {
        bits.read_local_words(first + b, local);
        for (g, [e, p, m]) in words.columns.iter().enumerate() {
            out[g * count + b] = [word32(local, *e), word32(local, *p), word32(local, *m)];
        }
    }
}

/// `C(u, v) = Σ_s κ_s e_s(u) f_s(v)` for every instance pair of every block
/// of `2^rounds` instances (block-major, then `u`-major). One word group
/// contributes `κ_{32 g} · (int(E_u ∧ P_v) − int(E_u ∧ M_v))`, all 32 of its
/// slots at once.
fn word_cross_terms(
    config: &FieldConfig,
    modulus: u128,
    words: &QuadraticWords,
    bits: &dyn InstanceBits,
    rounds: usize,
) -> Vec<F> {
    let size = 1usize << rounds;
    let groups = words.columns.len();
    // The low halves' sums stay below `2^96 · groups`, the high ones' below
    // `2^83 · groups`.
    assert!(groups < 1 << 30);
    let two64 = from_u128(config, modulus, (1u128 << 64) % modulus);
    let scratch = || {
        (
            vec![0u64; bits.words_per_instance()],
            vec![[0u32; 3]; groups * size],
            vec![[0i128; 2]; size * size],
        )
    };
    let block = |(local, group_words, acc): &mut (Vec<u64>, Vec<[u32; 3]>, Vec<[i128; 2]>),
                 index: usize,
                 out: &mut [F]| {
        read_group_words(words, bits, index * size, size, local, group_words);
        acc.fill([0; 2]);
        for (g, (low, high)) in words.weights.iter().enumerate() {
            let (low, high) = (i128::from(*low), i128::from(*high));
            let group = &group_words[g * size..(g + 1) * size];
            for (u, [e, _, _]) in group.iter().enumerate() {
                for (entry, [_, p, m]) in acc[u * size..(u + 1) * size].iter_mut().zip(group) {
                    let x = i128::from(i64::from(e & p) - i64::from(e & m));
                    entry[0] += low * x;
                    entry[1] += high * x;
                }
            }
        }
        for (value, [low, high]) in out.iter_mut().zip(acc.iter()) {
            *value = config.add(
                &from_i128(config, modulus, *low),
                &config.mul(&two64, &from_i128(config, modulus, *high)),
            );
        }
    };
    let blocks = bits.instance_capacity() >> rounds;
    let mut cross = vec![config.zero(); blocks * size * size];
    #[cfg(feature = "parallel")]
    cross
        .par_chunks_mut(size * size)
        .enumerate()
        .for_each_init(scratch, |scratch, (index, out)| block(scratch, index, out));
    #[cfg(not(feature = "parallel"))]
    {
        let mut scratch = scratch();
        for (index, out) in cross.chunks_mut(size * size).enumerate() {
            block(&mut scratch, index, out);
        }
    }
    cross
}

/// The sums of instance round `k = point.len() < rounds` from the cross
/// terms: per row pair `(a, b)` (rows of `2^k` instances, weights
/// `w = eq(r, ·)`), `S(x, y) = Σ_{j, j'} w_j w_j' C(x_j, y_j')` gives
/// `eq_q · [D_a + S(a, a), D_b + S(b, b), S(a, a) + S(b, b) − S(a, b) − S(b, a)]`.
fn word_round_sums(
    config: &FieldConfig,
    cross: &[F],
    rounds: usize,
    point: &[F],
    eq: &[F],
    d: &[F],
) -> Result<[F; 3], ProtocolError> {
    let zero = config.zero();
    let size = 1usize << rounds;
    let width = 1usize << point.len();
    let pairs = size / (2 * width);
    let w = eq_table(point, config).map_err(SpartanError::from)?;
    let ww = (0..width * width)
        .map(|x| config.mul(&w[x / width], &w[x % width]))
        .collect::<Vec<_>>();
    let block = |(index, cross): (usize, &[F])| -> [F; 3] {
        let form = |x: usize, y: usize| {
            let mut acc = Acc::zero();
            for j in 0..width {
                let row = &cross[(x + j) * size + y..(x + j) * size + y + width];
                for (weight, value) in ww[j * width..(j + 1) * width].iter().zip(row) {
                    config.mul_acc(&mut acc, weight, value);
                }
            }
            config.reduce(acc)
        };
        let mut total = [zero, zero, zero];
        for q in 0..pairs {
            let (a, b) = (2 * q * width, (2 * q + 1) * width);
            let (aa, bb) = (form(a, a), form(b, b));
            let across = config.add(&form(a, b), &form(b, a));
            let pair = index * pairs + q;
            let e = &eq[pair];
            let sums = [
                config.mul(e, &config.add(&d[2 * pair], &aa)),
                config.mul(e, &config.add(&d[2 * pair + 1], &bb)),
                config.mul(e, &config.sub(&config.add(&aa, &bb), &across)),
            ];
            total = sum3(config, total, sums);
        }
        total
    };
    #[cfg(feature = "parallel")]
    let sums = cross
        .par_chunks(size * size)
        .enumerate()
        .map(block)
        .reduce(|| [zero, zero, zero], |a, b| sum3(config, a, b));
    #[cfg(not(feature = "parallel"))]
    let sums = cross
        .chunks(size * size)
        .enumerate()
        .map(block)
        .fold([zero, zero, zero], |a, b| sum3(config, a, b));
    Ok(sums)
}

/// The word path's tables after `rounds = point.len()` folds: `[κ_s E, F]`
/// for every row (one block of instances) and slot, `E` and `F` read off
/// the slot's bit codes through byte tables of `eq(r, ·)`. With two rows or
/// more it also returns the next round's sums (`weigh`).
#[allow(clippy::too_many_arguments)]
fn word_materialize(
    config: &FieldConfig,
    words: &QuadraticWords,
    slot_weights: &[F],
    bits: &dyn InstanceBits,
    point: &[F],
    rows: usize,
    with_one: bool,
    weigh: &(dyn Fn(usize, PairSums) -> [F; 3] + Sync),
) -> Result<(Vec<[F; 2]>, [F; 3]), ProtocolError> {
    let zero = config.zero();
    let size = 1usize << point.len();
    let slot_count = slot_weights.len();
    let live = 32 * words.columns.len();
    let group = rows.min(2);
    let w = eq_table(point, config).map_err(SpartanError::from)?;
    // tables[256 t + c] = Σ of w[8 t + b] over c's set bits b.
    let bytes = size.div_ceil(8);
    let mut tables = vec![zero; 256 * bytes];
    for t in 0..bytes {
        for c in 1..256usize {
            let low = c.trailing_zeros() as usize;
            let term = w.get(8 * t + low).copied().unwrap_or(zero);
            tables[256 * t + c] = config.add(&tables[256 * t + (c & (c - 1))], &term);
        }
    }
    let value = |code: u32| -> F {
        let mut value = tables[(code & 0xff) as usize];
        for t in 1..bytes {
            value = config.add(&value, &tables[256 * t + ((code >> (8 * t)) & 0xff) as usize]);
        }
        value
    };
    let groups = words.columns.len();
    let scratch = || (vec![0u64; bits.words_per_instance()], vec![[0u32; 3]; groups * size]);
    let mut out = uninit_table(rows * slot_count);
    let sums = over_chunks(
        config,
        &mut out,
        group * slot_count,
        scratch,
        |(local, group_words), q, out| {
            for r in 0..group {
                let row = group * q + r;
                read_group_words(words, bits, row * size, size, local, group_words);
                let out = &mut out[r * slot_count..(r + 1) * slot_count];
                for g in 0..groups {
                    // codes[i][kind] bit b = bit i of instance b's word:
                    // 8 × 8 bit transposes of byte lanes, eight instances each.
                    let mut codes = [[0u32; 3]; 32];
                    let triples = &group_words[g * size..(g + 1) * size];
                    for (half, chunk) in triples.chunks(8).enumerate() {
                        for kind in 0..3 {
                            for lane in 0..4 {
                                let rows = chunk.iter().enumerate().fold(0u64, |x, (b, triple)| {
                                    x | (u64::from((triple[kind] >> (8 * lane)) as u8) << (8 * b))
                                });
                                let columns = transpose8(rows);
                                for j in 0..8 {
                                    codes[8 * lane + j][kind] |=
                                        u32::from((columns >> (8 * j)) as u8) << (8 * half);
                                }
                            }
                        }
                    }
                    for (i, [e, p, m]) in codes.iter().enumerate() {
                        let slot = 32 * g + i;
                        let f = config.sub(&value(*p), &value(*m));
                        out[slot].write([config.mul(&slot_weights[slot], &value(*e)), f]);
                    }
                }
                for entry in &mut out[live..] {
                    entry.write([zero, zero]);
                }
            }
            if group < 2 {
                return [zero, zero, zero];
            }
            let (a, b) = out.split_at(slot_count);
            let mut pair = PairSums::new(with_one);
            for (a, b) in a[..live].iter().zip(&b[..live]) {
                // SAFETY: both rows were written in full above.
                let (a, b) = unsafe { (a.assume_init_ref(), b.assume_init_ref()) };
                pair.add(config, *a, *b);
            }
            weigh(q, pair)
        },
    );
    // SAFETY: every chunk wrote each of its entries.
    Ok((unsafe { assume_init(out) }, sums))
}

/// Proves `μ = Σ_{i, s} P(i, s)` (see the module documentation).
///
/// `slot_weights` is `κ` padded to a power of two; slots past
/// `slots.left.len()` carry zero weight and zero factors.
///
/// Slots in word groups ([`QuadraticWords`]) take their first instance
/// rounds from word-level cross terms and then materialize field tables.
/// With `free` ([`LinearBatching::product_free_coefficients`]) they also
/// take `D` from those cross terms, which holds for a witness that
/// satisfies the relation.
///
/// Otherwise round 0 reads the bits directly (`E ∈ {0, 1}`, `F ∈ {−1, 0,
/// 1}`, so its sums are exact small multiples of `κ`). After the first and
/// second instance folds the tables are one- and two-byte codes into small
/// value tables; from the third fold on they are field tables. Every fold
/// pass also accumulates the next round's sums, so each round reads its
/// table once.
///
/// [`LinearBatching::product_free_coefficients`]: super::linear::LinearBatching::product_free_coefficients
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_product_sumcheck<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    instance_point: &[F],
    local_coefficients: &[F],
    free: Option<&[(u32, F)]>,
    slot_weights: &[F],
    slots: QuadraticSlots<'_>,
    bits: &dyn InstanceBits,
    claim: &F,
    boundary: &mut B,
) -> Result<ProductProverOutput, ProtocolError> {
    prove_product_sumcheck_with(
        transcript,
        config,
        instance_point,
        local_coefficients,
        free,
        slot_weights,
        slots,
        bits,
        claim,
        boundary,
        word_rounds(),
    )
}

/// [`prove_product_sumcheck`] with at most `word_rounds` word-path rounds.
#[allow(clippy::too_many_arguments)]
fn prove_product_sumcheck_with<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    instance_point: &[F],
    local_coefficients: &[F],
    free: Option<&[(u32, F)]>,
    slot_weights: &[F],
    slots: QuadraticSlots<'_>,
    bits: &dyn InstanceBits,
    claim: &F,
    boundary: &mut B,
    word_rounds: usize,
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
    let groups = if word_rounds > 0 {
        QuadraticWords::detect(config, slots, slot_weights, words)
    } else {
        None
    };
    let word_rounds = if groups.is_some() {
        word_rounds.min(instance_vars)
    } else {
        0
    };
    let cross = groups.as_ref().map(|groups| {
        let _scope = tracing::info_span!("product:word_cross_terms").entered();
        word_cross_terms(config, modulus, groups, bits, word_rounds)
    });

    let mut d = match (&cross, free) {
        (Some(cross), Some(free)) => {
            // On a satisfying witness `⟨Cᵀ eq(ξ), h_i⟩ = −Σ_s κ_s e_s f_s`,
            // the cross term `C(i, i)`: only the free terms are read.
            let _scope = tracing::info_span!("product:free_linear_values").entered();
            let mut extra = vec![zero; local_coefficients.len()];
            for (column, coefficient) in free {
                let entry = extra
                    .get_mut(*column as usize)
                    .ok_or(ProtocolError::InvalidGeometry)?;
                *entry = config.add(entry, coefficient);
            }
            let mut d = instance_linear_values(config, modulus, &extra, bits);
            let size = 1usize << word_rounds;
            for (instance, value) in d.iter_mut().enumerate() {
                let (block, u) = (instance >> word_rounds, instance & (size - 1));
                *value = config.sub(value, &cross[(block * size + u) * size + u]);
            }
            debug_assert_eq!(
                d,
                instance_linear_values(config, modulus, local_coefficients, bits),
                "the witness does not satisfy the relation"
            );
            d
        }
        _ => {
            let _scope = tracing::info_span!("product:instance_linear_values").entered();
            instance_linear_values(config, modulus, local_coefficients, bits)
        }
    };
    let mut rounds = Vec::with_capacity(instance_vars + slot_vars);
    let mut point = Vec::with_capacity(instance_vars + slot_vars);
    let mut current = claim.clone();
    let mut scale = one;

    // Round 0 reads the bits directly: `E ∈ {0, 1}` and `F ∈ {−1, 0, 1}`,
    // so every slot contributes a small multiple of κ, summed exactly.
    let mut sums = if let Some(cross) = &cross {
        let _scope = tracing::info_span!("product:round0").entered();
        let rest = eq_table(&instance_point[1..], config).map_err(SpartanError::from)?;
        word_round_sums(config, cross, word_rounds, &[], &rest, &d)?
    } else {
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

    let mut tables: Option<Tables> = cross.map(Tables::Words);
    let mut values: Option<FoldValues> = None;
    let mut derive_one = false;
    for round in 0..instance_vars {
        let eta = &instance_point[round];
        if derive_one {
            // l(1) from the claim: scale · [(1 − η) l(0) + η l(1)] = current.
            let inverse = *field::FieldOps::inverse_ct(config, &config.mul(&scale, eta)).value();
            let low = config.mul(&scale, &config.mul(&config.sub(&one, eta), &sums[0]));
            sums[1] = config.mul(&config.sub(&current, &low), &inverse);
        }
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
        // The next round's `X = 1` sums come from its claim when they can.
        let derive_next = instance_point
            .get(round + 1)
            .is_some_and(|eta| config.mul(&scale, eta) != zero);
        let with_one = !derive_next;
        let (folded, next_sums) = match tables.take() {
            Some(Tables::Words(cross)) if round + 1 < word_rounds => {
                let eq = next.as_deref().ok_or(ProtocolError::InvalidGeometry)?;
                let sums = word_round_sums(config, &cross, word_rounds, &point, eq, &d)?;
                (Tables::Words(cross), sums)
            }
            Some(Tables::Words(_)) => {
                let groups = groups.as_ref().ok_or(ProtocolError::InvalidGeometry)?;
                let (table, sums) = word_materialize(
                    config,
                    groups,
                    slot_weights,
                    bits,
                    &point,
                    rows,
                    with_one,
                    &weigh,
                )?;
                (Tables::Field(table), sums)
            }
            None => {
                // Instances → first-fold codes.
                let fv = FoldValues::first(config, modulus, slot_weights, &r);
                let mut codes = vec![ZERO_FOLD1; rows * slot_count];
                let scratch = || -> [Vec<u64>; 4] { std::array::from_fn(|_| vec![0u64; words]) };
                let sums = over_chunks(config, &mut codes, chunk_len, scratch, |w, q, out| {
                    for (k, buffer) in w.iter_mut().enumerate().take(2 * group) {
                        bits.read_local_words(2 * group * q + k, buffer);
                    }
                    let mut pair = PairSums::new(with_one);
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
                    let mut pair = PairSums::new(with_one);
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
                    let mut pair = PairSums::new(with_one);
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
                    let mut pair = PairSums::new(with_one);
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
        derive_one = derive_next;
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
    let (ProductRounds::Quadratic(rounds), [d, ehat, f]) = (&proof.rounds, &proof.terminal[..]) else {
        return Err(ProtocolError::InvalidGeometry);
    };
    validate_field_elements(&proof.terminal, config).map_err(SpartanError::from)?;
    let (mut point, last) = rounds
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
    let expected = config.mul(
        &scale,
        &config.add(&config.mul(d, &prefix), &config.mul(ehat, f)),
    );
    if expected != last {
        return Err(ProtocolError::InvalidProductTerminalClaim);
    }
    Ok((point, slot_point))
}

/// The local weights of the reduced claim and its value: `β` plus, per
/// factor `j` of every slot, `γ_j eq(r_s, s)` (times `κ_s` for the first
/// factor) on the factor's columns, and `v = D* + Σ_j γ_j T_j*` over the
/// terminal factor evaluations `T* = (Ê*, B*(, C*))`.
pub(crate) fn reduced_local_weights(
    config: &FieldConfig,
    local_coefficients: &[F],
    slot_weights: &[F],
    slots: ProductSlots<'_>,
    slot_point: &[F],
    terminal: &[F],
    gamma: &[F],
) -> Result<(Vec<F>, F), ProtocolError> {
    let eq_slot = eq_table(slot_point, config).map_err(SpartanError::from)?;
    let live = match slots {
        ProductSlots::Quadratic(slots) => slots.left.len(),
        ProductSlots::Cubic(factors) => factors.len(),
    };
    let factors = match slots {
        ProductSlots::Quadratic(_) => 2,
        ProductSlots::Cubic(_) => 3,
    };
    if eq_slot.len() != slot_weights.len()
        || live > slot_weights.len()
        || terminal.len() != factors + 1
        || gamma.len() != factors
    {
        return Err(ProtocolError::InvalidGeometry);
    }
    let mut weights = local_coefficients.to_vec();
    let mut add = |column: u32, value: &F| -> Result<(), ProtocolError> {
        if column == NO_COLUMN {
            return Ok(());
        }
        let entry = weights
            .get_mut(column as usize)
            .ok_or(ProtocolError::InvalidGeometry)?;
        *entry = config.add(entry, value);
        Ok(())
    };
    for slot in 0..live {
        let first = config.mul(&gamma[0], &config.mul(&eq_slot[slot], &slot_weights[slot]));
        match slots {
            ProductSlots::Quadratic(slots) => {
                let right = config.mul(&gamma[1], &eq_slot[slot]);
                let (plus, minus) = slots.right[slot];
                add(slots.left[slot], &first)?;
                add(plus, &right)?;
                add(minus, &config.neg(&right))?;
            }
            ProductSlots::Cubic(factors) => {
                let [a, b, c] = factors[slot];
                let weights = [
                    first,
                    config.mul(&gamma[1], &eq_slot[slot]),
                    config.mul(&gamma[2], &eq_slot[slot]),
                ];
                for ((plus, minus), weight) in [a, b, c].into_iter().zip(weights) {
                    add(plus, &weight)?;
                    add(minus, &config.neg(&weight))?;
                }
            }
        }
    }
    let value = gamma
        .iter()
        .zip(&terminal[1..])
        .fold(terminal[0], |value, (g, t)| config.add(&value, &config.mul(g, t)));
    Ok((weights, value))
}

/// A three-factor slot factor `h[plus] − h[minus]` on one instance's bits.
#[inline]
fn factor_bits(words: &[u64], (plus, minus): (u32, u32)) -> i32 {
    let side = |column: u32| if column == NO_COLUMN { 0 } else { bit(words, column) };
    side(plus) - side(minus)
}

/// A three-factor slot factor `h[plus] − h[minus]` on column values.
#[inline]
fn factor_value(config: &FieldConfig, (plus, minus): (u32, u32), value: &impl Fn(u32) -> F) -> F {
    match (plus != NO_COLUMN, minus != NO_COLUMN) {
        (true, true) => config.sub(&value(plus), &value(minus)),
        (true, false) => value(plus),
        (false, true) => config.neg(&value(minus)),
        (false, false) => config.zero(),
    }
}

/// `scale · eq(η_j, X) · l(X)` in coefficient form for the cubic `l` given by
/// `l(0)`, `l(1)`, `l(2)` and its `X³` coefficient `l(∞)`.
fn instance_round_polynomial3(
    config: &FieldConfig,
    scale: &F,
    eta: &F,
    half: &F,
    [l0, l1, l2, linf]: [F; 4],
) -> [F; 5] {
    let one = config.one();
    let a = config.sub(&one, eta);
    let b = config.sub(&config.add(eta, eta), &one);
    let three = config.add(&linf, &config.add(&linf, &linf));
    let six = config.add(&three, &three);
    // l(2) − 2 l(1) + l(0) = 2 c2 + 6 c3.
    let second = config.add(&config.sub(&l2, &config.add(&l1, &l1)), &l0);
    let c2 = config.mul(half, &config.sub(&second, &six));
    let c1 = config.sub(&config.sub(&config.sub(&l1, &l0), &c2), &linf);
    let coefficients = [
        config.mul(&a, &l0),
        config.add(&config.mul(&a, &c1), &config.mul(&b, &l0)),
        config.add(&config.mul(&a, &c2), &config.mul(&b, &c1)),
        config.add(&config.mul(&a, &linf), &config.mul(&b, &c2)),
        config.mul(&b, &linf),
    ];
    coefficients.map(|c| config.mul(scale, &c))
}

fn evaluate_coefficients(config: &FieldConfig, coefficients: &[F], x: &F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(config.zero(), |acc, c| config.add(&config.mul(&acc, x), c))
}

fn sum4(config: &FieldConfig, a: [F; 4], b: [F; 4]) -> [F; 4] {
    std::array::from_fn(|i| config.add(&a[i], &b[i]))
}

/// One row pair's cubic sums `Σ_s κ_s A B C` at `X = 0, 1, 2, ∞` over the
/// slots, reading the rows' column values through `read_a` / `read_b`.
fn cubic_pair_sums(
    config: &FieldConfig,
    factors: &[[(u32, u32); 3]],
    kappa: &[F],
    read_a: impl Fn(u32) -> F,
    read_b: impl Fn(u32) -> F,
) -> [F; 4] {
    let mut acc: [Acc; 4] = std::array::from_fn(|_| Acc::zero());
    let twice = |x: &F, y: &F| config.sub(&config.add(y, y), x);
    for (slot, [fa, fb, fc]) in factors.iter().enumerate() {
        let weight = &kappa[slot];
        let ea = config.mul(weight, &factor_value(config, *fa, &read_a));
        let eb = config.mul(weight, &factor_value(config, *fa, &read_b));
        let (ba, bb) = (factor_value(config, *fb, &read_a), factor_value(config, *fb, &read_b));
        let (ca, cb) = (factor_value(config, *fc, &read_a), factor_value(config, *fc, &read_b));
        config.mul_acc(&mut acc[0], &config.mul(&ea, &ba), &ca);
        config.mul_acc(&mut acc[1], &config.mul(&eb, &bb), &cb);
        config.mul_acc(
            &mut acc[2],
            &config.mul(&twice(&ea, &eb), &twice(&ba, &bb)),
            &twice(&ca, &cb),
        );
        config.mul_acc(
            &mut acc[3],
            &config.mul(&config.sub(&eb, &ea), &config.sub(&bb, &ba)),
            &config.sub(&cb, &ca),
        );
    }
    acc.map(|sum| config.reduce(sum))
}

/// `eq · [D_a + S(0), D_b + S(1), 2 D_b − D_a + S(2), S(∞)]`.
fn weigh3(config: &FieldConfig, eq: &F, da: &F, db: &F, sums: [F; 4]) -> [F; 4] {
    let d2 = config.sub(&config.add(db, db), da);
    [
        config.mul(eq, &config.add(da, &sums[0])),
        config.mul(eq, &config.add(db, &sums[1])),
        config.mul(eq, &config.add(&d2, &sums[2])),
        config.mul(eq, &sums[3]),
    ]
}

/// The per-instance local columns between instance rounds, stored as codes
/// of their bit histories while those stay small (or, on the word path, not
/// yet stored: its cross terms).
enum Base {
    Words(Vec<F>),
    /// `2^level` bits per column (levels 1 to 3), one byte each.
    Byte(Vec<u8>, usize),
    /// Sixteen bits per column.
    Word(Vec<u16>),
    Field(Vec<F>),
}

/// Instance rounds the three-factor word path takes from cross terms
/// (blocks of `2^WORD_ROUNDS3` instances, `8^WORD_ROUNDS3` terms per group).
const WORD_ROUNDS3: usize = 3;

/// [`WORD_ROUNDS3`], or `BITZ_PRODUCT3_WORD_ROUNDS` (0 to 3).
fn word_rounds3() -> usize {
    static ROUNDS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *ROUNDS.get_or_init(|| {
        std::env::var("BITZ_PRODUCT3_WORD_ROUNDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .map_or(WORD_ROUNDS3, |rounds: usize| rounds.min(WORD_ROUNDS3))
    })
}

/// Where one side of a word group's factor reads bit `i` from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operand {
    Zero,
    /// One column for every bit (the constant one).
    Broadcast(u32),
    /// Bit `(i + amount) mod 32` of the word at `base`.
    Rotate(u32, u32),
    /// Bit `i + amount` of the word at `base`, zero past bit 31.
    Shift(u32, u32),
}

impl Operand {
    /// The operand of the 32 columns `columns[i]` (bit `i`), if it has one
    /// of the four shapes and its words lie below `limit`.
    fn detect(columns: &[u32; 32], limit: u64) -> Option<Self> {
        if columns.iter().all(|c| *c == NO_COLUMN) {
            return Some(Self::Zero);
        }
        if columns.iter().all(|c| *c == columns[0]) {
            return (u64::from(columns[0]) < limit).then_some(Self::Broadcast(columns[0]));
        }
        let live = columns.iter().take_while(|c| **c != NO_COLUMN).count();
        if live < 32 {
            let amount = 32 - live as u32;
            let base = columns[0].checked_sub(amount)?;
            let fits = columns[live..].iter().all(|c| *c == NO_COLUMN)
                && (0..live).all(|i| columns[i] == columns[0] + i as u32)
                && u64::from(base) + 32 <= limit;
            return fits.then_some(Self::Shift(base, amount));
        }
        let base = *columns.iter().min()?;
        let amount = columns[0] - base;
        let fits = amount < 32
            && u64::from(base) + 32 <= limit
            && (0..32u32).all(|i| columns[i as usize] == base + (i + amount) % 32);
        fits.then_some(Self::Rotate(base, amount))
    }

    #[inline]
    fn read(self, words: &[u64]) -> u32 {
        match self {
            Self::Zero => 0,
            Self::Broadcast(column) => 0u32.wrapping_sub(bit(words, column) as u32),
            Self::Rotate(base, amount) => word32(words, base).rotate_right(amount),
            Self::Shift(base, amount) => word32(words, base) >> amount,
        }
    }
}

/// Three-factor slots in groups of 32: slot `32 g + i` reads bit `i` of
/// each of its factors' two operands, with weight `κ_{32 g} · 2^i` (SHA-256's
/// `Ch`, `Maj` and `Σ`/`σ` parities of the all-products relation).
struct CubicWords {
    /// `[A+, A−, B+, B−, C+, C−]` per group.
    operands: Vec<[Operand; 6]>,
    /// `κ_{32 g}`, canonical, as its low and high 64-bit halves.
    weights: Vec<(u64, u64)>,
    /// `κ_{32 g}`.
    scales: Vec<F>,
}

impl CubicWords {
    fn detect(
        config: &FieldConfig,
        factors: &[[(u32, u32); 3]],
        slot_weights: &[F],
        words: usize,
    ) -> Option<Self> {
        let live = factors.len();
        if live == 0 || live % 32 != 0 {
            return None;
        }
        let limit = 64 * words as u64;
        let mut operands = Vec::with_capacity(live / 32);
        let mut weights = Vec::with_capacity(live / 32);
        let mut scales = Vec::with_capacity(live / 32);
        for first in (0..live).step_by(32) {
            let side = |k: usize| -> [u32; 32] {
                std::array::from_fn(|i| {
                    let (plus, minus) = factors[first + i][k / 2];
                    if k % 2 == 0 { plus } else { minus }
                })
            };
            let group: [Operand; 6] = [
                Operand::detect(&side(0), limit)?,
                Operand::detect(&side(1), limit)?,
                Operand::detect(&side(2), limit)?,
                Operand::detect(&side(3), limit)?,
                Operand::detect(&side(4), limit)?,
                Operand::detect(&side(5), limit)?,
            ];
            let mut weight = slot_weights[first];
            for slot in first..first + 32 {
                if slot_weights[slot] != weight {
                    return None;
                }
                weight = config.add(&weight, &weight);
            }
            let weight = u128::from(config.to_integer(&slot_weights[first]));
            operands.push(group);
            weights.push((weight as u64, (weight >> 64) as u64));
            scales.push(slot_weights[first]);
        }
        Some(Self {
            operands,
            weights,
            scales,
        })
    }
}

/// A factor's bits on one instance as `(positive, negative)` masks.
#[inline]
fn trits(plus: u32, minus: u32) -> (u32, u32) {
    (plus & !minus, !plus & minus)
}

/// `T(u, v, w) = Σ_s κ_s A_s(u) B_s(v) C_s(w)` for every instance triple of
/// every block of `2^rounds` instances (block-major, then `u`, `v`, `w`): per
/// word group, the positive and negative masks of the three-trit products
/// weigh all 32 slots at once. Groups whose first two factors coincide (the
/// squares) are symmetric in `(u, v)`: they run over `u ≤ v` only, into a
/// second accumulator mirrored at the end.
fn cubic_cross_terms(
    config: &FieldConfig,
    modulus: u128,
    words: &CubicWords,
    bits: &dyn InstanceBits,
    rounds: usize,
) -> Vec<F> {
    let size = 1usize << rounds;
    let cube = size * size * size;
    let groups = words.operands.len();
    assert!(groups < 1 << 30);
    let two64 = from_u128(config, modulus, (1u128 << 64) % modulus);
    let square = words
        .operands
        .iter()
        .map(|ops| ops[0] == ops[2] && ops[1] == ops[3])
        .collect::<Vec<_>>();
    type Scratch = (Vec<u64>, Vec<[(u32, u32); 3]>, Vec<[i128; 2]>, Vec<[i128; 2]>);
    let scratch = || -> Scratch {
        (
            vec![0u64; bits.words_per_instance()],
            vec![[(0u32, 0u32); 3]; groups * size],
            vec![[0i128; 2]; cube],
            vec![[0i128; 2]; cube],
        )
    };
    let block = |(local, masks, acc, sym): &mut Scratch, index: usize, out: &mut [F]| {
        for b in 0..size {
            bits.read_local_words(index * size + b, local);
            for (g, operands) in words.operands.iter().enumerate() {
                let read = |k: usize| operands[k].read(local);
                masks[g * size + b] = [
                    trits(read(0), read(1)),
                    trits(read(2), read(3)),
                    trits(read(4), read(5)),
                ];
            }
        }
        acc.fill([0; 2]);
        sym.fill([0; 2]);
        for (g, (low, high)) in words.weights.iter().enumerate() {
            let (low, high) = (i128::from(*low), i128::from(*high));
            let group = &masks[g * size..(g + 1) * size];
            let (target, symmetric) = if square[g] { (&mut *sym, true) } else { (&mut *acc, false) };
            for (u, [(pa, na), _, _]) in group.iter().enumerate() {
                let first = if symmetric { u } else { 0 };
                for (v, [_, (pb, nb), _]) in group.iter().enumerate().skip(first) {
                    let (pab, nab) = ((pa & pb) | (na & nb), (pa & nb) | (na & pb));
                    if pab | nab == 0 {
                        continue;
                    }
                    let row = &mut target[(u * size + v) * size..(u * size + v + 1) * size];
                    for (entry, [_, _, (pc, nc)]) in row.iter_mut().zip(group) {
                        let positive = (pab & pc) | (nab & nc);
                        let negative = (pab & nc) | (nab & pc);
                        let x = i128::from(i64::from(positive) - i64::from(negative));
                        entry[0] += low * x;
                        entry[1] += high * x;
                    }
                }
            }
        }
        for (t, value) in out.iter_mut().enumerate() {
            let (u, v, w) = (t / (size * size), (t / size) % size, t % size);
            let mirrored = (u.min(v) * size + u.max(v)) * size + w;
            let [low, high] = acc[t];
            let (low, high) = (low + sym[mirrored][0], high + sym[mirrored][1]);
            *value = config.add(
                &from_i128(config, modulus, low),
                &config.mul(&two64, &from_i128(config, modulus, high)),
            );
        }
    };
    let blocks = bits.instance_capacity() >> rounds;
    let mut cross = vec![config.zero(); blocks * cube];
    #[cfg(feature = "parallel")]
    cross
        .par_chunks_mut(cube)
        .enumerate()
        .for_each_init(scratch, |scratch, (index, out)| block(scratch, index, out));
    #[cfg(not(feature = "parallel"))]
    {
        let mut scratch = scratch();
        for (index, out) in cross.chunks_mut(cube).enumerate() {
            block(&mut scratch, index, out);
        }
    }
    cross
}

/// The sums of three-factor instance round `k = point.len() < rounds` from
/// the cross terms: per row pair (`2^(k+1)` instances `t`, each factor
/// `Σ_t μ_t(X) · factor(t)` with `μ = (1 − X, X) ⊗ eq(r, ·)`), `l(X) = Σ
/// μ_t μ_t' μ_t'' T(t, t', t'')` at `X = 0, 1, 2, ∞`.
fn cubic_word_round_sums(
    config: &FieldConfig,
    cross: &[F],
    rounds: usize,
    point: &[F],
    eq: &[F],
    d: &[F],
) -> Result<[F; 4], ProtocolError> {
    let zero = config.zero();
    let one = config.one();
    let size = 1usize << rounds;
    let cube = size * size * size;
    let width = 1usize << point.len();
    let span = 2 * width;
    let pairs = size / span;
    let w = eq_table(point, config).map_err(SpartanError::from)?;
    let two = config.add(&one, &one);
    let minus_one = config.neg(&one);
    // (λ_0(X), λ_1(X)) at X = 0, 1, 2 and the leading coefficient.
    let lambdas = [(one, zero), (zero, one), (minus_one, two), (minus_one, one)];
    let coefficients = lambdas.map(|(l0, l1)| {
        let mu = (0..span)
            .map(|t| {
                let (lambda, j) = if t < width { (&l0, t) } else { (&l1, t - width) };
                config.mul(lambda, &w[j])
            })
            .collect::<Vec<_>>();
        (0..span * span * span)
            .map(|t| {
                let (t1, t2, t3) = (t / (span * span), (t / span) % span, t % span);
                config.mul(&config.mul(&mu[t1], &mu[t2]), &mu[t3])
            })
            .collect::<Vec<_>>()
    });
    let block = |(index, cross): (usize, &[F])| -> [F; 4] {
        let mut total = [zero; 4];
        for q in 0..pairs {
            let o = q * span;
            let mut acc: [Acc; 4] = std::array::from_fn(|_| Acc::zero());
            for t in 0..span * span * span {
                let (t1, t2, t3) = (t / (span * span), (t / span) % span, t % span);
                let value = &cross[((o + t1) * size + o + t2) * size + o + t3];
                for (acc, coefficients) in acc.iter_mut().zip(&coefficients) {
                    config.mul_acc(acc, &coefficients[t], value);
                }
            }
            let pair = index * pairs + q;
            let sums = acc.map(|sum| config.reduce(sum));
            total = sum4(config, total, weigh3(config, &eq[pair], &d[2 * pair], &d[2 * pair + 1], sums));
        }
        total
    };
    #[cfg(feature = "parallel")]
    let sums = cross
        .par_chunks(cube)
        .enumerate()
        .map(block)
        .reduce(|| [zero; 4], |a, b| sum4(config, a, b));
    #[cfg(not(feature = "parallel"))]
    let sums = cross
        .chunks(cube)
        .enumerate()
        .map(block)
        .fold([zero; 4], |a, b| sum4(config, a, b));
    Ok(sums)
}

/// Operand `op`'s column value at bit `i` of a row.
#[inline]
fn operand_value(op: Operand, i: u32, read: &impl Fn(u32) -> F, zero: &F) -> F {
    match op {
        Operand::Zero => *zero,
        Operand::Broadcast(column) => read(column),
        Operand::Rotate(base, amount) => read(base + (i + amount) % 32),
        Operand::Shift(base, amount) if i + amount < 32 => read(base + i + amount),
        Operand::Shift(..) => *zero,
    }
}

/// The factor `plus − minus` at bit `i` of a row.
#[inline]
fn operand_factor(
    config: &FieldConfig,
    (plus, minus): (Operand, Operand),
    i: u32,
    read: &impl Fn(u32) -> F,
    zero: &F,
) -> F {
    match (plus, minus) {
        (_, Operand::Zero) => operand_value(plus, i, read, zero),
        (Operand::Zero, _) => config.neg(&operand_value(minus, i, read, zero)),
        _ => config.sub(
            &operand_value(plus, i, read, zero),
            &operand_value(minus, i, read, zero),
        ),
    }
}

/// [`cubic_pair_sums`] by word group: `κ_{32 g} · Σ_i 2^i A_i B_i C_i` with
/// the powers of two applied by doubling (Horner over `i`), a factor that
/// is its own pair squared, and a third factor that is the same for every
/// bit pulled out of the group sum. Without `with_one` the `X = 1` sum is
/// left zero: the caller derives it from the round claim.
fn cubic_group_sums(
    config: &FieldConfig,
    words: &CubicWords,
    with_one: bool,
    read_a: impl Fn(u32) -> F,
    read_b: impl Fn(u32) -> F,
) -> [F; 4] {
    let zero = config.zero();
    let mut acc: [Acc; 4] = std::array::from_fn(|_| Acc::zero());
    for (ops, scale) in words.operands.iter().zip(&words.scales) {
        let (fa, fb, fc) = ((ops[0], ops[1]), (ops[2], ops[3]), (ops[4], ops[5]));
        let square = fa == fb;
        let uniform = |op: Operand| matches!(op, Operand::Zero | Operand::Broadcast(_));
        let outer = uniform(ops[4]) && uniform(ops[5]);
        let twice = |x: &F, y: &F| config.sub(&config.add(y, y), x);
        let mut u = [zero; 4];
        for i in (0..32u32).rev() {
            let (a0, a1) = (
                operand_factor(config, fa, i, &read_a, &zero),
                operand_factor(config, fa, i, &read_b, &zero),
            );
            let (a2, da) = (twice(&a0, &a1), config.sub(&a1, &a0));
            let mut terms = if square {
                [
                    config.square(&a0),
                    if with_one { config.square(&a1) } else { zero },
                    config.square(&a2),
                    config.square(&da),
                ]
            } else {
                let (b0, b1) = (
                    operand_factor(config, fb, i, &read_a, &zero),
                    operand_factor(config, fb, i, &read_b, &zero),
                );
                [
                    config.mul(&a0, &b0),
                    if with_one { config.mul(&a1, &b1) } else { zero },
                    config.mul(&a2, &twice(&b0, &b1)),
                    config.mul(&da, &config.sub(&b1, &b0)),
                ]
            };
            if !outer {
                let (c0, c1) = (
                    operand_factor(config, fc, i, &read_a, &zero),
                    operand_factor(config, fc, i, &read_b, &zero),
                );
                terms = [
                    config.mul(&terms[0], &c0),
                    if with_one { config.mul(&terms[1], &c1) } else { zero },
                    config.mul(&terms[2], &twice(&c0, &c1)),
                    config.mul(&terms[3], &config.sub(&c1, &c0)),
                ];
            }
            for (u, term) in u.iter_mut().zip(&terms) {
                *u = config.add(&config.add(u, u), term);
            }
        }
        if outer {
            let (c0, c1) = (
                operand_factor(config, fc, 0, &read_a, &zero),
                operand_factor(config, fc, 0, &read_b, &zero),
            );
            u = [
                config.mul(&u[0], &c0),
                config.mul(&u[1], &c1),
                config.mul(&u[2], &twice(&c0, &c1)),
                config.mul(&u[3], &config.sub(&c1, &c0)),
            ];
        }
        for (acc, u) in acc.iter_mut().zip(&u) {
            config.mul_acc(acc, scale, u);
        }
    }
    acc.map(|sum| config.reduce(sum))
}

/// A row pair's three-factor sums, by word group when the slots have them.
#[inline]
#[allow(clippy::too_many_arguments)]
fn pair_sums3(
    config: &FieldConfig,
    factors: &[[(u32, u32); 3]],
    kappa: &[F],
    groups: Option<&CubicWords>,
    with_one: bool,
    read_a: impl Fn(u32) -> F,
    read_b: impl Fn(u32) -> F,
) -> [F; 4] {
    match groups {
        Some(groups) => cubic_group_sums(config, groups, with_one, read_a, read_b),
        None => cubic_pair_sums(config, factors, kappa, read_a, read_b),
    }
}

/// What the three-factor prover returns.
pub(crate) struct Product3ProverOutput {
    pub rounds: SumcheckProof<F, 5>,
    pub terminal: [F; 4],
    pub instance_point: Vec<F>,
    pub slot_point: Vec<F>,
}

/// Proves `μ = Σ_{i, s} eq(η, i) [D(i) [s = 0] + Ê(i, s) B(i, s) C(i, s)]`
/// for three-factor slots, `Ê = κ_s A`.
///
/// The instance rounds fold each instance's local columns, not per-slot
/// factors: the factors are read off the folded columns on the fly. Round 0
/// reads the bits (exact small multiples of `κ`); the folded columns are
/// codes of their bit histories for four folds (value tables of 4, 16 and
/// 256 entries, then two 256-entry halves), field values after that. Each
/// fold pass accumulates the next round's sums at `X = 0, 1, 2, ∞`.
///
/// Slots in word groups ([`CubicWords`]) take their first instance rounds
/// from word-level cross terms, and with `free` also `D` (see
/// [`prove_product_sumcheck`]); their later sums run by group
/// ([`cubic_group_sums`]), `X = 1` from the round claim.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_product3_sumcheck<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    instance_point: &[F],
    local_coefficients: &[F],
    free: Option<&[(u32, F)]>,
    slot_weights: &[F],
    factors: &[[(u32, u32); 3]],
    bits: &dyn InstanceBits,
    claim: &F,
    boundary: &mut B,
) -> Result<Product3ProverOutput, ProtocolError> {
    prove_product3_sumcheck_with(
        transcript,
        config,
        instance_point,
        local_coefficients,
        free,
        slot_weights,
        factors,
        bits,
        claim,
        boundary,
        Some(word_rounds3()),
    )
}

/// [`prove_product3_sumcheck`] with at most `word_rounds` cross-term rounds
/// on word groups (`None`: the per-column path, no word groups at all).
#[allow(clippy::too_many_arguments)]
fn prove_product3_sumcheck_with<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    instance_point: &[F],
    local_coefficients: &[F],
    free: Option<&[(u32, F)]>,
    slot_weights: &[F],
    factors: &[[(u32, u32); 3]],
    bits: &dyn InstanceBits,
    claim: &F,
    boundary: &mut B,
    word_rounds: Option<usize>,
) -> Result<Product3ProverOutput, ProtocolError> {
    let instance_vars = instance_point.len();
    let slot_count = slot_weights.len();
    if !slot_count.is_power_of_two()
        || factors.len() > slot_count
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
    // Round 0 flushes its i128 sums every 64 slots: `64 · 27 q < 2^126`.
    if modulus >> 115 != 0 {
        return Err(ProtocolError::InvalidGeometry);
    }
    let zero = config.zero();
    let one = config.one();
    let half = from_u128(config, modulus, modulus.div_ceil(2));
    let words = bits.words_per_instance();
    let columns = 64 * words;
    let kappa = &slot_weights[..factors.len()];
    let groups = word_rounds.and_then(|_| CubicWords::detect(config, factors, slot_weights, words));
    let word_rounds = match (&groups, word_rounds) {
        (Some(_), Some(rounds)) => rounds.min(instance_vars),
        _ => 0,
    };
    let cross = groups.as_ref().filter(|_| word_rounds > 0).map(|groups| {
        let _scope = tracing::info_span!("product3:word_cross_terms").entered();
        cubic_cross_terms(config, modulus, groups, bits, word_rounds)
    });

    let mut d = match (&cross, free) {
        (Some(cross), Some(free)) => {
            // On a satisfying witness `⟨Cᵀ eq(ξ), h_i⟩ = −Σ_s κ_s A_s B_s C_s`,
            // the cross term `T(i, i, i)`: only the free terms are read.
            let _scope = tracing::info_span!("product3:free_linear_values").entered();
            let mut extra = vec![zero; local_coefficients.len()];
            for (column, coefficient) in free {
                let entry = extra
                    .get_mut(*column as usize)
                    .ok_or(ProtocolError::InvalidGeometry)?;
                *entry = config.add(entry, coefficient);
            }
            let mut d = instance_linear_values(config, modulus, &extra, bits);
            let size = 1usize << word_rounds;
            for (instance, value) in d.iter_mut().enumerate() {
                let (block, u) = (instance >> word_rounds, instance & (size - 1));
                let diagonal = block * size * size * size + (u * size + u) * size + u;
                *value = config.sub(value, &cross[diagonal]);
            }
            debug_assert_eq!(
                d,
                instance_linear_values(config, modulus, local_coefficients, bits),
                "the witness does not satisfy the relation"
            );
            d
        }
        _ => {
            let _scope = tracing::info_span!("product3:instance_linear_values").entered();
            instance_linear_values(config, modulus, local_coefficients, bits)
        }
    };
    let mut rounds = Vec::with_capacity(instance_vars + slot_vars);
    let mut point = Vec::with_capacity(instance_vars + slot_vars);
    let mut current = claim.clone();
    let mut scale = one;

    let mut sums = if let Some(cross) = &cross {
        let _scope = tracing::info_span!("product3:round0").entered();
        let rest = eq_table(&instance_point[1..], config).map_err(SpartanError::from)?;
        cubic_word_round_sums(config, cross, word_rounds, &[], &rest, &d)?
    } else {
        let _scope = tracing::info_span!("product3:round0").entered();
        let kappa_c = canonical_u128(config, kappa);
        let rest = eq_table(&instance_point[1..], config).map_err(SpartanError::from)?;
        let pair_sums = |(w0, w1): &mut (Vec<u64>, Vec<u64>), p: usize| -> [F; 4] {
            bits.read_local_words(2 * p, w0);
            bits.read_local_words(2 * p + 1, w1);
            let mut total = [zero; 4];
            for (block, chunk) in factors.chunks(64).enumerate() {
                let mut acc = [0i128; 4];
                for (offset, [fa, fb, fc]) in chunk.iter().enumerate() {
                    let (a0, a1) = (factor_bits(w0, *fa), factor_bits(w1, *fa));
                    let (b0, b1) = (factor_bits(w0, *fb), factor_bits(w1, *fb));
                    let (c0, c1) = (factor_bits(w0, *fc), factor_bits(w1, *fc));
                    let weight = kappa_c[64 * block + offset] as i128;
                    acc[0] += weight * i128::from(a0 * b0 * c0);
                    acc[1] += weight * i128::from(a1 * b1 * c1);
                    acc[2] += weight
                        * i128::from((2 * a1 - a0) * (2 * b1 - b0) * (2 * c1 - c0));
                    acc[3] += weight * i128::from((a1 - a0) * (b1 - b0) * (c1 - c0));
                }
                total = sum4(config, total, acc.map(|v| from_i128(config, modulus, v)));
            }
            weigh3(config, &rest[p], &d[2 * p], &d[2 * p + 1], total)
        };
        let buffers = || (vec![0u64; words], vec![0u64; words]);
        let half_instances = 1usize << (instance_vars - 1);
        #[cfg(feature = "parallel")]
        {
            (0..half_instances)
                .into_par_iter()
                .map_init(buffers, pair_sums)
                .reduce(|| [zero; 4], |a, b| sum4(config, a, b))
        }
        #[cfg(not(feature = "parallel"))]
        {
            let mut buffers = buffers();
            (0..half_instances)
                .map(|p| pair_sums(&mut buffers, p))
                .fold([zero; 4], |a, b| sum4(config, a, b))
        }
    };

    let mut base: Option<Base> = cross.map(Base::Words);
    let mut values: Vec<F> = Vec::new();
    let mut derive_one = false;
    for round in 0..instance_vars {
        let eta = &instance_point[round];
        if derive_one {
            // l(1) from the claim: scale · [(1 − η) l(0) + η l(1)] = current.
            let inverse = *field::FieldOps::inverse_ct(config, &config.mul(&scale, eta)).value();
            let low = config.mul(&scale, &config.mul(&config.sub(&one, eta), &sums[0]));
            sums[1] = config.mul(&config.sub(&current, &low), &inverse);
        }
        let coefficients = instance_round_polynomial3(config, &scale, eta, &half, sums);
        debug_assert_eq!(
            config.add(&coefficients[0], &evaluate_coefficients(config, &coefficients, &one)),
            current
        );
        absorb_field_elements(transcript, &coefficients, config);
        boundary
            .after_round(transcript, round)
            .map_err(SpartanError::from)?;
        let r: F = squeeze_field(transcript, config).map_err(SpartanError::from)?;
        current = evaluate_coefficients(config, &coefficients, &r);
        let factor = config.add(
            &config.sub(&one, eta),
            &config.mul(&r, &config.sub(&config.add(eta, eta), &one)),
        );
        scale = config.mul(&scale, &factor);
        rounds.push(coefficients);
        point.push(r);
        fold_in_place(config, &mut d, &r);

        let _scope = tracing::info_span!("product3:fold").entered();
        let rows = d.len();
        let group = rows.min(2);
        let chunk_len = group * columns;
        let next = if rows >= 2 {
            Some(eq_table(&instance_point[round + 2..], config).map_err(SpartanError::from)?)
        } else {
            None
        };
        let fold = |a: &F, b: &F| config.add(a, &config.mul(&r, &config.sub(b, a)));
        // The value table of the codes this round produces.
        let next_values: Vec<F> = match &base {
            None => vec![zero, config.sub(&one, &r), r, one],
            // Sixteen-bit codes: `(1 − r) V[low] + r V[high]` from two
            // 256-entry halves, not one 65 536-entry table.
            Some(Base::Byte(_, 3)) => {
                let low = config.sub(&one, &r);
                (0..512)
                    .map(|x| config.mul(if x < 256 { &low } else { &r }, &values[x % 256]))
                    .collect()
            }
            Some(Base::Byte(_, level)) => {
                let width = 1usize << level;
                let mask = (1usize << width) - 1;
                (0..1usize << (2 * width))
                    .map(|c| fold(&values[c & mask], &values[c >> width]))
                    .collect()
            }
            _ => Vec::new(),
        };
        // A sixteen-bit code's value from its two halves.
        let word_value = |table: &[F], code: u16| {
            config.add(&table[usize::from(code & 0xff)], &table[256 + usize::from(code >> 8)])
        };
        let weigh = |q: usize, sums: [F; 4]| match &next {
            Some(eq) => weigh3(config, &eq[q], &d[2 * q], &d[2 * q + 1], sums),
            None => [zero; 4],
        };
        // Word groups skip the next round's `X = 1` sums when its claim gives them.
        let derive_next = groups.is_some()
            && instance_point
                .get(round + 1)
                .is_some_and(|eta| config.mul(&scale, eta) != zero);
        let with_one = !derive_next;
        let (folded, next_sums) = match base.take() {
            Some(Base::Words(cross)) if round + 1 < word_rounds => {
                let eq = next.as_deref().ok_or(ProtocolError::InvalidGeometry)?;
                let sums = cubic_word_round_sums(config, &cross, word_rounds, &point, eq, &d)?;
                (Base::Words(cross), sums)
            }
            Some(Base::Words(_)) => {
                // Instances → codes of `2^level` bits per column, level =
                // the word rounds, with their value table `eq(r, ·)` sums.
                let level = word_rounds;
                let size = 1usize << level;
                let weights = eq_table(&point, config).map_err(SpartanError::from)?;
                let mut table = vec![zero; 1usize << size];
                for c in 1..table.len() {
                    let low = c.trailing_zeros() as usize;
                    table[c] = config.add(&table[c & (c - 1)], &weights[low]);
                }
                let mut codes = vec![0u8; rows * columns];
                let scratch = || vec![vec![0u64; words]; group * size];
                let sums = over_chunks4(config, &mut codes, chunk_len, scratch, |w, q, out| {
                    for (k, buffer) in w.iter_mut().enumerate() {
                        bits.read_local_words(group * size * q + k, buffer);
                    }
                    for (row, codes) in out.chunks_mut(columns).enumerate() {
                        let instances = &w[row * size..(row + 1) * size];
                        for (column, code) in codes.iter_mut().enumerate() {
                            let c = column as u32;
                            *code = instances
                                .iter()
                                .enumerate()
                                .fold(0u8, |code, (b, words)| code | ((bit(words, c) as u8) << b));
                        }
                    }
                    if group < 2 {
                        return [zero; 4];
                    }
                    let (ra, rb) = out.split_at(columns);
                    let sums = pair_sums3(
                        config,
                        factors,
                        kappa,
                        groups.as_ref(),
                        with_one,
                        |c| table[usize::from(ra[c as usize])],
                        |c| table[usize::from(rb[c as usize])],
                    );
                    weigh(q, sums)
                });
                values = table;
                (Base::Byte(codes, level), sums)
            }
            None => {
                let mut codes = vec![0u8; rows * columns];
                let scratch = || -> [Vec<u64>; 4] { std::array::from_fn(|_| vec![0u64; words]) };
                let sums = over_chunks4(config, &mut codes, chunk_len, scratch, |w, q, out| {
                    for (k, buffer) in w.iter_mut().enumerate().take(2 * group) {
                        bits.read_local_words(2 * group * q + k, buffer);
                    }
                    for (row, codes) in out.chunks_mut(columns).enumerate() {
                        let (w0, w1) = (&w[2 * row], &w[2 * row + 1]);
                        for (column, code) in codes.iter_mut().enumerate() {
                            let c = column as u32;
                            *code = (bit(w0, c) | (bit(w1, c) << 1)) as u8;
                        }
                    }
                    if group < 2 {
                        return [zero; 4];
                    }
                    let (ra, rb) = out.split_at(columns);
                    let sums = pair_sums3(
                        config,
                        factors,
                        kappa,
                        groups.as_ref(),
                        with_one,
                        |c| next_values[usize::from(ra[c as usize])],
                        |c| next_values[usize::from(rb[c as usize])],
                    );
                    weigh(q, sums)
                });
                (Base::Byte(codes, 1), sums)
            }
            Some(Base::Byte(source, level)) if level < 3 => {
                let width = 1u32 << level;
                let mut codes = vec![0u8; rows * columns];
                let sums = over_chunks4(config, &mut codes, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    for (row, codes) in out.chunks_mut(columns).enumerate() {
                        let a = &source[base + 2 * row * columns..base + (2 * row + 1) * columns];
                        let b = &source[base + (2 * row + 1) * columns..base + (2 * row + 2) * columns];
                        for ((code, a), b) in codes.iter_mut().zip(a).zip(b) {
                            *code = a | (b << width);
                        }
                    }
                    if group < 2 {
                        return [zero; 4];
                    }
                    let (ra, rb) = out.split_at(columns);
                    let sums = pair_sums3(
                        config,
                        factors,
                        kappa,
                        groups.as_ref(),
                        with_one,
                        |c| next_values[usize::from(ra[c as usize])],
                        |c| next_values[usize::from(rb[c as usize])],
                    );
                    weigh(q, sums)
                });
                (Base::Byte(codes, level + 1), sums)
            }
            Some(Base::Byte(source, _)) => {
                // Level 3 → sixteen-bit codes.
                let mut codes = vec![0u16; rows * columns];
                let sums = over_chunks4(config, &mut codes, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    for (row, codes) in out.chunks_mut(columns).enumerate() {
                        let a = &source[base + 2 * row * columns..base + (2 * row + 1) * columns];
                        let b = &source[base + (2 * row + 1) * columns..base + (2 * row + 2) * columns];
                        for ((code, a), b) in codes.iter_mut().zip(a).zip(b) {
                            *code = u16::from(*a) | (u16::from(*b) << 8);
                        }
                    }
                    if group < 2 {
                        return [zero; 4];
                    }
                    let (ra, rb) = out.split_at(columns);
                    let sums = pair_sums3(
                        config,
                        factors,
                        kappa,
                        groups.as_ref(),
                        with_one,
                        |c| word_value(&next_values, ra[c as usize]),
                        |c| word_value(&next_values, rb[c as usize]),
                    );
                    weigh(q, sums)
                });
                (Base::Word(codes), sums)
            }
            Some(Base::Word(source)) => {
                let table = &values;
                let mut out_values = vec![zero; rows * columns];
                let sums = over_chunks4(config, &mut out_values, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    for (row, values) in out.chunks_mut(columns).enumerate() {
                        let a = &source[base + 2 * row * columns..base + (2 * row + 1) * columns];
                        let b = &source[base + (2 * row + 1) * columns..base + (2 * row + 2) * columns];
                        for ((value, a), b) in values.iter_mut().zip(a).zip(b) {
                            *value = fold(&word_value(table, *a), &word_value(table, *b));
                        }
                    }
                    if group < 2 {
                        return [zero; 4];
                    }
                    let (ra, rb) = out.split_at(columns);
                    let sums = pair_sums3(
                        config,
                        factors,
                        kappa,
                        groups.as_ref(),
                        with_one,
                        |c| ra[c as usize],
                        |c| rb[c as usize],
                    );
                    weigh(q, sums)
                });
                (Base::Field(out_values), sums)
            }
            Some(Base::Field(source)) => {
                let mut out_values = vec![zero; rows * columns];
                let sums = over_chunks4(config, &mut out_values, chunk_len, || (), |_, q, out| {
                    let base = 2 * q * chunk_len;
                    for (row, values) in out.chunks_mut(columns).enumerate() {
                        let a = &source[base + 2 * row * columns..base + (2 * row + 1) * columns];
                        let b = &source[base + (2 * row + 1) * columns..base + (2 * row + 2) * columns];
                        for ((value, a), b) in values.iter_mut().zip(a).zip(b) {
                            *value = fold(a, b);
                        }
                    }
                    if group < 2 {
                        return [zero; 4];
                    }
                    let (ra, rb) = out.split_at(columns);
                    let sums = pair_sums3(
                        config,
                        factors,
                        kappa,
                        groups.as_ref(),
                        with_one,
                        |c| ra[c as usize],
                        |c| rb[c as usize],
                    );
                    weigh(q, sums)
                });
                (Base::Field(out_values), sums)
            }
        };
        if !next_values.is_empty() {
            values = next_values;
        }
        base = Some(folded);
        sums = next_sums;
        derive_one = derive_next;
    }

    // One instance row is left: its columns as field values, then per slot
    // `Ê = κ A`, `B`, `C` (zero past the live slots).
    let row: Vec<F> = match base {
        Some(Base::Byte(codes, _)) => codes.iter().map(|c| values[usize::from(*c)]).collect(),
        Some(Base::Word(codes)) => codes
            .iter()
            .map(|c| config.add(&values[usize::from(c & 0xff)], &values[256 + usize::from(c >> 8)]))
            .collect(),
        Some(Base::Field(values)) => values,
        Some(Base::Words(_)) | None => return Err(ProtocolError::InvalidGeometry),
    };
    let read = |c: u32| row[c as usize];
    let mut ehat = vec![zero; slot_count];
    let mut b_values = vec![zero; slot_count];
    let mut c_values = vec![zero; slot_count];
    for (slot, [fa, fb, fc]) in factors.iter().enumerate() {
        ehat[slot] = config.mul(&kappa[slot], &factor_value(config, *fa, &read));
        b_values[slot] = factor_value(config, *fb, &read);
        c_values[slot] = factor_value(config, *fc, &read);
    }

    // Slot rounds: `scale · [D · [s = 0] + Ê(s) B(s) C(s)]`, cubic in `X`.
    let d_star = d[0];
    let mut prefix = one;
    for round in 0..slot_vars {
        let pairs = ehat.len() / 2;
        let mut k = [zero; 4];
        for q in 0..pairs {
            let (e0, b0, c0) = (ehat[2 * q], b_values[2 * q], c_values[2 * q]);
            let de = config.sub(&ehat[2 * q + 1], &e0);
            let db = config.sub(&b_values[2 * q + 1], &b0);
            let dc = config.sub(&c_values[2 * q + 1], &c0);
            let (eb, ed) = (config.mul(&e0, &b0), config.mul(&de, &db));
            k[0] = config.add(&k[0], &config.mul(&eb, &c0));
            let cross = config.add(&config.mul(&de, &b0), &config.mul(&e0, &db));
            k[1] = config.add(&k[1], &config.add(&config.mul(&cross, &c0), &config.mul(&eb, &dc)));
            k[2] = config.add(&k[2], &config.add(&config.mul(&ed, &c0), &config.mul(&cross, &dc)));
            k[3] = config.add(&k[3], &config.mul(&ed, &dc));
        }
        let linear = config.mul(&d_star, &prefix);
        let coefficients = [
            config.mul(&scale, &config.add(&linear, &k[0])),
            config.mul(&scale, &config.sub(&k[1], &linear)),
            config.mul(&scale, &k[2]),
            config.mul(&scale, &k[3]),
            zero,
        ];
        debug_assert_eq!(
            config.add(&coefficients[0], &evaluate_coefficients(config, &coefficients, &one)),
            current
        );
        absorb_field_elements(transcript, &coefficients, config);
        boundary
            .after_round(transcript, instance_vars + round)
            .map_err(SpartanError::from)?;
        let r: F = squeeze_field(transcript, config).map_err(SpartanError::from)?;
        current = evaluate_coefficients(config, &coefficients, &r);
        prefix = config.mul(&prefix, &config.sub(&one, &r));
        fold_in_place(config, &mut ehat, &r);
        fold_in_place(config, &mut b_values, &r);
        fold_in_place(config, &mut c_values, &r);
        rounds.push(coefficients);
        point.push(r);
    }
    let terminal = [d_star, ehat[0], b_values[0], c_values[0]];
    debug_assert_eq!(
        config.mul(
            &scale,
            &config.add(
                &config.mul(&d_star, &prefix),
                &config.mul(&config.mul(&ehat[0], &b_values[0]), &c_values[0])
            )
        ),
        current
    );
    let slot_point = point.split_off(instance_vars);
    Ok(Product3ProverOutput {
        rounds: SumcheckProof {
            round_polynomials: rounds,
        },
        terminal,
        instance_point: point,
        slot_point,
    })
}

/// [`over_chunks`] for four sums.
fn over_chunks4<T: Send, S>(
    config: &FieldConfig,
    out: &mut [T],
    chunk_len: usize,
    scratch: impl Fn() -> S + Sync + Send,
    chunk: impl Fn(&mut S, usize, &mut [T]) -> [F; 4] + Sync + Send,
) -> [F; 4] {
    let zero = config.zero();
    #[cfg(feature = "parallel")]
    {
        out.par_chunks_mut(chunk_len)
            .enumerate()
            .map_init(scratch, |scratch, (q, rows)| chunk(scratch, q, rows))
            .reduce(|| [zero; 4], |a, b| sum4(config, a, b))
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut scratch = scratch();
        out.chunks_mut(chunk_len)
            .enumerate()
            .map(|(q, rows)| chunk(&mut scratch, q, rows))
            .fold([zero; 4], |a, b| sum4(config, a, b))
    }
}

/// Verifies the three-factor rounds and the terminal identity
/// `eq(η, r_i) · (D* · Π_l (1 − r_s,l) + Ê* · B* · C*)`, returning
/// `(r_i, r_s)`.
pub(crate) fn verify_product3_sumcheck<T: Transcript, B: RoundBoundaryPolicy>(
    transcript: &mut T,
    config: &FieldConfig,
    proof: &ProductSumcheckProof,
    instance_point: &[F],
    slot_vars: usize,
    claim: &F,
    boundary: &mut B,
) -> Result<(Vec<F>, Vec<F>), ProtocolError> {
    let instance_vars = instance_point.len();
    let (ProductRounds::Cubic(rounds), [d, ehat, b, c]) = (&proof.rounds, &proof.terminal[..]) else {
        return Err(ProtocolError::InvalidGeometry);
    };
    if instance_vars == 0 {
        return Err(ProtocolError::InvalidGeometry);
    }
    validate_field_elements(&proof.terminal, config).map_err(SpartanError::from)?;
    let (mut point, last) = rounds
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
    let product = config.mul(&config.mul(ehat, b), c);
    let expected = config.mul(&scale, &config.add(&config.mul(d, &prefix), &product));
    if expected != last {
        return Err(ProtocolError::InvalidProductTerminalClaim);
    }
    Ok((point, slot_point))
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

    /// Random instance bits (column 0 is the constant one), `η`, `β`, `κ`.
    fn setup(
        config: &FieldConfig,
        seed: &mut u64,
        instance_vars: usize,
        columns: usize,
        slot_count: usize,
    ) -> (Rows, Vec<F>, Vec<F>, Vec<F>) {
        let rows = Rows {
            words: columns / 64,
            instances: (0..1 << instance_vars)
                .map(|_| {
                    let mut words = (0..columns / 64)
                        .map(|_| next(seed) ^ (next(seed) << 40))
                        .collect::<Vec<_>>();
                    words[0] |= 1;
                    words
                })
                .collect(),
        };
        let eta = (0..instance_vars).map(|_| element(config, next(seed))).collect();
        let beta = (0..columns - 3).map(|_| element(config, next(seed))).collect();
        let kappa = (0..slot_count).map(|_| element(config, next(seed))).collect();
        (rows, eta, beta, kappa)
    }

    /// `Σ_i eq(η, i) [⟨β, h_i⟩ + Σ_s κ_s · slot_s(h_i)]`.
    fn claim(
        config: &FieldConfig,
        rows: &Rows,
        eta: &[F],
        beta: &[F],
        kappa: &[F],
        slot: impl Fn(&[u64], usize) -> i32,
        live: usize,
    ) -> F {
        let eq = eq_table(eta, config).unwrap();
        let mut claim = functional(config, eta, beta, rows);
        for (instance, words) in rows.instances.iter().enumerate() {
            for s in 0..live {
                let term = config.mul(&eq[instance], &kappa[s]);
                claim = match slot(words, s) {
                    1 => config.add(&claim, &term),
                    -1 => config.sub(&claim, &term),
                    0 => claim,
                    other => panic!("slot value {other}"),
                };
            }
        }
        claim
    }

    fn tamper(proof: &ProductSumcheckProof, round: usize) -> ProductSumcheckProof {
        let one = F::make_cfg(&Uint::from(FQ_MOD)).map(|c| c.one()).unwrap();
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let mut tampered = proof.clone();
        match &mut tampered.rounds {
            ProductRounds::Quadratic(rounds) => {
                rounds.round_polynomials[round][1] =
                    config.add(&rounds.round_polynomials[round][1], &one)
            }
            ProductRounds::Cubic(rounds) => {
                rounds.round_polynomials[round][1] =
                    config.add(&rounds.round_polynomials[round][1], &one)
            }
        }
        tampered
    }

    /// One to five instance variables: the last instance row comes out of
    /// the first-fold codes, the second-fold codes, the first field pass,
    /// and field-to-field passes.
    #[test]
    fn product_sumcheck_roundtrips_and_reduces_to_one_rank_one_functional() {
        for instance_vars in 1..=5 {
            quadratic_roundtrip(instance_vars);
        }
    }

    fn quadratic_roundtrip(instance_vars: usize) {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let mut seed = 0x5eed_u64 ^ instance_vars as u64;
        let (columns, live_slots, slot_count) = (128usize, 6usize, 8usize);
        let (rows, eta, beta, kappa) = setup(&config, &mut seed, instance_vars, columns, slot_count);
        let left = (0..live_slots)
            .map(|_| 1 + (next(&mut seed) % (columns as u64 - 4)) as u32)
            .collect::<Vec<_>>();
        let right = (0..live_slots)
            .map(|s| (3 * s as u32 + 1, 3 * s as u32 + 2))
            .collect::<Vec<_>>();
        let slots = QuadraticSlots {
            left: &left,
            right: &right,
        };
        let claim = claim(
            &config,
            &rows,
            &eta,
            &beta,
            &kappa,
            |w, s| bit(w, left[s]) * (bit(w, right[s].0) - bit(w, right[s].1)),
            live_slots,
        );

        let output = prove_product_sumcheck(
            &mut Blake3Transcript::new(),
            &config,
            &eta,
            &beta,
            None,
            &kappa,
            slots,
            &rows,
            &claim,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let proof = ProductSumcheckProof {
            rounds: ProductRounds::Quadratic(output.rounds),
            nonces: Vec::new(),
            terminal: output.terminal.to_vec(),
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
        let gamma = [element(&config, 7), element(&config, 11)];
        let (weights, value) = reduced_local_weights(
            &config,
            &beta,
            &kappa,
            ProductSlots::Quadratic(slots),
            &slot_point,
            &proof.terminal,
            &gamma,
        )
        .unwrap();
        assert_eq!(functional(&config, &instance_point, &weights, &rows), value);

        let one = config.one();
        assert!(verify(&tamper(&proof, instance_vars), &claim).is_err());
        let mut tampered = proof.clone();
        tampered.terminal[1] = config.add(&tampered.terminal[1], &one);
        assert!(matches!(
            verify(&tampered, &claim),
            Err(ProtocolError::InvalidProductTerminalClaim)
        ));
        assert!(verify(&proof, &config.add(&claim, &one)).is_err());
    }

    #[test]
    fn transpose8_moves_bit_8r_plus_c_to_8c_plus_r() {
        let mut seed = 0x7a_u64;
        for _ in 0..64 {
            let x = next(&mut seed) ^ (next(&mut seed) << 40);
            let t = transpose8(x);
            for r in 0..8 {
                for c in 0..8 {
                    assert_eq!((x >> (8 * r + c)) & 1, (t >> (8 * c + r)) & 1);
                }
            }
        }
    }

    /// Slots in word groups: the word path (one to four cross-term rounds)
    /// sends exactly what the per-slot path sends, over one to six instance
    /// variables and unaligned group columns.
    #[test]
    fn word_path_matches_the_slot_path() {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        for instance_vars in 1..=6 {
            let mut seed = 0x3057_u64 ^ instance_vars as u64;
            let (columns, groups, slot_count) = (256usize, 3usize, 128usize);
            let (rows, eta, beta, mut kappa) =
                setup(&config, &mut seed, instance_vars, columns, slot_count);
            let mut left = Vec::new();
            let mut right = Vec::new();
            for g in 0..groups {
                let base = |seed: &mut u64| 1 + (next(seed) % (columns as u64 - 33)) as u32;
                let (e, p, m) = (base(&mut seed), base(&mut seed), base(&mut seed));
                let mut weight = kappa[32 * g];
                for i in 0..32u32 {
                    left.push(e + i);
                    right.push((p + i, m + i));
                    kappa[32 * g + i as usize] = weight;
                    weight = config.add(&weight, &weight);
                }
            }
            let slots = QuadraticSlots {
                left: &left,
                right: &right,
            };
            let claim = claim(
                &config,
                &rows,
                &eta,
                &beta,
                &kappa,
                |w, s| bit(w, left[s]) * (bit(w, right[s].0) - bit(w, right[s].1)),
                left.len(),
            );
            let prove = |word_rounds: usize| {
                prove_product_sumcheck_with(
                    &mut Blake3Transcript::new(),
                    &config,
                    &eta,
                    &beta,
                    None,
                    &kappa,
                    slots,
                    &rows,
                    &claim,
                    &mut UngrindedRoundBoundary,
                    word_rounds,
                )
                .unwrap()
            };
            let reference = prove(0);
            for word_rounds in 1..=WORD_ROUNDS {
                let output = prove(word_rounds);
                assert_eq!(
                    output.rounds, reference.rounds,
                    "instance_vars={instance_vars} word_rounds={word_rounds}"
                );
                assert_eq!(output.terminal, reference.terminal);
                assert_eq!(output.instance_point, reference.instance_point);
                assert_eq!(output.slot_point, reference.slot_point);
            }
        }
    }

    /// A relation the witness satisfies, `Σ_i 2^i (pos_i − neg_i) − Σ_i 2^i
    /// e_i (p_i − m_i) = 0` per group (`pos = e ∧ p`, `neg = e ∧ m`): the word
    /// path's `D` from the free terms and the cross terms is the dense
    /// `⟨β, h_i⟩`, so the proof is the per-slot path's and reduces to `β`.
    #[test]
    fn word_path_takes_d_from_a_satisfied_relation() {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let (groups, words) = (2usize, 6usize);
        let columns = 64 * words;
        for instance_vars in 1..=6 {
            let mut seed = 0xd0d0_u64 ^ instance_vars as u64;
            // Group g: e, p, m, pos, neg words at 1 + 160 g + 32 k.
            let base = |g: usize, k: usize| (1 + 160 * g + 32 * k) as u32;
            let rows = Rows {
                words,
                instances: (0..1usize << instance_vars)
                    .map(|instance| {
                        let mut local = vec![0u64; words];
                        // The last instance is an all-zero padding slot.
                        if instance + 1 == 1 << instance_vars && instance_vars > 1 {
                            return local;
                        }
                        let mut put = |column: u32, word: u32| {
                            for i in 0..32 {
                                if (word >> i) & 1 == 1 {
                                    let c = column + i;
                                    local[(c / 64) as usize] |= 1 << (c % 64);
                                }
                            }
                        };
                        put(0, 1);
                        for g in 0..groups {
                            let (e, p, m) = (next(&mut seed) as u32, next(&mut seed) as u32, next(&mut seed) as u32);
                            for (k, word) in [e, p, m, e & p, e & m].into_iter().enumerate() {
                                put(base(g, k), word);
                            }
                        }
                        local
                    })
                    .collect(),
            };
            let slot_count = (32 * groups).next_power_of_two();
            let mut left = Vec::new();
            let mut right = Vec::new();
            let mut kappa = vec![config.zero(); slot_count];
            let mut beta = vec![config.zero(); columns];
            for g in 0..groups {
                // Row weight t_g: β gets ±t_g 2^i on pos/neg, κ gets −t_g 2^i.
                let mut weight = element(&config, next(&mut seed));
                for i in 0..32u32 {
                    let slot = 32 * g + i as usize;
                    left.push(base(g, 0) + i);
                    right.push((base(g, 1) + i, base(g, 2) + i));
                    kappa[slot] = config.neg(&weight);
                    beta[(base(g, 3) + i) as usize] = weight;
                    beta[(base(g, 4) + i) as usize] = config.neg(&weight);
                    weight = config.add(&weight, &weight);
                }
            }
            let free = [0u32, 7, 200, 333, 7]
                .map(|column| (column, element(&config, next(&mut seed))));
            for (column, coefficient) in &free {
                beta[*column as usize] = config.add(&beta[*column as usize], coefficient);
            }
            let eta = (0..instance_vars)
                .map(|_| element(&config, next(&mut seed)))
                .collect::<Vec<_>>();
            let slots = QuadraticSlots {
                left: &left,
                right: &right,
            };
            let claim = claim(
                &config,
                &rows,
                &eta,
                &beta,
                &kappa,
                |w, s| bit(w, left[s]) * (bit(w, right[s].0) - bit(w, right[s].1)),
                left.len(),
            );
            let mut free_part = vec![config.zero(); columns];
            for (column, coefficient) in &free {
                free_part[*column as usize] = config.add(&free_part[*column as usize], coefficient);
            }
            assert_eq!(claim, functional(&config, &eta, &free_part, &rows));
            let prove = |free: Option<&[(u32, F)]>, word_rounds: usize| {
                prove_product_sumcheck_with(
                    &mut Blake3Transcript::new(),
                    &config,
                    &eta,
                    &beta,
                    free,
                    &kappa,
                    slots,
                    &rows,
                    &claim,
                    &mut UngrindedRoundBoundary,
                    word_rounds,
                )
                .unwrap()
            };
            let reference = prove(None, 0);
            let output = prove(Some(&free[..]), WORD_ROUNDS);
            assert_eq!(output.rounds, reference.rounds, "instance_vars={instance_vars}");
            assert_eq!(output.terminal, reference.terminal);
            let proof = ProductSumcheckProof {
                rounds: ProductRounds::Quadratic(output.rounds),
                nonces: Vec::new(),
                terminal: output.terminal.to_vec(),
            };
            let (instance_point, slot_point) = verify_product_sumcheck(
                &mut Blake3Transcript::new(),
                &config,
                &proof,
                &eta,
                slot_count.ilog2() as usize,
                &claim,
                &mut UngrindedRoundBoundary,
            )
            .unwrap();
            let gamma = [element(&config, 5), element(&config, 9)];
            let (weights, value) = reduced_local_weights(
                &config,
                &beta,
                &kappa,
                ProductSlots::Quadratic(slots),
                &slot_point,
                &proof.terminal,
                &gamma,
            )
            .unwrap();
            assert_eq!(functional(&config, &instance_point, &weights, &rows), value);
        }
    }

    /// Three-factor slots over one to six instance variables: the columns
    /// leave the instance rounds as one-byte codes at levels one to three,
    /// sixteen-bit codes, and field values.
    #[test]
    fn product3_sumcheck_roundtrips_and_reduces_to_one_rank_one_functional() {
        for instance_vars in 1..=6 {
            cubic_roundtrip(instance_vars);
        }
    }

    fn cubic_roundtrip(instance_vars: usize) {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let mut seed = 0xc0be_u64 ^ instance_vars as u64;
        let (columns, live_slots, slot_count) = (128usize, 13usize, 16usize);
        let (rows, eta, beta, kappa) = setup(&config, &mut seed, instance_vars, columns, slot_count);
        // Every factor shape: a bit, a negated bit, a difference, the
        // constant one, zero.
        let factor = |seed: &mut u64| -> (u32, u32) {
            let column = |seed: &mut u64| 1 + (next(seed) % (columns as u64 - 4)) as u32;
            match next(seed) % 5 {
                0 => (column(seed), NO_COLUMN),
                1 => (NO_COLUMN, column(seed)),
                2 => (column(seed), column(seed)),
                3 => (0, NO_COLUMN),
                _ => (NO_COLUMN, NO_COLUMN),
            }
        };
        let factors = (0..live_slots)
            .map(|_| [factor(&mut seed), factor(&mut seed), factor(&mut seed)])
            .collect::<Vec<_>>();
        let slot_value = |w: &[u64], s: usize| {
            factors[s]
                .iter()
                .map(|f| factor_bits(w, *f))
                .product::<i32>()
        };
        let claim = claim(&config, &rows, &eta, &beta, &kappa, slot_value, live_slots);

        let output = prove_product3_sumcheck(
            &mut Blake3Transcript::new(),
            &config,
            &eta,
            &beta,
            None,
            &kappa,
            &factors,
            &rows,
            &claim,
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        let proof = ProductSumcheckProof {
            rounds: ProductRounds::Cubic(output.rounds),
            nonces: Vec::new(),
            terminal: output.terminal.to_vec(),
        };
        let verify = |proof: &ProductSumcheckProof, claim: &F| {
            verify_product3_sumcheck(
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
        let gamma = [element(&config, 7), element(&config, 11), element(&config, 13)];
        let (weights, value) = reduced_local_weights(
            &config,
            &beta,
            &kappa,
            ProductSlots::Cubic(&factors),
            &slot_point,
            &proof.terminal,
            &gamma,
        )
        .unwrap();
        assert_eq!(functional(&config, &instance_point, &weights, &rows), value);

        let one = config.one();
        assert!(verify(&tamper(&proof, instance_vars), &claim).is_err());
        let mut tampered = proof.clone();
        tampered.terminal[3] = config.add(&tampered.terminal[3], &one);
        assert!(matches!(
            verify(&tampered, &claim),
            Err(ProtocolError::InvalidProductTerminalClaim)
        ));
        assert!(verify(&proof, &config.add(&claim, &one)).is_err());
    }

    /// Three-factor slots in word groups of every operand shape (rotated and
    /// shifted words, the broadcast constant, zero): the word path sends
    /// exactly what the per-column path sends.
    #[test]
    fn cubic_word_path_matches_the_column_path() {
        let config = F::make_cfg(&Uint::from(FQ_MOD)).unwrap();
        let columns = 256usize;
        for instance_vars in 1..=5 {
            let mut seed = 0x3c3c_u64 ^ instance_vars as u64;
            let slot_count = 256usize;
            let (rows, eta, beta, mut kappa) =
                setup(&config, &mut seed, instance_vars, columns, slot_count);
            let word = |seed: &mut u64| 1 + (next(seed) % (columns as u64 - 33)) as u32;
            let mut factors = Vec::new();
            // (x ⊕ y ⊕ z)-style: one source word, rotations, a shifted third.
            let (source, other) = (word(&mut seed), word(&mut seed));
            for (g, (rx, ry, rz, shift)) in
                [(7u32, 18u32, 3u32, true), (6, 11, 25, false), (0, 5, 0, false)].into_iter().enumerate()
            {
                let base = if g == 2 { other } else { source };
                for i in 0..32u32 {
                    let x = base + (i + rx) % 32;
                    let y = base + (i + ry) % 32;
                    let z = if shift {
                        if i + rz < 32 { base + i + rz } else { NO_COLUMN }
                    } else {
                        base + (i + rz) % 32
                    };
                    factors.push([(x, y), (x, y), (z, y)]);
                }
            }
            // Ch-style: a bit, a difference, the constant one; and a zero factor.
            let (e, f, h) = (word(&mut seed), word(&mut seed), word(&mut seed));
            for i in 0..32u32 {
                factors.push([(e + i, NO_COLUMN), (f + i, h + i), (0, NO_COLUMN)]);
            }
            for i in 0..32u32 {
                factors.push([(NO_COLUMN, e + i), (NO_COLUMN, NO_COLUMN), (f + i, NO_COLUMN)]);
            }
            for g in 0..factors.len() / 32 {
                let mut weight = kappa[32 * g];
                for i in 0..32 {
                    kappa[32 * g + i] = weight;
                    weight = config.add(&weight, &weight);
                }
            }
            let slot_value = |w: &[u64], s: usize| {
                factors[s].iter().map(|f| factor_bits(w, *f)).product::<i32>()
            };
            let claim = claim(&config, &rows, &eta, &beta, &kappa, slot_value, factors.len());
            let prove = |word_rounds: Option<usize>| {
                prove_product3_sumcheck_with(
                    &mut Blake3Transcript::new(),
                    &config,
                    &eta,
                    &beta,
                    None,
                    &kappa,
                    &factors,
                    &rows,
                    &claim,
                    &mut UngrindedRoundBoundary,
                    word_rounds,
                )
                .unwrap()
            };
            let reference = prove(None);
            for word_rounds in 0..=WORD_ROUNDS3 {
                let output = prove(Some(word_rounds));
                assert_eq!(
                    output.rounds, reference.rounds,
                    "instance_vars={instance_vars} word_rounds={word_rounds}"
                );
                assert_eq!(output.terminal, reference.terminal);
                assert_eq!(output.instance_point, reference.instance_point);
                assert_eq!(output.slot_point, reference.slot_point);
            }
        }
    }
}
