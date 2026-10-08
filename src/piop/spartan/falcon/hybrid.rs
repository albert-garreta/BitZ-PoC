use super::SIGNATURE_BITS;
use super::ring_field::EXTENSION_DEGREE;
use super::{KECCAK_SLABS, N, PARAMETERS, SOURCE_COUNT};
// Non-ZK Falcon composition with compact binary Keccak and one shared PCS opening.
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
use flock_core::field::Gf128 as Gf;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::sync::Mutex;

#[path = "hybrid_size.rs"]
mod size;
#[path = "hybrid_grinding.rs"]
mod grinding;

fn error(e: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(e.to_string())
}
struct LinkGrinding;
// The existing binary claims/SHAKE wiring budget is 128. The additional
// padding polynomial has degree at most 17 + 10 + 1 (local, instance, merge),
// so 256 conservatively covers their union for every supported profile.
const LINK_ERROR_NUMERATOR: usize = 256;
impl GrindingDomain for LinkGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/link-grinding/v1";
}
struct OpeningGrinding;
impl GrindingDomain for OpeningGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/opening-grinding/v1";
}

/// Public keys, messages, signatures, and the joint binary source commitment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalconHybridStatement {
    pub public: FalconPublicStatement,
    pub source_root: [u8; 32],
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
    keccak: [PreparedKeccak; KECCAK_SLABS],
    geometry: shared::Geometry<SOURCE_COUNT>,
    target_bits: usize,
    max_batch: usize,
    bridge_mode: hybrid_bridge::BridgeMode,
    ligerito: ResolvedLigerito,
    pcs_grinding: GrindingPlan,
    scratch: Mutex<Scratch>,
}

/// Packed witness and its joint commitment. Consumed by proving so large auxiliary
/// tables can be folded in place without cloning them.
pub struct CommittedFalconHybrid {
    pub statement: FalconHybridStatement,
    traces: Vec<FalconVerificationTrace>,
    arithmetic: FalconSourceWitness,
    auxiliary: [KeccakAuxiliary; KECCAK_SLABS],
    packed: [Vec<Gf>; SOURCE_COUNT],
    data: shared::JointProverData<SOURCE_COUNT>,
}

#[derive(Clone, Debug)]
pub struct FalconHybridProof {
    arithmetic: FalconBindingPrefixProof,
    bridge: hybrid_bridge::Proof,
    keccak: [hybrid_keccak::PrefixProof; KECCAK_SLABS],
    links_nonces: Vec<u64>,
    joint: joint::Proof,
    opening: shared::JointProof,
    opening_nonces: Vec<u64>,
    pcs_nonces: Vec<u64>,
}

impl PreparedFalconHybrid {
    pub(crate) fn prepare(
        batch: usize,
        target_bits: usize,
        max_batch: usize,
    ) -> Result<Self, FalconError> {
        if max_batch == 0 || max_batch > 1024 || batch > max_batch {
            return Err(FalconError::InvalidBatchCapacity);
        }
        if !matches!(target_bits, 100 | 128) {
            return Err(error("hybrid security must be 100 or 128 bits"));
        }
        let layout = FalconSourceLayout::new(batch)?;
        let bridge_mode = if target_bits == 100 {
            hybrid_bridge::BridgeMode::Unsplit
        } else {
            hybrid_bridge::BridgeMode::TwoLimbs
        };
        let keccak: [PreparedKeccak; KECCAK_SLABS] = (0..KECCAK_SLABS)
            .map(|i| {
                let (first, count) = PARAMETERS.slab(i);
                PreparedKeccak::new_slab(batch, first, count).map_err(error)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .ok()
            .expect("profile slab count");
        let geometry = shared::Geometry::new(std::array::from_fn(|i| {
            if i == 0 {
                layout.source_bits().ilog2() as usize - 7
            } else {
                keccak[i - 1].packed_vars()
            }
        }))
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
        // At target 100 the search work is already tiny; additional positive
        // boundaries can cost more in dispatch than they save in hashes.
        let pcs_grinding = if target_bits == 128 {
            grinding::rebalance(pcs_grinding)
        } else {
            pcs_grinding
        };
        let prepared = Self {
            layout,
            keccak,
            geometry,
            target_bits,
            max_batch,
            bridge_mode,
            ligerito,
            pcs_grinding,
            scratch: Mutex::default(),
        };
        hybrid_bridge::validate_layout(
            &prepared.layout.bitz_params(),
            bridge_mode,
            prepared.prime_modulus_bounds().1,
        )?;
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

    /// The bridge is selected from validated public parameters, never proof shape.
    pub fn integer_bridge_name(&self) -> &'static str {
        match self.bridge_mode {
            hybrid_bridge::BridgeMode::Unsplit => "wfbitz-unsplit",
            hybrid_bridge::BridgeMode::TwoLimbs => "wfbitz-joint-limbs",
        }
    }

    /// Inclusive interval for the transcript-selected arithmetic prime.
    pub fn prime_modulus_bounds(&self) -> (u128, u128) {
        super::shared_ring::prime_bounds(self.target_bits).expect("prepared shared target")
    }

    pub fn live_arithmetic_bits_per_signature(&self) -> usize {
        self.layout.live_bits()
    }
    pub fn source_bits_per_signature(&self) -> [usize; SOURCE_COUNT] {
        std::array::from_fn(|i| {
            if i == 0 {
                self.layout.signature_stride()
            } else {
                (1 << self.keccak[i - 1].bit_vars()) / self.capacity()
            }
        })
    }

    pub fn security(&self) -> FalconHybridSecurity {
        let d = self.capacity().ilog2() as usize;
        let schedule = super::FalconSecuritySchedule::for_layout(self.target_bits, &self.layout)
            .expect("prepared target");
        let numerators = super::piop::FalconSecurityNumerators::for_layout(&self.layout);
        let prime_bits = self.prime_modulus_bounds().0.ilog2() as i32;
        let prime = |n: usize, g: u32| n as f64 * 2f64.powi(-prime_bits - g as i32);
        let binary = |n: usize| n as f64 * 2f64.powi(-128 - self.binary_grinding(n) as i32);
        let terms = vec![
            ("prime sampling", 2f64.powi(-144)),
            (
                "per-signature norm identity",
                prime(numerators.norm_instances, schedule.norm_instance_bits),
            ),
            (
                "integer relation batching",
                prime(
                    numerators.relation_batching,
                    schedule.relation_batching_bits,
                ),
            ),
            (
                "HashToPoint initial row point",
                prime(numerators.outer_point, schedule.outer_point_bits),
            ),
            (
                "combined integer outer sumcheck",
                prime(numerators.integer_rounds, schedule.integer_round_bits),
            ),
            (
                "shared ring outer and endpoint batch",
                (4 * d + 2 * N + 1) as f64
                    / crate::piop::spartan::falcon_parameters::field_cardinality(EXTENSION_DEGREE)
                        .as_f64(),
            ),
            (
                "integer polynomial projection",
                prime(
                    2 * EXTENSION_DEGREE - 2,
                    super::shared_ring::projection_grinding_bits(self.target_bits),
                ),
            ),
            (
                "linear constraints and terminal batching",
                prime(numerators.linear, schedule.linear_point_bits),
            ),
            (
                "prime source sumcheck",
                prime(numerators.binding, schedule.binding_round_bits),
            ),
            (
                "batched integer-to-binary forest",
                binary(hybrid_bridge::error_numerator(
                    &self.layout,
                    self.bridge_mode,
                )),
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
            (
                "SHAKE wiring, source padding and binary claim batching",
                binary(LINK_ERROR_NUMERATOR),
            ),
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
        let nonces: Vec<_> = public.signatures.iter().map(|s| s.nonce).collect();
        let (keccak_packed, auxiliary, samples) = {
            let _span = tracing::info_span!("falcon_hybrid:shake_witness").entered();
            self.generate_shake(&nonces, &public.messages)?
        };
        let traces = {
            use crate::piop::spartan::falcon_polynomial::PolynomialWorkspace;
            let _span = tracing::info_span!("falcon_hybrid:arithmetic_witness").entered();
            samples
                .into_par_iter()
                .enumerate()
                .map_init(
                    || PolynomialWorkspace::new(N),
                    |workspace, (i, words)| {
                        let htp = super::hash_to_point::from_shake_words(
                            Box::new(words),
                            KeccakTrace::default(),
                        )?;
                        super::verify::trace_from_parts(
                            public.public_keys[i].clone(),
                            public.signatures[i].clone(),
                            htp,
                            workspace,
                        )
                    },
                )
                .collect::<Result<Vec<_>, FalconError>>()?
        };
        let messages: Vec<&[u8]> = public.messages.iter().map(|m| m.as_slice()).collect();
        let arithmetic = {
            let _span = tracing::info_span!("falcon_hybrid:source_construction").entered();
            FalconSourceWitness::from_decoded_traces(
                self.layout,
                &messages,
                &public.signatures,
                &traces,
            )?
        };
        let packing_span = tracing::info_span!("falcon_hybrid:source_packing").entered();
        let mut arithmetic_packed = Vec::with_capacity(1 << self.geometry.physical_logs[0]);
        for row in arithmetic.rows() {
            arithmetic_packed.extend(row.chunks_exact(2).map(|w| Gf { lo: w[0], hi: w[1] }));
        }
        arithmetic_packed.resize(1 << self.geometry.physical_logs[0], Gf::ZERO);
        drop(packing_span);
        let packed: [Vec<Gf>; SOURCE_COUNT] = std::iter::once(arithmetic_packed)
            .chain(keccak_packed)
            .collect::<Vec<_>>()
            .try_into()
            .ok()
            .expect("profile sources");
        let rate = self.ligerito.prover().log_inv_rates[0];
        let (source_root, data) =
            shared::commit_sources(&self.geometry, packed.each_ref().map(Vec::as_slice), rate)
                .map_err(error)?;
        let statement = FalconHybridStatement {
            public,
            source_root,
        };
        Ok(CommittedFalconHybrid {
            statement,
            traces,
            arithmetic,
            auxiliary,
            packed,
            data,
        })
    }

    fn generate_shake(
        &self,
        nonces: &[[u8; 40]],
        messages: &[[u8; 32]],
    ) -> Result<
        (
            [Vec<Gf>; KECCAK_SLABS],
            [KeccakAuxiliary; KECCAK_SLABS],
            Vec<[u16; hybrid_keccak::SAMPLES]>,
        ),
        FalconError,
    > {
        let mut states: Vec<_> = nonces
            .iter()
            .zip(messages)
            .map(|(nonce, message)| hybrid_keccak::initial_state(nonce, message))
            .collect();
        let mut packed = Vec::new();
        let mut auxiliary = Vec::new();
        let mut outputs = Vec::new();
        for slab in &self.keccak {
            let (source, aux, out) = slab.generate_chain(&states).map_err(error)?;
            states = out[..self.batch()]
                .iter()
                .map(|chain| *chain.last().expect("nonempty slab"))
                .collect();
            packed.push(source);
            auxiliary.push(aux);
            outputs.push(out);
        }
        let samples = crate::utils::cfg_into_iter!(0..self.batch())
            .map(|signature| {
                std::array::from_fn(|sample| {
                    let byte = 2 * sample;
                    let permutation = byte / hybrid_keccak::RATE_BYTES;
                    let slab = Self::slab_for(permutation);
                    let local = permutation - PARAMETERS.slab(slab).0;
                    let out = &outputs[slab][signature][local];
                    let offset = byte % hybrid_keccak::RATE_BYTES;
                    let at = |i: usize| (out[i / 8] >> (8 * (i % 8))) as u8;
                    u16::from_be_bytes([at(offset), at(offset + 1)])
                })
            })
            .collect();
        Ok((
            packed.try_into().ok().expect("profile slabs"),
            auxiliary.try_into().ok().expect("profile slabs"),
            samples,
        ))
    }

    fn slab_for(permutation: usize) -> usize {
        (0..KECCAK_SLABS)
            .find(|&i| {
                let (first, count) = PARAMETERS.slab(i);
                (first..first + count).contains(&permutation)
            })
            .expect("profile permutation")
    }

    fn transcript(
        &self,
        statement: &FalconHybridStatement,
    ) -> Result<(Blake3Transcript, [u8; 32]), FalconError> {
        statement.public.validate(self.batch())?;
        let mut h = blake3::Hasher::new();
        h.update(crate::piop::spartan::falcon::PROTOCOL_ID.as_bytes());
        for n in [
            self.batch(),
            self.capacity(),
            self.target_bits,
            self.max_batch,
            N,
            EXTENSION_DEGREE,
            super::BETA_SQUARED as usize,
            super::HASH_TO_POINT_SAMPLES,
            SIGNATURE_BITS,
            crate::piop::spartan::falcon_parameters::extension_constant(EXTENSION_DEGREE) as usize,
        ] {
            h.update(&(n as u64).to_le_bytes());
        }
        for log in self.geometry.logs {
            h.update(&(log as u64).to_le_bytes());
        }
        h.update(b"bounded14:low13+4097*top;ring:Q12289,T^k+T+c;Dlen:N-1;profile/v2");
        h.update(b"source-layout:aligned16;polynomials:S1,S2,C,H;slack:S1-lane15;signature:header-nonce-and-mapped-signed-payload/v2");
        h.update(b"arithmetic-padding:random-local-and-instance-equality;active-holes;all-inactive;zero-target/v1");
        h.update(&(LINK_ERROR_NUMERATOR as u64).to_le_bytes());
        h.update(&self.binary_grinding(LINK_ERROR_NUMERATOR).to_le_bytes());
        h.update(&(self.layout.occupied_bits() as u64).to_le_bytes());

        h.update(b"shared:all-E;C,H,S1,S2:1,l,l2,l3;direct-beta-decoder;Hunsigned14;S2encoded-alias;live-mask;P(2k-1)-i128;target-selected-prime-and-bridge;merge-in-binder-block/v3");
        h.update(&(self.layout.live_bits() as u64).to_le_bytes());
        h.update(&(self.layout.public_key_offset() as u64).to_le_bytes());
        let (prime_min, prime_max) = self.prime_modulus_bounds();
        h.update(&prime_min.to_le_bytes());
        h.update(&prime_max.to_le_bytes());
        h.update(self.integer_bridge_name().as_bytes());
        let bridge_layout = self.layout.bitz_params();
        h.update(&(bridge_layout.row_vars as u64).to_le_bytes());
        h.update(&(bridge_layout.col_vars as u64).to_le_bytes());
        h.update(&super::shared_ring::projection_grinding_bits(self.target_bits).to_le_bytes());

        h.update(b"hash-to-point:transcript-bound-public-selection;canonical-mask;first-N-accepted;linear-C-remainder-routing;quadratic-rejection/v1");
        h.update(&(super::HASH_TO_POINT_SAMPLES as u64).to_le_bytes());
        let schedule = super::FalconSecuritySchedule::for_layout(self.target_bits, &self.layout)
            .expect("prepared security");
        for bits in [
            schedule.norm_instance_bits,
            schedule.outer_point_bits,
            schedule.relation_batching_bits,
            schedule.integer_round_bits,
            schedule.linear_point_bits,
            schedule.binding_round_bits,
        ] {
            h.update(&bits.to_le_bytes());
        }
        // The bridge schedule is public and deterministic, but bind it explicitly
        // so proofs cannot be replayed under a different bridge error budget.
        let bridge_numerator = hybrid_bridge::error_numerator(&self.layout, self.bridge_mode);
        h.update(&(bridge_numerator as u64).to_le_bytes());
        h.update(&self.binary_grinding(bridge_numerator).to_le_bytes());
        h.update(
            b"source:joint-rs-row,canonical-16-lanes;keccak:constant-prefix-mask,zero-inactive/v1",
        );
        for n in std::iter::once(self.geometry.position_log)
            .chain(std::iter::once(self.geometry.virtual_lane_log))
            .chain(self.geometry.physical_logs)
            .chain(self.geometry.lane_logs)
            .chain(self.geometry.offsets)
        {
            h.update(&(n as u64).to_le_bytes());
        }
        h.update(&statement.source_root);
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
        t.absorb_slice(b"bitz/falcon/shared-prime/statement/v4");
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
            self.bridge_mode,
            &bridge_claim.point,
            bridge_claim.modulus,
            self.binary_grinding(hybrid_bridge::error_numerator(
                &self.layout,
                self.bridge_mode,
            )),
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
                    &committed.statement.source_root,
                    &digest,
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
        let keccak = prefixes.try_into().expect("profile Keccak slabs");
        let k: [[BinaryClaim; 2]; KECCAK_SLABS] = k
            .try_into()
            .unwrap_or_else(|_| unreachable!("profile Keccak slabs"));
        drop(keccak_span);
        let links_span = tracing::info_span!("falcon_hybrid:link_coefficients").entered();
        let mut link_t = ProverBlockGrindingTranscript::<_, LinkGrinding>::new(
            &mut t,
            self.binary_grinding(LINK_ERROR_NUMERATOR),
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
        let opening = shared::prove_joint_sources_with_security(
            &mut opening_t,
            &self.geometry,
            &digest,
            committed.packed.each_ref().map(Vec::as_slice),
            &self.ligerito,
            &committed.data,
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
            self.bridge_mode,
            &bridge_claim.point,
            proof.arithmetic.binding_terminal[1],
            bridge_claim.modulus,
            &proof.bridge,
            self.binary_grinding(hybrid_bridge::error_numerator(
                &self.layout,
                self.bridge_mode,
            )),
        )?;
        let mut k = Vec::new();
        for slab in 0..KECCAK_SLABS {
            let claims = self.keccak[slab]
                .verify_prefix(
                    &proof.keccak[slab],
                    &statement.source_root,
                    &digest,
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
        let k: [[BinaryClaim; 2]; KECCAK_SLABS] = k
            .try_into()
            .unwrap_or_else(|_| unreachable!("profile Keccak slabs"));
        let mut link_t = VerifierBlockGrindingTranscript::<_, LinkGrinding>::new(
            &mut t,
            self.binary_grinding(LINK_ERROR_NUMERATOR),
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
        shared::verify_joint_with_security(
            &mut opening_t,
            &self.geometry,
            &digest,
            &statement.source_root,
            &point,
            proof.joint.value,
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
        a: &BinaryClaim,
        k: &[[BinaryClaim; 2]; KECCAK_SLABS],
    ) -> ([Coefficients; SOURCE_COUNT], Gf) {
        let a = std::slice::from_ref(a);
        t.absorb_slice(b"bitz/falcon-hybrid/binary-claims-shake-wiring-and-padding/v4");
        for claims in std::iter::once(a).chain(k.iter().map(|claims| claims.as_slice())) {
            t.absorb_slice(&(claims.len() as u64).to_le_bytes());
            for c in claims {
                t.absorb_slice(&c.value.lo.to_le_bytes());
                t.absorb_slice(&c.value.hi.to_le_bytes());
            }
        }
        let mut result = std::array::from_fn(|_| Coefficients::default());
        let mut target = Gf::ZERO;
        for (branch, claims) in std::iter::once(a)
            .chain(k.iter().map(|claims| claims.as_slice()))
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
        let local_point: Vec<Gf> = t.get_field_challenges(
            (1600 * hybrid_keccak::PERMUTATIONS + 16 * super::HASH_TO_POINT_SAMPLES)
                .next_power_of_two()
                .ilog2() as usize,
            &(),
        );
        let instance_point: Vec<Gf> = t.get_field_challenges(self.capacity().ilog2() as usize, &());
        let eta: Gf = t.get_field_challenge(&());
        let local = eq_table(&local_point);
        let instances = eq_table(&instance_point);
        let offsets = self.layout.offsets();
        let mut arithmetic = Vec::with_capacity(320 + 16 * super::HASH_TO_POINT_SAMPLES);
        let mut binary: [Vec<(usize, Gf)>; KECCAK_SLABS] = std::array::from_fn(|_| Vec::new());
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
                arithmetic.push((self.layout.signature_bit(1 + bit / 8, bit % 8), w));
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
                let slab = Self::slab_for(permutation);
                let previous = Self::slab_for(permutation - 1);
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
                let w = eta * local[1600 * hybrid_keccak::PERMUTATIONS + 16 * sample + bit];
                let slab = Self::slab_for((2 * sample) / hybrid_keccak::RATE_BYTES);
                binary[slab].push((
                    local_index(slab, self.keccak[slab].sample_bit_index(0, sample, bit)),
                    w,
                ));
                arithmetic.push((offsets.hash_words + 16 * sample + bit, w));
            }
        }
        self.add_arithmetic_padding(t, &mut result[0], &instance_point);
        if self.batch() == self.capacity() {
            result[0]
                .gathers
                .push(Gather::repeated(arithmetic, instance_point.clone()));
            for (slab, entries) in binary.into_iter().enumerate() {
                let mut repeat = instance_point.clone();
                // The four-permutation slab still has a padded signature
                // coordinate at batch one; its wiring selects signature zero.
                repeat.resize(self.keccak[slab].capacity().ilog2() as usize, Gf::ZERO);
                result[slab + 1]
                    .gathers
                    .push(Gather::repeated_at(entries, repeat, 7));
            }
            return (result, target);
        }
        // The public live prefix masks the repeat weights directly, so the
        // same local wiring is sorted and scanned only once per source.
        result[0].gathers.push(
            Gather::repeated(arithmetic, instance_point.clone()).with_repeat_limit(self.batch()),
        );
        for (slab, entries) in binary.into_iter().enumerate() {
            let mut repeat = instance_point.clone();
            repeat.resize(self.keccak[slab].capacity().ilog2() as usize, Gf::ZERO);
            result[slab + 1]
                .gathers
                .push(Gather::repeated_at(entries, repeat, 7).with_repeat_limit(self.batch()));
        }
        (result, target)
    }

    /// Authenticate every arithmetic padding bit through the existing joint
    /// source opening. A fresh random MLE of those bits must evaluate to zero.
    fn add_arithmetic_padding(
        &self,
        t: &mut impl Transcript,
        coefficients: &mut Coefficients,
        instance_point: &[Gf],
    ) {
        let point: Vec<Gf> =
            t.get_field_challenges(self.layout.signature_stride().ilog2() as usize, &());
        let scale: Gf = t.get_field_challenge(&());
        let local: Vec<_> = eq_table(&point)
            .into_iter()
            .map(|weight| weight * scale)
            .collect();
        let holes = local
            .iter()
            .enumerate()
            .filter_map(|(index, &weight)| self.layout.is_padding(index).then_some((index, weight)))
            .collect();
        coefficients
            .gathers
            .push(Gather::repeated(holes, instance_point.to_vec()).with_repeat_limit(self.batch()));
        if self.batch() < self.capacity() {
            // Disjoint from active holes, including for inactive internal holes.
            // Mask the repeat axis rather than expanding every inactive source.
            coefficients.gathers.push(
                Gather::repeated(
                    local.into_iter().enumerate().collect(),
                    instance_point.to_vec(),
                )
                .with_repeat_range(self.batch(), self.capacity()),
            );
        }
    }
}

#[cfg(test)]
pub(super) use crate::hybrid::joint_sumcheck::live_subcubes;
falcon_tests! {
mod tests {
    use super::super::{decode_signature_ct, HASH_TO_POINT_SAMPLES};
    use super::*;
    const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
    const MSG: &[u8] = include_bytes!("fixtures/message.bin");
    const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");

    fn public(batch: usize) -> FalconPublicStatement {
        FalconPublicStatement::from_bytes(&vec![PK; batch], &vec![MSG; batch], &vec![SIG; batch])
            .unwrap()
    }

    fn recommit(prepared: &PreparedFalconHybrid, committed: &mut CommittedFalconHybrid) {
        let (root, data) = shared::commit_sources(
            &prepared.geometry,
            committed.packed.each_ref().map(Vec::as_slice),
            prepared.ligerito.prover().log_inv_rates[0],
        )
        .unwrap();
        committed.statement.source_root = root;
        committed.data = data;
    }

    /// Exercise the compact transport through the complete Falcon verifier,
    /// including its statement identifier and external grinding nonce frame.
    fn assert_compact_opening_rejects_mutations(
        prepared: &PreparedFalconHybrid,
        statement: &FalconHybridStatement,
        proof: &FalconHybridProof,
    ) {
        let config = prepared.ligerito.verifier();
        let pcs = &proof.opening.ligerito;
        let occupied: usize = prepared
            .geometry
            .lane_logs
            .iter()
            .map(|&log| 1 << log)
            .sum();
        assert_eq!(occupied, if prepared.batch() == 1 { 13 } else { 11 });
        assert_eq!(pcs.initial_proof.opened_rows.len(), config.queries[0]);
        assert!(
            pcs.initial_proof
                .opened_rows
                .iter()
                .all(|row| row.len() == occupied)
        );
        let last = config.recursive_steps - 1;
        let final_columns = 1usize << config.recursive_log_msg_cols[last];
        let final_lanes = 1usize << config.recursive_ks[last];
        assert_eq!(final_columns, 32);
        assert_eq!(pcs.final_proof.len(), final_columns * final_lanes);
        assert_eq!(
            pcs.final_proof.len(),
            if prepared.batch() == 1 { 128 } else { 64 }
        );
        // Even before counting removed authentication hashes and row framing,
        // the full committed message is smaller than the former final payload.
        assert!(pcs.final_proof.len() < final_columns + config.queries[last + 1] * final_lanes);
        assert_eq!(pcs.initial_root, prepared.transcript(statement).unwrap().1);
        assert_eq!(pcs.recursive_roots.len(), config.recursive_steps);
        assert_eq!(pcs.grinding_nonces.len(), config.recursive_steps + 1);
        assert_eq!(
            proof
                .payload_size_breakdown()
                .iter()
                .map(|(_, bytes)| bytes)
                .sum::<usize>(),
            proof.payload_size_bytes()
        );

        let reject = |label: &str, altered: &FalconHybridProof| {
            assert!(
                prepared.verify(statement, altered).is_err(),
                "accepted {label}: batch={}, target={}",
                prepared.batch(),
                prepared.target_bits,
            );
        };
        for branch in 0..3 {
            let mut altered = proof.clone();
            altered.opening.ligerito.initial_proof.opened_rows[0]
                [prepared.geometry.offset(branch)] += Gf::ONE;
            reject("changed source lane", &altered);
        }
        if occupied < 16 {
            let mut altered = proof.clone();
            altered.opening.ligerito.initial_proof.opened_rows[0].resize(16, Gf::ZERO);
            reject("noncompact sixteen-lane row", &altered);
        }
        let mutations: &[(&str, fn(&mut FalconHybridProof))] = &[
            ("short initial row", |p| {
                p.opening.ligerito.initial_proof.opened_rows[0].pop();
            }),
            ("extra initial field", |p| {
                p.opening.ligerito.initial_proof.opened_rows[0].push(Gf::ZERO);
            }),
            ("missing initial query", |p| {
                p.opening.ligerito.initial_proof.opened_rows.pop();
            }),
            ("extra initial query", |p| {
                let row = p.opening.ligerito.initial_proof.opened_rows[0].clone();
                p.opening.ligerito.initial_proof.opened_rows.push(row);
            }),
            ("changed initial authentication", |p| {
                p.opening.ligerito.initial_proof.merkle_proof[0][0] ^= 1;
            }),
            ("changed first final-message value", |p| {
                p.opening.ligerito.final_proof[0] += Gf::ONE;
            }),
            ("changed last final-message value", |p| {
                *p.opening.ligerito.final_proof.last_mut().unwrap() += Gf::ONE;
            }),
            ("empty final message", |p| {
                p.opening.ligerito.final_proof.clear();
            }),
            ("short final message", |p| {
                p.opening.ligerito.final_proof.pop();
            }),
            ("extra final-message value", |p| {
                p.opening.ligerito.final_proof.push(Gf::ZERO);
            }),
            ("wrong last recursive root", |p| {
                p.opening.ligerito.recursive_roots.last_mut().unwrap()[0] ^= 1;
            }),
            ("changed final consumed sumcheck", |p| {
                let rounds = &mut p.opening.ligerito.sumcheck_transcript;
                let last_consumed = rounds.len() - 2;
                rounds[last_consumed].u_0 += Gf::ONE;
            }),
            ("wrong PCS statement identifier", |p| {
                p.opening.ligerito.initial_root[0] ^= 1;
            }),
            ("missing query nonce", |p| {
                p.opening.ligerito.grinding_nonces.pop();
            }),
            ("extra query nonce", |p| {
                p.opening.ligerito.grinding_nonces.push(0);
            }),
            ("extra fold nonce", |p| {
                p.opening.ligerito.fold_grinding_nonces.push(0);
            }),
            ("extra auxiliary PCS nonce", |p| {
                p.pcs_nonces.push(0);
            }),
        ];
        for &(label, mutate) in mutations {
            let mut altered = proof.clone();
            mutate(&mut altered);
            reject(label, &altered);
        }
        if !pcs.fold_grinding_nonces.is_empty() {
            let mut altered = proof.clone();
            altered.opening.ligerito.fold_grinding_nonces.pop();
            reject("missing fold nonce", &altered);
        }
        if !proof.pcs_nonces.is_empty() {
            let mut altered = proof.clone();
            altered.pcs_nonces.pop();
            reject("missing auxiliary PCS nonce", &altered);
        }
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
            let prepared = PreparedFalconHybrid::prepare(batch, 100, 1024).unwrap();
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
                let prepared = PreparedFalconHybrid::prepare(batch, target, 1024).unwrap();
                let security = prepared.security();
                assert!(security.algebraic_bits >= target as f64);
                let numerator =
                    hybrid_bridge::error_numerator(&prepared.layout, prepared.bridge_mode);
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
                assert_eq!(FalconSourceLayout::counts().total(), 100_482);
            }
        }
        assert!(PreparedFalconHybrid::prepare(1025, 128, 1024).is_err());
    }

    #[test]
    fn composition_meets_both_targets_with_exact_rational_bounds() {
        use num_bigint::BigUint;
        const SCALE: usize = 160;
        let ring_denominator = BigUint::from(super::super::Q as u64).pow(EXTENSION_DEGREE as u32);
        for batch in 1usize..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let d = layout.capacity().ilog2() as usize;
            for target in [100, 128] {
                let schedule = super::super::FalconSecuritySchedule::for_layout(target, &layout).unwrap();
                let n = super::super::piop::FalconSecurityNumerators::for_layout(&layout);
                // Sampler, complete PCS, and six binary components use disjoint budgets.
                let mut dyadic = (BigUint::from(1u8) << (SCALE - 144))
                    + (BigUint::from(1u8) << (SCALE - target - 2))
                    + (BigUint::from(6u8) << (SCALE - target - 8));
                let prime_bits = if target == 100 { 114 } else { 125 };
                for (numerator, bits) in [
                    (n.norm_instances, schedule.norm_instance_bits),
                    (n.relation_batching, schedule.relation_batching_bits),
                    (n.outer_point, schedule.outer_point_bits),
                    (n.integer_rounds, schedule.integer_round_bits),
                    (n.linear, schedule.linear_point_bits),
                    (n.binding, schedule.binding_round_bits),
                    (2 * EXTENSION_DEGREE - 2, super::super::shared_ring::projection_grinding_bits(target)),
                ] {
                    dyadic += BigUint::from(numerator) << (SCALE - prime_bits - bits as usize);
                }
                let error = dyadic * &ring_denominator + (BigUint::from(4 * d + 2 * N + 1) << SCALE);
                let budget = &ring_denominator << (SCALE - target);
                assert!(error <= budget, "batch {batch}, target {target}");
            }
        }
    }

    #[test]
    fn hybrid_falcon_roundtrip_and_tampering() {
        let prepared = PreparedFalconHybrid::prepare(1, 100, 1024).unwrap();
        let committed = prepared.commit(public(1)).unwrap();
        let native = super::super::verification_trace(PK, MSG, SIG).unwrap();
        assert_eq!(committed.traces[0].s1, native.s1);
        assert_eq!(committed.traces[0].ring_quotient, native.ring_quotient);
        assert_eq!(
            committed.traces[0].hash_to_point.words,
            native.hash_to_point.words
        );
        assert_eq!(committed.traces[0].norm, native.norm);
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();
        assert_compact_opening_rejects_mutations(&prepared, &statement, &proof);
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
        changed.source_root[0] ^= 1;
        assert!(prepared.verify(&changed, &proof).is_err());
        let mut changed = proof.clone();
        match &mut changed.bridge.sums {
            crate::bitz::column_sums::ColumnSums::Unsplit(sums) => sums[0] ^= 1,
            crate::bitz::column_sums::ColumnSums::Split(sums) => sums[0].lower ^= 1,
        }
        assert!(prepared.verify(&statement, &changed).is_err());
        let mut changed = proof.clone();
        changed.joint.value += Gf::ONE;
        assert!(prepared.verify(&statement, &changed).is_err());
        let mut changed = proof.clone();
        changed.pcs_nonces.push(0);
        assert!(prepared.verify(&statement, &changed).is_err());
    }

    #[test]
    #[ignore = "full profile matrix includes batch 1024; run serially for qualification"]
    fn joint_source_supported_profiles_roundtrip() {

            for target in [100, 128] {
                for batch in [1, 2, 3, 7, 8, 9, 1024] {
                    let prepared =
                        PreparedFalconHybrid::prepare(batch, target, 1024).unwrap();
                    let committed = prepared.commit(public(batch)).unwrap();
                    let statement = committed.statement.clone();
                    let proof = prepared.prove(committed).unwrap();
                    prepared.verify(&statement, &proof).unwrap();
                    assert!(prepared.security().algebraic_bits >= target as f64);
                    assert_eq!(
                        proof
                            .payload_size_breakdown()
                            .iter()
                            .map(|(_, n)| n)
                            .sum::<usize>(),
                        proof.payload_size_bytes(),
                    );
                }
            }

    }

    #[test]
    fn shared_profiles_bind_target_selected_prime_and_bridge() {
        for target in [100, 128] {
            for batch in [1, 3, 32, 1024] {
                let prepared = PreparedFalconHybrid::prepare(batch, target, 1024).unwrap();
                let (min, max) = prepared.prime_modulus_bounds();
                let expected = if target == 100 {
                    (
                        (1u128 << 114, (1u128 << 115) - (1u128 << 102) - 1),
                        "wfbitz-unsplit",
                    )
                } else {
                    ((1u128 << 125, (1u128 << 126) - 1), "wfbitz-joint-limbs")
                };
                assert_eq!((min, max), expected.0);
                assert_eq!(prepared.integer_bridge_name(), expected.1);
                assert!(prepared.security().algebraic_bits >= target as f64);

            }
        }
        assert!(PreparedFalconHybrid::prepare(1, 127, 1024).is_err());
    }

    #[test]
    fn shared_prime_hybrid_complete_proofs_and_protocol_binding() {
        for (batch, target) in [(1, 100), (3, 100), (9, 100), (1, 128), (3, 128)] {
            let prepared = PreparedFalconHybrid::prepare(batch, target, 1024).unwrap();
            assert_eq!(prepared.live_arithmetic_bits_per_signature(), 100_482);
            let committed = prepared.commit(public(batch)).unwrap();
            let statement = committed.statement.clone();
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&statement, &proof).unwrap();
            if batch <= 3 {
                assert_compact_opening_rejects_mutations(&prepared, &statement, &proof);
            }
            assert!(proof.payload_size_bytes() > 0);
            assert_eq!(
                proof.encode_integer_column_sums().len(),
                prepared.layout.bitz_params().cols() * if target == 100 { 16 } else { 20 }
            );
            assert_eq!(
                proof
                    .payload_size_breakdown()
                    .iter()
                    .map(|(_, bytes)| bytes)
                    .sum::<usize>(),
                proof.payload_size_bytes()
            );
            let other_target = PreparedFalconHybrid::prepare(
                batch,
                if target == 100 { 128 } else { 100 }, 1024)
            .unwrap();
            assert!(other_target.verify(&statement, &proof).is_err());
            let mut wrong_shape = proof.clone();
            wrong_shape.bridge.sums = if target == 100 {
                crate::bitz::column_sums::ColumnSums::Split(vec![
                        crate::bitz::column_sums::LargeNumber { lower: 0, upper: 0 };
                        proof.bridge.sums.len()
                    ])
            } else {
                crate::bitz::column_sums::ColumnSums::Unsplit(vec![0; proof.bridge.sums.len()])
            };
            assert!(prepared.verify(&statement, &wrong_shape).is_err());
            let mut wrong = statement.clone();
            wrong.public.public_keys[0].h[0] =
                (wrong.public.public_keys[0].h[0] + 1) % super::super::Q as u16;
            assert!(prepared.verify(&wrong, &proof).is_err());
            let mut wrong = statement.clone();
            wrong.source_root[0] ^= 1;
            assert!(prepared.verify(&wrong, &proof).is_err());
            let mut wrong = proof.clone();
            match &mut wrong.bridge.sums {
                crate::bitz::column_sums::ColumnSums::Unsplit(sums) => {
                    assert_eq!(target, 100);
                    sums[0] ^= 1;
                }
                crate::bitz::column_sums::ColumnSums::Split(sums) => {
                    assert_eq!(target, 128);
                    sums[0].lower ^= 1;
                }
            }
            assert!(prepared.verify(&statement, &wrong).is_err());
            let mut wrong = proof.clone();
            let ring = &mut wrong.arithmetic.ring;
            ring.lift[0] += 1;
            assert!(prepared.verify(&statement, &wrong).is_err());
        }
    }

    #[test]
    fn public_selection_mask_is_canonical_and_transcript_bound() {
        for target in [100, 128] {
            let prepared = PreparedFalconHybrid::prepare(3, target, 1024).unwrap();
            let committed = prepared.commit(public(3)).unwrap();
            let statement = committed.statement.clone();
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&statement, &proof).unwrap();

            let mut changed = proof.clone();
            let mask = &mut changed.arithmetic.selection_masks[1];
            let selected = (0..HASH_TO_POINT_SAMPLES)
                .find(|&j| mask[j / 8] >> (j % 8) & 1 != 0).unwrap();
            let unselected = (0..HASH_TO_POINT_SAMPLES)
                .find(|&j| mask[j / 8] >> (j % 8) & 1 == 0).unwrap();
            // The mask remains canonical and has exactly N ones. It must
            // nevertheless change the transcript and invalidate the proof.
            mask[selected / 8] ^= 1 << (selected % 8);
            mask[unselected / 8] ^= 1 << (unselected % 8);
            assert!(prepared.verify(&statement, &changed).is_err());

            let mut changed = proof.clone();
            changed.arithmetic.selection_masks[0].pop();
            assert!(prepared.verify(&statement, &changed).is_err());
            let mut changed = proof.clone();
            changed.arithmetic.selection_masks.pop();
            assert!(prepared.verify(&statement, &changed).is_err());
        }
    }

    #[test]
    fn recommitted_hash_to_point_words_remainders_and_rejection_bits_are_rejected() {
        for target in [100, 128] {
            let prepared = PreparedFalconHybrid::prepare(3, target, 1024).unwrap();
            let layout = prepared.layout;
            let offsets = layout.offsets();
            let cases = [
                ("SHAKE word", offsets.hash_words),
                ("quotient", offsets.hash_quotients),
                ("remainder", offsets.hash_remainders),
                ("rejection bit", offsets.hash_accept_ands),
                ("C unsigned top bit", layout.hash_point_bit(0, 13)),
            ];
            for (case, (name, local)) in cases.into_iter().enumerate() {
                let mut committed = prepared.commit(public(3)).unwrap();
                let flat = (case % 3) * layout.signature_stride() + local;
                assert!(!layout.is_padding(local));
                // Authenticate the changed bit in both prover representations
                // and the Merkle tree. Native traces deliberately stay fixed:
                // cached row evaluations must still bind to these source bits.
                committed.arithmetic.flip_bit(flat);
                let word = &mut committed.packed[0][flat / 128];
                if flat % 128 < 64 {
                    word.lo ^= 1 << (flat % 64);
                } else {
                    word.hi ^= 1 << (flat % 64);
                }
                recommit(&prepared, &mut committed);
                let statement = committed.statement.clone();
                if let Ok(proof) = prepared.prove(committed) {
                    assert!(
                        prepared.verify(&statement, &proof).is_err(),
                        "accepted recommitted {name} at target {target}",
                    );
                }
            }
        }
    }

    #[test]
    fn recommitted_arithmetic_padding_is_rejected() {
        for target in [100, 128] {
            let prepared = PreparedFalconHybrid::prepare(3, target, 1024).unwrap();
            let layout = prepared.layout;
            let stride = layout.signature_stride();
            let holes = [
                layout.s1_bit(0, 14),
                layout.s1_bit(super::super::NORM_BITS, 15),
                layout.s2_bit(0, SIGNATURE_BITS),
                layout.hash_point_bit(0, 14),
                layout.public_key_bit(super::super::N - 1, 14),
                layout.occupied_bits(),
                stride - 1,
                3 * stride,
                4 * stride - 1,
            ];
            for flat in holes {
                let mut committed = prepared.commit(public(3)).unwrap();
                assert!(!committed.arithmetic.bit(flat));
                assert!(flat >= 3 * stride || layout.is_padding(flat % stride));
                // Both prover representations and the Merkle commitment must
                // agree: this tests zero constraints, not a stale commitment.
                committed.arithmetic.flip_bit(flat);
                let word = &mut committed.packed[0][flat / 128];
                if flat % 128 < 64 {
                    word.lo ^= 1 << (flat % 64);
                } else {
                    word.hi ^= 1 << (flat % 64);
                }
                recommit(&prepared, &mut committed);
                let statement = committed.statement.clone();
                if let Ok(proof) = prepared.prove(committed) {
                    assert!(
                        prepared.verify(&statement, &proof).is_err(),
                        "accepted arithmetic padding bit {flat} at target {target}",
                    );
                }
            }
        }
    }

    #[test]
    fn hybrid_falcon_padding_and_wrong_nonce_branch() {
        let prepared = PreparedFalconHybrid::prepare(3, 100, 1024).unwrap();
        let committed = prepared.commit(public(3)).unwrap();
        let statement = committed.statement.clone();
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&statement, &proof).unwrap();
        assert_compact_opening_rejects_mutations(&prepared, &statement, &proof);

        let prepared = PreparedFalconHybrid::prepare(1, 100, 1024).unwrap();
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
        }
        recommit(&prepared, &mut committed);
        let statement = committed.statement.clone();
        if let Ok(proof) = prepared.prove(committed) {
            assert!(prepared.verify(&statement, &proof).is_err());
        }
    }

    #[test]
    fn hybrid_falcon_128_roundtrip() {
        for batch in [1, 3] {
            let prepared = PreparedFalconHybrid::prepare(batch, 128, 1024).unwrap();
            let committed = prepared.commit(public(batch)).unwrap();
            let statement = committed.statement.clone();
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&statement, &proof).unwrap();
            assert_compact_opening_rejects_mutations(&prepared, &statement, &proof);
            let mut changed = proof.clone();
            changed.links_nonces.clear();
            assert!(prepared.verify(&statement, &changed).is_err());
        }
    }

    #[test]
    fn hybrid_fused_batch_roundtrip_and_joint_source_authentication() {
        let prepared = PreparedFalconHybrid::prepare(9, 100, 1024).unwrap();
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
        let mut wrong = statement.clone();
        wrong.source_root[0] ^= 1;
        assert!(prepared.verify(&wrong, &proof).is_err());
        for branch in 0..3 {
            let mut wrong = proof.clone();
            wrong.opening.ligerito.initial_proof.opened_rows[0]
                [prepared.geometry.offset(branch)] += Gf::ONE;
            assert!(prepared.verify(&statement, &wrong).is_err());
        }
        let mut wrong = proof.clone();
        wrong.opening.ligerito.initial_proof.merkle_proof[0][0] ^= 1;
        assert!(prepared.verify(&statement, &wrong).is_err());
        let mut wrong = proof.clone();
        wrong
            .opening
            .ligerito
            .initial_proof
            .merkle_proof
            .push([0; 32]);
        assert!(prepared.verify(&statement, &wrong).is_err());
        let mut wrong = proof.clone();
        wrong.opening.ligerito.initial_proof.merkle_proof.pop();
        assert!(prepared.verify(&statement, &wrong).is_err());
        let mut wrong = proof.clone();
        wrong.keccak.swap(0, 1);
        assert!(prepared.verify(&statement, &wrong).is_err());
    }

    #[test]
    fn valid_keccak_tail_cannot_break_the_cross_slab_chain_link() {
        let prepared = PreparedFalconHybrid::prepare(1, 100, 1024).unwrap();
        let mut committed = prepared.commit(public(1)).unwrap();
        // This is a valid four-permutation Keccak chain, but its input is not
        // permutation 15's output from the other authenticated commitment.
        let (packed, auxiliary, _) = prepared.keccak[1].generate_chain(&[[0; 25]]).unwrap();
        committed.packed[2] = packed;
        recommit(&prepared, &mut committed);
        let (_, digest) = prepared.transcript(&committed.statement).unwrap();
        let root = &committed.statement.source_root;
        let (prefix, claims, _) = prepared.keccak[1]
            .prove_prefix(
                &committed.packed[2],
                auxiliary,
                root,
                &digest,
                &mut Blake3Transcript::new(),
                108,
            )
            .unwrap();
        assert_eq!(
            prepared.keccak[1]
                .verify_prefix(&prefix, root, &digest, &mut Blake3Transcript::new(), 108)
                .unwrap(),
            claims
        );
        let (_, auxiliary, _) = prepared.keccak[1].generate_chain(&[[0; 25]]).unwrap();
        committed.auxiliary[1] = auxiliary;
        let statement = committed.statement.clone();
        if let Ok(proof) = prepared.prove(committed) {
            assert!(prepared.verify(&statement, &proof).is_err());
        }
    }
}

}
