//! Combined Spartan and F2Z proof of the MultiSwap integer Mod-R1CS.
//!
//! Protocol order (identical for prover and verifier), following the
//! paper's Strategy 2 instantiation (large PIOP field, grinding
//! concentrated at the Step 5.0 reduction draw):
//!
//! 1. Bind the complete public statement: the integer circuit digest, the
//!    block layout, and the F2Z commitment to the witness/quotient bits.
//! 2. Sample the 128-bit fingerprint prime `Q` (the commit-before-prime
//!    order is the Zaratan fingerprint; the full-width interval makes the
//!    draw `<= 2^-114` sound without grinding).
//! 3. Project the integer matrices, assignment, and products modulo `Q`
//!    and run the stock Spartan PIOP (cubic outer + batched quadratic
//!    inner) over `F_Q`; every round message carries error `<= 3/Q`.
//! 4. Translate the terminal scaled assignment-MLE claim through the
//!    public bitification adjoint into one mod-`Q` tensor functional over
//!    the committed bit tensor.
//! 5. Step 5.0 ([`super::reduce`]): the prover sends the exact integer
//!    lift `mu'` of that tensor claim; after checking `mu' = mu (mod Q)`
//!    and the `d * Q^2` bound, a 10-bit grind and a fresh 113-bit
//!    reduction prime `q'` re-project the claim below the exponent-fold
//!    no-wrap boundary, and the runtime-`q'` F2Z opening discharges it.
//!
//! The opening runs through the virtual-map entry points with the exact
//! identity map, which the library recognizes and routes to the direct
//! base opening.
//!
//! Soundness floors are documented in [`super::prime`]: every step is at
//! or below `2^-114` — the fingerprint draw `2^-114.0`, Spartan rounds
//! `2^-125.4`, the grinded reduction draw `2^-114.2` — matching the floors
//! Limber's own implementation accepts, at a total grinding cost of
//! `2^10` hashes.

use blake3::Hasher;
use crypto_primitives::PrimeField;
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};
use num_bigint::BigUint;
use thiserror::Error;

use crate::{
    f2map::{cell_count, PreparedVirtualMap, PreparedVirtualMapError, RepeatedVirtualMap, VirtualMap},
    ligerito::packed_vars,
    ligerito_flock::{
        commit_rs_ligerito_rows, prove_mle_eval_mod_q_ligerito_virtual_runtime,
        sha_paper128_lig_configs, validate_ligerito_commitment,
        verify_mle_eval_mod_q_ligerito_virtual_runtime, FlockCommitHint, FlockRsError,
        IntEvalRsLigVirtProof,
    },
    pcs::{IntEvalParams, ProjectCanonicalU128},
    sparse_matrix::SparseMatrix,
    transcript::traits::Transcript,
};

use super::super::{
    absorb_spartan_message,
    f2z::f2z_generator,
    grinding::{grind_and_absorb, verify_and_absorb, GrindingDomain, GrindingError, GrindingRound},
    matrix::{eq_table, ScaledMleEvaluationClaim},
    piop::{
        prove_spartan_piop_with_strategy, verify_spartan_proof, SpartanError, SpartanPiopProof,
        SpartanReductionStrategy,
    },
    SpartanF2zField, SpartanField,
};
use super::{
    circuit::{MultiswapCircuit, MultiswapCircuitError, MULTISWAP_VALUE_BITS},
    prime::{
        sample_multiswap_fingerprint_context, sample_multiswap_reduction_prime,
        MultiswapFingerprintContext, MultiswapPrimeError, MultiswapPrimeProfile,
    },
    reduce::{step50_accepts_lift, step50_integer_lift, step50_reduce},
    relation::{
        MultiswapAssignment, MultiswapIntegerRelation, MultiswapLayout, MultiswapLayoutError,
        MULTISWAP_QUOS_SLOT_START, MULTISWAP_SLOTS, MULTISWAP_W_SLOT_START,
    },
};

const MULTISWAP_STATEMENT_DOMAIN: &[u8] = b"f2z/spartan-multiswap/statement/v2";
const MULTISWAP_BINDING_DOMAIN: &[u8] = b"f2z/spartan-multiswap/assignment-binding/v2";
const MULTISWAP_OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-multiswap/opening-claim/v2";
/// Local identity block repeated across gates; any power-of-two factor of
/// the cell count works, and the slot count keeps the local map small.
const IDENTITY_LOCAL_ROWS: usize = MULTISWAP_SLOTS;

enum MultiswapReductionGrinding {}

impl GrindingDomain for MultiswapReductionGrinding {
    const DOMAIN: &'static [u8] = b"f2z/spartan-multiswap/grinding/reduction/v2";
}

/// Failures in the MultiSwap Spartan/F2Z adapter.
#[derive(Debug, Error)]
pub enum MultiswapError {
    /// Circuit construction or the integer relation check failed.
    #[error(transparent)]
    Circuit(#[from] MultiswapCircuitError),

    /// Layout, matrix, or witness materialization failed.
    #[error(transparent)]
    Layout(#[from] MultiswapLayoutError),

    /// Runtime-prime sampling or validation failed.
    #[error(transparent)]
    Prime(#[from] MultiswapPrimeError),

    /// A Fiat--Shamir grinding nonce could not be produced or checked.
    #[error(transparent)]
    Grinding(#[from] GrindingError),

    /// The Spartan PIOP rejected.
    #[error(transparent)]
    Spartan(#[from] SpartanError),

    /// The identity virtual map could not be prepared.
    #[error(transparent)]
    VirtualMap(#[from] PreparedVirtualMapError),

    /// The F2Z opening rejected.
    #[error("the MultiSwap F2Z opening rejected: {0:?}")]
    F2z(FlockRsError),

    /// Ligerito configuration derivation failed.
    #[error("failed to derive a Ligerito configuration: {0}")]
    LigeritoConfig(String),

    /// Relation, witness, commitment, and layout geometry disagree.
    #[error("MultiSwap relation or witness geometry is inconsistent")]
    InvalidGeometry,

    /// The terminal Spartan claim has the wrong shape or field.
    #[error("the terminal Spartan claim is malformed")]
    InvalidClaim,

    /// A constant-only terminal claim carries a nonzero adjusted value.
    #[error("a constant-only terminal claim has a nonzero adjusted value")]
    InvalidConstantOnlyClaim,

    /// The Step 5.0 integer lift fails the mod-`Q` or magnitude check.
    #[error("the Step 5.0 integer lift is inconsistent with the mod-Q claim")]
    InvalidIntegerLift,
}

/// Setup-once, prime-independent bundle: the integer relation, the identity
/// opening map, the F2Z shape, and the statement digest.
pub struct PreparedMultiswapRelation {
    relation: MultiswapIntegerRelation,
    statement_digest: [u8; 32],
    map: RepeatedVirtualMap,
    params: IntEvalParams,
    profile: MultiswapPrimeProfile,
}

impl PreparedMultiswapRelation {
    /// Prepares the relation, layout, and identity map from a built circuit.
    pub fn new(circuit: &MultiswapCircuit) -> Result<Self, MultiswapError> {
        let relation = MultiswapIntegerRelation::new(circuit)?;
        let params = relation.layout().f2z_params();
        let cells = cell_count(&params);
        if cells % IDENTITY_LOCAL_ROWS != 0 {
            return Err(MultiswapError::InvalidGeometry);
        }
        let identity_columns = (0..IDENTITY_LOCAL_ROWS)
            .map(|index| vec![(index, true)])
            .collect::<Vec<_>>();
        let local = PreparedVirtualMap::new(
            SparseMatrix::try_from_columns(IDENTITY_LOCAL_ROWS, identity_columns)
                .expect("the identity block is a valid CSC matrix"),
        )?;
        let map = RepeatedVirtualMap::new(local, cells / IDENTITY_LOCAL_ROWS)?;
        debug_assert!(crate::f2map::VirtualMap::is_identity(&map));
        Ok(Self {
            relation,
            statement_digest: circuit.statement_digest(),
            map,
            params,
            profile: MultiswapPrimeProfile::new(),
        })
    }

    /// The prime-independent integer relation.
    pub const fn relation(&self) -> &MultiswapIntegerRelation {
        &self.relation
    }

    /// Shared block/bit layout.
    pub const fn layout(&self) -> &MultiswapLayout {
        self.relation.layout()
    }

    /// F2Z shape of the committed bit tensor.
    pub const fn params(&self) -> &IntEvalParams {
        &self.params
    }

    /// Runtime-prime profile.
    pub const fn profile(&self) -> MultiswapPrimeProfile {
        self.profile
    }

    /// Canonical digest of the integer circuit statement.
    pub const fn statement_digest(&self) -> &[u8; 32] {
        &self.statement_digest
    }

    /// Identity opening map.
    pub const fn map(&self) -> &RepeatedVirtualMap {
        &self.map
    }
}

/// Derives the production Ligerito configuration for the MultiSwap shape.
pub fn multiswap_lig_configs(
    p: &IntEvalParams,
) -> Result<(LigProverConfig, LigVerifierConfig), MultiswapError> {
    sha_paper128_lig_configs(packed_vars(p)).map_err(MultiswapError::LigeritoConfig)
}

/// Commits prebuilt packed witness/quotient bit rows.
pub fn commit_multiswap_witness(
    p: &IntEvalParams,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> Result<FlockCommitHint, MultiswapError> {
    validate_bit_rows(p, &rows)?;
    let hint = commit_rs_ligerito_rows(p, rows, pc);
    validate_ligerito_commitment(&hint.commitment, pc).map_err(MultiswapError::F2z)?;
    Ok(hint)
}

/// The two-prime MultiSwap proof.
#[derive(Clone)]
pub struct MultiswapProof {
    spartan: SpartanPiopProof<SpartanF2zField>,
    mu_prime: BigUint,
    reduction_nonce: u64,
    f2z: IntEvalRsLigVirtProof,
}

impl MultiswapProof {
    /// Outer and inner Spartan sumcheck proofs.
    pub const fn spartan(&self) -> &SpartanPiopProof<SpartanF2zField> {
        &self.spartan
    }

    /// Step 5.0 exact integer lift of the bitified terminal claim.
    pub const fn mu_prime(&self) -> &BigUint {
        &self.mu_prime
    }

    /// Grinding nonce immediately before the reduction-prime draw.
    pub const fn reduction_nonce(&self) -> u64 {
        self.reduction_nonce
    }

    /// Runtime-prime F2Z opening proof.
    pub const fn f2z(&self) -> &IntEvalRsLigVirtProof {
        &self.f2z
    }

    /// Field elements in the Spartan payload (round coefficients and
    /// terminal product claims), for analytic size accounting.
    pub fn spartan_payload_elements(&self) -> usize {
        4 * self.spartan.outer.sumcheck.round_polynomials.len()
            + 3
            + 3 * self.spartan.inner.round_polynomials.len()
    }

    /// Serialized byte length of the Step 5.0 integer lift.
    pub fn mu_prime_bytes(&self) -> usize {
        self.mu_prime.to_bytes_le().len()
    }
}

/// Proves the MultiSwap Mod-R1CS against a committed bit witness.
pub fn prove_multiswap_mod_r1cs<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedMultiswapRelation,
    assignment: &MultiswapAssignment,
    hint: &FlockCommitHint,
    pc: &LigProverConfig,
) -> Result<MultiswapProof, MultiswapError> {
    let p = prepared.params();
    if assignment.layout() != prepared.layout() {
        return Err(MultiswapError::InvalidGeometry);
    }
    validate_bit_rows(p, hint.rows())?;
    validate_ligerito_commitment(&hint.commitment, pc).map_err(MultiswapError::F2z)?;
    if hint.commitment.params.m != p.t + p.s {
        return Err(MultiswapError::InvalidGeometry);
    }

    let binding = assignment_binding(prepared, &hint.commitment);
    absorb_spartan_message(transcript, b"multiswap-statement", &binding);

    let fingerprint = {
        let _scope = crate::utils::prof::scope("multiswap:fingerprint_prime_prove");
        sample_multiswap_fingerprint_context(transcript, prepared.profile())?
    };
    let matrices = {
        let _scope = crate::utils::prof::scope("multiswap:relation_projection_prove");
        prepared
            .relation()
            .project::<SpartanF2zField>(fingerprint.field_config())?
    };
    let (assignment_mle, products) = {
        let _scope = crate::utils::prof::scope("multiswap:witness_projection_prove");
        assignment.project::<SpartanF2zField>(prepared.relation(), fingerprint.field_config())?
    };

    let (spartan, terminal_claim) = {
        let _scope = crate::utils::prof::scope("multiswap:spartan_prove");
        prove_spartan_piop_with_strategy(
            transcript,
            &matrices,
            &binding,
            products,
            assignment_mle,
            SpartanReductionStrategy::DelayedBarrett,
        )?
    };

    let opening = {
        let _scope = crate::utils::prof::scope("multiswap:bitify_prove");
        bitify_multiswap_claim(&terminal_claim, prepared.layout(), &fingerprint)?
    };

    let mu_prime = {
        let _scope = crate::utils::prof::scope("multiswap:integer_lift_prove");
        step50_integer_lift(hint.rows(), &opening.row_weights_q, &opening.col_weights_q)
    };
    if !step50_accepts_lift(
        &mu_prime,
        opening.claimed_q,
        fingerprint.q(),
        cell_count(p),
    ) {
        return Err(MultiswapError::InvalidIntegerLift);
    }
    absorb_opening_claim(transcript, &binding, &terminal_claim, &opening, &mu_prime);

    let reduction_nonce = {
        let _scope = crate::utils::prof::scope("multiswap:reduction_grinding_prove");
        grind_and_absorb::<MultiswapReductionGrinding, _>(
            transcript,
            GrindingRound::new(0),
            prepared.profile().reduction_grinding_bits() as u32,
        )?
    };
    let (q_prime, q_prime_bits) = {
        let _scope = crate::utils::prof::scope("multiswap:reduction_prime_prove");
        sample_multiswap_reduction_prime(transcript, prepared.profile())?
    };
    let (row_weights_reduced, _, _) = step50_reduce(
        &opening.row_weights_q,
        &opening.col_weights_q,
        &mu_prime,
        q_prime,
    );

    let f2z = {
        let _scope = crate::utils::prof::scope("multiswap:f2z_prove");
        prove_mle_eval_mod_q_ligerito_virtual_runtime(
            transcript,
            hint,
            hint.rows(),
            p,
            p,
            prepared.map(),
            &row_weights_reduced,
            q_prime,
            q_prime_bits,
            f2z_generator(),
            pc,
        )
        .map_err(MultiswapError::F2z)?
    };

    Ok(MultiswapProof {
        spartan,
        mu_prime,
        reduction_nonce,
        f2z,
    })
}

/// Verifies the MultiSwap proof, re-deriving both primes from the bound
/// transcript.
pub fn verify_multiswap_mod_r1cs<T: Transcript + Send>(
    transcript: &mut T,
    prepared: &PreparedMultiswapRelation,
    commitment: &Commitment,
    proof: &MultiswapProof,
    vc: &LigVerifierConfig,
) -> Result<(), MultiswapError> {
    let p = prepared.params();
    validate_ligerito_commitment(commitment, vc).map_err(MultiswapError::F2z)?;
    if commitment.params.m != p.t + p.s {
        return Err(MultiswapError::InvalidGeometry);
    }

    let binding = assignment_binding(prepared, commitment);
    absorb_spartan_message(transcript, b"multiswap-statement", &binding);

    let fingerprint = {
        let _scope = crate::utils::prof::scope("multiswap:fingerprint_prime_verify");
        sample_multiswap_fingerprint_context(transcript, prepared.profile())?
    };
    let matrices = {
        let _scope = crate::utils::prof::scope("multiswap:relation_projection_verify");
        prepared
            .relation()
            .project::<SpartanF2zField>(fingerprint.field_config())?
    };

    let terminal_claim = {
        let _scope = crate::utils::prof::scope("multiswap:spartan_verify");
        verify_spartan_proof(transcript, &matrices, &binding, &proof.spartan)?
    };

    let opening = {
        let _scope = crate::utils::prof::scope("multiswap:bitify_verify");
        bitify_multiswap_claim(&terminal_claim, prepared.layout(), &fingerprint)?
    };
    // Step 5.0: the claimed integer lift must land in the derived mod-Q
    // class and inside the d * Q^2 magnitude bound.
    if !step50_accepts_lift(
        &proof.mu_prime,
        opening.claimed_q,
        fingerprint.q(),
        cell_count(p),
    ) {
        return Err(MultiswapError::InvalidIntegerLift);
    }
    absorb_opening_claim(transcript, &binding, &terminal_claim, &opening, &proof.mu_prime);

    {
        let _scope = crate::utils::prof::scope("multiswap:reduction_grinding_verify");
        verify_and_absorb::<MultiswapReductionGrinding, _>(
            transcript,
            GrindingRound::new(0),
            prepared.profile().reduction_grinding_bits() as u32,
            proof.reduction_nonce,
        )?;
    }
    let (q_prime, q_prime_bits) = {
        let _scope = crate::utils::prof::scope("multiswap:reduction_prime_verify");
        sample_multiswap_reduction_prime(transcript, prepared.profile())?
    };
    let (row_weights_reduced, col_weights_reduced, claimed_reduced) = step50_reduce(
        &opening.row_weights_q,
        &opening.col_weights_q,
        &proof.mu_prime,
        q_prime,
    );

    let _scope = crate::utils::prof::scope("multiswap:f2z_verify");
    verify_mle_eval_mod_q_ligerito_virtual_runtime(
        transcript,
        commitment,
        &proof.f2z,
        p,
        p,
        prepared.map(),
        &row_weights_reduced,
        &col_weights_reduced,
        f2z_generator(),
        claimed_reduced,
        q_prime,
        q_prime_bits,
        vc,
    )
    .map_err(MultiswapError::F2z)
}

/// The bitified terminal claim over the fingerprint field: one dense
/// mod-`Q` row functional over the committed bit tensor, the clear column
/// functional, and the adjusted claimed value, all as canonical residues.
pub struct MultiswapOpeningClaim {
    row_weights_q: Vec<u128>,
    col_weights_q: Vec<u128>,
    claimed_q: u128,
}

impl MultiswapOpeningClaim {
    /// Dense per-folded-row weights, canonical in `[0, Q)`.
    pub fn row_weights(&self) -> &[u128] {
        &self.row_weights_q
    }

    /// Clear column weights, canonical in `[0, Q)`.
    pub fn col_weights(&self) -> &[u128] {
        &self.col_weights_q
    }

    /// Adjusted claimed value (public constant-block term removed).
    pub const fn claimed(&self) -> u128 {
        self.claimed_q
    }
}

/// Applies the adjoint of the public block bitification to the terminal
/// scaled assignment-MLE claim, over the runtime fingerprint field.
///
/// Spartan's point is low-coordinate-first: the gate coordinates come
/// first, then the two block selectors (`00` constant, `01` witness,
/// `10` quotient, `11` the empty zero block).  The nonzero Spartan scale is
/// folded onto the row side so the clear column table stays the raw
/// equality table, mirroring the u32 bridge's normalization.  All
/// arithmetic runs in the runtime Montgomery field, so any modulus the
/// Spartan layer accepts — including the full-width fingerprint primes —
/// is handled without `u128` overflow concerns.
pub fn bitify_multiswap_claim(
    claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    layout: &MultiswapLayout,
    fingerprint: &MultiswapFingerprintContext,
) -> Result<MultiswapOpeningClaim, MultiswapError> {
    let gate_vars = layout.gate_vars();
    if claim.point().len() != gate_vars + 2 {
        return Err(MultiswapError::InvalidClaim);
    }
    let config = fingerprint.field_config();
    let expected_encoding = SpartanF2zField::canonical_modulus_encoding(config);
    let validated = |value: &SpartanF2zField| -> Result<SpartanF2zField, MultiswapError> {
        if SpartanF2zField::canonical_modulus_encoding(value.cfg()) != expected_encoding
            || value.validate_element().is_err()
        {
            return Err(MultiswapError::InvalidClaim);
        }
        Ok(value.clone())
    };

    let gate_point = claim.point()[..gate_vars]
        .iter()
        .map(validated)
        .collect::<Result<Vec<_>, _>>()?;
    let block_low = validated(&claim.point()[gate_vars])?;
    let block_high = validated(&claim.point()[gate_vars + 1])?;
    let one = SpartanF2zField::one_with_cfg(config);
    let mut one_minus_low = one.clone();
    one_minus_low -= &block_low;
    let mut one_minus_high = one.clone();
    one_minus_high -= &block_high;

    let mul = |left: &SpartanF2zField, right: &SpartanF2zField| {
        let mut product = left.clone();
        product *= right;
        product
    };
    let constant_factor = mul(&one_minus_low, &one_minus_high);
    let witness_factor = mul(&block_low, &one_minus_high);
    let quotient_factor = mul(&one_minus_low, &block_high);

    let scale = validated(claim.scale())?;
    let value = validated(claim.value())?;
    let constant_evaluation = gate_point.iter().fold(constant_factor, |acc, coordinate| {
        let mut complement = one.clone();
        complement -= coordinate;
        mul(&acc, &complement)
    });
    let mut adjusted = value;
    adjusted -= &mul(&scale, &constant_evaluation);
    let claimed_q = adjusted.canonical_u128();

    let (s, h) = (layout.column_vars(), layout.high_gate_vars());
    let (gate_low, gate_high) = gate_point.split_at(s);
    let p = layout.f2z_params();
    let row_count = p.rows();
    let column_count = p.cols();

    if SpartanF2zField::is_zero(&witness_factor) && SpartanF2zField::is_zero(&quotient_factor) {
        // The variable blocks vanish at this block point; the F2Z protocol
        // still needs a nonempty row functional, so use a deterministic
        // dummy row with an all-zero clear read-off.
        if claimed_q != 0 {
            return Err(MultiswapError::InvalidConstantOnlyClaim);
        }
        let mut row_weights_q = vec![0u128; row_count];
        row_weights_q[0] = 1;
        return Ok(MultiswapOpeningClaim {
            row_weights_q,
            col_weights_q: vec![0u128; column_count],
            claimed_q: 0,
        });
    }

    // Move a nonzero Spartan scale onto the two block row factors; a zero
    // scale instead zeroes the clear column table so the claim reduces to
    // `0 = claimed`, exactly like the u32 bridge.
    let (witness_factor, quotient_factor, col_weights_q) = if SpartanF2zField::is_zero(&scale) {
        (witness_factor, quotient_factor, vec![0u128; column_count])
    } else {
        let table = eq_table(gate_low, config).map_err(SpartanError::from)?;
        (
            mul(&scale, &witness_factor),
            mul(&scale, &quotient_factor),
            table
                .iter()
                .map(ProjectCanonicalU128::canonical_u128)
                .collect(),
        )
    };

    let eq_high = eq_table(gate_high, config).map_err(SpartanError::from)?;
    debug_assert_eq!(eq_high.len(), 1usize << h);
    let mut row_weights_q = vec![0u128; row_count];
    for (slot_start, factor) in [
        (MULTISWAP_W_SLOT_START, witness_factor),
        (MULTISWAP_QUOS_SLOT_START, quotient_factor),
    ] {
        if SpartanF2zField::is_zero(&factor) {
            continue;
        }
        let mut bit_weight = factor;
        for bit in 0..MULTISWAP_VALUE_BITS {
            let row_base = (slot_start + bit) << h;
            for (gate_high_index, equality_weight) in eq_high.iter().enumerate() {
                row_weights_q[row_base | gate_high_index] =
                    mul(&bit_weight, equality_weight).canonical_u128();
            }
            let doubled = bit_weight.clone();
            bit_weight += &doubled;
        }
    }

    Ok(MultiswapOpeningClaim {
        row_weights_q,
        col_weights_q,
        claimed_q,
    })
}

fn validate_bit_rows(p: &IntEvalParams, rows: &[Vec<u64>]) -> Result<(), MultiswapError> {
    let row_bits = p.rows() * p.word_bits;
    if p.word_bits != 1
        || row_bits % u64::BITS as usize != 0
        || rows.len() != p.cols()
        || rows
            .iter()
            .any(|row| row.len() != row_bits / u64::BITS as usize)
    {
        return Err(MultiswapError::InvalidGeometry);
    }
    Ok(())
}

/// Digest binding the integer statement, layout, profile, and commitment.
fn assignment_binding(
    prepared: &PreparedMultiswapRelation,
    commitment: &Commitment,
) -> [u8; 32] {
    let layout = prepared.layout();
    let p = prepared.params();
    let profile = prepared.profile();
    let mut hasher = Hasher::new();
    hasher.update(MULTISWAP_BINDING_DOMAIN);
    hasher.update(MULTISWAP_STATEMENT_DOMAIN);
    hasher.update(prepared.statement_digest());
    hasher.update(&commitment.root);
    for value in [
        commitment.params.m,
        commitment.params.log_inv_rate,
        commitment.params.log_batch_size,
        layout.capacity(),
        layout.gate_vars(),
        layout.column_vars(),
        layout.high_gate_vars(),
        layout.assignment_len(),
        p.t,
        p.s,
        p.word_bits,
        MULTISWAP_VALUE_BITS,
        profile.reduction_grinding_bits(),
    ] {
        hasher.update(&(value as u64).to_le_bytes());
    }
    let (fingerprint_min, fingerprint_max) = profile.fingerprint_interval();
    let (reduction_min, reduction_max) = profile.reduction_interval();
    for bound in [fingerprint_min, fingerprint_max, reduction_min, reduction_max] {
        hasher.update(&bound.to_le_bytes());
    }
    hasher.update(&prepared.map().digest());
    *hasher.finalize().as_bytes()
}

/// Rebinds the derived terminal claim, its bitified image, and the Step 5.0
/// integer lift before the grinded reduction draw.
fn absorb_opening_claim(
    transcript: &mut impl Transcript,
    binding: &[u8; 32],
    terminal_claim: &ScaledMleEvaluationClaim<SpartanF2zField>,
    opening: &MultiswapOpeningClaim,
    mu_prime: &BigUint,
) {
    let mut hasher = Hasher::new();
    hasher.update(MULTISWAP_OPENING_CLAIM_DOMAIN);
    hasher.update(binding);
    hasher.update(&(terminal_claim.point().len() as u64).to_le_bytes());
    for coordinate in terminal_claim.point() {
        hasher.update(&coordinate.canonical_element_encoding());
    }
    hasher.update(&terminal_claim.scale().canonical_element_encoding());
    hasher.update(&terminal_claim.value().canonical_element_encoding());
    hasher.update(&opening.claimed_q.to_le_bytes());
    for weight in &opening.col_weights_q {
        hasher.update(&weight.to_le_bytes());
    }
    let mu_prime_bytes = mu_prime.to_bytes_le();
    hasher.update(&(mu_prime_bytes.len() as u64).to_le_bytes());
    hasher.update(&mu_prime_bytes);
    absorb_spartan_message(
        transcript,
        b"multiswap-opening-claim",
        hasher.finalize().as_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use crypto_primitives::FromWithConfig;
    use num_bigint::BigUint;

    use super::super::circuit::MultiswapDims;
    use super::*;
    use crate::transcript::Blake3Transcript;

    fn mini_setup() -> (
        PreparedMultiswapRelation,
        MultiswapAssignment,
        FlockCommitHint,
        LigProverConfig,
        LigVerifierConfig,
    ) {
        let circuit = MultiswapCircuit::build(MultiswapDims::mini()).unwrap();
        circuit.is_sat_integer().unwrap();
        let prepared = PreparedMultiswapRelation::new(&circuit).unwrap();
        let assignment = MultiswapAssignment::new(&circuit).unwrap();
        let (pc, vc) = multiswap_lig_configs(prepared.params()).unwrap();
        let rows = assignment.f2z_bit_rows();
        let hint = commit_multiswap_witness(prepared.params(), rows, &pc).unwrap();
        (prepared, assignment, hint, pc, vc)
    }

    /// Holds the shared env lock so tests that toggle transcript-shaping
    /// `F2Z_*` variables cannot flip them between our prove and verify.
    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::utils::QUAD_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn mini_multiswap_proof_roundtrips() {
        let _env = env_guard();
        let (prepared, assignment, hint, pc, vc) = mini_setup();
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_multiswap_mod_r1cs(
            &mut prover_transcript,
            &prepared,
            &assignment,
            &hint,
            &pc,
        )
        .unwrap();

        let mut verifier_transcript = Blake3Transcript::new();
        verify_multiswap_mod_r1cs(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &proof,
            &vc,
        )
        .unwrap();
    }

    #[test]
    fn mini_multiswap_rejects_a_tampered_spartan_claim() {
        let _env = env_guard();
        let (prepared, assignment, hint, pc, vc) = mini_setup();
        let mut prover_transcript = Blake3Transcript::new();
        let mut proof = prove_multiswap_mod_r1cs(
            &mut prover_transcript,
            &prepared,
            &assignment,
            &hint,
            &pc,
        )
        .unwrap();

        let mut transcript = Blake3Transcript::new();
        let fingerprint =
            sample_multiswap_fingerprint_context(&mut transcript, prepared.profile()).unwrap();
        proof.spartan.outer.az_mle_claim =
            SpartanF2zField::from_with_cfg(12345u64, fingerprint.field_config());
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(verify_multiswap_mod_r1cs(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &proof,
            &vc,
        )
        .is_err());
    }

    #[test]
    fn mini_multiswap_rejects_a_tampered_integer_lift() {
        let _env = env_guard();
        let (prepared, assignment, hint, pc, vc) = mini_setup();
        let mut prover_transcript = Blake3Transcript::new();
        let honest = prove_multiswap_mod_r1cs(
            &mut prover_transcript,
            &prepared,
            &assignment,
            &hint,
            &pc,
        )
        .unwrap();

        // Wrong residue class modulo Q: rejected deterministically.
        let mut wrong_class = honest.clone();
        wrong_class.mu_prime += BigUint::from(1u32);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(matches!(
            verify_multiswap_mod_r1cs(
                &mut verifier_transcript,
                &prepared,
                &hint.commitment,
                &wrong_class,
                &vc,
            ),
            Err(MultiswapError::InvalidIntegerLift)
        ));

        // Right class, wrong integer: the fresh reduction prime catches the
        // lie (up to the documented 2^-104 residual).
        let mut transcript = Blake3Transcript::new();
        let fingerprint =
            sample_multiswap_fingerprint_context(&mut transcript, prepared.profile()).unwrap();
        let mut wrong_lift = honest.clone();
        wrong_lift.mu_prime += BigUint::from(fingerprint.q());
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(verify_multiswap_mod_r1cs(
            &mut verifier_transcript,
            &prepared,
            &hint.commitment,
            &wrong_lift,
            &vc,
        )
        .is_err());
    }

    #[test]
    fn mini_multiswap_rejects_an_unsatisfied_witness() {
        // Corrupt one committed quotient: the committed integers no longer
        // satisfy the relation, so the prover's own consistency check, the
        // Spartan zerocheck, or the opening must reject.
        let _env = env_guard();
        let circuit = MultiswapCircuit::build(MultiswapDims::mini()).unwrap();
        let mut quos = circuit.quotients().to_vec();
        quos[0] += num_bigint::BigUint::from(1u32);
        let circuit = circuit.with_quotients_for_tests(quos);
        let prepared = PreparedMultiswapRelation::new(&circuit).unwrap();
        let assignment = MultiswapAssignment::new(&circuit).unwrap();
        let (pc, vc) = multiswap_lig_configs(prepared.params()).unwrap();
        let rows = assignment.f2z_bit_rows();
        let hint = commit_multiswap_witness(prepared.params(), rows, &pc).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let attempt = prove_multiswap_mod_r1cs(
            &mut prover_transcript,
            &prepared,
            &assignment,
            &hint,
            &pc,
        );
        if let Ok(proof) = attempt {
            let mut verifier_transcript = Blake3Transcript::new();
            assert!(verify_multiswap_mod_r1cs(
                &mut verifier_transcript,
                &prepared,
                &hint.commitment,
                &proof,
                &vc,
            )
            .is_err());
        }
    }
}
