# Unified arithmetic benchmark campaign

This suite implements the isolated arithmetic experiments for the proposed F2Z/circuit/Flock library. It does not migrate production arithmetic or remove production dependencies. The original GF/NTT suite and its recorded results remain available.

Measured results: [first ARM campaign](results/arithmetic-arm-01/REPORT.md), including raw data, source/binary archive and generated-code evidence.

Run from the repository root:

```sh
python3 experiments/field-regressions/run.py --suite arithmetic \
  --out experiments/field-regressions/results/arithmetic-arm-NEW \
  --target-dir /tmp/f2z-arithmetic-target
python3 experiments/field-regressions/gate.py \
  experiments/field-regressions/results/arithmetic-arm-NEW
python3 -m unittest discover -s experiments/field-regressions -p 'test_*.py'
```

Use a new output directory for every run. On an x86 host use the same command with `--arch x86_64` and a separate output directory. Results apply only to the architecture recorded in that run's frozen manifest; an ARM result does not establish x86 coverage. The x86 manifest requires PCLMUL. The existing shared field still takes its portable x86 path, which is intentionally exposed by that comparison.

## Comparisons

| Family | Primary baseline | Other variants / question |
|---|---|---|
| GF independent products, dependent chains | Vendored Flock `F128` | Shared crate, adapted Flock scalar-lane graph, ARM schoolbook/Karatsuba, unrolling |
| GF dot | F2Z `BinaryFieldGF128: WideMulAcc` | Shared delayed accumulators 1/2/4/8; eager Flock is a secondary control |
| GF square, inverse | Vendored Flock | Shared field, including their different zero-inversion conventions in correctness checks |
| GF fixed scalar / arithmetic butterfly | Existing `Prepared` adaptation of F2Z's fixed-scalar formula | Flock, shared, scalar-lane, zero/half/full specialized arithmetic; setup separate |
| Prime scalar products | Actual `RawMontyCtx::mul`, raw `u128` arrays | Transparent invariant brand, per-term canonicality checks, configured `MontyField<2>` arrays |
| Prime product MAC (`R²`) | `MontyProductAccumulator128` + optimized reducer, one accumulator | 2/4/8 accumulators, checks, eager raw products, crypto-bigint reduction, two temporary vector copies |
| Prime native MAC (`R`) | `MontyLinearAccumulator128` + optimized reducer, one accumulator | 2/4/8 accumulators and crypto-bigint reduction |
| Isolated prime reduction | Existing optimized reducer on a prepared 1024-term accumulator | Production crypto-bigint reference reducer, with the same raw residue output |
| Integer MAC | Actual `circuit::witgen::Z<L>` multiplication and addition | Fused fixed-width MAC, with and without per-term fixture-bound checks; L = 1/2/4/9 |
| P-256 wide products | Actual private `multiply_wide`, extracted verbatim | Fixed 9×9 and a public 4×4 width bound, both returning 18 limbs |
| Signed coefficient projection | Actual `RuntimeModulus<2>::reduce` | Fixed-storage crypto-bigint variable-time division; comparison only, not a proposed dependency migration |

`build.rs` extracts the arithmetic-only `RawMontyCtx` section and P-256 multiply function verbatim from the frozen production sources. It adds imports/type scaffolding and a P-256 call adapter; it does not rewrite their arithmetic. The delayed reducer and projection modules are compiled directly via source paths. GF and integer baselines link the actual F2Z/circuit crates. No production visibility changes are needed.

The fixed-scalar baseline is the pre-existing experiment's formula adaptation, **not** a call to private `FixedGfMul`. Those results test the arithmetic formula, not the private API's environment dispatch or a full NTT consumer. All other private baseline extraction boundaries fail the build if their source markers disappear.

## Measurement contract

- The runner freezes sources, lockfile, manifest, compiler flags, binary hash, CPU features and host information before measurement. Source and measurement hashes accompany the report. The binary and source snapshot paths are recorded in metadata.
- Native release, fat LTO, one codegen unit, one Rayon thread. Five fresh processes, different deterministic seeds, 32 samples per variant, shuffled order within every paired sample. Inputs/outputs are opaque to the optimizer at each complete pass. There is no function-pointer dispatch per arithmetic term.
- Calibrate iterations using each family's declared primary baseline. All variants in a paired case use the same iteration count. Buffers are allocated outside timing, except the explicit copy/materialization control. Context preparation is timed separately; no prime sampling or Miller–Rabin test is introduced.
- GF and prime arrays include tiny, cache-sized and large working sets; integer MACs include up to 65,536 terms and P-256 products up to 65,536 pairs. Projection uses 16 and 1,024 coefficients with 2/4/9-limb signed values and 100-/128-bit odd moduli. These projection sizes do not establish streaming-scale behavior.
- The 100-bit odd modulus and 128-bit modulus are representative modular-arithmetic fixtures. The harness needs oddness, not a primality claim. It does not benchmark primality testing. Setup includes Montgomery/Barrett context construction.
- The allocation audit runs outside timing and records the maximum of three warmed calls. Payload bytes are a declared live input/output estimate, not peak RSS; different variants' buffers can coexist. Whole-array copies are measured explicitly, not attributed to the transparent brand experiment.
- The brand experiment tests scalar layout and generated arithmetic only. It does not implement or prove the proposed generative context API, context-mixing safety, or library-wide zero-copy integration.

## Correctness and performance decisions

Every fresh process runs correctness checks before emitting completion. GF uses the independent bit-serial oracle, basis products, random products, square/inverse/codec checks and empty/ragged dot tails. Prime scalar tests compare against BigUint, including modulus boundaries; product and native accumulators are checked separately for `R²` and `R` scaling at 65/100/127/128-bit moduli and carry-heavy inputs. All timed implementations are compared with their baseline results. Integer MAC tests include full-width wrapping BigUint oracles plus exact signed sums for bounded timing inputs. P-256 products check independent full-width BigUint products and candidate/baseline equality. Projection checks positive and negative values, zero, signed extrema and independent modular arithmetic.

Timing inputs for integer MAC are signed 16-bit values, with at most 2^20 terms, so the exact signed sum fits every measured width. Their kernels retain the existing modulo-2^(64L) contract. The checked control validates this fixture contract per term; it is not a general checked integer implementation.

The fixed 9×9 P-256 kernel has a fixed loop schedule; the legacy kernel scans significant limbs. The public 4-limb specialization is valid only when that bound is established beforehand. Projection's reference uses explicitly variable-time division. These comparisons do **not** certify constant-time behavior or justify using a secret-dependent width choice in a replacement library.

The regression margin is 1%. Reports include median and P95 ratios and pointwise 95% hierarchical paired bootstrap intervals. P95 concerns averages of timed batches, not individual-call tail latency. A candidate passes only if both upper bounds and every process median are within the margin, with no allocation increase. A clear slowdown is a regression; uncertainty is inconclusive. Setup-only rows are diagnostics. One predeclared retry doubles samples for inconclusive selected cases; its result replaces the original regardless of direction. There is no retry-until-pass policy.

The manifest freezes a representative candidate per family before measurement; a gate failure is a valid experimental outcome. Comparison-only controls are not production recommendations. A favorable microbenchmark cannot replace later fused-sumcheck, NTT, protocol-correctness, memory and full-prover validation.
