use super::{
    BETA_SQUARED, BIT_PROTOCOL_ID, COEFFICIENT_LOG, COMPACT_LIVE_BITS, Cfg, FalconAlgebraicBinding,
    FalconAlgebraicStatement, FalconAlgebraicWitness, FalconError, LIVE_BITS, Layout, N, PRIME_MAX,
    PRIME_MIN, PROTOCOL_ID, Q, SLACK_BITS, Schedule, Source, WitnessData, arithmetic, compact,
    error, ring,
};
use crate::{
    hybrid::{
        block_grinding::{ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript},
        integer_bridge::{self as bridge, BridgeMode},
        joint_sumcheck as joint,
        opening::{
            self as shared,
            grinding::{GrindingContext, GrindingNonces},
        },
        sumcheck::Scratch,
    },
    ligerito_flock::{LigeritoSelection, ResolvedLigerito, grinding_plan::GrindingPlan},
    pcs::IntegerMatrixLayout,
    piop::spartan::grinding::GrindingDomain,
    transcript::{Blake3Transcript, traits::Transcript},
};
use flock_core::field::Gf128 as Gf;
use std::sync::Mutex;

crate::piop::spartan::grinding::grinding_domains! {
    OpeningGrinding => b"bitz/falcon1024-algebraic/opening-grinding/v2";
}

#[cfg(any(test, feature = "bench-internals"))]
struct BridgeMergeGrinding;
#[cfg(any(test, feature = "bench-internals"))]
impl GrindingDomain for BridgeMergeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-algebraic/compact-bridge-merge/v8";
}

/// Difficulties of the binary-field grinding groups. They and the Ligerito
/// blocks split one composed budget work-optimally (see
/// [`crate::hybrid::grinding_allocation`]); the prime-field schedule is fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BinaryGrinding {
    bridge: [u32; 3],
    bridge_merge: u32,
    joint: u32,
    opening: u32,
}

/// The composed error stays at most `ALLOCATION_BUDGET * 2^-target`, i.e. at
/// least `target + log2(16/15)` bits. The factor is exact in binary.
const ALLOCATION_BUDGET: f64 = 0.9375;

/// Accounting under the repository's computational grinding model, excluding
/// the separate 128-bit collision-security assumption for BLAKE3.
#[derive(Clone, Debug)]
pub struct FalconAlgebraicSecurity {
    pub target_bits: usize,
    pub algebraic_bits: f64,
    pub terms: Vec<(&'static str, f64)>,
}

// The minimum PCS domain uses one compact source; larger batches split its
// dense planes and exception tail so the physical commitment can shrink.
enum SourceGeometry {
    Single(shared::Geometry<1>),
    Split(shared::Geometry<3>),
}

impl SourceGeometry {
    fn new(layout: &Layout) -> Result<Self, FalconError> {
        let logs = compact::source_logs(layout);
        if compact::is_split(layout) {
            Ok(Self::Split(shared::Geometry::new(logs).map_err(error)?))
        } else {
            Ok(Self::Single(
                shared::Geometry::with_allocation_logs([logs[0]], [logs[0].max(12)])
                    .map_err(error)?,
            ))
        }
    }

    fn packed_log(&self) -> usize {
        match self {
            Self::Single(g) => g.packed_log(),
            Self::Split(g) => g.packed_log(),
        }
    }

    fn bit_log(&self) -> usize {
        self.packed_log() + 7
    }

    fn physical_logs(&self) -> &[usize] {
        match self {
            Self::Single(g) => &g.physical_logs,
            Self::Split(g) => &g.physical_logs,
        }
    }
}

enum SourceProverData {
    Single(shared::JointProverData<1>),
    Split(shared::JointProverData<3>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RingMode {
    Canonical,
    #[cfg(any(test, feature = "bench-internals"))]
    Factored,
}

impl RingMode {
    fn projection_degree(self) -> usize {
        match self {
            Self::Canonical => 10,
            #[cfg(any(test, feature = "bench-internals"))]
            Self::Factored => 20,
        }
    }
}

#[derive(Clone)]
enum RingProof {
    Canonical(ring::Proof),
    #[cfg(any(test, feature = "bench-internals"))]
    Factored(super::factored_ring::Proof),
}

// Delegate canonical Debug unchanged: benchmark digests cover the complete v9 proof.
impl std::fmt::Debug for RingProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canonical(proof) => proof.fmt(f),
            #[cfg(any(test, feature = "bench-internals"))]
            Self::Factored(proof) => f.debug_tuple("FactoredRing").field(proof).finish(),
        }
    }
}

impl RingProof {
    fn payload_size_bytes(&self) -> usize {
        match self {
            Self::Canonical(proof) => proof.payload_size_bytes(),
            #[cfg(any(test, feature = "bench-internals"))]
            Self::Factored(proof) => proof.payload_size_bytes(),
        }
    }
}

enum RingClaim {
    Canonical(ring::PreparedClaim),
    #[cfg(any(test, feature = "bench-internals"))]
    Factored(super::factored_ring::PreparedClaim),
}

impl RingClaim {
    #[cfg(any(test, feature = "bench-internals"))]
    fn canonical(&self) -> Result<&ring::PreparedClaim, FalconError> {
        match self {
            Self::Canonical(claim) => Ok(claim),
            Self::Factored(_) => Err(error("compact computation requires canonical ring")),
        }
    }
}

/// Reusable configuration for 1..=1024 algebraic Falcon statements.
pub struct PreparedFalconAlgebraic {
    layout: Layout,
    target_bits: usize,
    geometry: SourceGeometry,
    schedule: Schedule,
    ligerito: ResolvedLigerito,
    pcs_grinding: GrindingPlan,
    grinding: BinaryGrinding,
    compact_binding: bool,
    compute_variant: &'static str,
    ring_mode: RingMode,
    scratch: Mutex<Scratch>,
    coordinate_scratch: ring::CoordinateScratch,
    binding_scratch: arithmetic::BindingScratch,
}

/// The committed witness. Consumed by proving to avoid witness copies.
pub struct CommittedFalconAlgebraic {
    statement: FalconAlgebraicStatement,
    source_root: [u8; 32],
    target_bits: usize,
    witness: WitnessData,
    layout: Layout,
    source: Option<Source>,
    packed: [Vec<Gf>; 3],
    exceptions: Vec<Vec<u16>>,
    data: SourceProverData,
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
#[derive(Clone)]
pub struct FalconAlgebraicProof {
    source_root: [u8; 32],
    exceptions: Vec<Vec<u16>>,
    ring: RingProof,
    arithmetic: ArithmeticProof,
    bridges: Vec<bridge::Proof>,
    bridge_merge_nonces: Vec<u64>,
    joint: joint::Proof,
    opening: shared::JointProof,
    opening_nonces: Vec<u64>,
    pcs_nonces: Vec<u64>,
}

impl std::fmt::Debug for FalconAlgebraicProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut proof = f.debug_struct("FalconAlgebraicProof");
        proof
            .field("source_root", &self.source_root)
            .field("exceptions", &self.exceptions)
            .field("ring", &self.ring);
        // Keep the single-bridge diagnostic shape compact. Arithmetic Debug
        // includes all word terminals and their fresh merge nonce.
        match &self.arithmetic {
            ArithmeticProof::Raw(arithmetic)
                if self.bridges.len() == 1 && self.bridge_merge_nonces.is_empty() =>
            {
                proof
                    .field("arithmetic", arithmetic)
                    .field("bridge", &self.bridges[0]);
            }
            _ => {
                proof
                    .field("arithmetic", &self.arithmetic)
                    .field("bridges", &self.bridges)
                    .field("bridge_merge_nonces", &self.bridge_merge_nonces);
            }
        }
        proof
            .field("joint", &self.joint)
            .field("opening", &self.opening)
            .field("opening_nonces", &self.opening_nonces)
            .field("pcs_nonces", &self.pcs_nonces)
            .finish()
    }
}

#[derive(Clone, Debug)]
enum ArithmeticProof {
    Raw(arithmetic::Proof),
    #[cfg(any(test, feature = "bench-internals"))]
    Compact(arithmetic::CompactProof),
}

impl ArithmeticProof {
    fn payload_size_bytes(&self) -> usize {
        match self {
            Self::Raw(proof) => proof.payload_size_bytes(),
            #[cfg(any(test, feature = "bench-internals"))]
            Self::Compact(proof) => proof.payload_size_bytes(),
        }
    }
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
            (
                "coefficient_exceptions",
                self.exceptions
                    .iter()
                    .map(|indices| 2 + 2 * indices.len())
                    .sum(),
            ),
            ("ring", self.ring.payload_size_bytes()),
            ("norm_and_binding", self.arithmetic.payload_size_bytes()),
            (
                "bitz_bridge",
                self.bridges
                    .iter()
                    .map(|proof| {
                        proof.sums.encoded_len() + 32 * proof.forest.len() + 8 * proof.nonces.len()
                    })
                    .sum::<usize>()
                    + 8 * self.bridge_merge_nonces.len(),
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
    /// Use the word binder for every production batch size and security target.
    pub fn new(batch: usize, target_bits: usize) -> Result<Self, FalconError> {
        // Compact binding has not passed the proving-time gate. Keep it an
        // explicit benchmark experiment; production uses the word binder.
        let compute_variant = "padding";
        #[cfg(feature = "bench-internals")]
        let compute_variant = match std::env::var("BITZ_ARITHMETIC_COMPUTE_VARIANT").as_deref() {
            Ok("padding") => "padding",
            Ok("compact") => "compact",
            Ok("combined") => "combined",
            Err(std::env::VarError::NotPresent) => compute_variant,
            _ => return Err(error("unknown arithmetic computation benchmark variant")),
        };
        #[cfg(feature = "bench-internals")]
        match std::env::var("BITZ_ARITHMETIC_RING").as_deref() {
            Ok("factored") => {
                if compute_variant != "padding" {
                    return Err(error("factored ring requires raw word binding"));
                }
                return Self::new_factored_ring(batch, target_bits);
            }
            Err(std::env::VarError::NotPresent) | Ok("canonical") => {}
            _ => return Err(error("BITZ_ARITHMETIC_RING must be canonical or factored")),
        }
        if compute_variant != "padding" {
            return Self::with_compute_variant(batch, target_bits, compute_variant);
        }
        Self::new_with_binding(batch, target_bits, FalconAlgebraicBinding::Words)
    }

    /// Experimental degree-20 ring lift; never selected by production builds.
    #[cfg(any(test, feature = "bench-internals"))]
    #[doc(hidden)]
    pub fn new_factored_ring(batch: usize, target_bits: usize) -> Result<Self, FalconError> {
        Self::with_ring_configuration(
            batch,
            target_bits,
            "padding",
            FalconAlgebraicBinding::Words,
            RingMode::Factored,
        )
    }

    /// Select the binding relation over the same compact commitment. Both support
    /// partial batches; prover and verifier must select the same mode.
    pub fn new_with_binding(
        batch: usize,
        target_bits: usize,
        binding: FalconAlgebraicBinding,
    ) -> Result<Self, FalconError> {
        Self::with_configuration(batch, target_bits, "padding", binding)
    }

    fn with_compute_variant(
        batch: usize,
        target_bits: usize,
        compute_variant: &'static str,
    ) -> Result<Self, FalconError> {
        Self::with_configuration(
            batch,
            target_bits,
            compute_variant,
            FalconAlgebraicBinding::Bits,
        )
    }

    fn with_configuration(
        batch: usize,
        target_bits: usize,
        compute_variant: &'static str,
        binding: FalconAlgebraicBinding,
    ) -> Result<Self, FalconError> {
        Self::with_ring_configuration(
            batch,
            target_bits,
            compute_variant,
            binding,
            RingMode::Canonical,
        )
    }

    fn with_ring_configuration(
        batch: usize,
        target_bits: usize,
        compute_variant: &'static str,
        binding: FalconAlgebraicBinding,
        ring_mode: RingMode,
    ) -> Result<Self, FalconError> {
        if ring_mode != RingMode::Canonical
            && (binding != FalconAlgebraicBinding::Words || compute_variant != "padding")
        {
            return Err(error("factored ring requires raw word binding"));
        }
        let layout = Layout::new(batch)?;
        #[cfg(any(test, feature = "bench-internals"))]
        let layout = if compute_variant == "compact" {
            layout.padded()
        } else {
            layout
        };
        let compact_binding = cfg!(any(test, feature = "bench-internals"))
            && compact::is_split(&layout)
            && compute_variant != "padding";
        let word_binding = binding == FalconAlgebraicBinding::Words;
        if word_binding && compute_variant != "padding" {
            return Err(error("word binder requires raw computation"));
        }
        if !matches!(target_bits, 100 | 128) {
            return Err(error("algebraic security must be 100 or 128 bits"));
        }
        let d = layout.capacity().ilog2() as usize;
        let (binding_bits, terminal_merge_bits) =
            binding_grinding(&layout, compact_binding, word_binding, target_bits);
        let schedule = Schedule {
            norm_instance_bits: grind(target_bits, d, 125),
            norm_round_bits: grind(target_bits, 3 * (COEFFICIENT_LOG + 1 + d), 125),
            merge_bits: grind(target_bits, 3, 125),
            binding_bits,
            word_binding,
            terminal_merge_bits,
        };
        let geometry = SourceGeometry::new(&layout)?;
        let ligerito = LigeritoSelection::MATCHED_UDR
            .resolve(geometry.packed_log(), target_bits)
            .map_err(error)?;
        let (grinding, pcs_grinding) = allocate_grinding(
            &layout,
            compact_binding,
            &geometry,
            &ligerito,
            &schedule,
            target_bits,
            ring_mode,
        )?;
        for matrix in bridge_matrices(&layout, compact_binding) {
            bridge::validate_layout(&matrix, BridgeMode::TwoLimbs, PRIME_MAX)?;
        }
        let prepared = Self {
            layout,
            target_bits,
            geometry,
            schedule,
            ligerito,
            pcs_grinding,
            grinding,
            compact_binding,
            compute_variant,
            ring_mode,
            scratch: Mutex::default(),
            coordinate_scratch: ring::CoordinateScratch::default(),
            binding_scratch: arithmetic::BindingScratch::default(),
        };
        if prepared.security().algebraic_bits < target_bits as f64 {
            return Err(error("algebraic composition misses security target"));
        }
        Ok(prepared)
    }

    pub fn batch(&self) -> usize {
        self.layout.batch()
    }

    /// Coordinate and word-binding vector capacity cached for subsequent proofs.
    /// Excludes existing sumcheck scratch, allocator retention, and in-flight work.
    #[cfg(feature = "bench-internals")]
    #[doc(hidden)]
    pub fn retained_scratch_bytes(&self) -> usize {
        self.coordinate_scratch.retained_bytes() + self.binding_scratch.retained_bytes()
    }
    pub fn capacity(&self) -> usize {
        self.layout.allocation_capacity()
    }
    /// Power-of-two instance domain actually used by the arithmetic checks.
    pub fn arithmetic_capacity(&self) -> usize {
        self.layout.capacity()
    }
    /// Benchmark computation label. Both production binders use `padding`,
    /// retained from the logical-padding experiment. Use [`Self::protocol_id`]
    /// to identify the selected relation.
    pub fn compute_variant(&self) -> &'static str {
        self.compute_variant
    }
    /// Word computation uses v9; explicit bit computation retains its v7/v8 domain.
    pub fn protocol_id(&self) -> &'static str {
        #[cfg(any(test, feature = "bench-internals"))]
        if self.ring_mode == RingMode::Factored {
            return if N == 1024 {
                "bitz/falcon1024-algebraic/non-zk/v10-factored-ring"
            } else {
                "bitz/falcon512-algebraic/non-zk/v10-factored-ring"
            };
        }
        if self.schedule.word_binding {
            PROTOCOL_ID
        } else if self.uses_legacy_transcript() {
            if N == 1024 {
                "bitz/falcon1024-algebraic/non-zk/v7"
            } else {
                "bitz/falcon512-algebraic/non-zk/v7"
            }
        } else {
            BIT_PROTOCOL_ID
        }
    }

    #[inline]
    fn uses_compact_binding(&self) -> bool {
        cfg!(any(test, feature = "bench-internals")) && self.compact_binding
    }

    fn uses_legacy_transcript(&self) -> bool {
        !self.schedule.word_binding
            && !self.uses_compact_binding()
            && self.layout.capacity() == self.layout.allocation_capacity()
    }
    /// Logical source bits authenticated by the integer bridges. Word binding
    /// folds sixteen lanes per word before its prime-field binding sumcheck.
    pub fn logical_binding_bits(&self) -> usize {
        self.bridge_layouts()
            .iter()
            .map(|m| m.rows() * m.cols())
            .sum()
    }
    /// (row variables, column variables) for every active integer bridge.
    pub fn bridge_layouts(&self) -> Vec<IntegerMatrixLayout> {
        bridge_matrices(&self.layout, self.uses_compact_binding())
    }
    pub fn target_bits(&self) -> usize {
        self.target_bits
    }
    /// Maximum live committed value bits in the selected source representation.
    /// Excludes public exception-index metadata, used only for split sources.
    pub fn live_bits_per_signature(&self) -> usize {
        if compact::is_split(&self.layout) {
            COMPACT_LIVE_BITS
        } else {
            LIVE_BITS
        }
    }
    /// Size of the aligned virtual arithmetic view, before compact encoding.
    pub fn source_bits_per_signature(&self) -> usize {
        self.layout.signature_stride()
    }
    /// Total physical committed bits, including source padding and batch capacity.
    pub fn committed_source_bits(&self) -> usize {
        self.geometry
            .physical_logs()
            .iter()
            .map(|log| 128usize << log)
            .sum()
    }
    pub fn prime_modulus_bounds(&self) -> (u128, u128) {
        (PRIME_MIN, PRIME_MAX)
    }

    pub fn security(&self) -> FalconAlgebraicSecurity {
        let mut terms = fixed_terms(
            &self.layout,
            self.uses_compact_binding(),
            self.target_bits,
            &self.schedule,
            self.ring_mode,
        );
        let binary = |n: usize, bits: u32| n as f64 * 2f64.powi(-128 - bits as i32);
        for (index, matrix) in self.bridge_layouts().iter().enumerate() {
            terms.push((
                [
                    "BitZ product GKR (low)",
                    "BitZ product GKR (high)",
                    "BitZ product GKR (tail)",
                ][index],
                binary(
                    bridge::error_numerator(matrix, BridgeMode::TwoLimbs),
                    self.grinding.bridge[index],
                ),
            ));
        }
        if self.uses_compact_binding() {
            terms.push((
                "binary bridge claim merge",
                binary(2, self.grinding.bridge_merge),
            ));
        }
        terms.extend([
            (
                "binary source sumcheck",
                binary(2 * self.geometry.bit_log(), self.grinding.joint),
            ),
            ("ring switch", binary(256, self.grinding.opening)),
            (
                "Ligerito",
                self.pcs_grinding
                    .blocks
                    .iter()
                    .map(|block| block.raw_error * 2f64.powi(-(block.bits as i32)))
                    .sum(),
            ),
        ]);
        FalconAlgebraicSecurity {
            target_bits: self.target_bits,
            algebraic_bits: -terms.iter().map(|(_, p)| p).sum::<f64>().log2(),
            terms,
        }
    }

    fn carry_grinding(&self) -> u32 {
        carry_grinding(self.target_bits, self.ring_mode)
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
        if self.uses_compact_binding() {
            let packed = compact::CompactSource::from_data(&self.layout, &data)?;
            return self.commit_packed(statement, data, None, packed);
        }
        #[cfg(feature = "bench-internals")]
        let pack = super::packing_bench::constructor()?;
        #[cfg(not(feature = "bench-internals"))]
        let pack = Source::new;
        let source = pack(self.layout, &data);
        self.commit_source(statement, data, source)
    }

    fn commit_source(
        &self,
        statement: FalconAlgebraicStatement,
        witness: WitnessData,
        source: Source,
    ) -> Result<CommittedFalconAlgebraic, FalconError> {
        let compact::CompactSource { packed, exceptions } =
            compact::CompactSource::new(&self.layout, &source, &witness)?;
        self.commit_packed(
            statement,
            witness,
            Some(source),
            compact::CompactSource { packed, exceptions },
        )
    }

    fn commit_packed(
        &self,
        statement: FalconAlgebraicStatement,
        witness: WitnessData,
        source: Option<Source>,
        compact::CompactSource { packed, exceptions }: compact::CompactSource,
    ) -> Result<CommittedFalconAlgebraic, FalconError> {
        let rate = self.ligerito.prover().log_inv_rates[0];
        let (source_root, data) = match &self.geometry {
            SourceGeometry::Single(g) => {
                let (root, data) =
                    shared::commit_sources(g, [packed[0].as_slice()], rate).map_err(error)?;
                (root, SourceProverData::Single(data))
            }
            SourceGeometry::Split(g) => {
                let (root, data) =
                    shared::commit_sources(g, packed.each_ref().map(Vec::as_slice), rate)
                        .map_err(error)?;
                (root, SourceProverData::Split(data))
            }
        };
        Ok(CommittedFalconAlgebraic {
            statement,
            source_root,
            target_bits: self.target_bits,
            witness,
            layout: self.layout,
            source,
            packed,
            exceptions,
            data,
        })
    }

    fn transcript(
        &self,
        public: &FalconAlgebraicStatement,
        root: &[u8; 32],
        exceptions: &[Vec<u16>],
    ) -> Result<(Blake3Transcript, [u8; 32]), FalconError> {
        public.validate(self.batch())?;
        compact::validate_exceptions(&self.layout, exceptions)?;
        let mut h = blake3::Hasher::new();
        h.update(self.protocol_id().as_bytes());
        h.update(b"ring-coordinate-basis:alpha-powers/canonical/v1");
        if compact::is_split(&self.layout) {
            h.update(b"signed12-with-sparse-signed15-exceptions;aligned-small/split-large;virtual-lanes16;native11;two-limbs/v4");
        } else {
            h.update(b"signed15-single;virtual-lanes16;native11;two-limbs/v5");
        }
        for value in [
            N,
            Q as usize,
            BETA_SQUARED as usize,
            self.batch(),
            self.capacity(),
            self.target_bits,
            self.layout.signature_stride(),
            self.live_bits_per_signature(),
            compact::BASE_BITS,
            compact::MAX_EXCEPTIONS,
            compact::TAIL_STRIDE,
            SLACK_BITS,
            usize::from(compact::is_split(&self.layout)),
            self.layout.row_vars(),
            self.geometry.packed_log(),
        ] {
            h.update(&(value as u64).to_le_bytes());
        }
        for log in compact::source_logs(&self.layout) {
            h.update(&(log as u64).to_le_bytes());
        }
        for &log in self.geometry.physical_logs() {
            h.update(&(log as u64).to_le_bytes());
        }
        if !self.uses_legacy_transcript() {
            h.update(&(self.arithmetic_capacity() as u64).to_le_bytes());
            h.update(&(u64::from(self.uses_compact_binding())).to_le_bytes());
            for matrix in self.bridge_layouts() {
                h.update(&(matrix.row_vars as u64).to_le_bytes());
                h.update(&(matrix.col_vars as u64).to_le_bytes());
            }
        }
        if self.schedule.word_binding {
            h.update(
                b"word-binder:signed15/lane15/lower-popcount;public-masks;prime-terminal-merge/v9",
            );
            let tables = 2 + usize::from(self.batch() < self.arithmetic_capacity());
            h.update(&(tables as u64).to_le_bytes());
        }
        for indices in exceptions {
            h.update(&(indices.len() as u16).to_le_bytes());
            for index in indices {
                h.update(&index.to_le_bytes());
            }
        }
        h.update(&PRIME_MIN.to_le_bytes());
        h.update(&PRIME_MAX.to_le_bytes());
        for bits in [
            self.schedule.norm_instance_bits,
            self.schedule.norm_round_bits,
            self.schedule.merge_bits,
            self.schedule.binding_bits,
            self.carry_grinding(),
            self.grinding.bridge[0],
            self.grinding.joint,
            self.grinding.opening,
        ] {
            h.update(&bits.to_le_bytes());
        }
        if !self.uses_legacy_transcript() {
            for bits in [
                self.grinding.bridge[1],
                self.grinding.bridge[2],
                self.grinding.bridge_merge,
            ] {
                h.update(&bits.to_le_bytes());
            }
        }
        if self.schedule.word_binding {
            h.update(&self.schedule.terminal_merge_bits.to_le_bytes());
        }
        h.update(b"grinding:work-optimal-binary-and-pcs-allocation;budget=15/16/v1");
        let mut coefficients = Vec::with_capacity(4 * N * public.public_keys.len());
        for (key, target) in public.public_keys.iter().zip(&public.targets) {
            for coefficient in key.iter().chain(target) {
                coefficients.extend_from_slice(&coefficient.to_le_bytes());
            }
        }
        crate::piop::spartan::falcon::hash_statement_bytes(&mut h, &coefficients);
        h.update(root);
        h.update(&self.ligerito.digest());
        for block in &self.pcs_grinding.blocks {
            h.update(&block.bits.to_le_bytes());
        }
        let digest = *h.finalize().as_bytes();
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(self.protocol_id().as_bytes());
        transcript.absorb_slice(&digest);
        self.ligerito.bind(&mut transcript);
        Ok((transcript, digest))
    }

    #[tracing::instrument(skip_all, name = "falcon_algebraic:prove")]
    pub fn prove(
        &self,
        committed: CommittedFalconAlgebraic,
    ) -> Result<FalconAlgebraicProof, FalconError> {
        self.prove_observed(committed, |_| {})
    }

    fn prove_observed(
        &self,
        committed: CommittedFalconAlgebraic,
        observe: impl FnOnce(&mut Blake3Transcript),
    ) -> Result<FalconAlgebraicProof, FalconError> {
        if committed.layout != self.layout || committed.target_bits != self.target_bits {
            return Err(error("algebraic commitment configuration mismatch"));
        }
        let (mut t, digest) = self.transcript(
            &committed.statement,
            &committed.source_root,
            &committed.exceptions,
        )?;
        let field = sample_field(&mut t)?;
        let (ring, ring_claim) = {
            let _span = tracing::info_span!("falcon_algebraic:ring").entered();
            match self.ring_mode {
                RingMode::Canonical => {
                    let (proof, claim) = ring::prove_with_scratch(
                        &mut t,
                        &self.layout,
                        &committed.statement,
                        &committed.witness,
                        &field,
                        self.carry_grinding(),
                        &self.coordinate_scratch,
                    )?;
                    (RingProof::Canonical(proof), RingClaim::Canonical(claim))
                }
                #[cfg(any(test, feature = "bench-internals"))]
                RingMode::Factored => {
                    let (proof, claim) = super::factored_ring::prove_with_scratch(
                        &mut t,
                        &self.layout,
                        &committed.statement,
                        &committed.witness,
                        &field,
                        self.carry_grinding(),
                        &self.coordinate_scratch,
                    )?;
                    (RingProof::Factored(proof), RingClaim::Factored(claim))
                }
            }
        };
        let (arithmetic, bridges, coefficients, target, bridge_merge_nonces) =
            if self.uses_compact_binding() {
                #[cfg(not(any(test, feature = "bench-internals")))]
                unreachable!("compact binding requires bench-internals");
                #[cfg(any(test, feature = "bench-internals"))]
                {
                    let matrices = compact::bridge_layouts(&self.layout);
                    let (proof, claims) = {
                        let _span = tracing::info_span!("falcon_algebraic:arithmetic").entered();
                        arithmetic::prove_compact(
                            &mut t,
                            &self.layout,
                            &committed.witness,
                            committed.packed.each_ref().map(Vec::as_slice),
                            &committed.exceptions,
                            &matrices,
                            &field,
                            ring_claim.canonical()?,
                            &self.schedule,
                        )?
                    };
                    drop(ring_claim);
                    drop(committed.witness);
                    let span = tracing::info_span!("falcon_algebraic:bitz").entered();
                    let mut bridges = Vec::with_capacity(3);
                    let mut binary_claims = Vec::with_capacity(3);
                    for (index, (matrix, claim)) in matrices.iter().zip(&claims).enumerate() {
                        bind_bridge(&mut t, index);
                        let rows = bridge_rows(&committed.packed[index], matrix)?;
                        let (proof, binary) = bridge::prove_weighted(
                            &mut t,
                            matrix,
                            &rows,
                            BridgeMode::TwoLimbs,
                            &claim.column_point,
                            claim.modulus,
                            self.grinding.bridge[index],
                            &claim.row_weights,
                        )?;
                        bridges.push(proof);
                        binary_claims.push(binary);
                    }
                    drop(span);
                    bind_binary_claims(&mut t, &binary_claims);
                    let mut merge = ProverBlockGrindingTranscript::<_, BridgeMergeGrinding>::new(
                        &mut t,
                        self.grinding.bridge_merge,
                    );
                    let zeta: Gf = merge.get_field_challenge(&());
                    let nonces = merge.finish();
                    let (coefficients, target) = combine_binary_claims(binary_claims, zeta)?;
                    (
                        ArithmeticProof::Compact(proof),
                        bridges,
                        coefficients,
                        target,
                        nonces,
                    )
                }
            } else {
                let source = committed
                    .source
                    .as_ref()
                    .ok_or_else(|| error("missing raw arithmetic source"))?;
                let (proof, claim) = {
                    let _span = tracing::info_span!("falcon_algebraic:arithmetic").entered();
                    match ring_claim {
                        RingClaim::Canonical(claim) => arithmetic::prove_owned_with_scratch(
                            &mut t,
                            &self.layout,
                            &committed.witness,
                            source,
                            &field,
                            claim,
                            &self.schedule,
                            &self.binding_scratch,
                        )?,
                        #[cfg(any(test, feature = "bench-internals"))]
                        RingClaim::Factored(claim) => arithmetic::prove_factored_ring_with_scratch(
                            &mut t,
                            &self.layout,
                            &committed.witness,
                            source,
                            &field,
                            claim,
                            &self.schedule,
                            &self.binding_scratch,
                        )?,
                    }
                };
                drop(committed.witness);
                let (bridge, binary) = {
                    let _span = tracing::info_span!("falcon_algebraic:bitz").entered();
                    bridge::prove_weighted(
                        &mut t,
                        &self.layout.bitz_params(),
                        source.rows(),
                        BridgeMode::TwoLimbs,
                        &claim.column_point,
                        claim.modulus,
                        self.grinding.bridge[0],
                        &claim.row_weights,
                    )?
                };
                drop(claim);
                let target = binary.value;
                (
                    ArithmeticProof::Raw(proof),
                    vec![bridge],
                    compact::coefficients(&self.layout, &committed.exceptions, binary)?,
                    target,
                    Vec::new(),
                )
            };
        drop(committed.source);
        let mut scratch = self
            .scratch
            .lock()
            .map_err(|_| error("algebraic scratch lock"))?;
        let (joint, point) = match &self.geometry {
            SourceGeometry::Single(g) => joint::prove(
                &mut t,
                g,
                [committed.packed[0].as_slice()],
                [&coefficients[0]],
                target,
                self.grinding.joint,
                &mut scratch,
            ),
            SourceGeometry::Split(g) => joint::prove(
                &mut t,
                g,
                committed.packed.each_ref().map(Vec::as_slice),
                coefficients.each_ref(),
                target,
                self.grinding.joint,
                &mut scratch,
            ),
        }
        .map_err(error)?;
        drop(scratch);
        let mut pcs_nonces = Vec::new();
        let _opening_span = tracing::info_span!("falcon_algebraic:pcs").entered();
        let mut pcs = GrindingContext {
            plan: &self.pcs_grinding,
            nonces: GrindingNonces::Prove(&mut pcs_nonces),
        };
        let mut opening_t =
            ProverBlockGrindingTranscript::<_, OpeningGrinding>::new(&mut t, self.grinding.opening);
        let opening = match (&self.geometry, &committed.data) {
            (SourceGeometry::Single(g), SourceProverData::Single(data)) => {
                shared::prove_joint_sources_with_security(
                    &mut opening_t,
                    g,
                    &digest,
                    [committed.packed[0].as_slice()],
                    &self.ligerito,
                    data,
                    &point,
                    Some(&mut pcs),
                )
            }
            (SourceGeometry::Split(g), SourceProverData::Split(data)) => {
                shared::prove_joint_sources_with_security(
                    &mut opening_t,
                    g,
                    &digest,
                    committed.packed.each_ref().map(Vec::as_slice),
                    &self.ligerito,
                    data,
                    &point,
                    Some(&mut pcs),
                )
            }
            _ => return Err(error("algebraic source geometry mismatch")),
        }
        .map_err(error)?;
        let opening_nonces = opening_t.finish();
        observe(&mut t);
        Ok(FalconAlgebraicProof {
            source_root: committed.source_root,
            exceptions: committed.exceptions,
            ring,
            arithmetic,
            bridges,
            bridge_merge_nonces,
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
        self.verify_observed(public, proof, |_| {})
    }

    fn verify_observed(
        &self,
        public: &FalconAlgebraicStatement,
        proof: &FalconAlgebraicProof,
        observe: impl FnOnce(&mut Blake3Transcript),
    ) -> Result<(), FalconError> {
        let (mut t, digest) = self.transcript(public, &proof.source_root, &proof.exceptions)?;
        let field = sample_field(&mut t)?;
        let ring_claim = match (&proof.ring, self.ring_mode) {
            (RingProof::Canonical(proof), RingMode::Canonical) => {
                RingClaim::Canonical(ring::verify(
                    &mut t,
                    &self.layout,
                    public,
                    proof,
                    &field,
                    self.carry_grinding(),
                )?)
            }
            #[cfg(any(test, feature = "bench-internals"))]
            (RingProof::Factored(proof), RingMode::Factored) => {
                RingClaim::Factored(super::factored_ring::verify(
                    &mut t,
                    &self.layout,
                    public,
                    proof,
                    &field,
                    self.carry_grinding(),
                )?)
            }
            #[cfg(any(test, feature = "bench-internals"))]
            _ => return Err(error("ring proof version mismatch")),
        };
        let (coefficients, target) = match (&proof.arithmetic, self.uses_compact_binding()) {
            #[cfg(any(test, feature = "bench-internals"))]
            (ArithmeticProof::Compact(arithmetic), true) => {
                if proof.bridges.len() != 3 {
                    return Err(error("compact bridge count"));
                }
                let matrices = compact::bridge_layouts(&self.layout);
                let claims = arithmetic::verify_compact(
                    &mut t,
                    &self.layout,
                    &proof.exceptions,
                    &matrices,
                    &field,
                    ring_claim.canonical()?,
                    arithmetic,
                    &self.schedule,
                )?;
                drop(ring_claim);
                let mut binary_claims = Vec::with_capacity(3);
                for (index, ((matrix, claim), bridge)) in
                    matrices.iter().zip(&claims).zip(&proof.bridges).enumerate()
                {
                    bind_bridge(&mut t, index);
                    binary_claims.push(bridge::verify_weighted(
                        &mut t,
                        matrix,
                        BridgeMode::TwoLimbs,
                        &claim.column_point,
                        claim.value,
                        claim.modulus,
                        bridge,
                        self.grinding.bridge[index],
                        &claim.row_weights,
                    )?);
                }
                bind_binary_claims(&mut t, &binary_claims);
                let mut merge = VerifierBlockGrindingTranscript::<_, BridgeMergeGrinding>::new(
                    &mut t,
                    self.grinding.bridge_merge,
                    &proof.bridge_merge_nonces,
                );
                let zeta: Gf = merge.get_field_challenge(&());
                merge.finish().map_err(error)?;
                combine_binary_claims(binary_claims, zeta)?
            }
            (ArithmeticProof::Raw(arithmetic), false) => {
                if proof.bridges.len() != 1 || !proof.bridge_merge_nonces.is_empty() {
                    return Err(error("raw bridge shape"));
                }
                let claim = match &ring_claim {
                    RingClaim::Canonical(claim) => arithmetic::verify(
                        &mut t,
                        &self.layout,
                        &field,
                        claim,
                        arithmetic,
                        &self.schedule,
                    )?,
                    #[cfg(any(test, feature = "bench-internals"))]
                    RingClaim::Factored(claim) => arithmetic::verify_factored_ring(
                        &mut t,
                        &self.layout,
                        &field,
                        claim,
                        arithmetic,
                        &self.schedule,
                    )?,
                };
                drop(ring_claim);
                let binary = bridge::verify_weighted(
                    &mut t,
                    &self.layout.bitz_params(),
                    BridgeMode::TwoLimbs,
                    &claim.column_point,
                    claim.value,
                    claim.modulus,
                    &proof.bridges[0],
                    self.grinding.bridge[0],
                    &claim.row_weights,
                )?;
                drop(claim);
                let target = binary.value;
                (
                    compact::coefficients(&self.layout, &proof.exceptions, binary)?,
                    target,
                )
            }
            _ => return Err(error("arithmetic binding mode mismatch")),
        };
        let point = match &self.geometry {
            SourceGeometry::Single(g) => joint::verify(
                &mut t,
                g,
                [&coefficients[0]],
                target,
                self.grinding.joint,
                &proof.joint,
            ),
            SourceGeometry::Split(g) => joint::verify(
                &mut t,
                g,
                coefficients.each_ref(),
                target,
                self.grinding.joint,
                &proof.joint,
            ),
        }
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
            self.grinding.opening,
            &proof.opening_nonces,
        );
        match &self.geometry {
            SourceGeometry::Single(g) => shared::verify_joint_with_security(
                &mut opening_t,
                g,
                &digest,
                &proof.source_root,
                &point,
                proof.joint.value,
                &self.ligerito,
                &proof.opening,
                Some(&mut pcs),
            ),
            SourceGeometry::Split(g) => shared::verify_joint_with_security(
                &mut opening_t,
                g,
                &digest,
                &proof.source_root,
                &point,
                proof.joint.value,
                &self.ligerito,
                &proof.opening,
                Some(&mut pcs),
            ),
        }
        .map_err(error)?;
        opening_t.finish().map_err(error)?;
        observe(&mut t);
        Ok(())
    }
}

fn bridge_matrices(layout: &Layout, compact_binding: bool) -> Vec<IntegerMatrixLayout> {
    if compact_binding {
        compact::bridge_layouts(layout).to_vec()
    } else {
        vec![layout.bitz_params()]
    }
}

fn binding_numerator(layout: &Layout, compact_binding: bool, word_binding: bool) -> usize {
    if compact_binding {
        2 * bridge_matrices(layout, true)
            .iter()
            .map(|m| m.row_vars + m.col_vars)
            .sum::<usize>()
    } else {
        2 * binding_rounds(layout, word_binding)
    }
}

fn terminal_merge_numerator(layout: &Layout, word_binding: bool) -> usize {
    if word_binding {
        1 + usize::from(layout.batch() < layout.capacity())
    } else {
        0
    }
}

fn binding_grinding(
    layout: &Layout,
    compact_binding: bool,
    word_binding: bool,
    target_bits: usize,
) -> (u32, u32) {
    let numerator = binding_numerator(layout, compact_binding, word_binding);
    if !word_binding {
        return (grind(target_bits, numerator, 125), 0);
    }
    // Keep the old bit-domain difficulty and fund the terminal merge from
    // the four removed degree-two rounds: n/2^g + d/2^h <= (n+8)/2^g.
    let binding_bits = grind(target_bits, numerator + 8, 125);
    let degree = terminal_merge_numerator(layout, true);
    let terminal_merge_bits =
        grind(target_bits, degree, 125).max((binding_bits + degree.ilog2()).saturating_sub(3));
    (binding_bits, terminal_merge_bits)
}

#[cfg(any(test, feature = "bench-internals"))]
fn bind_bridge(t: &mut impl Transcript, index: usize) {
    t.absorb_slice(b"falcon-algebraic/compact-bridge/v8");
    t.absorb_slice(&(index as u64).to_le_bytes());
}

#[cfg(any(test, feature = "bench-internals"))]
fn bridge_rows(packed: &[Gf], matrix: &IntegerMatrixLayout) -> Result<Vec<Vec<u64>>, FalconError> {
    let words = matrix.rows() / 128;
    let logical = words * matrix.cols();
    if packed.len() < logical {
        return Err(error("compact bridge source length"));
    }
    Ok(packed[..logical]
        .chunks_exact(words)
        .map(|column| column.iter().flat_map(|word| [word.lo, word.hi]).collect())
        .collect())
}

#[cfg(any(test, feature = "bench-internals"))]
fn bind_binary_claims(t: &mut impl Transcript, claims: &[crate::hybrid::BinaryClaim]) {
    t.absorb_slice(b"falcon-algebraic/compact-bridge-claims/v8");
    // Every row weight is already determined by the prime opening and the
    // absorbed bridge messages/challenges. Bind the short terminal claims;
    // reabsorbing thousands of deterministic row weights adds no obligation.
    for claim in claims {
        t.absorb_slice(&claim.value.to_bytes());
        for value in &claim.high_point {
            t.absorb_slice(&value.to_bytes());
        }
    }
}

#[cfg(any(test, feature = "bench-internals"))]
fn combine_binary_claims(
    claims: Vec<crate::hybrid::BinaryClaim>,
    zeta: Gf,
) -> Result<([joint::Coefficients; 3], Gf), FalconError> {
    if claims.len() != 3 {
        return Err(error("compact binary claim count"));
    }
    let mut coefficients = std::array::from_fn(|_| joint::Coefficients::default());
    let mut target = Gf::ZERO;
    for (index, (mut claim, weight)) in claims
        .into_iter()
        .zip([Gf::ONE, zeta, zeta * zeta])
        .enumerate()
    {
        target += claim.value * weight;
        for value in &mut claim.low {
            *value *= weight;
        }
        coefficients[index].tensors.push(joint::Tensor {
            low: claim.low,
            high_point: claim.high_point,
            marginals: None,
        });
    }
    Ok((coefficients, target))
}

// Each prime-field group receives at most 2^(-target-4) error. Within a
// group every adaptive challenge boundary is ground at this difficulty, and
// `numerator` already sums its polynomial degrees over all rounds.
fn grind(target: usize, numerator: usize, domain_bits: u32) -> u32 {
    if numerator == 0 {
        return 0;
    }
    (target as u32 + 4 + numerator.next_power_of_two().ilog2()).saturating_sub(domain_bits)
}

fn carry_grinding(target_bits: usize, ring_mode: RingMode) -> u32 {
    grind(target_bits, ring_mode.projection_degree(), 125)
}

fn binding_rounds(layout: &Layout, word_binding: bool) -> usize {
    COEFFICIENT_LOG + 5 + layout.capacity().ilog2() as usize - 4 * usize::from(word_binding)
}

/// Ledger terms whose difficulties the prime-field schedule fixes.
fn fixed_terms(
    layout: &Layout,
    compact_binding: bool,
    target_bits: usize,
    schedule: &Schedule,
    ring_mode: RingMode,
) -> Vec<(&'static str, f64)> {
    let d = layout.capacity().ilog2() as usize;
    let prime = |n: usize, bits: u32| n as f64 * 2f64.powi(-125 - bits as i32);
    let mut terms = vec![
        ("prime sampling", 2f64.powi(-144)),
        // |E \ F_q| >= 2^149; includes instance batching and degree-(N-1) negacyclic residual.
        ("native ring identity", (N - 1 + d) as f64 * 2f64.powi(-149)),
        (
            "ring coordinate projection",
            prime(
                ring_mode.projection_degree(),
                carry_grinding(target_bits, ring_mode),
            ),
        ),
        (
            "per-signature norm identity",
            prime(d, schedule.norm_instance_bits),
        ),
        (
            "integer outer sumcheck",
            prime(3 * (COEFFICIENT_LOG + 1 + d), schedule.norm_round_bits),
        ),
        ("claim merge", prime(3, schedule.merge_bits)),
        (
            "source binding sumcheck",
            prime(
                binding_numerator(layout, compact_binding, schedule.word_binding),
                schedule.binding_bits,
            ),
        ),
    ];
    if schedule.word_binding {
        terms.push((
            "word terminal claim merge",
            prime(
                terminal_merge_numerator(layout, schedule.word_binding),
                schedule.terminal_merge_bits,
            ),
        ));
    }
    terms
}

/// Work-optimal difficulties for the binary groups and every Ligerito block,
/// under the budget left by the fixed terms (see [`ALLOCATION_BUDGET`]).
fn allocate_grinding(
    layout: &Layout,
    compact_binding: bool,
    geometry: &SourceGeometry,
    ligerito: &ResolvedLigerito,
    schedule: &Schedule,
    target_bits: usize,
    ring_mode: RingMode,
) -> Result<(BinaryGrinding, GrindingPlan), FalconError> {
    use crate::hybrid::grinding_allocation::{GrindingGroup, allocate};
    // Block structure, raw errors and native Flock work do not depend on the
    // plan's target; its difficulties are replaced below.
    let mut pcs = GrindingPlan::resolve(ligerito.security(), target_bits as u32).map_err(error)?;
    let gf = |numerator: usize| numerator as f64 * 2f64.powi(-128);
    let bit_log = geometry.bit_log();
    let matrices = bridge_matrices(layout, compact_binding);
    let bridge_count = matrices.len();
    let mut groups: Vec<_> = matrices
        .iter()
        .map(|params| GrindingGroup {
            raw_error: gf(bridge::error_numerator(params, BridgeMode::TwoLimbs)),
            sites: bridge::message_count(params) as f64,
            min_bits: 0,
        })
        .collect();
    if compact_binding {
        groups.push(GrindingGroup {
            raw_error: gf(2),
            sites: 1.0,
            min_bits: 0,
        });
    }
    let joint_index = groups.len();
    groups.push(GrindingGroup {
        raw_error: gf(2 * bit_log),
        sites: bit_log as f64,
        min_bits: 0,
    });
    groups.push(GrindingGroup {
        raw_error: gf(256),
        sites: (geometry.packed_log() + 8) as f64,
        min_bits: 0,
    });
    let pcs_start = groups.len();
    groups.extend(pcs.blocks.iter().map(|block| GrindingGroup {
        raw_error: block.raw_error,
        sites: 1.0,
        min_bits: block.native_bits.unwrap_or(0),
    }));
    let fixed: f64 = fixed_terms(layout, compact_binding, target_bits, schedule, ring_mode)
        .iter()
        .map(|(_, error)| error)
        .sum();
    let budget = ALLOCATION_BUDGET * 2f64.powi(-(target_bits as i32)) - fixed;
    let bits = allocate(&groups, budget).map_err(error)?;
    for (block, &b) in pcs.blocks.iter_mut().zip(&bits[pcs_start..]) {
        block.bits = b;
    }
    Ok((
        BinaryGrinding {
            bridge: std::array::from_fn(|index| if index < bridge_count { bits[index] } else { 0 }),
            bridge_merge: if compact_binding {
                bits[bridge_count]
            } else {
                0
            },
            joint: bits[joint_index],
            opening: bits[joint_index + 1],
        },
        pcs,
    ))
}

#[tracing::instrument(skip_all, name = "falcon_algebraic:prime")]
fn sample_field(t: &mut impl Transcript) -> Result<Cfg, FalconError> {
    crate::prime_sampling::sample_prime_context(t, PRIME_MIN, PRIME_MAX, 144).map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use field::RingOps;
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
    fn factored_ring_security_version_and_lift_are_bound() {
        for target in [100, 128] {
            for batch in [1, 3, 16, 17, 33, 65, 1024] {
                let canonical = PreparedFalconAlgebraic::new_with_binding(
                    batch,
                    target,
                    FalconAlgebraicBinding::Words,
                )
                .unwrap();
                let factored = PreparedFalconAlgebraic::new_factored_ring(batch, target).unwrap();
                assert_ne!(canonical.protocol_id(), factored.protocol_id());
                assert_eq!(
                    factored.carry_grinding(),
                    if target == 128 { 12 } else { 0 }
                );
                assert!(
                    factored.security().algebraic_bits
                        >= target as f64 + (16f64 / 15.0).log2() - 1e-9
                );
                let projection = factored
                    .security()
                    .terms
                    .into_iter()
                    .find(|(name, _)| *name == "ring coordinate projection")
                    .unwrap()
                    .1;
                assert_eq!(
                    projection,
                    20.0 * 2f64.powi(-125 - factored.carry_grinding() as i32)
                );
                let (public, _) = fixture(batch);
                let exceptions = if compact::is_split(&factored.layout) {
                    vec![Vec::new(); batch]
                } else {
                    Vec::new()
                };
                let (_, a) = canonical
                    .transcript(&public, &[0; 32], &exceptions)
                    .unwrap();
                let (_, b) = factored.transcript(&public, &[0; 32], &exceptions).unwrap();
                assert_ne!(a, b);
            }
        }
        assert!(
            PreparedFalconAlgebraic::with_ring_configuration(
                1,
                128,
                "padding",
                FalconAlgebraicBinding::Bits,
                RingMode::Factored
            )
            .is_err()
        );
    }

    #[test]
    fn factored_ring_end_to_end_and_cross_version_rejection() {
        for target in [100, 128] {
            for batch in [1, 3, 33] {
                let prepared = PreparedFalconAlgebraic::new_factored_ring(batch, target).unwrap();
                let old = PreparedFalconAlgebraic::new_with_binding(
                    batch,
                    target,
                    FalconAlgebraicBinding::Words,
                )
                .unwrap();
                let (public, witness) = fixture(batch);
                let committed = prepared.commit(public.clone(), witness.clone()).unwrap();
                let proof = prepared.prove(committed).unwrap();
                prepared.verify(&public, &proof).unwrap();
                assert!(old.verify(&public, &proof).is_err());
                let old_proof = old
                    .prove(old.commit(public.clone(), witness).unwrap())
                    .unwrap();
                old.verify(&public, &old_proof).unwrap();
                assert!(prepared.verify(&public, &old_proof).is_err());
                assert!(proof.payload_size_bytes() * 10 <= old_proof.payload_size_bytes() * 11);
                assert_eq!(
                    proof.ring.payload_size_bytes(),
                    old_proof.ring.payload_size_bytes() + 248
                );
                // Adding q preserves the native read-off. The prime-field
                // binding and the transcript must still reject the changed lift.
                let mut altered = proof.clone();
                let RingProof::Factored(ring) = &mut altered.ring else {
                    unreachable!()
                };
                ring.lift[0] += i128::from(Q);
                assert!(prepared.verify(&public, &altered).is_err());
                let mut malformed = proof.clone();
                let RingProof::Factored(ring) = &mut malformed.ring else {
                    unreachable!()
                };
                ring.lift[0] = i128::MIN;
                assert!(prepared.verify(&public, &malformed).is_err());
                let mut altered = public.clone();
                altered.targets[0][0] = (altered.targets[0][0] + 1) % Q as u16;
                assert!(prepared.verify(&altered, &proof).is_err());
            }
        }
    }

    #[test]
    fn configuration_bounds_cover_all_supported_batches() {
        let q = BigUint::from(Q as u64);
        assert!(q.pow(11) - &q >= (BigUint::from(1u8) << 149usize));
        for target in [100, 128] {
            for batch in 1..=1024 {
                // Resolve the PCS at power-of-two batch boundaries.
                let layout = Layout::new(batch).unwrap();
                assert_eq!(layout.capacity(), batch.next_power_of_two());
                let d = layout.capacity().ilog2() as usize;
                for numerator in [
                    d,
                    3 * (COEFFICIENT_LOG + 1 + d),
                    3,
                    2 * (COEFFICIENT_LOG + 1 + d),
                    terminal_merge_numerator(&layout, true),
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
                    assert!(
                        p.security().algebraic_bits >= target as f64 + (16f64 / 15.0).log2() - 1e-9
                    );
                    for block in &p.pcs_grinding.blocks {
                        assert!(block.bits >= block.native_bits.unwrap_or(0));
                    }
                    assert_eq!(
                        p.live_bits_per_signature(),
                        if compact::is_split(&layout) {
                            24 * N + 3 * compact::MAX_EXCEPTIONS + SLACK_BITS
                        } else {
                            30 * N + SLACK_BITS
                        }
                    );
                    assert_eq!(p.source_bits_per_signature(), 32 * N);
                    let old_bits = 32 * N * p.capacity();
                    let expected = if compact::is_split(&layout) {
                        old_bits * 13 / 16
                    } else {
                        old_bits
                    };
                    assert_eq!(p.committed_source_bits(), expected);
                }
            }
        }
        for batch in [0, 1025, usize::MAX] {
            assert!(PreparedFalconAlgebraic::new(batch, 100).is_err());
        }
        assert!(PreparedFalconAlgebraic::new(1, 127).is_err());
    }

    #[test]
    fn word_binding_schedule_preserves_the_removed_lane_round_budget() {
        for target in [100, 128] {
            for batch in 1..=1024 {
                let layout = Layout::new(batch).unwrap();
                let word_log = COEFFICIENT_LOG + 1 + layout.capacity().ilog2() as usize;
                let old_bit_log = layout.bitz_params().row_vars + layout.bitz_params().col_vars;
                let degree: usize = if batch.is_power_of_two() { 1 } else { 2 };
                let (binding_bits, terminal_bits) = binding_grinding(&layout, false, true, target);
                assert_eq!(binding_bits, grind(target, 2 * old_bit_log, 125));
                assert!(terminal_bits >= grind(target, degree, 125));
                assert!(terminal_bits >= (binding_bits + degree.ilog2()).saturating_sub(3));

                // Exact common-denominator comparison, independent of the
                // floating-point composed ledger and PCS configuration.
                let scale = binding_bits.max(terminal_bits);
                let combined = (BigUint::from(2 * word_log) << (scale - binding_bits))
                    + (BigUint::from(degree) << (scale - terminal_bits));
                let old = BigUint::from(2 * old_bit_log) << (scale - binding_bits);
                assert!(combined <= old, "target={target}, batch={batch}");
                assert!(
                    (combined << (target + 4)) <= (BigUint::from(1u8) << (125 + scale as usize))
                );

                assert_eq!(
                    binding_grinding(&layout, true, false, target),
                    (
                        grind(target, binding_numerator(&layout, true, false), 125),
                        0
                    ),
                    "compact computation must retain its grinding schedule"
                );
            }
        }
    }

    #[test]
    fn word_binding_schedule_and_security_are_bound() {
        for target in [100, 128] {
            for batch in [1, 3, 16, 17, 65, 1024] {
                let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
                assert_eq!(prepared.protocol_id(), PROTOCOL_ID);
                let word_log =
                    COEFFICIENT_LOG + 1 + prepared.arithmetic_capacity().ilog2() as usize;
                assert_eq!(
                    binding_numerator(&prepared.layout, false, true),
                    2 * word_log
                );
                assert_eq!(
                    prepared.schedule.binding_bits,
                    grind(target, 2 * word_log + 8, 125)
                );
                let merge_degree = 1 + usize::from(batch < prepared.arithmetic_capacity());
                assert_eq!(
                    terminal_merge_numerator(&prepared.layout, true),
                    merge_degree
                );
                let security = prepared.security();
                let merge_error = security
                    .terms
                    .iter()
                    .find(|(name, _)| *name == "word terminal claim merge")
                    .unwrap()
                    .1;
                assert_eq!(
                    merge_error,
                    merge_degree as f64
                        * 2f64.powi(-125 - prepared.schedule.terminal_merge_bits as i32)
                );
                assert!(security.algebraic_bits >= target as f64 + (16f64 / 15.0).log2() - 1e-9);

                let (public, _) = fixture(batch);
                let exceptions = if compact::is_split(&prepared.layout) {
                    vec![Vec::new(); batch]
                } else {
                    Vec::new()
                };
                let (_, before) = prepared.transcript(&public, &[0; 32], &exceptions).unwrap();
                let mut altered = prepared;
                altered.schedule.terminal_merge_bits += 1;
                let (_, after) = altered.transcript(&public, &[0; 32], &exceptions).unwrap();
                assert_ne!(
                    before, after,
                    "terminal grinding must be configuration-bound"
                );
            }
        }
    }

    #[test]
    fn end_to_end_and_tampering() {
        for (batch, target) in [100, 128]
            .into_iter()
            .flat_map(|target| [1, 3, 8, 9, 16, 65].map(|batch| (batch, target)))
        {
            let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
            let (public, witness) = fixture(batch);
            let committed = prepared.commit(public.clone(), witness).unwrap();
            let mut prover_next = [0u64; 4];
            let proof = prepared
                .prove_observed(committed, |t| {
                    prover_next = std::array::from_fn(|_| t.get_challenge());
                })
                .unwrap();
            let mut verifier_next = [0u64; 4];
            prepared
                .verify_observed(&public, &proof, |t| {
                    verifier_next = std::array::from_fn(|_| t.get_challenge());
                })
                .unwrap();
            assert_eq!(prover_next, verifier_next);
            let ArithmeticProof::Raw(arithmetic) = &proof.arithmetic else {
                panic!("production must use word binding")
            };
            assert_eq!(
                arithmetic.binding.round_polynomials.len(),
                COEFFICIENT_LOG + 1 + prepared.arithmetic_capacity().ilog2() as usize
            );
            assert_eq!(
                arithmetic.binding_terminal.len(),
                2 + usize::from(batch < prepared.arithmetic_capacity())
            );
            assert_eq!(proof.bridges.len(), 1);
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
                    // Split indices are transcript-bound; Single has no metadata.
                    1 => {
                        if compact::is_split(&prepared.layout) {
                            bad.exceptions[0][0] += 1;
                        } else {
                            bad.exceptions.push(vec![0]);
                        }
                    }
                    2 => match &mut bad.ring {
                        RingProof::Canonical(ring) => ring.carries[0] += 1,
                        RingProof::Factored(ring) => ring.lift[0] += 1,
                    },
                    3 => {
                        if compact::is_split(&prepared.layout) {
                            bad.exceptions.pop();
                        } else {
                            bad.exceptions.push(Vec::new());
                        }
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
    fn default_binding_depends_only_on_public_configuration() {
        for workers in [1, 3, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            pool.install(|| {
                for target in [100, 128] {
                    for batch in [1, 32, 256, 1023, 1024] {
                        let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
                        let bits = PreparedFalconAlgebraic::new_with_binding(
                            batch,
                            target,
                            FalconAlgebraicBinding::Bits,
                        )
                        .unwrap();
                        assert!(prepared.schedule.word_binding);
                        assert_eq!(prepared.protocol_id(), PROTOCOL_ID);
                        let bit_id = if bits.layout.capacity() != bits.layout.allocation_capacity()
                        {
                            BIT_PROTOCOL_ID
                        } else if N == 1024 {
                            "bitz/falcon1024-algebraic/non-zk/v7"
                        } else {
                            "bitz/falcon512-algebraic/non-zk/v7"
                        };
                        assert_eq!(bits.protocol_id(), bit_id);
                        assert_eq!(prepared.layout, bits.layout);
                        assert_eq!(
                            prepared.geometry.physical_logs(),
                            bits.geometry.physical_logs()
                        );
                        assert_eq!(prepared.source_bits_per_signature(), 32 * N);
                        assert!(
                            prepared.security().algebraic_bits
                                >= target as f64 + (16f64 / 15.0).log2() - 1e-9
                        );
                    }
                }
            });
        }
    }

    #[test]
    fn word_binding_security_ledger_and_geometry_are_explicit() {
        for target in [100, 128] {
            for batch in [1, 3, 32, 33, 64, 256, 1023, 1024] {
                let words = PreparedFalconAlgebraic::new_with_binding(
                    batch,
                    target,
                    FalconAlgebraicBinding::Words,
                )
                .unwrap();
                let bits = PreparedFalconAlgebraic::new_with_binding(
                    batch,
                    target,
                    FalconAlgebraicBinding::Bits,
                )
                .unwrap();
                assert_eq!(words.protocol_id(), PROTOCOL_ID);
                assert_ne!(bits.protocol_id(), PROTOCOL_ID);
                assert_eq!(
                    words.geometry.physical_logs(),
                    bits.geometry.physical_logs()
                );
                assert_eq!(
                    binding_rounds(&words.layout, true) + 4,
                    binding_rounds(&bits.layout, false)
                );
                assert!(
                    words.security().algebraic_bits >= target as f64 + (16f64 / 15.0).log2() - 1e-9
                );
                let ledger = words.security().terms;
                let merge = ledger
                    .iter()
                    .find(|(name, _)| *name == "word terminal claim merge")
                    .unwrap()
                    .1;
                assert_eq!(
                    merge,
                    terminal_merge_numerator(&words.layout, true) as f64
                        * 2f64.powi(-125 - words.schedule.terminal_merge_bits as i32)
                );
                assert!(merge <= 2f64.powi(-(target as i32) - 4));
                assert!(
                    !bits
                        .security()
                        .terms
                        .iter()
                        .any(|(name, _)| *name == "word terminal claim merge")
                );
            }
        }
    }

    #[test]
    fn word_binding_end_to_end_rejects_terminal_and_protocol_mutations() {
        let split_batch = (1 << 20) / (32 * N);
        for (batch, target) in [100, 128]
            .into_iter()
            .flat_map(|target| [1, 3, split_batch, split_batch + 1].map(|batch| (batch, target)))
        {
            let words = PreparedFalconAlgebraic::new_with_binding(
                batch,
                target,
                FalconAlgebraicBinding::Words,
            )
            .unwrap();
            let bits = PreparedFalconAlgebraic::new_with_binding(
                batch,
                target,
                FalconAlgebraicBinding::Bits,
            )
            .unwrap();
            let (public, witness) = fixture(batch);
            let bit_proof = bits
                .prove(bits.commit(public.clone(), witness.clone()).unwrap())
                .unwrap();
            bits.verify(&public, &bit_proof).unwrap();
            assert!(words.verify(&public, &bit_proof).is_err());
            let committed = words.commit(public.clone(), witness).unwrap();
            let mut prover_checkpoint = 0u128;
            let proof = words
                .prove_observed(committed, |t| prover_checkpoint = t.get_challenge())
                .unwrap();
            if batch == split_batch {
                assert!(proof.exceptions.iter().any(|indices| !indices.is_empty()));
            }
            words
                .verify_observed(&public, &proof, |t| {
                    assert_eq!(prover_checkpoint, t.get_challenge::<u128>())
                })
                .unwrap();
            let ArithmeticProof::Raw(arithmetic) = &proof.arithmetic else {
                panic!()
            };
            assert_eq!(
                arithmetic.binding.round_polynomials.len(),
                binding_rounds(&words.layout, true)
            );
            assert!(bits.verify(&public, &proof).is_err());
            let (mut t, _) = words
                .transcript(&public, proof.source_root(), &proof.exceptions)
                .unwrap();
            let field = sample_field(&mut t).unwrap();
            let terminals = 2 + usize::from(batch < words.arithmetic_capacity());
            assert_eq!(arithmetic.binding_terminal.len(), terminals);
            for mutation in 0..terminals + 2 {
                let mut bad = proof.clone();
                let ArithmeticProof::Raw(arithmetic) = &mut bad.arithmetic else {
                    panic!()
                };
                match mutation {
                    index if index < terminals => {
                        let value = &mut arithmetic.binding_terminal[mutation];
                        *value = field.add(value, &field.one());
                    }
                    index if index == terminals => {
                        arithmetic.terminal_merge_nonce =
                            if target == 128 { None } else { Some(0) };
                    }
                    _ => {
                        arithmetic.binding.round_polynomials.pop();
                    }
                }
                assert!(
                    words.verify(&public, &bad).is_err(),
                    "word mutation {mutation}, security {target}"
                );
            }
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
                assert_eq!(
                    combined.source.as_ref().unwrap().rows(),
                    separate.source.as_ref().unwrap().rows()
                );
                assert_eq!(combined.packed, separate.packed);
                assert_eq!(combined.source_root(), separate.source_root());
                let combined_proof = prepared.prove(combined).unwrap();
                prepared.verify(&public, &combined_proof).unwrap();
            }
        }
    }

    #[test]
    fn packing_preserves_complete_proofs_and_transcript_continuation() {
        for workers in [1, 2, 4, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .stack_size(8 << 20)
                .build()
                .unwrap();
            pool.install(|| {
                for target in [100, 128] {
                    for batch in [1, 3, 17, 33, 1024] {
                        let prepared = PreparedFalconAlgebraic::new(batch, target).unwrap();
                        let (public, witness) = fixture(batch);
                        let data = WitnessData::new(&public, witness).unwrap();
                        let mut reference = None;
                        for (name, pack) in [
                            ("serial", Source::new as fn(Layout, &WitnessData) -> Source),
                            ("parallel", Source::new_parallel),
                        ] {
                            let source = pack(prepared.layout, &data);
                            let committed = prepared
                                .commit_source(public.clone(), data.clone(), source)
                                .unwrap();
                            let root = *committed.source_root();
                            let mut prover_next = [0_u64; 4];
                            let proof = prepared
                                .prove_observed(committed, |t| {
                                    prover_next = std::array::from_fn(|_| t.get_challenge());
                                })
                                .unwrap();
                            let mut verifier_next = [0_u64; 4];
                            prepared
                                .verify_observed(&public, &proof, |t| {
                                    verifier_next = std::array::from_fn(|_| t.get_challenge());
                                })
                                .unwrap();
                            assert_eq!(prover_next, verifier_next);
                            // Debug includes the proof contents and all present nonces.
                            let contents = (
                                root,
                                format!("{proof:?}"),
                                prover_next,
                                proof.payload_size_breakdown(),
                            );
                            if let Some(expected) = &reference {
                                assert_eq!(
                                    &contents, expected,
                                    "degree={N}, batch={batch}, target={target}, workers={workers}, packing={name}"
                                );
                            } else {
                                reference = Some(contents);
                            }
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn serial_and_parallel_binding_preserve_complete_proof_contents() {
        const WORKERS: [usize; 7] = [3, 1, 2, 4, 8, 12, 16];
        let pools = WORKERS.map(|threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .stack_size(8 << 20)
                .build()
                .unwrap()
        });
        // Batch 17 crosses the construction cutoff for both degrees and has
        // inactive instances; batch 1024 covers the full production layout.
        for batch in [17, 1024] {
            let (mut public, mut witness) = fixture(batch);
            for instance in 0..batch {
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
            for (target, binding) in [100, 128].into_iter().flat_map(|target| {
                [FalconAlgebraicBinding::Bits, FalconAlgebraicBinding::Words]
                    .map(|binding| (target, binding))
            }) {
                let run = |pool: &rayon::ThreadPool, threads| {
                    pool.install(|| {
                        assert_eq!(rayon::current_num_threads(), threads);
                        assert_eq!(
                            arithmetic::optimized_binding(batch),
                            matches!(threads, 1 | 8 | 12 | 16)
                                || (N == 512 && batch == 1024 && matches!(threads, 2 | 4))
                        );
                        let prepared =
                            PreparedFalconAlgebraic::new_with_binding(batch, target, binding)
                                .unwrap();
                        let committed = prepared.commit(public.clone(), witness.clone()).unwrap();
                        let root = *committed.source_root();
                        let proof = prepared.prove(committed).unwrap();
                        assert_eq!(proof.source_root(), &root);
                        prepared.verify(&public, &proof).unwrap();
                        if matches!(threads, 12 | 16) {
                            for index in [0, 2] {
                                pools[index]
                                    .install(|| prepared.verify(&public, &proof))
                                    .unwrap();
                            }
                        }
                        // Derived Debug includes every proof field, including all
                        // ring, arithmetic, bridge, joint, opening and PCS nonces.
                        // This compares contents within one build, not a wire codec.
                        (root, format!("{proof:?}"))
                    })
                };
                let reference = run(&pools[0], WORKERS[0]);
                for (pool, threads) in pools.iter().zip(WORKERS).skip(1) {
                    let actual = run(pool, threads);
                    assert_eq!(
                        reference.0, actual.0,
                        "degree={N}, batch={batch}, security={target}, binding={binding:?}, threads={threads}"
                    );
                    assert_eq!(
                        reference.1, actual.1,
                        "degree={N}, batch={batch}, security={target}, binding={binding:?}, threads={threads}"
                    );
                }
            }
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
        for batch in [1, 65] {
            let prepared = PreparedFalconAlgebraic::new(batch, 100).unwrap();
            let mut public = FalconAlgebraicStatement {
                public_keys: vec![[0; N]; batch],
                targets: vec![[0; N]; batch],
            };
            public.targets[0][0] = 7000;
            let mut witness = FalconAlgebraicWitness {
                s1: vec![[0; N]; batch],
                s2: vec![[0; N]; batch],
            };
            witness.s1[0][0] = 7000;
            let committed = prepared.commit(public.clone(), witness.clone()).unwrap();
            assert_eq!(committed.witness.witness.s1[0][0], 7000);
            let proof = prepared.prove(committed).unwrap();
            prepared.verify(&public, &proof).unwrap();
            witness.s1[0][0] += 1;
            assert!(prepared.commit(public, witness).is_err());
        }
    }
    }

    #[test]
    fn exact_large_coefficients_and_maximum_exceptions_verify() {
        let mut maximum = 0i16;
        while u64::from((maximum + 1) as u16).pow(2) <= BETA_SQUARED {
            maximum += 1;
        }
        for batch in [1, 65] {
            let prepared = PreparedFalconAlgebraic::new(batch, 100).unwrap();
            for values in [
                vec![2048; compact::MAX_EXCEPTIONS],
                vec![maximum],
                vec![-maximum],
            ] {
                let public = FalconAlgebraicStatement {
                    public_keys: vec![[0; N]; batch],
                    targets: vec![[0; N]; batch],
                };
                let mut witness = FalconAlgebraicWitness {
                    s1: vec![[0; N]; batch],
                    s2: vec![[0; N]; batch],
                };
                witness.s2[0][..values.len()].copy_from_slice(&values);
                let committed = prepared.commit(public.clone(), witness.clone()).unwrap();
                assert_eq!(committed.witness.witness, witness);
                if compact::is_split(&prepared.layout) {
                    assert_eq!(committed.exceptions[0].len(), values.len());
                    assert!(committed.exceptions[1..].iter().all(Vec::is_empty));
                } else {
                    assert!(committed.exceptions.is_empty());
                }
                let proof = prepared.prove(committed).unwrap();
                prepared.verify(&public, &proof).unwrap();
            }
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
            // Single commits the malformed source unchanged. The word prover
            // must reject it, independently of commitment construction.
            let committed = prepared
                .commit_source(public.clone(), data.clone(), source)
                .unwrap();
            assert!(prepared.prove(committed).is_err());
        }
    }

    #[test]
    fn omitted_physical_padding_is_constrained() {
        let prepared = PreparedFalconAlgebraic::new(1, 100).unwrap();
        let (public, witness) = fixture(1);
        let data = WitnessData::new(&public, witness).unwrap();
        for index in [
            prepared.layout.source_bits(),
            prepared.committed_source_bits() - 1,
        ] {
            let source = Source::new(prepared.layout, &data);
            let mut packed = compact::CompactSource::new(&prepared.layout, &source, &data).unwrap();
            let word = &mut packed.packed[0][index / 128];
            if index % 128 < 64 {
                word.lo ^= 1 << (index % 64);
            } else {
                word.hi ^= 1 << (index % 64);
            }
            let committed = prepared
                .commit_packed(public.clone(), data.clone(), Some(source), packed)
                .unwrap();
            match prepared.prove(committed) {
                Err(_) => (),
                Ok(proof) => assert!(prepared.verify(&public, &proof).is_err()),
            }
        }
    }

    #[test]
    fn compact_computation_requires_explicit_experimental_selection() {
        for batch in [1, 8, 16, 32, 65, 1024] {
            for target in [100, 128] {
                let production = PreparedFalconAlgebraic::new(batch, target).unwrap();
                assert_eq!(production.compute_variant(), "padding");
                assert!(!production.compact_binding);
                assert_eq!(production.bridge_layouts().len(), 1);
                let experimental =
                    PreparedFalconAlgebraic::with_compute_variant(batch, target, "combined")
                        .unwrap();
                assert_eq!(
                    experimental.compact_binding,
                    compact::is_split(&experimental.layout)
                );
                assert!(!experimental.schedule.word_binding);
                assert!(experimental.security().algebraic_bits >= target as f64);
            }
        }
        assert!(
            PreparedFalconAlgebraic::with_configuration(
                1024,
                128,
                "combined",
                FalconAlgebraicBinding::Words,
            )
            .is_err()
        );
    }

    #[test]
    fn compact_bridges_and_combination_are_bound() {
        let prepared = PreparedFalconAlgebraic::with_compute_variant(65, 100, "combined").unwrap();
        let (public, witness) = fixture(65);
        let proof = prepared
            .prove(prepared.commit(public.clone(), witness).unwrap())
            .unwrap();
        assert_eq!(proof.bridges.len(), 3);
        for mutation in 0..4 {
            let mut bad = proof.clone();
            match mutation {
                0 => bad.bridges.swap(0, 1),
                1 => {
                    bad.bridges.pop();
                }
                2 => bad.bridge_merge_nonces.push(0),
                _ => {
                    let ArithmeticProof::Compact(arithmetic) = &mut bad.arithmetic else {
                        panic!()
                    };
                    arithmetic.bindings.swap(0, 1);
                }
            }
            assert!(prepared.verify(&public, &bad).is_err());
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
            for batch in [32, 512, 513, 1024] {
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
