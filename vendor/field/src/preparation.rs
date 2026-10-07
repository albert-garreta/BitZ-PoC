//! Reusable preparation with caller-owned inputs and outputs.

use crate::*;

#[derive(Clone, Debug)]
pub struct PreparedDivisor<const D: usize> {
    divisor: Uint<D>,
    reduction: crate::modular::reduction::Barrett<D>,
}
impl<const D: usize> PreparedDivisor<D> {
    pub fn new(divisor: Uint<D>) -> Result<Self, ContextError> {
        if divisor.ct_is_zero().declassify() {
            return Err(ContextError::ZeroDivisor);
        }
        Ok(Self {
            reduction: crate::modular::reduction::Barrett::new(&divisor),
            divisor,
        })
    }
    pub fn div_rem_ct<const N: usize>(&self, dividend: &Uint<N>) -> (Uint<N>, Uint<D>) {
        let mut quotient = Uint::ZERO;
        let remainder = self
            .reduction
            .divide_into(dividend, &self.divisor, |i, w| quotient.0[i] = w);
        (quotient, remainder)
    }
    pub fn div_rem_product_ct<const A: usize, const B: usize>(
        &self,
        dividend: &UintProduct<A, B>,
    ) -> (UintProduct<A, B>, Uint<D>) {
        let mut quotient = UintProduct::ZERO;
        let remainder = self
            .reduction
            .divide_into(dividend, &self.divisor, |i, w| quotient.set_word(i, w));
        (quotient, remainder)
    }

    /// Exact quotient and remainder of `a * b`, valid when both operands are
    /// below this divisor. Invalid operands return a false mask and `(0, 0)`.
    ///
    /// For a full-width divisor m, a*b < m² < m*2^(64*D), so one Barrett
    /// block suffices. Operand validation and selection have fixed bounds;
    /// dispatch for padded and power-of-two divisors uses only the public m.
    pub fn mul_div_rem_reduced_ct(&self, a: &Uint<D>, b: &Uint<D>) -> CtValue<(Uint<D>, Uint<D>)> {
        let valid = a.ct_lt(&self.divisor) & b.ct_lt(&self.divisor);
        let a = Uint::ct_select(&Uint::ZERO, a, valid);
        let b = Uint::ct_select(&Uint::ZERO, b, valid);
        let product = IntegerOps.mul_wide(&a, &b);
        CtValue::new(
            self.reduction
                .divide_reduced_product(&product, &self.divisor),
            valid,
        )
    }

    /// Square counterpart of [`Self::mul_div_rem_reduced_ct`], with the same
    /// validity and output contract and one product per distinct limb pair.
    pub fn square_div_rem_reduced_ct(&self, a: &Uint<D>) -> CtValue<(Uint<D>, Uint<D>)> {
        let valid = a.ct_lt(&self.divisor);
        let a = Uint::ct_select(&Uint::ZERO, a, valid);
        let product = IntegerOps.square_wide(&a);
        CtValue::new(
            self.reduction
                .divide_reduced_product(&product, &self.divisor),
            valid,
        )
    }
}

#[derive(Clone, Debug)]
pub struct PreparedOddInverse<const L: usize> {
    #[cfg(target_pointer_width = "64")]
    modulus: crypto_bigint::Odd<crypto_bigint::Uint<L>>,
    #[cfg(not(target_pointer_width = "64"))]
    ring: ModRingCtx<L>,
}
impl<const L: usize> PreparedOddInverse<L> {
    pub fn new(modulus: Uint<L>) -> Result<Self, ContextError> {
        if modulus.as_words()[0] & 1 == 0 {
            return Err(ContextError::EvenModulus);
        }
        // On 64-bit targets the only thing `ModRingCtx::new` would give us is
        // its `modulus > 1` check and its own copy of `modulus`'s words — the
        // Barrett reduction table it also builds is never used on this path,
        // since `inverse_ct` below reduces via `crypto_bigint`'s own inversion
        // instead. Do the same validation directly and skip that table.
        #[cfg(target_pointer_width = "64")]
        {
            if modulus.ct_le(&Uint::ONE).declassify() {
                return Err(ContextError::InvalidModulus);
            }
            Ok(Self {
                modulus: crypto_bigint::Odd::new(crypto_bigint::Uint::from_words(
                    *modulus.as_words(),
                ))
                .expect("prepared modulus is odd"),
            })
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            Ok(Self {
                ring: ModRingCtx::new(modulus)?,
            })
        }
    }
    /// Fixed-schedule inversion, with a zero output for nonunits. On 64-bit
    /// targets the backend batches divsteps instead of updating every full-width
    /// coefficient on each binary-GCD step. Both backends use the declared width.
    #[cfg(target_pointer_width = "64")]
    pub fn inverse_ct(&self, value: &Uint<L>) -> CtValue<Uint<L>> {
        let inverse =
            crypto_bigint::Uint::from_words(*value.as_words()).invert_odd_mod(&self.modulus);
        let valid = CtMask::from_lsb(inverse.is_some().to_u8() as u64);
        let value = inverse.unwrap_or(crypto_bigint::Uint::ZERO);
        CtValue::new(Uint::from_words(value.to_words()), valid)
    }

    #[cfg(not(target_pointer_width = "64"))]
    pub fn inverse_ct(&self, value: &Uint<L>) -> CtValue<Uint<L>> {
        let modulus = self.ring.modulus();
        let (mut u, mut v) = (
            *modulus,
            self.ring.to_integer(&self.ring.from_integer(value)),
        );
        let (mut r, mut s) = (Uint::ZERO, Uint::ONE);
        // Maintain u = value*r and v = value*s (mod m). Every active step
        // halves a positive operand, or subtracts two odds and halves the larger.
        // Thus bit_length(u)+bit_length(v) drops at least once per active step.
        for _ in 0..128 * L {
            let active = !u.ct_is_zero() & !v.ct_is_zero();
            let u_even = !u.bit(0).mask();
            let v_even = !v.bit(0).mask();
            let u_greater = v.ct_lt(&u);
            let halve_u = active & u_even;
            let halve_v = active & !u_even & v_even;
            let subtract_u = active & !u_even & !v_even & u_greater;
            let subtract_v = active & !u_even & !v_even & !u_greater;
            let difference_u = u.wrapping_sub(&v).shr(1);
            let difference_v = v.wrapping_sub(&u).shr(1);
            let difference_r = self.ring.sub(&Residue(r), &Residue(s)).0;
            let difference_s = self.ring.sub(&Residue(s), &Residue(r)).0;
            u = Uint::ct_select(&u, &u.shr(1), halve_u);
            u = Uint::ct_select(&u, &difference_u, subtract_u);
            v = Uint::ct_select(&v, &v.shr(1), halve_v);
            v = Uint::ct_select(&v, &difference_v, subtract_v);
            r = Uint::ct_select(&r, &self.halve_residue(&r), halve_u);
            r = Uint::ct_select(&r, &self.halve_residue(&difference_r), subtract_u);
            s = Uint::ct_select(&s, &self.halve_residue(&s), halve_v);
            s = Uint::ct_select(&s, &self.halve_residue(&difference_s), subtract_v);
        }
        let u_one = u.ct_eq(&Uint::ONE);
        let valid = u_one | v.ct_eq(&Uint::ONE);
        let inverse = Uint::ct_select(&s, &r, u_one);
        CtValue::new(Uint::ct_select(&Uint::ZERO, &inverse, valid), valid)
    }

    #[cfg(not(target_pointer_width = "64"))]
    fn halve_residue(&self, value: &Uint<L>) -> Uint<L> {
        let odd = value.bit(0).mask();
        let (sum, carry) = value.adc(self.ring.modulus());
        let mut half = Uint::ct_select(value, &sum, odd).shr(1);
        half.0[L - 1] |= (carry & odd.word()) << 63;
        half
    }
}

enum PowerTable<T> {
    Private(Vec<T>),
    Public {
        base: T,
        rows: Vec<Vec<T>>,
        window: usize,
    },
}

/// Provider-owned fixed-base preparation with explicit exponent timing contracts.
pub struct FixedBasePow<C: RingOps, const EXP_LIMBS: usize> {
    field: C,
    table: PowerTable<C::Elem>,
}
impl<C: RingOps, const E: usize> FixedBasePow<C, E> {
    pub fn new(field: C, base: C::Elem) -> Self {
        const {
            assert!(E > 0);
        }
        let mut powers = Vec::with_capacity(E * 64);
        let mut value = base;
        for _ in 0..E * 64 {
            powers.push(value);
            value = field.square(&value);
        }
        Self {
            field,
            table: PowerTable::Private(powers),
        }
    }
    /// Window tables are indexed only by public exponents passed to `pow_public`.
    /// Private exponentiation remains available through `pow_ct`, without using
    /// these tables or accessing an exponent-dependent address.
    pub fn new_public(field: C, base: C::Elem, window: usize) -> Self {
        const {
            assert!(E > 0);
        }
        assert!((1..=16).contains(&window), "window must be in 1..=16");
        let mut rows = Vec::with_capacity((64 * E).div_ceil(window));
        let mut power = base;
        for _ in 0..(64 * E).div_ceil(window) {
            let mut row = Vec::with_capacity(1 << window);
            let mut value = field.one();
            for _ in 0..1 << window {
                row.push(value);
                value = field.mul(&value, &power);
            }
            rows.push(row);
            for _ in 0..window {
                power = field.square(&power);
            }
        }
        Self {
            field,
            table: PowerTable::Public { base, rows, window },
        }
    }
    pub fn pow_ct(&self, exponent: &Uint<E>) -> C::Elem {
        let powers = match &self.table {
            PowerTable::Private(powers) => powers,
            PowerTable::Public { base, .. } => return self.field.pow_ct(base, exponent),
        };
        let mut value = self.field.one();
        for i in 0..E * 64 {
            let product = self.field.mul(&value, &powers[i]);
            value = C::Elem::ct_select(&value, &product, exponent.bit(i).mask());
        }
        value
    }
    /// Variable-time and public-exponent-only: indices and zero skips depend on
    /// the exponent. `pow_ct` always retains its private-input contract.
    pub fn pow_public(&self, exponent: &Uint<E>) -> C::Elem {
        let PowerTable::Public { rows, window, .. } = &self.table else {
            return self.pow_ct(exponent);
        };
        if E == 2 && *window == 8 {
            // The verifier's 128-bit public weights use byte windows. A
            // balanced product exposes independent field multiplications instead
            // of putting every table lookup on one long dependency chain.
            // A public width chooses a balanced prefix, retaining factor order
            // and dropping only high windows whose table entry is the identity.
            return if exponent.0[1] != 0 {
                self.pow_public_byte_pairs::<8>(rows, exponent)
            } else if exponent.0[0] >> 32 != 0 {
                self.pow_public_byte_pairs::<4>(rows, exponent)
            } else if exponent.0[0] >> 16 != 0 {
                self.pow_public_byte_pairs::<2>(rows, exponent)
            } else {
                self.pow_public_byte_pairs::<1>(rows, exponent)
            };
        }
        let mut value = self.field.one();
        for (i, row) in rows.iter().enumerate() {
            let bit = i * window;
            let (word, shift) = (bit / 64, bit % 64);
            let mut digit = exponent.0[word] >> shift;
            if shift + window > 64 && word + 1 < E {
                digit |= exponent.0[word + 1] << (64 - shift);
            }
            digit &= (1 << window) - 1;
            if digit != 0 {
                value = self.field.mul(&value, &row[digit as usize]);
            }
        }
        value
    }

    #[inline]
    fn pow_public_byte_pairs<const PAIRS: usize>(
        &self,
        rows: &[Vec<C::Elem>],
        exponent: &Uint<E>,
    ) -> C::Elem {
        let mut products = [self.field.one(); PAIRS];
        for (i, product) in products.iter_mut().enumerate() {
            let word = exponent.0[i / 4];
            let shift = (i % 4) * 16;
            let low = ((word >> shift) & 255) as usize;
            let high = ((word >> (shift + 8)) & 255) as usize;
            *product = self.field.mul(&rows[2 * i][low], &rows[2 * i + 1][high]);
        }
        let mut count = products.len();
        while count > 1 {
            for i in 0..count / 2 {
                products[i] = self.field.mul(&products[2 * i], &products[2 * i + 1]);
            }
            count /= 2;
        }
        products[0]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicProductBounds {
    pub lhs_limbs: usize,
    pub rhs_limbs: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeError {
    InputLengths,
    OutputLength,
    PublicWidth,
}
impl core::fmt::Display for ShapeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "arithmetic shape: {self:?}")
    }
}
impl std::error::Error for ShapeError {}

/// Reusable storage binding. Values must fit the declared public active widths;
/// the witness validation layer establishes this before private execution.
pub struct PreparedProducts<'a, const A: usize, const B: usize> {
    lhs: &'a [Uint<A>],
    rhs: &'a [Uint<B>],
    out: &'a mut [UintProduct<A, B>],
    bounds: PublicProductBounds,
}
impl<'a, const A: usize, const B: usize> PreparedProducts<'a, A, B> {
    pub fn new(
        lhs: &'a [Uint<A>],
        rhs: &'a [Uint<B>],
        out: &'a mut [UintProduct<A, B>],
        bounds: PublicProductBounds,
    ) -> Result<Self, ShapeError> {
        if lhs.len() != rhs.len() {
            return Err(ShapeError::InputLengths);
        }
        if lhs.len() != out.len() {
            return Err(ShapeError::OutputLength);
        }
        if bounds.lhs_limbs == 0
            || bounds.lhs_limbs > A
            || bounds.rhs_limbs == 0
            || bounds.rhs_limbs > B
        {
            return Err(ShapeError::PublicWidth);
        }
        Ok(Self {
            lhs,
            rhs,
            out,
            bounds,
        })
    }
    pub fn execute(&mut self) {
        use crate::integer::product::Words;
        for ((a, b), out) in self.lhs.iter().zip(self.rhs).zip(self.out.iter_mut()) {
            debug_assert!(
                a.as_words()[self.bounds.lhs_limbs..]
                    .iter()
                    .all(|v| *v == 0)
            );
            debug_assert!(
                b.as_words()[self.bounds.rhs_limbs..]
                    .iter()
                    .all(|v| *v == 0)
            );
            *out = UintProduct::ZERO;
            for i in 0..self.bounds.lhs_limbs {
                let mut carry = 0;
                for j in 0..self.bounds.rhs_limbs {
                    let sum = a.as_words()[i] as u128 * b.as_words()[j] as u128
                        + out.word(i + j) as u128
                        + carry;
                    out.set_word(i + j, sum as u64);
                    carry = sum >> 64;
                }
                out.set_word(i + self.bounds.rhs_limbs, carry as u64);
            }
        }
    }
    pub fn outputs(&self) -> &[UintProduct<A, B>] {
        self.out
    }
}

/// Retains a provider once for repeated projection of full declared integer widths.
pub struct PreparedIntegerProjection<C> {
    field: C,
}
impl<C> PreparedIntegerProjection<C> {
    pub fn new(field: C) -> Self {
        Self { field }
    }
    pub fn project<T>(&self, value: &T) -> C::Elem
    where
        C: IntegerEmbedding<T>,
    {
        self.field.from_integer(value)
    }
    pub fn project_into<T>(&self, input: &[T], out: &mut [C::Elem])
    where
        C: IntegerEmbedding<T>,
    {
        assert_eq!(input.len(), out.len());
        for (value, dst) in input.iter().zip(out) {
            *dst = self.field.from_integer(value);
        }
    }
}

#[cfg(test)]
mod public_power_tests {
    use super::*;

    // Frozen pre-optimization 16-byte algorithm, independent of the new helper.
    #[inline(never)]
    fn previous_pow<C: RingOps>(table: &FixedBasePow<C, 2>, exponent: &Uint<2>) -> C::Elem {
        let PowerTable::Public { rows, window, .. } = &table.table else {
            panic!("public table required");
        };
        assert_eq!(*window, 8);
        let mut products = [table.field.one(); 8];
        for (i, product) in products.iter_mut().enumerate() {
            let word = exponent.0[i / 4];
            let shift = (i % 4) * 16;
            let low = ((word >> shift) & 255) as usize;
            let high = ((word >> (shift + 8)) & 255) as usize;
            *product = table.field.mul(&rows[2 * i][low], &rows[2 * i + 1][high]);
        }
        let mut count = products.len();
        while count > 1 {
            for i in 0..count / 2 {
                products[i] = table.field.mul(&products[2 * i], &products[2 * i + 1]);
            }
            count /= 2;
        }
        products[0]
    }

    fn binary_power<C: RingOps>(field: &C, mut base: C::Elem, mut exponent: u128) -> C::Elem {
        let mut result = field.one();
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = field.mul(&result, &base);
            }
            base = field.square(&base);
            exponent >>= 1;
        }
        result
    }

    fn generator() -> Gf128 {
        const FACTORS: [u128; 9] = [3, 5, 17, 257, 641, 65537, 274177, 6700417, 67280421310721];
        (2..)
            .map(Gf128::from_polynomial_bits)
            .find(|&base| {
                FACTORS
                    .iter()
                    .all(|&factor| binary_power(&Gf128Ops, base, u128::MAX / factor) != Gf128::ONE)
            })
            .unwrap()
    }

    fn exponents() -> Vec<u128> {
        let mut values = vec![0, 1, 2, u128::MAX];
        for bit in [8, 16, 32, 64, 113, 127] {
            let boundary = 1u128 << bit;
            values.extend([boundary - 1, boundary, boundary + 1]);
        }
        let mut state = 0x437a_20d1_67ba_197fu128;
        for _ in 0..64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(state);
        }
        values
    }

    fn check<C: RingOps>(field: C, base: C::Elem)
    where
        C::Elem: std::fmt::Debug + PartialEq,
    {
        let table = FixedBasePow::<_, 2>::new_public(&field, base, 8);
        for exponent in exponents() {
            let words = Uint::from_words([exponent as u64, (exponent >> 64) as u64]);
            let expected = binary_power(&field, base, exponent);
            assert_eq!(table.pow_public(&words), expected, "exponent {exponent}");
            assert_eq!(previous_pow(&table, &words), expected);
            assert_eq!(table.pow_ct(&words), expected);
        }
    }

    #[test]
    fn public_byte_buckets_match_binary_power_and_previous_tree() {
        for base in [
            Gf128::ZERO,
            Gf128::ONE,
            generator(),
            Gf128::new(0xfabc_3811_dea9_8037, 0x8ef7_920d_0065_ab13),
        ] {
            check(Gf128Ops, base);
        }
    }

    #[test]
    fn public_byte_buckets_support_composite_ring_and_prime_providers() {
        let ring = ModRingCtx::new(Uint::from_words([18])).unwrap();
        for base in [0u64, 1, 2, 7, 9, 17] {
            check(&ring, ring.from_integer(&base));
        }
        let field = create_prime_field(Uint::from_words([17]));
        for base in [0u64, 1, 3, 16] {
            check(&field, field.from_integer(&base));
        }
    }

    #[test]
    fn public_fallback_windows_and_private_tables_match_binary_power() {
        let base = generator();
        for window in [1, 7, 9, 16] {
            let table = FixedBasePow::<_, 2>::new_public(Gf128Ops, base, window);
            for exponent in exponents() {
                let words = Uint::from_words([exponent as u64, (exponent >> 64) as u64]);
                assert_eq!(
                    table.pow_public(&words),
                    binary_power(&Gf128Ops, base, exponent)
                );
            }
        }
        let private = FixedBasePow::<_, 2>::new(Gf128Ops, base);
        for exponent in exponents() {
            let words = Uint::from_words([exponent as u64, (exponent >> 64) as u64]);
            let expected = binary_power(&Gf128Ops, base, exponent);
            assert_eq!(private.pow_ct(&words), expected);
            assert_eq!(private.pow_public(&words), expected);
        }
    }

    #[test]
    #[ignore = "isolated public exponentiation timing; run on an otherwise idle pinned CPU"]
    fn public_limb_power_abba_microbenchmark() {
        use std::{hint::black_box, time::Instant};

        #[inline(never)]
        fn current_pow(table: &FixedBasePow<Gf128Ops, 2>, exponent: &Uint<2>) -> Gf128 {
            table.pow_public(exponent)
        }
        fn timed(
            table: &FixedBasePow<Gf128Ops, 2>,
            exponents: &[Uint<2>],
            f: fn(&FixedBasePow<Gf128Ops, 2>, &Uint<2>) -> Gf128,
        ) -> f64 {
            let start = Instant::now();
            for _ in 0..16 {
                for exponent in exponents {
                    black_box(f(black_box(table), black_box(exponent)));
                }
            }
            start.elapsed().as_secs_f64() * 1000.0 / 16.0
        }

        let table = FixedBasePow::<_, 2>::new_public(Gf128Ops, generator(), 8);
        let mut random = 0x875b_2ad9_1ba6_3df1_46c2_1f83_9a07_68d3u128;
        let mut next = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            random
        };
        // Prime equality weights exercise the same 13-coordinate construction
        // as Falcon, then the same 113-bit integer split. Setup is not timed.
        let prime = FpCtx::from_prime_u128(u128::MAX - 158);
        let mut weights = vec![prime.one()];
        for _ in 0..13 {
            let r = prime.from_integer(&next());
            let complement = prime.sub(&prime.one(), &r);
            let old_len = weights.len();
            for i in 0..old_len {
                weights.push(prime.mul(&weights[i], &r));
                weights[i] = prime.mul(&weights[i], &complement);
            }
        }
        let full: Vec<_> = weights.iter().map(|w| prime.to_integer(w)).collect();
        let split: Vec<_> = full
            .iter()
            .flat_map(|w| {
                let value = u128::from(w.0[0]) | (u128::from(w.0[1]) << 64);
                let low = value & ((1u128 << 113) - 1);
                [
                    Uint::from_words([low as u64, (low >> 64) as u64]),
                    Uint::from_words([(value >> 113) as u64, 0]),
                ]
            })
            .collect();
        for (label, exponents) in [("8192_split_113_15", &split), ("8192_full_128", &full)] {
            for exponent in exponents {
                assert_eq!(
                    current_pow(&table, exponent),
                    previous_pow(&table, exponent)
                );
            }
            for _ in 0..4 {
                black_box(timed(&table, exponents, previous_pow));
                black_box(timed(&table, exponents, current_pow));
            }
            let mut previous = 0.0;
            let mut current = 0.0;
            for _ in 0..48 {
                previous += timed(&table, exponents, previous_pow);
                current += timed(&table, exponents, current_pow);
                current += timed(&table, exponents, current_pow);
                previous += timed(&table, exponents, previous_pow);
            }
            previous /= 96.0;
            current /= 96.0;
            eprintln!(
                "{label}: previous_ms={previous:.6} current_ms={current:.6} ratio={:.6} saved_ms={:.6}",
                current / previous,
                previous - current
            );
        }
    }
}
