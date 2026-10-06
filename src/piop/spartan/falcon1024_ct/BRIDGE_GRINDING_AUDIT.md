# Falcon hybrid bridge grinding audit

The native Falcon prover uses one partially reduced product tree from the
current `crate::bitz` PCS over both bounded integer limbs of its prime-field
row weights. SHAKE, HashToPoint,
Falcon arithmetic, every signature's norm bound, the three committed sources,
and the joint binary sumcheck and shared PCS opening remain enforced.

The statement uses `native-ring/non-zk/v5` and its transcript uses
`native-ring/statement/v5`; the bridge grinding domain remains v4.
The statement digest binds the derived bridge numerator and grinding difficulty.

## Scope and model

This calculation applies to the adapter in `hybrid_bridge::{prove, verify}`
and the arity-two BitZ forest/GKR it invokes. It uses the repository's
existing computational grinding model: a challenge block with raw error `e`
and difficulty `g` contributes `e * 2^-g` per unit of adversarial work. It does
not claim that grinding improves unconditional interactive soundness, nor add
security beyond the separately stated BLAKE3 bound.

Supported live batch sizes are 1 through 1024, padded to their next power of
two. The arithmetic source is a binary matrix with `d=row_vars=13` and
`c=col_vars=4+log2(capacity)`. Its layout has only row and column dimensions;
each cell is one bit. There are exactly two bounded limbs. The forest
uses an interleaved row grid of geometric width `t=d+1=14`, but reduces only
`d=13` product-tree levels. Its root-table width is therefore
`s=c+1=5+log2(capacity)`, ranging from 5 to 15. The unreduced coordinate is the
limb index; it is not multiplied away.

## Integer binding has no probabilistic loss

Prime row weights are lifted canonically and split by the bridge's local
`weight_limbs` helper. Its `limb_width` is `126-row_vars=113`; each limb is
folded with `bitz::fold::fold_columns` over the validated binary shape. The
production prime has 126 bits, so the second limb has at most 13 bits (the
bridge test also covers a 127-bit modulus). For every limb, an honest binary
column's integer sum is at most

```
2^13 * (2^113 - 1) < 2^126 < 2^128 - 1.
```

The verifier checks every supplied sum against the actual sum of the public
limb weights, rejects checked-add overflow and `u128::MAX`, and reconstructs
the prime claim from the supplied sums with exact modular arithmetic.
`smallest_generator()` tests all prime factors of `2^128-1`, whose product is
exactly that group order. Thus `sum -> generator^sum` is injective throughout
the accepted interval. A false integer sum cannot give the correct root.
There is no random-generator collision term or unaccounted limb collision.

Both limbs' sums are bound before the root challenge is sampled. The existing
prime-field read-off still combines them with radix `2^113`; using one forest
does not replace that check with a sum or product of the two limbs.

## Interleaved forest geometry

Write `A_l(r)=generator^weight_l[r]` for original source row `r` and limb
`l` in `{0,1}`. The source column `j` is reused for both limbs:

```
images'[2*r+l] = A_l(r)
packed_cols'[group][2*r+l] = packed_cols[group][r].
```

The derived leaf index is

```
j + 2^c * (l + 2*r),
```

so its coordinates are `[column | limb | original row]`, with each slice in
little-endian order. The BitZ product tree pairs halves, eliminating the
highest remaining row bit first. Reducing exactly 13 levels therefore
multiplies all 8192 original rows while retaining the low limb bit. The root
at index `j + 2^c*l` is

```
product_r A_l(r)^source_bit[j,r]
    = generator^sum_l[j].
```

This is precisely the order of the two supplied limb-sum vectors flattened
limb-major. Reducing all 14 geometric levels instead would multiply the two
limb roots together and would not establish the required claims.

The geometric row width remains 14 in the table-driven/JIT kernels, while
the proving loop visits only the 13 levels below the retained roots. At entry
the root point has `c+1` coordinates; each layer adds one row coordinate. The
final point has `c+14` coordinates, including the retained limb bit. A generic
partial-depth forest requires `depth <= geometric_row_vars` and
`root_point.len() == column_vars + geometric_row_vars - depth`; Falcon uses
only the fixed values above.

## Accepted forest and challenge blocks

The entry claim is the MLE of the `2^s` limb/column roots at a fresh point.
Each of the `d` layers sends two Gruen coefficients per sumcheck round, then
a pair of child evaluations. The next layer's claim follows by interpolation
of that pair. At layer `ell=0..d-1`, the incoming claim has `ell+s`
coordinates. The limb remains among these root coordinates at every layer.

1. **Root projection:** a wrong root table differs by a nonzero multilinear
   polynomial in `s` variables, of total degree at most `s`. Its `s` uniform
   GF(2^128) coordinates share one uninterrupted challenge block. Raw cost:
   `s/2^128`.
2. **Layer sumcheck:** each of the `ell+s` rounds proves an eq-weighted
   product, of degree at most three in the round variable. Each coefficient
   pair is absorbed before its challenge is sampled. Layer cost:
   `3*(ell+s)/2^128`.
3. **Child-pair reduction:** after checking the claimed product and
   absorbing both child evaluations, one challenge reduces the pair to its
   line interpolation. A false pair has interpolation error of degree at
   most one. Cost: `1/2^128`.

The verifier's Gruen reconstruction handles a zero incoming coordinate by
sending/checking the endpoint at one instead of dividing by zero. For a
nonzero coordinate its division is by that known nonzero value. A vanishing
accumulated equality factor does not add a separate exceptional-event term:
it is already a root of the same degree-at-most-three false-round polynomial.
The verifier checks the final eq factor times the two child values against
the running claim.

One global bridge grinder spans the root point and all layer messages. Each
sumcheck round and child-pair reduction starts a block by absorbing its
message. Coordinates within the root vector share one block. The nonce-seed
draw is forwarded directly and is not an additional unground protocol
challenge. The verifier consumes exactly the expected forest messages and
finishes the grinder; omitted, surplus, or invalid nonces cannot be accepted.

Consequently:

```
R = sum(ell+s, ell=0..d-1) = d*(d-1)/2 + d*s
blocks = 1 + R + d
numerator = s + 3*R + d.
```

The two limb root tables are projected together with one additional root
coordinate. This accounting differs from two independent forest proofs,
which would incur two root projections and two complete challenge schedules.
The chosen implementation retains the batched reduction.

## Terminal claim and coordinate order

Leaf `(j,l,r)` has binary-field value

```
1 + (A_l(r) + 1) * source_bit[j,r].
```

Split the terminal point as `[column_point | eta | row_point]`. Its value
`e` gives exactly the binary claim

```
low[r] = eq(row_point,r)
         * sum_l eq(eta,l) * (A_l(r) + 1)
high_point = column_point
target = e + 1.
```

The constant-one contribution evaluates to one at every point, since the
row, column, and limb equality weights each sum to one. Both limb copies
share the same original bit, so their deterministic row factors contract
before the existing joint binary sumcheck. It authenticates this claim
against the arithmetic source commitment, together with the Keccak claims
and explicit SHAKE/HashToPoint links. The joint sumcheck and PCS errors remain
separate terms in the whole hybrid report.

Exact shapes matter. The verifier accepts exactly two sum vectors, each of
length `2^c`, and exactly `R+d` pairs in the forest message stream. It runs
exactly `d` layers with `ell+s` sumcheck rounds per layer and consumes every
pair. The terminal point has `c+d+1` coordinates. No proof-supplied dimensions,
alternate degree schedule, or unused trailing messages override these
layout-derived lengths. The source constructor fixes column counts and row
lengths; the bridge checks weight counts and canonical weight bounds before
handing buffers to the optimized forest.

## Derived difficulties and composition

| Capacity | s | Sumcheck rounds R | Challenge blocks | Numerator | Bits at target 128 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 5 | 143 | 157 | 447 | 17 |
| 2 | 6 | 156 | 170 | 487 | 17 |
| 4 | 7 | 169 | 183 | 527 | 18 |
| 8 | 8 | 182 | 196 | 567 | 18 |
| 16 | 9 | 195 | 209 | 607 | 18 |
| 32 | 10 | 208 | 222 | 647 | 18 |
| 64 | 11 | 221 | 235 | 687 | 18 |
| 128 | 12 | 234 | 248 | 727 | 18 |
| 256 | 13 | 247 | 261 | 767 | 18 |
| 512 | 14 | 260 | 274 | 807 | 18 |
| 1024 | 15 | 273 | 287 | 847 | 18 |

The numerator is derived from the validated layout. The unchanged category
allocator uses

```
g = max(0, target + 8 + ceil(log2(numerator)) - 128).
```

The bridge therefore remains at most `2^-(target+8)`. Target 100 needs zero
grinding throughout the supported range. Target 128 needs 17 bits for capacities
one and two, and 18 bits for larger capacities. These counts and difficulties
are derived directly from the current forest geometry.

The complete native-ring composition includes seven prime category budgets
of `2^-133`, six binary budgets of `2^-136` including the bridge and both
Keccak slabs, a PCS budget of `2^-130`, and prime sampling `2^-144`.
Native carry batching additionally contributes at most `10/2^137`.
For native ideal batching/projection, `Q^11-Q>2^149` and `d+2046<2^12`,
so its error is below `2^-137`. The normalized sum is therefore at most

```
7/32 + 6/256 + 10/512 + 1/512 + 1/4 + 2^-16 < 0.514
```

at target 128 under the repository's computational grinding convention.
The whole-composition report checks actual terms at both supported targets for
every live batch size, including padded batches. See
[OPTIMIZATION_SECURITY.md](OPTIMIZATION_SECURITY.md) for the current allocation.

## Regression coverage

Relevant integration checks are: enumerate supported shapes and both
security targets; compare the partially reduced optimized forest against an
independent dense GKR oracle; verify the contracted terminal claim directly
against original source bits; check continuation challenges; and pin exact
message and nonce counts. Partial-depth tests need different images for the
two copies of each source row, so an accidental limb product or permutation
cannot pass unnoticed. Zero and one challenges exercise the Gruen endpoint
branches and vanishing equality factors.

Negative cases include altered and compensating limb sums, changed round
coefficients or child pairs, missing or surplus messages, and omitted,
surplus, invalid, wrong-domain, or wrong-difficulty nonces. Whole Falcon tests
must continue to reject invalid public signatures, SHAKE/HashToPoint links,
and individual norm violations.

These checks and the accounting above are implementation analysis, not an
independent cryptographic audit of the global Fiat-Shamir model.

## Code paths

- [`hybrid_bridge.rs`](hybrid_bridge.rs): local `limb_width` and `weight_limbs`
  helpers, magnitude/read-off checks, root
  derivation, interleaved source/images, forest adapter, endpoint contraction,
  shape validation, and grinder completion.
- [`src/bitz/fold.rs`](../../../bitz/fold.rs): bounded integer folds for each
  limb through `fold_columns` and the current PCS's binary `Shape`.
- [`src/bitz/forest.rs`](../../../bitz/forest.rs),
  [`src/bitz/gkr.rs`](../../../bitz/gkr.rs), and
  [`src/bitz/kernels.rs`](../../../bitz/kernels.rs): column-packed
  forest, partial product depth, MSB-first arity-two GKR, table-driven rounds
  and dense kernels.
- [`layout.rs`](layout.rs): fixed binary source dimensions and bit-index mapping.
- [`hybrid_keccak/grinding.rs`](hybrid_keccak/grinding.rs) and
  [`src/piop/spartan/grinding.rs`](../grinding.rs): block
  boundaries and domain-, difficulty-, and index-bound nonces.
- [`src/ligerito_flock/grinding_plan.rs`](../../../ligerito_flock/grinding_plan.rs)
  and [`src/hybrid/opening/grinding.rs`](../../../hybrid/opening/grinding.rs):
  current Flock work budgets and their shared-opening transcript adapter,
  with configuration, challenge-block, and nonce-consumption checks.
- [`hybrid.rs`](hybrid.rs): native-ring/v5 statement binding, category accounting, joint
  binary sumcheck, and shared PCS authentication.

The prefix helpers in `opening.rs` run under the enclosing Falcon statement
binding and feed this bridge directly.

The root challenge retains the transcript label
`bitz/falcon-hybrid/wfbitz-joint-limbs/v1`; this protocol label does not refer
to a separate Rust module or select another PCS implementation.
