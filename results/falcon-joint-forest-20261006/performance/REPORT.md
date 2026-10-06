# Joint HashToPoint forest: matched V3/V4 performance

The joint forest substantially reduces large-batch proof payloads, with a
modest 100-bit proving improvement in this sample and inconclusive 128-bit
proving cost. Peak memory is effectively unchanged. Small batches have much
less size benefit and do not show a general speed improvement.

## Method

Measured on `will`, AMD Ryzen 9 9950X3D, Linux x86_64, 16 Rayon threads, allowed
CPUs 0–31 (unpinned), Rust 1.98.1, release/fat LTO, `-C target-cpu=native`,
`falcon-hybrid`. The preserved V3 binary's source revision has identical build
inputs to pulled HEAD `3ca99b2142bc3dc9be70f1b5a5c89bd6d1cc9296`; intervening
changes are documentation and result artifacts. V4 is the uncommitted working
tree identified by source and binary SHA256 hashes in [manifest.json](manifest.json).
Both builds use the same Cargo.lock, CPU features, target-selected prime
families, source geometry, and integer bridge.

Each security target (100/128) and batch (1/32/1024) uses ten distinct seeds,
42–51. Each version runs in a separate process with one warmup and three
measured proofs. There are five V3-first and five V4-first pairs per cell,
across two five-seed invocation blocks. Benchmark processes run sequentially
under the shared host gate, with at least 88% idle before each process; no
swap growth occurred. Instrumented profiles run separately from latency.

Tables report the median of the ten per-seed process medians. Paired ratios
are geometric means of V4/V3, with deterministic 10,000-resample bootstrap
95% intervals over seed log ratios. These are exploratory, per-metric
intervals, not simultaneous performance qualification bounds. Repeating a
proof within a process measures timing noise; different seeds exercise
different transcripts and grinding outcomes.

All 488 generated proofs verified (480 in latency runs, four fresh-process
memory runs, four instrumented runs). Input digests, workload/build metadata,
and commitment roots matched in every pair. Cold/instrumented proof digests
and payloads matched their corresponding latency proofs. Binary and candidate
source hashes were checked again after measurement.

## Batch 1,024

| Target | Metric | V3 | V4 | Paired change, 95% interval |
| --- | --- | ---: | ---: | --- |
| 100 | Commit + prove | 737.29 ms | 710.63 ms | −3.0% [−6.3%, −0.4%] |
| 100 | Verify | 58.95 ms | 57.29 ms | −1.6% [−5.9%, +2.9%] |
| 100 | Stored payload | 1,604,466 B | 887,314 B | −44.6% |
| 128 | Commit + prove | 934.13 ms | 949.02 ms | +2.7% [−1.6%, +8.1%] |
| 128 | Verify | 62.86 ms | 61.34 ms | −3.0% [−4.9%, −1.2%] |
| 128 | Stored payload | 2,013,474 B | 1,297,834 B | −35.5% |

Prover time includes witness creation/commitment and proof generation;
upstream fixture key/signature generation and preparation are outside this
metric. Payload is the repository's canonical stored-payload accounting,
excluding Falcon transport framing and the public statement. It is not a
serialized wire-format length. Payload varies with transcript-dependent PCS
messages: V4 ranges are 884,114–892,466 B (100) and
1,294,874–1,301,402 B (128).

The earlier seed-42 measurements reproduce exactly: 892,466 B and
1,294,874 B. V3 seed-42 payloads are 1,601,794 B and 2,015,090 B. This is a
comparison against **V3 at matched security parameters**; the historical
1,751,402 B V2 measurement used a different 128-bit field/column profile.

The forest field-message accounting also checks independently of whole-proof
PCS variation: 723,536 B becomes 6,016 B, including the final 32-byte side
split and excluding grinding nonces, a 99.17% reduction. The existing
1,024-signature forest test checks 176 rounds and the exact byte count.

## Forest cost and memory

Separate, instrumented seed-42 trials show:

| Target | Forest time V3 → V4 | Of which grinding V3 → V4 |
| --- | ---: | ---: |
| 100 | 39.59 → 36.02 ms | 0 → 0 ms |
| 128 | 45.79 → 53.35 ms | 3.29 → 9.83 ms |

These single-trial profiles diagnose the tradeoff; they are not additional
samples for the latency intervals. Position rounds now compute a quadratic
weighted product with seven field multiplications per entry, compared with
ten for the preceding cubic equality-product accumulation, excluding table
folding and setup. The additional signature/side rounds act on small folded
tables. At 128 bits, the extra grinding is a real cost.

The ten-seed mean forest nonce-prefix count rises from 9.42 million to
24.57 million. The unchanged PCS folding schedule also happened to draw
more work in the V4 sample: 428.33 million versus 315.02 million mean nonce
prefixes. These are stored `nonce+1` sums, **not executed hash counts**; they
exclude SIMD/parallel overscan. They explain why whole-proof timing cannot
be attributed entirely to the forest. For example, seed 50's 128-bit V4/V3
prover ratio is 1.23, with PCS folding prefixes 643.19M versus 241.34M.
The 100-bit case has no corresponding grinding; its timing also exhibits
run-to-run variation, including a V3 seed-47 outlier. No outliers were
discarded. Ten seeds do not establish a tight 128-bit nonregression bound.

Fresh-process, uninstrumented, one-proof peak RSS (seed 42):

| Target | V3 | V4 |
| --- | ---: | ---: |
| 100 | 3,078,424 KiB | 3,078,784 KiB |
| 128 | 3,080,156 KiB | 3,076,360 KiB |

Both are approximately 2.94 GiB. The smaller proof does not materially reduce
the witness and working-table memory. Repeated-process latency RSS is not
used for this comparison because allocator retention can raise that peak.

## Smaller batches

| Target | Batch | Prover V3 → V4 | Paired prover change, 95% interval | Verify V3 → V4 | Payload V3 → V4 |
| --- | ---: | ---: | --- | ---: | ---: |
| 100 | 1 | 24.06 → 25.72 ms | +5.2% [−1.4%, +11.9%] | 15.95 → 16.69 ms | 207,218 → 205,026 B |
| 100 | 32 | 47.44 → 46.81 ms | −0.4% [−3.4%, +2.6%] | 18.50 → 19.15 ms | 430,002 → 407,618 B |
| 128 | 1 | 37.42 → 37.18 ms | −0.6% [−3.2%, +1.8%] | 16.99 → 16.97 ms | 242,866 → 241,754 B |
| 128 | 32 | 82.25 → 82.93 ms | +1.4% [−0.3%, +3.4%] | 20.28 → 20.51 ms | 522,738 → 503,378 B |

The 100-bit single-signature verifier shows a +4.0% paired change
[+0.6%, +7.4%]. Its prover result is inconclusive after extending from five
to ten seeds. At batch 32, payload savings are approximately 5.1% (100) and
3.7% (128); at batch 1, whole-proof savings are negligible. This supports
the change primarily as a large-batch size optimization. No single-thread,
other-host, native-profile, or memory-scaling performance claim is made.

## Reproduction and artifacts

The raw JSONL and process records are in `latency/`, `cold/`, and `stages/`.
Full precision statistics and paired samples are in [summary.json](summary.json).
[run.py](run.py) checks binary hashes, proof verification, ledger bounds,
workload metadata, and repeated-proof consistency; [summarize.py](summarize.py)
recomputes intervals and audits matching roots and diagnostic proofs.

Use binaries matching the manifest, and an empty output directory for a new
campaign; the runner refuses to overwrite process records. Run each command
under `scripts/bench_gate.py run --label NAME --min-idle 88 --hold-seconds 30
--poll-seconds 5 --swap-grow-gb 1 -- python3 PATH/run.py`:

```text
--seeds 42 43 44 45 46
--seeds 47 48 49 50 51
--batches 1 32 --seeds 42 43 44 45 46
--batches 1 32 --seeds 47 48 49 50 51
--mode cold --seeds 42
--mode stages --seeds 42
```

The actual execution logs are retained alongside this report; the final
small-batch confirmation used a 15-second initial idle hold. No Rust code was
changed during this performance check. The prior validation manifest retains
the 127 passing Rust tests and 66 passing Falcon script tests.
