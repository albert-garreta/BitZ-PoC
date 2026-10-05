//! Four committed Falcon operands reduced to one tensor query over the shared
//! arithmetic prime. The returned query still requires the source-bit binder.
//!
//! The ring certificate is encoded by its unique degree-1022 quotient D, so
//! e(Y)=(Y^1024+1)D(Y) has the required degree and ideal membership by construction.
//! The later 21-coefficient polynomial is an unreduced integer lift, not D.

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
    FalconError, FalconPublicStatement, FalconSourceLayout, FalconSourceWitness,
    FalconVerificationTrace, N, NONCE_BYTES, Q,
    constraints::SOURCE_RESIDUAL_BOUND,
    native_ring::{
        EXTENSION_DEGREE, Ext, absorb_extensions, certificate, equality_weights, sample_extension,
    },
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

const INNER_ROUNDS: usize = 10;
const LIFT_COEFFICIENTS: usize = 2 * EXTENSION_DEGREE - 1;
const PRIME_MIN: u128 = 1 << 125;
const PRIME_MAX: u128 = (1 << 126) - 1;
const DOMAIN: &[u8] = b"bitz/falcon1024-ct/shared-ring/v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Proof {
    pub certificate: Vec<Ext>,
    pub outer: Vec<[Ext; 4]>,
    /// Transcript order: C, H, S2, S1. Batching powers are 0, 1, 3, 2.
    pub evaluations: [Ext; 4],
    pub inner: Vec<[Ext; 3]>,
    pub inner_evaluation: Ext,
    pub lift: [i128; LIFT_COEFFICIENTS],
    pub projection_nonce: u64,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        2 * EXTENSION_DEGREE
            * (self.certificate.len() + 4 * self.outer.len() + 4 + 3 * self.inner.len() + 1)
            + 16 * LIFT_COEFFICIENTS
            + 8
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectedClaim {
    pub row: Vec<F>,
    pub column: Vec<F>,
    pub target: F,
}

pub(super) const fn projection_grinding_bits(target_bits: usize) -> u32 {
    if target_bits == 128 { 14 } else { 2 }
}

struct ProjectionGrinding;
impl GrindingDomain for ProjectionGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/shared-ring/projection-grinding/v1";
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:prove")]
pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    traces: &[FalconVerificationTrace],
    source: &FalconSourceWitness,
    target_bits: usize,
) -> Result<(Proof, Cfg, ProjectedClaim), FalconError> {
    validate_context(layout, statement, target_bits)?;
    if traces.len() != layout.batch()
        || source.layout() != layout
        || traces.iter().enumerate().any(|(s, trace)| {
            trace.public_key != statement.public_keys[s]
                || trace.signature != statement.signatures[s]
        })
    {
        return Err(error(
            "shared ring witness shape or public statement mismatch",
        ));
    }
    bind_parameters(transcript, layout, target_bits);
    let r = sample_point(transcript, b"instances", layout.capacity().ilog2() as usize)?;
    let lambda = equality_weights(&r);
    let certificate = certificate(traces, &lambda);
    transcript.absorb_slice(b"quotient-certificate");
    absorb_extensions(transcript, &certificate);
    let beta = challenge(transcript, b"polynomial-evaluation")?;
    let powers = powers(beta);
    let initial = quotient_evaluation(&certificate, beta, &powers);
    let tables = evaluation_tables(layout, traces, &powers);
    let (outer, t, evaluations) = prove_outer(transcript, lambda, tables, initial)?;
    check_outer_terminal(
        initial_after_rounds(&outer, &t, initial),
        &r,
        &t,
        &evaluations,
    )?;
    transcript.absorb_slice(b"outer-evaluations-C-H-S2-S1");
    absorb_extensions(transcript, &evaluations);
    let batching = challenge(transcript, b"operand-batching")?;
    let omega = operand_weights(batching);
    let tau = dot_ext(&omega, &evaluations);
    let mut row = equality_weights(&t);
    row[layout.batch()..].fill(Ext::ZERO);
    let combined = collapse_positions(traces, &row, &omega);
    let (inner, u, inner_evaluation) = prove_inner(transcript, powers, combined, tau)?;
    transcript.absorb_slice(b"inner-witness-evaluation");
    absorb_extensions(transcript, &[inner_evaluation]);
    check_inner_terminal(
        initial_after_rounds(&inner, &u, tau),
        beta,
        &u,
        inner_evaluation,
    )?;
    let (column, offset) = bit_query(layout, &row, &u, &omega)?;
    let target = inner_evaluation.sub(offset);
    let lift = grouped_lift(source, &row, &column);
    check_lift(layout, &lift, target)?;
    absorb_lift(transcript, &lift);
    let field = sample_shared_field(transcript, layout)?;
    transcript.absorb_slice(b"ring-prime-projection");
    let projection_nonce = {
        let _span = tracing::info_span!("falcon_shared_ring:projection_grinding").entered();
        grind_and_absorb(
            transcript,
            GrindingRound::<ProjectionGrinding>::new(0),
            projection_grinding_bits(target_bits),
        )
        .map_err(|e| error(e.to_string()))?
    };
    let alpha = squeeze_field(transcript, &field).map_err(|e| error(e.to_string()))?;
    let projected = project(&row, &column, &lift, alpha, &field);
    Ok((
        Proof {
            certificate,
            outer,
            evaluations,
            inner,
            inner_evaluation,
            lift,
            projection_nonce,
        },
        field,
        projected,
    ))
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:verify")]
pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    proof: &Proof,
    target_bits: usize,
) -> Result<(Cfg, ProjectedClaim), FalconError> {
    validate_context(layout, statement, target_bits)?;
    let rounds = layout.capacity().ilog2() as usize;
    if proof.certificate.len() != N - 1
        || proof.outer.len() != rounds
        || proof.inner.len() != INNER_ROUNDS
        || !proof.certificate.iter().all(|x| x.canonical())
        || !proof.outer.iter().flatten().all(|x| x.canonical())
        || !proof.evaluations.iter().all(|x| x.canonical())
        || !proof.inner.iter().flatten().all(|x| x.canonical())
        || !proof.inner_evaluation.canonical()
    {
        return Err(error("invalid shared ring proof encoding"));
    }
    bind_parameters(transcript, layout, target_bits);
    let r = sample_point(transcript, b"instances", rounds)?;
    transcript.absorb_slice(b"quotient-certificate");
    absorb_extensions(transcript, &proof.certificate);
    let beta = challenge(transcript, b"polynomial-evaluation")?;
    // The verifier needs only beta^N and D(beta), not all public operand evaluations.
    let mut beta_n = beta;
    for _ in 0..INNER_ROUNDS {
        beta_n = beta_n.mul(beta_n);
    }
    let initial = eval_ext(&proof.certificate, beta).mul(Ext::ONE.add(beta_n));
    let (t, outer_claim) = verify_sumcheck(transcript, b"outer-cubic", &proof.outer, initial)?;
    transcript.absorb_slice(b"outer-evaluations-C-H-S2-S1");
    absorb_extensions(transcript, &proof.evaluations);
    check_outer_terminal(outer_claim, &r, &t, &proof.evaluations)?;
    let batching = challenge(transcript, b"operand-batching")?;
    let omega = operand_weights(batching);
    let tau = dot_ext(&omega, &proof.evaluations);
    let (u, inner_claim) = verify_sumcheck(transcript, b"inner-quadratic", &proof.inner, tau)?;
    transcript.absorb_slice(b"inner-witness-evaluation");
    absorb_extensions(transcript, &[proof.inner_evaluation]);
    check_inner_terminal(inner_claim, beta, &u, proof.inner_evaluation)?;
    let mut row = equality_weights(&t);
    row[layout.batch()..].fill(Ext::ZERO);
    let (column, offset) = bit_query(layout, &row, &u, &omega)?;
    check_lift(layout, &proof.lift, proof.inner_evaluation.sub(offset))?;
    absorb_lift(transcript, &proof.lift);
    let field = sample_shared_field(transcript, layout)?;
    transcript.absorb_slice(b"ring-prime-projection");
    {
        let _span = tracing::info_span!("falcon_shared_ring:verify_projection_grinding").entered();
        verify_and_absorb(
            transcript,
            GrindingRound::<ProjectionGrinding>::new(0),
            projection_grinding_bits(target_bits),
            proof.projection_nonce,
        )
        .map_err(|e| error(e.to_string()))?;
    }
    let alpha = squeeze_field(transcript, &field).map_err(|e| error(e.to_string()))?;
    Ok((
        field.clone(),
        project(&row, &column, &proof.lift, alpha, &field),
    ))
}

fn validate_context(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    target_bits: usize,
) -> Result<(), FalconError> {
    statement.validate(layout.batch())?;
    if !layout.is_shared_prime() || !matches!(target_bits, 100 | 128) {
        return Err(error("unsupported shared ring layout or security target"));
    }
    if 2 * lift_bound(layout) >= PRIME_MIN || SOURCE_RESIDUAL_BOUND >= PRIME_MIN {
        return Err(error("shared ring bounds exceed the prime family"));
    }
    Ok(())
}

fn bind_parameters(t: &mut impl Transcript, layout: &FalconSourceLayout, target: usize) {
    t.absorb_slice(DOMAIN);
    for value in [
        Q as usize,
        N,
        EXTENSION_DEGREE,
        layout.batch(),
        layout.capacity(),
        layout.signature_stride(),
        layout.public_key_offset().expect("validated shared layout"),
        layout.row_vars(),
        layout.col_vars(),
        target,
    ] {
        t.absorb_slice(&(value as u64).to_le_bytes());
    }
    t.absorb_slice(
        b"f:T11+T+14;D:1023;P:i128le21;live-mask;C:u14;H:u14;S1:bounded14-6144;S2:encoded-signed12",
    );
    t.absorb_slice(&lift_bound(layout).to_le_bytes());
    t.absorb_slice(&SOURCE_RESIDUAL_BOUND.to_le_bytes());
    t.absorb_slice(&PRIME_MIN.to_le_bytes());
    t.absorb_slice(&PRIME_MAX.to_le_bytes());
    t.absorb_slice(&projection_grinding_bits(target).to_le_bytes());
}

fn challenge(t: &mut impl Transcript, label: &[u8]) -> Result<Ext, FalconError> {
    t.absorb_slice(label);
    // Every element of E is permitted, including zero and base-field elements.
    sample_extension(t, false)
}

fn sample_point(
    t: &mut impl Transcript,
    label: &[u8],
    len: usize,
) -> Result<Vec<Ext>, FalconError> {
    t.absorb_slice(label);
    t.absorb_slice(&(len as u64).to_le_bytes());
    (0..len).map(|_| sample_extension(t, false)).collect()
}

fn scale(value: Ext, scalar: i64) -> Ext {
    let scalar = scalar.rem_euclid(Q) as u64;
    Ext(value.0.map(|c| (u64::from(c) * scalar % Q as u64) as u16))
}

fn eval_ext(coefficients: &[Ext], point: Ext) -> Ext {
    coefficients
        .iter()
        .rev()
        .fold(Ext::ZERO, |a, &c| a.mul(point).add(c))
}

fn dot_ext(a: &[Ext], b: &[Ext]) -> Ext {
    a.iter()
        .zip(b)
        .fold(Ext::ZERO, |sum, (&a, &b)| sum.add(a.mul(b)))
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:beta_powers")]
fn powers(beta: Ext) -> Vec<Ext> {
    let mut powers = vec![Ext::ONE; N];
    for j in 1..N {
        powers[j] = powers[j - 1].mul(beta);
    }
    powers
}

fn quotient_evaluation(certificate: &[Ext], beta: Ext, powers: &[Ext]) -> Ext {
    eval_ext(certificate, beta).mul(Ext::ONE.add(powers[N - 1].mul(beta)))
}

fn operand_weights(lambda: Ext) -> [Ext; 4] {
    let square = lambda.mul(lambda);
    [Ext::ONE, lambda, square.mul(lambda), square]
}

fn coefficients(trace: &FalconVerificationTrace, j: usize) -> [u16; 4] {
    [
        trace.hash_to_point.point[j],
        trace.public_key.h[j],
        i64::from(trace.signature.s2[j]).rem_euclid(Q) as u16,
        i64::from(trace.s1[j]).rem_euclid(Q) as u16,
    ]
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:operand_evaluations")]
fn evaluation_tables(
    layout: &FalconSourceLayout,
    traces: &[FalconVerificationTrace],
    powers: &[Ext],
) -> [Vec<Ext>; 4] {
    let evaluate = |trace: &FalconVerificationTrace| {
        let mut sums = [[0u64; EXTENSION_DEGREE]; 4];
        for (j, power) in powers.iter().enumerate() {
            for (sum, coefficient) in sums.iter_mut().zip(coefficients(trace, j)) {
                for (value, &coordinate) in sum.iter_mut().zip(&power.0) {
                    *value += u64::from(coefficient) * u64::from(coordinate);
                }
            }
        }
        sums.map(|sum| Ext(sum.map(|c| (c % Q as u64) as u16)))
    };
    #[cfg(feature = "parallel")]
    let values: Vec<_> = traces.par_iter().map(evaluate).collect();
    #[cfg(not(feature = "parallel"))]
    let values: Vec<_> = traces.iter().map(evaluate).collect();
    std::array::from_fn(|operand| {
        let mut table = vec![Ext::ZERO; layout.capacity()];
        for (entry, value) in table.iter_mut().zip(&values) {
            *entry = value[operand];
        }
        table
    })
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:collapse_positions")]
fn collapse_positions(
    traces: &[FalconVerificationTrace],
    row: &[Ext],
    omega: &[Ext; 4],
) -> Vec<Ext> {
    let collapse = |j| {
        let mut sums = [[0u64; EXTENSION_DEGREE]; 4];
        for (trace, weight) in traces.iter().zip(row) {
            for (sum, coefficient) in sums.iter_mut().zip(coefficients(trace, j)) {
                for (value, &coordinate) in sum.iter_mut().zip(&weight.0) {
                    *value += u64::from(coefficient) * u64::from(coordinate);
                }
            }
        }
        let values = sums.map(|sum| Ext(sum.map(|c| (c % Q as u64) as u16)));
        dot_ext(omega, &values)
    };
    #[cfg(feature = "parallel")]
    return (0..N).into_par_iter().map(collapse).collect();
    #[cfg(not(feature = "parallel"))]
    (0..N).map(collapse).collect()
}

fn fold(table: &mut Vec<Ext>, challenge: Ext) {
    for j in 0..table.len() / 2 {
        table[j] = table[2 * j].add(challenge.mul(table[2 * j + 1].sub(table[2 * j])));
    }
    table.truncate(table.len() / 2);
}

fn outer_round(weights: &[Ext], tables: &[Vec<Ext>; 4]) -> [Ext; 4] {
    let mut polynomial = [Ext::ZERO; 4];
    for i in (0..weights.len()).step_by(2) {
        let value: [Ext; 4] = std::array::from_fn(|j| tables[j][i]);
        let delta: [Ext; 4] = std::array::from_fn(|j| tables[j][i + 1].sub(value[j]));
        let q = [
            value[0].sub(value[1].mul(value[2])).sub(value[3]),
            delta[0]
                .sub(value[1].mul(delta[2]))
                .sub(delta[1].mul(value[2]))
                .sub(delta[3]),
            Ext::ZERO.sub(delta[1].mul(delta[2])),
        ];
        let w = [weights[i], weights[i + 1].sub(weights[i])];
        for a in 0..2 {
            for b in 0..3 {
                polynomial[a + b] = polynomial[a + b].add(w[a].mul(q[b]));
            }
        }
    }
    polynomial
}

fn inner_round(left: &[Ext], right: &[Ext]) -> [Ext; 3] {
    let mut polynomial = [Ext::ZERO; 3];
    for i in (0..left.len()).step_by(2) {
        let a = [left[i], left[i + 1].sub(left[i])];
        let b = [right[i], right[i + 1].sub(right[i])];
        for x in 0..2 {
            for y in 0..2 {
                polynomial[x + y] = polynomial[x + y].add(a[x].mul(b[y]));
            }
        }
    }
    polynomial
}

fn round_challenge<const K: usize>(
    transcript: &mut impl Transcript,
    index: usize,
    polynomial: &[Ext; K],
    claim: Ext,
) -> Result<(Ext, Ext), FalconError> {
    if polynomial[0].add(eval_ext(polynomial, Ext::ONE)) != claim {
        return Err(error("shared ring sumcheck round identity"));
    }
    transcript.absorb_slice(&(index as u64).to_le_bytes());
    absorb_extensions(transcript, polynomial);
    let point = sample_extension(transcript, false)?;
    Ok((point, eval_ext(polynomial, point)))
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:outer_sumcheck")]
fn prove_outer(
    transcript: &mut impl Transcript,
    mut weights: Vec<Ext>,
    mut tables: [Vec<Ext>; 4],
    mut claim: Ext,
) -> Result<(Vec<[Ext; 4]>, Vec<Ext>, [Ext; 4]), FalconError> {
    transcript.absorb_slice(b"outer-cubic");
    let mut proof = Vec::new();
    let mut point = Vec::new();
    while weights.len() > 1 {
        let polynomial = outer_round(&weights, &tables);
        let (challenge, next) = round_challenge(transcript, proof.len(), &polynomial, claim)?;
        fold(&mut weights, challenge);
        for table in &mut tables {
            fold(table, challenge);
        }
        proof.push(polynomial);
        point.push(challenge);
        claim = next;
    }
    let evaluations = tables.map(|table| table[0]);
    let terminal = weights[0].mul(
        evaluations[0]
            .sub(evaluations[1].mul(evaluations[2]))
            .sub(evaluations[3]),
    );
    if terminal != claim {
        return Err(error("shared ring outer prover terminal"));
    }
    Ok((proof, point, evaluations))
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:inner_sumcheck")]
fn prove_inner(
    transcript: &mut impl Transcript,
    mut left: Vec<Ext>,
    mut right: Vec<Ext>,
    mut claim: Ext,
) -> Result<(Vec<[Ext; 3]>, Vec<Ext>, Ext), FalconError> {
    transcript.absorb_slice(b"inner-quadratic");
    let mut proof = Vec::new();
    let mut point = Vec::new();
    while left.len() > 1 {
        let polynomial = inner_round(&left, &right);
        let (challenge, next) = round_challenge(transcript, proof.len(), &polynomial, claim)?;
        fold(&mut left, challenge);
        fold(&mut right, challenge);
        proof.push(polynomial);
        point.push(challenge);
        claim = next;
    }
    if left[0].mul(right[0]) != claim {
        return Err(error("shared ring inner prover terminal"));
    }
    Ok((proof, point, right[0]))
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:verify_sumcheck", fields(degree = K - 1))]
fn verify_sumcheck<const K: usize>(
    transcript: &mut impl Transcript,
    label: &[u8],
    proof: &[[Ext; K]],
    mut claim: Ext,
) -> Result<(Vec<Ext>, Ext), FalconError> {
    transcript.absorb_slice(label);
    let mut point = Vec::with_capacity(proof.len());
    for (index, polynomial) in proof.iter().enumerate() {
        let (challenge, next) = round_challenge(transcript, index, polynomial, claim)?;
        point.push(challenge);
        claim = next;
    }
    Ok((point, claim))
}

fn initial_after_rounds<const K: usize>(proof: &[[Ext; K]], point: &[Ext], initial: Ext) -> Ext {
    match (proof.last(), point.last()) {
        (Some(polynomial), Some(&r)) => eval_ext(polynomial, r),
        _ => initial,
    }
}

fn check_outer_terminal(
    claim: Ext,
    r: &[Ext],
    t: &[Ext],
    evaluations: &[Ext; 4],
) -> Result<(), FalconError> {
    let equality = r.iter().zip(t).fold(Ext::ONE, |value, (&r, &t)| {
        value.mul(r.mul(t).add(Ext::ONE.sub(r).mul(Ext::ONE.sub(t))))
    });
    let residual = evaluations[0]
        .sub(evaluations[1].mul(evaluations[2]))
        .sub(evaluations[3]);
    if claim != equality.mul(residual) {
        return Err(error("shared ring outer terminal identity"));
    }
    Ok(())
}

fn check_inner_terminal(claim: Ext, beta: Ext, u: &[Ext], witness: Ext) -> Result<(), FalconError> {
    let mut power = beta;
    let mut public = Ext::ONE;
    for &coordinate in u {
        public = public.mul(Ext::ONE.sub(coordinate).add(coordinate.mul(power)));
        power = power.mul(power);
    }
    if claim != public.mul(witness) {
        return Err(error("shared ring inner terminal identity"));
    }
    Ok(())
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:bit_query")]
fn bit_query(
    layout: &FalconSourceLayout,
    row: &[Ext],
    point: &[Ext],
    omega: &[Ext; 4],
) -> Result<(Vec<Ext>, Ext), FalconError> {
    let offsets = layout.offsets();
    let key = layout
        .public_key_offset()
        .ok_or_else(|| error("missing shared public-key source"))?;
    let positions = equality_weights(point);
    let mut column = vec![Ext::ZERO; layout.signature_stride()];
    for (j, weight) in positions.into_iter().enumerate() {
        let c = weight.mul(omega[0]);
        let h = weight.mul(omega[1]);
        let s2 = weight.mul(omega[2]);
        let s1 = weight.mul(omega[3]);
        for bit in 0..14 {
            column[offsets.hash_point + 14 * j + bit] = scale(c, 1 << bit);
            column[key + 14 * j + bit] = scale(h, 1 << bit);
            column[offsets.s1 + 14 * j + bit] = scale(s1, if bit == 13 { 4097 } else { 1 << bit });
        }
        for bit in 0..12 {
            let stream = 12 * j + 11 - bit;
            let slot =
                offsets.encoded_signature + 8 * (1 + NONCE_BYTES + stream / 8) + 7 - stream % 8;
            column[slot] = scale(s2, if bit == 11 { -2048 } else { 1 << bit });
        }
    }
    let row_sum = row.iter().fold(Ext::ZERO, |sum, &v| sum.add(v));
    // Position equality weights sum to one. Only S1 has an affine offset.
    let offset = scale(row_sum.mul(omega[3]), -6144);
    Ok((column, offset))
}

/// Group eight canonical coordinate vectors by their witness byte before
/// multiplying by the row lift. This avoids a degree-20 convolution per bit.
#[tracing::instrument(skip_all, name = "falcon_shared_ring:integer_lift")]
fn grouped_lift(
    source: &FalconSourceWitness,
    row: &[Ext],
    column: &[Ext],
) -> [i128; LIFT_COEFFICIENTS] {
    let layout = source.layout();
    let mut sums = vec![[0u64; EXTENSION_DEGREE]; layout.batch()];
    for (byte, values) in column.chunks_exact(8).enumerate() {
        if values.iter().all(|&value| value == Ext::ZERO) {
            continue;
        }
        let low = nibble_sums(&values[..4]);
        let high = nibble_sums(&values[4..]);
        for (s, sum) in sums.iter_mut().enumerate() {
            let flat = s * layout.signature_stride() + 8 * byte;
            let r = flat & ((1 << layout.row_vars()) - 1);
            let word = source.rows()[flat >> layout.row_vars()][r / 64];
            let bits = ((word >> (r % 64)) & 255) as usize;
            if bits != 0 {
                for a in 0..EXTENSION_DEGREE {
                    sum[a] += low[bits & 15][a] + high[bits >> 4][a];
                }
            }
        }
    }
    let mut result = [0i128; LIFT_COEFFICIENTS];
    for (a, sum) in row.iter().zip(sums) {
        for i in 0..EXTENSION_DEGREE {
            for j in 0..EXTENSION_DEGREE {
                result[i + j] += i128::from(a.0[i]) * i128::from(sum[j]);
            }
        }
    }
    result
}

fn nibble_sums(values: &[Ext]) -> [[u64; EXTENSION_DEGREE]; 16] {
    let mut sums = [[0; EXTENSION_DEGREE]; 16];
    for bits in 1usize..16 {
        let bit = bits.trailing_zeros() as usize;
        let previous = bits & (bits - 1);
        for a in 0..EXTENSION_DEGREE {
            sums[bits][a] = sums[previous][a] + u64::from(values[bit].0[a]);
        }
    }
    sums
}

fn lift_bound(layout: &FalconSourceLayout) -> u128 {
    EXTENSION_DEGREE as u128 * layout.source_bits() as u128 * (Q as u128 - 1).pow(2)
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:lift_read_off")]
fn check_lift(
    layout: &FalconSourceLayout,
    lift: &[i128; LIFT_COEFFICIENTS],
    target: Ext,
) -> Result<(), FalconError> {
    if lift
        .iter()
        .any(|value| value.unsigned_abs() > lift_bound(layout))
    {
        return Err(error("shared ring integer lift magnitude"));
    }
    let mut reduced = lift.map(|value| value.rem_euclid(i128::from(Q)) as i64);
    for j in EXTENSION_DEGREE..LIFT_COEFFICIENTS {
        reduced[j - EXTENSION_DEGREE] -= 14 * reduced[j];
        reduced[j - EXTENSION_DEGREE + 1] -= reduced[j];
    }
    let actual = Ext(std::array::from_fn(|j| reduced[j].rem_euclid(Q) as u16));
    if actual != target {
        return Err(error("shared ring integer lift extension read-off"));
    }
    Ok(())
}

fn absorb_lift(transcript: &mut impl Transcript, lift: &[i128; LIFT_COEFFICIENTS]) {
    transcript.absorb_slice(b"canonical-integer-lift-i128le21");
    for coefficient in lift {
        transcript.absorb_slice(&coefficient.to_le_bytes());
    }
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:sample_prime")]
fn sample_shared_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
) -> Result<Cfg, FalconError> {
    transcript.absorb_slice(b"shared-prime-after-ring-lift");
    let field = crate::prime_sampling::sample_prime_context(transcript, PRIME_MIN, PRIME_MAX, 128)
        .map_err(|e| error(e.to_string()))?;
    if field.modulus_u128() <= 2 * lift_bound(layout)
        || field.modulus_u128() <= SOURCE_RESIDUAL_BOUND
    {
        return Err(error("shared ring sampled prime is too small"));
    }
    Ok(field)
}

#[tracing::instrument(skip_all, name = "falcon_shared_ring:project")]
fn project(
    row: &[Ext],
    column: &[Ext],
    lift: &[i128; LIFT_COEFFICIENTS],
    alpha: F,
    field: &Cfg,
) -> ProjectedClaim {
    // Fourteen-bit coordinate window tables amortize projection over all columns.
    const LOW: usize = 256;
    const HIGH: usize = (Q as usize - 1) / LOW + 1;
    const WINDOW: usize = LOW + HIGH;
    let mut tables = vec![field.zero(); EXTENSION_DEGREE * WINDOW];
    let mut power = field.one();
    for table in tables.chunks_exact_mut(WINDOW) {
        for i in 1..LOW {
            table[i] = field.add(&table[i - 1], &power);
        }
        let step = field.add(&table[LOW - 1], &power);
        for i in 1..HIGH {
            table[LOW + i] = field.add(&table[LOW + i - 1], &step);
        }
        power = field.mul(&power, &alpha);
    }
    let evaluate = |value: &Ext| {
        value
            .0
            .iter()
            .zip(tables.chunks_exact(WINDOW))
            .fold(field.zero(), |sum, (&c, table)| {
                field.add(
                    &sum,
                    &field.add(
                        &table[usize::from(c) & 255],
                        &table[LOW + (usize::from(c) >> 8)],
                    ),
                )
            })
    };
    #[cfg(feature = "parallel")]
    let column = column.par_iter().map(evaluate).collect();
    #[cfg(not(feature = "parallel"))]
    let column = column.iter().map(evaluate).collect();
    let row = row.iter().map(evaluate).collect();
    let target = lift.iter().rev().fold(field.zero(), |sum, &value| {
        let magnitude = F::from_with_cfg(value.unsigned_abs(), field);
        let coefficient = if value < 0 {
            field.neg(&magnitude)
        } else {
            magnitude
        };
        field.add(&field.mul(&sum, &alpha), &coefficient)
    });
    ProjectedClaim {
        row,
        column,
        target,
    }
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
    const MSG: &[u8] = include_bytes!("fixtures/message.bin");
    const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");

    fn fixture(
        batch: usize,
    ) -> (
        FalconSourceLayout,
        FalconPublicStatement,
        Vec<FalconVerificationTrace>,
        FalconSourceWitness,
    ) {
        let trace = super::super::verification_trace(PK, MSG, SIG).unwrap();
        let traces = vec![trace; batch];
        let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
        let statement = FalconPublicStatement::from_bytes(
            &vec![PK; batch],
            &vec![MSG; batch],
            &vec![SIG; batch],
        )
        .unwrap();
        let source =
            FalconSourceWitness::from_traces(layout, &vec![MSG; batch], &vec![SIG; batch], &traces)
                .unwrap();
        (layout, statement, traces, source)
    }

    fn element(seed: usize) -> Ext {
        Ext(std::array::from_fn(|i| {
            ((133 * seed + 733 * i + 19 * seed * i) % Q as usize) as u16
        }))
    }

    fn source_dot(claim: &ProjectedClaim, source: &FalconSourceWitness, field: &Cfg) -> F {
        let layout = source.layout();
        let per_signature = layout.signature_stride() >> layout.row_vars();
        source
            .rows()
            .chunks_exact(per_signature)
            .zip(&claim.row)
            .fold(field.zero(), |sum, (columns, row)| {
                let mut local = field.zero();
                for (column, words) in columns.iter().enumerate() {
                    for (word, &value) in words.iter().enumerate() {
                        let mut bits = value;
                        while bits != 0 {
                            let bit = bits.trailing_zeros() as usize;
                            let index = (column << layout.row_vars()) + 64 * word + bit;
                            local = field.add(&local, &claim.column[index]);
                            bits &= bits - 1;
                        }
                    }
                }
                field.add(&sum, &field.mul(row, &local))
            })
    }

    #[test]
    fn shared_ring_roundtrips_both_targets_and_padded_batches() {
        for (batch, target) in [(1, 100), (3, 100), (5, 100), (1, 128)] {
            let (layout, statement, traces, source) = fixture(batch);
            let mut pt = Blake3Transcript::new();
            let (proof, pf, claim) =
                prove(&mut pt, &layout, &statement, &traces, &source, target).unwrap();
            let mut vt = Blake3Transcript::new();
            let (vf, verified) = verify(&mut vt, &layout, &statement, &proof, target).unwrap();
            assert_eq!(pf.modulus_u128(), vf.modulus_u128());
            assert!((PRIME_MIN..=PRIME_MAX).contains(&pf.modulus_u128()));
            assert_eq!(claim, verified);
            assert_eq!(claim.target, source_dot(&claim, &source, &pf));
            assert!(claim.row[batch..].iter().all(|&v| v == pf.zero()));
            assert_eq!(
                sample_extension(&mut pt, false).unwrap(),
                sample_extension(&mut vt, false).unwrap()
            );
            assert_eq!(proof.outer.len(), layout.capacity().ilog2() as usize);
            assert_eq!(proof.inner.len(), INNER_ROUNDS);
        }
    }

    #[test]
    fn shared_ring_rejects_malformed_and_inconsistent_messages() {
        let (layout, statement, traces, source) = fixture(3);
        let (proof, _, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &statement,
            &traces,
            &source,
            100,
        )
        .unwrap();
        let reject = |bad: &Proof| {
            assert!(verify(&mut Blake3Transcript::new(), &layout, &statement, bad, 100).is_err());
        };
        let mut bad = proof.clone();
        bad.certificate.pop();
        reject(&bad);
        let mut bad = proof.clone();
        bad.certificate[0].0[0] = Q as u16;
        reject(&bad);
        let mut bad = proof.clone();
        bad.certificate[0] = bad.certificate[0].add(Ext::ONE);
        reject(&bad);
        let mut bad = proof.clone();
        bad.outer.push([Ext::ZERO; 4]);
        reject(&bad);
        let mut bad = proof.clone();
        bad.outer[0][1] = bad.outer[0][1].add(Ext::ONE);
        reject(&bad);
        for operand in 0..4 {
            let mut bad = proof.clone();
            bad.evaluations[operand] = bad.evaluations[operand].add(Ext::ONE);
            reject(&bad);
        }
        let mut bad = proof.clone();
        bad.inner.pop();
        reject(&bad);
        let mut bad = proof.clone();
        bad.inner[0][0].0[10] = Q as u16;
        reject(&bad);
        let mut bad = proof.clone();
        bad.inner[0][2] = bad.inner[0][2].add(Ext::ONE);
        reject(&bad);
        let mut bad = proof.clone();
        bad.inner_evaluation = bad.inner_evaluation.add(Ext::ONE);
        reject(&bad);
        let mut bad = proof.clone();
        bad.lift[20] += 1;
        reject(&bad);
        let mut bad = proof.clone();
        bad.lift[0] = i128::MIN;
        reject(&bad);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &statement,
                &proof,
                128
            )
            .is_err()
        );
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &FalconSourceLayout::new(3).unwrap(),
                &statement,
                &proof,
                100
            )
            .is_err()
        );
    }

    #[test]
    fn extension_sumcheck_rounds_and_position_order_match_direct_tables() {
        let weights: Vec<_> = (0..8).map(element).collect();
        let tables: [Vec<Ext>; 4] =
            std::array::from_fn(|j| (0..8).map(|i| element(9 * j + i + 3)).collect());
        let outer = outer_round(&weights, &tables);
        let inner = inner_round(&tables[0], &tables[1]);
        for z in [Ext::ZERO, Ext::ONE, element(117)] {
            let at = |table: &[Ext], pair: usize| {
                table[2 * pair]
                    .mul(Ext::ONE.sub(z))
                    .add(table[2 * pair + 1].mul(z))
            };
            let expected_outer = (0..4).fold(Ext::ZERO, |sum, pair| {
                let residual = at(&tables[0], pair)
                    .sub(at(&tables[1], pair).mul(at(&tables[2], pair)))
                    .sub(at(&tables[3], pair));
                sum.add(at(&weights, pair).mul(residual))
            });
            let expected_inner = (0..4).fold(Ext::ZERO, |sum, pair| {
                sum.add(at(&tables[0], pair).mul(at(&tables[1], pair)))
            });
            assert_eq!(eval_ext(&outer, z), expected_outer);
            assert_eq!(eval_ext(&inner, z), expected_inner);
        }
        let beta = element(213);
        let point: Vec<_> = (0..INNER_ROUNDS).map(|i| element(i + 55)).collect();
        let table = powers(beta);
        let expected = (0..N).fold(Ext::ZERO, |sum, j| {
            let eq = point.iter().enumerate().fold(Ext::ONE, |p, (bit, &u)| {
                p.mul(if j >> bit & 1 == 0 {
                    Ext::ONE.sub(u)
                } else {
                    u
                })
            });
            sum.add(eq.mul(table[j]))
        });
        check_inner_terminal(expected, beta, &point, Ext::ONE).unwrap();
        let mut reversed = point;
        reversed.reverse();
        assert!(check_inner_terminal(expected, beta, &reversed, Ext::ONE).is_err());
    }

    #[test]
    fn decoder_tensor_and_grouped_integer_lift_match_original_source_bits() {
        let (layout, _, traces, source) = fixture(3);
        let row = vec![element(4), element(8), element(12), Ext::ZERO];
        let point: Vec<_> = (0..INNER_ROUNDS).map(|i| element(44 + i)).collect();
        let omega = operand_weights(element(31));
        let (column, offset) = bit_query(&layout, &row, &point, &omega).unwrap();
        let collapsed = collapse_positions(&traces, &row, &omega);
        let expected = dot_ext(&collapsed, &equality_weights(&point)).sub(offset);
        let lift = grouped_lift(&source, &row, &column);
        check_lift(&layout, &lift, expected).unwrap();
        // Independently convolve a sparse subset, crossing physical word/byte boundaries.
        let mut sparse = vec![Ext::ZERO; layout.signature_stride()];
        let offsets = layout.offsets();
        for index in [
            offsets.hash_point,
            offsets.hash_point + 63,
            offsets.s1 + 13,
            offsets.s1 + 64,
            offsets.encoded_signature + 329,
            layout.public_key_offset().unwrap() + 13,
        ] {
            sparse[index] = element(index);
        }
        let grouped = grouped_lift(&source, &row, &sparse);
        let mut direct = [0i128; LIFT_COEFFICIENTS];
        for (s, a) in row.iter().enumerate() {
            for (h, b) in sparse.iter().enumerate().filter(|(_, b)| **b != Ext::ZERO) {
                if source.bit(s * layout.signature_stride() + h) {
                    for i in 0..EXTENSION_DEGREE {
                        for j in 0..EXTENSION_DEGREE {
                            direct[i + j] += i128::from(a.0[i]) * i128::from(b.0[j]);
                        }
                    }
                }
            }
        }
        assert_eq!(grouped, direct);
        assert!(
            lift.iter()
                .all(|&c| c >= 0 && c as u128 <= lift_bound(&layout))
        );
    }

    #[test]
    fn coordinate_window_projection_matches_integer_horner() {
        let field = field::FpCtx::from_prime_u128((1 << 127) - 1);
        let alpha = F::from_with_cfg(912345678u128, &field);
        let row = [element(1), element(2)];
        let column = [element(3), element(4), element(5)];
        let lift = std::array::from_fn(|i| {
            if i % 2 == 0 {
                (i as i128 + 1) * 91
            } else {
                -(i as i128 + 1) * 117
            }
        });
        let projected = project(&row, &column, &lift, alpha, &field);
        let oracle = |value: &Ext| {
            value.0.iter().rev().fold(field.zero(), |sum, &v| {
                field.add(
                    &field.mul(&sum, &alpha),
                    &F::from_with_cfg(u128::from(v), &field),
                )
            })
        };
        assert_eq!(projected.row, row.iter().map(oracle).collect::<Vec<_>>());
        assert_eq!(
            projected.column,
            column.iter().map(oracle).collect::<Vec<_>>()
        );
        let mut expected = field.zero();
        let mut power = field.one();
        for value in lift {
            let term = field.mul(&power, &F::from_with_cfg(value.unsigned_abs(), &field));
            expected = if value < 0 {
                field.sub(&expected, &term)
            } else {
                field.add(&expected, &term)
            };
            power = field.mul(&power, &alpha);
        }
        assert_eq!(projected.target, expected);
    }

    /// Diagnostic only: compare the column kernels serially, including output
    /// allocation and excluding their small, precomputed public tables.
    #[test]
    #[ignore = "projection kernel timings; run release on the benchmark host"]
    fn shared_ring_projection_kernel_benchmark() {
        #[cfg(feature = "parallel")]
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(projection_kernel_benchmark);
        #[cfg(not(feature = "parallel"))]
        projection_kernel_benchmark();
    }

    fn projection_kernel_benchmark() {
        use field::{FpLinearAcc, Reduce, Uint};
        use std::{hint::black_box, time::Instant};

        const LOW: usize = 256;
        const HIGH: usize = (Q as usize - 1) / LOW + 1;
        const WINDOW: usize = LOW + HIGH;
        const ITERATIONS: usize = 20;

        fn window<const SKIP_ZERO: bool, const DELAYED: bool>(
            column: &[Ext],
            tables: &[F],
            field: &Cfg,
        ) -> Vec<F> {
            column
                .iter()
                .map(|value| {
                    if SKIP_ZERO && *value == Ext::ZERO {
                        return field.zero();
                    }
                    if DELAYED {
                        // Exactly 22 field-by-integer-one terms retain scale R.
                        // The typed reduction takes a remainder, not an R^-1 REDC.
                        let mut sum = FpLinearAcc::<2, 1>::default();
                        let one = Uint::from_words([1]);
                        for (&coordinate, table) in value.0.iter().zip(tables.chunks_exact(WINDOW))
                        {
                            sum.accumulate(&table[usize::from(coordinate) & 255], &one);
                            sum.accumulate(&table[LOW + (usize::from(coordinate) >> 8)], &one);
                        }
                        field.reduce(sum)
                    } else {
                        value.0.iter().zip(tables.chunks_exact(WINDOW)).fold(
                            field.zero(),
                            |sum, (&coordinate, table)| {
                                field.add(
                                    &sum,
                                    &field.add(
                                        &table[usize::from(coordinate) & 255],
                                        &table[LOW + (usize::from(coordinate) >> 8)],
                                    ),
                                )
                            },
                        )
                    }
                })
                .collect()
        }

        fn coordinates(column: &[Ext], powers: &[F], field: &Cfg) -> Vec<F> {
            column
                .iter()
                .map(|value| {
                    if *value == Ext::ZERO {
                        return field.zero();
                    }
                    // Eleven field-by-u14 terms are within the accumulator's
                    // fewer-than-2^64-term capacity and retain scale R.
                    let mut sum = FpLinearAcc::<2, 1>::default();
                    for (&coordinate, power) in value.0.iter().zip(powers) {
                        sum.accumulate(power, &Uint::from_words([u64::from(coordinate)]));
                    }
                    field.reduce(sum)
                })
                .collect()
        }

        fn measure(mut kernel: impl FnMut() -> Vec<F>) -> f64 {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                black_box(kernel());
            }
            start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64
        }

        let layout = FalconSourceLayout::new_shared_prime(32).unwrap();
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"shared-ring-projection-kernel-benchmark/v1");
        let signature_point = sample_point(&mut transcript, b"signature-point", 5).unwrap();
        let row = equality_weights(&signature_point);
        let position_point =
            sample_point(&mut transcript, b"position-point", INNER_ROUNDS).unwrap();
        let omega = operand_weights(challenge(&mut transcript, b"operand-batching").unwrap());
        let (column, _) = bit_query(&layout, &row, &position_point, &omega).unwrap();
        let field = sample_shared_field(&mut transcript, &layout).unwrap();
        let alpha = squeeze_field(&mut transcript, &field).unwrap();
        let mut powers = vec![field.one(); EXTENSION_DEGREE];
        for i in 1..EXTENSION_DEGREE {
            powers[i] = field.mul(&powers[i - 1], &alpha);
        }
        let mut tables = vec![field.zero(); EXTENSION_DEGREE * WINDOW];
        for (table, power) in tables.chunks_exact_mut(WINDOW).zip(&powers) {
            for i in 1..LOW {
                table[i] = field.add(&table[i - 1], power);
            }
            let step = field.add(&table[LOW - 1], power);
            for i in 1..HIGH {
                table[LOW + i] = field.add(&table[LOW + i - 1], &step);
            }
        }

        let expected = project(&row, &column, &[0; LIFT_COEFFICIENTS], alpha, &field).column;
        for (index, value) in column.iter().enumerate() {
            let horner = value.0.iter().rev().fold(field.zero(), |sum, &v| {
                field.add(
                    &field.mul(&sum, &alpha),
                    &F::from_with_cfg(u128::from(v), &field),
                )
            });
            assert_eq!(expected[index], horner, "production projection at {index}");
        }
        for (name, actual) in [
            (
                "current_window",
                window::<false, false>(&column, &tables, &field),
            ),
            (
                "zero_skip_window",
                window::<true, false>(&column, &tables, &field),
            ),
            (
                "zero_skip_lookup_acc",
                window::<true, true>(&column, &tables, &field),
            ),
            (
                "zero_skip_coordinate_acc",
                coordinates(&column, &powers, &field),
            ),
        ] {
            for (index, (a, b)) in actual.iter().zip(&expected).enumerate() {
                assert_eq!(a, b, "{name} at {index}");
            }
        }

        let current = measure(|| {
            window::<false, false>(black_box(&column), black_box(&tables), black_box(&field))
        });
        let skipped = measure(|| {
            window::<true, false>(black_box(&column), black_box(&tables), black_box(&field))
        });
        let lookup_acc = measure(|| {
            window::<true, true>(black_box(&column), black_box(&tables), black_box(&field))
        });
        let coordinate_acc =
            measure(|| coordinates(black_box(&column), black_box(&powers), black_box(&field)));
        eprintln!(
            "shared_ring_projection_kernel_benchmark arch={} columns={} active={} iterations={} threads=1",
            std::env::consts::ARCH,
            column.len(),
            column.iter().filter(|&&value| value != Ext::ZERO).count(),
            ITERATIONS,
        );
        for (name, elapsed) in [
            ("current_window", current),
            ("zero_skip_window", skipped),
            ("zero_skip_lookup_acc", lookup_acc),
            ("zero_skip_coordinate_acc", coordinate_acc),
        ] {
            eprintln!(
                "{name}: mean_ms={elapsed:.6} ratio_to_current={:.6}",
                elapsed / current
            );
        }
    }
}
