//! Stored shared-prime integer proof without transcript-derived metadata.
//!
//! Verification reconstructs the ordinary PIOP view for the existing binder.
//! Reconstructed values are absorbed at their original transcript positions.

use super::*;

#[derive(Clone, Debug)]
struct SharedNormProof {
    instance_nonce: Option<u64>,
    claims: [F; 2],
    sumchecks: [SumcheckProof<F, 3>; 2],
    terminal: [[F; 2]; 2],
    grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug)]
struct SharedQuadraticProof {
    point_nonce: Option<u64>,
    sumcheck: SumcheckProof<F, 4>,
    terminal: OuterEvaluations<F>,
    grinding_nonces: Vec<u64>,
}

#[derive(Clone, Debug)]
struct SharedLeafProof {
    instance_nonce: Option<u64>,
    sumcheck: SumcheckProof<F, 4>,
    terminal: [F; 3],
    grinding_nonces: Vec<u64>,
}

/// Only messages and nonces are stored. The field, challenge points, norm slack,
/// fingerprint challenges, forest roots and forest endpoints are reconstructed.
#[derive(Clone, Debug)]
pub(in super::super) struct SharedFalconPiopProof {
    norm: SharedNormProof,
    compact_products: SharedQuadraticProof,
    fingerprint_nonce: Option<u64>,
    compaction_forest: PrimeProductForestProof,
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
                sumchecks: norm.sumchecks,
                terminal: norm.terminal,
                grinding_nonces: norm.grinding_nonces,
            },
            compact_products: SharedQuadraticProof {
                point_nonce: compact_products.point_nonce,
                sumcheck: compact_products.sumcheck,
                terminal: compact_products.terminal,
                grinding_nonces: compact_products.grinding_nonces,
            },
            fingerprint_nonce,
            compaction_forest,
            compaction_leaf: SharedLeafProof {
                instance_nonce: compaction_leaf.instance_nonce,
                sumcheck: compaction_leaf.sumcheck,
                terminal: compaction_leaf.terminal,
                grinding_nonces: compaction_leaf.grinding_nonces,
            },
        }
    }
}

impl SharedFalconPiopProof {
    /// Canonical scalar payload, using the same framing exclusions as the
    /// existing Falcon payload metric. No derived metadata is stored or counted.
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
                .map(|s| 16 * 3 * s.round_polynomials.len())
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
            + 4 * 16 * sumcheck.round_polynomials.len()
            + nonce(*point_nonce)
            + 8 * grinding_nonces.len();
        let forest_bytes = compaction_forest
            .layers
            .iter()
            .map(|layer| {
                let ProductForestLayerProof {
                    sumcheck,
                    evaluations,
                    grinding_nonces,
                    batching_nonce,
                    line_nonce,
                } = layer;
                sumcheck
                    .as_ref()
                    .map_or(0, |s| 4 * 16 * s.round_polynomials.len())
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
            + 4 * 16 * sumcheck.round_polynomials.len()
            + nonce(*instance_nonce)
            + 8 * grinding_nonces.len();
        norm_bytes + product_bytes + nonce(*fingerprint_nonce) + forest_bytes + leaf_bytes
    }
}

/// Replays the same shared integer transcript and returns the checked expanded
/// view needed by the source binder. `field` is the already-sampled ring prime.
pub(in super::super) fn verify_shared_falcon_piop_in_field(
    transcript: &mut impl Transcript,
    layout: &FalconSourceLayout,
    proof: &SharedFalconPiopProof,
    target_bits: usize,
    field: &Cfg,
) -> Result<FalconPiopProof, FalconError> {
    if !matches!(target_bits, 100 | 128) {
        return Err(piop("invalid shared PIOP security target"));
    }
    validate_shared_field(layout, field)?;
    bind_shared_header(transcript, layout, target_bits, field);
    let mut norm = NormProof {
        instance_point: Vec::new(),
        instance_nonce: proof.norm.instance_nonce,
        claims: proof.norm.claims,
        slack: field.zero(),
        sumchecks: proof.norm.sumchecks.clone(),
        terminal: proof.norm.terminal,
        point: Vec::new(),
        grinding_nonces: proof.norm.grinding_nonces.clone(),
    };
    (norm.instance_point, norm.point, norm.slack) =
        verify_norm_payload(transcript, layout, &norm, target_bits, field, false)?;

    transcript.absorb_slice(b"bitz/falcon1024-ct/compaction-products/v3");
    let mut compact_products = QuadraticRelationProof {
        point_nonce: proof.compact_products.point_nonce,
        sumcheck: proof.compact_products.sumcheck.clone(),
        terminal: proof.compact_products.terminal,
        point: Vec::new(),
        grinding_nonces: proof.compact_products.grinding_nonces.clone(),
    };
    let security = security_schedule(layout, target_bits)?;
    compact_products.point = verify_quadratic_payload(
        transcript,
        compaction_product_rounds(layout),
        &compact_products,
        target_bits,
        security,
        field,
        false,
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

    // The root equality already forces roots to these level-zero products.
    // No verifier randomness intervenes between absorbing roots and the first
    // layer evaluations, so replay the original duplicated roots at that point.
    let first = proof
        .compaction_forest
        .layers
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
        &proof.compaction_forest,
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

    let mut compaction_leaf = CompactionLeafProof {
        instance_point: Vec::new(),
        instance_nonce: proof.compaction_leaf.instance_nonce,
        sumcheck: proof.compaction_leaf.sumcheck.clone(),
        terminal: proof.compaction_leaf.terminal,
        point: Vec::new(),
        grinding_nonces: proof.compaction_leaf.grinding_nonces.clone(),
    };
    (compaction_leaf.instance_point, compaction_leaf.point) = verify_compaction_leaf_payload(
        transcript,
        layout,
        &compaction,
        &compaction_leaf,
        target_bits,
        field,
        false,
    )?;
    Ok(FalconPiopProof {
        modulus: field.modulus_u128(),
        norm,
        compact_products,
        fingerprint_nonce: proof.fingerprint_nonce,
        compaction_gamma,
        compaction_rank_scale,
        compaction,
        compaction_forest: proof.compaction_forest.clone(),
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
            let field = sample_field(&mut Blake3Transcript::new()).unwrap();
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
            assert_eq!(restored, expanded);
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
            assert!(compact.payload_size_bytes() > 0);
        }
    }

    #[test]
    fn compact_shared_piop_rejects_changed_messages_and_forest_shapes() {
        let layout = FalconSourceLayout::new_shared_prime(1).unwrap();
        let field = sample_field(&mut Blake3Transcript::new()).unwrap();
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
        bad.compaction_forest.layers[0].evaluations[0][0] = field.zero();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers[0].evaluations.pop();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_forest.layers.pop();
        reject(&bad);
        let mut bad = compact.clone();
        bad.compaction_leaf.terminal[0] = field.add(&bad.compaction_leaf.terminal[0], &field.one());
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
