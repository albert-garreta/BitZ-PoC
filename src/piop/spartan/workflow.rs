//! Generic composition of the Spartan PIOP with the F2Z integer PCS.
//!
//! Spartan always receives the field-valued assignment it constrains.  The
//! common prover witness is the committed integer witness `W`.  In
//! [`SpartanF2zOpening::Direct`] Spartan constrains that witness directly.  In
//! [`SpartanF2zOpening::Virtualized`] the variant supplies the virtual
//! assignment `h = M(W)`, where [`BinarySparseMatrix`] describes a structured
//! wordwise-XOR map supported by the F2Z virtual-opening backend.  Only `W` is
//! committed in either case.

use blake3::Hasher;
use crypto_primitives::crypto_bigint_monty::MontyField;
use flock_core::pcs::{
    commit::Commitment,
    ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig},
};

use crate::{
    ligerito::LOG_PACKING,
    ligerito_flock::{
        FlockCommitHint, IntEvalRsLigModQProof, IntEvalRsLigModQXorProof, VirtualXorClaim,
        VirtualXorVerifyClaim, prove_mle_eval_mod_q_ligerito,
        prove_mle_eval_mod_q_ligerito_claims_only, prove_mle_eval_mod_q_ligerito_with_virtual_xors,
        sha_lig_configs, verify_mle_eval_mod_q_ligerito,
        verify_mle_eval_mod_q_ligerito_claims_only,
        verify_mle_eval_mod_q_ligerito_with_virtual_xors,
    },
    pcs::{
        FQ_BITS, FQ_MOD, Fq, IntEvalParams, ProjectCanonicalU128, ShaF2Layout, eq_le_table_fq,
        fq_add, virtual_xor_params,
    },
    poly::mle::DenseMultilinearExtension,
    transcript::traits::Transcript,
};

use super::{
    PreparedConstraintMatrices, R1csProductMles, ScaledMleEvaluationClaim, SpartanField,
    SpartanPiopProof, absorb_spartan_message,
    f2z::{
        F2zOpeningClaim, SpartanF2zError, checked_pow2, f2z_generator, hash_code, packed_variables,
        profile_code, validate_commitment, validate_config_pair, validate_f2z_proof_shape,
    },
    prove_spartan_piop, verify_spartan_proof,
};

const ASSIGNMENT_BINDING_DOMAIN: &[u8] = b"f2z/spartan-f2z/generic-assignment/v1";
const OPENING_CLAIM_DOMAIN: &[u8] = b"f2z/spartan-f2z/generic-opening/v1";
const VIRTUALIZATION_MATRIX_DOMAIN: &[u8] = b"f2z/spartan-f2z/xor-matrix/v1";
const MIN_PRODUCTION_PACKED_VARS: usize = 15;

/// A Spartan field whose configured modulus is exactly F2Z's
/// `q = 2^100 - 15`.
///
/// Spartan itself remains generic over prime fields.  The current F2Z PCS has
/// a fixed read-off field, so the combined workflow additionally requires this
/// compatibility hook at the boundary.
pub trait F2zCompatibleField: SpartanField + ProjectCanonicalU128 {
    /// Returns true exactly for the runtime configuration with modulus
    /// [`FQ_MOD`].
    fn is_f2z_config(field_config: &Self::Config) -> bool;
}

impl<const LIMBS: usize> F2zCompatibleField for MontyField<LIMBS> {
    fn is_f2z_config(field_config: &Self::Config) -> bool {
        let modulus = field_config.modulus().get();
        let words = modulus.as_words();
        let expected_low = FQ_MOD as u64;
        let expected_high = (FQ_MOD >> 64) as u64;
        words.first().copied() == Some(expected_low)
            && words.get(1).copied() == Some(expected_high)
            && words.iter().skip(2).all(|&word| word == 0)
    }
}

/// Field-independent form of a scaled Spartan terminal claim after selecting
/// a maximum integer width.
///
/// This operation does not choose a physical F2Z tensor split.  The direct or
/// virtual opening compiler does that separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitifiedSpartanClaim {
    point: Vec<Fq>,
    scale: Fq,
    value: Fq,
    max_bits: usize,
}

impl BitifiedSpartanClaim {
    pub fn point(&self) -> &[Fq] {
        &self.point
    }

    pub const fn scale(&self) -> Fq {
        self.scale
    }

    pub const fn value(&self) -> Fq {
        self.value
    }

    pub const fn max_bits(&self) -> usize {
        self.max_bits
    }
}

/// Converts a generic Spartan terminal claim into the fixed F2Z read-off
/// field, retaining only the public maximum integer width.
pub fn bitify_spartan_claim<F>(
    claim: &ScaledMleEvaluationClaim<F>,
    max_bits: usize,
) -> Result<BitifiedSpartanClaim, SpartanF2zError>
where
    F: F2zCompatibleField,
{
    if !(1..=FQ_BITS).contains(&max_bits) {
        return Err(SpartanF2zError::InvalidBitWidth(max_bits));
    }
    validate_claim_field(claim)?;
    Ok(BitifiedSpartanClaim {
        point: claim
            .point()
            .iter()
            .map(|coordinate| Fq(coordinate.canonical_u128()))
            .collect(),
        scale: Fq(claim.scale().canonical_u128()),
        value: Fq(claim.value().canonical_u128()),
        max_bits,
    })
}

/// One row of a structured binary virtualization matrix.
///
/// At every trace position the output word is the XOR of `columns` from the
/// committed witness, followed by XOR with `constant`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinarySparseRow {
    columns: Box<[usize]>,
    constant: u128,
}

impl BinarySparseRow {
    pub fn new(columns: Vec<usize>, constant: u128) -> Self {
        Self {
            columns: columns.into_boxed_slice(),
            constant,
        }
    }

    pub fn columns(&self) -> &[usize] {
        &self.columns
    }

    pub const fn constant(&self) -> u128 {
        self.constant
    }

    fn zero() -> Self {
        Self::new(Vec::new(), 0)
    }
}

/// Public structured map `h = M(W)` supported by F2Z's virtual-XOR opener.
///
/// Rows are padded with public zero rows to a power of two.  The matrix is
/// sparse across committed word columns and is implicitly repeated over every
/// trace position; it is not a general dense binary matrix.
#[derive(Clone, Debug)]
pub struct BinarySparseMatrix {
    layout: ShaF2Layout,
    logical_row_count: usize,
    output_num_vars: usize,
    rows: Box<[BinarySparseRow]>,
    digest: [u8; 32],
}

impl BinarySparseMatrix {
    pub fn new(
        layout: ShaF2Layout,
        mut rows: Vec<BinarySparseRow>,
    ) -> Result<Self, SpartanF2zError> {
        validate_virtual_layout(&layout)?;
        if rows.is_empty() {
            return Err(SpartanF2zError::InvalidVirtualizationMatrix);
        }
        for row in &rows {
            if row.columns.iter().any(|&column| column >= layout.num_cols)
                || row.columns.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(SpartanF2zError::InvalidVirtualizationMatrix);
            }
        }

        let logical_row_count = rows.len();
        let padded_rows = logical_row_count
            .checked_next_power_of_two()
            .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?;
        rows.resize_with(padded_rows, BinarySparseRow::zero);
        let output_num_vars = padded_rows.trailing_zeros() as usize;
        let digest = virtualization_digest(&layout, logical_row_count, &rows)?;
        Ok(Self {
            layout,
            logical_row_count,
            output_num_vars,
            rows: rows.into_boxed_slice(),
            digest,
        })
    }

    pub const fn layout(&self) -> &ShaF2Layout {
        &self.layout
    }

    pub const fn logical_row_count(&self) -> usize {
        self.logical_row_count
    }

    pub const fn output_num_vars(&self) -> usize {
        self.output_num_vars
    }

    pub fn rows(&self) -> &[BinarySparseRow] {
        &self.rows
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn virtualized_num_vars(&self) -> Result<usize, SpartanF2zError> {
        self.layout
            .num_vars
            .checked_add(self.output_num_vars)
            .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)
    }
}

/// Prover input selecting how Spartan's assignment is tied to the F2Z
/// commitment.
///
/// The common witness argument to [`prove_spartan_and_f2z`] is always the
/// original committed witness `W`.  Direct mode uses it as Spartan's
/// assignment.  Only virtualized mode carries another assignment, namely the
/// already materialized `h = M(W)`.
pub enum SpartanF2zOpening<'a, F> {
    /// Spartan constrains the original committed integer witness `W`.
    Direct { params: &'a IntEvalParams },
    /// Spartan constrains `h = M(W)` while F2Z opens the commitment to `W`.
    Virtualized {
        virtualized_witness: DenseMultilinearExtension<F>,
        layout: &'a ShaF2Layout,
        matrix: &'a BinarySparseMatrix,
    },
}

impl<'a, F> SpartanF2zOpening<'a, F> {
    /// Returns the witness-free descriptor used by the verifier and transcript
    /// binding.
    pub const fn verifier_opening(&self) -> SpartanF2zVerifierOpening<'a> {
        match self {
            Self::Direct { params } => SpartanF2zVerifierOpening::Direct { params },
            Self::Virtualized { layout, matrix, .. } => {
                SpartanF2zVerifierOpening::Virtualized { layout, matrix }
            }
        }
    }
}

/// Public opening mode used by verification.  It deliberately contains no
/// prover witness.
#[derive(Clone, Copy)]
pub enum SpartanF2zVerifierOpening<'a> {
    Direct {
        params: &'a IntEvalParams,
    },
    Virtualized {
        layout: &'a ShaF2Layout,
        matrix: &'a BinarySparseMatrix,
    },
}

impl SpartanF2zVerifierOpening<'_> {
    pub const fn params(&self) -> &IntEvalParams {
        match self {
            Self::Direct { params } => params,
            Self::Virtualized { layout, .. } => &layout.p,
        }
    }
}

/// The F2Z half of a combined proof.
pub enum F2zOpeningProof {
    Direct(IntEvalRsLigModQProof),
    Virtualized {
        /// Present when identity rows were batched into the main committed
        /// witness opening; absent in claims-only mode.
        main_claimed: Option<Fq>,
        /// One claimed value for every active nontrivial XOR row, in matrix
        /// order.  These are checked against the Spartan terminal claim.
        xor_claimed: Vec<Fq>,
        proof: IntEvalRsLigModQXorProof,
    },
}

/// Combined Spartan PIOP and F2Z opening proof.
pub struct SpartanF2zProof<F> {
    pub spartan: SpartanPiopProof<F>,
    pub f2z: F2zOpeningProof,
}

impl<F> SpartanF2zProof<F> {
    pub const fn spartan(&self) -> &SpartanPiopProof<F> {
        &self.spartan
    }

    pub const fn f2z(&self) -> &F2zOpeningProof {
        &self.f2z
    }
}

/// Owned weights for a direct F2Z read-off.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectOpeningWeights {
    row_weights_q: Vec<u128>,
    col_weights: Vec<Fq>,
}

impl DirectOpeningWeights {
    pub fn row_weights_q(&self) -> &[u128] {
        &self.row_weights_q
    }

    pub fn col_weights(&self) -> &[Fq] {
        &self.col_weights
    }
}

/// One compiled nontrivial wordwise-XOR opening.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledVirtualXorClaim {
    output_row: usize,
    columns: Box<[usize]>,
    constant: u128,
    row_weights_q: Vec<u128>,
}

impl CompiledVirtualXorClaim {
    pub const fn output_row(&self) -> usize {
        self.output_row
    }

    pub fn columns(&self) -> &[usize] {
        &self.columns
    }

    pub const fn constant(&self) -> u128 {
        self.constant
    }

    pub fn row_weights_q(&self) -> &[u128] {
        &self.row_weights_q
    }
}

/// Result of applying the adjoint `M^T` to a bitified Spartan claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualizedF2zOpening {
    main: Option<DirectOpeningWeights>,
    xor_col_weights: Vec<Fq>,
    xors: Vec<CompiledVirtualXorClaim>,
}

impl VirtualizedF2zOpening {
    pub const fn main(&self) -> Option<&DirectOpeningWeights> {
        self.main.as_ref()
    }

    pub fn xor_col_weights(&self) -> &[Fq] {
        &self.xor_col_weights
    }

    pub fn xors(&self) -> &[CompiledVirtualXorClaim] {
        &self.xors
    }
}

/// Compiles a bitified claim directly against an integer witness table.
pub fn direct_opening_from_spartan_claim(
    claim: &BitifiedSpartanClaim,
    params: &IntEvalParams,
) -> Result<F2zOpeningClaim, SpartanF2zError> {
    validate_direct_geometry(claim, params)?;
    let (low, high) = claim.point.split_at(params.s);
    let row_weights_q = eq_le_table_fq(high)
        .into_iter()
        .map(|weight| weight.0)
        .collect();
    let col_weights = eq_le_table_fq(low)
        .into_iter()
        .map(|weight| claim.scale * weight)
        .collect();
    Ok(F2zOpeningClaim::new(
        row_weights_q,
        col_weights,
        claim.value,
    ))
}

/// Applies the public structured adjoint `M^T` to a bitified Spartan claim.
///
/// Identity rows are batched into one main opening.  Every other nonzero row
/// becomes a virtual-XOR claim.  A public zero row contributes nothing.
pub fn virtual_opening_from_spartan_claim(
    claim: &BitifiedSpartanClaim,
    matrix: &BinarySparseMatrix,
) -> Result<VirtualizedF2zOpening, SpartanF2zError> {
    validate_virtual_claim_geometry(claim, matrix)?;
    let layout = matrix.layout();
    let trace_vars = layout.num_vars;
    let (trace_point, output_point) = claim.point.split_at(trace_vars);
    let (trace_low, trace_high) = trace_point.split_at(layout.p.s);
    let eq_low = eq_le_table_fq(trace_low);
    let eq_high = eq_le_table_fq(trace_high);
    let eq_output = eq_le_table_fq(output_point);
    let scaled_cols = eq_low
        .iter()
        .copied()
        .map(|weight| claim.scale * weight)
        .collect::<Vec<_>>();

    let mut main_rows = vec![0_u128; checked_pow2(layout.p.t)?];
    let x_params = virtual_xor_params(layout);
    let mut xors = Vec::new();
    for (output_row, row) in matrix.rows.iter().enumerate() {
        let gamma = eq_output[output_row];
        if gamma == Fq(0) || (row.columns.is_empty() && row.constant == 0) {
            continue;
        }
        if row.columns.len() == 1 && row.constant == 0 {
            fill_virtual_weights(
                &mut main_rows,
                gamma,
                &eq_high,
                claim.max_bits,
                layout.tw,
                Some((row.columns[0], layout.log_cols)),
            )?;
        } else {
            let mut row_weights_q = vec![0_u128; checked_pow2(x_params.t)?];
            fill_virtual_weights(
                &mut row_weights_q,
                gamma,
                &eq_high,
                claim.max_bits,
                layout.tw,
                None,
            )?;
            xors.push(CompiledVirtualXorClaim {
                output_row,
                columns: row.columns.clone(),
                constant: row.constant,
                row_weights_q,
            });
        }
    }

    let main = if main_rows.iter().any(|&weight| weight != 0) {
        Some(DirectOpeningWeights {
            row_weights_q: main_rows,
            col_weights: scaled_cols.clone(),
        })
    } else if xors.is_empty() {
        // The map is zero at this output point.  Keep the F2Z commitment in
        // the proof with a deterministic zero-valued dummy main functional.
        main_rows[0] = 1;
        Some(DirectOpeningWeights {
            row_weights_q: main_rows,
            col_weights: vec![Fq(0); eq_low.len()],
        })
    } else {
        None
    };

    Ok(VirtualizedF2zOpening {
        main,
        xor_col_weights: scaled_cols,
        xors,
    })
}

/// Proves Spartan and discharges its terminal assignment claim with F2Z.
///
/// `original_witness` is always the base witness `W` represented by the F2Z
/// commitment hint.  Direct mode also uses it as Spartan's assignment.
/// Virtualized mode instead takes Spartan's assignment `h = M(W)` from the
/// opening variant.  The supplied product MLEs must correspond to whichever
/// assignment Spartan receives.
#[allow(clippy::too_many_arguments)]
pub fn prove_spartan_and_f2z<T, F>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<F>,
    original_witness: DenseMultilinearExtension<F>,
    products: R1csProductMles<F>,
    max_bits: usize,
    opening_mode: SpartanF2zOpening<'_, F>,
    hint: &FlockCommitHint,
) -> Result<SpartanF2zProof<F>, SpartanF2zError>
where
    T: Transcript + Send,
    F: F2zCompatibleField,
{
    let verifier_opening = opening_mode.verifier_opening();
    validate_original_witness_shape(&original_witness, max_bits, verifier_opening)?;
    let (spartan_witness, virtual_source_witness) = match opening_mode {
        SpartanF2zOpening::Direct { .. } => (original_witness, None),
        SpartanF2zOpening::Virtualized {
            virtualized_witness,
            ..
        } => (virtualized_witness, Some(original_witness)),
    };
    validate_prover_workflow(matrices, &spartan_witness, max_bits, verifier_opening, hint)?;
    let params = verifier_opening.params();
    let (pc, _) = configs_for_params(params)?;
    let assignment_binding =
        assignment_binding(matrices, max_bits, verifier_opening, &hint.commitment)?;

    let (spartan, terminal_claim) = {
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_prove");
        prove_spartan_piop(
            transcript,
            matrices,
            &assignment_binding,
            products,
            spartan_witness,
        )?
    };
    let bitified = bitify_spartan_claim(&terminal_claim, max_bits)?;

    let f2z = match verifier_opening {
        SpartanF2zVerifierOpening::Direct { params } => {
            let opening = direct_opening_from_spartan_claim(&bitified, params)?;
            absorb_direct_opening(
                transcript,
                matrices,
                &assignment_binding,
                &bitified,
                params,
                &opening,
            )?;
            let proof = {
                let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prove");
                prove_mle_eval_mod_q_ligerito(
                    transcript,
                    hint,
                    params,
                    opening.row_weights_q(),
                    FQ_BITS,
                    f2z_generator(),
                    &pc,
                )
            };
            F2zOpeningProof::Direct(proof)
        }
        SpartanF2zVerifierOpening::Virtualized { layout, matrix } => {
            let compiled = virtual_opening_from_spartan_claim(&bitified, matrix)?;
            let source_witness = virtual_source_witness
                .as_ref()
                .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?;
            let (main_claimed, xor_claimed) =
                evaluate_virtual_components(&compiled, matrix, source_witness)?;
            validate_component_sum(&bitified, main_claimed, &xor_claimed)?;
            absorb_virtual_opening(
                transcript,
                matrices,
                &assignment_binding,
                &bitified,
                matrix,
                &compiled,
                main_claimed,
                &xor_claimed,
            )?;
            let prover_claims = compiled
                .xors
                .iter()
                .map(|claim| VirtualXorClaim {
                    cols: &claim.columns,
                    constant: claim.constant,
                    external_rows: None,
                    row_weights_q: &claim.row_weights_q,
                })
                .collect::<Vec<_>>();
            let proof = {
                let _scope = crate::utils::prof::scope("spartan-f2z:f2z_prove");
                match &compiled.main {
                    Some(main) => prove_mle_eval_mod_q_ligerito_with_virtual_xors(
                        transcript,
                        hint,
                        layout,
                        &main.row_weights_q,
                        FQ_BITS,
                        &prover_claims,
                        f2z_generator(),
                        &pc,
                    ),
                    None => prove_mle_eval_mod_q_ligerito_claims_only(
                        transcript,
                        hint,
                        layout,
                        FQ_BITS,
                        &prover_claims,
                        f2z_generator(),
                        &pc,
                    ),
                }
            };
            F2zOpeningProof::Virtualized {
                main_claimed,
                xor_claimed,
                proof,
            }
        }
    };

    Ok(SpartanF2zProof { spartan, f2z })
}

/// Verifies both Spartan and its direct or virtualized F2Z discharge.
#[allow(clippy::too_many_arguments)]
pub fn verify_spartan_and_f2z<T, F>(
    transcript: &mut T,
    matrices: &PreparedConstraintMatrices<F>,
    max_bits: usize,
    opening_mode: SpartanF2zVerifierOpening<'_>,
    commitment: &Commitment,
    proof: &SpartanF2zProof<F>,
) -> Result<(), SpartanF2zError>
where
    T: Transcript + Send,
    F: F2zCompatibleField,
{
    validate_verifier_workflow(matrices, max_bits, opening_mode, commitment, &proof.f2z)?;
    let params = opening_mode.params();
    let (_, vc) = configs_for_params(params)?;
    let assignment_binding = assignment_binding(matrices, max_bits, opening_mode, commitment)?;
    let terminal_claim = {
        let _scope = crate::utils::prof::scope("spartan-f2z:spartan_verify");
        verify_spartan_proof(transcript, matrices, &assignment_binding, &proof.spartan)?
    };
    let bitified = bitify_spartan_claim(&terminal_claim, max_bits)?;

    match (opening_mode, &proof.f2z) {
        (SpartanF2zVerifierOpening::Direct { params }, F2zOpeningProof::Direct(f2z)) => {
            let opening = direct_opening_from_spartan_claim(&bitified, params)?;
            absorb_direct_opening(
                transcript,
                matrices,
                &assignment_binding,
                &bitified,
                params,
                &opening,
            )?;
            let _scope = crate::utils::prof::scope("spartan-f2z:f2z_verify");
            verify_mle_eval_mod_q_ligerito(
                transcript,
                commitment,
                f2z,
                params,
                opening.row_weights_q(),
                opening.col_weights(),
                f2z_generator(),
                opening.claimed(),
                FQ_BITS,
                &vc,
            )
            .map_err(SpartanF2zError::F2z)
        }
        (
            SpartanF2zVerifierOpening::Virtualized { layout, matrix },
            F2zOpeningProof::Virtualized {
                main_claimed,
                xor_claimed,
                proof: f2z,
            },
        ) => {
            let compiled = virtual_opening_from_spartan_claim(&bitified, matrix)?;
            validate_component_shape(&compiled, *main_claimed, xor_claimed)?;
            validate_component_sum(&bitified, *main_claimed, xor_claimed)?;
            absorb_virtual_opening(
                transcript,
                matrices,
                &assignment_binding,
                &bitified,
                matrix,
                &compiled,
                *main_claimed,
                xor_claimed,
            )?;
            let verify_claims = compiled
                .xors
                .iter()
                .zip(xor_claimed)
                .map(|(claim, &claimed)| VirtualXorVerifyClaim {
                    cols: &claim.columns,
                    constant: claim.constant,
                    has_external: false,
                    row_weights_q: &claim.row_weights_q,
                    col_weights: &compiled.xor_col_weights,
                    claimed,
                })
                .collect::<Vec<_>>();
            let _scope = crate::utils::prof::scope("spartan-f2z:f2z_verify");
            let obligations = match (&compiled.main, main_claimed) {
                (Some(main), Some(claimed)) => verify_mle_eval_mod_q_ligerito_with_virtual_xors(
                    transcript,
                    commitment,
                    f2z,
                    layout,
                    &main.row_weights_q,
                    &main.col_weights,
                    f2z_generator(),
                    *claimed,
                    FQ_BITS,
                    &verify_claims,
                    &vc,
                ),
                (None, None) => verify_mle_eval_mod_q_ligerito_claims_only(
                    transcript,
                    commitment,
                    f2z,
                    layout,
                    f2z_generator(),
                    FQ_BITS,
                    &verify_claims,
                    &vc,
                ),
                _ => return Err(SpartanF2zError::InvalidVirtualComponentClaims),
            }
            .map_err(SpartanF2zError::F2z)?;
            if obligations.is_empty() {
                Ok(())
            } else {
                Err(SpartanF2zError::InvalidVirtualComponentClaims)
            }
        }
        _ => Err(SpartanF2zError::ProofModeMismatch),
    }
}

fn validate_prover_workflow<F>(
    matrices: &PreparedConstraintMatrices<F>,
    witness: &DenseMultilinearExtension<F>,
    max_bits: usize,
    opening_mode: SpartanF2zVerifierOpening<'_>,
    hint: &FlockCommitHint,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    validate_relation_field(matrices)?;
    validate_assignment_shape(matrices, witness, opening_mode)?;
    let params = opening_mode.params();
    validate_opening_geometry(max_bits, opening_mode)?;
    let (pc, vc) = configs_for_params(params)?;
    validate_config_pair(params, &pc, &vc)?;
    validate_commitment(params, &hint.commitment, &pc)?;
    validate_bit_rows(params, hint.rows())
}

fn validate_verifier_workflow<F>(
    matrices: &PreparedConstraintMatrices<F>,
    max_bits: usize,
    opening_mode: SpartanF2zVerifierOpening<'_>,
    commitment: &Commitment,
    proof: &F2zOpeningProof,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    validate_relation_field(matrices)?;
    validate_relation_opening_shape(matrices, opening_mode)?;
    validate_opening_geometry(max_bits, opening_mode)?;
    let params = opening_mode.params();
    let (pc, vc) = configs_for_params(params)?;
    validate_config_pair(params, &pc, &vc)?;
    validate_commitment(params, commitment, &pc)?;
    match (opening_mode, proof) {
        (SpartanF2zVerifierOpening::Direct { params }, F2zOpeningProof::Direct(proof)) => {
            validate_f2z_proof_shape(params, proof)
        }
        (
            SpartanF2zVerifierOpening::Virtualized { .. },
            F2zOpeningProof::Virtualized {
                main_claimed,
                xor_claimed,
                ..
            },
        ) if main_claimed.is_none_or(|value| value.0 < FQ_MOD)
            && xor_claimed.iter().all(|value| value.0 < FQ_MOD) =>
        {
            Ok(())
        }
        (SpartanF2zVerifierOpening::Virtualized { .. }, F2zOpeningProof::Virtualized { .. }) => {
            Err(SpartanF2zError::InvalidVirtualComponentClaims)
        }
        _ => Err(SpartanF2zError::ProofModeMismatch),
    }
}

fn validate_relation_field<F>(
    matrices: &PreparedConstraintMatrices<F>,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    if !F::is_f2z_config(matrices.config()) {
        return Err(SpartanF2zError::UnsupportedFieldModulus);
    }
    Ok(())
}

fn validate_assignment_shape<F>(
    matrices: &PreparedConstraintMatrices<F>,
    witness: &DenseMultilinearExtension<F>,
    opening_mode: SpartanF2zVerifierOpening<'_>,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    validate_relation_opening_shape(matrices, opening_mode)?;
    let expected_len = checked_pow2(witness.num_vars)?;
    if witness.num_vars != matrices.num_column_vars() || witness.evaluations.len() != expected_len {
        return Err(SpartanF2zError::InvalidVirtualizedWitness);
    }
    Ok(())
}

fn validate_relation_opening_shape<F>(
    matrices: &PreparedConstraintMatrices<F>,
    opening_mode: SpartanF2zVerifierOpening<'_>,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let expected_vars = match opening_mode {
        SpartanF2zVerifierOpening::Direct { params } => params
            .t
            .checked_add(params.s)
            .ok_or(SpartanF2zError::InvalidDirectGeometry)?,
        SpartanF2zVerifierOpening::Virtualized { matrix, .. } => matrix.virtualized_num_vars()?,
    };
    if expected_vars != matrices.num_column_vars() {
        return Err(match opening_mode {
            SpartanF2zVerifierOpening::Direct { .. } => SpartanF2zError::InvalidDirectGeometry,
            SpartanF2zVerifierOpening::Virtualized { .. } => {
                SpartanF2zError::InvalidVirtualizedWitness
            }
        });
    }
    Ok(())
}

fn validate_opening_geometry(
    max_bits: usize,
    opening_mode: SpartanF2zVerifierOpening<'_>,
) -> Result<(), SpartanF2zError> {
    if !(1..=FQ_BITS).contains(&max_bits) {
        return Err(SpartanF2zError::InvalidBitWidth(max_bits));
    }
    match opening_mode {
        SpartanF2zVerifierOpening::Direct { params } => {
            validate_params(params)?;
            if params.word_bits != max_bits {
                return Err(SpartanF2zError::InvalidDirectGeometry);
            }
        }
        SpartanF2zVerifierOpening::Virtualized { layout, matrix } => {
            validate_virtual_opening_geometry(max_bits, layout, matrix)?;
        }
    }
    Ok(())
}

fn validate_original_witness_shape<F>(
    witness: &DenseMultilinearExtension<F>,
    max_bits: usize,
    opening_mode: SpartanF2zVerifierOpening<'_>,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    validate_opening_geometry(max_bits, opening_mode)?;
    let (expected_vars, error) = match opening_mode {
        SpartanF2zVerifierOpening::Direct { params } => (
            params
                .t
                .checked_add(params.s)
                .ok_or(SpartanF2zError::InvalidDirectGeometry)?,
            SpartanF2zError::InvalidDirectGeometry,
        ),
        SpartanF2zVerifierOpening::Virtualized { layout, .. } => (
            layout
                .num_vars
                .checked_add(layout.log_cols)
                .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?,
            SpartanF2zError::InvalidVirtualizedWitness,
        ),
    };
    let expected_len = checked_pow2(expected_vars)?;
    if witness.num_vars != expected_vars || witness.evaluations.len() != expected_len {
        return Err(error);
    }

    let upper_bound = (max_bits < u128::BITS as usize).then(|| 1_u128 << max_bits);
    let values_are_valid = witness.evaluations.iter().all(|value| {
        F::is_f2z_config(value.cfg())
            && value.validate_element().is_ok()
            && upper_bound.is_none_or(|bound| value.canonical_u128() < bound)
    });
    if !values_are_valid {
        return Err(error);
    }
    Ok(())
}

fn validate_virtual_opening_geometry(
    max_bits: usize,
    layout: &ShaF2Layout,
    matrix: &BinarySparseMatrix,
) -> Result<(), SpartanF2zError> {
    validate_virtual_layout(layout)?;
    validate_virtual_layout(matrix.layout())?;
    if !same_virtual_layout(layout, matrix.layout()) {
        return Err(SpartanF2zError::InvalidVirtualizationMatrix);
    }
    let word_bits = checked_pow2(layout.bit_vars)?;
    if max_bits != word_bits {
        return Err(SpartanF2zError::InvalidBitWidth(max_bits));
    }
    let bound = (max_bits < u128::BITS as usize).then(|| 1_u128 << max_bits);
    if bound.is_some_and(|bound| matrix.rows.iter().any(|row| row.constant >= bound)) {
        return Err(SpartanF2zError::InvalidVirtualizationMatrix);
    }
    Ok(())
}

fn same_virtual_layout(left: &ShaF2Layout, right: &ShaF2Layout) -> bool {
    left.p.t == right.p.t
        && left.p.s == right.p.s
        && left.p.word_bits == right.p.word_bits
        && left.num_cols == right.num_cols
        && left.log_cols == right.log_cols
        && left.bit_vars == right.bit_vars
        && left.num_vars == right.num_vars
        && left.tw == right.tw
        && left.x_fold_extra == right.x_fold_extra
}

fn validate_direct_geometry(
    claim: &BitifiedSpartanClaim,
    params: &IntEvalParams,
) -> Result<(), SpartanF2zError> {
    validate_params(params)?;
    if params.word_bits != claim.max_bits
        || claim.point.len()
            != params
                .t
                .checked_add(params.s)
                .ok_or(SpartanF2zError::InvalidDirectGeometry)?
    {
        return Err(SpartanF2zError::InvalidDirectGeometry);
    }
    Ok(())
}

fn validate_virtual_claim_geometry(
    claim: &BitifiedSpartanClaim,
    matrix: &BinarySparseMatrix,
) -> Result<(), SpartanF2zError> {
    validate_virtual_opening_geometry(claim.max_bits, matrix.layout(), matrix)?;
    if claim.point.len() != matrix.virtualized_num_vars()? {
        return Err(SpartanF2zError::InvalidClaimPoint);
    }
    Ok(())
}

fn validate_params(params: &IntEvalParams) -> Result<(), SpartanF2zError> {
    if !params.word_bits.is_power_of_two() || params.word_bits > u128::BITS as usize {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    let row_bit_vars = params
        .t
        .checked_add(params.word_bits.trailing_zeros() as usize)
        .ok_or(SpartanF2zError::InvalidF2zParameters)?;
    if row_bit_vars < LOG_PACKING
        || params.t.saturating_add(params.word_bits) > 126
        || params.t >= usize::BITS as usize
        || params.s >= usize::BITS as usize
    {
        return Err(SpartanF2zError::InvalidF2zParameters);
    }
    checked_pow2(params.t)?;
    checked_pow2(params.s)?;
    packed_variables(params)?;
    Ok(())
}

fn validate_virtual_layout(layout: &ShaF2Layout) -> Result<(), SpartanF2zError> {
    validate_params(&layout.p)?;
    if layout.p.word_bits != 1
        || layout.x_fold_extra != 0
        || layout.p.s == 0
        || layout.bit_vars > 7
        || layout.num_cols == 0
    {
        return Err(SpartanF2zError::InvalidVirtualizationMatrix);
    }
    let expected_log_cols = if layout.num_cols <= 1 {
        0
    } else {
        (usize::BITS - (layout.num_cols - 1).leading_zeros()) as usize
    };
    let padded_cols = checked_pow2(layout.log_cols)?;
    let expected_t = layout
        .bit_vars
        .checked_add(layout.log_cols)
        .and_then(|value| value.checked_add(layout.tw))
        .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?;
    let expected_num_vars = layout
        .tw
        .checked_add(layout.p.s)
        .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?;
    if layout.log_cols != expected_log_cols
        || layout.num_cols > padded_cols
        || layout.p.t != expected_t
        || layout.num_vars != expected_num_vars
    {
        return Err(SpartanF2zError::InvalidVirtualizationMatrix);
    }
    let x_t = layout
        .bit_vars
        .checked_add(layout.tw)
        .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?;
    if x_t >= usize::BITS as usize || x_t.saturating_add(1) > 126 {
        return Err(SpartanF2zError::InvalidVirtualizationMatrix);
    }
    checked_pow2(x_t)?;
    Ok(())
}

fn validate_claim_field<F>(claim: &ScaledMleEvaluationClaim<F>) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let valid = |value: &F| {
        F::is_f2z_config(value.cfg())
            && value.validate_element().is_ok()
            && value.canonical_u128() < FQ_MOD
    };
    if !valid(claim.scale())
        || !valid(claim.value())
        || claim.point().iter().any(|coordinate| !valid(coordinate))
    {
        return Err(SpartanF2zError::ClaimFieldMismatch);
    }
    Ok(())
}

fn validate_bit_rows(params: &IntEvalParams, rows: &[Vec<u64>]) -> Result<(), SpartanF2zError> {
    let row_bits = checked_pow2(
        params
            .t
            .checked_add(params.word_bits.trailing_zeros() as usize)
            .ok_or(SpartanF2zError::InvalidBitRows)?,
    )?;
    let columns = checked_pow2(params.s)?;
    if rows.len() != columns || row_bits % 64 != 0 {
        return Err(SpartanF2zError::InvalidBitRows);
    }
    let words = row_bits / 64;
    if rows.iter().any(|row| row.len() != words) {
        return Err(SpartanF2zError::InvalidBitRows);
    }
    Ok(())
}

fn configs_for_params(
    params: &IntEvalParams,
) -> Result<(LigProverConfig, LigVerifierConfig), SpartanF2zError> {
    validate_params(params)?;
    let packed = packed_variables(params)?;
    if packed < MIN_PRODUCTION_PACKED_VARS {
        return Err(SpartanF2zError::UnauditedF2zParameters);
    }
    sha_lig_configs(packed).map_err(SpartanF2zError::LigeritoConfig)
}

#[allow(clippy::arithmetic_side_effects)]
fn fill_virtual_weights(
    weights: &mut [u128],
    output_factor: Fq,
    eq_high: &[Fq],
    max_bits: usize,
    trace_high_vars: usize,
    source_column: Option<(usize, usize)>,
) -> Result<(), SpartanF2zError> {
    let high_count = checked_pow2(trace_high_vars)?;
    if eq_high.len() != high_count {
        return Err(SpartanF2zError::InvalidVirtualizationMatrix);
    }
    let mut bit_weight = Fq(1);
    for bit in 0..max_bits {
        for (trace_high, equality_weight) in eq_high.iter().copied().enumerate() {
            let base = match source_column {
                Some((column, log_columns)) => {
                    let bit_column = bit
                        .checked_mul(checked_pow2(log_columns)?)
                        .and_then(|value| value.checked_add(column))
                        .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?;
                    bit_column
                        .checked_mul(high_count)
                        .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?
                }
                None => bit
                    .checked_mul(high_count)
                    .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?,
            };
            let index = base
                .checked_add(trace_high)
                .ok_or(SpartanF2zError::InvalidVirtualizationMatrix)?;
            let Some(weight) = weights.get_mut(index) else {
                return Err(SpartanF2zError::InvalidVirtualizationMatrix);
            };
            let contribution = output_factor * bit_weight * equality_weight;
            *weight = fq_add(*weight, contribution.0);
        }
        bit_weight = bit_weight + bit_weight;
    }
    Ok(())
}

fn evaluate_virtual_components<F>(
    opening: &VirtualizedF2zOpening,
    matrix: &BinarySparseMatrix,
    original_witness: &DenseMultilinearExtension<F>,
) -> Result<(Option<Fq>, Vec<Fq>), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let main_claimed = opening
        .main
        .as_ref()
        .map(|main| {
            evaluate_committed_word_claim(
                original_witness,
                matrix.layout(),
                &main.row_weights_q,
                &main.col_weights,
            )
        })
        .transpose()?;
    let mut xor_claimed = Vec::with_capacity(opening.xors.len());
    for claim in &opening.xors {
        xor_claimed.push(evaluate_virtual_xor_claim(
            original_witness,
            matrix.layout(),
            claim,
            &claim.row_weights_q,
            &opening.xor_col_weights,
        )?);
    }
    Ok((main_claimed, xor_claimed))
}

fn evaluate_committed_word_claim<F>(
    witness: &DenseMultilinearExtension<F>,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[Fq],
) -> Result<Fq, SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let trace_count = checked_pow2(layout.num_vars)?;
    let padded_columns = checked_pow2(layout.log_cols)?;
    let clear_count = checked_pow2(layout.p.s)?;
    if witness.evaluations.len() != trace_count.saturating_mul(padded_columns)
        || col_weights.len() != clear_count
        || row_weights_q.len() != checked_pow2(layout.p.t)?
    {
        return Err(SpartanF2zError::InvalidVirtualizedWitness);
    }
    let clear_mask = clear_count - 1;
    let mut result = Fq(0);
    for column in 0..padded_columns {
        for trace in 0..trace_count {
            let mut word = witness.evaluations[column * trace_count + trace].canonical_u128();
            let clear = trace & clear_mask;
            let trace_high = trace >> layout.p.s;
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let row = bit
                    .checked_shl(layout.log_cols as u32)
                    .and_then(|value| value.checked_add(column))
                    .and_then(|value| value.checked_shl(layout.tw as u32))
                    .and_then(|value| value.checked_add(trace_high))
                    .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?;
                let weight = *row_weights_q
                    .get(row)
                    .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?;
                result = result + Fq(weight) * col_weights[clear];
                word &= word - 1;
            }
        }
    }
    Ok(result)
}

fn evaluate_virtual_xor_claim<F>(
    witness: &DenseMultilinearExtension<F>,
    layout: &ShaF2Layout,
    claim: &CompiledVirtualXorClaim,
    row_weights_q: &[u128],
    col_weights: &[Fq],
) -> Result<Fq, SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let trace_count = checked_pow2(layout.num_vars)?;
    let padded_columns = checked_pow2(layout.log_cols)?;
    let clear_count = checked_pow2(layout.p.s)?;
    let xor_params = virtual_xor_params(layout);
    if witness.evaluations.len() != trace_count.saturating_mul(padded_columns)
        || col_weights.len() != clear_count
        || row_weights_q.len() != checked_pow2(xor_params.t)?
    {
        return Err(SpartanF2zError::InvalidVirtualizedWitness);
    }
    let clear_mask = clear_count - 1;
    let mut result = Fq(0);
    for trace in 0..trace_count {
        let mut word = claim.constant;
        for &column in &claim.columns {
            let value = witness
                .evaluations
                .get(column * trace_count + trace)
                .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?;
            word ^= value.canonical_u128();
        }
        let clear = trace & clear_mask;
        let trace_high = trace >> layout.p.s;
        while word != 0 {
            let bit = word.trailing_zeros() as usize;
            let row = bit
                .checked_shl(layout.tw as u32)
                .and_then(|value| value.checked_add(trace_high))
                .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?;
            let weight = *row_weights_q
                .get(row)
                .ok_or(SpartanF2zError::InvalidVirtualizedWitness)?;
            result = result + Fq(weight) * col_weights[clear];
            word &= word - 1;
        }
    }
    Ok(result)
}

#[cfg(test)]
fn evaluate_bit_rows(
    rows: &[Vec<u64>],
    row_weights_q: &[u128],
    col_weights: &[Fq],
) -> Result<Fq, SpartanF2zError> {
    if rows.len() != col_weights.len()
        || rows
            .iter()
            .any(|row| row.len().saturating_mul(64) < row_weights_q.len())
    {
        return Err(SpartanF2zError::InvalidBitRows);
    }
    let mut value = Fq(0);
    for (row, &column_weight) in rows.iter().zip(col_weights) {
        let mut row_value = Fq(0);
        for (word_index, &word) in row.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let bit = bits.trailing_zeros() as usize;
                let index = word_index * 64 + bit;
                if index < row_weights_q.len() {
                    row_value = row_value + Fq(row_weights_q[index]);
                }
                bits &= bits - 1;
            }
        }
        value = value + column_weight * row_value;
    }
    Ok(value)
}

fn validate_component_shape(
    opening: &VirtualizedF2zOpening,
    main_claimed: Option<Fq>,
    xor_claimed: &[Fq],
) -> Result<(), SpartanF2zError> {
    if opening.main.is_some() != main_claimed.is_some()
        || opening.xors.len() != xor_claimed.len()
        || main_claimed.is_some_and(|value| value.0 >= FQ_MOD)
        || xor_claimed.iter().any(|value| value.0 >= FQ_MOD)
    {
        return Err(SpartanF2zError::InvalidVirtualComponentClaims);
    }
    Ok(())
}

fn validate_component_sum(
    claim: &BitifiedSpartanClaim,
    main_claimed: Option<Fq>,
    xor_claimed: &[Fq],
) -> Result<(), SpartanF2zError> {
    let sum = xor_claimed
        .iter()
        .copied()
        .fold(main_claimed.unwrap_or(Fq(0)), |sum, value| sum + value);
    if sum != claim.value {
        return Err(SpartanF2zError::VirtualClaimMismatch);
    }
    Ok(())
}

fn assignment_binding<F>(
    matrices: &PreparedConstraintMatrices<F>,
    max_bits: usize,
    opening_mode: SpartanF2zVerifierOpening<'_>,
    commitment: &Commitment,
) -> Result<[u8; 32], SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let mut hasher = Hasher::new();
    hasher.update(ASSIGNMENT_BINDING_DOMAIN);
    hasher.update(&commitment.root);
    hasher.update(matrices.field_modulus_encoding());
    hasher.update(matrices.digest());
    hash_commitment_params(&mut hasher, commitment)?;
    hash_usize(&mut hasher, max_bits)?;
    hash_params(&mut hasher, opening_mode.params())?;
    match opening_mode {
        SpartanF2zVerifierOpening::Direct { .. } => {
            hasher.update(&[0]);
        }
        SpartanF2zVerifierOpening::Virtualized { layout, matrix } => {
            hasher.update(&[1]);
            hash_layout(&mut hasher, layout)?;
            hasher.update(matrix.digest());
        }
    }
    Ok(*hasher.finalize().as_bytes())
}

fn absorb_direct_opening<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    assignment_binding: &[u8; 32],
    claim: &BitifiedSpartanClaim,
    params: &IntEvalParams,
    opening: &F2zOpeningClaim,
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let mut hasher = opening_digest_prefix(matrices, assignment_binding, claim)?;
    hasher.update(&[0]);
    hash_params(&mut hasher, params)?;
    hash_u128s(&mut hasher, opening.row_weights_q())?;
    hash_fqs(&mut hasher, opening.col_weights())?;
    hasher.update(&opening.claimed().0.to_le_bytes());
    absorb_spartan_message(
        transcript,
        OPENING_CLAIM_DOMAIN,
        hasher.finalize().as_bytes(),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn absorb_virtual_opening<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    assignment_binding: &[u8; 32],
    claim: &BitifiedSpartanClaim,
    matrix: &BinarySparseMatrix,
    opening: &VirtualizedF2zOpening,
    main_claimed: Option<Fq>,
    xor_claimed: &[Fq],
) -> Result<(), SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let mut hasher = opening_digest_prefix(matrices, assignment_binding, claim)?;
    hasher.update(&[1]);
    hasher.update(matrix.digest());
    match (&opening.main, main_claimed) {
        (Some(main), Some(claimed)) => {
            hasher.update(&[1]);
            hash_u128s(&mut hasher, &main.row_weights_q)?;
            hash_fqs(&mut hasher, &main.col_weights)?;
            hasher.update(&claimed.0.to_le_bytes());
        }
        (None, None) => {
            hasher.update(&[0]);
        }
        _ => return Err(SpartanF2zError::InvalidVirtualComponentClaims),
    }
    hash_usize(&mut hasher, opening.xors.len())?;
    for (xor, &claimed) in opening.xors.iter().zip(xor_claimed) {
        hash_usize(&mut hasher, xor.output_row)?;
        hash_usizes(&mut hasher, &xor.columns)?;
        hasher.update(&xor.constant.to_le_bytes());
        hash_u128s(&mut hasher, &xor.row_weights_q)?;
        hasher.update(&claimed.0.to_le_bytes());
    }
    hash_fqs(&mut hasher, &opening.xor_col_weights)?;
    absorb_spartan_message(
        transcript,
        OPENING_CLAIM_DOMAIN,
        hasher.finalize().as_bytes(),
    );
    Ok(())
}

fn opening_digest_prefix<F>(
    matrices: &PreparedConstraintMatrices<F>,
    assignment_binding: &[u8; 32],
    claim: &BitifiedSpartanClaim,
) -> Result<Hasher, SpartanF2zError>
where
    F: F2zCompatibleField,
{
    let mut hasher = Hasher::new();
    hasher.update(OPENING_CLAIM_DOMAIN);
    hasher.update(assignment_binding);
    hasher.update(matrices.digest());
    hash_fqs(&mut hasher, &claim.point)?;
    hasher.update(&claim.scale.0.to_le_bytes());
    hasher.update(&claim.value.0.to_le_bytes());
    hash_usize(&mut hasher, claim.max_bits)?;
    Ok(hasher)
}

fn virtualization_digest(
    layout: &ShaF2Layout,
    logical_rows: usize,
    rows: &[BinarySparseRow],
) -> Result<[u8; 32], SpartanF2zError> {
    let mut hasher = Hasher::new();
    hasher.update(VIRTUALIZATION_MATRIX_DOMAIN);
    hash_layout(&mut hasher, layout)?;
    hash_usize(&mut hasher, logical_rows)?;
    hash_usize(&mut hasher, rows.len())?;
    for row in rows {
        hash_usizes(&mut hasher, &row.columns)?;
        hasher.update(&row.constant.to_le_bytes());
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_commitment_params(
    hasher: &mut Hasher,
    commitment: &Commitment,
) -> Result<(), SpartanF2zError> {
    hash_usize(hasher, commitment.params.m)?;
    hash_usize(hasher, commitment.params.log_inv_rate)?;
    hash_usize(hasher, commitment.params.log_batch_size)?;
    hasher.update(&[profile_code(commitment.params.profile)]);
    hasher.update(&[hash_code(commitment.params.merkle_hash)]);
    Ok(())
}

fn hash_layout(hasher: &mut Hasher, layout: &ShaF2Layout) -> Result<(), SpartanF2zError> {
    hash_params(hasher, &layout.p)?;
    for value in [
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
    ] {
        hash_usize(hasher, value)?;
    }
    Ok(())
}

fn hash_params(hasher: &mut Hasher, params: &IntEvalParams) -> Result<(), SpartanF2zError> {
    hash_usize(hasher, params.t)?;
    hash_usize(hasher, params.s)?;
    hash_usize(hasher, params.word_bits)
}

fn hash_fqs(hasher: &mut Hasher, values: &[Fq]) -> Result<(), SpartanF2zError> {
    hash_usize(hasher, values.len())?;
    for value in values {
        hasher.update(&value.0.to_le_bytes());
    }
    Ok(())
}

fn hash_u128s(hasher: &mut Hasher, values: &[u128]) -> Result<(), SpartanF2zError> {
    hash_usize(hasher, values.len())?;
    for value in values {
        hasher.update(&value.to_le_bytes());
    }
    Ok(())
}

fn hash_usizes(hasher: &mut Hasher, values: &[usize]) -> Result<(), SpartanF2zError> {
    hash_usize(hasher, values.len())?;
    for &value in values {
        hash_usize(hasher, value)?;
    }
    Ok(())
}

fn hash_usize(hasher: &mut Hasher, value: usize) -> Result<(), SpartanF2zError> {
    let value = u64::try_from(value).map_err(|_| SpartanF2zError::BindingEncodingOverflow)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{FromWithConfig, crypto_bigint_monty::F128};

    use super::*;
    use crate::{
        ligerito_flock::{commit_rs_ligerito, commit_rs_ligerito_rows},
        pcs::{extract_virtual_xor_rows, fq_sub},
        piop::spartan::{
            f2z::spartan_f2z_field_config,
            matrix::{
                ConstraintMatrices, PreparedConstraintMatrices, SparseMatrix, build_assignment_mle,
                build_product_mles,
            },
            u32_mul::{U32MulWitness, prepare_u32_mul_relation, project_u32_mul_witness},
        },
        transcript::Blake3Transcript,
    };

    fn field_claim(point: &[Fq], scale: Fq, value: Fq) -> ScaledMleEvaluationClaim<F128> {
        let config = spartan_f2z_field_config();
        ScaledMleEvaluationClaim::new(
            point
                .iter()
                .map(|coordinate| F128::from_with_cfg(coordinate.0, &config))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            F128::from_with_cfg(scale.0, &config),
            F128::from_with_cfg(value.0, &config),
        )
    }

    fn mle_eval(values: &[Fq], point: &[Fq]) -> Fq {
        assert_eq!(values.len(), 1_usize << point.len());
        let mut table = values.to_vec();
        let mut active = table.len();
        for &coordinate in point {
            active /= 2;
            for index in 0..active {
                let left = table[2 * index];
                let right = table[2 * index + 1];
                table[index] = left + coordinate * Fq(fq_sub(right.0, left.0));
            }
        }
        table[0]
    }

    #[test]
    fn direct_bitification_matches_dense_integer_mle() {
        let params = IntEvalParams {
            t: 1,
            s: 1,
            word_bits: 64,
        };
        let data = [3_u128, 7, 11, 19];
        let point = [Fq(5), Fq(9)];
        let scale = Fq(13);
        let evaluation = mle_eval(&data.into_iter().map(Fq).collect::<Vec<_>>(), &point);
        let value = scale * evaluation;
        let bitified = bitify_spartan_claim(&field_claim(&point, scale, value), 64).unwrap();
        let opening = direct_opening_from_spartan_claim(&bitified, &params).unwrap();

        let mut read_off = Fq(0);
        for row in 0..params.rows() {
            for column in 0..params.cols() {
                let cell = Fq(data[params.cell_index(row, column)]);
                read_off = read_off
                    + Fq(opening.row_weights_q()[row]) * opening.col_weights()[column] * cell;
            }
        }
        assert_eq!(read_off, value);
        assert_eq!(opening.claimed(), value);
    }

    fn virtual_layout() -> ShaF2Layout {
        ShaF2Layout {
            p: IntEvalParams {
                t: 7,
                s: 2,
                word_bits: 1,
            },
            num_cols: 3,
            log_cols: 2,
            bit_vars: 3,
            num_vars: 4,
            tw: 2,
            x_fold_extra: 0,
        }
    }

    #[test]
    fn binary_sparse_matrix_is_canonical_and_pads_zero_rows() {
        let layout = virtual_layout();
        let matrix = BinarySparseMatrix::new(
            layout,
            vec![
                BinarySparseRow::new(vec![0], 0),
                BinarySparseRow::new(vec![0, 2], 0),
                BinarySparseRow::new(Vec::new(), 7),
            ],
        )
        .unwrap();
        assert_eq!(matrix.logical_row_count(), 3);
        assert_eq!(matrix.rows().len(), 4);
        assert_eq!(matrix.output_num_vars(), 2);
        assert_eq!(matrix.rows()[3], BinarySparseRow::zero());
        let changed = BinarySparseMatrix::new(
            layout,
            vec![
                BinarySparseRow::new(vec![0], 0),
                BinarySparseRow::new(vec![0, 2], 1),
                BinarySparseRow::new(Vec::new(), 7),
            ],
        )
        .unwrap();
        assert_ne!(matrix.digest(), changed.digest());

        assert!(matches!(
            BinarySparseMatrix::new(layout, vec![BinarySparseRow::new(vec![1, 1], 0)]),
            Err(SpartanF2zError::InvalidVirtualizationMatrix)
        ));
        assert!(matches!(
            BinarySparseMatrix::new(layout, vec![BinarySparseRow::new(vec![3], 0)]),
            Err(SpartanF2zError::InvalidVirtualizationMatrix)
        ));
    }

    #[test]
    fn virtualized_mode_rejects_a_layout_different_from_its_matrix() {
        let matrix_layout = virtual_layout();
        let matrix =
            BinarySparseMatrix::new(matrix_layout, vec![BinarySparseRow::new(vec![0], 0)]).unwrap();
        let mut different_layout = matrix_layout;
        different_layout.p.t = 8;
        different_layout.p.s = 1;
        different_layout.tw = 3;

        assert!(matches!(
            validate_virtual_opening_geometry(8, &different_layout, &matrix),
            Err(SpartanF2zError::InvalidVirtualizationMatrix)
        ));
    }

    #[test]
    fn virtual_adjoint_matches_materialized_h_mle() {
        let layout = virtual_layout();
        let matrix = BinarySparseMatrix::new(
            layout,
            vec![
                BinarySparseRow::new(vec![0], 0),
                BinarySparseRow::new(vec![0, 1], 0),
                BinarySparseRow::new(Vec::new(), 0),
                BinarySparseRow::new(vec![2], 0),
            ],
        )
        .unwrap();
        let trace_count = 1_usize << layout.num_vars;
        let source = (0..layout.num_cols)
            .map(|column| {
                (0..trace_count)
                    .map(|trace| ((17 * column + 11 * trace + 3) & 0xff) as u128)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let mut h = vec![Fq(0); trace_count * matrix.rows().len()];
        for trace in 0..trace_count {
            h[trace] = Fq(source[0][trace]);
            h[trace_count + trace] = Fq(source[0][trace] ^ source[1][trace]);
            h[3 * trace_count + trace] = Fq(source[2][trace]);
        }
        let point = [Fq(2), Fq(5), Fq(7), Fq(11), Fq(13), Fq(17)];
        let scale = Fq(19);
        let value = scale * mle_eval(&h, &point);
        let bitified = bitify_spartan_claim(&field_claim(&point, scale, value), 8).unwrap();
        let opening = virtual_opening_from_spartan_claim(&bitified, &matrix).unwrap();
        assert!(opening.main().is_some());
        assert_eq!(opening.xors().len(), 1);

        let mut committed_rows = vec![vec![0_u64; layout.p.rows() / 64]; layout.p.cols()];
        for (column, words) in source.iter().enumerate() {
            for (trace, &word) in words.iter().enumerate() {
                let row_high = trace >> layout.p.s;
                let clear = trace & (layout.p.cols() - 1);
                for bit in 0..8 {
                    if (word >> bit) & 1 == 0 {
                        continue;
                    }
                    let row =
                        (bit << (layout.log_cols + layout.tw)) | (column << layout.tw) | row_high;
                    committed_rows[clear][row / 64] |= 1_u64 << (row % 64);
                }
            }
        }

        let main = opening.main().unwrap();
        let main_claimed =
            evaluate_bit_rows(&committed_rows, main.row_weights_q(), main.col_weights()).unwrap();
        let xor = &opening.xors()[0];
        let xor_rows = extract_virtual_xor_rows(
            &layout,
            &committed_rows,
            xor.columns(),
            xor.constant(),
            None,
        );
        let xor_claimed =
            evaluate_bit_rows(&xor_rows, xor.row_weights_q(), opening.xor_col_weights()).unwrap();
        assert_eq!(main_claimed + xor_claimed, value);

        let config = spartan_f2z_field_config();
        let padded_columns = 1_usize << layout.log_cols;
        let mut field_source =
            vec![F128::from_with_cfg(0_u128, &config); padded_columns * trace_count];
        for (column, words) in source.iter().enumerate() {
            for (trace, &word) in words.iter().enumerate() {
                field_source[column * trace_count + trace] = F128::from_with_cfg(word, &config);
            }
        }
        let original_witness = DenseMultilinearExtension {
            evaluations: field_source,
            num_vars: layout.num_vars + layout.log_cols,
        };
        let (field_main, field_xors) =
            evaluate_virtual_components(&opening, &matrix, &original_witness).unwrap();
        assert_eq!(field_main, Some(main_claimed));
        assert_eq!(field_xors, [xor_claimed]);

        let mut wrong_original = original_witness.clone();
        wrong_original.evaluations[0] = F128::from_with_cfg(source[0][0] ^ 1, &config);
        let (wrong_main, wrong_xors) =
            evaluate_virtual_components(&opening, &matrix, &wrong_original).unwrap();
        assert!(matches!(
            validate_component_sum(&bitified, wrong_main, &wrong_xors),
            Err(SpartanF2zError::VirtualClaimMismatch)
        ));
        assert!(matches!(
            validate_component_sum(&bitified, Some(main_claimed), &[xor_claimed + Fq(1)]),
            Err(SpartanF2zError::VirtualClaimMismatch)
        ));
    }

    #[test]
    #[ignore = "production-minimum combined proof"]
    fn generic_direct_workflow_proves_and_verifies() {
        const MULTIPLICATIONS: usize = 1 << 14;
        let native = U32MulWitness::from_fn(MULTIPLICATIONS, |index| {
            let x = (index as u32).wrapping_mul(0x9e37_79b9);
            let y = (index as u32).rotate_left(13) ^ 0xa5a5_5a5a;
            (x, y)
        })
        .unwrap();
        let config = spartan_f2z_field_config();
        let relation = prepare_u32_mul_relation(*native.layout(), &config).unwrap();
        let (assignment, products) = project_u32_mul_witness::<F128>(&native, &config).unwrap();
        let data = native
            .assignment()
            .iter()
            .copied()
            .map(u128::from)
            .collect::<Vec<_>>();
        let params = IntEvalParams {
            t: 8,
            s: 8,
            word_bits: 64,
        };
        assert_eq!(params.cells(), data.len());
        let (pc, _) = configs_for_params(&params).unwrap();
        let hint = commit_rs_ligerito(&params, &data, &pc);
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_spartan_and_f2z(
            &mut prover_transcript,
            &relation,
            assignment,
            products,
            64,
            SpartanF2zOpening::Direct { params: &params },
            &hint,
        )
        .unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_spartan_and_f2z(
            &mut verifier_transcript,
            &relation,
            64,
            SpartanF2zVerifierOpening::Direct { params: &params },
            &hint.commitment,
            &proof,
        )
        .unwrap();
    }

    #[test]
    #[ignore = "production-minimum combined proof"]
    fn generic_virtualized_workflow_proves_and_verifies() {
        let layout = ShaF2Layout {
            p: IntEvalParams {
                t: 19,
                s: 3,
                word_bits: 1,
            },
            num_cols: 1 << 10,
            log_cols: 10,
            bit_vars: 6,
            num_vars: 6,
            tw: 3,
            x_fold_extra: 0,
        };
        let matrix = BinarySparseMatrix::new(
            layout,
            vec![
                BinarySparseRow::new(vec![0], 0),
                BinarySparseRow::new(vec![1, 2], 0x55aa),
            ],
        )
        .unwrap();
        let traces = 1_usize << layout.num_vars;
        let mut source = vec![vec![0_u64; traces]; 3];
        source[0][0] = 1;
        for trace in 0..traces {
            source[1][trace] = (trace as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            source[2][trace] = (trace as u64).rotate_left(17) ^ 0xa5a5_5a5_1234_5678;
        }
        let mut h = vec![0_u64; traces * 2];
        for trace in 0..traces {
            h[trace] = source[0][trace];
            h[traces + trace] = source[1][trace] ^ source[2][trace] ^ 0x55aa;
        }

        let config = spartan_f2z_field_config();
        let one = F128::from_with_cfg(1_u64, &config);
        let selector = SparseMatrix::try_from_rows(h.len(), vec![vec![(0, one.clone())]]).unwrap();
        let matrices =
            ConstraintMatrices::new(selector.clone(), selector.clone(), selector).unwrap();
        let matrices = PreparedConstraintMatrices::new(matrices, &config).unwrap();
        let field_h = h
            .iter()
            .copied()
            .map(|value| F128::from_with_cfg(value, &config))
            .collect::<Vec<_>>();
        let virtualized_witness = build_assignment_mle(&field_h, h.len(), &config).unwrap();
        let products = build_product_mles(
            core::slice::from_ref(&one),
            core::slice::from_ref(&one),
            core::slice::from_ref(&one),
            1,
            &config,
        )
        .unwrap();

        let mut bit_rows = vec![vec![0_u64; layout.p.rows() / 64]; layout.p.cols()];
        for (column, words) in source.iter().enumerate() {
            for (trace, &word) in words.iter().enumerate() {
                let row_high = trace >> layout.p.s;
                let clear = trace & (layout.p.cols() - 1);
                for bit in 0..64 {
                    if (word >> bit) & 1 == 0 {
                        continue;
                    }
                    let row =
                        (bit << (layout.log_cols + layout.tw)) | (column << layout.tw) | row_high;
                    bit_rows[clear][row / 64] |= 1_u64 << (row % 64);
                }
            }
        }
        let (pc, _) = configs_for_params(&layout.p).unwrap();
        let hint = commit_rs_ligerito_rows(&layout.p, bit_rows, &pc);

        let padded_source_columns = 1_usize << layout.log_cols;
        let mut original_values = vec![0_u64; padded_source_columns * traces];
        for (column, words) in source.iter().enumerate() {
            original_values[column * traces..(column + 1) * traces].copy_from_slice(words);
        }
        let field_original = original_values
            .into_iter()
            .map(|value| F128::from_with_cfg(value, &config))
            .collect::<Vec<_>>();
        let original_witness =
            build_assignment_mle(&field_original, field_original.len(), &config).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_spartan_and_f2z(
            &mut prover_transcript,
            &matrices,
            original_witness,
            products,
            64,
            SpartanF2zOpening::Virtualized {
                virtualized_witness,
                layout: &layout,
                matrix: &matrix,
            },
            &hint,
        )
        .unwrap();
        let mut verifier_transcript = Blake3Transcript::new();
        verify_spartan_and_f2z(
            &mut verifier_transcript,
            &matrices,
            64,
            SpartanF2zVerifierOpening::Virtualized {
                layout: &layout,
                matrix: &matrix,
            },
            &hint.commitment,
            &proof,
        )
        .unwrap();
    }
}
