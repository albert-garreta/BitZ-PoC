# GKR and PCS ownership optimization measurements

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

Prebuilt binaries are alternated in fresh processes, with one warmup and
five measured, verified proofs per process. Reported relative changes use
the geometric mean of paired process-median ratios; 95% intervals bootstrap
whole paired blocks. They describe this machine and fixture, not all hosts.
Small exploratory checkpoints use three blocks; final comparisons use six.
Compilation and allocation instrumentation do not run during latency captures.

`scripts/compare_prover_snapshots.py` checks exact SHA/P256 proof digests.
`scripts/compare_prover_workloads.py` covers native u32/u64/u128, full u32
W1/W8, SHA chains, and MultiSwap. All measured proofs must verify and sizes
must match. Transcript-state pins separately check serialized proof parts.
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

## GKR allocation checkpoint

At 1,024 compressions and ten threads, requested live heap during GKR peaks
at 279.92 MiB versus 282.60 MiB at baseline (2.68 MiB lower). GKR allocates
296.47 MiB cumulatively versus 308.13 MiB, and allocation calls fall from
198,593 to 155,261 (21.8%). Bit extraction falls from 5,414 allocations to
2 small descriptor allocations. The total process heap peak remains
799.25 MiB because setup dominates it. Whole-process RSS is approximately
664–668 MiB in these single fresh-process captures; that difference is too
small to claim a robust RSS improvement.

Final latency measurements and validation are recorded below as each
implementation commit is completed.
