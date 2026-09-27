//! The linear-batching PIOP: relations whose constraints are all linear in
//! a derived bit grid `h = M·f` (the SHA-256 compression and chain
//! relations). There is no nonlinear outer sumcheck: the verifier draws a
//! local-row point `ξ` and an instance point `η`, batches the public
//! input/output equalities and the shared constant behind two more
//! challenges, and the collapsed relation is one rank-one functional over
//! the assignment grid. A power-of-two batch in the product layout opens
//! that functional directly; otherwise a quadratic inner sumcheck over the
//! assignment domain first reduces it to a point (the legacy path).
//!
//! The steps, on one transcript:
//!
//! 1. the relation's statement frames, the opener policy digest, Round 0;
//! 2. the initial grinding boundary, the runtime prime, the runtime
//!    relation frames;
//! 3. `ξ`, `η`, the terminal boundary, the constant-one batch challenge,
//!    the public-IO batch challenges; the relation's batching; for a
//!    relation with product slots, the product sumcheck
//!    ([`super::product`]) and the draw that folds its three terminal
//!    claims into one rank-one functional at its instance point;
//! 4. (legacy) the inner sumcheck; the opening claim frame;
//! 5. the virtual opening of the derived grid.

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
            FlockCommitHint, IntEvalRsLigVirtProof, LigeritoStatementConfig, OodRound, ProverOod,
            ResolvedLigerito, VerifierOod, bind_prover_ood, bind_verifier_ood,
            prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime,
            verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime,
        },
        pcs::{GeneratedModQWeightSource, IntegerMatrixLayout, ModQWeightSource},
        transcript::traits::Transcript,
    },
    circuit::linear_map::binary::VirtualMap,
};

use super::product::{
    InstanceBits, LinearProductGrinding, ProductRounds, ProductSlots, ProductSumcheckProof,
    prove_product3_sumcheck, prove_product_sumcheck, reduced_local_weights,
    verify_product3_sumcheck, verify_product_sumcheck,
};
use super::{
    super::{
        absorb_field_elements, absorb_spartan_message,
        sha256::inner_sumcheck::{SHA256_INNER_PREFIX_MAX_VARS, Sha256InnerBitSource},
        squeeze_field,
        sumcheck::SumcheckProof,
    },
    BindingHasher, FieldConfig, ProtocolError, SpartanBitzField, check_boundary, bitz_generator,
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

/// The claim's rows as the opener's weight source.
fn weight_source<'a>(
    layout: &IntegerMatrixLayout,
    q_bits: usize,
    claim: &'a OpeningClaim<'a>,
) -> Result<GeneratedModQWeightSource<&'a (dyn Fn(usize) -> Option<u128> + Sync + 'a)>, ProtocolError>
{
    GeneratedModQWeightSource::new(layout, q_bits, &*claim.rows)
        .map_err(|()| ProtocolError::InvalidGeometry)
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
}

/// A linear-relation proof: the boundary nonces, the legacy inner sumcheck
/// (empty on the product path) and the virtual opening (`B`: the forest's
/// by default, or another opener's, see [`LinearProverOpener`]).
#[derive(Clone)]
pub struct LinearProof<B = IntEvalRsLigVirtProof> {
    initial_nonce: u64,
    inner: SumcheckProof<SpartanBitzField, 3>,
    inner_nonces: Vec<u64>,
    terminal_nonce: u64,
    /// The product sumcheck, for relations with product slots.
    product: Option<ProductSumcheckProof>,
    bitz: B,
}

impl<B> LinearProof<B> {
    pub const fn initial_nonce(&self) -> u64 {
        self.initial_nonce
    }

    pub const fn inner(&self) -> &SumcheckProof<SpartanBitzField, 3> {
        &self.inner
    }

    pub fn inner_nonces(&self) -> &[u64] {
        &self.inner_nonces
    }

    pub const fn terminal_nonce(&self) -> u64 {
        self.terminal_nonce
    }

    pub const fn bitz(&self) -> &B {
        &self.bitz
    }

    pub const fn product(&self) -> Option<&ProductSumcheckProof> {
        self.product.as_ref()
    }

    pub fn product_mut(&mut self) -> Option<&mut ProductSumcheckProof> {
        self.product.as_mut()
    }

    /// PIOP payload bytes: the two boundary nonces, the inner round
    /// coefficients and their nonces, and the product sumcheck.
    pub fn piop_bytes(&self) -> usize {
        2 * 8
            + 3 * 16 * self.inner.round_polynomials.len()
            + 8 * self.inner_nonces.len()
            + self.product.as_ref().map_or(0, ProductSumcheckProof::bytes)
    }

    pub fn initial_nonce_mut(&mut self) -> &mut u64 {
        &mut self.initial_nonce
    }

    pub fn inner_mut(&mut self) -> &mut SumcheckProof<SpartanBitzField, 3> {
        &mut self.inner
    }

    pub fn inner_nonces_mut(&mut self) -> &mut Vec<u64> {
        &mut self.inner_nonces
    }

    pub fn terminal_nonce_mut(&mut self) -> &mut u64 {
        &mut self.terminal_nonce
    }

    pub fn bitz_mut(&mut self) -> &mut B {
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
    rows: &(impl ModQWeightSource + Sync),
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
    hasher.usize(rows.row_count())?;
    // Every row weight's little-endian bytes in order, as one string.
    let mut weights = vec![0u8; 16 * rows.row_count()];
    let write = |(row, out): (usize, &mut [u8])| -> Result<(), ProtocolError> {
        let weight = rows
            .canonical_weight(row)
            .ok_or(ProtocolError::InvalidGeometry)?;
        out.copy_from_slice(&weight.to_le_bytes());
        Ok(())
    };
    #[cfg(feature = "parallel")]
    weights.par_chunks_mut(16).enumerate().try_for_each(write)?;
    #[cfg(not(feature = "parallel"))]
    weights.chunks_mut(16).enumerate().try_for_each(write)?;
    hasher.bytes(&weights);
    hasher.usize(claim.cols.len())?;
    for weight in &claim.cols {
        hasher.u128_le(*weight);
    }
    hasher.u128_le(claim.claimed);
    Ok(hasher.finalize())
}

/// Proves a linear relation against its committed source rows, the claim
/// discharged by the crate's forest opener.
pub(crate) fn prove_linear<T: Transcript + Send, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    witness: &S::Witness,
    hint: &FlockCommitHint,
    pc: &LigProverConfig,
    options: LinearProveOptions,
) -> Result<LinearProof, ProtocolError> {
    prove_linear_with(
        transcript,
        spec,
        statement,
        witness,
        hint,
        options,
        &ForestProverOpener { pc },
    )
}

/// [`prove_linear`] with the step-5 opener as a parameter.
pub(crate) fn prove_linear_with<T, S, O>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    witness: &S::Witness,
    hint: &FlockCommitHint,
    options: LinearProveOptions,
    opener: &O,
) -> Result<LinearProof<O::Opening>, ProtocolError>
where
    T: Transcript + Send,
    S: LinearRelationSpec,
    O: LinearProverOpener<S>,
{
    spec.validate_statement(statement)?;
    opener.validate(spec, statement, witness, hint)?;
    if options.prefix_vars > SHA256_INNER_PREFIX_MAX_VARS {
        return Err(ProtocolError::InvalidInnerPrefix {
            actual: options.prefix_vars,
            max: SHA256_INNER_PREFIX_MAX_VARS,
        });
    }
    opener.validate_commitment(hint)?;
    let f_layout = opener.committed_layout(spec);
    if hint.commitment.params.m != f_layout.row_vars + f_layout.col_vars {
        return Err(ProtocolError::InvalidGeometry);
    }
    let security = spec.security();
    let domains = spec.domains();

    let binding = {
        let _scope = tracing::info_span!("sha256:statement_bind_prover").entered();
        let (frames, binding) = spec.statement_frames(statement, &hint.commitment, opener.config())?;
        absorb_frames(transcript, &frames);
        binding
    };
    opener.bind(spec, transcript)?;
    let ood = bind_prover_ood(transcript, hint, security.ood);

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
            let (rounds, terminal, instance_point, slot_point) = match slots {
                ProductSlots::Quadratic(quadratic) => {
                    let output = prove_product_sumcheck(
                        transcript,
                        config,
                        &instance_point,
                        batching.local_coefficients(),
                        batching.product_free_coefficients(),
                        slot_weights,
                        quadratic,
                        &*bits,
                        batching.initial_claim(),
                        &mut boundary,
                    )?;
                    (
                        ProductRounds::Quadratic(output.rounds),
                        output.terminal.to_vec(),
                        output.instance_point,
                        output.slot_point,
                    )
                }
                ProductSlots::Cubic(factors) => {
                    let output = prove_product3_sumcheck(
                        transcript,
                        config,
                        &instance_point,
                        batching.local_coefficients(),
                        batching.product_free_coefficients(),
                        slot_weights,
                        factors,
                        &*bits,
                        batching.initial_claim(),
                        &mut boundary,
                    )?;
                    (
                        ProductRounds::Cubic(output.rounds),
                        output.terminal.to_vec(),
                        output.instance_point,
                        output.slot_point,
                    )
                }
            };
            let (reduced, value) = product_terminal_batch(
                transcript,
                config,
                &batching,
                slot_weights,
                slots,
                &slot_point,
                &terminal,
            )?;
            let batching = batching.reanchored(&instance_point, reduced, value, config)?;
            let proof = ProductSumcheckProof {
                rounds,
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
    let rows = weight_source(spec.opened_layout(), prime.modulus_bits(), &claim)?;
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

    let bitz = {
        let _step5 = tracing::info_span!("step5:open_prove").entered();
        let _scope = tracing::info_span!("sha256:bitz_prove").entered();
        opener.open(transcript, spec, witness, hint, &claim, &rows, &prime, ood)?
    };

    Ok(LinearProof {
        initial_nonce,
        inner,
        inner_nonces,
        terminal_nonce,
        product,
        bitz,
    })
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
/// transcript; the claim is discharged by the crate's forest opener.
pub(crate) fn verify_linear<T: Transcript + Send, S: LinearRelationSpec>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    commitment: &Commitment,
    proof: &LinearProof,
    vc: &LigVerifierConfig,
) -> Result<(), ProtocolError> {
    verify_linear_with(
        transcript,
        spec,
        statement,
        commitment,
        proof,
        &ForestVerifierOpener { vc },
    )
}

/// [`verify_linear`] with the step-5 opener as a parameter.
pub(crate) fn verify_linear_with<T, S, O>(
    transcript: &mut T,
    spec: &S,
    statement: &S::Statement,
    commitment: &Commitment,
    proof: &LinearProof<O::Opening>,
    opener: &O,
) -> Result<(), ProtocolError>
where
    T: Transcript + Send,
    S: LinearRelationSpec,
    O: LinearVerifierOpener<S>,
{
    spec.validate_statement(statement)?;
    opener.validate(spec, commitment)?;
    let f_layout = opener.committed_layout(spec);
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
    let security = spec.security();
    let domains = spec.domains();

    let binding = {
        let _scope = tracing::info_span!("sha256:statement_bind_verifier").entered();
        let (frames, binding) = spec.statement_frames(statement, commitment, opener.config())?;
        absorb_frames(transcript, &frames);
        binding
    };
    opener.bind(spec, transcript)?;
    let ood = bind_verifier_ood(
        transcript,
        packed_vars(f_layout),
        security.ood,
        opener.ood_round(&proof.bitz),
    )
    .map_err(ProtocolError::Bitz)?;

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
            let (instance_point, slot_point) = match slots {
                ProductSlots::Quadratic(_) => verify_product_sumcheck(
                    transcript,
                    config,
                    product,
                    &instance_point,
                    slot_vars,
                    batching.initial_claim(),
                    &mut boundary,
                )?,
                ProductSlots::Cubic(_) => verify_product3_sumcheck(
                    transcript,
                    config,
                    product,
                    &instance_point,
                    slot_vars,
                    batching.initial_claim(),
                    &mut boundary,
                )?,
            };
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
    let rows = weight_source(opened, prime.modulus_bits(), &claim)?;
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

    let _step5 = tracing::info_span!("step5:open_verify").entered();
    let _scope = tracing::info_span!("sha256:bitz_verify").entered();
    opener.verify(transcript, spec, commitment, &proof.bitz, &claim, &rows, &prime, ood)
}

/// Step 5's opener on the prover side: the configuration the statement
/// frames cite, the checks before any transcript work, what the opener
/// binds between the relation's frames and Round 0, and the discharge of
/// the rank-one claim. [`ForestProverOpener`] makes exactly the calls the
/// runner made before openers were pluggable, in the same order.
pub(crate) trait LinearProverOpener<S: LinearRelationSpec> {
    type Opening;

    fn config(&self) -> &dyn LigeritoStatementConfig;

    /// The committed grid: the commitment's `m` and Round 0's arity.
    fn committed_layout<'s>(&self, spec: &'s S) -> &'s IntegerMatrixLayout {
        spec.source_layout()
    }

    fn validate(
        &self,
        spec: &S,
        statement: &S::Statement,
        witness: &S::Witness,
        hint: &FlockCommitHint,
    ) -> Result<(), ProtocolError>;

    fn validate_commitment(&self, hint: &FlockCommitHint) -> Result<(), ProtocolError>;

    fn bind<T: Transcript>(&self, spec: &S, transcript: &mut T) -> Result<(), ProtocolError>;

    #[allow(clippy::too_many_arguments)]
    fn open<T: Transcript + Send, W: ModQWeightSource + Sync + ?Sized>(
        &self,
        transcript: &mut T,
        spec: &S,
        witness: &S::Witness,
        hint: &FlockCommitHint,
        claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
        ood: ProverOod,
    ) -> Result<Self::Opening, ProtocolError>;
}

/// Step 5's opener on the verifier side (see [`LinearProverOpener`]).
pub(crate) trait LinearVerifierOpener<S: LinearRelationSpec> {
    type Opening;

    fn config(&self) -> &dyn LigeritoStatementConfig;

    fn committed_layout<'s>(&self, spec: &'s S) -> &'s IntegerMatrixLayout {
        spec.source_layout()
    }

    fn validate(&self, spec: &S, commitment: &Commitment) -> Result<(), ProtocolError>;

    fn bind<T: Transcript>(&self, spec: &S, transcript: &mut T) -> Result<(), ProtocolError>;

    /// The Round-0 messages the opening carries.
    fn ood_round<'p>(&self, opening: &'p Self::Opening) -> Option<&'p OodRound>;

    #[allow(clippy::too_many_arguments)]
    fn verify<T: Transcript + Send, W: ModQWeightSource + Sync + ?Sized>(
        &self,
        transcript: &mut T,
        spec: &S,
        commitment: &Commitment,
        opening: &Self::Opening,
        claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
        ood: VerifierOod,
    ) -> Result<(), ProtocolError>;
}

/// The crate's own opener: the chunked exponent-fold forest over the derived
/// grid, then the dual-basis ring switch and Ligerito on the commitment.
pub(crate) struct ForestProverOpener<'c> {
    pub pc: &'c LigProverConfig,
}

impl<S: LinearRelationSpec> LinearProverOpener<S> for ForestProverOpener<'_> {
    type Opening = IntEvalRsLigVirtProof;

    fn config(&self) -> &dyn LigeritoStatementConfig {
        self.pc
    }

    fn validate(
        &self,
        spec: &S,
        statement: &S::Statement,
        witness: &S::Witness,
        hint: &FlockCommitHint,
    ) -> Result<(), ProtocolError> {
        spec.validate_opener_config(self.pc)?;
        spec.validate_witness(statement, witness, hint)
    }

    fn validate_commitment(&self, hint: &FlockCommitHint) -> Result<(), ProtocolError> {
        crate::ligerito_flock::validate_ligerito_commitment(&hint.commitment, self.pc)
            .map_err(ProtocolError::Bitz)
    }

    fn bind<T: Transcript>(&self, spec: &S, transcript: &mut T) -> Result<(), ProtocolError> {
        spec.ligerito()?.bind(transcript);
        Ok(())
    }

    fn open<T: Transcript + Send, W: ModQWeightSource + Sync + ?Sized>(
        &self,
        transcript: &mut T,
        spec: &S,
        witness: &S::Witness,
        hint: &FlockCommitHint,
        _claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
        ood: ProverOod,
    ) -> Result<IntEvalRsLigVirtProof, ProtocolError> {
        prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
            transcript,
            hint,
            spec.opened_rows(witness)?,
            spec.opened_layout(),
            spec.source_layout(),
            spec.map(),
            rows,
            prime.modulus_u128(),
            prime.modulus_bits(),
            bitz_generator(),
            spec.security().forest_round_grinding_bits,
            ood,
            self.pc,
        )
        .map_err(ProtocolError::Bitz)
    }
}

/// The verifier side of [`ForestProverOpener`].
pub(crate) struct ForestVerifierOpener<'c> {
    pub vc: &'c LigVerifierConfig,
}

impl<S: LinearRelationSpec> LinearVerifierOpener<S> for ForestVerifierOpener<'_> {
    type Opening = IntEvalRsLigVirtProof;

    fn config(&self) -> &dyn LigeritoStatementConfig {
        self.vc
    }

    fn validate(&self, spec: &S, commitment: &Commitment) -> Result<(), ProtocolError> {
        spec.validate_opener_config(self.vc)?;
        crate::ligerito_flock::validate_ligerito_commitment(commitment, self.vc)
            .map_err(ProtocolError::Bitz)
    }

    fn bind<T: Transcript>(&self, spec: &S, transcript: &mut T) -> Result<(), ProtocolError> {
        spec.ligerito()?.bind(transcript);
        Ok(())
    }

    fn ood_round<'p>(&self, opening: &'p IntEvalRsLigVirtProof) -> Option<&'p OodRound> {
        opening.ood.as_ref()
    }

    fn verify<T: Transcript + Send, W: ModQWeightSource + Sync + ?Sized>(
        &self,
        transcript: &mut T,
        spec: &S,
        commitment: &Commitment,
        opening: &IntEvalRsLigVirtProof,
        claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
        ood: VerifierOod,
    ) -> Result<(), ProtocolError> {
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
            transcript,
            commitment,
            opening,
            spec.opened_layout(),
            spec.source_layout(),
            spec.map(),
            rows,
            &claim.cols,
            bitz_generator(),
            claim.claimed,
            prime.modulus_u128(),
            prime.modulus_bits(),
            spec.security().forest_round_grinding_bits,
            ood,
            self.vc,
        )
        .map_err(ProtocolError::Bitz)
    }
}

/// The session tag of a block-layout BitZ opening's forked transcript.
#[cfg(feature = "bitz-parity")]
const WFBITZ_BLOCK_SESSION: &[u8] = b"bitz/linear/wfbitz-block/v1";

/// Step 5 through the worldfnd/BitZ scheme on a block-layout commitment
/// (`crate::wfbitz::block`): the rank-one claim on the derived grid is
/// BitZ's `LinearClaim` as it stands (canonical row weights, column
/// weights, target mod `q`), the fold and the GKR run over the derived rows
/// on a transcript forked after the claim frame, and the GKR's exit claim
/// is transposed through the block map and opened on the commitment under
/// the relation's resolved ladder. Nothing is drawn from the outer
/// transcript after the fork; Round 0 is the runner's, batched into the
/// final Ligerito opening as in [`super::wfbitz_opener`].
#[cfg(feature = "bitz-parity")]
pub(crate) struct WfbitzBlockOpener<'a, C> {
    /// The resolved ladder's configuration: what the statement frames cite
    /// and the commitment is checked against.
    pub config: &'a C,
    /// BitZ's scheme over the committed block grid, under the same ladder.
    pub pcs: &'a crate::wfbitz::Pcs,
    pub map: &'a crate::wfbitz::BlockRepeatedMap,
}

#[cfg(feature = "bitz-parity")]
impl<C> WfbitzBlockOpener<'_, C> {
    fn bind_opener<T: Transcript, S: LinearRelationSpec>(
        &self,
        spec: &S,
        transcript: &mut T,
    ) -> Result<(), ProtocolError> {
        spec.ligerito()?.bind(transcript);
        absorb_frames(
            transcript,
            &[
                frame(b"opener", WFBITZ_BLOCK_SESSION),
                frame(b"wfbitz-block-map", self.map.digest()),
            ],
        );
        Ok(())
    }

    /// The claim as BitZ states it, and the committed grid it is opened on.
    fn statement<S: LinearRelationSpec, W: ModQWeightSource + Sync + ?Sized>(
        spec: &S,
        claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
    ) -> Result<(crate::wfbitz::BitZParams, crate::wfbitz::Shape, crate::wfbitz::LinearClaim), ProtocolError>
    {
        let bitz = |what: &str, error: &dyn core::fmt::Debug| {
            ProtocolError::LigeritoConfig(format!("BitZ {what}: {error:?}"))
        };
        let opened = spec.opened_layout();
        let source = spec.source_layout();
        if opened.word_bits != 1 || source.word_bits != 1 {
            return Err(ProtocolError::InvalidGeometry);
        }
        let derived = crate::wfbitz::Shape::new(opened.row_vars, opened.col_vars)
            .map_err(|e| bitz("derived shape", &e))?;
        let committed = crate::wfbitz::Shape::new(source.row_vars, source.col_vars)
            .map_err(|e| bitz("committed shape", &e))?;
        let params = crate::wfbitz::BitZParams::new(derived, prime.modulus_u128(), bitz_generator())
            .map_err(|e| bitz("parameters", &e))?;
        let row_weights = (0..rows.row_count())
            .map(|row| rows.canonical_weight(row).ok_or(ProtocolError::InvalidGeometry))
            .collect::<Result<Vec<_>, _>>()?;
        let linear =
            crate::wfbitz::LinearClaim::new(&params, row_weights, claim.cols.clone(), claim.claimed)
                .map_err(|e| bitz("claim", &e))?;
        Ok((params, committed, linear))
    }
}

#[cfg(feature = "bitz-parity")]
impl<S: LinearRelationSpec> LinearProverOpener<S> for WfbitzBlockOpener<'_, LigProverConfig> {
    type Opening = super::wfbitz_opener::WfbitzOpeningProof;

    fn config(&self) -> &dyn LigeritoStatementConfig {
        self.config
    }

    fn validate(
        &self,
        spec: &S,
        statement: &S::Statement,
        witness: &S::Witness,
        hint: &FlockCommitHint,
    ) -> Result<(), ProtocolError> {
        spec.validate_opener_config(self.config)?;
        spec.validate_witness(statement, witness, hint)
    }

    fn validate_commitment(&self, hint: &FlockCommitHint) -> Result<(), ProtocolError> {
        crate::ligerito_flock::validate_ligerito_commitment(&hint.commitment, self.config)
            .map_err(ProtocolError::Bitz)
    }

    fn bind<T: Transcript>(&self, spec: &S, transcript: &mut T) -> Result<(), ProtocolError> {
        self.bind_opener(spec, transcript)
    }

    fn open<T: Transcript + Send, W: ModQWeightSource + Sync + ?Sized>(
        &self,
        transcript: &mut T,
        spec: &S,
        witness: &S::Witness,
        hint: &FlockCommitHint,
        claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
        ood: ProverOod,
    ) -> Result<Self::Opening, ProtocolError> {
        let (params, committed, linear) = Self::statement(spec, claim, rows, prime)?;
        let statement = crate::wfbitz::BlockStatement::new(params, committed, self.map, &linear)
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ block statement: {e:?}")))?;
        let ood = ood.into_bound_claim();
        let tag = super::wfbitz_opener::fork_tag(transcript);
        let mut state = crate::wfbitz::build_prover(WFBITZ_BLOCK_SESSION, &tag);
        crate::wfbitz::BitZProver::new(params, crate::wfbitz::WINDOW)
            .prove_block(
                &statement,
                self.pcs,
                hint,
                spec.opened_rows(witness)?,
                &mut state,
                ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
            )
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ prove: {e:?}")))?;
        let crate::wfbitz::Proof { narg_string, hints } = state.finish();
        Ok(super::wfbitz_opener::WfbitzOpeningProof {
            narg: narg_string,
            hints,
            ood: ood.map(|claim| claim.round),
        })
    }
}

#[cfg(feature = "bitz-parity")]
impl<S: LinearRelationSpec> LinearVerifierOpener<S> for WfbitzBlockOpener<'_, LigVerifierConfig> {
    type Opening = super::wfbitz_opener::WfbitzOpeningProof;

    fn config(&self) -> &dyn LigeritoStatementConfig {
        self.config
    }

    fn validate(&self, spec: &S, commitment: &Commitment) -> Result<(), ProtocolError> {
        spec.validate_opener_config(self.config)?;
        crate::ligerito_flock::validate_ligerito_commitment(commitment, self.config)
            .map_err(ProtocolError::Bitz)
    }

    fn bind<T: Transcript>(&self, spec: &S, transcript: &mut T) -> Result<(), ProtocolError> {
        self.bind_opener(spec, transcript)
    }

    fn ood_round<'p>(&self, opening: &'p Self::Opening) -> Option<&'p OodRound> {
        opening.ood.as_ref()
    }

    fn verify<T: Transcript + Send, W: ModQWeightSource + Sync + ?Sized>(
        &self,
        transcript: &mut T,
        spec: &S,
        commitment: &Commitment,
        opening: &Self::Opening,
        claim: &OpeningClaim<'_>,
        rows: &W,
        prime: &field::FpCtx<2>,
        ood: VerifierOod,
    ) -> Result<(), ProtocolError> {
        let (params, committed, linear) = Self::statement(spec, claim, rows, prime)?;
        let statement = crate::wfbitz::BlockStatement::new(params, committed, self.map, &linear)
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ block statement: {e:?}")))?;
        let ood = ood.into_bound_claim();
        let tag = super::wfbitz_opener::fork_tag(transcript);
        let proof = crate::wfbitz::Proof {
            narg_string: opening.narg.clone(),
            hints: opening.hints.clone(),
        };
        let state = crate::wfbitz::build_verifier(WFBITZ_BLOCK_SESSION, &tag, &proof);
        crate::wfbitz::BitZVerifier::new(params, crate::wfbitz::WINDOW)
            .verify_block(
                &statement,
                self.pcs,
                crate::wfbitz::Root(commitment.root),
                state,
                ood.as_ref().map(|(claim, _)| (claim.point.as_slice(), claim.y)),
            )
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ verify: {e:?}")))
    }
}
