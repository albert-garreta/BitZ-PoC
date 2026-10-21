//! GF(2^256) = GF(2^128)[Y] / (Y² + Y + C) in the verifier circuit
//! ([`flock_core::field::Gf256`]): committed pairs of GF(2^128) elements and
//! their linear combinations with public extension coefficients. The
//! extension Ligerito verifier draws every challenge publicly, so each
//! product it forms has a public factor and stays linear:
//! `(c₀ + c₁Y)(x_a + x_bY) = (c₀x_a + C·c₁x_b) + (c₁x_a + (c₀ + c₁)x_b)·Y`.

use circuit::Circuit;
use flock_core::field::Gf256 as E;

use super::bytes::{Byte, gf_bytes};
use super::val::{G, Lc, Public, witness};

/// `a + b·Y`, each coordinate committed or public.
#[derive(Clone)]
pub struct E2<B: circuit::BoolWitness> {
    pub a: G<B>,
    pub b: G<B>,
}

impl<B: circuit::BoolWitness> E2<B> {
    /// The transcript's encoding: `a` then `b`, 16 little-endian bytes each.
    pub fn bytes(&self) -> Vec<Byte<B>> {
        let mut out = gf_bytes(&self.a);
        out.extend(gf_bytes(&self.b));
        out
    }
}

/// A committed extension element.
pub fn witness_ext<CS: Circuit>(cs: &mut CS, value: E) -> E2<CS::Bool> {
    E2 { a: witness(cs, value.a), b: witness(cs, value.b) }
}

/// `A + B·Y` over [`Lc`] coordinates.
#[derive(Clone)]
pub struct Lc2<B: circuit::BoolWitness> {
    pub a: Lc<B>,
    pub b: Lc<B>,
}

impl<B: circuit::BoolWitness> Default for Lc2<B> {
    fn default() -> Self {
        Self { a: Lc::new(), b: Lc::new() }
    }
}

impl<B: circuit::BoolWitness> Lc2<B> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn of(x: &E2<B>) -> Self {
        Self { a: Lc::of(&x.a), b: Lc::of(&x.b) }
    }

    /// The base-field combination `x` embedded as `x + 0·Y`.
    pub fn of_base(x: &Lc<B>) -> Self {
        Self { a: x.clone(), b: Lc::new() }
    }

    pub fn add_const(&mut self, c: E) -> &mut Self {
        self.a.add_const(c.a);
        self.b.add_const(c.b);
        self
    }

    /// `+= c·x` for a public `c`.
    pub fn add(&mut self, c: E, x: &E2<B>) -> &mut Self {
        self.a.add(c.a, &x.a).add(E::C * c.b, &x.b);
        self.b.add(c.b, &x.a).add(c.a + c.b, &x.b);
        self
    }

    /// `+= c·x` for a public `c` and a base-field `x`.
    pub fn add_base(&mut self, c: E, x: &G<B>) -> &mut Self {
        self.a.add(c.a, x);
        self.b.add(c.b, x);
        self
    }

    pub fn add_lc2(&mut self, other: &Self) -> &mut Self {
        self.a.add_lc(&other.a);
        self.b.add_lc(&other.b);
        self
    }

    /// `+= c·other` for a public `c`.
    pub fn add_scaled(&mut self, c: E, other: &Self) -> &mut Self {
        self.a.add_scaled(c.a, &other.a).add_scaled(E::C * c.b, &other.b);
        self.b.add_scaled(c.b, &other.a).add_scaled(c.a + c.b, &other.b);
        self
    }

    pub fn settle<CS: Circuit<Bool = B>>(&self, cs: &mut CS) -> E2<B> {
        E2 { a: self.a.settle(cs), b: self.b.settle(cs) }
    }

    pub fn assert_zero<CS: Circuit<Bool = B>>(&self, cs: &mut CS, public: &Public, what: &str) {
        self.a.assert_zero(cs, public, what);
        self.b.assert_zero(cs, public, what);
    }
}

/// `RoundQuadExt`: `q(X) = c + b·X + a·X²` from a message `(u₀, u₂)` and the
/// running claim `t`, with `b = t + u₂` (so `q(0) + q(1) = t`).
pub struct Quad2<B: circuit::BoolWitness> {
    c: Lc2<B>,
    b: Lc2<B>,
    a: Lc2<B>,
}

impl<B: circuit::BoolWitness> Quad2<B> {
    /// The zero polynomial (a placeholder before the first message).
    pub fn zero() -> Self {
        Self { c: Lc2::new(), b: Lc2::new(), a: Lc2::new() }
    }

    pub fn from_msg(u0: &E2<B>, u2: &E2<B>, t_r: &Lc2<B>) -> Self {
        let mut b = t_r.clone();
        b.add(E::ONE, u2);
        Self { c: Lc2::of(u0), b, a: Lc2::of(u2) }
    }

    pub fn eval(&self, r: E) -> Lc2<B> {
        let mut out = self.c.clone();
        out.add_scaled(r, &self.b);
        out.add_scaled(r * r, &self.a);
        out
    }

    pub fn fold(&self, rhs: &Self, alpha: E) -> Self {
        let f = |x: &Lc2<B>, y: &Lc2<B>| {
            let mut out = x.clone();
            out.add_scaled(alpha, y);
            out
        };
        Self { c: f(&self.c, &rhs.c), b: f(&self.b, &rhs.b), a: f(&self.a, &rhs.a) }
    }
}

/// `enforced_sum_ext` over base-field rows (level 0):
/// `Σ_i eq(α, i)·Σ_j eq(v, j)·row_i[j]`.
pub fn enforced_sum_base<B: circuit::BoolWitness>(rows: &[Vec<G<B>>], v: &[E], alpha: &[E]) -> Lc2<B> {
    let eq = flock_core::pcs::ligerito::ext::build_eq_table_ext(v);
    let weights = alpha_weights(alpha, rows.len());
    let mut sum = Lc2::new();
    for (row, &w) in rows.iter().zip(&weights) {
        for (x, &e) in row.iter().zip(&eq) {
            sum.add_base(w * e, x);
        }
    }
    sum
}

/// `enforced_sum_ext` over extension rows (the recursive levels).
pub fn enforced_sum_ext<B: circuit::BoolWitness>(rows: &[Vec<E2<B>>], v: &[E], alpha: &[E]) -> Lc2<B> {
    let eq = flock_core::pcs::ligerito::ext::build_eq_table_ext(v);
    let weights = alpha_weights(alpha, rows.len());
    let mut sum = Lc2::new();
    for (row, &w) in rows.iter().zip(&weights) {
        for (x, &e) in row.iter().zip(&eq) {
            sum.add(w * e, x);
        }
    }
    sum
}

/// `alpha_weights`: `eq(α, i)` for the first `n` indices.
fn alpha_weights(alpha: &[E], n: usize) -> Vec<E> {
    if n == 0 {
        return Vec::new();
    }
    flock_core::pcs::ligerito::ext::build_eq_table_ext(alpha).into_iter().take(n).collect()
}
