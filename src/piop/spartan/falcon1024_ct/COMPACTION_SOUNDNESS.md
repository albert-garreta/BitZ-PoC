# Compaction soundness in native hybrid v3

This note justifies the v3 fingerprint and forest bounds. It uses the same
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

## Preserve the saved v2 budget

Let `R=2*(11+d)+55`, `A=3*R`, and `H_old=10*(2*B-1)+22*B`.
Recover the saved v2 difficulty pair `(r_old,c_old)` using its original exact
integer search. The new search minimizes expected work `R*2^r+21*2^c`
subject to

```
A/2^r + (10*(d+1)+11)/2^c <= A/2^r_old + H_old/2^c_old.
```

The comparison is exact using `u128` at a common power-of-two denominator;
no floating-point value controls protocol parameters. At `B=1024`, the
pair changes from `(18,25)` to `(18,17)`. This group's expected nonce work
falls from 730,071,040 to 28,180,480 attempts, and its bound improves.

The fingerprint changes from difficulty `19+ceil(log2(B))` to 19 for power-of-two
batches and 20 otherwise. The extra bit for partial batches preserves v2's
rounding margin: `2048/2^g_new <= 2048*B/2^g_old` for every supported batch.
Target 100 remains unground. Other security contributions and difficulty
schedules remain unchanged.

The hybrid proof and statement transcripts are versioned to v3 and bind the
new batching descriptor and full schedule. The forest and forest batching
nonce domains identify equality batching. The standalone arithmetic prover
also uses this forest, retaining its older conservative difficulty schedule;
its nested forest domain rejects prior forest transcripts.

## Implementation validation

Exact regression tests compare the changed group and fingerprint bounds with
v2 for every batch `1..1024`. Complete-ledger tests include all unchanged
terms at targets 100 and 128. Reference tests cover padded equality batching,
parallel norm preparation, candidate leaves, arithmetic binder messages,
compact binary lane folds, and the complete shared-opening transcript.
Tampering tests retain individual root checks, altered terminals, incorrect
schedules, source padding, and the existing norm/ring/hash links.
These checks validate implementation invariants, not a replacement security
proof or an independent audit of the repository's grinding convention.
