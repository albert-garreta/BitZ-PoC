//! Non-ZK Falcon composition with compact binary Keccak and one shared PCS opening.
use super::{
    FalconError, FalconPublicStatement, FalconSourceLayout, FalconSourceWitness,
    FalconVerificationTrace, KeccakTrace, decode_signature_ct, hybrid_bridge,
    hybrid_keccak::{
        self, KeccakAuxiliary, PreparedKeccak,
        grinding::{ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript},
    },
    hybrid_sumcheck::{self as joint, Coefficients, Gather, Tensor},
    opening::{FalconBindingPrefixProof, prove_binding_prefix, verify_binding_prefix},
};
use crate::{
    hybrid::{
        BinaryClaim, opening as shared,
        sumcheck::{Scratch, eq_table},
    },
    ligerito_flock::{
        LigeritoSelection, ResolvedLigerito,
        grinding::{GrindingContext, GrindingNonces, GrindingPlan},
    },
    piop::spartan::grinding::GrindingDomain,
    transcript::{Blake3Transcript, traits::Transcript},
};
use flock_core::{
    field::Gf128 as Gf,
    pcs::commit::{Commitment, ProverData, commit},
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::sync::Mutex;

fn error(e: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(e.to_string())
}
struct LinkGrinding;
impl GrindingDomain for LinkGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/link-grinding/v1";
}
struct OpeningGrinding;
impl GrindingDomain for OpeningGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/opening-grinding/v1";
}

/// Public keys, messages, and the two authenticated binary sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconHybridStatement {
    pub public: FalconPublicStatement,
    pub roots: [[u8; 32]; 2],
}

/// Union-bound terms in the existing computational grinding model. This is
/// algebraic/IOP accounting, separate from BLAKE3's 128-bit collision bound.
#[derive(Clone, Debug)]
pub struct FalconHybridSecurity {
    pub target_bits: usize,
    pub algebraic_bits: f64,
    pub terms: Vec<(&'static str, f64)>,
}

/// Prepared public circuit, reusable across witnesses of the same batch size.
pub struct PreparedFalconHybrid {
    layout: FalconSourceLayout,
    keccak: PreparedKeccak,
    geometry: shared::Geometry,
    target_bits: usize,
    ligerito: ResolvedLigerito,
    pcs_grinding: GrindingPlan,
    scratch: Mutex<Scratch>,
}

/// Packed witness and its commitments. Consumed by proving so large auxiliary
/// tables can be folded in place without cloning them.
pub struct CommittedFalconHybrid {
    pub statement: FalconHybridStatement,
    traces: Vec<FalconVerificationTrace>,
    arithmetic: FalconSourceWitness,
    auxiliary: KeccakAuxiliary,
    packed: [Vec<Gf>; 2],
    commitments: [Commitment; 2],
    data: [ProverData; 2],
}

#[derive(Clone, Debug)]
pub struct FalconHybridProof {
    arithmetic: FalconBindingPrefixProof,
    bridge: hybrid_bridge::Proof,
    keccak: hybrid_keccak::PrefixProof,
    links_nonces: Vec<u64>,
    joint: joint::Proof,
    opening: shared::Proof,
    opening_nonces: Vec<u64>,
    pcs_nonces: Vec<u64>,
}

impl PreparedFalconHybrid {
    /// Supports 1..=1024 live signatures, with canonical power-of-two padding.
    pub fn new(batch: usize, target_bits: usize) -> Result<Self, FalconError> {
        if !matches!(target_bits, 100 | 128) {
            return Err(error("hybrid security must be 100 or 128 bits"));
        }
        let layout = FalconSourceLayout::new_hybrid(batch)?;
        let keccak = PreparedKeccak::new(batch).map_err(error)?;
        let geometry = shared::Geometry::new([
            layout.source_bits().ilog2() as usize - 7,
            keccak.packed_vars(),
        ])
        .map_err(error)?;
        // Unique decoding avoids a level-zero list-selection obligation. The
        // exact Flock challenge-block plan raises the whole PCS budget above
        // the requested target, including its otherwise unground field draws.
        let ligerito = LigeritoSelection::MATCHED_UDR
            .resolve(geometry.packed_log(), target_bits)
            .map_err(error)?;
        let first =
            GrindingPlan::resolve(ligerito.security(), target_bits as u32).map_err(error)?;
        // Allocate one quarter of the total error budget to the whole PCS,
        // then divide that budget among its challenge blocks.
        let pcs_target = target_bits + 2 + first.blocks.len().next_power_of_two().ilog2() as usize;
        let pcs_grinding =
            GrindingPlan::resolve(ligerito.security(), pcs_target as u32).map_err(error)?;
        let prepared = Self {
            layout,
            keccak,
            geometry,
            target_bits,
            ligerito,
            pcs_grinding,
            scratch: Mutex::default(),
        };
        if prepared.security().algebraic_bits < target_bits as f64 {
            return Err(error(
                "Falcon hybrid composition misses its security target",
            ));
        }
        Ok(prepared)
    }

    pub fn batch(&self) -> usize {
        self.layout.batch()
    }
    pub fn capacity(&self) -> usize {
        self.layout.capacity()
    }
    pub fn target_bits(&self) -> usize {
        self.target_bits
    }
    pub fn source_bits_per_signature(&self) -> [usize; 2] {
        [self.layout.signature_stride(), 1 << 21]
    }

    pub fn security(&self) -> FalconHybridSecurity {
        let b = self.batch();
        let d = self.capacity().ilog2() as usize;
        let schedule = super::FalconSecuritySchedule::for_layout(self.target_bits, &self.layout)
            .expect("prepared target");
        let prime = |n: usize, g: u32| n as f64 * 2f64.powi(-125 - g as i32);
        let binary = |n: usize| n as f64 * 2f64.powi(-128 - self.binary_grinding(n) as i32);
        let terms = vec![
            ("prime sampling", 2f64.powi(-144)),
            (
                "per-signature norm identity",
                prime(d, schedule.norm_instance_bits),
            ),
            (
                "norm sumchecks",
                prime(4 * (10 + d), schedule.quadratic_round_bits),
            ),
            (
                "HashToPoint initial row point",
                prime(13 + d, schedule.outer_point_bits),
            ),
            (
                "HashToPoint product sumcheck",
                prime(3 * (13 + d), schedule.cubic_round_bits),
            ),
            (
                "ordered compaction fingerprints",
                prime(2048 * b, schedule.fingerprint_bits),
            ),
            (
                "ordered compaction forest",
                prime(165 + 10 * (2 * b - 1) + 22 * b, schedule.cubic_round_bits),
            ),
            (
                "linear constraints and terminal batching",
                prime(14 + d + 2 * b + 8, schedule.linear_point_bits),
            ),
            (
                "prime source sumcheck",
                prime(2 * (18 + d), schedule.binding_round_bits),
            ),
            ("integer-to-binary forests", binary(4096)),
            (
                "binary Keccak PIOP",
                self.keccak
                    .security((self.target_bits + 8) as u32)
                    .expect("prepared profile")
                    .error_bound(),
            ),
            ("SHAKE wiring and binary claim batching", binary(128)),
            ("joint binary sumcheck", binary(2 * self.geometry.bit_log())),
            ("ring switch and support padding", binary(256)),
            (
                "shared Ligerito",
                self.pcs_grinding
                    .blocks
                    .iter()
                    .map(|block| block.raw_error * 2f64.powi(-(block.bits as i32)))
                    .sum(),
            ),
        ];
        let algebraic_bits = -terms.iter().map(|(_, error)| error).sum::<f64>().log2();
        FalconHybridSecurity {
            target_bits: self.target_bits,
            algebraic_bits,
            terms,
        }
    }

    fn binary_grinding(&self, numerator: usize) -> u32 {
        (self.target_bits as u32 + 8 + numerator.next_power_of_two().ilog2()).saturating_sub(128)
    }

    /// Generate and commit the full witness, including SHAKE and HashToPoint.
    pub fn commit(
        &self,
        public: FalconPublicStatement,
        signatures: &[&[u8]],
    ) -> Result<CommittedFalconHybrid, FalconError> {
        let _span = tracing::info_span!("falcon_hybrid:witness_commit").entered();
        if public.batch() != self.batch()
            || public.messages.len() != self.batch()
            || signatures.len() != self.batch()
        {
            return Err(error("hybrid witness batch mismatch"));
        }
        let decoded = signatures
            .iter()
            .map(|s| decode_signature_ct(s))
            .collect::<Result<Vec<_>, _>>()?;
        let nonces: Vec<_> = decoded.iter().map(|s| s.nonce).collect();
        let witness = self
            .keccak
            .generate_witness(&nonces, &public.messages)
            .map_err(error)?;
        let traces = crate::utils::cfg_into_iter!(0..self.batch())
            .map(|i| {
                let bytes: Vec<_> = witness.samples[i]
                    .iter()
                    .flat_map(|w| w.to_be_bytes())
                    .collect();
                let htp = super::hash_to_point::from_shake_bytes(&bytes, KeccakTrace::default())?;
                super::verify::trace_from_parts(
                    public.public_keys[i].clone(),
                    decoded[i].clone(),
                    htp,
                    true,
                )
            })
            .collect::<Result<Vec<_>, FalconError>>()?;
        let messages: Vec<&[u8]> = public.messages.iter().map(|m| m.as_slice()).collect();
        let arithmetic =
            FalconSourceWitness::from_traces(self.layout, &messages, signatures, &traces)?;
        let mut arithmetic_packed = Vec::with_capacity(1 << self.geometry.physical_logs[0]);
        for row in arithmetic.rows() {
            arithmetic_packed.extend(row.chunks_exact(2).map(|w| Gf { lo: w[0], hi: w[1] }));
        }
        arithmetic_packed.resize(1 << self.geometry.physical_logs[0], Gf::ZERO);
        let packed = [arithmetic_packed, witness.packed];
        let rate = self.ligerito.prover().log_inv_rates[0];
        let (ca, da) = commit(&packed[0], &self.geometry.params(0, rate));
        let (ck, dk) = commit(&packed[1], &self.geometry.params(1, rate));
        let statement = FalconHybridStatement {
            public,
            roots: [ca.root, ck.root],
        };
        Ok(CommittedFalconHybrid {
            statement,
            traces,
            arithmetic,
            auxiliary: witness.auxiliary,
            packed,
            commitments: [ca, ck],
            data: [da, dk],
        })
    }

    fn transcript(
        &self,
        statement: &FalconHybridStatement,
    ) -> Result<(Blake3Transcript, [u8; 32]), FalconError> {
        if statement.public.batch() != self.batch()
            || statement.public.messages.len() != self.batch()
        {
            return Err(error("hybrid statement shape"));
        }
        if statement
            .public
            .public_keys
            .iter()
            .any(|key| key.h.iter().any(|&h| i64::from(h) >= super::Q))
        {
            return Err(error("noncanonical Falcon public key"));
        }
        let mut h = blake3::Hasher::new();
        h.update(b"bitz/falcon1024-ct/hybrid/non-zk/v1");
        for n in [
            self.batch(),
            self.capacity(),
            self.target_bits,
            self.geometry.logs[0],
            self.geometry.logs[1],
        ] {
            h.update(&(n as u64).to_le_bytes());
        }
        for root in &statement.roots {
            h.update(root);
        }
        for (pk, msg) in statement
            .public
            .public_keys
            .iter()
            .zip(&statement.public.messages)
        {
            for &coefficient in pk.h.iter() {
                h.update(&coefficient.to_le_bytes());
            }
            h.update(msg);
        }
        h.update(&self.ligerito.digest());
        for block in &self.pcs_grinding.blocks {
            h.update(&block.bits.to_le_bytes());
        }
        let digest = *h.finalize().as_bytes();
        let mut t = Blake3Transcript::new();
        t.absorb_slice(b"bitz/falcon-hybrid/statement/v1");
        t.absorb_slice(&digest);
        self.ligerito.bind(&mut t);
        Ok((t, digest))
    }

    pub fn prove(
        &self,
        committed: CommittedFalconHybrid,
    ) -> Result<FalconHybridProof, FalconError> {
        let (mut t, digest) = self.transcript(&committed.statement)?;
        let arithmetic_span = tracing::info_span!("falcon_hybrid:arithmetic_prefix").entered();
        let (arithmetic, _) = prove_binding_prefix(
            &mut t,
            &self.layout,
            &committed.statement.public,
            &committed.traces,
            &committed.arithmetic,
            self.target_bits,
        )?;
        drop(arithmetic_span);
        let bridge_span = tracing::info_span!("falcon_hybrid:binary_bridge").entered();
        let (bridge, a) = hybrid_bridge::prove(
            &mut t,
            &committed.arithmetic,
            &arithmetic.binding_point,
            arithmetic.piop.modulus,
            self.binary_grinding(4096),
        )?;
        drop(bridge_span);
        let keccak_span = tracing::info_span!("falcon_hybrid:keccak_prefix").entered();
        let (keccak, k) = self
            .keccak
            .prove_prefix(
                &committed.packed[1],
                committed.auxiliary,
                &committed.commitments[1],
                &mut t,
                (self.target_bits + 8) as u32,
            )
            .map_err(error)?;
        drop(keccak_span);
        let k = k.map(|c| BinaryClaim {
            low: c.low,
            high_point: c.high_point,
            value: c.value,
        });
        let mut link_t = ProverBlockGrindingTranscript::<_, LinkGrinding>::new(
            &mut t,
            self.binary_grinding(128),
        );
        let (coefficients, target) =
            self.coefficients(&mut link_t, &committed.statement.public, &a, &k);
        let links_nonces = link_t.finish();
        let joint_span = tracing::info_span!("falcon_hybrid:joint_sumcheck").entered();
        let (joint, point) = joint::prove(
            &mut t,
            &self.geometry,
            [&committed.packed[0], &committed.packed[1]],
            [&coefficients[0], &coefficients[1]],
            target,
            self.binary_grinding(2 * self.geometry.bit_log()),
            &mut *self
                .scratch
                .lock()
                .map_err(|_| error("hybrid scratch lock"))?,
        )
        .map_err(error)?;
        drop(joint_span);
        let opening_span = tracing::info_span!("falcon_hybrid:shared_opening").entered();
        let packed = self
            .geometry
            .virtual_packed([&committed.packed[0], &committed.packed[1]]);
        let mut pcs_nonces = Vec::new();
        let mut pcs = GrindingContext {
            plan: &self.pcs_grinding,
            nonces: GrindingNonces::Prove(&mut pcs_nonces),
        };
        let mut opening_t = ProverBlockGrindingTranscript::<_, OpeningGrinding>::new(
            &mut t,
            self.binary_grinding(256),
        );
        let opening = shared::prove_with_security(
            &mut opening_t,
            &self.geometry,
            &digest,
            packed,
            None,
            &self.ligerito,
            [&committed.data[0], &committed.data[1]],
            &point,
            Some(&mut pcs),
        )
        .map_err(error)?;
        let opening_nonces = opening_t.finish();
        drop(opening_span);
        Ok(FalconHybridProof {
            arithmetic,
            bridge,
            keccak,
            links_nonces,
            joint,
            opening,
            opening_nonces,
            pcs_nonces,
        })
    }

    pub fn verify(
        &self,
        statement: &FalconHybridStatement,
        proof: &FalconHybridProof,
    ) -> Result<(), FalconError> {
        let (mut t, digest) = self.transcript(statement)?;
        if proof.opening.ood.is_some() {
            return Err(error("unexpected hybrid OOD claim"));
        }
        verify_binding_prefix(
            &mut t,
            &self.layout,
            &statement.public,
            &proof.arithmetic,
            self.target_bits,
        )?;
        let a = hybrid_bridge::verify(
            &mut t,
            &self.layout,
            &proof.arithmetic.binding_point,
            proof.arithmetic.binding_terminal[1],
            proof.arithmetic.piop.modulus,
            &proof.bridge,
            self.binary_grinding(4096),
        )?;
        let commitment = Commitment {
            root: statement.roots[1],
            params: self
                .geometry
                .params(1, self.ligerito.prover().log_inv_rates[0]),
        };
        let k = self
            .keccak
            .verify_prefix(
                &proof.keccak,
                &commitment,
                &mut t,
                (self.target_bits + 8) as u32,
            )
            .map_err(error)?;
        let k = k.map(|c| BinaryClaim {
            low: c.low,
            high_point: c.high_point,
            value: c.value,
        });
        let mut link_t = VerifierBlockGrindingTranscript::<_, LinkGrinding>::new(
            &mut t,
            self.binary_grinding(128),
            &proof.links_nonces,
        );
        let (coefficients, target) = self.coefficients(&mut link_t, &statement.public, &a, &k);
        link_t.finish().map_err(error)?;
        let point = joint::verify(
            &mut t,
            &self.geometry,
            [&coefficients[0], &coefficients[1]],
            target,
            self.binary_grinding(2 * self.geometry.bit_log()),
            &proof.joint,
        )
        .map_err(error)?;
        let mut pcs = GrindingContext {
            plan: &self.pcs_grinding,
            nonces: GrindingNonces::Verify {
                values: &proof.pcs_nonces,
                cursor: 0,
            },
        };
        let mut opening_t = VerifierBlockGrindingTranscript::<_, OpeningGrinding>::new(
            &mut t,
            self.binary_grinding(256),
            &proof.opening_nonces,
        );
        shared::verify_with_security(
            &mut opening_t,
            &self.geometry,
            &digest,
            &statement.roots,
            &point,
            proof.joint.value,
            None,
            &self.ligerito,
            &proof.opening,
            Some(&mut pcs),
        )
        .map_err(error)?;
        opening_t.finish().map_err(error)
    }

    fn coefficients(
        &self,
        t: &mut impl Transcript,
        public: &FalconPublicStatement,
        a: &[BinaryClaim],
        k: &[BinaryClaim],
    ) -> ([Coefficients; 2], Gf) {
        t.absorb_slice(b"bitz/falcon-hybrid/binary-claims-and-shake-wiring/v1");
        for claims in [a, k] {
            t.absorb_slice(&(claims.len() as u64).to_le_bytes());
            for c in claims {
                t.absorb_slice(&c.value.lo.to_le_bytes());
                t.absorb_slice(&c.value.hi.to_le_bytes());
            }
        }
        let mut result = [Coefficients::default(), Coefficients::default()];
        let mut target = Gf::ZERO;
        for (branch, claims) in [a, k].into_iter().enumerate() {
            for c in claims {
                let scale: Gf = t.get_field_challenge(&());
                target += scale * c.value;
                result[branch].tensors.push(Tensor {
                    low: c.low.iter().map(|&w| w * scale).collect(),
                    high_point: c.high_point.clone(),
                });
            }
        }
        let local_point: Vec<Gf> = t.get_field_challenges(16, &());
        let instance_point: Vec<Gf> = t.get_field_challenges(self.capacity().ilog2() as usize, &());
        let eta: Gf = t.get_field_challenge(&());
        let local = eq_table(&local_point);
        let instances = eq_table(&instance_point);
        let offsets = self.layout.offsets();
        let mut arithmetic = Vec::with_capacity(320 + 16 * super::HASH_TO_POINT_SAMPLES);
        let mut binary = Vec::with_capacity(1600 + 19 * 3200 + 16 * super::HASH_TO_POINT_SAMPLES);
        // Remove the middle signature coordinates from BatchMajor addresses;
        // Gather::repeated_at inserts them back, with their equality weights.
        let local_index = |index: usize| {
            (index & ((1 << 12) - 1)) | ((index >> (12 + instance_point.len())) << 12)
        };
        for bit in 0..1600 {
            let w = eta * local[bit];
            binary.push((local_index(self.keccak.initial_bit_index(0, 0, bit)), w));
            if bit < 320 {
                arithmetic.push((offsets.encoded_signature + 8 + bit, w));
            } else {
                let byte = bit / 8;
                for (i, msg) in public.messages.iter().enumerate() {
                    let value = match byte {
                        40..=71 => msg[byte - 40],
                        72 => 0x1f,
                        135 => 0x80,
                        _ => 0,
                    };
                    if value >> (bit % 8) & 1 == 1 {
                        target += instances[i] * w;
                    }
                }
            }
        }
        for permutation in 1..20 {
            for bit in 0..1600 {
                let w = eta * local[permutation * 1600 + bit];
                binary.push((
                    local_index(self.keccak.initial_bit_index(0, permutation, bit)),
                    w,
                ));
                binary.push((
                    local_index(self.keccak.output_bit_index(0, permutation - 1, bit)),
                    w,
                ));
            }
        }
        for sample in 0..super::HASH_TO_POINT_SAMPLES {
            for bit in 0..16 {
                let w = eta * local[32000 + 16 * sample + bit];
                binary.push((local_index(self.keccak.sample_bit_index(0, sample, bit)), w));
                arithmetic.push((offsets.hash_words + 16 * sample + bit, w));
            }
        }
        // A prefix of live instances is a disjoint union of at most log(B)
        // Boolean subcubes. This masks padding without expanding the wiring.
        for (point, scale) in live_subcubes(&instance_point, self.batch()) {
            result[0].gathers.push(Gather::repeated(
                arithmetic.iter().map(|&(i, w)| (i, w * scale)).collect(),
                point.clone(),
            ));
            result[1].gathers.push(Gather::repeated_at(
                binary.iter().map(|&(i, w)| (i, w * scale)).collect(),
                point,
                12,
            ));
        }
        (result, target)
    }
}

fn live_subcubes(point: &[Gf], live: usize) -> Vec<(Vec<Gf>, Gf)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < live {
        let size = 1usize
            << ((live - start).ilog2().min(if start == 0 {
                point.len() as u32
            } else {
                start.trailing_zeros()
            }));
        let free = size.ilog2() as usize;
        let mut pinned = point.to_vec();
        let mut scale = Gf::ONE;
        for j in free..point.len() {
            let bit = start >> j & 1;
            scale *= if bit == 1 {
                point[j]
            } else {
                Gf::ONE + point[j]
            };
            pinned[j] = if bit == 1 { Gf::ONE } else { Gf::ZERO };
        }
        out.push((pinned, scale));
        start += size;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
    const MSG: &[u8] = include_bytes!("fixtures/message.bin");
    const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");

    fn public(batch: usize) -> FalconPublicStatement {
        FalconPublicStatement::from_bytes(&vec![PK; batch], &vec![MSG; batch]).unwrap()
    }

    #[test]
    fn hybrid_live_mask_matches_every_prefix() {
        let point: Vec<_> = (0..5)
            .map(|i| Gf {
                lo: 17 + i * 31,
                hi: i + 7,
            })
            .collect();
        let expected = eq_table(&point);
        for live in 1..=32 {
            let mut actual = vec![Gf::ZERO; 32];
            for (p, scale) in live_subcubes(&point, live) {
                for (a, v) in actual.iter_mut().zip(eq_table(&p)) {
                    *a += scale * v;
                }
            }
            for i in 0..32 {
                assert_eq!(actual[i], if i < live { expected[i] } else { Gf::ZERO });
            }
        }
    }

    #[test]
    fn hybrid_security_covers_supported_batch_sizes() {
        for target in [100, 128] {
            for batch in [1, 3, 32, 256, 1024] {
                let prepared = PreparedFalconHybrid::new(batch, target).unwrap();
                assert!(prepared.security().algebraic_bits >= target as f64);
                assert_eq!(prepared.layout.local_counts().total(), 198_935);
            }
        }
        assert!(PreparedFalconHybrid::new(1025, 128).is_err());
    }

    #[test]
    fn hybrid_falcon_roundtrip_and_tampering() {
        let prepared = PreparedFalconHybrid::new(1, 100).unwrap();
        let committed = prepared.commit(public(1), &[SIG]).unwrap();
        let native = super::super::verification_trace(PK, MSG, SIG).unwrap();
        assert_eq!(committed.traces[0].convolution, native.convolution);
        assert_eq!(
            committed.traces[0].hash_to_point.words,
            native.hash_to_point.words
        );
        assert_eq!(committed.traces[0].norm, native.norm);
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();
        let mut changed = statement.clone();
        changed.public.messages[0][0] ^= 1;
        assert!(prepared.verify(&changed, &proof).is_err());
        let mut changed = statement.clone();
        changed.roots.swap(0, 1);
        assert!(prepared.verify(&changed, &proof).is_err());
        let mut changed = proof.clone();
        changed.bridge.sums[0][0] ^= 1;
        assert!(prepared.verify(&statement, &changed).is_err());
        let mut changed = proof.clone();
        changed.joint.value += Gf::ONE;
        assert!(prepared.verify(&statement, &changed).is_err());
        let mut changed = proof.clone();
        changed.pcs_nonces.push(0);
        assert!(prepared.verify(&statement, &changed).is_err());
    }

    #[test]
    fn hybrid_falcon_padding_and_wrong_nonce_branch() {
        let prepared = PreparedFalconHybrid::new(3, 100).unwrap();
        let committed = prepared.commit(public(3), &[SIG; 3]).unwrap();
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();

        let prepared = PreparedFalconHybrid::new(1, 100).unwrap();
        let mut committed = prepared.commit(public(1), &[SIG]).unwrap();
        let mut nonce = decode_signature_ct(SIG).unwrap().nonce;
        nonce[0] ^= 1;
        // Both branches separately satisfy their PIOPs. Only the authenticated
        // SHAKE/nonce/sample links distinguish this from a valid composition.
        let wrong = prepared
            .keccak
            .generate_witness(&[nonce], &committed.statement.public.messages)
            .unwrap();
        committed.packed[1] = wrong.packed;
        committed.auxiliary = wrong.auxiliary;
        let (cm, data) = commit(
            &committed.packed[1],
            &prepared
                .geometry
                .params(1, prepared.ligerito.prover().log_inv_rates[0]),
        );
        committed.statement.roots[1] = cm.root;
        committed.commitments[1] = cm;
        committed.data[1] = data;
        let statement = committed.statement.clone();
        if let Ok(proof) = prepared.prove(committed) {
            assert!(prepared.verify(&statement, &proof).is_err());
        }
    }

    #[test]
    fn hybrid_falcon_128_roundtrip() {
        let prepared = PreparedFalconHybrid::new(1, 128).unwrap();
        let committed = prepared.commit(public(1), &[SIG]).unwrap();
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();
        let mut changed = proof.clone();
        changed.links_nonces.clear();
        assert!(prepared.verify(&statement, &changed).is_err());
    }
}
