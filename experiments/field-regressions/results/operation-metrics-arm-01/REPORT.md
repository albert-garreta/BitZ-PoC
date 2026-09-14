# Unified arithmetic operation metrics

Measured **125 variant cases across 28 operation families** on the Apple M1 Max. All correctness checks completed, and every new timed hot path allocated zero bytes. The eight preselected GF128 comparisons against F2Z finished with **2 pass, 0 regression and 6 inconclusive** results under the existing 1% gate.

These are operation-level measurements of current implementations and isolated prototypes. The complete unified library is not implemented or integrated. Baseline-only costs below establish comparison targets; they do not establish speedups for future implementations. No full-prover, hybrid or x86 speedup is established.

## Fresh GF128 comparisons

Times are microseconds per batch of 1,024 independent operations. Native input layouts and output buffers are prepared before timing. Ratios compare the selected implementation with **F2Z**, using paired process medians; they may differ slightly from quotients of the pooled times. A pass establishes the specified regression bound, not a strict speedup.

| Operation | F2Z µs | Flock µs | Shared µs | ARM scalar-lane µs | Selected/F2Z [95% CI] | Gate |
|---|---|---|---|---|---|---|
| add | 0.277 | 0.279 | 0.277 | — | 0.998 [0.991, 1.003] | inconclusive |
| mul | 0.928 | 0.872 | 0.929 | 0.874 | 0.943 [0.935, 0.950] | pass |
| square | 0.673 | 2.436 | 0.673 | — | 1.007 [0.996, 1.011] | inconclusive |
| inverse | 457.521 | 1109.026 | 457.357 | — | 0.998 [0.993, 1.004] | inconclusive |

F2Z already has specialized GF128 squaring and inversion. Earlier large gains were measured against Flock, so they cannot be attributed to replacing the F2Z implementations. Multiplication uses the adapted ARM scalar-lane candidate where shown. GF addition/subtraction is XOR; subtraction equivalence is checked outside timing. Inverse timings use the common nonzero-input domain.

All selected small/large GF128 cases:

| Operation | Elements | Selected | Time/F2Z [95% CI] | P95 ratio [95% CI] | Gate |
|---|---|---|---|---|---|
| mul | 16 | scalar_lanes | 0.974 [0.970, 0.982] | 0.978 [0.965, 1.000] | pass |
| mul | 1024 | scalar_lanes | 0.943 [0.935, 0.950] | 0.933 [0.907, 0.952] | pass |
| add | 16 | shared | 0.996 [0.960, 1.016] | 0.988 [0.954, 1.032] | inconclusive |
| add | 1024 | shared | 0.998 [0.991, 1.003] | 0.997 [0.980, 1.010] | inconclusive |
| square | 16 | shared | 1.002 [0.995, 1.007] | 0.994 [0.939, 1.018] | inconclusive |
| square | 1024 | shared | 1.007 [0.996, 1.011] | 1.011 [0.933, 1.094] | inconclusive |
| inverse | 16 | shared | 0.998 [0.992, 1.004] | 1.002 [0.969, 1.015] | inconclusive |
| inverse | 1024 | shared | 0.998 [0.993, 1.004] | 0.996 [0.956, 1.016] | inconclusive |

## Newly measured baseline costs

These rows measure current code only. Each batch contains 1,024 elements, except the round contains 1,024 weighted pairs. The per-element number is batch throughput, not isolated-call latency. No replacement speedup has been measured for these rows.

| Operation | Existing implementation | Batch µs | ns/element or pair |
|---|---|---|---|
| GF8 add | flock | 0.021 | 0.020 |
| GF8 multiply | flock | 1.488 | 1.453 |
| GF8 inverse | flock | 37.277 | 36.403 |
| B127 add | f2z | 0.277 | 0.271 |
| B127 multiply | f2z | 1.234 | 1.205 |
| B127 square | f2z | 0.665 | 0.649 |
| B127 inverse | f2z | 436.935 | 426.694 |
| GF128 fused weighted round | f2z_single_pair | 5.598 | 5.466 |

The fused round calls `WideMulAcc::eqf_single_pair_round` and consumes 2,048 left values, 2,048 right values and 1,024 weights. GF8 is the existing Flock implementation; B127 is the existing F2Z field, using its own modulus. These are not new owned-library implementations.

### Integer baselines

Microseconds per 1,024 operations. Widths are 64-bit limbs: 2 = 128 bits, 4 = 256 bits, 9 = 576 bits. The same mixed fixture includes successful operations, overflows, signed extrema and full-width random inputs. Checked timings retain the existing `Option` results and branches; they are not constant-time replacement measurements.

| Operation | 2 limbs µs | 4 limbs µs | 9 limbs µs |
|---|---|---|---|
| Wrapping add | 0.631 | 2.010 | 3.731 |
| Wrapping subtract | 0.668 | 1.658 | 3.962 |
| Checked unsigned add | 1.255 | 1.505 | 4.346 |
| Checked unsigned subtract | 1.243 | 1.721 | 4.725 |
| Checked unsigned multiply | 1.959 | 7.511 | 62.452 |
| Checked signed add | 1.161 | 2.035 | 4.632 |
| Checked signed subtract | 1.205 | 2.335 | 6.529 |
| Checked signed multiply | 3.910 | 9.934 | 32.026 |
| Unsigned compare | 0.779 | 2.004 | 6.269 |
| Exact widened unsigned multiply | 1.425 | 6.795 | 50.714 |
| Unsigned div/rem, 64-bit divisor | 44.921 | 95.417 | 273.624 |
| Unsigned div/rem, full-width divisor | 45.032 | 96.247 | 276.236 |

Wrapping add/subtract call `circuit::witgen::Z`; checked operations use the existing crypto-primitives wrappers. Widened multiplication produces all `2L` limbs, unlike a wrapping MAC. Division uses crypto-bigint with a public divisor whose nonzero status is validated before timing; it does not benchmark a future reciprocal-preparation API.

### Runtime prime baselines

Known prime `q = 2^127−1`; no primality testing is included. Inversion is the current variable-time operation applied independently, not a Montgomery batch inversion.

| Operation | µs/pass | ns/element or setup |
|---|---|---|
| 1,024 independent inversions | 872.979 | 852.519 |
| 1,024 powers, exponent 65537 (17-bit bound) | 429.523 | 419.456 |
| 1,024 powers, alternating exponent (127-bit bound) | 1991.136 | 1944.468 |
| Encode 1,024 canonical field elements | 2.117 | 2.067 |
| Decode 1,024 canonical field elements | 5.216 | 5.093 |
| One Montgomery + raw context setup | 0.122 | 122.192 |

Codec timings measure existing element-conversion components into preallocated buffers, excluding allocating wrappers, framing and full proof serialization. Decode uses public canonical fixtures. Prime selection remains low priority; the user-provided PIOP breakdown records 0.18 ms of a 53.52 ms PIOP, and is not a new measurement from this suite.

## Retained comparisons from earlier frozen campaigns

These are the latest applicable saved comparisons, not reruns in the new executable. Every row compares candidates with their baseline **inside its own campaign**. Do not divide absolute times across campaigns. Time ratios below 1 mean faster. The updated integer campaign supersedes the older two-limb and bounded-product prototypes.

| Operation, 1,024 terms/products | Baseline µs | Candidate µs | Time ratio | Gate |
|---|---|---|---|---|
| GF128 delayed dot, 1,024 terms | 0.696 | 0.696 | 1.000 | pass |
| 2-limb MAC, signed16 fixture | 0.689 | 0.657 | 0.952 | pass |
| 2-limb MAC, full128 fixture | 0.687 | 0.656 | 0.953 | pass |
| 1-limb MAC, signed16 fixture | 0.323 | 0.323 | 1.003 | inconclusive |
| 4-limb MAC, signed16 fixture | 3.112 | 4.130 | 1.328 | regression |
| 9-limb MAC, signed16 fixture | 21.263 | 17.197 | 0.810 | pass |
| 4-active-limb products, prepared | 21.329 | 6.778 | 0.318 | pass |
| 4-active-limb products, including validation | 21.329 | 8.613 | 0.403 | pass |
| 9-active-limb products, prepared | 60.005 | 54.805 | 0.914 | pass |
| 9-active-limb products, including validation | 60.005 | 54.843 | 0.914 | pass |
| Prime scalar multiply, q128, branded | 4.827 | 4.827 | 1.000 | inconclusive |
| Prime product MAC (R²), q128 | 2.390 | 2.362 | 0.988 | pass |
| Prime native MAC (R), q128 | 1.487 | 1.325 | 0.889 | pass |
| GF fixed scalar, zero | 0.742 | 0.195 | 0.273 | pass |
| GF fixed scalar, half width | 0.742 | 0.946 | 1.272 | regression |
| GF fixed scalar, full width | 0.741 | 0.742 | 1.001 | pass |
| GF butterfly, zero scalar | 1.103 | 0.590 | 0.534 | pass |
| GF butterfly, half-width scalar | 1.103 | 1.162 | 1.053 | regression |
| GF butterfly, full-width scalar | 1.103 | 1.103 | 1.001 | pass |

Baselines: GF dot uses actual F2Z `WideMulAcc`; integer MAC uses actual circuit `Z<L>`; bounded products use existing P-256 multiplication. Fixed-scalar/butterfly use the existing experiment’s prepared formula adaptation, not a call to the private production API. Prime scalar uses raw Montgomery context arithmetic; the two prime MACs use their respective existing delayed accumulators. Prime product MAC still regresses about 4–5% at 16 terms. A scoped prime API and mixed signed-coefficient MAC have not been benchmarked as completed replacements.

### Reduction and integer-to-prime projection controls

Reduction times are for **one reduction** of a previously built 1,024-term accumulator. The existing optimized reducer is already faster than the crypto-bigint reference; replacing it with that reference would lose performance.

| Reduction | Existing optimized µs | Reference µs | Reference/existing | Gate |
|---|---|---|---|---|
| Product accumulator, 100-bit modulus | 0.016 | 0.106 | 6.680 | regression |
| Product accumulator, 128-bit modulus | 0.016 | 0.105 | 6.609 | regression |
| Native accumulator, 100-bit modulus | 0.013 | 0.100 | 7.514 | regression |
| Native accumulator, 128-bit modulus | 0.013 | 0.098 | 7.435 | regression |

Projection compares the existing `RuntimeModulus` path against an external fixed-storage, variable-time crypto-bigint control. This identifies headroom; it is not an implemented owned constant-time replacement. Times below are for 1,024 signed coefficients.

| Projection | Existing µs | Reference µs | Reference/existing | Gate |
|---|---|---|---|---|
| q100, 2 limbs | 547.643 | 49.135 | 0.090 | pass |
| q100, 4 limbs | 1333.448 | 78.182 | 0.059 | pass |
| q100, 9 limbs | 3296.688 | 154.250 | 0.047 | pass |
| q128, 2 limbs | 22.809 | 46.437 | 2.021 | regression |
| q128, 4 limbs | 84.993 | 73.942 | 0.868 | pass |
| q128, 9 limbs | 231.477 | 149.957 | 0.648 | pass |

### NTT coverage

| Transform shape | Flock µs | Preserved-schedule candidate µs | Time ratio | Gate |
|---|---|---|---|---|
| log8_lanes1 | 2.532 | 2.604 | 1.028 | regression |
| log8_lanes32 | 45.833 | 45.634 | 0.994 | pass |
| log12_lanes8 | 406.552 | 404.602 | 0.991 | inconclusive |
| log15_lanes32 | 11004.000 | 11032.166 | 1.002 | regression |
| log16_lanes32 | 23117.500 | 23239.812 | 1.004 | regression |
| log17_lanes32 | 50293.771 | 50304.812 | 1.000 | inconclusive |
| log18_lanes32 | 105398.042 | 105490.895 | 0.997 | inconclusive |

The log15/log16 candidates add one hot allocation of 1,520 bytes; their near-parity medians do not pass the allocation gate. Log8/one-lane has a measured slowdown; other listed inconclusive cases do not establish the P95 bound. This one-thread campaign does not establish multicore NTT performance. There is no 1,024-point NTT row in this campaign.

## Method, limitations and evidence

The new campaign used five fresh processes × 32 shuffled paired samples, seed offset 5772157, one caller thread, `-C target-cpu=native`, optimization level 3, fat LTO and one codegen unit. The runner froze sources, dependencies, the manifest and executable before measurement. Retried cases: metrics_gf_add/1024, metrics_gf_add/16, metrics_gf_inverse/1024, metrics_gf_inverse/16, metrics_gf_square/1024, metrics_gf_square/16. Each retry follows the predeclared five-process × 64-sample policy and replaces the initial result; no retry-until-pass selection was used.

The 1% gate requires the median and P95 upper confidence bounds and every process median to be at most 1.01, correct outputs and no added allocations. Intervals are pointwise 95% hierarchical paired bootstrap intervals; P95 describes timed-batch averages, not individual-call tails. Core placement, temperature and unrelated host load were uncontrolled. The Apple M1 Max/64 GiB host identity comes from the earlier host observation; the runner’s sandboxed hardware query may be unavailable in metadata.

Validation: independent GF polynomial/BigUint/BigInt oracles, exhaustive GF8 products, carries and signed boundaries, quotient/remainder identities, prime powers/inverses, canonical codec checks, empty and ragged inputs. All 125 smoke cases matched the manifest. Every confirmation process completed correctness checks. The existing Python policy suite passed 17 tests. Independent reviews checked integer oracle semantics, output observability, divisor setup and report attribution; no remaining issues were found.

Remaining measurements require implementations: full owned-library/context API, exact signed-wide and mixed coefficient accumulators, fixed-schedule secret-input arithmetic, batch inversion, mutable fused folds and integrated proof paths. New baseline timings include current variable-time APIs and do not certify constant-time behavior. ARM results do not establish x86 performance. Production code was unchanged by this work.

- [Every new measurement, including 16-element batches and P95](measurements.md)
- [New operation CSV](operation-metrics.csv) and [retained comparison CSV](retained-comparisons.csv)
- [New statistics](summary.json), [frozen manifest](required_cases.json), [metadata and hashes](metadata.json), [gate output](gate.txt)
- Raw CSV/logs under `initial/` and optional `retry/`
- [Frozen source/executable archive](frozen-inputs.tar.gz) and [attachment hashes](attachments-sha256.json)
- [Reproduction and API scope](../../OPERATION_METRICS.md)
- Earlier reports: [arithmetic](../arithmetic-arm-01/REPORT.md), [updated integer kernels](../integer-focus-arm-03/REPORT.md), [GF/NTT statistics](../arm-gated-01/measurements.md)
