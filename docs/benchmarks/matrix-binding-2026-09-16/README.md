# MacBook matrix-binding qualification

The shared implementation improves the two priority paths: SHA binding takes **21.9% less time on one thread** and **15.1% less on four**, while P-256 terminal evaluation takes **29.2–29.3% less time**. These measurements use the production-style split-equality column functional. All runs were local; no `will`/x86 qualification was performed.

## Method

Baseline: `d9ea2ad3fd1bce6e59aff5fe9290fddc682e634e`, with a clean tracked working tree at the freeze. Unrelated untracked files were preserved. The original source archive, empty working-tree patch and environment capture remain in `/tmp/f2z-binding-baseline-20260916-163434/`. Both checkouts used identical standalone benchmark sources. The baseline checkout received only those harnesses and the allocation-measurement feature declaration.

Machine: ARM64 MacBook Pro, Darwin 25.5.0, Rust 1.98.1, LLVM 22.1.8. No explicit target-CPU override was added. Graph timing uses the circuit crate's release profile; proof timing preserves the root release profile (`lto=true`, `codegen-units=1`). Rayon was fixed to one or four threads. Allocation instrumentation was disabled during timing. Profile captures and compiler/test jobs were not run concurrently with the final timing campaign.

Graph figures are the median of three process medians, each with five warmups and 100 measurements. Baseline/current order alternated across processes. Construction uses five warmups and 15 measurements. Proofs use two warmups and 25 measurements per process; disputed results received four further alternating comparisons with 100 measurements each. Every timed proof was verified. Graph geometry is SHA-256 over 2 KB, or the P-256 digest-verification circuit, over `2^127-1`.

The terminal column oracle matches production's split equality and geometric suffix formula; the P-256 fixture uses an unaligned tail offset of `20,457 × 8`. Equality-factor preparation is outside traversal timing. This does **not** include all ECDSA statement preparation, corrections or transcript work.

**Correction to the earlier table:** the original 3.8 ms/5.4 ms terminal measurements used a synthetic constant-column oracle whose `power_sum` looped over every bit. Instruments attributed over half of P-256 time to that helper. Those absolute terminal numbers are not comparable with the corrected numbers below. Both baseline and current code were rerun with the same production-style oracle. Binding numbers remain comparable.

## Graph results

Time in microseconds; negative changes mean less time.

| Operation | Threads | Baseline | Current | Change |
|---|---:|---:|---:|---:|
| SHA prepare | 1 | 12.375 | 1.958 | −84.2% |
| SHA bind, reused | 1 | 2,516.791 | 1,966.125 | −21.9% |
| SHA terminal, reused | 1 | 1,825.167 | 1,540.417 | −15.6% |
| SHA prepare + bind | 1 | 2,764.625 | 2,193.458 | −20.7% |
| P-256 prepare | 1 | 28.000 | 21.708 | −22.5% |
| P-256 bind, reused | 1 | 4,715.791 | 3,044.166 | −35.4% |
| P-256 terminal, reused | 1 | 1,546.917 | 1,095.042 | −29.2% |
| P-256 prepare + bind | 1 | 5,122.334 | 3,471.542 | −32.2% |
| SHA prepare | 4 | 12.125 | 1.958 | −83.9% |
| SHA bind, reused | 4 | 909.500 | 771.875 | −15.1% |
| SHA terminal, reused | 4 | 1,818.833 | 1,541.084 | −15.3% |
| SHA prepare + bind | 4 | 1,170.000 | 1,025.250 | −12.4% |
| P-256 prepare | 4 | 25.417 | 21.667 | −14.8% |
| P-256 bind, reused | 4 | 1,435.334 | 993.875 | −30.8% |
| P-256 terminal, reused | 4 | 1,545.542 | 1,092.542 | −29.3% |
| P-256 prepare + bind | 4 | 1,905.834 | 1,444.458 | −24.2% |

Tape construction also improved: SHA **60.175 → 13.813 ms** (−77.0%); P-256 **119.633 → 20.753 ms** (−82.7%). An intermediate adapter implementation had regressed construction substantially. Recording packed ranges directly removed the per-bit nodes, coefficient multiplications and graph allocations responsible for that regression.

The SHA binding CPU capture attributed about 90% of samples to the packed-output traversal. Two independent doubling chains shorten its dependency chain. P-256 terminal execution now skips entire private packed-column ranges and uses scalar access for one-element aggregates. Neither optimization changes coordinate order or the bilinear form. Delayed graph-node accumulation and delayed terminal-dot experiments did not improve these workloads; the production circuit adapter retains the existing scalar FIOS kernel.

See [graph-summary.csv](graph-summary.csv), [all graph process results](graph-runs.csv), and [construction results](construction.csv).

## Complete proofs and uncertainty

Representative complete proofs cover 32,768 u64 multiplications and a chain of 128 SHA compressions. Witness generation and commitment preparation are outside the timed proof; proving and verification are measured separately. These workloads exercise native/block binding and repeated/chained virtual openings. They do not establish an ECDSA end-to-end speedup from the P-256 graph improvement.

The first three-process campaign showed an apparent 5.4% four-thread u64 prover regression. Four longer, alternating comparisons produced paired changes of **+0.70%, −0.35%, +0.76%, −0.97%** (median +0.18%). Thus the initial 5.4% result did not reproduce. One-thread SHA proving had paired changes **+0.85%, +0.16%, +2.11%, +0.56%** (median +0.71%). Its ratio of campaign medians is +1.14%, illustrating sensitivity to aggregation and drift. The smaller proof differences remain inconclusive at a strict 1% target; these measurements do **not** establish 1% parity for every complete-proof path.

No reproducible regression above 1% remained in the isolated graph operations. The large graph gains repeated across one/four-thread runs. All initial and follow-up proof measurements are retained in [proof-runs.csv](proof-runs.csv), including slower samples.

## Allocations and memory

Separate `bench-memory` runs count Rust allocation/reallocation calls, allocated bytes and additional peak live bytes during each phase. These figures are not process RSS. Preparing a graph now defers direction-specific scratch until that direction is used, so prepare and prepare+bind must be read together.

| Operation | Baseline allocation count → current | Baseline extra peak → current |
|---|---:|---:|
| SHA prepare | 9 → 6 | 1,467,480 → 1,160 B |
| SHA prepare + bind | 10 → 9 | 12,268,160 → 11,976,672 B |
| P-256 prepare | 9 → 6 | 1,221,536 → 16,920 B |
| P-256 prepare + bind | 10 → 9 | 20,671,632 → 20,349,536 B |
| Reused binding, both graphs | 0 → 0 | 0 → 0 B |
| Reused terminal, both graphs | 0 → 0 | 0 → 0 B |

Forward-only workspace: SHA **2,641,664 → 1,175,344 B**; P-256 **2,283,616 → 1,079,024 B**. Compiled topology is 5,546,204 B for SHA (baseline 5,570,488) and 7,570,760 B for P-256 (baseline 7,568,960). The small P-256 topology increase is retained; execution and construction both improve.

Separately, the ECDSA composite owner no longer stores duplicate raw and typed P-256 coefficient vectors. This eliminates one 16-byte-per-tail-element allocation. That saving is a code-level storage result; it is not included in the graph workspace totals above or claimed as measured complete-ECDSA peak memory.

A separate one-thread `/usr/bin/time -l` run, with two warmups and three prove/verify pairs, recorded whole-process maximum RSS: u64 **122,486,784 → 118,079,488 B**, SHA **97,910,784 → 94,846,976 B**. These single-run values include setup and retained inputs and are descriptive, not a statistical peak-memory guarantee. See [allocations.csv](allocations.csv) and [resident-memory.csv](resident-memory.csv).

## Correctness/build qualification

- Circuit linear maps: **31 tests passed**, including explicit-matrix/bilinear oracles, integer/field equivalence, mixed widths, reuse across moduli, retained field providers, foreign handles, packed low/full/zero cases, overflowing coefficient expressions, large delayed chunks and unaligned binary ranges.
- Selected root integration/protocol tests: **89 passed**, one pre-existing microbenchmark ignored. This covers ordinary/prefix/independent rows, dense/block/repeated/composite references, reused outputs, P-256 forward-vs-adjoint evaluation, virtual-opening routing/basis and tampering rejection.
- Default release build and complete proof verification passed. Root serial (`--no-default-features`), circuit minimal and ECDSA-enabled test builds passed.
- `git diff --check` passed. Existing unrelated compiler warnings remain.

## Reproduction

Use two independent checkouts; copy the same current harness into the baseline. Preserve both compiled binaries before switching builds. To reproduce graph latency:

```sh
cargo build --offline --release --manifest-path crates/circuit/Cargo.toml --example linear_map_bench
RAYON_NUM_THREADS=1 SAMPLES=100 crates/circuit/target/release/examples/linear_map_bench
RAYON_NUM_THREADS=4 SAMPLES=100 crates/circuit/target/release/examples/linear_map_bench p256
RAYON_NUM_THREADS=1 SAMPLES=15 crates/circuit/target/release/examples/linear_map_bench p256 construct
```

`PHASE=bind_reused` or `PHASE=terminal_reused` isolates one phase. For allocations, add the empty `bench-memory` feature to the baseline circuit manifest, build each checkout with `--features bench-memory`, and run separately from latency measurements. The current manifest already declares this feature.

```sh
CARGO_INCREMENTAL=0 cargo build --offline --release --example binding_proof_bench
RAYON_NUM_THREADS=1 SAMPLES=25 target/release/examples/binding_proof_bench
RAYON_NUM_THREADS=4 SAMPLES=25 target/release/examples/binding_proof_bench sha
```

Machine/build settings are also recorded in [environment.json](environment.json). MacBook measurements alone do not qualify x86 behavior.
