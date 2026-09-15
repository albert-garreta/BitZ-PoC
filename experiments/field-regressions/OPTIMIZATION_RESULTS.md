# ARM optimization results

Historical campaign. The subsequent [regression repair results](REPAIR_RESULTS.md)
record the final passing selections; the measurements below remain unchanged.

Implemented 34 operation families: **236 workloads / 627 variant rows** on one thread, plus **19 composite workloads / 38 rows** on all 10 physical cores. Hardware: Apple M1 Max, 64 GiB, Rust 1.98.1, `-C target-cpu=native`. Both campaigns used the **same executable**.

All measured variants passed correctness. Each campaign used five processes with 32 paired samples, followed by the single predefined five-process/64-sample retry for selected inconclusive cases. One thread finished with **149 selected passes, 20 regressions and 16 inconclusive results**. The 10-core subset finished with **9 selected passes, 4 regressions and 5 inconclusive results**. A passing row meets the 1% latency/allocation gate; it does not necessarily demonstrate a speedup.

**The suite does not pass as a blanket replacement.** Keep the existing implementation for rejected or unresolved workload cases. These changes are isolated experiments; production code and dependencies remain unchanged.

## Confirmed improvements

Array and MAC rows below use 1,024 elements/terms; sumcheck rows use 1,024 weighted pairs. Projection inputs are predominantly full-width signed integers, so these gains do not predict the cost of tiny relation coefficients. Times are microseconds for the whole batch. Speedup uses the frozen paired median ratio.

| Operation / contract | Baseline µs | Candidate µs | Paired speedup | Source |
|---|---:|---:|---:|---|
| Four-limb wrapping MAC, full-width | 3.157 | 2.881 | 1.10× | [integer.rs:85](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/integer.rs:85) |
| Two-limb exact signed MAC | 4.854 | 3.820 | 1.27× | [words.rs:255](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/words.rs:255) |
| Signed projection q100 / 2 limbs, prepared | 566.664 | 7.622 | 73.81× | [integer.rs:193](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/integer.rs:193) |
| Signed projection q128 / 2 limbs, prepared | 22.794 | 7.621 | 2.99× | [integer.rs:193](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/integer.rs:193) |
| Signed projection q100 / 9 limbs, prepared | 3366.874 | 94.791 | 35.28× | [integer.rs:193](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/integer.rs:193) |
| Signed projection q128 / 9 limbs, prepared | 235.411 | 95.406 | 2.47× | [integer.rs:193](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/integer.rs:193) |
| Prime native linear MAC, q128 | 1.526 | 1.352 | 1.13× | [prime.rs:175](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/prime.rs:175) |
| Public prime exponent 65537 | 432.656 | 162.297 | 2.67× | [prime.rs:247](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/prime.rs:247) |
| GF128 half-width fixed multiplier | 0.747 | 0.459 | 1.64× | [binary.rs:13](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:13) |
| GF128 half-width butterfly | 0.923 | 0.686 | 1.35× | [binary.rs:47](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:47) |
| GF128 single-pair weighted round | 5.741 | 4.685 | 1.22× | [binary.rs:143](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:143) |
| GF128 two-pair weighted round | 11.200 | 9.326 | 1.21× | [binary.rs:212](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:212) |
| GF128 fold plus weighted round | 11.952 | 11.261 | 1.06× | [binary.rs:212](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:212) |
| GF8 multiplication batch (existing vector kernel) | 1.481 | 0.068 | 21.82× | [binary.rs:398](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:398) |
| GF8 inverse vector chain | 37.172 | 1.628 | 22.69× | [binary.rs:438](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:438) |
| Dense packed F2 polynomial dot, 3×7 words | 8.248 | 6.572 | 1.26× | [binary.rs:584](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/src/campaign/candidates/binary.rs:584) |

GF8 multiplication reuses an existing Flock vector kernel; its denominator is the scalar F8 loop. Prepared reciprocal division likewise reuses crypto-bigint rather than introducing a new divisor kernel. The projection setup and one-shot comparisons are in the full table; notably q128/two-limb/n16 one-shot remains inconclusive.

## Composite workloads

NTT measurements include identical buffer reset copies. Packing-plus-OOD retains the packed Vec for the later opener; it does not include transcript grinding or that opening proof.

| Workload | Threads | Baseline µs | Candidate µs | Paired speedup | Gate |
|---|---:|---:|---:|---:|---|
| ood / log18 | 1 | 221.471 | 199.509 | 1.12× | pass |
| ntt / log12_lanes8 | 1 | 420.680 | 377.112 | 1.12× | pass |
| ntt / log16_lanes32 | 1 | 24058.021 | 23060.854 | 1.04× | inconclusive |
| ntt / log18_lanes32 | 1 | 107818.708 | 103932.333 | 1.04× | pass |
| packed_ood / logs16_14 | 1 | 271.453 | 301.917 | 0.90× | regression |
| packed_ood / logs18_18 | 1 | 841.219 | 978.417 | 0.87× | regression |
| ood / log18 | 10 | 127.146 | 140.708 | 0.90× | regression |
| ntt / log12_lanes8 | 10 | 460.164 | 250.682 | 1.80× | pass |
| ntt / log16_lanes32 | 10 | 5581.979 | 5730.105 | 0.99× | inconclusive |
| ntt / log18_lanes32 | 10 | 23168.542 | 23292.688 | 0.99× | inconclusive |
| packed_ood / logs16_14 | 10 | 324.922 | 266.951 | 1.24× | pass |
| packed_ood / logs18_18 | 10 | 589.951 | 689.823 | 0.85× | regression |

The one-thread OOD improvement does not carry over to larger ten-core workloads. Large ten-core NTTs are unresolved; log15/32 lanes also failed the allocation gate. Keep the production paths for those shapes. The tiny log8/one-lane NTT candidate is slower.

## Regressions and retained implementations

- Keep the existing small-width unsigned checked operations where the word implementation regresses. At two limbs/n1024, checked addition takes about 1.74× the baseline time.
- Keep the existing four-limb **exact unsigned** MAC: the fused candidate is about 3% slower. This is separate from the faster four-limb **wrapping** MAC.
- Keep crypto-bigint for large exact products and the original B127 multiplication. The generic schoolbook and alternative B127 schedules lose.
- Full-width GF fixed-scalar dispatch adds overhead; the demonstrated specialization wins are zero and half-width public constants.
- Zero-masked inversion and fixed-address phi8 have different input-timing contracts from the zero-skipping/table controls. The faster public controls do not justify input-dependent dispatch for private values.
- Fixed-schedule polynomial dots improve dense cases but lose on sparse zero-heavy inputs (about 1.89× and 3.11× time for 3×7 and 9×9 words at n1024). Preserve the timing-contract distinction.
- Packing improvements depend on shape and thread count. Several larger packing-plus-OOD cases regress despite reducing allocation counts.

## Review and reproducibility

- [Candidate implementation and contracts](OPTIMIZATION.md), [source-linked baseline catalogue](BASELINES.md), and [public API plan](UNIFIED_LIBRARY_API.md).
- [All one-thread measurements](results/optimization-arm-01/measurements.md) and [all ten-core measurements](results/optimization-arm-10cores-01/measurements.md). Raw CSVs, logs, manifests, hashes and metadata are in those directories.
- [Per-workload decisions](optimization_decisions.json) record frozen selection eligibility. Diagnostic winners are not silently promoted.
- [Frozen ARM assembly review](results/optimization-arm-01/arm-audit/REVIEW.md): the initial projection borrow branch was replaced with CtSelect before confirmation. This is a bounded review, not a whole-prover timing certification.
- The Python harness tests pass (17 tests). The strict complete-manifest gate correctly rejects the full replacement; the ten-core run intentionally covers only the composite subset.

These measurements establish operation-level results on this Mac. They do not establish an end-to-end prover speedup or performance on another architecture. Unchanged operations retain their earlier baseline measurements; general modular rings, signed division, private fixed-base exponentiation and production migration remain outside this implemented experiment.
