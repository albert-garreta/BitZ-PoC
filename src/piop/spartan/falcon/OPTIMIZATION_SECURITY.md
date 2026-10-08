# Falcon security accounting

SharedPrime combines the native Falcon ring proof, integer norm and HashToPoint
reductions, binary Keccak, and one joint source PCS opening. Supported Falcon
degrees are 512 and 1024; validated profiles select ring extension degree
`k` from 9, 10, and 11. The protocol identifier is
`bitz/falcon/shared-prime/non-zk/v5`.

## Model and fields

Under this repository's computational proof-of-work/random-oracle convention,
a challenge block with algebraic numerator `n`, field size `p`, and difficulty
`g` contributes `n/(p*2^g)` per unit of adversarial work. This does not improve
an interactive sumcheck's raw statistical soundness and is not an independent
Fiat–Shamir security theorem. BLAKE3 collision security is separate.

The arithmetic prime lies in `[2^114, 2^115-2^102-1]` at target 100 and
`[2^125, 2^126-1]` at target 128. The smaller interval supports the unsplit
integer-to-binary bridge; target 128 uses two bounded limbs.
`PreparedFalconHybrid::security()` sums the complete ledger and rejects a
configuration below its target. The native ring field has exactly `12289^k`
elements; its sampler includes zero and base-field elements.

## Public selection-mask obligations

The proof prefix contains `ceil(D/8)` public bytes per signature, where
`D=717` for Falcon-512 and `D=1311` for Falcon-1024. A set bit identifies a
selected candidate. The verifier rejects incorrect lengths, nonzero unused
high bits, or a popcount other than `N`, then derives the increasing selected
indices `i[0]..i[N-1]` and the final selected position `c=i[N-1]`. The masks
are fixed in the transcript before the native ring proof and its challenges.
They add 90 or 164 bytes per signature, with no additional commitment.

The original source retains each authenticated SHAKE word `t`, three-bit
quotient `u`, bounded remainder `v`, rejection bit `e`, and the compact output
`C`. It enforces `t=12289*u+v` and `e=u_bit2*u_bit0`. Since `t` is 16-bit and
`v` always lies in `[0,12288]`, the division equation forces `u` into `0..5`;
thus `e=1` exactly for rejected candidates. Public selection bits `a[j]`
enter only as constants in these linear relations:

```
e[j] = 1-a[j]       for j <= c
C[k] = v[i[k]]      for k = 0..N-1.
```

These relations prove that the mask chooses the first `N` accepted draws in
order. After `c`, acceptance need not equal the zero selection bit. Those
mask-comparison row slots are zero. The verifier's exact-popcount check
replaces committed prefix/cutoff/count witnesses. No polynomial-tree,
prefix-count, cutoff, or selected-bit source columns remain.

The only nonlinear HashToPoint rows are the `D` rejection products. They are
padded to `next_power_of_two(D)` and proved by one equality-weighted
sumcheck of degree three. Its three unweighted terminal row MLEs `A,B,C`
are authenticated by the existing source binder. Inactive signatures and
candidate padding have zero `A`, `B`, and `C`. There is no H2P projection
challenge, fingerprint, grand product, or GKR stage. The native ring proof,
integer-to-binary BitZ GKR, and SHAKE/Keccak proof remain.

Remainders use the bounded decoder `low13+4097*top`; compact output `C` retains its
unsigned-14 decoder. Arbitrary decoded scalar residuals are bounded below
`SOURCE_RESIDUAL_BOUND`, which is below both prime families. Selection
residuals are at most one, and compact-output residuals at most 16383.
See [COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for the full relation.

| Per-signature geometry | Falcon-512 | Falcon-1024 |
| --- | ---: | ---: |
| Public selection-mask bytes | 90 | 164 |
| Total live arithmetic bits | 52,637 | 100,482 |
| Occupied source extent | 57,731 | 110,695 |
| Arithmetic signature stride | `2^16` | `2^17` |
| Linear rows / padded stride | 3,524 / 4,096 | 6,504 / 8,192 |
| Rejection rows / padded stride | 717 / 1,024 | 1,311 / 2,048 |

## Prime-field ledger and grinding

Let `B` be the live batch, `d=log2(next_power_of_two(B))`,
`m=log2(linear_stride)`, `a=log2(signature_stride)`, and
`r=ceil(log2(D))` (10 or 11). The first six categories below are exactly
`FalconSecurityNumerators::for_layout`.

| Reduction | Error numerator | Schedule field |
| --- | ---: | --- |
| Norm instance batching | `d` | `norm_instance_bits` |
| Norm sumchecks | `4*(log2(N)+d)` | `quadratic_round_bits` |
| HashToPoint initial row point | `r+d` | `outer_point_bits` |
| HashToPoint rejection R1CS sumcheck | `3*(r+d)` | `cubic_round_bits` |
| Linear constraints and endpoint batching | `m+d+B+8` | `linear_point_bits` |
| Arithmetic-source sumcheck | `2*(a+d)` | `binding_round_bits` |
| Native-ring integer-lift projection | `2k-2` | Separate ring schedule |

The eight binder endpoints are five norm claims and the three rejection row
claims. No H2P polynomial projection, fingerprint, candidate-leaf, or
compaction-forest error category remains. For each of the first six categories,
the implementation uses

```
g(0) = 0
g(n) = max(0, ceil(log2(n)) + target + 5 - prime_floor_bits)
prime_floor_bits = 114 at target 100, or 125 at target 128.
```

Each category therefore receives at most `2^-(target+5)` work-normalized
error. At target 128 this is `8+ceil(log2(n))`. At target 100 only large-batch
linear constraints and endpoint batching require grinding; the other five
categories need none over the supported batch range. Verifiers require a
nonce exactly when the derived difficulty is nonzero, including target-100
linear batching. No H2P projection nonce remains.

The independent native-ring integer-lift projection retains two grinding
bits at target 100 and 14 at target 128. Prime sampling contributes at most
`2^-144`. The ring error is `(4d+2N+1)/12289^k`. Profile selection uses the
maximum batch's `d` and an exact 192-bit comparison against
`2^-(target+2)`; the prepared ledger uses the actual batch and checks the
complete composition.

## Binary obligations and PCS

The ledger retains the bounded integer-to-binary BitZ forest/GKR, every
Keccak slab, public SHAKE wiring and source padding, binary claim batching,
the joint binary sumcheck, ring switching, and support padding. Their
difficulties come from the work-optimal allocation below. The
activity-mask constant-column identity is multilinear within the existing
Keccak outer dimension and introduces no additional unbound endpoint.

The source matrix has 8192 rows; the column count follows the compact source
stride.
With `r_b=13`, `c=a-13+d`, and `s=c` for the unsplit bridge or `s=c+1` for
two limbs, its error numerator is
`s+3*(r_b*(r_b-1)/2+r_b*s)+r_b`. Its difficulty comes from the allocation
below.
See [BRIDGE_GRINDING_AUDIT.md](BRIDGE_GRINDING_AUDIT.md) for the current
geometry, message counts, and limb-injectivity argument.

The initial oracle is one vector-valued RS codeword with complete canonical
rows. Apply the unique-decoding continuation to that joint oracle; separate
proximity guarantees for logical sources do not establish joint proximity.
Each PCS challenge block's difficulty comes from the allocation below.
Canonical zero-lane expansion and full final-message authentication change
stored encodings, not these obligations.

## Work-optimal grinding allocation

The prime-field schedule above is fixed. Every other ground group shares
the budget `(15/16)*2^-target` minus the fixed terms (prime sampling, the
ring error, and the prime-field ledger). These groups are the bridge GKR,
each Keccak slab, SHAKE wiring and padding, the joint binary sumcheck,
ring switching, and each Ligerito challenge block. A group with raw union
error `e` over `s` grinding sites contributes `e*2^-g` at difficulty `g`
and costs `s*2^g` expected hashes.
`crate::hybrid::grinding_allocation::allocate` minimizes the total work
under the budget by marginal analysis, and keeps every Ligerito block at
or above Flock's native fold work. The composed ledger therefore stays at
least `target+log2(16/15)` bits. `prepare` still rejects any configuration
below target. The exact-rational composition test checks every batch at
both targets.

The previous split gave every binary group `2^-(target+8)` and every PCS
block `2^-(target+2+ceil(log2(h)))`. It overpaid most where raw errors are
largest: level-0 UDR folds, about `2^-110` raw at Falcon-512 batch 1,024,
were ground to `2^-135` each. That split is not needed by any proof
obligation, since only the composed sum enters the target.

## Enforcement and validation

Before challenges, the statement binds the source root, public data, degree,
extension polynomial, maximum batch, security target, layout, activity
policy, and derived schedule fields. The native-ring certificate and its
integer lift precede their respective challenges; the actual prime is
sampled after the lift is bound. Public selection masks precede the native-ring
challenges and all arithmetic challenges. The rejection row point nonce
precedes its equality point, and every round message precedes its sumcheck
challenge. Nonces bind their domain, round, and difficulty;
verifiers consume all messages and nonces.

The implementation tests exact integer stage allocations, exact rational
composition, both prime families, native-ring profile selection, source
layouts, and padded batches. Rejection tests compare packed prover rows with
the committed-bit decoder and an independent dense proof, exercise all
quotient bit patterns, reject malformed proof messages, and compare
prover/verifier transcripts. Mask and source-binding tests reject malformed
or changed selection metadata, changed rejection bits, compact outputs,
and unauthenticated terminal claims. Runtime qualification cannot weaken
these parameters; measurements from earlier revisions do not qualify this
layout.
