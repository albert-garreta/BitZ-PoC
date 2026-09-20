//! Full-product multiplication AIR for the u64 and u128 workloads.
//!
//! Schoolbook multiplication over 16-bit limbs with a carry chain, every value
//! bit-decomposed — the same range-check policy as the u32 AIR in
//! `mod32_air.rs`, so the three Plonky3-FRI rows share one arithmetization
//! style. With `L` limbs per operand (4 for u64, 8 for u128) a row holds the
//! `L + L` operand limbs, the `2L` product limbs, `2L - 2` carries, and the
//! bits of all of them; `2L - 1` degree-2 equations
//! `Σ_{i+j=k} a_i·b_j + c_{k-1} = z_k + 2^16·c_k` chain the columns, the last
//! one landing its carry directly in the top product limb. Every equation's
//! two sides stay below `2^36 ≪ p`, so a Goldilocks equality is an integer
//! equality, and the telescoped chain proves `a·b = z` exactly over the
//! integers with `z < 2^(32L)`. Width is the price of bit range checks: 382
//! columns for u64 and 813 for u128.
use super::{Corpus, Workload};
use p3_air::{Air, AirBuilder, BaseAir, WindowAccess};
use p3_field::{PrimeCharacteristicRing, PrimeField64};
use p3_goldilocks::Goldilocks as Val;
use p3_matrix::dense::RowMajorMatrix;

pub(super) const LIMB_BITS: usize = 16;
const LIMB_MASK: u128 = (1 << LIMB_BITS) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WideMulAir {
    /// Limbs per operand: 4 for u64, 8 for u128.
    limbs: usize,
}

impl WideMulAir {
    pub(super) fn for_workload(workload: Workload) -> Self {
        let limbs = match workload {
            Workload::U64 => 4,
            Workload::U128 => 8,
            Workload::U32 => panic!("the u32 workload uses the wrapping AIR"),
        };
        Self { limbs }
    }
    pub(super) const fn limbs(&self) -> usize {
        self.limbs
    }
    /// Product limbs: `2L`.
    const fn product_limbs(&self) -> usize {
        2 * self.limbs
    }
    /// Carries `c_0 … c_{2L-3}`; the last equation's carry is the top product limb.
    const fn carries(&self) -> usize {
        2 * self.limbs - 2
    }
    /// Value columns in order: `a` limbs, `b` limbs, `z` limbs, carries.
    pub(super) const fn value_columns(&self) -> usize {
        6 * self.limbs - 2
    }
    const fn a(&self, i: usize) -> usize {
        i
    }
    const fn b(&self, i: usize) -> usize {
        self.limbs + i
    }
    const fn z(&self, i: usize) -> usize {
        2 * self.limbs + i
    }
    const fn carry(&self, k: usize) -> usize {
        4 * self.limbs + k
    }
    /// Upper bounds of the carries, from the limb bounds alone: the `k`-th
    /// diagonal has `min(k+1, 2L-1-k)` products below `2^32` plus the
    /// previous carry.
    fn carry_bounds(&self) -> Vec<u128> {
        let mut bounds = Vec::with_capacity(self.carries());
        let mut previous = 0u128;
        for k in 0..self.carries() {
            let terms = (k + 1).min(self.product_limbs() - 1 - k) as u128;
            let total = terms * LIMB_MASK * LIMB_MASK + previous;
            previous = total >> LIMB_BITS;
            bounds.push(previous);
        }
        bounds
    }
    /// Bit width of each value column: 16 for limbs, the bound's width for carries.
    pub(super) fn value_bits(&self, column: usize) -> usize {
        if column < 4 * self.limbs {
            LIMB_BITS
        } else {
            let bound = self.carry_bounds()[column - 4 * self.limbs];
            (128 - bound.leading_zeros()) as usize
        }
    }
    /// First bit column of a value column.
    fn bit_offset(&self, column: usize) -> usize {
        self.value_columns() + (0..column).map(|c| self.value_bits(c)).sum::<usize>()
    }
    pub(super) fn width(&self) -> usize {
        self.bit_offset(self.value_columns())
    }
    /// Boolean, recomposition and product constraints.
    pub(super) fn constraint_count(&self) -> usize {
        (self.width() - self.value_columns()) + self.value_columns() + (self.product_limbs() - 1)
    }

    pub(super) fn set_value(&self, row: &mut [Val], column: usize, value: u128) {
        assert!(value >> self.value_bits(column) == 0, "column {column} value {value} out of range");
        row[column] = Val::from_u64(value as u64);
        let offset = self.bit_offset(column);
        for bit in 0..self.value_bits(column) {
            row[offset + bit] = Val::from_u64(((value >> bit) & 1) as u64);
        }
    }

    /// Fills one row from the operands, computing the exact product limb by limb.
    fn fill_row(&self, row: &mut [Val], a: u128, b: u128) {
        let limb = |v: u128, i: usize| (v >> (LIMB_BITS * i)) & LIMB_MASK;
        for i in 0..self.limbs {
            self.set_value(row, self.a(i), limb(a, i));
            self.set_value(row, self.b(i), limb(b, i));
        }
        let mut carry = 0u128;
        for k in 0..self.product_limbs() - 1 {
            let mut total = carry;
            for i in 0..self.limbs {
                if k >= i && k - i < self.limbs {
                    total += limb(a, i) * limb(b, k - i);
                }
            }
            self.set_value(row, self.z(k), total & LIMB_MASK);
            carry = total >> LIMB_BITS;
            if k < self.carries() {
                self.set_value(row, self.carry(k), carry);
            }
        }
        self.set_value(row, self.z(self.product_limbs() - 1), carry);
    }

    pub(super) fn generate(&self, corpus: &Corpus) -> RowMajorMatrix<Val> {
        let width = self.width();
        let mut values = Val::zero_vec(width * corpus.len());
        let rows = values.chunks_exact_mut(width);
        match corpus.workload {
            Workload::U64 => {
                for (row, &(a, b)) in rows.zip(corpus.inputs()) {
                    self.fill_row(row, u128::from(a), u128::from(b));
                }
            }
            Workload::U128 => {
                for (row, &(a, b)) in rows.zip(corpus.wide_inputs()) {
                    self.fill_row(row, a, b);
                }
            }
            Workload::U32 => unreachable!("checked at construction"),
        }
        RowMajorMatrix::new(values, width)
    }

    /// Reads the operands and the product back from a row.
    fn read_row(&self, row: &[Val]) -> (u128, u128, u128, u128) {
        let value = |column: usize| u128::from(row[column].as_canonical_u64());
        let recombine = |first: usize, count: usize| {
            (0..count).fold(0u128, |acc, i| acc | value(first + i) << (LIMB_BITS * i))
        };
        let a = recombine(self.a(0), self.limbs);
        let b = recombine(self.b(0), self.limbs);
        let lo = recombine(self.z(0), self.limbs);
        let hi = recombine(self.z(self.limbs), self.limbs);
        (a, b, lo, hi)
    }

    pub(super) fn audit(&self, corpus: &Corpus) -> super::WitnessAudit {
        let started = std::time::Instant::now();
        let trace = self.generate(corpus);
        let generation_ms = started.elapsed().as_secs_f64() * 1000.;
        let rows: Vec<_> = trace
            .values
            .chunks_exact(self.width())
            .map(|row| self.read_row(row))
            .collect();
        match corpus.workload {
            Workload::U64 => super::WitnessAudit::check(
                corpus,
                rows.iter()
                    .map(|&(a, b, lo, hi)| [a as u64, b as u64, lo as u64, hi as u64])
                    .collect(),
                generation_ms,
                "Plonky3 16-bit limb assignment",
                false,
            ),
            Workload::U128 => super::WitnessAudit::check_wide(
                corpus,
                rows.iter().map(|&(a, b, lo, hi)| [a, b, lo, hi]).collect(),
                generation_ms,
                "Plonky3 16-bit limb assignment",
            ),
            Workload::U32 => unreachable!("checked at construction"),
        }
    }
}

impl<F> BaseAir<F> for WideMulAir {
    fn width(&self) -> usize {
        WideMulAir::width(self)
    }
    fn max_constraint_degree(&self) -> Option<usize> {
        Some(2)
    }
    fn main_next_row_columns(&self) -> Vec<usize> {
        vec![]
    }
}

impl<AB: AirBuilder> Air<AB> for WideMulAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let row = main.current_slice();
        for column in 0..self.value_columns() {
            let offset = self.bit_offset(column);
            let mut value = AB::Expr::ZERO;
            for bit in 0..self.value_bits(column) {
                let v = row[offset + bit];
                builder.assert_bool(v);
                value += v.into() * AB::Expr::from_u64(1 << bit);
            }
            builder.assert_eq(row[column], value);
        }
        let base = AB::Expr::from_u64(1 << LIMB_BITS);
        for k in 0..self.product_limbs() - 1 {
            let mut lhs = AB::Expr::ZERO;
            for i in 0..self.limbs {
                if k >= i && k - i < self.limbs {
                    lhs += row[self.a(i)].into() * row[self.b(k - i)].into();
                }
            }
            if k > 0 {
                lhs += row[self.carry(k - 1)].into();
            }
            let outgoing = if k < self.carries() {
                row[self.carry(k)]
            } else {
                row[self.z(self.product_limbs() - 1)]
            };
            builder.assert_eq(lhs, row[self.z(k)].into() + base.clone() * outgoing.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary_corpus(workload: Workload) -> Corpus {
        if workload == Workload::U128 {
            let values = [0u128, 1, LIMB_MASK, LIMB_MASK + 1, u64::MAX as u128, u128::MAX];
            let mut inputs: Vec<_> = values.into_iter().flat_map(|a| values.map(|b| (a, b))).collect();
            inputs.resize(inputs.len().next_power_of_two(), (0, 0));
            return Corpus::from_wide_inputs(workload, inputs);
        }
        let values = [0u64, 1, LIMB_MASK as u64, LIMB_MASK as u64 + 1, u32::MAX as u64, u64::MAX];
        let mut inputs: Vec<_> = values.into_iter().flat_map(|a| values.map(|b| (a, b))).collect();
        inputs.resize(inputs.len().next_power_of_two(), (0, 0));
        Corpus::from_inputs(workload, inputs)
    }

    #[test]
    fn widths_and_carry_bounds_are_as_documented() {
        let u64_air = WideMulAir::for_workload(Workload::U64);
        let u128_air = WideMulAir::for_workload(Workload::U128);
        assert_eq!(u64_air.value_columns(), 22);
        assert_eq!(u128_air.value_columns(), 46);
        // Every equation side stays far below Goldilocks: the widest diagonal
        // holds L products below 2^32 plus a carry.
        for air in [u64_air, u128_air] {
            let bounds = air.carry_bounds();
            assert_eq!(bounds.len(), air.carries());
            for (k, &bound) in bounds.iter().enumerate() {
                let terms = (k + 1).min(air.product_limbs() - 1 - k) as u128;
                assert!(terms * LIMB_MASK * LIMB_MASK + bound < 1 << 36);
                assert!(air.value_bits(air.carry(k)) <= 20);
            }
            // The last equation's carry is the top product limb, which is
            // 16 bits for every product below 2^(32L).
            let top = LIMB_MASK * LIMB_MASK + bounds[air.carries() - 1];
            assert!(top >> LIMB_BITS <= LIMB_MASK);
            assert_eq!(<WideMulAir as BaseAir<Val>>::width(&air), air.width());
        }
        assert_eq!(u64_air.width(), 382);
        assert_eq!(u128_air.width(), 813);
    }

    #[test]
    fn products_are_exact_at_the_boundaries_and_bad_rows_are_rejected() {
        for workload in [Workload::U64, Workload::U128] {
            let air = WideMulAir::for_workload(workload);
            let corpus = boundary_corpus(workload);
            let trace = air.generate(&corpus);
            p3_air::check_constraints(&air, &trace, &[]);
            let width = air.width();
            for (i, row) in trace.values.chunks_exact(width).enumerate() {
                let (a, b, lo, hi) = air.read_row(row);
                let expected = if workload == Workload::U128 {
                    let (x, y) = corpus.wide_inputs()[i];
                    assert_eq!((a, b), (x, y));
                    bitz::piop::spartan::mul_u128_full(x, y)
                } else {
                    let (x, y) = corpus.inputs()[i];
                    assert_eq!((a, b), (u128::from(x), u128::from(y)));
                    let product = u128::from(x) * u128::from(y);
                    (product & u64::MAX as u128, product >> 64)
                };
                assert_eq!((lo, hi), expected, "{workload:?} row {i}");
            }
            assert_eq!(air.audit(&corpus).digest, corpus.digest);
            let invalid = |trace: RowMajorMatrix<Val>| {
                assert!(std::panic::catch_unwind(|| p3_air::check_constraints(&air, &trace, &[])).is_err());
            };
            // A changed product limb, a changed carry, an out-of-range limb
            // (bits do not recompose) and a non-Boolean bit all fail.
            for column in [air.z(0), air.z(air.product_limbs() - 1), air.carry(0), air.carry(air.carries() - 1)] {
                let mut wrong = trace.clone();
                let current = wrong.values[column].as_canonical_u64() as u128;
                air.set_value(&mut wrong.values[..width], column, current ^ 1);
                invalid(wrong);
            }
            let mut wrong = trace.clone();
            wrong.values[air.a(0)] = Val::from_u64(1 << LIMB_BITS);
            invalid(wrong);
            let mut wrong = trace;
            wrong.values[air.bit_offset(0)] = Val::TWO;
            invalid(wrong);
        }
    }
}
