//! Exact integer norms and authentication against the one committed bit source.
//!
//! The native ring reduction, norm endpoint, and slack are joined only after all
//! their claimed values are fixed. The resulting linear form over coefficient
//! words runs through the degree-two word binder; its terminal evaluations are
//! merged into one BitZ bridge statement.

use field::{BatchMulAcc, FpLinearAcc, Reduce, RingOps, WideMul};

use crate::{
    piop::spartan::{
        absorb_field_elements, falcon_integer,
        grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
        matrix::eq_table,
        squeeze_field,
    },
    sumcheck::{
        SumcheckProof,
        boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary},
        proof::validate_field_elements,
    },
    transcript::traits::Transcript,
};

use super::binding::{self, WordForm};
use super::ring::PreparedClaim;
use super::{BETA_SQUARED, Cfg, F, FalconError, Layout, N, Schedule, Source, WitnessData};

const COEFFICIENT_LOG: usize = super::COEFFICIENT_LOG;
const NORM_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/integer-outer/v1";
const MERGE_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/merge/v4";
const BINDING_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/binding/v3";
const TERMINAL_DOMAIN: &[u8] = b"bitz/falcon1024-algebraic/terminal-merge/v1";

struct NormInstance;
impl GrindingDomain for NormInstance {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/norm-instance/v2";
}
struct NormRound;
impl GrindingDomain for NormRound {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/norm-round/v2";
}
struct Merge;
impl GrindingDomain for Merge {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/merge/v3";
}
struct BindingRound;
impl GrindingDomain for BindingRound {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/binding-round/v3";
}
struct TerminalMerge;
impl GrindingDomain for TerminalMerge {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/grinding/terminal-merge/v1";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NormProof {
    pub instance_nonce: Option<u64>,
    pub slack: F,
    pub sumcheck: SumcheckProof<F, 4>,
    pub terminal: F,
    pub grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Proof {
    pub norm: NormProof,
    pub merge_nonce: Option<u64>,
    pub binding: SumcheckProof<F, 3>,
    /// `[T(r), b(r)]`: decoded coefficients and spare-lane bits at the
    /// binder's point.
    pub binding_terminal: [F; 2],
    pub binding_nonces: Vec<u64>,
    pub terminal_nonce: Option<u64>,
}

impl Proof {
    pub(super) fn payload_size_bytes(&self) -> usize {
        // Canonical field words, round coefficients, terminal messages and
        // present nonces. Container framing is excluded from this payload size.
        16 * (2
            + 4 * self.norm.sumcheck.round_polynomials.len()
            + 3 * self.binding.round_polynomials.len()
            + 2)
            + 8 * (usize::from(self.norm.instance_nonce.is_some())
                + self.norm.grinding_nonces.len()
                + usize::from(self.merge_nonce.is_some())
                + self.binding_nonces.len()
                + usize::from(self.terminal_nonce.is_some()))
    }
}

/// This is a pending PCS obligation, not a standalone proof of the relation.
/// The row weights carry the lane patterns; the column point spans the word
/// variables above the bridge's row block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BridgeClaim {
    pub column_point: Vec<F>,
    pub row_weights: Vec<u128>,
    pub modulus: u128,
    pub value: F,
}

pub(super) struct NormClaims {
    pub instance_point: Vec<F>,
    pub point: Vec<F>,
    pub terminal: F,
    pub slack: F,
}

pub(super) fn prove(
    transcript: &mut impl Transcript,
    layout: &Layout,
    data: &WitnessData,
    source: &Source,
    field: &Cfg,
    ring: &PreparedClaim,
    schedule: &Schedule,
) -> Result<(Proof, BridgeClaim), FalconError> {
    if source.layout() != layout {
        return Err(failure("algebraic source layout mismatch"));
    }
    validate_field(layout, field)?;
    let (norm, norm_claims) = prove_norm(transcript, layout, data, field, schedule)?;
    let _binding_span = tracing::info_span!("falcon_algebraic:binding").entered();
    transcript.absorb_slice(MERGE_DOMAIN);
    let merge_nonce = prove_nonce::<Merge>(transcript, schedule.merge_bits)?;
    let eta = sample(transcript, field)?;
    let (form, target) = WordForm::new(
        layout,
        ring,
        &norm_claims,
        eta,
        field,
        binding_parallel(layout.batch())?,
    )?;
    transcript.absorb_slice(BINDING_DOMAIN);
    let mut boundary =
        ProverGrindingRoundBoundary::<BindingRound>::with_round_offset(schedule.binding_bits, 0);
    let output = binding::prove(transcript, &form, source, target, &mut boundary)?;
    let binding_nonces = boundary.into_nonces();
    absorb_field_elements(transcript, &output.terminal, field);
    transcript.absorb_slice(TERMINAL_DOMAIN);
    let terminal_nonce = prove_nonce::<TerminalMerge>(transcript, schedule.terminal_bits)?;
    let zeta = sample(transcript, field)?;
    let claim = bind_opening(
        transcript,
        layout,
        &output.point,
        &zeta,
        &output.terminal,
        field,
    )?;
    Ok((
        Proof {
            norm,
            merge_nonce,
            binding: output.proof,
            binding_terminal: output.terminal,
            binding_nonces,
            terminal_nonce,
        },
        claim,
    ))
}

pub(super) fn verify(
    transcript: &mut impl Transcript,
    layout: &Layout,
    field: &Cfg,
    ring: &PreparedClaim,
    proof: &Proof,
    schedule: &Schedule,
) -> Result<BridgeClaim, FalconError> {
    validate_field(layout, field)?;
    let norm = verify_norm(transcript, layout, &proof.norm, field, schedule)?;
    transcript.absorb_slice(MERGE_DOMAIN);
    verify_nonce::<Merge>(transcript, schedule.merge_bits, proof.merge_nonce)?;
    let eta = sample(transcript, field)?;
    let (form, target) = WordForm::new(
        layout,
        ring,
        &norm,
        eta,
        field,
        binding_parallel(layout.batch())?,
    )?;
    transcript.absorb_slice(BINDING_DOMAIN);
    let mut boundary = VerifierGrindingRoundBoundary::<BindingRound>::new(
        schedule.binding_bits,
        &proof.binding_nonces,
    );
    let (point, final_claim) = proof
        .binding
        .verify_with_round_boundary(transcript, target, form.word_vars(), field, &mut boundary)
        .map_err(failure)?;
    validate_field_elements(&proof.binding_terminal, field).map_err(failure)?;
    let expected = {
        let _span = tracing::info_span!("falcon_algebraic:binding_evaluation").entered();
        field.add(
            &field.mul(
                &form.coefficient_weight_at(&point)?,
                &proof.binding_terminal[0],
            ),
            &field.mul(&form.spare_weight_at(&point)?, &proof.binding_terminal[1]),
        )
    };
    if final_claim != expected {
        return Err(failure("algebraic binding terminal mismatch"));
    }
    absorb_field_elements(transcript, &proof.binding_terminal, field);
    transcript.absorb_slice(TERMINAL_DOMAIN);
    verify_nonce::<TerminalMerge>(transcript, schedule.terminal_bits, proof.terminal_nonce)?;
    let zeta = sample(transcript, field)?;
    bind_opening(
        transcript,
        layout,
        &point,
        &zeta,
        &proof.binding_terminal,
        field,
    )
}

#[tracing::instrument(skip_all, name = "falcon_algebraic:integer_outer")]
fn prove_norm(
    transcript: &mut impl Transcript,
    layout: &Layout,
    data: &WitnessData,
    field: &Cfg,
    schedule: &Schedule,
) -> Result<(NormProof, NormClaims), FalconError> {
    if data.witness.s1.len() != layout.batch()
        || data.witness.s2.len() != layout.batch()
        || data.slacks.len() != layout.batch()
    {
        return Err(failure("integer witness shape mismatch"));
    }
    transcript.absorb_slice(NORM_DOMAIN);
    let instance_nonce = prove_nonce::<NormInstance>(transcript, schedule.norm_instance_bits)?;
    let instance_point = sample_point(transcript, layout.capacity().ilog2() as usize, field)?;
    let weights = eq_table(&instance_point, field).map_err(failure)?;
    let mut slack = FpLinearAcc::<2, 1>::default();
    for (value, weight) in data.slacks.iter().zip(&weights) {
        field.mul_acc(&mut slack, weight, value);
    }
    let slack = field.reduce(slack);
    absorb_field_elements(transcript, &[slack], field);
    let initial = field.sub(&norm_target(layout, &weights, field), &slack);
    let mut boundary =
        ProverGrindingRoundBoundary::<NormRound>::with_round_offset(schedule.norm_round_bits, 0);
    let output = falcon_integer::prove(
        field,
        transcript,
        COEFFICIENT_LOG + 1,
        &instance_point,
        None,
        field.zero(),
        initial,
        |row| {
            let instance = row / (2 * N);
            let j = row % (2 * N);
            if instance >= layout.batch() {
                [0]
            } else if j < N {
                [data.witness.s1[instance][j]]
            } else {
                [data.witness.s2[instance][j - N]]
            }
        },
        &mut boundary,
    )
    .map_err(failure)?;
    let terminal = output.terminal[0];
    Ok((
        NormProof {
            instance_nonce,
            slack,
            sumcheck: output.proof,
            terminal,
            grinding_nonces: boundary.into_nonces(),
        },
        NormClaims {
            instance_point,
            point: output.point,
            terminal,
            slack,
        },
    ))
}

fn verify_norm(
    transcript: &mut impl Transcript,
    layout: &Layout,
    proof: &NormProof,
    field: &Cfg,
    schedule: &Schedule,
) -> Result<NormClaims, FalconError> {
    transcript.absorb_slice(NORM_DOMAIN);
    verify_nonce::<NormInstance>(
        transcript,
        schedule.norm_instance_bits,
        proof.instance_nonce,
    )?;
    let instance_point = sample_point(transcript, layout.capacity().ilog2() as usize, field)?;
    let weights = eq_table(&instance_point, field).map_err(failure)?;
    validate_field_elements(&[proof.slack, proof.terminal], field).map_err(failure)?;
    absorb_field_elements(transcript, &[proof.slack], field);
    let initial = field.sub(&norm_target(layout, &weights, field), &proof.slack);
    let mut boundary = VerifierGrindingRoundBoundary::<NormRound>::new(
        schedule.norm_round_bits,
        &proof.grinding_nonces,
    );
    let (point, claim) = proof
        .sumcheck
        .verify_with_round_boundary(
            transcript,
            initial,
            COEFFICIENT_LOG + 1 + layout.capacity().ilog2() as usize,
            field,
            &mut boundary,
        )
        .map_err(failure)?;
    if claim
        != falcon_integer::terminal(
            field,
            COEFFICIENT_LOG + 1,
            &instance_point,
            None,
            field.zero(),
            &point,
            &[proof.terminal],
        )
    {
        return Err(failure("integer norm terminal mismatch"));
    }
    absorb_field_elements(transcript, &[proof.terminal], field);
    Ok(NormClaims {
        instance_point,
        point,
        terminal: proof.terminal,
        slack: proof.slack,
    })
}

fn norm_target(layout: &Layout, weights: &[F], field: &Cfg) -> F {
    let sum = weights[..layout.batch()]
        .iter()
        .fold(field.zero(), |sum, weight| field.add(&sum, weight));
    field.reduce(field.mul_wide(&sum, &BETA_SQUARED))
}

fn binding_workers() -> usize {
    #[cfg(feature = "parallel")]
    return rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    1
}

/// Keep single-worker runs and small batches serial; both paths produce
/// identical proofs. Ten-worker Falcon-512 measurements (2026-10-10)
/// qualified the parallel path for 256..=65536.
pub(super) fn parallel_binding(batch: usize) -> bool {
    binding_workers() > 1 && batch >= 256
}

fn binding_parallel(batch: usize) -> Result<bool, FalconError> {
    #[cfg(feature = "bench-internals")]
    match std::env::var("BITZ_FALCON_BINDING_PARALLEL") {
        Ok(value) if value == "serial" => return Ok(false),
        Ok(value) if value == "auto" => {
            return Ok(2 * N * batch >= 8192 && binding_workers() > 1);
        }
        Err(std::env::VarError::NotPresent) => {}
        _ => {
            return Err(failure(
                "BITZ_FALCON_BINDING_PARALLEL must be serial or auto",
            ));
        }
    }
    Ok(parallel_binding(batch))
}

/// Merge the binder's two terminal tables with `zeta` into one bridge
/// statement over the committed bits.
fn bind_opening(
    transcript: &mut impl Transcript,
    layout: &Layout,
    point: &[F],
    zeta: &F,
    terminal: &[F; 2],
    field: &Cfg,
) -> Result<BridgeClaim, FalconError> {
    let value = field.add(&terminal[0], &field.mul(zeta, &terminal[1]));
    transcript.absorb_slice(b"bitz/falcon1024-algebraic/source-opening/v3");
    transcript.absorb_slice(&field.modulus_u128().to_le_bytes());
    absorb_field_elements(transcript, point, field);
    absorb_field_elements(transcript, &[value], field);
    Ok(BridgeClaim {
        column_point: binding::column_point(point, layout),
        row_weights: binding::bridge_row_weights(point, zeta, layout, field)?,
        modulus: field.modulus_u128(),
        value,
    })
}

fn validate_field(layout: &Layout, field: &Cfg) -> Result<(), FalconError> {
    // Each raw signed15/slack27 norm residual is below 2^40. This bound is
    // applied before random batching, never to the weighted batched residual.
    if field.modulus_u128() <= (1u128 << 40) || layout.source_bits() as u128 >= field.modulus_u128()
    {
        return Err(failure("algebraic integer or padding equation may wrap"));
    }
    Ok(())
}

fn prove_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    bits: u32,
) -> Result<Option<u64>, FalconError> {
    if bits == 0 {
        return Ok(None);
    }
    grind_and_absorb(transcript, GrindingRound::<D>::new(0), bits)
        .map(Some)
        .map_err(failure)
}

fn verify_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    bits: u32,
    nonce: Option<u64>,
) -> Result<(), FalconError> {
    match (bits, nonce) {
        (0, None) => Ok(()),
        (0, Some(_)) | (_, None) => Err(failure("invalid algebraic grinding nonce shape")),
        (_, Some(nonce)) => {
            verify_and_absorb(transcript, GrindingRound::<D>::new(0), bits, nonce).map_err(failure)
        }
    }
}

fn sample(transcript: &mut impl Transcript, field: &Cfg) -> Result<F, FalconError> {
    squeeze_field(transcript, field).map_err(failure)
}
fn sample_point(
    transcript: &mut impl Transcript,
    count: usize,
    field: &Cfg,
) -> Result<Vec<F>, FalconError> {
    (0..count).map(|_| sample(transcript, field)).collect()
}
#[cfg(test)]
fn unsigned(value: u128, field: &Cfg) -> F {
    <F as crate::piop::spartan::SpartanField>::from_with_cfg(value, field)
}
#[cfg(test)]
fn signed(value: i128, field: &Cfg) -> F {
    let value_field = unsigned(value.unsigned_abs(), field);
    if value < 0 {
        field.neg(&value_field)
    } else {
        value_field
    }
}
fn failure(error: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::{FalconAlgebraicWitness, SLACK_BITS};
    use super::*;
    use crate::piop::spartan::SpartanField;
    use crate::piop::spartan::falcon_bit_layout::{coefficient_bit, slack_bit};
    use crate::transcript::Blake3Transcript;
    use field::Uint;

    fn field() -> Cfg {
        F::make_cfg(&Uint::from((1u128 << 127) - 1)).unwrap()
    }

    fn schedule() -> Schedule {
        Schedule {
            norm_instance_bits: 0,
            norm_round_bits: 0,
            merge_bits: 0,
            binding_bits: 0,
            terminal_bits: 0,
        }
    }

    fn data(batch: usize) -> WitnessData {
        let mut witness = FalconAlgebraicWitness {
            s1: vec![[0; N]; batch],
            s2: vec![[0; N]; batch],
        };
        let mut slacks = Vec::new();
        for i in 0..batch {
            witness.s1[i][i] = -(i as i16 + 3);
            witness.s2[i][N - i - 1] = 3000 + i as i16;
            let norm = witness.s1[i]
                .iter()
                .chain(&witness.s2[i])
                .map(|&v| (i64::from(v) * i64::from(v)) as u64)
                .sum::<u64>();
            slacks.push(BETA_SQUARED - norm);
        }
        WitnessData {
            witness,
            slacks,
            quotients: vec![vec![0; N - 1]; batch],
        }
    }

    fn ring(data: &WitnessData, field: &Cfg) -> PreparedClaim {
        let len = data.witness.s1.len() * N;
        let weights_s1: Vec<_> = (0..len)
            .map(|i| unsigned((i % 37 + 1) as u128, field))
            .collect();
        let weights_s2: Vec<_> = (0..len)
            .map(|i| unsigned((i % 43 + 1) as u128, field))
            .collect();
        let target = [&data.witness.s1, &data.witness.s2]
            .into_iter()
            .zip([&weights_s1, &weights_s2])
            .fold(field.zero(), |sum, (values, weights)| {
                values
                    .iter()
                    .flatten()
                    .zip(weights)
                    .fold(sum, |sum, (&value, weight)| {
                        field.add(&sum, &field.mul(weight, &signed(value as i128, field)))
                    })
            });
        PreparedClaim {
            weights_s1,
            weights_s2,
            target,
        }
    }

    /// The bridge statement evaluated directly on the committed bits.
    fn open_bridge(source: &Source, claim: &BridgeClaim, field: &Cfg) -> F {
        let p = source.layout().bitz_params();
        let columns = eq_table(&claim.column_point, field).unwrap();
        assert_eq!(claim.row_weights.len(), p.rows());
        assert_eq!(columns.len(), p.cols());
        (0..source.layout().source_bits())
            .filter(|&index| source.bit(index))
            .fold(field.zero(), |sum, index| {
                field.add(
                    &sum,
                    &field.mul(
                        &unsigned(claim.row_weights[index % p.rows()], field),
                        &columns[index / p.rows()],
                    ),
                )
            })
    }

    #[test]
    fn singleton_and_padded_batch_open_the_same_bits() {
        let field = field();
        for batch in [1, 3] {
            let layout = Layout::new(batch).unwrap();
            let data = data(batch);
            let source = Source::new(layout, &data);
            let ring = ring(&data, &field);
            let (proof, claim) = prove(
                &mut Blake3Transcript::new(),
                &layout,
                &data,
                &source,
                &field,
                &ring,
                &schedule(),
            )
            .unwrap();
            let verified = verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule(),
            )
            .unwrap();
            assert_eq!(claim, verified);
            assert_eq!(claim.value, open_bridge(&source, &claim, &field));
            assert_eq!(claim.column_point.len(), layout.bitz_params().col_vars);
            assert!(claim.row_weights.iter().all(|&w| w < claim.modulus));
        }
    }

    #[test]
    fn invalid_slack_and_cross_witness_binding_are_rejected() {
        let field = field();
        let layout = Layout::new(1).unwrap();
        let data = data(1);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        let mut wrong_slack = data.clone();
        wrong_slack.slacks[0] += 1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &wrong_slack,
                &source,
                &field,
                &ring,
                &schedule()
            )
            .is_err()
        );
        let mut other_witness = data.clone();
        // Sign reversal preserves the norm but must not authenticate to the
        // original source or native-ring claim.
        other_witness.witness.s1[0][0] *= -1;
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &other_witness,
                &source,
                &field,
                &ring,
                &schedule()
            )
            .is_err()
        );
    }

    #[test]
    fn signatures_cannot_borrow_each_others_norm_budget() {
        let field = field();
        let layout = Layout::new(2).unwrap();
        let excessive = (BETA_SQUARED.isqrt() + 1) as i16;
        let excessive_norm = (excessive as u64).pow(2);
        let mut data = WitnessData {
            witness: FalconAlgebraicWitness {
                s1: vec![[0; N]; 2],
                s2: vec![[0; N]; 2],
            },
            slacks: vec![0, 2 * BETA_SQUARED - excessive_norm],
            quotients: vec![vec![0; N - 1]; 2],
        };
        data.witness.s2[0][0] = excessive;
        // The unweighted batch total is exactly 2B, despite the first
        // signature having squared norm above B.
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        assert!(
            prove(
                &mut Blake3Transcript::new(),
                &layout,
                &data,
                &source,
                &field,
                &ring,
                &schedule(),
            )
            .is_err()
        );
    }

    #[test]
    fn altered_bits_and_both_kinds_of_padding_are_rejected() {
        let field = field();
        let layout = Layout::new(3).unwrap();
        let data = data(3);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        for index in [
            0,
            coefficient_bit(N, 1, 0, 14),
            slack_bit(0),
            slack_bit(SLACK_BITS),
            3 * layout.signature_stride(),
            3 * layout.signature_stride() + 15,
            layout.source_bits() - 2,
        ] {
            let mut altered = source.clone();
            altered.flip(index);
            assert!(
                prove(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &data,
                    &altered,
                    &field,
                    &ring,
                    &schedule()
                )
                .is_err(),
                "bit {index} was not bound"
            );
        }
    }

    #[test]
    fn malformed_messages_and_nonces_are_rejected() {
        let field = field();
        let layout = Layout::new(1).unwrap();
        let data = data(1);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        let (proof, _) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &data,
            &source,
            &field,
            &ring,
            &schedule(),
        )
        .unwrap();
        let rejects = |proof: &Proof| {
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &field,
                    &ring,
                    proof,
                    &schedule()
                )
                .is_err()
            )
        };
        let mut altered = proof.clone();
        altered.norm.terminal = field.add(&altered.norm.terminal, &field.one());
        rejects(&altered);
        let mut altered = proof.clone();
        altered.norm.slack = field.add(&altered.norm.slack, &field.one());
        rejects(&altered);
        let mut altered = proof.clone();
        altered.norm.sumcheck.round_polynomials.pop();
        rejects(&altered);
        let mut altered = proof.clone();
        altered.binding.round_polynomials.pop();
        rejects(&altered);
        for slot in 0..2 {
            let mut altered = proof.clone();
            altered.binding_terminal[slot] =
                field.add(&altered.binding_terminal[slot], &field.one());
            rejects(&altered);
        }
        let mut altered = proof.clone();
        altered.merge_nonce = Some(0);
        rejects(&altered);
        let mut altered = proof.clone();
        altered.binding_nonces.push(0);
        rejects(&altered);
        let mut altered = proof.clone();
        altered.terminal_nonce = Some(0);
        rejects(&altered);
    }

    #[test]
    fn preceding_transcript_domains_cannot_be_replayed() {
        /// Downgrades every versioned algebraic domain tag by one.
        struct LegacyDomains(Blake3Transcript);
        impl Transcript for LegacyDomains {
            fn get_challenge<T: crate::transcript::traits::ConstTranscribable>(&mut self) -> T {
                self.0.get_challenge()
            }
            fn begin_sampling(&mut self) {
                self.0.begin_sampling();
            }
            fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
                self.0.fill_sampling_bytes(output);
            }
            fn absorb_inner(&mut self, value: &[u8]) {
                if value.starts_with(b"bitz/falcon1024-algebraic/")
                    && value.len() >= 3
                    && value[value.len() - 2] == b'v'
                    && value[value.len() - 1].is_ascii_digit()
                {
                    let mut legacy = value.to_vec();
                    let version = legacy.last_mut().unwrap();
                    *version = if *version == b'1' { b'9' } else { *version - 1 };
                    self.0.absorb_inner(&legacy);
                } else {
                    self.0.absorb_inner(value);
                }
            }
        }
        let field = field();
        let layout = Layout::new(1).unwrap();
        let data = data(1);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        // Keep the new geometry here to isolate domain separation itself.
        let (proof, claim) = prove(
            &mut LegacyDomains(Blake3Transcript::new()),
            &layout,
            &data,
            &source,
            &field,
            &ring,
            &schedule(),
        )
        .unwrap();
        assert_eq!(
            verify(
                &mut LegacyDomains(Blake3Transcript::new()),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule(),
            )
            .unwrap(),
            claim
        );
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule(),
            )
            .is_err()
        );
    }

    #[test]
    fn grinded_messages_roundtrip_and_require_all_nonces() {
        let field = field();
        let layout = Layout::new(3).unwrap();
        let data = data(3);
        let source = Source::new(layout, &data);
        let ring = ring(&data, &field);
        let schedule = Schedule {
            norm_instance_bits: 2,
            norm_round_bits: 2,
            merge_bits: 2,
            binding_bits: 2,
            terminal_bits: 2,
        };
        let (mut proof, claim) = prove(
            &mut Blake3Transcript::new(),
            &layout,
            &data,
            &source,
            &field,
            &ring,
            &schedule,
        )
        .unwrap();
        assert_eq!(
            proof.binding_nonces.len(),
            proof.binding.round_polynomials.len()
        );
        assert!(proof.terminal_nonce.is_some());
        assert_eq!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule
            )
            .unwrap(),
            claim
        );
        let mut without_terminal = proof.clone();
        without_terminal.terminal_nonce = None;
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &without_terminal,
                &schedule
            )
            .is_err()
        );
        proof.norm.grinding_nonces.pop();
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &field,
                &ring,
                &proof,
                &schedule
            )
            .is_err()
        );
    }
}
