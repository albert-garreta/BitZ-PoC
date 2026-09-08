# Native end-to-end multiplication comparison

`mul_e2e_compare` proves batches of u32 × u32 → u64 and BabyBear
multiplications with F2Z, Binius64, Plonky3-WHIR, and Limber-Hyrax. Every
warmup and measured trial regenerates the native witness, produces the complete
proof, and verifies it. This complements the existing `u32_pcs_compare` and
`baby_bear_pcs_compare` terminal-opening benchmarks.

```sh
# Both workloads, all four backends, 2^15 multiplications, five samples
# plus one warmup per workload/backend/size.
bash scripts/run_native_mul_compare.sh

# Select sizes, repetitions, workloads, and backends independently.
F2Z_BENCH_SHAPES="15 16" F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u32 babybear" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir limber" \
RAYON_NUM_THREADS=8 bash scripts/run_native_mul_compare.sh
```

The full three-backend sweep uses exponents 15–24 for BabyBear and 15–25
for u32. Run each workload separately so its size limit is respected:

BabyBear multiplication, exponents 15–24:

```sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20 21 22 23 24" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="babybear" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir" \
bash scripts/run_native_mul_compare.sh
```

u32 multiplication, exponents 15–25:

```sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20 21 22 23 24 25" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u32" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir" \
bash scripts/run_native_mul_compare.sh
```

Each case includes one warmup and five measured trials. Use 21 measured
trials for the final comparison. Accepted size ranges describe harness input
limits; completion at the largest sizes depends on the backend and available
memory. The native measurements recorded so far cover exponents 15–17.

The direct Cargo command is:

```sh
RUSTFLAGS="-Ctarget-cpu=native" RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES=15 F2Z_BENCH_REPS=5 \
cargo bench --bench mul_e2e_compare --features bench-internals,native-mul-compare
```

Run one benchmark process at a time, after other builds and benchmark jobs
finish. The harness runs workloads, backends, warmups, and samples sequentially;
only the selected prover uses Rayon threads. Run `mul_witness_compare` separately
from `mul_e2e_compare` so they do not contend for CPU or memory bandwidth.

The size exponent is the number of logical multiplications, not native
constraint rows. Supported exponents are 4–24 for BabyBear and 4–25 for u32;
selecting F2Z requires at least 15. A shape list shared by both workloads must
stay within 4–24. Small exponents are useful for checking the other adapters. Native trace
widths differ substantially, particularly Limber's explicit input range checks;
large multiplication counts need correspondingly larger memory budgets.

## Reusing the existing witnesses

The root seeds, per-shape seed derivation, operand sampling, canonical F2Z
assignment types, and BLAKE3 witness-digest format match the existing PCS
benchmarks. `F2Z_BENCH_SEED` overrides the per-workload root seed in both
workflows. For a fixed workload, seed, and size, all backends receive one shared
immutable operand corpus. Their trace metadata records the same
`witness_digest_blake3` as the PCS baseline. Regression tests pin the 2^15
digests to the saved September 6 PCS campaign.

Each backend derives its auxiliary values from those operands on every trial.
F2Z builds its packed assignment, Binius evaluates its wire circuit, Plonky3
builds and transposes its AIR trace, and Limber builds its integer assignment
and quotient vectors. Generated products/remainders are checked against the
canonical integer computation.

These are native prover comparisons. The logical multiplication inputs match;
the constraint layouts, PIOPs, challenge fields, and encoded witnesses differ.
All witnesses are private; this benchmark proves satisfaction of the native
multiplication relation, not a public statement binding a published corpus.
The digest is benchmark provenance, not an extra cryptographic public input.

## Separate witness-equivalence benchmark

```sh
RUSTFLAGS="-Ctarget-cpu=native" RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="10 15" F2Z_BENCH_REPS=5 \
cargo bench --bench mul_witness_compare \
  --features bench-internals,native-mul-compare
```

This benchmark uses the same workload/backend/seed selectors. It builds each
native witness, reads its actual input and output values, reconstructs the
canonical assignment, compares every row with the reference, and hashes the
recovered assignment. A changed value, row count, or digest aborts the run.
The native full-proof harness also performs this audit before accepting timings.

The check is equality of canonical multiplication data, not equality of native
auxiliary-wire arrays or field encodings. In particular, native Plonky3
BabyBear has no integer quotient column: the audit explicitly marks that
quotient as reconstructed from its native inputs and remainder. F2Z, Binius,
and Limber read the quotient from their generated assignments. Binius uses
the same witness filler as its full prover; Limber also validates its exact
and modular rows before comparison.

It writes `witness-checks.jsonl` with per-trial digests, representations,
quotient provenance, and witness-generation times, plus
`witness-summary.json` with warmup-excluded medians. Its default size is 2^10,
with five measured trials and one warmup. Circuit compilation, recovery, and
equality checks are outside witness-generation timings. No PCS or PIOP is run
by this independent equivalence benchmark.

## Timing boundaries

Every sample records monotonic intervals, rather than adding phase medians:

| Metric | Measured work |
| --- | --- |
| `witness_ms` | Native witness generation; includes Binius's internal witness packing |
| `commit_ms` | Initial witness commitment (F2Z includes bit packing) |
| `piop_ms` | Actual constraint proof/reduction, before the PCS opening |
| `opening_ms` | PCS opening and required claim bridge/ring switching |
| `pcs_ms` | Union of initial commitment and opening intervals |
| `online_prover_ms` | Complete native prover call after initial witness generation |
| `witness_to_proof_ms` | Initial witness generation through proof readiness |
| `verify_ms` | Complete native verification |

F2Z uses its existing `step3:piop_prove` scope; prime sampling/projection is
reported separately as preparation. Its opening includes Steps 4 and 5.
Limber uses its `imod_modp_piop` scope; runtime-prime projection is separate.
Binius uses the interval between initial witness commitment and ring switching.
Plonky3 uses the interval between initial WHIR commitment and opening, covering
the AIR zerocheck/sumcheck reduction. Missing native phase instrumentation is
an error, never a fabricated zero or a PCS-only fallback.

Binius packs the witness inside its online prover, so `witness_ms` overlaps
`online_prover_ms`. Do not add those columns. `witness_to_proof_ms` measures
the combined interval directly. Public setup/circuit compilation and random
corpus sampling happen outside trials. Setup is recorded separately. Proof
serialization, when measured outside proof readiness, is not included in
prover time. Proof sizes are optional; no estimated Limber proof size is
reported as measured bytes.

## Native configurations

- F2Z: Lambda100, transcript-sampled prime, production Spartan reduction and
  F2Z/Ligerito opening, one-bit packing.
- Binius64: native IMUL and bit constraints, ring switching/BaseFold, inverse
  rate 2 and a 100-bit FRI query target. u32 operands are range checked; the
  BabyBear circuit constrains `a*b = p*q+c`, canonical operands/remainder, and
  nonoverflowing reconstruction.
- Plonky3: Goldilocks AIR with 32-bit input decompositions for u32; native
  BabyBear AIR for field multiplication. Both use a degree-5 challenge field,
  WHIR Johnson-bound parameters, folding 4, inverse rate 2, a 100-bit target,
  and a maximum of 12 PoW bits.
- Limber: integer Mod-R1CS with exact bit constraints for operand ranges,
  canonical BabyBear remainders, IntEval/Hyrax, `log_t_f=64`, `log_t=32`, `k=9`.
  Its native security policy is recorded separately; the harness does not
  assert all four systems have identical security guarantees.

## Outputs and verification

The runner reserves a new `PerfRuns/<UTC>-native-mul` directory, or uses
`F2Z_MUL_COMPARE_OUTPUT_DIR`; it refuses to overwrite existing results.
`trace.jsonl` contains canonical `zkperf.trace/v1` intervals, `samples.jsonl`
contains raw per-trial metrics, and `metrics.csv` plus `summary.json` contain
warmup-excluded medians. When the zk-proof-profiler script is installed, the
runner validates traces and renders `reports/intervals.html`; set
`ZK_TRACE_SCRIPT` to override its location. A failed proof aborts the campaign; it is never emitted
as a successful sample.

```sh
cargo test --release --test native_mul_compare \
  --features bench-internals,native-mul-compare
```
