# Unified arithmetic benchmark measurements

Allowed slowdown: 1.0%. Ratios are candidate / baseline. Intervals are pointwise 95% hierarchical paired bootstrap CIs.
P95 describes timed-batch averages, not individual-call tails. Missing architectures/cases are unmeasured. Diagnostics cannot be selected.

| Arch | Operation | Size | Variant | Selected | Median ratio [CI] | P95 ratio [CI] | P50 / P95 (µs) | Payload (MiB) | Alloc calls / bytes | Status |
|---|---|---|---|---|---|---|---:|---:|---:|---|
| aarch64 | opt_exact_signed_mac | l2_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n16 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n16 | signed_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | fused_exact | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l2_n1024 | signed_columns |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n16 | signed_columns | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l4_n1024 | signed_columns | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n16 | signed_columns | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | wide_then_add |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | fused_exact |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_exact_signed_mac | l9_n1024 | signed_columns | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 26.740 / 27.861 | 0.000 | 2 / 163840 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | 0.971 [0.957, 0.976] | 0.967 [0.953, 0.978] | 25.868 / 26.977 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | 0.984 [0.974, 0.988] | 0.979 [0.970, 0.991] | 26.182 / 27.319 | 0.000 | 2 / 98304 | pass |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | public_nonzero | yes | 0.964 [0.960, 0.973] | 0.963 [0.957, 0.976] | 25.766 / 26.883 | 0.000 | 2 / 98288 | pass |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_mixed_n1024 | public_nonzero | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_zero_n1024 | public_nonzero | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_nonzero_n1024 | public_nonzero | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_mixed_n1024 | public_nonzero | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q128_zero_n1024 | public_nonzero | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | reuse_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | prepared_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | tiled_1024 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | tiled_256 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | vector_1024 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | vector_2048 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | tiled_collect |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log10 | tiled_grouped | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | reuse_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | prepared_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | tiled_1024 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | tiled_256 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | vector_1024 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | vector_2048 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | tiled_collect |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log16 | tiled_grouped | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | production_blocked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | reuse_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | indexed_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | collect_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | vector_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | prepared_products |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | tiled_1024 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | tiled_256 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | vector_1024 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | vector_2048 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | tiled_collect |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ood | log18 | tiled_grouped | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | half_depth |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes1 | incremental | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | half_depth |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log8_lanes32 | incremental | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | production |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | depth_first |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | tiled_half |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | half_depth |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_ntt | log12_lanes8 | incremental | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | chunked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | direct_tiles |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 16 | direct_pass | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | production_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | chunked |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | direct_tiles |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_gf_grid | 1024 | direct_pass | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n16 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_n1024 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n16 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_n1024 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n16 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_zero_n1024 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.076 / 0.087 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | 1.138 [0.995, 1.258] | 1.139 [0.991, 1.249] | 0.086 / 0.088 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | 0.951 [0.943, 0.989] | 0.957 [0.940, 0.992] | 0.075 / 0.083 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | 1.175 [1.038, 1.307] | 1.174 [1.066, 1.290] | 0.089 / 0.092 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | 0.756 [0.671, 0.778] | 0.757 [0.663, 0.784] | 0.057 / 0.067 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_masks |  | 0.865 [0.758, 0.971] | 0.855 [0.768, 0.967] | 0.065 / 0.074 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks8 |  | 1.020 [1.012, 1.070] | 1.032 [1.006, 1.053] | 0.077 / 0.088 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks32 |  | 0.990 [0.956, 1.046] | 0.990 [0.959, 1.028] | 0.075 / 0.084 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_classified |  | 0.745 [0.695, 0.763] | 0.744 [0.693, 0.775] | 0.056 / 0.067 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_scan | yes | 0.971 [0.957, 1.013] | 0.978 [0.961, 1.014] | 0.077 / 0.084 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n1024 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n16 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_sparse_prefix_n1024 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n16 | public_scan | yes | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | production_zero_skip |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | fixed_schedule |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_fused |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_adaptive |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | row_density |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_masks |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks8 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_blocks32 |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_classified |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_f2_poly_dot | a3_b7_alternating_n1024 | public_scan | yes | — | — | — | — | — | unmeasured |
