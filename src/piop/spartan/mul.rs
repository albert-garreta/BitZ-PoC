//! Compact witnesses shared by the native integer multiplication relations.
use std::marker::PhantomData;

use super::SpartanMatrixError;
use crate::{pcs::IntegerMatrixLayout, sumcheck::outer::OuterRows};

mod sealed {
    pub trait Sealed {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for u128 {}
}

/// The supported native operand widths and their exact double-width products.
pub trait MulWord: sealed::Sealed + Copy + Default + Eq + std::fmt::Debug + Send + Sync {
    type Product: Copy + Send + Sync;
    const BITS: usize;
    fn multiply(x: Self, y: Self) -> (Self, Self);
    fn join(lo: Self, hi: Self) -> Self::Product;
    fn as_u128(self) -> u128;
    fn pack(
        witness: &MulWitness<Self>,
        layout: MulLayout<Self>,
        rows: &mut [Vec<u64>],
        high_gates: usize,
    );
}

/// Supplied product limbs are preserved, so the proof checks the caller's claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MulRow<T> {
    pub x: T,
    pub y: T,
    pub lo: T,
    pub hi: T,
}

impl<T: MulWord> MulRow<T> {
    pub fn new(x: T, y: T) -> Self {
        let (lo, hi) = T::multiply(x, y);
        Self { x, y, lo, hi }
    }
    pub fn product(&self) -> T::Product {
        T::join(self.lo, self.hi)
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error, Eq, PartialEq)]
pub enum MulError {
    #[error("a multiplication batch must not be empty")]
    EmptyBatch,
    #[error("the multiplication domain is too large")]
    DomainTooLarge,
    #[error("packed rows do not match the committed layout")]
    InvalidRowShape,
    #[error(transparent)]
    SpartanMatrix(#[from] SpartanMatrixError),
}

/// Assignment and commitment geometry; the word type also selects the relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MulLayout<T: MulWord> {
    pub(super) multiplications: usize,
    pub(super) capacity: usize,
    pub(super) gate_vars: usize,
    col_vars: usize,
    word: PhantomData<T>,
}

impl<T: MulWord> MulLayout<T> {
    pub fn new(multiplications: usize) -> Result<Self, MulError> {
        if multiplications == 0 {
            return Err(MulError::EmptyBatch);
        }
        let capacity = multiplications
            .max(256)
            .checked_next_power_of_two()
            .ok_or(MulError::DomainTooLarge)?;
        capacity
            .checked_mul(4 * T::BITS)
            .ok_or(MulError::DomainTooLarge)?;
        let gate_vars = capacity.trailing_zeros() as usize;
        let col_vars = Self::default_col_vars(gate_vars);
        Ok(Self {
            multiplications,
            capacity,
            gate_vars,
            col_vars,
            word: PhantomData,
        })
    }
    /// The relation commits each operand and product limb as bits.
    pub const fn committed_layout(&self) -> IntegerMatrixLayout {
        self.bitz_params()
    }
    pub const fn multiplications(&self) -> usize {
        self.multiplications
    }
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
    pub const fn gate_vars(&self) -> usize {
        self.gate_vars
    }
    pub const fn col_vars(&self) -> usize {
        self.col_vars
    }
    pub const fn assignment_len(&self) -> usize {
        self.capacity * if T::BITS == 64 { 5 } else { 4 }
    }
    pub const fn assignment_vars(&self) -> usize {
        self.gate_vars + if T::BITS == 64 { 3 } else { 2 }
    }
    pub const fn padded_assignment_len(&self) -> usize {
        1 << self.assignment_vars()
    }
    /// Measured Wfbitz split, clamped to keep the word-parallel packer
    /// eligible: at least 64 high gates per row.
    fn default_col_vars(gate_vars: usize) -> usize {
        let n = gate_vars + (4 * T::BITS).ilog2() as usize;
        let rows = crate::wfbitz::params::reference_log_rows(n);
        n.saturating_sub(rows).min(gate_vars.saturating_sub(6))
    }
    /// Move gate coordinates from rows to columns relative to the default split.
    pub fn with_split_shift(mut self, shift: i8) -> Result<Self, MulError> {
        let columns = Self::default_col_vars(self.gate_vars) as i64 + i64::from(shift);
        if columns < 0 || columns > self.gate_vars as i64 {
            return Err(MulError::DomainTooLarge);
        }
        self.col_vars = columns as usize;
        Ok(self)
    }
    /// Opening grid for the four binary operand/product blocks.
    pub const fn bitz_params(&self) -> IntegerMatrixLayout {
        IntegerMatrixLayout {
            row_vars: self.gate_vars - self.col_vars + (4 * T::BITS).trailing_zeros() as usize,
            col_vars: self.col_vars,
            word_bits: 1,
        }
    }
    pub const fn bitz_cell(&self, bit_slot: usize, gate: usize) -> Option<(usize, usize)> {
        if bit_slot >= 4 * T::BITS || gate >= self.capacity {
            return None;
        }
        let high = self.gate_vars - self.col_vars;
        Some((
            (bit_slot << high) | (gate >> self.col_vars),
            gate & ((1 << self.col_vars) - 1),
        ))
    }
}

/// Four padded native blocks `[x | y | lo | hi]`.
/// The constant assignment block and MLE padding are implicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MulWitness<T: MulWord> {
    pub(super) layout: MulLayout<T>,
    values: [Box<[T]>; 4],
}

impl<T: MulWord> MulWitness<T> {
    pub fn from_inputs(inputs: &[(T, T)]) -> Result<Self, MulError> {
        Self::from_fn(inputs.len(), |i| inputs[i])
    }
    pub fn from_fn(
        multiplications: usize,
        input: impl FnMut(usize) -> (T, T),
    ) -> Result<Self, MulError> {
        Self::from_fn_with_layout(MulLayout::new(multiplications)?, input)
    }
    pub fn from_fn_with_layout(
        layout: MulLayout<T>,
        mut input: impl FnMut(usize) -> (T, T),
    ) -> Result<Self, MulError> {
        Self::from_row_fn(layout, |i| {
            let (x, y) = input(i);
            MulRow::new(x, y)
        })
    }
    pub fn from_rows(rows: &[MulRow<T>]) -> Result<Self, MulError> {
        Self::from_rows_with_layout(MulLayout::new(rows.len())?, rows)
    }
    pub fn from_rows_with_layout(
        layout: MulLayout<T>,
        rows: &[MulRow<T>],
    ) -> Result<Self, MulError> {
        if rows.len() != layout.multiplications {
            return Err(MulError::DomainTooLarge);
        }
        Self::from_row_fn(layout, |i| rows[i])
    }
    pub(super) fn from_row_fn(
        layout: MulLayout<T>,
        mut input: impl FnMut(usize) -> MulRow<T>,
    ) -> Result<Self, MulError> {
        // Separate blocks let the allocator reuse medium-sized buffers; joining
        // them into one large allocation causes repeated page faults on glibc.
        // Large partially filled batches retain demand-zero padding pages.
        if layout.multiplications < layout.capacity
            && 4 * layout.capacity * std::mem::size_of::<T>() >= 16 * 1024 * 1024
        {
            let mut values =
                std::array::from_fn(|_| vec![T::default(); layout.capacity].into_boxed_slice());
            let [xs, ys, los, his] = values.each_mut().map(|v| &mut v[..layout.multiplications]);
            for i in 0..layout.multiplications {
                let row = input(i);
                xs[i] = row.x;
                ys[i] = row.y;
                los[i] = row.lo;
                his[i] = row.hi;
            }
            return Ok(Self { layout, values });
        }
        let mut values = std::array::from_fn(|_| Box::<[T]>::new_uninit_slice(layout.capacity));
        let [xs, ys, los, his] = &mut values;
        for block in [&mut *xs, &mut *ys, &mut *los, &mut *his] {
            block[layout.multiplications..].fill(std::mem::MaybeUninit::new(T::default()));
        }
        let [xs, ys, los, his] = values.each_mut().map(|v| &mut v[..layout.multiplications]);
        for i in 0..layout.multiplications {
            let row = input(i);
            xs[i].write(row.x);
            ys[i].write(row.y);
            los[i].write(row.lo);
            his[i].write(row.hi);
        }
        // SAFETY: each of the four blocks has its live prefix and padding
        // initialized above. A panicking callback only drops MaybeUninit<T>.
        let values = values.map(|block| unsafe { block.assume_init() });
        Ok(Self { layout, values })
    }
    pub const fn layout(&self) -> &MulLayout<T> {
        &self.layout
    }
    pub fn with_split_shift(mut self, shift: i8) -> Result<Self, MulError> {
        self.layout = self.layout.with_split_shift(shift)?;
        Ok(self)
    }
    #[inline]
    fn blocks(&self) -> [&[T]; 4] {
        self.values.each_ref().map(|v| v.as_ref())
    }
    pub(crate) fn native_products(&self) -> super::raw_monty::NativeWideProducts<'_, T> {
        let live = self.layout.multiplications;
        let [x, y, lo, hi] = self.blocks().map(|v| &v[..live]);
        super::raw_monty::NativeWideProducts::new(x, y, lo, hi, live.next_power_of_two())
    }
    pub fn x_values(&self) -> &[T] {
        self.blocks()[0]
    }
    pub fn y_values(&self) -> &[T] {
        self.blocks()[1]
    }
    pub fn z_lo_values(&self) -> &[T] {
        self.blocks()[2]
    }
    pub fn z_hi_values(&self) -> &[T] {
        self.blocks()[3]
    }
    pub fn product(&self, row: usize) -> T::Product {
        T::join(self.z_lo_values()[row], self.z_hi_values()[row])
    }
    pub fn rows(&self) -> impl ExactSizeIterator<Item = MulRow<T>> + '_ {
        (0..self.layout.multiplications).map(|i| MulRow {
            x: self.x_values()[i],
            y: self.y_values()[i],
            lo: self.z_lo_values()[i],
            hi: self.z_hi_values()[i],
        })
    }
    /// Packs the compact commitment grid returned by `committed_layout`.
    pub fn bitz_bit_rows(&self) -> Vec<Vec<u64>> {
        self.bit_rows(self.layout)
    }
    /// Reuses caller-owned rows, avoiding allocation churn when packing batches
    /// of the same shape. Every output word is overwritten, including padding.
    pub fn write_bitz_bit_rows(&self, rows: &mut [Vec<u64>]) -> Result<(), MulError> {
        let layout = self.layout;
        let p = layout.bitz_params();
        let words = (p.rows() * p.word_bits).div_ceil(64);
        if rows.len() != p.cols() || rows.iter().any(|row| row.len() != words) {
            return Err(MulError::InvalidRowShape);
        }
        self.pack_rows(layout, rows);
        Ok(())
    }
    fn bit_rows(&self, layout: MulLayout<T>) -> Vec<Vec<u64>> {
        let params = layout.bitz_params();
        let words = (params.rows() * params.word_bits).div_ceil(64);
        let mut rows = (0..params.cols())
            .map(|_| vec![0; words])
            .collect::<Vec<_>>();
        self.pack_rows(layout, &mut rows);
        rows
    }
    fn pack_rows(&self, layout: MulLayout<T>, rows: &mut [Vec<u64>]) {
        let high_gates = 1 << (layout.gate_vars - layout.col_vars);
        if high_gates % 64 == 0 {
            T::pack(self, layout, rows, high_gates);
        } else {
            for row in rows.iter_mut() {
                row.fill(0);
            }
            self.write_bit_rows_bitwise_at(layout, rows);
        }
    }
    #[cfg(test)]
    pub(super) fn write_bit_rows_bitwise(&self, rows: &mut [Vec<u64>]) {
        self.write_bit_rows_bitwise_at(self.layout, rows);
    }
    fn write_bit_rows_bitwise_at(&self, layout: MulLayout<T>, rows: &mut [Vec<u64>]) {
        for (i, row) in self.rows().enumerate() {
            for (limb, value) in [row.x, row.y, row.lo, row.hi].into_iter().enumerate() {
                let mut bits = value.as_u128();
                while bits != 0 {
                    let slot = limb * T::BITS + bits.trailing_zeros() as usize;
                    let (bit, c) = layout.bitz_cell(slot, i).expect("valid witness coordinate");
                    rows[c][bit / 64] |= 1 << (bit % 64);
                    bits &= bits - 1;
                }
            }
        }
    }
}

impl<T: MulWord> OuterRows for MulWitness<T> {
    type AB = T;
    type C = T::Product;
    fn dimensions(&self) -> (usize, usize, usize) {
        let n = self.layout.multiplications.next_power_of_two();
        (n, n, n)
    }
    #[inline(always)]
    fn a(&self, row: usize) -> T {
        self.x_values()[row]
    }
    #[inline(always)]
    fn b(&self, row: usize) -> T {
        self.y_values()[row]
    }
    #[inline(always)]
    fn c(&self, row: usize) -> T::Product {
        self.product(row)
    }
}

impl MulWord for u32 {
    type Product = u64;
    const BITS: usize = 32;
    #[inline(always)]
    fn multiply(x: Self, y: Self) -> (Self, Self) {
        let p = u64::from(x) * u64::from(y);
        (p as u32, (p >> 32) as u32)
    }
    #[inline(always)]
    fn join(lo: Self, hi: Self) -> u64 {
        u64::from(lo) | (u64::from(hi) << 32)
    }
    fn as_u128(self) -> u128 {
        self.into()
    }
    fn pack(w: &MulWitness<Self>, layout: MulLayout<Self>, rows: &mut [Vec<u64>], high: usize) {
        let limbs = w.blocks();
        super::slot_rows::pack_slot_major_u32::<32, 1, 8>(
            rows,
            layout.col_vars,
            high,
            layout.multiplications,
            limbs,
        );
    }
}
impl MulWord for u64 {
    type Product = u128;
    const BITS: usize = 64;
    #[inline(always)]
    fn multiply(x: Self, y: Self) -> (Self, Self) {
        let p = u128::from(x) * u128::from(y);
        (p as u64, (p >> 64) as u64)
    }
    #[inline(always)]
    fn join(lo: Self, hi: Self) -> u128 {
        u128::from(lo) | (u128::from(hi) << 64)
    }
    fn as_u128(self) -> u128 {
        self.into()
    }
    fn pack(w: &MulWitness<Self>, layout: MulLayout<Self>, rows: &mut [Vec<u64>], high: usize) {
        let [x, y, lo, hi] = w.blocks().map(|v| &v[..layout.multiplications]);
        super::slot_rows::pack_slot_major_rows_w1_words::<4, _>(
            rows,
            layout.col_vars,
            high,
            layout.multiplications,
            |i| [x[i], y[i], lo[i], hi[i]],
        );
    }
}
impl MulWord for u128 {
    type Product = field::Uint<4>;
    const BITS: usize = 128;
    #[inline(always)]
    fn multiply(x: Self, y: Self) -> (Self, Self) {
        super::u128_mul::mul_u128_full(x, y)
    }
    #[inline(always)]
    fn join(lo: Self, hi: Self) -> Self::Product {
        field::Uint::from_words([lo as u64, (lo >> 64) as u64, hi as u64, (hi >> 64) as u64])
    }
    fn as_u128(self) -> u128 {
        self
    }
    fn pack(w: &MulWitness<Self>, layout: MulLayout<Self>, rows: &mut [Vec<u64>], high: usize) {
        let [x, y, lo, hi] = w.blocks().map(|v| &v[..layout.multiplications]);
        super::slot_rows::pack_slot_major_rows_w1_words::<8, _>(
            rows,
            layout.col_vars,
            high,
            layout.multiplications,
            |i| {
                [
                    x[i] as u64,
                    (x[i] >> 64) as u64,
                    y[i] as u64,
                    (y[i] >> 64) as u64,
                    lo[i] as u64,
                    (lo[i] >> 64) as u64,
                    hi[i] as u64,
                    (hi[i] >> 64) as u64,
                ]
            },
        );
    }
}

impl From<MulError> for super::protocol::ProtocolError {
    fn from(error: MulError) -> Self {
        Self::Relation(Box::new(error))
    }
}

impl MulWitness<u32> {
    pub(crate) fn inner_witness(&self) -> super::raw_monty::RawWitness<'_> {
        super::raw_monty::RawWitness::Wide(super::raw_monty::NativeBlockWitness::u32(
            self.layout.capacity,
            self.x_values(),
            self.y_values(),
            self.z_lo_values(),
            self.z_hi_values(),
        ))
    }
}
impl MulWitness<u64> {
    pub(crate) fn inner_witness(&self) -> super::raw_monty::RawWitness<'_> {
        super::raw_monty::RawWitness::Wide(super::raw_monty::NativeBlockWitness::u64(
            self.layout.capacity,
            self.x_values(),
            self.y_values(),
            self.z_lo_values(),
            self.z_hi_values(),
        ))
    }
}
impl MulWitness<u128> {
    pub(crate) fn inner_witness(&self) -> super::raw_monty::RawWitness<'_> {
        super::raw_monty::RawWitness::Wide(super::raw_monty::NativeBlockWitness::new(
            self.layout.capacity,
            self.x_values(),
            self.y_values(),
            self.z_lo_values(),
            self.z_hi_values(),
        ))
    }
}

/// Flat CSC construction shared by all multiplication selectors.
pub(super) fn selector_matrix<T: MulWord, C: Clone>(
    layout: &MulLayout<T>,
    blocks: &[(usize, C)],
) -> Result<circuit::linear_map::CscMatrix<Box<[C]>>, SpartanMatrixError> {
    let rows = layout.multiplications;
    let mut offsets = vec![0; layout.assignment_len() + 1];
    let mut indices = Vec::with_capacity(rows * blocks.len());
    let mut coefficients = Vec::with_capacity(rows * blocks.len());
    let mut previous = 0;
    for (block, coefficient) in blocks {
        let offset = block * layout.capacity;
        offsets[previous..=offset].fill(indices.len());
        let start = indices.len();
        for row in 0..rows {
            offsets[offset + row + 1] = start + row + 1;
            indices.push(row);
            coefficients.push(coefficient.clone());
        }
        previous = offset + rows + 1;
    }
    offsets[previous..].fill(indices.len());
    Ok(circuit::linear_map::CscMatrix::try_from_csc_parts(
        rows,
        offsets,
        indices,
        coefficients,
    )?)
}

impl<T: MulWord> MulLayout<T> {
    pub(super) fn validate_protocol_geometry(&self) -> Result<(), super::protocol::ProtocolError> {
        let p = self.bitz_params();
        if p.col_vars > self.gate_vars || crate::wfbitz::Shape::new(p.row_vars, p.col_vars).is_err()
        {
            return Err(super::protocol::ProtocolError::InvalidBitzParameters);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
