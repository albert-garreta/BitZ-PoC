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
protocol identifier is `bitz/falcon/shared-prime/non-zk/v5`. Proofs must be
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
2. Send canonical packed public selection masks with exactly N ones each.
   Bind them before the ring challenges. They specify linear routing; the
   arithmetic proof checks them against the SHAKE-authenticated rejection bits.
   In `E = F_12289[T]/(T^k + T + c_k)`, prove the ring identity using its quotient
   polynomial, a signature sumcheck, and an operand batch over `C,H,S2,S1`.
   Transpose affine decoders into bit coefficients, combining aliases before
   canonical integer lifting. Keep signature and local-position coefficients
   factored.
3. Fix the `2k−1` coefficients of an integer polynomial and check its evaluation
   in `E`. Sample the arithmetic prime and fresh projection point. Check the
   polynomial against the projected committed bits. The inclusive prime
   intervals are `[2^114, 2^115−2^102−1]` at 100 bits and `[2^125, 2^126−1]`
   at 128 bits. Coefficient and source-residual bounds prevent wraparound.
4. Run one cubic integer outer sumcheck combining the norm and quadratic
   rejection rows. Compute the public S2 norm directly; project the S1
   endpoint, slack, and three rejection endpoints to the committed source.
   Public-mask validity and selected coefficient routing are linear constraints
   on existing committed bits. There are no intermediate HashToPoint polynomial
   witnesses, and no HashToPoint grand-product/GKR proof.
   Store only compact sumcheck messages and endpoint values; derive
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

Falcon-1024 uses 100,482 live arithmetic bits and 6,504 linear rows per signature,
padded to strides of 131,072 bits and 8,192 rows. The first `4*N*16` positions
form aligned coefficient blocks ordered `S1,S2,C,H`. The coefficient encodings
remain bounded14 minus 6144, signed12, unsigned14, and unsigned14 respectively.
Norm slack bits occupy lane 15 of the first 27 `S1` coefficient rows. Signature
header/nonce bytes and the remaining HashToPoint columns follow these blocks;
the exact CT payload is reconstructed from the aligned `S2` bits without a
second copy. SHAKE nonce and sample links use the same address map.

For Falcon-1024 there are 10,213 internal padding bits and 20,377 trailing padding
bits. The occupied extent is 110,695, distinct from the 100,482 live-bit count.
Every internal hole, trailing bit, and inactive signature is constrained to zero.
Use the layout's coefficient, signature-byte, and slack address methods rather
than assuming a contiguous live prefix. Keccak domains retain their previous
sizes. Falcon-512 uses 52,637 live bits in a 65,536-bit stride and 3,524 linear
rows in a 4,096-row stride. Quadratic rejection domains have 1,024 / 2,048
rows. Compared with the previous grand-product protocol, removing prefix
counters saves 7,180 / 14,432 live bits without changing the padded domains.
Masks add 90 / 164 bytes per signature to the proof payload, outside the source.
At batch 1,024 the Falcon-1024 column table occupies 262,144 bytes at 100 bits
and 327,680 bytes at 128 bits. It remains a substantial proof-size cost.

The implementation preserves packed source buffers, factored ring coefficients,
compiled binder templates, compact zero-lane and final-message encodings, and
SIMD Keccak/grinding kernels. Security targets, code rates, and query counts
are unchanged. At target 128, PCS grinding is reallocated using only public error bounds:
the new summed error cannot exceed the previous allocation, every native
minimum remains enforced, and the schedule is bound into the statement.

HashToPoint qualification uses the full Falcon archive at
`results/falcon-power-basis-20261007/results-candidate`, for both degrees,
both targets, batch 1,024, seed 42, and 1/2/4/8/16 threads. Preserve its build,
affinity, one-warmup/five-sample schedule, and input digests. Total prover time
and verification time must not increase. Process peak RSS and payload are
reported separately, following the user's timing-first qualification. The comparison
script `scripts/compare_falcon_h2p.py` also requires independent-bootstrap
one-sided 95% upper timing ratios at most 1.00. This estimates timing variation
on the archived fixed input, not multi-seed grinding variation. Partial
measurements never qualify. Algebraic Falcon
is outside this performance campaign.
