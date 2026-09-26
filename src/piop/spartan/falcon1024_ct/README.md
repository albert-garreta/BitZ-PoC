# Falcon-1024 CT proofs, v4 public signatures and experimental hybrid

This module contains two non-ZK proof backends for Falcon-1024 constant-time
signatures. The `falcon` backend supports 1–32 signatures and one BitZ source
commitment. The experimental `falcon-hybrid` backend supports 1–1024 signatures,
with separate arithmetic and binary Keccak commitments authenticated through one
joint opening. Both prove SHAKE256, HashToPoint rejection and ordered compaction,
the Falcon ring equation, and the norm bound. Native witness generation is not
used as a substitute for those proof constraints.

Public statements contain the public keys, 32-byte messages, and exact signatures
(nonce and `s2`). Both backends bind the canonical CT signature bytes to their
authenticated witness copies. These protocols do not provide zero knowledge;
the remaining auxiliary witness is not guaranteed to be hidden.

## Public signatures and independent benchmark inputs

The outer commitment-bound and hybrid statement domains are now `v4`.
Every CT signature byte has a linear equality against its eight committed bits,
including the nonce and signed coefficient payload. Absorbing signatures into
the transcript complements these equality constraints; it does not replace them.
There are 931,752 linear rows per legacy signature and 10,152 per hybrid signature,
which fit the existing row domains. The internal nonlinear PIOP is unchanged.

Both proof benchmarks use only distinct keypairs and 32-byte messages generated
reproducibly with `fn-dsa = 0.3.0` in `HASH_ID_ORIGINAL_FALCON` mode. The default
batch contains 32 signatures; each selected batch size generates that many keys
and signatures. Fixture manifests, file overrides, and repeated-input pools have
been removed. Obsolete fixture arguments and environment variables are rejected.
Upstream generation and verification, CT conversion, and native preflight checks
run before prover timing. Every measured trial still constructs and commits the
complete witness, proves it, and verifies the proof. Upstream native verification
is reported separately for the same batch. The fixed seed is for benchmark keys
only. The benchmark records the seed, upstream version, input digest, and public
input relation so runs can be reproduced.
The hybrid benchmark's JSON schema is `bitz/falcon-hybrid/v3`; `input_digest`
identifies the generated batch in the preparation and trial records.

The performance tables below predate public-signature binding and independent
generated inputs unless explicitly marked otherwise; they are historical data.

### Generated-input validation on 2026-09-26

The portable release build, with no `RUSTFLAGS` override, verified a warmup and
three measured batches of 32 distinct upstream-generated signatures at target
128, seed 42, and 16 Rayon threads on an AMD Ryzen 9 9950X3D. Median times:

| Stage | Milliseconds per 32-signature batch |
| --- | ---: |
| Upstream native verification, sequential, including key decoding | 0.580 |
| Witness construction and commitments | 146.018 |
| Proof generation | 553.180 |
| Total prover | 699.198 |
| Proof verification | 136.530 |

Input generation and preflight took 298.116 ms, and reusable preparation took
3.440 ms; both are outside total prover time. Total prover throughput was
45.77 signatures/second (21.85 ms/signature). This is a measurement of the new
public-signature relation on distinct keys, not a matched speedup comparison
with the historical repeated-fixture tables. Raw trials, source hashes, and
validation logs are retained locally in `bench_results/falcon-upstream-20260926/`.
The legacy benchmark also verified a warmup and one measured 32-signature batch
with the same input digest, target, and thread count. Its single measured sample
took 4,428.833 ms for the total prover and 296.928 ms for proof verification;
this is a smoke measurement rather than a median.

Validation passed all 72 Falcon library tests and three upstream integration
tests. These include canonical conversion, reproducible distinct inputs, direct
public-byte/source equality checks, and rejection when a proof is paired with
a different valid signature for the same public key and message.

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
For the hybrid source, [opening_compact.rs](opening_compact.rs) first combines
all contributions to each integer word. It caches these compact coefficients
across both traversals and emits their bit weights in source order, combining
overlapping words and individual-bit corrections before each bit is emitted.
The repeated linear template and common terminal equality tables are shared
across signatures. Signed bit weights use successive doublings. The target is
computed from affine constants and live-instance equality contractions, without
replaying the coefficient emitter. The legacy emitter remains an independent
oracle for coefficient and transcript equivalence tests.
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
let public = FalconPublicStatement::from_bytes(&public_keys, &messages, &signatures)?;
let committed = prepared.commit(public)?;
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
sources reserve **16 × 2^16 + 4 × 2^16 bits per capacity slot**, in two
separately committed permutation groups. This removes the old 12 dummy
permutations per signature. Padded signature slots still contain valid Keccak
chains. At batch one only, the four-permutation group has a second padded
signature slot to satisfy Flock's eight-block minimum.

For capacity at least eight, a chain-aware witness producer executes each
permutation once, emitting its compact circuit witness and retaining output
lanes for the next permutation and for SHAKE sample extraction. Smaller
capacities use the reference setup and packed producer. Physical addresses are
`[7 in-word | signature | permutation within group | 9 chunk]`.

[hybrid_keccak.rs](hybrid_keccak.rs) uses Flock's compact Keccak encoder: each
permutation stores input, output, and 24 chi-AND vectors in a 65,536-bit block
whose useful prefix has 42,560 bits. Intermediate theta/rho/pi states are
implicit binary linear functions. The circuit's transpose recurrence evaluates
their contributions without expanding the corresponding dense matrices.
The prefix uses the existing binary commitment and caller transcript and stops
at two normalized linear claims; it does not make another commitment.

The three sources are connected by authenticated random linear copy checks:

1. The first Keccak input contains the arithmetic source's 40-byte nonce,
   the public message, and the exact SHAKE suffix/padding and capacity zeros.
2. Each of the next 19 permutation inputs equals the preceding 1600-bit output.
3. The extracted rate bytes equal the arithmetic source's 1311 big-endian
   16-bit words, which feed the proved HashToPoint calculation.

These checks include the full sponge state, not only the output rate. Public
messages and all three roots enter the common transcript before proof challenges.
Two unrelated proofs sharing an opener would not establish these equalities.

[hybrid_bridge.rs](hybrid_bridge.rs) converts the prime-field source claim to
one binary linear claim using a merged exponent-fold forest. The 126-bit row
weights have 113-bit and 13-bit limbs. Both integer folds share a nibble-table
scan of the existing source rows, and both limbs share one forest's challenge
rounds. Flat power tables and borrowed bit rows avoid repacking the source.
Both limb sums are bound before any forest challenges; the final limb coordinate
contracts their row weights into one claim on the same committed bits. This
does not identify a prime-field MLE with a binary-field MLE.
The conservative merged-forest error numerator is
`s + 3*(d*(d-1)/2 + d*s) + d`, where `d=13` and `s=col_vars+1` includes the
limb coordinate. Its maximum 887 remains below the existing budget of 4096.
[hybrid_sumcheck.rs](hybrid_sumcheck.rs) combines these claims, the Keccak claims,
and the copy checks using structured tensor and gather terms. It folds the
first seven bits without constructing a full field-element coefficient table,
then hands one terminal to a shared ring switch and Ligerito continuation.
All three roots remain authenticated, including the chain link from permutation 15
to permutation 16 across the two Keccak commitments. The current hybrid selects the
unique-decoding Ligerito profile and has no Round-0 OOD message.

The `falcon-hybrid` feature currently enables `falcon`, the existing `hybrid`
feature, and the vendored `flock-prover` dependency. Consequently its build also
includes the existing Binius64 and metrics dependencies inherited through
`hybrid`; this Keccak circuit itself uses Flock, not the Binius SHA-256 circuit.

### Binary HashToPoint experiment

[hybrid_hash_to_point.rs](hybrid_hash_to_point.rs) implements a separate Binius64
word circuit for all 1311 candidates. It constrains 16-bit inputs, reduction
modulo 12289, rejection at 61445, and the first 1024 accepted residues in order.
A fixed nine-pass compaction network routes records containing a residue and
its original displacement. Range checks reject underflow through the invalid
record sentinel. Selection uses XOR/AND masks rather than Binius's generic
selector, keeping the circuit free of integer/binary multiplication oracles.

`add_hash_to_point` accepts private or public wires. `BinaryHashToPoint` exposes
a complete standalone proof with the exact sample and output arrays as public
inputs, plus circuit statistics and online timings. It is not connected to
`PreparedFalconHybrid`: the latter continues to prove HashToPoint in its prime
branch. A shared composition would additionally need authenticated sample/output
copy checks and a security budget for all reductions. The standalone constructor's
security parameter controls the FRI query target only, not that full budget.

The isolated benchmark includes witness generation, commitment and opening but
excludes SHAKE and Falcon arithmetic. It must not be reported as full signature
proving or as an equivalent 128-bit composition:

```sh
CARGO_TARGET_DIR=target/falcon-native RUSTFLAGS="-C target-cpu=native" \
  cargo bench --offline --profile release --features falcon-hybrid \
  --bench falcon_hash_to_point -- --batch 1 --security 128 --threads 16
```

`--prepare-only` reports the circuit's allocation without proving; larger batches
should first be checked for witness growth. The existing full Falcon benchmark
continues to include SHAKE, HashToPoint, commitments, and all proof stages.

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
the maximum 2048-tree forest. Seven prime category budgets of `2^-133`, six
binary-stage budgets of `2^-136` (including both Keccak prefixes), a whole-PCS
budget of `2^-130`, and the
`2^-144` whole-search prime-sampling term give a conservative whole-composition
bound above 129 bits:

```text
error / 2^-128 <= 7/32 + 6/256 + 1/4 + 2^-16 < 0.493.
```

These are analytical bounds under the stated grinding model, not measured attack costs
or performance results. The legacy backend retains its separate schedule.

For the two binary Keccak prefixes, `m = 20 + log2(capacity)` and
`m = 18 + log2(capacity)` respectively, except for the small-batch padding
noted above. Each raw error is bounded by `(4*m + 256) / 2^128`; the security
report adds both contributions. A caller component target of `target + 8` gives
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
with legacy roots. The three-source hybrid statement and claim-combination
domains now use **v3** for the merged bridge; its bridge-local domains use v2.
Keccak-prefix domains remain v2. All permutation groups and roots remain bound.
Regenerate proofs from earlier hybrid versions. Prime arithmetic constraints and
its challenge schedule are unchanged by the parallel implementation.

Streaming eliminates the full binding coefficient vector, but does not make the
entire prover constant-memory. For one signature, coefficient-cache values use
at most 1 MiB plus metadata, and the four-coordinate folded coefficient table
uses 4 MiB. The folded table scales with batch capacity. The first Keccak outer
fold still uses about 48 MiB per capacity slot. Packed witnesses, subsequent
folds, and commitment/opening state also remain. Prefix accumulation and
coefficient replay run on disjoint signature partitions. Each active worker
has its own bounded coefficient cache; folded-table writes need no atomics.
Whole-process peak RSS includes these allocations and must not be interpreted
as binder-only memory. These figures
describe the legacy prime-Keccak backend.

The hybrid also retains substantial witness and opening state. At capacity
1024, its packed arithmetic and Keccak sources alone occupy 32 MiB and 160 MiB.
The virtual shared-opening table is half the previous two-source table.
Keccak A/B buffers, a lincheck copy, folded field tables, Merkle codewords, and
shared-opening workspaces add to this. Structured wiring avoids a dense table
over the entire bit domain; it does not make the full prover constant-memory.

## Validation

RustCrypto's `sha3` crate is a test-only SHAKE256 oracle. Differential tests in
[keccak.rs](keccak.rs) cover absorption and squeezing at 136-byte rate boundaries,
including Falcon's 72-byte input and full 2,622-byte output. Tests in
[hash_to_point.rs](hash_to_point.rs) independently hash the nonce followed by the
message and check the sampled words and selected residues. The hybrid test in
[hybrid.rs](hybrid.rs) checks both the samples and their packed source bits against
RustCrypto for batches 1, 3, and 8, including the transition between Keccak groups.

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
passed **574 tests**, with zero failures and five ignored tests. The native build
also covers the new three-source padding mask, dense sumcheck equivalence with
cached marginals, every fused Keccak witness buffer, cross-group chain-link
rejection, partitioned streaming, and direct cubic forest evaluations. This includes
end-to-end proofs at both security targets and preparation checks through
1024 signatures. The existing `falcon1024_ct` and `hybrid_u32_sha256` benchmark
clients also pass `cargo check` with `falcon-hybrid` enabled.

Run from the repository root:

```sh
cargo test --offline --release --features falcon-hybrid --test falcon_upstream
cargo test --offline --release --features falcon-hybrid --lib rustcrypto
cargo test --offline --release --features falcon --lib falcon1024_ct
cargo test --offline --release --features falcon --lib streaming_
cargo test --offline --release --features falcon-hybrid --lib falcon1024_ct
cargo test --offline --release --features falcon-hybrid --lib
```

The complete hybrid benchmark verifies every generated proof and reports
witness/commit time, proving time, verification time, total prover throughput,
and process peak RSS. Fetch dependencies once with `cargo fetch` before using
the offline benchmark script on a fresh checkout. Security must be selected explicitly:

```sh
scripts/bench_falcon_native.sh \
  --batch 32 --security 128 --seed 42 --threads 16 --warmup 1 --iterations 3
```

The legacy proof benchmark uses the same independent generator and seed:

```sh
BITZ_FALCON_BATCH=32 BITZ_FALCON_SEED=42 BITZ_BENCH_LAMBDA=128 \
  RAYON_NUM_THREADS=16 cargo bench --offline --profile release \
  --features falcon,span-metrics --bench falcon1024_ct
```

The upstream native-verification baseline is sequential and includes public-key
decoding; the prover and proof verifier use the configured Rayon thread pool.

The script builds for the host CPU with `-C target-cpu=native` in an isolated
`target/falcon-native` directory; this binary is hardware-specific. Ordinary
Cargo builds retain their portable defaults. Benchmark metadata reports the
selected GF(2^128) kernel and compiled features. Explicit `RUSTFLAGS` override
the script's default. To collect diagnostic stage timings, set
`BITZ_FALCON_STAGE_TIMINGS=1`; JSON stage records go to stderr and nested times
overlap. Run latency measurements separately with that variable unset.

Run larger batches only with sufficient memory. Every input is generated by the
independent Rust implementation. Compare the total prover column when witness
generation and commitments must count toward throughput; input generation and
public-statement decoding are excluded from this column.

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


## Native kernels and parallel prover: 16-thread results

The previous 44.41 ms/signature measurement used portable GF(2^128) arithmetic.
The native rebuild selects the existing PCLMUL kernel and available AVX-512,
VPCLMUL, and GFNI paths. Native baseline commitments exactly match the portable
baseline commitments. The library's default build remains portable; use the
native benchmark script above to reproduce accelerated timings.

A 2026-09-25 campaign on the same Ryzen 9 9950X3D used target 128, batch 256,
16 Rayon threads, release fat LTO, and one codegen unit. Native baseline and
optimized binaries each ran one warmup plus three measured trials, sequentially
without concurrent builds or tests. Every trial verified. The fixture repeats
the bundled signature and shared public key.

| Implementation | Witness + commitments | Proof generation | Full prover/signature | Verification/batch |
| --- | ---: | ---: | ---: | ---: |
| Previous reported portable build | 2017.27 ms | 9353.14 ms | 44.41 ms | 928.59 ms |
| Native rebuild of c1e75c53 | 46.47 ms | 6953.60 ms | 27.34 ms | 890.97 ms |
| Native with parallel binding/forest and compact Keccak groups | 33.50 ms | 1636.33 ms | **6.52 ms** | 919.24 ms |

Each phase is its own median; full prover uses the median of combined per-trial
times and includes witness generation, all commitments, and proof generation.
It excludes reusable preparation and verification. Optimized samples span
6.450–6.541 ms/signature, versus 27.264–27.404 ms for the matched native baseline.
This is 4.19x faster than that native baseline and 6.81x faster than the earlier
reported portable result, with throughput of 153.35 signatures/s. Verification
did not improve in this campaign. Process peak RSS across the trials fell from
2,690,364 KiB to 1,447,640 KiB.

A separate cold diagnostic profile identifies the remaining proof costs:

| Stage | Time/signature |
| --- | ---: |
| Prime arithmetic prefix | 3.38 ms |
| Prime-to-binary conversion | 2.23 ms |
| Joint binary sumcheck | 0.65 ms |
| Shared opening | 0.28 ms |
| Both Keccak prefixes | 0.17 ms |

The arithmetic prefix includes binding target construction (0.58 ms/signature),
binding coefficient streams plus inner sumcheck (1.54 ms), compaction forest
(0.80 ms), HashToPoint products (0.18 ms), norm (0.09 ms), and sampling/fingerprint
work. Nested spans overlap; do not add these arithmetic substeps to the
arithmetic total. This diagnostic run is excluded from the latency medians.

Raw JSONL, process timings, stage profile, and build metadata are retained in
`bench_results/falcon-native-20260925/` (an ignored local artifact directory).

A final single cold trial at batch 1024, target 128 and 16 threads verified
successfully: full prover 6.522 s (**6.37 ms/signature**), witness/commit 0.191 s,
proof generation 6.331 s, verification 3.599 s, and peak RSS 4,063,324 KiB.
This maximum-batch result is preliminary (one trial), not a multi-trial median.
It was run through `scripts/bench_falcon_native.sh` after the final test build.

## Compact word binding and merged bridge: 16-thread results

A second 2026-09-25 campaign compared the saved native `7a461976` executable
with compact word binding and the hybrid v3 merged bridge. Both used the same
Ryzen 9 9950X3D, native release kernels, target 128, batch 256, 16 Rayon threads,
and repeated bundled signature/shared key. Each ran one warmup and three
measured trials. Every trial verified and all three commitment roots exactly
matched the baseline. Our compilation and tests finished before the candidate
timing runs; cases ran sequentially.

| Metric | Native 7a461976, rerun | Compact binding + merged bridge |
| --- | ---: | ---: |
| Witness + commitments/batch | 35.54 ms | 34.61 ms |
| Proof generation/batch | 1717.50 ms | 1319.22 ms |
| Full prover/batch | 1752.67 ms | 1354.53 ms |
| Full prover/signature | 6.85 ms | **5.29 ms** |
| Verification/batch | 920.88 ms | **691.78 ms** |
| Whole-process peak RSS | 1,458,920 KiB | 1,611,272 KiB |

The full prover is 1.294x faster (22.7% less time); verification takes 24.9% less
time. Compact coefficient retention increases batch-256 process peak RSS by
10.4%. Phase medians are computed separately, while full prover uses each
trial's witness/commit plus proof time. Candidate samples span 5.288–5.424
ms/signature; baseline samples span 6.718–7.177. The earlier 6.52 ms result is
a historical measurement, not this campaign's matched baseline. Changing the
transcript changes deterministic grinding seeds; this repeated fixture does
not measure their variation across fresh signatures.

A separate cold profile measured binding target construction at **0.001
ms/signature**, coefficient emission plus inner sumcheck at **0.85 ms**, and
the complete prime-to-binary bridge at **1.41 ms**. The complete arithmetic
prefix was 2.66 ms, prime compaction forest 0.81 ms, joint binary sumcheck
0.68 ms, Keccak prefixes 0.19 ms, and shared opening 0.31 ms. These nested
diagnostic times are excluded from the medians. Integer folds and power tables
accounted for only 1.44 ms of the bridge's 360.07 ms/batch; its merged forest
remains the main bridge cost. Explicit grinding spans recorded 597.77 ms across
the whole diagnostic run and are subsets of the enclosing protocol stages.

One cold maximum-batch trial at 1024 signatures also verified: full prover
5.084 s (**4.96 ms/signature**), witness/commit 187.63 ms, proof 4.896 s,
verification 2.620 s, and process peak RSS 4,042,572 KiB. This is preliminary,
not a multi-trial median. Neither measured batch reaches 1 ms/signature.

The standalone binary HashToPoint candidate verified all trials at the FRI-128
setting, using one warmup and three samples per case:

| Batch | Witness + commitment + standalone proof/signature |
| --- | ---: |
| 1 | 4.41 ms |
| 16 | 1.98 ms |
| 256 | 2.10 ms |

Its per-signature circuit uses 35,336 AND constraints and 9,521 zero constraints,
with 41,498 live hidden words and 2,335 public words. The committed allocation is
65,536 64-bit words (2^22 bits), with no multiplication oracles. Reusable setup
at batch 256 took 24.39 s and is excluded from online timings. This experiment
alone already exceeds the 1 ms target, grows the source substantially, and has
not demonstrated a faster complete Falcon composition. It remains separate;
the default hybrid continues to prove SHAKE and HashToPoint with its existing
authenticated links and accounted security profile.

Validation passed 583 native release library tests, with zero failures and five
ignored tests, including the binary HashToPoint proof and boundary tampering,
compact/dense coefficient and transcript equivalence, both merged-forest
schedules, and compensating limb-sum forgeries. Serial Falcon and the legacy/new
benchmark clients passed `cargo check`. Raw trials, diagnostic spans, build/test
logs and executable/source hashes are retained in
`bench_results/falcon-word-binder-20260925/` (ignored local artifacts).

## GKR buffer reuse and AVX-512 grinding: 16-thread results

The next implementation preserves the hybrid v3 constraints, security schedule,
and transcript. It adds runtime-dispatched AVX512F BLAKE3 nonce scans (16 lanes,
with AVX2/scalar fallbacks), and uses 256-nonce work chunks for short AVX-512
parallel searches while retaining 1024-nonce chunks for longer searches. The
parallel threshold and grinding difficulties are unchanged. Isolated native
kernel measurements on the Ryzen 9 9950X3D measured about 1.92x AVX2 throughput.

The multi-claim bridge forest now supports contiguous storage, recycles its
buffers through a prepared-object workspace, and shares witness-bit selector
decoding across the two limbs. Limb weights and field products remain distinct.
Lookup tables move into their final consumers instead of being cloned. The
bridge also reuses the arithmetic prefix's canonical row weights. Scratch is
reset on geometry changes, bounded in retained capacity, and reuses matching
allocation size classes so small root layers cannot consume large JIT buffers.

Matched native release runs compared the saved `e5f46958` library with these
changes using the same updated benchmark harness, target 128, 16 Rayon threads,
one warmup and three measured trials per case. Every one of the 40 proofs
verified. Each of the 20 corresponding baseline/candidate pairs had identical
commitment roots and complete proof Debug digests. This digest is an exact-build
comparison aid, not a canonical wire encoding. All 15 measured trial pairs were
faster after the changes.

| Workload | Batch | Baseline ms/signature | Optimized ms/signature | Less time, ratio of medians |
| --- | ---: | ---: | ---: | ---: |
| Repeated bundled signature, shared key | 256 | 5.205 | **4.036** | 22.5% |
| Repeated bundled signature, shared key | 1024 | 5.138 | **3.842** | 25.2% |
| Four signatures, shared key | 256 | 5.400 | **4.494** | 16.8% |
| Eight signatures, two keys | 256 | 5.141 | **4.026** | 21.7% |
| Eight signatures, two keys | 1024 | 5.031 | **3.815** | 24.2% |

Full prover time includes witness generation, commitments, SHAKE, HashToPoint,
and all proof stages. It excludes fixture loading, public-statement decoding,
reusable setup, verification, and proof Debug hashing. Varied cases cycle their
four/eight valid fixture pool across the batch and rotate the starting offset
between trials; they are not batches of hundreds of unique signatures. The
fixtures were generated and checked with the local Falcon reference signer.
Grinding varies across these transcripts: paired trial reductions range from
11.4% to 27.5%, so unpaired minima/maxima should not be used to infer a speedup.
Process order alternates across cases; the three-sample medians still have
ordinary run-to-run uncertainty.

Buffer retention has a measurable memory cost. Repeated-fixture process peak
RSS grew from **1.52 to 2.03 GiB** at batch 256 and **5.14 to 7.16 GiB** at batch
1024. These are whole-process high-water marks including warmup, verification,
and retained workspaces. The existing `BITZ_FLAT_FOREST=0` override keeps the
lower-memory per-tree path while retaining the faster grinding kernel:

| Mode | Batch 256 ms/signature / peak RSS | Batch 1024 ms/signature / peak RSS |
| --- | ---: | ---: |
| Default flat forest with reuse | 4.036 / 2.03 GiB | 3.842 / 7.16 GiB |
| `BITZ_FLAT_FOREST=0` | 4.170 / 1.53 GiB | 3.990 / 5.14 GiB |

This separate ablation attributes roughly 3–4% additional whole-prover time
reduction to the flat/reuse path; most of the overall gain comes from grinding.
Both modes produced identical proofs. Retaining more tree levels was also
tested: the existing L4 schedule was about 10–12% slower than automatic L8 on
the baseline at these batch sizes and used more memory. L8 remains the default;
the unsupported multi-claim L2 schedule was not added.

Separate single-trial cold diagnostic runs measured grinding at **2.32 to 1.26
ms/signature** and the two inclusive GKR forest stages at **2.34 to 1.63 ms**.
Those timings overlap and are excluded from the latency medians. Diagnostic
events now record full span ancestry to attribute grinding to its enclosing
forest, arithmetic, or opening stage.

These historical runs used fixture manifests and file overrides. That harness
has been removed; current proof benchmarks generate distinct inputs with
`fn-dsa` as described above. The historical results remain for reference.
Validation passed **590 native release library tests**, with zero failures and
seven ignored tests. All six merged-forest tests also passed separately with
`BITZ_JIT_R1=0` and `BITZ_JIT_GRID=0`, covering the alternative JIT paths. The
serial Falcon configuration passed `cargo check`; native and generic-target
grinding checks matched scalar BLAKE3, including nonce carries and search tails.
Raw trials, generated fixtures, executable/source hashes, comparison scripts,
profiles, and validation logs are in
`bench_results/falcon-gkr-reuse-20260925/` (ignored local artifacts).
