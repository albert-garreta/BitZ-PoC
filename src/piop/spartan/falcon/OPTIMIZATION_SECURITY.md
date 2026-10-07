# Falcon security accounting

SharedPrime combines ring and integer reductions, binary Keccak, and one joint
source PCS opening. The supported Falcon degrees are 512 and 1024, with ring
extension degrees 9, 10, and 11 selected by the validated profile.

## Model and fields

Under this repository's computational proof-of-work/random-oracle convention,
a challenge block with algebraic numerator `n`, field size `p`, and difficulty
`g` contributes `n/(p*2^g)` per unit of adversarial work. This does not improve
an interactive sumcheck's raw statistical soundness and is not an independent
Fiat–Shamir security theorem. BLAKE3 collision security is separate.

The arithmetic prime satisfies `p >= 2^114` at 100 bits and `p >= 2^125` at
128 bits. `PreparedFalconHybrid::security()` sums the complete ledger and
rejects a configuration below its target. The ring field has exactly `12289^k`
elements; its sampler includes zero and base-field elements.

## Arithmetic obligations

Let `B` be the live batch, `d=log2(next_power_of_two(B))`, `L=COMPACTION_LOG`,
`m=log2(linear_stride)`, and `a=log2(signature_stride)`.

| Reduction | Error numerator |
| --- | ---: |
| Norm instance batching | `d` |
| Norm sumchecks | `4*(log2(N)+d)` |
| HashToPoint initial row point | `L+d` |
| Rejection and candidate-leaf sumchecks | `6*(L+d)` |
| Joint compaction forest rounds | `2*(L*(d+1)+L*(L-1)/2)` |
| Forest root and line reductions | `d+L` |
| Ordered compaction fingerprint | `2^L` |
| Linear constraints and endpoint batching | `m+d+B+12` |
| Arithmetic-source sumcheck | `2*(a+d)` |
| Integer-polynomial projection | `2k-2` |

The fingerprint argument fixes one incorrect signature before its challenge,
so it requires no extra batch union factor. The leaf proof inherits the
forest's signature point. See [COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md).

At 128 bits, the norm, initial-point, fingerprint, binder, and source-sumcheck
categories each use `g(n)=8+ceil(log2(n))`, with `g(0)=0`. The combined rejection,
leaf, and forest category allocates cubic, weighted-quadratic, and root/line
difficulties using exact integer optimization, satisfying

```
3*R_cubic/2^r + 2*R_forest/2^q + (d+L)/2^c <= 1/256,
R_cubic = 2*(L+d),
R_forest = L*(d+1)+L*(L-1)/2.
```

At 100 bits these arithmetic categories need no grinding. The independent
integer-polynomial projection uses 2 grinding bits at 100 bits and 14 at
128 bits. Prime sampling contributes at most `2^-144`.

The ring error is `(4d+2N+1)/12289^k`. Profile selection uses the maximum
batch's `d` and an exact 192-bit comparison against `2^-(target+2)`; the
prepared ledger uses the actual batch and checks the complete composition.

## Binary obligations and PCS

The ledger includes the bounded integer-to-binary bridge, every Keccak slab,
public SHAKE wiring and claim batching, the joint binary sumcheck, ring
switching, and support padding. Their allocations include an eight-bit margin
above the requested target. The activity-mask constant-column identity is
multilinear within the existing Keccak outer dimension; it introduces no
additional degree or unbound endpoint.

The initial oracle is one vector-valued RS codeword with complete canonical
rows. Apply the unique-decoding continuation to that joint oracle; separate
proximity guarantees for logical sources do not establish joint proximity.
For `h` PCS challenge blocks, each targets
`target+2+ceil(log2(h))`, so their sum is at most `2^-(target+2)`.
Canonical zero-lane expansion and full final-message authentication change
stored encodings, not these obligations.

## Enforcement and validation

The protocol identifier is `bitz/falcon/shared-prime/non-zk/v1`. Before
challenges, the statement binds the source root, public data, degree, extension
polynomial, maximum batch, security target, layout, activity policy, and every
derived schedule field. The actual prime is sampled after the integer lift is
bound. Nonces bind their domain, round, and difficulty; verifiers must consume
all messages and nonces.

Tests compare ring selection with independent big integers, check complete
arithmetic budgets, and exercise retained extensions, both degrees and targets,
padded batches, non-Boolean projection/decoder identities, weighted norms,
malformed proofs, and source authentication. Runtime qualification cannot
weaken these parameters. Measurements from historical revisions are not a
qualification of this cleanup.
