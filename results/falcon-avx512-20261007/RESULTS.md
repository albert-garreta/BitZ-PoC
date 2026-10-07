# Paired AVX-512 Falcon weight generation

The shared field backend now generates two degree-11 power sequences together
using AVX-512. Both algebraic Falcon-512 and Falcon-1024 call the paired API.
The full Falcon frontend uses a different tensor lift and does not call this
recurrence, so this change makes no incremental performance claim for full Falcon.

The field implementation is in `vendor/field/src/q12289/avx512.rs`, with safe
dispatch in `PowerBasis::fill_powers_pair`. Builds targeting x86-64 with both
AVX512F and AVX512BW use the kernel for degree 11. Other builds and extension
degrees use the existing portable recurrence. Dispatch follows the repository's
compile-time target-feature convention; it is not runtime CPU dispatch.

Two independent vectors occupy lanes 0–10 and 16–26. Each iteration broadcasts
each sequence's top coefficient, shifts the coordinates with full-width
permutations, applies the feedback rule, and stores exactly eleven `u16` values
per output. Centered feedback and a rounded quotient allow exact reduction with
16-bit SIMD multiplication. The signed remainder is bounded by 8,448 in absolute
value; after adding the predecessor, two unsigned minima give canonical residues.
The code documents the bounds and masked-store safety requirements.

The coordinates match the old recurrence exactly. The source layout, transcript,
integer-carry interpretation, proof format, and security accounting are unchanged.
The benchmark checks identical input digests, protocol IDs, payload sizes and
breakdowns, source geometry, and security estimates. Serialized proof byte equality
was not separately measured.

## Measurement method

- Host: AMD Ryzen 9 9950X3D (`will`); Rust 1.98.1; release, fat LTO,
  one codegen unit, `RUSTFLAGS='-C target-cpu=native'`.
- Baseline: preserved binaries whose source hashes match commit `9903fdb6c`,
  before this SIMD change. Candidate: the same source with the paired field
  kernel and one shared algebraic ring call-site change.
- Batch: 1,024 distinct original-Falcon signatures; dimensions 512 and 1024;
  security targets 100 and 128; threads 1, 2, 4, 8, and 16.
- Each configuration runs baseline/candidate, then candidate/baseline. Each
  process performs one warm-up and five measured proofs. Tables report medians
  of ten measured proofs per implementation/configuration.
- Affinity, input seed, cached native inputs, and auxiliary pool placement match
  between binaries. Main thread and auxiliary workers use CPU 0; Rayon workers
  use CPUs 0 through threads−1. Compilation and other benchmark jobs are idle.
- Total prover includes witness derivation, packing, commitment, and proving.
  Native input generation and external hashing are excluded from both binaries.
  Traced coordinate-stage runs are separate from end-to-end timings.

## Validation

Generic and native field tests pass, including degree-9/10 fallback, paired
sequence ordering, empty and short outputs, sparse cross-lane cases, sentinels
around output slices, and comparison with ordinary extension multiplication.
The exhaustive ignored test was run explicitly: every canonical feedback/top
pair is checked at both predecessor endpoints. Falcon integration tests pass:
175 passed, with four pre-existing large qualification tests ignored.
Independent review found no blocking arithmetic, feature-gating, memory-safety,
or call-site issue. Formatting and diff whitespace checks pass.

All **504** matrix and traced proofs verified. Payload sizes, source geometry,
protocol IDs, input digests, and security estimates match across implementations.

## Results

At the 128-bit target on one thread, total proving time improves by **1.3–1.4%**
and verification by **9.3–9.4%**. The weight-generation kernel is **3.6×** faster,
but represents only a small fraction of the prover. At 16 threads the end-to-end
results are mixed: prover changes are approximately zero and verifier changes
range from a 1.7% regression to a 1.2% improvement. These measurements do not
establish a consistent 16-thread end-to-end gain.

### Field kernel

Two sequences per signature, 1,024 signatures, one thread. Includes basis setup,
start conversion, per-call feedback preparation, and retained output stores.
The case order alternates; two warm-ups precede ten samples.

| Dimension | Individual sequences (ms) | Paired SIMD (ms) | Speedup |
|---|---:|---:|---:|
| 512 | 9.380 | 2.591 | 3.62× |
| 1024 | 14.868 | 4.170 | 3.57× |

### End-to-end timings

All times are milliseconds. Each table uses 1,024 signatures and medians of ten
measured runs. Raw samples and dispersion are retained in `matrix/`.

**100-bit target**

| Falcon dimension | Threads | Baseline prover | SIMD prover | Baseline verify | SIMD verify |
|---|---:|---:|---:|---:|---:|
| 512 | 1 | 357.43 | 353.80 | 54.69 | 48.92 |
| 512 | 2 | 222.16 | 219.50 | 41.52 | 38.63 |
| 512 | 4 | 156.20 | 155.30 | 34.98 | 33.98 |
| 512 | 8 | 123.69 | 122.72 | 31.78 | 30.94 |
| 512 | 16 | 119.44 | 119.29 | 30.91 | 31.43 |
| 1024 | 1 | 724.10 | 716.10 | 112.81 | 102.52 |
| 1024 | 2 | 452.60 | 447.19 | 86.51 | 81.16 |
| 1024 | 4 | 319.36 | 317.16 | 73.67 | 71.49 |
| 1024 | 8 | 250.32 | 249.92 | 66.96 | 65.95 |
| 1024 | 16 | 235.87 | 236.11 | 63.84 | 64.55 |

**128-bit target**

| Falcon dimension | Threads | Baseline prover | SIMD prover | Baseline verify | SIMD verify |
|---|---:|---:|---:|---:|---:|
| 512 | 1 | 401.05 | 395.73 | 54.88 | 49.72 |
| 512 | 2 | 245.25 | 242.92 | 42.16 | 39.21 |
| 512 | 4 | 169.04 | 167.97 | 35.76 | 34.44 |
| 512 | 8 | 130.87 | 129.84 | 32.82 | 31.59 |
| 512 | 16 | 124.86 | 124.58 | 31.81 | 31.86 |
| 1024 | 1 | 795.74 | 784.84 | 113.82 | 103.25 |
| 1024 | 2 | 488.72 | 483.61 | 86.67 | 82.14 |
| 1024 | 4 | 338.90 | 335.90 | 74.44 | 71.91 |
| 1024 | 8 | 261.57 | 261.45 | 68.26 | 66.65 |
| 1024 | 16 | 244.30 | 244.28 | 65.66 | 64.87 |

### Coordinate-stage timings

Separate traced runs, one thread, 128-bit target, one warm-up and five measured
runs. This stage also includes work outside the recurrence, such as public
polynomial evaluation.

| Dimension | Stage | Baseline (ms) | SIMD (ms) |
|---|---|---:|---:|
| 512 | prove | 8.82 | 3.33 |
| 512 | verify | 8.92 | 3.46 |
| 1024 | prove | 22.30 | 11.50 |
| 1024 | verify | 22.50 | 11.90 |

### Proof payload

Payload accounting is identical before and after, at every measured thread count.
These are payload bytes, using the existing benchmark accounting.

| Dimension | Security target | Bytes | KiB |
|---|---:|---:|---:|
| 512 | 100 | 227,066 | 221.74 |
| 512 | 128 | 264,474 | 258.28 |
| 1024 | 100 | 314,770 | 307.39 |
| 1024 | 128 | 361,570 | 353.10 |

## Reproduction

The baseline and candidate metadata record source and binary hashes. In the
baseline manifest, `baseline_commit` is the canonical revision (`9903fdb6c`);
the older `commit` field records its parent, because the manifest was captured
before the preceding work was committed. The relevant baseline source hashes
match `9903fdb6c`. ELF compiler metadata confirms Rust 1.98.1 for both binaries.
Actual per-process commands and environment variables are in `matrix/metadata.json`;
the runner is `bench.py`. Rebuild the baseline from `9903fdb6c` and the candidate
from the changed tree, then retain each executable as
`<run-root>/{baseline,candidate}/falcon_algebraic`.

From the repository root, build each version with:

```sh
CARGO_TARGET_DIR=/tmp/falcon-simplify-candidate-target CARGO_BUILD_JOBS=4 \
RUSTFLAGS='-C target-cpu=native' \
cargo build --release --locked --offline --features falcon-hybrid --example falcon_algebraic
```

Run the comparison and optional separate traces:

```sh
python3 results/falcon-avx512-20261007/bench.py \
  --repo "$PWD" --root <run-root> --out <new-matrix-output-directory>
python3 results/falcon-avx512-20261007/trace.py \
  --repo "$PWD" --root <run-root> --out <new-trace-output-directory>
```

The field example can be linked against the field library from the same build:

```sh
rustc --edition 2024 -C opt-level=3 -C target-cpu=native \
  -C lto=fat -C codegen-units=1 \
  --extern field=/tmp/falcon-simplify-candidate-target/release/deps/libfield-c79c232b0047b701.rlib \
  -L dependency=/tmp/falcon-simplify-candidate-target/release/deps \
  vendor/field/examples/q12289_power_pair.rs -o /tmp/falcon-field-pair-bench
taskset -c 0 /tmp/falcon-field-pair-bench
```

Cargo's field artifact suffix may differ on another checkout. `kernel.jsonl`
contains all kernel samples, and `kernel-assembly.txt` records the native
512-bit recurrence assembly. Earlier prototype timings using Rust 1.94.0 are
not used in these result tables.

Run field tests with `bash results/falcon-avx512-20261007/field-tests.sh`.
The Falcon integration command was:

```sh
CARGO_TARGET_DIR=/tmp/falcon-simplify-candidate-target CARGO_BUILD_JOBS=4 \
RUSTFLAGS='-C target-cpu=native' \
cargo test --release --locked --offline --features falcon-hybrid --lib --no-run
RAYON_NUM_THREADS=4 \
  /tmp/falcon-simplify-candidate-target/release/deps/bitz-b0347a22deb10013 \
  falcon --test-threads=1
```

Test and build logs are in `tests/`. The isolated field harness compiles the
actual field sources with edition 2024 and denies unsafe operations lacking an
explicit unsafe block. Generic tests exercise the portable dispatcher and call
the SIMD kernel only after runtime feature detection; native tests exercise the
SIMD dispatcher as well. The exhaustive test was run explicitly on this host.
