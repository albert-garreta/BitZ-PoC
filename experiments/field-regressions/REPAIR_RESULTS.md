# Regression repair results

ARM experiments on the M1 Max. Both thread counts use the same frozen executable. Production code and x86 are unchanged.

- **1 thread(s):** 271 pass selected checks; 105 retain the actual baseline. Retention is not a speedup.
  Strict frozen-manifest gate: **PASS**.
- **10 thread(s):** 19 pass selected checks; 7 retain the actual baseline. Retention is not a speedup.
  Strict frozen-manifest gate: **PASS**.

## Measured improvements

Ratios use paired process medians. Primitive rows below have 1,024 terms/items; composite shapes are stated explicitly.

| Operation | Threads | Baseline / candidate (µs) | Speedup | Source |
|---|---:|---:|---:|---|
| Two-limb checked unsigned addition | 1 | 0.727 / 0.666 | 1.09× | [words.rs:117](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/words.rs:117) |
| Four-limb exact unsigned MAC | 1 | 9.287 / 7.925 | 1.17× | [words.rs:517](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/words.rs:517) |
| Public sparse 9×9 polynomial dot | 1 | 20.235 / 16.947 | 1.15× | [binary.rs:684](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:684) |
| Packing, logs18/18 | 1 | 407.172 / 279.680 | 1.46× | [composite.rs:507](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:507) |
| Packing + OOD, logs18/18 | 1 | 848.640 / 738.391 | 1.15× | [composite.rs:548](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:548) |
| Packing + OOD, logs18/18 | 10 | 609.904 / 345.232 | 1.76× | [composite.rs:548](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:548) |
| NTT, log16/32 lanes | 1 | 23745.000 / 20937.979 | 1.14× | [composite.rs:367](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:367) |
| NTT, log15/8 lanes | 10 | 1181.682 / 810.464 | 1.44× | [composite.rs:367](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:367) |
| NTT, log18/32 lanes | 1 | 106120.083 / 94161.938 | 1.13× | [composite.rs:367](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:367) |
| NTT, log18/32 lanes | 10 | 21528.146 / 20542.020 | 1.06× | [composite.rs:367](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/composite.rs:367) |

## Previously failing cases

| Threads | Workload | Previous time ratio | New choice | New time ratio | Result |
|---:|---|---:|---|---:|---|
| 1 | opt_integer_mac/l1_signed16_n16 | 1.054× | retain circuit_z | 1.000× | pass |
| 1 | opt_integer_mac/l1_full_n16 | 1.051× | retain circuit_z | 1.000× | pass |
| 1 | opt_uint_add/l1_n16 | 1.063× | retain circuit_z | 1.000× | pass |
| 1 | opt_uint_add/l2_n16 | 1.027× | native_batch | 0.734× | pass |
| 1 | opt_uint_sub/l1_n16 | 1.068× | retain circuit_z | 1.000× | pass |
| 1 | opt_uint_sub/l2_n16 | 1.027× | native_batch | 0.728× | pass |
| 1 | opt_uint_checked_add/l2_n16 | 1.615× | limb_option | 0.893× | pass |
| 1 | opt_uint_checked_add/l2_n1024 | 1.736× | limb_option | 0.918× | pass |
| 1 | opt_uint_checked_sub/l1_n1024 | 1.432× | retain vendor_option | 1.000× | pass |
| 1 | opt_uint_checked_sub/l2_n16 | 1.032× | native_option | 0.584× | pass |
| 1 | opt_uint_checked_sub/l2_n1024 | 1.017× | native_option | 0.559× | pass |
| 1 | opt_exact_unsigned_mac/l4_n16 | 1.029× | column_exact | 0.868× | pass |
| 1 | opt_exact_unsigned_mac/l4_n1024 | 1.031× | column_exact | 0.855× | pass |
| 1 | opt_pack/logs18_18 | 1.081× | tiled_write | 0.685× | pass |
| 1 | opt_packed_ood/logs16_14 | 1.114× | tiled_indexed_ood | 0.793× | pass |
| 1 | opt_packed_ood/logs18_18 | 1.145× | tiled_indexed_ood | 0.872× | pass |
| 1 | opt_f2_poly_dot/a3_b7_sparse_n16 | 1.512× | retain production_zero_skip | 1.000× | pass |
| 1 | opt_f2_poly_dot/a3_b7_sparse_n1024 | 1.887× | retain production_zero_skip | 1.000× | pass |
| 1 | opt_f2_poly_dot/a9_b9_sparse_n16 | 3.998× | row_density | 0.976× | pass |
| 1 | opt_f2_poly_dot/a9_b9_sparse_n1024 | 3.109× | row_density | 0.871× | pass |
| 10 | opt_ood/log16 | 1.202× | retain production_blocked | 1.000× | pass |
| 10 | opt_ood/log18 | 1.110× | retain production_blocked | 1.000× | pass |
| 10 | opt_ntt/log15_lanes32 | 0.988× | retain production | 1.000× | pass |
| 10 | opt_packed_ood/logs18_18 | 1.181× | tiled_indexed_ood | 0.568× | pass |

## Evidence and limits

- [Repair changes and allocation-audit correction](REPAIR_NOTES.md). The audit still measures maximum allocations for one call; it now samples 64 calls to cover periodic Rayon queue allocations.
- [Every selected decision](repair_decisions.json). Baseline retentions and unresolved selections are explicit.
- [Frozen source and executable archive](results/repair-arm-02/archive.json) and [ARM instruction review](results/repair-arm-02/arm-audit/REVIEW.md). Replay commands are in the repair notes.
- Public polynomial variants retain a variable-time public-input contract. The fixed-schedule private-input kernel remains separate; the faster public path does not establish a private-input speedup.
- All-zero, alternating, dense-prefix and sparse-prefix polynomial workloads are included, alongside the original dense/sparse fixtures.
- Correctness runs before timing. These are operation-level results, not an end-to-end prover or x86 performance claim.
- [1-thread measurements](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/repair-arm-02/measurements.md), including rejected diagnostic variants, with raw logs and hashes in the same directory.
- [10-thread measurements](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/repair-arm-10cores-03/measurements.md), including rejected diagnostic variants, with raw logs and hashes in the same directory.
