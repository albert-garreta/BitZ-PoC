// Accounting for the stored proof payload; Falcon does not yet define a
// transport codec. Scalar widths are canonical, not Rust allocation sizes.
use super::super::opening::FalconBindingPrefixProof;
use super::*;

const FIELD_BYTES: usize = 16;
const NONCE_BYTES: usize = 8;
const HASH_BYTES: usize = 32;

impl FalconHybridProof {
    /// Encode just the integer column table: 16 bytes per unsplit column or
    /// 20 bytes per split column, in column order. The prepared profile fixes
    /// the representation and count. This does not encode the rest of Falcon.
    pub fn encode_integer_column_sums(&self) -> Vec<u8> {
        self.bridge.sums.encode()
    }

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
            ("prime_norm", (0, 0)),
            ("prime_h2p_rows", (0, 0)),
            ("prime_binder", (0, 0)),
            ("binary_bridge", (0, 0)),
            ("binary_keccak", (0, 0)),
            ("binary_links", (0, 0)),
            ("binary_joint", (0, 0)),
            ("binary_opening", (0, 0)),
            ("pcs_auxiliary", (0, 0)),
            ("pcs_queries", (0, 0)),
            ("pcs_folds", (0, 0)),
        ]);
        let mut visit = |category, nonce| add_nonce(&mut counts, category, nonce);
        let FalconBindingPrefixProof {
            selection_masks: _,
            piop,
            ring,
            linear_point_nonce,
            binding: _,
            binding_terminal: _,
            binding_nonces,
        } = arithmetic;
        piop.visit_grinding_nonces(&mut visit);
        visit("ring_projection", ring.projection_nonce);
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
        let shared::JointProof { ring: _, ligerito } = opening;
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

    /// Size of this proof's compact stored payload in bytes. Prime/binary
    /// fields use 16 bytes, extension elements use K canonical u16 coordinates,
    /// nonces use eight bytes, and hashes use 32 bytes. Integer sums use 16 bytes
    /// for the lower sum
    /// and four bytes for the upper sum when split. Keccak and compact Ligerito
    /// use their bincode serialization sizes, including internal framing.
    /// Ligerito stores only occupied initial lanes and the final pre-fold
    /// message; reconstructed rows and final paths are not proof payload.
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

    /// Disjoint byte counts under the same accounting as `payload_size_bytes`.
    /// This inspects stored messages only; it does not change the transcript.
    pub fn payload_size_breakdown(&self) -> Vec<(&'static str, usize)> {
        let piop = self.arithmetic.piop.payload_size_bytes();
        let ring = self.arithmetic.ring.payload_size_bytes();
        let selection = self
            .arithmetic
            .selection_masks
            .iter()
            .map(Vec::len)
            .sum::<usize>();
        let pcs = &self.opening.ligerito;
        let rows_bytes = |rows: &[Vec<flock_core::field::Gf128>]| {
            FIELD_BYTES * rows.iter().map(Vec::len).sum::<usize>()
        };
        let initial_rows = rows_bytes(&pcs.initial_proof.opened_rows);
        let initial_paths = HASH_BYTES * pcs.initial_proof.merkle_proof.len();
        let recursive_rows = pcs
            .recursive_proofs
            .iter()
            .map(|proof| rows_bytes(&proof.opened_rows))
            .sum::<usize>();
        let recursive_paths = HASH_BYTES
            * pcs
                .recursive_proofs
                .iter()
                .map(|proof| proof.merkle_proof.len())
                .sum::<usize>();
        let final_message = FIELD_BYTES * pcs.final_proof.len();
        let roots = HASH_BYTES * (1 + pcs.recursive_roots.len());
        let sumchecks = 2 * FIELD_BYTES * pcs.sumcheck_transcript.len();
        let ood = FIELD_BYTES * pcs.ood_values.len();
        let nonces = NONCE_BYTES * (pcs.grinding_nonces.len() + pcs.fold_grinding_nonces.len());
        let pcs_payload = initial_rows
            + initial_paths
            + recursive_rows
            + recursive_paths
            + final_message
            + roots
            + sumchecks
            + ood
            + nonces;
        let parts = vec![
            ("arithmetic_piop", piop),
            ("ring_certificate_and_reduction", ring),
            ("hash_to_point_selection_masks", selection),
            (
                "arithmetic_source_binding",
                arithmetic_bytes(&self.arithmetic) - piop - ring - selection,
            ),
            (
                "integer_column_sums_first_limb",
                self.bridge.sums.lower_encoded_len(),
            ),
            (
                "integer_column_sums_remaining_limbs",
                self.bridge.sums.upper_encoded_len(),
            ),
            (
                "binary_bridge_gkr",
                2 * FIELD_BYTES * self.bridge.forest.len(),
            ),
            (
                "binary_bridge_nonces",
                NONCE_BYTES * self.bridge.nonces.len(),
            ),
            (
                "keccak_piops",
                self.keccak.iter().map(serialized_bytes).sum(),
            ),
            ("binary_link_nonces", NONCE_BYTES * self.links_nonces.len()),
            ("joint_binary_sumcheck", joint_bytes(&self.joint)),
            (
                "opening_ring_switch",
                FIELD_BYTES * self.opening.ring.s_v.len(),
            ),
            ("source_authentication_joint", initial_paths),
            ("pcs_initial_opened_rows", initial_rows),
            ("pcs_recursive_opened_rows", recursive_rows),
            ("pcs_recursive_authentication", recursive_paths),
            ("pcs_final_message", final_message),
            ("pcs_roots", roots),
            ("pcs_sumchecks", sumchecks),
            ("pcs_ood_values", ood),
            ("pcs_nonces", nonces),
            ("pcs_bincode_framing", serialized_bytes(pcs) - pcs_payload),
            (
                "opening_and_auxiliary_nonces",
                NONCE_BYTES * (self.opening_nonces.len() + self.pcs_nonces.len()),
            ),
        ];
        debug_assert_eq!(
            parts.iter().map(|(_, bytes)| bytes).sum::<usize>(),
            self.payload_size_bytes()
        );
        parts
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
        selection_masks,
        piop,
        ring,
        linear_point_nonce,
        binding,
        binding_terminal,
        binding_nonces,
    } = proof;
    selection_masks.iter().map(Vec::len).sum::<usize>()
        + piop.payload_size_bytes()
        + ring.payload_size_bytes()
        + nonce_bytes(linear_point_nonce)
        + sumcheck_bytes(binding)
        + FIELD_BYTES * binding_terminal.len()
        + NONCE_BYTES * binding_nonces.len()
}

fn bridge_bytes(proof: &hybrid_bridge::Proof) -> usize {
    let hybrid_bridge::Proof {
        sums,
        forest,
        nonces,
    } = proof;
    sums.encoded_len() + 2 * FIELD_BYTES * forest.len() + NONCE_BYTES * nonces.len()
}

fn joint_bytes(proof: &joint::Proof) -> usize {
    let joint::Proof {
        rounds,
        value: _,
        nonces,
    } = proof;
    FIELD_BYTES * (2 * rounds.len() + 1) + NONCE_BYTES * nonces.len()
}

fn opening_bytes(proof: &shared::JointProof) -> usize {
    let shared::JointProof { ring, ligerito } = proof;
    let crate::ligerito::RingSwitchProof { s_v } = ring;
    FIELD_BYTES * s_v.len() + serialized_bytes(ligerito)
}
falcon_tests! {
mod tests {
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
    fn bridge_payload_counts_each_integer_sum_and_binary_pair() {
        let proof = hybrid_bridge::Proof {
            sums: crate::bitz::column_sums::ColumnSums::Split(vec![
                crate::bitz::column_sums::LargeNumber { lower: 1, upper: 4 },
                crate::bitz::column_sums::LargeNumber { lower: 2, upper: 5 },
            ]),
            forest: vec![[Gf::zero(); 2]; 7],
            nonces: vec![0; 11],
        };
        assert_eq!(
            bridge_bytes(&proof),
            proof.sums.encode().len() + 7 * 32 + 11 * 8
        );
        assert_eq!(proof.sums.encoded_len(), 40);
    }

    #[test]
    fn shared_piop_payload_counts_only_stored_coefficients_and_endpoints() {
        use crate::piop::spartan::falcon_profiles::n1024_k11::{
            FalconSourceLayout, piop::prove_falcon_piop_in_field, verification_trace,
        };
        let layout = FalconSourceLayout::new(1).unwrap();
        let trace = verification_trace(
            include_bytes!("fixtures/public_key.bin"),
            include_bytes!("fixtures/message.bin"),
            include_bytes!("fixtures/signature_ct.bin"),
        )
        .unwrap();
        let (prime_min, prime_max) =
            crate::piop::spartan::falcon_profiles::n1024_k11::shared_ring::prime_bounds(100).unwrap();
        let field = crate::prime_sampling::sample_prime_context(
            &mut crate::transcript::Blake3Transcript::new(),
            prime_min,
            prime_max,
            128,
        )
        .unwrap();
        let (stored, _) = prove_falcon_piop_in_field(
            &mut crate::transcript::Blake3Transcript::new(),
            &layout,
            &[trace],
            100,
            &field,
        )
        .unwrap();
        // Distinct synthetic nonces exercise every stored arithmetic boundary.
        let mut audited = stored.clone();
        audited.norm.instance_nonce = Some(0);
        audited.norm.grinding_nonces = vec![1, 2];
        audited.h2p_rejection.rows.point_nonce = Some(3);
        audited.h2p_rejection.rows.grinding_nonces = vec![4];
        let mut counts = std::collections::BTreeMap::new();
        audited.visit_grinding_nonces(|category, nonce| add_nonce(&mut counts, category, nonce));
        assert_eq!(counts["prime_norm"], (3, 6));
        assert_eq!(counts["prime_h2p_rows"], (2, 9));
        // One signature: two 10-round norms and one 11-round rejection relation.
        // At target 100 none of these arithmetic messages has a stored nonce.
        let norm = (2 + 4 + 2 * 10 * 2) * FIELD_BYTES;
        let rejection = (3 + 11 * 3) * FIELD_BYTES;
        assert_eq!(stored.payload_size_bytes(), norm + rejection);
    }
}

}
