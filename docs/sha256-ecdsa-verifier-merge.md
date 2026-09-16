# SHA-256 + ECDSA verifier merge

Merge of `BitZ-PoC` branch `sha256-ecdsa-verifier-opt` at
`b725f2b3c51b12ba7f11d4090a3b8f9ecc75493e` into master
`07680319a4ab73b6d45c3644420755155866cd34` (2026-09-16).

## Integration

The verifier evaluates the P-256 matrix tape forward, computes the ring-switch
basis with the repetition plane engine and a factored identity tail, and uses
block lookup tables for long tails. Shared improvements include closed-form
dual unpacking, sparse rho-table construction, and size-gated equality-table
parallelism. The SHA matrix dot product applies each high equality factor once
per block.

The merge retains master's `vendor/field` providers, fixed-width `IntegerTable`
coefficients, v2 relation digest, native binary multiply accumulators, direct
identity-tail indexing, and collection into the final dense buffer on the prover
and fallback paths. Tape preparation reuses `PreparedSignedProjection`; the old
upstream Montgomery/Horner kernels and BigInt coefficient storage are not
reintroduced. Forward graph construction uses `RawTerm::is_zero()` because the
coefficient is now an interned ID, not an integer value.

The benchmark emits a BLAKE3 proof digest outside the timed proof and verifier
sections. Both benchmark builds use this identical reporting harness. The baseline
library, circuit and field sources are the original master snapshot.

## Measured results

Times are milliseconds, medians of ten samples per state. `N` is the number
of SHA-256 compressions followed by one P-256 ECDSA verification. Setup and
witness generation are excluded from `prove` (`commit + protocol`); setup,
witness and end-to-end prover measurements are also retained in `summary.json`.

| Mode | N | Target | Rate | Threads | Verify master → merged | Speedup | Prove master → merged |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |
| Split | 8 | 100 | 1/2 | 1 | 22.27 → 5.58 | 3.99× | 136.40 → 140.22 |
| Split | 8 | 100 | 1/2 | 10 | 7.33 → 4.71 | 1.56× | 34.21 → 32.68 |
| Split | 128 | 100 | 1/2 | 1 | 27.93 → 7.01 | 3.98× | 137.51 → 137.27 |
| Split | 128 | 100 | 1/2 | 10 | 8.18 → 5.11 | 1.60× | 41.33 → 39.22 |
| All rows | 8 | 100 | 1/2 | 1 | 21.91 → 5.66 | 3.87× | 138.52 → 141.15 |
| All rows | 8 | 100 | 1/2 | 10 | 7.47 → 4.79 | 1.56× | 33.29 → 36.10 |
| All rows | 128 | 100 | 1/2 | 1 | 26.99 → 6.84 | 3.95× | 138.12 → 139.63 |
| All rows | 128 | 100 | 1/2 | 10 | 8.10 → 5.04 | 1.61× | 42.28 → 39.58 |
| Split | 128 | 100 | 1/8 | 1 | 27.30 → 6.41 | 4.26× | 136.31 → 136.54 |
| Split | 128 | 100 | 1/8 | 10 | 7.67 → 4.63 | 1.66× | 39.36 → 37.74 |
| Split | 128 | 128 | 1/2 | 1 | 28.51 → 7.75 | 3.68× | 917.83 → 916.34 |
| Split | 128 | 128 | 1/2 | 10 | 8.99 → 5.77 | 1.56× | 124.38 → 126.65 |

Verification improves in every configuration: 3.68–4.26× with one thread and
1.56–1.66× with ten threads. This is primarily a verifier improvement; the
campaign does not establish a general prover speedup. The initial prover
medians range from 6.4% faster to 8.4% slower. The slowest relative case
(all rows, eight compressions, ten threads) was repeated with 18 samples per
state, in the same A/B then B/A ordering: proving measured 33.87 →
35.75 ms (5.5% slower), while verification measured
7.11 → 4.50 ms (1.58× faster). Individual merged proving
process medians were 37.88 and 33.47 ms, indicating substantial process-to-process
variation. This small-case proving regression is retained in the report; there
is no claim that the merge improves every prover workload. The follow-up's
samples and summary are stored under `prover-followup/`.

For the 128-compression, split, 100-bit, single-thread case, median coefficient
evaluation falls from 6.58 to 1.77 ms and ring switching from 17.64 to 3.22 ms.
The ring-switch weight preparation alone falls from 4.48 to 0.18 ms. These
phase measurements are nested and should not be added together. Relation setup
is roughly 1.1 seconds in both states and is amortized across repeated proofs.

In the separate eight-compression allocation test, single-thread verifier
incremental peak live allocations fall from 44,863,801 to 14,019,361 bytes
(68.75% lower) for split/100. Both modes and both targets show the same
approximately 69% reduction. This measures allocations made during verification,
excluding the prepared relation and proof; it is not process RSS.

Every measured proof has the same digest and wire size between master and the
merge. The timings are for this CPU, fixtures and explicit parameters; they are
not a cross-machine performance guarantee.

## Validation

- Nine circuit matrix-tape tests pass, including forward/reverse equivalence,
  unreduced canonical inputs, small moduli, and absence of the dense output
  allocation during a forward-only pass.
- Thirty focused SHA+ECDSA, plane, identity-tail and opening unit tests pass.
- All twenty transcript-state pins pass, including the four SHA+ECDSA pins.
- The separate ECDSA regression test preserves all four exact proof hashes and
  final challenges (both outer modes, security targets 100 and 128).
- Four SHA+ECDSA comparison tests and four virtual-opening integration tests
  pass. One unrelated Spartan2 Perfetto instrumentation test remains ignored
  by its existing default.
- The library also checks with `--no-default-features --features ecdsa`.

## Benchmark method

The two states are built in separate source/target directories with Rust 1.98.1,
`--release`, fat LTO, one codegen unit and `RUSTFLAGS='-C target-cpu=native'` on
an AMD Ryzen 9 9950X3D. Each configuration runs five samples after an unmeasured
warmup, twice: baseline then merged on pass one, merged then baseline on pass
two, with configuration order reversed. Reported values are medians of all ten
samples. Every proof is verified. Proof digests and wire sizes must match across
both states and passes for each configuration.

The campaign runs under `scripts/bench_gate.py` after builds and tests finish.
The runner clears inherited F2Z, Rayon and Perfetto options and sets the same
explicit worker count, security target, seed and Ligerito profile for both states.

Compact samples, source/binary hashes, allocation measurements, the exact runner,
validation output and summary are checked in under
[`benchmarks/results/sha256-ecdsa-verifier-merge-20260916`](../benchmarks/results/sha256-ecdsa-verifier-merge-20260916/).
Full worker logs and samples remain in the ignored
`bench_results/sha256-ecdsa-verifier-merge-20260916/` in the working checkout.

## Reproducing the comparison

The checked-in `run.py` is the exact campaign runner; its `ROOT`, `BUILD`, `OUT`
and trace-processor path identify this machine's directories. Point them at the
corresponding directories on another machine. Build baseline and merged workers
separately, with no builds or tests running during the timed campaign:

```sh
# Baseline sources: archive 07680319a4ab73b6d45c3644420755155866cd34.
# Copy the merged benches/sha256_ecdsa_compare.rs into that archive so both
# workers emit the proof digest; leave all baseline library sources unchanged.
# Run this in each source tree, using a different target directory:
env RUSTFLAGS='-C target-cpu=native' CARGO_BUILD_JOBS=8 \
  cargo build --offline --locked --release --features sha256-ecdsa-compare \
  --bench sha256_ecdsa_compare --target-dir /path/to/variant-target

python3 scripts/bench_gate.py run --label ecdsa-verifier-merge \
  --min-idle 88 --hold-seconds 10 --poll-seconds 2 \
  --max-idle-wait-seconds 300 -- python3 /path/to/run.py
```

The runner checks proof digests and sizes and requires more than a 5% verifier
improvement in every tested configuration. The 100-bit runs use `custom:1:4`
(rate 1/2) or `custom:3:4` (rate 1/8). The 128-bit runs use `udrg:1:4:128`,
with identical grinding and query parameters in both workers. An initial attempt
using `custom:1:4` at target 128 was rejected by the existing security check;
those incomplete measurements are excluded, and the entire campaign was rerun
with the supported profile.

Relevant validation commands (root commands use
`--features sha256-ecdsa-compare`; use separate target directories as desired):

```sh
cargo test --offline --locked --manifest-path crates/circuit/Cargo.toml \
  --lib matrix_wengert::tests
cargo test --offline --locked --features sha256-ecdsa-compare --lib -- \
  ecdsa_sha256 virt_batch::tests \
  verifier_basis_matches_streamed_basis_on_the_ecdsa_map \
  chained_compact_tail_weights_and_planes_match_generic virtual_planes_match_cellwise
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=10 \
  cargo test --offline --locked --release --features sha256-ecdsa-compare \
  --test transcript_state_pins --test virtual_open --test sha256_ecdsa_comparison \
  -- --test-threads=1
RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=1 \
  cargo test --offline --locked --release --features sha256-ecdsa-compare \
  --test ecdsa_reduction_regression -- --ignored --nocapture --test-threads=1
cargo check --offline --locked --no-default-features --features ecdsa
```
