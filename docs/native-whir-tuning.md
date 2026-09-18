# Native AIR + WHIR benchmark runs

The optional `plonky3-whir` mod32 multiplication backend and fixed-IV SHA
comparison retain the multilinear AIR zerocheck/sumcheck PIOP. WHIR commits
Reed–Solomon codewords and proves the PIOP's prescribed multilinear evaluation
claims directly. The multiplication runner also retains `plonky3-fri` as a
default backend with its univariate STARK quotient PIOP and fixed FRI settings.
The two adapters share the same mod32 AIR and native witness generator, while
their PIOPs and security reports remain distinct. WHIR results explicitly
record both the encoding and opening claim. Paper files and historical
measurements are unchanged; old full-product u32 and BabyBear multiplication
results do not represent the new mod32 relation.

## Tuning and measurement

Every ordinary invocation tunes independently for each workload and input size.
SHA uses BabyBear extension degrees 4 and 5; mod32 multiplication uses
Goldilocks extension degrees 2 and 5. Both search constant folding parameters
2 and 4, starting log-inverse rates 1, 2, and 3, grinding caps 8 and 12, and
either the native round
rate schedule or a schedule capped at log-inverse rate 4. This is 48 candidates.
The rate cap prevents late rounds from accumulating excessive redundancy and
exceeding the grinding budget at larger sizes. The folding parameter
counts variables folded, so these are arities 4 and 16. Query, OOD, and grinding
schedules are derived by the pinned WHIR implementation for these rates.

Only security-eligible candidates run. One verified preliminary trial selects
up to four fastest candidates. Each finalist receives one warmup and five tuning
trials, and the lowest median wins. Ties use parameter ordering. Set
`--tuning-reps` for multiplication (`BITZ_WHIR_TUNING_REPS` for SHA) to change the finalist repetition count. The winner stays
fixed for that case's warmup and fresh measured repetitions. Every repetition
regenerates the witness and verifies its complete proof.

The objective and primary throughput denominator are `witness_to_proof_ms`:
start native witness generation and stop when the complete PCS proof is ready.
Public setup, tuning, serialization, and verification are excluded from this
interval. The existing online-prover and verification metrics remain separate.
The multiplication RSS child uses the same selected parameters and records
whole-case memory, including setup and verification.

Multiplication stores tuning evidence in its campaign manifest; SHA saves
`whir-<workload>-<exponent>.json`. Both include every candidate's
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

Multiplication uses the canonical Cargo benchmark flags:

```sh
cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  proof --workload u32-mod32 --backends plonky3-whir --log-n 15,16 --threads 8

cargo bench --bench mul_compare --features native-mul-compare,bench-internals -- \
  proof --workload u32-mod32 --backends plonky3-whir --log-n 15 --threads 8 \
  --whir-degree 5 --whir-folding 4 --whir-pow 12 --log-inv-rate 1 --memory rss
```

Without explicit WHIR parameters, the latency worker selects an eligible
configuration using verified tuning trials. `--tuning-reps` controls finalist
repetitions. The manifest stores the selection and tuning evidence; the RSS
worker replays the exact selection and never tunes. Heap runs require explicit
parameters and a separate instrumented binary. See the
[multiplication guide](native-mul-compare.md) for the current result format.

The independent SHA comparison retains its own experiment-specific interface:

```sh
RAYON_NUM_THREADS=8 BITZ_SHA_COMPARE_EXPONENTS=7 \
BITZ_SHA_COMPARE_BACKENDS=plonky3-whir bash scripts/run_native_sha256_compare.sh
```
