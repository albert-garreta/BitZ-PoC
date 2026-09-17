# GKR and PCS ownership optimization measurements

For the subsequent analytic-constant and compact-coefficient changes, see the
[September 16 follow-up report](gkr-four-optimization-results.md). It records
the retained changes, rejected kernel/workspace experiments, and the limits of
the shortened final validation. Results from the two campaigns are not additive.

Baseline: `50684158ab8721632cba71b0886a3311f37404c0`. The changes preserve
the protocol, transcript order, proof encoding, grinding, and verifier policy.
This work prioritizes prover latency; it retains useful column layouts,
encoded codewords, Merkle trees, and field-valued fold buffers.

## GKR changes

- `GroupBufs<'a, F>` and `QuadBitGroup<'a>` borrow immutable packed bits.
  Native column rows supply aligned halves directly; small or lane-only
  callers retain a locally owned extraction fallback. Pair and leaf layers
  share these slices.
- `EqInnerGroupMixed<'a, F>` accepts borrowed equality points through `Cow`.
  JIT layer generation and its following sumcheck share one suffix arena.
- The x86 PCLMUL dense grid kernel specializes zero, one, or two pending
  folds. For two challenges it uses
  `a + r0(a+b) + r1(a+c) + r0*r1(a+b+c+d)`, reducing the sum of the three
  products once. Grid products accumulate unreduced until the end of a tile.
  Other architectures and unsupported feature sets retain their existing paths.
- The forward column transpose remains available for the later reduction;
  borrowing removes the inverse transformation and payload copies.

## Method

Machine: AMD Ryzen 9 9950X3D, Linux 6.17.0-41-generic, rustc 1.98.1
(LLVM 22.1.8). Native builds use
`RUSTFLAGS='-C target-cpu=native -C force-frame-pointers=yes'` and the existing
fat-LTO, one-codegen-unit bench profile. Affinity is CPU 0 for one thread
and CPUs 0–9 for ten threads. The primary forest schedule is L4.

The capture scripts use Linux affinity APIs, `taskset`, and GNU `time`.
Prebuilt binaries are alternated in fresh processes with one warmup. Primary
SHA/P256 comparisons measure five verified proofs per process; the broad
workload screen measures three. Reported relative changes use
the geometric mean of paired process-median ratios; 95% intervals bootstrap
whole paired blocks. They describe this machine and fixture, not all hosts.
Primary SHA/P256 comparisons use six blocks. Exploratory checkpoints and
the broad workload screen use three; uncertain cases receive longer repeats.
Compilation and allocation instrumentation do not run during latency captures.

`scripts/compare_prover_snapshots.py` checks exact SHA/P256 proof digests.
`scripts/compare_prover_workloads.py` covers native u32/u64/u128, full u32
W1/W8, SHA chains, and MultiSwap. All measured proofs must verify and sizes
must match. Transcript-state pins separately check serialized proof parts.
Verifier timings come from the existing benches immediately after proving.
The legacy SHA/u32 end-to-end estimate adds separately measured witness time
to proof time; it is not an enclosing wall-clock span.

The frozen baseline uses identical reporting-only changes in
`benches/common/mod.rs` and `benches/mul_e2e_compare/f2z.rs`. These expose
the exact `mc:forest` span rather than the broader forest-plus-reduction
summary. Binary hashes, commands, raw logs, and summaries are retained under
`PerfRuns/ownership-20260916/` (local artifacts, not committed binaries).

Allocation captures use `examples/gkr_capture.rs` with `bench-peak-memory`
and `scripts/compare_prover_memory.py`. Requested live Rust heap, allocated
bytes, allocation calls, and whole-process RSS are distinct measurements.
Phase counters include allocations by workers while the main-thread span
is active. Their timings must not be used as latency evidence. Setup can
dominate whole-process peak even when GKR's peak falls.

## Exploratory checkpoints

For 1,024 SHA compressions plus P-256, kernel plus borrowing reduced GKR
time by 5.6% at ten threads and 5.0% at one thread in three paired blocks.
Using the regular Karatsuba field multiply for the grid's left weights
provided a further 2.1% / 1.1% reduction in a separate comparison. These
separate percentages must not be added to claim a final speedup.

No protocol-level verifier speedup is expected from the GKR changes.
Memory improvements likewise do not imply proportional time savings.

The completed GKR commit, `be94f90c`, was also compared directly with the
baseline in six paired blocks. At 1,024 compressions, GKR time fell 6.6%
[4.2, 8.8] at ten threads and 5.9% [5.8, 6.0] at one thread. At 128
compressions it fell 12.3% and 3.5%, respectively. The combined results
below use independent baseline-versus-final captures.

## GKR allocation checkpoint

At 1,024 compressions and ten threads, requested live heap during GKR peaks
at 279.92 MiB versus 282.60 MiB at baseline (2.68 MiB lower). GKR allocates
296.47 MiB cumulatively versus 308.13 MiB, and allocation calls fall from
198,593 to 155,261 (21.8%). Bit extraction falls from 5,414 allocations to
2 small descriptor allocations. The total process heap peak remains
799.25 MiB because setup dominates it. Whole-process RSS is approximately
664–668 MiB in these single fresh-process captures; that difference is too
small to claim a robust RSS improvement.

The combined latency and validation results follow below.


## PCS changes

- The Ligerito `SumcheckProver<'a>` and basis entry points accept either an
  owned vector or a borrowed slice through `Cow<'a, [Gf128]>`. Folding reads
  the initial slice and produces owned smaller buffers. Borrowed memory
  never enters the scratch pool. Recursive commitments read current folds
  directly, and terminal folds/messages move into the proof.
- Query rows are borrowed during basis induction, then moved into proof
  storage, removing their second copy.
- `FlockCommitHint` shares immutable source rows through `Arc` and caches
  alternate columns in `OnceLock` on demand. The ECDSA witness and hint
  share the same source allocation; independently supplied witnesses still
  receive the content-equality check. Encoded codewords and Merkle trees
  remain owned and materialized.
- `PackedSourcePlanes<'a>` borrows instance weights while retaining both
  useful table layouts on the prover path.
- SHA/P256 packers write to their final columns. Assignment jobs cover
  whole transpose tiles even when a tile spans multiple columns; each tile
  is transformed once. The unused flat intermediate and its copy disappear.
- Row-bit weights reuse their equality buffer as output. Each row computes
  `alpha^w` once and advances bit powers by squaring, removing the base
  vector and repeated square-and-multiply chains. This affects both prover
  reduction and verifier weight evaluation.
- Coefficient and inner-matrix temporaries are dropped after their final
  use; single-chunk weights adopt their vector directly. Prepared Ligerito
  configurations are borrowed instead of cloned in both prover and verifier.

The GKR schedule, lazy depth, arity, thread thresholds, and security settings
are unchanged. Deeper lazy schedules, dense-buffer pools, broad prepared
matrix removal, and new accelerator backends remain separate experiments;
this implementation does not claim their potential savings.

## Combined allocation results

Fresh processes, ten threads, L4. Values are MiB of requested live Rust
heap, including allocations already live at phase entry.

| SHA compressions + P-256 | Phase | Baseline peak | Combined peak |
|---|---|---:|---:|
| 128 | Proving | 210.97 | 179.36 |
| 128 | GKR | 195.92 | 137.39 |
| 128 | Verification | 136.92 | 133.91 |
| 1,024 | Proving / GKR | 282.60 | 220.30 |
| 1,024 | Verification | 148.32 | 143.03 |

At 1,024 compressions this is **22.0% lower peak during proving**.
Most of the additional reduction over the GKR commit comes from releasing
matrix temporaries before GKR. Forest allocations themselves retain the
GKR commit's 21.8% reduction in allocation calls. Commitment allocations
fall from 2,115 to 19 because it shares source rows and skips the unused
transpose. Setup remains the global heap peak at 799.25 MiB. There is no
substantiated whole-process peak-RSS improvement in these captures.

## Correctness and build validation

- 475 release library tests pass, including the independent row-weight
  oracle, packing across column boundaries, shared-row mutation checks,
  lazy/eager forests, quad paths, tampering checks, and SHA/P256 relations.
- 21 existing transcript/serialized-proof pins pass without updating pins,
  including SHA/P256 split/all-rows at both security levels, u32 W1/W8,
  u64/u128, SHA chains, and MultiSwap.
- Four virtual-opening tests, four Ligerito-policy tests, the forest
  grinding test, and the explicitly enabled ECDSA proof-byte/allocation
  regression test pass. The x86 grid differential test compares complete
  buffers and messages against the generic kernel, including untouched tails.
- Vendored Ligerito's borrowed lookahead test and both basis round trips pass.
  The standalone vendor lockfile needed offline reconciliation; its tracked
  contents were restored and the resolved test lockfile is retained with artifacts.
- `cargo check --no-default-features --lib --features ecdsa` passes.

An existing quad test changed process-wide environment variables while
unrelated tests ran. The first default-parallel run exposed this race;
serial execution passed. The quad test now runs its environment changes in
an isolated child process, and the final default-parallel suite passes.

Core commands (all Cargo commands used `--offline`; root builds also used
`--locked`):

```sh
RUSTFLAGS='-C target-cpu=native -C force-frame-pointers=yes' \
RAYON_NUM_THREADS=10 cargo test --release --features ecdsa,bench-internals \
  --lib --test transcript_state_pins --test virtual_open \
  --test forest_grinding --test ligerito_protocols

RUSTFLAGS='-C target-cpu=native -C force-frame-pointers=yes' \
cargo bench --no-run \
  --features sha256-ecdsa-compare,native-mul-compare,bench-internals \
  --bench sha256_ecdsa_compare --bench sha256_chain --bench u32_mul \
  --bench mul_e2e_compare --bench multiswap

RUSTFLAGS='-C target-cpu=native -C force-frame-pointers=yes' \
cargo build --release --example gkr_capture \
  --features ecdsa,span-metrics,bench-peak-memory
```

Run the comparison scripts under `scripts/bench_gate.py run` with
`PERFETTO_TRACE_PROCESSOR` pointing to the native trace processor. Their
manifests record exact binary hashes and arguments. Keep allocation captures
separate from the latency campaign.

## Combined SHA/P256 latency

Six paired blocks, five measured proofs after warmup per process. Positive
percentages mean less time; brackets give the paired 95% interval.

| Compressions | Threads | GKR time saved | Witness through proof time saved | Verifier time saved |
|---:|---:|---:|---:|---:|
| 128 | 1 | 3.6% [3.5, 3.7] | 1.7% [1.6, 1.7] | 0.5% [0.2, 0.8] |
| 128 | 10 | 12.0% [11.0, 13.1] | 1.7% [-0.0, 3.4] | -1.7% [-6.5, 2.9] |
| 1,024 | 1 | 3.3% [1.6, 4.9] | 1.7% [0.8, 2.6] | 0.1% [-0.3, 0.5] |
| 1,024 | 10 | 11.0% [8.3, 13.8] | 5.0% [2.5, 7.6] | -1.6% [-3.8, 0.8] |

The primary 1,024-compression, ten-thread case has an estimated 11.0%
GKR reduction, with an 8.3–13.8% interval: the point estimate meets the 10%
target, but the data do not establish a guaranteed minimum of 10%.
The 128-compression end-to-end and ten-thread verifier results are noisy;
no verifier speedup is claimed for this workload. Proof digests match
exactly throughout.

Direct comparisons of the PCS changes against the GKR commit do not
resolve an additional latency improvement statistically. Three paired
blocks at 1,024 compressions and ten threads estimate 4.3% less overall
proving time and 1.6% less GKR time, with both intervals including zero.
A six-block one-thread repeat estimates 0.4% less overall time and 2.5%
more GKR time (GKR interval: 0.1% faster to 4.9% slower). The final
implementation still improves on the original one-thread baseline, but
the PCS commit's clear additional benefit is lower live heap. The
separate campaigns must not be combined into additive speedup claims.

## Workload regression screen

Positive numbers mean less time. The initial screen used three paired
blocks and three measured proofs per process; rows marked † use a fresh
nine-block, five-proof repeat. All proofs verified and proof sizes matched.
Native multiplication covers `2^15` and `2^19` operations; full u32 covers
W1 and W8. SHA chains cover 128, 1,024, and 4,096 compressions. MultiSwap
uses the existing shape-0 fixture with 1, 4, and 8 batches.

| Workload | Threads | End-to-end time saved | GKR time saved | Verifier time saved |
|---|---:|---:|---:|---:|
| u32-mod32-n15 | 1 | 3.5% | 4.1% | 0.6% |
| u32-mod32-n15 | 10 | 4.3% | 5.8% | -0.1% |
| u32-mod32-n19 | 1 | 3.7% | 4.2% | 2.0% |
| u32-mod32-n19 | 10 | 3.1% | 3.8% | 0.7% |
| u64-n15 | 1 | 3.7% | 4.5% | 1.0% |
| u64-n15 | 10 | 3.8% | 5.8% | -0.4% |
| u64-n19 | 1 | 5.7% | 6.6% | 3.9% |
| u64-n19 | 10 | 4.7% | 5.9% | 2.5% |
| u128-n15 | 1 | 5.5% | 6.3% | 0.8% |
| u128-n15 | 10 | 7.2% | 8.6% | 1.7% |
| u128-n19 | 1 | 2.9% | 3.7% | 2.2% |
| u128-n19 † | 10 | 5.1% | 5.9% | -2.0% |
| full-u32-w1-n15 | 1 | 3.5% | 4.2% | 0.8% |
| full-u32-w1-n15 | 10 | 6.6% | 7.1% | 1.5% |
| full-u32-w1-n19 | 1 | 6.0% | 7.0% | 3.6% |
| full-u32-w1-n19 | 10 | 5.3% | 5.4% | 3.5% |
| full-u32-w8-n15 | 1 | 4.2% | 4.1% | 13.6% |
| full-u32-w8-n15 | 10 | 2.1% | 1.9% | 1.6% |
| full-u32-w8-n19 | 1 | 5.5% | 6.1% | 24.6% |
| full-u32-w8-n19 | 10 | 4.3% | 4.7% | 6.2% |
| sha-n7 | 1 | 1.3% | 6.5% | -0.3% |
| sha-n7 † | 10 | 6.8% | 15.2% | -0.5% |
| sha-n10 | 1 | 1.4% | 2.9% | -0.3% |
| sha-n10 | 10 | 3.3% | 8.9% | -0.9% |
| sha-n12 | 1 | 3.3% | 4.9% | 0.3% |
| sha-n12 | 10 | 2.1% | 7.3% | -2.1% |
| multiswap-b1 | 1 | 3.4% | 4.2% | -1.0% |
| multiswap-b1 | 10 | 3.7% | 6.3% | -1.2% |
| multiswap-b4 | 1 | 2.3% | 3.2% | 0.2% |
| multiswap-b4 † | 10 | 2.1% | 3.2% | 1.0% |
| multiswap-b8 | 1 | 1.8% | 2.5% | -1.1% |
| multiswap-b8 † | 10 | 2.7% | 3.8% | -5.0% |

The noisy four-batch MultiSwap cell improves on the longer repeat: 2.1%
end-to-end and 3.2% GKR. The two initially flagged single-thread verifier
cases (native u32 at `2^19`, and full u32 W8 at `2^19`) improve in the final
implementation. This is not a claim that every verifier timing improves:
the repeated `2^19` u128, ten-thread verifier is **2.0% slower** (paired
95% interval: 0.2–4.2% slower), while its proving time falls 5.1%.

The longer eight-batch MultiSwap repeat confirms a verifier regression:
**5.0% slower** [3.0, 6.9], alongside 2.7% less end-to-end proving time
[1.9, 3.5] and 3.8% less GKR time [2.6, 4.9]. Existing trace spans localize
the increase to relation projection: the median of process medians rises
from 37.60 to 39.97 ms. Its PCS opening verification decreases slightly,
8.30 to 8.25 ms. Relation projection's code was not changed; the underlying
cause of its timing change is unresolved. These timings follow proving in
the same process and do not establish isolated verifier-service performance.
The phase breakdown is retained in
`PerfRuns/ownership-20260916/b2-multiswap8-repeat/verifier-phases.json`.

The final changes improve the measured prover workloads, but **do not pass
a blanket no-verifier-regression requirement**. The u128 and MultiSwap
tradeoffs above remain. A 3% slowdown was used as a material-regression
screening threshold; uncertainty near that threshold is reported rather
than treated as a guaranteed pass. Other small changes and intervals
remain in the raw summaries.
