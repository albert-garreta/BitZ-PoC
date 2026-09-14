# Unified arithmetic: measured ARM results

Implemented and measured the isolated benchmark suite on an **Apple M1 Max, 64 GiB, one thread**. All 408 required variant cases completed. All correctness checks passed across five initial processes and five retry processes. The 79 predeclared candidate cases finished with **50 pass, 22 regression, 7 inconclusive** under the 1% policy. The performance gate therefore fails; these are candidate regressions, not newly introduced production bugs.

Five initial processes × 32 paired samples; 15 inconclusive selected operation/size cases received the single predeclared retry at five processes × 64 samples. Source, binary, raw samples and manifest are frozen. Core placement, thermal state and unrelated host load were uncontrolled. Five setup-only cases are complete diagnostics and intentionally have no performance pass label.

## Representative comparisons

All rows below use 1,024 terms/pairs except isolated reduction. Prime arithmetic uses the 128-bit modulus. Ratios are candidate time / the stated current baseline; lower is faster. Intervals are pointwise 95% CIs for the median ratio. The gate also checks P95, every process median and allocations.

| Operation / candidate | Current baseline | Baseline → candidate (µs/pass) | Time ratio [95% CI] | Gate status |
|---|---|---:|---:|---|
| GF dot: shared wide ×1 | `f2z_wide` | 0.696 → 0.696 | 1.0001 [0.9997, 1.0013] | pass |
| GF independent products: current shared | `flock` | 0.859 → 0.913 | 1.0629 [1.0618, 1.0632] | regression |
| GF independent products: adapted scalar lanes | `flock` | 0.859 → 0.859 | 0.9995 [0.9989, 1.0000] | pass |
| Prime products: transparent brand | `raw_ctx` | 4.827 → 4.827 | 1.0000 [0.9993, 1.0006] | inconclusive |
| Prime products: configured elements | `raw_ctx` | 4.827 → 7.747 | 1.6046 [1.6018, 1.6074] | regression |
| Prime product MAC: four accumulators | `existing_delayed` | 2.390 → 2.362 | 0.9877 [0.9861, 0.9887] | pass |
| Prime native MAC: four accumulators | `existing_delayed` | 1.487 → 1.325 | 0.8895 [0.8885, 0.8933] | pass |
| Prime product MAC: per-term checks | `existing_delayed` | 2.390 → 2.916 | 1.2191 [1.2170, 1.2227] | regression |
| Prime product MAC: two temporary copies | `existing_delayed` | 2.390 → 2.895 | 1.2115 [1.2084, 1.2153] | regression |
| Product reduction only: crypto-bigint reference | `existing_optimized` | 0.016 → 0.105 | 6.6085 [6.6045, 8.7248] | regression |
| Integer MAC: fused, nine limbs | `circuit_z` | 21.263 → 17.197 | 0.8097 [0.8076, 0.8100] | pass |
| Integer MAC: fused, two limbs | `circuit_z` | 0.675 → 1.581 | 2.3443 [2.3375, 2.3814] | regression |
| P-256 product: public four-limb bound | `existing_p256` | 21.045 → 6.625 | 0.3148 [0.3139, 0.3155] | pass |
| P-256 product: always nine limbs, four active | `existing_p256` | 21.045 → 64.554 | 3.0749 [2.9361, 3.1025] | regression |
| Signed projection: fixed crypto-bigint, 128-bit modulus | `runtime_modulus` | 231.477 → 149.957 | 0.6482 [0.6420, 0.6532] | pass |
| Signed projection: fixed crypto-bigint, 100-bit modulus | `runtime_modulus` | 3296.688 → 154.250 | 0.0468 [0.0466, 0.0470] | pass |

## Implications for the library plan

- **Keep the current fast baselines.** Shared GF dot matches existing F2Z delayed accumulation. Comparing only with eager Flock would attribute an existing optimization to the new library. The current shared scalar GF multiply is about 6.3% slower at 1,024 elements; the adapted Flock scalar-lane graph matches Flock there. Its largest independent-product case remains inconclusive.
- **Choose MAC kernels by operation and public width.** Four accumulators save about 11% for the measured native prime MAC, but only about 1.2% for field-product MAC at 1,024 terms. Four-way product accumulation regresses roughly 4–5% at 16 terms. A single universal accumulator count is not justified.
- **Fusion is width-sensitive.** The fused nine-limb integer MAC saves about 19% at 1,024 terms, while the two-limb version takes 2.34× as long and the four-limb version about 1.33×. Keep dedicated small-width schedules. “Fused” alone is not a performance guarantee.
- **Preserve public width specialization.** The public four-limb P-256 product takes about 31.5% of the existing helper time on four-active-limb inputs. Always computing nine limbs takes 3.07× the existing time on that same workload. Even nine-active-limb inputs regress with this fixed-nine prototype. This measures helper arithmetic/codegen, not complete ECDSA throughput.
- **Keep checks and materialization visible.** Per-term canonicality checks add about 22% to the 128-bit product MAC at 1,024 terms. Two temporary vector copies add about 21% and two allocations. The transparent brand has a 1.0000 median ratio, but its retry P95 upper bound is 1.0126, so it correctly remains inconclusive under the 1% rule.
- **Preserve existing fixed-scalar kernels.** The generic zero/half/full specialization candidate loses against the existing prepared formula for the measured half-width fixed multiplies and butterflies. Passing zero-scalar cases does not establish general parity.
- **Projection deserves a focused follow-up.** The fixed-storage crypto-bigint control improves these synthetic signed-coefficient cases, especially the current 100-bit fallback. It is explicitly variable-time and retains an external arithmetic dependency; these measurements are a comparison target for an owned reducer, not an implementation of that reducer.

## Generated-code evidence

- Raw prime products and branded products both compile to 89 instructions / 356 bytes with the same multiplication/reduction instruction counts. Per-term validation expands the function to 103 instructions and adds two in-loop range-check branches. [Raw](assembly/prime-products-unchecked.s), [branded](assembly/prime-branded.s), [checked](assembly/prime-products-checked.s).
- The two-limb baseline forms the product cross terms before adding the accumulator. The fused candidate chains two multiply-adds through the running high word. That longer loop-carried dependency is consistent with the measured slowdown despite fewer loop instructions. [Baseline](assembly/integer-existing2.s), [fused](assembly/integer-fused2.s).
- The public four-limb P-256 specialization unrolls the limb products inside the batch loop. The nine-limb version retains a limb loop; the baseline retains its significant-length helper. These caller/codegen effects are included in the helper measurements. [Four limbs](assembly/p256-bound4.s), [nine limbs](assembly/p256-fixed9.s), [baseline helper](assembly/p256-kernel.s).
- F2Z and shared one-accumulator GF dots each compile to 46 instructions with the same static PMULL instruction count. [F2Z](assembly/gf-f2z-dot.s), [shared](assembly/gf-shared-dot.s).

## Coverage and reproduction

`will` timed out over SSH; **x86 remains unmeasured**. No ARM speed estimate is used for x86. Complete NTTs, fused sumcheck consumers, full provers, constant-time certification and peak RSS are outside this arithmetic campaign. The fixed-scalar baseline uses the existing formula adaptation rather than calling the private `FixedGfMul` API. Integer timing inputs are bounded signed 16-bit values stored at each tested capacity; P-256 and projection inputs exercise wider limbs. See [the suite contract](../../ARITHMETIC.md) for details.

Validation: the full arithmetic correctness run passed in all ten processes; all 16 reporting-policy tests passed; `cargo check --offline --features arithmetic-campaign,legacy-matrix` passed. The final gate exits 1 because the measured candidate set contains regressions and inconclusive cases, as recorded in [gate.txt](gate.txt).

- [All 408 variant results](measurements.md)
- [Machine-readable summary](summary.json) and [frozen metadata](metadata.json)
- [Frozen source and ARM binary archive](frozen-inputs.tar.gz)
- [Host observation and x86 connection result](host-observation.json)
- Raw initial CSV/logs: `initial/run-{0..4}.{csv,log}`; retry CSV/logs: `retry/run-{0..4}.{csv,log}`.

```sh
python3 experiments/field-regressions/run.py --suite arithmetic \
  --out experiments/field-regressions/results/arithmetic-arm-NEW \
  --target-dir /tmp/f2z-arithmetic-target
python3 experiments/field-regressions/gate.py \
  experiments/field-regressions/results/arithmetic-arm-NEW
```
