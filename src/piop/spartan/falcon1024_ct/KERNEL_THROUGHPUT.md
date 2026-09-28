# Falcon prover kernel optimizations, 2026-09-27

This records the earlier kernel pass. The subsequent SIMD, arithmetic replay,
nonce-hashing and factored-tensor work is in
[SIMD_THROUGHPUT.md](SIMD_THROUGHPUT.md).

All four follow-up optimizations are implemented without changing the v3
protocol. The matched seed-42 run improves from **1.195 to 1.047 ms/signature**,
or **837 to 955 signatures/second**, including witness generation and
commitments. This is **12.4% less proving time**. The 1,000 signatures/second
objective is **not yet reached**.

## Matched measurements

AMD Ryzen 9 9950X3D, 16 Rayon workers, 1,024 distinct original Falcon-1024
signatures, target 128, native release (`-C target-cpu=native`, opt3, fat LTO,
one codegen unit). Each executable/input pair has one warmup and three measured
trials; values below are medians. Baseline is the saved executable from
`68d530b3`. No builds or tests overlapped benchmarks. Baseline cases were run
first, followed by optimized cases; an additional consecutive baseline/optimized
pair checks the noisier seed-43 result.

Full proving includes witness generation, commitments, SHAKE, HashToPoint,
native ideal certificates/projections/carries, all sumchecks, grinding and the
shared opening. Input generation, upstream validation/decoding, reusable
preparation, verification and Debug hashing remain outside prover timing,
identically for both executables. These are batch-amortized times, not the
latency of an individual proof.

| Metric, seed 42 | Saved v3 rerun | Optimized v3 |
| --- | ---: | ---: |
| Full proving, ms/signature | 1.194902 | 1.046769 |
| Signatures/second | 836.9 | 955.3 |
| Verification, ms/batch | 72.317 | 71.014 |
| Process peak RSS, GiB | 4.540 | 3.532 |
| Stored proof payload, bytes | 2,439,524 | 2,439,524 |
| Reported work-normalized security bits | 129.375910 | 129.375910 |

Peak RSS decreases by about 22%. Proof-only throughput is 1,093 signatures/sec,
but that excludes witness generation/commitments and therefore does **not**
satisfy the full-prover target. Stored payload excludes public statements and
transport framing; Falcon still has no wire codec.

| Input seed | Baseline ms/signature | Optimized ms/signature | Less prover time |
| --- | ---: | ---: | ---: |
| 42 | 1.194902 | 1.046769 | 12.4% |
| 43 | 1.200963 | 1.147428 | 4.5% |
| 44 | 1.173335 | 1.042112 | 11.2% |

The median of the three per-seed medians is 1.046769 ms/signature, versus
1.194902 for the baseline. A separate seed-43 paired repeat measured
1.200709 → 1.077371 ms/signature (928.2 signatures/sec). The original result
remains in the table rather than selecting the faster repeat. Fixed-seed nonce
searches are deterministic; this repeat measures runtime variation, not new
grinding luck. Another 4.5% reduction from the seed-42 result would reach 1 ms;
the slower seed-43 measurements require more headroom.

## Implemented changes

1. **Reuse folded binary gather coefficients.** After seven binary prefix
   rounds, replay the retained word coefficients instead of walking all
   original bit entries and folding them again. Sorted word indices preserve
   repeated signature axes, live subcubes, overlaps and logical bounds. Physical
   padding and the global 16-lane opening domain remain authenticated.
   For the current batch-1024 gather layout, this pass visits 694,272 word
   entries instead of 107,184,128 bit entries; this is not a whole-prover
   operation-count reduction.
2. **Cache folded witness bytes in the arithmetic binder.** After the three
   prefix challenges, build all 256 weighted byte sums once in a 4 KiB table.
   Tail preparation and its first fold use that table, including masked final
   blocks. Other prefix widths retain the generic arithmetic path. Ordinary,
   signed and bounded14 decoders are unchanged.
3. **Use four-lane VPCLMUL bridge kernels.** Vectorize dense weighted sums,
   fused folds, JIT folds/products, scaling and contractions. Keep scalar tails,
   tiny inputs and irregular bucket updates. Compile the backend only when all
   required x86 ISA features are enabled; other builds retain their supported
   backend. Native builds exercise the actual SIMD code in differential tests.
4. **Stream PCS messages and defer large tables.** Stream the initial message
   and lookahead over reusable tiles. Preserve both existing challenge/grinding
   boundaries, then materialize the witness and basis after two folds. At
   batch 1,024, their combined logical size is 128 MiB rather than 512 MiB.
   Compile the folded ring projection as an F2-linear map; contract padding
   masks and OOD terms exactly, even where witness lanes are empty. Vectorize
   lookahead accumulation and two-round folding. Unsupported small geometries
   retain the materialized path.

## Protocol and security invariants

No constraint, witness column, challenge, grinding difficulty, verifier check,
field parameter or transcript domain was removed or changed. This remains
`native-ring/non-zk/v3`. In particular, the arithmetic source still contains
100,578 live bits and 131,072 padded bits per capacity slot, with 8,605 auxiliary
arithmetic values per live signature. Norms, per-signature compaction roots,
native ideal/carry relations, SHAKE links and all source-padding obligations
are unchanged.

Every matched benchmark case has the **same complete proof Debug digest**,
source roots, input digest, stored payload and reported security bound as its
baseline case. Reference tests also compare exact sumcheck messages and entire
serialized shared-opening proofs/transcripts, not just verifier acceptance.
These provide regression evidence for the algebraic equivalences; they are not
a new security proof or independent audit.

The unchanged 129.375910-bit report uses the repository's computational
grinding/Fiat–Shamir convention described in
[OPTIMIZATION_SECURITY.md](OPTIMIZATION_SECURITY.md) and
[COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md). It is not a claim of
unconditional statistical 128-bit soundness. Prover kernels process public
signature statements and are not claimed to be constant time.

## Remaining costs

Separate cold diagnostic runs, seed 42, milliseconds per signature. These are
not the warmed benchmark medians. Nested rows must not be added to their parents.

| Stage | Baseline | Optimized |
| --- | ---: | ---: |
| Witness and commitments | 0.1540 | 0.1531 |
| Norm | 0.0240 | 0.0235 |
| Rejection | 0.0166 | 0.0155 |
| Compaction forest | 0.0464 | 0.0441 |
| Candidate leaf authentication | 0.0594 | 0.0590 |
| Native ideal/projection/carries | 0.0110 | 0.0106 |
| Arithmetic binder | 0.2029 | 0.1729 |
| Binder tail preparation (nested) | 0.0702 | 0.0582 |
| Binder tail folding (nested) | 0.0572 | 0.0398 |
| Integer-to-binary bridge | 0.2222 | 0.2053 |
| Keccak prefix | 0.1073 | 0.1078 |
| Joint binary authentication | 0.1535 | 0.1229 |
| Shared PCS opening | 0.2172 | 0.1422 |

The new PCS profile splits into 0.01285 ms for ring switching, 0.01611 ms
for initial basis/messages and 0.11320 ms for continuation, including 0.00659 ms
for constructing the folded tables. PCS grinding alone is 0.06680 ms/signature
inside continuation. Other Spartan grinding totals 0.04945 ms/signature,
nested in the other stages. The bridge and binder remain the largest kernels;
this change does not claim another unmeasured speedup there.

One concrete follow-up is the bridge's first JIT-round bucket accumulation,
which still uses scalar unreduced products. A code-level count at batch 1,024
gives 33,554,432 slots and 67,108,864 such products across the four bottom
levels. Four-lane products followed by collision-safe XOR scattering could
accelerate this pass, but its isolated runtime and resulting gain have not
been measured. Duplicate bucket indices require differential coverage before
replacing the reference path.

## Smaller batches

Same seed, threads, target, build and trial conditions. Small batches retain
fixed proof overhead and do not improve consistently.

| Batch | Baseline ms/signature | Optimized ms/signature |
| --- | ---: | ---: |
| 1 | 44.390 | 44.693 |
| 3 | 21.207 | 21.697 |
| 32 | 3.052 | 2.878 |
| 1,024 | 1.195 | 1.047 |

## Validation and reproduction

- 668 distinct portable library tests passed, with seven ignored. The full run
  passed 667; one new test initially requested an unsupported tiny PCS geometry.
  After correcting that fixture to the minimum supported size, all six opening
  tests passed in the focused rerun. No production fix was needed.
- Eleven native bridge tests, six native opening tests and the native vendor
  lookahead oracle passed. Coverage includes scalar/SIMD parity, offset slices,
  vector tails, task boundaries, nonzero physical padding, unused lanes,
  Boolean/random folds, and UDR/Johnson-OOD proof/transcript equivalence.
- Three upstream integration tests and both serial Falcon/hybrid build checks
  passed. The modified Rust files pass formatting checks.
- All 58 benchmark proofs verified: 48 original matched trials, two diagnostic
  proofs and eight proofs in the additional seed-43 pair. Each pair has exact
  proof-digest, commitment-root, input, payload and security equality.
- Independent reviews checked the gather replay, binder cache, SIMD bridge,
  PCS lookahead, deferred continuation and exact folded basis construction.

Raw commands, executable hashes, trials, proof digests, peak RSS, stage events,
validation logs and source snapshots are retained in
`bench_results/falcon-1000-20260927/`. The release executable was built before
the final test-only fixture adjustment; production source is identical.

```sh
CARGO_TARGET_DIR=target/falcon-native RUSTFLAGS='-C target-cpu=native' \
  cargo bench --offline --profile release --features falcon-hybrid \
  --bench falcon_hybrid -- --batch 1024 --security 128 --seed 42 \
  --threads 16 --warmup 1 --iterations 3
```

Repeat with seeds 43 and 44. Enable `BITZ_FALCON_STAGE_TIMINGS=1` only for
separate diagnostic runs. See [THROUGHPUT.md](THROUGHPUT.md) for the preceding
protocol-v3 changes and their comparison with v2.
