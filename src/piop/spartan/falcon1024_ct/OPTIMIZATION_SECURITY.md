# Falcon security accounting

The sole Falcon proving backend combines native-ring arithmetic, binary Keccak,
and one shared PCS opening. This note describes its current budget directly.

## Security model

A challenge with algebraic error numerator `n`, field size `p`, and grinding
difficulty `g` contributes `n / (p * 2^g)` under the repository's computational
proof-of-work/random-oracle convention. The arithmetic prime satisfies
`p >= 2^125`. This convention does not improve an interactive sumcheck's raw
statistical soundness and is not an independent proof of Fiat–Shamir security.
BLAKE3's 128-bit collision bound is stated separately.

`PreparedFalconHybrid::security()` sums all configured terms. Preparation
rejects a configuration below the requested target. Target 100 needs no
arithmetic grinding; target 128 uses the budgets below.

## Prime reduction groups

Let `B` be the live batch and `d = log2(next_power_of_two(B))`. Each of seven
prime reduction groups receives at most `2^-(target+5)` work-normalized error:

| Group | Error numerator before grinding |
| --- | ---: |
| Norm and leaf instance batching together | `2*d` |
| Norm sumchecks | `4*(10+d)` |
| HashToPoint initial row point | `11+d` |
| Rejection, leaf, and forest reductions together | Defined below |
| Ordered compaction fingerprints | `2048` |
| Linear constraints and terminal batching | `13+d+B+12` |
| Prime source sumcheck | `2*(17+d)` |

For a single numerator `n`, use
`g(n) = max(0, target+5+ceil(log2(n))-125)`, with `g(0)=0`.
The fingerprint argument fixes one incorrect signature before the challenge;
it needs no batch-size union factor or partial-batch rounding adjustment.

## Current compaction allocation

The two cubic sample/leaf sumchecks and product forest have
`R = 2*(11+d)+55` rounds, with total numerator `A=3*R`.
Ten forest equality-weight draws and eleven line draws have total numerator
`H=10*(d+1)+11`. See [COMPACTION_SOUNDNESS.md](COMPACTION_SOUNDNESS.md) for the
nonzero-vector argument and challenge order.

At target 128, choose cubic difficulty `r` and forest-claim difficulty `c`
subject to the explicit group budget

```
A/2^r + H/2^c <= 1/256.
```

Multiplying by `1/p <= 2^-125` gives the required `2^-133` group bound.
One deterministic integer search minimizes `R*2^r + 21*2^c` within its bounded
candidate range, using a common power-of-two denominator. The initial uniform
pair is feasible by construction. No floating-point decision or saved protocol
schedule determines these difficulties. Target 100 uses `(0,0)`.

## Remaining terms and composition

The complete report also includes:

- Prime sampling error `2^-144`.
- Native ideal batching/projection error at most
  `(d+2046)/(12289^11-12289)`.
- Native coordinate carry batching, `10/p`, with 12 grinding bits at target 128.
- The wfbitz integer-to-binary bridge, both binary Keccak prefixes, SHAKE wiring,
  binary claim batching, the joint sumcheck, ring switching, and support padding.
  These binary components reserve an eight-bit margin over the requested target.
- Shared Ligerito. If its plan has `k` challenge blocks, each targets
  `target+2+ceil(log2(k))`, so their sum is at most `2^-(target+2)`.

The seven prime groups together spend at most `7/32` of the target error
budget, leaving room for these separately counted terms. Native carries and
certificates precede their challenges, and all three source roots and public
inputs precede the reductions they authenticate.

## Enforcement and validation

Prover and verifier derive difficulties from the same validated layout; proofs
cannot supply a weaker schedule. The header binds every schedule field, and
nonce seeds bind domain, round, and difficulty. Every nonce is consumed.
The statement and arithmetic PIOP domains are updated for this allocation;
old proofs must be regenerated. There is no historical backend selector.

Direct budget tests cover every live batch from 1 through 1024. The complete
ledger covers both supported targets, and weaker-nonce rejection tests remain.
Reference checks cover padding, equality batching, candidate leaves, optimized
arithmetic kernels, and binary/PCS transcripts. These are implementation checks,
not substitutes for the underlying security argument.

This cleanup was validated by static review, compilation, and linting only;
its runtime tests and benchmarks were not executed.
