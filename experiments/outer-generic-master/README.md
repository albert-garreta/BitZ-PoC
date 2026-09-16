# Generic outer sumcheck qualification against master

## Acceptance requirements

For each multiplication width (32, 64, 128), exponent (12, 15, 17, 19), and
thread count (1, 10), require independently:

1. generic ordinary / master ordinary <= 0.95;
2. generic K=3 / generic ordinary <= 0.95.

Each requirement has 24 cells. Both reversed-order sessions must pass; no
cross-cell averaging. K=1,2,4 remain correctness targets. These benchmarks
measure the outer stage, including equality preparation and required input
projection, with verification outside the timer. They do not time full proofs.

## References

- Master: `ac0aa44c5785db30f889fce8c5cdc98264a0e686`, verified against local master,
  origin/master and the live remote master when this campaign started.
- Pre-optimization generic: `f86193f1dcbb43a4bba48ea20670914df334f750`.
- `a3450385` is **not** the acceptance baseline for this campaign.

The master adapter calls its unchanged native u32 kernel. For u64/u128 it uses
master's unchanged compact field projection and ordinary field kernel, with
projection timed. `prepare_master.py` extracts master and installs only the
benchmark driver; it does not transplant newer arithmetic into master.

Master predates the current transcript sampling interface. Each master proof
verifies against its own verifier. Optimization compatibility is checked against
f86193f1, including continuing-transcript digests. The drivers also hash every
input row in one common integer encoding to enforce identical fixtures.

## Reproduction

Run from the repository root, one build/campaign at a time:

```sh
python3 experiments/outer-generic-master/prepare_master.py \
  bench_results/outer-generic-master-20260916/master-source
python3 experiments/outer-generic-master/build.py \
  --revision master --output bench_results/outer-generic-master-20260916/builds/master
python3 experiments/outer-generic-master/build.py \
  --revision current --output bench_results/outer-generic-master-20260916/builds/current
python3 experiments/outer-generic-master/qualify.py \
  --master bench_results/outer-generic-master-20260916/builds/master/outer \
  --current bench_results/outer-generic-master-20260916/builds/current/outer \
  --out bench_results/outer-generic-master-20260916/qualification
```

The scripts require fresh output directories. Builds use Rust 1.98.1,
`RUSTFLAGS=-C target-cpu=native`, the locked dependency graph, and the exact
executable reported by Cargo's JSON output. Each frozen executable has a SHA-256
and a source snapshot. Timing runs use the shared `/tmp/f2z-bench.lock`, require
sustained 95% host idle, and record Linux CPU/load/memory counters.

`--ordinary-only` qualifies the first requirement while developing ordinary.
Each case runs one excluded warmup and 21 measured proofs in each of two
sessions. Implementation order and case order are reversed/balanced. Cells with
a ratio above 0.93 in either session receive 101-sample confirmation in both
orders. Initial failures remain in the output alongside confirmations.

`profile.py` enables separate setup, coefficient, first-fold and continuation
wall-clock measurements through the same verified benchmark driver. Phase
instrumentation is opt-in and compiled only with `bench-internals`.

## Current status

**Post-cleanup outer qualification passed.** The frozen `simplified-current`
candidate passed all 24 cells for each requirement in both properly reversed
21-sample sessions. Its worst ordinary/master ratio was **0.870601** and its
worst skip/ordinary ratio was **0.870709**. No cell required 101-sample
confirmation. SHA/P-256 application qualification is still in progress.

Executable SHA-256:
`28098fce634a5175914ffca4921598442e10ce312bb247158cac641aff98867f`.
The source snapshot, separate acceptance tables, and all raw samples are in
`bench_results/outer-generic-master-20260916/{builds/simplified-current,simplified}/`.
These results qualify the measured Linux/Ryzen host and frozen source.

The pre-cleanup candidate skip-v6 also passed both requirements; its worst
ordinary/master ratio was 0.919425 and worst skip/ordinary ratio was 0.874457.
Direct before/after cleanup measurements are retained separately.

Candidates skip-v1 through skip-v3 failed the second requirement. Skip-v3
passed all u32/u64 cells and failed six u128 cells; all six also failed their
101-sample confirmations. Initial failures remain recorded. Phase measurements
identified skip-message coefficient generation as the dominant wide-input cost.
Skip-v2 removed stack gathers and unnecessary full-width product checks;
skip-v3 removed repeated sign/magnitude processing from mixed accumulation.
Skip-v4 transposed the finite-difference traversal with fewer live temporaries;
only the two u128, 2^12-row cells missed the skip threshold. Skip-v5 added an
x86-64 borrow chain but still missed those initial cells. Skip-v6 also expressed
addition as an explicit carry chain and passed. Portable fallbacks and
independent BigInt carry/borrow checks through 20 limbs remain.

**Ordering correction:** through skip-v4, reversing both the case list and its
enumeration parity accidentally kept each case's implementation order unchanged
between sessions. Those runs are development measurements, not final reversed-
order qualification. From skip-v5 onward, the original case index is preserved
when reversing the list, so each case also reverses implementation order. All
earlier samples and failures remain retained.

All raw samples, source snapshots, executable hashes, initial and confirmation
acceptance tables, and phase measurements are retained under the ignored
`bench_results/outer-generic-master-20260916/` directory. Do not cite the overall
implementation as qualified until both requirements pass on the same final
executable.

## Arithmetic and protocol invariants

`OuterArithmetic<AB,C>` owns arithmetic choices; the first-round traversal,
equality buckets, finite differences, parallel partitioning, prefix folding,
and field continuation are shared. Production storage adapters borrow native
split products and reconstruct only the scalar read at a public row index.
Retained native skip arithmetic is test-only under `native_skip/reference.rs`.

For K<=4, the maximum positive or negative coefficient sum in interpolation
and its finite-difference intermediates is 1,374,322,688. The working types are:

| Input | A/B work | C and residual work |
| --- | --- | --- |
| u32 | i64 | i128 |
| u64 | i128 | Z<3> |
| u128, signed Z<2> | Z<3> | Z<5> |
| signed Z<9> | Z<10> | Z<20> |

The first three residual magnitude bounds are 126, 190, and 318 bits;
ordinary first-round products need only 64, 128, and 256 magnitude bits.
These bounds are public type/circuit bounds. Arithmetic does not trim leading
words or select a schedule from private values.

Mixed MAC weights represent `coefficient * 2^(64*j) * R^2 mod p`. Reducing
once therefore produces the required Montgomery scale R. With <=2^32 residuals
per bucket and <=32 words per residual, the unreduced sum is <p*2^101,
including one signed radix correction per residual. Thus a single REDC is
valid for fields with at least two limbs. One-limb fields use general reduction.
The separate field-product reducer selects its schedule from the public modulus
and term count, with the bound `terms*p < R`; larger bounds use general reduction.

`check_pins.py EXECUTABLE OUTPUT` checks ordinary and K=1..4 across all widths,
sizes, both thread counts, and both public-generic/production entrypoints against
frozen f86193f1 proof-and-continuing-transcript digests. The root sumcheck tests
also cover arbitrary claims, zero/one equality coordinates, vanishing scales,
empty tails, malformed dimensions, grinding, and independent legacy references.
Dependency-crate arithmetic tests must be run separately with
`cargo test --manifest-path vendor/field/Cargo.toml`.

`compare_previous.py --previous FROZEN_F861 --current EXECUTABLE --out OUTPUT`
reports supplementary interleaved K=3 timings against the pre-optimization
generic API. It does not replace either acceptance gate.
Use `--previous-sha256 HASH --protocols ordinary skip-3` to compare a frozen
pre-cleanup executable against the simplified code, retaining both orders and
all individual configurations.

## Simplification validation

The requested simplify-rust review covered all eight dimensions, with findings
and the reconciled plan under ignored `.tmp/simplify/`. Accepted changes reuse the one-term fold for singleton inputs, complete
test-only gating of reference support, and remove unused imports,
redundant closures, and stale comments. Independent arithmetic oracles, public
signatures, fixed-width bounds, error checks, and portable fallbacks remain.
Sharing the prepared continuation handoff was also tested, but a repeated
u32 2^19-row, ten-thread slowdown in the cleanup comparison led to retaining the
original separate wrappers and prepared-state layout. Failed candidates and
101-sample fixed-CPU confirmations remain in the artifacts.
Clippy runs use the matching Rust 1.98.1 component; the repository has unrelated
pre-existing warnings. Benchmark samples and cleanup artifacts are not committed.

## SHA-256 chain and P-256 campaign

The requested full campaign is retained under
`bench_results/sha256-p256-master-generic-20260916/`: 48 configurations per
revision, five measured repetitions after one excluded warmup. It covers all
three backends, exponents 4–7, one and ten threads, and both rates/profiles.
Each frozen worker has a SHA-256, build command, source snapshot, and compiler
version in `builds/`. Master is the same pinned `ac0aa44c` used above.
`comparison/REPORT.md` and `comparison/comparison.csv` contain the completed
initial simplified-candidate comparison, including all increases above 5%.

Generate the same report from any completed paired campaign with:

```sh
python3 experiments/outer-generic-master/sha256_report.py \
  MASTER_CAMPAIGN CURRENT_CAMPAIGN FRESH_REPORT_DIRECTORY
```

Time summaries are per-configuration medians. Spartan PIOP is protocol time
minus PCS opening, computed for each sample before aggregation; outer and inner
sumchecks are subphases. Proving is commitment plus protocol; the separate
fresh-input prover metric also includes witness generation. Median subphases
need not sum to the median total. Peak RSS covers the whole worker, including
setup and verification. Proof material includes the serialized proof and its
counted authentication material. The report checks matching fixture identifiers
and security accounting, and keeps the full raw samples. A five-sample observed
increase is a regression flag, not a statistical confidence claim. Comparison
with master measures all branch changes, not simplification in isolation.
