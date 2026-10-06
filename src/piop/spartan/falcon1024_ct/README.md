# Falcon-1024 CT proofs

The `falcon-hybrid` feature provides non-ZK Falcon proofs for 1–1024
signatures, using binary Keccak and one shared opening of three source
commitments. It proves SHAKE256, HashToPoint rejection and
ordered compaction, the Falcon ring equation, and each signature's norm bound.
Native witness generation does not substitute for proof constraints.

The public statement contains each public key, 32-byte message, and exact CT
signature (nonce and `s2`). Every signature byte has a linear equality against
its committed bits. The auxiliary witness is not guaranteed to be hidden.
The `falcon` feature also exposes native verification and reference helpers;
it does not select a second proving backend.

## API

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

The inputs above are slices of byte slices. Preparation is reusable for the same
live batch size and security target (100 or 128). Proving consumes the committed
witness so large buffers can be folded without cloning. Proofs currently have
an in-memory Rust API, without a Falcon wire codec. `payload_size_bytes()`
counts stored proof payload, excluding public statements and transport framing.

`PreparedFalconHybrid::new_shared_prime(batch, target_bits)` selects the
experimental `SharedPrimeV2` protocol. `new` retains the native-coordinate-carry
protocol. Each prepared verifier accepts only its own protocol and layout.
Old shared-prime proofs must be regenerated; the native format is unchanged.
See [SHARED_PRIME_V2.md](SHARED_PRIME_V2.md) for the V2 bounds and transcript.

`FalconSourceLayout::new` rounds the live batch up to a power-of-two capacity.
Both profiles use the optimized integer relations and compact binder. See
[NATIVE_RING.md](NATIVE_RING.md) for the native ideal proof and coordinate carry
bounds, and [COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for the ordered
compaction argument.

## Shared-prime reduction

The opt-in path in [shared_ring.rs](shared_ring.rs) implements this schedule:

1. Commit the source bits, including public-key coefficients and the existing
   encoded signature bits used by `S2`. Work in
   `E = F_12289[theta]/(theta^11 + theta + 14)`.
2. Read the degree-1022 quotient encoding the ideal polynomial
   `e(Y) = (Y^1024 + 1)D(Y)`, then sample its evaluation point. Run the cubic
   signature sumcheck with all four operands `C,H,S2,S1` committed.
3. Batch the fixed operand endpoints and expand the affine decoders directly
   with position weights `beta^j`, retaining the live-signature mask and S2
   bit permutation. Include the geometric sum in the S1 affine offset. There
   is no coefficient-domain inner sumcheck.
4. Fix the 21 coefficients of the exact integer polynomial `P(T)`. Check their
   bounds and projection to `E`, then sample one prime in
   `[2^114, 2^115 - 2^102 - 1]` and the fresh projection point. Require
   `p > max(2 H_P, H_src)`, with `H_src = 43,013,625,445` over every
   decoder-allowed assignment. The upper cap also satisfies the conservative
   BitZ exponent bound.
5. Run the existing specialized norm, rejection, and compaction proofs using
   that prime. Combine their bit-linear claims with the projected ring query
   in [the streaming binder](opening_joined.rs), then run one bit-query
   sumcheck.
6. Continue through one unsplit product forest over the original bits and
   the shared PCS opening. The bridge retains 8192 rows, sends one bounded
   integer sum per column, and has no limb coordinate. Its final binary claim
   authenticates the original row and column slots.

The source profile appends 14,336 public-key bits and 1024 public-binding rows;
the live counts become 114,914 bits and 5482 linear rows per signature. Padded
strides remain 131,072 bits and 8192 rows. `S2` uses the original signature
slots, and both branches authenticate the same committed source bits.

The shared arithmetic proof stores only transmitted values. Verification
derives challenge points and reconstructs checked compaction endpoints before
passing them to the binder. Proof payload accounting uses this compact form.
Final V2 validation and matched performance results are pending. Historical
x86 and Mac measurements describe earlier shared-prime revisions and do not
establish parity for V2. The native route remains the default.

## Committed sources and constraints

The following counts describe the default native profile.

| Arithmetic source per signature | Bits |
| --- | ---: |
| Constant, message, and encoded signature | 12,873 |
| HashToPoint words | 20,976 |
| Quotients, bounded remainders, and rejection bits | 23,598 |
| Prefix counts | 14,432 |
| HashToPoint output | 14,336 |
| Biased centered `s1` | 14,336 |
| Norm slack | 27 |
| Total live arithmetic bits | **100,578** |
| Allocated arithmetic slot | **131,072** |

Each signature has 4,458 scalar linear rows padded to 8,192, and 1,311
rejection rows padded to 2,048. Norm, native ideal, compaction forest, and cubic
leaf reductions are separate obligations. No range-slack, selected-product,
prime-field Keccak, or integer ring-quotient columns are committed.

Keccak uses two permutation-major slabs of 16 and 4 permutations. Each
permutation occupies a 65,536-bit compact circuit block. At batch one, the
four-permutation slab has a second padded signature slot to satisfy Flock's
minimum geometry. Padded slots contain valid Keccak chains. The optimized
chain producer handles capacities at least eight; smaller capacities retain
the reference setup and packed producer.

Authenticated copy checks connect the sources:

1. The first Keccak input contains the committed nonce, public message, and
   exact SHAKE suffix, padding, and capacity zeros.
2. Every subsequent input equals the previous full 1600-bit output, including
   the transition between the two slabs.
3. Extracted rate bytes equal the 1,311 big-endian words consumed by HashToPoint.

The native arithmetic terminal passes through a two-limb integer-to-binary
bridge using the current BitZ PCS's [integer folds](../../../bitz/fold.rs) and
[product GKR](../../../bitz/forest.rs). The fixed binary source layout has
13 row variables; [the native bridge](hybrid_bridge.rs) splits prime-field row
weights into 113-bit limbs. Shared-prime V2 instead folds canonical weights
in the 115-bit prime field directly through the same integer-folding kernels.
The joint binary sumcheck binds arithmetic, both Keccak claims, and these copy
checks to the shared ring-switch/Ligerito opening.
Falcon uses `MATCHED_UDR` with no initial OOD message. Physical padding and all
three roots remain authenticated. See
[BRIDGE_GRINDING_AUDIT.md](BRIDGE_GRINDING_AUDIT.md) for the native bridge's
index map and error bound, and [SHARED_PRIME_V2.md](SHARED_PRIME_V2.md) for the
unsplit shared-prime bridge.

## Security and protocol domains

`PreparedFalconHybrid::security()` reports the complete configured error sum;
preparation rejects a profile below its requested target. The report uses the
repository's computational grinding convention: raw challenge error `e` with
`g` grinding bits contributes `e / 2^g` per unit of adversarial work. This is
not unconditional statistical soundness. BLAKE3's collision bound is separate.

The schedule allocates explicit budgets to protocol groups. At the 128-bit
target, V2 adds 11 bits to retained prime-challenge grinding difficulties to
account for its smaller prime family. Expected work per affected block rises
by 2048 times; this carries no prover-speed guarantee. At the 100-bit target,
integer-prime challenges retain zero grinding and projection retains two bits;
the complete bound is recalculated with the smaller prime. See
[SHARED_PRIME_V2.md](SHARED_PRIME_V2.md) for the V2 accounting and
[OPTIMIZATION_SECURITY.md](OPTIMIZATION_SECURITY.md) for the preceding
allocation and validation obligations. Complete-ledger validation must cover
both targets and every batch from 1 through 1024.

The native statement domains are `native-ring/non-zk/v4` and
`native-ring/statement/v4`; the shared-prime profile uses
`shared-prime/non-zk/v2` and `shared-prime/statement/v2`.
Earlier shared-prime proofs must be regenerated. Subprotocol domain
separators retain their own versions: those labels separate
live proof phases and do not enable old backends or old-proof parsing.
The native bridge retains `bitz/falcon-hybrid/wfbitz-joint-limbs/v1`; the V2
shared bridge uses separate unsplit binding, root-query, and grinding domains.
Both implementations use `crate::bitz`.

The shared opening resolves its Flock work budgets through
[`GrindingPlan`](../../../ligerito_flock/grinding_plan.rs). Its
[host-transcript adapter](../../../hybrid/opening/grinding.rs) checks the
plan against the selected configuration, validates the challenge-block and
nonce counts, and keeps the prover and verifier transcripts synchronized.

## Validation and benchmarking

Independent checks include native signature verification, upstream `fn-dsa`
interoperability, RustCrypto SHAKE comparisons, scalar/matrix reference
computations, transcript parity, source padding, and malformed-proof rejection.
The native exact checker recomputes the public-key/signature product rather
than trusting cached arithmetic. Reference implementations remain independent
of the optimized prover.

Compile-only validation:

```sh
cargo check --offline --locked --lib --tests --benches --features falcon-hybrid
cargo check --offline --locked --lib --no-default-features --features falcon
```

The retained benchmark generates distinct upstream keys, messages, and
signatures, converts through the canonical CT encoder, and verifies both the
upstream signature and the native BitZ relation. Total prover time includes
witness generation, all commitments, and every proof stage; key generation,
preparation, statement decoding, and verification are excluded.

```sh
scripts/bench_falcon_native.sh \
  --batch 1024 --security 128 --seed 42 --threads 16 --warmup 1 --iterations 3
```

The script selects native CPU instructions in an isolated target directory;
ordinary Cargo builds remain portable. `BITZ_FALCON_STAGE_TIMINGS=1` enables
diagnostic spans. Nested timings overlap and must not be added together.

The `falcon_hybrid` benchmark accepts `--protocol native|shared-prime`.
[`run_falcon_campaign.py`](../../../../scripts/run_falcon_campaign.py) records
the complete x86 diagnostic matrix at an exact clean revision, and
[`compare_falcon_benchmarks.py`](../../../../scripts/compare_falcon_benchmarks.py)
checks provenance, matched security settings, timing, proof size, and fresh
process peak memory. A diagnostic matrix alone does not establish parity;
the timing acceptance gate requires multiple seeds and paired process runs.

The following measurements describe pre-V2 shared-prime implementations.
Final V2 validation and benchmark results are pending.

The pre-V2 shared-prime implementation evaluates the verifier's canonical ring
projection directly at the binding endpoint and shares its equality-weight
buffers with the integer verifier. The prover keeps the ring tensor separate
from the unscaled integer query while folding one bit table. Shared integer
proofs omit each round's linear coefficient, reconstructing it before the
original transcript absorption; verification retains only endpoint claims.
Those earlier changes preserved the field choices, grinding schedule, and
authenticated relation. Their incremental payload saving is `1552 + 64*m`
bytes, where `m = log2(padded_batch)`.

[`run_falcon_paired.py`](../../../../scripts/run_falcon_paired.py) runs balanced
AB/BA processes against the preserved native baseline executable. Its default
covers five seeds, thirty process pairs per cell, and the complete matrix;
subset runs remain diagnostic and cannot satisfy the full acceptance gate.

At `764b7b05`, will (Ryzen 9 9950X3D, Rust 1.98.1, release,
`-C target-cpu=native`) passed 110 Falcon tests and four factored-overlay tests,
and verified all 112 proofs in the 16-case diagnostic matrix. The compression
changes the stored proof representation; focused tests check reconstruction,
rejection of malformed messages, and parity of the expanded transcript.
Against native baseline `4491309f`, seed-42 medians after one warmup and five
measured proofs were (prover time includes witness commitment):

| Security / batch / threads | Native prover ms | Shared prover ms | Native verifier ms | Shared verifier ms |
| --- | ---: | ---: | ---: | ---: |
| 100 / 1 / 1 | 33.85 | 38.76 | 14.92 | 15.98 |
| 100 / 1024 / 16 | 790.60 | 802.54 | 71.79 | 61.16 |
| 128 / 1 / 1 | 170.99 | 189.85 | 15.21 | 16.51 |
| 128 / 1024 / 16 | 894.12 | 954.20 | 75.54 | 63.14 |

These runs do not establish timing parity. The strict comparison still reports
fresh-process peak memory increases in 10 of 16 cases. Both batch-1024,
16-thread memory measurements decreased, but several small-batch timings
remain slower than native. Relative to shared-prime revision `1d62b520`, the
verifier median decreased in all 16 cases; the prover median decreased in
eight. Single-seed timings do not isolate grinding variance.

In the seed-42 matrix, measured canonical stored payload decreases against
native in every tested security/batch combination, including the previous
100-bit, batch-one regression (207,104 native bytes versus 206,636 shared-prime
bytes). These sizes are for seed 42; query-dependent encodings can vary:

| Signatures | Shared 100-bit bytes | Shared 128-bit bytes |
| --- | ---: | ---: |
| 1 | 206,636 | 241,068 |
| 3 (capacity 4) | 281,836 | 336,260 |
| 32 | 438,412 | 523,020 |
| 1024 | 1,867,852 | 2,011,036 |

The subsequent five-seed paired diagnostic at 16 threads completed 40 pairs
and 480 verified proofs. It found single-signature payload increases of
2,092 bytes at 100-bit security/seed 45 and 100/420 bytes at 128-bit
security/seeds 45/46. Single-signature fresh-process RSS increased by
504–936 KiB. At batch 1024, payload and RSS decreased for every matched seed;
current payload ranged from 1,867,852–1,871,980 bytes at 100 bits and
2,010,396–2,017,820 bytes at 128 bits. The precise components responsible for
these seed-dependent differences have not been measured.

| Security / batch (16 threads) | Paired prover ratio | Paired verifier ratio |
| --- | ---: | ---: |
| 100 / 1 | 1.0995 | 1.0755 |
| 128 / 1 | 1.0724 | 1.0646 |
| 100 / 1024 | 1.0190 | 0.8717 |
| 128 / 1024 | 1.0383 | 0.8486 |

Ratios are geometric means of matched process medians, shared/native. This
diagnostic subset does not satisfy the full timing acceptance requirements;
the strict payload/RSS gates remain failed. Paired artifacts are under
`.tmp/falcon-paired-764b7b05559ddf9257242032d9b518e23f1e419f`.

This accounting excludes Falcon transport framing and the public statement.
At batch 1024, the two-limb bridge's integer sums alone occupy 524,288 bytes
(`2 * 16384 * 16`); the compaction forest's child evaluations occupy another
720,896 bytes (`11 * 2048 * 2 * 16`). These are retained proof components,
not temporary allocations. The final PCS already uses one shared opening.

Isolated one-thread kernel measurements reduced ring endpoint evaluation from
2.428 to 1.128 ms. The factored binding kernel decreased by 2.2%, 3.2%, and
6.8% at batches 1, 3, and 32, respectively. These are kernel measurements,
not end-to-end speedups. Grinding diagnostics retained nonce and transcript
parity across 450 measurements; neither alternative chunk size consistently
improved the existing scheduler, so production grinding remains unchanged.

Raw manifests and logs are retained under
`.tmp/falcon-shared-candidate-764b7b05559ddf9257242032d9b518e23f1e419f` and
`.tmp/falcon-regression-fixes-764b7b05559ddf9257242032d9b518e23f1e419f` on will
and in the implementation worktree.

## x86 GKR optimization

Revision `e370e3b3` retains two changes to the x86 BitZ kernels: four-lane
arithmetic for JIT bucket accumulation, and direct 128-bit loads when assembling
four lookup values for the JIT fold. Repeated bucket indices accumulate in lane
order. The fold checks complete tables once per group and retains checked
lookup for shorter tables. Other architectures retain their existing kernels.
Field choices, integer bounds, grinding, proof messages, and source bindings
are unchanged.

The preceding `764b7b05` profile on will, at batch 1024 and 16 threads, identified
the following costs at the 100-bit target. Each number is a median of six
instrumented trials across seeds 42, 43, and 44:

| Stage | Milliseconds |
| --- | ---: |
| GKR forest | 143.00 |
| Witness generation and commitments | 140.82 |
| Shared bit binding | 129.88 |
| Ring proof | 7.24 |

The bucket microbenchmark improved by 37–82% across its tested group counts
and patterns. The production fold microbenchmark improved by approximately
24–29%. These percentages describe isolated kernels, not complete proofs.
An alternative gather kernel was slower. A ring-row contraction experiment
gave little parallel benefit and added scratch storage, so it was removed.

The retained code passed 78 BitZ tests, 110 Falcon tests, and four factored
binding tests on will. Differential coverage includes repeated bucket indices,
every vector tail length, both sumcheck endpoints, fold modes, short tables,
and malformed shapes. Kernel logs are retained under
`.tmp/falcon-final-validation-e370e3b35da692d86e814c687e597c3a1d0a6bea` and
`.tmp/falcon-jit-validation-75d49dcda0fd3b91203385e3151f75110fbf0307`.

The final seed-42 matrix verified 112 proofs with the same Debug digests and
payload sizes as `764b7b05`. Total prover medians decreased in 13 of 16 cells.
The exceptions were both 16-thread batch-32 cells (0.74% and 0.37% higher) and
the 128-bit, 16-thread batch-1024 cell (8.46% higher). Fresh-process RSS
increased in six cells by 28–1812 KiB. The strict comparison therefore remains
failed, and this single-seed matrix does not establish timing parity. Raw
results are under
`.tmp/falcon-shared-candidate-e370e3b35da692d86e814c687e597c3a1d0a6bea` and
`.tmp/falcon-bottleneck-e370e3b35da692d86e814c687e597c3a1d0a6bea`.

Separate profiles measured the GKR forest at 138.167 ms (previously 142.996)
at 100 bits and 171.744 ms (previously 174.417) at 128 bits. These are medians
of six instrumented trials per target, at batch 1024 and 16 threads. All 18
profiled proof runs also preserved the reference digest and payload.

An uninstrumented AB/BA comparison then alternated the saved `764b7b05` and
`e370e3b3` binaries at batch 1024 and 16 threads, using seeds 42, 43, and 44.
Each process ran one warmup and three measured proofs. The 12 pairs verified
96 proofs with identical digests and payloads across both revisions and orders.
Geometric means of the six process-median ratios per target were:

| Security | Total prover ratio | Proof-only ratio | Verifier ratio |
| --- | ---: | ---: | ---: |
| 100 | 0.98683 | 0.98357 | 0.99722 |
| 128 | 0.99452 | 0.99461 | 1.00026 |

Total prover time includes witness generation and commitments. The observed
decreases are 1.32% and 0.55%; these diagnostic pairs do not satisfy the full
timing acceptance campaign. The isolated 8.46% slower case was not reproduced
as an aggregate slowdown in this paired run, but neither that result nor the
kernel measurements establishes complete parity against the original native
prover. The stricter matrix and RSS findings above remain recorded. Artifacts:
`.tmp/falcon-current-profiles-e370e3b35da6` and
`.tmp/falcon-incremental-pairs-e370e3b35da692d86e814c687e597c3a1d0a6bea`.

## Existing measurement reports

These reports describe earlier implementations and budgets. They are historical
measurements, not results for the shared-prime backend.

Current v3 kernel performance and validation are in
[SIMD_THROUGHPUT.md](SIMD_THROUGHPUT.md). Earlier kernel work is measured in
[KERNEL_THROUGHPUT.md](KERNEL_THROUGHPUT.md), and the preceding v3 protocol
changes are measured in [THROUGHPUT.md](THROUGHPUT.md).
The broader [seed validation](SEED_VALIDATION.md) records slower cases as well:
nine of twelve additional seed medians exceeded 1,000 signatures/second.
The [nonce investigation](GRINDING_VARIANCE.md) explains the repeatable slow case
and identifies an analytical PCS grinding-budget reallocation to investigate.
