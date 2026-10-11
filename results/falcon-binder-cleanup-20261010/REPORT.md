# Falcon binder cleanup and review

Source: `ef2508f3adaf153b891825d1c2f6dc4a69853898` → `454b623e0bdda096721504e9e937cc5a02dc2308`, branch `codex/falcon-binder-experiments`.

The retained cleanup removes **773 net source lines** (78 insertions, 851 deletions across ten files). It preserves the previously qualified folded decoder, factored equality weights, batched reductions, word binder, and production dispatch policies. This report compares cleanup against the already optimized pre-cleanup build; the earlier comparison against `51b472c55` is in [the optimization report](../falcon-binder-20261010/REPORT.md).

**Disposition:** retain the simpler ownership and proven dead-code removal; reject the hot-loop deduplication. Proof contents are preserved in the tested cases. Prover nonregression is statistically unresolved, and the ten-pair full 16-worker verifier comparison has a small +0.329% cost increase.

## Findings and retained fixes

- **Unreachable implementation:** removed the crate-private dense `CompositeMultilinearExtension`, its packed adapter, and helpers/tests reachable only through it. Whole-workspace references, rust-analyzer and compiler checks agreed that there were no production constructors. ECDSA's live `CompactCompositeMle`, shared helpers and independent dense/proof oracle remain; its implementation is unchanged. A malformed-input regression now exercises the live factored adapter.
- **Redundant ownership:** word proofs cloned 8192 `u128` row weights, retaining two 128 KiB allocations through the bridge. A single owned `BridgeClaim` removes that clone and duplicate return value. Its fields now always mean a column point and mandatory row weights. Both binder modes use the existing weighted bridge API, eliminating mode-dependent field meanings, branches and an optional-weight unwrap.
- **Unused experiment interface:** removed the arbitrary group-cap environment override, runner option and associated documentation. Production group caps and the three measured kernel switches remain.
- **Local simplification:** collapsed the single-caller constructor wrapper and corrected comments about custom proof `Debug` formatting.

No actionable correctness or protocol-soundness defect remained in the scoped review. This is not a formal security proof. The bit path still absorbs the full opening point, modulus and value before deriving the same row equality weights and retaining the column coordinates. The word path preserves its weights and transcript. Legacy v5 selection, v6 domains, padding checks, terminal merge grinding, wide-accumulator bounds, sparse lane-15 data and allocation reuse remain.

## Refactors rejected after measurement

The initial cleanup (`ea5fe3158`) extracted shared public-weight/product accumulation helpers and common bridge validation. It preserved proof contents but measured small regressions, including arithmetic 1-worker total time +0.604%, full 4-worker +0.141% and full 8-worker +0.332%, with descriptive intervals above parity.

An `#[inline(always)]` follow-up (`3e7072cb2`) still had positive total-time estimates in all eight cells. Arithmetic 1-worker was +0.140%; full 1/4-worker were +0.182%/+0.194%, also above parity under those intervals. Therefore the final source restores the original hot loops and bridge checks. The two campaigns remain historical evidence; their results are not selected as final performance estimates. They do not isolate the causal cost of each individual cleanup.

## Validation

The final source passed **164 targeted native release tests**: Falcon-512 algebraic 35, Falcon-1024 algebraic 37, integer bridge 16, packed sumcheck 47, profile opening 24, transcript framing 3, polynomial helper 1 and structured MLE 1. Separate development checks passed 49 ECDSA/packed tests, 25 arithmetic-filter tests, 16 bridge tests and one structured-MLE test. These filters overlap; they are not summed into a larger count. Six Python runner tests passed.

Matched production benchmarks use Falcon-512, batch 1024, security 128 at **1, 4, 8 and 16 workers**. Each cell has five seed-paired baseline/candidate processes, one warmup and five measured trials per process. Processes run sequentially with randomized order, pinned CPU affinities and tracing disabled. Both binaries use public defaults and are built without `bench-internals`. Targeted compatibility checks additionally cover v5 Falcon-512 batch 3/security 128 and Falcon-1024 batch 1/security 100 at 1/4 workers.

Host: AMD Ryzen 9 9950X3D, 16 physical cores/32 logical CPUs. Rust 1.98.1, release profile, `RUSTFLAGS="-C target-cpu=native"`, feature `falcon-hybrid`. Worker CPUs: `6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9`; main CPU 6. Arithmetic final seeds are 55–59; clean full seeds are 60–64. The accepted final matrix and v5 checks comprise 88 verified processes and 488 trials including warmups. Actual protocol IDs, source commitments, complete proof-content digests and payload sizes match across baseline/cleanup and all tested worker counts for each fixture. These digests cover deterministic Debug content, not a portable wire serialization.

## Final measurements

Final arithmetic comes from `retained-performance`; full comes from `recovered-full-performance`. Positive values mean higher cost. The earlier full rows of `retained-performance` are superseded.

| Variant | Workers | Total change [95% interval] | Prove-only | Verifier | Lifetime RSS |
|:--|--:|--:|--:|--:|--:|
| arithmetic | 1 | +0.187% [-0.118, +0.557] | +0.242% | +0.319% | +0.859% |
| arithmetic | 4 | +0.229% [-0.273, +0.829] | +0.092% | +0.879% | -0.109% |
| arithmetic | 8 | -0.046% [-0.285, +0.147] | -0.042% | +0.641% | -0.137% |
| arithmetic | 16 | +0.299% [-0.127, +0.838] | -0.009% | +0.261% | -0.231% |
| full | 1 | -0.128% [-0.247, +0.043] | -0.112% | +0.010% | -0.010% |
| full | 4 | -0.223% [-0.394, -0.070] | -0.270% | -0.019% | +0.472% |
| full | 8 | +0.026% [-0.078, +0.134] | +0.093% | -0.352% | +0.180% |
| full | 16 | -0.216% [-0.678, +0.395] | -0.145% | +0.456% | -0.163% |

Total-time point estimates range from −0.223% to +0.299%. Full 4-worker total/prove-only improves under these intervals; all other total/prove-only intervals cross parity. This establishes no conclusive prover slowdown in the tested matrix, but does not prove exact nonregression.

Full 16-worker verification measures +0.456% [0.268%, 0.675%], while full 8-worker verification measures −0.352% [−0.554%, −0.139%]. All other verifier intervals cross parity. A fixed five-seed 16-worker follow-up is recorded separately below; the positive original result remains visible. All full RSS intervals cross parity; full 4-worker is +0.472% [−0.561%, 1.516%]. Arithmetic 8-worker RSS is −0.137% [−0.238%, −0.019%]; other arithmetic RSS intervals cross parity. The evidence does not establish universal verifier or peak-memory nonregression.

Timing changes are geometric means of seed-paired process-median ratios. Five process pairs, rather than 25 iterations, are the independent units. Intervals are descriptive 95% paired bootstraps on one host and are not adjusted for multiple comparisons. Intervals crossing parity do not prove equality. Total time includes witness/commit and prove time. RSS is the maximum lifetime high-water mark including setup, allocator retention and proof-digest formatting, so removing one 128 KiB allocation does not itself establish a measurable peak-RSS reduction.

## Fixed verifier follow-up

The positive full 16-worker verifier result triggered a fixed five-seed follow-up,
registered in `verifier-followup-plan.md` before execution: seeds 65–69, order seed
20261014, identical binaries, affinity, warmup and sample counts. All 10 processes
and 60 trials passed verification and proof-identity checks. Combined final-source
functional coverage is **98 processes and 548 trials**, including the v5 checks.

| Metric at 16 workers | Original five pairs | Fresh five pairs [95% interval] | All ten pairs [95% interval] |
|:--|--:|--:|--:|
| Total proving | -0.216% | +1.200% [-0.328, +3.644] | +0.490% [-0.377, +1.805] |
| Prove-only | -0.145% | +0.960% [-0.143, +2.675] | +0.406% [-0.216, +1.369] |
| Verification | +0.456% | +0.202% [-0.052, +0.446] | +0.329% [+0.147, +0.504] |
| Lifetime RSS | -0.163% | +0.286% [+0.087, +0.495] | +0.061% [-0.154, +0.273] |

The original positive verifier result remains supported by the combined descriptive
interval: **+0.329% [0.147%, 0.504%]**. Strict verifier nonregression is therefore
not met in these observations. The held-out interval alone crosses parity. The
five-pair intervals enumerate all 5^5 resamples; the ten-pair intervals use 100,000
deterministic paired log-ratio resamples. The case was selected after the initial
result, so these are follow-up descriptions, not selection-adjusted guarantees.

The follow-up contains a +5.923% total-time pair at seed 65; it is retained without
exclusion. Combined total and prove-only intervals cross parity and are wider than
the original matrix's intervals. Thus “no conclusive prover slowdown” does not
mean a zero-cost bound. Follow-up RSS alone increases +0.286% [0.087%, 0.495%]; the
combined RSS interval crosses parity. Neither verifier nor memory tradeoffs should
be hidden by the allocation-count reduction. No source changes or additional
significance-driven sampling followed this fixed check.

## Provenance incident and recovery

During inspection, `objcopy --dump-section .text=<file> <binary>` was incorrectly treated as read-only. Without an explicit output executable, it rewrote both the frozen full baseline and the final full executable before `retained-performance`. The audit detected the changed baseline SHA-256. Its full measurements are preserved but superseded; arithmetic and v5 compatibility measurements were unaffected.

The original frozen full hash is `79dd92b01e5bc2f1d74305306c36fe9013862bffd5a41870d7ff881f50c7dbb2`; the rewritten baseline hash is `ff7174b92cbe9fa92b9c719063b7d9b00e18cdd948234e88861fec7b292af751`. The original source was rebuilt at the same path/toolchain/flags and matched the exact original hash before restoration. Final source was then restored and a fresh full candidate built with SHA-256 `ae69fcf83d379d220e998aec054384e7ad16be33029d7667c353d1ead6f6c835`. `provenance-recovery.json` records the successful recovery, commands, altered hashes and recovered hashes. The separate `recovered-full-performance` campaign repeats all four worker counts. No original raw result or historical manifest is rewritten.

## Evidence and limits

`retained-source-manifest.json` identifies final source. Test logs and `retained-unit-test-summary.json` retain commands and outcomes. Campaign manifests and CSV/JSON summaries preserve configurations, executable hashes and paired results. `investigation/` records the eight requested simplify dimensions and reconciled plan. Full raw process output, binaries, source backups and build logs remain locally under `.tmp/simplify/validation/`.

The requested `simplify-rust` and `rust-reviewer` workflows used reference searches, stable rust-analyzer, compiler checks, curated Clippy and independent source/evidence review. The optional similarity-rs, jscpd, ast-grep and cargo-machete tools were unavailable; no clean scans are claimed for them. Existing repository Clippy warnings remain. No unrelated original-checkout changes were included.

Curated text copies use LF line endings and one final newline; original raw logs remain unchanged in the ignored validation directory. `ARTIFACTS.sha256` hashes the curated copies.
