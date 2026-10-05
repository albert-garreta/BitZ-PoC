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
experimental shared-prime protocol. `new` retains the native-coordinate-carry
protocol. Each prepared verifier accepts only its own protocol and layout.

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
3. Batch the fixed operand endpoints and run the degree-two coefficient
   sumcheck. Expand the actual affine decoders to a tensor query on those bits.
4. Fix the 21 coefficients of the exact integer polynomial `P(T)`. Check their
   bounds and projection to `E`, then sample one 126-bit prime and the fresh
   projection point. Require `p > max(2 H_P, H_src)`, with
   `H_src = 43,013,625,445` over every decoder-allowed assignment.
5. Run the existing specialized norm, rejection, and compaction proofs using
   that prime. Combine their bit-linear claims with the projected ring query
   in [the streaming binder](opening_joined.rs), then run one bit-query
   sumcheck.
6. Continue through the existing two-limb binary bridge and shared PCS opening.
   This implementation keeps 8192 bridge rows and radix `2^113`; it does not
   instantiate the document's unsplit-bridge example.

The source profile appends 14,336 public-key bits and 1024 public-binding rows;
the live counts become 114,914 bits and 5482 linear rows per signature. Padded
strides remain 131,072 bits and 8192 rows. `S2` uses the original signature
slots, and both branches authenticate the same committed source bits.

The shared arithmetic proof stores only transmitted values. Verification
derives challenge points and reconstructs checked compaction endpoints before
passing them to the binder. Proof payload accounting uses this compact form.
The full-prover performance gate remains open: the initial x86 campaign found
regressions, and the new route is not the default.

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

The arithmetic terminal passes through a two-limb integer-to-binary bridge
using the current BitZ PCS's [integer folds](../../../bitz/fold.rs) and
[product GKR](../../../bitz/forest.rs). The fixed binary source layout has
13 row variables; [the bridge](hybrid_bridge.rs) splits prime-field row weights
into 113-bit limbs before folding each limb through `bitz::fold::fold_columns`.
The joint binary sumcheck binds arithmetic, both Keccak claims, and these copy
checks to the shared ring-switch/Ligerito opening.
Falcon uses `MATCHED_UDR` with no initial OOD message. Physical padding and all
three roots remain authenticated. See
[BRIDGE_GRINDING_AUDIT.md](BRIDGE_GRINDING_AUDIT.md) for the bridge's index map
and error bound.

## Security and protocol domains

`PreparedFalconHybrid::security()` reports the complete configured error sum;
preparation rejects a profile below its requested target. The report uses the
repository's computational grinding convention: raw challenge error `e` with
`g` grinding bits contributes `e / 2^g` per unit of adversarial work. This is
not unconditional statistical soundness. BLAKE3's collision bound is separate.

The current schedule allocates explicit budgets to current protocol groups;
it does not reconstruct historical schedules. See
[OPTIMIZATION_SECURITY.md](OPTIMIZATION_SECURITY.md) for the allocation and
validation obligations. Both targets and every batch from 1 through 1024 are
covered by the complete-ledger tests.

The native statement domains are `native-ring/non-zk/v4` and
`native-ring/statement/v4`; the shared-prime profile uses
`shared-prime/non-zk/v1` and `shared-prime/statement/v1`. Earlier proofs must be
regenerated. Current
subprotocol domain separators retain their own versions: those labels separate
live proof phases and do not enable old backends or old-proof parsing.
The bridge retains `bitz/falcon-hybrid/wfbitz-joint-limbs/v1` as a transcript
label while its implementation uses `crate::bitz`.

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

The shared-prime implementation now evaluates the verifier's canonical ring
projection directly at the binding endpoint and shares its equality-weight
buffers with the integer verifier. The prover keeps the ring tensor separate
from the unscaled integer query while folding one bit table. Shared integer
proofs omit each round's linear coefficient, reconstructing it before the
original transcript absorption; verification retains only endpoint claims.
These changes preserve the field choices, grinding schedule, and authenticated
relation. The incremental payload saving is `1552 + 64*m` bytes, where
`m = log2(padded_batch)`.

[`run_falcon_paired.py`](../../../../scripts/run_falcon_paired.py) runs balanced
AB/BA processes against the preserved native baseline executable. Its default
covers five seeds, thirty process pairs per cell, and the complete matrix;
subset runs remain diagnostic and cannot satisfy the full acceptance gate.

At `1d62b520`, will (Ryzen 9 9950X3D, Rust 1.98.1, release,
`-C target-cpu=native`) passed 108 Falcon tests and verified all 112 proofs in
the 16-case diagnostic matrix. Every proof digest matched the pre-optimization
shared-prime revision `344a1656`. Against native baseline `4491309f`, seed-42
medians after one warmup and five measured proofs were:

| Security / batch / threads | Native prover ms | Shared prover ms | Native verifier ms | Shared verifier ms |
| --- | ---: | ---: | ---: | ---: |
| 100 / 1 / 1 | 33.85 | 36.55 | 14.92 | 16.86 |
| 100 / 1024 / 16 | 790.60 | 808.69 | 71.79 | 62.20 |
| 128 / 1 / 1 | 170.99 | 187.02 | 15.21 | 17.96 |
| 128 / 1024 / 16 | 894.12 | 1010.13 | 75.54 | 65.47 |

These runs do not establish timing parity. The strict gate also reports peak
memory increases in several cases and a 1084-byte payload increase for the
100-bit, batch-one proof (207,104 to 208,188 bytes). Payloads decreased for the
other tested security/batch combinations. Full raw campaign manifests and
stage logs are retained under `.tmp/falcon-shared-candidate-1d62b520c22f9f57db3afb52a5b311ea7205a3cd`
and `.tmp/falcon-shared-profiles-1d62b520c22f` on will and in the implementation
worktree.

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
