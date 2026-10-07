> Historical notes from before the SharedPrime-only cleanup. These APIs,
> domains, and measurements do not describe the current implementation.

# Native Falcon throughput optimizations (v3)

For the subsequent optimizations that preserve this protocol and its exact
proof transcripts, see [KERNEL_THROUGHPUT.md](KERNEL_THROUGHPUT.md).

Implemented the six planned changes. The matched seed-42 benchmark improves
from **2.311 to 1.169 ms/signature**,
from 432.8 to **855.7 signatures/second**,
or **49.4% less proving time**.
The 1,000 signatures/second target has **not** been reached.

## Measurement conditions

AMD Ryzen 9 9950X3D, 16 Rayon workers, 1024 distinct original Falcon-1024
signatures, target 128, native release (`-C target-cpu=native`, opt3, fat LTO,
one codegen unit), default thread stacks. Each executable/input pair has one
warmup and three measured trials; values are medians. Builds and tests did not
overlap benchmarks. Run order was baseline/new at seed42, new/baseline at
seed43, and baseline/new at seed44.

Baseline is the saved v2 executable from commit `e74bda6e`. Full proving
includes witness generation, commitments, SHAKE, HashToPoint, native ideal
certificate/projection/carries, every sumcheck and grinding boundary, and the
shared opening. Signing/input generation, upstream validation and decoding,
reusable preparation, proof verification, and Debug hashing stay outside prover
timing, identically for both versions. These are batch-amortized times;
single-signature proof latency is reported separately below.

| Metric, seed42 | Saved v2 rerun | Optimized v3 |
| --- | ---: | ---: |
| Full proving, ms/signature | 2.311 | 1.169 |
| Signatures/second | 432.8 | 855.7 |
| Verification, ms/batch | 72.365 | 71.423 |
| Process peak RSS, GiB | 5.001 | 4.529 |
| Stored proof payload, bytes | 2,440,260 | 2,439,524 |
| Reported work-normalized security bits | 129.365218 | 129.375910 |

The payload excludes Falcon transport framing and the public statement; there
is still no Falcon wire codec. Merkle-opening size can vary with transcript
queries. Input digests and all three source commitment roots match across both
versions for every matched case. Witness/constraint counts and the virtual
binary domain are unchanged. New proof transcripts differ from v2.

## Input-seed variation

Nonce searches are deterministic for a fixed statement and transcript;
repeating a seed measures runtime noise, not fresh nonce-search variability.
These three predetermined seeds therefore complement the within-seed trials.

| Seed | v2 ms/signature | v3 ms/signature | Less prover time |
| --- | ---: | ---: | ---: |
| 42 | 2.311 | 1.169 | 49.4% |
| 43 | 1.900 | 1.223 | 35.6% |
| 44 | 1.661 | 1.230 | 25.9% |

The median of the three per-seed medians changes from 1.900
to 1.223 ms/signature (817.9 signatures/second).
The optimized range is 1.169–1.230 ms/signature, so the conclusion
does not rely on the fastest seed. Seed42's 49.4% improvement is not a claim
about every input.

## Implemented changes

1. Remove the unnecessary batch factor in the fingerprint bound, using a fixed
   incorrect ordered-compaction instance selected before the challenge.
2. Use equality weights for forest batching and the nonzero error-vector bound
   for common line reductions. Allocate difficulties against the exact saved
   v2 budget. At batch1024 cubic/forest/fingerprint settings are18/17/19 bits;
   partial batches use20 fingerprint bits to preserve prior rounding margin.
3. Stream ring-switch marginal accumulation from source branches, avoiding the
   full equality table and early virtual witness copy. Fuse packed/basis/padding
   construction with initial sumcheck and lookahead messages. Existing Ligerito
   continuation still requires full packed and basis vectors afterward.
4. Keep only live binary lanes through the first folds (`11 -> 6 -> 3 -> 2`),
   while retaining the full16-lane domain and physical-padding authentication.
   Retained binary scratch falls from768 to544MiB at batch1024.
5. Group arithmetic prefix work by witness byte and directly replay folded
   word coefficients. Signed, ordinary and bounded14 decoders stay identical;
   shared templates, cached weights, and generic/small-batch fallbacks remain.
6. Parallelize norm and compaction preparation. Accumulate coefficient squares
   exactly as bounded integers before applying a common signature weight;
   reuse the shared rank-fingerprint table across signatures.

## Security

The complete reported bound improves from 129.365218 to
129.375910 bits under the repository's existing computational
PoW/Fiat–Shamir convention. This is not unconditional statistical128-bit
soundness or a new audit of that convention. Every batch1..1024 is checked
against v2 at both supported targets; changed group comparisons use exact
integer arithmetic. All native ideal/carry, norm, HashToPoint, SHAKE, source,
bridge, PCS, and individual compaction-root checks remain in place.

The proof/statement domains are versioned to v3; forest domains identify the
new equality batching. Earlier proofs must be regenerated. See
[COMPACTION_SOUNDNESS.md](../COMPACTION_SOUNDNESS.md) for the fixed-source and
nonzero-vector arguments, challenge order and rounding rules, and
[OPTIMIZATION_SECURITY.md](../OPTIMIZATION_SECURITY.md) for model assumptions.

## Smaller batches

These use seed42 and the same thread/security/build/trial conditions. Small
batches retain substantial fixed proof overhead and are sensitive to transcript
nonce changes; the result is not a universal speedup at every batch size.

| Batch | v2 ms/signature | v3 ms/signature |
| --- | ---: | ---: |
| 1 | 44.704 | 44.854 |
| 3 | 20.883 | 21.623 |
| 32 | 3.821 | 2.974 |
| 1024 | 2.311 | 1.169 |

## Separate stage profiles

Single cold diagnostic runs, seed42, milliseconds per signature. Grinding is
nested inside other stages and must not be added to them. These profiles are
separate from the warm matched medians and do not isolate each source change
from transcript-dependent effects.

| Stage | v2 | v3 |
| --- | ---: | ---: |
| All grinding (nested) | 0.8999 | 0.0496 |
| Witness and commitments | 0.1527 | 0.1596 |
| Norm | 0.0730 | 0.0242 |
| Rejection sumcheck | 0.0193 | 0.0172 |
| Compaction forest | 0.2472 | 0.0467 |
| Candidate leaf authentication | 0.0528 | 0.0580 |
| Native ideal/projection/carries | 0.0100 | 0.0111 |
| Arithmetic binder | 0.2694 | 0.2012 |
| Integer-to-binary bridge | 0.2246 | 0.2149 |
| Keccak prefix | 0.1078 | 0.1037 |
| Joint binary authentication | 0.1870 | 0.1545 |
| Shared PCS opening | 0.2803 | 0.2223 |

Binder prefix accumulation decreases from0.1279 to0.0632 ms/signature. Tail
preparation changes only from0.0750 to0.0705; the direct-word replay did not
remove that bottleneck. Norm preparation and lower grinding dominate the other
large gains. The next substantial kernels are the shared PCS opening, bridge,
and arithmetic binder, each around0.20–0.22 ms/signature in the diagnostic run.

Reaching1ms requires another14.4% reduction from the seed42 median, or18.2%
from the three-seed median. The next candidates are fused bridge forest passes,
further PCS buffer/pass elimination, and binder tail construction/folding.
Those are remaining optimization opportunities, not measured gains. Any further parameter change must preserve the complete
security bound and all authentication obligations.

## Validation and reproducibility

- Full library suite:659 passed,7 ignored. The three subsequently added forest
  algebra/compaction reference tests passed in the16-test PIOP rerun, giving
  **662 distinct passing library tests**.
- Three upstream Falcon integration tests passed; serial Falcon and hybrid builds passed.
- All48 matched timing proofs and two diagnostic proofs verified across
  batches1,3,32,1024 and seeds42,43,44.
- Exact schedule/group/fingerprint and full-ledger regressions cover every
  supported batch at targets100 and128. Existing altered-root, false-native-
  certificate, carry, norm, ordering, padding, and hash-link tests pass.
- Dense-reference parity covers grouped binder messages/transcripts, signed
  and bounded folded words, all65,535 nonempty lane supports, physical padding,
  N=1/2/3/4 binary source geometry, complete shared-opening proof bytes and
  verifier acceptance with both UDR and Johnson/OOD configurations.
- Independent reviews checked the soundness argument and all three optimized
  prover kernels. They identified the partial-batch rounding issue, which was
  fixed before testing and benchmarking.

Raw commands, binary/source hashes, proof digests, timing trials, stage events,
validation logs and source diff are retained in
`bench_results/falcon-throughput-20260926/`.

```sh
CARGO_TARGET_DIR=target/falcon-native RUSTFLAGS='-C target-cpu=native' \
  cargo bench --offline --profile release --features falcon-hybrid \
  --bench falcon_hybrid -- --batch 1024 --security 128 --seed 42 \
  --threads 16 --warmup 1 --iterations 3
```

Use seeds43 and44 for the additional comparisons. Enable
`BITZ_FALCON_STAGE_TIMINGS=1` only for separate diagnostic runs.
