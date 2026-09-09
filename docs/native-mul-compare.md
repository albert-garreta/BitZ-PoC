# Native end-to-end multiplication comparison

Run the comparison through `scripts/run_native_mul_compare.sh`. For the u32
Limber entry, the runner invokes the authors' `int_mult` example in a separate
Limber checkout. F2Z, Binius64 and Plonky3-WHIR continue to use the integrated
`mul_e2e_compare` benchmark. The BabyBear Limber entry uses the existing
in-process BabyBear adapter. The integrated harness also supports independent
u64 × u64 → u128 and u128 × u128 → u256 products with F2Z and Binius64.

## Limber's author-provided entry point

Set `LIMBER_REPO` to the Limber repository containing
`examples/int_mult.rs` (introduced upstream in commit
`47b10aa9ccfc993bcc6a2ea2c5f0e4b9f003822b`). If unset, the runner looks for a
sibling `limber-impl` checkout. It does not modify or patch that repository.

For each selected exponent, every warmup and measured sample runs this command
with the Limber repository as its working directory:

```sh
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
cargo +1.97.1 run --release --example int_mult -- --bits 32 --log-gates 15
```

Only `--log-gates` varies with `F2Z_BENCH_SHAPES`. The command uses the example's
default `k` and native security parameters; it does not inject our adapter's
`IntEvalParams::derive(64, 32, 9, ...)`. Rust 1.97.1 must be installed.
The author command requires eight threads. Encoded Cargo rustflags and `DUMP`
are removed from its environment so they cannot override these flags or add
proof-dump I/O.

The example proves a **wired chain modulo 2^32**:
`c_i = a_i * b_i mod 2^32`, `a_(i+1) = c_i`. At `--log-gates L`, it has
**2^L - 1 gates**, 2^L constraints and 2^(L+1) witness variables. It generates
its own deterministic witness. The other u32 backends currently prove
**2^L independent u32 × u32 → u64 products**. These are different workloads:
the runner records the author's results as `u32-mod32-chain`, with
`shared_corpus=false`; it does not claim witness equivalence with the other
u32 rows.

## Commands

Run from the BitZ/F2Z repository root:

```sh
LIMBER_REPO="$HOME/code/limber-impl" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="15 16 17 18 19 20" \
F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS="u32" \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir limber" \
bash scripts/run_native_mul_compare.sh
```

For only the authors' Limber run:

```sh
LIMBER_REPO="$HOME/code/limber-impl" \
RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES=15 F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_WORKLOADS=u32 F2Z_MUL_COMPARE_BACKENDS=limber \
bash scripts/run_native_mul_compare.sh
```

Append `--dry-run` to inspect routing, the Limber working directory and its
exact commands without creating result files. One warmup precedes the selected
number of measured samples for each size. All jobs and samples run sequentially.

Select `F2Z_MUL_COMPARE_WORKLOADS=babybear` for the existing BabyBear comparison,
or `"u32 babybear"` for both workloads. The default is both workloads and all
four backends, at exponent 15. The Limber author example accepts exponents up
to 24; F2Z requires at least 15. A u32 sweep through 25 remains available when
Limber is excluded. u64 supports exponents through 24, and u128 through 23,
with only F2Z and Binius64 selected. Mixed workload selections use the smallest
applicable limit. The accepted ranges are input limits, not memory guarantees.

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

The direct Cargo benchmark runs the integrated backends:

```sh
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES=15 F2Z_BENCH_REPS=5 \
F2Z_MUL_COMPARE_BACKENDS="f2z binius64 plonky3-whir" \
cargo bench --bench mul_e2e_compare --features bench-internals,native-mul-compare
```

Selecting `u32` + `limber` directly in that benchmark is rejected with a pointer
to the shell runner. The old u32 adapter cannot silently supply proof timings.

## Outputs and timing boundaries

Each invocation creates `PerfRuns/<UTC>-native-mul/`, or the directory selected
by `F2Z_MUL_COMPARE_OUTPUT_DIR`. Existing directories are refused.
`campaign.json` records routing and completion/failure.

The author example writes results under `limber-int-mult/`:

- `manifest.json`: checkout/revision, dirty status, source and lockfile hashes,
  toolchain, environment and measurement boundary.
- One raw log per exponent and trial, preserving Cargo and example output.
- `samples.jsonl`: command, working directory, exact gate count, derived
  parameters, verification success and the author's reported metrics.
- `metrics.csv` and `summary.json`: warmup-excluded medians for setup, witness
  generation, commit+prove, verification and proof bytes.

The runner parses the example's internal timers, excluding Cargo/build/process
startup. The example prints timings to 0.1 ms. It reports commit+prove together;
separate commitment, PIOP and PCS-opening timings are unavailable. Its proof
size is the reported eval-argument bytes plus analytical sumcheck bytes, using
the exact byte counts rather than rounded KB. This is the authors' size
convention, not the integrated adapters' commitment-inclusive size convention.
Peak RSS is unavailable for this command and recorded as null. A nonzero exit,
missing verification line, mismatched shape or incomplete output fails the run.

The integrated backends retain `metrics.csv`, `summary.json`, `samples.jsonl`,
`memory.jsonl`, `trace.jsonl` and `cargo-bench.log` at the run root. If both
workloads include Limber, the BabyBear adapter has its own `babybear-limber/`
directory. A Limber-only u32 run has only the author-example results and the
campaign manifest. The profiler renders interval reports for integrated
results when `ZK_TRACE_SCRIPT` or the installed `zk_trace.py` is available.
The author command has no synthetic interval trace.

## Integrated backend measurement contract

The following applies to the integrated Rust harness, excluding the authors'
u32 Limber example.

For a fixed workload, seed and exponent, the integrated backends receive the
same immutable operand corpus. Root seeds, sampling, canonical assignments and
BLAKE3 witness digests match the PCS benchmarks. Every trial regenerates the
native witness, produces a full proof and verifies it. Random operand sampling
and public setup are excluded from prover timings.

F2Z uses Lambda100 and its transcript-sampled-prime Spartan/F2Z path. Binius64
uses its native circuit, ring switching/BaseFold and a 100-bit FRI query target.
Plonky3 uses its native AIR and degree-5 WHIR with Johnson-bound parameters.
The BabyBear Limber adapter uses integer Mod-R1CS with canonical operands and
remainders, IntEval/Hyrax and `log_t_f=64, log_t=32, k=9`. Security accounting is
specific to each implementation.

`F2Z_BINIUS_LOG_INV_RATE=<k>` selects Binius inverse rate `2^k` (default `k=1`).
The query count follows from the rate: 241, 148, 121, and 110 queries at
rates 1/2, 1/4, 1/8, and 1/16. Results record `log_inv_rate` and `fri_queries`;
the paper's u64 and u128 tables include both rate 1/2 and rate 1/8 runs.

| Metric | Measured work |
| --- | --- |
| `witness_ms` | Native witness generation; includes Binius internal packing |
| `commit_ms` | Initial commitment, including F2Z bit packing |
| `piop_ms` | Constraint proof/reduction before the PCS opening |
| `opening_ms` | PCS opening and required bridge/ring switching |
| `pcs_ms` | Union of commitment and opening intervals |
| `online_prover_ms` | Complete prover call after initial witness generation |
| `witness_to_proof_ms` | Witness generation through proof readiness |
| `verify_ms` | Complete verification |

F2Z prime projection is preparation. Its opening includes Steps 4 and 5.
Binius witness packing overlaps its online prover time; do not add those
columns. Timing totals use interval boundaries, not sums of phase medians.
Missing required instrumentation fails the integrated run.

Proof sizes include commitments. Binius and Plonky3 serialize their full
proofs. F2Z combines its root, fixed-width PIOP/nonces and serialized opening.
The BabyBear Limber adapter combines serialized commitments/openings and
analytical sumcheck payloads:
`scalar_bytes * (3 * log2(num_cons) + 2 * (log2(num_vars) + 1) + 6)`.

Peak RSS is measured by an additional verified proof in a fresh process per
case, including corpus generation and setup. Linux uses `/proc/self/status`
and macOS uses `getrusage`, normalized to bytes. Set
`F2Z_MUL_COMPARE_MEMORY=0` to skip this pass; missing memory is null, never zero.
This flag does not add a memory pass to the authors' Limber command.

## Witness-equivalence checks

`mul_witness_compare` remains a separate check of the integrated adapters and
the legacy u32 Limber assignment builder. It does not invoke or certify the
authors' chained `int_mult` workload.

```sh
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
F2Z_BENCH_SHAPES="10 15" F2Z_BENCH_REPS=5 \
cargo bench --bench mul_witness_compare --features bench-internals,native-mul-compare
```

It reconstructs canonical rows from each native witness and checks values,
counts and digests. Plonky3 BabyBear reconstructs the integer quotient from its
operands and remainder; the other adapters read their generated assignments.
Results are `witness-checks.jsonl` and `witness-summary.json`.

## Validation

```sh
python3 -m unittest discover -s scripts -p test_native_mul_runner.py
cargo test --release --test native_mul_compare --features bench-internals,native-mul-compare
```
