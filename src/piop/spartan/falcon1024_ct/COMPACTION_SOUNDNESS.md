# Joint HashToPoint compaction forest

This note describes the joint forest used by native v5 and shared-prime V4.
It assumes the source-commitment binding, Fiat–Shamir, and work-normalized
model of [the security ledger](OPTIMIZATION_SECURITY.md). Grinding does not
turn the argument into unconditional statistical security.

## Fix the source before the fingerprint

Condition on the source commitments authenticating one fixed bit source and
on the separately accounted division, rejection, prefix, and source-binding
checks being sound. All three source roots and the public statement precede
the shared fingerprint challenges `gamma,rho`.

The 16-bit sample, 3-bit quotient, bounded remainder, and integer division
relation imply `0 <= q <= 5`; `d=q_2*q_0` is exactly rejection. The prefix
recurrence with `P_0=0` is the accepted count. Its 11-bit encoding, the sample
count 1311, and `P_M[10]=1` imply at least 1024 accepted samples. Therefore
`(1-d_i)*(1-P_i[10])` selects exactly the first 1024 accepted samples, whose
prefix ranks are exactly `0..1023`. These integers embed injectively in either
arithmetic prime family.

For signature `s`, define

```text
I_s(gamma,rho) = product(selected i, gamma + rho*P_i + r_i)
O_s(gamma,rho) = product(j=0..1023, gamma + rho*j + c_j)
F_s = I_s - O_s.
```

The monic linear factors in `gamma` have distinct rank tags. Unique
factorization in `F_p[gamma,rho]` implies that `F_s` is nonzero whenever its
ordered output differs from the selected input. Padding factors are one.
The conservative total-degree bound remains 2048 (1024 suffices).

If any signature is wrong, fix one such signature from the committed source
before sampling `gamma,rho`. Its nonzero fingerprint vanishes with probability
at most `2048/p`. No batch-size union factor is needed. Condition below on
the resulting vector of fingerprint differences remaining nonzero.

## Start with a signed root equality

Let `d=log2(next_power_of_two(B))`. Pad signatures to this capacity with
candidate and output trees whose leaves are all one, so every padded root
difference is zero. Sample a fresh signature point `tau` after the source and
fingerprint are fixed. The claimed starting identity is

```text
0 = sum_s eq(tau,s) * (I_s - O_s).
```

A nonzero root-difference vector has a nonzero multilinear extension of
degree at most `d`. Thus this batching step loses an incorrect signature
with probability at most `d/p`, rather than checking an unweighted global
product. At batch one, no signature challenge or root-batching nonce exists.

Represent candidate/output by one additional Boolean tree coordinate `b`.
The first multiplication layer uses weights `+1,-1` on this coordinate.
There is no transmitted root vector and no prover-selected common root value.
The root sumcheck reduces the signed equality directly to child claims.

## Weighted quadratic rounds

At layer `ell`, keep the tree coordinates and the `ell` position coordinates
inside the same sumcheck. There are `d+1+ell` rounds. Its factors are the
multilinear child tables `L` and `R`; equality weights are handled explicitly
by the verifier instead of being counted as a third polynomial factor.

For an ordinary coordinate with equality parameter `r`, let the quadratic
round polynomial `h` sum `L*R` over remaining Boolean coordinates with their
remaining weights. Verify

```text
C = (1-r)*h(0) + r*h(1),
```

then sample a fresh `u` and continue with `C=h(u)`. For the signed root-side
coordinate the check is `C=h(0)-h(1)`. This is a weighted sumcheck, not a
substitution of a product of averages for an average of products.

Each round needs two field elements. If `h(z)=a+b*z+c*z^2`, an ordinary
round sends `b,c`, recovering `a=C-r*(b+c)`. This uses no division, including
when `r` is zero or one. A signed round sends `a,c` and recovers `b=-C-c`.
Round messages precede their grinding nonce and challenge.

After all coordinates are reduced, send just `L(u),R(u)` and check their
product against the final scalar claim. Absorb both before sampling the
fresh line challenge `lambda`. Continue at the next layer with

```text
C_next = (1-lambda)*L(u) + lambda*R(u).
```

Condition on all preceding checks retaining a false claim. An incorrect
weighted round polynomial differs from the correct one by a nonzero
polynomial of degree at most two, giving error at most `2/p`. At the layer
endpoint, a false product implies a nonzero pair of child errors. Its affine
combination vanishes at at most one `lambda`, giving error at most `1/p`.
No new tree-batching challenge is needed at subsequent layers.

There are eleven layers, so

```text
R_forest = sum(ell=0..10, d+1+ell) = 11*(d+1)+55
forest error <= (d + 2*R_forest + 11)/p.
```

For batch 1024 this is `(10+352+11)/p = 373/p`, excluding the separately
accounted fingerprint and terminal authentication. Folding position
coordinates first lets the added tree-coordinate rounds operate on small
tables after each tree has reduced to one child pair.

## Authenticate the joint leaf claim

The forest finishes at a signature point, side coordinate, and 11 position
coordinates. The prover sends one candidate value `a` and one output value
`b`. Check that their side-coordinate affine combination equals the forest
claim. A false mixed claim necessarily leaves at least one false endpoint;
this deterministic split introduces no additional random challenge.

Authenticate `a` with the existing `11+d`-round cubic candidate-leaf
sumcheck, using the inherited signature point. Its three terminal tables
remain affine in committed bits: accepted flags, the prefix-high-bit
selector, and the fingerprint weighted by the inherited signature and
position equality tables. No fresh leaf instance-batching challenge remains.
Authenticate `b` as one weighted affine claim on committed output bits.
Both claims include the public contribution of all-one padded signature
trees. Their terminal claims enter the same source-bit binder and shared PCS
opening as the other arithmetic obligations.

A weighted Boolean table's multilinear extension generally differs from the
product of its factors' multilinear extensions. The binder reconstructs the
former from source coefficients; replacing it with the latter off the Boolean
cube would not authenticate the leaf proof.

## Grinding and payload accounting

At target 128 the prime is at least `2^125`. Put

```text
R_cubic = 2*(11+d)
H = d+11
M = 11 + (d>0).
```

Choose cubic, forest-round, and root/line difficulties `r,q,c` to minimize
`R_cubic*2^r + R_forest*2^q + M*2^c`, subject to

```text
3*R_cubic/2^r + 2*R_forest/2^q + H/2^c <= 1/256.
```

This group contributes at most `2^-133` under work-normalized accounting.
Target 100 retains zero arithmetic grinding. Fingerprint grinding remains
separately budgeted. Difficulties, dimensions, and versions are transcript-bound.

For batch 1024, compact forest field messages occupy
`176*2*16 + 11*2*16 = 5,984` bytes. The final candidate/output split adds
32 bytes. These counts exclude nonces, the candidate-leaf proof, other
arithmetic messages, and the final PCS opening; they are not an end-to-end
proof-size or performance measurement.
