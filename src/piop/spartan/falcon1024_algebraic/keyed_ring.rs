//! Experimental keyed integer ring check; built only for tests and benchmarks.
//!
//! The ring relation `s1 + h*s2 = t` in `Z_q[X]/(X^N+1)` holds iff `q` divides
//! every coefficient of the integer residual
//! `R_i = s1_i + (h_i*s2_i mod (X^N+1)) - t_i`. Independent uniform
//! `A_i in F_q[Y]` with `D` coefficients batch the live instances: if some `R_i`
//! is nonzero modulo `q`, so is `sum_i A_i R_i`, except with probability `q^-D`
//! (`F_q[Y]` has no zero divisors). The prover sends the exact integer quotient
//! `K = sum_i A_i (R_i / q)`, so `sum_i A_i R_i - q K` is an integer polynomial of
//! degree below `N + D - 1` that is zero iff the batch relation holds. It is
//! projected at random points of `F_p`: one point when the prime is large enough,
//! two independent points merged by a fresh coefficient otherwise.
//!
//! At a point `xi`, the weight of `s2[i][j]` is
//! `A_i(xi) * (h_i(X) X^j mod (X^N+1))(xi)`. The prover fills it with a
//! two-term recurrence in `j`. The verifier never materializes it: the
//! dependence on `j` is shared by all instances, so the multilinear extension
//! costs one inner product per public key (see `word_binding`).

use field::{BatchMulAcc, FpSignedLinearAcc, Reduce, RingOps};
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        SpartanField,
        grinding::{GrindingRound, grind_and_absorb, verify_and_absorb},
        squeeze_field,
    },
    transcript::traits::Transcript,
};

use super::ring::PreparedClaim;
use super::{Cfg, F, FalconAlgebraicStatement, FalconError, Layout, N, Q, WitnessData};

/// Coefficients of each instance's batching polynomial `A_i`.
pub(super) const BATCHING_COEFFICIENTS: usize = 11;
/// Coefficients of `K`, whose degree is below `N + D - 1`.
pub(super) const QUOTIENT_LEN: usize = N + BATCHING_COEFFICIENTS - 1;
/// Degree of the projected integer polynomial in each point.
pub(super) const PROJECTION_DEGREE: usize = QUOTIENT_LEN - 1;
const COEFFICIENT_ABS_BOUND: u128 = 1 << 14;
const DOMAIN: &[u8] = b"bitz/falcon-algebraic/keyed-integer-ring/v1";

crate::piop::spartan::grinding::grinding_domains! {
    PointGrinding => b"bitz/falcon-algebraic/keyed-integer-ring/point-grinding/v1";
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Proof {
    pub quotient: Vec<i64>,
    pub point_nonce: Option<u64>,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        8 * self.quotient.len() + 8 * usize::from(self.point_nonce.is_some())
    }
}

/// Succinct ring weights for the verifier. For each point `xi_p`, `rows[p][i]`
/// is `c_p * A_i(xi_p)`; the weight of `s1[i][j]` is `rows[p][i] * xi_p^j` and
/// that of `s2[i][j]` is `rows[p][i] * (h_i(X) X^j mod (X^N+1))(xi_p)`.
pub(super) struct VerifierClaim {
    pub points: Vec<F>,
    pub rows: Vec<Vec<F>>,
    pub target: F,
}

/// One point suffices when `PROJECTION_DEGREE / p` needs little grinding;
/// smaller primes use two independent points and a merge coefficient.
pub(super) const fn points_for_domain(domain_bits: u32) -> usize {
    if domain_bits >= 120 { 1 } else { 2 }
}

/// Error numerator of the single grinding site before the points are drawn.
/// One point errs with probability at most `PROJECTION_DEGREE / p`. With two
/// points, both vanish with probability below `(PROJECTION_DEGREE / p)^2 < 1/p`
/// and the merge coefficient errs with probability `1/p`.
pub(super) const fn point_numerator(points: usize) -> usize {
    if points == 1 { PROJECTION_DEGREE } else { 2 }
}

/// Batching error `q^-D` of the instance polynomials.
pub(super) fn batching_error() -> f64 {
    (Q as f64).powi(-(BATCHING_COEFFICIENTS as i32))
}

/// Bound on every coefficient of `R_i = s1 + h*s2 - t` for signed15 witnesses.
fn residual_bound() -> u128 {
    COEFFICIENT_ABS_BOUND * (1 + N as u128 * (Q as u128 - 1)) + (Q as u128 - 1)
}

/// Bound on every coefficient of `K = sum_i A_i (R_i / q)`.
pub(super) fn quotient_bound(batch: usize) -> u128 {
    batch as u128
        * BATCHING_COEFFICIENTS as u128
        * (Q as u128 - 1)
        * (residual_bound() / Q as u128)
}

/// Bound on the coefficients of `sum_i A_i R_i - q K` for every in-range witness
/// and accepted quotient; `p > 2 * bound` keeps a nonzero difference nonzero.
pub(super) fn no_wrap_bound(batch: usize) -> u128 {
    batch as u128 * BATCHING_COEFFICIENTS as u128 * (Q as u128 - 1) * residual_bound()
        + Q as u128 * quotient_bound(batch)
}

#[tracing::instrument(skip_all, name = "falcon_keyed_ring:prove")]
pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    data: &WitnessData,
    field: &Cfg,
    grinding_bits: u32,
    points: usize,
) -> Result<(Proof, PreparedClaim), FalconError> {
    validate_context(layout, public, field, grinding_bits, points)?;
    if data.witness.s1.len() != layout.batch() || data.witness.s2.len() != layout.batch() {
        return Err(error("keyed ring witness dimensions"));
    }
    bind(transcript, layout, field, grinding_bits, points);
    let polynomials = instance_polynomials(transcript, layout.batch());
    let quotient = {
        let _span = tracing::info_span!("falcon_keyed_ring:quotient").entered();
        calculate_quotient(public, data, &polynomials)?
    };
    check_quotient(&quotient, layout.batch())?;
    absorb_quotient(transcript, &quotient);
    let point_nonce = if grinding_bits == 0 {
        None
    } else {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<PointGrinding>::new(0),
                grinding_bits,
            )
            .map_err(|e| error(e.to_string()))?,
        )
    };
    let (points, coefficients) = sample_points(transcript, field, points)?;
    let _span = tracing::info_span!("falcon_keyed_ring:weights").entered();
    let (rows, target) =
        rows_and_target(public, &polynomials, &quotient, &points, &coefficients, field);
    let (weights_s1, weights_s2) = weights(public, &rows, &points, field);
    Ok((
        Proof {
            quotient,
            point_nonce,
        },
        PreparedClaim {
            weights_s1,
            weights_s2,
            target,
        },
    ))
}

#[tracing::instrument(skip_all, name = "falcon_keyed_ring:verify")]
pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    proof: &Proof,
    field: &Cfg,
    grinding_bits: u32,
    points: usize,
) -> Result<VerifierClaim, FalconError> {
    validate_context(layout, public, field, grinding_bits, points)?;
    if proof.point_nonce.is_some() != (grinding_bits != 0) || proof.quotient.len() != QUOTIENT_LEN
    {
        return Err(error("invalid keyed ring proof encoding"));
    }
    check_quotient(&proof.quotient, layout.batch())?;
    bind(transcript, layout, field, grinding_bits, points);
    let polynomials = instance_polynomials(transcript, layout.batch());
    absorb_quotient(transcript, &proof.quotient);
    if let Some(nonce) = proof.point_nonce {
        verify_and_absorb(
            transcript,
            GrindingRound::<PointGrinding>::new(0),
            grinding_bits,
            nonce,
        )
        .map_err(|e| error(e.to_string()))?;
    }
    let (points, coefficients) = sample_points(transcript, field, points)?;
    let (rows, target) = rows_and_target(
        public,
        &polynomials,
        &proof.quotient,
        &points,
        &coefficients,
        field,
    );
    Ok(VerifierClaim {
        points,
        rows,
        target,
    })
}

fn validate_context(
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    field: &Cfg,
    grinding_bits: u32,
    points: usize,
) -> Result<(), FalconError> {
    // The statement transcript has already rejected noncanonical coefficients.
    if public.public_keys.len() != layout.batch() || public.targets.len() != layout.batch() {
        return Err(error("keyed ring public input dimensions"));
    }
    if field.modulus_u128() / 2 <= no_wrap_bound(layout.batch()) {
        return Err(error("keyed ring integer identity exceeds the prime"));
    }
    if grinding_bits > 256 || !(1..=2).contains(&points) {
        return Err(error("invalid keyed ring configuration"));
    }
    Ok(())
}

fn bind(
    transcript: &mut impl Transcript,
    layout: &Layout,
    field: &Cfg,
    grinding_bits: u32,
    points: usize,
) {
    transcript.absorb_slice(DOMAIN);
    for value in [
        N as u64,
        Q as u64,
        BATCHING_COEFFICIENTS as u64,
        layout.batch() as u64,
        layout.capacity() as u64,
        points as u64,
    ] {
        transcript.absorb_slice(&value.to_le_bytes());
    }
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    transcript.absorb_slice(&grinding_bits.to_le_bytes());
    transcript.absorb_slice(&quotient_bound(layout.batch()).to_le_bytes());
}

/// Uniform `A_i` from one seed: chunks of instances read disjoint keyed XOF
/// streams, so prover and verifier expand them in parallel and identically.
fn instance_polynomials(
    transcript: &mut impl Transcript,
    batch: usize,
) -> Vec<[u16; BATCHING_COEFFICIENTS]> {
    const CHUNK: usize = 256;
    transcript.absorb_slice(b"instance-polynomials");
    let mut seed = [0u8; 32];
    transcript.begin_sampling();
    transcript.fill_sampling_bytes(&mut seed);
    let q = Q as u16;
    let mut polynomials = vec![[0u16; BATCHING_COEFFICIENTS]; batch];
    polynomials
        .par_chunks_mut(CHUNK)
        .enumerate()
        .for_each(|(chunk, polynomials)| {
            let mut hasher = blake3::Hasher::new_keyed(&seed);
            hasher.update(&(chunk as u64).to_le_bytes());
            let mut stream = hasher.finalize_xof();
            let mut buffer = [0u8; 256];
            let mut position = buffer.len();
            for coefficient in polynomials.iter_mut().flatten() {
                loop {
                    if position == buffer.len() {
                        stream.fill(&mut buffer);
                        position = 0;
                    }
                    let candidate = u16::from_le_bytes([buffer[position], buffer[position + 1]]);
                    position += 2;
                    // 5q < 2^16: rejection keeps every residue equally likely.
                    if candidate < 5 * q {
                        *coefficient = candidate % q;
                        break;
                    }
                }
            }
        });
    polynomials
}

/// Exact `K = sum_i A_i k_i` with `k_i = R_i / q`. Each `k_i` is recovered from
/// `h_i*s2_i` modulo a 31-bit NTT prime: `|k_i| < 2^25` is far below half of it.
fn calculate_quotient(
    public: &FalconAlgebraicStatement,
    data: &WitnessData,
    polynomials: &[[u16; BATCHING_COEFFICIENTS]],
) -> Result<Vec<i64>, FalconError> {
    let tables = exact::Tables::get();
    (0..polynomials.len())
        .into_par_iter()
        .try_fold(
            || (vec![0i64; QUOTIENT_LEN], exact::Workspace::default()),
            |(mut sum, mut workspace), i| {
                let k = workspace.quotient(
                    tables,
                    &public.public_keys[i],
                    &data.witness.s1[i],
                    &data.witness.s2[i],
                    &public.targets[i],
                )?;
                for (a, &coefficient) in polynomials[i].iter().enumerate() {
                    let coefficient = i64::from(coefficient);
                    for (total, &value) in sum[a..a + N].iter_mut().zip(k.iter()) {
                        *total += coefficient * i64::from(value);
                    }
                }
                Ok((sum, workspace))
            },
        )
        .map(|state: Result<_, FalconError>| state.map(|(sum, _)| sum))
        .try_reduce(
            || vec![0i64; QUOTIENT_LEN],
            |mut left, right| {
                for (left, right) in left.iter_mut().zip(right) {
                    *left += right;
                }
                Ok(left)
            },
        )
}

fn check_quotient(quotient: &[i64], batch: usize) -> Result<(), FalconError> {
    let bound = quotient_bound(batch);
    if quotient.len() != QUOTIENT_LEN
        || quotient
            .iter()
            .any(|value| u128::from(value.unsigned_abs()) > bound)
    {
        return Err(error("keyed ring quotient is out of bounds"));
    }
    Ok(())
}

fn absorb_quotient(transcript: &mut impl Transcript, quotient: &[i64]) {
    transcript.absorb_slice(b"integer-batch-quotient");
    let mut bytes = Vec::with_capacity(8 * quotient.len());
    for value in quotient {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

fn sample_points(
    transcript: &mut impl Transcript,
    field: &Cfg,
    points: usize,
) -> Result<(Vec<F>, Vec<F>), FalconError> {
    transcript.absorb_slice(b"projection-points");
    let xs = (0..points)
        .map(|_| squeeze_field(transcript, field))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| error(e.to_string()))?;
    let mut coefficients = vec![field.one()];
    for _ in 1..points {
        transcript.absorb_slice(b"projection-merge");
        coefficients.push(squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?);
    }
    Ok((xs, coefficients))
}

fn powers(x: &F, len: usize, field: &Cfg) -> Vec<F> {
    let mut powers = Vec::with_capacity(len);
    let mut value = field.one();
    for _ in 0..len {
        powers.push(value);
        value = field.mul(&value, x);
    }
    powers
}

fn small(value: u64, field: &Cfg) -> F {
    F::from_with_cfg(u128::from(value), field)
}

/// A shared prime-field vector as four 32-bit limbs of its canonical integers.
/// A dot product with 16-bit public coefficients is then four widening u64
/// multiply-accumulates per term and one reduction: every limb sum stays below
/// `N * 2^48 < 2^58`.
pub(super) struct LimbVector {
    limbs: [Vec<u32>; 4],
}

impl LimbVector {
    pub(super) fn new(values: &[F], field: &Cfg) -> Self {
        let mut limbs: [Vec<u32>; 4] = std::array::from_fn(|_| Vec::with_capacity(values.len()));
        for value in values {
            let integer = u128::from(field.to_integer(value));
            for (index, limb) in limbs.iter_mut().enumerate() {
                limb.push((integer >> (32 * index)) as u32);
            }
        }
        Self { limbs }
    }

    /// `sum_k x[k] values[k]` over the shorter of the two lengths, at most `N`.
    pub(super) fn dot(&self, x: &[u16], field: &Cfg) -> F {
        debug_assert!(x.len().min(self.limbs[0].len()) <= N);
        let sums = self.limbs.each_ref().map(|limb| {
            limb.iter()
                .zip(x)
                .map(|(&limb, &x)| u64::from(limb) * u64::from(x))
                .sum::<u64>()
        });
        let low = u128::from(sums[0]) + (u128::from(sums[1]) << 32);
        let high = u128::from(sums[2]) + (u128::from(sums[3]) << 32);
        let rest = (low >> 64) + high;
        field.from_unsigned_words(&[low as u64, rest as u64, (rest >> 64) as u64])
    }
}

/// `rows[p][i] = c_p A_i(xi_p)` and the claimed value
/// `sum_p c_p (q K(xi_p) + sum_i A_i(xi_p) t_i(xi_p))`.
fn rows_and_target(
    public: &FalconAlgebraicStatement,
    polynomials: &[[u16; BATCHING_COEFFICIENTS]],
    quotient: &[i64],
    points: &[F],
    coefficients: &[F],
    field: &Cfg,
) -> (Vec<Vec<F>>, F) {
    let mut rows = Vec::with_capacity(points.len());
    let mut target = field.zero();
    for (x, c) in points.iter().zip(coefficients) {
        let powers = powers(x, QUOTIENT_LEN, field);
        let mut sum = FpSignedLinearAcc::<2, 1>::default();
        for (power, &value) in powers.iter().zip(quotient) {
            field.mul_acc(&mut sum, power, &value);
        }
        let quotient_at = field.mul(&field.reduce(sum), &small(Q as u64, field));
        let limbs = LimbVector::new(&powers[..N], field);
        let (row, targets): (Vec<F>, Vec<F>) = polynomials
            .par_iter()
            .zip(&public.targets)
            .map(|(polynomial, t)| {
                let row = field.mul(c, &limbs.dot(polynomial, field));
                (row, field.mul(&row, &limbs.dot(t, field)))
            })
            .unzip();
        let targets = targets
            .into_par_iter()
            .reduce(|| field.zero(), |a, b| field.add(&a, &b));
        target = field.add(&target, &field.add(&field.mul(c, &quotient_at), &targets));
        rows.push(row);
    }
    (rows, target)
}

/// Dense prover weights. `s1`: `sum_p rows[p][i] xi_p^j`. `s2`: the recurrence
/// `V_{j+1} = xi V_j - rows[p][i] (xi^N + 1) h_i[N-1-j]` from
/// `V_0 = rows[p][i] h_i(xi)`, since multiplying by `X` modulo `X^N+1` moves the
/// top coefficient to the constant term with a sign change.
fn weights(
    public: &FalconAlgebraicStatement,
    rows: &[Vec<F>],
    points: &[F],
    field: &Cfg,
) -> (Vec<F>, Vec<F>) {
    let batch = public.public_keys.len();
    let powers: Vec<Vec<F>> = points.iter().map(|x| powers(x, N + 1, field)).collect();
    let wraps: Vec<F> = powers
        .iter()
        .map(|powers| field.add(&powers[N], &field.one()))
        .collect();
    let residues: Vec<F> = (0..Q as u64).map(|value| small(value, field)).collect();
    let limbs: Vec<LimbVector> = powers
        .iter()
        .map(|powers| LimbVector::new(&powers[..N], field))
        .collect();
    let mut weights_s1 = vec![field.zero(); batch * N];
    let mut weights_s2 = vec![field.zero(); batch * N];
    weights_s1
        .par_chunks_mut(N)
        .zip(weights_s2.par_chunks_mut(N))
        .enumerate()
        .for_each(|(i, (s1, s2))| {
            let h = &public.public_keys[i];
            for (p, powers) in powers.iter().enumerate() {
                let row = rows[p][i];
                let first = p == 0;
                for (weight, power) in s1.iter_mut().zip(&powers[..N]) {
                    let term = field.mul(&row, power);
                    *weight = if first { term } else { field.add(weight, &term) };
                }
                let mut value = field.mul(&row, &limbs[p].dot(h, field));
                let wrap = field.mul(&row, &wraps[p]);
                let x = powers[1];
                for j in 0..N {
                    s2[j] = if first { value } else { field.add(&s2[j], &value) };
                    if j + 1 < N {
                        let carry = field.mul(&wrap, &residues[usize::from(h[N - 1 - j])]);
                        value = field.sub(&field.mul(&x, &value), &carry);
                    }
                }
            }
        });
    (weights_s1, weights_s2)
}

/// Shared coefficient factors for the verifier at the binder's terminal
/// coefficient weights `e_s1`, `e_s2` (the multilinear eq weights of the
/// coefficient index for each side): per point, `sum_j e_s1[j] xi^j` and the
/// vector `G[k] = sum_j e_s2[j] sigma(j,k) xi^((j+k) mod N)`, so the s2 value of
/// instance `i` is `<h_i, G>`. `G` obeys `G[k+1] = xi G[k] - e_s2[N-1-k](xi^N+1)`.
pub(super) fn coefficient_factors(
    points: &[F],
    e_s1: &[F],
    e_s2: &[F],
    field: &Cfg,
) -> Vec<(F, LimbVector)> {
    points
        .iter()
        .map(|x| {
            let powers = powers(x, N + 1, field);
            let wrap = field.add(&powers[N], &field.one());
            let dot = |weights: &[F]| {
                weights
                    .iter()
                    .zip(&powers)
                    .fold(field.zero(), |sum, (w, p)| field.add(&sum, &field.mul(w, p)))
            };
            let s1 = dot(e_s1);
            let mut g = Vec::with_capacity(N);
            let mut value = dot(e_s2);
            for k in 0..N {
                g.push(value);
                if k + 1 < N {
                    value = field.sub(
                        &field.mul(x, &value),
                        &field.mul(&e_s2[N - 1 - k], &wrap),
                    );
                }
            }
            (s1, LimbVector::new(&g, field))
        })
        .collect()
}

/// `sum_p rows[p][i] (s1_p + <h_i, G_p>)` for one live instance.
pub(super) fn instance_value(
    rows: &[Vec<F>],
    factors: &[(F, LimbVector)],
    key: &[u16; N],
    instance: usize,
    field: &Cfg,
) -> F {
    let mut value = field.zero();
    for (row, (s1, g)) in rows.iter().zip(factors) {
        let local = field.add(s1, &g.dot(key, field));
        value = field.add(&value, &field.mul(&row[instance], &local));
    }
    value
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

/// Negacyclic products modulo the 31-bit NTT prime `15 * 2^27 + 1`, in
/// Montgomery form with `R = 2^32`, enough to recover the small quotients
/// `(s1 + h*s2 - t) / q` exactly.
mod exact {
    use super::{FalconError, N, Q};
    use std::sync::OnceLock;

    pub(super) const P: u32 = 0x7800_0001;
    const GENERATOR: u64 = 31;

    const fn negative_inverse() -> u32 {
        let mut inverse: u32 = 1;
        let mut i = 0;
        while i < 5 {
            inverse = inverse.wrapping_mul(2u32.wrapping_sub(P.wrapping_mul(inverse)));
            i += 1;
        }
        inverse.wrapping_neg()
    }
    const NEGATIVE_INVERSE: u32 = negative_inverse();

    #[inline(always)]
    fn mul(a: u32, b: u32) -> u32 {
        let t = u64::from(a) * u64::from(b);
        let m = (t as u32).wrapping_mul(NEGATIVE_INVERSE);
        let u = ((t + u64::from(m) * u64::from(P)) >> 32) as u32;
        if u >= P { u - P } else { u }
    }

    #[inline(always)]
    fn add(a: u32, b: u32) -> u32 {
        let sum = a + b;
        if sum >= P { sum - P } else { sum }
    }

    #[inline(always)]
    fn sub(a: u32, b: u32) -> u32 {
        if a >= b { a - b } else { a + P - b }
    }

    fn pow(base: u64, mut exponent: u64) -> u64 {
        let modulus = u64::from(P);
        let (mut result, mut base) = (1u64, base % modulus);
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = result * base % modulus;
            }
            base = base * base % modulus;
            exponent >>= 1;
        }
        result
    }

    pub(super) struct Tables {
        /// Montgomery forms of psi^bitrev(k) for a primitive 2N-th root psi.
        zetas: Vec<u32>,
        /// N^-1 R^2: undoes the pointwise R^-1 and the inverse transform's N.
        scale: u32,
        /// q^-1 modulo P, as an ordinary residue.
        q_inverse: u64,
    }

    impl Tables {
        pub(super) fn get() -> &'static Self {
            static TABLES: OnceLock<Tables> = OnceLock::new();
            TABLES.get_or_init(Self::new)
        }

        fn new() -> Self {
            let modulus = u64::from(P);
            let r = (1u64 << 32) % modulus;
            let r2 = ((1u128 << 64) % u128::from(P)) as u64;
            let psi = pow(GENERATOR, (modulus - 1) / (2 * N as u64));
            assert_eq!(pow(psi, N as u64), modulus - 1, "psi has order 2N");
            let bits = N.ilog2();
            // Montgomery forms x R, so mul(zeta, a) = psi^e a in ordinary form.
            let zetas = (0..N)
                .map(|k| {
                    let reversed = (k as u64).reverse_bits() >> (64 - bits);
                    (pow(psi, reversed) * r % modulus) as u32
                })
                .collect();
            let n_inverse = pow(N as u64, modulus - 2);
            Self {
                zetas,
                scale: (n_inverse * r2 % modulus) as u32,
                q_inverse: pow(Q as u64, modulus - 2),
            }
        }

        fn forward(&self, a: &mut [u32; N]) {
            let mut k = 0;
            let mut len = N / 2;
            while len > 0 {
                let mut start = 0;
                while start < N {
                    k += 1;
                    let zeta = self.zetas[k];
                    for j in start..start + len {
                        let t = mul(zeta, a[j + len]);
                        a[j + len] = sub(a[j], t);
                        a[j] = add(a[j], t);
                    }
                    start += 2 * len;
                }
                len /= 2;
            }
        }

        fn inverse(&self, a: &mut [u32; N]) {
            let mut k = N;
            let mut len = 1;
            while len < N {
                let mut start = 0;
                while start < N {
                    k -= 1;
                    let zeta = P - self.zetas[k];
                    for j in start..start + len {
                        let t = a[j];
                        a[j] = add(t, a[j + len]);
                        a[j + len] = mul(zeta, sub(t, a[j + len]));
                    }
                    start += 2 * len;
                }
                len *= 2;
            }
            for value in a.iter_mut() {
                *value = mul(self.scale, *value);
            }
        }

        /// `h * s` in `Z_P[X]/(X^N+1)`, with canonical output.
        pub(super) fn product(&self, h: &[u16; N], s: &[i16; N], out: &mut [u32; N]) {
            let mut right = [0u32; N];
            for ((left, right), (&h, &s)) in out.iter_mut().zip(&mut right).zip(h.iter().zip(s)) {
                *left = u32::from(h);
                *right = if s < 0 { P - (-i32::from(s)) as u32 } else { s as u32 };
            }
            self.forward(out);
            self.forward(&mut right);
            for (left, right) in out.iter_mut().zip(&right) {
                *left = mul(*left, *right);
            }
            self.inverse(out);
        }
    }

    pub(super) struct Workspace {
        product: Box<[u32; N]>,
        quotient: Box<[i32; N]>,
    }

    impl Default for Workspace {
        fn default() -> Self {
            Self {
                product: Box::new([0; N]),
                quotient: Box::new([0; N]),
            }
        }
    }

    impl Workspace {
        /// `(s1 + h*s2 - t) / q` coefficientwise, exactly. Rejects a witness
        /// whose residual is not divisible by q (the quotient would not be small).
        pub(super) fn quotient(
            &mut self,
            tables: &Tables,
            h: &[u16; N],
            s1: &[i16; N],
            s2: &[i16; N],
            t: &[u16; N],
        ) -> Result<&[i32; N], FalconError> {
            tables.product(h, s2, &mut self.product);
            let modulus = i64::from(P);
            let half = modulus / 2;
            let limit = (super::residual_bound() / Q as u128) as i64;
            for ((quotient, &product), (&s1, &t)) in self
                .quotient
                .iter_mut()
                .zip(self.product.iter())
                .zip(s1.iter().zip(t))
            {
                let residual =
                    (i64::from(s1) + i64::from(product) - i64::from(t)).rem_euclid(modulus);
                let value = (residual as u64 * tables.q_inverse % P as u64) as i64;
                let value = if value > half { value - modulus } else { value };
                if value.abs() > limit {
                    return Err(super::error("keyed ring residual is not divisible by q"));
                }
                *quotient = value as i32;
            }
            Ok(&*self.quotient)
        }
    }

    #[cfg(test)]
    pub(super) fn naive_product(h: &[u16; N], s: &[i16; N]) -> Vec<i64> {
        let mut out = vec![0i64; N];
        for (i, &h) in h.iter().enumerate() {
            for (j, &s) in s.iter().enumerate() {
                let term = i64::from(h) * i64::from(s);
                if i + j < N {
                    out[i + j] += term;
                } else {
                    out[i + j - N] -= term;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use field::Uint;

    fn field() -> Cfg {
        F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
    }

    fn pseudo(seed: u64, i: usize) -> u64 {
        let mut x = seed ^ (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        x ^= x >> 31;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^ (x >> 29)
    }

    #[test]
    fn exact_products_match_schoolbook_negacyclic_multiplication() {
        let tables = exact::Tables::get();
        for seed in 0..4u64 {
            let h: [u16; N] = std::array::from_fn(|i| (pseudo(seed, i) % Q as u64) as u16);
            let s: [i16; N] = std::array::from_fn(|i| {
                ((pseudo(seed + 9, i) % (1 << 15)) as i64 - (1 << 14)) as i16
            });
            let mut out = [0u32; N];
            tables.product(&h, &s, &mut out);
            for (got, want) in out.iter().zip(exact::naive_product(&h, &s)) {
                assert_eq!(i64::from(*got), want.rem_euclid(i64::from(exact::P)));
            }
        }
    }

    #[test]
    fn coefficient_factors_match_dense_weights() {
        let f = field();
        let batch = 3;
        let public = FalconAlgebraicStatement {
            public_keys: (0..batch)
                .map(|b| std::array::from_fn(|i| (pseudo(b as u64, i) % Q as u64) as u16))
                .collect(),
            targets: vec![[0; N]; batch],
        };
        let points = vec![F::from_with_cfg(97u128, &f), F::from_with_cfg(1234567u128, &f)];
        let rows: Vec<Vec<F>> = (0..2)
            .map(|p| {
                (0..batch)
                    .map(|i| F::from_with_cfg((11 + 7 * p + 3 * i) as u128, &f))
                    .collect()
            })
            .collect();
        let (s1, s2) = weights(&public, &rows, &points, &f);
        let e_s1: Vec<F> = (0..N)
            .map(|j| F::from_with_cfg(pseudo(5, j) as u128, &f))
            .collect();
        let e_s2: Vec<F> = (0..N)
            .map(|j| F::from_with_cfg(pseudo(6, j) as u128, &f))
            .collect();
        let factors = coefficient_factors(&points, &e_s1, &e_s2, &f);
        for i in 0..batch {
            let mut dense = f.zero();
            for j in 0..N {
                dense = f.add(&dense, &f.mul(&s1[i * N + j], &e_s1[j]));
                dense = f.add(&dense, &f.mul(&s2[i * N + j], &e_s2[j]));
            }
            let succinct = instance_value(&rows, &factors, &public.public_keys[i], i, &f);
            assert_eq!(dense, succinct);
        }
    }

    #[test]
    fn limb_dot_products_match_field_arithmetic() {
        let f = field();
        let values: Vec<F> = (0..N)
            .map(|i| f.neg(&F::from_with_cfg(u128::from(pseudo(11, i)) << 61 | 12345, &f)))
            .collect();
        let limbs = LimbVector::new(&values, &f);
        for seed in 0..3 {
            let x: Vec<u16> = (0..N).map(|i| pseudo(seed, i) as u16).collect();
            let want = values.iter().zip(&x).fold(f.zero(), |sum, (v, &x)| {
                f.add(&sum, &f.mul(v, &small(u64::from(x), &f)))
            });
            assert_eq!(limbs.dot(&x, &f), want);
        }
        assert_eq!(limbs.dot(&[u16::MAX; 3], &f), {
            let three = values[..3].iter().fold(f.zero(), |sum, v| f.add(&sum, v));
            f.mul(&three, &small(u64::from(u16::MAX), &f))
        });
    }

    #[test]
    fn recurrence_matches_negacyclic_definition() {
        let f = field();
        let public = FalconAlgebraicStatement {
            public_keys: vec![std::array::from_fn(|i| (pseudo(3, i) % Q as u64) as u16)],
            targets: vec![[0; N]],
        };
        let x = F::from_with_cfg(31337u128, &f);
        let rows = vec![vec![f.one()]];
        let (_, s2) = weights(&public, &rows, std::slice::from_ref(&x), &f);
        let powers = powers(&x, N, &f);
        for j in [0, 1, 7, N / 2, N - 1] {
            // (h X^j mod X^N+1)(x) from its coefficients.
            let mut want = f.zero();
            for (k, &h) in public.public_keys[0].iter().enumerate() {
                let term = f.mul(&small(u64::from(h), &f), &powers[(j + k) % N]);
                want = if j + k < N { f.add(&want, &term) } else { f.sub(&want, &term) };
            }
            assert_eq!(s2[j], want);
        }
    }
}
