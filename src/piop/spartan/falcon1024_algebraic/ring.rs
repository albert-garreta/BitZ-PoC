//! Native Falcon ideal check, followed by a bounded integer-coordinate lift.
//!
//! The returned linear claims are not authenticated until the caller binds both
//! coefficient vectors to the same BitZ source used by the norm proof.

use field::{FpLinearAcc, Reduce, RingOps, Uint};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    piop::spartan::{
        SpartanField,
        falcon_extension::FalconExtension,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        squeeze_field,
    },
    transcript::traits::Transcript,
};

use super::{Cfg, F, FalconAlgebraicStatement, FalconError, Layout, N, Q, WitnessData};

pub(super) const EXTENSION_DEGREE: usize = 11;
pub(super) type Ext = FalconExtension<EXTENSION_DEGREE>;
const DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/native-ring/v2";
const COEFFICIENT_ABS_BOUND: u64 = 1 << 14;

/// The quotient is fixed before alpha; the carries are fixed before xi.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Proof {
    pub certificate: Vec<Ext>,
    pub carries: [i64; EXTENSION_DEGREE],
    pub projection_nonce: Option<u64>,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        2 * EXTENSION_DEGREE * self.certificate.len()
            + 8 * EXTENSION_DEGREE
            + usize::from(self.projection_nonce.is_some()) * 8
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
    s1: Vec<Ext>,
    s2: Vec<Ext>,
    target: Ext,
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
    let certificate = certificate(&data.quotients, &lambda);
    transcript.absorb_slice(b"quotient-certificate");
    absorb_extensions(transcript, &certificate);
    transcript.absorb_slice(b"polynomial-evaluation");
    let alpha = sample_extension(transcript, true)?;
    let coordinates = coordinates(public, &lambda, &powers(alpha), &certificate);
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
            certificate,
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
    if proof.certificate.len() != N - 1
        || !proof.certificate.iter().all(|x| x.canonical())
        || proof.projection_nonce.is_some() != (grinding_bits != 0)
    {
        return Err(error("invalid algebraic ring proof encoding"));
    }
    check_carries(&proof.carries, layout.batch())?;
    bind_parameters(transcript, layout, field, grinding_bits);
    let lambda = instance_weights(transcript, layout)?;
    transcript.absorb_slice(b"quotient-certificate");
    absorb_extensions(transcript, &proof.certificate);
    transcript.absorb_slice(b"polynomial-evaluation");
    let alpha = sample_extension(transcript, true)?;
    let coordinates = coordinates(public, &lambda, &powers(alpha), &proof.certificate);
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
    if data.witness.s1.len() != layout.batch()
        || data.witness.s2.len() != layout.batch()
        || data.quotients.len() != layout.batch()
        || data.quotients.iter().any(|row| {
            row.len() != N - 1 || row.iter().any(|&coefficient| i64::from(coefficient) >= Q)
        })
    {
        return Err(error(
            "algebraic ring witness dimensions or quotient encoding",
        ));
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

fn certificate(quotients: &[Vec<u16>], lambda: &[Ext]) -> Vec<Ext> {
    let coefficient = |j: usize| {
        let mut sum = [0u64; EXTENSION_DEGREE];
        for (quotient, weight) in quotients.iter().zip(lambda) {
            for (total, &coordinate) in sum.iter_mut().zip(&weight.0) {
                *total += u64::from(quotient[j]) * u64::from(coordinate);
            }
        }
        Ext::new(sum.map(|x| (x % Q as u64) as u16))
    };
    #[cfg(feature = "parallel")]
    if quotients.len() >= 8 {
        return (0..N - 1).into_par_iter().map(coefficient).collect();
    }
    (0..N - 1).map(coefficient).collect()
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
    powers: &[Ext],
    certificate: &[Ext],
) -> Coordinates {
    let batch = public.public_keys.len();
    let d_at_alpha = certificate
        .iter()
        .zip(powers)
        .fold(Ext::ZERO, |sum, (&coefficient, &power)| {
            sum.add(coefficient.mul(power))
        });
    let mut s1 = vec![Ext::ZERO; batch * N];
    let mut s2 = vec![Ext::ZERO; batch * N];
    let mut targets = vec![Ext::ZERO; batch];
    let fill = |(i, ((a, b), target)): (usize, ((&mut [Ext], &mut [Ext]), &mut Ext))| {
        let lambda_h = lambda[i].mul(eval_public(&public.public_keys[i], powers));
        *target = lambda[i].mul(eval_public(&public.targets[i], powers));
        for ((a, b), &power) in a.iter_mut().zip(b).zip(powers) {
            // These products must be performed in E before lifting coordinates.
            *a = lambda[i].mul(power);
            *b = lambda_h.mul(power);
        }
    };
    #[cfg(feature = "parallel")]
    s1.par_chunks_mut(N)
        .zip(s2.par_chunks_mut(N))
        .zip(targets.par_iter_mut())
        .enumerate()
        .for_each(fill);
    #[cfg(not(feature = "parallel"))]
    s1.chunks_mut(N)
        .zip(s2.chunks_mut(N))
        .zip(targets.iter_mut())
        .enumerate()
        .for_each(fill);
    let target = targets
        .into_iter()
        .fold(Ext::ZERO, Ext::add)
        .sub(powers[N].add(Ext::ONE).mul(d_at_alpha));
    Coordinates { s1, s2, target }
}

fn calculate_carries(
    coordinates: &Coordinates,
    data: &WitnessData,
) -> Result<[i64; EXTENSION_DEGREE], FalconError> {
    let mut totals = [0i64; EXTENSION_DEGREE];
    for (weights, witness) in [
        (&coordinates.s1, &data.witness.s1),
        (&coordinates.s2, &data.witness.s2),
    ] {
        for (weight, &coefficient) in weights.iter().zip(witness.iter().flatten()) {
            for (total, &coordinate) in totals.iter_mut().zip(&weight.0) {
                *total += i64::from(coordinate) * i64::from(coefficient);
            }
        }
    }
    let mut carries = [0i64; EXTENSION_DEGREE];
    for k in 0..EXTENSION_DEGREE {
        let difference = totals[k] - i64::from(coordinates.target.0[k]);
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
    let weight = |value: &Ext| {
        // Eleven 126-bit field representatives times fourteen-bit coordinates
        // fit in the 192-bit linear accumulator, retaining Montgomery scale.
        let mut sum = FpLinearAcc::<2, 1>::default();
        for (&coordinate, power) in value.0.iter().zip(&xi_powers) {
            sum.accumulate(power, &Uint::from_words([u64::from(coordinate)]));
        }
        field.reduce(sum)
    };
    #[cfg(feature = "parallel")]
    let weights_s1 = coordinates.s1.par_iter().map(weight).collect();
    #[cfg(not(feature = "parallel"))]
    let weights_s1 = coordinates.s1.iter().map(weight).collect();
    #[cfg(feature = "parallel")]
    let weights_s2 = coordinates.s2.par_iter().map(weight).collect();
    #[cfg(not(feature = "parallel"))]
    let weights_s2 = coordinates.s2.iter().map(weight).collect();
    let target = (0..EXTENSION_DEGREE).fold(field.zero(), |sum, k| {
        let value = i128::from(coordinates.target.0[k]) + i128::from(Q) * i128::from(carries[k]);
        let magnitude = F::from_with_cfg(value.unsigned_abs(), field);
        let value = if value < 0 {
            field.neg(&magnitude)
        } else {
            magnitude
        };
        field.add(&sum, &field.mul(&xi_powers[k], &value))
    });
    PreparedClaim {
        weights_s1,
        weights_s2,
        target,
    }
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::super::FalconAlgebraicWitness;
    use super::*;
    use crate::transcript::{Blake3Transcript, traits::ConstTranscribable};
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
        let mut quotient = proof.clone();
        quotient.certificate[0] = quotient.certificate[0].add(Ext::ONE);
        let mut carry = proof.clone();
        carry.carries[0] += 1;
        for altered in [quotient, carry] {
            // Ring verification produces a claim; the shared source binder is
            // what rejects it, so check that obligation explicitly here.
            let claim = verify(
                &mut Blake3Transcript::new(),
                &layout,
                &public,
                &altered,
                &field,
                0,
            )
            .unwrap();
            assert_ne!(dot(&claim, &data, &field), claim.target);
        }
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
    fn malformed_quotients_carries_and_nonce_shapes_are_rejected() {
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
        let mut short = proof.clone();
        short.certificate.pop();
        let mut long = proof.clone();
        long.certificate.push(Ext::ZERO);
        let mut noncanonical = proof.clone();
        noncanonical.certificate[0].0[1] = Q as u16;
        let mut out_of_bound = proof.clone();
        out_of_bound.carries[0] = carry_bound(1) as i64 + 1;
        let mut signed_minimum = proof.clone();
        signed_minimum.carries[0] = i64::MIN;
        let mut nonce = proof.clone();
        nonce.projection_nonce = Some(0);
        for altered in [
            short,
            long,
            noncanonical,
            out_of_bound,
            signed_minimum,
            nonce,
        ] {
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
