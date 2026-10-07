// Shared-prime integer messages with derived metadata omitted.
//
// The verifier reconstructs each full round before absorption, preserving the
// prover transcript. Weighted forest rounds already store two coefficients.
// Only the checked endpoint claims survive verification.

use super::*;
use crate::sumcheck::{
    SumcheckError,
    boundary::RoundBoundaryPolicy,
    proof::{evaluate_polynomial, reconstruct_round_coefficients},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CompactSumcheck<const COEFFS: usize> {
    /// Coefficients [c0, c2, ...]; c1 = claim - 2*c0 - c2 - ... .
    pub(in super::super) round_polynomials: Vec<[F; COEFFS]>,
}

impl<const COEFFS: usize> CompactSumcheck<COEFFS> {
    pub(super) fn from_full<const FULL: usize>(proof: SumcheckProof<F, FULL>) -> Self {
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

/// Reconstruct c1 from the incoming claim and absorb the full coefficient array.
pub(super) fn verify_round_proofs<const FULL: usize, const COMPACT: usize, const K: usize>(
    proofs: [&CompactSumcheck<COMPACT>; K],
    transcript: &mut impl Transcript,
    initial_claims: &[F; K],
    rounds: usize,
    field: &Cfg,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<(Vec<F>, [F; K]), SumcheckError> {
    assert_eq!(FULL, COMPACT + 1);
    if K == 0 {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    boundary.validate(rounds)?;
    validate_field_elements(initial_claims, field)?;
    for proof in proofs {
        let actual = proof.round_polynomials.len();
        if actual != rounds {
            return Err(SumcheckError::InvalidRoundCount {
                expected: rounds,
                actual,
            });
        }
        for coefficients in &proof.round_polynomials {
            validate_field_elements(coefficients, field)?;
        }
    }
    let zero = field.zero();
    let mut claims = *initial_claims;
    let mut point = Vec::with_capacity(rounds);
    for round in 0..rounds {
        let coefficients: [[F; FULL]; K] = std::array::from_fn(|i| {
            reconstruct_round_coefficients(
                &claims[i],
                &proofs[i].round_polynomials[round],
                &zero,
                field,
            )
        });
        for coefficients in &coefficients {
            absorb_field_elements(transcript, coefficients, field);
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
/// Borrowed endpoints for the source binder, derived by the prover or verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopClaimRef<'a> {
    pub modulus: u128,
    pub norm: NormClaimRef<'a>,
    pub compact_products: QuadraticClaimRef<'a>,
    pub compaction_gamma: F,
    pub compaction_rank_scale: F,
    pub compaction: &'a CompactionProof,
    pub compaction_leaf: LeafClaimRef<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct NormClaims {
    /// Random signature weights; inactive signatures contribute zero.
    pub(in super::super) instance_point: Vec<F>,
    pub(in super::super) terminal: [[F; 2]; 2],
    pub(in super::super) slack: F,
    pub(in super::super) point: Vec<F>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct QuadraticClaims {
    pub(in super::super) terminal: OuterEvaluations<F>,
    pub(in super::super) point: Vec<F>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct LeafClaims {
    /// Inherited from the verified forest; no fresh signature challenge.
    pub(in super::super) instance_point: Vec<F>,
    pub(in super::super) terminal: [F; 3],
    pub(in super::super) point: Vec<F>,
}
/// Derived endpoints for subsequent source authentication, without round messages or nonces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopClaims {
    pub(in super::super) modulus: u128,
    pub(in super::super) norm: NormClaims,
    pub(in super::super) compact_products: QuadraticClaims,
    pub(in super::super) compaction_gamma: F,
    pub(in super::super) compaction_rank_scale: F,
    pub(in super::super) compaction: CompactionProof,
    pub(in super::super) compaction_leaf: LeafClaims,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct NormProof {
    pub(in super::super) instance_nonce: Option<u64>,
    pub(in super::super) claims: [F; 2],
    pub(in super::super) sumchecks: [CompactSumcheck<2>; 2],
    pub(in super::super) terminal: [[F; 2]; 2],
    pub(in super::super) grinding_nonces: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct QuadraticRelationProof {
    pub(in super::super) point_nonce: Option<u64>,
    pub(in super::super) sumcheck: CompactSumcheck<3>,
    pub(in super::super) terminal: OuterEvaluations<F>,
    pub(in super::super) grinding_nonces: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
/// Authenticates forest leaves through the MLE of the weighted operand table,
/// rather than a product of independently evaluated weight and operand MLEs.
pub(in super::super) struct CompactionLeafProof {
    pub(in super::super) sumcheck: CompactSumcheck<3>,
    pub(in super::super) terminal: [F; 3],
    pub(in super::super) grinding_nonces: Vec<u64>,
}
/// Stored messages and nonces. The verifier derives all endpoint metadata and
/// the omitted sumcheck coefficients. Forest rounds already use their compact
/// weighted encoding; no derived points are retained in this proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct FalconPiopProof {
    pub(in super::super) norm: NormProof,
    pub(in super::super) compact_products: QuadraticRelationProof,
    pub(in super::super) fingerprint_nonce: Option<u64>,
    pub(in super::super) compaction_forest: PrimeProductForestProof,
    pub(in super::super) compaction_endpoints: [F; 2],
    pub(in super::super) compaction_leaf: CompactionLeafProof,
}

impl FalconPiopProof {
    /// Visit every stored arithmetic nonce without expanding compact rounds.
    pub(in super::super) fn visit_grinding_nonces(&self, mut visit: impl FnMut(&'static str, u64)) {
        let Self {
            norm,
            compact_products,
            fingerprint_nonce,
            compaction_forest,
            compaction_endpoints: _,
            compaction_leaf,
        } = self;
        let NormProof {
            instance_nonce,
            claims: _,
            sumchecks: _,
            terminal: _,
            grinding_nonces,
        } = norm;
        for &nonce in instance_nonce.iter().chain(grinding_nonces) {
            visit("prime_norm", nonce);
        }
        let QuadraticRelationProof {
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
        for &nonce in compaction_forest.root_nonce.iter() {
            visit("prime_forest", nonce);
        }
        for ProductForestLayerProof {
            round_polynomials: _,
            evaluations: _,
            grinding_nonces,
            line_nonce,
        } in &compaction_forest.layers
        {
            for &nonce in grinding_nonces.iter().chain(line_nonce) {
                visit("prime_forest", nonce);
            }
        }
        for &nonce in &compaction_leaf.grinding_nonces {
            visit("prime_leaf", nonce);
        }
    }

    /// Canonical scalar payload, excluding container framing.
    pub(in super::super) fn payload_size_bytes(&self) -> usize {
        let Self {
            norm,
            compact_products,
            fingerprint_nonce,
            compaction_forest,
            compaction_endpoints: _,
            compaction_leaf,
        } = self;
        let nonce = |value: Option<u64>| 8 * usize::from(value.is_some());
        let NormProof {
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
        let QuadraticRelationProof {
            point_nonce,
            sumcheck,
            terminal: _,
            grinding_nonces,
        } = compact_products;
        let product_bytes = 3 * 16
            + 3 * 16 * sumcheck.round_polynomials.len()
            + nonce(*point_nonce)
            + 8 * grinding_nonces.len();
        let forest_bytes = nonce(compaction_forest.root_nonce)
            + compaction_forest
                .layers
                .iter()
                .map(|layer| {
                    2 * 16 * layer.round_polynomials.len()
                        + 2 * 16
                        + 8 * layer.grinding_nonces.len()
                        + nonce(layer.line_nonce)
                })
                .sum::<usize>();
        let CompactionLeafProof {
            sumcheck,
            terminal,
            grinding_nonces,
        } = compaction_leaf;
        let leaf_bytes = 16 * terminal.len()
            + 3 * 16 * sumcheck.round_polynomials.len()
            + 8 * grinding_nonces.len();
        norm_bytes + product_bytes + nonce(*fingerprint_nonce) + forest_bytes + 2 * 16 + leaf_bytes
    }
}

/// Verifies borrowed compact messages in the already-sampled ring field. Forest
/// evaluations and round arrays are never cloned into the returned endpoints.
pub(in super::super) fn verify_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &FalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopClaims, FalconError> {
    if !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid shared PIOP security target"));
    }
    validate_shared_field(target_bits, field)?;
    bind_shared_header(transcript, layout, target_bits, field);
    let norm = verify_norm(transcript, layout, &proof.norm, target_bits, field)?;

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
    let security = security_schedule(layout, target_bits)?;
    let compact_products = verify_quadratic(
        transcript,
        compaction_product_rounds(layout),
        &proof.compact_products,
        target_bits,
        security,
        field,
    )?;

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

    let compaction = verify_product_forest_payload(
        transcript,
        &proof.compaction_forest.layers,
        proof.compaction_forest.root_nonce,
        proof.compaction_endpoints,
        layout.batch(),
        target_bits,
        security,
        field,
    )?;
    let compaction_leaf = verify_compaction_leaf(
        transcript,
        layout,
        &compaction,
        &proof.compaction_leaf,
        target_bits,
        field,
    )?;
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
falcon_tests! {
mod tests {
    use super::super::super::verification_trace;
    use super::*;
    use crate::transcript::Blake3Transcript;

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
    fn compact_piop_derives_the_same_claims_and_transcript() {
        for (batch, target) in [(1, 100), (3, 100), (1, 128)] {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let (prime_min, prime_max) =
                super::super::super::shared_ring::prime_bounds(target).unwrap();
            let field = crate::prime_sampling::sample_prime_context(
                &mut Blake3Transcript::new(),
                prime_min,
                prime_max,
                128,
            )
            .unwrap();
            let mut prover = RecordingTranscript::new();
            let (proof, claims) = prove_falcon_piop_in_field(
                &mut prover,
                &layout,
                &vec![fixture(); batch],
                target,
                &field,
            )
            .unwrap();
            let mut verifier = RecordingTranscript::new();
            let restored =
                verify_falcon_piop_in_field(&mut verifier, &layout, &proof, target, &field)
                    .unwrap();
            assert_eq!(restored, claims);
            assert_eq!(prover.absorbed, verifier.absorbed);
            assert_eq!(
                prover.get_challenge::<u128>(),
                verifier.get_challenge::<u128>()
            );

            let m = layout.capacity().ilog2() as usize;
            let forest_rounds =
                COMPACTION_LOG * (m + 1) + COMPACTION_LOG * (COMPACTION_LOG - 1) / 2;
            assert_eq!(
                proof.norm.sumchecks[0].round_polynomials.len(),
                COEFFICIENT_LOG + m
            );
            assert_eq!(
                proof.compact_products.sumcheck.round_polynomials.len(),
                COMPACTION_LOG + m
            );
            assert_eq!(
                proof.compaction_leaf.sumcheck.round_polynomials.len(),
                COMPACTION_LOG + m
            );
            assert_eq!(proof.compaction_forest.layers.len(), COMPACTION_LOG);
            assert_eq!(
                proof
                    .compaction_forest
                    .layers
                    .iter()
                    .map(|l| l.round_polynomials.len())
                    .sum::<usize>(),
                forest_rounds,
            );
            let mut nonces = 0;
            proof.visit_grinding_nonces(|_, _| nonces += 1);
            // Fourteen fixed field messages, two quadratic norm streams,
            // two cubic streams, and weighted forest rounds/child pairs.
            let fields = 14
                + 4 * (COEFFICIENT_LOG + m)
                + 6 * (COMPACTION_LOG + m)
                + 2 * forest_rounds
                + 2 * COMPACTION_LOG;
            assert_eq!(proof.payload_size_bytes(), 16 * fields + 8 * nonces);
        }
    }

    #[test]
    fn compact_piop_rejects_changed_messages_and_forest_shapes() {
        let layout = FalconSourceLayout::new(1).unwrap();
        let (prime_min, prime_max) = super::super::super::shared_ring::prime_bounds(100).unwrap();
        let field = crate::prime_sampling::sample_prime_context(
            &mut Blake3Transcript::new(),
            prime_min,
            prime_max,
            128,
        )
        .unwrap();
        let (compact, _) = prove_falcon_piop_in_field(
            &mut Blake3Transcript::new(),
            &layout,
            &[fixture()],
            100,
            &field,
        )
        .unwrap();
        let reject = |proof: &FalconPiopProof| {
            assert!(
                verify_falcon_piop_in_field(
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
        bad.compaction_forest.layers[0].evaluations[0] = field.zero();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_endpoints[0] = field.add(&bad.compaction_endpoints[0], &field.one());
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers.pop();
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
        bad.compaction_forest.layers[4].round_polynomials.pop();
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
        let mut bad = compact.clone();
        bad.compaction_forest.root_nonce = Some(0);
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers[0].round_polynomials.clear();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers[0].round_polynomials[0][1] =
            unsigned(field.modulus_u128(), &wider);
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers[0].evaluations[0] = unsigned(field.modulus_u128(), &wider);
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_endpoints[1] = unsigned(field.modulus_u128(), &wider);
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers[0].grinding_nonces.push(0);
        reject(&bad);
    }
}

}
