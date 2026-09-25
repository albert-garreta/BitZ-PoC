# Falcon-1024 CT proof, v2

This module proves batches of 1–32 Falcon-1024 constant-time signatures against
one BitZ commitment to source bits. Native trace construction remains independent
of the transcript-selected prime field. The commitment-bound protocol combines
the nonlinear PIOP, a terminal linear binding claim, an inner sumcheck, and a BitZ
opening; see [opening.rs](opening.rs) and [piop.rs](piop.rs).

## Structured binding

`BindingForm` describes the claim `sum_i c_i f_i = T` without allocating its full
coefficient vector. It retains factored equality weights, batching challenges,
and a 1024-element ring adjoint per public key. The adjoint contracts Falcon's
negacyclic multiplication at the coefficient level before distributing signed
bit weights. Its polynomial multiplication uses Karatsuba above 32 coefficients
and schoolbook multiplication below, entirely in the proof field.

The prover streams additive coefficient updates into `StreamingMle` and the
shared packed inner-sumcheck engine. One traversal accumulates four prefix rounds;
a second writes the coefficient table already folded over those coordinates.
Overlapping updates and cache evictions preserve the same ordinary sumcheck
messages as a dense table. The verifier evaluates `C(r)` directly, contracting
shared instance factors before evaluating local wiring. It never constructs or
folds a full-domain coefficient vector. These are explicit forward/adjoint
kernels; Falcon does not instantiate the generic Wengert tape.

## Keccak layout

[keccak.rs](keccak.rs) reuses column parities rather than expanding eleven input
bits for every theta output. Before the rho/pi permutation, the exact equations
are

```text
sum_y A[x,y,z] = C[x,z] + 2*u[x,z]
A[x,y,z] + C[x-1,z] + C[x+1,z-1] = B[x,y,z] + 2*v[x,y,z]
```

Here `u` has two committed bits and `v` has one. Boolean source bits and these
integer equalities enforce parity and quotient ranges. The relation stays in the
prime field. [layout.rs](layout.rs) and [source.rs](source.rs) define the packing:

| Per signature | v1 | v2 |
| --- | ---: | ---: |
| Live source bits | 4,785,959 | 3,710,759 |
| Padded source bits | 2^23 | 2^22 |
| Linear rows | 776,583 | 930,183 |
| Padded linear-row stride | 2^20 | 2^20 |

`KeccakRows` reads chi operands from packed trace words, preserving iota, padding,
and unused batch slots. It removes the three initial dense outer-product tables;
field-valued tables are still allocated after the first challenge fold.

## Shared compaction forest

Each signature retains separate candidate/output trees of 2048 leaves and its
own root-equality check. Corresponding layers across all trees share a sumcheck
of `eq(r,x) * sum_t rho^t L_t(x) R_t(x)`. Every non-root layer gets a fresh
batching challenge after its claims are fixed. Child evaluations are absorbed
before the shared line challenge; final leaf claims remain bound to source bits.

At the maximum 64 trees, there are 55 cubic round challenges, ten batching
challenges of degree at most 63, and eleven line challenges of degree one per
tree. The 128-bit profile grinds every such challenge at 21 bits with separate
round, batching, and line domains. Under the existing computational grinding
analysis, the forest error contribution is bounded by

```text
(55*3 + 10*63 + 11*64) / (2^125 * 2^21) < 2^-135.
```

Other argument blocks retain their separate security budgets. The 100-bit profile
uses no grinding; the same forest numerator over `2^125` is below `2^-114`.
The forest supplies cubic group arithmetic but reuses the shared sumcheck round
helper and verifier. The ordinary outer engine accepts only one `A*B-C` terminal
triple, and the batched inner engine handles degree-two products.

## Compatibility and memory

The commitment-bound and PIOP headers use `v2`, as does the new forest domain.
The auxiliary witness, source stride, and proof structure changed. **Regenerate
v1 commitments and proofs; they are incompatible with v2.**

Streaming eliminates the full binding coefficient vector, but does not make the
entire prover constant-memory. For one signature, coefficient-cache values use
at most 1 MiB plus metadata, and the four-coordinate folded coefficient table
uses 4 MiB. The folded table scales with batch capacity. The first Keccak outer
fold still uses about 48 MiB per capacity slot. Packed witnesses, subsequent
folds, and commitment/opening state also remain. Prefix accumulation and
coefficient replay currently run serially. Whole-process peak RSS includes these
allocations and must not be interpreted as binder-only memory.

## Validation

[opening_binding_tests.rs](opening_binding_tests.rs) compares the ring adjoint
against an explicit negacyclic matrix and structured endpoints against dense
binding with distinct keys and padded instances. Tests in [piop.rs](piop.rs)
compare lazy Keccak proofs/transcripts with the original dense construction,
check forest batches 1/3/32 against direct leaf evaluations, and reject tampered
roots, terminals, layers, messages, and nonces. Exact-constraint and end-to-end
tests cover corrupted parity witnesses. Shared streaming tests cover prefix
lengths 0–4, overlapping signed updates, eviction, padding, and malformed input.

Run from the repository root:

```sh
cargo test --offline --release --features falcon --lib falcon1024_ct
cargo test --offline --release --features falcon --lib streaming_
cargo test --offline --release --features falcon --lib
```

The full release library suite passed on 2026-09-25: 518 passed, 5 ignored,
0 failed. The combined `falcon,ecdsa` feature check also passed.

## Preliminary performance

A 32-signature batch at the 100-bit target, with one Rayon thread on an AMD
Ryzen 9 9950X3D, measured the following against revision
`2f2ac242a85d51684b8006bee19217021edd86a4`:

| Metric | v1 | v2 |
| --- | ---: | ---: |
| Proof generation | 76.67 s | 41.16 s |
| Verification | 19.36 s | 420 ms |
| Whole-process peak RSS | 5.87 GiB | 2.45 GiB |

Both binaries used the default bench profile and `falcon,span-metrics` features.
Each ran one warmup and one measured trial using the repeated bundled fixture;
all proofs verified. These are preliminary single-sample timings. Peak RSS
includes the entire benchmark process, not only binding or verification.


The candidate also verified at batch 32 with the 128-bit profile: 44.33 s proving,
424 ms verification, and 2.45 GiB peak RSS, using the same warmup/sample setup.
No batch-32 128-bit baseline was measured. The grinding difficulties are unchanged.

## 16-thread measurements

The same optimized executable was rerun with `RAYON_NUM_THREADS=16` on the
same Ryzen 9 9950X3D. Cases ran sequentially, without concurrent compilation or
tests. Each case used one warmup and three measured trials; the table reports
medians. Every trial reported 16 Rayon threads and verified successfully.
The one-thread column is the previous measurement, with its smaller sample count.

| Batch | Target | Proving, 1 thread | Proving, 16 threads | Prover speedup | Verification, 16 threads |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | 100 | 1.239 s | 0.475 s | 2.61× | 184.5 ms |
| 1 | 128 | 4.405 s | 0.720 s | 6.12× | 185.6 ms |
| 32 | 100 | 41.155 s | 13.254 s | 3.11× | 423.8 ms |
| 32 | 128 | 44.335 s | 13.474 s | 3.29× | 425.2 ms |

These compare thread counts for v2, not v1 against v2. Witness generation and
commitment are excluded from the proving column, matching the earlier tables.
Verification stays close to the one-thread values (187–188 ms for one signature
and 420–424 ms for batch 32). The sumcheck kernels and expensive nonce searches
use Rayon; coefficient streaming, verifier binding evaluation, and product-forest
arithmetic still use serial loops. This limits parallel speedup. These timings
do not isolate the contribution of each stage.
