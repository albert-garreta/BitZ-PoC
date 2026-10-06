//! Accounting for the stored proof payload; Falcon does not yet define a
//! transport codec. Scalar widths are canonical, not Rust allocation sizes.
use super::super::{
    native_ring::{EXTENSION_DEGREE, NativeRingProof},
    opening::FalconBindingPrefixProof,
    piop::{
        CompactionLeafProof, CompactionProof, FalconPiopProof, NormProof, PrimeProductForestProof,
        ProductForestLayerProof, QuadraticRelationProof,
    },
};
use super::*;

const FIELD_BYTES: usize = 16;
const NONCE_BYTES: usize = 8;
const HASH_BYTES: usize = 32;
const EXTENSION_BYTES: usize = 2 * EXTENSION_DEGREE;

impl FalconHybridProof {
    /// Untimed diagnostics as `(category, stored_nonce_boundaries, prefix_sum)`.
    /// `prefix_sum` is sum(nonce + 1), accumulated in u128. It describes the
    /// serial ascending nonce prefixes, not executed hashes: parallel/SIMD
    /// overscan is excluded, and zero-difficulty placeholders still count as
    /// one stored prefix entry. No proof or transcript data is changed.
    pub fn grinding_diagnostics(&self) -> Vec<(&'static str, usize, u128)> {
        let Self {
            arithmetic,
            bridge,
            keccak,
            links_nonces,
            joint,
            opening,
            opening_nonces,
            pcs_nonces,
        } = self;
        let mut counts = std::collections::BTreeMap::from([
            ("ring_projection", (0, 0)),
            ("native_ring_carry", (0, 0)),
            ("prime_norm", (0, 0)),
            ("prime_products", (0, 0)),
            ("prime_fingerprint", (0, 0)),
            ("prime_forest", (0, 0)),
            ("prime_leaf", (0, 0)),
            ("prime_binder", (0, 0)),
            ("binary_bridge", (0, 0)),
            ("binary_keccak", (0, 0)),
            ("binary_links", (0, 0)),
            ("binary_joint", (0, 0)),
            ("binary_opening", (0, 0)),
            ("pcs_auxiliary", (0, 0)),
            ("pcs_queries", (0, 0)),
            ("pcs_folds", (0, 0)),
            ("pcs_ood", (0, 0)),
        ]);
        let mut visit = |category, nonce| add_nonce(&mut counts, category, nonce);
        let FalconBindingPrefixProof {
            piop,
            ring,
            linear_point_nonce,
            binding: _,
            binding_point: _,
            binding_terminal: _,
            binding_nonces,
        } = arithmetic;
        match piop {
            super::super::opening::PiopProof::Native(proof) => {
                visit_native_prime_nonces(proof, &mut visit)
            }
            super::super::opening::PiopProof::Shared(proof) => {
                proof.visit_grinding_nonces(&mut visit)
            }
        }
        match ring {
            super::super::opening::RingProof::Native(proof) => {
                let NativeRingProof {
                    certificate: _,
                    instance_point: _,
                    alpha: _,
                    carries: _,
                    carry_nonce,
                } = proof;
                for &nonce in carry_nonce.iter() {
                    visit("native_ring_carry", nonce);
                }
            }
            super::super::opening::RingProof::Shared(proof) => {
                let super::super::shared_ring::Proof {
                    certificate: _,
                    outer: _,
                    evaluations: _,
                    lift: _,
                    projection_nonce,
                } = proof;
                visit("ring_projection", *projection_nonce);
            }
        }
        for &nonce in linear_point_nonce.iter().chain(binding_nonces) {
            visit("prime_binder", nonce);
        }
        let hybrid_bridge::Proof {
            sums: _,
            forest: _,
            nonces,
        } = bridge;
        for &nonce in nonces {
            visit("binary_bridge", nonce);
        }
        for hybrid_keccak::PrefixProof {
            zerocheck: _,
            lincheck: _,
            grinding_nonces,
        } in keccak
        {
            for &nonce in grinding_nonces {
                visit("binary_keccak", nonce);
            }
        }
        for &nonce in links_nonces {
            visit("binary_links", nonce);
        }
        let joint::Proof {
            rounds: _,
            value: _,
            nonces,
        } = joint;
        for &nonce in nonces {
            visit("binary_joint", nonce);
        }
        for &nonce in opening_nonces {
            visit("binary_opening", nonce);
        }
        for &nonce in pcs_nonces {
            visit("pcs_auxiliary", nonce);
        }
        let shared::Proof {
            ood,
            ring: _,
            ligerito,
            paths: _,
        } = opening;
        if let Some(crate::ligerito_flock::OodRound { y: _, nonce }) = ood {
            for &nonce in nonce.iter() {
                visit("pcs_ood", nonce);
            }
        }
        let flock_core::pcs::ligerito::LigeritoProof {
            initial_root: _,
            initial_proof: _,
            recursive_roots: _,
            recursive_proofs: _,
            final_proof: _,
            sumcheck_transcript: _,
            grinding_nonces,
            ood_values: _,
            fold_grinding_nonces,
        } = ligerito;
        for &nonce in grinding_nonces {
            visit("pcs_queries", nonce);
        }
        for &nonce in fold_grinding_nonces {
            visit("pcs_folds", nonce);
        }
        counts
            .into_iter()
            .map(|(category, (boundaries, prefix_sum))| (category, boundaries, prefix_sum))
            .collect()
    }

    /// Size of this proof's stored payload in bytes, including redundant
    /// challenge points. Prime/binary fields use 16 bytes, extension elements
    /// use eleven canonical u16 coordinates, carries/nonces use eight bytes,
    /// and hashes use 32 bytes. Keccak and Ligerito use their existing bincode
    /// serialization sizes, including those components' internal framing.
    ///
    /// This excludes the public statement and Falcon-level vector lengths,
    /// option tags, protocol headers, and transport framing. It is a payload
    /// accounting metric, not a wire-format proof size or an allocation size.
    pub fn payload_size_bytes(&self) -> usize {
        let Self {
            arithmetic,
            bridge,
            keccak,
            links_nonces,
            joint,
            opening,
            opening_nonces,
            pcs_nonces,
        } = self;
        arithmetic_bytes(arithmetic)
            + bridge_bytes(bridge)
            + keccak.iter().map(serialized_bytes).sum::<usize>()
            + NONCE_BYTES * links_nonces.len()
            + joint_bytes(joint)
            + opening_bytes(opening)
            + NONCE_BYTES * (opening_nonces.len() + pcs_nonces.len())
    }
}

fn add_nonce(
    counts: &mut std::collections::BTreeMap<&'static str, (usize, u128)>,
    category: &'static str,
    nonce: u64,
) {
    let entry = counts.entry(category).or_default();
    entry.0 += 1;
    entry.1 += u128::from(nonce) + 1;
}

fn visit_native_prime_nonces(proof: &FalconPiopProof, mut visit: impl FnMut(&'static str, u64)) {
    let FalconPiopProof {
        modulus: _,
        norm,
        compact_products,
        fingerprint_nonce,
        compaction_gamma: _,
        compaction_rank_scale: _,
        compaction: _,
        compaction_forest,
        compaction_leaf,
    } = proof;
    let NormProof {
        instance_point: _,
        instance_nonce,
        claims: _,
        slack: _,
        sumchecks: _,
        terminal: _,
        point: _,
        grinding_nonces,
    } = norm;
    for &nonce in instance_nonce.iter().chain(grinding_nonces) {
        visit("prime_norm", nonce);
    }
    let QuadraticRelationProof {
        point_nonce,
        sumcheck: _,
        terminal: _,
        point: _,
        grinding_nonces,
    } = compact_products;
    for &nonce in point_nonce.iter().chain(grinding_nonces) {
        visit("prime_products", nonce);
    }
    for &nonce in fingerprint_nonce.iter() {
        visit("prime_fingerprint", nonce);
    }
    let PrimeProductForestProof { root_nonce, layers } = compaction_forest;
    for &nonce in root_nonce.iter() {
        visit("prime_forest", nonce);
    }
    for ProductForestLayerProof {
        round_polynomials: _,
        evaluations: _,
        grinding_nonces,
        line_nonce,
    } in layers
    {
        for &nonce in grinding_nonces.iter().chain(line_nonce) {
            visit("prime_forest", nonce);
        }
    }
    let CompactionLeafProof {
        instance_point: _,
        sumcheck: _,
        terminal: _,
        point: _,
        grinding_nonces,
    } = compaction_leaf;
    for &nonce in grinding_nonces {
        visit("prime_leaf", nonce);
    }
}

fn serialized_bytes(value: &impl serde::Serialize) -> usize {
    bincode::serialized_size(value).expect("proof component serialization size") as usize
}

fn nonce_bytes(nonce: &Option<u64>) -> usize {
    usize::from(nonce.is_some()) * NONCE_BYTES
}

fn sumcheck_bytes<F, const COEFFICIENTS: usize>(
    proof: &crate::sumcheck::SumcheckProof<F, COEFFICIENTS>,
) -> usize {
    proof.round_polynomials.len() * COEFFICIENTS * FIELD_BYTES
}

fn arithmetic_bytes(proof: &FalconBindingPrefixProof) -> usize {
    let FalconBindingPrefixProof {
        piop,
        ring,
        linear_point_nonce,
        binding,
        binding_point,
        binding_terminal,
        binding_nonces,
    } = proof;
    (match piop {
        super::super::opening::PiopProof::Native(proof) => piop_bytes(proof),
        super::super::opening::PiopProof::Shared(proof) => proof.payload_size_bytes(),
    }) + match ring {
        super::super::opening::RingProof::Native(proof) => native_bytes(proof),
        super::super::opening::RingProof::Shared(proof) => proof.payload_size_bytes(),
    } + nonce_bytes(linear_point_nonce)
        + sumcheck_bytes(binding)
        + FIELD_BYTES * (binding_point.len() + binding_terminal.len())
        + NONCE_BYTES * binding_nonces.len()
}

fn native_bytes(proof: &NativeRingProof) -> usize {
    let NativeRingProof {
        certificate,
        instance_point,
        alpha: _,
        carries,
        carry_nonce,
    } = proof;
    EXTENSION_BYTES * (certificate.len() + instance_point.len() + 1)
        + NONCE_BYTES * carries.len()
        + nonce_bytes(carry_nonce)
}

fn piop_bytes(proof: &FalconPiopProof) -> usize {
    let FalconPiopProof {
        modulus: _,
        norm,
        compact_products,
        fingerprint_nonce,
        compaction_gamma: _,
        compaction_rank_scale: _,
        compaction,
        compaction_forest,
        compaction_leaf,
    } = proof;
    // The stored modulus and both stored fingerprint challenges.
    3 * FIELD_BYTES
        + norm_bytes(norm)
        + quadratic_bytes(compact_products)
        + nonce_bytes(fingerprint_nonce)
        + compaction_bytes(compaction)
        + forest_bytes(compaction_forest)
        + leaf_bytes(compaction_leaf)
}

fn norm_bytes(proof: &NormProof) -> usize {
    let NormProof {
        instance_point,
        instance_nonce,
        claims,
        slack: _,
        sumchecks,
        terminal,
        point,
        grinding_nonces,
    } = proof;
    FIELD_BYTES
        * (instance_point.len()
            + claims.len()
            + 1
            + terminal.iter().map(|pair| pair.len()).sum::<usize>()
            + point.len())
        + sumchecks.iter().map(sumcheck_bytes).sum::<usize>()
        + nonce_bytes(instance_nonce)
        + NONCE_BYTES * grinding_nonces.len()
}

fn quadratic_bytes(proof: &QuadraticRelationProof) -> usize {
    let QuadraticRelationProof {
        point_nonce,
        sumcheck,
        terminal,
        point,
        grinding_nonces,
    } = proof;
    let crate::sumcheck::outer::OuterEvaluations {
        ax: _,
        bx: _,
        cx: _,
    } = terminal;
    nonce_bytes(point_nonce)
        + sumcheck_bytes(sumcheck)
        + FIELD_BYTES * (3 + point.len())
        + NONCE_BYTES * grinding_nonces.len()
}

fn compaction_bytes(proof: &CompactionProof) -> usize {
    let CompactionProof {
        instance_point,
        terminal_point,
        candidate: _,
        output: _,
    } = proof;
    FIELD_BYTES * (2 + instance_point.len() + terminal_point.len())
}

fn forest_bytes(proof: &PrimeProductForestProof) -> usize {
    let PrimeProductForestProof { root_nonce, layers } = proof;
    nonce_bytes(root_nonce)
        + layers
            .iter()
            .map(
                |ProductForestLayerProof {
                     round_polynomials,
                     evaluations,
                     grinding_nonces,
                     line_nonce,
                 }| {
                    2 * FIELD_BYTES * round_polynomials.len()
                        + FIELD_BYTES * evaluations.len()
                        + NONCE_BYTES * grinding_nonces.len()
                        + nonce_bytes(line_nonce)
                },
            )
            .sum::<usize>()
}

fn leaf_bytes(proof: &CompactionLeafProof) -> usize {
    let CompactionLeafProof {
        instance_point,
        sumcheck,
        terminal,
        point,
        grinding_nonces,
    } = proof;
    FIELD_BYTES * (instance_point.len() + terminal.len() + point.len())
        + sumcheck_bytes(sumcheck)
        + NONCE_BYTES * grinding_nonces.len()
}

fn bridge_bytes(proof: &hybrid_bridge::Proof) -> usize {
    let hybrid_bridge::Proof {
        sums,
        forest,
        nonces,
    } = proof;
    FIELD_BYTES * sums.iter().map(Vec::len).sum::<usize>()
        + 2 * FIELD_BYTES * forest.len()
        + NONCE_BYTES * nonces.len()
}

fn joint_bytes(proof: &joint::Proof) -> usize {
    let joint::Proof {
        rounds,
        value: _,
        nonces,
    } = proof;
    FIELD_BYTES * (2 * rounds.len() + 1) + NONCE_BYTES * nonces.len()
}

fn opening_bytes(proof: &shared::Proof<3>) -> usize {
    let shared::Proof {
        ood,
        ring,
        ligerito,
        paths,
    } = proof;
    let crate::ligerito::RingSwitchProof { s_v } = ring;
    ood.as_ref()
        .map_or(0, |crate::ligerito_flock::OodRound { y: _, nonce }| {
            FIELD_BYTES + nonce_bytes(nonce)
        })
        + FIELD_BYTES * s_v.len()
        + serialized_bytes(ligerito)
        + HASH_BYTES * paths.iter().map(Vec::len).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::super::super::native_ring::Ext;
    use super::*;

    #[test]
    fn nonce_prefix_accounting_keeps_zero_and_full_u64_values_exact() {
        let mut counts = std::collections::BTreeMap::new();
        add_nonce(&mut counts, "test", 0);
        add_nonce(&mut counts, "test", u64::MAX);
        add_nonce(&mut counts, "other", 7);
        assert_eq!(counts["test"], (2, (1u128 << 64) + 1));
        assert_eq!(counts["other"], (1, 8));
    }

    #[test]
    fn native_payload_counts_canonical_certificate_carries_and_challenges() {
        let mut proof = NativeRingProof {
            certificate: vec![Ext([7; EXTENSION_DEGREE]); 1023],
            instance_point: vec![Ext([9; EXTENSION_DEGREE]); 10],
            alpha: Ext([3; EXTENSION_DEGREE]),
            carries: [-17; EXTENSION_DEGREE],
            carry_nonce: Some(29),
        };
        // Direct byte writing is independent of the size formula. This is not
        // a Falcon codec: it intentionally contains no framing or decoding.
        let mut bytes = Vec::new();
        for element in proof
            .certificate
            .iter()
            .chain(&proof.instance_point)
            .chain(std::iter::once(&proof.alpha))
        {
            for coordinate in element.0 {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
        }
        for carry in proof.carries {
            bytes.extend_from_slice(&carry.to_le_bytes());
        }
        bytes.extend_from_slice(&proof.carry_nonce.unwrap().to_le_bytes());
        assert_eq!(native_bytes(&proof), bytes.len());
        let baseline = native_bytes(&proof);
        proof.certificate.push(Ext([0; EXTENSION_DEGREE]));
        assert_eq!(native_bytes(&proof) - baseline, 22);
        proof.instance_point.push(Ext([0; EXTENSION_DEGREE]));
        assert_eq!(native_bytes(&proof) - baseline, 44);
        proof.carry_nonce = None;
        assert_eq!(native_bytes(&proof) - baseline, 36);
        proof.certificate.clear();
        proof.instance_point.clear();
        assert_eq!(native_bytes(&proof), 22 + 11 * 8);
    }

    #[test]
    fn bridge_payload_counts_each_integer_sum_and_binary_pair() {
        let proof = hybrid_bridge::Proof {
            sums: vec![vec![1, 2, 3], vec![4, 5]],
            forest: vec![[Gf::zero(); 2]; 7],
            nonces: vec![0; 11],
        };
        assert_eq!(bridge_bytes(&proof), 5 * 16 + 7 * 32 + 11 * 8);
    }

    #[test]
    fn shared_piop_payload_counts_only_stored_coefficients_and_endpoints() {
        use crate::piop::spartan::falcon1024_ct::{
            FalconSourceLayout, piop::prove_falcon_piop_in_field, verification_trace,
        };
        let layout = FalconSourceLayout::new_shared_prime(1).unwrap();
        let trace = verification_trace(
            include_bytes!("fixtures/public_key.bin"),
            include_bytes!("fixtures/message.bin"),
            include_bytes!("fixtures/signature_ct.bin"),
        )
        .unwrap();
        let (prime_min, prime_max) =
            crate::piop::spartan::falcon1024_ct::shared_ring::prime_bounds(100).unwrap();
        let field = crate::prime_sampling::sample_prime_context(
            &mut crate::transcript::Blake3Transcript::new(),
            prime_min,
            prime_max,
            128,
        )
        .unwrap();
        let full = prove_falcon_piop_in_field(
            &mut crate::transcript::Blake3Transcript::new(),
            &layout,
            &[trace],
            100,
            &field,
        )
        .unwrap();
        let full_bytes = piop_bytes(&full);
        // Distinct synthetic nonces exercise every stored arithmetic field,
        // including optional forest root/line boundaries. Compression
        // must preserve this inventory even though it removes other fields.
        let mut audited = full.clone();
        audited.norm.instance_nonce = Some(0);
        audited.norm.grinding_nonces = vec![1, 2];
        audited.compact_products.point_nonce = Some(3);
        audited.compact_products.grinding_nonces = vec![4];
        audited.fingerprint_nonce = Some(5);
        audited.compaction_forest.root_nonce = Some(6);
        audited.compaction_leaf.grinding_nonces = vec![7, 8];
        for (i, layer) in audited.compaction_forest.layers.iter_mut().enumerate() {
            layer.grinding_nonces = vec![10 + i as u64];
            layer.line_nonce = Some(50 + i as u64);
        }
        let mut native_nonces = Vec::new();
        visit_native_prime_nonces(&audited, |category, nonce| {
            native_nonces.push((category, nonce))
        });
        let layers = audited.compaction_forest.layers.len();
        let mut compact_nonces = Vec::new();
        audited
            .into_shared()
            .visit_grinding_nonces(|category, nonce| compact_nonces.push((category, nonce)));
        assert_eq!(native_nonces, compact_nonces);
        let mut counts = std::collections::BTreeMap::new();
        for (category, nonce) in native_nonces {
            add_nonce(&mut counts, category, nonce);
        }
        assert_eq!(counts["prime_norm"], (3, 6));
        assert_eq!(counts["prime_products"], (2, 9));
        assert_eq!(counts["prime_fingerprint"], (1, 6));
        assert_eq!(counts["prime_leaf"], (2, 17));
        assert_eq!(
            counts["prime_forest"],
            (
                1 + 2 * layers,
                (7 + 62 * layers + layers * (layers - 1)) as u128
            )
        );
        let stored = full.into_shared();
        // The forest is already compact; omit only derived points and the
        // norm, rejection, and candidate-leaf linear coefficients.
        assert_eq!(
            full_bytes - stored.payload_size_bytes(),
            752 + 42 * FIELD_BYTES
        );
    }
}
