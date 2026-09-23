use crate::{
    ligerito::xi_combined_rows_packed,
    pcs::IntegerMatrixLayout,
    piop::sumcheck::multi_degree::MultiDegreeSumcheckProof,
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    sumcheck::{UngrindedRoundBoundary, inner::{InitialClaims, binary}},
    transcript::traits::{Transcribable, Transcript},
};

use super::{EvalClaim, StructuredLinearClaim};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredSumcheckProof {
    sumcheck: MultiDegreeSumcheckProof<Gf>,
}

impl StructuredSumcheckProof {
    pub(crate) fn proof_size_bytes(&self) -> usize {
        self.sumcheck.get_num_bytes()
    }
}

pub fn prove_structured_sumcheck(
    transcript: &mut impl Transcript,
    layout: &IntegerMatrixLayout,
    packed_columns: &[Vec<u64>],
    claim: &StructuredLinearClaim,
) -> (StructuredSumcheckProof, Vec<EvalClaim>) {
    assert!(!claim.terms.is_empty());
    let row_len = layout.rows() * layout.word_bits;
    let mut pairs = Vec::with_capacity(claim.terms.len());
    for term in &claim.terms {
        assert_eq!(term.row_weights.len(), row_len);
        assert_eq!(term.column_point.len(), layout.col_vars);
        let column_weights = build_eq_x_r_vec(&term.column_point, &()).unwrap_or(vec![Gf::ONE]);
        pairs.push([
            term.row_weights.clone(),
            xi_combined_rows_packed(layout, packed_columns, &column_weights),
        ]);
    }
    let (values, weights) = binary::inputs(pairs, row_len.ilog2() as usize);
    let output = crate::sumcheck::inner::prove_batched_inner_sumcheck(
        &field::Gf128Ops,
        transcript,
        InitialClaims::Compute,
        values,
        weights,
        &mut UngrindedRoundBoundary,
    )
    .expect("valid structured bit inner products");
    debug_assert_eq!(
        output.proofs.iter().map(|proof| {
            let [_, c1, c2] = proof.round_polynomials[0];
            c1 + c2
        }).sum::<Gf>(),
        claim.value,
    );
    let point = output.point.clone();
    let evaluations = output
        .terminal_evaluations
        .iter()
        .zip(&claim.terms)
        .filter_map(|([row, value], term)| {
            (*row != Gf::ZERO).then(|| EvalClaim {
                point: point.iter().chain(&term.column_point).copied().collect(),
                value: *value,
            })
        })
        .collect();
    (
        StructuredSumcheckProof {
            sumcheck: binary::encode(output).0,
        },
        evaluations,
    )
}

pub fn verify_structured_sumcheck(
    transcript: &mut impl Transcript,
    proof: &StructuredSumcheckProof,
    layout: &IntegerMatrixLayout,
    claim: &StructuredLinearClaim,
) -> Option<Vec<EvalClaim>> {
    if claim.terms.is_empty()
        || claim.terms.iter().any(|term| {
            term.row_weights.len() != layout.rows() * layout.word_bits
                || term.column_point.len() != layout.col_vars
        })
        || proof.sumcheck.claimed_sums().iter().copied().sum::<Gf>() != claim.value
    {
        return None;
    }
    let row_vars = (layout.rows() * layout.word_bits).ilog2() as usize;
    let subclaims = proof
        .sumcheck
        .verify_as_subprotocol(transcript, row_vars, &vec![2; claim.terms.len()], &())
        .ok()?;
    let eq = build_eq_x_r_vec(subclaims.point(), &()).unwrap_or(vec![Gf::ONE]);
    let mut evaluations = Vec::with_capacity(claim.terms.len());
    for (term, &expected) in claim
        .terms
        .iter()
        .zip(subclaims.expected_evaluations())
    {
        let row = term
            .row_weights
            .iter()
            .zip(&eq)
            .map(|(&weight, &eq)| weight * eq)
            .sum::<Gf>();
        if row == Gf::ZERO {
            if expected != Gf::ZERO {
                return None;
            }
        } else {
            evaluations.push(EvalClaim {
                point: subclaims
                    .point()
                    .iter()
                    .chain(&term.column_point)
                    .copied()
                    .collect(),
                value: expected * row.invert_nonzero(),
            });
        }
    }
    Some(evaluations)
}
