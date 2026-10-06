//! Non-ZK Falcon composition with compact binary Keccak and one shared PCS opening.
use super::{
    FalconError, FalconPublicStatement, FalconSourceLayout, FalconSourceWitness,
    FalconVerificationTrace, KeccakTrace, encode_signature_ct, hybrid_bridge,
    hybrid_keccak::{
        self, KeccakAuxiliary, PreparedKeccak,
        grinding::{ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript},
    },
    hybrid_sumcheck::{self as joint, Coefficients, Gather, Tensor},
    opening::{FalconBindingPrefixProof, prove_binding_prefix, verify_binding_prefix},
};
use crate::{
    hybrid::{
        BinaryClaim,
        opening::{
            self as shared,
            grinding::{GrindingContext, GrindingNonces},
        },
        sumcheck::{Scratch, eq_table},
    },
    ligerito_flock::{LigeritoSelection, ResolvedLigerito, grinding_plan::GrindingPlan},
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

#[path = "hybrid_size.rs"]
mod size;

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

/// Public keys, messages, signatures, and the three authenticated binary sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconHybridStatement {
    pub public: FalconPublicStatement,
    pub roots: [[u8; 32]; 3],
}

/// Union-bound terms in the existing computational grinding model. This is
/// algebraic/IOP accounting, separate from BLAKE3's 128-bit collision bound.
#[derive(Clone, Debug)]
pub struct FalconHybridSecurity {
    pub target_bits: usize,
    pub algebraic_bits: f64,
    pub terms: Vec<(&'static str, f64)>,
}

/// Explicit protocol selection; the established native-carry prover remains default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FalconProtocol {
    NativeCarry,
    SharedPrimeV2,
}

/// Prepared public circuit, reusable across witnesses of the same batch size.
pub struct PreparedFalconHybrid {
    layout: FalconSourceLayout,
    keccak: [PreparedKeccak; 2],
    geometry: shared::Geometry<3>,
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
    auxiliary: [KeccakAuxiliary; 2],
    packed: [Vec<Gf>; 3],
    commitments: [Commitment; 3],
    data: [ProverData; 3],
}

#[derive(Clone, Debug)]
pub struct FalconHybridProof {
    arithmetic: FalconBindingPrefixProof,
    bridge: hybrid_bridge::Proof,
    keccak: [hybrid_keccak::PrefixProof; 2],
    links_nonces: Vec<u64>,
    joint: joint::Proof,
    opening: shared::Proof<3>,
    opening_nonces: Vec<u64>,
    pcs_nonces: Vec<u64>,
}

impl PreparedFalconHybrid {
    /// Supports 1..=1024 live signatures, with canonical power-of-two padding.
    pub fn new(batch: usize, target_bits: usize) -> Result<Self, FalconError> {
        Self::with_protocol(batch, target_bits, FalconProtocol::NativeCarry)
    }

    /// Experimental four-operand ring reduction and integer-polynomial lift.
    pub fn new_shared_prime(batch: usize, target_bits: usize) -> Result<Self, FalconError> {
        Self::with_protocol(batch, target_bits, FalconProtocol::SharedPrimeV2)
    }

    pub fn with_protocol(
        batch: usize,
        target_bits: usize,
        protocol: FalconProtocol,
    ) -> Result<Self, FalconError> {
        if !matches!(target_bits, 100 | 128) {
            return Err(error("hybrid security must be 100 or 128 bits"));
        }
        let layout = match protocol {
            FalconProtocol::NativeCarry => FalconSourceLayout::new(batch)?,
            FalconProtocol::SharedPrimeV2 => FalconSourceLayout::new_shared_prime(batch)?,
        };
        let keccak = [
            PreparedKeccak::new_slab(batch, 0, 16).map_err(error)?,
            PreparedKeccak::new_slab(batch, 16, 4).map_err(error)?,
        ];
        let geometry = shared::Geometry::new([
            layout.source_bits().ilog2() as usize - 7,
            keccak[0].packed_vars(),
            keccak[1].packed_vars(),
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
    pub fn protocol(&self) -> FalconProtocol {
        if self.layout.is_shared_prime() {
            FalconProtocol::SharedPrimeV2
        } else {
            FalconProtocol::NativeCarry
        }
    }
    pub fn live_arithmetic_bits_per_signature(&self) -> usize {
        self.layout.live_bits()
    }
    pub fn source_bits_per_signature(&self) -> [usize; 3] {
        [
            self.layout.signature_stride(),
            (1 << self.keccak[0].bit_vars()) / self.capacity(),
            (1 << self.keccak[1].bit_vars()) / self.capacity(),
        ]
    }

    pub fn security(&self) -> FalconHybridSecurity {
        let b = self.batch();
        let d = self.capacity().ilog2() as usize;
        let schedule = super::FalconSecuritySchedule::for_layout(self.target_bits, &self.layout)
            .expect("prepared target");
        let prime_bits = if self.layout.is_shared_prime() {
            114
        } else {
            125
        };
        let prime = |n: usize, g: u32| n as f64 * 2f64.powi(-prime_bits - g as i32);
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
                prime(11 + d, schedule.outer_point_bits),
            ),
            (
                "HashToPoint product sumcheck",
                prime(3 * (11 + d), schedule.cubic_round_bits),
            ),
            (
                "ordered compaction fingerprints",
                prime(2048, schedule.fingerprint_bits),
            ),
            (
                "ordered compaction forest sumchecks",
                prime(165, schedule.cubic_round_bits),
            ),
            (
                "ordered compaction forest claim reductions",
                prime(10 * (d + 1) + 11, schedule.forest_claim_bits),
            ),
            (
                "compaction leaf instance batching",
                prime(d, schedule.norm_instance_bits),
            ),
            (
                "compaction leaf sumcheck",
                prime(3 * (11 + d), schedule.cubic_round_bits),
            ),
            (
                if self.layout.is_shared_prime() {
                    "shared ring outer and endpoint batch"
                } else {
                    "native ideal batching and projection"
                },
                if self.layout.is_shared_prime() {
                    (4 * d + 2049) as f64 / (super::Q as f64).powi(11)
                } else {
                    (d + 2046) as f64 / ((super::Q as f64).powi(11) - super::Q as f64)
                },
            ),
            (
                if self.layout.is_shared_prime() {
                    "integer polynomial projection"
                } else {
                    "native coordinate carry batching"
                },
                if self.layout.is_shared_prime() {
                    prime(
                        20,
                        super::shared_ring::projection_grinding_bits(self.target_bits),
                    )
                } else {
                    prime(10, if self.target_bits == 128 { 12 } else { 0 })
                },
            ),
            (
                "linear constraints and terminal batching",
                prime(13 + d + b + 12, schedule.linear_point_bits),
            ),
            (
                "prime source sumcheck",
                prime(2 * (17 + d), schedule.binding_round_bits),
            ),
            (
                "batched integer-to-binary forest",
                binary(hybrid_bridge::error_numerator(&self.layout)),
            ),
            (
                "binary Keccak PIOP",
                self.keccak
                    .iter()
                    .map(|slab| {
                        slab.security((self.target_bits + 8) as u32)
                            .expect("prepared profile")
                            .error_bound()
                    })
                    .sum(),
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
    ) -> Result<CommittedFalconHybrid, FalconError> {
        let _span = tracing::info_span!("falcon_hybrid:witness_commit").entered();
        public.validate(self.batch())?;
        let encoded = public
            .signatures
            .iter()
            .map(encode_signature_ct)
            .collect::<Result<Vec<_>, _>>()?;
        let signatures: Vec<&[u8]> = encoded.iter().map(|s| s.as_slice()).collect();
        let nonces: Vec<_> = public.signatures.iter().map(|s| s.nonce).collect();
        let (keccak_packed, auxiliary, samples) = self.generate_shake(&nonces, &public.messages)?;
        let traces = crate::utils::cfg_into_iter!(0..self.batch())
            .map(|i| {
                let bytes: Vec<_> = samples[i].iter().flat_map(|w| w.to_be_bytes()).collect();
                let htp = super::hash_to_point::from_shake_bytes(&bytes, KeccakTrace::default())?;
                super::verify::trace_from_parts(
                    public.public_keys[i].clone(),
                    public.signatures[i].clone(),
                    htp,
                    true,
                )
            })
            .collect::<Result<Vec<_>, FalconError>>()?;
        let messages: Vec<&[u8]> = public.messages.iter().map(|m| m.as_slice()).collect();
        let arithmetic =
            FalconSourceWitness::from_traces(self.layout, &messages, &signatures, &traces)?;
        let mut arithmetic_packed = Vec::with_capacity(1 << self.geometry.physical_logs[0]);
        for row in arithmetic.rows() {
            arithmetic_packed.extend(row.chunks_exact(2).map(|w| Gf { lo: w[0], hi: w[1] }));
        }
        arithmetic_packed.resize(1 << self.geometry.physical_logs[0], Gf::ZERO);
        let [k16, k4] = keccak_packed;
        let packed = [arithmetic_packed, k16, k4];
        let rate = self.ligerito.prover().log_inv_rates[0];
        let (ca, da) = commit(&packed[0], &self.geometry.params(0, rate));
        let (ck, dk) = commit(&packed[1], &self.geometry.params(1, rate));
        let (ct, dt) = commit(&packed[2], &self.geometry.params(2, rate));
        let statement = FalconHybridStatement {
            public,
            roots: [ca.root, ck.root, ct.root],
        };
        Ok(CommittedFalconHybrid {
            statement,
            traces,
            arithmetic,
            auxiliary,
            packed,
            commitments: [ca, ck, ct],
            data: [da, dk, dt],
        })
    }

    fn generate_shake(
        &self,
        nonces: &[[u8; 40]],
        messages: &[[u8; 32]],
    ) -> Result<
        (
            [Vec<Gf>; 2],
            [KeccakAuxiliary; 2],
            Vec<[u16; hybrid_keccak::SAMPLES]>,
        ),
        FalconError,
    > {
        let initial: Vec<_> = nonces
            .iter()
            .zip(messages)
            .map(|(nonce, message)| hybrid_keccak::initial_state(nonce, message))
            .collect();
        let (k16, a16, out16) = self.keccak[0].generate_chain(&initial).map_err(error)?;
        let next: Vec<_> = out16[..self.batch()].iter().map(|out| out[15]).collect();
        let (k4, a4, out4) = self.keccak[1].generate_chain(&next).map_err(error)?;
        let samples = crate::utils::cfg_into_iter!(0..self.batch())
            .map(|signature| {
                std::array::from_fn(|sample| {
                    let byte = 2 * sample;
                    let permutation = byte / hybrid_keccak::RATE_BYTES;
                    let offset = byte % hybrid_keccak::RATE_BYTES;
                    let out = if permutation < 16 {
                        &out16[signature][permutation]
                    } else {
                        &out4[signature][permutation - 16]
                    };
                    let byte_at = |i: usize| (out[i / 8] >> (8 * (i % 8))) as u8;
                    u16::from_be_bytes([byte_at(offset), byte_at(offset + 1)])
                })
            })
            .collect();
        Ok(([k16, k4], [a16, a4], samples))
    }

    fn transcript(
        &self,
        statement: &FalconHybridStatement,
    ) -> Result<(Blake3Transcript, [u8; 32]), FalconError> {
        statement.public.validate(self.batch())?;
        let mut h = blake3::Hasher::new();
        h.update(if self.layout.is_shared_prime() {
            b"bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2".as_slice()
        } else {
            b"bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4".as_slice()
        });
        for n in [
            self.batch(),
            self.capacity(),
            self.target_bits,
            self.geometry.logs[0],
            self.geometry.logs[1],
            self.geometry.logs[2],
        ] {
            h.update(&(n as u64).to_le_bytes());
        }
        h.update(b"bounded14:low13+4097*top;native:Q12289,theta11+theta+14;Dlen1023;carry22528BN");
        if self.layout.is_shared_prime() {
            h.update(b"shared:all-E;C,H,S1,S2:1,l,l2,l3;direct-beta-decoder;Hunsigned14;S2encoded-alias;live-mask;P21-i128;prime115-capped;unsplit8192;merge-in-binder-block/v2");
            h.update(&(self.layout.live_bits() as u64).to_le_bytes());
            h.update(
                &(self.layout.public_key_offset().expect("shared H slots") as u64).to_le_bytes(),
            );
            h.update(&super::shared_ring::projection_grinding_bits(self.target_bits).to_le_bytes());
        }
        h.update(b"compaction:fixed-bad-signature;forest:eq-batching,nonzero-vector-line/v3");
        let schedule = super::FalconSecuritySchedule::for_layout(self.target_bits, &self.layout)
            .expect("prepared security");
        for bits in [
            schedule.norm_instance_bits,
            schedule.outer_point_bits,
            schedule.quadratic_round_bits,
            schedule.cubic_round_bits,
            schedule.forest_claim_bits,
            schedule.fingerprint_bits,
            schedule.linear_point_bits,
            schedule.binding_round_bits,
            if self.target_bits == 128 { 12 } else { 0 },
        ] {
            h.update(&bits.to_le_bytes());
        }
        // The bridge schedule is public and deterministic, but bind it explicitly
        // so proofs cannot be replayed under a different bridge error budget.
        let bridge_numerator = hybrid_bridge::error_numerator(&self.layout);
        h.update(&(bridge_numerator as u64).to_le_bytes());
        h.update(&self.binary_grinding(bridge_numerator).to_le_bytes());
        for root in &statement.roots {
            h.update(root);
        }
        for ((pk, msg), signature) in statement
            .public
            .public_keys
            .iter()
            .zip(&statement.public.messages)
            .zip(&statement.public.signatures)
        {
            for &coefficient in pk.h.iter() {
                h.update(&coefficient.to_le_bytes());
            }
            h.update(msg);
            h.update(&encode_signature_ct(signature)?);
        }
        h.update(&self.ligerito.digest());
        for block in &self.pcs_grinding.blocks {
            h.update(&block.bits.to_le_bytes());
        }
        let digest = *h.finalize().as_bytes();
        let mut t = Blake3Transcript::new();
        t.absorb_slice(if self.layout.is_shared_prime() {
            b"bitz/falcon-hybrid/shared-prime/statement/v2".as_slice()
        } else {
            b"bitz/falcon-hybrid/native-ring/statement/v4".as_slice()
        });
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
        let (arithmetic, row_weights, bridge_claim) = prove_binding_prefix(
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
            &bridge_claim.point,
            bridge_claim.modulus,
            self.binary_grinding(hybrid_bridge::error_numerator(&self.layout)),
            &row_weights,
        )?;
        drop(row_weights);
        drop(bridge_span);
        let keccak_span = tracing::info_span!("falcon_hybrid:keccak_prefix").entered();
        let mut prefixes = Vec::new();
        let mut k = Vec::new();
        let mut marginals = Vec::new();
        for (slab, auxiliary) in committed.auxiliary.into_iter().enumerate() {
            let (proof, claims, cached) = self.keccak[slab]
                .prove_prefix(
                    &committed.packed[slab + 1],
                    auxiliary,
                    &committed.commitments[slab + 1],
                    &mut t,
                    (self.target_bits + 8) as u32,
                )
                .map_err(error)?;
            prefixes.push(proof);
            k.push(claims.map(|c| BinaryClaim {
                low: c.low,
                high_point: c.high_point,
                value: c.value,
            }));
            marginals.push(cached);
        }
        let keccak = prefixes.try_into().expect("two Keccak slabs");
        let k: [[BinaryClaim; 2]; 2] = k
            .try_into()
            .unwrap_or_else(|_| unreachable!("two Keccak slabs"));
        drop(keccak_span);
        let links_span = tracing::info_span!("falcon_hybrid:link_coefficients").entered();
        let mut link_t = ProverBlockGrindingTranscript::<_, LinkGrinding>::new(
            &mut t,
            self.binary_grinding(128),
        );
        let (mut coefficients, target) =
            self.coefficients(&mut link_t, &committed.statement.public, &a, &k);
        let links_nonces = link_t.finish();
        drop(links_span);
        for (branch, cached) in marginals.into_iter().enumerate() {
            for (tensor, values) in coefficients[branch + 1].tensors.iter_mut().zip(cached) {
                tensor.marginals = Some(values);
            }
        }
        let joint_span = tracing::info_span!("falcon_hybrid:joint_sumcheck").entered();
        let (joint, point) = joint::prove(
            &mut t,
            &self.geometry,
            committed.packed.each_ref().map(Vec::as_slice),
            coefficients.each_ref(),
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
        let mut pcs_nonces = Vec::new();
        let mut pcs = GrindingContext {
            plan: &self.pcs_grinding,
            nonces: GrindingNonces::Prove(&mut pcs_nonces),
        };
        let mut opening_t = ProverBlockGrindingTranscript::<_, OpeningGrinding>::new(
            &mut t,
            self.binary_grinding(256),
        );
        let opening = shared::prove_sources_with_security(
            &mut opening_t,
            &self.geometry,
            &digest,
            committed.packed.each_ref().map(Vec::as_slice),
            None,
            &self.ligerito,
            committed.data.each_ref(),
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
        let bridge_claim = verify_binding_prefix(
            &mut t,
            &self.layout,
            &statement.public,
            &proof.arithmetic,
            self.target_bits,
        )?;
        let a = hybrid_bridge::verify(
            &mut t,
            &self.layout,
            &bridge_claim.point,
            proof.arithmetic.binding_terminal[1],
            bridge_claim.modulus,
            &proof.bridge,
            self.binary_grinding(hybrid_bridge::error_numerator(&self.layout)),
        )?;
        let mut k = Vec::new();
        for slab in 0..2 {
            let commitment = Commitment {
                root: statement.roots[slab + 1],
                params: self
                    .geometry
                    .params(slab + 1, self.ligerito.prover().log_inv_rates[0]),
            };
            let claims = self.keccak[slab]
                .verify_prefix(
                    &proof.keccak[slab],
                    &commitment,
                    &mut t,
                    (self.target_bits + 8) as u32,
                )
                .map_err(error)?;
            k.push(claims.map(|c| BinaryClaim {
                low: c.low,
                high_point: c.high_point,
                value: c.value,
            }));
        }
        let k: [[BinaryClaim; 2]; 2] = k
            .try_into()
            .unwrap_or_else(|_| unreachable!("two Keccak slabs"));
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
            coefficients.each_ref(),
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
        k: &[[BinaryClaim; 2]; 2],
    ) -> ([Coefficients; 3], Gf) {
        t.absorb_slice(b"bitz/falcon-hybrid/binary-claims-and-shake-wiring/v3");
        for claims in [a, k[0].as_slice(), k[1].as_slice()] {
            t.absorb_slice(&(claims.len() as u64).to_le_bytes());
            for c in claims {
                t.absorb_slice(&c.value.lo.to_le_bytes());
                t.absorb_slice(&c.value.hi.to_le_bytes());
            }
        }
        let mut result = std::array::from_fn(|_| Coefficients::default());
        let mut target = Gf::ZERO;
        for (branch, claims) in [a, k[0].as_slice(), k[1].as_slice()]
            .into_iter()
            .enumerate()
        {
            for c in claims {
                let scale: Gf = t.get_field_challenge(&());
                target += scale * c.value;
                result[branch].tensors.push(Tensor {
                    low: c.low.iter().map(|&w| w * scale).collect(),
                    high_point: c.high_point.clone(),
                    marginals: None,
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
        let mut binary: [Vec<(usize, Gf)>; 2] = std::array::from_fn(|_| Vec::new());
        // Slab addresses are [7 in-word | signature | permutation | chunk].
        // Remove the signature axis for the factored repeated gather.
        let local_index = |slab: usize, index: usize| {
            (index & 127) | ((index >> (7 + self.keccak[slab].capacity().ilog2())) << 7)
        };
        for bit in 0..1600 {
            let w = eta * local[bit];
            binary[0].push((
                local_index(0, self.keccak[0].initial_bit_index(0, 0, bit)),
                w,
            ));
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
        for permutation in 1..hybrid_keccak::PERMUTATIONS {
            for bit in 0..1600 {
                let w = eta * local[permutation * 1600 + bit];
                let slab = usize::from(permutation >= 16);
                let previous = usize::from(permutation > 16);
                binary[slab].push((
                    local_index(
                        slab,
                        self.keccak[slab].initial_bit_index(0, permutation, bit),
                    ),
                    w,
                ));
                binary[previous].push((
                    local_index(
                        previous,
                        self.keccak[previous].output_bit_index(0, permutation - 1, bit),
                    ),
                    w,
                ));
            }
        }
        for sample in 0..super::HASH_TO_POINT_SAMPLES {
            for bit in 0..16 {
                let w = eta * local[32000 + 16 * sample + bit];
                let slab = usize::from((2 * sample) / hybrid_keccak::RATE_BYTES >= 16);
                binary[slab].push((
                    local_index(slab, self.keccak[slab].sample_bit_index(0, sample, bit)),
                    w,
                ));
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
            for slab in 0..2 {
                let mut repeat = point.clone();
                // At batch one the four-permutation slab has one padded
                // signature coordinate to meet Flock's eight-block minimum.
                repeat.resize(self.keccak[slab].capacity().ilog2() as usize, Gf::ZERO);
                result[slab + 1].gathers.push(Gather::repeated_at(
                    binary[slab].iter().map(|&(i, w)| (i, w * scale)).collect(),
                    repeat,
                    7,
                ));
            }
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
    use super::super::decode_signature_ct;
    use super::*;
    const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
    const MSG: &[u8] = include_bytes!("fixtures/message.bin");
    const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");

    fn public(batch: usize) -> FalconPublicStatement {
        FalconPublicStatement::from_bytes(&vec![PK; batch], &vec![MSG; batch], &vec![SIG; batch])
            .unwrap()
    }

    #[test]
    fn hybrid_shake_matches_rustcrypto_samples_and_source_bits() {
        use rand::{RngExt, SeedableRng, rngs::StdRng};
        use sha3::{
            Shake256,
            digest::{ExtendableOutput, Update, XofReader},
        };

        let mut rng = StdRng::seed_from_u64(0x4859_4252_4944);
        for batch in [1, 3, 8] {
            let prepared = PreparedFalconHybrid::new(batch, 100).unwrap();
            let nonces: Vec<[u8; 40]> = (0..batch)
                .map(|_| std::array::from_fn(|_| rng.random()))
                .collect();
            let messages: Vec<[u8; 32]> = (0..batch)
                .map(|_| std::array::from_fn(|_| rng.random()))
                .collect();
            let (packed, _, samples) = prepared.generate_shake(&nonces, &messages).unwrap();
            assert_eq!(samples.len(), batch);
            for signature in 0..batch {
                let mut oracle = Shake256::default();
                oracle.update(&nonces[signature]);
                oracle.update(&messages[signature]);
                let mut expected = [0; 2622];
                oracle.finalize_xof().read(&mut expected);
                let actual: Vec<_> = samples[signature]
                    .iter()
                    .flat_map(|word| word.to_be_bytes())
                    .collect();
                assert_eq!(actual, expected, "batch={batch}, signature={signature}");

                // Check the bits committed by both Keccak groups, including
                // the transition after the sixteenth 136-byte output block.
                for (byte_index, byte) in expected.into_iter().enumerate() {
                    let permutation = byte_index / 136;
                    let group = usize::from(permutation >= 16);
                    for bit in 0..8 {
                        let index = prepared.keccak[group].output_bit_index(
                            signature,
                            permutation,
                            8 * (byte_index % 136) + bit,
                        );
                        let word = packed[group][index / 128];
                        let half = if index % 128 < 64 { word.lo } else { word.hi };
                        assert_eq!(
                            (half >> (index % 64)) & 1,
                            u64::from((byte >> bit) & 1),
                            "batch={batch}, signature={signature}, byte={byte_index}, bit={bit}"
                        );
                    }
                }
            }
        }
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
            for batch in 1..=1024 {
                let prepared = PreparedFalconHybrid::new(batch, target).unwrap();
                let security = prepared.security();
                assert!(security.algebraic_bits >= target as f64);
                let numerator = hybrid_bridge::error_numerator(&prepared.layout);
                let bits = prepared.binary_grinding(numerator);
                assert_eq!(
                    bits,
                    if target == 100 {
                        0
                    } else if numerator <= 512 {
                        17
                    } else {
                        18
                    }
                );
                let bridge_error = security
                    .terms
                    .iter()
                    .find(|(name, _)| *name == "batched integer-to-binary forest")
                    .unwrap()
                    .1;
                assert_eq!(
                    bridge_error,
                    (numerator as f64) * 2f64.powi(-128 - bits as i32)
                );
                assert!(bridge_error <= 2f64.powi(-(target as i32) - 8));
                assert_eq!(FalconSourceLayout::counts().total(), 100_578);
            }
        }
        assert!(PreparedFalconHybrid::new(1025, 128).is_err());
    }

    #[test]
    fn hybrid_falcon_roundtrip_and_tampering() {
        let prepared = PreparedFalconHybrid::new(1, 100).unwrap();
        let committed = prepared.commit(public(1)).unwrap();
        let native = super::super::verification_trace(PK, MSG, SIG).unwrap();
        assert_eq!(committed.traces[0].s1, native.s1);
        assert_eq!(committed.traces[0].native_quotient, native.native_quotient);
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
        changed.public.signatures[0].nonce[0] ^= 1;
        assert!(prepared.verify(&changed, &proof).is_err());
        let mut changed = statement.clone();
        changed.public.signatures[0].s2[0] = if changed.public.signatures[0].s2[0] == 0 {
            1
        } else {
            0
        };
        assert_ne!(changed.public.signatures, statement.public.signatures);
        assert!(prepared.verify(&changed, &proof).is_err());
        let mut changed = statement.clone();
        changed.public.signatures.clear();
        assert!(prepared.verify(&changed, &proof).is_err());
        let mut changed = statement.clone();
        changed.public.signatures[0].s2[0] = -2048;
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
    fn shared_prime_hybrid_complete_proofs_and_protocol_binding() {
        for (batch, target) in [(1, 100), (3, 100), (9, 100), (1, 128)] {
            let prepared = PreparedFalconHybrid::new_shared_prime(batch, target).unwrap();
            let native = PreparedFalconHybrid::new(batch, target).unwrap();
            assert_eq!(prepared.protocol(), FalconProtocol::SharedPrimeV2);
            assert_eq!(prepared.live_arithmetic_bits_per_signature(), 114_914);
            assert_eq!(
                prepared.source_bits_per_signature(),
                native.source_bits_per_signature()
            );
            let committed = prepared.commit(public(batch)).unwrap();
            let statement = committed.statement.clone();
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&statement, &proof).unwrap();
            assert!(native.verify(&statement, &proof).is_err());
            assert!(proof.payload_size_bytes() > 0);
            assert!(proof.arithmetic.binding_point.is_empty());
            let mut wrong = proof.clone();
            wrong
                .arithmetic
                .binding_point
                .push(proof.arithmetic.binding_terminal[0]);
            assert!(prepared.verify(&statement, &wrong).is_err());
            let mut wrong = statement.clone();
            wrong.public.public_keys[0].h[0] =
                (wrong.public.public_keys[0].h[0] + 1) % super::super::Q as u16;
            assert!(prepared.verify(&wrong, &proof).is_err());
            for branch in 0..3 {
                let mut wrong = statement.clone();
                wrong.roots[branch][0] ^= 1;
                assert!(prepared.verify(&wrong, &proof).is_err());
            }
            let mut wrong = proof.clone();
            assert_eq!(wrong.bridge.sums.len(), 1);
            wrong.bridge.sums[0][0] ^= 1;
            assert!(prepared.verify(&statement, &wrong).is_err());
            let mut wrong = proof.clone();
            let super::super::opening::RingProof::Shared(ring) = &mut wrong.arithmetic.ring else {
                panic!("shared ring expected");
            };
            ring.lift[0] += 1;
            assert!(prepared.verify(&statement, &wrong).is_err());
        }
    }

    #[test]
    fn hybrid_falcon_padding_and_wrong_nonce_branch() {
        let prepared = PreparedFalconHybrid::new(3, 100).unwrap();
        let committed = prepared.commit(public(3)).unwrap();
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();

        let prepared = PreparedFalconHybrid::new(1, 100).unwrap();
        let mut committed = prepared.commit(public(1)).unwrap();
        let mut nonce = decode_signature_ct(SIG).unwrap().nonce;
        nonce[0] ^= 1;
        // Both branches separately satisfy their PIOPs. Only the authenticated
        // SHAKE/nonce/sample links distinguish this from a valid composition.
        let (wrong, auxiliary, _) = prepared
            .generate_shake(&[nonce], &committed.statement.public.messages)
            .unwrap();
        committed.auxiliary = auxiliary;
        for (slab, packed) in wrong.into_iter().enumerate() {
            let branch = slab + 1;
            committed.packed[branch] = packed;
            let (cm, data) = commit(
                &committed.packed[branch],
                &prepared
                    .geometry
                    .params(branch, prepared.ligerito.prover().log_inv_rates[0]),
            );
            committed.statement.roots[branch] = cm.root;
            committed.commitments[branch] = cm;
            committed.data[branch] = data;
        }
        let statement = committed.statement.clone();
        if let Ok(proof) = prepared.prove(committed) {
            assert!(prepared.verify(&statement, &proof).is_err());
        }
    }

    #[test]
    fn hybrid_falcon_128_roundtrip() {
        let prepared = PreparedFalconHybrid::new(1, 128).unwrap();
        let committed = prepared.commit(public(1)).unwrap();
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();
        let mut changed = proof.clone();
        changed.links_nonces.clear();
        assert!(prepared.verify(&statement, &changed).is_err());
    }

    #[test]
    fn hybrid_fused_batch_roundtrip_and_all_three_roots_are_bound() {
        let prepared = PreparedFalconHybrid::new(9, 100).unwrap();
        let committed = prepared.commit(public(9)).unwrap();
        assert_eq!(
            prepared.source_bits_per_signature(),
            [1 << 17, 1 << 20, 1 << 18]
        );
        let native = super::super::verification_trace(PK, MSG, SIG).unwrap();
        for trace in &committed.traces {
            assert_eq!(trace.hash_to_point.words, native.hash_to_point.words);
        }
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();
        for branch in 0..3 {
            let mut wrong = statement.clone();
            wrong.roots[branch][0] ^= 1;
            assert!(prepared.verify(&wrong, &proof).is_err());
            let mut wrong = proof.clone();
            wrong.opening.paths[branch][0][0] ^= 1;
            assert!(prepared.verify(&statement, &wrong).is_err());
        }
        let mut wrong = proof.clone();
        wrong.keccak.swap(0, 1);
        assert!(prepared.verify(&statement, &wrong).is_err());
    }

    #[test]
    fn valid_keccak_tail_cannot_break_the_cross_slab_chain_link() {
        let prepared = PreparedFalconHybrid::new(1, 100).unwrap();
        let mut committed = prepared.commit(public(1)).unwrap();
        // This is a valid four-permutation Keccak chain, but its input is not
        // permutation 15's output from the other authenticated commitment.
        let (packed, auxiliary, _) = prepared.keccak[1].generate_chain(&[[0; 25]]).unwrap();
        let (cm, data) = commit(
            &packed,
            &prepared
                .geometry
                .params(2, prepared.ligerito.prover().log_inv_rates[0]),
        );
        let (prefix, claims, _) = prepared.keccak[1]
            .prove_prefix(&packed, auxiliary, &cm, &mut Blake3Transcript::new(), 108)
            .unwrap();
        assert_eq!(
            prepared.keccak[1]
                .verify_prefix(&prefix, &cm, &mut Blake3Transcript::new(), 108)
                .unwrap(),
            claims
        );
        let (_, auxiliary, _) = prepared.keccak[1].generate_chain(&[[0; 25]]).unwrap();
        committed.packed[2] = packed;
        committed.auxiliary[1] = auxiliary;
        committed.commitments[2] = cm;
        committed.data[2] = data;
        committed.statement.roots[2] = committed.commitments[2].root;
        let statement = committed.statement.clone();
        if let Ok(proof) = prepared.prove(committed) {
            assert!(prepared.verify(&statement, &proof).is_err());
        }
    }
}
