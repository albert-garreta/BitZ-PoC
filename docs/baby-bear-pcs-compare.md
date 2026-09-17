# Archived BabyBear prescribed-point PCS comparison

This diagnostic experiment is excluded from the paper benchmark suite. It
measures terminal PCS claims and does not prove the multiplication constraints.
Use the [native full-proving comparison](../README.md#integer-multiplication) for u32 and
BabyBear comparisons in the paper. The details below document the historical
PCS experiment.

This benchmark compares the BitZ opening path with Plonky3 WHIR on the same
deterministically generated logical BabyBear multiplication witness. The
default campaign requests `2^15` through `2^24` multiplication slots, uses one
warmup followed by 21 measured trials per runnable backend/shape cell, and runs
Rayon with 10 threads. BitZ runs at every default size. Under the frozen WHIR
configuration, WHIR uses a degree-5 BabyBear extension and a 4-bit per-round
proof-of-work cap, allowing the complete requested size sweep to run.

Run it from the repository root:

```sh
scripts/run_baby_bear_pcs_compare.sh
```

The runner defaults `RUSTFLAGS` to `-Ctarget-cpu=native`. Existing environment
overrides are retained, including `RUSTFLAGS`, `RAYON_NUM_THREADS`,
`BITZ_BENCH_SHAPES`, `BITZ_BENCH_REPS`, `BITZ_BENCH_SEED`, and
`BITZ_PCS_COMPARE_BACKENDS`. `BITZ_PCS_COMPARE_WHIR_DEGREE=4` selects the
degree-4 unique-decoding profile; the default value `5` selects the
degree-5 Johnson-bound profile. For example, a short diagnostic run is:

```sh
BITZ_BENCH_SHAPES="15 16" BITZ_BENCH_REPS=3 \
RAYON_NUM_THREADS=8 scripts/run_baby_bear_pcs_compare.sh
```

The corresponding degree-4 run is:

```sh
BITZ_BENCH_SHAPES="15 16" BITZ_BENCH_REPS=3 \
BITZ_PCS_COMPARE_WHIR_DEGREE=4 RAYON_NUM_THREADS=8 \
scripts/run_baby_bear_pcs_compare.sh
```

Every invocation reserves a new
`PerfRuns/<UTC>-baby-bear-bitz-vs-whir/` directory and refuses to overwrite an
existing directory or trace. `BITZ_PCS_COMPARE_TRACE_PATH` may override the
default trace location, and `BITZ_PCS_COMPARE_CAMPAIGN_PATH` may override the
campaign-manifest location. Relative paths are resolved from the repository
root. `BITZ_PCS_COMPARE_CAMPAIGN_ID` may supply an explicit identifier;
otherwise the timestamped run name is used. The identifier is embedded in
every run and series ID so separately generated traces can be merged safely.

## Campaign manifest contract

The benchmark writes one JSON object with this compact contract:

```json
{
  "schema": "baby-bear-pcs-compare-campaign/v2",
  "campaign_id": "2026-09-03T12-34-56Z",
  "cells": [
    {
      "implementation": "plonky3-whir",
      "log_multiplications": 24,
      "status": "measured",
      "challenge_extension_degree": 5,
      "configured_max_pow_bits": 4,
      "derived_max_pow_bits": 4
    }
  ]
}
```

`cells` contains exactly one entry for every combination of implementation
(`bitz` or `plonky3-whir`) and exponent (`15` through `24`). `status` is exactly
one of `measured`, `unavailable`, or `not_requested`. `reason` is an optional
non-empty string; `required_pow_bits` and `budget` are optional non-negative
integers. Every WHIR cell records `challenge_extension_degree` and
`configured_max_pow_bits`; measured WHIR cells also record the shape-derived
`derived_max_pow_bits`. A `measured` cell must have a corresponding trace series, while an
`unavailable` or `not_requested` cell must not. The comparison reporter checks
the complete matrix and this trace consistency before writing outputs. The
canonical trace remains limited to real warmup and measured trials.

## What is shared, and what differs

Both backends receive the same canonical integer columns `A`, `B`, `C`, and
`K`, generated once from the same shape seed, with every row satisfying the exact
integer identity

```text
A * B = C + p * K,  where p = 2^31 - 2^27 + 1.
```

They represent the same assignment MLE
`f = [e0 | A | B | C | K | 0 | 0 | 0]` and authenticate the same prescribed
terminal relation `D * f(x, beta) = V`. Public setup and logical witness
generation are excluded from measured PCS work. The logical witness is reused
between the two backend runs; each backend performs its own excluded setup.

The encodings are intentionally backend-native. BitZ commits the 31-bit
decompositions of the four columns in its binary/GF(2^128) machinery and opens
the integer MLE modulo `q = 2^100 - 15`. WHIR commits four native
BabyBear columns and uses the degree-5 BabyBear binomial extension for
challenges. Its four columns add two selector variables to the gate domain;
the adapter reverses the low-coordinate-first gate point once at the Plonky3
boundary because Plonky3 uses lexicographic big-endian point order. Therefore
commitments, proof encodings, and backend-native challenge values are not
expected to be byte-identical; the fair comparison is the same logical
witness and prescribed opening obligation.

## Measured phase boundary

The report keeps the following boundaries explicit:

- `materialize`: BitZ bit packing or WHIR native-column construction.
- `commit`: commitment generation and protocol-native public transcript/root
  binding from the materialized witness.
- `claim setup`: prescribed-point sampling, terminal-value derivation, and
  transcript binding. This is reported separately and excluded from total PCS
  prover time because it models a terminal claim already supplied by the
  relation protocol.
- `opening`: the backend opening proof. WHIR's `open_at` computes and includes
  the four claimed column evaluations; BitZ includes its claim bridge, weights,
  and opening proof work.
- `verify`: opening verification and enforcement of linkage to
  `D * f(x, beta) = V`.

Total PCS prover time is the overlap-safe union of `materialize`, `commit`, and
`opening`; it is not a sum of independently rounded medians. Verification is
separate.

BitZ exposes its existing nested profiler scopes beneath these five comparison
rows, while the WHIR adapter currently exposes only the five coarse comparison
rows. That produces a tiny asymmetric instrumentation overhead and means the
WHIR report is not an internal-stage timeline; the top-level phase boundaries
remain directly comparable.

## Security and byte accounting

The default target is 100-bit security. BitZ uses the fixed 100-bit modulus
`q = 2^100 - 15` and its GF(2^128)-based commitment/opening stack. The WHIR
configuration is non-hiding,
uses a constant folding factor of 4, starting log inverse rate 1, the Johnson
bound, and a configured maximum of 4 proof-of-work bits per round. The
degree-5 challenge field is about 155 bits wide, so WHIR can trade additional
queries for low grinding while retaining the 100-bit target. Preflight derives
at most 4 PoW bits for each default shape from `2^15` through `2^24`.

“Unavailable” means unavailable under this frozen configuration, not
unsupported by WHIR in general. The runner does not change the extension
degree or PoW cap by size, because doing so would silently change both the
protocol configuration and measured work. The canonical trace records the
challenge extension degree, shape-derived OOD samples, query counts, and
actual grinding bits for instantiated cells. The campaign manifest records
the extension degree, configured cap, and derived maximum PoW for every
measured WHIR cell. The comparison JSON, CSV, Markdown, and HTML preserve
those metrics and reject mixed-degree aggregation. BitZ records its
instantiated security profile and backend parameters. Results must be
interpreted at the recorded configuration rather than from the target label
alone.

Wire size is reported as initial commitment bytes plus public terminal-claim
bytes plus opening-proof bytes. WHIR proof serialization uses `postcard` and
includes the four claimed evaluations; its initial commitment is serialized
and counted separately. BitZ uses its native proof codec. This is wire-oriented
accounting, not an in-memory object-size comparison.

## Outputs

The timestamped run directory contains:

- `logs/cargo-bench.log`: complete build and benchmark output.
- `metadata/campaign.json`: the complete backend/size preflight matrix. Each
  cell is `measured`, `unavailable`, or `not_requested`; unavailable cells may
  include a reason, required PoW bits, and the configured budget. WHIR cells
  also record extension degree, configured PoW cap, and derived maximum PoW.
- `traces/baby-bear-pcs-compare.jsonl`: canonical interval and artifact trace.
- `reports/canonical/`: profiler `summary.json`, `metrics.csv`, and
  `intervals.html`.
- `reports/comparison/`: comparison `summary.json`, `metrics.csv`,
  `comparison.md`, and `comparison.html`.

The JSONL trace contains only trials that actually ran; it never fabricates
spans for unavailable cells. The comparison report renders manifest-declared
unavailable cells as `N/A (unavailable)`, unselected cells as `not requested`,
and unexpected absent trace data as `missing`.

The runner validates the JSONL trace before generating either report. Git
revision, dirty state, CPU, build profile, thread count, seed, and benchmark
configuration are supplied to the trace metadata; explicit
`BITZ_PCS_COMPARE_*` metadata overrides are preserved.
