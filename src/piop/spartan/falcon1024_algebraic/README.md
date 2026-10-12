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
integers; every coefficient allowed by the norm bound fits a signed15 value.
Both committed encodings preserve each supplied value exactly. `s1`
need not be centered, and `s2` is not restricted to the CT wire
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

The two coefficient vectors share one encoding between the ring and integer
norm proofs. Above the minimum PCS allocation, a compact encoding stores each
coefficient's low twelve bits.
Coefficients in `-2048..=2047` reconstruct their three remaining signed15 bits
by sign extension. An exception records its coefficient index and the three
explicit high bits. Every coefficient requiring an exception has square at
least `2^22`, so the norm limits their number to `floor(B/2^22)` across both
vectors. No normalization or
truncation of a supplied witness is performed.

The same implementation also backs `falcon512_algebraic`, with the following
per-signature capacities for the compact layout:

| Component | Falcon-512 | Falcon-1024 |
|---|---:|---:|
| Twelve coefficient bits | 12,288 | 24,576 |
| Maximum exceptions | 8 | 16 |
| Explicit high bits at maximum exceptions | 24 | 48 |
| Nonnegative norm slack | 26 | 27 |
| Maximum live committed bits | **12,338** | **24,651** |
| Signed15 live bits at the PCS floor | 15,386 | 30,747 |

In the compact layout, exception indices are public proof metadata, stored as
`u16` indices with a `u16` count per signature. Their payload is
`2 + 2*exception_count` bytes per
signature, separate from the bit counts above. Counts and strictly increasing,
in-range indices are validated and absorbed before the first arithmetic
challenge. The API is non-hiding; the exception positions are not secret.

The arithmetic instance capacity is `C = batch.next_power_of_two()`. It is
independent of the minimum PCS allocation of 32/16 slots for Falcon-512/1024.
The original signed15 view has sixteen lanes per coefficient and `32*N` bits
per signature. Lane 15 of the first 26/27 coefficients holds norm slack; other
lane-15 entries are zero. Small batches use only `32*N*C` logical bits in the
norm, source binding, and integer bridge. The shared opening checks that the
remaining physical allocation is zero. At batch 1 the logical arithmetic
source is 16,384/32,768 bits instead of 524,288.

At the minimum PCS configuration, one source keeps the original signed15
coefficients and norm slack in sixteen-bit lanes and allocates `2^19` source
bits. It has no exception metadata or compact decoding step. Compression
cannot reduce this allocation, so this path avoids its extra work and
authenticates the logical source directly with one binary tensor. The opening
geometry records logical support separately from its physical allocation. Its live
counts remain 15,386/30,747 bits per signature for Falcon-512/1024.

Above that floor, physical storage separates the low bytes, next four bits,
and an exception/slack tail. The three sources reduce physical committed
bits from `32*N*capacity` to `26*N*capacity`, an **18.75% reduction** including
source padding. This split starts at batch 33 for Falcon-512 and batch 17 for
Falcon-1024. Its shared opening domain is half the old domain. The tail is
padded to the shared opener's row size. The compact decoder and shared opening
bind this storage to the signed15 arithmetic view and constrain unused bits.
`live_bits_per_signature()` reports worst-case live bits for the
selected layout; `LIVE_BITS` is the signed15 maximum and `COMPACT_LIVE_BITS`
is the compact maximum.
`source_bits_per_signature()` reports the aligned virtual stride, and
`committed_source_bits()` reports the total physical allocation.
`capacity()` retains its previous PCS-capacity meaning; `arithmetic_capacity()`
reports the instance domain. `logical_binding_bits()` and `bridge_layouts()`
report the source-bit domains authenticated by the bridges. Production's
word sumcheck has one entry per sixteen of these logical bits.

After binding public inputs, exception metadata, and the source root, the protocol samples a prime
`2^125 <= p < 2^126`. Ring equations are batched over
`E = F_12289[theta]/(theta^11 + theta + 14)`. The degree-at-most-`N-1`
negacyclic residual is evaluated directly at a random non-base-field point
`alpha`. For each public key, the weights on `s2[j]` are
`W_j = (h(X)*X^j mod (X^N+1))(alpha)`. They obey the linear-time recurrence
`W_0 = h(alpha)` and `W_(j+1) = alpha*W_j - (alpha^N+1)*h[N-1-j]`.
This eliminates the cached quotient polynomials and the batched quotient
certificate, removing 11,242/22,506 certificate bytes for Falcon-512/1024.
Eleven bounded integer coordinate carries remain and reduce the linear claim
to `F_p`; extension products are completed before lifting canonical coordinates.

The integer outer uses independent instance weights and one cubic sumcheck
of `eq(rho,instance)*S(instance,j)^2`, where S concatenates S1 and S2.
Its target is the weighted bound minus the weighted nonnegative slack.
Row coordinates precede instance coordinates. The first round accumulates
squares in native integers and uses the field library's mixed products and
prepared signed folds; subsequent rounds fold one field table, with
equality weights kept factored. There are no square-output witness columns.
Every raw signed15/slack27 norm residual has absolute value below `2^40`, so
the prime-field equations represent exact integer equations. Ring conversion
residuals are below `2^50` for every supported batch. These bounds apply to each
equation before random batching.

A fresh four-claim merge combines the ring claim, the single norm endpoint,
slack, and padding. The production binder sums out the four bit-lane variables
before its degree-two sumcheck. For a word `w = (coefficient, side, instance)`,
define the following uniform linear functions of its sixteen committed bits:

```text
D(w) = sum(l=0..13, 2^l*b(w,l)) - 2^14*b(w,14)
H(w) = b(w,15)
C(w) = sum(l=0..14, b(w,l)).
```

The public weights combine ring and norm coefficients for `D`, slack and
lane-15 padding masks for `H`, and the inactive-instance mask for `C`. All
side, coefficient-position and activity masks belong to these public weights;
the three word functions are defined uniformly even on padding. The sumcheck
therefore has `log2(N)+1+log2(C)` rounds: 21 for Falcon-1024 at batch 1024,
instead of the previous 25. When every logical instance is active, the weight
of `C` is identically zero and only `D` and `H` are needed.

The prover reuses the ring/norm weights `Lambda` and allocates its sole dense
`[Lambda,D]` pair after the first word fold, whose arithmetic uses native
signed coefficients. For the coefficient and side rounds, it aggregates `H`
across instances into at most 32 positions:
`U[j] = sum_i slack_weight[i]*H[i,j]` and `V[j] = sum_i H[i,j]`.
After those rounds, it projects the original sparse `H` entries to one value
per represented instance and folds those values through the instance rounds.

For a partial batch, popcount folding is deferred until the instance rounds
and uses a capacity-sized table. This avoids three dense pairs of word
tables. Source scanning rejects malformed padding; the verifier independently
enforces the padding relation and authenticates the uniform word functions.

All two or three terminal values are absorbed before a fresh, independently
ground prime-field challenge `zeta`. Their combination uses weights
`[1,zeta]` or `[1,zeta,zeta^2]`. One two-limb BitZ bridge authenticates the
combined value against the original committed bits, including when physical
storage is split. Its public row weights multiply the word-row equality
weights by the combined lane decoder; all word functions share the same
column point. The bridge verifier reconstructs these weights from the
sumcheck point, `zeta`, and prepared layout.

Earlier R7 paths already used a word-sized dense suffix after four lane
rounds. This change removes those rounds and their prefix work; it does not
halve that existing suffix domain. Historical R7 timings do not establish a
speedup for the word binder.

An experimental compact computation binds three logical planes directly,
without constructing the original signed15 bit source. It remains disabled by
default because the measured binding overhead causes proving-time regressions.
It can be selected only in a `bench-internals` build with
`BITZ_ARITHMETIC_COMPUTE_VARIANT=combined` (or `compact` to retain old logical
padding); `padding` selects the default behavior. This is separate from the
already enabled compact physical storage described above.
The `compute_variant()` metadata retains this historical selection label;
`padding` now runs word binding. Use `protocol_id()` to distinguish v9 results
from earlier implementations with the same label.

| Plane | Logical bits | Bridge row variables |
|---|---:|---:|
| Low eight bits | `16*N*C` | 12 |
| Next four bits | `8*N*C` | 11 |
| Exceptions and slack | `max(TAIL_STRIDE*C, 65536)` | 10 |

For an exception flag `e` fixed by public metadata, the exact integer decoder is

```text
s = sum(k=0..10, 2^k*b[k]) - 2048*b[11]
    + e*(4096*b[11] + 4096*u[0] + 8192*u[1] - 16384*u[2]).
```

Coefficient weights are shared between the ring and norm reductions and streamed
through this decoder. All three partial sums are absorbed before any branch
sumcheck challenge, and their sum must equal the merged target. Each branch
has its own domain label and BitZ bridge. After all binary bridge claims are
fixed, a fresh binary-field challenge combines their coefficients and targets
with weights `[1,zeta,zeta^2]`. The existing joint sumcheck and shared opening
authenticate all three planes. The generic shared opener retains its previous
transcript for callers using the original geometry constructor, including full
Falcon verification.
SHAKE and HashToPoint proof components are absent; BitZ's own product GKR remains.

`PreparedFalconAlgebraic::new(batch, security)` selects word binding for
all supported degrees, batch sizes and security targets, including partially
filled capacities. The relation choice does not depend on the platform or
worker count. `new_with_binding(..., FalconAlgebraicBinding::Words)` selects
the same relation explicitly; `FalconAlgebraicBinding::Bits` retains the bit
relation over the same compact commitment. Prover and verifier must select
the same relation.

`protocol_id()` identifies the effective relation: production word proofs use
`non-zk/v9`, while explicit bit proofs retain their v7/v8 domains. Bit proofs
with the original logical capacity retain v7; reduced logical support and
experimental compact computation use v8. The earlier full-capacity-only
`word-binder/v9` relation is superseded by the production word relation above.
The older v5/v6 relations used a different ring reduction and commitment encoding.

The explicit bit binder retains its streamed prefix and byte-bucket preparation,
with optimized construction/evaluation at one, eight and sixteen workers above
the small-batch threshold. Falcon-512 batches of 1024 also retain their qualified
two- and four-worker paths. Full Falcon signature profiles retain their existing
bit binder. The [earlier full-capacity word-binder note](../../../../docs/falcon-word-binder.md)
describes that historical relation; the uniform two/three-terminal relation and
its current grinding budget are specified here.

The verifier reads the public polynomials and does linear work in their size.
There is no Orthus-style public-input preprocessing in this interface.

## Security accounting

The selected decoder, exception indices, physical geometry, direct ring check,
logical support, combined integer outer, bridge matrices, and allocated grinding
schedule are bound into the protocol domain. Production word binding uses v9
at every batch size, additionally binding the word decoder, terminal count,
and terminal-merge grinding difficulty. The experimental compact computation
retains its v8 domain. `prepared.protocol_id()` reports the effective domain,
while `PROTOCOL_ID` names the production protocol.
The six production prime-field groups each receive at most `2^(-target-4)`;
the explicit bit relation and separate compact experiment retain five. Production keeps the binding
difficulty `g` for the former bit domain and funds the terminal merge from
the four removed degree-two rounds. For `k` word variables and terminal-merge
degree `e` in `{1,2}`, its difficulty `h` satisfies
`h >= max(0, g + log2(e) - 3)`, in addition to the per-group bound. Thus
`2k/2^g + e/2^h <= (2k+8)/2^g`: the combined binder and terminal merge consume
no more error budget than the former bit binder. Compact-computation
grinding is unchanged. The degree numerators of the prime and binary groups
are:

| Group | Numerator | Domain lower bound |
|---|---:|---:|
| Ring coordinate projection | 10 | `2^125` |
| Norm instance batching | d | `2^125` |
| Integer outer sumcheck | 3(11+d) | `2^125` |
| Four-claim merge | 3 | `2^125` |
| Production word binder | `2(11+d)` | `2^125` |
| Word terminal claim merge | 1 if `batch=C`, otherwise 2 | `2^125` |
| Explicit bit binder | `2(15+d)` | `2^125` |
| Experimental compact binder | `2*sum(branch bit logs)` | `2^125` |
| Each BitZ product GKR | `s + 3*(r*(r-1)/2 + r*s) + r` | `2^128` |
| Compact binary claim merge | 2 | `2^128` |
| Binary source sumcheck | 2m | `2^128` |
| Ring switch | 256 | `2^128` |

This table uses Falcon-1024 and `d = log2(arithmetic_capacity)`.
`m = log2(shared opening bit domain)`: it is 20 at the minimum allocation and
`15+d` in split mode. For a bridge with row log `r` and column log `c`,
`s=c+1` accounts for its two limbs. Single mode has one binding domain and
bridge; experimental compact computation has three. The prime-field terminal
merge is present only in production word binding; the binary bridge-claim
merge is present only in compact computation. All terminal values are fixed
before the prime merge challenge, giving a degree-one or degree-two error
polynomial even though the lane decoder depends on that challenge. The native ring identity now has
error at most `(1023+d)/2^149`, or generally `(N-1+d)/2^149`, because it checks
the reduced residual directly. Prime sampling requests a 144-bit
composite-acceptance budget. The allocation is recomputed for the actual
opening geometry, including its Ligerito challenge blocks.

The terminal evaluations are absorbed before the merge nonce and challenge;
its difficulty and the selected binder relation are bound into the v9
configuration digest. The BitZ product-GKR numerator is unchanged. At degree
512, subtract one from the parenthesized round counts for the norm, binder,
and binary source sumchecks.

The binary groups and every Ligerito challenge block share the remaining
budget `(15/16)*2^-target - fixed`. It is split work-optimally by
`crate::hybrid::grinding_allocation`, accounting for each group's raw error and
number of grinding sites. The composition stays at least
`target + log2(16/15)` bits. Each adaptive challenge boundary uses its group's
allocated difficulty. `security()` exposes the composed ledger under the
repository's computational grinding model, with BLAKE3 collision security
stated separately. No zero-knowledge guarantee is made.

## Validation and benchmarking

Unit tests cover exact norm boundaries, non-centered and signed coefficients,
independent schoolbook negacyclic multiplication and direct ring weights,
bounded coordinate carries, exhaustive signed15 decoder reconstruction,
maximum exception counts, malformed exception metadata, compact/virtual claim
equivalence, norm budget borrowing, inconsistent source bits, padding, malformed proofs,
public input changes, configuration replay and original Falcon interoperability.
Word-binding tests check the reduced round count, two/three terminal shape,
one-bridge integration, fresh-merge schedule binding and prover/verifier
transcript continuation.
Large end-to-end cases for batches 32, 512, 513 and 1024 are explicitly ignored
by default; these include the largest arithmetic-capacity boundary and both
terminal counts at both security targets.

Validation and benchmark commands:

```sh
cargo test --features falcon-hybrid --lib falcon1024_algebraic
cargo test --features falcon-hybrid --lib hybrid::integer_bridge
cargo test --features falcon-hybrid --lib hybrid::joint_sumcheck
RAYON_NUM_THREADS=4 cargo bench --profile release --features falcon-hybrid --bench falcon_algebraic -- --batch 32 --security 128 --threads 4 --warmup 1 --iterations 5
RAYON_NUM_THREADS=8 cargo run --release --features falcon-hybrid --example falcon_algebraic -- --batch 32 --security 128 --threads 8 --warmup 1 --iterations 5
```

The benchmark generates distinct original-Falcon cases using the existing fn-dsa
benchmark helper. External generation, decoding, and hashing are reported
separately. Total prover time includes centered-witness reconstruction,
slack and compact-source preparation, commitment and proving. `--trace` exposes component
span timings; omit tracing for timing comparisons. Set `RAYON_NUM_THREADS` and
`--threads` to the same count so both worker pools agree. Warm-up trials are
marked separately in the output. Verification uses the configured global pool.
JSON output reports verification time, stored proof payload, physical committed
source size, aligned virtual arithmetic size and Linux process peak RSS.
Physical allocation savings and live-bit savings are distinct, and neither is
a prediction of proving speed. Payload accounting excludes public
inputs and outer transport framing; this API does not define a proof wire codec.

The [October 2026 performance report](../../../../results/arithmetic-falcon-compact-compute-20261011/README.md)
preserves the native Linux/x86-64 comparisons at one and eight workers,
including the slower compact-computation experiments. Performance on other
hardware and worker counts has not been qualified by that study. The
[thread-scaling report](../../../../results/falcon-thread-scaling-20261011/REPORT.md)
qualifies the separate thread and word-binder changes. Neither report measures
the newly combined compact-commitment and word-binder implementation.
