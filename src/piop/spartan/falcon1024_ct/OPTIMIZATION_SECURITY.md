# Falcon optimization security accounting

The v2 performance changes preserve the same Falcon relation, witness layout,
field sizes, native certificate/carry bounds, commitment configuration, and
challenge ordering. The grinding schedule changes; the remaining optimizations
are exact evaluations of the existing prover computations.

## What the security claim means

The repository assigns a challenge with algebraic error numerator `n` and
grinding difficulty `g` the work-normalized contribution `n / (p * 2^g)`.
The Falcon arithmetic prime satisfies `p >= 2^125`. The reported protocol
security sums these contributions with every other protocol error term.

This is the existing computational proof-of-work/random-oracle convention.
It is not a statement that grinding improves an interactive sumcheck's raw
statistical soundness, nor an independent proof of the Fiat–Shamir or grinding
model. The optimization establishes a non-increasing bound within that model.

## Count every changed challenge

Let `B` be the live batch and `d = log2(next_power_of_two(B))`.

| Challenge family | Draws | Error numerator |
| --- | ---: | ---: |
| Rejection sumcheck | `11+d` | `3*(11+d)` |
| Candidate-leaf sumcheck | `11+d` | `3*(11+d)` |
| Product-forest sumchecks | `sum(level=1..10, level)=55` | `165` |
| Product-forest powers batching | `10` | `10*(2*B-1)` |
| Product-forest line reductions | `11` | `11*(2*B)` |

Each signature retains its individual input/output root equality. The line
numerator conservatively includes a degree-one failure for each of the `2*B`
trees. Neither the reductions nor their batching polynomials change.

Define `R = 2*(11+d)+55`, `A = 3*R`, and `H = 10*(2*B-1)+22*B`.
The previous uniform difficulty was `u = 8+ceil(log2(A+H))`, giving

```
old_group_error <= (A+H) / (2^125 * 2^u).
```

The new schedule chooses cubic-round difficulty `r` and forest-claim difficulty
`c` only when

```
A / 2^r + H / 2^c <= (A+H) / 2^u.
```

`compaction_grinding_bits` performs this comparison with exact `u128` integers
at common denominator `2^(u+1)`, without floating-point decisions. The old pair
`(u,u)` is always feasible. Among pairs from 1 through `u+1`, it minimizes
expected nonce attempts `R*2^r + 21*2^c`, retaining the old pair on equal cost.
The search is bounded and deterministic from public layout parameters.

For `B=1024`, `R=97`, `A=291`, `H=42998`, and `u=24`. It selects `(r,c)=(18,25)`:

```
old numerator at denominator 2^25: 43289*2             = 86578
new numerator at denominator 2^25: 291*128 + 42998     = 80246
new/old group error = 80246/86578                     = 0.926864
new/old expected work = (97*2^18+21*2^25)/(118*2^24)  = 0.368776
```

Thus this group's bound improves by 7.31% and expected nonce work decreases
by 63.12%. These are analytical quantities; actual latency depends on the
transcript-derived nonce searches. Other schedules, including the 29-bit
fingerprint, 12-bit native carry boundary, binary protocols, and PCS, retain
their previous settings. Target 100 remains unground. The standalone nonhybrid
backend retains its previous difficulty schedule.

## Enforcement and compatibility

Prover and verifier derive the schedule from the same validated batch layout.
Cubic sumcheck boundaries use `cubic_round_bits`; only forest powers and line
boundaries use `forest_claim_bits`. Proofs cannot supply either difficulty.
The grinding seed already binds its domain, round index, and difficulty.

The native hybrid domain is versioned to
`bitz/falcon1024-ct/hybrid/native-ring/non-zk/v2`, and its statement transcript to
`bitz/falcon-hybrid/native-ring/statement/v2`. The header binds the additional
schedule field before challenges. Earlier proofs must be regenerated.

## Exact prover computation changes

- Sparse binary gathers sum signature-weighted bits once per referenced word;
  cached values produce the same seven round messages. Repeat-axis insertion,
  duplicate entries, and logical padding retain their original interpretation.
- The arithmetic binder caches all 256 ternary extensions of an 8-bit witness
  block, widening exact signed table entries into the existing field operations.
  Public target contributions are computed independently and reduced in the
  same field. These prover kernels are not advertised as constant time.
- The shared PCS combines padding basis updates. On the Boolean cube, the full
  equality basis plus each disjoint source restriction is exactly the equality
  basis supported on padding. The same challenges and verifier identity remain.

## Validation obligations

Tests compare the changed group's exact rational error and expected work with
the previous schedule for every `B=1..1024`; the complete protocol ledger also
checks both supported targets and every batch. Forest proofs generated with a
weaker cubic or claim difficulty must fail the expected verifier schedule.

Reference parity tests cover every cached witness byte, complete arithmetic
sumcheck transcripts, every packed binary round and repeat-axis placement,
duplicate/canceling entries, physical padding, and direct versus separate PCS
padding updates. Existing malformed-proof and commitment-link tampering tests
remain applicable. These tests check implementation invariants; they do not
substitute for the underlying protocol security argument.
