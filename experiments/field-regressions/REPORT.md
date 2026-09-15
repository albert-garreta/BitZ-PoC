# Regression audit: measured results

The current shared library is **not yet demonstrated regression-free**. The
experiment found concrete runtime and storage regressions, and a candidate that
recovers the existing multiplication schedule. Production files were unchanged.

Measurements used this Mac's `aarch64-apple-darwin` target, Rust 1.98.1 / LLVM
22.1.8, native CPU flags, fat LTO, one codegen unit, and one Rayon thread. Each
round used three fresh processes with different deterministic inputs and shuffled
candidate order. All arithmetic, NTT, and matrix equivalence checks passed.
System load, core placement, and thermals were not controlled.

## Findings that block migration

| Proposed replacement | Measured result | Decision |
|---|---|---|
| Current shared scalar multiply in an output-array loop | About 6% slower at 1,024–1,048,576 elements; reproduced in subsequent builds | Preserve the existing kernel schedule |
| Generic shared NTT loop, 32 lanes × 32,768 points | 13.5% slower; 95% relative-time CI 1.125–1.141 | Preserve the complete production NTT schedule |
| NTT loop retaining zero/half-width twiddles | Still 15.3% slower at the largest size in the first round | Twiddle specializations alone do not replace cache/fusion scheduling |
| Prepared fixed-twiddle NTT | 12.1% slower at the largest size, although faster at smaller sizes | No universal replacement |
| Two-element NEON kernel used for a 1,024-term dot product | 99.1% slower than the existing eager dot loop | Reject for this workload; this is not the historical NTT batching measurement |
| Sixteen wide accumulators | 27.6% slower than eager dot at 1,024 terms | Reject; assembly keeps the accumulator array on the stack |
| Inline P-256 `Z<9>` coefficients | About 10% faster matrix application, but 9.39–9.89× matrix payload | Reject under the storage regression gate |
| Compact u16 coefficient IDs | About 2.4% slower for the full matrix and 4.6–4.7% for prefixes | Saves storage but does not satisfy a strict no-slowdown gate |

The NTT comparisons include complete transforms and the same reset copy in every
candidate. The reset alone was about 3.4% of the largest baseline timing. These
are changes to complete consumer implementations, not isolated scalar-multiply
attributions. Complete sumcheck, folding, and prover integrations remain untested.

## Follow-up experiments

Unrolling shared product loops by 2, 4, or 8 did not recover the roughly 6%
regression. It also hurt the smallest product batches.

The `scalar_lanes` candidate keeps Flock's six-PMULL product graph and returns
`field::F128`. Its median product times were 0.998–1.000× Flock across the tested
sizes. The 19-instruction repeated product loop is byte-identical to Flock's in
the final binary; dispatch/entry blocks are separate. Its largest upper CI bound
was 1.0083, so the **zero-tolerance statistical gate remains inconclusive** for
products. This supports retaining that kernel behind the unified interface, not
claiming a proven zero-cost migration.

A single deferred-reduction accumulator passed the strict runtime gate for all
four dot sizes. At 1,024 terms its final relative time was 0.8092
[0.8051, 0.8142]: about **1.24× throughput**, or 19.1% less time. The preliminary
cached-binary 1.64× result is not the result of this fresh comparison. Two and
four accumulators were similar at larger sizes; one was better at 16 terms.
Sixteen accumulators introduced stack traffic. Keep the simple one-accumulator
candidate for this tested batch-MAC shape; consumer-level adoption is separate.

For independent products, retaining Flock's arithmetic is the best supported
choice pending a concrete integration. For large NTTs, retain the existing full
kernel. For matrix storage, retain the u32 interned layout. These are the best
supported configurations among the tested candidates, not global optima.

## Exact storage regression

The actual synthesized P-256 A/B/C matrices have `E = 4,241,586` entries,
`D = 3,358` distinct coefficients, and `R = 21,183` rows. A nine-limb coefficient
occupies 72 bytes; an inline AoS entry occupies 80 bytes after alignment.

```text
indexed u32 = 8E + 72D + 8(R+1) =  34,343,936 bytes =  32.75 MiB
indexed u16 = 6E + 72D + 8(R+1) =  25,860,764 bytes =  24.66 MiB
inline AoS  = 80E      + 8(R+1) = 339,496,352 bytes = 323.77 MiB
inline SoA  = 76E      + 8(R+1) = 322,530,008 bytes = 307.59 MiB
```

These are exact payload sizes for the benchmark layouts, excluding shared
inputs/outputs and allocator overhead. The interned baseline uses a nine-limb
pool; this is not a measurement of every allocation in the existing prover.
Small-prefix measurements keep the complete pool, which can make inline storage
look smaller for tiny prefixes. The complete-matrix gate catches the expansion.

Reducing every entry repeats each unique-coefficient reduction on average
`E/D ≈ 1,263` times. With the same public-coefficient BigInt reducer and output
construction, the measured setup was roughly 1.157 seconds per-entry versus
1.047 milliseconds pooled: a 1,113× paired slowdown. This is a setup comparison,
not a total prover speedup or a benchmark of the future library's reducer.

Matrix application uses the actual sparsity/coefficient structure and synthetic
unsigned 16-bit vectors. Every row is verified against BigInt; a bound proves
the exact signed result fits 576 bits. It does not substitute for a valid ECDSA
witness or the eventual unified integer multiplication API.

## Timing and constant-time limits

The inspected native product loops contain six PMULL instructions, fixed-address
loads/stores relative to public loop indices, and a public loop-count branch.
The old and shared loops differ in product ordering; copying Flock's scalar-lane
graph produces identical repeated-loop instructions and removes the observed
6% gap. This is evidence of code-generation sensitivity, not a hardware-counter
proof of the precise stall mechanism. No CPU sampling or hardware-counter trace
was collected; conclusions about stack traffic come from disassembly.

This is not a constant-time certification. In particular, the shared portable
`clmul64` source branches on an operand bit and still needs compiler/target
analysis. Inversion, division, runtime-prime arithmetic, and integer edge-case
semantics need their own audit. The BigInt oracle/reduction benchmarks are
variable-time and operate on public coefficients/test data only.

x86 performance remains unmeasured. The current shared GF128 selector uses its
portable backend on x86, while vendored Flock has PCLMUL support. Migration on
`will` must remain blocked until that backend is preserved and measured there.

## Reproduction and evidence

- [Harness and methodology](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/README.md), [strict gate](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/gate.py).
- [First complete round](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/native-02/measurements.md): 24 paired samples per
  case per process; coefficient reduction uses 12 because each pass is expensive.
- [Unrolling round](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/native-03/measurements.md): 32 samples per process.
- [Scalar-lane confirmation](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/native-04/measurements.md): 32 samples per process.
- Each result directory contains raw CSVs, P10/P50/P90/P99 batch latency data,
  pointwise 95% paired bootstrap CIs, per-process ratios, setup observations,
  source hashes, compiler/build metadata, and a dependency lockfile.
- [Product-loop disassembly](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/native-04/products.s),
  [byte comparison](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/native-04/loop-comparison.json), and
  [wide-accumulator disassembly](/Users/johnwu/code/zk/f2z-pcs/experiments/field-regressions/results/native-02/wide.s).

The strict gate was checked against both passing MAC cases and deliberately
regressing product/NTT/storage replacements. It rejects uncertainty rather than
equating "not statistically slower" with "proven equal." No production candidate
was promoted. Further work should be driven by the actual migration and its
consumers; this audit makes no claim that the original exhaustive optimization
search has converged across architectures or the future unified library.
