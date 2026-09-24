use super::*;
use crate::{poly::utils::build_eq_x_r_vec, transcript::Blake3Transcript};
use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use super::upper::DyadicUpperScratch;

#[test]
fn chunk_specs_cover_exactly() {
    for ell1 in 1..40 {
        for width in 1..=ell1 {
            let chunks = chunk_specs(ell1, width);
            let mut next = 0;
            for (index, chunk) in chunks.iter().enumerate() {
                assert_eq!(chunk.chunk, index);
                assert_eq!(chunk.factor_start, next);
                assert!((1..=width).contains(&chunk.width));
                next += chunk.width;
            }
            assert_eq!(next, ell1);
        }
    }
}

#[test]
fn dyadic_plan_is_exact() {
    for q in [1usize, 2, 3, 5, 6, 7, 8, 9, 13, 22, 31, 32, 33] {
        let plan = DyadicPlan::new(q);
        assert_eq!(plan.blocks.len(), q.count_ones() as usize);
        let mut next = 0;
        let mut previous = usize::MAX;
        for block in &plan.blocks {
            assert_eq!(block.chunk_start, next);
            assert_eq!(block.n_chunks, 1 << block.depth);
            assert!(block.n_chunks <= previous);
            assert_eq!(block.chunk_start % block.n_chunks, 0);
            next += block.n_chunks;
            previous = block.n_chunks;
        }
        assert_eq!(next, q);
        assert_eq!(plan.block_product_nodes() + plan.root_merge_nodes(), q - 1);
    }
}

#[test]
fn packed_layout_is_tight_and_aligned() {
    let layout = PackedLayout::new([5, 4, 2, 1]);
    assert_eq!(layout.real_len, 54);
    assert_eq!(layout.padded_len, 64);
    for block in &layout.blocks {
        assert_eq!(block.offset % block.len(), 0);
    }
}

#[test]
fn table_rows_are_subset_products() {
    let chunks = chunk_specs(7, 4);
    let layout = PackedLayout::new(chunks.iter().map(|chunk| chunk.width));
    let y = (0..7)
        .map(|index| Gf::from_polynomial_words([index as u64 + 2, 0]))
        .collect::<Vec<_>>();
    let table = ChunkProductTable { y: &y, chunks: &chunks, layout: &layout };
    let mut values = vec![Gf::ZERO; layout.padded_len];
    table.materialize(&mut values);
    for (chunk, block) in chunks.iter().zip(&layout.blocks) {
        for pattern in 0..block.len() {
            let expected = (0..chunk.width)
                .filter(|bit| pattern >> bit & 1 == 1)
                .fold(Gf::ONE, |acc, bit| acc * y[chunk.factor_start + bit]);
            assert_eq!(values[block.offset + pattern], expected);
        }
    }
    assert!(values[layout.real_len..].iter().all(|value| *value == Gf::ZERO));
}

#[test]
fn public_table_eval_matches_materialized_mle_with_partial_chunk() {
    let chunks = chunk_specs(13, 5);
    let layout = PackedLayout::new(chunks.iter().map(|chunk| chunk.width));
    let y = (0..13)
        .map(|index| Gf::from_polynomial_words([(index as u64 + 2) * 17, 0]))
        .collect::<Vec<_>>();
    let table = ChunkProductTable { y: &y, chunks: &chunks, layout: &layout };
    let mut values = vec![Gf::ZERO; layout.padded_len];
    table.materialize(&mut values);
    for seed in 0..16u64 {
        let point = (0..layout.dim)
            .map(|coordinate| Gf::from_polynomial_words([seed * 29 + coordinate as u64 + 3, 0]))
            .collect::<Vec<_>>();
        assert_eq!(table.eval(&point), crate::merged_forest::mle_at(&values, &point));
    }
}

#[test]
fn table_row_encoding_is_polynomial_basis() {
    assert_eq!(encode_table_row(0), Gf::ZERO);
    assert_eq!(encode_table_row(1), Gf::ONE);
    assert_eq!(encode_table_row(0x1234), Gf::from_polynomial_words([0x1234, 0]));
}

#[test]
fn estimate_counts_partial_chunk_exactly() {
    let estimate = LogupCutEstimate::new(13, 8, 5);
    assert_eq!(estimate.n_chunks, 3);
    assert_eq!(estimate.last_chunk_bits, 3);
    assert_eq!(estimate.dyadic_blocks, vec![2, 1]);
    assert_eq!(estimate.source_real_rows, 24);
    assert_eq!(estimate.source_padded_rows, 32);
    assert_eq!(estimate.table_real_rows, 72);
    assert_eq!(estimate.table_padded_rows, 128);
}

#[test]
fn dyadic_upper_proves_exact_unpadded_cut() {
    // Six dyadic blocks exercise the non-power-of-two root merge. The merge
    // must keep every live claim at one shared sumcheck point.
    let plan = DyadicPlan::new(63);
    let r2 = 3;
    let columns = 1usize << r2;
    let cut_values = (0..plan.n_chunks * columns)
        .map(|i| Gf::from_polynomial_words([i as u64 + 3, (i as u64 + 11).rotate_left(17)]))
        .collect::<Vec<_>>();
    let root = (0..columns)
        .map(|column| {
            (0..plan.n_chunks)
                .map(|chunk| cut_values[column * plan.n_chunks + chunk])
                .product::<Gf>()
        })
        .collect::<Vec<_>>();
    let root_point = (0..r2)
        .map(|i| Gf::from_polynomial_words([31 + i as u64, 71 + i as u64]))
        .collect::<Vec<_>>();
    let root_eq = build_eq_x_r_vec(&root_point, &()).unwrap();
    let root_value = root.iter().zip(root_eq).map(|(&v, e)| v * e).sum();

    let mut prover_transcript = Blake3Transcript::new();
    let mut product_workspace = ProductWorkspace::default();
    product_workspace.reserve(
        plan.blocks.len().next_power_of_two() / 2,
        columns,
        r2,
    );
    let mut upper_scratch = DyadicUpperScratch::new(&plan, r2);
    let cut_value = |column: usize, chunk: usize| {
        cut_values[column * plan.n_chunks + chunk]
    };
    let (proof, claims) = prove_dyadic_upper(
        &mut prover_transcript,
        &plan,
        r2,
        &root_point,
        root_value,
        &cut_value,
        &mut product_workspace,
        &mut upper_scratch,
        &mut Default::default(),
    );
    assert_eq!(claims.len(), plan.blocks.len());
    for (claim, block) in claims.iter().zip(&plan.blocks) {
        let eq = build_eq_x_r_vec(&claim.point, &()).unwrap();
        let mut expected = Gf::ZERO;
        for column in 0..columns {
            for local_chunk in 0..block.n_chunks {
                let index = local_chunk | (column << block.depth);
                expected += eq[index]
                    * cut_values[column * plan.n_chunks + block.chunk_start + local_chunk];
            }
        }
        assert_eq!(claim.value, expected);
    }

    let mut verifier_transcript = Blake3Transcript::new();
    let verified = verify_dyadic_upper(
        &mut verifier_transcript,
        &proof,
        &plan,
        r2,
        &root_point,
        root_value,
    )
    .unwrap();
    assert_eq!(verified, claims);
}

#[test]
fn rational_fraction_tree_replays_and_binds_leaves() {
    let dimension = 4;
    let tau = Gf::from_polynomial_words([0x1234, 0x5678]);
    let numerators = (0..13)
        .map(|i| Gf::from_polynomial_words([i as u64 + 1, 3 * i as u64 + 9]))
        .collect::<Vec<_>>();
    let denominators = (0..13)
        .map(|i| tau + Gf::from_polynomial_words([i as u64 + 31, 0]))
        .collect::<Vec<_>>();
    assert!(denominators.iter().all(|value| *value != Gf::ZERO));
    let mut left = FractionTreeWitness::default();
    let mut right = FractionTreeWitness::default();
    left.rebuild(&numerators, &denominators, dimension, Gf::ZERO, tau);
    right.rebuild(&numerators, &denominators, dimension, Gf::ZERO, tau);
    let mut product_workspace = ProductWorkspace::default();
    product_workspace.reserve(1, 1 << (dimension - 1), dimension);

    let mut prover_transcript = Blake3Transcript::new();
    let (proof, claims) = prove_rational(
        &mut prover_transcript,
        &mut left,
        &mut right,
        &mut product_workspace,
    );
    let mut verifier_transcript = Blake3Transcript::new();
    let verified = verify_rational(&mut verifier_transcript, &proof, dimension, dimension).unwrap();
    assert_eq!(verified, claims);

    for claim in [&claims.left, &claims.right] {
        let eq = build_eq_x_r_vec(&claim.point, &()).unwrap();
        let num = numerators
            .iter()
            .zip(&eq)
            .map(|(&value, &weight)| value * weight)
            .sum::<Gf>();
        let den = denominators
            .iter()
            .zip(&eq)
            .map(|(&value, &weight)| value * weight)
            .sum::<Gf>()
            + tau * eq[numerators.len()..].iter().copied().sum::<Gf>();
        assert_eq!(claim.num, num);
        assert_eq!(claim.den, den);
    }
}

#[test]
fn inner_product_reduces_to_one_committed_evaluation() {
    let dimension = 6;
    let committed = (0..1usize << dimension)
        .map(|i| Gf::from_polynomial_words([i as u64 + 17, (3 * i) as u64 + 5]))
        .collect::<Vec<_>>();
    let public = (0..1usize << dimension)
        .map(|i| Gf::from_polynomial_words([(5 * i) as u64 + 1, i as u64 + 29]))
        .collect::<Vec<_>>();
    let claim = committed
        .iter()
        .zip(&public)
        .map(|(&left, &right)| left * right)
        .sum();
    let mut prover_transcript = Blake3Transcript::new();
    let (proof, evaluation) =
        prove_inner_product(&mut prover_transcript, &committed, &public, claim);
    let mut verifier_transcript = Blake3Transcript::new();
    let verified = verify_inner_product(
        &mut verifier_transcript,
        &proof,
        dimension,
        claim,
        |point| {
            let eq = build_eq_x_r_vec(point, &()).unwrap();
            public.iter().zip(eq).map(|(&value, weight)| value * weight).sum()
        },
    )
    .unwrap();
    assert_eq!(verified, evaluation);
    assert_eq!(prover_transcript.state_digest(), verifier_transcript.state_digest());
}

#[test]
fn pushforward_and_structured_index_claim_match_materialized_tables() {
    let ell1 = 13;
    let chunk_bits = 5;
    let r2 = 2;
    let columns = 1usize << r2;
    let chunks = chunk_specs(ell1, chunk_bits);
    let plan = DyadicPlan::new(chunks.len());
    let table_layout = PackedLayout::new(chunks.iter().map(|chunk| chunk.width));
    let y = (0..ell1)
        .map(|index| Gf::from_polynomial_words([(17 * index + 5) as u64, 0]))
        .collect::<Vec<_>>();
    let bits = (0..ell1 * columns)
        .map(|index| (index * 0x9e37 + index / columns * 13) & 4 != 0)
        .collect::<Vec<_>>();
    let patterns = (0..columns)
        .flat_map(|column| {
            let bits = &bits;
            chunks.iter().map(move |chunk| {
                (0..chunk.width).fold(0u16, |pattern, bit| {
                    pattern
                        | ((bits[(chunk.factor_start + bit) * columns + column] as u16) << bit)
                })
            })
        })
        .collect::<Vec<_>>();
    let table = ChunkProductTable { y: &y, chunks: &chunks, layout: &table_layout };
    let mut table_values = vec![Gf::ZERO; table_layout.padded_len];
    table.materialize(&mut table_values);
    let n_chunks = chunks.len();
    let cut_values = (0..chunks.len())
        .flat_map(|chunk| {
            let table_values = &table_values;
            let patterns = &patterns;
            let table_layout = &table_layout;
            (0..columns).map(move |column| {
                table_values[table_layout.blocks[chunk].offset
                    | patterns[column * n_chunks + chunk] as usize]
            })
        })
        .collect::<Vec<_>>();
    let claims = plan
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let point = (0..block.depth + r2)
                .map(|coordinate| {
                    Gf::from_polynomial_words(
                        [(31 * block_index + 7 * coordinate + 3) as u64, 0],
                    )
                })
                .collect::<Vec<_>>();
            let values = (0..columns)
                .flat_map(|column| {
                    let cut_values = &cut_values;
                    (0..block.n_chunks).map(move |local_chunk| {
                        cut_values[(block.chunk_start + local_chunk) * columns + column]
                    })
                })
                .collect::<Vec<_>>();
            CutClaim {
                block: block_index,
                value: crate::merged_forest::mle_at(&values, &point),
                point,
            }
        })
        .collect::<Vec<_>>();
    let merged = merge_cut_claims(
        &mut Blake3Transcript::new(),
        &plan,
        r2,
        &claims,
    )
    .unwrap();
    let mut source_weights = vec![Gf::ZERO; merged.source_layout.real_len];
    let mut pushforward = vec![Gf::ZERO; table_layout.padded_len];
    fill_pushforward(
        &merged,
        &plan,
        &chunks,
        &table_layout,
        r2,
        &claims,
        &patterns,
        &mut source_weights,
        &mut pushforward,
    );
    assert_eq!(
        table_values
            .iter()
            .zip(&pushforward)
            .map(|(&table, &weight)| table * weight)
            .sum::<Gf>(),
        merged.claim,
    );

    let source_point = (0..merged.source_layout.dim)
        .map(|coordinate| Gf::from_polynomial_words([(19 * coordinate + 11) as u64, 0]))
        .collect::<Vec<_>>();
    let mut padded_source_weights = vec![Gf::ZERO; merged.source_layout.padded_len];
    padded_source_weights[..source_weights.len()].copy_from_slice(&source_weights);
    let source_weight_eval = crate::merged_forest::mle_at(&padded_source_weights, &source_point);
    assert_eq!(source_weight_eval, eval_source_weight(&merged, &claims, &source_point));
    let mut source_index = vec![Gf::ZERO; merged.source_layout.padded_len];
    for (block, packed) in plan.blocks.iter().zip(&merged.source_layout.blocks) {
        for column in 0..columns {
            for local_chunk in 0..block.n_chunks {
                let local = local_chunk | (column << block.depth);
                let chunk = block.chunk_start + local_chunk;
                source_index[packed.offset + local] = encode_table_row(
                    table_layout.blocks[chunk].offset
                        | patterns[column * n_chunks + chunk] as usize,
                );
            }
        }
    }
    let index_eval = crate::merged_forest::mle_at(&source_index, &source_point);
    let tau = Gf::from_polynomial_words([0x12345, 0]);
    let structured = derive_source_index_claim(
        &merged,
        &plan,
        &chunks,
        &table_layout,
        r2,
        &claims,
        tau,
        &source_point,
        source_weight_eval,
        tau + index_eval,
    )
    .unwrap();
    let direct = structured
        .terms
        .iter()
        .map(|term| {
            let column_weights = build_eq_x_r_vec(&term.column_point, &()).unwrap();
            term.row_weights
                .iter()
                .enumerate()
                .flat_map(|(row, &row_weight)| {
                    let bits = &bits;
                    column_weights.iter().enumerate().map(move |(column, &column_weight)| {
                        if bits[row * columns + column] {
                            row_weight * column_weight
                        } else {
                            Gf::ZERO
                        }
                    })
                })
                .sum::<Gf>()
        })
        .sum::<Gf>();
    assert_eq!(direct, structured.value);

    let table_point = (0..table_layout.dim)
        .map(|coordinate| Gf::from_polynomial_words([(23 * coordinate + 13) as u64, 0]))
        .collect::<Vec<_>>();
    let mut table_rows = vec![Gf::ZERO; table_layout.padded_len];
    for (row, value) in table_rows[..table_layout.real_len].iter_mut().enumerate() {
        *value = encode_table_row(row);
    }
    assert_eq!(
        eval_table_encoding(&table_layout, &table_point),
        crate::merged_forest::mle_at(&table_rows, &table_point),
    );
}

#[test]
fn logup_cut_runs_through_both_ligerito_openings() {
    use crate::{
        ligerito::packed_vars,
        ligerito_flock::{LigConfig, commit_rs_ligerito, lig_configs},
        pcs::{IntegerMatrixLayout, smallest_generator},
    };

    let layout = IntegerMatrixLayout {
        row_vars: 7,
        col_vars: 7,
        word_bits: 1,
    };
    let data = (0..layout.cells())
        .map(|index| ((index * 0x9e37 + index.rotate_left(3) + 11) & 1) as u128)
        .collect::<Vec<_>>();
    let row_weights = (0..layout.rows())
        .map(|row| (3 * row + 7) as u128)
        .collect::<Vec<_>>();
    let (pc, vc) = lig_configs(
        packed_vars(&layout),
        LigConfig::Adhoc {
            log_batch: 1,
            log_inv_rate: 3,
        },
    )
    .unwrap();
    let hint = commit_rs_ligerito(&layout, &data, &pc);
    let config = LogupCutConfig {
        chunk_bits: 3,
        aux_component_bits: 64,
        memory_limit_bytes: 0,
    };
    let mut scratch = LogupCutScratch::new(layout, config).unwrap();
    let alpha = smallest_generator();
    let mut prover_transcript = Blake3Transcript::new();
    let mut proof = prove_logup_cut(
        &mut prover_transcript,
        &hint,
        &layout,
        &row_weights,
        alpha,
        &pc,
        config,
        &mut scratch,
    )
    .unwrap();
    let mut verifier_transcript = Blake3Transcript::new();
    verify_logup_cut(
        &mut verifier_transcript,
        &hint.commitment,
        &proof,
        &layout,
        &row_weights,
        alpha,
        &vc,
        config,
    )
    .unwrap();
    assert_eq!(prover_transcript.state_digest(), verifier_transcript.state_digest());
    assert!(logup_cut_proof_size(&proof).total() > 0);
    for column in 0..layout.cols() {
        let expected = (0..layout.rows())
            .map(|row| row_weights[row] * data[layout.cell_index(row, column)])
            .sum::<u128>();
        assert_eq!(proof.v[column], expected);
    }

    proof.v[0] ^= 1;
    assert!(verify_logup_cut(
        &mut Blake3Transcript::new(),
        &hint.commitment,
        &proof,
        &layout,
        &row_weights,
        alpha,
        &vc,
        config,
    )
    .is_err());

    let capped = LogupCutConfig {
        memory_limit_bytes: 1,
        ..config
    };
    assert!(matches!(
        LogupCutScratch::new(layout, capped),
        Err(LogupCutError::MemoryLimit)
    ));
}
