# Native AIR + WHIR benchmark runs

The native multiplication and fixed-IV SHA comparisons retain the multilinear
AIR zerocheck/sumcheck PIOP. WHIR commits Reed–Solomon codewords and proves the
PIOP's prescribed multilinear evaluation claims directly. There is no
`p3_uni_stark` quotient PIOP, Gemini reduction, or FRI substitution. Results keep
the `plonky3-whir` backend name and explicitly record both the encoding and the
opening claim. Historical PCS-only profiles and paper files are unchanged.

## Tuning and measurement

Every ordinary invocation tunes independently for each workload and input size.
BabyBear candidates use extension degrees 4 and 5; Goldilocks candidates use
degrees 2 and 5. Both search constant folding parameters 2 and 4, starting
log-inverse rates 1, 2, and 3, grinding caps 8 and 12, and either the native round
rate schedule or a schedule capped at log-inverse rate 4. This is 48 candidates.
The rate cap prevents late rounds from accumulating excessive redundancy and
exceeding the grinding budget at larger sizes. The folding parameter
counts variables folded, so these are arities 4 and 16. Query, OOD, and grinding
schedules are derived by the pinned WHIR implementation for these rates.

Only security-eligible candidates run. One verified preliminary trial selects
up to four fastest candidates. Each finalist receives one warmup and five tuning
trials, and the lowest median wins. Ties use parameter ordering. Set
`F2Z_WHIR_TUNING_REPS` to change the finalist repetition count. The winner stays
fixed for that case's warmup and fresh measured repetitions. Every repetition
regenerates the witness and verifies its complete proof.

The objective and primary throughput denominator are `witness_to_proof_ms`:
start native witness generation and stop when the complete PCS proof is ready.
Public setup, tuning, serialization, and verification are excluded from this
interval. The existing online-prover and verification metrics remain separate.
The multiplication RSS child uses the same selected parameters and records
whole-case memory, including setup and verification.

Each case saves `whir-<workload>-<exponent>.json`, including every candidate's
eligibility, preliminary/finalist timings, selected parameters, security report,
and total tuning time. A case without an eligible candidate saves its reason
instead of producing a measurement. Unexpected prover, verifier, or subprocess
failures stop execution with diagnostics; they are not reported as OOM results.

## Security accounting

`native-air-whir-johnson-union/v1` uses the pinned `p3_whir::SecurityAssumption`
Johnson-bound primitives and reports their evaluated terms for the actual
stacked commitment. It accounts for initial and intermediate OOD tests,
proximity queries, query combinations, folding, final sumcheck, and initial
claim batching. The single-AIR zerocheck contributes constraint batching,
the random equality point, and its degree-dependent sumcheck bound, with a
conservative allowance for the initial Johnson list. The commitment/transcript
hash bound caps the result. This model is specific to the supported AIRs:
one trace commitment, no preprocessed commitment, no lookups, and no next-row
openings. It is not a univariate-STARK AIR/DEEP bound.

For each candidate and workload size, the harness finds the lowest integer
round target from 100 through 116 whose evaluated combined report reaches
100 bits. This calculation precedes FFT allocation and proving. The selected
round target and complete schedule are recorded; merely requesting a target
never passes the gate. Replay derives the schedule again for the requested size.
Grinding is credited only at the corresponding site, using the pinned WHIR
work-factor accounting. The report records this model and its assumptions;
it is not an independent cryptographic audit or a guarantee under other
soundness or random-oracle models. Other backends keep their existing security
policies and labels, including Binius's query-phase-only guarantee.

## Running and replaying

Linux and macOS use the same runners. Choose a thread count explicitly when
comparing runs. `environment.json` records machine, compiler, flags, dependency
lock digest, and thread information. A missing CPU probe is `unknown` rather
than an invented machine label. External HTML rendering is optional through
`ZK_TRACE_SCRIPT`; the benchmarks themselves need no private profiler checkout.

```sh
RAYON_NUM_THREADS=8 F2Z_BENCH_SHAPES="15 16" \
F2Z_MUL_COMPARE_BACKENDS=plonky3-whir \
bash scripts/run_native_mul_compare.sh

RAYON_NUM_THREADS=8 F2Z_SHA_COMPARE_EXPONENTS="10 11" \
F2Z_SHA_COMPARE_BACKENDS=plonky3-whir \
bash scripts/run_native_sha256_compare.sh
```

Explicit replay bypasses tuning and rechecks security for the requested case:

```sh
F2Z_WHIR_CONFIG=/absolute/path/to/whir-u32-15.json \
F2Z_MUL_COMPARE_WORKLOADS=u32 F2Z_MUL_COMPARE_BACKENDS=plonky3-whir \
RAYON_NUM_THREADS=8 bash scripts/run_native_mul_compare.sh
```

`F2Z_WHIR_CONFIG` accepts either a saved tuning record or a JSON parameter object
with `extension_degree`, `folding`, `starting_log_inv_rate`, and `max_pow_bits`.
The optional `max_round_log_inv_rate` field selects the round-rate cap; omitting
it retains the native schedule.
SHA's existing explicit `F2Z_SHA_COMPARE_P3_*` overrides also bypass tuning;
the shared replay file takes precedence. Its legacy pilot switch controls the
other backends' pilot phases; ordinary WHIR runs still tune at each size.

Fresh output files are required. Compare the saved configurations and machines
alongside timings; separately tuned runs can choose different parameters.

## Validation

```sh
RUSTFLAGS=-Ctarget-cpu=native cargo test --release \
  --test native_mul_compare --test native_sha256_compare \
  --features bench-internals,native-mul-compare,native-sha256-compare
```

The release tests exercise real proof rejection for wrong multiplication
outputs and out-of-range operands, as well as altered WHIR/PIOP proof fields.
SHA tests prove and verify the full AIR and reject changed public inputs,
outputs, ordering, and malformed encodings. Tuner tests cover eligibility,
fresh invocation selection, and security checks against modified query counts.
