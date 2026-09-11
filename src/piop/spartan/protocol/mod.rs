//! The one F2Z protocol every relation runs.
//!
//! A relation describes itself once — its committed bit tensor, its
//! constraint matrices, how its assignment blocks map to bit slots, the
//! domain strings it binds under — and this module runs the protocol of
//! paper §2.1 for it:
//!
//! 1. bind the statement (the relation's assignment binding, the opener
//!    policy and Round 0 of the opening),
//! 2. grind and sample the Step-2 prime, project the relation,
//! 3. run the Spartan PIOP over the runtime field, every drawn challenge
//!    behind one grinding boundary at the profile difficulty,
//! 4. bitify the terminal claim, bind the bridge digest, grind the
//!    terminal boundary,
//! 5. open the committed tensor through the runtime-q F2Z opener.
//!
//! Transcripts are byte-for-byte those of the protocols this module
//! replaced; the pins in `tests/transcript_state_pins.rs` hold them.

pub mod binding;
pub mod bitify;

use std::{borrow::Cow, sync::OnceLock};

use crypto_primitives::{PrimeField, crypto_bigint_uint::Uint};
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{LigeritoProfile, ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};
use thiserror::Error;

use crate::{
    ext_proj::{PrimeSamplingError, ProjArith, sample_prime_in_interval},
    ligerito::{LOG_PACKING, packed_vars},
    ligerito_flock::{
        FlockCommitHint, FlockRsError, IntEvalRsLigModQProof, LigeritoSelection, ModQOpeningKind,
        ResolvedLigerito, bind_prover_ood, bind_verifier_ood, commit_rs_ligerito_rows,
        prove_mle_eval_mod_q_ligerito_with_weight_chunks,
        verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime,
    },
    pcs::IntegerMatrixLayout,
    poly::{mle::DenseMultilinearExtension, univariate::binary_gf128::BinaryFieldGF128},
    transcript::traits::Transcript,
};

use super::{
    ConstraintMatrices, ConstraintMatricesSkeleton, PreparedConstraintMatrices, SpartanField,
    absorb_spartan_message,
    grinding::{
        GrindingError, ProverGrindingTranscript, VerifierGrindingTranscript,
        grind_and_absorb_in_domain, verify_and_absorb_in_domain,
    },
    matrix::{self, ModulusIndependentCoefficient, ScaledMleEvaluationClaim, SpartanMatrixCoefficient},
    piop::{
        SpartanError, SpartanPiopProof, SpartanReductionStrategy,
        prove_spartan_piop_native_u64_borrowed, prove_spartan_piop_native_u64_with_strategy,
        prove_spartan_piop_native_u64_with_univariate_skip_borrowed,
        prove_spartan_piop_raw_products_native_assignment,
        prove_spartan_piop_raw_products_raw_witness, prove_spartan_piop_with_strategy,
        verify_spartan_proof, verify_spartan_univariate_skip_proof,
    },
    profile::{IopInstanceFacts, IopSecurityParams, IopSecurityProfile, Lambda100, ProfileError},
    raw_monty::{NativeProducts, RawMontyCoefficient, RawMontyCtx, RawProducts, RawWitness},
    sumcheck::R1csProductMles,
    univariate_skip::UnivariateSkipSpartanPiopProof,
};

pub use binding::BindingHasher;
pub use bitify::{BitifiedClaim, BitifiedRows, BlockTable, SlotRange};

/// Runtime-configured Spartan field used by every F2Z relation.
pub type SpartanF2zField = crypto_primitives::crypto_bigint_monty::F128;

/// Embedded, validator-gated Ligerito profiles begin at a 22-variable
/// committed bit MLE: seven slot variables plus fifteen gate variables.
pub const MIN_PRODUCTION_GATE_VARS: usize = 15;

/// Failures in layout validation, claim translation, or either proof system.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// A relation-level failure (layout, witness or matrix construction).
    #[error(transparent)]
    Relation(Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error(transparent)]
    Spartan(#[from] SpartanError),

    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    #[error("the F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    #[error("the sampled modulus is not supported by the runtime field")]
    UnsupportedFieldModulus,

    #[error("the prepared relation does not match the witness layout")]
    RelationWitnessLayoutMismatch,

    #[error("the compact bit rows do not match the relation layout")]
    InvalidBitRows,

    #[error("the F2Z parameters are invalid for the relation layout")]
    InvalidF2zParameters,

    #[error("the combined Spartan/F2Z proof requires at least 2^15 gate slots")]
    UnauditedF2zParameters,

    #[error("the commitment parameters do not match the derived F2Z configuration")]
    CommitmentConfigMismatch,

    #[error("the commitment does not match the prepared terminal-opening statement")]
    PreparedOpeningCommitmentMismatch,

    #[error("the terminal Spartan claim has the wrong point shape")]
    InvalidClaimPoint,

    #[error("a terminal Spartan claim element does not use the sampled runtime field")]
    ClaimFieldMismatch,

    #[error("a constant-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOnlyClaim,

    #[error("a host length does not fit the canonical transcript encoding")]
    BindingEncodingOverflow,

    #[error("the block table is not a complete little-endian block map")]
    InvalidBlockTable,

    /// The security profile could not be instantiated at this shape.
    #[error(transparent)]
    Profile(#[from] ProfileError),

    /// Runtime-prime sampling failed.
    #[error(transparent)]
    PrimeSampling(#[from] PrimeSamplingError),

    /// A Fiat--Shamir grinding nonce could not be produced or checked.
    #[error(transparent)]
    Grinding(#[from] GrindingError),

    /// The relation supports single-prime profiles only (the two-prime
    /// Strategy 2 belongs to the MultiSwap-style adapters).
    #[error("this relation requires a single-prime security profile")]
    UnsupportedProfile,

    /// A proof selected a different univariate-prefix width than the one
    /// whose degree was included in the prepared security profile.
    #[error("the proof uses univariate skip K={actual}; expected K={expected}")]
    UnexpectedUnivariateSkipVariables { expected: u8, actual: u8 },

    /// The derived prime interval must keep the row weights to one
    /// exponent-fold chunk (`q_bits <= c_w`); the profile guarantees this,
    /// so a violation is an internal error.
    #[error("the runtime prime produced a multi-chunk row functional")]
    MultiChunkRuntimeWeights,

    /// The relation's PIOP witness representation does not fit its kernel.
    #[error("the relation's PIOP witness representation does not fit its kernel")]
    UnsupportedKernel,
}

impl ProtocolError {
    /// Wraps a relation-level error.
    pub fn relation(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Relation(Box::new(error))
    }
}

/// The transcript domain strings and profiler scope labels of one relation.
#[derive(Clone, Copy, Debug)]
pub struct Domains {
    /// Tag of the statement frame carrying the assignment binding.
    pub statement_tag: &'static [u8],
    /// Domain bound before the Step-2 prime draw.
    pub prime_sampling: &'static [u8],
    /// Grinding domains of the initial, per-draw PIOP and terminal boundaries.
    pub initial_grinding: &'static [u8],
    pub piop_grinding: &'static [u8],
    pub terminal_grinding: &'static [u8],
    /// Domain of the bridge digest bound between Spartan and F2Z.
    pub bitified_claim: &'static [u8],
    /// The opener statement domain.
    pub opening: ModQOpeningKind,
    /// Profiler scope labels.
    pub scopes: Scopes,
}

/// The per-relation profiler scope labels (static, as the profiler needs).
#[derive(Clone, Copy, Debug)]
pub struct Scopes {
    pub relation_projection_prove: &'static str,
    pub relation_projection_verify: &'static str,
    pub witness_projection_prove: &'static str,
    pub spartan_prove: &'static str,
    pub spartan_verify: &'static str,
    pub bitify_prover: &'static str,
    pub bitify_verifier: &'static str,
    pub f2z_prove: &'static str,
    pub f2z_verify: &'static str,
    pub f2z_prepare_prover: &'static str,
    pub f2z_prepare_verifier: &'static str,
}

/// Builds the [`Scopes`] of a relation from its scope prefix.
#[macro_export]
macro_rules! protocol_scopes {
    ($prefix:literal) => {
        $crate::piop::spartan::protocol::Scopes {
            relation_projection_prove: concat!($prefix, ":relation_projection_prove"),
            relation_projection_verify: concat!($prefix, ":relation_projection_verify"),
            witness_projection_prove: concat!($prefix, ":witness_projection_prove"),
            spartan_prove: concat!($prefix, ":spartan_prove"),
            spartan_verify: concat!($prefix, ":spartan_verify"),
            bitify_prover: concat!($prefix, ":bitify_prover"),
            bitify_verifier: concat!($prefix, ":bitify_verifier"),
            f2z_prove: concat!($prefix, ":f2z_prove"),
            f2z_verify: concat!($prefix, ":f2z_verify"),
            f2z_prepare_prover: concat!($prefix, ":f2z_prepare_prover"),
            f2z_prepare_verifier: concat!($prefix, ":f2z_prepare_verifier"),
        }
    };
}

/// Which Spartan outer reduction the relation runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kernel {
    /// The cubic outer sumcheck over every row variable.
    Plain,
    /// A known-zero univariate prefix skip over the `skip_vars` low row
    /// variables, then the cubic tail.
    UnivariateSkip { skip_vars: usize },
}

/// The prover's view of a witness at the runtime prime: whichever table
/// representation the relation's Spartan kernel consumes.
pub enum PiopWitness<'w> {
    /// Exact `u64` products and a borrowed native assignment (the leading
    /// entries of the padded column domain; the rest is zero).
    Native {
        az: Cow<'w, [u64]>,
        bz: Cow<'w, [u64]>,
        cz: Cow<'w, [u64]>,
        assignment: &'w [u64],
    },
    /// Owned native tables under an explicit reduction strategy.
    NativeOwned {
        products: R1csProductMles<u64>,
        assignment: DenseMultilinearExtension<u64>,
        strategy: SpartanReductionStrategy,
    },
    /// Raw product residues and a borrowed native assignment.
    RawProductsNative {
        products: RawProducts,
        assignment: &'w [u64],
    },
    /// Raw product residues and a raw assignment.
    RawProductsRaw {
        products: RawProducts,
        witness: RawWitness<'w>,
    },
    /// Field-valued tables under an explicit reduction strategy.
    Field {
        products: R1csProductMles<SpartanF2zField>,
        assignment: DenseMultilinearExtension<SpartanF2zField>,
        strategy: SpartanReductionStrategy,
    },
}

/// Prover-side options that do not move the transcript.
#[derive(Clone, Copy, Debug)]
pub struct ProveOptions {
    /// The Spartan reduction strategy for relations that offer a choice
    /// (transcript-neutral).
    pub strategy: SpartanReductionStrategy,
}

impl Default for ProveOptions {
    fn default() -> Self {
        Self {
            strategy: SpartanReductionStrategy::DelayedBarrett,
        }
    }
}

/// The static description of one relation: everything the protocol needs
/// besides a witness.
pub trait RelationSpec: Sync {
    /// The constraint-matrix coefficient type.
    type Coefficient: SpartanMatrixCoefficient<SpartanF2zField>
        + ModulusIndependentCoefficient<SpartanF2zField>
        + RawMontyCoefficient
        + Send
        + Sync;
    /// The prover's witness.
    type Witness: ?Sized + Sync;

    fn domains(&self) -> &'static Domains;

    /// Geometry of the committed bit tensor.
    fn committed_layout(&self) -> IntegerMatrixLayout;

    /// Number of gate coordinates of the assignment MLE (the assignment has
    /// `gate_vars + selector_vars` variables).
    fn gate_vars(&self) -> usize;

    /// The public statement facts the security-profile derivation consumes.
    fn instance_facts(&self) -> IopInstanceFacts;

    /// The exact constraint matrices (prime-independent coefficients).
    fn constraint_matrices(&self) -> Result<ConstraintMatrices<Self::Coefficient>, ProtocolError>;

    /// Rejects layouts the protocol cannot run.
    fn validate_geometry(&self) -> Result<(), ProtocolError>;

    /// How the assignment blocks map to bit slots.
    fn block_table(&self) -> BlockTable;

    fn kernel(&self) -> Kernel;

    /// Uniform difficulty for the opener's wrapped challenges.
    fn opener_grinding_bits(&self, security: &IopSecurityParams) -> u32 {
        security.forest_round_grinding_bits
    }

    fn check_witness(&self, witness: &Self::Witness) -> Result<(), ProtocolError>;

    /// The digest binding layout, commitment and the profile's public
    /// parameters — everything fixed BEFORE the prime draw.
    fn assignment_binding(
        &self,
        commitment: &Commitment,
        security: &IopSecurityParams,
        ligerito: &LigProverConfig,
    ) -> Result<[u8; 32], ProtocolError>;

    /// Writes the relation's constants section of the bridge digest (between
    /// the runtime prime and the terminal claim).
    fn hash_bridge_constants(&self, hasher: &mut BindingHasher) -> Result<(), ProtocolError>;

    /// The witness tables the Spartan kernel consumes at the runtime prime.
    fn piop_witness<'w>(
        &self,
        witness: &'w Self::Witness,
        ctx: &RawMontyCtx,
        options: ProveOptions,
    ) -> Result<PiopWitness<'w>, ProtocolError>;
}

/// The prime-independent prefix of a relation: the exact constraint
/// matrices with their prime-independent preparation (skeleton digest,
/// padded widths, selector layout — instantiated per transcript draw), the
/// relation description and the instantiated security profile — everything
/// the Spartan PIOP and bitification consume, and nothing of the standalone
/// opener. Compositions that discharge the bitified claim through their own
/// opener (the hybrid of [`crate::hybrid`]) prepare this directly.
pub struct PreparedRelationPrefix<S: RelationSpec> {
    spec: S,
    skeleton: ConstraintMatricesSkeleton<SpartanF2zField, S::Coefficient>,
    security: IopSecurityParams,
}

impl<S: RelationSpec> PreparedRelationPrefix<S> {
    /// Prepares the prefix under an explicit single-prime profile.
    pub fn new<P: IopSecurityProfile>(spec: S) -> Result<Self, ProtocolError> {
        spec.validate_geometry()?;
        let security = instantiate_profile::<P, S>(&spec)?;
        let skeleton =
            ConstraintMatricesSkeleton::new(spec.constraint_matrices()?).map_err(SpartanError::from)?;
        Ok(Self {
            spec,
            skeleton,
            security,
        })
    }

    /// The relation description this prefix was prepared from.
    pub const fn layout(&self) -> &S {
        &self.spec
    }

    /// F2Z geometry of the committed bit tensor.
    pub fn params(&self) -> IntegerMatrixLayout {
        self.spec.committed_layout()
    }

    /// The instantiated security parameters and their accounting.
    pub const fn security(&self) -> &IopSecurityParams {
        &self.security
    }

    pub const fn skeleton(&self) -> &ConstraintMatricesSkeleton<SpartanF2zField, S::Coefficient> {
        &self.skeleton
    }
}

/// The setup-once, prime-independent bundle of a relation: its
/// [`PreparedRelationPrefix`] plus the validator-gated Ligerito
/// prover/verifier configuration of the standalone opener.
pub struct PreparedRelation<S: RelationSpec> {
    prefix: PreparedRelationPrefix<S>,
    selection: LigeritoSelection,
    ligerito: ResolvedLigerito,
}

impl<S: RelationSpec> PreparedRelation<S> {
    /// Prepares the relation at the default [`Lambda100`] profile.
    pub fn new(spec: S) -> Result<Self, ProtocolError> {
        Self::new_with_profile::<Lambda100>(spec)
    }

    /// Prepares the relation under an explicit single-prime profile with the
    /// profile's default opener (Johnson+OOD at 100 bits).
    pub fn new_with_profile<P: IopSecurityProfile>(spec: S) -> Result<Self, ProtocolError> {
        Self::new_with_profile_and_ligerito::<P>(
            spec,
            LigeritoSelection::for_target(P::LIGERITO_TARGET_BITS),
        )
    }

    /// Prepares the relation under an explicit single-prime profile and an
    /// explicit Ligerito opener geometry.
    pub fn new_with_profile_and_ligerito<P: IopSecurityProfile>(
        spec: S,
        selection: LigeritoSelection,
    ) -> Result<Self, ProtocolError> {
        let mut prefix = PreparedRelationPrefix::new::<P>(spec)?;
        if prefix.spec.gate_vars() < MIN_PRODUCTION_GATE_VARS {
            return Err(ProtocolError::UnauditedF2zParameters);
        }
        let p = prefix.params();
        let ligerito = selection
            .resolve(packed_variables(&p)?, prefix.security.ligerito_target_bits)
            .map_err(ProtocolError::LigeritoConfig)?;
        prefix.security.adopt_ood_round(ligerito.ood_bits())?;
        validate_config_pair(&p, ligerito.prover(), ligerito.verifier())?;
        Ok(Self {
            prefix,
            selection,
            ligerito,
        })
    }

    /// The opener-independent prefix.
    pub const fn prefix(&self) -> &PreparedRelationPrefix<S> {
        &self.prefix
    }

    /// The relation description this bundle was prepared from.
    pub const fn layout(&self) -> &S {
        &self.prefix.spec
    }

    /// F2Z geometry of the committed bit tensor.
    pub fn params(&self) -> IntegerMatrixLayout {
        self.prefix.params()
    }

    /// The instantiated security parameters and their accounting.
    pub const fn security(&self) -> &IopSecurityParams {
        &self.prefix.security
    }

    /// The opener geometry this relation was prepared with.
    pub const fn ligerito(&self) -> LigeritoSelection {
        self.selection
    }

    pub const fn ligerito_configuration(&self) -> &ResolvedLigerito {
        &self.ligerito
    }

    pub const fn skeleton(&self) -> &ConstraintMatricesSkeleton<SpartanF2zField, S::Coefficient> {
        &self.prefix.skeleton
    }

    /// The digest binding layout, commitment and the profile's public
    /// parameters — everything fixed BEFORE the prime draw.
    pub fn assignment_binding(&self, commitment: &Commitment) -> Result<[u8; 32], ProtocolError> {
        self.prefix.spec.assignment_binding(
            commitment,
            &self.prefix.security,
            self.ligerito.prover(),
        )
    }
}

/// Instantiates the single-prime profile `P` at the relation's instance facts.
pub fn instantiate_profile<P: IopSecurityProfile, S: RelationSpec>(
    spec: &S,
) -> Result<IopSecurityParams, ProtocolError> {
    let security = P::instantiate(&spec.instance_facts())?;
    if security.projection_full_width || security.reduction.is_some() {
        return Err(ProtocolError::UnsupportedProfile);
    }
    Ok(security)
}

/// The Spartan part of a proof, in the shape the relation's kernel produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpartanProof {
    Plain(SpartanPiopProof<SpartanF2zField>),
    UnivariateSkip(UnivariateSkipSpartanPiopProof<SpartanF2zField>),
}

impl SpartanProof {
    /// Field elements in the payload, for analytic size accounting.
    pub fn payload_elements(&self) -> usize {
        match self {
            Self::Plain(proof) => {
                4 * proof.outer.sumcheck.round_polynomials.len()
                    + 3
                    + 3 * proof.inner.round_polynomials.len()
            }
            Self::UnivariateSkip(proof) => {
                proof.outer.skip.finite_q_evaluations.len()
                    + 1
                    + 4 * proof.outer.tail.sumcheck.round_polynomials.len()
                    + 3
                    + 3 * proof.inner.round_polynomials.len()
            }
        }
    }

    /// The plain cubic-outer proof, if that is the kernel's shape.
    pub const fn plain(&self) -> Option<&SpartanPiopProof<SpartanF2zField>> {
        match self {
            Self::Plain(proof) => Some(proof),
            Self::UnivariateSkip(_) => None,
        }
    }

    /// The univariate-skip proof, if that is the kernel's shape.
    pub const fn univariate_skip(&self) -> Option<&UnivariateSkipSpartanPiopProof<SpartanF2zField>> {
        match self {
            Self::UnivariateSkip(proof) => Some(proof),
            Self::Plain(_) => None,
        }
    }
}

/// The transcript messages of the protocol prefix (steps 2–4): the grinding
/// nonces around the prime draw and the opening, and the Spartan reduction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpartanPrefixProof {
    pub initial_nonce: u64,
    pub piop_nonces: Vec<u64>,
    pub spartan: SpartanProof,
    pub terminal_nonce: u64,
}

/// A proof over a transcript-selected prime: the Spartan reduction, the
/// runtime-q F2Z opening and any profile-selected grinding nonces.
#[derive(Clone)]
pub struct Proof {
    prefix: SpartanPrefixProof,
    f2z: IntEvalRsLigModQProof,
}

impl Proof {
    /// Spartan outer and inner sumcheck proofs.
    pub const fn spartan(&self) -> &SpartanProof {
        &self.prefix.spartan
    }

    /// Runtime-prime F2Z opening proof.
    pub const fn f2z(&self) -> &IntEvalRsLigModQProof {
        &self.f2z
    }

    /// The prefix messages.
    pub const fn prefix(&self) -> &SpartanPrefixProof {
        &self.prefix
    }

    pub const fn initial_nonce(&self) -> u64 {
        self.prefix.initial_nonce
    }

    pub const fn terminal_nonce(&self) -> u64 {
        self.prefix.terminal_nonce
    }

    pub fn piop_nonces(&self) -> &[u64] {
        &self.prefix.piop_nonces
    }

    /// Field elements in the Spartan payload, for analytic size accounting.
    pub fn spartan_payload_elements(&self) -> usize {
        self.prefix.spartan.payload_elements()
    }

    /// Transmitted grinding nonces (initial/terminal boundaries when armed,
    /// the per-draw PIOP nonces, and the forest section in the F2Z stream).
    pub fn grinding_nonce_count(&self, security: &IopSecurityParams) -> usize {
        usize::from(security.initial_grinding_bits > 0)
            + usize::from(security.terminal_grinding_bits > 0)
            + self.prefix.piop_nonces.len()
            + self.f2z.grinding_nonces.len()
    }

    /// Serialized size in bytes of the proof: the Spartan payload as 16-byte
    /// field elements, the nonces as 8-byte words, and the F2Z opening's
    /// exact codec bytes.
    pub fn size_bytes(&self, security: &IopSecurityParams) -> usize {
        self.spartan_payload_elements() * 16
            + (self.grinding_nonce_count(security) - self.f2z.grinding_nonces.len()) * 8
            + self.f2z.to_bytes().len()
    }

    /// Splits the proof into its parts (tests and codecs).
    pub fn into_parts(self) -> (SpartanPrefixProof, IntEvalRsLigModQProof) {
        (self.prefix, self.f2z)
    }

    /// Assembles a proof from its parts (tests and codecs).
    pub const fn from_parts(prefix: SpartanPrefixProof, f2z: IntEvalRsLigModQProof) -> Self {
        Self { prefix, f2z }
    }

    /// Mutable access to the opening proof (tests).
    pub fn f2z_mut(&mut self) -> &mut IntEvalRsLigModQProof {
        &mut self.f2z
    }

    /// Mutable access to the prefix messages (tests).
    pub fn prefix_mut(&mut self) -> &mut SpartanPrefixProof {
        &mut self.prefix
    }
}

/// Commits prebuilt compact bit rows under the prepared relation's
/// profile-selected Ligerito configuration.
///
/// Accepting ownership of `rows` lets benchmarks time bitification separately
/// and move the packed store into the commitment without retaining a duplicate.
pub fn commit<S: RelationSpec>(
    prepared: &PreparedRelation<S>,
    rows: Vec<Vec<u64>>,
) -> Result<FlockCommitHint, ProtocolError> {
    let p = prepared.params();
    prepared.prefix.spec.validate_geometry()?;
    validate_bit_rows(&p, &rows)?;
    validate_config_pair(&p, prepared.ligerito.prover(), prepared.ligerito.verifier())?;

    // All assertion-bearing shape requirements of the low-level commit have
    // been checked above.
    let hint = commit_rs_ligerito_rows(&p, rows, prepared.ligerito.prover());
    validate_commitment(&p, &hint.commitment, prepared.ligerito.prover())?;
    Ok(hint)
}

/// Proves the relation under its prepared security profile with the default
/// prover options.
pub fn prove<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prepared: &PreparedRelation<S>,
    witness: &S::Witness,
    hint: &FlockCommitHint,
) -> Result<Proof, ProtocolError> {
    prove_with_options(transcript, prepared, witness, hint, ProveOptions::default())
}

/// Proves the relation: commit-before-prime, a transcript-sampled Step-2
/// prime, the Spartan PIOP over that runtime field, bitification, and the
/// runtime-q F2Z opening.
pub fn prove_with_options<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prepared: &PreparedRelation<S>,
    witness: &S::Witness,
    hint: &FlockCommitHint,
    options: ProveOptions,
) -> Result<Proof, ProtocolError> {
    let prefix = &prepared.prefix;
    let spec = &prefix.spec;
    spec.check_witness(witness)?;
    let p = prepared.params();
    let pc = prepared.ligerito.prover();
    let vc = prepared.ligerito.verifier();
    validate_config_pair(&p, pc, vc)?;
    validate_bit_rows(&p, hint.rows())?;
    validate_commitment(&p, &hint.commitment, pc)?;
    let security = &prefix.security;
    let domains = spec.domains();
    let scopes = &domains.scopes;

    let binding = prepared.assignment_binding(&hint.commitment)?;
    absorb_spartan_message(transcript, domains.statement_tag, &binding);
    prepared.ligerito.bind(transcript);
    let ood = bind_prover_ood(transcript, hint, security.ood);

    let proved = prove_prefix(transcript, prefix, witness, &binding, options)?;
    let (q, q_bits, _, arith) = proved.prime;

    // Steps 5.1–5.3: the runtime-q F2Z opening (one chunk by construction).
    let f2z = {
        let _step5 = crate::utils::prof::scope("step5:open_prove");
        let _scope = crate::utils::prof::scope(scopes.f2z_prove);
        let chunks = {
            let _scope = crate::utils::prof::scope(scopes.f2z_prepare_prover);
            bitify::prepare_chunks(&proved.opening, &proved.table, q_bits, &arith)?
        };
        if chunks.len() != 1 {
            return Err(ProtocolError::MultiChunkRuntimeWeights);
        }
        prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            transcript,
            domains.opening,
            hint,
            &p,
            &chunks,
            &proved.bridge_digest,
            q_bits,
            f2z_generator(),
            spec.opener_grinding_bits(security),
            ood,
            pc,
        )
        .map_err(ProtocolError::F2z)?
    };
    let _ = q;

    Ok(Proof {
        prefix: proved.messages,
        f2z,
    })
}

/// Verifies a proof, re-deriving the prime from the bound transcript.
pub fn verify<T: Transcript + Send, S: RelationSpec>(
    transcript: &mut T,
    prepared: &PreparedRelation<S>,
    commitment: &Commitment,
    proof: &Proof,
) -> Result<(), ProtocolError> {
    let prefix = &prepared.prefix;
    let spec = &prefix.spec;
    let p = prepared.params();
    let pc = prepared.ligerito.prover();
    let vc = prepared.ligerito.verifier();
    validate_config_pair(&p, pc, vc)?;
    validate_commitment(&p, commitment, pc)?;
    let security = &prefix.security;
    let domains = spec.domains();
    let scopes = &domains.scopes;
    check_proof_kernel(spec.kernel(), &proof.prefix.spartan)?;

    let binding = prepared.assignment_binding(commitment)?;
    absorb_spartan_message(transcript, domains.statement_tag, &binding);
    prepared.ligerito.bind(transcript);
    let ood = bind_verifier_ood(
        transcript,
        packed_variables(&p)?,
        security.ood,
        proof.f2z.ood.as_ref(),
    )
    .map_err(ProtocolError::F2z)?;

    let verified = verify_prefix(transcript, prefix, &binding, &proof.prefix)?;
    let (q, q_bits, _, arith) = verified.prime;

    let _step5 = crate::utils::prof::scope("step5:open_verify");
    let _scope = crate::utils::prof::scope(scopes.f2z_verify);
    let (chunks, col_weights_q) = {
        let _scope = crate::utils::prof::scope(scopes.f2z_prepare_verifier);
        let chunks = bitify::prepare_chunks(&verified.opening, &verified.table, q_bits, &arith)?;
        let col_weights_q: Vec<u128> = bitify::column_weights(&verified.opening, &arith)?
            .iter()
            .map(|weight| weight.0)
            .collect();
        (chunks, col_weights_q)
    };
    if chunks.len() != 1 {
        return Err(ProtocolError::MultiChunkRuntimeWeights);
    }
    verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
        transcript,
        domains.opening,
        commitment,
        &proof.f2z,
        &p,
        &chunks,
        &col_weights_q,
        &verified.bridge_digest,
        f2z_generator(),
        verified.opening.claimed.0,
        q,
        q_bits,
        spec.opener_grinding_bits(security),
        ood,
        vc,
    )
    .map_err(ProtocolError::F2z)
}

/// The prover's output of the protocol prefix: the transcript messages and
/// the bitified claim the opening discharges.
pub struct ProvedPrefix {
    pub messages: SpartanPrefixProof,
    pub opening: BitifiedClaim,
    pub bridge_digest: [u8; 32],
    pub prime: RuntimePrime,
    pub table: BlockTable,
}

/// The verifier's output of the protocol prefix.
pub struct VerifiedPrefix {
    pub opening: BitifiedClaim,
    pub bridge_digest: [u8; 32],
    pub prime: RuntimePrime,
    pub table: BlockTable,
}

/// Steps 2–4 of the protocol after the statement has been bound: the initial
/// grinding boundary, the Step-2 prime draw and relation projection, the
/// Spartan PIOP under per-draw grinding, bitification and the terminal
/// boundary.
pub fn prove_prefix<T: Transcript, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    witness: &S::Witness,
    binding: &[u8; 32],
    options: ProveOptions,
) -> Result<ProvedPrefix, ProtocolError> {
    let spec = &prefix.spec;
    spec.check_witness(witness)?;
    let security = &prefix.security;
    let domains = spec.domains();
    let scopes = &domains.scopes;

    // Step 2: pre-draw grinding, prime sample, and relation projection into
    // the runtime field.
    let step2_scope = crate::utils::prof::scope("step2:project_prove");
    let initial_nonce =
        grind_boundary(transcript, domains.initial_grinding, security.initial_grinding_bits)?;
    let prime = sample_mod_q(transcript, domains.prime_sampling, security)?;
    let (q, _, config, arith) = &prime;
    let matrices = {
        let _scope = crate::utils::prof::scope(scopes.relation_projection_prove);
        PreparedConstraintMatrices::<SpartanF2zField, S::Coefficient>::from_skeleton(
            &prefix.skeleton,
            config,
        )
        .map_err(SpartanError::from)?
    };
    let piop_witness = {
        let _scope = crate::utils::prof::scope(scopes.witness_projection_prove);
        let ctx = RawMontyCtx::new(config);
        spec.piop_witness(witness, &ctx, options)?
    };
    drop(step2_scope);

    // Step 3: the Spartan PIOP over F_q, every drawn challenge preceded by
    // one PIOP grinding boundary at the profile's difficulty (a transparent
    // pass-through at λ = 100).
    let (spartan, terminal_claim, piop_nonces) = {
        let _step3 = crate::utils::prof::scope("step3:piop_prove");
        let _scope = crate::utils::prof::scope(scopes.spartan_prove);
        let mut grinder = ProverGrindingTranscript::<_>::new_in_domain(
            transcript,
            piop_wrap_bits(security),
            domains.piop_grinding,
        );
        let (spartan, terminal_claim) =
            run_kernel(&mut grinder, spec.kernel(), &matrices, binding, piop_witness)?;
        (spartan, terminal_claim, grinder.finish())
    };

    // Step 4: bitification at the runtime prime, plus the terminal
    // boundary protecting the opening challenges.
    let step4_scope = crate::utils::prof::scope("step4:bitify_prove");
    let table = spec.block_table();
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope(scopes.bitify_prover);
        bitify_and_bind(spec, &matrices, binding, &terminal_claim, &table, *q, arith)?
    };
    let terminal_nonce =
        grind_boundary(transcript, domains.terminal_grinding, security.terminal_grinding_bits)?;
    drop(step4_scope);

    Ok(ProvedPrefix {
        messages: SpartanPrefixProof {
            initial_nonce,
            piop_nonces,
            spartan,
            terminal_nonce,
        },
        opening,
        bridge_digest,
        prime,
        table,
    })
}

/// Verifier twin of [`prove_prefix`].
pub fn verify_prefix<T: Transcript, S: RelationSpec>(
    transcript: &mut T,
    prefix: &PreparedRelationPrefix<S>,
    binding: &[u8; 32],
    messages: &SpartanPrefixProof,
) -> Result<VerifiedPrefix, ProtocolError> {
    let spec = &prefix.spec;
    let security = &prefix.security;
    let domains = spec.domains();
    let scopes = &domains.scopes;
    check_proof_kernel(spec.kernel(), &messages.spartan)?;

    let step2_scope = crate::utils::prof::scope("step2:project_verify");
    check_boundary(
        transcript,
        domains.initial_grinding,
        security.initial_grinding_bits,
        messages.initial_nonce,
    )?;
    let prime = sample_mod_q(transcript, domains.prime_sampling, security)?;
    let (q, _, config, arith) = &prime;
    let matrices = {
        let _scope = crate::utils::prof::scope(scopes.relation_projection_verify);
        PreparedConstraintMatrices::<SpartanF2zField, S::Coefficient>::from_skeleton(
            &prefix.skeleton,
            config,
        )
        .map_err(SpartanError::from)?
    };
    drop(step2_scope);

    let terminal_claim = {
        let _step3 = crate::utils::prof::scope("step3:piop_verify");
        let _scope = crate::utils::prof::scope(scopes.spartan_verify);
        let mut grinder = VerifierGrindingTranscript::<_>::new_in_domain(
            transcript,
            piop_wrap_bits(security),
            &messages.piop_nonces,
            domains.piop_grinding,
        );
        let terminal_claim = match &messages.spartan {
            SpartanProof::Plain(spartan) => {
                verify_spartan_proof(&mut grinder, &matrices, binding, spartan)?
            }
            SpartanProof::UnivariateSkip(spartan) => {
                verify_spartan_univariate_skip_proof(&mut grinder, &matrices, binding, spartan)?
            }
        };
        grinder.finish()?;
        terminal_claim
    };

    let step4_scope = crate::utils::prof::scope("step4:bitify_verify");
    let table = spec.block_table();
    let (opening, bridge_digest) = {
        let _scope = crate::utils::prof::scope(scopes.bitify_verifier);
        bitify_and_bind(spec, &matrices, binding, &terminal_claim, &table, *q, arith)?
    };
    check_boundary(
        transcript,
        domains.terminal_grinding,
        security.terminal_grinding_bits,
        messages.terminal_nonce,
    )?;
    drop(step4_scope);

    Ok(VerifiedPrefix {
        opening,
        bridge_digest,
        prime,
        table,
    })
}

/// Runs the relation's Spartan kernel on the witness tables.
fn run_kernel<C, T: Transcript>(
    transcript: &mut T,
    kernel: Kernel,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, C>,
    binding: &[u8; 32],
    witness: PiopWitness<'_>,
) -> Result<(SpartanProof, ScaledMleEvaluationClaim<SpartanF2zField>), ProtocolError>
where
    C: SpartanMatrixCoefficient<SpartanF2zField> + RawMontyCoefficient,
{
    let (spartan, claim) = match (kernel, witness) {
        (
            Kernel::UnivariateSkip { skip_vars },
            PiopWitness::Native {
                az,
                bz,
                cz,
                assignment,
            },
        ) => {
            let products = NativeProducts {
                az: &az,
                bz: &bz,
                cz: &cz,
            };
            let (proof, claim) = prove_spartan_piop_native_u64_with_univariate_skip_borrowed(
                transcript, matrices, binding, products, assignment, skip_vars,
            )?;
            (SpartanProof::UnivariateSkip(proof), claim)
        }
        (Kernel::UnivariateSkip { .. }, _) => return Err(ProtocolError::UnsupportedKernel),
        (
            Kernel::Plain,
            PiopWitness::Native {
                az,
                bz,
                cz,
                assignment,
            },
        ) => {
            let products = NativeProducts {
                az: &az,
                bz: &bz,
                cz: &cz,
            };
            let (proof, claim) = prove_spartan_piop_native_u64_borrowed(
                transcript, matrices, binding, products, assignment,
            )?;
            (SpartanProof::Plain(proof), claim)
        }
        (
            Kernel::Plain,
            PiopWitness::NativeOwned {
                products,
                assignment,
                strategy,
            },
        ) => {
            let (proof, claim) = prove_spartan_piop_native_u64_with_strategy(
                transcript, matrices, binding, products, assignment, strategy,
            )?;
            (SpartanProof::Plain(proof), claim)
        }
        (
            Kernel::Plain,
            PiopWitness::RawProductsNative {
                products,
                assignment,
            },
        ) => {
            let (proof, claim) = prove_spartan_piop_raw_products_native_assignment(
                transcript, matrices, binding, products, assignment,
            )?;
            (SpartanProof::Plain(proof), claim)
        }
        (Kernel::Plain, PiopWitness::RawProductsRaw { products, witness }) => {
            let (proof, claim) = prove_spartan_piop_raw_products_raw_witness(
                transcript, matrices, binding, products, witness,
            )?;
            (SpartanProof::Plain(proof), claim)
        }
        (
            Kernel::Plain,
            PiopWitness::Field {
                products,
                assignment,
                strategy,
            },
        ) => {
            let (proof, claim) = prove_spartan_piop_with_strategy(
                transcript, matrices, binding, products, assignment, strategy,
            )?;
            (SpartanProof::Plain(proof), claim)
        }
    };
    Ok((spartan, claim))
}

/// The proof's Spartan shape must be the one the relation's kernel produces
/// (checked before any transcript operation).
fn check_proof_kernel(kernel: Kernel, spartan: &SpartanProof) -> Result<(), ProtocolError> {
    let expected = match kernel {
        Kernel::Plain => 0,
        Kernel::UnivariateSkip { skip_vars } => skip_vars as u8,
    };
    let actual = match spartan {
        SpartanProof::Plain(_) => 0,
        SpartanProof::UnivariateSkip(proof) => proof.outer.skip.skip_vars,
    };
    if expected != actual {
        return Err(ProtocolError::UnexpectedUnivariateSkipVariables { expected, actual });
    }
    Ok(())
}

/// Bitifies the terminal claim and computes the bridge digest.
#[allow(clippy::too_many_arguments)]
fn bitify_and_bind<S: RelationSpec>(
    spec: &S,
    matrices: &PreparedConstraintMatrices<SpartanF2zField, S::Coefficient>,
    binding: &[u8; 32],
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    table: &BlockTable,
    q: u128,
    arith: &ProjArith,
) -> Result<(BitifiedClaim, [u8; 32]), ProtocolError> {
    let opening = bitify::bitify(
        terminal_claim,
        spec.committed_layout(),
        spec.gate_vars(),
        table,
        q,
        arith,
    )?;
    let digest = bitify::bridge_digest(
        spec.domains().bitified_claim,
        binding,
        matrices.field_modulus_encoding(),
        matrices.digest(),
        q,
        |hasher| spec.hash_bridge_constants(hasher),
        terminal_claim,
        &opening,
    )?;
    Ok((opening, digest))
}

/// Uniform per-draw PIOP grinding difficulty: the maximum requirement of
/// any single drawn challenge.
pub fn piop_wrap_bits(security: &IopSecurityParams) -> u32 {
    security
        .initial_grinding_bits
        .max(security.piop_round_grinding_bits)
}

/// One grinding boundary at round index 0 (no bytes at difficulty 0).
pub fn grind_boundary<T: Transcript>(
    transcript: &mut T,
    domain: &[u8],
    bits: u32,
) -> Result<u64, GrindingError> {
    if bits == 0 {
        return Ok(0);
    }
    grind_and_absorb_in_domain(transcript, domain, 0, bits)
}

/// Verifier twin of [`grind_boundary`].
pub fn check_boundary<T: Transcript>(
    transcript: &mut T,
    domain: &[u8],
    bits: u32,
    nonce: u64,
) -> Result<(), GrindingError> {
    if bits == 0 {
        if nonce != 0 {
            return Err(GrindingError::InvalidNonce { nonce, bits });
        }
        return Ok(());
    }
    verify_and_absorb_in_domain(transcript, domain, 0, bits, nonce)
}

/// The sampled Step-2 prime with its runtime field configuration and
/// canonical arithmetic.
pub type RuntimePrime = (
    u128,
    usize,
    <SpartanF2zField as PrimeField>::Config,
    ProjArith,
);

/// Samples the Step-2 prime from the profile interval under the relation's
/// domain and builds its runtime field configuration and arithmetic.
pub fn sample_mod_q(
    transcript: &mut impl Transcript,
    domain: &[u8],
    security: &IopSecurityParams,
) -> Result<RuntimePrime, ProtocolError> {
    absorb_spartan_message(transcript, b"prime-domain", domain);
    absorb_spartan_message(
        transcript,
        b"prime-min",
        &security.projection_min.to_le_bytes(),
    );
    absorb_spartan_message(
        transcript,
        b"prime-max",
        &security.projection_max.to_le_bytes(),
    );
    let q = sample_prime_in_interval(transcript, security.projection_min, security.projection_max)?;
    absorb_spartan_message(transcript, b"prime-q", &q.to_le_bytes());
    runtime_field(q)
}

/// The runtime field configuration and canonical arithmetic of `q`.
pub fn runtime_field(q: u128) -> Result<RuntimePrime, ProtocolError> {
    let config = SpartanF2zField::make_cfg(&Uint::from(q))
        .map_err(|_| ProtocolError::UnsupportedFieldModulus)?;
    SpartanF2zField::validate_config(&config).map_err(|_| ProtocolError::UnsupportedFieldModulus)?;
    let q_bits = (u128::BITS - q.leading_zeros()) as usize;
    Ok((q, q_bits, config, ProjArith::new(q)))
}

/// The committed bit rows must have exactly the layout's shape.
pub fn validate_bit_rows(p: &IntegerMatrixLayout, rows: &[Vec<u64>]) -> Result<(), ProtocolError> {
    let row_count = checked_pow2(p.row_vars)?;
    let col_count = checked_pow2(p.col_vars)?;
    let row_bits = row_count
        .checked_mul(p.word_bits)
        .ok_or(ProtocolError::InvalidBitRows)?;
    if row_bits % u64::BITS as usize != 0 || rows.len() != col_count {
        return Err(ProtocolError::InvalidBitRows);
    }
    let words_per_col = row_bits / u64::BITS as usize;
    if rows.iter().any(|row| row.len() != words_per_col) {
        return Err(ProtocolError::InvalidBitRows);
    }
    Ok(())
}

/// The prover and verifier Ligerito configurations must agree and fit the
/// layout.
pub fn validate_config_pair(
    p: &IntegerMatrixLayout,
    pc: &LigProverConfig,
    vc: &LigVerifierConfig,
) -> Result<(), ProtocolError> {
    let m_p = packed_variables(p)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(ProtocolError::InvalidF2zParameters);
    };
    if log_inv_rate == 0
        || pc.initial_k >= m_p
        || pc.initial_k != vc.initial_k
        || pc.log_inv_rates != vc.log_inv_rates
        || pc.recursive_steps != vc.recursive_steps
        || pc.initial_log_msg_cols != vc.initial_log_msg_cols
        || pc.initial_log_num_interleaved != vc.initial_log_num_interleaved
        || pc.recursive_log_msg_cols != vc.recursive_log_msg_cols
        || pc.recursive_ks != vc.recursive_ks
        || pc.queries != vc.queries
        || pc.grinding_bits != vc.grinding_bits
        || pc.fold_grinding_bits != vc.fold_grinding_bits
        || pc.ood_samples != vc.ood_samples
        || pc.merkle_hash != vc.merkle_hash
    {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    Ok(())
}

/// The commitment must have been produced under the prepared configuration.
pub fn validate_commitment(
    p: &IntegerMatrixLayout,
    commitment: &Commitment,
    pc: &LigProverConfig,
) -> Result<(), ProtocolError> {
    let m_p = packed_variables(p)?;
    let Some(&log_inv_rate) = pc.log_inv_rates.first() else {
        return Err(ProtocolError::InvalidF2zParameters);
    };
    let params = &commitment.params;
    if params.m != m_p + LOG_PACKING
        || params.log_inv_rate != log_inv_rate
        || params.log_batch_size != pc.initial_k
        || params.profile != LigeritoProfile::default()
        || params.merkle_hash != pc.merkle_hash
    {
        return Err(ProtocolError::CommitmentConfigMismatch);
    }
    Ok(())
}

/// The packed-word variable count of the committed tensor, checked against
/// the shared packing rule.
pub fn packed_variables(p: &IntegerMatrixLayout) -> Result<usize, ProtocolError> {
    if !p.word_bits.is_power_of_two() || p.word_bits > u128::BITS as usize {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    let row_bit_vars = p
        .row_vars
        .checked_add(p.word_bits.trailing_zeros() as usize)
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    let expected = row_bit_vars
        .checked_sub(LOG_PACKING)
        .and_then(|folded| folded.checked_add(p.col_vars))
        .ok_or(ProtocolError::InvalidF2zParameters)?;
    if packed_vars(p) != expected {
        return Err(ProtocolError::InvalidF2zParameters);
    }
    Ok(expected)
}

pub fn checked_pow2(exponent: usize) -> Result<usize, ProtocolError> {
    let exponent = u32::try_from(exponent).map_err(|_| ProtocolError::InvalidF2zParameters)?;
    1_usize
        .checked_shl(exponent)
        .ok_or(ProtocolError::InvalidF2zParameters)
}

/// The GF(2^128) generator every opening runs against.
pub fn f2z_generator() -> BinaryFieldGF128 {
    static GENERATOR: OnceLock<BinaryFieldGF128> = OnceLock::new();
    *GENERATOR.get_or_init(crate::pcs::smallest_generator)
}

/// The PCS-only terminal-opening benchmark path: the F2Z opening of an
/// externally supplied terminal claim at the fixed comparison field, with
/// relation projection, profile derivation and statement binding excluded
/// from every timer.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod terminal {
    use super::*;
    use crate::pcs::{FQ_BITS, FQ_MOD};

    /// How the terminal-opening statement binds the relation and commitment.
    #[derive(Clone, Copy)]
    pub enum Binding<S> {
        /// The relation's paper assignment binding (profile parameters and
        /// Ligerito configuration included).
        Paper,
        /// A relation-provided digest of the layout and commitment alone.
        Custom(fn(&S, &Commitment) -> Result<[u8; 32], ProtocolError>),
    }

    /// What the terminal-opening statement frame carries under its tag.
    #[derive(Clone, Copy)]
    pub enum StatementPayload {
        /// The bridge digest of the bitified claim (the u32 path).
        BridgeDigest,
        /// The assignment binding, followed by a `relation_tag` frame with
        /// the relation digest (the BabyBear path).
        AssignmentBinding { relation_tag: &'static [u8] },
    }

    /// Setup-once context retaining only the public digests and configs.
    pub struct PreparedTerminalOpening<S: RelationSpec> {
        spec: S,
        params: IntegerMatrixLayout,
        security: IopSecurityParams,
        ligerito: ResolvedLigerito,
        binding: Binding<S>,
        assignment_binding: [u8; 32],
        relation_modulus_encoding: Box<[u8]>,
        relation_digest: [u8; 32],
        statement_tag: &'static [u8],
        payload: StatementPayload,
    }

    impl<S: RelationSpec> PreparedTerminalOpening<S> {
        pub const fn ligerito_configuration(&self) -> &ResolvedLigerito {
            &self.ligerito
        }

        pub const fn security(&self) -> &IopSecurityParams {
            &self.security
        }

        pub const fn params(&self) -> &IntegerMatrixLayout {
            &self.params
        }

        fn statement_binding(&self, commitment: &Commitment) -> Result<[u8; 32], ProtocolError> {
            match self.binding {
                Binding::Paper => self.spec.assignment_binding(
                    commitment,
                    &self.security,
                    self.ligerito.prover(),
                ),
                Binding::Custom(binding) => binding(&self.spec, commitment),
            }
        }

        fn validate_commitment(&self, commitment: &Commitment) -> Result<(), ProtocolError> {
            validate_commitment(&self.params, commitment, self.ligerito.prover())?;
            if self.statement_binding(commitment)? != self.assignment_binding {
                return Err(ProtocolError::PreparedOpeningCommitmentMismatch);
            }
            Ok(())
        }

        fn bind_claim(
            &self,
            transcript: &mut impl Transcript,
            terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
        ) -> Result<(BitifiedClaim, [u8; 32], BlockTable), ProtocolError> {
            let arith = ProjArith::new(FQ_MOD);
            let table = self.spec.block_table();
            let opening = bitify::bitify(
                terminal_claim,
                self.params,
                self.spec.gate_vars(),
                &table,
                FQ_MOD,
                &arith,
            )?;
            let bridge_digest = bitify::bridge_digest(
                self.spec.domains().bitified_claim,
                &self.assignment_binding,
                &self.relation_modulus_encoding,
                &self.relation_digest,
                FQ_MOD,
                |hasher| self.spec.hash_bridge_constants(hasher),
                terminal_claim,
                &opening,
            )?;
            match self.payload {
                StatementPayload::BridgeDigest => {
                    absorb_spartan_message(transcript, self.statement_tag, &bridge_digest);
                }
                StatementPayload::AssignmentBinding { relation_tag } => {
                    absorb_spartan_message(transcript, self.statement_tag, &self.assignment_binding);
                    absorb_spartan_message(transcript, relation_tag, &self.relation_digest);
                }
            }
            self.ligerito.bind(transcript);
            Ok((opening, bridge_digest, table))
        }
    }

    /// Prepares the fixed-q, PCS-only terminal-opening context.
    pub fn prepare<S: RelationSpec + Clone>(
        prepared: &PreparedRelation<S>,
        commitment: &Commitment,
        statement_tag: &'static [u8],
        payload: StatementPayload,
        binding: Binding<S>,
    ) -> Result<PreparedTerminalOpening<S>, ProtocolError> {
        let config = SpartanF2zField::make_cfg(&Uint::from(FQ_MOD))
            .map_err(|_| ProtocolError::UnsupportedFieldModulus)?;
        let matrices = PreparedConstraintMatrices::<SpartanF2zField, S::Coefficient>::from_skeleton(
            prepared.skeleton(),
            &config,
        )
        .map_err(SpartanError::from)?;
        let params = prepared.params();
        validate_commitment(&params, commitment, prepared.ligerito_configuration().prover())?;
        let mut terminal = PreparedTerminalOpening {
            spec: prepared.layout().clone(),
            params,
            security: prepared.security().clone(),
            ligerito: prepared.ligerito_configuration().clone(),
            binding,
            assignment_binding: [0; 32],
            relation_modulus_encoding: matrices.field_modulus_encoding().into(),
            relation_digest: *matrices.digest(),
            statement_tag,
            payload,
        };
        terminal.assignment_binding = terminal.statement_binding(commitment)?;
        Ok(terminal)
    }

    /// Commits already-materialized bit rows with the exact configuration
    /// retained by the PCS-only context.
    pub fn commit<S: RelationSpec>(
        prepared: &PreparedTerminalOpening<S>,
        rows: Vec<Vec<u64>>,
    ) -> Result<FlockCommitHint, ProtocolError> {
        validate_bit_rows(&prepared.params, &rows)?;
        let hint = commit_rs_ligerito_rows(&prepared.params, rows, prepared.ligerito.prover());
        prepared.validate_commitment(&hint.commitment)?;
        Ok(hint)
    }

    /// Proves one already-derived terminal assignment-MLE claim, with all
    /// Spartan work deliberately outside the benchmark boundary.
    pub fn prove<T: Transcript + Send, S: RelationSpec>(
        transcript: &mut T,
        prepared: &PreparedTerminalOpening<S>,
        hint: &FlockCommitHint,
        terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    ) -> Result<IntEvalRsLigModQProof, ProtocolError> {
        prepared.validate_commitment(&hint.commitment)?;
        validate_bit_rows(&prepared.params, hint.rows())?;
        let (opening, bridge_digest, table) = prepared.bind_claim(transcript, terminal_claim)?;
        let ood = bind_prover_ood(transcript, hint, prepared.security.ood);
        let chunks = bitify::prepare_chunks(&opening, &table, FQ_BITS, &ProjArith::new(FQ_MOD))?;
        if chunks.len() != 1 {
            return Err(ProtocolError::MultiChunkRuntimeWeights);
        }
        prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            transcript,
            prepared.spec.domains().opening,
            hint,
            &prepared.params,
            &chunks,
            &bridge_digest,
            FQ_BITS,
            f2z_generator(),
            prepared.spec.opener_grinding_bits(&prepared.security),
            ood,
            prepared.ligerito.prover(),
        )
        .map_err(ProtocolError::F2z)
    }

    /// Verifies the PCS-only terminal opening from public data alone.
    pub fn verify<T: Transcript + Send, S: RelationSpec>(
        transcript: &mut T,
        prepared: &PreparedTerminalOpening<S>,
        commitment: &Commitment,
        terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
        proof: &IntEvalRsLigModQProof,
    ) -> Result<(), ProtocolError> {
        prepared.validate_commitment(commitment)?;
        let (opening, bridge_digest, table) = prepared.bind_claim(transcript, terminal_claim)?;
        let ood = bind_verifier_ood(
            transcript,
            packed_variables(&prepared.params)?,
            prepared.security.ood,
            proof.ood.as_ref(),
        )
        .map_err(ProtocolError::F2z)?;
        let arith = ProjArith::new(FQ_MOD);
        let chunks = bitify::prepare_chunks(&opening, &table, FQ_BITS, &arith)?;
        if chunks.len() != 1 {
            return Err(ProtocolError::MultiChunkRuntimeWeights);
        }
        let col_weights = bitify::column_weights(&opening, &arith)?;
        crate::ligerito_flock::verify_mle_eval_mod_q_ligerito_with_weight_chunks(
            transcript,
            prepared.spec.domains().opening,
            commitment,
            proof,
            &prepared.params,
            &chunks,
            &col_weights,
            &bridge_digest,
            f2z_generator(),
            opening.claimed,
            FQ_BITS,
            prepared.spec.opener_grinding_bits(&prepared.security),
            ood,
            prepared.ligerito.verifier(),
        )
        .map_err(ProtocolError::F2z)
    }
}
