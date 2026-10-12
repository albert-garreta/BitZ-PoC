# Falcon first word-fold optimization

The first signed15 witness fold now prepares its challenge weights once and reduces one unsigned wide accumulator per output. The implementation is shared by algebraic Falcon-512 and Falcon-1024. The selected change improves full proving time at 12 workers by roughly 1–2% under the default preparation policy. It does not reach the 23 ms target.

## Implementation and arithmetic bound

For decoded signed15 values `a,b`, the original output is `a + r*(b-a)`. Define `u(w) = ((w ^ 0x4000) & 0x7fff)`. This maps a signed15 word to its value plus `B=16384`, ignoring the separate spare bit 15. Then:

```text
D = (1-r)*u(a) + r*u(b) - B
```

Prepare field coefficients `[1-r, r, -B]` once and evaluate against unsigned words `[u(a), u(b), 1]` through the existing `PreparedLinearCombination<u64>` kernel. The bias correction participates in the same accumulator. This removes repeated signed field conversion and general wide reduction from the inner loop, with one Montgomery reduction per decoded output.

Each prepared weight is below the modulus `p`, and unsigned operands are at most 32767, so the accumulated integer is less than `65535*p`. This fits the existing accumulator and is below `p*R` for the two-limb field's `R=2^128`. The output remains in the field's usual Montgomery representation. There is no witness-dependent lookup or branch in the new arithmetic and no additional allocation. Coefficient interpolation, later folds, proof structure, and transcript operations retain their existing definitions.

Changes are confined to `word_binding.rs` and its tests. A tracing span identifies the first fold. A 655-file source audit confirms that the isolated tested snapshot and workspace match, with only those two source files changed relative to the initial snapshot. Existing work in the workspace was included identically in both binaries.

## Final 12-worker results: default configuration

Batch 1024, security target 128, five fresh seeds (45–49), three warmups and 15 samples per seed and variant. Timings are proof-only medians of seed medians. Percentage changes are geometric means of paired seed ratios, so they need not equal the ratio of the two displayed medians.

| Falcon degree | Workers | Baseline ms | Optimized ms | Paired change |
|---|---:|---:|---:|---:|
| 512 | 12 | 46.871 | 46.206 | -1.69% |
| 1024 | 12 | 91.765 | 89.912 | -1.84% |

These results use the normal coefficient-preparation selector. The fold optimization runs at every worker count. The selector separately enables optimized coefficient preparation at 1, 8, and 16 workers for sufficiently large batches; that policy was not changed here.

## Final results with optimized preparation held fixed

Batch 1024, security target 128, seeds 42–44, two warmups and nine samples. Both binaries force the same optimized preparation strategy using the existing bench-internals override. This separates the fold change from the selector's slower default preparation at 12 workers.

| Falcon degree | Workers | Baseline ms | Optimized ms | Paired change |
|---|---:|---:|---:|---:|
| 512 | 8 | 34.098 | 32.688 | -4.13% |
| 512 | 12 | 37.290 | 36.446 | -2.24% |
| 512 | 16 | 38.395 | 37.585 | -2.05% |
| 1024 | 8 | 63.432 | 60.924 | -4.09% |
| 1024 | 12 | 67.523 | 65.920 | -2.47% |
| 1024 | 16 | 68.031 | 66.666 | -1.86% |

The 12-worker forced-preparation timings are experimental settings, not the default 12-worker result above. Eight workers remain faster on this machine for these inputs.

## Validation

- 10 word-binding tests passed, including an independent bit-level reference for every first-fold output and next-round sum at signed15 extremes, padded batch sizes, Boolean and non-Boolean challenges, and two moduli.
- Two complete-proof-content tests passed for both Falcon sizes and security levels across serial and parallel execution.
- 3,740 sample and warmup proofs verified and matched their frozen baseline for input digest, source commitment, complete proof Debug digest, payload size, and payload breakdown. Cache-priming proofs are excluded from this count. The Debug fingerprint checks every proof field; it is not a wire-format serialization claim.
- The default sweep covered 96 configurations: degrees 512/1024, batches 1/3/9/17/513/1024, security targets 100/128, and workers 1/8/12/16, using three seeds with one warmup and three samples.
- No proof or total-prover timing in the sweep exceeded the +3% review threshold. The worst short-run proof increase, +2.37% at N512/b17/s128/t16, became +0.08% in a five-fresh-seed, 31-sample confirmation; its paired bootstrap interval includes equality. A consistent +1.00% at N1024/b3/s100/t12 became -0.07%, also with an interval including equality.
- Formatting and diff checks passed. These finite measurements do not guarantee latency or the absence of regressions on other CPUs, inputs, or workloads.

## Alternatives evaluated

Signed prepared interpolation was substantially faster than the original arithmetic. Unsigned biased interpolation with its correction in the same accumulator improved on it at 12 workers. Moving the correction outside the reduction or using a signed difference was slower in the arithmetic kernel. Fusing the first fold with the subsequent product-sum loop was also slower and was discarded. Earlier diagnostic kernels and trace runs are retained separately; their timings are not substituted for the final untraced proving results.

## Reproduction and evidence

CPU: AMD Ryzen 9 9950X3D. Both frozen binaries use the same source snapshot, dependencies, compiler, features, and native release configuration (`falcon-hybrid,bench-internals`, `RUSTFLAGS=-C target-cpu=native`, LTO, one codegen unit). Builds and tests finished before performance runs. Processes ran sequentially with alternating variant order, fixed worker affinity, and identical cached Falcon inputs; setup and fixture generation are outside the proving timer. The host was not reserved exclusively.

- `build-environment.json`: compiler, binary hashes, and final source hashes.
- `baseline-source-sha256.json`, `final-source-audit.json`, and `final.patch`: source provenance and the exact change.
- `final-build.log`, `final-word-tests.log`, and `final-proof-contents-tests.log`: successful final build and tests.
- `validation.json`: completed run counts and comparison fields.
- Each `final-*` directory contains metadata, summaries, per-seed ratios, and raw commands/results. The metadata records exact CPU pins, flags, and binary hashes.
- `compare.py`: paired runner; uses the existing qualification checker at `../falcon-speed-integration-20261012/qualify.py`.

For the clean frozen binaries, pass `--baseline-fold default --candidate-fold default`. The old experimental fold flags only apply to the scratch implementation preserved in `exploration.patch`. Use both `--*-strategy default` for production selection, or both `--*-strategy optimized` for a controlled preparation comparison. Preserve identical strategy, fixtures, and affinity between the two binaries.
