//! Native Falcon ideal check, followed by a bounded integer-coordinate lift.
//!
//! The returned linear claims are not authenticated until the caller binds both
//! coefficient vectors to the same BitZ source used by the norm proof.

use field::{
    BatchMulAcc, FpSignedLinearAcc, PreparedLinearCombination, Reduce, RingOps,
    q12289::{PowerBasis, PowerCoordinates},
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        falcon_extension::FalconExtension,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        squeeze_field,
    },
    transcript::traits::Transcript,
};

use super::{Cfg, F, FalconAlgebraicStatement, FalconError, Layout, N, Q, WitnessData};

pub(super) const EXTENSION_DEGREE: usize = 11;
pub(super) type Ext = FalconExtension<EXTENSION_DEGREE>;
const DOMAIN: &[u8] = b"bitz/falcon-algebraic/native-ring/negacyclic-adjoint/v4";
type CoordinatesInAlpha = PowerCoordinates<EXTENSION_DEGREE>;
const COEFFICIENT_ABS_BOUND: u64 = 1 << 14;

/// The coefficient witness is committed before alpha; carries are fixed before xi.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Proof {
    pub carries: [i64; EXTENSION_DEGREE],
    pub projection_nonce: Option<u64>,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        8 * EXTENSION_DEGREE + usize::from(self.projection_nonce.is_some()) * 8
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedClaim {
    /// Live signature-major coefficients, without padded instances.
    pub weights_s1: Vec<F>,
    pub weights_s2: Vec<F>,
    pub target: F,
}

struct Coordinates {
    s1: Vec<CoordinatesInAlpha>,
    s2: Vec<CoordinatesInAlpha>,
    target: CoordinatesInAlpha,
}

struct ProjectionGrinding;
impl GrindingDomain for ProjectionGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/native-ring/projection-grinding/v2";
}

#[tracing::instrument(skip_all, name = "falcon_algebraic_ring:prove")]
pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    data: &WitnessData,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<(Proof, PreparedClaim), FalconError> {
    validate_context(layout, public, field, grinding_bits)?;
    validate_witness(layout, data)?;
    bind_parameters(transcript, layout, field, grinding_bits);
    let lambda = instance_weights(transcript, layout)?;
    transcript.absorb_slice(b"polynomial-evaluation");
    let alpha = sample_extension(transcript, true)?;
    let coordinates = coordinates(public, &lambda, alpha)?;
    let carries = calculate_carries(&coordinates, data)?;
    check_carries(&carries, layout.batch())?;
    absorb_carries(transcript, &carries);
    let projection_nonce = if grinding_bits == 0 {
        None
    } else {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<ProjectionGrinding>::new(0),
                grinding_bits,
            )
            .map_err(|e| error(e.to_string()))?,
        )
    };
    transcript.absorb_slice(b"coordinate-collapse");
    let xi = squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?;
    let claim = collapse_claim(&coordinates, &carries, xi, field);
    Ok((
        Proof {
            carries,
            projection_nonce,
        },
        claim,
    ))
}

#[tracing::instrument(skip_all, name = "falcon_algebraic_ring:verify")]
pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    proof: &Proof,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<PreparedClaim, FalconError> {
    validate_context(layout, public, field, grinding_bits)?;
    if proof.projection_nonce.is_some() != (grinding_bits != 0) {
        return Err(error("invalid algebraic ring proof encoding"));
    }
    check_carries(&proof.carries, layout.batch())?;
    bind_parameters(transcript, layout, field, grinding_bits);
    let lambda = instance_weights(transcript, layout)?;
    transcript.absorb_slice(b"polynomial-evaluation");
    let alpha = sample_extension(transcript, true)?;
    let coordinates = coordinates(public, &lambda, alpha)?;
    absorb_carries(transcript, &proof.carries);
    if let Some(nonce) = proof.projection_nonce {
        verify_and_absorb(
            transcript,
            GrindingRound::<ProjectionGrinding>::new(0),
            grinding_bits,
            nonce,
        )
        .map_err(|e| error(e.to_string()))?;
    }
    transcript.absorb_slice(b"coordinate-collapse");
    let xi = squeeze_field(transcript, field).map_err(|e| error(e.to_string()))?;
    Ok(collapse_claim(&coordinates, &proof.carries, xi, field))
}

fn validate_context(
    layout: &Layout,
    public: &FalconAlgebraicStatement,
    field: &Cfg,
    grinding_bits: u32,
) -> Result<(), FalconError> {
    if public.public_keys.len() != layout.batch() || public.targets.len() != layout.batch() {
        return Err(error("algebraic ring public input dimensions"));
    }
    if public
        .public_keys
        .iter()
        .chain(&public.targets)
        .flatten()
        .any(|&x| i64::from(x) >= Q)
    {
        return Err(error("algebraic ring public coefficient is not canonical"));
    }
    if field.modulus_u128() <= no_wrap_bound(layout.batch()) {
        return Err(error("algebraic ring coordinate lift exceeds the prime"));
    }
    if grinding_bits > 256 {
        return Err(error("invalid algebraic ring grinding difficulty"));
    }
    Ok(())
}

fn validate_witness(layout: &Layout, data: &WitnessData) -> Result<(), FalconError> {
    if data.witness.s1.len() != layout.batch() || data.witness.s2.len() != layout.batch() {
        return Err(error("algebraic ring witness dimensions"));
    }
    if data
        .witness
        .s1
        .iter()
        .chain(&data.witness.s2)
        .flatten()
        .any(|&s| !(-(1 << 14)..(1 << 14)).contains(&i32::from(s)))
    {
        return Err(error(
            "algebraic ring witness exceeds the signed 15-bit range",
        ));
    }
    Ok(())
}

fn bind_parameters(
    transcript: &mut impl Transcript,
    layout: &Layout,
    field: &Cfg,
    grinding_bits: u32,
) {
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(Q as u64).to_le_bytes());
    // theta^11 + theta + 14, in ascending coefficient order.
    for coefficient in [14u16, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] {
        transcript.absorb_slice(&coefficient.to_le_bytes());
    }
    transcript.absorb_slice(&(N as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.batch() as u64).to_le_bytes());
    transcript.absorb_slice(&(layout.capacity() as u64).to_le_bytes());
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    transcript.absorb_slice(&grinding_bits.to_le_bytes());
    transcript.absorb_slice(&carry_bound(layout.batch()).to_le_bytes());
}

fn instance_weights(
    transcript: &mut impl Transcript,
    layout: &Layout,
) -> Result<Vec<Ext>, FalconError> {
    transcript.absorb_slice(b"instance-batching");
    let point = (0..layout.capacity().ilog2())
        .map(|_| sample_extension(transcript, false))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(equality_weights(&point))
}

/// Uniform sampling, optionally conditioned on being outside the base field.
/// Eleven is prime, so every non-base element generates this extension.
fn sample_extension(transcript: &mut impl Transcript, non_base: bool) -> Result<Ext, FalconError> {
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
                return Err(error("algebraic ring field sampling exhausted"));
            }
        }
        if !non_base || coordinates[1..].iter().any(|&coordinate| coordinate != 0) {
            let result = Ext::new(coordinates);
            absorb_extensions(transcript, &[result]);
            return Ok(result);
        }
    }
    Err(error("algebraic ring non-base sampling exhausted"))
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
    transcript.absorb_slice(b"integer-coordinate-carries");
    let mut bytes = [0u8; 8 * EXTENSION_DEGREE];
    for (chunk, carry) in bytes.chunks_exact_mut(8).zip(carries) {
        chunk.copy_from_slice(&carry.to_le_bytes());
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

fn powers(alpha: Ext) -> Vec<Ext> {
    let mut result = Vec::with_capacity(N + 1);
    result.push(Ext::ONE);
    for j in 1..=N {
        result.push(result[j - 1].mul(alpha));
    }
    result
}

fn eval_public(polynomial: &[u16; N], powers: &[Ext]) -> Ext {
    let mut sum = [0u64; EXTENSION_DEGREE];
    for (&coefficient, power) in polynomial.iter().zip(powers) {
        for (total, &coordinate) in sum.iter_mut().zip(&power.0) {
            *total += u64::from(coefficient) * u64::from(coordinate);
        }
    }
    Ext::new(sum.map(|x| (x % Q as u64) as u16))
}

#[tracing::instrument(skip_all, name = "falcon_algebraic_ring:coordinates")]
fn coordinates(
    public: &FalconAlgebraicStatement,
    lambda: &[Ext],
    alpha: Ext,
) -> Result<Coordinates, FalconError> {
    // The transcript samples alpha outside F_q. Since 11 is prime, its powers
    // form a basis. Still reject a singular basis rather than assuming invertibility.
    let basis = PowerBasis::try_new(alpha)
        .map_err(|e| error(format!("invalid algebraic ring power basis: {e:?}")))?;
    let powers = powers(alpha);
    let batch = public.public_keys.len();
    let wrap = powers[N].add(Ext::ONE);
    let mut s1 = vec![CoordinatesInAlpha::ZERO; batch * N];
    let mut s2 = vec![CoordinatesInAlpha::ZERO; batch * N];
    let mut targets = vec![Ext::ZERO; batch];
    // Four independent SIMD states hide the dependency chain in each pair.
    // Portable builds keep one signature per task and the existing recurrence.
    const PAIRS: usize = if cfg!(all(
        target_arch = "x86_64",
        target_feature = "avx512f",
        target_feature = "avx512bw"
    )) {
        4
    } else {
        1
    };
    #[cfg(feature = "parallel")]
    let workers = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let workers = 1;
    // Keep at least one independent task per worker for small batches.
    let group_size = if batch / PAIRS >= workers { PAIRS } else { 1 };
    let prepare = |i: usize, target: &mut Ext| {
        let lambda_h = lambda[i].mul(eval_public(&public.public_keys[i], &powers));
        *target = lambda[i].mul(eval_public(&public.targets[i], &powers));
        // W_j = (h(X) X^j mod (X^N+1))(alpha). Multiplication by X gives
        // W_{j+1} = alpha W_j - (alpha^N+1) h[N-1-j]. Apply lambda first
        // to obtain canonical adjoint coordinates in O(N*K) per signature.
        let subtract = basis.encode(lambda[i].mul(wrap));
        ([basis.encode(lambda[i]), basis.encode(lambda_h)], subtract)
    };
    let fill = |(chunk, ((a, b), targets)): (
        usize,
        (
            (&mut [CoordinatesInAlpha], &mut [CoordinatesInAlpha]),
            &mut [Ext],
        ),
    )| {
        let start = chunk * group_size;
        if PAIRS > 1 && targets.len() == PAIRS {
            let prepared: [_; PAIRS] = std::array::from_fn(|i| prepare(start + i, &mut targets[i]));
            let mut a = a.chunks_mut(N);
            let mut b = b.chunks_mut(N);
            basis.fill_powers_adjoint_pairs(
                prepared.map(|v| v.0),
                prepared.map(|v| v.1),
                std::array::from_fn(|i| public.public_keys[start + i].as_slice()),
                std::array::from_fn(|_| [a.next().unwrap(), b.next().unwrap()]),
            );
        } else {
            for (i, ((a, b), target)) in a
                .chunks_mut(N)
                .zip(b.chunks_mut(N))
                .zip(targets)
                .enumerate()
            {
                let (starts, subtract) = prepare(start + i, target);
                basis.fill_powers_adjoint_pair(
                    starts,
                    subtract,
                    &public.public_keys[start + i],
                    [a, b],
                );
            }
        }
    };
    #[cfg(feature = "parallel")]
    s1.par_chunks_mut(group_size * N)
        .zip(s2.par_chunks_mut(group_size * N))
        .zip(targets.par_chunks_mut(group_size))
        .enumerate()
        .for_each(fill);
    #[cfg(not(feature = "parallel"))]
    s1.chunks_mut(group_size * N)
        .zip(s2.chunks_mut(group_size * N))
        .zip(targets.chunks_mut(group_size))
        .enumerate()
        .for_each(fill);
    let target = basis.encode(targets.into_iter().fold(Ext::ZERO, Ext::add));
    Ok(Coordinates { s1, s2, target })
}

#[tracing::instrument(skip_all, name = "falcon_algebraic_ring:carries")]
fn calculate_carries(
    coordinates: &Coordinates,
    data: &WitnessData,
) -> Result<[i64; EXTENSION_DEGREE], FalconError> {
    let batch = data.witness.s1.len();
    let sum = |start: usize, end: usize| {
        let mut totals = [0i64; EXTENSION_DEGREE];
        for (weights, witness) in [
            (&coordinates.s1, &data.witness.s1),
            (&coordinates.s2, &data.witness.s2),
        ] {
            for (weight, &coefficient) in weights[start * N..end * N]
                .iter()
                .zip(witness[start..end].iter().flatten())
            {
                for (total, &coordinate) in totals.iter_mut().zip(weight.coordinates()) {
                    *total += i64::from(coordinate) * i64::from(coefficient);
                }
            }
        }
        totals
    };
    #[cfg(feature = "parallel")]
    let totals = if batch >= 128 && rayon::current_num_threads() > 1 {
        // At least eight tasks amortize scheduling. The sum of absolute terms
        // is below 2^49 even at batch 1024, so integer reassociation is exact.
        const CHUNK: usize = 16;
        (0..batch.div_ceil(CHUNK))
            .into_par_iter()
            .map(|chunk| sum(chunk * CHUNK, ((chunk + 1) * CHUNK).min(batch)))
            .reduce(
                || [0; EXTENSION_DEGREE],
                |mut left, right| {
                    for (left, right) in left.iter_mut().zip(right) {
                        *left += right;
                    }
                    left
                },
            )
    } else {
        sum(0, batch)
    };
    #[cfg(not(feature = "parallel"))]
    let totals = sum(0, batch);
    let mut carries = [0i64; EXTENSION_DEGREE];
    for k in 0..EXTENSION_DEGREE {
        let difference = totals[k] - i64::from(coordinates.target.coordinates()[k]);
        if difference % Q != 0 {
            return Err(error("algebraic ring claim has no integer coordinate lift"));
        }
        carries[k] = difference / Q;
    }
    Ok(carries)
}

pub(super) fn carry_bound(batch: usize) -> u64 {
    2 * batch as u64 * N as u64 * COEFFICIENT_ABS_BOUND
}

pub(super) fn no_wrap_bound(batch: usize) -> u128 {
    (2 * Q as u128 - 1) * u128::from(carry_bound(batch)) + Q as u128 - 1
}

fn check_carries(carries: &[i64; EXTENSION_DEGREE], batch: usize) -> Result<(), FalconError> {
    if carries
        .iter()
        .any(|carry| carry.unsigned_abs() > carry_bound(batch))
    {
        return Err(error("algebraic ring coordinate carry is out of bounds"));
    }
    Ok(())
}

#[tracing::instrument(skip_all, name = "falcon_algebraic_ring:collapse")]
fn collapse_claim(
    coordinates: &Coordinates,
    carries: &[i64; EXTENSION_DEGREE],
    xi: F,
    field: &Cfg,
) -> PreparedClaim {
    let mut xi_powers = [field.one(); EXTENSION_DEGREE];
    for k in 1..EXTENSION_DEGREE {
        xi_powers[k] = field.mul(&xi_powers[k - 1], &xi);
    }
    // Prepare an extra Montgomery factor once for both vectors. With weights
    // below p and coordinates at most q-1, S <= 11*(q-1)*(p-1) < p*2^128.
    // One REDC therefore returns the required Montgomery scale without a
    // general remainder reduction for each eleven-term dot product.
    let prepared =
        <Cfg as PreparedLinearCombination<u64>>::prepare_linear_combination(field, xi_powers);
    let weight = |value: &CoordinatesInAlpha| {
        <Cfg as PreparedLinearCombination<u64>>::linear_combination(&prepared, |k| {
            u64::from(value.coordinates()[k])
        })
    };
    #[cfg(feature = "parallel")]
    let weights_s1 = coordinates.s1.par_iter().map(weight).collect();
    #[cfg(not(feature = "parallel"))]
    let weights_s1 = coordinates.s1.iter().map(weight).collect();
    #[cfg(feature = "parallel")]
    let weights_s2 = coordinates.s2.par_iter().map(weight).collect();
    #[cfg(not(feature = "parallel"))]
    let weights_s2 = coordinates.s2.iter().map(weight).collect();
    // Eleven bounded signed coordinates share one mixed reduction; carries
    // remain native integers throughout this dot product.
    let mut target = FpSignedLinearAcc::<2, 2>::default();
    for k in 0..EXTENSION_DEGREE {
        let value = i128::from(coordinates.target.coordinates()[k])
            + i128::from(Q) * i128::from(carries[k]);
        field.mul_acc(&mut target, &xi_powers[k], &value);
    }
    PreparedClaim {
        weights_s1,
        weights_s2,
        target: field.reduce(target),
    }
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::super::FalconAlgebraicWitness;
    use super::*;
    use crate::{
        piop::spartan::SpartanField,
        transcript::{Blake3Transcript, traits::ConstTranscribable},
    };
    use field::Uint;
    use std::collections::VecDeque;

    fn fixture(batch: usize) -> (Layout, FalconAlgebraicStatement, WitnessData) {
        let layout = Layout::new(batch).unwrap();
        let mut public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]; batch],
            targets: vec![[0; N]; batch],
        };
        let mut witness = FalconAlgebraicWitness {
            s1: vec![[0; N]; batch],
            s2: vec![[0; N]; batch],
        };
        for i in 0..batch {
            // h=X^1023, s2=(i+1)X, s1=-2. The product wraps with a minus sign.
            public.public_keys[i][N - 1] = 1;
            witness.s1[i][0] = -2;
            witness.s2[i][1] = (i + 1) as i16;
            public.targets[i][0] = (Q - 3 - i as i64) as u16;
        }
        let data = WitnessData::new(&public, witness).unwrap();
        (layout, public, data)
    }

    fn field() -> Cfg {
        // The ring reduction only needs its exact no-wrap bound. The enclosing
        // protocol separately enforces its narrower transcript-sampled family.
        F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
    }

    #[test]
    fn prepared_collapse_matches_individual_field_operations() {
        // For alpha=Z, encode preserves the coordinates, including q-1.
        let basis =
            PowerBasis::try_new(Ext::new(std::array::from_fn(|k| u16::from(k == 1)))).unwrap();
        let mut values = vec![
            [0; EXTENSION_DEGREE],
            [1; EXTENSION_DEGREE],
            [(Q - 1) as u16; EXTENSION_DEGREE],
        ];
        for coordinate in 0..EXTENSION_DEGREE {
            values.push(std::array::from_fn(|k| {
                if k == coordinate { (Q - 1) as u16 } else { 0 }
            }));
        }
        let mut state = 0x1234_5678_9abc_def0u64;
        for _ in 0..257 {
            values.push(std::array::from_fn(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state % Q as u64) as u16
            }));
        }
        let coordinates = Coordinates {
            s1: values.iter().map(|&v| basis.encode(Ext::new(v))).collect(),
            s2: values
                .iter()
                .rev()
                .map(|&v| basis.encode(Ext::new(v)))
                .collect(),
            target: basis.encode(Ext::new([(Q - 1) as u16; EXTENSION_DEGREE])),
        };
        let bound = carry_bound(1024) as i64;
        let carries = std::array::from_fn(|k| [0, 1, -1, bound, -bound][k % 5]);
        for prime in [(1u128 << 61) - 1, (1u128 << 127) - 1] {
            let field = F::make_cfg(&Uint::from(prime)).unwrap();
            for xi in [0, 1, prime - 1, 1234567890123456789] {
                let xi = F::from_with_cfg(xi, &field);
                let actual = collapse_claim(&coordinates, &carries, xi, &field);
                let mut powers = [field.one(); EXTENSION_DEGREE];
                for k in 1..EXTENSION_DEGREE {
                    powers[k] = field.mul(&powers[k - 1], &xi);
                }
                for (weights, inputs) in [
                    (&actual.weights_s1, &coordinates.s1),
                    (&actual.weights_s2, &coordinates.s2),
                ] {
                    for (weight, input) in weights.iter().zip(inputs) {
                        let expected = input.coordinates().iter().zip(powers).fold(
                            field.zero(),
                            |sum, (&c, power)| {
                                field.add(
                                    &sum,
                                    &field.mul(&F::from_with_cfg(u64::from(c), &field), &power),
                                )
                            },
                        );
                        assert_eq!(*weight, expected);
                    }
                }
                let expected = (0..EXTENSION_DEGREE).fold(field.zero(), |sum, k| {
                    let value = i128::from(coordinates.target.coordinates()[k])
                        + i128::from(Q) * i128::from(carries[k]);
                    field.add(
                        &sum,
                        &field.mul(&F::from_with_cfg(value, &field), &powers[k]),
                    )
                });
                assert_eq!(actual.target, expected);
            }
        }
    }

    fn dot(claim: &PreparedClaim, data: &WitnessData, field: &Cfg) -> F {
        let mut sum = field.zero();
        for (weights, coefficients) in [
            (&claim.weights_s1, &data.witness.s1),
            (&claim.weights_s2, &data.witness.s2),
        ] {
            for (weight, &coefficient) in weights.iter().zip(coefficients.iter().flatten()) {
                sum = field.add(
                    &sum,
                    &field.mul(weight, &F::from_with_cfg(i64::from(coefficient), field)),
                );
            }
        }
        sum
    }

    fn schoolbook_target(h: &[u16; N], s1: &[i16; N], s2: &[i16; N]) -> [u16; N] {
        let mut result = s1.map(i64::from);
        for (j, &b) in s2.iter().enumerate() {
            for (k, &a) in h.iter().enumerate() {
                let product = i64::from(a) * i64::from(b);
                if j + k < N {
                    result[j + k] += product;
                } else {
                    result[j + k - N] -= product;
                }
            }
        }
        result.map(|value| value.rem_euclid(Q) as u16)
    }

    // Evaluate each independently shifted, reduced polynomial. This deliberately
    // uses no adjoint recurrence and covers every coefficient crossing X^N=-1.
    fn shifted_public_evaluation(h: &[u16; N], shift: usize, powers: &[Ext]) -> Ext {
        let mut sums = [0i64; EXTENSION_DEGREE];
        for (k, &coefficient) in h.iter().enumerate() {
            let index = k + shift;
            let sign = if index < N { 1 } else { -1 };
            for (sum, &coordinate) in sums.iter_mut().zip(&powers[index % N].0) {
                *sum += sign * i64::from(coefficient) * i64::from(coordinate);
            }
        }
        Ext::new(sums.map(|value| value.rem_euclid(Q) as u16))
    }

    #[test]
    fn alpha_coordinates_and_carries_match_independent_fixed_basis_reference() {
        let (_, mut public, mut data) = fixture(5);
        for i in 0..5 {
            public.public_keys[i] = std::array::from_fn(|j| ((i * 37 + j * 101) as i64 % Q) as u16);
            data.witness.s1[i] = std::array::from_fn(|j| (j as i16 % 17) - 8);
            data.witness.s2[i] = std::array::from_fn(|j| (j as i16 % 11) - 5);
            public.targets[i] = schoolbook_target(
                &public.public_keys[i],
                &data.witness.s1[i],
                &data.witness.s2[i],
            );
        }
        let alpha = Ext::new(std::array::from_fn(|k| (97 * k + 321) as u16));
        let basis = PowerBasis::try_new(alpha).unwrap();
        let lambda: Vec<_> = (0..5)
            .map(|i| Ext::new(std::array::from_fn(|k| ((i + 1) * 123 + k * 17) as u16)))
            .collect();
        #[cfg(feature = "parallel")]
        let actual = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| coordinates(&public, &lambda, alpha))
            .unwrap();
        #[cfg(not(feature = "parallel"))]
        let actual = coordinates(&public, &lambda, alpha).unwrap();
        let mut reference = Coordinates {
            s1: vec![],
            s2: vec![],
            target: CoordinatesInAlpha::ZERO,
        };
        let powers = powers(alpha);
        let mut target = Ext::ZERO;
        for i in 0..5 {
            target = target.add(lambda[i].mul(eval_public(&public.targets[i], &powers)));
            for (j, &power) in powers[..N].iter().enumerate() {
                reference.s1.push(basis.encode(lambda[i].mul(power)));
                reference
                    .s2
                    .push(basis.encode(lambda[i].mul(shifted_public_evaluation(
                        &public.public_keys[i],
                        j,
                        &powers,
                    ))));
            }
        }
        reference.target = basis.encode(target);
        assert_eq!(actual.s1, reference.s1);
        assert_eq!(actual.s2, reference.s2);
        assert_eq!(actual.target, reference.target);
        assert_eq!(
            calculate_carries(&actual, &data).unwrap(),
            calculate_carries(&reference, &data).unwrap()
        );
        #[cfg(feature = "parallel")]
        for workers in [8, 16] {
            let other = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
                .install(|| coordinates(&public, &lambda, alpha))
                .unwrap();
            assert_eq!(other.s1, reference.s1);
            assert_eq!(other.s2, reference.s2);
            assert_eq!(other.target, reference.target);
        }
        assert!(coordinates(&public, &lambda, Ext::ONE).is_err());
    }

    #[test]
    fn ring_claim_matches_negacyclic_witness_for_padded_batches() {
        let field = field();
        for (batch, grinding_bits) in [(1, 0), (3, 0), (8, 0), (1, 3)] {
            let (layout, public, data) = fixture(batch);
            let mut prover = Blake3Transcript::new();
            let (proof, claim) =
                prove(&mut prover, &layout, &public, &data, &field, grinding_bits).unwrap();
            let mut verifier = Blake3Transcript::new();
            let checked = verify(
                &mut verifier,
                &layout,
                &public,
                &proof,
                &field,
                grinding_bits,
            )
            .unwrap();
            assert_eq!(claim, checked);
            assert_eq!(dot(&claim, &data, &field), claim.target);
            assert_eq!(claim.weights_s1.len(), batch * N);
            assert_eq!(claim.weights_s2.len(), batch * N);
            assert_eq!(
                proof.payload_size_bytes(),
                88 + usize::from(grinding_bits != 0) * 8
            );
            assert_eq!(
                prover.get_challenge::<u64>(),
                verifier.get_challenge::<u64>()
            );
        }
    }

    #[test]
    fn tampering_changes_the_authenticated_claim_or_is_rejected() {
        let (layout, public, data) = fixture(3);
        let field = field();
        let (proof, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &data,
            &field,
            0,
        )
        .unwrap();
        let mut carry = proof.clone();
        carry.carries[0] += 1;
        // Ring verification produces a claim; the shared source binder is
        // what rejects it, so check that obligation explicitly here.
        let claim = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &carry,
            &field,
            0,
        )
        .unwrap();
        assert_ne!(dot(&claim, &data, &field), claim.target);
        let mut wrong_target = public.clone();
        wrong_target.targets[0][0] -= 1;
        let claim = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &wrong_target,
            &proof,
            &field,
            0,
        )
        .unwrap();
        assert_ne!(dot(&claim, &data, &field), claim.target);
        let mut wrong_key = public.clone();
        wrong_key.public_keys[0][N - 1] = 2;
        let claim = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &wrong_key,
            &proof,
            &field,
            0,
        )
        .unwrap();
        assert_ne!(dot(&claim, &data, &field), claim.target);
    }

    #[test]
    fn malformed_carries_and_nonce_shapes_are_rejected() {
        let (layout, public, data) = fixture(1);
        let field = field();
        let (proof, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &data,
            &field,
            0,
        )
        .unwrap();
        let mut out_of_bound = proof.clone();
        out_of_bound.carries[0] = carry_bound(1) as i64 + 1;
        let mut signed_minimum = proof.clone();
        signed_minimum.carries[0] = i64::MIN;
        let mut nonce = proof.clone();
        nonce.projection_nonce = Some(0);
        for altered in [out_of_bound, signed_minimum, nonce] {
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &public,
                    &altered,
                    &field,
                    0
                )
                .is_err()
            );
        }
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &proof,
                &field,
                1
            )
            .is_err()
        );
    }

    #[test]
    fn coordinate_bound_covers_the_entire_encoded_witness_range() {
        for batch in [1, 3, 1024] {
            let terms = 2 * batch as u128 * N as u128;
            let encoded_sum_bound = terms * (Q as u128 - 1) * (1 << 14);
            let h = u128::from(carry_bound(batch));
            assert!(encoded_sum_bound + Q as u128 - 1 <= h * Q as u128);
            assert_eq!(
                no_wrap_bound(batch),
                encoded_sum_bound + Q as u128 - 1 + Q as u128 * h
            );
            assert!(no_wrap_bound(batch) < 1 << 50);
            assert!(check_carries(&[h as i64; EXTENSION_DEGREE], batch).is_ok());
            assert!(check_carries(&[-(h as i64); EXTENSION_DEGREE], batch).is_ok());
        }
        let (layout, public, _) = fixture(1);
        let small = field::FpCtx::from_prime_u128(12289);
        assert!(validate_context(&layout, &public, &small, 0).is_err());
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_carries_include_the_final_partial_chunk() {
        const BATCH: usize = 129;
        let mut alpha = Ext::ZERO;
        alpha.0[1] = 1;
        let basis = PowerBasis::try_new(alpha).unwrap();
        let patterns = [
            basis.encode(Ext::ZERO),
            basis.encode(Ext::ONE),
            basis.encode(Ext::new([(Q - 1) as u16; EXTENSION_DEGREE])),
            basis.encode(Ext::new(std::array::from_fn(|k| (97 * k + 43) as u16))),
        ];
        let mut data = WitnessData {
            witness: FalconAlgebraicWitness {
                s1: (0..BATCH)
                    .map(|i| std::array::from_fn(|j| [-16384, -1, 0, 1, 16383][(i + j) % 5]))
                    .collect(),
                s2: (0..BATCH)
                    .map(|i| std::array::from_fn(|j| [16383, -2049, 2048, -16384][(i + j) % 4]))
                    .collect(),
            },
            slacks: vec![0; BATCH],
        };
        // The final one-signature task has substantial signed contributions.
        // These are encoding-boundary inputs, independent of the norm gate.
        data.witness.s1[BATCH - 1].fill(16383);
        data.witness.s2[BATCH - 1].fill(-16384);
        let mut coordinates = Coordinates {
            s1: (0..BATCH * N).map(|j| patterns[j % 4]).collect(),
            s2: (0..BATCH * N).map(|j| patterns[(j / 7 + 1) % 4]).collect(),
            target: CoordinatesInAlpha::ZERO,
        };
        // Independent coordinate-major i128 dot products supply both a valid
        // target residue and the exact quotient expected from every worker count.
        let totals: [i128; EXTENSION_DEGREE] = std::array::from_fn(|k| {
            let mut total = 0i128;
            for signature in 0..BATCH {
                for j in 0..N {
                    let index = signature * N + j;
                    total += i128::from(coordinates.s1[index].coordinates()[k])
                        * i128::from(data.witness.s1[signature][j]);
                    total += i128::from(coordinates.s2[index].coordinates()[k])
                        * i128::from(data.witness.s2[signature][j]);
                }
            }
            total
        });
        let residues = totals.map(|value| value.rem_euclid(i128::from(Q)) as u16);
        coordinates.target = basis.encode(Ext::new(residues));
        // alpha is the fixed-basis generator, so these are also alpha coordinates.
        assert_eq!(coordinates.target.coordinates(), &residues);
        let expected = totals.map(|value| value.div_euclid(i128::from(Q)) as i64);
        for workers in [1, 2, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let actual = pool.install(|| calculate_carries(&coordinates, &data).unwrap());
            assert_eq!(actual, expected, "degree={N}, workers={workers}");
        }
    }

    #[test]
    fn an_invalid_ring_witness_has_no_coordinate_lift() {
        let (layout, public, mut data) = fixture(1);
        data.witness.s2[0][1] += 1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &data,
                &field(),
                0
            )
            .is_err()
        );
    }

    #[test]
    fn direct_ring_check_preserves_signed15_boundaries_and_wraparound() {
        let (layout, mut public, mut data) = fixture(1);
        let field = field();
        public.public_keys[0] =
            std::array::from_fn(|j| if j % 2 == 0 { 0 } else { (Q - 1) as u16 });
        data.witness.s1[0] = std::array::from_fn(|j| if j % 2 == 0 { -16384 } else { 16383 });
        data.witness.s2[0] = std::array::from_fn(|j| if j % 3 == 0 { 16383 } else { -16384 });
        public.targets[0] = schoolbook_target(
            &public.public_keys[0],
            &data.witness.s1[0],
            &data.witness.s2[0],
        );
        // The ring layer supports the whole encoding range. The enclosing norm
        // proof separately rejects this deliberately oversized norm.
        let (proof, claim) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &public,
            &data,
            &field,
            0,
        )
        .unwrap();
        assert_eq!(dot(&claim, &data, &field), claim.target);
        assert!(check_carries(&proof.carries, 1).is_ok());
        for invalid in [-16385, 16384] {
            data.witness.s1[0][0] = invalid;
            assert!(validate_witness(&layout, &data).is_err());
        }
    }

    struct ScriptedTranscript(VecDeque<u16>);
    impl Transcript for ScriptedTranscript {
        fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
            panic!("extension sampling uses the rejection stream")
        }
        fn begin_sampling(&mut self) {}
        fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
            output.copy_from_slice(&self.0.pop_front().expect("script exhausted").to_le_bytes());
        }
        fn absorb_inner(&mut self, _: &[u8]) {}
    }

    #[test]
    fn extension_sampler_rejects_biased_tail_and_base_field_evaluations() {
        let mut candidates = vec![5 * Q as u16, u16::MAX];
        candidates.extend([0; EXTENSION_DEGREE]);
        let mut expected = [0; EXTENSION_DEGREE];
        expected[1] = 1;
        candidates.extend(expected);
        let mut transcript = ScriptedTranscript(candidates.into());
        assert_eq!(
            sample_extension(&mut transcript, true).unwrap(),
            Ext::new(expected)
        );
        assert!(transcript.0.is_empty());
        let mut transcript = ScriptedTranscript(vec![u16::MAX; 128].into());
        assert!(sample_extension(&mut transcript, false).is_err());
    }
}
