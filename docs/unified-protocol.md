# The unified BitZ protocol runner

Every benchmark relation used to carry its own copy of the Fiat–Shamir
protocol: statement binding, prime draw, grinding boundaries, Spartan
PIOP, bitification, bridge digest and opening — one per relation, tens of
thousands of lines in total. Since branch `unified-piop` there is one
protocol module, `src/piop/spartan/protocol/`, and each relation only
describes itself. The transcripts are byte-for-byte those of the
protocols the module replaced; `tests/transcript_state_pins.rs` pins the
prover and verifier transcript states (plus the proof bytes) of every
relation at a small shape and fails on any drift.

## Layout

| Path | Role |
|---|---|
| `protocol/mod.rs` | the runner: `commit`, `prove`/`verify` (direct exponent-fold discharge), `prove_virtual`/`verify_virtual` (virtual map onto a derived grid), `prove_reduced`/`verify_reduced` (Strategy 2: exact integer lift + grinded fresh-prime reduction), `prove_prefix`/`verify_prefix` (statement → bitified claim, for callers with their own discharge) and the `terminal` sub-API used by the bench-internals probes |
| `protocol/binding.rs` | `BindingHasher`: the canonical BLAKE3 encodings every assignment binding and claim digest are built from |
| `protocol/bitify.rs` | `BlockTable`, `bitify`, `bridge_digest`, the `Modular` arithmetic trait (`ProjArith` below 2^126, `FieldArith` for full-width fingerprint primes) |
| `protocol/linear.rs` | the linear-batching mode: relations whose constraints are all linear in a derived bit grid (no nonlinear outer sumcheck) — `prove_linear`/`verify_linear` |

The paper's §2.1 steps map onto the runner as follows. A `RelationSpec`
supplies everything marked *relation*; the runner owns the rest.

1. **Statement.** *relation*: the assignment binding (one BLAKE3 digest
   over the commitment, the relation digest, the layout and the security
   profile) and its statement tag. Runner: the Ligerito policy digest and
   Round 0 (the out-of-domain sample) when the `Schedule` enables them.
2. **Prime.** Runner: the initial grinding boundary, then the Step-2 prime
   draw under the relation's `prime_sampling` domain (`prime-domain`,
   `prime-min`, `prime-max`, the transcript-driven draw, `prime-q`).
   Relations with their own draw (MultiSwap's full-width fingerprint
   prime, the fixed CM-AND field) override `runtime_prime`. *relation*:
   the projection of its matrices to the runtime field (`MatrixSource`:
   a q-independent skeleton projected on demand, fixed matrices, or
   per-prime construction).
3. **PIOP.** Runner: the Spartan PIOP over the runtime field — the plain
   cubic outer sumcheck or the K=3 univariate-skip kernel (`Kernel`), then
   ρ and the inner sumcheck — every drawn challenge behind one grinding
   boundary at the profile difficulty when the schedule grinds the PIOP.
   *relation*: the `PiopWitness` representation its kernel consumes
   (dense native, raw Montgomery products, or field elements).
4. **Bitification.** Runner: the terminal Spartan claim is turned into one
   functional over the committed bit tensor through the relation's
   `BlockTable` (which assignment blocks occupy which bit slots, scaled on
   the row or the column side), bound through the bridge digest, and the
   terminal boundary is ground.
5. **Discharge.** Runner: the direct runtime-q exponent fold, the virtual
   opening through the relation's `VirtualMap`, or the Strategy-2 lift,
   grinded reduction prime and reduced opening.

## The relations

| Relation | Spec | Mode | Kernel |
|---|---|---|---|
| u32×u32→u64 (`bitz.rs`) | `U32MulLayout` | direct | univariate skip K=3 |
| u64×u64→u128 (`u64_bitz.rs`) | `U64MulLayout` | direct | plain |
| u128×u128→u256 (`u128_bitz.rs`) | `U128MulLayout` | direct | plain |
| BabyBear (`baby_bear_bitz.rs`) | `BabyBearMulLayout` | direct | plain |
| RSA MultiSwap (`multiswap/proof.rs`) | `MultiswapSpec` | reduced (Strategy 2) | plain |
| CM-AND (`cm.rs`) | `CmAndSpec` | virtual | plain |
| Hybrid mod-2^32 × SHA (`bitz/hybrid.rs`) | `U32MulLayout` prefix | `prove_prefix` + the hybrid's own forest discharge | univariate skip K=3 |
| SHA-256 compressions (`sha256/proof.rs`) | `Sha256CompressionSpec` | linear (product opening, or legacy inner sumcheck for non-power-of-two batches) | — |
| SHA-256 chain (`sha256/chain.rs`) | `PreparedSha256ChainBatch` | linear (product opening) | — |
| SHA-256 + P-256 ECDSA (`ecdsa_sha256/proof.rs`) | — | its own composite kernel (grinded outer sumcheck, batched matrix + linear rows in one inner sumcheck, opening with per-block flock grinding) on the shared boundaries | composite |

The per-relation `prove_*`/`verify_*` functions the benches call are thin
wrappers over the runner; their proof types are aliases of the runner's
`Proof` / `LinearProof`, and their error types are aliases of the one
`ProtocolError`.

The linear mode (`LinearRelationSpec`) shares the runner's statement,
policy, Round-0, boundary and prime steps and adds the linear PIOP: the
local-row point ξ and the instance point η, the terminal boundary, the
constant-one and public-I/O batch challenges, the relation's batching of
its collapsed constraints (`LinearBatching`), the rank-one opening claim
(or the legacy quadratic inner sumcheck first), the claim frame and the
virtual opening of the derived grid.

## Pins

`tests/transcript_state_pins.rs` records, per relation and profile, the
BLAKE3 state of the prover's and the verifier's transcript at the end of
the protocol (they differ by design: flock's Ligerito desynchronises the
two after its last level) and a digest of the proof bytes, all recorded
on the pre-refactor code (master `5d2aea9`). Run

```sh
RUSTFLAGS="-C target-cpu=native" cargo test --release --features unchecked --test transcript_state_pins
RUSTFLAGS="-C target-cpu=native" cargo test --release --features unchecked,bench-internals --test transcript_state_pins
RUSTFLAGS="-C target-cpu=native" cargo test --release --features hybrid,ecdsa,bench-internals --test transcript_state_pins
```

`BITZ_RECORD_PINS=1` prints the pin tuples for re-recording after a
deliberate transcript change.
