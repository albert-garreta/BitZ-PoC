//! Nonlinear and integer arithmetic of the verifier circuit, all with LINEAR
//! rows (`A = B = 0`), the form BitZ's structured outer relation proves:
//!
//! - an AND `r = a ∧ b` is one committed bit with `2r = a + b − (a ⊕ b)`;
//!   up to [`ANDS_PER_ROW`] of them share one row (`Σ 2^{k+1} r_k = Σ 2^k
//!   (a_k + b_k − (a_k ⊕ b_k))` forces every `r_k` by uniqueness of binary
//!   digits);
//! - a GF(2^128) product of two committed values is a Karatsuba tree of
//!   `3^7 = 2187` ANDs, then the (free) pentanomial reduction;
//! - `g^η` for a public base and committed exponent bits: one bit-times-word
//!   AND row (128 ANDs) per exponent bit;
//! - a congruence mod the public prime `q`: one integer row
//!   `Σ d_i x_i + c − Σ 2^b y_b − Σ 2^b q k_b = 0` with every coefficient
//!   pre-reduced mod `q` (so it fits an `i128`) and a committed quotient `k`.

use std::array;

use circuit::gf128::{self, Gf as Bits};
use circuit::{BoolRepresentation, BoolWitness, Circuit, HintResult, PackedBits};
use field::Gf128 as Gf;
use num_traits::Zero;

use super::val::{G, Public, gf_of, u128_of};

/// Integer limbs of every arithmetic row (signed 256-bit sums).
pub const LIMBS: usize = 4;

/// ANDs batched into one row (coefficients up to `2^65`).
pub const ANDS_PER_ROW: usize = 64;

type Z<CS> = <CS as Circuit>::Z<LIMBS>;
type C<CS> = <CS as Circuit>::Coefficient<LIMBS>;

fn coeff_u128<CS: Circuit>(v: u128) -> C<CS> {
    CS::coefficient_from_le_words::<LIMBS>(&[v as u64, (v >> 64) as u64])
}

fn coeff_pow2<CS: Circuit>(k: usize) -> C<CS> {
    let mut words = [0u64; LIMBS];
    words[k / 64] = 1u64 << (k % 64);
    CS::coefficient_from_le_words::<LIMBS>(&words)
}

/// `Σ (coefficient · bit)` as a Z expression; each term lifts its bit.
pub struct Row<CS: Circuit> {
    sum: Z<CS>,
}

impl<CS: Circuit> Default for Row<CS> {
    fn default() -> Self {
        Self { sum: Z::<CS>::zero() }
    }
}

impl<CS: Circuit> Row<CS> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bit(&mut self, cs: &mut CS, bit: &CS::Bool, coeff: u128, negative: bool) -> &mut Self {
        if coeff == 0 {
            return self;
        }
        let cell = cs.bitz::<LIMBS>(bit.clone());
        let c = coeff_u128::<CS>(coeff);
        if negative {
            self.sum -= cell * c;
        } else {
            self.sum += cell * c;
        }
        self
    }

    pub fn bit_pow2(&mut self, cs: &mut CS, bit: &CS::Bool, k: usize, negative: bool) -> &mut Self {
        let cell = cs.bitz::<LIMBS>(bit.clone());
        let c = coeff_pow2::<CS>(k);
        if negative {
            self.sum -= cell * c;
        } else {
            self.sum += cell * c;
        }
        self
    }

    pub fn constant(&mut self, value: u128, negative: bool) -> &mut Self {
        if value != 0 {
            let c = Z::<CS>::from(coeff_u128::<CS>(value));
            if negative {
                self.sum -= c;
            } else {
                self.sum += c;
            }
        }
        self
    }

    /// Asserts the sum is zero (a linear row).
    pub fn assert_zero(self, cs: &mut CS) {
        cs.assert_r1c::<LIMBS>(Z::<CS>::zero(), Z::<CS>::zero(), self.sum);
    }
}

/// Committed bits computed by `f` from the values of `inputs` (witness
/// backends; recorders never call it).
pub fn hint_bits<CS: Circuit>(
    cs: &mut CS,
    inputs: Vec<CS::Bool>,
    n: usize,
    f: impl Fn(&[bool]) -> Vec<bool> + Send + Sync + 'static,
) -> Vec<CS::Bool> {
    let mut out = Vec::with_capacity(n);
    let f = std::sync::Arc::new(f);
    // Exactly `n` committed bits: 64-bit hints, then the remainder in
    // power-of-two pieces.
    macro_rules! piece {
        ($width:literal, $offset:expr) => {{
            let (inputs, f, offset) = (inputs.clone(), f.clone(), $offset);
            let packed = cs.hint::<1, $width, 1, _>(move |ctx| {
                let values: Vec<bool> = inputs.iter().map(|b| ctx.eval_bool(b)).collect();
                let bits = f(&values);
                HintResult::Ok(PackedBits::from_fn(|i| bits.get(offset + i).copied().unwrap_or(false)))
            });
            for i in 0..$width {
                out.push(packed.bit(i));
            }
        }};
    }
    while n - out.len() >= 64 {
        piece!(64, out.len());
    }
    let rest = n - out.len();
    if rest & 32 != 0 {
        piece!(32, out.len());
    }
    if rest & 16 != 0 {
        piece!(16, out.len());
    }
    if rest & 8 != 0 {
        piece!(8, out.len());
    }
    if rest & 4 != 0 {
        piece!(4, out.len());
    }
    if rest & 2 != 0 {
        piece!(2, out.len());
    }
    if rest & 1 != 0 {
        piece!(1, out.len());
    }
    debug_assert_eq!(out.len(), n);
    out
}

/// `a_k ∧ b_k` for every pair: committed bits, one row per
/// [`ANDS_PER_ROW`] pairs.
pub fn and_pairs<CS: Circuit>(cs: &mut CS, pairs: &[(CS::Bool, CS::Bool)]) -> Vec<CS::Bool> {
    let mut out = Vec::with_capacity(pairs.len());
    for chunk in pairs.chunks(ANDS_PER_ROW) {
        let inputs: Vec<CS::Bool> = chunk.iter().flat_map(|(a, b)| [a.clone(), b.clone()]).collect();
        let n = chunk.len();
        let r = hint_bits(cs, inputs, n, move |v| (0..n).map(|k| v[2 * k] & v[2 * k + 1]).collect());
        let mut row = Row::<CS>::new();
        for (k, ((a, b), rk)) in chunk.iter().zip(&r).enumerate() {
            let x = cs.xor(a.clone(), b.clone());
            row.bit_pow2(cs, rk, k + 1, false)
                .bit_pow2(cs, a, k, true)
                .bit_pow2(cs, b, k, true)
                .bit_pow2(cs, &x, k, false);
        }
        row.assert_zero(cs);
        out.extend(r);
    }
    out
}

/// `a ∧ b_k` for one bit against a word.
pub fn and_bit_word<CS: Circuit>(cs: &mut CS, a: &CS::Bool, word: &[CS::Bool]) -> Vec<CS::Bool> {
    let pairs: Vec<_> = word.iter().map(|b| (a.clone(), b.clone())).collect();
    and_pairs(cs, &pairs)
}

/// Carry-less product of two `n`-coefficient polynomials (`n` a power of
/// two), Karatsuba down to single ANDs: `3^{log n}` committed bits.
pub fn clmul<CS: Circuit>(cs: &mut CS, a: &[CS::Bool], b: &[CS::Bool]) -> Vec<CS::Bool> {
    // Collect every leaf AND first so they batch into full rows.
    fn plan<B: BoolWitness, CS: Circuit<Bool = B>>(
        cs: &mut CS,
        a: &[B],
        b: &[B],
        leaves: &mut Vec<(B, B)>,
    ) -> Vec<Vec<usize>> {
        // Returns, per output coefficient, the leaf indices XORed into it.
        let n = a.len();
        if n == 1 {
            leaves.push((a[0].clone(), b[0].clone()));
            return vec![vec![leaves.len() - 1]];
        }
        let h = n / 2;
        let (a0, a1) = a.split_at(h);
        let (b0, b1) = b.split_at(h);
        let p0 = plan(cs, a0, b0, leaves);
        let p2 = plan(cs, a1, b1, leaves);
        let s: Vec<B> = (0..h).map(|i| cs.xor(a0[i].clone(), a1[i].clone())).collect();
        let t: Vec<B> = (0..h).map(|i| cs.xor(b0[i].clone(), b1[i].clone())).collect();
        let p1 = plan(cs, &s, &t, leaves);
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); 2 * n - 1];
        for (i, terms) in p0.iter().enumerate() {
            out[i].extend(terms);
            out[i + h].extend(terms);
        }
        for (i, terms) in p2.iter().enumerate() {
            out[i + 2 * h].extend(terms);
            out[i + h].extend(terms);
        }
        for (i, terms) in p1.iter().enumerate() {
            out[i + h].extend(terms);
        }
        out
    }
    assert_eq!(a.len(), b.len());
    assert!(a.len().is_power_of_two());
    let mut leaves = Vec::new();
    let supports = plan(cs, a, b, &mut leaves);
    let products = and_pairs(cs, &leaves);
    supports
        .into_iter()
        .map(|mut terms| {
            // XOR with multiplicity: a leaf listed twice cancels (ascending
            // leaves of odd multiplicity).
            terms.sort_unstable();
            let mut bits: Vec<CS::Bool> = Vec::with_capacity(terms.len());
            let mut i = 0;
            while i < terms.len() {
                let mut j = i + 1;
                while j < terms.len() && terms[j] == terms[i] {
                    j += 1;
                }
                if (j - i) % 2 == 1 {
                    bits.push(products[terms[i]].clone());
                }
                i = j;
            }
            cs.xor_many(bits)
        })
        .collect()
}

/// `a · b` in GF(2^128) for committed `a`, `b`: 2187 ANDs.
pub fn mul_bits<CS: Circuit>(cs: &mut CS, a: &Bits<CS::Bool>, b: &Bits<CS::Bool>) -> Bits<CS::Bool> {
    let product = clmul(cs, a, b);
    debug_assert_eq!(product.len(), 255);
    let mut out: Bits<CS::Bool> = array::from_fn(|i| product[i].clone());
    for j in 0..127 {
        let column = gf128::reduction_column(j);
        for (i, slot) in out.iter_mut().enumerate() {
            if (column >> i) & 1 == 1 {
                *slot = cs.xor(slot.clone(), product[128 + j].clone());
            }
        }
    }
    out
}

/// `a · b` for GF(2^128) values; free when either is public.
pub fn mul<CS: Circuit>(cs: &mut CS, a: &G<CS::Bool>, b: &G<CS::Bool>) -> G<CS::Bool> {
    match (a, b) {
        (G::P(x), G::P(y)) => G::P(*x * *y),
        (G::P(c), G::V(v)) | (G::V(v), G::P(c)) => {
            G::V(gf128::linear_map(cs, v, &gf128::mul_const_columns(u128_of(*c))))
        }
        (G::V(x), G::V(y)) => G::V(mul_bits(cs, x, y)),
    }
}

/// `base^η` for a public base and committed exponent bits (LSB first).
pub fn pow_public_base<CS: Circuit>(cs: &mut CS, base: Gf, exponent: &[CS::Bool]) -> G<CS::Bool> {
    let mut acc: G<CS::Bool> = G::P(Gf::ONE);
    let mut power = base;
    for bit in exponent {
        // acc' = acc · (1 + η·(power − 1)) = acc + η · acc·(power − 1).
        let delta = power + Gf::ONE;
        acc = match &acc {
            G::P(v) => {
                let d = u128_of(*v * delta);
                let bits = gf128::constant::<CS::Bool>(u128_of(*v));
                G::V(array::from_fn(|i| {
                    if (d >> i) & 1 == 1 {
                        cs.xor(bits[i].clone(), bit.clone())
                    } else {
                        bits[i].clone()
                    }
                }))
            }
            G::V(v) => {
                let scaled = gf128::linear_map(cs, v, &gf128::mul_const_columns(u128_of(delta)));
                let r = and_bit_word(cs, bit, &scaled);
                G::V(array::from_fn(|i| cs.xor(v[i].clone(), r[i].clone())))
            }
        };
        power = power * power;
    }
    acc
}

// ---------------------------------------------------------------------
// Integers mod the public prime q.
// ---------------------------------------------------------------------

/// A residue mod `q`: public, or committed little-endian bits of an integer
/// in that class.
#[derive(Clone)]
pub enum Zq<B: BoolWitness> {
    P(u128),
    V(Vec<B>),
}

pub fn mulmod(a: u128, b: u128, q: u128) -> u128 {
    use crypto_bigint::{NonZero, U128};
    let product = U128::from_u128(a).widening_mul(&U128::from_u128(b));
    u128::from(U128::rem_wide_vartime(product, &NonZero::<U128>::new_unwrap(U128::from_u128(q))))
}

/// `2·x mod q` for `x < q < 2^127`.
fn double_mod(x: u128, q: u128) -> u128 {
    let d = x << 1;
    if d >= q { d - q } else { d }
}

/// `2^k mod q` for `k < len` (`q < 2^127`).
fn pow2_table(len: usize, q: u128) -> Vec<u128> {
    let mut out = Vec::with_capacity(len);
    let mut v = 1u128 % q;
    for _ in 0..len {
        out.push(v);
        v = double_mod(v, q);
    }
    out
}

/// Commits `width` bits of `value`.
pub fn witness_int<CS: Circuit>(cs: &mut CS, value: u128, width: usize) -> Vec<CS::Bool> {
    assert!(width <= 128);
    let packed = cs.hint::<1, 128, 2, _>(move |_| {
        HintResult::Ok(PackedBits::from_words([value as u64, (value >> 64) as u64]))
    });
    (0..width).map(|i| packed.bit(i)).collect()
}

/// The integer value of committed bits (witness backends).
pub fn int_value<CS: Circuit>(cs: &CS, bits: &[CS::Bool]) -> Option<u128> {
    super::val::try_value(cs, bits)
}

/// Asserts `x ≤ bound` for committed bits: `bound − x` committed in
/// `bitlen(bound)` bits, one row.
pub fn assert_le<CS: Circuit>(cs: &mut CS, x: &[CS::Bool], bound: u128) {
    let width = 128 - bound.leading_zeros() as usize;
    let xs = x.to_vec();
    let d = hint_bits(cs, xs, width, move |v| {
        let x = v.iter().enumerate().fold(0u128, |a, (i, &b)| a | (u128::from(b) << i));
        let d = bound.wrapping_sub(x);
        (0..width).map(|i| (d >> i) & 1 == 1).collect()
    });
    let mut row = Row::<CS>::new();
    for (i, b) in x.iter().enumerate() {
        row.bit_pow2(cs, b, i, false);
    }
    for (i, b) in d.iter().enumerate() {
        row.bit_pow2(cs, b, i, false);
    }
    row.constant(bound, true);
    row.assert_zero(cs);
}

/// One term of a congruence: `coeff · value` (coefficient mod `q`).
pub struct ModTerm<'a, B: BoolWitness> {
    pub coeff: u128,
    pub value: &'a Zq<B>,
}

/// Asserts `Σ coeff_i · x_i + constant ≡ 0 (mod q)` with one row per chunk
/// of at most `2^12` committed bits (chunks reduce to committed residues
/// first). `q < 2^113`.
pub fn assert_mod_q<CS: Circuit>(
    cs: &mut CS,
    terms: &[ModTerm<'_, CS::Bool>],
    constant: u128,
    q: u128,
    public: &Public,
    what: &str,
) {
    assert!(q < (1u128 << 113) && q > 2);
    let mut constant = constant % q;
    // (bit, coefficient mod q) pairs of the committed terms.
    let mut bits: Vec<(CS::Bool, u128)> = Vec::new();
    for term in terms {
        let c = term.coeff % q;
        match term.value {
            Zq::P(v) => constant = (constant + mulmod(c, *v % q, q)) % q,
            Zq::V(xs) => {
                // c·2^k mod q, doubling per bit.
                let mut d = c;
                for b in xs {
                    if d != 0 {
                        bits.push((b.clone(), d));
                    }
                    d = double_mod(d, q);
                }
            }
        }
    }
    if bits.is_empty() {
        if constant != 0 {
            public.borrow_mut().fail(format!("public congruence failed: {what}"));
        }
        return;
    }
    const CHUNK: usize = 1 << 12;
    if bits.len() > CHUNK {
        // Reduce each chunk to a committed residue r_c ≡ Σ_chunk, then
        // assert Σ r_c + constant ≡ 0.
        let mut partials: Vec<Zq<CS::Bool>> = Vec::new();
        for chunk in bits.chunks(CHUNK) {
            let r = reduce_chunk(cs, chunk, q);
            partials.push(Zq::V(r));
        }
        let terms: Vec<ModTerm<'_, CS::Bool>> =
            partials.iter().map(|p| ModTerm { coeff: 1, value: p }).collect();
        assert_mod_q(cs, &terms, constant, q, public, what);
        return;
    }
    // Σ d_i b_i + constant = k·q with k < #terms + 1.
    let kbits = 128 - ((bits.len() as u128) + 1).leading_zeros() as usize;
    let inputs: Vec<CS::Bool> = bits.iter().map(|(b, _)| b.clone()).collect();
    let coeffs: Vec<u128> = bits.iter().map(|(_, d)| *d).collect();
    let k = hint_bits(cs, inputs, kbits, move |v| {
        let mut sum = constant;
        for (bit, d) in v.iter().zip(&coeffs) {
            if *bit {
                sum += *d;
            }
        }
        let k = sum / q;
        (0..kbits).map(|i| (k >> i) & 1 == 1).collect()
    });
    let mut row = Row::<CS>::new();
    for (b, d) in &bits {
        row.bit(cs, b, *d, false);
    }
    row.constant(constant, false);
    for (i, kb) in k.iter().enumerate() {
        row.bit(cs, kb, q << i, true);
    }
    row.assert_zero(cs);
}

/// A committed residue `r ≡ Σ d_i b_i (mod q)`, `r < 2^{bitlen q}`.
fn reduce_chunk<CS: Circuit>(cs: &mut CS, bits: &[(CS::Bool, u128)], q: u128) -> Vec<CS::Bool> {
    let width = 128 - q.leading_zeros() as usize;
    let kbits = 128 - ((bits.len() as u128) + 1).leading_zeros() as usize;
    let inputs: Vec<CS::Bool> = bits.iter().map(|(b, _)| b.clone()).collect();
    let coeffs: Vec<u128> = bits.iter().map(|(_, d)| *d).collect();
    let both = hint_bits(cs, inputs, width + kbits, move |v| {
        let mut sum = 0u128;
        for (bit, d) in v.iter().zip(&coeffs) {
            if *bit {
                sum += *d;
            }
        }
        let (r, k) = (sum % q, sum / q);
        (0..width)
            .map(|i| (r >> i) & 1 == 1)
            .chain((0..kbits).map(|i| (k >> i) & 1 == 1))
            .collect()
    });
    let (r, k) = both.split_at(width);
    let mut row = Row::<CS>::new();
    for (b, d) in bits {
        row.bit(cs, b, *d, false);
    }
    for (i, rb) in r.iter().enumerate() {
        row.bit_pow2(cs, rb, i, true);
    }
    for (i, kb) in k.iter().enumerate() {
        row.bit(cs, kb, q << i, true);
    }
    row.assert_zero(cs);
    r.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recursion::val::PublicStream;
    use circuit::compact::CompactRecorder;
    use circuit::witgen::CheckingWitgen;

    fn rand(state: &mut u64) -> u128 {
        let mut next = || {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            *state
        };
        (u128::from(next()) << 64) | u128::from(next())
    }

    fn eval_g(cs: &CheckingWitgen, g: &G<bool>) -> Gf {
        match g {
            G::P(v) => *v,
            G::V(bits) => gf_of(super::super::val::try_value(cs, bits).unwrap()),
        }
    }

    #[test]
    fn karatsuba_and_pow_match_native() {
        let mut state = 0x1234_5678_9abc_def0;
        for _ in 0..4 {
            let (x, y) = (gf_of(rand(&mut state)), gf_of(rand(&mut state)));
            let mut w = CheckingWitgen::new();
            let a = super::super::val::witness(&mut w, x);
            let b = super::super::val::witness(&mut w, y);
            let p = mul(&mut w, &a, &b);
            assert_eq!(w.first_failure, None);
            assert_eq!(eval_g(&w, &p), x * y);
        }
        let base = gf_of(rand(&mut state));
        for e in [0u128, 1, 2, 5, 0xdead_beef, rand(&mut state) >> 2] {
            let mut w = CheckingWitgen::new();
            let bits = witness_int(&mut w, e, 126);
            let g = pow_public_base(&mut w, base, &bits);
            assert_eq!(w.first_failure, None);
            let mut expected = Gf::ONE;
            let mut p = base;
            for i in 0..126 {
                if (e >> i) & 1 == 1 {
                    expected = expected * p;
                }
                p = p * p;
            }
            assert_eq!(eval_g(&w, &g), expected, "exponent {e:#x}");
        }
        // A wrong AND result is caught by its row.
        let mut rec = CompactRecorder::new();
        let a = gf128::witness(&mut rec, 0);
        let b = gf128::witness(&mut rec, 0);
        mul_bits(&mut rec, &a, &b);
        let st = rec.finish_structured();
        assert_eq!(st.committed, 256 + 2187);
    }

    #[test]
    fn congruences_and_range_checks() {
        let q = (1u128 << 100) - 15;
        let mut state = 99;
        for count in [1usize, 3, 40, 70] {
            let coeffs: Vec<u128> = (0..count).map(|_| rand(&mut state) % q).collect();
            let xs: Vec<u128> = (0..count).map(|_| rand(&mut state) >> 1).collect();
            let mut target = 0u128;
            for (c, x) in coeffs.iter().zip(&xs) {
                target = (target + mulmod(*c, *x % q, q)) % q;
            }
            for (delta, ok) in [(0u128, true), (1, false)] {
                let mut w = CheckingWitgen::new();
                let public = PublicStream::discover();
                let vals: Vec<Zq<bool>> = xs.iter().map(|&x| Zq::V(witness_int(&mut w, x, 127))).collect();
                let t = Zq::V(witness_int(&mut w, (target + delta) % q, 100));
                let mut terms: Vec<ModTerm<'_, bool>> =
                    coeffs.iter().zip(&vals).map(|(&c, v)| ModTerm { coeff: c, value: v }).collect();
                terms.push(ModTerm { coeff: q - 1, value: &t });
                assert_mod_q(&mut w, &terms, 0, q, &public, "test");
                assert_eq!(w.first_failure.is_none(), ok, "count {count} delta {delta}");
            }
        }
        for (x, bound, ok) in [(5u128, 5u128, true), (6, 5, false), (0, 1 << 120, true), ((1 << 120) + 1, 1 << 120, false)] {
            let mut w = CheckingWitgen::new();
            let bits = witness_int(&mut w, x, 125);
            assert_le(&mut w, &bits, bound);
            assert_eq!(w.first_failure.is_none(), ok, "{x} <= {bound}");
        }
    }
}

/// A committed residue `r ≡ a·b (mod q)` for committed `a`, `b`: the
/// schoolbook bit products as ANDs (`|a|·|b|` committed bits) and one
/// congruence with the coefficients `2^{i+j} mod q`.
pub fn mul_mod_q<CS: Circuit>(
    cs: &mut CS,
    a: &Zq<CS::Bool>,
    b: &Zq<CS::Bool>,
    q: u128,
    public: &Public,
) -> Zq<CS::Bool> {
    let (Zq::V(x), Zq::V(y)) = (a, b) else {
        // A public factor is a public coefficient: the product is linear.
        let (c, v) = match (a, b) {
            (Zq::P(c), v) | (v, Zq::P(c)) => (*c, v),
            _ => unreachable!(),
        };
        return match v {
            Zq::P(w) => Zq::P(mulmod(c % q, *w % q, q)),
            Zq::V(_) => {
                let value = residue_value(cs, &[(c % q, v)], 0, q);
                let width = 128 - q.leading_zeros() as usize;
                let r = witness_int(cs, value.unwrap_or(0), width);
                assert_le(cs, &r, q - 1);
                let rz = Zq::V(r);
                let terms = [ModTerm { coeff: c % q, value: v }, ModTerm { coeff: q - 1, value: &rz }];
                assert_mod_q(cs, &terms, 0, q, public, "public-coefficient product");
                rz
            }
        };
    };
    let pairs: Vec<(CS::Bool, CS::Bool)> = x
        .iter()
        .flat_map(|xi| y.iter().map(move |yj| (xi.clone(), yj.clone())))
        .collect();
    let ands = and_pairs(cs, &pairs);
    let value = match (int_value(cs, x), int_value(cs, y)) {
        (Some(u), Some(v)) => Some(mulmod(u % q, v % q, q)),
        _ => None,
    };
    let width = 128 - q.leading_zeros() as usize;
    let r = witness_int(cs, value.unwrap_or(0), width);
    assert_le(cs, &r, q - 1);
    // Σ_{i,j} (2^{i+j} mod q)·(x_i ∧ y_j) − r ≡ 0.
    let terms: Vec<Zq<CS::Bool>> = ands.iter().map(|bit| Zq::V(vec![bit.clone()])).collect();
    let mut mod_terms: Vec<ModTerm<'_, CS::Bool>> = Vec::with_capacity(terms.len() + 1);
    let pow2 = pow2_table(x.len() + y.len(), q);
    let mut k = 0;
    for i in 0..x.len() {
        for j in 0..y.len() {
            mod_terms.push(ModTerm { coeff: pow2[i + j], value: &terms[k] });
            k += 1;
        }
    }
    let rz = Zq::V(r);
    mod_terms.push(ModTerm { coeff: q - 1, value: &rz });
    assert_mod_q(cs, &mod_terms, 0, q, public, "modular product");
    rz
}

/// The value of `constant + Σ c·x mod q` (witness backends).
fn residue_value<CS: Circuit>(cs: &CS, terms: &[(u128, &Zq<CS::Bool>)], constant: u128, q: u128) -> Option<u128> {
    let mut acc = constant % q;
    for (c, x) in terms {
        let v = match x {
            Zq::P(v) => *v,
            Zq::V(bits) => int_value(cs, bits)?,
        };
        acc = (acc + mulmod(*c % q, v % q, q)) % q;
    }
    Some(acc)
}
