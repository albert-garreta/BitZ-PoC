# Falcon-1024 CT proofs, v3 and experimental hybrid

This module contains two non-ZK proof backends for Falcon-1024 constant-time
signatures. The `falcon` backend supports 1–32 signatures and one BitZ source
commitment. The experimental `falcon-hybrid` backend supports 1–1024 signatures,
with separate arithmetic and binary Keccak commitments authenticated through one
joint opening. Both prove SHAKE256, HashToPoint rejection and ordered compaction,
the Falcon ring equation, and the norm bound. Native witness generation is not
used as a substitute for those proof constraints.

The signature and nonce are not public-statement fields. These protocols do not
provide zero knowledge or establish formal nonce hiding: proof messages and
openings can disclose witness information.

## Changes in v3

[piop.rs](piop.rs) now checks the norm bound separately for every signature.
It samples an instance equality point after commitment and proves the randomly
weighted per-instance norm identities, including each signature's bounded
slack. An excessive norm in one signature can no longer consume unused slack
from another. The binder authenticates both weighted and unweighted terminal
operands. Padded instances have zero contribution.

HashToPoint's nonlinear rows use four word relations per candidate: selection,
selected rank, selected remainder, and rejection. This replaces the former
27 bit-product rows; fixed-width source encodings and the other range constraints
justify the word equalities. Compaction still proves that the first 1024
accepted samples appear in order.

The 128-bit profile additionally grinds the initial random row point of every
outer product sumcheck. Grinding only the later sumcheck rounds would leave the
initial random collapse unprotected above the prime-field ceiling. The new
norm-instance point has its own grinding boundary. Source and PIOP transcript
headers are versioned `v3`.

## Structured binding

`BindingForm` describes the claim `sum_i c_i f_i = T` without allocating its full
coefficient vector. It retains factored equality weights, batching challenges,
and a lazy prover cache with one 1024-element ring adjoint per distinct public
key. Complete key equality determines reuse, without changing instance order.
The common local row weights are independent of the instance factor; the
prover scales a shared adjoint while emitting each signature's coefficients.
It does not store a scaled adjoint copy for every repeated key.
The adjoint contracts Falcon's
negacyclic multiplication at the coefficient level before distributing signed
bit weights. Its polynomial multiplication uses Karatsuba above 32 coefficients
and schoolbook multiplication below, entirely in the proof field.

The prover streams additive coefficient updates into `StreamingMle` and the
shared packed inner-sumcheck engine. One traversal accumulates four prefix rounds;
a second writes the coefficient table already folded over those coordinates.
Overlapping updates and cache evictions preserve the same ordinary sumcheck
messages as a dense table. The verifier evaluates `C(r)` directly, contracting
shared instance factors before evaluating local wiring. It never constructs or
folds a full-domain coefficient vector. For the ring contribution, it first
forms `h_eff = sum_s alpha_s * beta_s * h_s` in the proof field, where `alpha_s`
is the linear-row instance weight and `beta_s` the source-endpoint instance
weight. It computes one adjoint of this effective key even when all keys differ,
then evaluates the local signed-bit functional. Only live instances contribute.
Target calculation and verification never initialize the prover's adjoint cache.
These are explicit forward/adjoint kernels; Falcon does not instantiate the
generic Wengert tape. This reuse and contraction preserve the v3 proof format,
transcript, constraints, and grinding schedule.

## Prime-field Keccak layout

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

For the legacy backend's maximum 64 trees, there are 55 cubic round challenges, ten batching
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

## Experimental binary Keccak composition

[hybrid.rs](hybrid.rs) exports `PreparedFalconHybrid`, `CommittedFalconHybrid`,
`FalconHybridStatement`, `FalconHybridProof`, and `FalconHybridSecurity` through
[mod.rs](mod.rs). The public API supports security targets 100 and 128 and rounds
the live batch length up to a power-of-two capacity. Preparation is reusable:

```rust,ignore
use bitz::piop::spartan::falcon1024_ct::{
    FalconPublicStatement, PreparedFalconHybrid,
};

let prepared = PreparedFalconHybrid::new(public_keys.len(), 128)?;
let public = FalconPublicStatement::from_bytes(&public_keys, &messages)?;
let committed = prepared.commit(public, &signatures)?;
let statement = committed.statement.clone();
let proof = prepared.prove(committed)?;
prepared.verify(&statement, &proof)?;
let report = prepared.security();
```

Here `public_keys`, `messages`, and `signatures` are slices of byte slices;
messages have the supported fixed length of 32 bytes. `prove` consumes the
committed witness so its large buffers can be folded without cloning.
The experimental proof currently has an in-memory Rust API, without a wire
serialization format.

The arithmetic source has **198,935 live bits**, padded to **2^18 bits per
capacity slot**. It contains the encoded signature, the 1311 sampled words,
HashToPoint/compaction auxiliaries, ring witnesses, and norm slack. The binary
source reserves **32 × 2^16 = 2^21 bits per capacity slot**. Each slot holds
20 SHAKE permutations and 12 valid dummy Keccak(0) permutations; padded signature
slots are also filled with valid dummy permutations.

[hybrid_keccak.rs](hybrid_keccak.rs) uses Flock's compact Keccak encoder: each
permutation stores input, output, and 24 chi-AND vectors in a 65,536-bit block
whose useful prefix has 42,560 bits. Intermediate theta/rho/pi states are
implicit binary linear functions. The circuit's transpose recurrence evaluates
their contributions without expanding the corresponding dense matrices.
The prefix uses the existing binary commitment and caller transcript and stops
at two normalized linear claims; it does not make another commitment.

The two sources are connected by authenticated random linear copy checks:

1. The first Keccak input contains the arithmetic source's 40-byte nonce,
   the public message, and the exact SHAKE suffix/padding and capacity zeros.
2. Each of the next 19 permutation inputs equals the preceding 1600-bit output.
3. The extracted rate bytes equal the arithmetic source's 1311 big-endian
   16-bit words, which feed the proved HashToPoint calculation.

These checks include the full sponge state, not only the output rate. Public
messages and both roots enter the common transcript before proof challenges.
Two unrelated proofs sharing an opener would not establish these equalities.

[hybrid_bridge.rs](hybrid_bridge.rs) converts the prime-field source claim to
binary linear claims with bounded integer limbs and exponent-fold forests;
it does not identify a prime-field MLE with a binary-field MLE.
[hybrid_sumcheck.rs](hybrid_sumcheck.rs) combines these claims, the Keccak claims,
and the copy checks using structured tensor and gather terms. It folds the
first seven bits without constructing a full field-element coefficient table,
then hands one terminal to a shared ring switch and Ligerito continuation.
Both original roots remain authenticated. The current hybrid selects the
unique-decoding Ligerito profile and has no Round-0 OOD message.

The `falcon-hybrid` feature currently enables `falcon`, the existing `hybrid`
feature, and the vendored `flock-prover` dependency. Consequently its build also
includes the existing Binius64 and metrics dependencies inherited through
`hybrid`; this Keccak circuit itself uses Flock, not the Binius SHA-256 circuit.

## Hybrid security report

`PreparedFalconHybrid::security()` returns the actual configured union-bound
terms as error probabilities and `algebraic_bits = -log2(sum(terms))`.
Preparation rejects a profile whose report misses the requested target. The
report covers prime sampling, per-signature norms, initial row points and
sumchecks, compaction fingerprints and forest, terminal binding, integer-to-binary
forests, binary Keccak, SHAKE wiring and batching, joint sumcheck, ring switching,
support padding, and shared Ligerito.

This is algebraic/IOP accounting in the existing **computational grinding
model**: a block with raw error `e` and `g` grinding bits contributes
`e / 2^g` per unit of adversarial work. It is not an unconditional improvement
to interactive soundness. BLAKE3's 128-bit collision bound is stated separately
and is not an extra algebraic error term in this report.

The prime field satisfies `p > 2^125`. The 100-bit hybrid profile leaves prime
grinding disabled: even at 1024 signatures the combined prime contribution is
below `2^-103.969`, before adding the separately reported binary stages. At
target 128, the hybrid derives each prime difficulty from the actual batch:

```text
g(n) = max(0, 128 + 5 + ceil(log2(n)) - 125), with g(0) = 0.
```

Here `n` is that stage's audited degree/occurrence numerator. The shared cubic
difficulty covers the sum of the HashToPoint product-sumcheck and entire
compaction-forest numerators. This avoids charging a one-signature proof for
the maximum 2048-tree forest. Seven prime category budgets of `2^-133`, five
binary-stage budgets of `2^-136`, a whole-PCS budget of `2^-130`, and the
`2^-144` whole-search prime-sampling term give a conservative whole-composition
bound above 129 bits:

```text
error / 2^-128 <= 7/32 + 5/256 + 1/4 + 2^-16 < 0.489.
```

These are analytical bounds under the stated grinding model, not measured attack costs
or performance results. The legacy backend retains its separate schedule.

For the binary Keccak prefix, `m = 21 + log2(capacity)` and the raw error is
bounded by `(4*m + 256) / 2^128`. A caller component target of `target + 8` gives
zero extra grinding at target 100 and 17 bits at target 128. Each uninterrupted
challenge block has one nonce; vector coordinates share it. The bound includes
the constant-column check across all permutations, and tests check that the
seven fixed zerocheck coordinates give an injective encoding of all 128 Boolean
residuals. The other binary stages reserve the same eight-bit margin. Ligerito
reserves two bits for the whole PCS; if its schedule has `k` challenge blocks,
each block targets `target + 2 + ceil(log2(k))` bits. Their sum is at most
`2^-(target+2)`. This allocation stays within the implementation's grinding cap
while preserving the complete composition budget. The actual report and its
preparation-time target check remain authoritative. Every supplied nonce must
be consumed and verified.

## Compatibility and memory

The legacy commitment-bound and PIOP headers now use `v3`; existing forest
domains retain `v2`. The new norm and initial-row-point proof fields change
the transcript. **Regenerate earlier proofs.** The hybrid has its own versioned
statement and different source layouts; hybrid roots are not interchangeable
with legacy roots.

Streaming eliminates the full binding coefficient vector, but does not make the
entire prover constant-memory. For one signature, coefficient-cache values use
at most 1 MiB plus metadata, and the four-coordinate folded coefficient table
uses 4 MiB. The folded table scales with batch capacity. The first Keccak outer
fold still uses about 48 MiB per capacity slot. Packed witnesses, subsequent
folds, and commitment/opening state also remain. Prefix accumulation and
coefficient replay currently run serially. Whole-process peak RSS includes these
allocations and must not be interpreted as binder-only memory. These figures
describe the legacy prime-Keccak backend.

The hybrid also retains substantial witness and opening state. At capacity
1024, its packed arithmetic and Keccak sources alone occupy 32 MiB and 256 MiB.
Keccak A/B buffers, a lincheck copy, folded field tables, Merkle codewords, and
shared-opening workspaces add to this. Structured wiring avoids a dense table
over the entire bit domain; it does not make the full prover constant-memory.

## Validation

[opening_binding_tests.rs](opening_binding_tests.rs) compares the ring adjoint
against an explicit negacyclic matrix and structured endpoints against dense
binding with distinct keys and padded instances. Tests in [piop.rs](piop.rs)
compare lazy Keccak proofs/transcripts with the original dense construction,
check forest batches 1/3/32 against direct leaf evaluations, and reject tampered
roots, terminals, layers, messages, and nonces. Exact-constraint and end-to-end
tests cover corrupted parity witnesses. New tests cover per-instance norm
overspending, packed HashToPoint products, binary Keccak input/chaining/sample
mapping, constant-wire pins, prefix transcript replay, dense coefficient
oracles, and corrupted proofs/roots/nonces. Shared streaming tests cover prefix
lengths 0–4, overlapping signed updates, eviction, padding, and malformed input.
Adjoint-reuse tests compare complete coefficients and sumcheck transcripts with
the former per-instance construction, including repeated, mixed, and distinct
keys, zero instance weights, and padded endpoints. An explicit negacyclic matrix
checks the field-valued adjoint on coefficients much larger than Falcon's modulus.
Verifier and target evaluation tests also check that the prover cache stays empty.

On 2026-09-25, the complete release library suite with `falcon-hybrid` enabled
passed **567 tests**, with zero failures and five ignored tests. This includes
end-to-end proofs at both security targets and preparation checks through
1024 signatures. The existing `falcon1024_ct` and `hybrid_u32_sha256` benchmark
clients also pass `cargo check` with `falcon-hybrid` enabled.

Run from the repository root:

```sh
cargo test --offline --release --features falcon --lib falcon1024_ct
cargo test --offline --release --features falcon --lib streaming_
cargo test --offline --release --features falcon-hybrid --lib falcon1024_ct
cargo test --offline --release --features falcon-hybrid --lib
```

The complete hybrid benchmark verifies every generated proof and reports
witness/commit time, proving time, verification time, total prover throughput,
and process peak RSS. Security must be selected explicitly:

```sh
cargo bench --offline --features falcon-hybrid --bench falcon_hybrid -- \
  --batch 32 --security 128 --threads 16 --warmup 1 --iterations 3
```

Run larger batches only with sufficient memory. The benchmark defaults to a
repeated bundled fixture, which it identifies in its output; fixture files can
be overridden with `BITZ_FALCON_PUBLIC_KEY`, `BITZ_FALCON_SIGNATURE`, and
`BITZ_FALCON_MESSAGE`. Compare the total prover column when witness generation
and commitments must count toward throughput.

## Adjoint reuse: matched 16-thread comparison

On 2026-09-25, the executable from the initial hybrid implementation was
preserved before adding key reuse and verifier contraction. Both executables
then ran the same repeated-key fixture at target 128 on the Ryzen 9 9950X3D,
with 16 Rayon threads, identical release codegen, and no `RUSTFLAGS` override.
Runs were sequential without concurrent tests or compilation. Fixture digests,
commitment roots, and configured security bounds match between each pair.

Batches 32 and 256 used one warmup plus three measured trials per executable;
the table reports medians. Batch 1024 used one cold trial per executable and is
preliminary. All 18 trials, including warmups, verified. Full prover time includes
witness generation, both commitments, and proof generation.

| Batch | Full prover before | Full prover after | Verify before | Verify after |
| --- | ---: | ---: | ---: | ---: |
| 32 | 1.899 s | 1.847 s | 0.205 s | 0.151 s |
| 256 | 11.733 s | 11.369 s | 1.357 s | 0.929 s |
| 1024 | 48.013 s | 46.893 s | 5.331 s | 3.603 s |

Verification time fell by **26.1%, 31.6%, and 32.4%**, respectively. Observed
full-prover times fell by 2.7%, 3.1%, and 2.3%; these small gains are less
conclusive because the three-trial prover ranges overlap. For example, at
batch 256 the before range was 11.722–11.788 s and the after range was
11.368–11.747 s, while verifier ranges were clearly separated at
1.354–1.359 s and 0.925–0.930 s. Witness/commitment time was essentially unchanged.

The resulting full-prover cost is **57.73, 44.41, and 45.79 ms/signature** for
these batches; the 1 ms goal remains unmet. This benchmark measures shared-key
prover reuse. Mixed and distinct keys are covered by differential tests, but
their performance was not measured in this campaign. Raw trials, timing ranges,
executable/source hashes, and the before/after source snapshots are retained in
`bench_results/falcon-adjoint-reuse-20260925/`.

## Initial hybrid performance: 16 threads

These measurements precede the adjoint reuse and contraction comparison above.

Measured on 2026-09-25 on the same AMD Ryzen 9 9950X3D, with 16 Rayon threads.
Cases ran sequentially without concurrent compilation or tests, and every
generated proof verified. Batches 1 and 32 used one warmup and three measured
trials; batches 256 and 1024 used one measured trial without warmup, so their
numbers are preliminary. All cases repeat the bundled valid signature fixture.

The total prover column includes SHAKE/HashToPoint witness generation, both
commitments, and the complete proof. Verification and reusable preparation are
separate. Each median is computed from its own trial measurements; total prover
time uses the median of the per-trial sums. RSS is the whole-process peak.

| Batch | Target | Witness + commit | Prove batch | Total prover / signature | Verify batch | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 100 | 0.005 s | 0.084 s | 89.97 ms | 0.039 s | 0.05 GiB |
| 1 | 128 | 0.006 s | 0.469 s | 475.25 ms | 0.043 s | 0.05 GiB |
| 32 | 100 | 0.217 s | 1.143 s | 42.50 ms | 0.204 s | 0.36 GiB |
| 32 | 128 | 0.219 s | 1.669 s | 59.02 ms | 0.204 s | 0.36 GiB |
| 256 | 100 | 2.048 s | 8.897 s | 42.75 ms | 1.354 s | 1.83 GiB |
| 256 | 128 | 2.039 s | 9.801 s | 46.25 ms | 1.360 s | 1.83 GiB |
| 1024 | 100 | 8.941 s | 35.558 s | 43.46 ms | 5.312 s | 6.71 GiB |
| 1024 | 128 | 8.937 s | 38.804 s | 46.62 ms | 5.318 s | 6.74 GiB |

At batch 32 and target 128, the historical v2 total prover median was
443.04 ms/signature; this hybrid is **7.51× faster**, at 59.02 ms/signature.
That is a historical comparison across protocols: the hybrid includes the v3
norm and challenge-boundary fixes described above. The old executable used
the bench profile with debug information; this measurement used the release
profile without it. Both use fat LTO, one codegen unit, and no `RUSTFLAGS`
override. The release benchmark was built with
`cargo test --offline --release --features falcon-hybrid --lib --bench falcon_hybrid --no-run`
and its executable was invoked directly with the options documented above.

**The 1 ms/signature goal is not reached.** Target-128 throughput levels off
around 46 ms/signature (about 21.5 signatures/second); moving from batch 256
to 1024 does not improve it in these samples. Even witness generation plus
commitment alone costs about 8–9 ms/signature at those batch sizes. Further
improvement needs faster per-signature kernels, not only a larger batch.

Raw JSONL, `/usr/bin/time -v` output, executable/source hashes, sample counts,
and the historical comparison are saved locally in
`bench_results/falcon-hybrid-20260925/`. Proof byte size is not reported because
the experimental proof does not yet have a wire codec. The following tables
retain the historical v2 measurements and do not measure the hybrid or v3 fixes.

## Historical v2 performance: one thread

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
No batch-32 128-bit baseline was measured. Those v2 runs retained the v1 grinding
difficulties; they predate the v3 boundaries and the hybrid's derived schedule.

## Historical v2 performance: 16 threads

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
