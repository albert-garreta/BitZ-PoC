# Compaction soundness in the native Falcon prover

This note justifies the current fingerprint and forest bounds. It uses the same
commitment binding, Fiat–Shamir, and work-normalized grinding assumptions as
[the security ledger](OPTIMIZATION_SECURITY.md). It does not upgrade those
assumptions to unconditional statistical security.

## Fix the source before the fingerprint

Condition on the source commitments authenticating one fixed bit source, and
on the separately accounted division, rejection, prefix, and source-binding
checks being sound. All three source roots and the public statement precede
the shared fingerprint challenges `gamma,rho`.

The 16-bit sample, 3-bit quotient, bounded remainder, and integer division
relation imply `0 <= q <= 5`; `d=q_2*q_0` is exactly rejection. The prefix
recurrence with `P_0=0` is the accepted count. Its 11-bit encoding, the sample
count 1311, and `P_M[10]=1` imply at least 1024 accepted samples. Therefore
`(1-d_i)*(1-P_i[10])` selects exactly the first 1024 accepted samples, whose
prefix ranks are exactly `0..1023`. All these integer values embed injectively
in the arithmetic field, whose prime is at least `2^125`.

For signature `s`, define the two product polynomials:

```
I_s(gamma,rho) = product(selected i, gamma + rho*P_i + r_i)
O_s(gamma,rho) = product(j=0..1023, gamma + rho*j + c_j)
F_s = I_s - O_s
```

The monic linear factors in `gamma` have distinct rank tags. Unique
factorization in `F_p[gamma,rho]` implies that `F_s` is nonzero whenever its
ordered output differs from the selected input. Candidate padding factors
are one. The conservative total-degree bound remains 2048 (1024 suffices).

If any signature has an incorrect ordered-compaction output, choose the first
such signature from the fixed source **before** drawing `gamma,rho`. Acceptance requires every
individual input/output root equality, so it requires this fixed nonzero
polynomial to vanish. Thus fingerprint failure is at most `2048/p`, with no
batch-size union factor. Adaptive false tree claims instead fall under the
forest and final authentication failure events below; this argument does not
assume the prover honestly constructs the product trees.

## Preserve a nonzero forest error vector

There are `T=2*B` trees. At each layer, compare the claimed tree evaluations to
the true product-tree MLEs determined by the source and fingerprint. Keep the
invariant that the vector of differences is nonzero.

For each of ten layer batching draws, all current claims are already fixed.
Pad the error vector with zeros to `next_power_of_two(T)`, sample a fresh
point `tau`, and combine claims with `eq(tau,t)`. A nonzero error vector has a
nonzero multilinear extension of total degree at most
`d_T=ceil(log2(T))`. Losing the error at this step costs at most `d_T/p`.
The layer sumcheck retains degree three, costing `3/p` per round; the ten
layers contain 55 rounds in total.

At level zero, every root product is checked directly. At subsequent levels,
condition on batching and sumcheck not losing the error. At least one claimed
child pair differs from its true pair. Every pair is absorbed before the
fresh common line challenge `lambda`. The next error vector has components
`(1-lambda)*e_left + lambda*e_right`. Select one nonzero pair before drawing
`lambda`; its nonzero affine polynomial vanishes at at most one point.
The event that the *entire vector* vanishes is therefore bounded by `1/p`,
without a union over trees. There are eleven line draws.

Consequently the forest claim-reduction numerator is
`10*ceil(log2(2*B))+11 = 10*(d+1)+11`, where
`d=log2(next_power_of_two(B))`. The terminal vector is authenticated by the
existing candidate-leaf proof, output-leaf reconstruction, and shared binder.
Their batching errors remain separately present in the complete ledger.
No equality across different signatures replaces any individual root check.

## Current security budget

Let `R=2*(11+d)+55`, `A=3*R`, and `H=10*(d+1)+11`. At target 128,
the cubic difficulty `r` and forest-claim difficulty `c` satisfy

```
A/2^r + H/2^c <= 1/256.
```

Since `p>=2^125`, the combined contribution is at most `2^-133`.
The deterministic allocation uses exact integers and minimizes nonce work
within its bounded search range. It has no dependency on historical schedules.
Fingerprint difficulty is 19 for every batch: `2048/(2^125*2^19)=2^-133`.
Target 100 remains unground. The other groups retain separate budgets in
[the complete ledger](OPTIMIZATION_SECURITY.md).

The statement transcripts are versioned to v4 and bind the full schedule.
The forest and its nonce domains separately identify equality batching.

## Implementation validation

Direct rational-budget tests cover every live batch from 1 through 1024.
Complete-ledger tests include all terms at targets 100 and 128. Independent
reference tests cover padded equality batching, candidate leaves, arithmetic
binder messages, binary lane folds, and the shared-opening transcript.
Tampering tests retain individual root checks, altered terminals, incorrect
schedules, source padding, and norm/ring/hash links. During this cleanup these
tests were compiled only, not executed.
