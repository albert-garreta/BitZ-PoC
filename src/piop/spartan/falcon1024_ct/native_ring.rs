//! Native Falcon ideal check and bounded coordinate lift into the existing
//! arithmetic binder. The returned claim is not authenticated until the caller
//! binds its weights to the committed `c` and decoded `s1` source columns.

use field::RingOps;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        SpartanBitzField, SpartanField,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        squeeze_field,
    },
    transcript::traits::Transcript,
};

use super::{
    FalconError, FalconPublicStatement, FalconSourceLayout, FalconVerificationTrace, N, Q,
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

pub const EXTENSION_DEGREE: usize = 11;
pub const CARRY_GRINDING_BITS: u32 = 12;
const DOMAIN: &[u8] = b"bitz/falcon1024-ct/native-ring/v1";

/// Canonical coordinates in F_12289[theta]/(theta^11 + theta + 14).
/// Proof input is validated before arithmetic; coordinates are deliberately
/// explicit so noncanonical proof encodings can be rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ext(pub [u16; EXTENSION_DEGREE]);

impl Ext {
    const ZERO: Self = Self([0; EXTENSION_DEGREE]);
    const ONE: Self = Self([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    fn canonical(self) -> bool {
        self.0.iter().all(|&x| i64::from(x) < Q)
    }

    fn is_base_field(self) -> bool {
        self.0[1..].iter().all(|&x| x == 0)
    }

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| {
            let sum = self.0[i] + rhs.0[i];
            if sum >= Q as u16 { sum - Q as u16 } else { sum }
        }))
    }

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(std::array::from_fn(|i| {
            let value = self.0[i] + Q as u16 - rhs.0[i];
            if value >= Q as u16 {
                value - Q as u16
            } else {
                value
            }
        }))
    }

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let mut product = [0i64; 2 * EXTENSION_DEGREE - 1];
        for (i, &a) in self.0.iter().enumerate() {
            for (j, &b) in rhs.0.iter().enumerate() {
                product[i + j] += i64::from(a) * i64::from(b);
            }
        }
        // theta^(11+k) = -theta^(k+1) - 14 theta^k, k <= 9.
        for j in EXTENSION_DEGREE..product.len() {
            product[j - EXTENSION_DEGREE] -= 14 * product[j];
            product[j - EXTENSION_DEGREE + 1] -= product[j];
        }
        Self(std::array::from_fn(|j| product[j].rem_euclid(Q) as u16))
    }

    #[cfg(test)]
    fn pow(mut self, mut exponent: u64) -> Self {
        let mut result = Self::ONE;
        while exponent != 0 {
            if exponent & 1 != 0 {
                result = result.mul(self);
            }
            self = self.mul(self);
            exponent >>= 1;
        }
        result
    }
}

/// The quotient polynomial is fixed before `alpha`; carries are fixed before
/// the independent prime-field coordinate-batching challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRingProof {
    pub certificate: Vec<Ext>,
    pub instance_point: Vec<Ext>,
    pub alpha: Ext,
    pub carries: [i64; EXTENSION_DEGREE],
    pub carry_nonce: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedNativeClaim {
    /// Live signature-major coefficients of `c - s1`; no padding entries.
    pub weights: Vec<F>,
    pub target: F,
}

struct CarryGrinding;
impl GrindingDomain for CarryGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/grinding/native-carry/v1";
}

#[tracing::instrument(skip_all, name = "falcon_native:prove")]
pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    field: &Cfg,
    target_bits: u32,
) -> Result<(NativeRingProof, PreparedNativeClaim), FalconError> {
    validate_context(layout, statement, field, target_bits)?;
    if traces.len() != layout.batch()
        || traces.iter().enumerate().any(|(s, trace)| {
            trace.public_key != statement.public_keys[s]
                || trace.signature != statement.signatures[s]
        })
    {
        return Err(error("native ring trace/public statement mismatch"));
    }
    bind_parameters(transcript, layout, field, target_bits);
    let instance_point = sample_instance_point(transcript, layout)?;
    let lambda = equality_weights(&instance_point);
    let certificate = certificate(traces, &lambda);
    absorb_extensions(transcript, &certificate);
    let alpha = sample_generator(transcript)?;
    let powers = powers(alpha);
    let t = projection_target(statement, &lambda, &powers, &certificate);
    let coordinates = projection_coordinates(&lambda[..layout.batch()], &powers);
    let carries = calculate_carries(&coordinates, t, traces)?;
    check_carries(&carries, layout.batch())?;
    absorb_carries(transcript, &carries);
    let carry_nonce = if target_bits == 128 {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<CarryGrinding>::new(0),
                CARRY_GRINDING_BITS,
            )
            .map_err(|e| error(e.to_string()))?,
        )
    } else {
        None
    };
    let xi = squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?;
    let claim = collapse_claim(&coordinates, t, &carries, xi, field);
    Ok((
        NativeRingProof {
            certificate,
            instance_point,
            alpha,
            carries,
            carry_nonce,
        },
        claim,
    ))
}

#[tracing::instrument(skip_all, name = "falcon_native:verify")]
pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    proof: &NativeRingProof,
    field: &Cfg,
    target_bits: u32,
) -> Result<PreparedNativeClaim, FalconError> {
    validate_context(layout, statement, field, target_bits)?;
    if proof.certificate.len() != N - 1
        || !proof.certificate.iter().all(|x| x.canonical())
        || proof.instance_point.len() != layout.capacity().trailing_zeros() as usize
        || !proof.instance_point.iter().all(|x| x.canonical())
        || !proof.alpha.canonical()
    {
        return Err(error("invalid native ring proof encoding"));
    }
    check_carries(&proof.carries, layout.batch())?;
    bind_parameters(transcript, layout, field, target_bits);
    let instance_point = sample_instance_point(transcript, layout)?;
    if instance_point != proof.instance_point {
        return Err(error("native ring instance challenge mismatch"));
    }
    let lambda = equality_weights(&instance_point);
    absorb_extensions(transcript, &proof.certificate);
    let alpha = sample_generator(transcript)?;
    if alpha != proof.alpha {
        return Err(error("native ring projection challenge mismatch"));
    }
    let powers = powers(alpha);
    let t = projection_target(statement, &lambda, &powers, &proof.certificate);
    absorb_carries(transcript, &proof.carries);
    match (target_bits, proof.carry_nonce) {
        (128, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<CarryGrinding>::new(0),
            CARRY_GRINDING_BITS,
            nonce,
        )
        .map_err(|e| error(e.to_string()))?,
        (100, None) => {}
        _ => return Err(error("native carry grinding nonce shape mismatch")),
    }
    let xi = squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?;
    let coordinates = projection_coordinates(&lambda[..layout.batch()], &powers);
    Ok(collapse_claim(&coordinates, t, &proof.carries, xi, field))
}

fn validate_context(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    field: &Cfg,
    target_bits: u32,
) -> Result<(), FalconError> {
    statement.validate(layout.batch())?;
    if !layout.is_hybrid() || !matches!(target_bits, 100 | 128) {
        return Err(error("unsupported native ring layout or security target"));
    }
    if field.modulus_u128() <= no_wrap_bound(layout.batch()) {
        return Err(error("native coordinate lift exceeds the arithmetic prime"));
    }
    Ok(())
}

fn bind_parameters(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    field: &Cfg,
    target_bits: u32,
) {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(Q as u64).to_le_bytes());
    // All coefficients of theta^11 + theta + 14, in ascending order.
    let polynomial: [u16; 12] = [14, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
    for coefficient in polynomial {
        transcript.absorb_slice(&coefficient.to_le_bytes());
    }
    transcript.absorb_slice(&(N as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    transcript.absorb_slice(&target_bits.to_le_bytes());
    transcript.absorb_slice(&CARRY_GRINDING_BITS.to_le_bytes());
    transcript.absorb_slice(&carry_bound(layout.batch()).to_le_bytes());
}

fn sample_instance_point(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
) -> Result<Vec<Ext>, FalconError> {
    (0..layout.capacity().trailing_zeros())
        .map(|_| sample_extension(transcript, false))
        .collect()
}

fn sample_generator(transcript: &mut impl Transcript) -> Result<Ext, FalconError> {
    // Degree 11 is prime, so the base field is its only proper subfield.
    sample_extension(transcript, true)
}

fn sample_extension(transcript: &mut impl Transcript, generator: bool) -> Result<Ext, FalconError> {
    transcript.begin_sampling();
    for _ in 0..128 {
        let mut coordinates = [0u16; EXTENSION_DEGREE];
        for coordinate in &mut coordinates {
            let mut accepted = false;
            for _ in 0..128 {
                let mut bytes = [0u8; 2];
                transcript.fill_sampling_bytes(&mut bytes);
                let candidate = u16::from_le_bytes(bytes);
                if candidate < 5 * Q as u16 {
                    *coordinate = candidate % Q as u16;
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                return Err(error("native field sampling exhausted"));
            }
        }
        let result = Ext(coordinates);
        if !generator || !result.is_base_field() {
            absorb_extensions(transcript, &[result]);
            return Ok(result);
        }
    }
    Err(error("native generator sampling exhausted"))
}

fn absorb_extensions(transcript: &mut impl Transcript, values: &[Ext]) {
    let mut bytes = Vec::with_capacity(8 + values.len() * EXTENSION_DEGREE * 2);
    bytes.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for value in values {
        for coordinate in value.0 {
            bytes.extend_from_slice(&coordinate.to_le_bytes());
        }
    }
    transcript.absorb_slice(&bytes);
}

fn absorb_carries(transcript: &mut impl Transcript, carries: &[i64; EXTENSION_DEGREE]) {
    let mut bytes = [0u8; EXTENSION_DEGREE * 8];
    for (chunk, value) in bytes.chunks_exact_mut(8).zip(carries) {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

fn equality_weights(point: &[Ext]) -> Vec<Ext> {
    let mut weights = vec![Ext::ONE];
    for &r in point.iter().rev() {
        let mut next = Vec::with_capacity(2 * weights.len());
        for weight in weights {
            next.push(weight.mul(Ext::ONE.sub(r)));
            next.push(weight.mul(r));
        }
        weights = next;
    }
    weights
}

#[tracing::instrument(skip_all, name = "falcon_native:certificate")]
fn certificate(traces: &[FalconVerificationTrace], lambda: &[Ext]) -> Vec<Ext> {
    let coefficient = |j: usize| {
        let mut sum = [0u64; EXTENSION_DEGREE];
        for (trace, weight) in traces.iter().zip(lambda) {
            let scalar = u64::from(trace.native_quotient[j]);
            for (total, &coordinate) in sum.iter_mut().zip(&weight.0) {
                *total += scalar * u64::from(coordinate);
            }
        }
        Ext(sum.map(|x| (x % Q as u64) as u16))
    };
    #[cfg(feature = "parallel")]
    if traces.len() >= 8 {
        return (0..N - 1).into_par_iter().map(coefficient).collect();
    }
    (0..N - 1).map(coefficient).collect()
}

fn powers(alpha: Ext) -> Vec<Ext> {
    let mut powers = Vec::with_capacity(N + 1);
    powers.push(Ext::ONE);
    for j in 1..=N {
        powers.push(powers[j - 1].mul(alpha));
    }
    powers
}

#[tracing::instrument(skip_all, name = "falcon_native:projection_public")]
fn projection_target(
    statement: &FalconPublicStatement,
    lambda: &[Ext],
    powers: &[Ext],
    certificate: &[Ext],
) -> Ext {
    let mut d_at_alpha = Ext::ZERO;
    for (&coefficient, &power) in certificate.iter().zip(powers) {
        d_at_alpha = d_at_alpha.add(coefficient.mul(power));
    }
    let public_product = |s: usize| {
        let mut h = [0i64; EXTENSION_DEGREE];
        let mut s2 = [0i64; EXTENSION_DEGREE];
        for j in 0..N {
            for k in 0..EXTENSION_DEGREE {
                h[k] += i64::from(statement.public_keys[s].h[j]) * i64::from(powers[j].0[k]);
                s2[k] += i64::from(statement.signatures[s].s2[j]) * i64::from(powers[j].0[k]);
            }
        }
        let h = Ext(h.map(|x| x.rem_euclid(Q) as u16));
        let s2 = Ext(s2.map(|x| x.rem_euclid(Q) as u16));
        lambda[s].mul(h.mul(s2))
    };
    #[cfg(feature = "parallel")]
    let public_sum = if statement.batch() >= 8 {
        (0..statement.batch())
            .into_par_iter()
            .map(public_product)
            .reduce(|| Ext::ZERO, Ext::add)
    } else {
        (0..statement.batch())
            .map(public_product)
            .fold(Ext::ZERO, Ext::add)
    };
    #[cfg(not(feature = "parallel"))]
    let public_sum = (0..statement.batch())
        .map(public_product)
        .fold(Ext::ZERO, Ext::add);
    powers[N].add(Ext::ONE).mul(d_at_alpha).add(public_sum)
}

#[tracing::instrument(skip_all, name = "falcon_native:coordinates")]
fn projection_coordinates(lambda: &[Ext], powers: &[Ext]) -> Vec<Ext> {
    let mut coordinates = vec![Ext::ZERO; lambda.len() * N];
    let fill = |(chunk, &weight): (&mut [Ext], &Ext)| {
        for (coordinate, &power) in chunk.iter_mut().zip(powers) {
            *coordinate = weight.mul(power);
        }
    };
    #[cfg(feature = "parallel")]
    if lambda.len() >= 8 {
        coordinates
            .par_chunks_mut(N)
            .zip(lambda.par_iter())
            .for_each(fill);
        return coordinates;
    }
    coordinates.chunks_mut(N).zip(lambda).for_each(fill);
    coordinates
}

#[tracing::instrument(skip_all, name = "falcon_native:carries")]
fn calculate_carries(
    coordinates: &[Ext],
    target: Ext,
    traces: &[FalconVerificationTrace],
) -> Result<[i64; EXTENSION_DEGREE], FalconError> {
    let mut totals = [0i64; EXTENSION_DEGREE];
    for (chunk, trace) in coordinates.chunks_exact(N).zip(traces) {
        for (j, coordinate) in chunk.iter().enumerate() {
            let delta = i64::from(trace.hash_to_point.point[j]) - i64::from(trace.s1[j]);
            for (total, &a) in totals.iter_mut().zip(&coordinate.0) {
                *total += delta * i64::from(a);
            }
        }
    }
    let mut carries = [0i64; EXTENSION_DEGREE];
    for k in 0..EXTENSION_DEGREE {
        let difference = totals[k] - i64::from(target.0[k]);
        if difference % Q != 0 {
            return Err(error(
                "native ideal projection has no integer coordinate lift",
            ));
        }
        carries[k] = difference / Q;
    }
    Ok(carries)
}

fn carry_bound(batch: usize) -> u64 {
    22528 * batch as u64 * N as u64
}

fn no_wrap_bound(batch: usize) -> u128 {
    (2 * Q as u128 - 1) * u128::from(carry_bound(batch)) + Q as u128 - 1
}

fn check_carries(carries: &[i64; EXTENSION_DEGREE], batch: usize) -> Result<(), FalconError> {
    if carries
        .iter()
        .any(|h| h.unsigned_abs() > carry_bound(batch))
    {
        return Err(error("native coordinate carry is out of bounds"));
    }
    Ok(())
}

#[tracing::instrument(skip_all, name = "falcon_native:collapse")]
fn collapse_claim(
    coordinates: &[Ext],
    target: Ext,
    carries: &[i64; EXTENSION_DEGREE],
    xi: F,
    field: &Cfg,
) -> PreparedNativeClaim {
    let mut xi_powers = [field.one(); EXTENSION_DEGREE];
    for k in 1..EXTENSION_DEGREE {
        xi_powers[k] = field.mul(&xi_powers[k - 1], &xi);
    }
    // Canonical coordinates have only fourteen bits. Window tables replace
    // eleven prime-field conversions and multiplications per word with small
    // table lookups and additions; the tables occupy about 53 KiB in total.
    const LOW_VALUES: usize = 256;
    const HIGH_VALUES: usize = (Q as usize - 1) / LOW_VALUES + 1;
    const WINDOW_VALUES: usize = LOW_VALUES + HIGH_VALUES;
    let mut multiples = vec![field.zero(); EXTENSION_DEGREE * WINDOW_VALUES];
    for (row, &power) in multiples.chunks_exact_mut(WINDOW_VALUES).zip(&xi_powers) {
        for low in 1..LOW_VALUES {
            row[low] = field.add(&row[low - 1], &power);
        }
        let high_step = field.add(&row[LOW_VALUES - 1], &power);
        for high in 1..HIGH_VALUES {
            row[LOW_VALUES + high] = field.add(&row[LOW_VALUES + high - 1], &high_step);
        }
    }
    let weight = |a: &Ext| {
        a.0.iter().zip(multiples.chunks_exact(WINDOW_VALUES)).fold(
            field.zero(),
            |sum, (&coordinate, row)| {
                let term = field.add(
                    &row[usize::from(coordinate) & 255],
                    &row[LOW_VALUES + (usize::from(coordinate) >> 8)],
                );
                field.add(&sum, &term)
            },
        )
    };
    #[cfg(feature = "parallel")]
    let weights = coordinates.par_iter().map(weight).collect();
    #[cfg(not(feature = "parallel"))]
    let weights = coordinates.iter().map(weight).collect();
    let target = (0..EXTENSION_DEGREE).fold(field.zero(), |sum, k| {
        let lifted = i128::from(target.0[k]) + i128::from(Q) * i128::from(carries[k]);
        let magnitude = F::from_with_cfg(lifted.unsigned_abs(), field);
        let value = if lifted < 0 {
            field.sub(&field.zero(), &magnitude)
        } else {
            magnitude
        };
        field.add(&sum, &field.mul(&xi_powers[k], &value))
    });
    PreparedNativeClaim { weights, target }
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    fn base_inverse(a: u16) -> u16 {
        let mut exponent = Q as u64 - 2;
        let mut x = u64::from(a);
        let mut result = 1;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = result * x % Q as u64;
            }
            x = x * x % Q as u64;
            exponent >>= 1;
        }
        result as u16
    }

    fn polynomial_gcd(mut a: Vec<u16>, mut b: Vec<u16>) -> Vec<u16> {
        let trim = |v: &mut Vec<u16>| {
            while v.last() == Some(&0) {
                v.pop();
            }
        };
        trim(&mut a);
        trim(&mut b);
        while !b.is_empty() {
            let inverse = i64::from(base_inverse(*b.last().unwrap()));
            while a.len() >= b.len() {
                let shift = a.len() - b.len();
                let scale = i64::from(*a.last().unwrap()) * inverse % Q;
                for j in 0..b.len() {
                    a[j + shift] =
                        (i64::from(a[j + shift]) - scale * i64::from(b[j])).rem_euclid(Q) as u16;
                }
                trim(&mut a);
            }
            (a, b) = (b, a);
        }
        a
    }

    #[test]
    fn extension_polynomial_passes_rabin_irreducibility_test() {
        let theta = Ext([0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let theta_q = theta.pow(Q as u64);
        let gcd = polynomial_gcd(
            vec![14, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            theta_q.sub(theta).0.to_vec(),
        );
        assert_eq!(gcd.len(), 1, "gcd(f, X^q-X) must be one");
        let mut frobenius = theta;
        for _ in 0..EXTENSION_DEGREE {
            frobenius = frobenius.pow(Q as u64);
        }
        assert_eq!(frobenius, theta, "f must divide X^(q^11)-X");
        assert_ne!(theta_q, theta);
    }

    #[test]
    fn extension_algebra_and_equality_weights() {
        let mut state = 9u64;
        let mut random = || {
            Ext(std::array::from_fn(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state % Q as u64) as u16
            }))
        };
        for _ in 0..100 {
            let (a, b, c) = (random(), random(), random());
            assert_eq!(a.mul(b.add(c)), a.mul(b).add(a.mul(c)));
            assert_eq!(a.mul(b).mul(c), a.mul(b.mul(c)));
            assert_eq!(a.add(b).sub(b), a);
            assert_eq!(a.mul(Ext::ONE), a);
            assert!(a.mul(b).canonical());
        }
        let point = [random(), random(), random()];
        let weights = equality_weights(&point);
        for (s, &weight) in weights.iter().enumerate() {
            let expected = point.iter().enumerate().fold(Ext::ONE, |acc, (k, &r)| {
                acc.mul(if s >> k & 1 == 1 { r } else { Ext::ONE.sub(r) })
            });
            assert_eq!(weight, expected);
        }
        assert_eq!(weights.into_iter().fold(Ext::ZERO, Ext::add), Ext::ONE);
    }

    #[test]
    fn coordinate_window_collapse_matches_direct_field_expression() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let xi = F::from_with_cfg((1u128 << 120) + 97, &field);
        let coordinates: Vec<_> = (0..Q as u16)
            .map(|x| {
                Ext(std::array::from_fn(|k| {
                    ((u32::from(x) * (k as u32 + 1)) % Q as u32) as u16
                }))
            })
            .collect();
        let claim = collapse_claim(&coordinates, Ext::ZERO, &[0; 11], xi, &field);
        for (a, actual) in coordinates.iter().zip(claim.weights) {
            let expected = a.0.iter().rev().fold(field.zero(), |sum, &coordinate| {
                field.add(
                    &field.mul(&sum, &xi),
                    &F::from_with_cfg(u128::from(coordinate), &field),
                )
            });
            assert_eq!(actual, expected);
        }
        assert_eq!(claim.target, field.zero());
    }

    #[cfg(feature = "falcon-hybrid")]
    fn fixture(
        batch: usize,
    ) -> (
        FalconSourceLayout,
        FalconPublicStatement,
        Vec<FalconVerificationTrace>,
    ) {
        let trace = super::super::verification_trace(
            include_bytes!("fixtures/public_key.bin"),
            include_bytes!("fixtures/message.bin"),
            include_bytes!("fixtures/signature_ct.bin"),
        )
        .unwrap();
        let statement = FalconPublicStatement {
            public_keys: vec![trace.public_key.clone(); batch],
            signatures: vec![trace.signature.clone(); batch],
            messages: vec![*include_bytes!("fixtures/message.bin"); batch],
        };
        (
            FalconSourceLayout::new_hybrid(batch).unwrap(),
            statement,
            vec![trace; batch],
        )
    }

    #[cfg(feature = "falcon-hybrid")]
    fn authenticates(
        claim: &PreparedNativeClaim,
        traces: &[FalconVerificationTrace],
        field: &Cfg,
    ) -> bool {
        let actual = claim.weights.chunks_exact(N).zip(traces).fold(
            field.zero(),
            |sum, (weights, trace)| {
                weights.iter().enumerate().fold(sum, |sum, (j, weight)| {
                    let delta = i128::from(trace.hash_to_point.point[j]) - i128::from(trace.s1[j]);
                    let magnitude = F::from_with_cfg(delta.unsigned_abs(), field);
                    let value = if delta < 0 {
                        field.sub(&field.zero(), &magnitude)
                    } else {
                        magnitude
                    };
                    field.add(&sum, &field.mul(weight, &value))
                })
            },
        );
        actual == claim.target
    }

    #[test]
    #[cfg(feature = "falcon-hybrid")]
    fn native_projection_carries_roundtrip_and_authenticate_padded_batches() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        for batch in [1, 3, 32] {
            let (layout, statement, traces) = fixture(batch);
            for target in [100, 128] {
                let mut prover = Blake3Transcript::new();
                let (proof, claim) =
                    prove(&mut prover, &layout, &statement, &traces, &field, target).unwrap();
                let mut verifier = Blake3Transcript::new();
                let verified =
                    verify(&mut verifier, &layout, &statement, &proof, &field, target).unwrap();
                assert_eq!(claim, verified);
                assert_eq!(
                    prover.get_challenge::<u128>(),
                    verifier.get_challenge::<u128>()
                );
                assert!(authenticates(&claim, &traces, &field));
                assert_eq!(proof.certificate.len(), N - 1);
                assert_eq!(proof.carry_nonce.is_some(), target == 128);
                assert!(!proof.alpha.is_base_field());
            }
        }
    }

    #[test]
    #[cfg(feature = "falcon-hybrid")]
    fn native_projection_rejects_tampering_and_noncanonical_coordinates() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let (layout, statement, traces) = fixture(3);
        let (proof, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &statement,
            &traces,
            &field,
            100,
        )
        .unwrap();
        let mut bad = proof.clone();
        bad.certificate[0].0[0] = (bad.certificate[0].0[0] + 1) % Q as u16;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &bad,
                &field,
                100
            )
            .is_err()
        );
        bad = proof.clone();
        bad.certificate[0].0[0] = Q as u16;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &bad,
                &field,
                100
            )
            .is_err()
        );
        bad = proof.clone();
        bad.certificate.push(Ext::ZERO);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &bad,
                &field,
                100
            )
            .is_err()
        );
        bad = proof.clone();
        bad.instance_point[0] = Ext::ZERO;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &bad,
                &field,
                100
            )
            .is_err()
        );
        bad = proof.clone();
        bad.carries[0] = i64::MIN;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &bad,
                &field,
                100
            )
            .is_err()
        );
        bad = proof.clone();
        bad.carries[0] += 1;
        let bad_claim = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &statement,
            &bad,
            &field,
            100,
        )
        .unwrap();
        assert!(!authenticates(&bad_claim, &traces, &field));
        bad = proof;
        bad.carry_nonce = Some(0);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &bad,
                &field,
                100
            )
            .is_err()
        );
    }

    #[test]
    #[cfg(feature = "falcon-hybrid")]
    fn invalid_native_relation_has_no_coordinate_lift() {
        let field = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        let (layout, statement, mut traces) = fixture(1);
        traces[0].s1[0] += 1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &traces,
                &field,
                100
            )
            .is_err()
        );
        let too_small = field::FpCtx::from_prime_u128(65537);
        assert!(validate_context(&layout, &statement, &too_small, 100).is_err());
    }

    #[test]
    fn bounded_carries_prevent_prime_modular_wrap() {
        // In a too-small proof field, h = q^-1 makes 0 = -1 + q*h
        // hold modulo p without holding over Z. Our no-wrap check excludes it.
        let p = 65537i64;
        let inverse = (1..p).find(|h| Q * h % p == 1).unwrap();
        assert_eq!((-1 + Q * inverse) % p, 0);
        assert_ne!(-1 + Q * inverse, 0);
        assert!((p as u128) < no_wrap_bound(1));
        assert!(no_wrap_bound(1024) < 1u128 << 50);
        assert!(check_carries(&[carry_bound(1024) as i64 + 1; 11], 1024).is_err());
    }
}
