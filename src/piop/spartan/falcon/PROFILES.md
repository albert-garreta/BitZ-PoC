# Compile-time Falcon profiles

Enable `falcon-hybrid` and choose a preset or declare a profile:

```rust
use bitz::falcon_profile;

falcon_profile! {
    pub MyFalcon512 {
        n: 512,
        security_bits: 128,
        max_batch: 1024,
        ring_extension: Auto,
    }
}
```

Use `FalconPublicStatement::<MyFalcon512>` and
`PreparedFalconHybrid::<MyFalcon512>::new(batch)` from
`bitz::piop::spartan::falcon_profiles`. All profiles use SharedPrime.
`Explicit(9)`, `Explicit(10)`, or `Explicit(11)` pins the extension; invalid
security combinations fail during constant evaluation. Preparation also
validates manual trait implementations, the live batch, and the complete
security composition.

## Independent parameters

`N` is 512 or 1024, the Falcon polynomial length in
`F_12289[X]/(X^N+1)`. `k` defines the ring challenge field
`F_12289[T]/(T^k+T+c_k)`. These do not change BitZ's binary field or
its arithmetic-prime policy. The registered irreducible polynomials are
`T^9+T+60`, `T^10+T+4`, and `T^11+T+14`. Ring challenges sample all extension
field elements, including base-field elements. Independent tests check
irreducibility and multiplication.

With `d=ceil(log2(max_batch))`, the ring soundness bound is

\[
\epsilon_{\rm ring}\le\frac{4d+2N+1}{12289^k}.
\]

`Auto` selects the smallest registered `k` satisfying
`epsilon_ring <= 2^-(security_bits+2)`, using exact 192-bit integer arithmetic.
For both supported degrees and `max_batch <= 1024`, Auto uses `k=9` at
100 bits and `k=11` at 128 bits. The maximum batch, rather than the actual live
batch, fixes a profile's field. Preparation checks the full error ledger,
including grinding and PCS obligations.

## Implementation

Six concrete `(N,k)` backends share one source implementation. There is no
protocol selector, experimental small-degree catalog, or compatibility
constructor. Degree-1024 fixture tests compile once; profile tests cover both
degrees and all retained extensions.

SHAKE slabs follow the descending power-of-two decomposition of the required
permutation count. Logical arithmetic and Keccak sources project into one joint
commitment. Canonical rows include structural zero lanes. Activity masks and
padding checks remain part of the proof.

Transcripts bind degree, defining polynomial, norm and encoding parameters,
maximum batch, target, source geometry, and the public statement/root. The
protocol identifier is `bitz/falcon/shared-prime/non-zk/v1`.

## Checks

```sh
cargo test --features falcon-hybrid --test falcon_profiles
cargo test --features falcon-hybrid --lib falcon
cargo test --features falcon-hybrid --doc falcon_profile
```

The qualification benchmark accepts `--degree 512|1024`, `--k auto|9|10|11`,
`--security 100|128`, `--batch`, `--seed`, `--threads`, `--warmup`, and
`--iterations`. It verifies every proof and reports witness/commitment/proving
and verification separately, alongside proof payload and grinding diagnostics.
