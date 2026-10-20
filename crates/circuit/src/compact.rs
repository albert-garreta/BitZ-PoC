//! A compact recording backend for circuits too large for
//! [`crate::constraints::ConstraintGenerator`] (whose `BTreeSet`/`BTreeMap`
//! combinations cost hundreds of bytes per term).
//!
//! F2 combinations are sorted index vectors; integer coefficients are
//! `±2^k` (any `k < 2^15`) or small integers, which covers every gadget in
//! this crate that the BitZ relations use (word lifts, hinted sums, spread
//! products, bit-times-word rows). The recorded circuit is
//!
//! - `M`: one row per lifted cell, the committed bits whose XOR it is (plus a
//!   constant flag), in `bitz`-call order;
//! - `A`, `B`, `C`: one row per `assert_r1c`, over the cells (column `i + 1`
//!   is cell `i`, column 0 the constant one), entries `(column, code)` with
//!   `code` a [`Coeff`].
//!
//! Committed bits are numbered in `hint` order and cells in `bitz` order,
//! exactly as the witness backends (`Witgen`, `CheckingWitgen`) emit them.

use std::iter::Sum;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};
use std::sync::Arc;

use num_traits::{One, Zero};
use rayon::prelude::*;

use crate::{BoolWitness, Circuit, HintResult, PackedBits, ScalarBits, WitnessContext};

/// Support of a [`CBit`]: sorted ids, inline up to three (most supports are
/// one or two committed bits, and recording allocates nothing for them).
pub type Terms = smallvec::SmallVec<[u32; 3]>;

/// A sparse F2 combination: sorted committed-bit indices and a constant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CBit {
    pub constant: bool,
    pub terms: Terms,
}

impl From<bool> for CBit {
    fn from(constant: bool) -> Self {
        Self {
            constant,
            terms: Terms::new(),
        }
    }
}

impl BoolWitness for CBit {
    type Repr<const N: usize, const M: usize> = ScalarBits<Self, N>;
}

fn xor_sorted(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => {
                out.push(a[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(b[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

/// An integer coefficient: `±2^shift` or a small integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coeff {
    Pow2 { neg: bool, shift: u16 },
    Small(i128),
}

impl Coeff {
    fn normalize(self) -> Self {
        match self {
            Self::Small(v) if v != 0 && v.unsigned_abs().is_power_of_two() => Self::Pow2 {
                neg: v < 0,
                shift: v.unsigned_abs().trailing_zeros() as u16,
            },
            other => other,
        }
    }

    fn as_small(self) -> Option<i128> {
        match self {
            Self::Small(v) => Some(v),
            Self::Pow2 { neg, shift } if shift < 126 => {
                let v = 1i128 << shift;
                Some(if neg { -v } else { v })
            }
            Self::Pow2 { .. } => None,
        }
    }

    pub fn is_zero(self) -> bool {
        matches!(self, Self::Small(0))
    }

    /// The coefficient as little-endian two's-complement words of `limbs`.
    pub fn to_words(self, limbs: usize) -> Vec<u64> {
        let mut w = vec![0u64; limbs];
        let (neg, mag): (bool, Vec<u64>) = match self {
            Self::Small(v) => {
                let m = v.unsigned_abs();
                let mut mw = vec![0u64; limbs];
                mw[0] = m as u64;
                if limbs > 1 {
                    mw[1] = (m >> 64) as u64;
                }
                (v < 0, mw)
            }
            Self::Pow2 { neg, shift } => {
                let mut mw = vec![0u64; limbs];
                mw[shift as usize / 64] = 1u64 << (shift % 64);
                (neg, mw)
            }
        };
        if neg {
            // two's complement: !mag + 1
            let mut carry = 1u64;
            for (dst, m) in w.iter_mut().zip(&mag) {
                let (s, c) = (!m).overflowing_add(carry);
                *dst = s;
                carry = u64::from(c);
            }
        } else {
            w.copy_from_slice(&mag);
        }
        w
    }
}

impl From<u64> for Coeff {
    fn from(v: u64) -> Self {
        Self::Small(i128::from(v)).normalize()
    }
}

impl Zero for Coeff {
    fn zero() -> Self {
        Self::Small(0)
    }
    fn is_zero(&self) -> bool {
        matches!(self, Self::Small(0))
    }
}

impl One for Coeff {
    fn one() -> Self {
        Self::Pow2 {
            neg: false,
            shift: 0,
        }
    }
}

impl Add for Coeff {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Small(0), x) | (x, Self::Small(0)) => x,
            (Self::Pow2 { neg: a, shift: s }, Self::Pow2 { neg: b, shift: t }) if s == t => {
                if a == b {
                    Self::Pow2 { neg: a, shift: s + 1 }
                } else {
                    Self::Small(0)
                }
            }
            (x, y) => {
                let (a, b) = (
                    x.as_small().expect("wide coefficient sum is unsupported"),
                    y.as_small().expect("wide coefficient sum is unsupported"),
                );
                Self::Small(a.checked_add(b).expect("coefficient overflow")).normalize()
            }
        }
    }
}

impl AddAssign for Coeff {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Neg for Coeff {
    type Output = Self;
    fn neg(self) -> Self {
        match self {
            Self::Pow2 { neg, shift } => Self::Pow2 { neg: !neg, shift },
            Self::Small(v) => Self::Small(-v),
        }
    }
}

impl Sub for Coeff {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self + -rhs
    }
}

impl SubAssign for Coeff {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for Coeff {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Small(0), _) | (_, Self::Small(0)) => Self::Small(0),
            (Self::Pow2 { neg: a, shift: s }, Self::Pow2 { neg: b, shift: t }) => Self::Pow2 {
                neg: a != b,
                shift: s.checked_add(t).expect("shift overflow"),
            },
            (x, y) => {
                let (a, b) = (
                    x.as_small().expect("wide coefficient product is unsupported"),
                    y.as_small().expect("wide coefficient product is unsupported"),
                );
                Self::Small(a.checked_mul(b).expect("coefficient overflow")).normalize()
            }
        }
    }
}

impl Sum for Coeff {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), Add::add)
    }
}

/// An integer combination of cells (indices) plus a constant.
#[derive(Clone, Debug, Default)]
pub struct CLin {
    pub constant: Option<Coeff>,
    pub terms: Vec<(u32, Coeff)>,
}

impl From<Coeff> for CLin {
    fn from(c: Coeff) -> Self {
        Self {
            constant: (!c.is_zero()).then_some(c),
            terms: Vec::new(),
        }
    }
}

impl Zero for CLin {
    fn zero() -> Self {
        Self::default()
    }
    fn is_zero(&self) -> bool {
        self.constant.is_none() && self.terms.is_empty()
    }
}

impl Add for CLin {
    type Output = Self;
    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl AddAssign for CLin {
    fn add_assign(&mut self, rhs: Self) {
        self.constant = match (self.constant, rhs.constant) {
            (None, x) | (x, None) => x,
            (Some(a), Some(b)) => {
                let s = a + b;
                (!s.is_zero()).then_some(s)
            }
        };
        self.terms.extend(rhs.terms);
    }
}

impl Neg for CLin {
    type Output = Self;
    fn neg(mut self) -> Self {
        self.constant = self.constant.map(Neg::neg);
        for t in &mut self.terms {
            t.1 = -t.1;
        }
        self
    }
}

impl Sub for CLin {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self + -rhs
    }
}

impl SubAssign for CLin {
    fn sub_assign(&mut self, rhs: Self) {
        *self += -rhs;
    }
}

impl Mul<Coeff> for CLin {
    type Output = Self;
    fn mul(mut self, rhs: Coeff) -> Self {
        if rhs.is_zero() {
            return Self::zero();
        }
        self.constant = self.constant.map(|c| c * rhs);
        for t in &mut self.terms {
            t.1 = t.1 * rhs;
        }
        self
    }
}

impl Sum for CLin {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), Add::add)
    }
}

/// One recorded sparse integer matrix in CSR form (column, coefficient).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompactRows {
    pub offsets: Vec<u64>,
    pub columns: Vec<u32>,
    pub coeffs: Vec<Coeff>,
}

impl CompactRows {
    fn new() -> Self {
        Self {
            offsets: vec![0],
            ..Self::default()
        }
    }

    fn push(&mut self, lin: CLin) {
        let mut terms = lin.terms;
        terms.sort_unstable_by_key(|t| t.0);
        if let Some(c) = lin.constant {
            self.columns.push(0);
            self.coeffs.push(c);
        }
        let mut i = 0;
        while i < terms.len() {
            let (col, mut c) = terms[i];
            let mut j = i + 1;
            while j < terms.len() && terms[j].0 == col {
                c += terms[j].1;
                j += 1;
            }
            if !c.is_zero() {
                self.columns.push(col + 1);
                self.coeffs.push(c);
            }
            i = j;
        }
        self.offsets.push(self.columns.len() as u64);
    }

    pub fn rows(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn row(&self, r: usize) -> impl Iterator<Item = (u32, Coeff)> + '_ {
        let (s, e) = (self.offsets[r] as usize, self.offsets[r + 1] as usize);
        self.columns[s..e].iter().copied().zip(self.coeffs[s..e].iter().copied())
    }

    pub fn nnz(&self) -> usize {
        self.columns.len()
    }
}

/// The recorded circuit.
#[derive(Clone, Debug, Default)]
pub struct CompactCircuit {
    /// Committed bits (hint outputs).
    pub committed: usize,
    /// `M`, CSR: cell `i`'s committed-bit support is
    /// `m_terms[m_offsets[i]..m_offsets[i + 1]]`; `m_const[i]` its constant.
    pub m_offsets: Vec<u64>,
    pub m_terms: Vec<u32>,
    pub m_const: Vec<bool>,
    pub a: CompactRows,
    pub b: CompactRows,
    pub c: CompactRows,
    /// Integer limbs each row was asserted at (its declared width).
    pub row_limbs: Vec<u16>,
}

impl CompactCircuit {
    pub fn cells(&self) -> usize {
        self.m_offsets.len() - 1
    }

    pub fn rows(&self) -> usize {
        self.a.rows()
    }

    /// Checks `h = M·g ⊕ const` cell by cell and every row over the integers
    /// (`i128`; rows with wider values are reported as errors). `h` is the
    /// witness backends' integer witness: entry 0 the constant one, cell `i`
    /// at entry `i + 1`.
    pub fn check_witness(&self, g: &crate::witgen::PackedWitness, h: &crate::witgen::PackedWitness) -> Result<(), String> {
        if g.bit_len() != self.committed || h.bit_len() != self.cells() + 1 {
            return Err(format!(
                "witness shape: {} committed vs {}, {} cells vs {}",
                g.bit_len(),
                self.committed,
                h.bit_len(),
                self.cells() + 1
            ));
        }
        for i in 0..self.cells() {
            let (s, e) = (self.m_offsets[i] as usize, self.m_offsets[i + 1] as usize);
            let v = self.m_terms[s..e].iter().fold(self.m_const[i], |acc, &j| acc ^ g.bit(j as usize));
            if v != h.bit(i + 1) {
                return Err(format!("cell {i}: M·g = {v}"));
            }
        }
        let value = |rows: &CompactRows, r: usize| -> Option<i128> {
            let mut acc = 0i128;
            for (col, coeff) in rows.row(r) {
                if h.bit(col as usize) {
                    acc = acc.checked_add(coeff.as_small()?)?;
                }
            }
            Some(acc)
        };
        for r in 0..self.rows() {
            let (a, b, c) = (value(&self.a, r), value(&self.b, r), value(&self.c, r));
            match (a, b, c) {
                (Some(a), Some(b), Some(c)) => {
                    if a.checked_mul(b) != Some(c) {
                        return Err(format!("row {r}: {a} * {b} != {c}"));
                    }
                }
                _ => return Err(format!("row {r}: values exceed i128")),
            }
        }
        Ok(())
    }
}

/// Placeholder ids for block input slots: `PLACEHOLDER + slot` (above every
/// committed index a recording can reach).
pub const PLACEHOLDER: u32 = 0xF000_0000;

/// Ids of lazily recorded linear outputs: `LAZY + 128·group + bit` (see
/// [`LinearGroup`]); below [`PLACEHOLDER`], above every committed index.
pub const LAZY: u32 = 0xE000_0000;

/// A dense F2-linear image of 128-bit inputs, kept unexpanded: output bit
/// `i` is `constant_i ⊕ XOR_{(k, j) : bit i of terms[k].0[j]} terms[k].1[j]`.
/// Recording backends return its outputs as single `LAZY` ids, so a dense
/// public-coefficient combination costs its inputs, not `64` support terms
/// per input bit.
#[derive(Clone, Debug, Default)]
pub struct LinearGroup {
    pub terms: Vec<([u128; 128], Vec<CBit>)>,
    pub constant: u128,
}

impl LinearGroup {
    /// Output bit `i` as an explicit combination.
    pub fn expand_bit(&self, i: usize) -> CBit {
        let mut terms: Vec<u32> = Vec::new();
        let mut constant = (self.constant >> i) & 1 == 1;
        for (columns, inputs) in &self.terms {
            for (j, &column) in columns.iter().enumerate() {
                if (column >> i) & 1 == 1 {
                    terms.extend_from_slice(&inputs[j].terms);
                    constant ^= inputs[j].constant;
                }
            }
        }
        terms.sort_unstable();
        CBit { constant, terms: Terms::from_vec(parity(terms)) }
    }
}

/// Keeps the ids that occur an odd number of times in a sorted list.
fn parity(terms: Vec<u32>) -> Vec<u32> {
    let mut kept = Vec::with_capacity(terms.len());
    let mut a = 0;
    while a < terms.len() {
        let mut e = a + 1;
        while e < terms.len() && terms[e] == terms[a] {
            e += 1;
        }
        if (e - a) % 2 == 1 {
            kept.push(terms[a]);
        }
        a = e;
    }
    kept
}

/// The local relation every instance of one block kind records.
///
/// Local index space of `M` supports and outputs: `h < hints` is the block's
/// `h`-th committed bit, `hints + j` its input slot `j`. Row columns: 0 is
/// the constant, `c + 1` local cell `c`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockTemplate {
    pub kind: &'static str,
    pub slots: usize,
    pub hints: usize,
    pub m_offsets: Vec<u32>,
    pub m_terms: Vec<u32>,
    pub m_const: Vec<bool>,
    pub a: CompactRows,
    pub b: CompactRows,
    pub c: CompactRows,
    pub row_limbs: Vec<u16>,
    pub outputs: Vec<CBit>,
}

impl BlockTemplate {
    pub fn cells(&self) -> usize {
        self.m_const.len()
    }

    pub fn rows(&self) -> usize {
        self.row_limbs.len()
    }

    pub fn m_nnz(&self) -> usize {
        self.m_terms.len()
    }

    /// Support of local cell `c`.
    pub fn cell(&self, c: usize) -> &[u32] {
        &self.m_terms[self.m_offsets[c] as usize..self.m_offsets[c + 1] as usize]
    }
}

/// One recorded instance of a template.
#[derive(Clone, Debug)]
pub struct BlockInstance {
    pub template: u16,
    /// Global index of the block's first committed bit, cell and row.
    pub hint_base: u32,
    pub cell_base: u32,
    pub row_base: u32,
    /// Slot `j`'s value in the enclosing circuit (global committed supports).
    pub inputs: Vec<CBit>,
}

/// A recording split into repeated blocks and the remaining "tail".
///
/// Global indices are the witness backends' emission orders (hints, `bitz`
/// calls, rows), so `flatten` reproduces the plain recording exactly.
#[derive(Clone, Debug, Default)]
pub struct StructuredCircuit {
    pub committed: usize,
    pub cells: usize,
    pub rows: usize,
    /// Shared, so a recorder's forks (see [`Circuit::map_independent`]) see
    /// them without copying.
    pub templates: Vec<Arc<BlockTemplate>>,
    pub instances: Vec<BlockInstance>,
    /// Committed bits allocated outside blocks (ascending global indices).
    pub tail_hints: Vec<u32>,
    /// Tail cells: global indices and `M` rows over global committed bits.
    pub tail_cells: Vec<u32>,
    pub tail_m_offsets: Vec<u64>,
    pub tail_m_terms: Vec<u32>,
    pub tail_m_const: Vec<bool>,
    /// Tail rows: global indices, over global cell columns (`c + 1`).
    pub tail_rows: Vec<u32>,
    pub tail_a: CompactRows,
    pub tail_b: CompactRows,
    pub tail_c: CompactRows,
    pub tail_row_limbs: Vec<u16>,
    /// Lazy linear groups referenced by tail cells (`LAZY` ids).
    pub groups: Vec<LinearGroup>,
}

fn substitute(
    local: &[u32],
    constant: bool,
    hint_base: u32,
    hints: u32,
    inputs: &[CBit],
) -> CBit {
    // Fast path: no slot in the support (sorted, so its last id is a hint).
    if local.last().is_none_or(|&t| t < hints) {
        return CBit { constant, terms: local.iter().map(|&t| hint_base + t).collect() };
    }
    let mut terms: Vec<u32> = Vec::with_capacity(local.len());
    let mut constant = constant;
    let mut expanded: Vec<u32> = Vec::new();
    for &t in local {
        if t < hints {
            terms.push(hint_base + t);
        } else {
            let input = &inputs[(t - hints) as usize];
            constant ^= input.constant;
            expanded.extend_from_slice(&input.terms);
        }
    }
    if expanded.is_empty() {
        terms.sort_unstable();
        return CBit { constant, terms: Terms::from_vec(terms) };
    }
    terms.extend(expanded);
    terms.sort_unstable();
    // Cancel pairs (an XOR of supports).
    let mut out = Vec::with_capacity(terms.len());
    let mut i = 0;
    while i < terms.len() {
        let mut j = i + 1;
        while j < terms.len() && terms[j] == terms[i] {
            j += 1;
        }
        if (j - i) % 2 == 1 {
            out.push(terms[i]);
        }
        i = j;
    }
    CBit { constant, terms: Terms::from_vec(out) }
}

impl StructuredCircuit {
    /// Instance `i`'s cell `c`, in the enclosing circuit.
    pub fn instance_cell(&self, i: usize, c: usize) -> CBit {
        let inst = &self.instances[i];
        let t = &self.templates[inst.template as usize];
        substitute(t.cell(c), t.m_const[c], inst.hint_base, t.hints as u32, &inst.inputs)
    }

    /// Instance `i`'s outputs, in the enclosing circuit.
    pub fn instance_outputs(&self, i: usize) -> Vec<CBit> {
        let inst = &self.instances[i];
        let t = &self.templates[inst.template as usize];
        t.outputs
            .iter()
            .map(|o| substitute(&o.terms, o.constant, inst.hint_base, t.hints as u32, &inst.inputs))
            .collect()
    }

    pub fn tail_m_nnz(&self) -> usize {
        self.tail_m_terms.len()
    }

    /// Tail cell `k` with its lazy linear outputs expanded.
    pub fn tail_cell(&self, k: usize) -> CBit {
        let (s, e) = (self.tail_m_offsets[k] as usize, self.tail_m_offsets[k + 1] as usize);
        let raw = &self.tail_m_terms[s..e];
        let mut constant = self.tail_m_const[k];
        if raw.iter().all(|&t| !(LAZY..PLACEHOLDER).contains(&t)) {
            return CBit { constant, terms: Terms::from_slice(raw) };
        }
        let mut terms = Vec::new();
        for &t in raw {
            if (LAZY..PLACEHOLDER).contains(&t) {
                let id = (t - LAZY) as usize;
                let bit = self.groups[id / 128].expand_bit(id % 128);
                constant ^= bit.constant;
                terms.extend(bit.terms);
            } else {
                terms.push(t);
            }
        }
        terms.sort_unstable();
        CBit { constant, terms: Terms::from_vec(parity(terms)) }
    }

    /// Expanded `M` nonzeros of the lazy groups (what they would cost eagerly).
    pub fn group_m_nnz(&self) -> usize {
        self.groups
            .iter()
            .map(|g| g.terms.iter().map(|(cols, ins)| cols.iter().zip(ins).map(|(c, b)| c.count_ones() as usize * b.terms.len()).sum::<usize>()).sum::<usize>())
            .sum()
    }

    /// A digest of the whole recording (every index, support, row and
    /// group), for checking that two recorders produce the same circuit.
    pub fn digest(&self) -> [u8; 32] {
        fn words(h: &mut Vec<u8>, xs: impl IntoIterator<Item = u64>) {
            for x in xs {
                h.extend_from_slice(&x.to_le_bytes());
            }
        }
        fn bit(h: &mut Vec<u8>, b: &CBit) {
            h.push(b.constant as u8);
            words(h, [b.terms.len() as u64]);
            words(h, b.terms.iter().map(|&t| u64::from(t)));
        }
        fn rows(h: &mut Vec<u8>, r: &CompactRows) {
            words(h, [r.offsets.len() as u64]);
            words(h, r.offsets.iter().copied());
            words(h, r.columns.iter().map(|&c| u64::from(c)));
            for c in &r.coeffs {
                match *c {
                    Coeff::Pow2 { neg, shift } => {
                        h.extend_from_slice(&[0, neg as u8]);
                        h.extend_from_slice(&shift.to_le_bytes());
                    }
                    Coeff::Small(v) => {
                        h.push(1);
                        h.extend_from_slice(&v.to_le_bytes());
                    }
                }
            }
        }
        let mut h: Vec<u8> = Vec::new();
        h.extend_from_slice(b"circuit/structured/v1");
        words(&mut h, [self.committed, self.cells, self.rows, self.templates.len(), self.instances.len()].map(|x| x as u64));
        for t in &self.templates {
            h.extend_from_slice(t.kind.as_bytes());
            words(&mut h, [t.slots, t.hints, t.m_offsets.len(), t.m_terms.len(), t.outputs.len()].map(|x| x as u64));
            words(&mut h, t.m_offsets.iter().map(|&x| u64::from(x)));
            words(&mut h, t.m_terms.iter().map(|&x| u64::from(x)));
            h.extend(t.m_const.iter().map(|&b| b as u8));
            for r in [&t.a, &t.b, &t.c] {
                rows(&mut h, r);
            }
            words(&mut h, t.row_limbs.iter().map(|&x| u64::from(x)));
            for o in &t.outputs {
                bit(&mut h, o);
            }
        }
        for inst in &self.instances {
            words(&mut h, [inst.template, 0].map(u64::from));
            words(&mut h, [inst.hint_base, inst.cell_base, inst.row_base].map(u64::from));
            words(&mut h, [inst.inputs.len() as u64]);
            for b in &inst.inputs {
                bit(&mut h, b);
            }
        }
        for v in [&self.tail_hints, &self.tail_cells, &self.tail_m_terms, &self.tail_rows] {
            words(&mut h, [v.len() as u64]);
            words(&mut h, v.iter().map(|&x| u64::from(x)));
        }
        words(&mut h, self.tail_m_offsets.iter().copied());
        h.extend(self.tail_m_const.iter().map(|&b| b as u8));
        for r in [&self.tail_a, &self.tail_b, &self.tail_c] {
            rows(&mut h, r);
        }
        words(&mut h, self.tail_row_limbs.iter().map(|&x| u64::from(x)));
        words(&mut h, [self.groups.len() as u64]);
        for g in &self.groups {
            h.extend_from_slice(&g.constant.to_le_bytes());
            words(&mut h, [g.terms.len() as u64]);
            for (columns, inputs) in &g.terms {
                for c in columns {
                    h.extend_from_slice(&c.to_le_bytes());
                }
                for b in inputs {
                    bit(&mut h, b);
                }
            }
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update_rayon(&h);
        *hasher.finalize().as_bytes()
    }

    /// `M` nonzeros of the flat recording.
    pub fn flat_m_nnz(&self) -> usize {
        let mut nnz = self.tail_m_terms.len();
        for i in 0..self.instances.len() {
            let t = &self.templates[self.instances[i].template as usize];
            for c in 0..t.cells() {
                nnz += self.instance_cell(i, c).terms.len();
            }
        }
        nnz
    }

    /// The plain recording: every cell and row in global order.
    pub fn flatten(&self) -> CompactCircuit {
        let mut out = CompactCircuit {
            committed: self.committed,
            m_offsets: vec![0],
            a: CompactRows::new(),
            b: CompactRows::new(),
            c: CompactRows::new(),
            ..CompactCircuit::default()
        };
        // Cells: instances and tail cells merged by global index.
        let (mut next_inst, mut next_tail) = (0usize, 0usize);
        for g in 0..self.cells as u32 {
            if next_tail < self.tail_cells.len() && self.tail_cells[next_tail] == g {
                let cell = self.tail_cell(next_tail);
                out.m_terms.extend_from_slice(&cell.terms);
                out.m_const.push(cell.constant);
                out.m_offsets.push(out.m_terms.len() as u64);
                next_tail += 1;
                continue;
            }
            while next_inst < self.instances.len() {
                let inst = &self.instances[next_inst];
                let t = &self.templates[inst.template as usize];
                if g < inst.cell_base + t.cells() as u32 {
                    break;
                }
                next_inst += 1;
            }
            let inst = &self.instances[next_inst];
            let cell = self.instance_cell(next_inst, (g - inst.cell_base) as usize);
            out.m_terms.extend_from_slice(&cell.terms);
            out.m_const.push(cell.constant);
            out.m_offsets.push(out.m_terms.len() as u64);
        }
        // Rows likewise.
        let (mut next_inst, mut next_tail) = (0usize, 0usize);
        let remap = |rows: &CompactRows, r: usize, base: u32| -> CLin {
            CLin {
                constant: None,
                terms: rows
                    .row(r)
                    .map(|(col, coeff)| if col == 0 { (u32::MAX, coeff) } else { (base + col - 1, coeff) })
                    .collect(),
            }
        };
        let push = |dst: &mut CompactRows, lin: CLin| {
            let mut lin = lin;
            let constant: Vec<Coeff> = lin.terms.iter().filter(|t| t.0 == u32::MAX).map(|t| t.1).collect();
            lin.terms.retain(|t| t.0 != u32::MAX);
            lin.constant = constant.into_iter().reduce(|a, b| a + b).filter(|c| !c.is_zero());
            dst.push(lin);
        };
        for g in 0..self.rows as u32 {
            if next_tail < self.tail_rows.len() && self.tail_rows[next_tail] == g {
                for (src, dst) in [(&self.tail_a, &mut out.a), (&self.tail_b, &mut out.b), (&self.tail_c, &mut out.c)] {
                    push(dst, remap(src, next_tail, 0));
                }
                out.row_limbs.push(self.tail_row_limbs[next_tail]);
                next_tail += 1;
                continue;
            }
            while next_inst < self.instances.len() {
                let inst = &self.instances[next_inst];
                let t = &self.templates[inst.template as usize];
                if g < inst.row_base + t.rows() as u32 {
                    break;
                }
                next_inst += 1;
            }
            let inst = &self.instances[next_inst];
            let t = &self.templates[inst.template as usize];
            let r = (g - inst.row_base) as usize;
            for (src, dst) in [(&t.a, &mut out.a), (&t.b, &mut out.b), (&t.c, &mut out.c)] {
                push(dst, remap(src, r, inst.cell_base));
            }
            out.row_limbs.push(t.row_limbs[r]);
        }
        out
    }
}

/// A block being recorded.
#[derive(Debug)]
struct OpenBlock {
    kind: &'static str,
    inputs: Vec<CBit>,
    hint_base: u32,
    cell_base: u32,
    row_base: u32,
    m_offsets: Vec<u32>,
    m_terms: Vec<u32>,
    m_const: Vec<bool>,
    a: CompactRows,
    b: CompactRows,
    c: CompactRows,
    row_limbs: Vec<u16>,
}

/// The recording backend.
#[derive(Debug)]
pub struct CompactRecorder {
    out: StructuredCircuit,
    block: Option<OpenBlock>,
    /// Replay known templates instead of recording block bodies again (see
    /// [`Circuit::try_replay_block`]); off, every instance is recorded and
    /// checked against its template.
    pub replay: bool,
    /// Record dense linear maps as [`LinearGroup`]s (outside blocks).
    pub lazy: bool,
    /// Record [`Circuit::map_independent`] calls on the thread pool.
    pub parallel: bool,
    /// Lazy group ids start here (a fork's: its parent's count at the fork).
    group_base: usize,
    phase: &'static str,
    mark: (usize, usize),
    /// Running `(cells, M nnz)` including block internals, for the tallies.
    totals: (usize, usize),
    /// `(phase, cells, M nnz)` tallies (see [`Circuit::phase`]).
    pub phases: Vec<(&'static str, usize, usize)>,
    /// Record the largest-support cell per phase: `(phase, terms, cell)`.
    pub widest: Vec<(&'static str, usize, usize)>,
}

impl Default for CompactRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl CompactRecorder {
    pub fn new() -> Self {
        Self {
            out: StructuredCircuit {
                tail_m_offsets: vec![0],
                tail_a: CompactRows::new(),
                tail_b: CompactRows::new(),
                tail_c: CompactRows::new(),
                ..StructuredCircuit::default()
            },
            block: None,
            replay: true,
            lazy: true,
            parallel: true,
            group_base: 0,
            phase: "(start)",
            mark: (0, 0),
            totals: (0, 0),
            phases: Vec::new(),
            widest: Vec::new(),
        }
    }

    fn close_phase(&mut self) {
        let now = self.totals;
        let (dc, dn) = (now.0 - self.mark.0, now.1 - self.mark.1);
        match self.phases.iter_mut().find(|p| p.0 == self.phase) {
            Some(p) => {
                p.1 += dc;
                p.2 += dn;
            }
            None => self.phases.push((self.phase, dc, dn)),
        }
        self.mark = now;
    }

    /// The structured recording.
    pub fn finish_structured(mut self) -> StructuredCircuit {
        assert!(self.block.is_none(), "unclosed block");
        self.close_phase();
        self.out
    }

    /// The plain recording.
    pub fn finish(self) -> CompactCircuit {
        self.finish_structured().flatten()
    }

    /// Finishes, returning the per-phase tallies too.
    pub fn finish_with_phases(
        mut self,
    ) -> (StructuredCircuit, Vec<(&'static str, usize, usize)>, Vec<(&'static str, usize, usize)>) {
        assert!(self.block.is_none(), "unclosed block");
        self.close_phase();
        let phases = std::mem::take(&mut self.phases);
        let widest = std::mem::take(&mut self.widest);
        (self.out, phases, widest)
    }

    fn note_width(&mut self, width: usize, index: usize) {
        match self.widest.iter_mut().find(|w| w.0 == self.phase) {
            Some(w) if w.1 < width => {
                w.1 = width;
                w.2 = index;
            }
            Some(_) => {}
            None => self.widest.push((self.phase, width, index)),
        }
    }

    /// The counts `(committed, cells, rows, lazy groups)` a fork starts from.
    fn base(&self) -> (usize, usize, usize, usize) {
        (self.out.committed, self.out.cells, self.out.rows, self.group_base + self.out.groups.len())
    }

    /// A recorder continuing this one's index spaces from its current counts,
    /// for a sub-circuit recorded on another thread (see
    /// [`Circuit::map_independent`]).
    fn fork(&self) -> Self {
        let mut sub = Self::new();
        let (committed, cells, rows, groups) = self.base();
        sub.out.committed = committed;
        sub.out.cells = cells;
        sub.out.rows = rows;
        sub.out.templates = self.out.templates.clone();
        sub.group_base = groups;
        sub.replay = self.replay;
        sub.lazy = self.lazy;
        sub.parallel = false;
        sub.phase = self.phase;
        sub
    }

    /// Moves a fork's own indices (those from `base` on), in its recording
    /// and in the handles `returned` from it, up by `by` (committed, cells,
    /// rows, groups): the sizes of the forks spliced before it.
    fn shift(&mut self, returned: &mut [Vec<CBit>], base: (usize, usize, usize, usize), by: (u32, u32, u32, u32)) {
        let (c0, cells0) = (base.0 as u32, base.1 as u32);
        let lazy0 = LAZY + 128 * base.3 as u32;
        let (dc, dcell, drow, dg) = by;
        let term = |t: u32| -> u32 {
            if t < LAZY {
                if t >= c0 { t + dc } else { t }
            } else if t < PLACEHOLDER {
                if t >= lazy0 { t + 128 * dg } else { t }
            } else {
                t
            }
        };
        let bits = |bits: &mut [CBit]| {
            for b in bits {
                for t in b.terms.iter_mut() {
                    *t = term(*t);
                }
            }
        };
        for r in returned {
            bits(r);
        }
        let out = &mut self.out;
        for inst in &mut out.instances {
            inst.hint_base += dc;
            inst.cell_base += dcell;
            inst.row_base += drow;
            bits(&mut inst.inputs);
        }
        out.tail_hints.iter_mut().for_each(|h| *h += dc);
        out.tail_cells.iter_mut().for_each(|c| *c += dcell);
        out.tail_m_terms.iter_mut().for_each(|t| *t = term(*t));
        out.tail_rows.iter_mut().for_each(|r| *r += drow);
        for rows in [&mut out.tail_a, &mut out.tail_b, &mut out.tail_c] {
            // Column `cell + 1`; 0 is the constant.
            for col in &mut rows.columns {
                if *col > cells0 {
                    *col += dcell;
                }
            }
        }
        for group in &mut out.groups {
            for (_, inputs) in &mut group.terms {
                bits(inputs);
            }
        }
    }

    /// Splices a shifted fork (forked at `base` with `templates` templates).
    fn append(&mut self, sub: Self, base: (usize, usize, usize, usize), templates: usize) {
        let mut part = sub.out;
        let map: Vec<u16> = part
            .templates
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if i < templates {
                    return i as u16;
                }
                match self.out.templates.iter().position(|p| p.kind == t.kind) {
                    Some(p) => {
                        assert!(self.out.templates[p] == *t, "instance of `{}` differs from its template", t.kind);
                        p as u16
                    }
                    None => {
                        self.out.templates.push(t.clone());
                        (self.out.templates.len() - 1) as u16
                    }
                }
            })
            .collect();
        for inst in &mut part.instances {
            inst.template = map[inst.template as usize];
        }
        let out = &mut self.out;
        out.committed += part.committed - base.0;
        out.cells += part.cells - base.1;
        out.rows += part.rows - base.2;
        out.instances.extend(part.instances);
        out.tail_hints.extend(part.tail_hints);
        out.tail_cells.extend(part.tail_cells);
        let terms = out.tail_m_terms.len() as u64;
        out.tail_m_offsets.extend(part.tail_m_offsets[1..].iter().map(|o| o + terms));
        out.tail_m_terms.extend(part.tail_m_terms);
        out.tail_m_const.extend(part.tail_m_const);
        out.tail_rows.extend(part.tail_rows);
        for (dst, src) in [(&mut out.tail_a, part.tail_a), (&mut out.tail_b, part.tail_b), (&mut out.tail_c, part.tail_c)] {
            let columns = dst.columns.len() as u64;
            dst.offsets.extend(src.offsets[1..].iter().map(|o| o + columns));
            dst.columns.extend(src.columns);
            dst.coeffs.extend(src.coeffs);
        }
        out.tail_row_limbs.extend(part.tail_row_limbs);
        out.groups.extend(part.groups);
        self.totals.0 += sub.totals.0;
        self.totals.1 += sub.totals.1;
    }
}

impl Circuit for CompactRecorder {
    type Bool = CBit;
    type Coefficient<const LIMBS: usize> = Coeff;
    type Z<const LIMBS: usize> = CLin;

    fn coefficient_from_le_words<const LIMBS: usize>(words: &[u64]) -> Coeff {
        let set: Vec<(usize, u64)> = words
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, w)| *w != 0)
            .collect();
        match set.as_slice() {
            [] => Coeff::zero(),
            [(i, w)] if w.is_power_of_two() => Coeff::Pow2 {
                neg: false,
                shift: (64 * i + w.trailing_zeros() as usize) as u16,
            },
            _ => {
                assert!(words.iter().skip(2).all(|w| *w == 0), "wide constant coefficient");
                let v = u128::from(words[0]) | (u128::from(words.get(1).copied().unwrap_or(0)) << 64);
                Coeff::Small(i128::try_from(v).expect("coefficient fits i128")).normalize()
            }
        }
    }

    fn xor(&mut self, lhs: CBit, rhs: CBit) -> CBit {
        CBit {
            constant: lhs.constant ^ rhs.constant,
            terms: if lhs.terms.is_empty() {
                rhs.terms
            } else if rhs.terms.is_empty() {
                lhs.terms
            } else {
                Terms::from_vec(xor_sorted(&lhs.terms, &rhs.terms))
            },
        }
    }

    fn xor_many(&mut self, bits: Vec<CBit>) -> CBit {
        let mut constant = false;
        let mut terms: Vec<u32> = Vec::with_capacity(bits.iter().map(|b| b.terms.len()).sum());
        for b in bits {
            constant ^= b.constant;
            terms.extend(b.terms);
        }
        terms.sort_unstable();
        let mut out = Vec::with_capacity(terms.len());
        let mut i = 0;
        while i < terms.len() {
            let mut j = i + 1;
            while j < terms.len() && terms[j] == terms[i] {
                j += 1;
            }
            if (j - i) % 2 == 1 {
                out.push(terms[i]);
            }
            i = j;
        }
        CBit { constant, terms: Terms::from_vec(out) }
    }

    fn linear_maps_128(&mut self, maps: &[(&[CBit; 128], [u128; 128])], constant: u128) -> [CBit; 128] {
        if self.lazy && self.block.is_none() {
            let id = self.group_base + self.out.groups.len();
            assert!(LAZY as usize + 128 * (id + 1) <= PLACEHOLDER as usize, "lazy id space exhausted");
            self.out.groups.push(LinearGroup {
                terms: maps.iter().map(|(input, columns)| (*columns, input.to_vec())).collect(),
                constant: 0,
            });
            let base = LAZY + 128 * id as u32;
            return core::array::from_fn(|i| CBit {
                constant: (constant >> i) & 1 == 1,
                terms: smallvec::smallvec![base + i as u32],
            });
        }
        // One index vector per output bit, filled straight from the inputs'
        // supports (no per-bit `CBit` clones), then parity-merged.
        let mut per_bit: Vec<Vec<u32>> = (0..128).map(|_| Vec::new()).collect();
        let mut constants = constant;
        for (input, columns) in maps {
            for (j, &column) in columns.iter().enumerate() {
                let b = &input[j];
                if b.terms.is_empty() && !b.constant {
                    continue;
                }
                let mut col = column;
                while col != 0 {
                    let i = col.trailing_zeros() as usize;
                    per_bit[i].extend_from_slice(&b.terms);
                    if b.constant {
                        constants ^= 1u128 << i;
                    }
                    col &= col - 1;
                }
            }
        }
        let mut out: Vec<CBit> = per_bit
            .into_iter()
            .enumerate()
            .map(|(i, mut terms)| {
                terms.sort_unstable();
                let mut kept = Vec::with_capacity(terms.len());
                let mut a = 0;
                while a < terms.len() {
                    let mut e = a + 1;
                    while e < terms.len() && terms[e] == terms[a] {
                        e += 1;
                    }
                    if (e - a) % 2 == 1 {
                        kept.push(terms[a]);
                    }
                    a = e;
                }
                CBit { constant: (constants >> i) & 1 == 1, terms: Terms::from_vec(kept) }
            })
            .collect();
        core::array::from_fn(|i| std::mem::take(&mut out[i]))
    }

    fn hint<const LIMBS: usize, const N: usize, const M: usize, H>(&mut self, _: H) -> ScalarBits<CBit, N>
    where
        H: Fn(&dyn WitnessContext<CLin, CBit, Coeff>) -> HintResult<PackedBits<N, M>>
            + Send
            + Sync
            + 'static,
    {
        let base = self.out.committed as u32;
        assert!((base as usize + N) < LAZY as usize, "committed index space exhausted");
        self.out.committed += N;
        if self.block.is_none() {
            self.out.tail_hints.extend(base..base + N as u32);
        }
        ScalarBits(std::array::from_fn(|i| CBit {
            constant: false,
            terms: smallvec::smallvec![base + i as u32],
        }))
    }

    fn phase(&mut self, label: &'static str) {
        self.close_phase();
        self.phase = label;
    }

    fn bitz<const LIMBS: usize>(&mut self, value: CBit) -> CLin {
        let index = self.out.cells as u32;
        self.out.cells += 1;
        self.totals.0 += 1;
        self.totals.1 += value.terms.len();
        self.note_width(value.terms.len(), index as usize);
        match &mut self.block {
            Some(b) => {
                b.m_terms.extend_from_slice(&value.terms);
                b.m_offsets.push(b.m_terms.len() as u32);
                b.m_const.push(value.constant);
            }
            None => {
                self.out.tail_cells.push(index);
                self.out.tail_m_terms.extend_from_slice(&value.terms);
                self.out.tail_m_offsets.push(self.out.tail_m_terms.len() as u64);
                self.out.tail_m_const.push(value.constant);
            }
        }
        CLin {
            constant: None,
            terms: vec![(index, Coeff::one())],
        }
    }

    fn assert_r1c<const LIMBS: usize>(&mut self, a: CLin, b: CLin, c: CLin) {
        let index = self.out.rows as u32;
        self.out.rows += 1;
        match &mut self.block {
            Some(block) => {
                block.a.push(a);
                block.b.push(b);
                block.c.push(c);
                block.row_limbs.push(LIMBS as u16);
            }
            None => {
                self.out.tail_rows.push(index);
                self.out.tail_a.push(a);
                self.out.tail_b.push(b);
                self.out.tail_c.push(c);
                self.out.tail_row_limbs.push(LIMBS as u16);
            }
        }
    }

    fn sign_extend_z<const FROM_LIMBS: usize, const TO_LIMBS: usize>(&mut self, v: CLin) -> CLin {
        v
    }

    fn begin_block(&mut self, kind: &'static str, inputs: Vec<CBit>) -> Vec<CBit> {
        assert!(self.block.is_none(), "blocks do not nest");
        let slots = inputs.len();
        self.block = Some(OpenBlock {
            kind,
            inputs,
            hint_base: self.out.committed as u32,
            cell_base: self.out.cells as u32,
            row_base: self.out.rows as u32,
            m_offsets: vec![0],
            m_terms: Vec::new(),
            m_const: Vec::new(),
            a: CompactRows::new(),
            b: CompactRows::new(),
            c: CompactRows::new(),
            row_limbs: Vec::new(),
        });
        (0..slots as u32)
            .map(|j| CBit {
                constant: false,
                terms: smallvec::smallvec![PLACEHOLDER + j],
            })
            .collect()
    }

    fn try_replay_block(&mut self, kind: &'static str, inputs: Vec<CBit>) -> Result<Vec<CBit>, Vec<CBit>> {
        if !self.replay || self.block.is_some() {
            return Err(inputs);
        }
        let Some(id) = self.out.templates.iter().position(|t| t.kind == kind) else {
            return Err(inputs);
        };
        let t = &self.out.templates[id];
        let (hints, cells, rows, nnz) = (t.hints, t.cells(), t.rows(), t.m_nnz());
        assert_eq!(inputs.len(), t.slots, "block input count");
        let (hint_base, cell_base, row_base) = (self.out.committed as u32, self.out.cells as u32, self.out.rows as u32);
        self.out.committed += hints;
        self.out.cells += cells;
        self.out.rows += rows;
        self.totals.0 += cells;
        self.totals.1 += nnz;
        self.out.instances.push(BlockInstance { template: id as u16, hint_base, cell_base, row_base, inputs });
        Ok(self.out.instance_outputs(self.out.instances.len() - 1))
    }

    fn end_block(&mut self, outputs: Vec<CBit>) -> Vec<CBit> {
        let block = self.block.take().expect("end_block without begin_block");
        let hints = self.out.committed as u32 - block.hint_base;
        let slots = block.inputs.len() as u32;
        let localize = |t: u32| -> u32 {
            if t >= PLACEHOLDER {
                assert!(t - PLACEHOLDER < slots);
                hints + (t - PLACEHOLDER)
            } else {
                assert!(
                    t >= block.hint_base && t < block.hint_base + hints,
                    "a block references a committed bit outside its inputs"
                );
                t - block.hint_base
            }
        };
        // Localized supports stay sorted: hints map monotonically below the
        // slots, and placeholders sort after every committed index.
        let m_terms: Vec<u32> = block.m_terms.iter().map(|&t| localize(t)).collect();
        let cells = block.m_const.len() as u32;
        let relocate = |rows: &CompactRows| -> CompactRows {
            let mut out = rows.clone();
            for col in &mut out.columns {
                if *col != 0 {
                    let cell = *col - 1;
                    assert!(
                        cell >= block.cell_base && cell < block.cell_base + cells,
                        "a block row references a cell outside the block"
                    );
                    *col = cell - block.cell_base + 1;
                }
            }
            out
        };
        let local_outputs: Vec<CBit> = outputs
            .iter()
            .map(|o| CBit {
                constant: o.constant,
                terms: o.terms.iter().map(|&t| localize(t)).collect(),
            })
            .collect();
        let template = BlockTemplate {
            kind: block.kind,
            slots: slots as usize,
            hints: hints as usize,
            m_offsets: block.m_offsets.clone(),
            m_terms,
            m_const: block.m_const.clone(),
            a: relocate(&block.a),
            b: relocate(&block.b),
            c: relocate(&block.c),
            row_limbs: block.row_limbs.clone(),
            outputs: local_outputs,
        };
        let id = match self.out.templates.iter().position(|t| t.kind == block.kind) {
            Some(id) => {
                assert!(*self.out.templates[id] == template, "instance of `{}` differs from its template", block.kind);
                id
            }
            None => {
                self.out.templates.push(Arc::new(template));
                self.out.templates.len() - 1
            }
        };
        let inst = BlockInstance {
            template: id as u16,
            hint_base: block.hint_base,
            cell_base: block.cell_base,
            row_base: block.row_base,
            inputs: block.inputs,
        };
        self.out.instances.push(inst);
        self.out.instance_outputs(self.out.instances.len() - 1)
    }

    /// Forks one recorder per chunk of calls, records them on the thread
    /// pool, then shifts each fork's own indices (and the handles its calls
    /// return) past the forks before it and splices them in order: the
    /// sequential recording exactly.
    fn map_independent(&mut self, count: usize, body: &(dyn Fn(&mut Self, usize) -> Vec<CBit> + Sync)) -> Vec<Vec<CBit>> {
        if !self.parallel || self.block.is_some() || count < 2 {
            return (0..count).map(|k| body(self, k)).collect();
        }
        let chunks = count.min(2 * rayon::current_num_threads());
        let base = self.base();
        let templates = self.out.templates.len();
        let this = &*self;
        let mut subs: Vec<(CompactRecorder, Vec<Vec<CBit>>)> = (0..chunks)
            .into_par_iter()
            .map(|c| {
                let mut sub = this.fork();
                let returned = (c * count / chunks..(c + 1) * count / chunks).map(|k| body(&mut sub, k)).collect();
                (sub, returned)
            })
            .collect();
        let mut at = (0u32, 0u32, 0u32, 0u32);
        let shifts: Vec<(u32, u32, u32, u32)> = subs
            .iter()
            .map(|(sub, _)| {
                let by = at;
                at.0 += (sub.out.committed - base.0) as u32;
                at.1 += (sub.out.cells - base.1) as u32;
                at.2 += (sub.out.rows - base.2) as u32;
                at.3 += sub.out.groups.len() as u32;
                by
            })
            .collect();
        subs.par_iter_mut().zip(shifts).for_each(|((sub, returned), by)| sub.shift(returned, base, by));
        let mut out = Vec::with_capacity(count);
        for (sub, returned) in subs {
            self.append(sub, base, templates);
            out.extend(returned);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::ConstraintGenerator;
    use crate::sha256;

    /// Block compressions: one template whatever the inputs (constants,
    /// words, overlapping supports), and the flattened recording is the
    /// witness's.
    #[test]
    fn blake3_blocks_share_one_template_and_flatten_to_the_witness() {
        use crate::blake3::{self, Input};
        use crate::sha256::Word;
        use crate::witgen::CheckingWitgen;
        fn run<CS: Circuit>(cs: &mut CS) -> Vec<CS::Bool> {
            let w = cs.hint::<1, 512, 8, _>(|_| {
                HintResult::Ok(PackedBits::from_fn(|i| (i * 7 + i / 5) % 3 == 0))
            });
            let word = |k: usize| Word::new(std::array::from_fn(|b| crate::BoolRepresentation::bit(&w, 32 * k + b)));
            let cv: [Input<CS::Bool>; 8] = std::array::from_fn(|i| if i % 3 == 0 { Input::Const(blake3::IV[i]) } else { Input::Word(word(i)) });
            let block: [Input<CS::Bool>; 16] = std::array::from_fn(|i| if i == 5 { Input::Const(0) } else { Input::Word(word(i)) });
            let out = blake3::compress_block(cs, &cv, &block, 3, 64, blake3::CHUNK_START);
            // Chain: the second block reads the first one's output.
            let cv2: [Input<CS::Bool>; 8] = std::array::from_fn(|i| Input::Word(out[i].clone()));
            let block2: [Input<CS::Bool>; 16] = std::array::from_fn(|i| Input::Word(if i < 8 { out[8 + i].clone() } else { word(i) }));
            let out2 = blake3::compress_block(cs, &cv2, &block2, 0, 17, blake3::PARENT);
            // A tail check on an output bit.
            let x = cs.xor(out2[0].bit(0), out2[1].bit(3));
            let cell = cs.bitz::<1>(x);
            let _ = cell;
            (0..32).map(|b| out2[2].bit(b)).collect()
        }
        let mut rec = CompactRecorder::new();
        let _ = run(&mut rec);
        let st = rec.finish_structured();
        assert_eq!(st.templates.len(), 1);
        assert_eq!(st.instances.len(), 2);
        assert_eq!(st.templates[0].slots, blake3::BLOCK_SLOTS);
        let flat = st.flatten();
        let mut w = CheckingWitgen::new();
        let outs = run(&mut w);
        assert_eq!(w.first_failure, None);
        // The outputs agree with the native compression.
        let (g, h) = w.into_witnesses();
        flat.check_witness(&g, &h).expect("flattened blocks match the witness");
        let _ = outs;
    }

    /// Forked recording ([`Circuit::map_independent`] on the thread pool):
    /// independent sub-circuits with their own committed bits, blocks, tail
    /// cells, rows and lazy groups, whose returned handles later code reads,
    /// record exactly the sequential circuit, which flattens to the witness.
    #[test]
    fn forked_recording_is_the_sequential_one() {
        use crate::sha256::Word;
        use crate::witgen::CheckingWitgen;
        use crate::BoolRepresentation;
        fn run<CS: Circuit>(cs: &mut CS) {
            let msg = cs.hint::<1, 512, 8, _>(|_| HintResult::Ok(PackedBits::from_fn(|i| (i * 5 + i / 7) % 3 == 0)));
            let msg: Vec<CS::Bool> = (0..512).map(|i| msg.bit(i)).collect();
            // The first round's forks record the block body, the second's
            // replay the template.
            let returned = cs.map_independent(18, &|cs: &mut CS, k: usize| {
                let w = cs.hint::<1, 32, 1, _>(move |_| HintResult::Ok(PackedBits::from_fn(|i| (i + k) % 4 == 1)));
                let block: [Word<CS::Bool, 32>; 16] =
                    std::array::from_fn(|j| Word::new(std::array::from_fn(|b| if j == 0 { w.bit(b) } else { msg[32 * j + b].clone() })));
                let out = sha256::compress_block(cs, &block, &sha256::initial_state());
                let bits: [CS::Bool; 128] = std::array::from_fn(|i| out[i / 32].bit(i % 32));
                let columns: [u128; 128] = std::array::from_fn(|j| ((j as u128 + 1) * 0x9e37_79b9_7f4a_7c15) ^ ((k as u128) << 64));
                let mixed = cs.linear_maps_128(&[(&bits, columns)], 0x55);
                let cell = cs.bitz::<1>(mixed[3].clone());
                // A bit squares to itself.
                cs.assert_r1c::<1>(cell.clone(), cell.clone(), cell);
                (0..64).map(|b| out[4 + b / 32].bit(b % 32)).collect()
            });
            // Later code reads every call's returned handles.
            for (k, bits) in returned.iter().enumerate() {
                let x = cs.xor(bits[k % 32].clone(), bits[63 - k % 32].clone());
                let _ = cs.bitz::<1>(x);
            }
        }
        fn twice<CS: Circuit>(cs: &mut CS) {
            run(cs);
            run(cs);
        }
        let mut seq = CompactRecorder::new();
        seq.parallel = false;
        twice(&mut seq);
        let mut par = CompactRecorder::new();
        twice(&mut par);
        let (seq, par) = (seq.finish_structured(), par.finish_structured());
        assert_eq!(par.templates.len(), 1);
        assert_eq!(par.instances.len(), 36);
        assert_eq!(par.groups.len(), 36);
        assert_eq!(seq.digest(), par.digest(), "forked recording");
        let mut w = CheckingWitgen::new();
        twice(&mut w);
        assert_eq!(w.first_failure, None);
        let (g, h) = w.into_witnesses();
        par.flatten().check_witness(&g, &h).expect("the forked recording flattens to the witness");
    }

    /// The compact recording of the SHA-256 compression matches the
    /// reference generator row for row.
    #[test]
    fn compact_recording_matches_the_reference_generator() {
        let mut generator = ConstraintGenerator::new(sha256::COMPRESSION_INPUT_BITS);
        let inputs = generator.inputs();
        let _ = sha256::compression_circuit(&mut generator, &inputs);
        let reference = generator.into_matrices();

        let mut rec = CompactRecorder::new();
        let ins = rec.hint::<1, { sha256::COMPRESSION_INPUT_BITS }, 12, _>(|_| unreachable!());
        let _ = sha256::compression_circuit(&mut rec, &ins.0);
        let compact = rec.finish();
        assert_eq!(compact.committed + 1, reference.m.column_count());
        // Reference M row 0 is the implicit constant; cell i is its row i + 1.
        assert_eq!(compact.cells() + 1, reference.m.row_count());
        for i in 0..compact.cells() {
            let row = reference.m.row(i + 1).unwrap();
            let mut expect: Vec<u32> = row.indices().iter().filter(|&&c| c != 0).map(|&c| c as u32 - 1).collect();
            expect.sort_unstable();
            let s = compact.m_offsets[i] as usize;
            let e = compact.m_offsets[i + 1] as usize;
            assert_eq!(&compact.m_terms[s..e], expect.as_slice(), "M row {i}");
            assert_eq!(compact.m_const[i], row.indices().contains(&0), "M const {i}");
        }
        assert_eq!(compact.rows(), reference.c.row_count());
        for r in 0..compact.rows() {
            let row = reference.c.row(r).unwrap();
            let got: Vec<(u32, Vec<u64>)> = compact.c.row(r).map(|(col, c)| (col, c.to_words(1))).collect();
            let expect: Vec<(u32, Vec<u64>)> = row
                .iter()
                .map(|(col, coeff)| (col as u32, coeff.to_vec()))
                .collect();
            assert_eq!(got, expect, "C row {r}");
        }
    }
}
