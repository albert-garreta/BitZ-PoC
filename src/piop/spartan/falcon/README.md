# Falcon proofs

The `falcon-hybrid` feature proves batches of 1–1024 Falcon-512 or Falcon-1024
signatures using SharedPrime. The proof checks SHAKE256, HashToPoint rejection
and ordered compaction, the Falcon ring equation, and each signature's norm.
It is not zero knowledge. Public inputs include each public key, 32-byte message,
and exact constant-time signature encoding (nonce and `s2`).

## API

```rust,ignore
use bitz::piop::spartan::falcon_profiles::{
    Falcon1024_128, FalconPublicStatement, PreparedFalconHybrid,
};

let prepared = PreparedFalconHybrid::<Falcon1024_128>::new(public_keys.len())?;
let public = FalconPublicStatement::<Falcon1024_128>::from_bytes(
    &public_keys, &messages, &signatures,
)?;
let committed = prepared.commit(public)?;
let statement = committed.statement.clone();
let proof = prepared.prove(committed)?;
prepared.verify(&statement, &proof)?;
```

Preparation is reusable for the same live batch. Proving consumes the committed
witness so large buffers can be folded without cloning. The four presets are
`Falcon512_100`, `Falcon512_128`, `Falcon1024_100`, and `Falcon1024_128`.
[PROFILES.md](PROFILES.md) describes explicit extension selection and validation.
The `falcon` feature retains direct signature verification and reference helpers.

There is one proving protocol and no NativeCarry or old-proof verifier. The
protocol identifier is `bitz/falcon/shared-prime/non-zk/v1`. Proofs must be
regenerated. Falcon currently exposes in-memory proofs without a full transport
codec. `payload_size_bytes()` counts stored messages, excluding the public
statement and transport framing; `payload_size_breakdown()` reports disjoint
components. Column sums have their own canonical encoding.

## Reductions

1. Generate live Keccak computations and arithmetic bits. Inactive Keccak
   instances have constant wire zero and an all-zero witness; active instances
   have constant wire one. Commit one Merkle tree over complete canonical
   encoded rows of the joint source. Logical source projections do not create
   separate source trees. Bind the statement, root, layout, activity policy,
   circuit identifiers, and security parameters before challenges.
2. In `E = F_12289[T]/(T^k + T + c_k)`, prove the ring identity using its quotient
   polynomial, a signature sumcheck, and an operand batch over `C,H,S2,S1`.
   Transpose affine decoders into bit coefficients, combining aliases before
   canonical integer lifting. Keep signature and local-position coefficients
   factored.
3. Fix the `2k−1` coefficients of an integer polynomial and check its evaluation
   in `E`. Sample the arithmetic prime and fresh projection point. Check the
   polynomial against the projected committed bits. The inclusive prime
   intervals are `[2^114, 2^115−2^102−1]` at 100 bits and `[2^125, 2^126−1]`
   at 128 bits. Coefficient and source-residual bounds prevent wraparound.
4. Run norm, rejection, and joint compaction sumchecks on the smaller arithmetic
   source. Store only compact sumcheck messages and endpoint values; derive
   challenge points during verification. The binder combines exact public
   equalities and authenticated endpoints with the factored ring projection,
   then reduces them to one arithmetic-source evaluation.
5. The BitZ bridge checks integer column sums and authenticates the same bits
   in the binary field. Use one unsplit sum per column at 100 bits; at 128 bits
   use two bounded limbs, encoding the upper limb in four bytes. Both modes
   produce one binary evaluation claim.
6. Binary Keccak proofs, public wiring, activity and padding checks, and the
   bridge claim join one binary sumcheck. Ring switching and recursive PCS
   folding authenticate its endpoint against the joint source root. There is
   one initial source multiproof; recursive folding commitments remain.

See [ONE_SOURCE.md](ONE_SOURCE.md) for the projection geometry,
[COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for ordered compaction, and
[BRIDGE_GRINDING_AUDIT.md](BRIDGE_GRINDING_AUDIT.md) for bridge bounds.

## Layout and performance

Falcon-1024 uses 114,914 live arithmetic bits and 5,482 linear rows per signature,
padded to strides of 131,072 bits and 8,192 rows. Public-key bits account for
14,336 bits; `S2` uses the existing encoded-signature slots. At batch 1,024 the
column table occupies 262,144 bytes at 100 bits and 327,680 bytes at 128 bits.
It remains a substantial proof-size cost.

The implementation preserves packed source buffers, factored ring coefficients,
compiled binder templates, compact zero-lane and final-message encodings, and
SIMD Keccak/grinding kernels. Simplification does not change security targets,
code rates, query counts, or grinding requirements.

Runtime qualification compares fresh builds against `5f9b23edd`, with both
Falcon degrees, both security targets, and batches 1, 3, 32, and 1,024. Every case
uses 60 fixed seeds, one warmup and three measured repetitions, 16 threads,
balanced execution order, and paired bootstrap bounds. Both total proving and
verification must have a one-sided 95% upper time ratio at most 1.02. The payload
gate compares equivalent public query shapes; changed transcript challenges
can change authentication-path lengths. Partial measurements are not a pass.
