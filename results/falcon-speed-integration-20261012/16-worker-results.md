# Falcon results with 16 workers

The integrated implementation is faster than its frozen baseline for large
batches. No source changes were made for this measurement. All **164 pairs**
matched input digests, source roots, complete-proof debug digests, payload
sizes and breakdowns; all **2,496 proof executions verified**.

## Batch 1024

Total prover time includes witness derivation, packing, commitment and proving;
it excludes parameter setup and external input generation. Values below are
geometric means of per-seed medians, so each displayed reduction agrees with
the before/after comparison.

| Mode | Degree | Security | Baseline ms | Integrated ms | Total reduction | Proof-only reduction | Verification reduction |
|---|---:|---:|---:|---:|---:|---:|---:|
| arithmetic | 512 | 100 | 56.02 | 47.11 | 15.90% | 13.79% | 19.47% |
| arithmetic | 512 | 128 | 59.66 | 50.73 | 14.96% | 12.76% | 20.01% |
| arithmetic | 1024 | 100 | 110.78 | 92.07 | 16.88% | 13.33% | 21.02% |
| arithmetic | 1024 | 128 | 115.16 | 96.09 | 16.56% | 13.10% | 21.03% |
| full | 512 | 100 | 283.87 | 264.74 | 6.74% | 5.75% | 37.38% |
| full | 512 | 128 | 345.19 | 325.35 | 5.75% | 4.75% | 35.66% |
| full | 1024 | 100 | 541.94 | 504.17 | 6.97% | 5.16% | 44.65% |
| full | 1024 | 128 | 668.77 | 624.61 | 6.60% | 5.30% | 41.69% |

The arithmetic 512/128-bit row uses its longer four-seed confirmation, which
measured a 14.96% total reduction (paired-seed bootstrap 95% interval:
14.59–15.27%). The original three-seed screen gave 10.49%, with a visibly
slow candidate process in one seed. Other headline rows use the main matrix.

## Workload and placement

- AMD Ryzen 9 9950X3D, native release binaries with LTO and `falcon-hybrid`.
- Sixteen workers pinned to sixteen distinct physical cores across both CCDs,
  preserving the previous first-eight placement. No SMT siblings are used.
- Main thread and auxiliary workers retain the earlier policy of CPU 6 affinity.
- The matrix covers full/arithmetic modes, degrees 512/1024, security 100/128,
  batches 1/3/32/513/1024: 40 configurations, three seeds 42–44, one warmup and
  three measured proofs per variant. Baseline/candidate order alternates.
- Confirmation adds 32 small-arithmetic pairs and 8 small-full pairs with seeds
  45–52, three warmups and 15 samples; four large-arithmetic pairs use seeds
  45–48, three warmups and 9 samples. Campaigns ran sequentially, including
  report generation, with no overlapping builds or tests.

The initial tiny-batch arithmetic results were noisy, including pauses that
produced large apparent changes in opposite directions. Longer confirmation
found near-neutral proof times and neutral-to-faster total times. In particular,
the 128-bit batch 1 proof changes were +0.22% for degree 512 and −0.15% for
degree 1024, with paired-seed intervals including zero. The apparent 2.4% total
gain at degree 512/security 100 is influenced by one slow baseline process,
so it should not be treated as a robust small-case speedup.

The longer full-Falcon confirmation **did retain a small-case regression**:
degree 1024, batch 1, security 100, sixteen workers. Total prover time increased
**2.82%** (paired-seed bootstrap 95% interval +1.38% to +4.20%), from
16.731 to 17.203 ms by paired-seed geometric means. Proof-only time increased
3.53%, and verification increased 4.17%. Six of eight seeds showed sustained
slowdowns; this should be treated as an observed tradeoff rather than dismissed
as isolated pauses. The process RSS also changed for those seeds, but neither
that observation nor these timings establishes the cause.

Peak RSS is a process-lifetime high-water mark, not per-proof allocation.
For arithmetic degree 1024/batch 1024, the candidate retained approximately
7–8 MiB more RSS than baseline (about 3.5–3.8%) across the main seeds.

## Relation to the earlier eight-worker measurements

These compare the same frozen candidate and input seeds 42–44 at batch 1024,
security 128; they were measured at different times.

| Mode | Degree | Eight workers ms | Sixteen workers ms |
|---|---:|---:|---:|
| arithmetic | 512 | 37.20 | 53.31 |
| arithmetic | 1024 | 69.04 | 96.09 |
| full | 512 | 405.76 | 325.35 |
| full | 1024 | 781.50 | 624.61 |

Arithmetic currently enables its optimized `BindingForm` strategy only for
one/eight workers (`arithmetic.rs`, `optimized_binding`). Sixteen workers use
the existing legacy preparation/evaluation strategy, while other word-binding
stages still parallelize. Full Falcon uses a separate path. Therefore the
arithmetic comparison includes a strategy change and is not pure thread-count
scaling. The sixteen-worker configuration also spans both CCDs; these results
do not isolate how much each factor contributes.

## Evidence

- [Complete 40-configuration matrix](qualification-16workers/report.md)
- [Small arithmetic confirmation](confirmation-16workers-small-arithmetic/report.md)
- [Large arithmetic confirmation](confirmation-16workers-large-arithmetic/report.md)
- [Small full-Falcon confirmation](confirmation-16workers-small-full/report.md)
- [Machine-readable headline results](16-worker-summary.json)

All 729 recorded source hashes still match the tested candidate, and all four
frozen executable hashes match their original manifests. These are measured
results for this host and finite input sample, not a universal nonregression
guarantee. Raw logs, per-process commands, exact affinity checks and paired
results are retained in each campaign directory.
