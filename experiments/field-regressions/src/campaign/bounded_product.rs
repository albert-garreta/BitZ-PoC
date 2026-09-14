//! Wide products selected by a caller-supplied public unsigned bound.
//!
//! Nine-limb storage does not require nine active limbs. Preparation checks the
//! declared bound once while retaining immutable borrows of both operands;
//! execution cannot invalidate that proof or select a width from operand values.
use crate::{Case, Rng, case_requested, measure, production_p256};
use num_bigint::BigUint;
use std::{array, hint::black_box};

/// A public bound shared by both unsigned operands in a batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicProductBound {
    FourLimbs,
    NineLimbs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductBatchError {
    InputLengthMismatch,
    OutputLengthMismatch,
    ExceedsPublicBound,
}

/// A validated batch with exclusive access to its complete 18-limb outputs.
///
/// There is no allocation or conversion of the input arrays. The four-limb
/// bound means unsigned values below 2^256, not signed four-limb values.
pub struct PreparedProducts<'a> {
    a: &'a [[u64; 9]],
    b: &'a [[u64; 9]],
    out: &'a mut [[u64; 18]],
    bound: PublicProductBound,
}

impl<'a> PreparedProducts<'a> {
    #[inline(never)]
    pub fn try_new(
        a: &'a [[u64; 9]],
        b: &'a [[u64; 9]],
        out: &'a mut [[u64; 18]],
        bound: PublicProductBound,
    ) -> Result<Self, ProductBatchError> {
        if a.len() != b.len() {
            return Err(ProductBatchError::InputLengthMismatch);
        }
        if a.len() != out.len() {
            return Err(ProductBatchError::OutputLengthMismatch);
        }
        if bound == PublicProductBound::FourLimbs {
            // Complete the public-length scan before reporting invalid data.
            // This verifies the supplied bound; it never discovers a dispatch
            // width from a secret value. An error reveals bound validity.
            let mut excess = 0;
            for (a, b) in a.iter().zip(b) {
                for i in 4..9 {
                    excess |= a[i] | b[i];
                }
            }
            if excess != 0 {
                return Err(ProductBatchError::ExceedsPublicBound);
            }
        }
        Ok(Self { a, b, out, bound })
    }

    /// Execute a previously validated batch. Both lengths and the width proof
    /// remain valid because this object owns the relevant borrows.
    #[inline(never)]
    pub fn execute(&mut self) {
        match self.bound {
            PublicProductBound::FourLimbs => {
                products_fixed::<4>(self.a, self.b, self.out);
            }
            PublicProductBound::NineLimbs => {
                products_direct::<9>(self.a, self.b, self.out);
            }
        }
    }

    // Frozen pre-selection control: retain its original two fixed-width kernels.
    #[inline(never)]
    fn execute_schoolbook(&mut self) {
        match self.bound {
            PublicProductBound::FourLimbs => {
                products_fixed::<4>(self.a, self.b, self.out);
            }
            PublicProductBound::NineLimbs => {
                products_fixed::<9>(self.a, self.b, self.out);
            }
        }
    }

    #[inline(never)]
    pub fn execute_direct(&mut self) {
        match self.bound {
            PublicProductBound::FourLimbs => products_direct::<4>(self.a, self.b, self.out),
            PublicProductBound::NineLimbs => products_direct::<9>(self.a, self.b, self.out),
        }
    }

    #[inline(never)]
    pub fn execute_comba(&mut self) {
        match self.bound {
            PublicProductBound::FourLimbs => {
                for ((a, b), out) in self.a.iter().zip(self.b).zip(self.out.iter_mut()) {
                    comba4(a, b, out);
                }
            }
            PublicProductBound::NineLimbs => products_fixed::<9>(self.a, self.b, self.out),
        }
    }

    pub fn outputs(&self) -> &[[u64; 18]] {
        self.out
    }
}

/// Convenient one-shot API, whose validation cost is measured separately from
/// repeated execution of a prepared batch.
#[inline(never)]
pub fn multiply_batch(
    a: &[[u64; 9]],
    b: &[[u64; 9]],
    out: &mut [[u64; 18]],
    bound: PublicProductBound,
) -> Result<(), ProductBatchError> {
    PreparedProducts::try_new(a, b, out, bound)?.execute();
    Ok(())
}

#[inline(never)]
fn multiply_batch_direct(
    a: &[[u64; 9]],
    b: &[[u64; 9]],
    out: &mut [[u64; 18]],
    bound: PublicProductBound,
) -> Result<(), ProductBatchError> {
    PreparedProducts::try_new(a, b, out, bound)?.execute_direct();
    Ok(())
}

#[inline(never)]
fn multiply_batch_schoolbook(
    a: &[[u64; 9]],
    b: &[[u64; 9]],
    out: &mut [[u64; 18]],
    bound: PublicProductBound,
) -> Result<(), ProductBatchError> {
    PreparedProducts::try_new(a, b, out, bound)?.execute_schoolbook();
    Ok(())
}

#[inline(never)]
fn multiply_batch_comba(
    a: &[[u64; 9]],
    b: &[[u64; 9]],
    out: &mut [[u64; 18]],
    bound: PublicProductBound,
) -> Result<(), ProductBatchError> {
    PreparedProducts::try_new(a, b, out, bound)?.execute_comba();
    Ok(())
}

// The arithmetic is the existing experiment's fixed-width winning kernel.
// Its loop bounds are public compile-time constants and all output limbs are
// overwritten, including the ten zero upper limbs in the four-limb case.
#[inline(always)]
fn fixed_product<const ACTIVE: usize>(a: [u64; 9], b: [u64; 9]) -> [u64; 18] {
    let mut out = [0; 18];
    for i in 0..ACTIVE {
        let mut carry = 0u128;
        for j in 0..ACTIVE {
            let sum = a[i] as u128 * b[j] as u128 + out[i + j] as u128 + carry;
            out[i + j] = sum as u64;
            carry = sum >> 64;
        }
        out[i + ACTIVE] = carry as u64;
    }
    out
}

#[inline(always)]
fn products_fixed<const ACTIVE: usize>(a: &[[u64; 9]], b: &[[u64; 9]], out: &mut [[u64; 18]]) {
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        *out = fixed_product::<ACTIVE>(a, b);
    }
}

// The first row needs no existing output or initial zeroing. Subsequent rows
// read only limbs initialized by the previous row; each row writes its carry
// into the next uninitialized limb. This also avoids an 18-limb return value.
#[inline(always)]
fn direct_product<const ACTIVE: usize>(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    let mut carry = 0u128;
    for j in 0..ACTIVE {
        let product = a[0] as u128 * b[j] as u128 + carry;
        out[j] = product as u64;
        carry = product >> 64;
    }
    out[ACTIVE] = carry as u64;
    for i in 1..ACTIVE {
        let mut carry = 0u128;
        for j in 0..ACTIVE {
            let sum = a[i] as u128 * b[j] as u128 + out[i + j] as u128 + carry;
            out[i + j] = sum as u64;
            carry = sum >> 64;
        }
        out[i + ACTIVE] = carry as u64;
    }
    out[2 * ACTIVE..].fill(0);
}

#[inline(always)]
fn products_direct<const ACTIVE: usize>(a: &[[u64; 9]], b: &[[u64; 9]], out: &mut [[u64; 18]]) {
    for ((a, b), out) in a.iter().zip(b).zip(out) {
        direct_product::<ACTIVE>(a, b, out);
    }
}

#[inline(always)]
fn add_comba_product(acc: &mut [u64; 3], a: u64, b: u64) {
    let product = a as u128 * b as u128;
    let (low, carry) = acc[0].overflowing_add(product as u64);
    let high = acc[1] as u128 + (product >> 64) + carry as u128;
    acc[0] = low;
    acc[1] = high as u64;
    // At most four products share a column, so this limb is at most three.
    acc[2] += (high >> 64) as u64;
}

// Each column writes its final limb once. The explicit four-limb schedule
// avoids a variable inner-loop bound and retains a 192-bit carry accumulator.
#[inline(always)]
fn comba4(a: &[u64; 9], b: &[u64; 9], out: &mut [u64; 18]) {
    let mut acc = [0; 3];
    macro_rules! column {
        ($column:literal, $(($i:literal, $j:literal)),+) => {{
            $(add_comba_product(&mut acc, a[$i], b[$j]);)+
            out[$column] = acc[0];
            acc = [acc[1], acc[2], 0];
        }};
    }
    column!(0, (0, 0));
    column!(1, (0, 1), (1, 0));
    column!(2, (0, 2), (1, 1), (2, 0));
    column!(3, (0, 3), (1, 2), (2, 1), (3, 0));
    column!(4, (1, 3), (2, 2), (3, 1));
    column!(5, (2, 3), (3, 2));
    column!(6, (3, 3));
    out[7] = acc[0];
    out[8..].fill(0);
}

// Keep the original experiment's call boundary and length checks for both
// controls, so the old fixed4 winner is not weakened to favor the prepared API.
#[inline(never)]
fn products_control<const ACTIVE: usize>(a: &[[u64; 9]], b: &[[u64; 9]], out: &mut [[u64; 18]]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((&a, &b), out) in a.iter().zip(b).zip(out) {
        *out = match ACTIVE {
            0 => production_p256::product(a, b),
            4 => fixed_product::<4>(a, b),
            _ => fixed_product::<9>(a, b),
        };
    }
}

fn biguint<const L: usize>(a: &[u64; L]) -> BigUint {
    BigUint::from_bytes_le(&a.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}

fn check_batch(a: &[[u64; 9]], b: &[[u64; 9]], bound: PublicProductBound) {
    let expected: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(a, b)| biguint(a) * biguint(b))
        .collect();
    let mut out = vec![[u64::MAX; 18]; a.len()];
    products_control::<0>(a, b, &mut out);
    for (out, expected) in out.iter().zip(&expected) {
        assert_eq!(&biguint(out), expected);
    }
    products_control::<9>(a, b, &mut out);
    for (out, expected) in out.iter().zip(&expected) {
        assert_eq!(&biguint(out), expected);
    }
    if bound == PublicProductBound::FourLimbs {
        products_control::<4>(a, b, &mut out);
        for (out, expected) in out.iter().zip(&expected) {
            assert_eq!(&biguint(out), expected);
        }
    }
    out.fill([u64::MAX; 18]);
    let mut prepared = PreparedProducts::try_new(a, b, &mut out, bound).unwrap();
    // Repeat execution to ensure the API overwrites rather than accumulates.
    for _ in 0..2 {
        prepared.execute();
        for (out, expected) in prepared.outputs().iter().zip(&expected) {
            assert_eq!(&biguint(out), expected);
        }
        prepared.execute_schoolbook();
        for (out, expected) in prepared.outputs().iter().zip(&expected) {
            assert_eq!(&biguint(out), expected);
        }
        prepared.execute_direct();
        for (out, expected) in prepared.outputs().iter().zip(&expected) {
            assert_eq!(&biguint(out), expected);
        }
        prepared.execute_comba();
        for (out, expected) in prepared.outputs().iter().zip(&expected) {
            assert_eq!(&biguint(out), expected);
        }
    }
    multiply_batch(a, b, &mut out, bound).unwrap();
    for (out, expected) in out.iter().zip(&expected) {
        assert_eq!(&biguint(out), expected);
    }
    multiply_batch_direct(a, b, &mut out, bound).unwrap();
    for (out, expected) in out.iter().zip(&expected) {
        assert_eq!(&biguint(out), expected);
    }
    multiply_batch_schoolbook(a, b, &mut out, bound).unwrap();
    for (out, expected) in out.iter().zip(&expected) {
        assert_eq!(&biguint(out), expected);
    }
    multiply_batch_comba(a, b, &mut out, bound).unwrap();
    for (out, expected) in out.iter().zip(&expected) {
        assert_eq!(&biguint(out), expected);
    }
}

fn verify(rng: &mut Rng) {
    for (active, bound) in [
        (4, PublicProductBound::FourLimbs),
        (9, PublicProductBound::NineLimbs),
    ] {
        let mut boundaries = vec![[0; 9], array::from_fn(|i| u64::from(i == 0))];
        boundaries.push(array::from_fn(|i| if i < active { u64::MAX } else { 0 }));
        boundaries.push(array::from_fn(
            |i| if i + 1 == active { 1 << 63 } else { 0 },
        ));
        boundaries.push(array::from_fn(|i| {
            if i < active { 0xaaaa_aaaa_aaaa_aaaa } else { 0 }
        }));
        boundaries.push(array::from_fn(|i| {
            if i < active { 0x5555_5555_5555_5555 } else { 0 }
        }));
        // Single-limb basis cases at both ends of every possible carry chain.
        for limb in 0..active {
            for word in [1, 1 << 63, u64::MAX] {
                boundaries.push(array::from_fn(|i| if i == limb { word } else { 0 }));
            }
        }
        let a: Vec<_> = boundaries
            .iter()
            .flat_map(|a| boundaries.iter().map(move |_| *a))
            .collect();
        let b: Vec<_> = boundaries
            .iter()
            .flat_map(|_| boundaries.iter().copied())
            .collect();
        check_batch(&a, &b, bound);
        for n in [0, 1, 2, 3, 7, 8, 9, 15, 17, 31, 33, 257] {
            let a: Vec<_> = (0..n)
                .map(|_| array::from_fn(|i| if i < active { rng.next() } else { 0 }))
                .collect();
            let b: Vec<_> = (0..n)
                .map(|_| array::from_fn(|i| if i < active { rng.next() } else { 0 }))
                .collect();
            check_batch(&a, &b, bound);
        }
    }

    let mut untouched = [[0x1234; 18]; 2];
    assert_eq!(
        PreparedProducts::try_new(&[[0; 9]], &[], &mut [], PublicProductBound::FourLimbs).err(),
        Some(ProductBatchError::InputLengthMismatch)
    );
    assert_eq!(
        PreparedProducts::try_new(
            &[[0; 9]],
            &[[0; 9]],
            &mut untouched,
            PublicProductBound::FourLimbs
        )
        .err(),
        Some(ProductBatchError::OutputLengthMismatch)
    );
    for limb in 4..9 {
        for side in 0..2 {
            let mut a = [[0; 9]; 2];
            let mut b = [[0; 9]; 2];
            if side == 0 {
                a[1][limb] = 1
            } else {
                b[1][limb] = 1
            }
            assert_eq!(
                multiply_batch(&a, &b, &mut untouched, PublicProductBound::FourLimbs),
                Err(ProductBatchError::ExceedsPublicBound)
            );
            assert_eq!(untouched, [[0x1234; 18]; 2]);
            // Exactly the same values are valid under an explicit full bound.
            check_batch(&a, &b, PublicProductBound::NineLimbs);
        }
    }
}

pub fn run(samples: usize, rng: &mut Rng) {
    verify(rng);
    for (active, bound) in [
        (4, PublicProductBound::FourLimbs),
        (9, PublicProductBound::NineLimbs),
    ] {
        for n in [16, 1024, 65536] {
            let size = format!("active{active}_n{n}");
            if !case_requested("bounded_product", &size) {
                continue;
            }
            let a: Vec<_> = (0..n)
                .map(|_| array::from_fn(|i| if i < active { rng.next() } else { 0 }))
                .collect();
            let b: Vec<_> = (0..n)
                .map(|_| array::from_fn(|i| if i < active { rng.next() } else { 0 }))
                .collect();
            // The independent oracle covers every timed input, not a prefix.
            check_batch(&a, &b, bound);
            let mut baseline_out = vec![[0; 18]; n];
            let mut fixed9_out = vec![[0; 18]; n];
            let mut fixed4_out = vec![[0; 18]; n];
            let mut prepared_out = vec![[0; 18]; n];
            let mut complete_out = vec![[0; 18]; n];
            let mut selected_out = vec![[0; 18]; n];
            let mut selected_complete_out = vec![[0; 18]; n];
            let mut direct_out = vec![[0; 18]; n];
            let mut direct_complete_out = vec![[0; 18]; n];
            let mut comba_out = vec![[0; 18]; n];
            let mut comba_complete_out = vec![[0; 18]; n];
            let mut prepared = PreparedProducts::try_new(&a, &b, &mut prepared_out, bound).unwrap();
            let mut selected = PreparedProducts::try_new(&a, &b, &mut selected_out, bound).unwrap();
            let mut direct = PreparedProducts::try_new(&a, &b, &mut direct_out, bound).unwrap();
            let mut comba = PreparedProducts::try_new(&a, &b, &mut comba_out, bound).unwrap();
            let mut cases = vec![
                Case::new("existing_p256", n * 288, || {
                    products_control::<0>(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut baseline_out),
                    );
                    black_box(&baseline_out);
                }),
                Case::new("fixed9", n * 288, || {
                    products_control::<9>(black_box(&a), black_box(&b), black_box(&mut fixed9_out));
                    black_box(&fixed9_out);
                }),
                Case::new("prepared_bound", n * 288, || {
                    black_box(&mut prepared).execute_schoolbook();
                    black_box(prepared.outputs());
                }),
                Case::new("validate_execute", n * 288, || {
                    multiply_batch_schoolbook(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut complete_out),
                        black_box(bound),
                    )
                    .unwrap();
                    black_box(&complete_out);
                }),
                Case::new("prepared_selected", n * 288, || {
                    black_box(&mut selected).execute();
                    black_box(selected.outputs());
                }),
                Case::new("validate_execute_selected", n * 288, || {
                    multiply_batch(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut selected_complete_out),
                        black_box(bound),
                    )
                    .unwrap();
                    black_box(&selected_complete_out);
                }),
                Case::new("prepared_direct", n * 288, || {
                    black_box(&mut direct).execute_direct();
                    black_box(direct.outputs());
                }),
                Case::new("validate_execute_direct", n * 288, || {
                    multiply_batch_direct(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut direct_complete_out),
                        black_box(bound),
                    )
                    .unwrap();
                    black_box(&direct_complete_out);
                }),
            ];
            if active == 4 {
                cases.push(Case::new("public_bound4", n * 288, || {
                    products_control::<4>(black_box(&a), black_box(&b), black_box(&mut fixed4_out));
                    black_box(&fixed4_out);
                }));
                cases.push(Case::new("prepared_comba4", n * 288, || {
                    black_box(&mut comba).execute_comba();
                    black_box(comba.outputs());
                }));
                cases.push(Case::new("validate_execute_comba4", n * 288, || {
                    multiply_batch_comba(
                        black_box(&a),
                        black_box(&b),
                        black_box(&mut comba_complete_out),
                        black_box(bound),
                    )
                    .unwrap();
                    black_box(&comba_complete_out);
                }));
            }
            measure("bounded_product", &size, &mut cases, samples, rng);
        }
    }
}
