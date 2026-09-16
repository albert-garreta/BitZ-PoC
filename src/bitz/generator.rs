//! A compact backend for the vendored circuit trait: their
//! `ConstraintGenerator` records every rank-1 constraint as three
//! `BTreeMap<usize, BigInt>` linear combinations and every `f2z` as a
//! `BTreeSet` of Boolean witnesses, a hundred-odd bytes per nonzero and
//! nine gigabytes at 608 SHA-256 blocks. This backend records the same
//! constraints straight into the compact matrices
//! ([`CompactMatrix`]: four bytes of column and two of coefficient code per
//! nonzero) and forgets the Boolean side altogether — `M` comes from the
//! vendored `MTransposeGenerator`, which is compact already. Linear
//! combinations live only while a gadget builds them; a row sorts, merges
//! and drops zeros exactly as their `BTreeMap` did, so the rows are the
//! same entries in the same order and the digests agree.

use std::iter::Sum;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

use circuit::{BoolWitness, Circuit, HintResult, PackedBits, ScalarBits, WitnessContext};
use num_bigint::BigInt;
use num_traits::Zero;

use super::spartan::matrix::{CompactMatrix, CompactMatrixBuilder, IntegerCoefficient, MatrixError};

/// The Boolean side, forgotten: this backend records the Z side only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Forgotten;

impl From<bool> for Forgotten {
    fn from(_: bool) -> Self {
        Self
    }
}

impl BoolWitness for Forgotten {
    type Repr<const N: usize, const M: usize> = ScalarBits<Self, N>;
}

/// An integer linear combination over the `h` variables. Terms accumulate
/// unsorted and possibly repeated; a row normalises them (their `BTreeMap`
/// semantics: sorted by variable, summed, zero sums dropped).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lc {
    constant: BigInt,
    terms: Vec<(u32, BigInt)>,
}

impl Lc {
    fn witness(index: u32) -> Self {
        Self {
            constant: BigInt::zero(),
            terms: vec![(index, BigInt::from(1u64))],
        }
    }

    /// The row: the constant at column 0 if nonzero, then variable `i` at
    /// column `i + 1` with its summed coefficient, zeros dropped, ascending.
    fn into_row(mut self, row: &mut Vec<(usize, IntegerCoefficient)>) {
        row.clear();
        if !self.constant.is_zero() {
            row.push((0, IntegerCoefficient::new(&self.constant)));
        }
        self.terms.sort_unstable_by_key(|(index, _)| *index);
        let mut terms = self.terms.into_iter().peekable();
        while let Some((index, mut coefficient)) = terms.next() {
            while let Some((_, next)) = terms.next_if(|(next_index, _)| *next_index == index) {
                coefficient += next;
            }
            if !coefficient.is_zero() {
                row.push((index as usize + 1, IntegerCoefficient::new(&coefficient)));
            }
        }
    }
}

impl From<BigInt> for Lc {
    fn from(constant: BigInt) -> Self {
        Self {
            constant,
            terms: Vec::new(),
        }
    }
}

impl Zero for Lc {
    fn zero() -> Self {
        Self::default()
    }

    fn is_zero(&self) -> bool {
        if !self.constant.is_zero() {
            return false;
        }
        let mut row = Vec::new();
        self.clone().into_row(&mut row);
        row.is_empty()
    }
}

impl Add for Lc {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl AddAssign for Lc {
    fn add_assign(&mut self, rhs: Self) {
        self.constant += rhs.constant;
        self.terms.extend(rhs.terms);
    }
}

impl Neg for Lc {
    type Output = Self;

    fn neg(mut self) -> Self {
        self.constant = -self.constant;
        for (_, coefficient) in &mut self.terms {
            *coefficient = -std::mem::take(coefficient);
        }
        self
    }
}

impl Sub for Lc {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        self + -rhs
    }
}

impl SubAssign for Lc {
    fn sub_assign(&mut self, rhs: Self) {
        *self += -rhs;
    }
}

impl Mul<BigInt> for Lc {
    type Output = Self;

    fn mul(mut self, rhs: BigInt) -> Self {
        if rhs.is_zero() {
            return Self::zero();
        }
        self.constant *= &rhs;
        for (_, coefficient) in &mut self.terms {
            *coefficient *= &rhs;
        }
        self
    }
}

impl Sum for Lc {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::zero(), Add::add)
    }
}

/// The compact R1CS of a circuit: `A`, `B`, `C` over `h` (`h_len` entries,
/// the constant one first), `f` of `f_len` bits (the constant one first).
#[derive(Debug, Clone)]
pub struct CompactCircuit {
    pub matrices: [CompactMatrix; 3],
    pub h_len: usize,
    pub f_len: usize,
}

/// The backend: their `ConstraintGenerator`'s numbering (inputs first,
/// hints in allocation order; one `h` variable per `f2z` in call order; one
/// row per `assert_r1c`), the rows kept compactly.
#[derive(Debug)]
pub struct CompactGenerator {
    input_witnesses: usize,
    next_boolean_witness: usize,
    h_count: usize,
    builders: [CompactMatrixBuilder; 3],
    row: Vec<(usize, IntegerCoefficient)>,
    error: Option<MatrixError>,
}

impl CompactGenerator {
    pub fn new(input_witnesses: usize) -> Self {
        Self {
            input_witnesses,
            next_boolean_witness: input_witnesses,
            h_count: 0,
            builders: [
                CompactMatrixBuilder::new(),
                CompactMatrixBuilder::new(),
                CompactMatrixBuilder::new(),
            ],
            row: Vec::new(),
            error: None,
        }
    }

    pub fn input(&self, index: usize) -> Forgotten {
        assert!(index < self.input_witnesses, "input witness is not allocated");
        Forgotten
    }

    pub fn inputs(&self) -> Vec<Forgotten> {
        vec![Forgotten; self.input_witnesses]
    }

    /// The matrices, `h_len` and `f_len`.
    pub fn finish(self) -> Result<CompactCircuit, MatrixError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let h_len = self.h_count + 1;
        let [a, b, c] = self.builders;
        Ok(CompactCircuit {
            matrices: [a.finish(h_len)?, b.finish(h_len)?, c.finish(h_len)?],
            h_len,
            f_len: self.next_boolean_witness + 1,
        })
    }

    fn push(&mut self, which: usize, lc: Lc) {
        if self.error.is_some() {
            return;
        }
        lc.into_row(&mut self.row);
        if let Err(error) = self.builders[which]
            .push_row(self.row.iter().map(|(column, coefficient)| (*column, coefficient)))
        {
            self.error = Some(error);
        }
    }
}

impl Circuit for CompactGenerator {
    type Bool = Forgotten;
    type Coefficient<const LIMBS: usize> = BigInt;
    type Z<const LIMBS: usize> = Lc;

    fn xor(&mut self, _: Forgotten, _: Forgotten) -> Forgotten {
        Forgotten
    }

    fn hint<const LIMBS: usize, const N: usize, const M: usize, H>(
        &mut self,
        _: H,
    ) -> ScalarBits<Forgotten, N>
    where
        H: Fn(&dyn WitnessContext<Lc, Forgotten, BigInt>) -> HintResult<PackedBits<N, M>>
            + Send
            + Sync
            + 'static,
    {
        assert_eq!(M, N.div_ceil(64), "incorrect packed limb count");
        self.next_boolean_witness = self
            .next_boolean_witness
            .checked_add(N)
            .expect("Boolean witness count overflow");
        ScalarBits(std::array::from_fn(|_| Forgotten))
    }

    fn f2z<const LIMBS: usize>(&mut self, _: Forgotten) -> Lc {
        let index = u32::try_from(self.h_count).expect("too many h variables for the compact form");
        self.h_count += 1;
        Lc::witness(index)
    }

    fn assert_r1c<const LIMBS: usize>(&mut self, a: Lc, b: Lc, c: Lc) {
        self.push(0, a);
        self.push(1, b);
        self.push(2, c);
    }

    fn sign_extend_z<const FROM_LIMBS: usize, const TO_LIMBS: usize>(&mut self, value: Lc) -> Lc {
        assert!(TO_LIMBS >= FROM_LIMBS, "cannot sign-extend into fewer limbs");
        value
    }
}

#[cfg(test)]
mod tests {
    use circuit::constraints::ConstraintGenerator;
    use circuit::sha256::{COMPRESSION_INPUT_BITS, compression_circuit};

    use super::super::spartan::matrix::compact_from_vendored;
    use super::*;

    /// The compact backend records the vendored backend's rows, entry for
    /// entry, on the abc compression.
    #[test]
    fn the_compact_generator_records_the_vendored_rows() {
        let mut vendored = ConstraintGenerator::new(COMPRESSION_INPUT_BITS);
        let symbolic = vendored.boxed_inputs::<COMPRESSION_INPUT_BITS>();
        let _ = compression_circuit(&mut vendored, symbolic.as_ref());
        let integer = vendored.into_matrices();
        let (expected, map) = compact_from_vendored(&integer).unwrap();

        let mut compact = CompactGenerator::new(COMPRESSION_INPUT_BITS);
        let inputs: Box<[Forgotten; COMPRESSION_INPUT_BITS]> =
            compact.inputs().into_boxed_slice().try_into().unwrap();
        let _ = compression_circuit(&mut compact, inputs.as_ref());
        let circuit = compact.finish().unwrap();
        assert_eq!(circuit.h_len, map.row_count());
        assert_eq!(circuit.f_len, map.column_count());
        assert_eq!(circuit.matrices, expected);
    }

    #[test]
    fn rows_normalise_like_a_btree_map() {
        let mut row = Vec::new();
        let lc = Lc::witness(3) + Lc::witness(1) + Lc::witness(3) * BigInt::from(-1)
            + Lc::from(BigInt::from(5))
            + Lc::witness(0) * BigInt::from(4);
        lc.into_row(&mut row);
        assert_eq!(
            row,
            vec![
                (0, IntegerCoefficient::Small(5)),
                (1, IntegerCoefficient::PowerOfTwo { shift: 2, negative: false }),
                (2, IntegerCoefficient::PowerOfTwo { shift: 0, negative: false }),
            ]
        );
        assert!((Lc::witness(7) - Lc::witness(7)).is_zero());
        assert!(!(Lc::witness(7) + Lc::witness(7)).is_zero());
    }
}
