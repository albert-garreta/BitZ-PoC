//! Shared-prime integer messages with derived metadata and linear coefficients omitted.
//!
//! The verifier reconstructs each full round before absorption, preserving the
//! established transcript. Only the checked endpoint claims survive verification.

use super::*;
use crate::sumcheck::{
    SumcheckError,
    boundary::RoundBoundaryPolicy,
    proof::{evaluate_polynomial, reconstruct_round_coefficients},
};

#[derive(Clone, Debug)]
pub(super) struct CompactSumcheck<const COEFFS: usize> {
    /// Coefficients [c0, c2, ...]; c1 = claim - 2*c0 - c2 - ... .
    round_polynomials: Vec<[F; COEFFS]>,
}

impl<const COEFFS: usize> CompactSumcheck<COEFFS> {
    fn from_full<const FULL: usize>(proof: SumcheckProof<F, FULL>) -> Self {
        assert_eq!(FULL, COEFFS + 1);
        Self {
            round_polynomials: proof
                .round_polynomials
                .into_iter()
                .map(|round| std::array::from_fn(|i| round[if i == 0 { 0 } else { i + 1 }]))
                .collect(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum RoundProofRef<'a, const FULL: usize, const COMPACT: usize> {
    Full(&'a SumcheckProof<F, FULL>),
    Compact(&'a CompactSumcheck<COMPACT>),
}

impl<'a, const FULL: usize, const COMPACT: usize> From<&'a SumcheckProof<F, FULL>>
    for RoundProofRef<'a, FULL, COMPACT>
{
    fn from(proof: &'a SumcheckProof<F, FULL>) -> Self {
        Self::Full(proof)
    }
}

/// Native proofs keep using the existing verifier. Compact proofs reconstruct
/// c1 from the incoming claim and absorb the identical full coefficient array.
pub(super) fn verify_round_proofs<const FULL: usize, const COMPACT: usize, const K: usize>(
    proofs: [RoundProofRef<'_, FULL, COMPACT>; K],
    transcript: &mut impl Transcript,
    initial_claims: &[F; K],
    rounds: usize,
    field: &Cfg,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<(Vec<F>, [F; K]), SumcheckError> {
    if proofs
        .iter()
        .all(|proof| matches!(proof, RoundProofRef::Full(_)))
    {
        let full = proofs.map(|proof| match proof {
            RoundProofRef::Full(proof) => proof,
            RoundProofRef::Compact(_) => unreachable!("all proofs are full"),
        });
        return SumcheckProof::verify_batch_with_round_boundary(
            full,
            transcript,
            initial_claims,
            rounds,
            field,
            boundary,
        );
    }
    if K == 0 {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    boundary.validate(rounds)?;
    validate_field_elements(initial_claims, field)?;
    for proof in proofs {
        let actual = match proof {
            RoundProofRef::Full(proof) => {
                for coefficients in &proof.round_polynomials {
                    validate_field_elements(coefficients, field)?;
                }
                proof.round_polynomials.len()
            }
            RoundProofRef::Compact(proof) => {
                for coefficients in &proof.round_polynomials {
                    validate_field_elements(coefficients, field)?;
                }
                proof.round_polynomials.len()
            }
        };
        if actual != rounds {
            return Err(SumcheckError::InvalidRoundCount {
                expected: rounds,
                actual,
            });
        }
    }
    let zero = field.zero();
    let mut claims = *initial_claims;
    let mut point = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let coefficients: [[F; FULL]; K] = std::array::from_fn(|i| match proofs[i] {
            RoundProofRef::Full(proof) => proof.round_polynomials[round],
            RoundProofRef::Compact(proof) => reconstruct_round_coefficients(
                &claims[i],
                &proof.round_polynomials[round],
                &zero,
                field,
            ),
        });
        for (coefficients, claim) in coefficients.iter().zip(&claims) {
            absorb_field_elements(transcript, coefficients, field);
            let at_one = coefficients.iter().fold(zero, |sum, c| field.add(&sum, c));
            if field.add(&coefficients[0], &at_one) != *claim {
                return Err(SumcheckError::InvalidRoundClaim { round });
            }
        }
        boundary.after_round(transcript, round)?;
        let challenge = squeeze_field(transcript, field)?;
        for (coefficients, claim) in coefficients.iter().zip(&mut claims) {
            *claim = evaluate_polynomial(coefficients, &challenge, &zero, field);
        }
        point.push(challenge);
    }
    Ok((point, claims))
}

pub(super) struct NormPayload<'a> {
    pub instance_nonce: Option<u64>,
    pub claims: [F; 2],
    pub sumchecks: [RoundProofRef<'a, 3, 2>; 2],
    pub terminal: [[F; 2]; 2],
    pub grinding_nonces: &'a [u64],
    pub instance_point: Option<&'a [F]>,
    pub point: Option<&'a [F]>,
    pub slack: Option<F>,
}
impl<'a> From<&'a NormProof> for NormPayload<'a> {
    fn from(proof: &'a NormProof) -> Self {
        Self {
            instance_nonce: proof.instance_nonce,
            claims: proof.claims,
            sumchecks: [&proof.sumchecks[0], &proof.sumchecks[1]].map(RoundProofRef::from),
            terminal: proof.terminal,
            grinding_nonces: &proof.grinding_nonces,
            instance_point: Some(&proof.instance_point),
            point: Some(&proof.point),
            slack: Some(proof.slack),
        }
    }
}
pub(super) struct QuadraticPayload<'a> {
    pub point_nonce: Option<u64>,
    pub sumcheck: RoundProofRef<'a, 4, 3>,
    pub terminal: OuterEvaluations<F>,
    pub grinding_nonces: &'a [u64],
    pub point: Option<&'a [F]>,
}
impl<'a> From<&'a QuadraticRelationProof> for QuadraticPayload<'a> {
    fn from(proof: &'a QuadraticRelationProof) -> Self {
        Self {
            point_nonce: proof.point_nonce,
            sumcheck: (&proof.sumcheck).into(),
            terminal: proof.terminal,
            grinding_nonces: &proof.grinding_nonces,
            point: Some(&proof.point),
        }
    }
}
pub(super) struct LeafPayload<'a> {
    pub instance_nonce: Option<u64>,
    pub sumcheck: RoundProofRef<'a, 4, 3>,
    pub terminal: [F; 3],
    pub grinding_nonces: &'a [u64],
    pub instance_point: Option<&'a [F]>,
    pub point: Option<&'a [F]>,
}
impl<'a> From<&'a CompactionLeafProof> for LeafPayload<'a> {
    fn from(proof: &'a CompactionLeafProof) -> Self {
        Self {
            instance_nonce: proof.instance_nonce,
            sumcheck: (&proof.sumcheck).into(),
            terminal: proof.terminal,
            grinding_nonces: &proof.grinding_nonces,
            instance_point: Some(&proof.instance_point),
            point: Some(&proof.point),
        }
    }
}
pub(super) struct ForestLayerPayload<'a> {
    pub sumcheck: Option<RoundProofRef<'a, 4, 3>>,
    pub evaluations: &'a [[F; 2]],
    pub grinding_nonces: &'a [u64],
    pub batching_nonce: Option<u64>,
    pub line_nonce: Option<u64>,
}
impl<'a> From<&'a ProductForestLayerProof> for ForestLayerPayload<'a> {
    fn from(proof: &'a ProductForestLayerProof) -> Self {
        Self {
            sumcheck: proof.sumcheck.as_ref().map(RoundProofRef::from),
            evaluations: &proof.evaluations,
            grinding_nonces: &proof.grinding_nonces,
            batching_nonce: proof.batching_nonce,
            line_nonce: proof.line_nonce,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct NormClaimRef<'a> {
    pub instance_point: &'a [F],
    pub terminal: &'a [[F; 2]; 2],
    pub slack: F,
    pub point: &'a [F],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct QuadraticClaimRef<'a> {
    pub terminal: OuterEvaluations<F>,
    pub point: &'a [F],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct LeafClaimRef<'a> {
    pub instance_point: &'a [F],
    pub terminal: &'a [F; 3],
    pub point: &'a [F],
}
/// Borrowed endpoint interface shared by native proofs and checked shared claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopClaimRef<'a> {
    pub modulus: u128,
    pub norm: NormClaimRef<'a>,
    pub compact_products: QuadraticClaimRef<'a>,
    pub compaction_gamma: F,
    pub compaction_rank_scale: F,
    pub compaction: &'a [CompactionProof],
    pub compaction_leaf: LeafClaimRef<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NormClaims {
    instance_point: Vec<F>,
    terminal: [[F; 2]; 2],
    slack: F,
    point: Vec<F>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct QuadraticClaims {
    terminal: OuterEvaluations<F>,
    point: Vec<F>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct LeafClaims {
    instance_point: Vec<F>,
    terminal: [F; 3],
    point: Vec<F>,
}
/// Authenticated endpoints only: no proof messages, forest layers, or nonces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopClaims {
    modulus: u128,
    norm: NormClaims,
    compact_products: QuadraticClaims,
    compaction_gamma: F,
    compaction_rank_scale: F,
    compaction: Vec<CompactionProof>,
    compaction_leaf: LeafClaims,
}

impl FalconPiopProof {
    pub(in super::super) fn as_claim_ref(&self) -> FalconPiopClaimRef<'_> {
        FalconPiopClaimRef {
            modulus: self.modulus,
            norm: NormClaimRef {
                instance_point: &self.norm.instance_point,
                terminal: &self.norm.terminal,
                slack: self.norm.slack,
                point: &self.norm.point,
            },
            compact_products: QuadraticClaimRef {
                terminal: self.compact_products.terminal,
                point: &self.compact_products.point,
            },
            compaction_gamma: self.compaction_gamma,
            compaction_rank_scale: self.compaction_rank_scale,
            compaction: &self.compaction,
            compaction_leaf: LeafClaimRef {
                instance_point: &self.compaction_leaf.instance_point,
                terminal: &self.compaction_leaf.terminal,
                point: &self.compaction_leaf.point,
            },
        }
    }
}
impl FalconPiopClaims {
    pub(in super::super) fn as_claim_ref(&self) -> FalconPiopClaimRef<'_> {
        FalconPiopClaimRef {
            modulus: self.modulus,
            norm: NormClaimRef {
                instance_point: &self.norm.instance_point,
                terminal: &self.norm.terminal,
                slack: self.norm.slack,
                point: &self.norm.point,
            },
            compact_products: QuadraticClaimRef {
                terminal: self.compact_products.terminal,
                point: &self.compact_products.point,
            },
            compaction_gamma: self.compaction_gamma,
            compaction_rank_scale: self.compaction_rank_scale,
            compaction: &self.compaction,
            compaction_leaf: LeafClaimRef {
                instance_point: &self.compaction_leaf.instance_point,
                terminal: &self.compaction_leaf.terminal,
                point: &self.compaction_leaf.point,
            },
        }
    }
}

#[derive(Clone, Debug)]
struct SharedNormProof {
    instance_nonce: Option<u64>,
    claims: [F; 2],
    sumchecks: [CompactSumcheck<2>; 2],
    terminal: [[F; 2]; 2],
    grinding_nonces: Vec<u64>,
}
#[derive(Clone, Debug)]
struct SharedQuadraticProof {
    point_nonce: Option<u64>,
    sumcheck: CompactSumcheck<3>,
    terminal: OuterEvaluations<F>,
    grinding_nonces: Vec<u64>,
}
#[derive(Clone, Debug)]
struct SharedLeafProof {
    instance_nonce: Option<u64>,
    sumcheck: CompactSumcheck<3>,
    terminal: [F; 3],
    grinding_nonces: Vec<u64>,
}
#[derive(Clone, Debug)]
struct SharedForestLayerProof {
    sumcheck: Option<CompactSumcheck<3>>,
    evaluations: Vec<[F; 2]>,
    grinding_nonces: Vec<u64>,
    batching_nonce: Option<u64>,
    line_nonce: Option<u64>,
}
impl<'a> From<&'a SharedForestLayerProof> for ForestLayerPayload<'a> {
    fn from(proof: &'a SharedForestLayerProof) -> Self {
        Self {
            sumcheck: proof.sumcheck.as_ref().map(RoundProofRef::Compact),
            evaluations: &proof.evaluations,
            grinding_nonces: &proof.grinding_nonces,
            batching_nonce: proof.batching_nonce,
            line_nonce: proof.line_nonce,
        }
    }
}

/// Stored messages and nonces. The verifier derives all endpoint metadata and
/// each sumcheck's linear coefficient; none is retained in this proof.
#[derive(Clone, Debug)]
pub(in super::super) struct SharedFalconPiopProof {
    norm: SharedNormProof,
    compact_products: SharedQuadraticProof,
    fingerprint_nonce: Option<u64>,
    compaction_forest: Vec<SharedForestLayerProof>,
    compaction_leaf: SharedLeafProof,
}

impl FalconPiopProof {
    pub(in super::super) fn into_shared(self) -> SharedFalconPiopProof {
        let Self {
            modulus: _,
            norm,
            compact_products,
            fingerprint_nonce,
            compaction_gamma: _,
            compaction_rank_scale: _,
            compaction: _,
            compaction_forest,
            compaction_leaf,
        } = self;
        SharedFalconPiopProof {
            norm: SharedNormProof {
                instance_nonce: norm.instance_nonce,
                claims: norm.claims,
                sumchecks: norm.sumchecks.map(CompactSumcheck::from_full),
                terminal: norm.terminal,
                grinding_nonces: norm.grinding_nonces,
            },
            compact_products: SharedQuadraticProof {
                point_nonce: compact_products.point_nonce,
                sumcheck: CompactSumcheck::from_full(compact_products.sumcheck),
                terminal: compact_products.terminal,
                grinding_nonces: compact_products.grinding_nonces,
            },
            fingerprint_nonce,
            compaction_forest: compaction_forest
                .layers
                .into_iter()
                .map(|layer| SharedForestLayerProof {
                    sumcheck: layer.sumcheck.map(CompactSumcheck::from_full),
                    evaluations: layer.evaluations,
                    grinding_nonces: layer.grinding_nonces,
                    batching_nonce: layer.batching_nonce,
                    line_nonce: layer.line_nonce,
                })
                .collect(),
            compaction_leaf: SharedLeafProof {
                instance_nonce: compaction_leaf.instance_nonce,
                sumcheck: CompactSumcheck::from_full(compaction_leaf.sumcheck),
                terminal: compaction_leaf.terminal,
                grinding_nonces: compaction_leaf.grinding_nonces,
            },
        }
    }
}

impl SharedFalconPiopProof {
    /// Visit every stored arithmetic nonce without expanding compact rounds.
    pub(in super::super) fn visit_grinding_nonces(&self, mut visit: impl FnMut(&'static str, u64)) {
        let Self {
            norm,
            compact_products,
            fingerprint_nonce,
            compaction_forest,
            compaction_leaf,
        } = self;
        let SharedNormProof {
            instance_nonce,
            claims: _,
            sumchecks: _,
            terminal: _,
            grinding_nonces,
        } = norm;
        for &nonce in instance_nonce.iter().chain(grinding_nonces) {
            visit("prime_norm", nonce);
        }
        let SharedQuadraticProof {
            point_nonce,
            sumcheck: _,
            terminal: _,
            grinding_nonces,
        } = compact_products;
        for &nonce in point_nonce.iter().chain(grinding_nonces) {
            visit("prime_products", nonce);
        }
        for &nonce in fingerprint_nonce.iter() {
            visit("prime_fingerprint", nonce);
        }
        for SharedForestLayerProof {
            sumcheck: _,
            evaluations: _,
            grinding_nonces,
            batching_nonce,
            line_nonce,
        } in compaction_forest
        {
            for &nonce in grinding_nonces
                .iter()
                .chain(batching_nonce)
                .chain(line_nonce)
            {
                visit("prime_forest", nonce);
            }
        }
        let SharedLeafProof {
            instance_nonce,
            sumcheck: _,
            terminal: _,
            grinding_nonces,
        } = compaction_leaf;
        for &nonce in instance_nonce.iter().chain(grinding_nonces) {
            visit("prime_leaf", nonce);
        }
    }

    /// Canonical scalar payload with the same framing exclusions as native.
    pub(in super::super) fn payload_size_bytes(&self) -> usize {
        let Self {
            norm,
            compact_products,
            fingerprint_nonce,
            compaction_forest,
            compaction_leaf,
        } = self;
        let nonce = |value: Option<u64>| 8 * usize::from(value.is_some());
        let SharedNormProof {
            instance_nonce,
            claims,
            sumchecks,
            terminal,
            grinding_nonces,
        } = norm;
        let norm_bytes = 16 * (claims.len() + terminal.iter().map(|x| x.len()).sum::<usize>())
            + sumchecks
                .iter()
                .map(|s| 16 * 2 * s.round_polynomials.len())
                .sum::<usize>()
            + nonce(*instance_nonce)
            + 8 * grinding_nonces.len();
        let SharedQuadraticProof {
            point_nonce,
            sumcheck,
            terminal: _,
            grinding_nonces,
        } = compact_products;
        let product_bytes = 3 * 16
            + 3 * 16 * sumcheck.round_polynomials.len()
            + nonce(*point_nonce)
            + 8 * grinding_nonces.len();
        let forest_bytes = compaction_forest
            .iter()
            .map(|layer| {
                let SharedForestLayerProof {
                    sumcheck,
                    evaluations,
                    grinding_nonces,
                    batching_nonce,
                    line_nonce,
                } = layer;
                sumcheck
                    .as_ref()
                    .map_or(0, |s| 3 * 16 * s.round_polynomials.len())
                    + 2 * 16 * evaluations.len()
                    + 8 * grinding_nonces.len()
                    + nonce(*batching_nonce)
                    + nonce(*line_nonce)
            })
            .sum::<usize>();
        let SharedLeafProof {
            instance_nonce,
            sumcheck,
            terminal,
            grinding_nonces,
        } = compaction_leaf;
        let leaf_bytes = 16 * terminal.len()
            + 3 * 16 * sumcheck.round_polynomials.len()
            + nonce(*instance_nonce)
            + 8 * grinding_nonces.len();
        norm_bytes + product_bytes + nonce(*fingerprint_nonce) + forest_bytes + leaf_bytes
    }
}

/// Verifies borrowed compact messages in the already-sampled ring field. Forest
/// evaluations and round arrays are never cloned into the returned endpoints.
pub(in super::super) fn verify_shared_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &SharedFalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopClaims, FalconError> {
    if !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid shared PIOP security target"));
    }
    validate_shared_field(layout, target_bits, field)?;
    if target_bits == 100 && !proof.compact_products.grinding_nonces.is_empty() {
        return Err(piop("unexpected shared product grinding nonces"));
    }
    bind_shared_header(transcript, layout, target_bits, field);
    let (instance_point, point, slack) = verify_norm_payload(
        transcript,
        layout,
        NormPayload {
            instance_nonce: proof.norm.instance_nonce,
            claims: proof.norm.claims,
            sumchecks: [&proof.norm.sumchecks[0], &proof.norm.sumchecks[1]]
                .map(RoundProofRef::Compact),
            terminal: proof.norm.terminal,
            grinding_nonces: &proof.norm.grinding_nonces,
            instance_point: None,
            point: None,
            slack: None,
        },
        target_bits,
        field,
    )?;
    let norm = NormClaims {
        instance_point,
        terminal: proof.norm.terminal,
        slack,
        point,
    };

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
    let security = security_schedule(layout, target_bits)?;
    let point = verify_quadratic_payload(
        transcript,
        compaction_product_rounds(layout),
        QuadraticPayload {
            point_nonce: proof.compact_products.point_nonce,
            sumcheck: RoundProofRef::Compact(&proof.compact_products.sumcheck),
            terminal: proof.compact_products.terminal,
            grinding_nonces: &proof.compact_products.grinding_nonces,
            point: None,
        },
        target_bits,
        security,
        field,
    )?;
    let compact_products = QuadraticClaims {
        terminal: proof.compact_products.terminal,
        point,
    };

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction/fingerprint/v1");
    match (target_bits, proof.fingerprint_nonce) {
        (128, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<FingerprintGrinding>::new(0),
            security.fingerprint_bits,
            nonce,
        )
        .map_err(|e| piop(e.to_string()))?,
        (100, None) => {}
        _ => return Err(piop("invalid fingerprint grinding nonce")),
    }
    let compaction_gamma = squeeze(transcript, field)?;
    let compaction_rank_scale = squeeze(transcript, field)?;

    // No challenge intervenes between the formerly stored roots and level zero.
    // Derive their duplicate absorption from canonical first-layer products.
    let first = proof
        .compaction_forest
        .first()
        .ok_or_else(|| piop("missing shared compaction root layer"))?;
    if first.evaluations.len() != 2 * layout.batch() {
        return Err(piop("shared compaction root count mismatch"));
    }
    validate_field_elements(&first.evaluations.concat(), field).map_err(|e| piop(e.to_string()))?;
    let roots: Vec<_> = first
        .evaluations
        .iter()
        .map(|[a, b]| field.mul(a, b))
        .collect();
    let (point, claims) = verify_product_forest_payload(
        transcript,
        proof.compaction_forest.iter().map(ForestLayerPayload::from),
        &roots,
        target_bits,
        security,
        field,
    )?;
    let compaction: Vec<_> = roots
        .chunks_exact(2)
        .zip(claims.chunks_exact(2))
        .map(|(roots, claims)| CompactionProof {
            candidate: PrimeProductTreeProof {
                root: roots[0],
                terminal_point: point.clone(),
                terminal_claim: claims[0],
            },
            output: PrimeProductTreeProof {
                root: roots[1],
                terminal_point: point.clone(),
                terminal_claim: claims[1],
            },
        })
        .collect();
    let (instance_point, point) = verify_compaction_leaf_payload(
        transcript,
        layout,
        &compaction,
        LeafPayload {
            instance_nonce: proof.compaction_leaf.instance_nonce,
            sumcheck: RoundProofRef::Compact(&proof.compaction_leaf.sumcheck),
            terminal: proof.compaction_leaf.terminal,
            grinding_nonces: &proof.compaction_leaf.grinding_nonces,
            instance_point: None,
            point: None,
        },
        target_bits,
        field,
    )?;
    let compaction_leaf = LeafClaims {
        instance_point,
        terminal: proof.compaction_leaf.terminal,
        point,
    };
    Ok(FalconPiopClaims {
        modulus: field.modulus_u128(),
        norm,
        compact_products,
        compaction_gamma,
        compaction_rank_scale,
        compaction,
        compaction_leaf,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{piop::spartan::falcon1024_ct::verification_trace, transcript::Blake3Transcript};

    struct RecordingTranscript {
        inner: Blake3Transcript,
        absorbed: Vec<Vec<u8>>,
    }

    impl RecordingTranscript {
        fn new() -> Self {
            Self {
                inner: Blake3Transcript::new(),
                absorbed: Vec::new(),
            }
        }
    }

    impl Transcript for RecordingTranscript {
        fn get_challenge<T: crate::transcript::traits::ConstTranscribable>(&mut self) -> T {
            self.inner.get_challenge()
        }

        fn begin_sampling(&mut self) {
            self.inner.begin_sampling();
        }

        fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
            self.inner.fill_sampling_bytes(output);
        }

        fn absorb_inner(&mut self, bytes: &[u8]) {
            self.absorbed.push(bytes.to_vec());
            self.inner.absorb_inner(bytes);
        }
    }

    fn fixture() -> FalconVerificationTrace {
        verification_trace(
            include_bytes!("fixtures/public_key.bin"),
            include_bytes!("fixtures/message.bin"),
            include_bytes!("fixtures/signature_ct.bin"),
        )
        .unwrap()
    }

    #[test]
    fn compact_shared_piop_reconstructs_every_omitted_value_and_transcript() {
        for (batch, target) in [(1, 100), (3, 100), (1, 128)] {
            let layout = FalconSourceLayout::new_shared_prime(batch).unwrap();
            let (prime_min, prime_max) =
                crate::piop::spartan::falcon1024_ct::shared_ring::prime_bounds(target).unwrap();
            let field = crate::prime_sampling::sample_prime_context(
                &mut Blake3Transcript::new(),
                prime_min,
                prime_max,
                128,
            )
            .unwrap();
            let mut prover = RecordingTranscript::new();
            let expanded = prove_falcon_piop_in_field(
                &mut prover,
                &layout,
                &vec![fixture(); batch],
                target,
                &field,
            )
            .unwrap();
            let compact = expanded.clone().into_shared();
            let mut verifier = RecordingTranscript::new();
            let restored = verify_shared_falcon_piop_in_field(
                &mut verifier,
                &layout,
                &compact,
                target,
                &field,
            )
            .unwrap();
            assert_eq!(restored.as_claim_ref(), expanded.as_claim_ref());
            assert_eq!(prover.absorbed, verifier.absorbed);
            let mut full_verifier = RecordingTranscript::new();
            verify_falcon_piop_in_field(&mut full_verifier, &layout, &expanded, target, &field)
                .unwrap();
            assert_eq!(prover.absorbed, full_verifier.absorbed);
            assert_eq!(
                prover.get_challenge::<u128>(),
                verifier.get_challenge::<u128>()
            );
            let m = layout.capacity().ilog2() as usize;
            let omitted_fields = 1
                + 2
                + 1
                + expanded.norm.instance_point.len()
                + expanded.norm.point.len()
                + expanded.compact_products.point.len()
                + expanded.compaction_leaf.instance_point.len()
                + expanded.compaction_leaf.point.len()
                + expanded
                    .compaction
                    .iter()
                    .map(|pair| {
                        4 + pair.candidate.terminal_point.len() + pair.output.terminal_point.len()
                    })
                    .sum::<usize>();
            assert_eq!(16 * omitted_fields, 416 * batch + 576 + 80 * m);
            let omitted_round_coefficients = expanded
                .norm
                .sumchecks
                .iter()
                .map(|s| s.round_polynomials.len())
                .sum::<usize>()
                + expanded.compact_products.sumcheck.round_polynomials.len()
                + expanded
                    .compaction_forest
                    .layers
                    .iter()
                    .map(|layer| {
                        layer
                            .sumcheck
                            .as_ref()
                            .map_or(0, |s| s.round_polynomials.len())
                    })
                    .sum::<usize>()
                + expanded.compaction_leaf.sumcheck.round_polynomials.len();
            assert_eq!(omitted_round_coefficients, 97 + 4 * m);
            assert_eq!(compact.norm.sumchecks[0].round_polynomials[0].len(), 2);
            assert_eq!(
                compact.compact_products.sumcheck.round_polynomials[0].len(),
                3
            );
            for (original, compact) in expanded.norm.sumchecks.iter().zip(&compact.norm.sumchecks) {
                for (full, stored) in original
                    .round_polynomials
                    .iter()
                    .zip(&compact.round_polynomials)
                {
                    assert_eq!(*stored, [full[0], full[2]]);
                }
            }
            assert!(compact.payload_size_bytes() > 0);
        }
    }

    #[test]
    fn compact_shared_piop_rejects_changed_messages_and_forest_shapes() {
        let layout = FalconSourceLayout::new_shared_prime(1).unwrap();
        let (prime_min, prime_max) =
            crate::piop::spartan::falcon1024_ct::shared_ring::prime_bounds(100).unwrap();
        let field = crate::prime_sampling::sample_prime_context(
            &mut Blake3Transcript::new(),
            prime_min,
            prime_max,
            128,
        )
        .unwrap();
        let expanded = prove_falcon_piop_in_field(
            &mut Blake3Transcript::new(),
            &layout,
            &[fixture()],
            100,
            &field,
        )
        .unwrap();
        let compact = expanded.clone().into_shared();
        let reject = |proof: &SharedFalconPiopProof| {
            assert!(
                verify_shared_falcon_piop_in_field(
                    &mut Blake3Transcript::new(),
                    &layout,
                    proof,
                    100,
                    &field,
                )
                .is_err()
            )
        };
        let mut bad = compact.clone();
        bad.norm.claims[0] = field.add(&bad.norm.claims[0], &field.one());
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest[0].evaluations[0][0] = field.zero();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest[0].evaluations.pop();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.pop();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_leaf.terminal[0] = field.add(&bad.compaction_leaf.terminal[0], &field.one());
        reject(&bad);
        // Missing or extra stored rounds must fail rather than being padded or
        // ignored during reconstruction. Cover every sumcheck family.
        let mut bad = compact.clone();
        bad.norm.sumchecks[0].round_polynomials.pop();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compact_products
            .sumcheck
            .round_polynomials
            .push([field.zero(); 3]);
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest[4]
            .sumcheck
            .as_mut()
            .unwrap()
            .round_polynomials
            .pop();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_leaf.sumcheck.round_polynomials.clear();
        reject(&bad);
        let mut bad = compact.clone();
        bad.norm.sumchecks[0].round_polynomials[0][1] =
            field.add(&bad.norm.sumchecks[0].round_polynomials[0][1], &field.one());
        reject(&bad);
        let mut bad = compact.clone();
        let wider = field::FpCtx::from_prime_u128((1u128 << 127) - 1);
        bad.compact_products.sumcheck.round_polynomials[0][0] =
            unsigned(field.modulus_u128(), &wider);
        reject(&bad);
        let mut bad = compact.clone();
        bad.compact_products.grinding_nonces.push(0);
        reject(&bad);
        // The uncompressed entry point must still reject inconsistent metadata.
        let mut bad = expanded;
        bad.norm.slack = field.add(&bad.norm.slack, &field.one());
        assert!(
            verify_falcon_piop_in_field(&mut Blake3Transcript::new(), &layout, &bad, 100, &field,)
                .is_err()
        );
    }
}
