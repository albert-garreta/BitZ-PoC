> Historical notes from before the SharedPrime-only cleanup. These APIs,
> domains, and measurements do not describe the current implementation.

# Falcon SIMD and streaming follow-up, 2026-09-27

**Follow-up:** [validation on twelve additional seeds](SEED_VALIDATION.md) found
that nine seed medians exceeded 1,000 signatures/sec. The gain holds in most
matched comparisons, but the broader results do not establish a consistent
1,000/sec rate. The original three-seed measurements below are retained.

This follow-up optimizes prover kernels while retaining the
`native-ring/non-zk/v3` protocol and the existing verifier. The comparison
baseline is commit `8a924fac`, with its release executable saved separately.

Full proving now measures **0.953 ms/signature, or 1,049 signatures/second**,
on the matched seed-42 workload, including witness generation and commitments.
That is **7.5% less proving time** than the saved baseline rerun. All three
measured seed medians exceed 1,000 signatures/second at the unchanged target-128
settings. The largest new gain comes from factoring cached Keccak tensor
weights.

## Matched results

| Input seed | Baseline ms/signature | Optimized ms/signature | Optimized signatures/sec | Less prover time |
| --- | ---: | ---: | ---: | ---: |
| 42 | 1.030302 | 0.953316 | 1,049.0 | 7.5% |
| 43 | 1.064826 | 0.979239 | 1,021.2 | 8.0% |
| 44 | 1.034719 | 0.970218 | 1,030.7 | 6.2% |

A consecutive baseline/optimized seed-42 repeat measured **1.026503 →
0.928775 ms/signature**, or 974.2 → 1,076.7 signatures/sec. The table retains
the original trials. Repeating a fixed input uses the same nonces, so this
checks runtime behavior without adding another grinding sample.

| Other metric, seed 42 | Baseline | Optimized |
| --- | ---: | ---: |
| Verification, ms/batch | 71.069 | 75.877 |
| Process peak RSS, GiB | 3.532 | 3.347 |
| Stored proof payload, bytes | 2,439,524 | 2,439,524 |
| Reported work-normalized security bits | 129.375910 | 129.375910 |

Peak RSS decreases by 5.3%. The smaller high-table allocation does not translate
directly into the same reduction in process peak RSS, since other phases have
their own live allocations. Verification was slower in the primary trials.

Verification in the paired repeat measured 74.359 → 73.682 ms/batch; there is
no consistent verification speedup across these runs.

Small batches retain fixed proof overhead and do not improve consistently:

| Batch, seed 42 | Baseline ms/signature | Optimized ms/signature |
| --- | ---: | ---: |
| 1 | 43.696 | 44.256 |
| 3 | 21.416 | 20.267 |
| 32 | 2.835 | 2.822 |
| 1,024 | 1.030 | 0.953 |

## Implementation

1. **Four-lane binary sumchecks.** Use VPCLMUL for dense folds and compact
   occupied-lane folds, accumulating unreduced products before field reduction.
   Falcon's contiguous `11 → 6 → 3 → 2` occupied-lane layouts use adjacent loads
   and masked tails. Other supports use the checked general path; tiny inputs
   and builds without the required instruction set retain scalar fallbacks.
   Missing virtual lanes remain zero operands of the same polynomial. Physical
   padding, including nonzero padding in differential tests, remains present.
2. **Karatsuba field products.** Replace four schoolbook partial products with
   three Karatsuba partial products in the four-lane reduced and unreduced
   multiplication kernels. The unreduced accumulator's exact representation is
   preserved, including cancellation and horizontal folding.
3. **Stream final arithmetic coefficients.** The Falcon binder emits final
   folded coefficients once in increasing index order. Assign them directly,
   and accumulate the first tail-round message as each adjacent pair completes.
   Validate ordering, bounds and canonical field values. Missing coefficients
   are zero; generic additive sources and the small-batch fallback retain their
   previous paths.
4. **Specialize nonce hashing.** For the existing 16-byte and 32-byte seeds,
   prepare the seed-only first-round BLAKE3 operations once per scan chunk.
   Compute only the final-round operations needed for the first digest word;
   difficulties above 32 bits still check candidate nonces with the full hash.
   Preserve ascending nonce selection, 64-bit carry handling, scalar tails,
   all challenge boundaries and all grinding difficulties.
5. **Factor cached Keccak tensor weights.** When bit marginals are already
   available and the packed prefix consumes the tensor's low coordinates,
   retain two equality factors instead of a full high-coordinate table.
   Align the split to 64-position tiles, fold the final low scalar into the
   tile weight, and retain one field multiplication per source word. The four
   batch-1,024 Keccak high maps occupy about 1.02 MiB rather than 320 MiB.
   Uncached tensors and wider arbitrary low factors retain the dense path.
   Skip byte-table lookup work for a zero packed word; its exact value is zero,
   while its coefficient and all padding authentication remain unchanged.

The first combined candidate showed gains in binder preparation and PCS
grinding but little end-to-end improvement. Replacing compact SIMD gathers with
contiguous loads saved only about 1.17 ms per batch in an isolated kernel test;
this is too small to establish the throughput objective by itself. The retained
attempt's measurements are in `bench_results/falcon-simd-20260927/attempt1/`.

Isolated native arithmetic microbenchmarks measured 16.6% less time for reduced
four-lane products and 24.9% less for unreduced products. Specialized nonce
hashing used 13.9% less time per attempted nonce for 16-byte seeds and 20.1% less
for 32-byte seeds. These measurements exclude the rest of the protocol and are
not estimates of whole-prover speedup.

## Stage diagnostics

Separate cold diagnostic runs, seed 42, milliseconds per signature. These are
not the warmed trial medians. Nested rows must not be added to their parents.

| Stage | Baseline | Optimized |
| --- | ---: | ---: |
| Witness and commitments | 0.1532 | 0.1626 |
| Norm | 0.0234 | 0.0259 |
| Rejection | 0.0158 | 0.0182 |
| Compaction forest | 0.0462 | 0.0480 |
| Candidate leaf authentication | 0.0586 | 0.0631 |
| Native ideal/projection/carries | 0.0106 | 0.0115 |
| Arithmetic binder | 0.1735 | 0.1655 |
| Binder tail preparation (nested) | 0.0585 | 0.0378 |
| Integer-to-binary bridge | 0.2060 | 0.2142 |
| Keccak prefix | 0.1073 | 0.1068 |
| Joint binary authentication | 0.1287 | 0.0505 |
| Shared PCS opening | 0.1486 | 0.1339 |
| PCS grinding (nested) | 0.0682 | 0.0576 |

An additional diagnostic uses the saved executable immediately before the
tensor factorization, with the same contiguous SIMD kernels. Its joint binary
stage takes 0.1245 ms/signature, versus 0.0505 after factoring. The new spans
split this into:

| Joint substage | Before factoring | Factored |
| --- | ---: | ---: |
| Packed setup | 0.0614 | 0.0035 |
| Compact-table preparation | 0.0365 | 0.0192 |
| Compact lane folding | 0.0215 | 0.0224 |
| Dense suffix folding | 0.0023 | 0.0027 |

This identifies table construction and coefficient preparation as the decisive
costs removed in this pass. The bridge and arithmetic binder remain the largest
individual proof stages. These one-trial profiles also show runtime variation
in stages whose implementation did not change; they should not replace the
matched warmed medians.

## Security and equivalence

These changes remove repeated computation, not protocol obligations. Field
parameters, constraints, committed witness bits, challenge distributions,
grinding difficulties and verifier checks are unchanged. The arithmetic source
still has 100,578 live bits and 131,072 padded bits per capacity slot, and 8,605
auxiliary arithmetic values per live signature. Norms, rejection, ordered
compaction, native ideals/carries, SHAKE links and source padding remain checked.

At batch 1,024 and target 128, the existing work-normalized security report
remains 129.375910 bits. Its computational grinding/Fiat–Shamir convention is
described in [OPTIMIZATION_SECURITY.md](../OPTIMIZATION_SECURITY.md),
[COMPACTION_SOUNDNESS.md](../COMPACTION_SOUNDNESS.md) and
[BRIDGE_GRINDING_AUDIT.md](../BRIDGE_GRINDING_AUDIT.md). Exact transcript comparison
is regression evidence for these implementation changes, not a new security
proof. This is a public-signature, non-ZK prover; kernels are not claimed to be
constant time.

Every primary matched case has the same complete proof Debug digest, input
digest, source roots, stored payload and reported security bound as its
baseline case. The saved pre-factorization diagnostic also matches those
values. New tests compare exact sumcheck messages, not only verifier acceptance.

## Validation

- Full portable library suite: **676 passed, seven ignored**. This run used
  16 Rayon workers and one test at a time, with default worker stack sizes.
  Concurrent tests exposed an existing debug-build stack overflow in the pinned
  Binius dependency's recursive Rayon producers containing inline
  `Cow<[Word; 128]>` tails. The debugger capture is retained; no dependency or
  stack-size changes were made. The serial run passed the complete suite.
- Thirteen native joint-kernel and joint-proof tests passed on the final source.
  They cover arbitrary lane supports, scalar parity, unaligned guarded slices,
  masked stores, Boolean/random challenges, factored/dense coefficient parity,
  nonzero physical padding, tampering and complete transcript equality.
- Three native field-kernel tests passed, including all 16,384 monomial product
  pairs, random/boundary inputs, offset slices and exact unreduced accumulators.
  The nonce tests compare BLAKE3 words and minimum nonces across supported
  backends, including nonce-word carries and tails.
- Thirty-six focused binder tests passed. Ordered-final replay tests cover
  missing pairs, partial blocks, invalid order/indices/residues, generic
  fallbacks and complete dense-proof equality.
- Three upstream integration tests, both serial feature checks, and formatting
  checks passed. The hybrid serial check was repeated on the final source.
  The full portable run preceded the final equivalent power-of-two
  modulo-to-mask substitution; the final native tests cover that substitution.
- Independent reviews checked unsafe kernels, prefix masks, nonce hashing,
  binder replay and the factored tensor/padding algebra.
- All **59 benchmark proofs verified**: 50 primary trials/diagnostics, eight
  proofs in the consecutive repeat, and one pre-factorization diagnostic.
  Every baseline/optimized pair has exact proof-digest, commitment-root,
  input-digest, payload and reported security equality. The original first
  candidate's additional 25 proofs are retained separately under `attempt1/`.

## Measurement conditions

AMD Ryzen 9 9950X3D, 16 Rayon workers, native release (`-C target-cpu=native`,
opt3, fat LTO, one codegen unit), target 128. Each regular executable/input case
uses one warmup and three measured trials, reporting medians. Seeds 42, 43 and
44 each generate 1,024 distinct original Falcon-1024 signatures. Smaller batch
checks use seed 42. Builds, tests and microbenchmarks do not overlap timed runs.

Full prover timing includes witness generation, commitments, SHAKE,
HashToPoint, native certificates/projections/carries, all sumchecks, grinding
and the shared opening. Input generation, upstream validation/decoding,
reusable preparation, proof verification and proof Debug hashing remain outside
prover timing, identically for both executables. Per-signature results are
batch-amortized times, not single-proof latency. Proof payload excludes the
public statement and transport framing; Falcon has no wire codec.

Raw commands, executable hashes, input/proof digests, source snapshots, trial
results, process peak RSS, separate stage diagnostics and validation logs are
retained in `bench_results/falcon-simd-20260927/`.

```sh
CARGO_TARGET_DIR=target/falcon-native RUSTFLAGS='-C target-cpu=native' \
  cargo bench --offline --profile release --features falcon-hybrid \
  --bench falcon_hybrid --no-run --message-format=json

python3 bench_results/falcon-simd-20260927/run.py optimized-b1024-s42 \
  bench_results/falcon-simd-20260927/optimized 1024 42
```
