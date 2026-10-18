//! GF(2^128) and GF(2^256) arithmetic in the BitZ circuit language, in
//! Flock's (and Binius's) basis: `GF(2^128) = F2[x]/(x^128 + x^7 + x^2 + x + 1)`
//! with bit `i` the coefficient of `x^i`, and `GF(2^256) = GF(2^128)[u]/(u^2 +
//! u + x^-1)`.
//!
//! An element is 128 F2 linear combinations. Addition, squaring and
//! multiplication by a public constant are F2-linear, hence free: they only
//! rewrite linear combinations. A product of two variable elements is ONE
//! rank-1 row over the integers. With the "spread" encodings
//! `A = Σ_i a_i 2^{8i}` and `B = Σ_j b_j 2^{8j}`, the integer product is
//! `Σ_k c_k 2^{8k}` with `c_k = #{(i, j) : i + j = k, a_i b_j = 1}`; since
//! `c_k ≤ min(k + 1, 255 − k) ≤ 128 < 2^8`, no slot carries into the next. The
//! hint commits every `c_k` in binary (1546 bits in all), the row asserts
//! `A · B = Σ_k c_k 2^{8k}` over the integers, and the carry-less product's
//! coefficient `k` is the low bit of `c_k`. Reduction modulo the pentanomial
//! is F2-linear in those low bits.

use std::array;

use num_traits::{One, Zero};

use crate::{BoolWitness, Circuit, HintResult, PackedBits, WitnessContext};

/// Integer limbs of the product row (`A·B < 2^2034`, `C < 2^2040`).
pub const MUL_LIMBS: usize = 32;

/// Binary width of slot `k` of the integer product: `bitlen(min(k+1, 255−k))`.
pub const fn slot_width(k: usize) -> usize {
    let m = if k + 1 < 255 - k { k + 1 } else { 255 - k };
    (usize::BITS - m.leading_zeros()) as usize
}

/// Committed bits per variable product.
pub const SLOT_BITS: usize = {
    let mut total = 0;
    let mut k = 0;
    while k < 255 {
        total += slot_width(k);
        k += 1;
    }
    total
};
const SLOT_LIMBS: usize = SLOT_BITS.div_ceil(64);

const _: () = assert!(SLOT_BITS == 1546);

/// A GF(2^128) element: bit `i` is the coefficient of `x^i`.
pub type Gf<BW> = [BW; 128];

/// A GF(2^256) element `c0 + c1·u`.
#[derive(Clone)]
pub struct Gf256<BW> {
    pub c0: Gf<BW>,
    pub c1: Gf<BW>,
}

// ---------------------------------------------------------------------------
// Native reference arithmetic (Flock's `F128`/`F256`), on `u128`.
// ---------------------------------------------------------------------------

/// Carry-less 128×128 → 256-bit product, as `(lo, hi)`.
pub fn clmul(a: u128, b: u128) -> (u128, u128) {
    let (mut lo, mut hi) = (0u128, 0u128);
    for i in 0..128 {
        if (a >> i) & 1 == 1 {
            lo ^= b << i;
            if i != 0 {
                hi ^= b >> (128 - i);
            }
        }
    }
    (lo, hi)
}

/// `x^{128+j} mod (x^128 + x^7 + x^2 + x + 1)`, for `j < 128`.
pub fn reduction_column(j: usize) -> u128 {
    // x^128 = x^7 + x^2 + x + 1; fold repeatedly.
    let mut v: u128 = 1u128 << j; // x^j · x^128 represented as pending high part
    let mut out = 0u128;
    // multiply v (a polynomial of degree < 128) by x^128 ≡ 0x87.
    loop {
        let (lo, hi) = clmul(v, 0x87);
        out ^= lo;
        if hi == 0 {
            break;
        }
        v = hi;
    }
    out
}

/// Native GF(2^128) product.
pub fn mul_native(a: u128, b: u128) -> u128 {
    let (lo, hi) = clmul(a, b);
    let mut out = lo;
    for j in 0..128 {
        if (hi >> j) & 1 == 1 {
            out ^= reduction_column(j);
        }
    }
    out
}

/// Native GF(2^128) inverse (`a^{2^128 − 2}`; 0 maps to 0).
pub fn inv_native(a: u128) -> u128 {
    let mut r = 1u128;
    let mut cur = mul_native(a, a);
    for _ in 1..128 {
        r = mul_native(r, cur);
        cur = mul_native(cur, cur);
    }
    r
}

/// `x^{-1}`, the constant of the quadratic extension `u^2 = u + x^{-1}`.
pub const QUADRATIC_NONRESIDUE: u128 = (1u128 << 127) | 0x43;

/// Native GF(2^256) product (Flock's Karatsuba with `u^2 = u + x^{-1}`).
pub fn mul256_native(a: (u128, u128), b: (u128, u128)) -> (u128, u128) {
    let p0 = mul_native(a.0, b.0);
    let p1 = mul_native(a.1, b.1);
    let p2 = mul_native(a.0 ^ a.1, b.0 ^ b.1);
    (p0 ^ mul_native(QUADRATIC_NONRESIDUE, p1), p2 ^ p0)
}

// ---------------------------------------------------------------------------
// Free (F2-linear) operations.
// ---------------------------------------------------------------------------

pub fn constant<BW: BoolWitness>(value: u128) -> Gf<BW> {
    array::from_fn(|i| BW::from((value >> i) & 1 == 1))
}

pub fn zero<BW: BoolWitness>() -> Gf<BW> {
    constant(0)
}

pub fn add<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>, b: &Gf<CS::Bool>) -> Gf<CS::Bool> {
    array::from_fn(|i| circuit.xor(a[i].clone(), b[i].clone()))
}

pub fn add_const<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>, c: u128) -> Gf<CS::Bool> {
    array::from_fn(|i| {
        if (c >> i) & 1 == 1 {
            circuit.xor(a[i].clone(), CS::Bool::from(true))
        } else {
            a[i].clone()
        }
    })
}

/// Applies the F2-linear map whose column `j` (the image of `x^j`) is
/// `columns[j]`.
pub fn linear_map<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf<CS::Bool>,
    columns: &[u128; 128],
) -> Gf<CS::Bool> {
    array::from_fn(|i| {
        let mut acc = CS::Bool::zero();
        for (j, column) in columns.iter().enumerate() {
            if (column >> i) & 1 == 1 {
                acc = circuit.xor(acc, a[j].clone());
            }
        }
        acc
    })
}

/// `a · x` in GF(2^128): a shift and a conditional reduction.
pub const fn mul_x(a: u128) -> u128 {
    (a << 1) ^ if a >> 127 == 1 { 0x87 } else { 0 }
}

/// Columns of `a ↦ c·a`: `c·x^j` for `j < 128`.
pub fn mul_const_columns(c: u128) -> [u128; 128] {
    let mut columns = [0u128; 128];
    let mut v = c;
    for column in &mut columns {
        *column = v;
        v = mul_x(v);
    }
    columns
}

/// Columns of `a ↦ a^{2^k}`.
pub fn frobenius_columns(k: usize) -> [u128; 128] {
    array::from_fn(|j| {
        let mut v = 1u128 << j;
        for _ in 0..k {
            v = mul_native(v, v);
        }
        v
    })
}

pub fn mul_const<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>, c: u128) -> Gf<CS::Bool> {
    match c {
        0 => zero(),
        1 => a.clone(),
        _ => linear_map(circuit, a, &mul_const_columns(c)),
    }
}

pub fn square<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>) -> Gf<CS::Bool> {
    linear_map(circuit, a, &frobenius_columns(1))
}

/// Reduces a 255-coefficient carry-less product modulo the pentanomial.
fn reduce<CS: Circuit>(circuit: &mut CS, product: &[CS::Bool]) -> Gf<CS::Bool> {
    debug_assert_eq!(product.len(), 255);
    let mut out: Gf<CS::Bool> = array::from_fn(|i| product[i].clone());
    for j in 0..127 {
        let column = reduction_column(j);
        for (i, slot) in out.iter_mut().enumerate() {
            if (column >> i) & 1 == 1 {
                *slot = circuit.xor(slot.clone(), product[128 + j].clone());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Variable products: one integer rank-1 row each.
// ---------------------------------------------------------------------------

/// An element together with its spread integer encoding `Σ a_i 2^{8i}`, so an
/// operand reused across products is lifted once.
#[derive(Clone)]
pub struct Lifted<CS: Circuit> {
    pub bits: Gf<CS::Bool>,
    spread: CS::Z<MUL_LIMBS>,
}

fn power_of_two<CS: Circuit>(exponent: usize) -> CS::Coefficient<MUL_LIMBS> {
    let mut words = [0u64; MUL_LIMBS];
    words[exponent / 64] = 1u64 << (exponent % 64);
    CS::coefficient_from_le_words::<MUL_LIMBS>(&words)
}

pub fn lift<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>) -> Lifted<CS> {
    let mut spread = CS::Z::<MUL_LIMBS>::zero();
    for (i, bit) in a.iter().enumerate() {
        let cell = circuit.bitz::<MUL_LIMBS>(bit.clone());
        spread += cell * power_of_two::<CS>(8 * i);
    }
    Lifted {
        bits: a.clone(),
        spread,
    }
}

fn evaluate<ZW, BW: BoolWitness, C>(
    context: &dyn WitnessContext<ZW, BW, C>,
    a: &Gf<BW>,
) -> u128 {
    a.iter()
        .enumerate()
        .fold(0u128, |v, (i, bit)| v | (u128::from(context.eval_bool(bit)) << i))
}

/// The slot counts `c_k` of `a ⊗ b` packed at their widths.
fn slot_bits(a: u128, b: u128) -> PackedBits<SLOT_BITS, SLOT_LIMBS> {
    let mut counts = [0u16; 255];
    for i in 0..128 {
        if (a >> i) & 1 == 1 {
            let mut rest = b;
            while rest != 0 {
                let j = rest.trailing_zeros() as usize;
                counts[i + j] += 1;
                rest &= rest - 1;
            }
        }
    }
    let mut words = [0u64; SLOT_LIMBS];
    let mut offset = 0;
    for (k, &count) in counts.iter().enumerate() {
        for t in 0..slot_width(k) {
            if (count >> t) & 1 == 1 {
                words[offset / 64] |= 1u64 << (offset % 64);
            }
            offset += 1;
        }
    }
    PackedBits::from_words(words)
}

pub fn mul_lifted<CS: Circuit>(
    circuit: &mut CS,
    a: &Lifted<CS>,
    b: &Lifted<CS>,
) -> Gf<CS::Bool> {
    let (abits, bbits) = (a.bits.clone(), b.bits.clone());
    let slots = circuit.hint::<MUL_LIMBS, SLOT_BITS, SLOT_LIMBS, _>(move |context| {
        HintResult::Ok(slot_bits(evaluate(context, &abits), evaluate(context, &bbits)))
    });
    let mut c = CS::Z::<MUL_LIMBS>::zero();
    let mut low = Vec::with_capacity(255);
    let mut offset = 0;
    for k in 0..255 {
        for t in 0..slot_width(k) {
            let bit = crate::BoolRepresentation::bit(&slots, offset);
            if t == 0 {
                low.push(bit.clone());
            }
            let cell = circuit.bitz::<MUL_LIMBS>(bit);
            c += cell * power_of_two::<CS>(8 * k + t);
            offset += 1;
        }
    }
    circuit.assert_r1c::<MUL_LIMBS>(a.spread.clone(), b.spread.clone(), c);
    reduce(circuit, &low)
}

pub fn mul<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>, b: &Gf<CS::Bool>) -> Gf<CS::Bool> {
    let a = lift(circuit, a);
    let b = lift(circuit, b);
    mul_lifted(circuit, &a, &b)
}

/// Asserts that every bit of `a` is zero: one lifted cell per bit and one
/// linear row (a sum of distinct powers of two vanishes only if every bit does).
pub fn assert_zero<CS: Circuit>(circuit: &mut CS, a: &[CS::Bool]) {
    for chunk in a.chunks(64) {
        let mut sum = CS::Z::<2>::zero();
        let mut power = CS::Coefficient::<2>::one();
        for bit in chunk {
            let cell = circuit.bitz::<2>(bit.clone());
            sum += cell * power.clone();
            power += power.clone();
        }
        circuit.assert_r1c::<2>(CS::Z::<2>::zero(), CS::Z::<2>::zero(), sum);
    }
}

pub fn assert_eq<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>, b: &Gf<CS::Bool>) {
    let diff = add(circuit, a, b);
    assert_zero(circuit, &diff);
}

/// Re-commits `bits` as fresh witness bits with an equality check, so later
/// lifts reference one committed bit each instead of the original (possibly
/// wide) F2 combinations. Costs `n` committed bits and `n` cells.
pub fn refresh_bits<CS: Circuit>(circuit: &mut CS, bits: &[CS::Bool]) -> Vec<CS::Bool> {
    let mut out = Vec::with_capacity(bits.len());
    for chunk in bits.chunks(128) {
        let src = chunk.to_vec();
        let n = src.len();
        let packed = circuit.hint::<1, 128, 2, _>(move |ctx| {
            HintResult::Ok(PackedBits::from_fn(|i| i < n && ctx.eval_bool(&src[i])))
        });
        let fresh: Vec<CS::Bool> = (0..128).map(|i| crate::BoolRepresentation::bit(&packed, i)).collect();
        let diff: Vec<CS::Bool> = (0..128)
            .map(|i| match chunk.get(i) {
                Some(b) => circuit.xor(fresh[i].clone(), b.clone()),
                None => fresh[i].clone(),
            })
            .collect();
        assert_zero(circuit, &diff);
        out.extend(fresh.into_iter().take(n));
    }
    out
}

pub fn refresh<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>) -> Gf<CS::Bool> {
    let v = refresh_bits(circuit, a);
    array::from_fn(|i| v[i].clone())
}

/// `a^{-1}` (witnessed) with `a · a^{-1} = 1` asserted; unsatisfiable at 0.
pub fn inv<CS: Circuit>(circuit: &mut CS, a: &Gf<CS::Bool>) -> Gf<CS::Bool> {
    let abits = a.clone();
    let packed = circuit.hint::<1, 128, 2, _>(move |context| {
        let v = inv_native(evaluate(context, &abits));
        HintResult::Ok(PackedBits::from_words([v as u64, (v >> 64) as u64]))
    });
    let inverse: Gf<CS::Bool> = array::from_fn(|i| crate::BoolRepresentation::bit(&packed, i));
    let product = mul(circuit, a, &inverse);
    let one = add_const(circuit, &product, 1);
    assert_zero(circuit, &one);
    inverse
}

/// Allocates a fresh committed element holding `value` (proof data).
pub fn witness<CS: Circuit>(circuit: &mut CS, value: u128) -> Gf<CS::Bool> {
    let packed = circuit.hint::<1, 128, 2, _>(move |_| {
        HintResult::Ok(PackedBits::from_words([value as u64, (value >> 64) as u64]))
    });
    array::from_fn(|i| crate::BoolRepresentation::bit(&packed, i))
}

// ---------------------------------------------------------------------------
// GF(2^256).
// ---------------------------------------------------------------------------

pub fn from_base<BW: BoolWitness>(a: Gf<BW>) -> Gf256<BW> {
    Gf256 { c0: a, c1: zero() }
}

pub fn add256<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf256<CS::Bool>,
    b: &Gf256<CS::Bool>,
) -> Gf256<CS::Bool> {
    Gf256 {
        c0: add(circuit, &a.c0, &b.c0),
        c1: add(circuit, &a.c1, &b.c1),
    }
}

/// `a · b` in GF(2^256): three base products (Karatsuba).
pub fn mul256<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf256<CS::Bool>,
    b: &Gf256<CS::Bool>,
) -> Gf256<CS::Bool> {
    let p0 = mul(circuit, &a.c0, &b.c0);
    let p1 = mul(circuit, &a.c1, &b.c1);
    let a01 = add(circuit, &a.c0, &a.c1);
    let b01 = add(circuit, &b.c0, &b.c1);
    let p2 = mul(circuit, &a01, &b01);
    let p1x = mul_const(circuit, &p1, QUADRATIC_NONRESIDUE);
    Gf256 {
        c0: add(circuit, &p0, &p1x),
        c1: add(circuit, &p2, &p0),
    }
}

/// `a · b` with `b` in the base field: two base products.
pub fn mul256_base<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf256<CS::Bool>,
    b: &Gf<CS::Bool>,
) -> Gf256<CS::Bool> {
    let lb = lift(circuit, b);
    mul256_base_lifted(circuit, a, &lb)
}

/// [`mul256_base`] with the base operand lifted once by the caller.
pub fn mul256_base_lifted<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf256<CS::Bool>,
    b: &Lifted<CS>,
) -> Gf256<CS::Bool> {
    let a0 = lift(circuit, &a.c0);
    let a1 = lift(circuit, &a.c1);
    Gf256 {
        c0: mul_lifted(circuit, &a0, b),
        c1: mul_lifted(circuit, &a1, b),
    }
}

/// Constant GF(2^256) element `c0 + c1·u`.
pub fn constant256<BW: BoolWitness>(c0: u128, c1: u128) -> Gf256<BW> {
    Gf256 {
        c0: constant(c0),
        c1: constant(c1),
    }
}

/// `a · (c0 + c1·u)` for a public constant: F2-linear, free.
pub fn mul256_const<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf256<CS::Bool>,
    c: (u128, u128),
) -> Gf256<CS::Bool> {
    // (a0 + a1 u)(c0 + c1 u) = (a0 c0 + a1 c1 x^-1) + (a0 c1 + a1 c0 + a1 c1) u.
    let a0c0 = mul_const(circuit, &a.c0, c.0);
    let a1c1x = mul_const(circuit, &a.c1, mul_native(c.1, QUADRATIC_NONRESIDUE));
    let a0c1 = mul_const(circuit, &a.c0, c.1);
    let a1c = mul_const(circuit, &a.c1, c.0 ^ c.1);
    Gf256 {
        c0: add(circuit, &a0c0, &a1c1x),
        c1: add(circuit, &a0c1, &a1c),
    }
}

/// `a · u`: F2-linear, free.
pub fn mul256_u<CS: Circuit>(circuit: &mut CS, a: &Gf256<CS::Bool>) -> Gf256<CS::Bool> {
    mul256_const(circuit, a, (0, 1))
}

/// `a^2`: F2-linear, free.
pub fn square256<CS: Circuit>(circuit: &mut CS, a: &Gf256<CS::Bool>) -> Gf256<CS::Bool> {
    let s0 = square(circuit, &a.c0);
    let s1 = square(circuit, &a.c1);
    let s1x = mul_const(circuit, &s1, QUADRATIC_NONRESIDUE);
    Gf256 {
        c0: add(circuit, &s0, &s1x),
        c1: s1,
    }
}

pub fn add256_const<CS: Circuit>(
    circuit: &mut CS,
    a: &Gf256<CS::Bool>,
    c: (u128, u128),
) -> Gf256<CS::Bool> {
    Gf256 {
        c0: add_const(circuit, &a.c0, c.0),
        c1: add_const(circuit, &a.c1, c.1),
    }
}

/// `eq(point, ·)` over `{0,1}^n`, index bit `j` ↔ `point[j]` (LSB first, as
/// Flock's `IndexOrder::LowToHigh`). One product per new entry.
pub fn eq_table<CS: Circuit>(circuit: &mut CS, point: &[Gf<CS::Bool>]) -> Vec<Gf<CS::Bool>> {
    let mut table: Vec<Gf<CS::Bool>> = vec![constant(1)];
    for (v, c) in point.iter().enumerate() {
        let half = 1usize << v;
        let mut high = Vec::with_capacity(half);
        for low in &table {
            high.push(if v == 0 { c.clone() } else { mul(circuit, low, c) });
        }
        for (low, h) in table.iter_mut().zip(&high) {
            *low = add(circuit, low, h);
        }
        table.extend(high);
    }
    table
}

/// [`eq_table`] over GF(2^256) coordinates.
pub fn eq_table256<CS: Circuit>(
    circuit: &mut CS,
    point: &[Gf256<CS::Bool>],
) -> Vec<Gf256<CS::Bool>> {
    let mut table: Vec<Gf256<CS::Bool>> = vec![constant256(1, 0)];
    for (v, c) in point.iter().enumerate() {
        let half = 1usize << v;
        let mut high = Vec::with_capacity(half);
        for low in &table {
            high.push(if v == 0 { c.clone() } else { mul256(circuit, low, c) });
        }
        for (low, h) in table.iter_mut().zip(&high) {
            *low = add256(circuit, low, h);
        }
        table.extend(high);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::ConstraintGenerator;
    use crate::stats::{Dummy, Stats};
    use crate::witgen::Witgen;

    fn value(a: &Gf<bool>) -> u128 {
        a.iter()
            .enumerate()
            .fold(0u128, |v, (i, &b)| v | (u128::from(b) << i))
    }

    fn rng(seed: u64) -> impl FnMut() -> u128 {
        let mut s = seed ^ 0x9e37_79b9_7f4a_7c15;
        move || {
            let mut next = || {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                s
            };
            (u128::from(next()) << 64) | u128::from(next())
        }
    }

    #[test]
    fn native_mul_is_a_field() {
        let mut r = rng(1);
        for _ in 0..20 {
            let (a, b, c) = (r(), r(), r());
            assert_eq!(mul_native(a, b), mul_native(b, a));
            assert_eq!(mul_native(mul_native(a, b), c), mul_native(a, mul_native(b, c)));
            assert_eq!(mul_native(a, b ^ c), mul_native(a, b) ^ mul_native(a, c));
            if a != 0 {
                assert_eq!(mul_native(a, inv_native(a)), 1);
            }
        }
        // x · x^127 = x^128 = x^7 + x^2 + x + 1.
        assert_eq!(mul_native(2, 1u128 << 127), 0x87);
        assert_eq!(mul_native(QUADRATIC_NONRESIDUE, 2), 1);
    }

    fn inputs(values: &[u128]) -> Vec<bool> {
        values
            .iter()
            .flat_map(|v| (0..128).map(move |i| (v >> i) & 1 == 1))
            .collect()
    }

    #[test]
    fn product_matches_native_and_satisfies_its_matrices() {
        let mut r = rng(7);
        for _ in 0..4 {
            let (a, b, c) = (r(), r(), r());
            let ins = inputs(&[a, b, c]);
            let mut witgen = Witgen::with_inputs(&ins);
            let ga: Gf<bool> = array::from_fn(|i| ins[i]);
            let gb: Gf<bool> = array::from_fn(|i| ins[128 + i]);
            let gc: Gf<bool> = array::from_fn(|i| ins[256 + i]);
            let ab = mul(&mut witgen, &ga, &gb);
            assert_eq!(value(&ab), mul_native(a, b));
            let sq = square(&mut witgen, &ga);
            assert_eq!(value(&sq), mul_native(a, a));
            let ac = mul_const(&mut witgen, &ga, c);
            assert_eq!(value(&ac), mul_native(a, c));
            let ia = inv(&mut witgen, &ga);
            assert_eq!(value(&ia), inv_native(a));
            let p = mul256(
                &mut witgen,
                &Gf256 { c0: ga, c1: gb },
                &Gf256 { c0: gc, c1: ab },
            );
            let expected = mul256_native((a, b), (c, mul_native(a, b)));
            assert_eq!((value(&p.c0), value(&p.c1)), expected);

            let mut generator = ConstraintGenerator::new(ins.len());
            let s: Vec<_> = (0..ins.len()).map(|i| generator.input(i)).collect();
            let sa: Gf<_> = array::from_fn(|i| s[i].clone());
            let sb: Gf<_> = array::from_fn(|i| s[128 + i].clone());
            let sc: Gf<_> = array::from_fn(|i| s[256 + i].clone());
            let sab = mul(&mut generator, &sa, &sb);
            let _ = square(&mut generator, &sa);
            let _ = mul_const(&mut generator, &sa, c);
            let _ = inv(&mut generator, &sa);
            let _ = mul256(
                &mut generator,
                &Gf256 { c0: sa, c1: sb },
                &Gf256 { c0: sc, c1: sab },
            );
            generator
                .into_matrices()
                .check_witness(witgen.witness())
                .expect("GF witness satisfies M/A/B/C");
        }
    }

    #[test]
    fn a_wrong_product_is_rejected() {
        let mut r = rng(9);
        let (a, b) = (r(), r());
        let ins = inputs(&[a, b]);
        let mut witgen = Witgen::with_inputs(&ins);
        let ga: Gf<bool> = array::from_fn(|i| ins[i]);
        let gb: Gf<bool> = array::from_fn(|i| ins[128 + i]);
        let _ = mul(&mut witgen, &ga, &gb);
        let mut generator = ConstraintGenerator::new(ins.len());
        let s: Vec<_> = (0..ins.len()).map(|i| generator.input(i)).collect();
        let _ = mul(
            &mut generator,
            &array::from_fn(|i| s[i].clone()),
            &array::from_fn(|i| s[128 + i].clone()),
        );
        let matrices = generator.into_matrices();
        // Flip one committed slot bit: the integer identity must break.
        let mut words = witgen.witness().packed_words().to_vec();
        let bit = 256 + 700;
        words[bit / 64] ^= 1u64 << (bit % 64);
        let total = ins.len() + SLOT_BITS;
        let tampered = Witgen::with_packed_inputs_and_capacity(&words, total, total);
        assert!(matrices.check_witness(witgen.witness()).is_ok());
        assert!(matrices.check_witness(tampered.witness()).is_err());
    }

    #[test]
    fn product_cost() {
        let mut stats = Stats::new(256);
        let _ = mul(&mut stats, &[Dummy; 128], &[Dummy; 128]);
        assert_eq!(stats.witnesses, 256 + 1546);
        assert_eq!(stats.bitz_calls, 256 + 1546);
        assert_eq!(stats.constraints, 1);
    }
}
