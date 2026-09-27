//! Accounting for the stored proof payload; Falcon does not yet define a
//! transport codec. Scalar widths are canonical, not Rust allocation sizes.
use super::super::{
    native_ring::{EXTENSION_DEGREE, NativeRingProof},
    opening::FalconBindingPrefixProof,
    piop::{
        CompactionLeafProof, CompactionProof, FalconPiopProof, NormProof, PrimeProductForestProof,
        PrimeProductTreeProof, ProductForestLayerProof, QuadraticRelationProof,
    },
};
use super::*;

const FIELD_BYTES: usize = 16;
const NONCE_BYTES: usize = 8;
const HASH_BYTES: usize = 32;
const EXTENSION_BYTES: usize = 2 * EXTENSION_DEGREE;

impl FalconHybridProof {
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
        native_ring,
        linear_point_nonce,
        binding,
        binding_point,
        binding_terminal,
        binding_nonces,
    } = proof;
    piop_bytes(piop)
        + native_bytes(native_ring)
        + nonce_bytes(linear_point_nonce)
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
        + compaction
            .iter()
            .map(|CompactionProof { candidate, output }| tree_bytes(candidate) + tree_bytes(output))
            .sum::<usize>()
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

fn tree_bytes(proof: &PrimeProductTreeProof) -> usize {
    let PrimeProductTreeProof {
        root: _,
        terminal_point,
        terminal_claim: _,
    } = proof;
    FIELD_BYTES * (2 + terminal_point.len())
}

fn forest_bytes(proof: &PrimeProductForestProof) -> usize {
    let PrimeProductForestProof { layers } = proof;
    layers
        .iter()
        .map(
            |ProductForestLayerProof {
                 sumcheck,
                 evaluations,
                 grinding_nonces,
                 batching_nonce,
                 line_nonce,
             }| {
                sumcheck.as_ref().map_or(0, sumcheck_bytes)
                    + 2 * FIELD_BYTES * evaluations.len()
                    + NONCE_BYTES * grinding_nonces.len()
                    + nonce_bytes(batching_nonce)
                    + nonce_bytes(line_nonce)
            },
        )
        .sum()
}

fn leaf_bytes(proof: &CompactionLeafProof) -> usize {
    let CompactionLeafProof {
        instance_point,
        instance_nonce,
        sumcheck,
        terminal,
        point,
        grinding_nonces,
    } = proof;
    FIELD_BYTES * (instance_point.len() + terminal.len() + point.len())
        + nonce_bytes(instance_nonce)
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
}
