> Historical notes from before the SharedPrime-only cleanup. These APIs,
> domains, and measurements do not describe the current implementation.

# Shared-prime Falcon V4

Historical forest protocol. Superseded by the [joint source commitment](../ONE_SOURCE.md);
current APIs select `FalconProtocol::SharedPrime` and reject earlier proofs.

`PreparedFalconHybrid::new_shared_prime(batch, target_bits)` selects
`SharedPrimeV4` for 1–1024 signatures. The native profile remains the default
and receives the same joint HashToPoint forest under its v5 transcript.
Both profiles require new proofs after this change.

V4 replaces the forest's per-tree child pairs with a joint sumcheck over
signature, candidate/output side, and position coordinates. It begins from
a randomly weighted sum of individual input/output root differences, keeping
each signature's ordered compaction obligation. Each multiplication layer
ends with one child pair. A weighted quadratic encoding sends two field
elements per round, and the final aggregate candidate/output claims both
reach the committed source bits. See [COMPACTION_SOUNDNESS.md](../COMPACTION_SOUNDNESS.md).

## Parameters retained from V3

| Target | Shared prime family (inclusive) | Column sums | Projection difficulty |
| --- | --- | --- | ---: |
| 100 | `2^114 ..= 2^115 - 2^102 - 1` | One unsplit `u128` | 2 |
| 128 | `2^125 ..= 2^126 - 1` | Two bounded `u128` limbs | 14 |

The ring proof, integer lift, committed source layout, bridge geometry, and
one shared PCS opening retain the V3 construction. The statement binds these
parameters and the revised arithmetic schedule. The compaction argument does
not provide zero knowledge.

## Component accounting

For batch 1024, the preceding forest's 11 layers each transmitted 2048 child
pairs: 720,896 bytes. Its 55 cubic rounds added 2,640 bytes in the compact
shared proof. V4 uses 176 weighted quadratic rounds and eleven child pairs:

```text
176 * 2 * 16 + 11 * 2 * 16 = 5,984 bytes.
```

The final scalar candidate/output split adds 32 bytes. This gives 6,016 field
message bytes versus the preceding forest's 723,536 bytes, a reduction of
717,520 bytes before nonces. The existing candidate-leaf sumcheck remains;
it uses the inherited signature point and removes the fresh instance draw.
Source authentication replaces the per-signature output claims with one
weighted claim. Payload counters also include all grinding nonces.

These are component counts. Complete proof sizes depend on the rest of the
proof and on transcript-dependent PCS messages; V2/V3 proof measurements
must not be relabeled as V4 results. The often cited 1,751,402-byte batch-1024
proof was a **128-bit V2** result in the
[V2 report](../../../../../results/falcon-shared-prime-v2-20261006/REPORT.txt).

## Security and work

The forest's raw error bound is `(d+2*R_forest+11)/p`, where
`d=log2(next_power_of_two(batch))` and `R_forest=11*(d+1)+55`.
Fingerprint, candidate-leaf, and source-binding terms remain separately
accounted. See [OPTIMIZATION_SECURITY.md](../OPTIMIZATION_SECURITY.md) for the
exact integer allocation and complete composition.

At target 100 the arithmetic rounds remain unground. At target 128 the
planner budgets quadratic forest rounds separately from cubic rejection and
leaf rounds. The resulting compaction group's expected nonce work at batch
1024 is about 1.95 times its previous count. This does not predict complete
proving time, field-operation cost, verifier time, or peak memory; those
require measurements of V4.

The current outer domains are `shared-prime/non-zk/v4` and
`shared-prime/statement/v4`. Native outer domains advance to v5. The parent
statement binds the signed-root weighted forest and every schedule field.

## Working-tree validation (2026-10-06)

The optimized Falcon suite passed 126 tests; two manual kernel benchmarks
remained ignored. The additional native exact-rational ledger test passed
separately. Both native and shared ledgers cover all 1–1024 batches at both
targets. Library/test/benchmark compilation with `falcon-hybrid`, library
compilation with `--no-default-features --features falcon`, and all 66 Falcon
Python script tests passed.

One full shared-prime proof at each target was generated and verified for
1024 distinct upstream Falcon signatures (seed 42, 16 threads, release build
with native CPU instructions, no warmup, one measured trial):

| Security target | Complete stored proof payload | Verified |
| --- | ---: | --- |
| 100 | 892,466 bytes | Yes |
| 128 | 1,294,874 bytes | Yes |

Payload excludes Falcon transport framing and the public statement, matching
the existing accounting. These single-trial runs establish proof sizes and
successful verification; they do not establish a performance regression or
speedup relative to V3. Raw records and working-tree build provenance are in
[the validation artifacts](../../../../../results/falcon-joint-forest-20261006/validation.json),
[100-bit proof](../../../../../results/falcon-joint-forest-20261006/shared-100-b1024.jsonl),
and [128-bit proof](../../../../../results/falcon-joint-forest-20261006/shared-128-b1024.jsonl).

## Matched performance check (2026-10-06)

A subsequent V3/V4 comparison used ten paired input seeds, one warmup and
three measured proofs per process, and balanced version order on an AMD
Ryzen 9 9950X3D with 16 threads. At batch 1024, median complete stored
payloads fell from 1,604,466 to 887,314 bytes (100 bits) and from 2,013,474
to 1,297,834 bytes (128 bits). Median commit-plus-prove times were
737.29 → 710.63 ms and 934.13 → 949.02 ms respectively.

The paired prover changes were −3.0% (95% interval −6.3% to −0.4%) at 100
bits and +2.7% (−1.6% to +8.1%) at 128 bits. These exploratory intervals
support a modest 100-bit improvement in this sample; 128-bit proving cost
remains inconclusive. A separate profile confirmed increased forest
grinding at 128 bits. Fresh-process peak RSS remained about 2.94 GiB.

Small batches showed much less size benefit and no general speed gain:
single-signature verification at 100 bits was about 4% slower. All 488
benchmark proofs verified. See the
[performance report](../../../../../results/falcon-joint-forest-20261006/performance/REPORT.md)
for full statistics, memory and stage profiles, limitations, and reproduction.
