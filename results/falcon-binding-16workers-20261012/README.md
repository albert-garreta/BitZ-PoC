# Falcon arithmetic binding at 16 workers

The existing optimized binding preparation now also applies at 16 workers. It
factors the equality weights and fills independent signature slices in parallel,
instead of constructing a full equality table and filling coefficients serially.
The same policy enables parallel per-instance verifier evaluation.

The batch thresholds remain unchanged: Falcon1024 starts at batch 9, Falcon512 at
batch 17. Other worker counts retain their previous policy. GKR, the public API,
proof structure, and transcript are unchanged. The implementation and test changes
for this task are recorded in `task.patch`.

## Production results

These compare production builds with benchmark overrides disabled. Both include
the current ring improvements and the separate first-word-fold optimization. The
only source difference between the two builds is whether 16 workers qualify for
optimized binding preparation. The source base is `7fbadd3da`; the older
`strategy-*` experiments used `b30f5e445` and are reported separately.

Batch 1024, 16 workers, four fresh seeds (45–48), three warmups and fifteen
measured proofs per process. Times are geometric means of per-seed medians.
Total prover time includes witness preparation/commitment and proving; it excludes
parameter setup and external input generation.

| Degree | Security | Total before ms | Total after ms | Total reduction | Proof-only reduction | Verification reduction |
|---|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 45.85 | 36.16 | 21.15% | 22.04% | 53.97% |
| 512 | 128 | 49.74 | 40.13 | 19.33% | 19.98% | 51.19% |
| 1024 | 100 | 90.00 | 66.64 | 25.95% | 27.10% | 57.71% |
| 1024 | 128 | 94.29 | 70.64 | 25.08% | 26.10% | 56.76% |

For Falcon1024/security128, proof-only time falls from 90.39 to 66.80 ms.
These improvements compare the two 16-worker implementations; they do not imply
that 16 workers outperform 8 workers.

A separate instrumented diagnostic at Falcon1024/batch1024/security128/16 workers
locates the improvement:

| Inclusive stage | Before ms | After ms |
|---|---:|---:|
| Prover coefficient preparation | 27.25 | 2.71 |
| Entire binding stage | 36.71 | 13.14 |
| Verifier coefficient preparation | 26.68 | 2.61 |

Preparation is nested inside binding; these times must not be added. Tracing
overhead means these numbers are localization evidence, not headline latencies.

## Correctness and regression checks

- All 304 Falcon library tests passed, including coefficient/reference comparisons,
  noncanonical input rejection, and complete-proof equivalence at 1/2/8/16 workers.
  A proof produced with 16 workers is also verified in the legacy 2-worker pool.
- Final production qualification contains 272 pairs and 5,440 verified warmup/sample
  proofs. Every pair matches input digests, commitment roots, complete-proof Debug
  digests, payload sizes and breakdowns, protocol IDs, and build flags. Eight
  separately traced proofs also verified and matched.
- The 16-worker matrix covers both degrees, both security targets, and batches
  1/7/8/9/15/16/17/32/33/65/513/1024. All 90 pairs in the 30 configurations affected
  by the policy change improved proof, total-prover, and verification time.
- Controls cover 1 and 8 workers at batches 1/17/1024. Longer confirmations resolve
  the small adverse signals: the extended 8-worker Falcon512/b17/s100 control
  measures +0.048% total time, with a paired-seed bootstrap 95% interval of
  -0.566% to +0.645%. The unchanged 16-worker Falcon512/b16/s100 control measures
  +0.354%, with an interval including equality. No material repeatable timing
  regression remains in the measured cases.
- A small peak-RSS increase at Falcon1024/b9/s128 did not reproduce. One b65/s128
  confirmation seed has +3.108% RSS; its aggregate is +0.385%, with an interval
  of -0.718% to +2.157%. RSS is a process-lifetime high-water mark, not per-proof
  allocation. Large-batch aggregate RSS changes range from -0.37% to +0.05%.

The Debug digest checks every proof field within these builds; it is not a
wire-codec serialization claim. Finite measurements on this host cannot guarantee
performance on other machines or inputs. Bootstrap intervals resample paired
seed ratios, and the host was not reserved exclusively.

## Reproduction and provenance

CPU: AMD Ryzen 9 9950X3D. Builds use Rust 1.98.1, release/LTO, one codegen unit,
`falcon-hybrid`, and `RUSTFLAGS=-C target-cpu=native`, without `bench-internals`.
Runs are sequential with alternating variant order. Sixteen workers use distinct
physical cores `6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9`; the main thread and auxiliary
workers use CPU 6. Builds and tests finish before timing starts.

`compare.py` reuses the previous integration qualification checker. To reproduce
the headline campaign with rebuilt, frozen binaries:

```sh
python3 -B results/falcon-binding-16workers-20261012/compare.py \
  --baseline-binary /path/to/baseline --candidate-binary /path/to/candidate \
  --baseline-strategy default --candidate-strategy default \
  --build-description 'release; features=falcon-hybrid; RUSTFLAGS=-C target-cpu=native' \
  --degrees 512 1024 --batches 1024 --securities 100 128 --threads 16 \
  --seeds 45 46 47 48 --warmup 3 --iterations 15 \
  --output results/falcon-binding-16workers-20261012/reproduction
```

- `production-build.json` records the source base, snapshot, flags, and executable
  hashes. The paired source manifests differ only in `arithmetic.rs`.
- `production-candidate.patch` reconstructs the tested candidate on the recorded
  base, including the pre-existing first-fold work. Removing only `16` from the
  selector produces the benchmark baseline. `task.patch` isolates this task's work.
- `test-provenance.json` records the reused test executable and matching core
  source hashes. All library/dependency sources match the candidate; the root
  manifest differs only by an unrelated example registration. Tests were run
  afresh; their output is in `falcon-tests.log`.
- `production-matrix`, `production-controls`, `production-large-confirmation`, and
  `confirmation-*` retain per-process commands, environment, raw results, and
  summaries. `production-traces` retains the separate diagnostic.
- `headline-results.json` and `validation.json` contain aggregate results and
  counts. The 563-file final source audit matches the workspace except for its
  unrelated `Cargo.toml` example registration.
- The initial `strategy-*` experiments contain 78 pairs and 1,404 verified proofs.
  Their noisy observations are preserved, but fresh production confirmations
  establish the reported gains. Untimed cache-priming proofs are excluded from
  all headline proof counts.
