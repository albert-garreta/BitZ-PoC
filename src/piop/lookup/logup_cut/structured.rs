use crate::{
    pcs::IntegerMatrixLayout,
    piop::sumcheck::multi_degree::MultiDegreeSumcheckProof,
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    sumcheck::{UngrindedRoundBoundary, inner::{InitialClaims, binary}},
    transcript::traits::{Transcribable, Transcript},
};

use super::{EvalClaim, StructuredLinearClaim};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

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
    let values = xi_combined_rows_packed_batch(
        layout,
        packed_columns,
        &claim
            .terms
            .iter()
            .map(|term| term.column_point.as_slice())
            .collect::<Vec<_>>(),
    );
    let mut pairs = Vec::with_capacity(claim.terms.len());
    for (term, values) in claim.terms.iter().zip(values) {
        assert_eq!(term.row_weights.len(), row_len);
        assert_eq!(term.column_point.len(), layout.col_vars);
        pairs.push([term.row_weights.clone(), values]);
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

fn xi_combined_rows_packed_batch(
    layout: &IntegerMatrixLayout,
    packed_columns: &[Vec<u64>],
    points: &[&[Gf]],
) -> Vec<Vec<Gf>> {
    let rows = layout.rows() * layout.word_bits;
    let columns = layout.cols();
    let groups = columns.div_ceil(64);
    assert!(!points.is_empty());
    assert!(points.iter().all(|point| point.len() == layout.col_vars));
    assert!(packed_columns.len() >= groups);
    assert!(packed_columns[..groups].iter().all(|column| column.len() == rows));

    let split = layout.col_vars.min(6);
    let eq = points
        .iter()
        .map(|point| {
            (
                build_eq_x_r_vec(&point[..split], &()).unwrap_or(vec![Gf::ONE]),
                build_eq_x_r_vec(&point[split..], &()).unwrap_or(vec![Gf::ONE]),
            )
        })
        .collect::<Vec<_>>();

    #[cfg(feature = "parallel")]
    let partials = {
        let group_chunk = groups.div_ceil(rayon::current_num_threads()).max(1);
        packed_columns[..groups]
            .par_chunks(group_chunk)
            .enumerate()
            .map(|(chunk, sources)| {
                let mut outputs = vec![vec![Gf::ZERO; rows]; points.len()];
                let mut table = vec![Gf::ZERO; 8 << 8];
                for (local, source) in sources.iter().enumerate() {
                    accumulate_packed_group(
                        columns,
                        chunk * group_chunk + local,
                        source,
                        &eq,
                        &mut outputs,
                        &mut table,
                    );
                }
                outputs
            })
            .collect::<Vec<_>>()
    };
    #[cfg(feature = "parallel")]
    {
        let mut outputs = vec![vec![Gf::ZERO; rows]; points.len()];
        for partial in partials {
            for (output, partial) in outputs.iter_mut().zip(partial) {
                output.iter_mut().zip(partial).for_each(|(out, add)| *out += add);
            }
        }
        outputs
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut outputs = vec![vec![Gf::ZERO; rows]; points.len()];
        let mut table = vec![Gf::ZERO; 8 << 8];
        for (group, source) in packed_columns[..groups].iter().enumerate() {
            accumulate_packed_group(
                columns,
                group,
                source,
                &eq,
                &mut outputs,
                &mut table,
            );
        }
        outputs
    }
}

fn accumulate_packed_group(
    columns: usize,
    group: usize,
    source: &[u64],
    eq: &[(Vec<Gf>, Vec<Gf>)],
    outputs: &mut [Vec<Gf>],
    table: &mut [Gf],
) {
    for ((low, high), output) in eq.iter().zip(outputs) {
        table.fill(Gf::ZERO);
        let high = field::PreparedGf128Mul::new(high[group]);
        for pos in 0..8usize {
            let base_column = (group << 6) | (pos << 3);
            let subset = &mut table[pos << 8..(pos + 1) << 8];
            for bit in 0..8usize {
                let column = base_column + bit;
                if column >= columns {
                    break;
                }
                let weight = high.mul(&low[column & 63]);
                let flag = 1usize << bit;
                for mask in 0..flag {
                    subset[mask | flag] = subset[mask] + weight;
                }
            }
        }
        for (&word, output) in source.iter().zip(output) {
            let mut word = word;
            let mut pos = 0usize;
            while word != 0 {
                let byte = (word & 0xff) as usize;
                if byte != 0 {
                    *output += table[(pos << 8) | byte];
                }
                word >>= 8;
                pos += 1;
            }
        }
    }
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
