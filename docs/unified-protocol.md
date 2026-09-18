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
| `protocol/mod.rs` | the runner: `commit`, `prove`/`verify` (relation-selected direct or virtual discharge), `prove_virtual`/`verify_virtual` (virtual map onto a derived grid), `prove_reduced`/`verify_reduced` (Strategy 2: exact integer lift + grinded fresh-prime reduction), `prove_piop`/`verify_piop` (after statement binding → bitified claim, for callers with their own discharge) and the `terminal` sub-API used by the bench-internals probes |
| `protocol/binding.rs` | `BindingHasher`: the canonical BLAKE3 encodings every assignment binding and claim digest are built from |
| `protocol/bitify.rs` | `BlockTable`, `bitify`, `bridge_digest`, using the existing runtime field arithmetic |
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
| u32×u32→u64 (`bitz.rs`) | `MulLayout<u32>` | direct for W=1/8; virtual otherwise | univariate skip K=3 |
| u64×u64→u128 (`u64_bitz.rs`) | `MulLayout<u64>` | direct for W=1; virtual otherwise | plain |
| u128×u128→u256 (`u128_bitz.rs`) | `MulLayout<u128>` | direct for W=1; virtual otherwise | plain |
| BabyBear (`baby_bear_bitz.rs`) | `BabyBearMulLayout` | direct | plain |
| RSA MultiSwap (`multiswap/proof.rs`) | `MultiswapSpec` | reduced (Strategy 2) | plain |
| CM-AND (`cm.rs`) | `CmAndSpec` | virtual | plain |
| Hybrid mod-2^32 × SHA (`bitz/hybrid.rs`) | `MulLayout<u32>` prefix | `prove_piop` + the hybrid's own forest discharge | univariate skip K=3 |
| SHA-256 compressions (`sha256/proof.rs`) | `Sha256CompressionSpec` | linear (product opening, or legacy inner sumcheck for non-power-of-two batches) | — |
| SHA-256 chain (`sha256/chain.rs`) | `PreparedSha256ChainBatch` | linear (product opening) | — |
| SHA-256 + P-256 ECDSA (`ecdsa_sha256/proof.rs`) | — | its own composite kernel (grinded outer sumcheck, batched matrix + linear rows in one inner sumcheck, opening with per-block flock grinding) on the shared boundaries | composite |

Multiplication and BabyBear callers use the shared runner directly. Relation
proof, claim, preparation and error aliases have been removed; callers name
`Proof`, `LinearProof`, `PreparedRelation<Spec>`, `BitifiedClaim` and
`ProtocolError`. Wrappers that perform relation-specific preparation or
opening policy selection remain.

The multiplication witness interface is described in
[the native witness section below](#native-multiplication-witnesses).

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

## Native multiplication witnesses

`MulLayout<T>`, `MulWitness<T>` and `MulRow<T>` support `u32`, `u64` and
`u128`. A witness stores four padded native blocks `[x | y | lo | hi]`.
The blocks use separate buffers to preserve allocator reuse for partial
batches. The constant assignment block is implicit. `from_fn`
computes exact product limbs; `from_rows` preserves supplied limbs so that
incorrect claims remain detectable.

```rust
use bitz::piop::spartan::{MulWitness, protocol::{self, PreparedRelation}};
use bitz::transcript::Blake3Transcript;

let witness = MulWitness::<u64>::from_fn(1 << 15, |i| (i as u64, u64::MAX))?;
let prepared = PreparedRelation::new(*witness.layout())?;
let hint = protocol::commit(&prepared, witness.bitz_bit_rows())?;
let proof = protocol::prove(
    &mut Blake3Transcript::new(), &prepared, &witness, &hint,
)?;
protocol::verify(
    &mut Blake3Transcript::new(), &prepared, &hint.commitment, &proof,
)?;
```

The native width selects arithmetic and statement policy. Existing
assignments are preserved: u32/u128 have an exact double-width product
block; u64 has separate low/high blocks and its existing eight-block MLE
domain. The u32 univariate skip and u64/u128 plain outer protocols remain
unchanged.

### Inside the prover

1. Validate the witness layout and bind the commitment and public relation.
2. Draw the runtime prime and project the prepared selector matrices.
3. Read the compact witness through `OuterRows`; `OuterArithmetic` handles
   each declared operand/product width. Products are joined on demand.
4. Run the existing generic inner sumcheck over borrowed native blocks and
   factored matrix weights. Implicit constant/zero blocks remain implicit;
   projection and the first fold stay fused.
5. Translate the terminal claim using `BlockTable` and bind the opening
   functional.
6. Open directly or through the prepared public packing map. `Proof`
   contains an `Opening::Direct` or `Opening::Virtual`; verification checks
   that the variant matches the relation before processing the proof.

The optimized internal `RawWitness` / `NativeWeights` representations remain.
Replacing them with eagerly projected dense vectors caused a measured
1.9–3.1× slowdown in the isolated inner-sumcheck experiment. Ordinary callers
only pass `&MulWitness<T>`.

CM-AND also implements `OuterRows` directly. Its production prover borrows
`&CmAndWitness`; it no longer clones an assignment into a projected wrapper
or allocates three signed row tables. Explicit dense reference projections
use the existing `EvaluatedSpartanAssignment` bundle, also shared by the
u32 and BabyBear native reference helpers.

### Packing widths

Use `MulWitness::<T>::from_fn_with_word_bits(n, W, input)` or
`MulLayout::<T>::new_with_word_bits(n, W)`. A custom layout can be passed to
`from_fn_with_layout` or `from_rows_with_layout`.

The logical width must satisfy `W >= 1` and `t + W <= 126`, where `t` is
computed for the opening layout, including padded cells. Constructors also
check host-index overflow. A zero-bit integer cell has no representation.

For non-power-of-two widths, the physical stride is
`P = W.next_power_of_two()`. Each native limb uses
`next_power_of_two(ceil(T::BITS / W))` cells; unused cells and high bits are
public zeros. The bound used for exponent-fold weight chunks is
`c_w = 127 - t - W`.

Existing W=1 paths and u32 W=8 keep their commitment layouts and proof bytes.
Other widths commit the compact W=1 source, then open the padded derived
grid through `MulLayout<T>`'s public virtual map. The map, logical width and
physical geometry are bound into the statement; the smaller value bound is
established by the map's support, independently of witness values.

`committed_layout()` describes `bitz_bit_rows()`. `bitz_params()` describes the
opening grid. They differ for the virtual packing path. New widths can cost
more than the existing direct modes because of the virtual opening.

### Qualification

The deterministic `qualification_probe` example measures witness generation,
packing, commitment, proving and verification separately, verifies every
proof, and prints a digest and proof size. Build timing and allocation
versions separately; allocator instrumentation changes latency.

```sh
cargo build --release --locked --example qualification_probe
RAYON_NUM_THREADS=8 target/release/examples/qualification_probe u64 18 6 1
cargo build --release --locked --example qualification_probe --features bench-peak-memory
RAYON_NUM_THREADS=8 target/release/examples/qualification_probe u64 18 0 1
```

Arguments are native type, log batch size, measured repetitions, and W.
Repetition zero is warmup. `BITZ_QUALIFICATION_N` overrides the batch size;
`BITZ_QUALIFICATION_WITNESS_ONLY=1` isolates construction.
With packing-only mode, `BITZ_QUALIFICATION_REUSE_ROWS=1` measures repeated
writes into existing buffers through `write_bitz_bit_rows`. This avoids glibc
heap-trimming costs that can increase when the witness allocation shrinks;
the allocating API can pay those costs in allocate-and-discard loops.
`BITZ_QUALIFICATION_PREFIX_ONLY=1` isolates the Spartan reduction and verifies
its terminal claim, excluding commitment and opening. `BITZ_QUALIFICATION_PACK_ONLY=1` measures
witness construction followed by packing, without proving. This allows checks
just above a power of two, where demand-zero padding matters.

`mul_compare` shares one typed witness-to-proof routine across all three
native types. `BITZ_MUL_WORD_BITS` selects W for that benchmark and `u32_mul`.
The paper-table CLI retains its existing W=1/8 selection.
