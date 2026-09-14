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
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | production_one_inverse |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_vartime |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | compact_ct |  | — | — | — | — | — | unmeasured |
| aarch64 | opt_batch_inverse | q100_nonzero_n1024 | public_nonzero | yes | — | — | — | — | — | unmeasured |
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
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | production_zero_skip |  | 1.000 [1.000, 1.000] | 1.000 [1.000, 1.000] | 0.085 / 0.094 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | fixed_schedule |  | 1.037 [0.936, 1.070] | 1.044 [0.919, 1.071] | 0.087 / 0.093 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_fused |  | 0.945 [0.923, 0.952] | 0.938 [0.920, 0.956] | 0.080 / 0.089 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_adaptive |  | 1.062 [0.962, 1.102] | 1.078 [0.955, 1.105] | 0.089 / 0.096 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | row_density |  | 0.738 [0.717, 0.824] | 0.747 [0.719, 0.807] | 0.064 / 0.077 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_masks |  | 0.863 [0.836, 0.998] | 0.891 [0.833, 0.994] | 0.073 / 0.093 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks8 |  | 1.019 [1.011, 1.043] | 1.028 [1.001, 1.049] | 0.088 / 0.096 | 0.001 | 0 / 0 | regression |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_blocks32 |  | 0.998 [0.970, 1.008] | 0.994 [0.970, 1.011] | 0.084 / 0.094 | 0.001 | 0 / 0 | inconclusive |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_classified |  | 0.775 [0.757, 0.820] | 0.771 [0.756, 0.815] | 0.066 / 0.076 | 0.001 | 0 / 0 | pass |
| aarch64 | opt_f2_poly_dot | a3_b7_dense_prefix_n16 | public_scan | yes | 0.967 [0.945, 0.979] | 0.960 [0.933, 0.976] | 0.082 / 0.091 | 0.001 | 0 / 0 | pass |
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
