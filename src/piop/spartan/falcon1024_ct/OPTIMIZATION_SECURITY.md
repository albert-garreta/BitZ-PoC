# Falcon security accounting

The native and shared-prime profiles combine arithmetic relations, binary
Keccak, and one shared PCS opening. Both use the joint HashToPoint forest.

## Security model

A challenge with algebraic error numerator `n`, field size `p`, and grinding
difficulty `g` contributes `n / (p * 2^g)` under the repository's computational
proof-of-work/random-oracle convention. This does not improve an interactive
sumcheck's raw statistical soundness and is not an independent proof of
Fiat–Shamir security. BLAKE3's 128-bit collision bound is stated separately.

The native prime satisfies `p >= 2^125`. The shared profile has `p >= 2^114`
at target 100 and `p >= 2^125` at target 128. `PreparedFalconHybrid::security()`
sums terms using the selected floor and rejects a configuration below its
requested target. Target 100 needs no arithmetic grinding; the separate ring
projection retains two grinding bits. Target 128 uses the budgets below.

## Prime reduction groups

Let `B` be the live batch and `d = log2(next_power_of_two(B))`. At target 128,
each of seven prime groups receives at most `2^-133` work-normalized error:

| Group | Error numerator before grinding |
| --- | ---: |
| Norm instance batching | `d` |
| Norm sumchecks | `4*(10+d)` |
| HashToPoint initial row point | `11+d` |
| Rejection, leaf, and joint forest reductions | Defined below |
| Ordered compaction fingerprints | `2048` |
| Linear constraints and terminal batching | `13+d+B+12` |
| Prime source sumcheck | `2*(17+d)` |

For a single numerator `n`, use `g(n)=8+ceil(log2(n))`, with `g(0)=0`.
The fingerprint argument fixes one incorrect signature before its challenge;
it needs no batch-size union factor. The linear group retains its previous
conservative bound even though the joint forest has one output claim.
Candidate-leaf authentication now inherits the forest's signature point and
has no additional instance-batching error term.

## Joint compaction allocation

The rejection and candidate-leaf cubic sumchecks have
`R_cubic=2*(11+d)` rounds. The forest has `R_forest=11*(d+1)+55` weighted
quadratic rounds. Its initial signature batching and eleven scalar line
reductions have total numerator `H=d+11`, across `M=11+(d>0)` nonce blocks.
See [COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for the signed root
identity, challenge ordering, and source authentication.

Choose cubic difficulty `r`, forest-round difficulty `q`, and root/line
difficulty `c` subject to

```text
3*R_cubic/2^r + 2*R_forest/2^q + H/2^c <= 1/256.
```

Multiplying by `1/p <= 2^-125` gives the required group bound. A deterministic
integer search minimizes `R_cubic*2^r + R_forest*2^q + M*2^c` using a common
power-of-two denominator. The uniform schedule is feasible by construction;
the bounded search includes every potentially cheaper schedule. No floating
point or saved historical schedule determines difficulties. Target 100 uses
`(r,q,c)=(0,0,0)`.

At batch 1024 the group has 42 cubic rounds, 176 quadratic rounds, and twelve
root/line nonce blocks. The allocation is `(17,17,17)`, with an expected
30,146,560 nonce trials. The preceding forest's corresponding group used
15,466,496 expected trials. These are analytic counts for this group only,
not measured prover times; other groups and field arithmetic also change.

## Remaining terms and composition

The complete report also includes:

- Prime sampling error `2^-144`.
- Native ideal batching/projection error at most
  `(d+2046)/(12289^11-12289)`, and native coordinate carry batching `10/p`
  with twelve grinding bits at target 128.
- For the shared profile, ring error `(4*d+2049)/12289^11` and degree-20
  integer-polynomial projection. Projection difficulties are 2 and 14 at
  targets 100 and 128 respectively.
- The integer-to-binary bridge, both binary Keccak prefixes, SHAKE wiring,
  binary claim batching, joint sumcheck, ring switching, and support padding.
  These binary components reserve an eight-bit margin over the target.
- Shared Ligerito. If its plan has `k` challenge blocks, each targets
  `target+2+ceil(log2(k))`, so their sum is at most `2^-(target+2)`.

The seven prime groups together spend at most `7/32` of the target error
budget at target 128, leaving room for separately counted terms. At target
100, exact rational composition uses the actual selected prime floor rather
than applying this target-128 allocation. Source roots and public inputs
precede the reductions they authenticate.

## Enforcement and validation

Prover and verifier derive difficulties from the validated layout; proofs
cannot supply a weaker schedule. The statement binds every schedule field,
including the separate forest-round difficulty. Nonce seeds bind domain,
round, and difficulty; every nonce must be consumed.

The current outer domains are native v5 and shared-prime V4. Earlier proofs
must be regenerated. Subprotocol labels version the weighted forest, signed
root relation, inherited leaf point, and final source binding.

Exact-integer group and complete-ledger tests cover every batch from 1 through
1024 and both targets. Reference and tampering tests cover padding, weighted
round reconstruction, individual signature root errors, altered terminals,
and source authentication. Tests are implementation checks, not substitutes
for the soundness argument. No new end-to-end benchmark result is implied by
this accounting; previous V2/V3 measurements describe their respective
historical protocols.
