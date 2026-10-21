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
    sumcheck::{RoundBoundaryPolicy, SumcheckProof},
};
use crate::sumcheck::proof::{eq_eval, validate_field_elements};
use crate::sumcheck::inner::native::{raw_shared, shared_raw};
use crate::transcript::traits::Transcript;

type F = SpartanBitzField;

/// Proof-of-work domain of the product sumcheck's round boundaries.
pub(crate) struct LinearProductGrinding;

impl GrindingDomain for LinearProductGrinding {
    const DOMAIN: &'static [u8] = b"bitz/spartan-linear/grinding/product-slots/v1";
}

/// The product-slot sumcheck: `instance_vars + slot_vars` cubic round
/// messages (coefficient form), their grinding nonces, and the terminal
/// evaluations `[D(r_i), Ê(r_i, r_s), F(r_i, r_s)]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductSumcheckProof {
    pub(crate) rounds: SumcheckProof<F, 4>,
    pub(crate) nonces: Vec<u64>,
    pub(crate) terminal: Vec<F>,
}

impl ProductSumcheckProof {
    pub const fn rounds(&self) -> &SumcheckProof<F, 4> {
        &self.rounds
    }

    pub fn nonces(&self) -> &[u64] {
        &self.nonces
    }

    pub fn terminal(&self) -> &[F] {
        &self.terminal
    }

    pub fn rounds_mut(&mut self) -> &mut SumcheckProof<F, 4> {
        &mut self.rounds
    }

    pub fn terminal_mut(&mut self) -> &mut [F] {
        &mut self.terminal
    }

    /// Number of round messages.
    pub fn round_count(&self) -> usize {
        self.rounds.round_polynomials.len()
    }

    /// Serialized payload: 16-byte coefficients (four per round), 8-byte
    /// nonces.
    pub fn bytes(&self) -> usize {
        16 * (4 * self.round_count() + self.terminal.len()) + 8 * self.nonces.len()
    }
}

/// A relation's local product slots: slot `s` is
/// `h[left_s] · (h[plus_s] − h[minus_s])`.
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

/// `⟨β, h_i⟩` of one instance's local bits with fixed coefficients `β`, from
/// nibble tables (nibbles where `β` vanishes are skipped).
struct NibbleDot {
    active: Vec<usize>,
    /// `table[16 k + v]`: `β` summed over the set bits `v` of the `k`-th
    /// active nibble, each entry reduced (4096 of them still fit a u128).
    table: Vec<u128>,
    modulus: u128,
}

impl NibbleDot {
    fn new(config: &FieldConfig, modulus: u128, coefficients: &[F], words: usize) -> Self {
        let beta = canonical_u128(config, coefficients);
        let nibbles = coefficients.len().div_ceil(4);
        assert!(nibbles <= 16 * words, "the local coefficients outrun the local bits");
        let active = (0..nibbles)
            .filter(|nibble| beta[4 * nibble..(4 * nibble + 4).min(beta.len())].iter().any(|b| *b != 0))
            .collect::<Vec<_>>();
        let mut table = vec![0u128; 16 * active.len()];
        for (index, nibble) in active.iter().enumerate() {
            for value in 1..16usize {
                let low = value.trailing_zeros() as usize;
                let term = beta.get(4 * nibble + low).copied().unwrap_or(0);
                table[16 * index + value] = (table[16 * index + (value & (value - 1))] + term) % modulus;
            }
        }
        assert!(active.len() as u128 * modulus < u128::MAX / 2);
        Self {
            active,
            table,
            modulus,
        }
    }

    #[inline]
    fn value(&self, config: &FieldConfig, local: &[u64]) -> F {
        let mut sum = 0u128;
        for (index, nibble) in self.active.iter().enumerate() {
            let value = (local[nibble / 16] >> (4 * (nibble % 16))) & 0xf;
            sum += self.table[16 * index + value as usize];
        }
        from_u128(config, self.modulus, sum)
    }
}

/// `D(i) = ⟨β, h_i⟩` for every instance slot ([`NibbleDot`]).
fn instance_linear_values(
    config: &FieldConfig,
    modulus: u128,
    local_coefficients: &[F],
    bits: &dyn InstanceBits,
) -> Vec<F> {
    let words = bits.words_per_instance();
    let dot = NibbleDot::new(config, modulus, local_coefficients, words);
    let one_instance = |local: &mut Vec<u64>, instance: usize| {
        bits.read_local_words(instance, local);
        dot.value(config, local)
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

/// [`fold2`] on raw residues ([`Lazy`]): `a + r (b + q − a)` with one
/// reduction each.
#[inline(always)]
fn fold2_lazy(config: &FieldConfig, lazy: Lazy, r: u128, [e0, f0]: [F; 2], [e1, f1]: [F; 2]) -> [F; 2] {
    let fold = |a: F, b: F| {
        let (a, b) = (raw_shared(a), raw_shared(b));
        shared_raw(config, lazy.reduce_once(a + lazy.mul(r, b + lazy.q - a)))
    };
    [fold(e0, e1), fold(f0, f1)]
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
        slots: ProductSlots<'_>,
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
/// slots at once. With `free`, also every instance's `⟨free, h_i⟩`, read in
/// the same pass.
fn word_cross_terms(
    config: &FieldConfig,
    modulus: u128,
    words: &QuadraticWords,
    bits: &dyn InstanceBits,
    rounds: usize,
    free: Option<&NibbleDot>,
) -> (Vec<F>, Vec<F>) {
    let size = 1usize << rounds;
    let groups = words.columns.len();
    // The low halves' sums stay below `2^96 · groups`, the high ones' below
    // `2^83 · groups`.
    assert!(groups < 1 << 30);
    let two64 = from_u128(config, modulus, (1u128 << 64) % modulus);
    type Scratch = (Vec<u64>, Vec<[u32; 3]>, Vec<[i128; 2]>);
    let scratch = || -> Scratch {
        (
            vec![0u64; bits.words_per_instance()],
            vec![[0u32; 3]; groups * size],
            vec![[0i128; 2]; size * size],
        )
    };
    let block = |(local, group_words, acc): &mut Scratch,
                 index: usize,
                 out: &mut [F],
                 free_out: &mut [F]| {
        for b in 0..size {
            bits.read_local_words(index * size + b, local);
            for (g, [e, p, m]) in words.columns.iter().enumerate() {
                group_words[g * size + b] = [word32(local, *e), word32(local, *p), word32(local, *m)];
            }
            if let Some(free) = free {
                free_out[b] = free.value(config, local);
            }
        }
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
    let mut free_values = vec![config.zero(); if free.is_some() { blocks * size } else { 0 }];
    #[cfg(feature = "parallel")]
    if free.is_some() {
        cross
            .par_chunks_mut(size * size)
            .zip(free_values.par_chunks_mut(size))
            .enumerate()
            .for_each_init(scratch, |scratch, (index, (out, free_out))| {
                block(scratch, index, out, free_out)
            });
    } else {
        cross
            .par_chunks_mut(size * size)
            .enumerate()
            .for_each_init(scratch, |scratch, (index, out)| block(scratch, index, out, &mut []));
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut scratch = scratch();
        for (index, out) in cross.chunks_mut(size * size).enumerate() {
            if free.is_some() {
                block(&mut scratch, index, out, &mut free_values[index * size..(index + 1) * size]);
            } else {
                block(&mut scratch, index, out, &mut []);
            }
        }
    }
    (cross, free_values)
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
    // The same values as raw residues summed without reduction (below
    // `bytes · q`), when the modulus allows lazy arithmetic.
    let lazy = Lazy::new(config).filter(|_| bytes <= 2);
    let raw_tables = tables.iter().map(|entry| raw_shared(*entry)).collect::<Vec<_>>();
    let raw_weights = slot_weights.iter().map(|weight| raw_shared(*weight)).collect::<Vec<_>>();
    let raw_value = |code: u32| -> u128 {
        let mut value = raw_tables[(code & 0xff) as usize];
        for t in 1..bytes {
            value += raw_tables[256 * t + ((code >> (8 * t)) & 0xff) as usize];
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
                    match lazy {
                        Some(lazy) => {
                            for (i, [e, p, m]) in codes.iter().enumerate() {
                                let slot = 32 * g + i;
                                // E < 2q; P, M reduced once, F = P − M canonical.
                                let (p, m) = (lazy.reduce_once(raw_value(*p)), lazy.reduce_once(raw_value(*m)));
                                let f = lazy.reduce_once(p + lazy.q - m);
                                let ke = lazy.mul(raw_weights[slot], raw_value(*e));
                                out[slot].write([shared_raw(config, ke), shared_raw(config, f)]);
                            }
                        }
                        None => {
                            for (i, [e, p, m]) in codes.iter().enumerate() {
                                let slot = 32 * g + i;
                                let f = config.sub(&value(*p), &value(*m));
                                out[slot].write([config.mul(&slot_weights[slot], &value(*e)), f]);
                            }
                        }
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

/// The free terms of `β` (`(column, coefficient)` pairs, repeats summed) as
/// nibble tables over the local columns.
fn free_nibble_dot(
    config: &FieldConfig,
    modulus: u128,
    columns: usize,
    free: &[(u32, F)],
    words: usize,
) -> Result<NibbleDot, ProtocolError> {
    let mut extra = vec![config.zero(); columns];
    for (column, coefficient) in free {
        let entry = extra
            .get_mut(*column as usize)
            .ok_or(ProtocolError::InvalidGeometry)?;
        *entry = config.add(entry, coefficient);
    }
    Ok(NibbleDot::new(config, modulus, &extra, words))
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
    slots: ProductSlots<'_>,
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
    slots: ProductSlots<'_>,
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
    let lazy = Lazy::new(config);
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
    // On a satisfying witness `⟨Cᵀ eq(ξ), h_i⟩ = −Σ_s κ_s e_s f_s`, the
    // cross term `C(i, i)`: only the free terms of `β` are read, inside the
    // cross-term pass.
    let free_dot = match (&groups, free) {
        (Some(_), Some(free)) => Some(free_nibble_dot(config, modulus, local_coefficients.len(), free, words)?),
        _ => None,
    };
    let (cross, free_values) = match groups.as_ref() {
        Some(groups) => {
            let _scope = tracing::info_span!("product:word_cross_terms").entered();
            let (cross, free_values) =
                word_cross_terms(config, modulus, groups, bits, word_rounds, free_dot.as_ref());
            (Some(cross), free_values)
        }
        None => (None, Vec::new()),
    };

    let mut d = match (&cross, &free_dot) {
        (Some(cross), Some(_)) => {
            let size = 1usize << word_rounds;
            let d = free_values
                .iter()
                .enumerate()
                .map(|(instance, value)| {
                    let (block, u) = (instance >> word_rounds, instance & (size - 1));
                    config.sub(value, &cross[(block * size + u) * size + u])
                })
                .collect::<Vec<_>>();
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
        let r_raw = raw_shared(r);
        let fold_pair = |a: [F; 2], b: [F; 2]| match lazy {
            Some(lazy) => fold2_lazy(config, lazy, r_raw, a, b),
            None => fold2(config, &r, a, b),
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
                        let a = fold_pair(value(0, slot), value(1, slot));
                        out[slot].write(a);
                        if group == 2 {
                            let b = fold_pair(value(2, slot), value(3, slot));
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
                        let a = fold_pair(value(0, slot), value(1, slot));
                        out[slot].write(a);
                        if group == 2 {
                            let b = fold_pair(value(2, slot), value(3, slot));
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
    let (rounds, [d, ehat, f]) = (&proof.rounds, &proof.terminal[..]) else {
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
/// terminal factor evaluations `T* = (Ê*, F*)`.
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
    let live = slots.left.len();
    if eq_slot.len() != slot_weights.len()
        || live > slot_weights.len()
        || slots.right.len() != live
        || terminal.len() != 3
        || gamma.len() != 2
    {
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
    for slot in 0..live {
        let first = config.mul(&gamma[0], &config.mul(&eq_slot[slot], &slot_weights[slot]));
        let right = config.mul(&gamma[1], &eq_slot[slot]);
        let (plus, minus) = slots.right[slot];
        add(slots.left[slot], &first)?;
        add(plus, &right)?;
        add(minus, &config.neg(&right))?;
    }
    let value = gamma
        .iter()
        .zip(&terminal[1..])
        .fold(terminal[0], |value, (g, t)| config.add(&value, &config.mul(g, t)));
    Ok((weights, value))
}

/// Montgomery arithmetic on raw `u128` residues with lazy reduction: sums
/// stay below a small multiple of `q` and only products reduce. REDC needs
/// just `a · b < q · 2^128`, which holds for `a, b < 8 q` when `q < 2^122`.
#[derive(Clone, Copy, Debug)]
struct Lazy {
    q: u128,
    q0: u64,
    q1: u64,
    neg_inv: u64,
}

impl Lazy {
    fn new(config: &FieldConfig) -> Option<Self> {
        let q = u128::from(*config.modulus());
        if q >> 122 != 0 || q & 1 == 0 {
            return None;
        }
        let q0 = q as u64;
        // q0 · inv ≡ 1 mod 2^64 by Newton's iteration (6 steps from 1).
        let mut inv = 1u64;
        for _ in 0..6 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(q0.wrapping_mul(inv)));
        }
        Some(Self {
            q,
            q0,
            q1: (q >> 64) as u64,
            neg_inv: inv.wrapping_neg(),
        })
    }

    /// `v − q` when `v ≥ q`.
    #[inline(always)]
    fn reduce_once(self, v: u128) -> u128 {
        let (reduced, borrow) = v.overflowing_sub(self.q);
        if borrow { v } else { reduced }
    }

    /// `a · b / 2^128 mod q`, canonical, for `a · b < q · 2^128`.
    #[inline(always)]
    fn mul(self, a: u128, b: u128) -> u128 {
        let (a0, a1, b0, b1) = (a as u64, (a >> 64) as u64, b as u64, (b >> 64) as u64);
        let p00 = u128::from(a0) * u128::from(b0);
        let p01 = u128::from(a0) * u128::from(b1);
        let p10 = u128::from(a1) * u128::from(b0);
        let p11 = u128::from(a1) * u128::from(b1);
        let mid = (p00 >> 64) + u128::from(p01 as u64) + u128::from(p10 as u64);
        let mid2 = (mid >> 64) + (p01 >> 64) + (p10 >> 64) + u128::from(p11 as u64);
        let (t0, t1, t2, t3) = (p00 as u64, mid as u64, mid2 as u64, ((mid2 >> 64) + (p11 >> 64)) as u64);
        let m0 = t0.wrapping_mul(self.neg_inv);
        let mq0 = u128::from(m0) * u128::from(self.q0);
        let mq1 = u128::from(m0) * u128::from(self.q1);
        let c0 = u128::from(t0) + u128::from(mq0 as u64);
        let c1 = u128::from(t1) + (mq0 >> 64) + u128::from(mq1 as u64) + (c0 >> 64);
        let c2 = u128::from(t2) + (mq1 >> 64) + (c1 >> 64);
        let c3 = u128::from(t3) + (c2 >> 64);
        let m1 = (c1 as u64).wrapping_mul(self.neg_inv);
        let n0 = u128::from(m1) * u128::from(self.q0);
        let n1 = u128::from(m1) * u128::from(self.q1);
        let d1 = u128::from(c1 as u64) + u128::from(n0 as u64);
        let d2 = u128::from(c2 as u64) + (n0 >> 64) + u128::from(n1 as u64) + (d1 >> 64);
        let d3 = u128::from(c3 as u64) + (n1 >> 64) + (d2 >> 64);
        // The product bound keeps the result below 2q < 2^123.
        self.reduce_once(u128::from(d2 as u64) | (u128::from(d3 as u64) << 64))
    }
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
        let rounds = &mut tampered.rounds.round_polynomials;
        rounds[round][1] = config.add(&rounds[round][1], &one);
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
        let slots = ProductSlots {
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
            rounds: output.rounds,
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
            slots,
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
            let slots = ProductSlots {
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
            let slots = ProductSlots {
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
                rounds: output.rounds,
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
                slots,
                &slot_point,
                &proof.terminal,
                &gamma,
            )
            .unwrap();
            assert_eq!(functional(&config, &instance_point, &weights, &rows), value);
        }
    }
}
