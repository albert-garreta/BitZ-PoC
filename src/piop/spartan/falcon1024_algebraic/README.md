# Falcon-1024 algebraic proofs

This separate, non-hiding API proves knowledge of short signature coefficients
for public key/target pairs. It uses the existing BitZ commitment and Ligerito
opening. Enable the `falcon-hybrid` feature and import
`bitz::piop::spartan::falcon1024_algebraic`.

For each public pair `(h,t)`, the witness `(s1,s2)` must satisfy:

```text
s1 + h*s2 = t                   in F_12289[X]/(X^1024 + 1)
sum_j(s1[j]^2 + s2[j]^2) <= 70_265_242     over the integers
```

Polynomial coefficients are listed in ascending degree. Public coefficients
must be canonical residues in `0..12289`. Witness coefficients are signed
integers; every coefficient allowed by the norm bound fits the signed15 source
encoding. `s1` need not be centered, and `s2` is not restricted to the CT wire
encoding's range. The public statement contains no message, nonce, or signature
bytes. The caller must establish that `t` is the desired HashToPoint output.

```rust,ignore
use bitz::piop::spartan::falcon1024_algebraic::{
    FalconAlgebraicStatement, FalconAlgebraicWitness, PreparedFalconAlgebraic,
};

// Each vector entry is an array of 1024 coefficients.
let public = FalconAlgebraicStatement { public_keys, targets };
let witness = FalconAlgebraicWitness { s1, s2 };
let prepared = PreparedFalconAlgebraic::new(public.public_keys.len(), 128)?;
let committed = prepared.commit(public.clone(), witness)?;
let proof = prepared.prove(committed)?;
prepared.verify(&public, &proof)?;
```

`FalconAlgebraicWitness::from_s2(&public, s2)` derives centered `s1` and checks
the norm without hashing. `check_algebraic_statement` checks the relation
natively. Prepared configurations accept batches `1..=1024` and security targets
100 or 128. The existing full-verification Falcon profiles remain available.

## Reduction and shared witness

The source contains 30,747 live bits per signature: two signed15 coefficient
vectors and a 27-bit nonnegative norm slack. Each coefficient occupies a 16-bit
address block: lanes 0..14 contain its signed15 encoding, and lane 15 of the
first 27 coefficients contains the corresponding norm-slack bit. All other
lane-15 positions are padding. Each signature occupies 32,768 bits, leaving
2,021 padding bits. The batch capacity is the next power of two, with a minimum of sixteen slots to
meet the PCS minimum size. An authenticated linear zero-sum
claim enforces all internal and inactive-slot padding. The generic shared opener
also enforces its structural zero lanes.

After binding the public inputs and source root, the protocol samples a prime
`2^125 <= p < 2^126`. Ring equations are batched over
`E = F_12289[theta]/(theta^11 + theta + 14)`. A degree-at-most-1022 quotient
certificate is fixed before its random non-base-field evaluation point.
Because `h,t` are public, the evaluated equation is linear in both signature
components. Eleven bounded integer coordinate carries reduce this claim to
`F_p`; extension products are completed before lifting canonical coordinates.

The norm proof uses independent instance weights and two quadratic sumchecks.
Every raw signed15/slack27 norm residual has absolute value below `2^40`, so
the prime-field equations represent exact integer equations. Ring conversion
residuals are below `2^50` for every supported batch. These bounds apply to each
equation before random batching.

A fresh seven-claim merge combines the ring claim, four norm endpoints, slack,
and padding. Coefficient index and bit-lane variables are separate: the binder
evaluates the signed decoder once, aggregates its prefix by witness byte, and
folds the common decoder before replaying coefficient weights. A streamed sumcheck binds them to one multilinear evaluation of
the original bits. The existing two-limb BitZ bridge, binary product GKR, binary
source sumcheck, ring switch, and Ligerito opening authenticate that evaluation.
SHAKE and HashToPoint proof components are absent; BitZ's own product GKR remains.

The verifier reads the public polynomials and does linear work in their size.
There is no Orthus-style public-input preprocessing in this interface.

## Security accounting

The aligned layout is bound into the v4 transcript domain; commitments and
proofs from preceding layouts or grinding schedules cannot be reused. The five
prime-field groups each receive at most `2^(-target-4)`. Their degree
numerators are:

| Group | Numerator | Domain lower bound |
|---|---:|---:|
| Ring coordinate projection | 10 | `2^125` |
| Norm instance batching | d | `2^125` |
| Two norm sumchecks | 4(10+d) | `2^125` |
| Seven-claim merge | 6 | `2^125` |
| Bit-source binder | 2(15+d) | `2^125` |
| BitZ product GKR | 367+40d | `2^128` |
| Binary source sumcheck | 2(16+d) | `2^128` |
| Ring switch | 256 | `2^128` |

Here `d = log2(capacity)`. The native ring identity has error at most
`(2046+d)/2^149`, and prime sampling requests a 144-bit composite-acceptance
budget.

The three binary groups and every Ligerito challenge block share the remaining
budget `(15/16)*2^-target - fixed`. It is split work-optimally by
`crate::hybrid::grinding_allocation`, so groups with larger raw errors receive
proportionally more of it. The composition stays at least
`target + log2(16/15)` bits. Each adaptive challenge boundary uses its group's
allocated difficulty. `security()` exposes the composed ledger under the
repository's computational grinding model, with BLAKE3 collision security
stated separately. No zero-knowledge guarantee is made.

## Validation and benchmarking

Unit tests cover exact norm boundaries, non-centered and signed coefficients,
independent schoolbook negacyclic multiplication, ring carries/certificates,
norm budget borrowing, inconsistent source bits, padding, malformed proofs,
public input changes, configuration replay and original Falcon interoperability.
Large end-to-end cases for batches 32 and 1024 are explicitly ignored by default.

Validation and benchmark commands:

```sh
cargo test --features falcon-hybrid --lib falcon1024_algebraic
cargo test --release --features falcon-hybrid --lib large_batches_at_both_security_targets -- --ignored
cargo test --features falcon-hybrid --lib hybrid::integer_bridge
cargo test --features falcon-hybrid --lib hybrid::joint_sumcheck
RAYON_NUM_THREADS=4 cargo run --release --features falcon-hybrid --example falcon_algebraic -- --batch 32 --security 128 --threads 4 --warmup 1 --iterations 5
```

The example generates distinct original-Falcon cases using the existing fn-dsa
benchmark helper. External generation, decoding, and hashing are reported
separately. Total prover time includes centered-witness reconstruction,
slack/quotient preparation, commitment and proving. `--trace` exposes component
span timings; omit tracing for timing comparisons. Set `RAYON_NUM_THREADS` and
`--threads` to the same count so both worker pools agree. Warm-up trials are
marked separately in the output. Verification uses the configured global pool.
JSON output reports verification time, stored proof payload,
source size and Linux process peak RSS. Payload accounting excludes public
inputs and outer transport framing; this API does not define a proof wire codec.
