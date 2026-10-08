use super::{
    BETA_SQUARED, COEFFICIENT_LOG, Cfg, FalconAlgebraicStatement, FalconAlgebraicWitness,
    FalconError, LIVE_BITS, Layout, N, PRIME_MAX, PRIME_MIN, PROTOCOL_ID, Q, SLACK_BITS, Schedule,
    Source, WitnessData, arithmetic, error, ring,
};
use crate::{
    hybrid::{
        block_grinding::{ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript},
        integer_bridge::{self as bridge, BridgeMode},
        joint_sumcheck::{self as joint, Coefficients, Tensor},
        opening::{
            self as shared,
            grinding::{GrindingContext, GrindingNonces},
        },
        sumcheck::Scratch,
    },
    ligerito_flock::{LigeritoSelection, ResolvedLigerito, grinding_plan::GrindingPlan},
    piop::spartan::grinding::GrindingDomain,
    transcript::{Blake3Transcript, traits::Transcript},
};
use flock_core::field::Gf128 as Gf;
use std::sync::Mutex;

struct OpeningGrinding;
impl GrindingDomain for OpeningGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-algebraic/opening-grinding/v2";
}

/// Accounting under the repository's computational grinding model, excluding
/// the separate 128-bit collision-security assumption for BLAKE3.
#[derive(Clone, Debug)]
pub struct FalconAlgebraicSecurity {
    pub target_bits: usize,
    pub algebraic_bits: f64,
    pub terms: Vec<(&'static str, f64)>,
}

/// Reusable configuration for 1..=1024 algebraic Falcon statements.
pub struct PreparedFalconAlgebraic {
    layout: Layout,
    target_bits: usize,
    geometry: shared::Geometry<1>,
    schedule: Schedule,
    ligerito: ResolvedLigerito,
    pcs_grinding: GrindingPlan,
    scratch: Mutex<Scratch>,
}

/// The single committed witness. Consumed by proving to avoid witness copies.
pub struct CommittedFalconAlgebraic {
    statement: FalconAlgebraicStatement,
    source_root: [u8; 32],
    target_bits: usize,
    witness: WitnessData,
    source: Source,
    packed: Vec<Gf>,
    data: shared::JointProverData<1>,
}

impl CommittedFalconAlgebraic {
    pub fn statement(&self) -> &FalconAlgebraicStatement {
        &self.statement
    }
    pub fn source_root(&self) -> &[u8; 32] {
        &self.source_root
    }
}

/// Non-hiding proof. Public h,t are supplied separately to `verify`.
#[derive(Clone, Debug)]
pub struct FalconAlgebraicProof {
    source_root: [u8; 32],
    ring: ring::Proof,
    arithmetic: arithmetic::Proof,
    bridge: bridge::Proof,
    joint: joint::Proof,
    opening: shared::JointProof,
    opening_nonces: Vec<u64>,
    pcs_nonces: Vec<u64>,
}

impl FalconAlgebraicProof {
    pub fn source_root(&self) -> &[u8; 32] {
        &self.source_root
    }

    /// Stored canonical scalar payload plus bincode's compact Ligerito payload.
    /// Includes the commitment root; excludes public inputs and outer transport
    /// framing. This is not a transport codec or a Rust allocation-size metric.
    pub fn payload_size_bytes(&self) -> usize {
        self.payload_size_breakdown()
            .iter()
            .map(|(_, bytes)| bytes)
            .sum()
    }

    pub fn payload_size_breakdown(&self) -> Vec<(&'static str, usize)> {
        vec![
            ("commitment", 32),
            ("ring", self.ring.payload_size_bytes()),
            ("norm_and_binding", self.arithmetic.payload_size_bytes()),
            (
                "bitz_bridge",
                self.bridge.sums.encoded_len()
                    + 32 * self.bridge.forest.len()
                    + 8 * self.bridge.nonces.len(),
            ),
            (
                "binary_sumcheck",
                16 * (2 * self.joint.rounds.len() + 1) + 8 * self.joint.nonces.len(),
            ),
            (
                "pcs",
                16 * self.opening.ring.s_v.len()
                    + bincode::serialized_size(&self.opening.ligerito)
                        .expect("proof serialization size") as usize
                    + 8 * (self.opening_nonces.len() + self.pcs_nonces.len()),
            ),
        ]
    }
}

impl PreparedFalconAlgebraic {
    pub fn new(batch: usize, target_bits: usize) -> Result<Self, FalconError> {
        let layout = Layout::new(batch)?;
        if !matches!(target_bits, 100 | 128) {
            return Err(error("algebraic security must be 100 or 128 bits"));
        }
        let d = layout.capacity().ilog2() as usize;
        let schedule = Schedule {
            norm_instance_bits: grind(target_bits, d, 125),
            norm_round_bits: grind(target_bits, 4 * (COEFFICIENT_LOG + d), 125),
            merge_bits: grind(target_bits, 6, 125),
            binding_bits: grind(target_bits, 2 * (COEFFICIENT_LOG + 5 + d), 125),
        };
        let geometry = shared::Geometry::new([COEFFICIENT_LOG - 2 + d]).map_err(error)?;
        let ligerito = LigeritoSelection::MATCHED_UDR
            .resolve(geometry.packed_log(), target_bits)
            .map_err(error)?;
        let first =
            GrindingPlan::resolve(ligerito.security(), target_bits as u32).map_err(error)?;
        let pcs_target = target_bits + 2 + first.blocks.len().next_power_of_two().ilog2() as usize;
        let pcs_grinding =
            GrindingPlan::resolve(ligerito.security(), pcs_target as u32).map_err(error)?;
        bridge::validate_layout(&layout.bitz_params(), BridgeMode::TwoLimbs, PRIME_MAX)?;
        let prepared = Self {
            layout,
            target_bits,
            geometry,
            schedule,
            ligerito,
            pcs_grinding,
            scratch: Mutex::default(),
        };
        if prepared.security().algebraic_bits < target_bits as f64 {
            return Err(error("algebraic composition misses security target"));
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
    pub fn live_bits_per_signature(&self) -> usize {
        LIVE_BITS
    }
    pub fn source_bits_per_signature(&self) -> usize {
        self.layout.signature_stride()
    }
    pub fn prime_modulus_bounds(&self) -> (u128, u128) {
        (PRIME_MIN, PRIME_MAX)
    }

    pub fn security(&self) -> FalconAlgebraicSecurity {
        let d = self.capacity().ilog2() as usize;
        let prime = |n: usize, bits: u32| n as f64 * 2f64.powi(-125 - bits as i32);
        let binary = |n: usize| n as f64 * 2f64.powi(-128 - self.binary_grinding(n) as i32);
        let terms = vec![
            ("prime sampling", 2f64.powi(-144)),
            // |E \ F_q| >= 2^149; includes instance batching and degree-(2N-2) identity.
            (
                "native ring identity",
                (2 * N - 2 + d) as f64 * 2f64.powi(-149),
            ),
            (
                "ring coordinate projection",
                prime(10, self.carry_grinding()),
            ),
            (
                "per-signature norm identity",
                prime(d, self.schedule.norm_instance_bits),
            ),
            (
                "norm sumchecks",
                prime(4 * (COEFFICIENT_LOG + d), self.schedule.norm_round_bits),
            ),
            ("claim merge", prime(6, self.schedule.merge_bits)),
            (
                "source binding sumcheck",
                prime(2 * (COEFFICIENT_LOG + 5 + d), self.schedule.binding_bits),
            ),
            (
                "BitZ product GKR",
                binary(bridge::error_numerator(
                    &self.layout.bitz_params(),
                    BridgeMode::TwoLimbs,
                )),
            ),
            (
                "binary source sumcheck",
                binary(2 * self.geometry.bit_log()),
            ),
            ("ring switch", binary(256)),
            (
                "Ligerito",
                self.pcs_grinding
                    .blocks
                    .iter()
                    .map(|block| block.raw_error * 2f64.powi(-(block.bits as i32)))
                    .sum(),
            ),
        ];
        FalconAlgebraicSecurity {
            target_bits: self.target_bits,
            algebraic_bits: -terms.iter().map(|(_, p)| p).sum::<f64>().log2(),
            terms,
        }
    }

    fn carry_grinding(&self) -> u32 {
        grind(self.target_bits, 10, 125)
    }
    fn binary_grinding(&self, numerator: usize) -> u32 {
        grind(self.target_bits, numerator, 128)
    }

    /// Validate, pack and commit the algebraic witness. No signature hashing.
    #[tracing::instrument(skip_all, name = "falcon_algebraic:commit")]
    pub fn commit(
        &self,
        statement: FalconAlgebraicStatement,
        witness: FalconAlgebraicWitness,
    ) -> Result<CommittedFalconAlgebraic, FalconError> {
        statement.validate(self.batch())?;
        let data = {
            let _span = tracing::info_span!("falcon_algebraic:witness").entered();
            WitnessData::new(&statement, witness)?
        };
        self.commit_data(statement, data)
    }

    /// Derive centered s1, check its norm, and commit using one product per signature.
    /// Targets must already be computed; no message hashing or decoding is performed.
    #[tracing::instrument(skip_all, name = "falcon_algebraic:commit")]
    pub fn commit_from_s2(
        &self,
        statement: FalconAlgebraicStatement,
        s2: Vec<[i16; N]>,
    ) -> Result<CommittedFalconAlgebraic, FalconError> {
        statement.validate(self.batch())?;
        if s2.len() != self.batch() {
            return Err(FalconError::InvalidBatchCapacity);
        }
        let data = {
            let _span = tracing::info_span!("falcon_algebraic:witness").entered();
            WitnessData::from_s2(&statement, s2)?
        };
        self.commit_data(statement, data)
    }

    fn commit_data(
        &self,
        statement: FalconAlgebraicStatement,
        data: WitnessData,
    ) -> Result<CommittedFalconAlgebraic, FalconError> {
        let source = Source::new(self.layout, &data);
        self.commit_source(statement, data, source)
    }

    fn commit_source(
        &self,
        statement: FalconAlgebraicStatement,
        witness: WitnessData,
        source: Source,
    ) -> Result<CommittedFalconAlgebraic, FalconError> {
        let packed: Vec<_> = source
            .rows()
            .iter()
            .flat_map(|column| column.chunks_exact(2))
            .map(|pair| Gf {
                lo: pair[0],
                hi: pair[1],
            })
            .collect();
        let (source_root, data) = shared::commit_sources(
            &self.geometry,
            [packed.as_slice()],
            self.ligerito.prover().log_inv_rates[0],
        )
        .map_err(error)?;
        Ok(CommittedFalconAlgebraic {
            statement,
            source_root,
            target_bits: self.target_bits,
            witness,
            source,
            packed,
            data,
        })
    }

    fn transcript(
        &self,
        public: &FalconAlgebraicStatement,
        root: &[u8; 32],
    ) -> Result<(Blake3Transcript, [u8; 32]), FalconError> {
        public.validate(self.batch())?;
        let mut h = blake3::Hasher::new();
        h.update(PROTOCOL_ID.as_bytes());
        h.update(b"ring-coordinate-basis:alpha-powers/canonical/v1");
        h.update(if SLACK_BITS == 27 {
            b"signed15-lanes16,slack27-lane15;native11;two-limbs;one-source;zero-padding/v2"
        } else {
            b"signed15-lanes16,slack26-lane15;native11;two-limbs;one-source;zero-padding/v2"
        });
        for value in [
            N,
            Q as usize,
            BETA_SQUARED as usize,
            self.batch(),
            self.capacity(),
            self.target_bits,
            self.layout.signature_stride(),
            LIVE_BITS,
            self.layout.row_vars(),
            self.geometry.packed_log(),
        ] {
            h.update(&(value as u64).to_le_bytes());
        }
        h.update(&PRIME_MIN.to_le_bytes());
        h.update(&PRIME_MAX.to_le_bytes());
        for bits in [
            self.schedule.norm_instance_bits,
            self.schedule.norm_round_bits,
            self.schedule.merge_bits,
            self.schedule.binding_bits,
            self.carry_grinding(),
            self.binary_grinding(bridge::error_numerator(
                &self.layout.bitz_params(),
                BridgeMode::TwoLimbs,
            )),
            self.binary_grinding(2 * self.geometry.bit_log()),
            self.binary_grinding(256),
        ] {
            h.update(&bits.to_le_bytes());
        }
        for (key, target) in public.public_keys.iter().zip(&public.targets) {
            for coefficient in key.iter().chain(target) {
                h.update(&coefficient.to_le_bytes());
            }
        }
        h.update(root);
        h.update(&self.ligerito.digest());
        for block in &self.pcs_grinding.blocks {
            h.update(&block.bits.to_le_bytes());
        }
        let digest = *h.finalize().as_bytes();
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(PROTOCOL_ID.as_bytes());
        transcript.absorb_slice(&digest);
        self.ligerito.bind(&mut transcript);
        Ok((transcript, digest))
    }

    #[tracing::instrument(skip_all, name = "falcon_algebraic:prove")]
    pub fn prove(
        &self,
        committed: CommittedFalconAlgebraic,
    ) -> Result<FalconAlgebraicProof, FalconError> {
        if committed.source.layout() != &self.layout || committed.target_bits != self.target_bits {
            return Err(error("algebraic commitment configuration mismatch"));
        }
        let (mut t, digest) = self.transcript(&committed.statement, &committed.source_root)?;
        let field = sample_field(&mut t)?;
        let (ring, ring_claim) = {
            let _span = tracing::info_span!("falcon_algebraic:ring").entered();
            ring::prove(
                &mut t,
                &self.layout,
                &committed.statement,
                &committed.witness,
                &field,
                self.carry_grinding(),
            )?
        };
        let (arithmetic, row_weights, claim) = {
            let _span = tracing::info_span!("falcon_algebraic:arithmetic").entered();
            arithmetic::prove(
                &mut t,
                &self.layout,
                &committed.witness,
                &committed.source,
                &field,
                &ring_claim,
                &self.schedule,
            )?
        };
        let (bridge, a) = {
            let _span = tracing::info_span!("falcon_algebraic:bitz").entered();
            bridge::prove(
                &mut t,
                &self.layout.bitz_params(),
                committed.source.rows(),
                BridgeMode::TwoLimbs,
                &claim.point,
                claim.modulus,
                self.binary_grinding(bridge::error_numerator(
                    &self.layout.bitz_params(),
                    BridgeMode::TwoLimbs,
                )),
                &row_weights,
            )?
        };
        drop(row_weights);
        let coefficients = Coefficients {
            tensors: vec![Tensor {
                low: a.low,
                high_point: a.high_point,
                marginals: None,
            }],
            gathers: Vec::new(),
        };
        let (joint, point) = joint::prove(
            &mut t,
            &self.geometry,
            [committed.packed.as_slice()],
            [&coefficients],
            a.value,
            self.binary_grinding(2 * self.geometry.bit_log()),
            &mut *self
                .scratch
                .lock()
                .map_err(|_| error("algebraic scratch lock"))?,
        )
        .map_err(error)?;
        let mut pcs_nonces = Vec::new();
        let _opening_span = tracing::info_span!("falcon_algebraic:pcs").entered();
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
            [committed.packed.as_slice()],
            &self.ligerito,
            &committed.data,
            &point,
            Some(&mut pcs),
        )
        .map_err(error)?;
        let opening_nonces = opening_t.finish();
        Ok(FalconAlgebraicProof {
            source_root: committed.source_root,
            ring,
            arithmetic,
            bridge,
            joint,
            opening,
            opening_nonces,
            pcs_nonces,
        })
    }

    #[tracing::instrument(skip_all, name = "falcon_algebraic:verify")]
    pub fn verify(
        &self,
        public: &FalconAlgebraicStatement,
        proof: &FalconAlgebraicProof,
    ) -> Result<(), FalconError> {
        let (mut t, digest) = self.transcript(public, &proof.source_root)?;
        let field = sample_field(&mut t)?;
        let ring_claim = ring::verify(
            &mut t,
            &self.layout,
            public,
            &proof.ring,
            &field,
            self.carry_grinding(),
        )?;
        let claim = arithmetic::verify(
            &mut t,
            &self.layout,
            &field,
            &ring_claim,
            &proof.arithmetic,
            &self.schedule,
        )?;
        let a = bridge::verify(
            &mut t,
            &self.layout.bitz_params(),
            BridgeMode::TwoLimbs,
            &claim.point,
            claim.value,
            claim.modulus,
            &proof.bridge,
            self.binary_grinding(bridge::error_numerator(
                &self.layout.bitz_params(),
                BridgeMode::TwoLimbs,
            )),
        )?;
        let coefficients = Coefficients {
            tensors: vec![Tensor {
                low: a.low,
                high_point: a.high_point,
                marginals: None,
            }],
            gathers: Vec::new(),
        };
        let point = joint::verify(
            &mut t,
            &self.geometry,
            [&coefficients],
            a.value,
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
            &proof.source_root,
            &point,
            proof.joint.value,
            &self.ligerito,
            &proof.opening,
            Some(&mut pcs),
        )
        .map_err(error)?;
        opening_t.finish().map_err(error)
    }
}

// Eight arithmetic/binary groups split half the total error budget. Within a
// group every adaptive challenge boundary is ground at this difficulty, and
// `numerator` already sums its polynomial degrees over all rounds.
fn grind(target: usize, numerator: usize, domain_bits: u32) -> u32 {
    if numerator == 0 {
        return 0;
    }
    (target as u32 + 4 + numerator.next_power_of_two().ilog2()).saturating_sub(domain_bits)
}

fn sample_field(t: &mut impl Transcript) -> Result<Cfg, FalconError> {
    crate::prime_sampling::sample_prime_context(t, PRIME_MIN, PRIME_MAX, 144).map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;

    pub(super) fn fixture(batch: usize) -> (FalconAlgebraicStatement, FalconAlgebraicWitness) {
        let mut key = [0; N];
        key[0] = 1;
        key[N - 1] = 7;
        let mut s2 = [0i16; N];
        s2[1] = 3000;
        s2[17] = -13;
        let mut target = [0; N];
        for j in 0..N {
            target[j] = i64::from(s2[j]).rem_euclid(Q) as u16;
        }
        target[0] = (-7 * i64::from(s2[1])).rem_euclid(Q) as u16;
        target[16] = (-7 * i64::from(s2[17])).rem_euclid(Q) as u16;
        let public = FalconAlgebraicStatement {
            public_keys: vec![key; batch],
            targets: vec![target; batch],
        };
        let witness = FalconAlgebraicWitness {
            s1: vec![[0; N]; batch],
            s2: vec![s2; batch],
        };
        (public, witness)
    }

    #[test]
    fn configuration_bounds_cover_all_supported_batches() {
        let q = BigUint::from(Q as u64);
        assert!(q.pow(11) - &q >= (BigUint::from(1u8) << 149usize));
        for target in [100, 128] {
            for batch in 1..=1024 {
                // Resolve the PCS at power-of-two batch boundaries.
                let layout = Layout::new(batch).unwrap();
                assert_eq!(
                    layout.capacity(),
                    batch.next_power_of_two().max((1 << 19) / (32 * N))
                );
                let d = layout.capacity().ilog2() as usize;
                for numerator in [
                    d,
                    4 * (COEFFICIENT_LOG + d),
                    6,
                    2 * (COEFFICIENT_LOG + 5 + d),
                    10,
                ] {
                    let g = grind(target, numerator, 125);
                    assert!(
                        (BigUint::from(numerator) << (target + 4))
                            <= (BigUint::from(1u8) << (125 + g as usize))
                    );
                }
                if batch.is_power_of_two() {
                    let p = PreparedFalconAlgebraic::new(batch, target).unwrap();
                    assert!(p.security().algebraic_bits >= target as f64);
                    assert_eq!(p.live_bits_per_signature(), 2 * N * 15 + SLACK_BITS);
                    assert_eq!(p.source_bits_per_signature(), 32 * N);
                    assert_eq!(p.geometry.physical_logs, [COEFFICIENT_LOG - 2 + d]);
                }
            }
        }
        for batch in [0, 1025, usize::MAX] {
            assert!(PreparedFalconAlgebraic::new(batch, 100).is_err());
        }
        assert!(PreparedFalconAlgebraic::new(1, 127).is_err());
    }

    #[test]
    fn end_to_end_and_tampering() {
        for (batch, target) in [100, 128]
            .into_iter()
            .flat_map(|target| [1, 3, 8, 9, 16].map(|batch| (batch, target)))
        {
            let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
            let (public, witness) = fixture(batch);
            let committed = prepared.commit(public.clone(), witness).unwrap();
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&public, &proof).unwrap();
            assert_eq!(
                proof.payload_size_bytes(),
                proof
                    .payload_size_breakdown()
                    .iter()
                    .map(|(_, n)| n)
                    .sum::<usize>()
            );
            let mut bad_public = public.clone();
            bad_public.targets[0][1] ^= 1;
            assert!(prepared.verify(&bad_public, &proof).is_err());
            bad_public = public.clone();
            bad_public.public_keys[0][0] ^= 1;
            assert!(prepared.verify(&bad_public, &proof).is_err());
            for mutation in 0..8 {
                let mut bad = proof.clone();
                match mutation {
                    0 => bad.source_root[0] ^= 1,
                    1 => {
                        bad.ring.certificate[0].0[0] = (bad.ring.certificate[0].0[0] + 1) % Q as u16
                    }
                    2 => bad.ring.carries[0] += 1,
                    3 => {
                        bad.ring.certificate.pop();
                    }
                    4 => {
                        bad.joint.rounds.pop();
                    }
                    5 => bad.joint.value += Gf::ONE,
                    6 => {
                        bad.opening_nonces.push(0);
                    }
                    _ => {
                        bad.pcs_nonces.push(0);
                    }
                }
                assert!(
                    prepared.verify(&public, &bad).is_err(),
                    "mutation {mutation}"
                );
            }
            let other =
                PreparedFalconAlgebraic::new(batch, if target == 100 { 128 } else { 100 }).unwrap();
            assert!(other.verify(&public, &proof).is_err());
        }
    }

    #[test]
    fn combined_commit_matches_separate_preparation() {
        for target in [100, 128] {
            for batch in [1, 3] {
                let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
                let (public, witness) = fixture(batch);
                let separate_witness =
                    FalconAlgebraicWitness::from_s2(&public, witness.s2.clone()).unwrap();
                let separate = prepared.commit(public.clone(), separate_witness).unwrap();
                let combined = prepared.commit_from_s2(public.clone(), witness.s2).unwrap();
                assert_eq!(combined.witness.witness, separate.witness.witness);
                assert_eq!(combined.witness.slacks, separate.witness.slacks);
                assert_eq!(combined.witness.quotients, separate.witness.quotients);
                assert_eq!(combined.source.rows(), separate.source.rows());
                assert_eq!(combined.packed, separate.packed);
                assert_eq!(combined.source_root(), separate.source_root());
                let separate_proof = prepared.prove(separate).unwrap();
                let combined_proof = prepared.prove(combined).unwrap();
                prepared.verify(&public, &separate_proof).unwrap();
                prepared.verify(&public, &combined_proof).unwrap();
                assert_eq!(
                    combined_proof.payload_size_breakdown(),
                    separate_proof.payload_size_breakdown()
                );
            }
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn serial_and_optimized_binding_preserve_complete_proof_contents() {
        const BATCH: usize = 1024;
        let pools = [1, 8].map(|threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .stack_size(8 << 20)
                .build()
                .unwrap()
        });
        let (mut public, mut witness) = fixture(BATCH);
        for instance in 0..BATCH {
            let k = 7 + (instance % 11) as i64;
            let a = 1000 + (17 * instance % 2000) as i64;
            let b = -(13 + (instance % 61) as i64);
            let c = (instance % 23) as i64 - 11;
            public.public_keys[instance][N - 1] = k as u16;
            witness.s1[instance][0] = c as i16;
            witness.s2[instance][1] = a as i16;
            witness.s2[instance][17] = b as i16;
            // c + (1 + k*x^(N-1))*(a*x + b*x^17), with x^N = -1.
            for (coefficient, value) in [(0, c - k * a), (1, a), (16, -k * b), (17, b)] {
                public.targets[instance][coefficient] = value.rem_euclid(Q) as u16;
            }
        }
        for target in [100, 128] {
            let run = |pool: &rayon::ThreadPool, threads| {
                pool.install(|| {
                    assert_eq!(rayon::current_num_threads(), threads);
                    assert_eq!(arithmetic::optimized_binding(BATCH), threads == 8);
                    let prepared = PreparedFalconAlgebraic::new(BATCH, target).unwrap();
                    let committed = prepared.commit(public.clone(), witness.clone()).unwrap();
                    let root = *committed.source_root();
                    let proof = prepared.prove(committed).unwrap();
                    assert_eq!(proof.source_root(), &root);
                    prepared.verify(&public, &proof).unwrap();
                    // Derived Debug includes every proof field, including all
                    // ring, arithmetic, bridge, joint, opening and PCS nonces.
                    // This compares contents within one build, not a wire codec.
                    (root, format!("{proof:?}"))
                })
            };
            let serial = run(&pools[0], 1);
            let optimized = run(&pools[1], 8);
            assert_eq!(serial.0, optimized.0, "degree={N}, security={target}");
            assert_eq!(serial.1, optimized.1, "degree={N}, security={target}");
        }
    }

    #[test]
    fn combined_commit_rejects_batch_mismatches() {
        let prepared = PreparedFalconAlgebraic::new(1, 100).unwrap();
        let (public, witness) = fixture(1);
        for s2 in [vec![], vec![witness.s2[0]; 2], vec![witness.s2[0]; 1025]] {
            assert!(matches!(
                prepared.commit_from_s2(public.clone(), s2),
                Err(FalconError::InvalidBatchCapacity)
            ));
        }
        let (wrong_public, wrong_witness) = fixture(2);
        assert!(matches!(
            prepared.commit_from_s2(wrong_public, wrong_witness.s2),
            Err(FalconError::InvalidBatchCapacity)
        ));
    }

    algebraic_1024_tests! {
    #[test]
    fn supplied_noncentered_s1_still_requires_a_valid_relation() {
        let prepared = PreparedFalconAlgebraic::new(1, 100).unwrap();
        let mut target = [0; N];
        target[0] = 7000;
        let public = FalconAlgebraicStatement {
            public_keys: vec![[0; N]],
            targets: vec![target],
        };
        let mut s1 = [0; N];
        s1[0] = 7000;
        let mut witness = FalconAlgebraicWitness {
            s1: vec![s1],
            s2: vec![[0; N]],
        };
        let committed = prepared.commit(public.clone(), witness.clone()).unwrap();
        assert_eq!(committed.witness.witness.s1[0][0], 7000);
        let proof = prepared.prove(committed).unwrap();
        prepared.verify(&public, &proof).unwrap();
        witness.s1[0][0] += 1;
        assert!(prepared.commit(public, witness).is_err());
    }
    }

    #[test]
    fn recommitted_invalid_assignments_are_rejected() {
        let prepared = PreparedFalconAlgebraic::new(3, 100).unwrap();
        let (public, witness) = fixture(3);
        let data = WitnessData::new(&public, witness).unwrap();
        for index in [
            16 * SLACK_BITS + 15,
            3 * prepared.layout.signature_stride(),
            16 * N,
        ] {
            let mut source = Source::new(prepared.layout, &data);
            source.flip(index);
            let committed = prepared
                .commit_source(public.clone(), data.clone(), source)
                .unwrap();
            assert!(prepared.prove(committed).is_err());
        }
    }

    algebraic_1024_tests! {
    #[test]
    fn original_falcon_fixture_uses_only_algebraic_inputs() {
        use crate::piop::spartan::falcon_profiles::n1024_k11::verification_trace;
        // Hashing happens only in fixture preparation, outside commit/prove/verify.
        let trace = verification_trace(
            include_bytes!("../falcon/fixtures/public_key.bin"),
            include_bytes!("../falcon/fixtures/message.bin"),
            include_bytes!("../falcon/fixtures/signature_ct.bin"),
        )
        .unwrap();
        let public = FalconAlgebraicStatement {
            public_keys: vec![*trace.public_key.h],
            targets: vec![*trace.hash_to_point.point],
        };
        let witness = FalconAlgebraicWitness {
            s1: vec![*trace.s1],
            s2: vec![*trace.signature.s2],
        };
        let prepared = PreparedFalconAlgebraic::new(1, 100).unwrap();
        let combined = prepared
            .commit_from_s2(public.clone(), witness.s2.clone())
            .unwrap();
        let committed = prepared.commit(public.clone(), witness).unwrap();
        assert_eq!(combined.source_root(), committed.source_root());
        for committed in [combined, committed] {
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&public, &proof).unwrap();
        }
    }
    }

    #[test]
    #[ignore = "large end-to-end qualification; run explicitly"]
    fn large_batches_at_both_security_targets() {
        for target in [100, 128] {
            for batch in [32, 1024] {
                let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
                let (public, witness) = fixture(batch);
                let proof = prepared
                    .prove(prepared.commit(public.clone(), witness).unwrap())
                    .unwrap();
                prepared.verify(&public, &proof).unwrap();
            }
        }
    }
}
