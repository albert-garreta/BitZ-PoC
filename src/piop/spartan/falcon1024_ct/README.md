# Falcon-1024 CT proofs

The `falcon-hybrid` feature provides one non-ZK Falcon prover for 1–1024
signatures, using native-ring arithmetic, binary Keccak, and one shared opening
of three source commitments. It proves SHAKE256, HashToPoint rejection and
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

`FalconSourceLayout::new` rounds the live batch up to a power-of-two capacity.
There is one arithmetic layout and one compact binder. See
[NATIVE_RING.md](NATIVE_RING.md) for the native ideal proof and coordinate carry
bounds, and [COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for the ordered
compaction argument.

## Committed sources and constraints

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
using the wfbitz forest. The joint binary sumcheck binds arithmetic, both Keccak
claims, and these copy checks to the shared ring-switch/Ligerito opening.
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

The statement domains are `native-ring/non-zk/v4` and
`native-ring/statement/v4`. Earlier proofs must be regenerated. Current
subprotocol domain separators retain their own versions: those labels separate
live proof phases and do not enable old backends or old-proof parsing.

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

## Existing measurement reports

These reports describe the v3 implementation before backend retirement and the
current explicit budget. They are historical measurements, not results of this
cleanup; no runtime tests or benchmarks were executed for this change.

Current v3 kernel performance and validation are in
[KERNEL_THROUGHPUT.md](KERNEL_THROUGHPUT.md); the preceding v3 protocol changes
are measured in [THROUGHPUT.md](THROUGHPUT.md).
