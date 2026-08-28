//! Composition of Spartan's outer and inner sumchecks.

use blake3::Hasher;
use thiserror::Error;

use crate::{poly::mle::DenseMultilinearExtension, transcript::traits::Transcript};

use super::{
    SpartanField, absorb_spartan_message,
    matrix::{
        MleClaimError, PreparedConstraintMatrices, ScaledMleEvaluationClaim, SpartanMatrixError,
        make_equality_factors,
    },
    squeeze_field,
    sumcheck::{
        OuterSumcheckProof, R1csProductMles, SumcheckError, SumcheckProof, prove_inner_sumcheck,
        prove_outer_sumcheck,
    },
};

/// Domain separator for the native Spartan PIOP transcript.
///
/// Version two identifies the target-native transcript fork: it uses BLAKE3
/// statement digests, runtime field configurations, exact challenge sampling,
/// and an explicit assignment-oracle binding.
pub const SPARTAN_PIOP_DOMAIN: &[u8] = b"f2z/spartan/piop/v2";

/// Domain separator for the assignment-oracle commitment in the PIOP
/// statement.
pub const SPARTAN_ASSIGNMENT_ORACLE_DOMAIN: &[u8] = b"f2z/spartan/assignment-oracle/v1";

const NONSUCCINCT_ASSIGNMENT_DIGEST_DOMAIN: &[u8] = b"f2z/spartan/full-assignment-digest/v1";

/// The cubic outer and quadratic inner sumcheck proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpartanPiopProof<F> {
    pub outer: OuterSumcheckProof<F>,
    pub inner: SumcheckProof<F, 3>,
}

/// Failures while composing or checking the Spartan PIOP.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SpartanError {
    #[error(transparent)]
    Matrix(#[from] SpartanMatrixError),

    #[error(transparent)]
    Sumcheck(#[from] SumcheckError),

    #[error(transparent)]
    MleClaim(#[from] MleClaimError),

    #[error("Az, Bz, and Cz do not use the prepared row domain")]
    InvalidProductDimensions,

    #[error("the assignment does not use the prepared column domain")]
    InvalidAssignmentDimensions,

    #[error("the assignment has nonzero values outside the logical column range")]
    InvalidAssignmentPadding,

    #[error("a dense multilinear-extension table has invalid shape")]
    InvalidMleShape,

    #[error("a proof or witness value uses a different field configuration")]
    FieldConfigurationMismatch,

    #[error("the supplied opening claim is not the claim derived from the proof")]
    InvalidMleClaim,
}

/// Runs the complete outer and inner Spartan reductions.
///
/// The caller supplies field-valued `A/B/C` products and the complete R1CS
/// assignment. Constraint generation and the eventual succinct opening of the
/// returned assignment-MLE claim are deliberately outside this native PIOP.
/// `assignment_oracle_binding` must canonically commit to that eventual
/// opening oracle (normally a PCS commitment) and is absorbed before any
/// Fiat--Shamir challenge. Use [`prove_spartan_nonsuccinct`] while no PCS is
/// connected; it binds a canonical digest of the complete assignment table.
pub fn prove_spartan_piop<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    assignment_oracle_binding: &[u8; 32],
    products: R1csProductMles<F>,
    assignment: DenseMultilinearExtension<F>,
) -> Result<(SpartanPiopProof<F>, ScaledMleEvaluationClaim<F>), SpartanError>
where
    F: SpartanField,
{
    validate_prover_inputs(matrices, &products, &assignment)?;
    absorb_statement(transcript, matrices, assignment_oracle_binding);

    let field_config = matrices.config();
    let tau = (0..matrices.num_row_vars())
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<F>>();
    let equality_factors = make_equality_factors(&tau, field_config)?;
    let outer = prove_outer_sumcheck(
        transcript,
        F::zero_with_cfg(field_config),
        equality_factors,
        products,
        field_config,
    )?;

    // The outer prover absorbed [Az(r_x), Bz(r_x), Cz(r_x)] before returning.
    let rho = squeeze_field(transcript, field_config);
    let inner_initial_claim = batched_product_claim(
        &outer.proof.az_mle_claim,
        &outer.proof.bz_mle_claim,
        &outer.proof.cz_mle_claim,
        &rho,
    );
    let batched_matrix = matrices.bind_and_batch(&outer.eval_points, &rho)?;
    let inner = prove_inner_sumcheck(
        transcript,
        inner_initial_claim,
        batched_matrix,
        assignment,
        field_config,
    )?;

    let claim = ScaledMleEvaluationClaim::new(
        inner.sumcheck.eval_points.into_boxed_slice(),
        inner.batched_matrix_evaluation,
        inner.sumcheck.final_claim,
    );
    let proof = SpartanPiopProof {
        outer: outer.proof,
        inner: inner.sumcheck.proof,
    };

    Ok((proof, claim))
}

/// Verifies both sumchecks and returns the terminal scaled assignment claim
/// `D(r_y) * h(r_y) = final_claim`.
pub fn verify_spartan_proof<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    assignment_oracle_binding: &[u8; 32],
    proof: &SpartanPiopProof<F>,
) -> Result<ScaledMleEvaluationClaim<F>, SpartanError>
where
    F: SpartanField,
{
    validate_proof(matrices, proof)?;
    absorb_statement(transcript, matrices, assignment_oracle_binding);

    let field_config = matrices.config();
    let tau = (0..matrices.num_row_vars())
        .map(|_| squeeze_field(transcript, field_config))
        .collect::<Vec<F>>();
    let outer = proof.outer.verify(
        transcript,
        F::zero_with_cfg(field_config),
        &tau,
        field_config,
    )?;

    // The outer verifier absorbed [Az(r_x), Bz(r_x), Cz(r_x)] before returning.
    let rho = squeeze_field(transcript, field_config);
    let inner_initial_claim = batched_product_claim(
        &outer.az_mle_claim,
        &outer.bz_mle_claim,
        &outer.cz_mle_claim,
        &rho,
    );
    let (column_point, final_claim) = proof.inner.verify(
        transcript,
        inner_initial_claim,
        matrices.num_column_vars(),
        field_config,
    )?;
    let matrix_evaluation = matrices.evaluate_batched(&outer.eval_points, &rho, &column_point)?;

    Ok(ScaledMleEvaluationClaim::new(
        column_point.into_boxed_slice(),
        matrix_evaluation,
        final_claim,
    ))
}

/// Proves the PIOP while binding the complete assignment table itself.
///
/// This is the sound native path until a hiding PCS commitment is available.
/// The digest is binding, not hiding; callers who need witness privacy should
/// use [`prove_spartan_piop`] with a canonical 32-byte binding of the PCS
/// commitment.
pub fn prove_spartan_nonsuccinct<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    products: R1csProductMles<F>,
    assignment: DenseMultilinearExtension<F>,
) -> Result<(SpartanPiopProof<F>, ScaledMleEvaluationClaim<F>), SpartanError>
where
    F: SpartanField,
{
    validate_prover_inputs(matrices, &products, &assignment)?;
    let assignment_binding = nonsuccinct_assignment_digest(matrices, &assignment)?;
    prove_spartan_piop(
        transcript,
        matrices,
        &assignment_binding,
        products,
        assignment,
    )
}

/// Verifies the PIOP, checks that `mle_claim` is the transcript-derived claim,
/// and discharges it against a complete assignment table.
///
/// This is intentionally nonsuccinct. It is the native integration seam to be
/// replaced by the F2Z PCS opening protocol later.
pub fn verify_spartan_with_mle_claim<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    proof: &SpartanPiopProof<F>,
    mle_claim: &ScaledMleEvaluationClaim<F>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
{
    validate_assignment(matrices, assignment)?;
    let assignment_binding = nonsuccinct_assignment_digest(matrices, assignment)?;
    let expected_claim = verify_spartan_proof(transcript, matrices, &assignment_binding, proof)?;
    if mle_claim != &expected_claim {
        return Err(SpartanError::InvalidMleClaim);
    }
    mle_claim.nonsuccinct_verify(assignment, matrices.config())?;
    Ok(())
}

fn absorb_statement<F>(
    transcript: &mut impl Transcript,
    matrices: &PreparedConstraintMatrices<F>,
    assignment_oracle_binding: &[u8; 32],
) where
    F: SpartanField,
{
    absorb_spartan_message(transcript, b"protocol", SPARTAN_PIOP_DOMAIN);
    absorb_spartan_message(
        transcript,
        b"field-modulus",
        matrices.field_modulus_encoding(),
    );
    absorb_spartan_message(transcript, b"matrix-statement", matrices.digest());
    absorb_spartan_message(
        transcript,
        SPARTAN_ASSIGNMENT_ORACLE_DOMAIN,
        assignment_oracle_binding,
    );
}

fn validate_prover_inputs<F>(
    matrices: &PreparedConstraintMatrices<F>,
    products: &R1csProductMles<F>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
{
    let row_vars = matrices.num_row_vars();
    if products.az.num_vars != row_vars
        || products.bz.num_vars != row_vars
        || products.cz.num_vars != row_vars
    {
        return Err(SpartanError::InvalidProductDimensions);
    }

    for mle in [&products.az, &products.bz, &products.cz] {
        validate_mle_shape(mle)?;
        validate_elements_field(&mle.evaluations, matrices)?;
    }
    validate_assignment(matrices, assignment)
}

fn validate_assignment<F>(
    matrices: &PreparedConstraintMatrices<F>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
{
    if assignment.num_vars != matrices.num_column_vars() {
        return Err(SpartanError::InvalidAssignmentDimensions);
    }
    validate_mle_shape(assignment)?;
    validate_elements_field(&assignment.evaluations, matrices)?;

    let one = F::one_with_cfg(matrices.config());
    if assignment.evaluations.first() != Some(&one) {
        return Err(SpartanMatrixError::InvalidAssignmentConstant.into());
    }
    let zero = F::zero_with_cfg(matrices.config());
    if assignment.evaluations[matrices.matrices().column_count()..]
        .iter()
        .any(|value| value != &zero)
    {
        return Err(SpartanError::InvalidAssignmentPadding);
    }
    Ok(())
}

fn nonsuccinct_assignment_digest<F>(
    matrices: &PreparedConstraintMatrices<F>,
    assignment: &DenseMultilinearExtension<F>,
) -> Result<[u8; 32], SpartanError>
where
    F: SpartanField,
{
    // Keep this helper independently defensive because it is the binding used
    // by both prover and verifier before the first challenge.
    validate_assignment(matrices, assignment)?;

    let mut hasher = Hasher::new();
    hasher.update(NONSUCCINCT_ASSIGNMENT_DIGEST_DOMAIN);
    hash_binding_bytes(&mut hasher, matrices.field_modulus_encoding())?;
    hash_binding_usize(&mut hasher, matrices.matrices().column_count())?;
    hash_binding_usize(&mut hasher, assignment.num_vars)?;
    hash_binding_usize(&mut hasher, assignment.evaluations.len())?;
    for value in &assignment.evaluations {
        hash_binding_bytes(&mut hasher, &value.canonical_element_encoding())?;
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_binding_bytes(hasher: &mut Hasher, bytes: &[u8]) -> Result<(), SpartanError> {
    hash_binding_usize(hasher, bytes.len())?;
    hasher.update(bytes);
    Ok(())
}

fn hash_binding_usize(hasher: &mut Hasher, value: usize) -> Result<(), SpartanError> {
    let value = u64::try_from(value).map_err(|_| SpartanMatrixError::DomainTooLarge)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

fn validate_proof<F>(
    matrices: &PreparedConstraintMatrices<F>,
    proof: &SpartanPiopProof<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
{
    if proof.outer.sumcheck.round_polynomials.len() != matrices.num_row_vars() {
        return Err(SumcheckError::InvalidRoundCount {
            expected: matrices.num_row_vars(),
            actual: proof.outer.sumcheck.round_polynomials.len(),
        }
        .into());
    }
    if proof.inner.round_polynomials.len() != matrices.num_column_vars() {
        return Err(SumcheckError::InvalidRoundCount {
            expected: matrices.num_column_vars(),
            actual: proof.inner.round_polynomials.len(),
        }
        .into());
    }

    for round in &proof.outer.sumcheck.round_polynomials {
        validate_elements_field(round, matrices)?;
    }
    validate_elements_field(
        &[
            proof.outer.az_mle_claim.clone(),
            proof.outer.bz_mle_claim.clone(),
            proof.outer.cz_mle_claim.clone(),
        ],
        matrices,
    )?;
    for round in &proof.inner.round_polynomials {
        validate_elements_field(round, matrices)?;
    }
    Ok(())
}

fn validate_mle_shape<F>(mle: &DenseMultilinearExtension<F>) -> Result<(), SpartanError> {
    let expected = 1usize
        .checked_shl(u32::try_from(mle.num_vars).map_err(|_| SpartanError::InvalidMleShape)?)
        .ok_or(SpartanError::InvalidMleShape)?;
    if mle.evaluations.len() != expected {
        return Err(SpartanError::InvalidMleShape);
    }
    Ok(())
}

fn validate_elements_field<F>(
    values: &[F],
    matrices: &PreparedConstraintMatrices<F>,
) -> Result<(), SpartanError>
where
    F: SpartanField,
{
    for value in values {
        if F::canonical_modulus_encoding(value.cfg()) != matrices.field_modulus_encoding() {
            return Err(SpartanError::FieldConfigurationMismatch);
        }
        value.validate_element().map_err(SpartanMatrixError::from)?;
    }
    Ok(())
}

fn batched_product_claim<F>(az: &F, bz: &F, cz: &F, rho: &F) -> F
where
    F: SpartanField,
{
    let rho_squared = rho.clone() * rho;
    let mut claim = az.clone();
    claim += &(rho.clone() * bz);
    claim += &(rho_squared * cz);
    claim
}

#[cfg(test)]
mod tests {
    use crypto_primitives::{
        FromWithConfig, PrimeField,
        crypto_bigint_monty::{F128, F192},
        crypto_bigint_uint::Uint,
    };

    use crate::transcript::{Blake3Transcript, traits::Transcript};

    use super::*;
    use crate::piop::spartan::matrix::{
        ConstraintMatrices, SparseMatrix, build_assignment_mle, build_product_mles,
    };

    const Q100: u128 = (1_u128 << 100) - 15;
    const ROWS: usize = 5;
    const COLUMNS: usize = 7;

    fn config(modulus: u128) -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(modulus)).expect("odd prime test modulus")
    }

    fn field(value: u128, config: &<F128 as PrimeField>::Config) -> F128 {
        F128::from_with_cfg(value, config)
    }

    fn multiply(
        matrix: &SparseMatrix<F128>,
        assignment: &[F128],
        config: &<F128 as PrimeField>::Config,
    ) -> Vec<F128> {
        assert_eq!(assignment.len(), matrix.column_count());
        let mut products = vec![F128::zero_with_cfg(config); matrix.row_count()];
        for (column, entries) in matrix.columns().enumerate() {
            for (row, coefficient) in entries {
                products[*row] += &(coefficient.clone() * &assignment[column]);
            }
        }
        products
    }

    fn fixture(
        config: &<F128 as PrimeField>::Config,
    ) -> (
        PreparedConstraintMatrices<F128>,
        R1csProductMles<F128>,
        DenseMultilinearExtension<F128>,
    ) {
        let assignment_values: Vec<_> = (0..COLUMNS)
            .map(|column| field(if column == 0 { 1 } else { (column + 1) as u128 }, config))
            .collect();
        let a = SparseMatrix::try_from_rows(
            COLUMNS,
            (0..ROWS)
                .map(|row| {
                    vec![
                        (0, field((row + 2) as u128, config)),
                        (row + 1, field((2 * row + 3) as u128, config)),
                    ]
                })
                .collect(),
        )
        .unwrap();
        let b = SparseMatrix::try_from_rows(
            COLUMNS,
            (0..ROWS)
                .map(|row| {
                    vec![
                        (0, field((row + 5) as u128, config)),
                        (row + 2, field((3 * row + 7) as u128, config)),
                    ]
                })
                .collect(),
        )
        .unwrap();
        let az = multiply(&a, &assignment_values, config);
        let bz = multiply(&b, &assignment_values, config);
        let c = SparseMatrix::try_from_rows(
            COLUMNS,
            az.iter()
                .zip(&bz)
                .map(|(az, bz)| vec![(0, az.clone() * bz)])
                .collect(),
        )
        .unwrap();
        let cz = multiply(&c, &assignment_values, config);
        assert!(
            az.iter()
                .zip(&bz)
                .zip(&cz)
                .all(|((az, bz), cz)| az.clone() * bz == *cz)
        );

        let prepared =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), config)
                .unwrap();
        let products = build_product_mles(&az, &bz, &cz, ROWS, config).unwrap();
        let assignment = build_assignment_mle(&assignment_values, COLUMNS, config).unwrap();
        (prepared, products, assignment)
    }

    fn round_trip(modulus: u128) {
        let config = config(modulus);
        let (matrices, products, assignment) = fixture(&config);
        let assignment_binding = nonsuccinct_assignment_digest(&matrices, &assignment).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let (proof, claim) = prove_spartan_nonsuccinct(
            &mut prover_transcript,
            &matrices,
            products,
            assignment.clone(),
        )
        .unwrap();
        assert_eq!(proof.outer.sumcheck.round_polynomials.len(), 3);
        assert_eq!(proof.inner.round_polynomials.len(), 3);

        let mut verifier_transcript = Blake3Transcript::new();
        let expected_claim = verify_spartan_proof(
            &mut verifier_transcript,
            &matrices,
            &assignment_binding,
            &proof,
        )
        .unwrap();
        assert_eq!(claim, expected_claim);
        claim.nonsuccinct_verify(&assignment, &config).unwrap();

        // Equal continuation proves that every challenge and absorption stayed
        // in lockstep across the prover and verifier implementations.
        let prover_continuation: u128 = prover_transcript.get_challenge();
        let verifier_continuation: u128 = verifier_transcript.get_challenge();
        assert_eq!(prover_continuation, verifier_continuation);

        let mut complete_transcript = Blake3Transcript::new();
        verify_spartan_with_mle_claim(
            &mut complete_transcript,
            &matrices,
            &proof,
            &claim,
            &assignment,
        )
        .unwrap();
    }

    #[test]
    fn piop_is_generic_across_runtime_prime_configurations() {
        round_trip(Q100);
        round_trip((1_u128 << 127) - 1);
    }

    #[test]
    fn piop_is_generic_across_montgomery_limb_widths_and_zero_variable_domains() {
        let modulus = <F192 as crypto_primitives::Field>::Modulus::from(Q100);
        let config = F192::make_cfg(&modulus).unwrap();
        let one = F192::one_with_cfg(&config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, one.clone())]]).unwrap();
        let b = a.clone();
        let c = a.clone();
        let matrices =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config)
                .unwrap();
        let products = build_product_mles(
            std::slice::from_ref(&one),
            std::slice::from_ref(&one),
            std::slice::from_ref(&one),
            1,
            &config,
        )
        .unwrap();
        let assignment = build_assignment_mle(std::slice::from_ref(&one), 1, &config).unwrap();

        let mut prover_transcript = Blake3Transcript::new();
        let (proof, claim) = prove_spartan_nonsuccinct(
            &mut prover_transcript,
            &matrices,
            products,
            assignment.clone(),
        )
        .unwrap();
        assert!(proof.outer.sumcheck.round_polynomials.is_empty());
        assert!(proof.inner.round_polynomials.is_empty());

        let mut verifier_transcript = Blake3Transcript::new();
        verify_spartan_with_mle_claim(
            &mut verifier_transcript,
            &matrices,
            &proof,
            &claim,
            &assignment,
        )
        .unwrap();
    }

    #[test]
    fn tampered_round_and_assignment_are_rejected() {
        let config = config(Q100);
        let (matrices, products, assignment) = fixture(&config);
        let assignment_binding = nonsuccinct_assignment_digest(&matrices, &assignment).unwrap();
        let mut prover_transcript = Blake3Transcript::new();
        let (proof, claim) = prove_spartan_nonsuccinct(
            &mut prover_transcript,
            &matrices,
            products,
            assignment.clone(),
        )
        .unwrap();

        let mut tampered_proof = proof.clone();
        tampered_proof.outer.sumcheck.round_polynomials[0][0] += &F128::one_with_cfg(&config);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &tampered_proof,
            )
            .is_err()
        );

        let mut tampered_terminal = proof.clone();
        tampered_terminal.outer.az_mle_claim += &F128::one_with_cfg(&config);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_spartan_proof(
                &mut verifier_transcript,
                &matrices,
                &assignment_binding,
                &tampered_terminal,
            )
            .is_err()
        );

        let foreign_config = self::config((1_u128 << 127) - 1);
        let mut foreign_proof = proof.clone();
        foreign_proof.outer.sumcheck.round_polynomials[0][0] = field(1, &foreign_config);
        let mut rejected_transcript = Blake3Transcript::new();
        assert_eq!(
            verify_spartan_proof(
                &mut rejected_transcript,
                &matrices,
                &assignment_binding,
                &foreign_proof,
            ),
            Err(SpartanError::FieldConfigurationMismatch)
        );
        let mut untouched_transcript = Blake3Transcript::new();
        assert_eq!(
            rejected_transcript.get_challenge::<u128>(),
            untouched_transcript.get_challenge::<u128>(),
            "foreign-config proofs must fail before transcript mutation"
        );

        let mut tampered_assignment = assignment;
        tampered_assignment.evaluations[1] += &F128::one_with_cfg(&config);
        let mut verifier_transcript = Blake3Transcript::new();
        assert!(
            verify_spartan_with_mle_claim(
                &mut verifier_transcript,
                &matrices,
                &proof,
                &claim,
                &tampered_assignment,
            )
            .is_err()
        );
    }

    #[test]
    fn nonsuccinct_verifier_rejects_the_zero_assignment_forgery() {
        let config = config(Q100);
        let one = F128::one_with_cfg(&config);
        let zero = F128::zero_with_cfg(&config);
        let a = SparseMatrix::try_from_rows(1, vec![vec![(0, one.clone())]]).unwrap();
        let b = a.clone();
        let c = SparseMatrix::try_from_rows(1, vec![Vec::new()]).unwrap();
        let matrices =
            PreparedConstraintMatrices::new(ConstraintMatrices::new(a, b, c).unwrap(), &config)
                .unwrap();

        // Without validating and pre-challenge binding of the assignment,
        // these all-zero terminal claims would discharge against h = [0].
        let forged_proof = SpartanPiopProof {
            outer: OuterSumcheckProof {
                sumcheck: SumcheckProof {
                    round_polynomials: Vec::new(),
                },
                az_mle_claim: zero.clone(),
                bz_mle_claim: zero.clone(),
                cz_mle_claim: zero.clone(),
            },
            inner: SumcheckProof {
                round_polynomials: Vec::new(),
            },
        };
        let forged_claim = ScaledMleEvaluationClaim::new(
            Vec::new().into_boxed_slice(),
            zero.clone(),
            zero.clone(),
        );
        let forged_assignment = DenseMultilinearExtension {
            evaluations: vec![zero],
            num_vars: 0,
        };
        let mut transcript = Blake3Transcript::new();

        assert_eq!(
            verify_spartan_with_mle_claim(
                &mut transcript,
                &matrices,
                &forged_proof,
                &forged_claim,
                &forged_assignment,
            ),
            Err(SpartanError::Matrix(
                SpartanMatrixError::InvalidAssignmentConstant
            ))
        );
    }
}
