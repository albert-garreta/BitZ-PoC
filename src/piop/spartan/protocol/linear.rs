//! The linear-batching PIOP: relations whose constraints are linear in a
//! derived bit grid `h = M·f`, possibly plus product slots `h[l] (h[p] −
//! h[m])` on their output side (the SHA-256 compression relation, whose
//! `Ch` is one slot per round and bit). There is no nonlinear outer
//! sumcheck: the verifier draws a local-row point `ξ` and an instance point
//! `η`, batches the public input/output equalities and the shared constant
//! behind two more challenges, and the collapsed relation is one rank-one
//! functional over the assignment grid once the product sumcheck
//! ([`super::product`]) has reduced the slots. A power-of-two batch in the
//! product layout opens that functional directly; otherwise a quadratic
//! inner sumcheck over the assignment domain first reduces it to a point
//! (the legacy path).
//!
//! The steps, on one transcript:
//!
//! 1. the relation's statement frames, the opener policy digest, Round 0;
//! 2. the initial grinding boundary, the runtime prime, the runtime
//!    relation frames;
//! 3. `ξ`, `η`, the terminal boundary, the constant-one batch challenge,
//!    the public-IO batch challenges; the relation's batching; for a
//!    relation with product slots, the product sumcheck and the draw that
//!    folds its three terminal claims into one rank-one functional at its
//!    instance point;
//! 4. (legacy) the inner sumcheck; the opening claim frame;
//! 5. the opening of the derived grid: virtual (the transpose through the
//!    map), or through a block-layout commitment ([`crate::bitz::block`]).

use super::bitz_opener::{self, BitZOpeningProof, opening_params, opening_shape, pcs_from_config};
use super::product::{
    InstanceBits, LinearProductGrinding, ProductSlots, ProductSumcheckProof,
    prove_product_sumcheck, reduced_local_weights, verify_product_sumcheck,
};
use crate::bitz::{BlockRepeatedMap, BlockStatement, LinearClaim, VirtualStatement};
use crate::sumcheck::boundary::{ProverGrindingRoundBoundary, VerifierGrindingRoundBoundary};
use crate::sumcheck::inner::{
    packed::{PackedInput, Sha256InnerGrinding},
    prove_inner_sumcheck,
};
use field::RingOps;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};

use {
    crate::{
        ligerito::packed_vars,
        ligerito_flock::{
            FlockCommitHint, LigeritoStatementConfig, ResolvedLigerito,
            bind_prover_ood, bind_verifier_ood,
        },
        pcs::IntegerMatrixLayout,
        transcript::traits::Transcript,
    },
    circuit::linear_map::binary::VirtualMap,
};

use super::{
    super::{
        absorb_field_elements, absorb_spartan_message,
        sha256::inner_sumcheck::{SHA256_INNER_PREFIX_MAX_VARS, Sha256InnerBitSource},
        squeeze_field,
        sumcheck::SumcheckProof,
    },
    BindingHasher, FieldConfig, ProtocolError, SpartanBitzField, check_boundary,
    grind_boundary,
};
use crate::piop::spartan::profile::IopSecurityParams;
use crate::poly::mle::FactoredMultilinearExtension;

/// The transcript domain strings of one linear relation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LinearDomains {
    pub initial_grinding: &'static [u8],
    /// The terminal boundary before the batching challenges.
    pub terminal_grinding: &'static [u8],
    pub local_point: &'static [u8],
    pub instance_point: &'static [u8],
    pub constant_one: &'static [u8],
    pub public_io: &'static [u8],
    /// Tag and digest domain of the opening-claim frame.
    pub claim_tag: &'static [u8],
    pub claim_domain: &'static [u8],
    /// Extra bytes after the digest domain on the product path.
    pub claim_variant: Option<&'static [u8]>,
}

/// The relation's batching of its collapsed constraints: the initial claim
/// `μ`, the factored coefficient table `V` of the legacy inner sumcheck,
/// and its closed-form evaluation.
pub(crate) trait LinearBatching {
    fn initial_claim(&self) -> &SpartanBitzField;

    fn factored_matrix_mle(
        &self,
        config: &FieldConfig,
    ) -> Result<FactoredMultilinearExtension<'_, SpartanBitzField>, ProtocolError>;

    fn evaluate(
        &self,
        point: &[SpartanBitzField],
        config: &FieldConfig,
    ) -> Result<SpartanBitzField, ProtocolError>;

    /// The collapsed local coefficients `β`, one per local assignment column.
    fn local_coefficients(&self) -> &[SpartanBitzField];

    /// `κ = Kᵀ eq(·, ξ)` padded to a power of two, when the relation has
    /// product slots.
    fn product_slot_weights(&self) -> Option<&[SpartanBitzField]> {
        None
    }

    /// The terms of `β` beyond the collapsed relation `Cᵀ eq(·, ξ)` (the
    /// constant-one and public batching), as `(column, coefficient)` pairs.
    /// When the witness satisfies the relation, `⟨β, h_i⟩` is their value
    /// on `h_i` minus `Σ_s κ_s slot_s(h_i)`, which the product prover may
    /// use instead of the dense `⟨β, h_i⟩`.
    fn product_free_coefficients(&self) -> Option<&[(u32, SpartanBitzField)]> {
        None
    }

    /// The same batching at another instance point, with other local
    /// coefficients and claim (the product sumcheck's reduced claim).
    fn reanchored(
        &self,
        instance_point: &[SpartanBitzField],
        local_coefficients: Vec<SpartanBitzField>,
        claim: SpartanBitzField,
        config: &FieldConfig,
    ) -> Result<Self, ProtocolError>
    where
        Self: Sized;
}

/// The rank-one functional the virtual opening discharges: the canonical
/// row weight of every opened row on demand, the clear column weights and
/// the claimed value.
pub(crate) struct OpeningClaim<'a> {
    pub rows: Box<dyn Fn(usize) -> Option<u128> + Sync + 'a>,
    pub cols: Vec<u128>,
    pub claimed: u128,
}



/// The static description of one linear relation.
pub(crate) trait LinearRelationSpec: Sync {
    type Statement: ?Sized + Sync;
    type Witness: ?Sized + Sync;
    type Map: VirtualMap;
    type Batching: LinearBatching;

    fn domains(&self) -> &'static LinearDomains;
    fn security(&self) -> &IopSecurityParams;
    fn ligerito(&self) -> Result<&ResolvedLigerito, ProtocolError>;

    /// The committed source tensor.
    fn source_layout(&self) -> &IntegerMatrixLayout;
    /// The derived grid the opening runs against.
    fn opened_layout(&self) -> &IntegerMatrixLayout;
    fn map(&self) -> &Self::Map;
    /// Whether the batch opens its rank-one claim directly (no inner sumcheck).
    fn product_layout(&self) -> bool;

    fn local_vars(&self) -> usize;
    fn instance_vars(&self) -> usize;
    fn public_io_count(&self) -> usize;

    fn validate_statement(&self, statement: &Self::Statement) -> Result<(), ProtocolError>;
    fn validate_opener_config(
        &self,
        config: &dyn LigeritoStatementConfig,
    ) -> Result<(), ProtocolError>;
    fn validate_witness(
        &self,
        statement: &Self::Statement,
        witness: &Self::Witness,
        hint: &FlockCommitHint,
    ) -> Result<(), ProtocolError>;

    /// The statement frames in absorption order, and the assignment binding
    /// the opening claim later cites.
    fn statement_frames(
        &self,
        statement: &Self::Statement,
        commitment: &Commitment,
        config: &dyn LigeritoStatementConfig,
    ) -> Result<(Vec<(&'static [u8], Vec<u8>)>, [u8; 32]), ProtocolError>;

    fn runtime_prime<T: Transcript>(
        &self,
        transcript: &mut T,
    ) -> Result<field::FpCtx<2>, ProtocolError>;

    /// The frames bound after the prime draw.
    fn runtime_relation_frames(&self, config: &FieldConfig) -> Vec<(&'static [u8], Vec<u8>)>;

    #[allow(clippy::too_many_arguments)]
    fn batching(
        &self,
        statement: &Self::Statement,
        local_point: &[SpartanBitzField],
        instance_point: &[SpartanBitzField],
        slot_weights: Vec<SpartanBitzField>,
        public_io_batch: SpartanBitzField,
        constant_one: SpartanBitzField,
        reducer: &field::FpCtx<2>,
        config: &FieldConfig,
    ) -> Result<Self::Batching, ProtocolError>;

    /// The product path's rank-one claim.
    fn product_claim<'a>(
        &'a self,
        batching: &'a Self::Batching,
        prime: &'a field::FpCtx<2>,
    ) -> Result<OpeningClaim<'a>, ProtocolError>;

    /// The legacy path's claim from the inner sumcheck's terminal point.
    fn inner_claim<'a>(
        &'a self,
        point: &[SpartanBitzField],
        coefficient_evaluation: &SpartanBitzField,
        final_claim: SpartanBitzField,
        prime: &'a field::FpCtx<2>,
    ) -> Result<OpeningClaim<'a>, ProtocolError>;

    /// The assignment grid bits the legacy inner sumcheck multiplies by.
    fn inner_bits<'a>(
        &'a self,
        witness: &'a Self::Witness,
    ) -> Result<Box<dyn Sha256InnerBitSource + 'a>, ProtocolError>;

    /// The derived rows the opening runs against.
    fn opened_rows<'a>(&self, witness: &'a Self::Witness) -> Result<&'a [Vec<u64>], ProtocolError>;

    /// The relation's local product slots, if its rows carry any.
    fn product_slots(&self) -> Option<ProductSlots<'_>> {
        None
    }

    /// Prover-side per-instance access to the opened assignment's bits, for
    /// the product sumcheck.
    fn instance_bits<'a>(
        &'a self,
        _witness: &'a Self::Witness,
    ) -> Result<Box<dyn InstanceBits + 'a>, ProtocolError> {
        Err(ProtocolError::UnsupportedKernel)
    }

    /// The block map, when the source is committed in the block layout of
    /// `h = (Id ⊗ M) f` ([`crate::bitz::block`]) and opened through it; the
    /// virtual opening otherwise.
    fn block_map(&self) -> Option<&BlockRepeatedMap> {
        None
    }
}

/// A linear-relation proof: the PIOP (boundary nonces, the legacy inner
/// sumcheck, empty on the product path, and the product sumcheck for
/// relations with product slots) and the opening.
#[derive(Clone)]
pub struct LinearProof {
    piop: LinearPiopProof,
    bitz: BitZOpeningProof,
}

/// A linear relation's PIOP messages: [`LinearProof`] without its opening,
/// for a protocol that opens the claim together with another relation's
/// on one commitment.
#[derive(Clone)]
pub struct LinearPiopProof {
    pub initial_nonce: u64,
    pub inner: SumcheckProof<SpartanBitzField, 3>,
    pub inner_nonces: Vec<u64>,
    pub terminal_nonce: u64,
    pub product: Option<ProductSumcheckProof>,
}

impl LinearPiopProof {
    /// Payload bytes: the two boundary nonces, the inner round coefficients
    /// and their nonces, and the product sumcheck.
    pub fn bytes(&self) -> usize {
        2 * 8
            + 3 * 16 * self.inner.round_polynomials.len()
            + 8 * self.inner_nonces.len()
            + self.product.as_ref().map_or(0, ProductSumcheckProof::bytes)
    }
}

/// The PIOP's opening claim on the opened grid: the runtime prime, every
/// row's canonical weight, the column weights and the claimed value.
pub(crate) struct LinearOpeningClaim {
    pub prime: field::FpCtx<2>,
    pub rows: Vec<u128>,
    pub cols: Vec<u128>,
    pub claimed: u128,
}

impl LinearProof {
    pub const fn initial_nonce(&self) -> u64 {
        self.piop.initial_nonce
    }

    pub const fn inner(&self) -> &SumcheckProof<SpartanBitzField, 3> {
        &self.piop.inner
    }

    pub fn inner_nonces(&self) -> &[u64] {
        &self.piop.inner_nonces
    }

    pub const fn terminal_nonce(&self) -> u64 {
        self.piop.terminal_nonce
    }

    pub const fn bitz(&self) -> &BitZOpeningProof {
        &self.bitz
    }

    pub const fn product(&self) -> Option<&ProductSumcheckProof> {
        self.piop.product.as_ref()
    }

    pub fn product_mut(&mut self) -> Option<&mut ProductSumcheckProof> {
        self.piop.product.as_mut()
    }

    /// PIOP payload bytes ([`LinearPiopProof::bytes`]).
    pub fn piop_bytes(&self) -> usize {
        self.piop.bytes()
    }

    pub fn initial_nonce_mut(&mut self) -> &mut u64 {
        &mut self.piop.initial_nonce
    }

    pub fn inner_mut(&mut self) -> &mut SumcheckProof<SpartanBitzField, 3> {
        &mut self.piop.inner
    }

    pub fn inner_nonces_mut(&mut self) -> &mut Vec<u64> {
        &mut self.piop.inner_nonces
    }

    pub fn terminal_nonce_mut(&mut self) -> &mut u64 {
        &mut self.piop.terminal_nonce
    }

    pub fn bitz_mut(&mut self) -> &mut BitZOpeningProof {
        &mut self.bitz
    }
}

/// Prover-side options that do not move the transcript.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LinearProveOptions {
    /// The legacy inner prover's packed-prefix width.
    pub prefix_vars: usize,
}

/// One statement frame: its tag and payload.
pub(crate) fn frame(tag: &'static [u8], bytes: impl AsRef<[u8]>) -> (&'static [u8], Vec<u8>) {
    (tag, bytes.as_ref().to_vec())
}

fn absorb_frames<T: Transcript>(transcript: &mut T, frames: &[(&'static [u8], Vec<u8>)]) {
    for (tag, bytes) in frames {
        absorb_spartan_message(transcript, tag, bytes);
    }
}

fn challenge_point<T: Transcript>(
    transcript: &mut T,
    domain: &[u8],
    vars: usize,
    config: &FieldConfig,
) -> Result<Vec<SpartanBitzField>, ProtocolError> {
    absorb_spartan_message(transcript, b"challenge-domain", domain);
    Ok((0..vars)
        .map(|_| squeeze_field(transcript, config))
        .collect::<Result<Vec<_>, _>>()?)
}

/// The batching challenges after the terminal boundary.
struct BatchChallenges {
    constant_one: SpartanBitzField,
    slot_weights: Vec<SpartanBitzField>,
    public_io_batch: SpartanBitzField,
}

fn batch_challenges<T: Transcript, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    config: &FieldConfig,
) -> Result<BatchChallenges, ProtocolError> {
    let domains = spec.domains();
    let constant_one = challenge_point(transcript, domains.constant_one, 1, config)?
        .pop()
        .expect("one challenge");
    let mut public = challenge_point(
        transcript,
        domains.public_io,
        spec.public_io_count() + 1,
        config,
    )?;
    let public_io_batch = public.pop().expect("the batch weight");
    Ok(BatchChallenges {
        constant_one,
        slot_weights: public,
        public_io_batch,
    })
}

/// The opening-claim digest:
///
/// `domain ‖ [variant] ‖ binding ‖ |ξ| ξ ‖ |η| η ‖ [|r| r ‖ V(r)] ‖ α₀ ‖ |λ| λ ‖ α_pub ‖ |rows| rows ‖ |cols| cols ‖ claimed`
#[allow(clippy::too_many_arguments)]
fn claim_digest(
    domains: &LinearDomains,
    binding: &[u8; 32],
    local_point: &[SpartanBitzField],
    instance_point: &[SpartanBitzField],
    assignment: Option<(&[SpartanBitzField], &SpartanBitzField)>,
    challenges: &BatchChallenges,
    claim: &OpeningClaim<'_>,
    rows: &[u128],
    field_config: &FieldConfig,
) -> Result<[u8; 32], ProtocolError> {
    let mut hasher = BindingHasher::new();
    hasher.bytes(domains.claim_domain);
    if assignment.is_none() {
        if let Some(variant) = domains.claim_variant {
            hasher.bytes(variant);
        }
    }
    hasher.bytes(binding);
    for point in [local_point, instance_point] {
        hasher.usize(point.len())?;
        for coordinate in point {
            hasher.element(coordinate, field_config);
        }
    }
    if let Some((point, evaluation)) = assignment {
        hasher.usize(point.len())?;
        for coordinate in point {
            hasher.element(coordinate, field_config);
        }
        hasher.element(evaluation, field_config);
    }
    hasher.element(&challenges.constant_one, field_config);
    hasher.usize(challenges.slot_weights.len())?;
    for coordinate in &challenges.slot_weights {
        hasher.element(coordinate, field_config);
    }
    hasher.element(&challenges.public_io_batch, field_config);
    hasher.usize(rows.len())?;
    // Every row weight's little-endian bytes in order, as one string.
    let mut weights = vec![0u8; 16 * rows.len()];
    let write = |(out, weight): (&mut [u8], &u128)| out.copy_from_slice(&weight.to_le_bytes());
    #[cfg(feature = "parallel")]
    weights.par_chunks_mut(16).zip(rows).for_each(write);
    #[cfg(not(feature = "parallel"))]
    weights.chunks_mut(16).zip(rows).for_each(write);
    hasher.bytes(&weights);
    hasher.usize(claim.cols.len())?;
    for weight in &claim.cols {
        hasher.u128_le(*weight);
    }
    hasher.u128_le(claim.claimed);
    Ok(hasher.finalize())
}

/// The prover's input checks: the statement, the opener configuration, the
/// witness against the commitment, the inner prefix width.
pub(crate) fn check_linear_prover<S: LinearRelationSpec>(
    spec: &S,
    statement: &S::Statement,
    witness: &S::Witness,
    hint: &FlockCommitHint,
    pc: &LigProverConfig,
    options: LinearProveOptions,
) -> Result<(), ProtocolError> {
    spec.validate_statement(statement)?;
    spec.validate_opener_config(pc)?;
    spec.validate_witness(statement, witness, hint)?;
    if options.prefix_vars > SHA256_INNER_PREFIX_MAX_VARS {
        return Err(ProtocolError::InvalidInnerPrefix {
            actual: options.prefix_vars,
            max: SHA256_INNER_PREFIX_MAX_VARS,
        });
    }
    crate::ligerito_flock::validate_ligerito_commitment(&hint.commitment, pc)
        .map_err(ProtocolError::Bitz)?;
    let f_layout = spec.source_layout();
    if hint.commitment.params.m != f_layout.row_vars + f_layout.col_vars {
        return Err(ProtocolError::InvalidGeometry);
    }
    Ok(())
}

/// Binds the relation's statement frames and its opener configuration;
/// returns the assignment binding the opening claim cites.
pub(crate) fn bind_linear_statement<T: Transcript, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    commitment: &Commitment,
    config: &dyn LigeritoStatementConfig,
) -> Result<[u8; 32], ProtocolError> {
    let binding = {
        let _scope = tracing::info_span!("sha256:statement_bind").entered();
        let (frames, binding) = spec.statement_frames(statement, commitment, config)?;
        absorb_frames(transcript, &frames);
        binding
    };
    spec.ligerito()?.bind(transcript);
    Ok(binding)
}

/// Proves a linear relation against its committed source rows.
pub(crate) fn prove_linear<T: Transcript + Send, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    witness: &S::Witness,
    hint: &FlockCommitHint,
    pc: &LigProverConfig,
    options: LinearProveOptions,
) -> Result<LinearProof, ProtocolError> {
    check_linear_prover(spec, statement, witness, hint, pc, options)?;
    let binding = bind_linear_statement(transcript, spec, statement, &hint.commitment, pc)?;
    let ood = bind_prover_ood(transcript, hint, spec.security().ood);
    let (piop, claim) = prove_linear_piop(transcript, spec, statement, witness, binding, options)?;

    let bitz = {
        let _step5 = tracing::info_span!("step5:open_prove").entered();
        let _scope = tracing::info_span!("sha256:bitz_prove").entered();
        let f_layout = spec.source_layout();
        let params = opening_params(spec.opened_layout(), claim.prime.modulus_u128())?;
        let committed = opening_shape(f_layout)?;
        let native_claim = LinearClaim::new(&params, claim.rows, claim.cols, claim.claimed)
            .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))?;
        let pcs = pcs_from_config(&committed, pc)?.with_native_policy(spec.security().native_policy()?);
        match spec.block_map() {
            Some(map) => {
                let opening = BlockStatement::new(params, committed, map, &native_claim)
                    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ block statement: {error:?}")))?;
                bitz_opener::prove_block_opening(
                    transcript, &opening, &pcs, hint, spec.opened_rows(witness)?, ood,
                )?
            }
            None => {
                let opening = VirtualStatement::new(params, committed, spec.map(), &native_claim)
                    .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ statement: {error:?}")))?;
                bitz_opener::prove_virtual_opening(
                    transcript, &opening, &pcs, hint, spec.opened_rows(witness)?, ood,
                )?
            }
        }
    };

    Ok(LinearProof { piop, bitz })
}

/// The PIOP after the statement and Round 0: the initial boundary, the
/// runtime prime, the batching, the product (or legacy inner) sumcheck and
/// the bound opening claim, which it returns for the opening.
pub(crate) fn prove_linear_piop<T: Transcript + Send, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    witness: &S::Witness,
    binding: [u8; 32],
    options: LinearProveOptions,
) -> Result<(LinearPiopProof, LinearOpeningClaim), ProtocolError> {
    let security = spec.security();
    let domains = spec.domains();

    // Commit first, then derive the one runtime prime. The exact signed
    // relation stays q-independent; only its one local-row collapse is
    // performed in the sampled field.
    let step2_scope = tracing::info_span!("step2:project_prove").entered();
    let initial_nonce = {
        let _scope = tracing::info_span!("sha256:initial_grinding_prove").entered();
        grind_boundary(
            transcript,
            domains.initial_grinding,
            security.initial_grinding_bits,
        )?
    };
    let prime = {
        let _scope = tracing::info_span!("sha256:runtime_prime_sample_prover").entered();
        spec.runtime_prime(transcript)?
    };
    absorb_frames(transcript, &spec.runtime_relation_frames(&prime));
    drop(step2_scope);

    // Collapse the local rows and form the product-structured residual.
    // Power-of-two batches open it directly; partial batches use the legacy
    // assignment-domain sumcheck below. There is no nonlinear outer sumcheck.
    let step3_scope = tracing::info_span!("step3:piop_prove").entered();
    let config = &prime;
    let local_point = challenge_point(transcript, domains.local_point, spec.local_vars(), config)?;
    let instance_point = challenge_point(
        transcript,
        domains.instance_point,
        spec.instance_vars(),
        config,
    )?;
    let terminal_nonce = {
        let _scope = tracing::info_span!("sha256:public_batch_grinding_prove").entered();
        grind_boundary(
            transcript,
            domains.terminal_grinding,
            security.terminal_grinding_bits,
        )?
    };
    let challenges = batch_challenges(transcript, spec, config)?;
    let reducer = {
        let _scope = tracing::info_span!("sha256:reducer_init_prover").entered();
        crate::utils::delayed_reduction::prepare_field(config)
            .map_err(super::super::sumcheck::SumcheckError::from)
            .map_err(super::super::piop::SpartanError::from)?
    };
    let batching = {
        let _scope = tracing::info_span!("sha256:product_batch_prepare_prover").entered();
        spec.batching(
            statement,
            &local_point,
            &instance_point,
            challenges.slot_weights.clone(),
            challenges.public_io_batch.clone(),
            challenges.constant_one.clone(),
            &reducer,
            config,
        )?
    };
    let (batching, product) = match spec.product_slots() {
        None => (batching, None),
        Some(slots) => {
            let _scope = tracing::info_span!("sha256:product_sumcheck_prove").entered();
            let slot_weights = batching
                .product_slot_weights()
                .ok_or(ProtocolError::InvalidGeometry)?;
            let bits = spec.instance_bits(witness)?;
            let mut boundary = ProverGrindingRoundBoundary::<LinearProductGrinding>::with_round_offset(
                security.piop_round_grinding_bits,
                0,
            );
            let output = prove_product_sumcheck(
                transcript,
                config,
                &instance_point,
                batching.local_coefficients(),
                batching.product_free_coefficients(),
                slot_weights,
                slots,
                &*bits,
                batching.initial_claim(),
                &mut boundary,
            )?;
            let terminal = output.terminal.to_vec();
            let (reduced, value) = product_terminal_batch(
                transcript,
                config,
                &batching,
                slot_weights,
                slots,
                &output.slot_point,
                &terminal,
            )?;
            let batching = batching.reanchored(&output.instance_point, reduced, value, config)?;
            let proof = ProductSumcheckProof {
                rounds: output.rounds,
                nonces: boundary.into_nonces(),
                terminal,
            };
            (batching, Some(proof))
        }
    };

    let (inner, inner_nonces, claim, assignment) = if spec.product_layout() {
        drop(step3_scope);
        let step4_scope = tracing::info_span!("step4:bitify_prove").entered();
        let claim = {
            let _scope = tracing::info_span!("sha256:direct_opening_prepare_prover").entered();
            spec.product_claim(&batching, &prime)?
        };
        drop(step4_scope);
        (
            SumcheckProof {
                round_polynomials: Vec::new(),
            },
            Vec::new(),
            claim,
            None,
        )
    } else {
        let (inner, inner_nonces) = {
            let _scope = tracing::info_span!("sha256:spartan_inner_prove").entered();
            let factored = batching.factored_matrix_mle(config)?;
            let bits = spec.inner_bits(witness)?;
            {
                let coefficients = &factored;
                let mut boundary =
                    ProverGrindingRoundBoundary::<Sha256InnerGrinding>::with_round_offset(
                        security.piop_round_grinding_bits,
                        0,
                    );
                prove_inner_sumcheck(
                    config,
                    transcript,
                    batching.initial_claim().clone(),
                    PackedInput::new(
                        coefficients,
                        &*bits,
                        spec.opened_layout().row_vars + spec.opened_layout().col_vars,
                        coefficients.live_len(),
                        options.prefix_vars,
                    ),
                    (),
                    &mut boundary,
                )
                .map(|out| (out, boundary.into_nonces()))
            }
            .map_err(super::super::piop::SpartanError::from)?
        };
        if inner.final_claim
            != config.mul(
                &(inner.terminal_evaluations[0].clone()),
                &(&inner.terminal_evaluations[1]),
            )
        {
            return Err(ProtocolError::InvalidInnerTerminalClaim);
        }
        drop(step3_scope);
        let step4_scope = tracing::info_span!("step4:bitify_prove").entered();
        let claim = {
            let _scope = tracing::info_span!("sha256:opening_prepare_prover").entered();
            spec.inner_claim(
                &inner.point,
                &inner.terminal_evaluations[0],
                inner.final_claim.clone(),
                &prime,
            )?
        };
        drop(step4_scope);
        (
            inner.proof,
            inner_nonces,
            claim,
            Some((inner.point, inner.terminal_evaluations[0])),
        )
    };
    let rows = canonical_rows(spec.opened_layout(), &claim)?;
    {
        let _step4 = tracing::info_span!("step4:bitify_prove").entered();
        let _scope = tracing::info_span!("sha256:opening_claim_absorb_prover").entered();
        let digest = claim_digest(
            domains,
            &binding,
            &local_point,
            &instance_point,
            assignment
                .as_ref()
                .map(|(point, evaluation)| (point.as_slice(), evaluation)),
            &challenges,
            &claim,
            &rows,
            &config,
        )?;
        absorb_spartan_message(transcript, domains.claim_tag, &digest);
    }

    let OpeningClaim { cols, claimed, .. } = claim;
    Ok((
        LinearPiopProof { initial_nonce, inner, inner_nonces, terminal_nonce, product },
        LinearOpeningClaim { prime: prime.clone(), rows, cols, claimed },
    ))
}

/// The transcript domain of the draw that folds the product sumcheck's
/// three terminal claims into one.
const PRODUCT_TERMINAL_BATCH_DOMAIN: &[u8] = b"bitz/spartan-linear/product-terminal-batch/v1";

/// Absorbs the product sumcheck's terminal evaluations, draws `γ₁, γ₂`, and
/// returns the reduced claim's local weights and value.
fn product_terminal_batch<T: Transcript, Bt: LinearBatching>(
    transcript: &mut T,
    config: &FieldConfig,
    batching: &Bt,
    slot_weights: &[SpartanBitzField],
    slots: ProductSlots<'_>,
    slot_point: &[SpartanBitzField],
    terminal: &[SpartanBitzField],
) -> Result<(Vec<SpartanBitzField>, SpartanBitzField), ProtocolError> {
    absorb_field_elements(transcript, terminal, config);
    let gamma = challenge_point(
        transcript,
        PRODUCT_TERMINAL_BATCH_DOMAIN,
        terminal.len().saturating_sub(1),
        config,
    )?;
    reduced_local_weights(
        config,
        batching.local_coefficients(),
        slot_weights,
        slots,
        slot_point,
        terminal,
        &gamma,
    )
}

/// Verifies a linear-relation proof, re-deriving the prime from the bound
/// transcript.
pub(crate) fn verify_linear<T: Transcript + Send, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    commitment: &Commitment,
    proof: &LinearProof,
    vc: &LigVerifierConfig,
) -> Result<(), ProtocolError> {
    check_linear_verifier(spec, statement, commitment, &proof.piop, vc)?;
    let f_layout = spec.source_layout();
    let security = spec.security();
    let binding = bind_linear_statement(transcript, spec, statement, commitment, vc)?;
    let ood = bind_verifier_ood(
        transcript,
        packed_vars(f_layout),
        security.ood,
        proof.bitz.ood.as_ref(),
    )
    .map_err(ProtocolError::Bitz)?;
    let claim = verify_linear_piop(transcript, spec, statement, &proof.piop, binding)?;

    let _step5 = tracing::info_span!("step5:open_verify").entered();
    let _scope = tracing::info_span!("sha256:bitz_verify").entered();
    let params = opening_params(spec.opened_layout(), claim.prime.modulus_u128())?;
    let committed = opening_shape(f_layout)?;
    let native_claim = LinearClaim::new(&params, claim.rows, claim.cols, claim.claimed)
        .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ claim: {error:?}")))?;
    let pcs = pcs_from_config(&committed, vc)?.with_native_policy(security.native_policy()?);
    match spec.block_map() {
        Some(map) => {
            let opening = BlockStatement::new(params, committed, map, &native_claim)
                .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ block statement: {error:?}")))?;
            bitz_opener::verify_block_opening(transcript, &opening, &pcs, commitment, &proof.bitz, ood)
        }
        None => {
            let opening = VirtualStatement::new(params, committed, spec.map(), &native_claim)
                .map_err(|error| ProtocolError::LigeritoConfig(format!("BitZ statement: {error:?}")))?;
            bitz_opener::verify_virtual_opening(transcript, &opening, &pcs, commitment, &proof.bitz, ood)
        }
    }
}

/// The verifier's input checks: the statement, the opener configuration and
/// the commitment, the PIOP proof's shape.
pub(crate) fn check_linear_verifier<S: LinearRelationSpec>(
    spec: &S,
    statement: &S::Statement,
    commitment: &Commitment,
    proof: &LinearPiopProof,
    vc: &LigVerifierConfig,
) -> Result<(), ProtocolError> {
    spec.validate_statement(statement)?;
    spec.validate_opener_config(vc)?;
    crate::ligerito_flock::validate_ligerito_commitment(commitment, vc)
        .map_err(ProtocolError::Bitz)?;
    let f_layout = spec.source_layout();
    let opened = spec.opened_layout();
    let product_layout = spec.product_layout();
    let expected_inner_rounds = if product_layout {
        0
    } else {
        opened.row_vars + opened.col_vars
    };
    if commitment.params.m != f_layout.row_vars + f_layout.col_vars
        || proof.inner.round_polynomials.len() != expected_inner_rounds
        || (product_layout && !proof.inner_nonces.is_empty())
        || spec.product_slots().is_some() != proof.product.is_some()
    {
        return Err(ProtocolError::InvalidGeometry);
    }
    Ok(())
}

/// [`prove_linear_piop`]'s verifier: re-derives the prime and the batching,
/// checks the sumchecks and binds the opening claim, which it returns.
pub(crate) fn verify_linear_piop<T: Transcript + Send, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    proof: &LinearPiopProof,
    binding: [u8; 32],
) -> Result<LinearOpeningClaim, ProtocolError> {
    let opened = spec.opened_layout();
    let product_layout = spec.product_layout();
    let security = spec.security();
    let domains = spec.domains();

    let step2_scope = tracing::info_span!("step2:project_verify").entered();
    {
        let _scope = tracing::info_span!("sha256:initial_grinding_verify").entered();
        check_boundary(
            transcript,
            domains.initial_grinding,
            security.initial_grinding_bits,
            proof.initial_nonce,
        )?;
    }
    let prime = {
        let _scope = tracing::info_span!("sha256:runtime_prime_sample_verifier").entered();
        spec.runtime_prime(transcript)?
    };
    absorb_frames(transcript, &spec.runtime_relation_frames(&prime));
    drop(step2_scope);

    let step3_scope = tracing::info_span!("step3:piop_verify").entered();
    let config = &prime;
    let local_point = challenge_point(transcript, domains.local_point, spec.local_vars(), config)?;
    let instance_point = challenge_point(
        transcript,
        domains.instance_point,
        spec.instance_vars(),
        config,
    )?;
    {
        let _scope = tracing::info_span!("sha256:public_batch_grinding_verify").entered();
        check_boundary(
            transcript,
            domains.terminal_grinding,
            security.terminal_grinding_bits,
            proof.terminal_nonce,
        )?;
    }
    let challenges = batch_challenges(transcript, spec, config)?;
    let reducer = {
        let _scope = tracing::info_span!("sha256:reducer_init_verifier").entered();
        crate::utils::delayed_reduction::prepare_field(config)
            .map_err(super::super::sumcheck::SumcheckError::from)
            .map_err(super::super::piop::SpartanError::from)?
    };
    let batching = {
        let _scope = tracing::info_span!("sha256:product_batch_prepare_verifier").entered();
        spec.batching(
            statement,
            &local_point,
            &instance_point,
            challenges.slot_weights.clone(),
            challenges.public_io_batch.clone(),
            challenges.constant_one.clone(),
            &reducer,
            config,
        )?
    };
    let batching = match (spec.product_slots(), proof.product.as_ref()) {
        (Some(slots), Some(product)) => {
            let _scope = tracing::info_span!("sha256:product_sumcheck_verify").entered();
            let slot_weights = batching
                .product_slot_weights()
                .ok_or(ProtocolError::InvalidGeometry)?;
            if !slot_weights.len().is_power_of_two() {
                return Err(ProtocolError::InvalidGeometry);
            }
            let mut boundary = VerifierGrindingRoundBoundary::<LinearProductGrinding>::new(
                security.piop_round_grinding_bits,
                &product.nonces,
            );
            let slot_vars = slot_weights.len().ilog2() as usize;
            let (instance_point, slot_point) = verify_product_sumcheck(
                transcript,
                config,
                product,
                &instance_point,
                slot_vars,
                batching.initial_claim(),
                &mut boundary,
            )?;
            let (reduced, value) = product_terminal_batch(
                transcript,
                config,
                &batching,
                slot_weights,
                slots,
                &slot_point,
                &product.terminal,
            )?;
            batching.reanchored(&instance_point, reduced, value, config)?
        }
        (None, None) => batching,
        _ => return Err(ProtocolError::InvalidGeometry),
    };

    let (claim, assignment) = if product_layout {
        drop(step3_scope);
        let _step4 = tracing::info_span!("step4:bitify_verify").entered();
        let _scope = tracing::info_span!("sha256:direct_opening_prepare_verifier").entered();
        (spec.product_claim(&batching, &prime)?, None)
    } else {
        let (assignment_point, inner_claim) = {
            let _scope = tracing::info_span!("sha256:spartan_inner_verify").entered();
            proof
                .inner
                .verify_grinded::<Sha256InnerGrinding>(
                    transcript,
                    batching.initial_claim().clone(),
                    opened.row_vars + opened.col_vars,
                    config,
                    &proof.inner_nonces,
                    security.piop_round_grinding_bits,
                )
                .map_err(super::super::piop::SpartanError::from)?
        };
        drop(step3_scope);
        let _step4 = tracing::info_span!("step4:bitify_verify").entered();
        let collapsed_evaluation = batching.evaluate(&assignment_point, config)?;
        let claim = {
            let _scope = tracing::info_span!("sha256:opening_prepare_verifier").entered();
            spec.inner_claim(
                &assignment_point,
                &collapsed_evaluation,
                inner_claim,
                &prime,
            )?
        };
        (claim, Some((assignment_point, collapsed_evaluation)))
    };
    let rows = canonical_rows(opened, &claim)?;
    {
        let _step4 = tracing::info_span!("step4:bitify_verify").entered();
        let _scope = tracing::info_span!("sha256:opening_claim_absorb_verifier").entered();
        let digest = claim_digest(
            domains,
            &binding,
            &local_point,
            &instance_point,
            assignment
                .as_ref()
                .map(|(point, evaluation)| (point.as_slice(), evaluation)),
            &challenges,
            &claim,
            &rows,
            &config,
        )?;
        absorb_spartan_message(transcript, domains.claim_tag, &digest);
    }
    let OpeningClaim { cols, claimed, .. } = claim;
    Ok(LinearOpeningClaim { prime: prime.clone(), rows, cols, claimed })
}

/// Every opened row's canonical weight, in row order.
fn canonical_rows(layout: &IntegerMatrixLayout, claim: &OpeningClaim<'_>) -> Result<Vec<u128>, ProtocolError> {
    let row = |row: usize| (claim.rows)(row).ok_or(ProtocolError::InvalidGeometry);
    #[cfg(feature = "parallel")]
    let rows = (0..layout.rows()).into_par_iter().map(row).collect();
    #[cfg(not(feature = "parallel"))]
    let rows = (0..layout.rows()).map(row).collect();
    rows
}
