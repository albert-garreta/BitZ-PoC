# Native end-to-end multiplication comparison

`mul_e2e_compare` proves batches of u32 × u32 → u64, BabyBear,
u64 × u64 → u128, and u128 × u128 → u256 multiplications with F2Z, Binius64,
Plonky3-WHIR, and Limber-Hyrax (the u64 and u128 workloads run on F2Z and
Binius64 only, see below). Every
warmup and measured trial regenerates the native witness, produces the complete
proof, and verifies it. This is the full-proving multiplication comparison for
the paper benchmark suite; commitment, constraint proving, opening, and
verification are all exercised on every trial.

```sh
# Both workloads, all four backends, 2^15 multiplications, five samples
# plus one warmup and one isolated memory pass per workload/backend/size.
bash scripts/run_native_mul_compare.sh

# Select sizes, repetitions, workloads, and backends independently.
F2Z_BENCH_SHAPES="15 16" F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u32 babybear" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir limber" \
RAYON_NUM_THREADS=8 bash scripts/run_native_mul_compare.sh
```

The full sweeps use all four backends, including Limber-Hyrax, at exponents
15–24 for BabyBear and 15–25 for u32. Run each workload separately so its size
limit is respected:

BabyBear multiplication, exponents 15–24:

```sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20 21 22 23 24" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="babybear" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir limber" \
bash scripts/run_native_mul_compare.sh
```

u32 multiplication, exponents 15–25:

```sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20 21 22 23 24 25" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u32" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir limber" \
bash scripts/run_native_mul_compare.sh
```

Each case includes one warmup and five measured trials. Use 21 measured
trials for the final comparison. Accepted size ranges describe harness input
limits; completion at the largest sizes depends on the backend and available
memory. The native measurements recorded so far cover exponents 15–17.



u64 × u64 → u128 multiplication, exponents 15–24, F2Z and Binius64 only:

```sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20 21 22 23 24" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u64" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64" \
bash scripts/run_native_mul_compare.sh
```

The u64 workload draws random full 64-bit operands and proves the exact
128-bit product as two 64-bit limbs. F2Z uses the `u64_mul` relation:
one integer R1CS row `x · y = z_lo + 2^64 · z_hi` per multiplication with
the limb base as a public coefficient of matrix `C`, 256 committed bits per
multiplication (`2^(n+8)` bits, so its size limit is that of BabyBear), the
Spartan PIOP over the transcript-sampled prime with the exact `u64`
assignment entering the inner sumcheck natively and only the `2^128`-sized
products reduced into the field (as raw residues built from the witness
limbs, since they have no native `u64` first round), and the same Lambda100
profile and validated-UDR Ligerito opener as the u32 relation.
Binius64 asserts both words of its native `imul` against witness words and
needs no operand range checks. The Plonky3 adapter's AIR decomposes 32-bit
operands and the Limber program uses `u64` linear-combination coefficients,
so selecting either with the u64 workload is rejected at startup. F2Z's
`proof_bytes` are now recorded for every workload (Spartan payload as 16-byte
elements, nonces as 8-byte words, plus the F2Z opening's exact codec bytes).

u128 × u128 → u256 multiplication, exponents 15–23, F2Z and Binius64 only
(run the backends separately: on a 16 GB machine Binius64's bignum prover pages
from 2^18, so its clean sizes are 2^15–2^17, while F2Z runs to 2^21; the
paper's tables list Binius64 at both rate 1/2 (the default) and rate 1/8,
`F2Z_BINIUS_LOG_INV_RATE=3`, so run it once per rate):

```sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20 21" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u128" \
F2Z_MUL_COMPARE_BACKENDS="f2z" \
bash scripts/run_native_mul_compare.sh
RUSTFLAGS="-Ctarget-cpu=native" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u128" \
F2Z_MUL_COMPARE_BACKENDS="binius64" \
F2Z_BINIUS_LOG_INV_RATE=3 \
bash scripts/run_native_mul_compare.sh
```

The u128 workload draws random full 128-bit operands and proves the exact
256-bit product as two 128-bit halves. F2Z uses the `u128_mul` relation: one
integer R1CS row `x · y = z` per multiplication over the four-block assignment
`[1 | x | y | z]` (128-, 128-, and 256-bit entries; the block selector is two
Boolean coordinates, since a public coefficient as large as `2^128` could not
be modulus independent), 512 committed bits per multiplication (`2^(n+9)`
bits, so its size limit is one below u64's), the Spartan PIOP over the
transcript-sampled prime on raw residues built straight from the witness
limbs for both the products and the assignment (neither has a native `u64`
first round), and the same Lambda100 profile and validated-UDR Ligerito
opener as the narrower relations. Binius64 uses its bignum circuit: the
four native `imul` limb products of the two-limb operands, accumulated with
carry chains into the four witness limbs of the product, again without range
checks. The Plonky3 and Limber adapters reject the workload at startup as
they do for u64. The `mul_witness_compare` audit reconstructs the canonical
assignment from each backend's native 128-bit values and hashes every entry
as 32 little-endian bytes under its own domain.


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
constraint rows. Supported exponents are 4–23 for u128, 4–24 for BabyBear and
u64, and 4–25 for u32; selecting F2Z requires at least 15. A shape list shared
by several workloads must stay within the tightest of those ranges. Small exponents are useful for checking the other adapters. Native trace
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
prover time.

## Proof sizes and peak memory

Every backend reports a positive `proof_bytes` value, including the initial
commitment and the proof needed to verify it. Public setup parameters and the
public relation are excluded. Size accounting runs outside the prover and
verification timing intervals.

| Backend | Size encoding |
| --- | --- |
| F2Z | Commitment root, 16 bytes per Spartan payload field element, 8 bytes per transmitted nonce outside the opening, and the actual canonical F2Z opening bytes. Opening nonces are counted only inside that serialization. |
| Binius64 | Actual finalized native transcript bytes, including the commitment. |
| Plonky3-WHIR | Actual postcard serialization of the complete native proof, including the commitment. |
| Limber | Actual canonical commitment and batch-opening bytes (both W and Q), plus the fixed-width PIOP payload described below. |

F2Z and the pinned Limber dependency do not expose a complete-proof serializer.
Their sizes use component payload accounting, excluding any hypothetical outer
container framing. Limber's unsegmented PIOP has three field elements per outer
sumcheck round, two per inner round, five outer claims, and one witness
evaluation: `scalar_bytes * (3 * log2(num_cons) + 2 * (log2(num_vars) + 1) + 6)`.
The padded dimensions are public, and the scalar encoding is currently 16
bytes. The sampled modulus is derived from the transcript. This PIOP count is
derived from the pinned driver's shape, rather than a serialized whole proof.
The backend's `config.proof_size_encoding` records the accounting used.

Peak memory is enabled by default. Each workload/backend/size runs one
additional proof in a fresh child process, before the parent's backend setup.
The child uses the same corpus and Rayon thread count and must verify its proof.
`peak_rss_bytes` is the OS high-water resident set size for the whole child:
corpus generation, public setup, witness generation, commitment, proving,
verification, and proof-size accounting. It includes resident runtime and
library memory; it is not an allocator-only or prover-only measurement.
The child has no warmup, and its memory sample is separate from the latency
trials and their medians. Linux uses `VmHWM` from `/proc/self/status`; macOS
uses `getrusage(RUSAGE_SELF)`. Both are normalized to bytes. Linux's current
address-space counter avoids carrying a pre-exec peak into the new case.

Set `F2Z_MUL_COMPARE_MEMORY=0` for a run without the extra memory pass. Disabled
memory is `na` in stdout, blank in CSV, and `null` in the summary; it is never
reported as zero. A failed memory pass aborts the campaign.

## Native configurations

- F2Z: Lambda100, transcript-sampled prime, production Spartan reduction and
  F2Z/Ligerito opening, one-bit packing.
- Binius64: native IMUL and bit constraints, ring switching/BaseFold, inverse
  rate 2 and a 100-bit FRI query target. `F2Z_BINIUS_LOG_INV_RATE=<k>` selects
  inverse rate `2^k` instead (the query count follows from the rate: 241, 148,
  121, and 110 queries at rates 1/2, 1/4, 1/8, and 1/16); the run records
  `log_inv_rate` and `fri_queries` in its config, and the paper's u64 and
  u128 tables list both rate 1/2 and rate 1/8 rows. u32 operands are range checked; the
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
contains raw per-trial metrics including proof size, and `metrics.csv` plus
`summary.json` contain warmup-excluded medians including `proof_bytes`.
The CSV also has `peak_rss_bytes`; the summary has `peak_rss_bytes` and the full
`memory` record. `memory.jsonl` contains one isolated memory record per case,
including its corpus digest, verified proof size, and measurement boundary.

Both the shell runner and direct Cargo command print one
`RESULT schema=native-mul/2` line to stdout per completed case. It identifies
the workload, backend, size, and sample count, then reports `witness_ms`,
`online_prover_ms`, `verify_ms`, median `proof_bytes`, and the separate
`peak_rss_bytes` measurement. The runner also saves stdout and stderr in
`cargo-bench.log`.

When the zk-proof-profiler script is installed, the
runner validates traces and renders `reports/intervals.html`; set
`ZK_TRACE_SCRIPT` to override its location. A failed proof aborts the campaign; it is never emitted
as a successful sample.

```sh
cargo test --release --test native_mul_compare \
  --features bench-internals,native-mul-compare
```
