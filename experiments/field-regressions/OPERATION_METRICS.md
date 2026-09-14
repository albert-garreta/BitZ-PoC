# Operation metrics for the unified arithmetic library

This suite fills measurement gaps left by the [arithmetic campaign](ARITHMETIC.md) and [integer optimization campaign](INTEGER_FOCUS.md). It measures existing implementations and the available GF128 candidates. A baseline-only measurement is a cost to compare against when the proposed replacement exists; it is not a measured speedup for that replacement.

Results and a consolidated view of the earlier campaigns: [ARM operation metrics](results/operation-metrics-arm-01/REPORT.md).

The next candidate comparisons use the [source-linked baseline catalogue](BASELINES.md), which identifies existing implementations, stronger controls and measurements still needed.

## Reproduction

From the repository root, using a fresh output directory:

```sh
python3 experiments/field-regressions/run.py --suite operation-metrics \
  --seed-offset 5772157 \
  --out experiments/field-regressions/results/operation-metrics-arm-NEW \
  --target-dir /tmp/f2z-arithmetic-target
python3 experiments/field-regressions/gate.py \
  experiments/field-regressions/results/operation-metrics-arm-NEW
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

The current optimization phase covers ARM/Apple Silicon and portable candidates only. x86 kernel development and evaluation are deferred. Existing x86 manifest entries remain available for a future separately authorized campaign; they do not represent current measurements. Runtime feature records accompany every run.

## New coverage

| Operations | Implementations and scope |
|---|---|
| GF128 add, multiply, square, inverse | Direct F2Z baseline; vendored Flock and shared field; ARM scalar-lane multiplication candidate. Native storage is prepared before timing. These are loops of independent scalar operations. |
| GF8 add, multiply, inverse | Existing Flock baseline. |
| B127 add, multiply, square, inverse | Existing F2Z baseline. |
| Fused GF128 sumcheck round | Actual `WideMulAcc::eqf_single_pair_round`; `n` weighted pairs consume `2n` left and right values and `n` weights. |
| Wrapping integer add/subtract | Existing `circuit::witgen::Z<L>`, widths 2, 4, 9 limbs. |
| Checked signed/unsigned add/subtract/multiply | Existing crypto-primitives wrappers and their actual `Option` results, widths 2, 4, 9. |
| Unsigned comparison and exact widened multiplication | Existing unsigned comparison and crypto-bigint `widening_mul`; widened results contain all `2L` limbs. |
| Unsigned quotient and remainder | Existing crypto-bigint `div_rem` with a public, prevalidated nonzero divisor, both 64-bit and full-width shapes. This is not a new reciprocal-based prepared-divisor implementation. |
| Prime context setup | Existing Montgomery constants plus `RawMontyCtx`, known prime `2^127-1`; no prime selection or primality testing. |
| Prime inverse/power | Existing `MontyField<2>`; individual variable-time inversions and bounded exponentiation. Exponents: 65537/17 bits and alternating bits/127 bits. |
| Prime canonical encode/decode | Existing element-conversion components into preallocated buffers; excludes allocating wrappers and protocol framing. Decode uses public valid fixtures and retains canonical range checks. |

Each array family uses 16 and 1,024 elements; context setup is one call. There are 125 required variant cases on ARM and 123 on x86. Eight GF128 candidate cases are selected for the existing 1% gate. Other families are baseline-only and intentionally have no selected candidate. Baseline rows compare with themselves in the generic statistics output; their ratio of 1 is not evidence about an unimplemented replacement.

GF additions are XOR; available F2Z/shared subtraction APIs are checked for equivalence outside timing. GF128 zero-inversion conventions differ between implementations, so timings use the common nonzero domain. The new prime inverse loop is not Montgomery batch inversion.

## Validation and measurement

Every process checks results before reporting completion. GF128 uses independent bit-serial multiplication, basis/boundary inputs and inverse identities; GF8 multiplication is checked exhaustively; B127 uses an independent polynomial oracle and inverse exponentiation. Integer checks use BigUint/BigInt, cover mixed valid and overflowing operations and verify both quotient and remainder. Prime checks use BigUint modular exponentiation and compare codecs against existing Spartan encoding, including invalid canonical values.

Five fresh processes use 32 shuffled paired samples per case, one caller thread, native release optimization, fat LTO and one codegen unit. The runner freezes sources, the required-case manifest, dependencies, flags and executable before timing. It retries only inconclusive selected cases once, using five processes with 64 samples. This policy is unchanged from earlier campaigns. See the arithmetic campaign for the confidence-interval and allocation-audit definitions.

No allocation or layout conversion is hidden inside the scalar comparison loops. Timed output buffers are preallocated and fully overwritten. Checked integers intentionally include the existing checked-result representation and branches. These measurements do not certify constant-time behavior, or predict the cost of replacing variable-time operations with fixed-schedule operations.

The complete unified API, context branding, mixed signed coefficient accumulators, constant-time implementations, batch inversion, mutable fused folds and full protocol integration still need their own implementation and measurements. Existing arithmetic/NTT experiments remain relevant for the kernels they actually compare. No full-prover or hybrid speedup follows from adding these microbenchmarks.
