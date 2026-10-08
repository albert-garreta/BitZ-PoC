use super::{COEFFICIENT_LOG, SIGNATURE_BITS};
// Four committed Falcon operands reduced to one tensor query over the shared
// arithmetic prime. The returned query still requires the source-bit binder.
//
// The ring certificate is encoded by its unique quotient D of degree at most N-2, so
// e(Y)=(Y^N+1)D(Y) has the required degree and ideal membership by construction.
// The later (2k-1)-coefficient polynomial is an unreduced integer lift, not D.

use field::{BatchMulAcc, FpLinearAcc, FpSignedLinearAcc, Reduce, RingOps, Uint};
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
    FalconVerificationTrace, N, Q,
    constraints::SOURCE_RESIDUAL_BOUND,
    ring_field::{
        EXTENSION_DEGREE, Ext, absorb_extensions, certificate, equality_weights, sample_extension,
    },
};

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

const LIFT_COEFFICIENTS: usize = 2 * EXTENSION_DEGREE - 1;
const DOMAIN: &[u8] = b"bitz/falcon1024-ct/shared-ring/v5";

/// Target 100 uses one unsplit bounded exponent. Target 128 retains the
/// native prime family and its two bounded bridge limbs.
pub(super) fn prime_bounds(target_bits: usize) -> Result<(u128, u128), FalconError> {
    match target_bits {
        100 => Ok((1 << 114, (1 << 115) - (1 << 102) - 1)),
        128 => Ok((1u128 << 125, (1u128 << 126) - 1)),
        _ => Err(error("unsupported shared prime security target")),
    }
}

#[cfg(test)]
pub(super) fn prime_floor_bits(target_bits: usize) -> Result<usize, FalconError> {
    Ok(prime_bounds(target_bits)?.0.ilog2() as usize)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Proof {
    pub certificate: Vec<Ext>,
    pub outer: Vec<[Ext; 4]>,
    /// Transcript order: C, H, S2, S1. Batching powers are 0, 1, 3, 2.
    pub evaluations: [Ext; 4],
    pub lift: [i128; LIFT_COEFFICIENTS],
    pub projection_nonce: u64,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        2 * EXTENSION_DEGREE * (self.certificate.len() + 4 * self.outer.len() + 4)
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

/// Verifier recipe for the same canonical-coordinate projection as `ProjectedClaim`.
/// Only the small extension-field factors survive until the binding endpoint.
pub(super) struct VerifierProjectedClaim {
    row: Vec<Ext>,
    beta: Ext,
    omega: [Ext; 4],
    alpha: F,
    pub target: F,
}

impl VerifierProjectedClaim {
    #[tracing::instrument(skip_all, name = "falcon_shared_ring:project_endpoint")]
    pub(super) fn evaluate(
        &self,
        layout: &FalconSourceLayout,
        local: &[F],
        instances: &[F],
        field: &Cfg,
    ) -> Result<F, FalconError> {
        if local.len() != layout.signature_stride() || instances.len() != self.row.len() {
            return Err(error("projected ring endpoint dimensions"));
        }
        // Sum the canonical integer coordinates first, then evaluate at alpha.
        // This does NOT project an extension-field sum: reduction modulo q must
        // happen separately for every public bit coefficient before accumulation.
        let mut column = [FpLinearAcc::<2, 1>::default(); EXTENSION_DEGREE];
        let positions = powers(self.beta);
        visit_bit_query(layout, &positions, &self.omega, |slot, value| {
            for (sum, coordinate) in column.iter_mut().zip(value.0) {
                sum.accumulate(&local[slot], &Uint::from_words([u64::from(coordinate)]));
            }
        })?;
        let mut row = [FpLinearAcc::<2, 1>::default(); EXTENSION_DEGREE];
        for (value, weight) in self.row.iter().zip(instances) {
            for (sum, coordinate) in row.iter_mut().zip(value.0) {
                sum.accumulate(weight, &Uint::from_words([u64::from(coordinate)]));
            }
        }
        // At most 54*N terms per column coordinate: 126+14+16 < 192
        // bits, so the three-limb linear accumulator cannot overflow.
        let evaluate = |coordinates: [FpLinearAcc<2, 1>; EXTENSION_DEGREE]| {
            coordinates
                .into_iter()
                .rev()
                .fold(field.zero(), |sum, coordinate| {
                    field.add(&field.mul(&sum, &self.alpha), &field.reduce(coordinate))
                })
        };
        Ok(field.mul(&evaluate(row), &evaluate(column)))
    }
}

pub(super) const fn projection_grinding_bits(target_bits: usize) -> u32 {
    if target_bits == 128 { 14 } else { 2 }
}

struct ProjectionGrinding;
impl GrindingDomain for ProjectionGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/shared-ring/projection-grinding/v3";
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
    bind_parameters(transcript, layout, target_bits)?;
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
    // Transpose the coefficient decoder directly onto the committed bits.
    // The shared prime-field binder authenticates this tensor query later.
    let (column, offset) = bit_query(layout, &row, &powers, &omega)?;
    let target = tau.sub(offset);
    let lift = grouped_lift(source, &row, &column);
    check_lift(layout, &lift, target)?;
    absorb_lift(transcript, &lift);
    let field = sample_shared_field(transcript, layout, target_bits)?;
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
) -> Result<(Cfg, VerifierProjectedClaim), FalconError> {
    validate_context(layout, statement, target_bits)?;
    let rounds = layout.capacity().ilog2() as usize;
    if proof.certificate.len() != N - 1
        || proof.outer.len() != rounds
        || !proof.certificate.iter().all(|x| x.canonical())
        || !proof.outer.iter().flatten().all(|x| x.canonical())
        || !proof.evaluations.iter().all(|x| x.canonical())
    {
        return Err(error("invalid shared ring proof encoding"));
    }
    bind_parameters(transcript, layout, target_bits)?;
    let r = sample_point(transcript, b"instances", rounds)?;
    transcript.absorb_slice(b"quotient-certificate");
    absorb_extensions(transcript, &proof.certificate);
    let beta = challenge(transcript, b"polynomial-evaluation")?;
    // Only the polynomial power and its geometric sum are needed before the
    // deferred source endpoint. Doubling also covers beta = 0, 1, and -1.
    let (beta_n, position_sum) = polynomial_weights_summary(beta);
    let initial = eval_ext(&proof.certificate, beta).mul(Ext::ONE.add(beta_n));
    let (t, outer_claim) = verify_sumcheck(transcript, b"outer-cubic", &proof.outer, initial)?;
    transcript.absorb_slice(b"outer-evaluations-C-H-S2-S1");
    absorb_extensions(transcript, &proof.evaluations);
    check_outer_terminal(outer_claim, &r, &t, &proof.evaluations)?;
    let batching = challenge(transcript, b"operand-batching")?;
    let omega = operand_weights(batching);
    let tau = dot_ext(&omega, &proof.evaluations);
    let mut row = equality_weights(&t);
    row[layout.batch()..].fill(Ext::ZERO);
    let offset = decoder_offset(&row, position_sum, &omega);
    check_lift(layout, &proof.lift, tau.sub(offset))?;
    absorb_lift(transcript, &proof.lift);
    let field = sample_shared_field(transcript, layout, target_bits)?;
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
        VerifierProjectedClaim {
            row,
            beta,
            omega,
            alpha,
            target: project_lift(&proof.lift, alpha, &field),
        },
    ))
}

fn validate_context(
    layout: &FalconSourceLayout,
    statement: &FalconPublicStatement,
    target_bits: usize,
) -> Result<(), FalconError> {
    statement.validate(layout.batch())?;
    if !matches!(target_bits, 100 | 128) {
        return Err(error("unsupported shared ring layout or security target"));
    }
    let (prime_min, _) = prime_bounds(target_bits)?;
    if 2 * lift_bound(layout) >= prime_min || SOURCE_RESIDUAL_BOUND >= prime_min {
        return Err(error("shared ring bounds exceed the prime family"));
    }
    Ok(())
}

fn bind_parameters(
    t: &mut impl Transcript,
    layout: &FalconSourceLayout,
    target: usize,
) -> Result<(), FalconError> {
    let (prime_min, prime_max) = prime_bounds(target)?;
    t.absorb_slice(DOMAIN);
    for value in [
        Q as usize,
        N,
        EXTENSION_DEGREE,
        crate::piop::spartan::falcon_parameters::extension_constant(EXTENSION_DEGREE) as usize,
        SIGNATURE_BITS,
        // Preserve the established framing word after removing prefix counters.
        COEFFICIENT_LOG + 1,
        layout.batch(),
        layout.capacity(),
        layout.signature_stride(),
        layout.occupied_bits(),
        layout.public_key_offset(),
        layout.row_vars(),
        layout.col_vars(),
        target,
    ] {
        t.absorb_slice(&(value as u64).to_le_bytes());
    }
    t.absorb_slice(
        b"f:T^k+T+c;D:N-1;direct-beta-decoder;P:i128le(2k-1);live-mask;layout:aligned16;C:u14;H:u14;S1:bounded14-6144;S2:lsb-signed;aliases:sum-in-E",
    );
    t.absorb_slice(&lift_bound(layout).to_le_bytes());
    t.absorb_slice(&SOURCE_RESIDUAL_BOUND.to_le_bytes());
    t.absorb_slice(&prime_min.to_le_bytes());
    t.absorb_slice(&prime_max.to_le_bytes());
    t.absorb_slice(&projection_grinding_bits(target).to_le_bytes());
    Ok(())
}

fn challenge(t: &mut impl Transcript, label: &[u8]) -> Result<Ext, FalconError> {
    t.absorb_slice(label);
    // Every element of E is permitted, including zero and base-field elements.
    sample_extension(t)
}

fn sample_point(
    t: &mut impl Transcript,
    label: &[u8],
    len: usize,
) -> Result<Vec<Ext>, FalconError> {
    t.absorb_slice(label);
    t.absorb_slice(&(len as u64).to_le_bytes());
    (0..len).map(|_| sample_extension(t)).collect()
}

fn scale(value: Ext, scalar: i64) -> Ext {
    let scalar = scalar.rem_euclid(Q) as u64;
    Ext::new(value.0.map(|c| (u64::from(c) * scalar % Q as u64) as u16))
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

/// Return beta^N and sum_{j=0}^{N-1} beta^j without division.
fn polynomial_weights_summary(beta: Ext) -> (Ext, Ext) {
    let mut power = beta;
    let mut sum = Ext::ONE;
    for _ in 0..COEFFICIENT_LOG {
        sum = sum.mul(Ext::ONE.add(power));
        power = power.mul(power);
    }
    (power, sum)
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
        sums.map(|sum| Ext::new(sum.map(|c| (c % Q as u64) as u16)))
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
    let point = sample_extension(transcript)?;
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

#[tracing::instrument(skip_all, name = "falcon_shared_ring:bit_query")]
fn bit_query(
    layout: &FalconSourceLayout,
    row: &[Ext],
    positions: &[Ext],
    omega: &[Ext; 4],
) -> Result<(Vec<Ext>, Ext), FalconError> {
    if row.len() != layout.capacity() {
        return Err(error("shared ring signature weights shape"));
    }
    let mut column = vec![Ext::ZERO; layout.signature_stride()];
    visit_bit_query(layout, positions, omega, |slot, value| {
        column[slot] = column[slot].add(value);
    })?;
    let position_sum = positions
        .iter()
        .fold(Ext::ZERO, |sum, &value| sum.add(value));
    Ok((column, decoder_offset(row, position_sum, omega)))
}

fn decoder_offset(row: &[Ext], position_sum: Ext, omega: &[Ext; 4]) -> Ext {
    let row_sum = row.iter().fold(Ext::ZERO, |sum, &v| sum.add(v));
    // Only S1 has an affine offset. Polynomial weights need not sum to one.
    scale(row_sum.mul(omega[3]).mul(position_sum), -6144)
}

fn visit_bit_query(
    layout: &FalconSourceLayout,
    positions: &[Ext],
    omega: &[Ext; 4],
    visit: impl FnMut(usize, Ext),
) -> Result<(), FalconError> {
    let offsets = layout.offsets();
    let ranges = [
        offsets.hash_point..offsets.hash_point + 16 * N,
        offsets.public_key..offsets.public_key + 16 * N,
        offsets.s2..offsets.s2 + 16 * N,
        offsets.s1..offsets.s1 + 16 * N,
    ];
    visit_decoder_query(layout.signature_stride(), &ranges, positions, omega, visit)
}

/// Coalesce any decoder aliases in E before their canonical coordinates are
/// lifted. The usual four disjoint ranges stream without allocating a column.
fn visit_decoder_query(
    domain_len: usize,
    ranges: &[std::ops::Range<usize>; 4],
    positions: &[Ext],
    omega: &[Ext; 4],
    mut visit: impl FnMut(usize, Ext),
) -> Result<(), FalconError> {
    if positions.len() != N
        || ranges.iter().any(|range| {
            range.end > domain_len || range.end.checked_sub(range.start) != Some(16 * N)
        })
    {
        return Err(error("shared ring decoder query dimensions"));
    }
    let overlaps = (0..4).any(|a| {
        (a + 1..4).any(|b| ranges[a].start < ranges[b].end && ranges[b].start < ranges[a].end)
    });
    if overlaps {
        let mut column = vec![Ext::ZERO; domain_len];
        visit_decoder_terms(ranges, positions, omega, |slot, value| {
            column[slot] = column[slot].add(value);
        });
        for (slot, value) in column.into_iter().enumerate() {
            if value != Ext::ZERO {
                visit(slot, value);
            }
        }
    } else {
        // Every decoder visits distinct live slots in its range, so disjoint
        // ranges guarantee that these are already canonical slot sums.
        visit_decoder_terms(ranges, positions, omega, visit);
    }
    Ok(())
}

fn visit_decoder_terms(
    ranges: &[std::ops::Range<usize>; 4],
    positions: &[Ext],
    omega: &[Ext; 4],
    mut emit: impl FnMut(usize, Ext),
) {
    for (j, &weight) in positions.iter().enumerate() {
        let c = weight.mul(omega[0]);
        let h = weight.mul(omega[1]);
        let s2 = weight.mul(omega[2]);
        let s1 = weight.mul(omega[3]);
        for bit in 0..14 {
            emit(ranges[0].start + 16 * j + bit, scale(c, 1 << bit));
            emit(ranges[1].start + 16 * j + bit, scale(h, 1 << bit));
            emit(
                ranges[3].start + 16 * j + bit,
                scale(s1, if bit == 13 { 4097 } else { 1 << bit }),
            );
        }
        for bit in 0..SIGNATURE_BITS {
            let slot = ranges[2].start + 16 * j + bit;
            emit(
                slot,
                scale(
                    s2,
                    if bit == SIGNATURE_BITS - 1 {
                        -(1 << (SIGNATURE_BITS - 1))
                    } else {
                        1 << bit
                    },
                ),
            );
        }
    }
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
    let rows = |start: usize, end: usize| {
        let mut sums = vec![[0u64; EXTENSION_DEGREE]; end - start];
        for (byte, values) in column.chunks_exact(8).enumerate() {
            if values.iter().all(|&value| value == Ext::ZERO) {
                continue;
            }
            let low = nibble_sums(&values[..4]);
            let high = nibble_sums(&values[4..]);
            for (local, sum) in sums.iter_mut().enumerate() {
                let flat = (start + local) * layout.signature_stride() + 8 * byte;
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
        for (a, sum) in row[start..end].iter().zip(sums) {
            for i in 0..EXTENSION_DEGREE {
                for j in 0..EXTENSION_DEGREE {
                    result[i + j] += i128::from(a.0[i]) * i128::from(sum[j]);
                }
            }
        }
        result
    };
    #[cfg(feature = "parallel")]
    if layout.batch() >= 128 && rayon::current_num_threads() > 1 {
        // Reuse each byte's lookup tables across a block of signatures. Each
        // worker has a disjoint source-row range, and exact integer addition
        // combines their polynomials within the same global coefficient bound.
        let chunk = layout
            .batch()
            .div_ceil(rayon::current_num_threads())
            .max(64);
        return (0..layout.batch())
            .into_par_iter()
            .step_by(chunk)
            .map(|start| rows(start, (start + chunk).min(layout.batch())))
            .reduce(
                || [0; LIFT_COEFFICIENTS],
                |mut left, right| {
                    for (a, b) in left.iter_mut().zip(right) {
                        *a += b;
                    }
                    left
                },
            );
    }
    rows(0, layout.batch())
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
        reduced[j - EXTENSION_DEGREE] -= i64::from(
            crate::piop::spartan::falcon_parameters::extension_constant(EXTENSION_DEGREE),
        ) * reduced[j];
        reduced[j - EXTENSION_DEGREE + 1] -= reduced[j];
    }
    let actual = Ext::new(std::array::from_fn(|j| reduced[j].rem_euclid(Q) as u16));
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
    target_bits: usize,
) -> Result<Cfg, FalconError> {
    let (prime_min, prime_max) = prime_bounds(target_bits)?;
    transcript.absorb_slice(b"shared-prime-after-ring-lift");
    let field = crate::prime_sampling::sample_prime_context(transcript, prime_min, prime_max, 128)
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
    let mut powers = [field.one(); EXTENSION_DEGREE];
    for i in 1..EXTENSION_DEGREE {
        powers[i] = field.mul(&powers[i - 1], &alpha);
    }
    let evaluate = |value: &Ext| {
        if *value == Ext::ZERO {
            return field.zero();
        }
        // Public canonical coordinates are integers below q. Eleven field ×
        // u14 terms fit the accumulator and retain Montgomery scale R; reduce
        // once with the linear accumulator's remainder operation (not REDC).
        let mut sum = FpLinearAcc::<2, 1>::default();
        for (&coordinate, power) in value.0.iter().zip(&powers) {
            sum.accumulate(power, &Uint::from_words([u64::from(coordinate)]));
        }
        field.reduce(sum)
    };
    #[cfg(feature = "parallel")]
    let column = column.par_iter().map(evaluate).collect();
    #[cfg(not(feature = "parallel"))]
    let column = column.iter().map(evaluate).collect();
    let row = row.iter().map(evaluate).collect();
    ProjectedClaim {
        row,
        column,
        target: project_lift(lift, alpha, field),
    }
}

fn project_lift(lift: &[i128; LIFT_COEFFICIENTS], alpha: F, field: &Cfg) -> F {
    // At most 21 field × i128 terms fit the signed linear accumulator.
    // Its Montgomery scale is R, so only the final remainder is needed.
    let mut sum = FpSignedLinearAcc::<2, 2>::default();
    let mut power = field.one();
    for value in lift {
        field.mul_acc(&mut sum, &power, value);
        power = field.mul(&power, &alpha);
    }
    field.reduce(sum)
}

fn error(message: impl Into<String>) -> FalconError {
    FalconError::Piop(message.into())
}
falcon_tests! {
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
        let layout = FalconSourceLayout::new(batch).unwrap();
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
        Ext::new(std::array::from_fn(|i| {
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
            let (prime_min, prime_max) = prime_bounds(target).unwrap();
            assert!((prime_min..=prime_max).contains(&pf.modulus_u128()));
            assert_eq!(claim.target, verified.target);
            let point: Vec<F> = (0..layout.source_bits().ilog2())
                .map(|i| F::from_with_cfg(u128::from(i + 7), &pf))
                .collect();
            let split = layout.signature_stride().ilog2() as usize;
            let local = crate::piop::spartan::matrix::eq_table(&point[..split], &pf).unwrap();
            let instances = crate::piop::spartan::matrix::eq_table(&point[split..], &pf).unwrap();
            let dot = |a: &[F], b: &[F]| {
                a.iter()
                    .zip(b)
                    .fold(pf.zero(), |s, (x, y)| pf.add(&s, &pf.mul(x, y)))
            };
            assert_eq!(
                verified.evaluate(&layout, &local, &instances, &pf).unwrap(),
                pf.mul(&dot(&claim.row, &instances), &dot(&claim.column, &local))
            );
            assert_eq!(claim.target, source_dot(&claim, &source, &pf));
            assert!(claim.row[batch..].iter().all(|&v| v == pf.zero()));
            assert_eq!(
                sample_extension(&mut pt).unwrap(),
                sample_extension(&mut vt).unwrap()
            );
            assert_eq!(proof.outer.len(), layout.capacity().ilog2() as usize);
            assert_eq!(
                proof.payload_size_bytes(),
                2 * EXTENSION_DEGREE * (N - 1 + 4 * proof.outer.len() + 4)
                    + 16 * LIFT_COEFFICIENTS
                    + 8,
            );
        }
    }

    #[test]
    fn lazy_projection_matches_materialized_canonical_coordinates() {
        let (layout, _, _, _) = fixture(3);
        let field = F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap();
        let mut row = vec![element(17), element(41), element(99), Ext::ZERO];
        let beta = element(213);
        let positions = powers(beta);
        let omega = [element(7), element(8), element(9), element(10)];
        let (column, _) = bit_query(&layout, &row, &positions, &omega).unwrap();
        let dot = |a: &[F], b: &[F]| {
            a.iter()
                .zip(b)
                .fold(field.zero(), |s, (x, y)| field.add(&s, &field.mul(x, y)))
        };
        for alpha in [
            field.zero(),
            field.one(),
            field.neg(&field.one()),
            F::from_with_cfg(1729u128, &field),
        ] {
            let materialized = project(&row, &column, &[0; LIFT_COEFFICIENTS], alpha, &field);
            let lazy = VerifierProjectedClaim {
                row: row.clone(),
                beta,
                omega,
                alpha,
                target: field.zero(),
            };
            // Dense maximum representatives also exercise the delayed-reduction bound.
            let local = vec![field.neg(&field.one()); layout.signature_stride()];
            let instances = vec![field.neg(&field.one()); layout.capacity()];
            assert_eq!(
                lazy.evaluate(&layout, &local, &instances, &field).unwrap(),
                field.mul(
                    &dot(&materialized.row, &instances),
                    &dot(&materialized.column, &local)
                )
            );
            assert!(
                lazy.evaluate(&layout, &local[1..], &instances, &field)
                    .is_err()
            );
            for slot in [
                layout.s1_bit(0, 13),
                layout.public_key_bit(N - 1, 13),
                layout.s2_bit(0, 0),
                layout.signature_stride() - 1,
            ] {
                let mut local = vec![field.zero(); layout.signature_stride()];
                local[slot] = field.one();
                assert_eq!(
                    lazy.evaluate(&layout, &local, &instances, &field).unwrap(),
                    field.mul(
                        &dot(&materialized.row, &instances),
                        &materialized.column[slot]
                    )
                );
            }
        }
        row.fill(Ext::ZERO);
        let lazy = VerifierProjectedClaim {
            row,
            beta,
            omega,
            alpha: field.one(),
            target: field.zero(),
        };
        assert_eq!(
            lazy.evaluate(
                &layout,
                &vec![field.one(); layout.signature_stride()],
                &vec![field.one(); layout.capacity()],
                &field
            )
            .unwrap(),
            field.zero()
        );
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
        bad.outer[0][0].0[10] = Q as u16;
        reject(&bad);
        let mut bad = proof.clone();
        bad.evaluations[2].0[9] = Q as u16;
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
                &FalconSourceLayout::new(2).unwrap(),
                &statement,
                &proof,
                100
            )
            .is_err()
        );
    }

    #[test]
    fn extension_outer_sumcheck_rounds_match_direct_tables() {
        let weights: Vec<_> = (0..8).map(element).collect();
        let tables: [Vec<Ext>; 4] =
            std::array::from_fn(|j| (0..8).map(|i| element(9 * j + i + 3)).collect());
        let outer = outer_round(&weights, &tables);
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
            assert_eq!(eval_ext(&outer, z), expected_outer);
        }
    }

    #[test]
    fn polynomial_power_sum_and_decoder_offset_cover_zero_one_and_minus_one() {
        let row = [element(2), element(19), Ext::ZERO, element(7)];
        let omega = operand_weights(element(113));
        for beta in [Ext::ZERO, Ext::ONE, Ext::ZERO.sub(Ext::ONE), element(44)] {
            let positions = powers(beta);
            let (last, sum) = polynomial_weights_summary(beta);
            assert_eq!(last, positions[N - 1].mul(beta));
            assert_eq!(sum, positions.iter().fold(Ext::ZERO, |sum, &v| sum.add(v)));
            let expected = row.iter().fold(Ext::ZERO, |total, &weight| {
                positions.iter().fold(total, |total, &position| {
                    total.add(scale(weight.mul(position).mul(omega[3]), -6144))
                })
            });
            assert_eq!(decoder_offset(&row, sum, &omega), expected);
        }
        assert_eq!(
            polynomial_weights_summary(Ext::ONE).1,
            scale(Ext::ONE, N as i64)
        );
        assert_eq!(
            polynomial_weights_summary(Ext::ZERO.sub(Ext::ONE)).1,
            Ext::ZERO
        );
        assert_eq!(decoder_offset(&[Ext::ZERO; 4], Ext::ONE, &omega), Ext::ZERO);
    }

    #[test]
    fn direct_decoder_tensor_matches_raw_signed_words_and_masked_padding() {
        let layout = FalconSourceLayout::new(3).unwrap();
        let offsets = layout.offsets();
        let key = layout.public_key_offset();
        let stride = layout.signature_stride();
        // Padding deliberately contains nonzero bits: its vanishing row weight,
        // rather than an accidental all-zero fixture, must remove it.
        let mut bits = vec![false; layout.source_bits()];
        let mut decoded = vec![vec![[0i64; 4]; N]; layout.capacity()];
        for (s, coefficients) in decoded.iter_mut().enumerate() {
            let base = s * stride;
            for (j, coefficient) in coefficients.iter_mut().enumerate() {
                let unsigned = [0u16, 1, (Q - 1) as u16, Q as u16, 16383, 8192];
                let signed = [0u16, 1, 2047, 2048, 2049, 4095];
                let bounded = [0u16, 1, 8191, 8192, 16383, 4096];
                let c = unsigned[(j + s) % unsigned.len()];
                let h = unsigned[(3 * j + 2 * s + 1) % unsigned.len()];
                let s2 = signed[(5 * j + s + 2) % signed.len()];
                let s1 = bounded[(j + 3 * s + 4) % bounded.len()];
                for (offset, word) in [(offsets.hash_point, c), (key, h), (offsets.s1, s1)] {
                    for bit in 0..14 {
                        bits[base + offset + 16 * j + bit] = word >> bit & 1 != 0;
                    }
                }
                for digit in 0..SIGNATURE_BITS {
                    bits[base + offsets.s2 + 16 * j + digit] = s2 >> digit & 1 != 0;
                }
                *coefficient = [
                    i64::from(c),
                    i64::from(h),
                    i64::from(s2) - if s2 & 2048 != 0 { 4096 } else { 0 },
                    i64::from(s1 & 8191) + 4097 * i64::from(s1 >> 13) - 6144,
                ];
            }
        }
        let row = [element(4), element(8), element(12), Ext::ZERO];
        let mut mixtures = (0..4)
            .map(|operand| {
                let mut omega = [Ext::ZERO; 4];
                omega[operand] = Ext::ONE;
                omega
            })
            .collect::<Vec<_>>();
        mixtures.push(operand_weights(element(31)));
        for beta in [Ext::ZERO, Ext::ONE, Ext::ZERO.sub(Ext::ONE), element(57)] {
            let positions = powers(beta);
            let endpoints: [Ext; 4] = std::array::from_fn(|operand| {
                decoded
                    .iter()
                    .zip(row)
                    .fold(Ext::ZERO, |sum, (coefficients, weight)| {
                        let value = coefficients.iter().rev().fold(Ext::ZERO, |value, c| {
                            value.mul(beta).add(scale(Ext::ONE, c[operand]))
                        });
                        sum.add(weight.mul(value))
                    })
            });
            for omega in &mixtures {
                let (column, offset) = bit_query(&layout, &row, &positions, omega).unwrap();
                let slots: Vec<_> = column
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| **value != Ext::ZERO)
                    .collect();
                let actual = row.iter().enumerate().fold(offset, |sum, (s, &weight)| {
                    let local = slots.iter().fold(Ext::ZERO, |sum, &(slot, &value)| {
                        if bits[s * stride + slot] {
                            sum.add(value)
                        } else {
                            sum
                        }
                    });
                    sum.add(weight.mul(local))
                });
                assert_eq!(actual, dot_ext(omega, &endpoints));
                assert!(
                    column.iter().enumerate()
                        .all(|(slot, &value)| !layout.is_padding(slot) || value == Ext::ZERO)
                );
            }
        }
    }

    #[test]
    fn decoder_aliases_are_summed_in_extension_before_coordinate_projection() {
        let disjoint = [0..16 * N, 16 * N..32 * N, 32 * N..48 * N, 48 * N..64 * N];
        let aliases = [0..16 * N, 0..16 * N, 3..3 + 16 * N, 7..7 + 16 * N];
        let domain = 7 + 16 * N;
        let positions = powers(element(31));
        let omega = operand_weights(element(13));
        let mut expected = vec![Ext::ZERO; domain];
        visit_decoder_query(64 * N, &disjoint, &positions, &omega, |slot, value| {
            let operand = disjoint
                .iter()
                .position(|range| range.contains(&slot))
                .unwrap();
            let mapped = aliases[operand].start + slot - disjoint[operand].start;
            expected[mapped] = expected[mapped].add(value);
        })
        .unwrap();
        let mut actual = vec![Ext::ZERO; domain];
        let mut seen = vec![false; domain];
        visit_decoder_query(domain, &aliases, &positions, &omega, |slot, value| {
            assert!(
                !seen[slot],
                "a source slot must be projected only after all aliases are added"
            );
            seen[slot] = true;
            assert!(value.canonical());
            actual[slot] = value;
        })
        .unwrap();
        assert_eq!(actual, expected);

        // C + (-H) on identical slots is zero in E. Projecting their two
        // unreduced canonical representatives first would incorrectly give q.
        let omega = [Ext::ONE, Ext::ZERO.sub(Ext::ONE), Ext::ZERO, Ext::ZERO];
        let field = field::FpCtx::from_prime_u128((1 << 127) - 1);
        let mut projected = [FpLinearAcc::<2, 1>::default(); EXTENSION_DEGREE];
        visit_decoder_query(
            domain,
            &aliases,
            &vec![Ext::ONE; N],
            &omega,
            |slot, value| {
                let local = F::from_with_cfg(slot as u128 + 1, &field);
                for (sum, coordinate) in projected.iter_mut().zip(value.0) {
                    sum.accumulate(&local, &Uint::from_words([u64::from(coordinate)]));
                }
            },
        )
        .unwrap();
        assert!(
            projected
                .into_iter()
                .all(|coordinate| field.reduce(coordinate) == field.zero())
        );

        assert!(visit_decoder_query(domain, &aliases, &positions[1..], &omega, |_, _| {}).is_err());
        assert!(visit_decoder_query(domain - 1, &aliases, &positions, &omega, |_, _| {}).is_err());
        let mut reversed = aliases;
        reversed[0] = 1..0;
        assert!(visit_decoder_query(domain, &reversed, &positions, &omega, |_, _| {}).is_err());
    }

    #[test]
    fn decoder_tensor_and_grouped_integer_lift_match_original_source_bits() {
        let (layout, _, traces, source) = fixture(3);
        let row = vec![element(4), element(8), element(12), Ext::ZERO];
        let beta = element(44);
        let positions = powers(beta);
        let omega = operand_weights(element(31));
        let (column, offset) = bit_query(&layout, &row, &positions, &omega).unwrap();
        // Independently evaluate the four ordinary coefficient polynomials by
        // Horner, then interpolate signatures; no coefficient-domain MLE.
        let expected = traces
            .iter()
            .zip(&row)
            .fold(Ext::ZERO, |sum, (trace, &weight)| {
                let values: [Ext; 4] = std::array::from_fn(|operand| {
                    (0..N).rev().fold(Ext::ZERO, |value, j| {
                        let coefficient = match operand {
                            0 => i64::from(trace.hash_to_point.point[j]),
                            1 => i64::from(trace.public_key.h[j]),
                            2 => i64::from(trace.signature.s2[j]),
                            _ => i64::from(trace.s1[j]),
                        };
                        value.mul(beta).add(scale(Ext::ONE, coefficient))
                    })
                });
                sum.add(weight.mul(dot_ext(&omega, &values)))
            })
            .sub(offset);
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
            layout.s2_bit(1, 1),
            layout.public_key_offset() + 13,
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
    fn coordinate_projection_matches_integer_horner() {
        let field = field::FpCtx::from_prime_u128((1 << 127) - 1);
        let alpha = F::from_with_cfg(912345678u128, &field);
        let row = [element(1), element(2), Ext::ZERO];
        let column = [
            element(3),
            element(4),
            Ext::ZERO,
            Ext::new([(Q - 1) as u16; EXTENSION_DEGREE]),
        ];
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
        // Exercise signed accumulator carries and cancellation independently
        // of the tighter protocol bound, including the asymmetric i128 minimum.
        let extremes = std::array::from_fn(|i| match i % 4 {
            0 => i128::MIN,
            1 => i128::MAX,
            2 => -1,
            _ => 0,
        });
        for alpha in [field.zero(), field.one(), field.neg(&field.one()), alpha] {
            let expected = extremes.iter().rev().fold(field.zero(), |sum, value| {
                let magnitude = F::from_with_cfg(value.unsigned_abs(), &field);
                let coefficient = if *value < 0 { field.neg(&magnitude) } else { magnitude };
                field.add(&field.mul(&sum, &alpha), &coefficient)
            });
            assert_eq!(project_lift(&extremes, alpha, &field), expected);
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn blocked_integer_lift_matches_serial_with_distinct_rows_and_partial_block() {
        let batch = 129;
        let layout = FalconSourceLayout::new(batch).unwrap();
        let trace = super::super::verification_trace(PK, MSG, SIG).unwrap();
        let mut traces = vec![trace; batch];
        for (s, trace) in traces.iter_mut().enumerate() {
            // Distinct source rows expose accidental reuse of another block's
            // witness bits. This test checks the lift, not signature validity.
            trace.public_key.h[0] = ((73 * s + 19) % Q as usize) as u16;
            trace.public_key.h[N - 1] = ((97 * s + 31) % Q as usize) as u16;
        }
        let source =
            FalconSourceWitness::from_traces(layout, &vec![MSG; batch], &vec![SIG; batch], &traces)
                .unwrap();
        let mut row: Vec<_> = (0..layout.capacity()).map(|s| element(s + 17)).collect();
        row[batch..].fill(Ext::ZERO);
        let positions = powers(element(44));
        let (column, _) =
            bit_query(&layout, &row, &positions, &operand_weights(element(31))).unwrap();
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| grouped_lift(&source, &row, &column))
        };
        let serial = run(1);
        assert_eq!(run(4), serial);
        assert_eq!(run(16), serial);
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

        fn measure<T>(mut kernel: impl FnMut() -> T) -> f64 {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                black_box(kernel());
            }
            start.elapsed().as_secs_f64() * 1000.0 / ITERATIONS as f64
        }

        let layout = FalconSourceLayout::new(32).unwrap();
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"shared-ring-projection-kernel-benchmark/v1");
        let signature_point = sample_point(&mut transcript, b"signature-point", 5).unwrap();
        let row = equality_weights(&signature_point);
        let beta = challenge(&mut transcript, b"polynomial-evaluation").unwrap();
        let positions = powers(beta);
        let omega = operand_weights(challenge(&mut transcript, b"operand-batching").unwrap());
        let (column, _) = bit_query(&layout, &row, &positions, &omega).unwrap();
        let field = sample_shared_field(&mut transcript, &layout, 100).unwrap();
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

        let endpoint: Vec<_> = (0..layout.source_bits().ilog2())
            .map(|_| squeeze_field(&mut transcript, &field).unwrap())
            .collect();
        let split = layout.signature_stride().ilog2() as usize;
        let local = crate::piop::spartan::matrix::eq_table(&endpoint[..split], &field).unwrap();
        let instances = crate::piop::spartan::matrix::eq_table(&endpoint[split..], &field).unwrap();
        let lazy = VerifierProjectedClaim {
            row: row.clone(),
            beta,
            omega,
            alpha,
            target: field.zero(),
        };
        let materialized_endpoint = || {
            let (column, _) = bit_query(&layout, &row, &positions, &omega).unwrap();
            let claim = project(&row, &column, &[0; LIFT_COEFFICIENTS], alpha, &field);
            let dot = |a: &[F], b: &[F]| {
                a.iter().zip(b).fold(field.zero(), |sum, (x, y)| {
                    field.add(&sum, &field.mul(x, y))
                })
            };
            field.mul(&dot(&claim.row, &instances), &dot(&claim.column, &local))
        };
        let lazy_endpoint = || lazy.evaluate(&layout, &local, &instances, &field).unwrap();
        assert_eq!(materialized_endpoint(), lazy_endpoint());
        let materialized_ms = measure(materialized_endpoint);
        let lazy_ms = measure(lazy_endpoint);
        eprintln!(
            "projection_endpoint materialized_ms={materialized_ms:.6} lazy_ms={lazy_ms:.6} ratio={:.6}",
            lazy_ms / materialized_ms
        );

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

}
