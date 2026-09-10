//! Non-ZK composition of multiplication modulo 2^32 and Binius64 SHA-256.
//!
//! Each multiplication proves x*y = z + 2^32*w for four committed u32 limbs.
//! The integer backend uses the fixed linear reconstruction p = z + 2^32*w
//! and proves x*y = p. The compact witness remains 128 bits per operation.
//!
//! Order: commit both witnesses, Round 0 (the out-of-domain sample of the
//! virtual packed witness), Spartan/F2Z GKR, Binius SHA PIOP, joint bit
//! sumcheck, one ring switch, one Ligerito continuation. The SHA workload is
//! a sequential compression chain starting from the standard SHA-256 IV.
mod channel;
mod codec;
pub mod mod32_binius;
mod opening;
mod security;
mod sha;
pub(crate) mod sumcheck;

pub use crate::piop::spartan::u32_mul::U32MulMod32Row;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::{
    ligerito_flock::OodRoundParams,
    piop::spartan::{
        f2z::{PreparedU32MulRelation, hybrid as mul},
        u32_mul::{U32MulLayout, U32MulWitness},
    },
    transcript::{Blake3Transcript, traits::Transcript},
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use flock_core::{
    field::F128,
    pcs::commit::{ProverData, commit},
};
pub use security::{CompositionProfile, LIGERITO_COMPONENT_BITS, SecurityReport, SecurityTerm};
pub use sha::{SHA256_IV, chaining_value};

pub const DEFAULT_MULTIPLICATIONS: usize = 1 << 20;
pub const DEFAULT_SHA_COMPRESSIONS: usize = 1 << 16;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid hybrid proof or parameters: {0}")]
    Invalid(&'static str),
    #[error("Ligerito configuration: {0}")]
    Config(String),
    #[error("Binius: {0}")]
    Binius(String),
    #[error(transparent)]
    Multiplication(#[from] crate::piop::spartan::f2z::SpartanF2zError),
    #[error(transparent)]
    Relation(#[from] crate::piop::spartan::u32_mul::U32MulError),
    #[error(transparent)]
    Spartan(#[from] crate::piop::spartan::piop::SpartanError),
    #[error(transparent)]
    Grinding(#[from] crate::piop::spartan::grinding::GrindingError),
    #[error(transparent)]
    Codec(#[from] crate::proof_codec::CodecError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Parameters {
    pub multiplications: usize,
    pub sha_compressions: usize,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            multiplications: DEFAULT_MULTIPLICATIONS,
            sha_compressions: DEFAULT_SHA_COMPRESSIONS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Statement {
    pub parameters: Parameters,
    pub roots: [[u8; 32]; 2],
    pub final_sha_state: [u32; 8],
}

#[derive(Clone, Debug)]
pub struct HybridProof {
    multiplication: mul::PrefixProof,
    sha: Vec<u128>,
    joint: sumcheck::Proof,
    opening: opening::Proof,
}

pub(crate) struct BinaryClaim {
    pub low: Vec<F128>,
    pub high_point: Vec<F128>,
    pub value: F128,
}

/// Prepared circuit and verifier parameters. Preparation is independent of the
/// witness and final SHA state; it is reusable across instances of this shape.
pub struct PreparedHybrid {
    parameters: Parameters,
    multiplication: PreparedU32MulRelation,
    sha: sha::ShaRelation,
    geometry: opening::Geometry,
    /// Round-0 parameters (`step0:ood-draw` grinding), derived from the
    /// shared opener's configuration at the composition profile's λ.
    ood: Option<OodRoundParams>,
    ligerito: crate::ligerito_flock::ResolvedLigerito,
    security: SecurityReport,
}

/// Witness and the two initial commitments, retained until proof generation.
pub struct CommittedHybrid {
    statement: Statement,
    multiplication: U32MulWitness,
    rows: Vec<Vec<u64>>,
    sha: binius_core::constraint_system::ValueVec,
    packed: [Vec<F128>; 2],
    data: [ProverData; 2],
}

impl CommittedHybrid {
    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    /// The committed x, y, modular result z, and carry w for every live row.
    pub fn multiplication_rows(&self) -> impl ExactSizeIterator<Item = U32MulMod32Row> + '_ {
        self.multiplication.mod32_rows()
    }
}

impl PreparedHybrid {
    pub fn new(parameters: Parameters) -> Result<Self, Error> {
        Self::new_with_ligerito(parameters, crate::ligerito_flock::LigeritoSelection::JOHNSON)
    }

    pub fn new_with_ligerito(parameters: Parameters, selection: crate::ligerito_flock::LigeritoSelection) -> Result<Self, Error> {
        if cfg!(feature = "unchecked") {
            return Err(Error::Invalid(
                "hybrid proofs require checked arithmetic and constraints",
            ));
        }
        if !parameters.multiplications.is_power_of_two()
            || !(1 << 15..=1 << 22).contains(&parameters.multiplications)
        {
            return Err(Error::Invalid(
                "multiplication count must be a power of two from 2^15 to 2^22",
            ));
        }
        if !parameters.sha_compressions.is_power_of_two()
            || !(2..=1 << 16).contains(&parameters.sha_compressions)
        {
            return Err(Error::Invalid(
                "SHA compression count must be a power of two from 2 to 2^16",
            ));
        }
        let layout = U32MulLayout::new(parameters.multiplications)?;
        let multiplication =
            PreparedU32MulRelation::new_with_profile::<security::CompositionProfile>(layout)?;
        let sha = sha::ShaRelation::new(parameters.sha_compressions)?;
        let geometry = opening::Geometry::new([
            crate::ligerito::packed_vars(&multiplication.params()),
            sha.verifier.log_witness_elems(),
        ])?;
        let ligerito = selection.resolve(geometry.packed_log(), security::LIGERITO_COMPONENT_BITS).map_err(Error::Config)?;
        if ligerito.prover().initial_k != 4 || ligerito.prover().log_inv_rates[0] != 3 {
            return Err(Error::Invalid("hybrid requires Ligerito rate 1/8 and initial_k=4"));
        }
        let security = security::account(&multiplication, &sha.verifier, &geometry, &ligerito)?;
        let ood = opening::ood_parameters(&ligerito)?.map(|(_, params)| params);
        Ok(Self {
            parameters,
            multiplication,
            sha,
            geometry,
            ood,
            ligerito,
            security,
        })
    }

    pub fn parameters(&self) -> Parameters {
        self.parameters
    }
    pub fn security(&self) -> &SecurityReport {
        &self.security
    }
    pub fn packed_witness_logs(&self) -> [usize; 2] {
        self.geometry.logs
    }
    /// Round-0 (out-of-domain sample) parameters of the shared opening.
    pub fn ood_round(&self) -> Option<OodRoundParams> {
        self.ood
    }

    pub fn ligerito_configuration(&self) -> &crate::ligerito_flock::ResolvedLigerito { &self.ligerito }
    pub fn physical_packed_witness_logs(&self) -> [usize; 2] { self.geometry.physical_logs }

    /// Generate z = x*y mod 2^32 and w = floor(x*y / 2^32), then commit
    /// the four 32-bit limbs and the chained SHA witness.
    pub fn commit(
        &self,
        inputs: &[(u32, u32)],
        blocks: &[[u32; 16]],
    ) -> Result<CommittedHybrid, Error> {
        let multiplication = U32MulWitness::from_inputs(inputs)?;
        self.commit_multiplication(multiplication, blocks)
    }

    /// Commit supplied claims x*y = z + 2^32*w, with all four limbs u32.
    /// Supplied z and w are retained; the proof checks the equality.
    pub fn commit_mod32(
        &self,
        rows: &[U32MulMod32Row],
        blocks: &[[u32; 16]],
    ) -> Result<CommittedHybrid, Error> {
        self.commit_multiplication(U32MulWitness::from_mod32_rows(rows)?, blocks)
    }

    fn commit_multiplication(
        &self,
        multiplication: U32MulWitness,
        blocks: &[[u32; 16]],
    ) -> Result<CommittedHybrid, Error> {
        if multiplication.layout() != self.multiplication.layout()
            || blocks.len() != self.parameters.sha_compressions
        {
            return Err(Error::Invalid("witness workload counts"));
        }
        let rows_scope = crate::utils::prof::scope("hc:mul_bit_rows");
        let rows = multiplication.f2z_bit_rows();
        drop(rows_scope);
        let pack_scope = crate::utils::prof::scope("hc:mul_pack");
        let words_per_row = rows.first().map_or(0, |row| row.len() / 2);
        let mut packed_mul = vec![F128::ZERO; rows.len() * words_per_row];
        crate::utils::cfg_chunks_mut!(packed_mul, words_per_row.max(1))
            .zip(crate::utils::cfg_iter!(rows))
            .for_each(|(dst, row)| {
                for (word, w) in dst.iter_mut().zip(row.chunks_exact(2)) {
                    *word = F128 { lo: w[0], hi: w[1] };
                }
            });
        drop(pack_scope);
        let sha_scope = crate::utils::prof::scope("hc:sha_populate");
        let final_sha_state = chaining_value(blocks);
        let sha = self.sha.populate(blocks, final_sha_state)?;
        drop(sha_scope);
        let sha_pack_scope = crate::utils::prof::scope("hc:sha_pack");
        let mut packed_sha = self.sha.pack(&sha);
        packed_mul.resize(1 << self.geometry.physical_logs[0], F128::ZERO);
        packed_sha.resize(1 << self.geometry.physical_logs[1], F128::ZERO);
        drop(sha_pack_scope);
        let commit_mul_scope = crate::utils::prof::scope("hc:commit_mul");
        let (c_mul, d_mul) = commit(&packed_mul, &self.geometry.params(0));
        drop(commit_mul_scope);
        let commit_sha_scope = crate::utils::prof::scope("hc:commit_sha");
        let (c_sha, d_sha) = commit(&packed_sha, &self.geometry.params(1));
        drop(commit_sha_scope);
        let statement = Statement {
            parameters: self.parameters,
            roots: [c_mul.root, c_sha.root],
            final_sha_state,
        };
        Ok(CommittedHybrid {
            statement,
            multiplication,
            rows,
            sha,
            packed: [packed_mul, packed_sha],
            data: [d_mul, d_sha],
        })
    }

    fn transcript(&self, statement: &Statement) -> Result<(Blake3Transcript, [u8; 32]), Error> {
        if statement.parameters != self.parameters {
            return Err(Error::Invalid("statement workload parameters"));
        }
        let mut h = blake3::Hasher::new();
        h.update(b"f2z/hybrid-u32-mod32-sha256/non-zk/lanes4-padding/v5");
        for n in [
            self.parameters.multiplications,
            self.parameters.sha_compressions,
            100,
            self.geometry.logs[0],
            self.geometry.logs[1],
            self.geometry.physical_logs[0],
            self.geometry.physical_logs[1],
            self.geometry.position_log,
            self.geometry.virtual_lane_log,
        ] {
            h.update(&(n as u64).to_le_bytes());
        }
        for root in &statement.roots {
            h.update(root);
        }
        for word in statement.final_sha_state {
            h.update(&word.to_be_bytes());
        }
        h.update(
            &bincode::serialize(self.ligerito.security())
                .map_err(|e| Error::Config(e.to_string()))?,
        );
        let digest = *h.finalize().as_bytes();
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"hybrid/statement/v5");
        transcript.absorb_slice(&digest);
        self.ligerito.bind(&mut transcript);
        Ok((transcript, digest))
    }

    pub fn prove(&self, committed: &CommittedHybrid) -> Result<HybridProof, Error> {
        let (mut t, digest) = self.transcript(&committed.statement)?;
        let sources = [&committed.packed[0][..], &committed.packed[1][..]];
        // Round 0 precedes every other challenge: it pins the committed
        // virtual witness to one element of the opener's level-0 list.
        tracing::info!("Round 0: out-of-domain sample of the virtual packed witness");
        let ood_scope = crate::utils::prof::scope("hybrid:ood_round");
        let packed = self.geometry.virtual_packed(sources);
        let ood = opening::prove_ood(&mut t, self.ood, &packed);
        drop(ood_scope);
        tracing::info!("proving multiplication constraints and GKR");
        let (multiplication, a) = mul::prove(
            &mut t,
            &self.multiplication,
            &committed.multiplication,
            &committed.rows,
            &digest,
        )?;
        tracing::info!("proving chained SHA constraints");
        let sha_scope = crate::utils::prof::scope("hybrid:sha_piop");
        let (sha, b) = self.sha.prove(&mut t, &committed.sha)?;
        drop(sha_scope);
        tracing::info!("proving shared bit sumcheck");
        let sumcheck_scope = crate::utils::prof::scope("hybrid:joint_sumcheck");
        let (joint, point) = sumcheck::prove(&mut t, &self.geometry, sources, [&a, &b]);
        drop(sumcheck_scope);
        tracing::info!("ring switching and opening both roots with Ligerito");
        let opening_scope = crate::utils::prof::scope("hybrid:opening_iop");
        let opening = opening::prove(
            &mut t,
            &self.geometry,
            &digest,
            packed,
            ood.as_ref(),
            &self.ligerito,            [&committed.data[0], &committed.data[1]],
            &point,
        )?;
        drop(opening_scope);
        Ok(HybridProof {
            multiplication,
            sha,
            joint,
            opening,
        })
    }

    pub fn verify(&self, statement: &Statement, proof: &HybridProof) -> Result<(), Error> {
        let (mut t, digest) = self.transcript(statement)?;
        let ood = opening::verify_ood(&mut t, &self.geometry, self.ood, proof.opening.ood.as_ref())?;
        let a = mul::verify(&mut t, &self.multiplication, &digest, &proof.multiplication)?;
        let public = self.sha.public(statement.final_sha_state);
        let b = self.sha.verify(&mut t, &public, &proof.sha)?;
        let point = sumcheck::verify(&mut t, &self.geometry, [&a, &b], &proof.joint)?;
        opening::verify(
            &mut t,
            &self.geometry,
            &digest,
            &statement.roots,
            &point,
            proof.joint.value,
            ood.as_ref(),
            &self.ligerito,            &proof.opening,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_sources_recommitted_nonzero_are_rejected_in_both_regimes() {
        use crate::ligerito_flock::LigeritoSelection;
        let parameters = Parameters { multiplications: 1 << 15, sha_compressions: 4 };
        let inputs: Vec<_> = (0..1u32 << 15).map(|i| (i, i.wrapping_mul(31337))).collect();
        let blocks = [[17u32; 16]; 4];
        for selection in [LigeritoSelection::JOHNSON, LigeritoSelection::MATCHED_UDR] {
            let prepared = PreparedHybrid::new_with_ligerito(parameters, selection).unwrap();
            let mut committed = prepared.commit(&inputs, &blocks).unwrap();
            let proof = prepared.prove(&committed).unwrap();
            prepared.verify(committed.statement(), &proof).unwrap();
            let bytes = proof.to_bytes();
            let decoded = prepared.proof_from_bytes(committed.statement(), &bytes).unwrap();
            prepared.verify(committed.statement(), &decoded).unwrap();
            let branch = (0..2).find(|&b| prepared.geometry.physical_logs[b] > prepared.geometry.logs[b]).expect("padded source");
            let padding_index = 1 << prepared.geometry.logs[branch];
            committed.packed[branch][padding_index] = F128::ONE;
            let (commitment, data) = commit(&committed.packed[branch], &prepared.geometry.params(branch));
            committed.data[branch] = data;
            committed.statement.roots[branch] = commitment.root;
            // Both the RS codeword and Merkle root now authenticate the bad
            // padding. Only the fresh, zero-valued linear check excludes it.
            let invalid = prepared.prove(&committed).unwrap();
            assert!(prepared.verify(committed.statement(), &invalid).is_err());
        }
    }

    #[test]
    fn supported_geometries_bound_initial_leaf_width() {
        for multiplication_log in 15..=22 {
            for sha_log in 9..=24 {
                let g = opening::Geometry::new([multiplication_log, sha_log]).unwrap();
                assert_eq!(g.virtual_lane_log, 4);
                assert_eq!(g.params(0).n_positions(), g.params(1).n_positions());
                for selection in [crate::ligerito_flock::LigeritoSelection::JOHNSON, crate::ligerito_flock::LigeritoSelection::MATCHED_UDR] {
                let resolved = selection.resolve(g.packed_log(), security::LIGERITO_COMPONENT_BITS).unwrap();
                let security = resolved.security();
                assert!(security.validate().is_ok());
                // The commit rate must equal the opener's level-0 rate.
                assert_eq!(g.params(0).log_inv_rate, security.levels[0].log_inv_rate);
                assert_eq!(g.params(1).log_inv_rate, security.levels[0].log_inv_rate);
                assert!(opening::ood_parameters(&resolved).is_ok());
                }
            }
        }
    }

    #[test]
    fn field_representations_agree() {
        use binius_field::Field;
        use binius_verifier::config::B128;
        assert_eq!(u128::from(B128::ONE), 1);
        let values = [0, 1, 2, u128::MAX, 0x0123456789abcdef0123456789abcdef];
        for a in values {
            for b in values {
                let x = F128 {
                    lo: a as u64,
                    hi: (a >> 64) as u64,
                };
                let y = F128 {
                    lo: b as u64,
                    hi: (b >> 64) as u64,
                };
                let z = x * y;
                assert_eq!(
                    u128::from(B128::new(a) * B128::new(b)),
                    z.lo as u128 | ((z.hi as u128) << 64)
                );
            }
        }
    }

    #[test]
    fn chained_sha_matches_known_digest() {
        // SHA-256("abc") is one padded block. Check the native compression
        // semantics, including the chosen IV and SHA word byte order.
        let mut block = [0u32; 16];
        block[0] = 0x61626380;
        block[15] = 24;
        assert_eq!(
            chaining_value(&[block]),
            [
                0xba7816bf, 0x8f01cfea, 0x414140de, 0x5dae2223, 0xb00361a3, 0x96177a9c, 0xb410ff61,
                0xf20015ad
            ]
        );
        let two = [block, [0u32; 16]];
        assert_ne!(chaining_value(&two), chaining_value(&two[1..]));
    }

    #[test]
    fn modular_multiplication_claims_bind_each_limb() {
        let parameters = Parameters {
            multiplications: 1 << 15,
            sha_compressions: 4,
        };
        let prepared = PreparedHybrid::new(parameters).unwrap();
        let edges = [
            U32MulMod32Row::new(u32::MAX, u32::MAX),
            U32MulMod32Row::new(65_536, 65_536),
            U32MulMod32Row::new(u32::MAX, 2),
            U32MulMod32Row::new(0, u32::MAX),
            U32MulMod32Row::new(1, u32::MAX),
        ];
        assert_eq!((edges[0].z, edges[0].w), (1, u32::MAX - 1));
        assert_eq!((edges[1].z, edges[1].w), (0, 1));
        let rows: Vec<_> = (0..parameters.multiplications)
            .map(|i| edges[i % edges.len()])
            .collect();
        let blocks = [[0x12345678; 16]; 4];
        let committed = prepared.commit_mod32(&rows, &blocks).unwrap();
        assert_eq!(committed.multiplication_rows().collect::<Vec<_>>(), rows);
        let proof = prepared.prove(&committed).unwrap();
        prepared.verify(committed.statement(), &proof).unwrap();

        // The operand-only convenience API generates the exact same four
        // committed limbs; explicit claims can supply either result limb.
        let inputs: Vec<_> = rows.iter().map(|row| (row.x, row.y)).collect();
        let generated = prepared.commit(&inputs, &blocks).unwrap();
        assert_eq!(generated.statement(), committed.statement());
        for limb in 0..4 {
            let mut changed = rows.clone();
            let row = &mut changed[0];
            match limb {
                0 => row.x ^= 1,
                1 => row.y ^= 1,
                2 => row.z ^= 1,
                _ => row.w ^= 1,
            }
            let invalid = prepared.commit_mod32(&changed, &blocks).unwrap();
            assert_eq!(invalid.multiplication_rows().next(), Some(changed[0]));
            assert_ne!(invalid.statement().roots[0], committed.statement().roots[0]);
            assert!(prepared.verify(invalid.statement(), &proof).is_err());
            // Test the committed false relation, not just a host-side limb check.
            if let Ok(forged) = prepared.prove(&invalid) {
                assert!(prepared.verify(invalid.statement(), &forged).is_err());
            }
        }
    }

    #[test]
    fn hybrid_roundtrip_and_tampering() {
        let prepared = PreparedHybrid::new(Parameters {
            multiplications: 1 << 15,
            sha_compressions: 4,
        })
        .unwrap();
        eprintln!(
            "security={:?} packed={:?}",
            prepared.security(),
            prepared.packed_witness_logs()
        );
        let inputs: Vec<_> = (0..1u32 << 15)
            .map(|i| (i.wrapping_mul(0x9e3779b9), u32::MAX - i))
            .collect();
        let blocks = [
            [0xabcdef01; 16],
            [0x12345678; 16],
            [0xdeadbeef; 16],
            [0x76543210; 16],
        ];
        let committed = prepared.commit(&inputs, &blocks).unwrap();
        let proof = prepared.prove(&committed).unwrap();
        prepared.verify(committed.statement(), &proof).unwrap();
        let bytes = proof.to_bytes();
        let mut legacy = bytes.clone();
        legacy[4] = 4;
        assert!(
            prepared
                .proof_from_bytes(committed.statement(), &legacy)
                .is_err()
        );
        let decoded = prepared
            .proof_from_bytes(committed.statement(), &bytes)
            .unwrap();
        assert_eq!(bytes, decoded.to_bytes());
        prepared.verify(committed.statement(), &decoded).unwrap();
        for len in [0, 7, 8, 16, bytes.len() / 2, bytes.len() - 1] {
            assert!(
                prepared
                    .proof_from_bytes(committed.statement(), &bytes[..len])
                    .is_err()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(
            prepared
                .proof_from_bytes(committed.statement(), &trailing)
                .is_err()
        );
        // Wire layout: magic (8), OOD presence (8), value (16), nonce
        // presence (8), nonce (8), two prefix nonces (16), then their count.
        assert!(prepared.ood_round().unwrap().grinding_bits > 0);
        let mut malformed = bytes.clone();
        malformed[64..72].fill(255);
        assert!(
            prepared
                .proof_from_bytes(committed.statement(), &malformed)
                .is_err()
        );
        let mut noncanonical = bytes.clone();
        let first_field = 88 + proof.multiplication.piop_nonces.len() * 8;
        noncanonical[first_field..first_field + 16].fill(255);
        assert!(
            prepared
                .proof_from_bytes(committed.statement(), &noncanonical)
                .is_err()
        );
        let mut statement = committed.statement().clone();
        statement.final_sha_state[0] ^= 1;
        assert!(
            prepared
                .sha
                .populate(&blocks, statement.final_sha_state)
                .is_err()
        );
        assert!(prepared.verify(&statement, &proof).is_err());
        for branch in 0..2 {
            let mut statement = committed.statement().clone();
            statement.roots[branch][0] ^= 1;
            assert!(prepared.verify(&statement, &proof).is_err());
        }
        let mut changed = proof.clone();
        changed.joint.value += F128::ONE;
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        let mut changed = proof.clone();
        changed.sha.push(0);
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        let mut changed = proof.clone();
        changed.opening.ligerito.grinding_nonces.push(0);
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        // Round 0: the claimed value, its nonce and the deeper levels'
        // out-of-domain values and fold nonces are all bound.
        let mut changed = proof.clone();
        changed.opening.ood.as_mut().unwrap().y += Gf::one();
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        let mut changed = proof.clone();
        changed.opening.ood.as_mut().unwrap().nonce = changed.opening.ood.unwrap().nonce.map(|nonce| nonce ^ 1);
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        let mut changed = proof.clone();
        changed.opening.ood = None;
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        assert!(!proof.opening.ligerito.ood_values.is_empty());
        let mut changed = proof.clone();
        changed.opening.ligerito.ood_values[0] += F128::ONE;
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        let mut changed = proof.clone();
        changed.opening.ligerito.ood_values.push(F128::ONE);
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        assert!(!proof.opening.ligerito.fold_grinding_nonces.is_empty());
        let mut changed = proof.clone();
        changed.opening.ligerito.fold_grinding_nonces.pop();
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        let mut changed = proof.clone();
        changed.multiplication.sums[0] = u128::MAX;
        assert!(prepared.verify(committed.statement(), &changed).is_err());
        for branch in 0..2 {
            let mut changed = proof.clone();
            changed.opening.ligerito.initial_proof.opened_rows[0]
                [prepared.geometry.offset(branch)] += F128::ONE;
            assert!(prepared.verify(committed.statement(), &changed).is_err());
        }
        let padding = 1 << prepared.geometry.lane_logs[1];
        if padding < prepared.geometry.lanes() / 2 {
            let mut changed = proof.clone();
            changed.opening.ligerito.initial_proof.opened_rows[0]
                [prepared.geometry.offset(1) + padding] = F128::ONE;
            assert!(prepared.verify(committed.statement(), &changed).is_err());
        }
    }
}
