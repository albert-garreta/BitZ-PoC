# Algebraic Falcon word binder

The public constructor
`PreparedFalconAlgebraic::new_with_binding(batch, security, FalconAlgebraicBinding::Words)` selects
this relation for either algebraic Falcon degree. `protocol_id()` reports the
selected protocol. The default constructor selects this relation for Falcon-512,
batch 1024, security 128, and keeps the bit binder for other configurations. Use
`FalconAlgebraicBinding::Bits` explicitly to verify an existing v5 proof. This
note describes the proof relation and security accounting, without making a
performance claim.

This relation requires `batch == capacity`, including the PCS minimum
capacity (32 signatures for Falcon-512, 16 for Falcon-1024). An explicit word
request for a partial capacity returns an error. The ordinary constructor
continues to support partial batches with the bit binder. The full Falcon
signature verifier keeps its existing binder: its lane-dependent reduction
modulo 12289 prevents this two-table factorization from applying directly.

## Relation

Write `c = (coefficient, side, instance)` in low-coordinate-first order. Every
source word contains committed bits `b(c,0), ..., b(c,15)`. Define, on **every**
word,

```text
A(c) = sum_{lane=0..13} 2^lane b(c,lane) - 2^14 b(c,14)
B(c) = b(c,15)
```

The existing ring and norm endpoint merge supplies the public coefficient
weight `Lambda(c)`. Let `eta` be the existing four-claim merge challenge,
`padding = eta^3`, and `s(instance)` be the already merged slack weight. With
`L = 26` for Falcon-512 and `L = 27` for Falcon-1024, the second public weight is

```text
H(coefficient, side, instance) =
    2^coefficient s(instance)    if side == 0 and coefficient < L
    padding                     otherwise.
```

The binder proves the same merged target through

```text
sum_c [Lambda(c) A(c) + H(c) B(c)] = target.
```

Public selectors are in `H`; the meaning of `B` never depends on the column.
The multilinear extensions give a polynomial with degree at most two in each
word coordinate. With `m = log2(N)` and `k = log2(capacity)`, sumcheck has
`m + 1 + k` rounds instead of the bit binder's `m + 5 + k`.

The honest prover stores `B` sparsely on the live slack positions, and stores
`D = H - padding` on the same support. It scans every source word first and
rejects any nonzero lane-15 padding bit. For omitted coefficient or side
coordinates, both `D` and `B` fold by `(1-r)`, while `padding` remains constant.
This is only a representation optimization: the verifier's opening always
includes lane 15 of **every** committed word.

## Transcript and commitment binding

After sumcheck, the prover supplies `a = A_tilde(r)` and `b = B_tilde(r)`. The
verifier checks its final claim equals `Lambda_tilde(r) a + H_tilde(r) b`.
Both terminal values are canonically validated and absorbed under a terminal
domain before a separate grinding nonce and challenge `zeta` are drawn. The
new challenge merges the pending commitment obligations into `a + zeta b`.

The row variables are the four bit lanes followed by nine low coefficient
coordinates. The bridge therefore receives the canonical public row weights

```text
eq(low_coefficient_point, row_coefficient)
    * (signed15_decoder[lane] + zeta * [lane == 15])
```

and the remaining word coordinates as its column point. In Falcon-512 these
remaining coordinates start with side; in Falcon-1024 they start with the
highest coefficient coordinate. The modulus, canonical row weights, column
point, and merged value are absorbed before the bridge. The verifier derives
the row weights itself and rejects noncanonical weights or malformed shapes;
weights are not proof-supplied data.

The generalized bridge preserves the previous equality-point APIs. Those
wrappers retain the old transcript and forest messages, so full Falcon and
other existing bridge callers retain their proof relation.

## Security accounting and versioning

The new protocol identifier ends in `word-binder/v6`. The configuration digest
binds the word mode, full-capacity restriction, and terminal merge difficulty.
Bit mode keeps its v5 identifier and digest. A prepared verifier chooses the
relation; proof data cannot select a different binder.

| Prime-field group | Bit binder numerator | Word binder numerator |
| --- | ---: | ---: |
| Ring coordinate projection | 10 | 10 |
| Norm instance batching | k | k |
| Integer outer sumcheck | 3(m+1+k) | 3(m+1+k) |
| Four-claim merge | 3 | 3 |
| Source binding sumcheck | 2(m+5+k) | 2(m+1+k) |
| Terminal commitment merge | absent | 1 |

Each group receives at most `2^(-target-4)` under the repository's
computational grinding model. Its difficulty is
`max(0, target + 4 + ceil(log2(numerator)) - 125)` at every adaptive boundary.
In particular, the new degree-one merge has difficulty 7 at target 128 and 0
at target 100. Both terminal values precede this challenge, so an inconsistent
pair contributes a degree-one failure event.

The bridge geometry and bounded canonical weights are unchanged, leaving its
error numerator unchanged. The binary groups and PCS blocks are reallocated
against the remaining budget after the additional fixed term. The composed
budget remains `(15/16) * 2^-target`; `security()` exposes every term. The
separate BLAKE3 collision-security assumption is unchanged.

Tests cover public weighted bridges in both limb modes, noncanonical and
altered weights, equality-wrapper transcript equivalence, word-mode ledger
and geometry, terminal and nonce mutations, and rejection across protocol
versions. Arithmetic tests separately exercise the sparse representation and
its dense reference relation.
