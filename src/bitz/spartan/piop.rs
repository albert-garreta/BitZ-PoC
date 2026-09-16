//! Their `spartan/src/piop.rs`: the composition of the outer and inner
//! sumchecks, and a canonical byte form for the proof they carry out of
//! band (`spartan.bin`, the feasibility doc's §2.2 item 4).
//!
//! Transcript order (all public messages and squeezes; nothing reaches the
//! narg string): absorb the constraint digest; squeeze `tau`; the outer
//! sumcheck; absorb `[Ah(r_x), Bh(r_x), Ch(r_x)]`; squeeze `rho`; the inner
//! sumcheck.

use super::super::fq::Fq;
use super::super::transcript::{ProverState, VerifierState};
use super::matrix::{MatrixError, PreparedConstraintMatrices};
use super::poly::eq_table;
use super::sumcheck::{
    OuterSumcheckProof, Products, SumcheckError, SumcheckProof, mle_evaluate,
    prove_inner_sumcheck, prove_outer_sumcheck,
};

/// The two sumcheck proofs comprising the reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpartanPiopProof {
    pub outer: OuterSumcheckProof,
    pub inner: SumcheckProof<3>,
}

/// The terminal claim `scale · h(point) = value`: `point = r_y`,
/// `scale = D(r_y)`, `value` the inner sumcheck's final claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaledMleEvaluationClaim {
    pub point: Vec<Fq>,
    pub scale: Fq,
    pub value: Fq,
}

impl ScaledMleEvaluationClaim {
    /// Checks the claim against the complete assignment table.
    pub fn nonsuccinct_verify(&self, assignment: &[Fq]) -> bool {
        assignment.len() == 1usize << self.point.len()
            && self.scale * mle_evaluate(assignment, &self.point) == self.value
    }
}

/// Failures while composing or checking the PIOP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpartanError {
    Matrix(MatrixError),
    Sumcheck(SumcheckError),
    InvalidProductDimensions,
    InvalidAssignmentDimensions,
    /// `spartan.bin` is not one canonical proof of the stated dimensions.
    Codec,
}

impl From<MatrixError> for SpartanError {
    fn from(error: MatrixError) -> Self {
        Self::Matrix(error)
    }
}

impl From<SumcheckError> for SpartanError {
    fn from(error: SumcheckError) -> Self {
        Self::Sumcheck(error)
    }
}

/// Runs both provers; returns the proof and the terminal claim.
pub fn prove_spartan_piop(
    transcript: &mut ProverState,
    matrices: &PreparedConstraintMatrices,
    products: &Products,
    assignment: &[Fq],
) -> Result<(SpartanPiopProof, ScaledMleEvaluationClaim), SpartanError> {
    let num_row_vars = matrices.num_row_vars();
    let num_column_vars = matrices.num_column_vars();
    let rows = 1usize << num_row_vars;
    if products.az.len() != rows || products.bz.len() != rows || products.cz.len() != rows {
        return Err(SpartanError::InvalidProductDimensions);
    }
    if assignment.len() != 1usize << num_column_vars {
        return Err(SpartanError::InvalidAssignmentDimensions);
    }

    transcript.public_message(matrices.digest());
    let tau: Vec<Fq> = (0..num_row_vars).map(|_| transcript.squeeze_fq()).collect();
    let started = std::time::Instant::now();
    let outer = prove_outer_sumcheck(transcript, Fq::ZERO, eq_table(&tau), products)?;
    super::super::trace("  spartan outer", started);

    // The outer prover absorbed the three evaluations before returning.
    let rho = transcript.squeeze_fq();
    let inner_initial_claim = outer.proof.az_mle_claim
        + rho * outer.proof.bz_mle_claim
        + rho * rho * outer.proof.cz_mle_claim;
    let started = std::time::Instant::now();
    let batched_matrix = matrices.bind_and_batch(&outer.eval_points, rho)?;
    super::super::trace("  spartan bind+batch", started);
    let started = std::time::Instant::now();
    // The assignment is `h`, Boolean by construction.
    let inner = prove_inner_sumcheck(transcript, inner_initial_claim, batched_matrix, assignment, true)?;
    super::super::trace("  spartan inner", started);

    let claim = ScaledMleEvaluationClaim {
        point: inner.eval_points,
        scale: inner.batched_matrix_evaluation,
        value: inner.final_claim,
    };
    let proof = SpartanPiopProof {
        outer: outer.proof,
        inner: inner.proof,
    };
    Ok((proof, claim))
}

/// Verifies both sumchecks; returns the terminal claim with `scale`
/// recomputed through `evaluate_batched`.
pub fn verify_spartan_proof(
    transcript: &mut VerifierState<'_>,
    matrices: &PreparedConstraintMatrices,
    proof: &SpartanPiopProof,
) -> Result<ScaledMleEvaluationClaim, SpartanError> {
    let num_row_vars = matrices.num_row_vars();
    let num_column_vars = matrices.num_column_vars();
    transcript.public_message(matrices.digest());
    let tau: Vec<Fq> = (0..num_row_vars).map(|_| transcript.squeeze_fq()).collect();
    let outer = proof.outer.verify(transcript, Fq::ZERO, &tau)?;

    let rho = transcript.squeeze_fq();
    let inner_initial_claim =
        outer.az_mle_claim + rho * outer.bz_mle_claim + rho * rho * outer.cz_mle_claim;
    let (column_point, final_claim) =
        proof
            .inner
            .verify(transcript, inner_initial_claim, num_column_vars)?;
    let started = std::time::Instant::now();
    let scale = matrices.evaluate_batched(&outer.eval_points, rho, &column_point)?;
    super::super::trace("  v: spartan evaluate", started);
    Ok(ScaledMleEvaluationClaim {
        point: column_point,
        scale,
        value: final_claim,
    })
}

impl SpartanPiopProof {
    /// The canonical byte form: outer round polynomials in round order
    /// (`[c0, c1, c2, c3]`, 16 LE bytes each), `[Ah, Bh, Ch]`, inner round
    /// polynomials (`[c0, c1, c2]`), then the terminal claim as
    /// `point ‖ scale ‖ value`.
    pub fn to_bytes(&self, terminal: &ScaledMleEvaluationClaim) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            16 * (4 * self.outer.sumcheck.round_polynomials.len()
                + 3
                + 3 * self.inner.round_polynomials.len()
                + terminal.point.len()
                + 2),
        );
        for round in &self.outer.sumcheck.round_polynomials {
            for coefficient in round {
                bytes.extend_from_slice(&coefficient.to_bytes());
            }
        }
        for value in [
            self.outer.az_mle_claim,
            self.outer.bz_mle_claim,
            self.outer.cz_mle_claim,
        ] {
            bytes.extend_from_slice(&value.to_bytes());
        }
        for round in &self.inner.round_polynomials {
            for coefficient in round {
                bytes.extend_from_slice(&coefficient.to_bytes());
            }
        }
        for coordinate in &terminal.point {
            bytes.extend_from_slice(&coordinate.to_bytes());
        }
        bytes.extend_from_slice(&terminal.scale.to_bytes());
        bytes.extend_from_slice(&terminal.value.to_bytes());
        bytes
    }

    /// The inverse of [`Self::to_bytes`] for the stated dimensions; every
    /// residue must be canonical.
    pub fn from_bytes(
        bytes: &[u8],
        num_row_vars: usize,
        num_column_vars: usize,
    ) -> Result<(Self, ScaledMleEvaluationClaim), SpartanError> {
        let expected = 16 * (4 * num_row_vars + 3 + 3 * num_column_vars + num_column_vars + 2);
        if bytes.len() != expected {
            return Err(SpartanError::Codec);
        }
        let mut at = 0usize;
        let mut next = || -> Result<Fq, SpartanError> {
            let value = Fq::from_bytes(bytes[at..at + 16].try_into().expect("16 bytes"))
                .ok_or(SpartanError::Codec)?;
            at += 16;
            Ok(value)
        };
        let mut outer_rounds = Vec::with_capacity(num_row_vars);
        for _ in 0..num_row_vars {
            outer_rounds.push([next()?, next()?, next()?, next()?]);
        }
        let az_mle_claim = next()?;
        let bz_mle_claim = next()?;
        let cz_mle_claim = next()?;
        let mut inner_rounds = Vec::with_capacity(num_column_vars);
        for _ in 0..num_column_vars {
            inner_rounds.push([next()?, next()?, next()?]);
        }
        let mut point = Vec::with_capacity(num_column_vars);
        for _ in 0..num_column_vars {
            point.push(next()?);
        }
        let scale = next()?;
        let value = next()?;
        Ok((
            Self {
                outer: OuterSumcheckProof {
                    sumcheck: SumcheckProof {
                        round_polynomials: outer_rounds,
                    },
                    az_mle_claim,
                    bz_mle_claim,
                    cz_mle_claim,
                },
                inner: SumcheckProof {
                    round_polynomials: inner_rounds,
                },
            },
            ScaledMleEvaluationClaim {
                point,
                scale,
                value,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use circuit::constraints::ConstraintGenerator;
    use circuit::sha256::{COMPRESSION_HINT_BITS, COMPRESSION_INPUT_BITS, compression_circuit};
    use circuit::witgen::{PackedWitness, Witgen};

    use super::super::super::transcript::{build_prover, build_verifier};
    use super::super::matrix::SparseMatrix;
    use super::*;

    fn splitmix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    const ROWS: usize = 5;
    const COLUMNS: usize = 7;

    fn multiply(matrix: &SparseMatrix, assignment: &[Fq]) -> Vec<Fq> {
        matrix
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .fold(Fq::ZERO, |sum, &(column, coefficient)| sum + coefficient * assignment[column])
            })
            .collect()
    }

    fn padded(values: Vec<Fq>) -> Vec<Fq> {
        let mut values = values;
        let len = values.len().max(1).next_power_of_two();
        values.resize(len, Fq::ZERO);
        values
    }

    /// Their `random_satisfying_r1cs_reduces_through_both_sumchecks`: `h[0] = 1`
    /// and `C[i, 0] = (A_i h)(B_i h)` make every row hold.
    #[test]
    fn random_satisfying_r1cs_reduces_through_both_sumchecks() {
        let mut state = 0x5a17_c0de;
        let mut bits = vec![true];
        bits.extend((1..COLUMNS).map(|_| splitmix(&mut state) & 1 == 1));
        let values: Vec<Fq> = bits.iter().map(|&b| Fq::from(b)).collect();
        let random_matrix = |state: &mut u64| {
            SparseMatrix::new(
                (0..ROWS)
                    .map(|_| {
                        let mut entries = vec![(0, Fq::new(u128::from(splitmix(state) % 9 + 1)))];
                        entries.extend((1..COLUMNS).filter_map(|column| {
                            (splitmix(state) & 1 == 1)
                                .then(|| (column, Fq::new(u128::from(splitmix(state) % 9 + 1))))
                        }));
                        entries
                    })
                    .collect(),
                COLUMNS,
            )
        };
        let a = random_matrix(&mut state);
        let b = random_matrix(&mut state);
        let az = multiply(&a, &values);
        let bz = multiply(&b, &values);
        let c = SparseMatrix::new(
            az.iter().zip(&bz).map(|(&x, &y)| vec![(0, x * y)]).collect(),
            COLUMNS,
        );
        let cz = multiply(&c, &values);
        let m_rows: Vec<Vec<usize>> = (0..COLUMNS).map(|column| vec![column]).collect();
        let matrices = PreparedConstraintMatrices::from_parts(&m_rows, COLUMNS, a, b, c).unwrap();
        let products = Products {
            az: padded(az),
            bz: padded(bz),
            cz: padded(cz),
        };
        assert!(products.satisfied());
        let assignment = padded(values);

        let session = b"spartan/piop/random-r1cs/fq/v1";
        let instance = b"five-rows-seven-columns";
        let mut prover = build_prover(session, instance);
        let (proof, claim) =
            prove_spartan_piop(&mut prover, &matrices, &products, &assignment).unwrap();
        assert_eq!(proof.outer.sumcheck.round_polynomials.len(), 3);
        assert_eq!(proof.inner.round_polynomials.len(), 3);
        let transcript_proof = prover.finish();
        assert!(transcript_proof.narg_string.is_empty());

        let mut verifier = build_verifier(session, instance, &transcript_proof);
        let checked = verify_spartan_proof(&mut verifier, &matrices, &proof).unwrap();
        verifier.check_eof().unwrap();
        assert_eq!(checked, claim);
        assert!(claim.nonsuccinct_verify(&assignment));

        let bytes = proof.to_bytes(&claim);
        assert_eq!(bytes.len(), 16 * (4 * 3 + 3 + 3 * 3 + 3 + 2));
        let (decoded, decoded_claim) = SpartanPiopProof::from_bytes(&bytes, 3, 3).unwrap();
        assert_eq!(decoded, proof);
        assert_eq!(decoded_claim, claim);
    }

    /// Their `sha256_compression_verifies_through_spartan_piop`, through the
    /// vendored circuit crate: the abc compression, 184 rows → 2^8, 20,457
    /// assignment entries → 2^15.
    #[test]
    fn sha256_compression_verifies_through_spartan_piop() {
        const ABC_BLOCK: [u32; 16] =
            [0x61626380, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00000018];
        const INITIAL_STATE: [u32; 8] = circuit::sha256::INITIAL_STATE;
        let inputs: Vec<bool> = (0..COMPRESSION_INPUT_BITS)
            .map(|index| {
                let (word, bit) = if index < 512 {
                    (ABC_BLOCK[index / 32], index % 32)
                } else {
                    (INITIAL_STATE[(index - 512) / 32], (index - 512) % 32)
                };
                word >> bit & 1 == 1
            })
            .collect();
        let inputs: Box<[bool; COMPRESSION_INPUT_BITS]> = inputs.into_boxed_slice().try_into().unwrap();

        let mut witgen = Witgen::with_inputs_and_capacity(
            inputs.as_ref(),
            COMPRESSION_INPUT_BITS + COMPRESSION_HINT_BITS,
        );
        let _digest = compression_circuit(&mut witgen, inputs.as_ref());
        let (f, h): (PackedWitness, PackedWitness) = witgen.into_witnesses();

        let mut generator = ConstraintGenerator::new(COMPRESSION_INPUT_BITS);
        let symbolic = generator.boxed_inputs::<COMPRESSION_INPUT_BITS>();
        let _ = compression_circuit(&mut generator, symbolic.as_ref());
        let integer = generator.into_matrices();
        assert_eq!(integer.m.row_count(), 20_457);
        assert_eq!(integer.m.column_count(), 7_145);
        assert_eq!(integer.a.row_count(), 184);
        assert!(integer.is_satisfied(&f));

        let matrices = PreparedConstraintMatrices::from_vendored(&integer).unwrap();
        assert_eq!(matrices.num_row_vars(), 8);
        assert_eq!(matrices.num_column_vars(), 15);
        let products = matrices.products(&h).unwrap();
        assert!(products.satisfied());
        let assignment = matrices.assignment(&h).unwrap();

        let session = b"spartan/piop/sha256-compression/v1";
        let instance = b"abc-single-compression";
        let mut prover = build_prover(session, instance);
        let (proof, claim) =
            prove_spartan_piop(&mut prover, &matrices, &products, &assignment).unwrap();
        assert_eq!(proof.outer.sumcheck.round_polynomials.len(), 8);
        assert_eq!(proof.inner.round_polynomials.len(), 15);
        let transcript_proof = prover.finish();
        let mut verifier = build_verifier(session, instance, &transcript_proof);
        let checked = verify_spartan_proof(&mut verifier, &matrices, &proof).unwrap();
        verifier.check_eof().unwrap();
        assert_eq!(checked, claim);
        assert!(claim.nonsuccinct_verify(&assignment));
    }
}
