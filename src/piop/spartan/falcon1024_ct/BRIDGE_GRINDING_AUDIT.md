# Falcon hybrid bridge grinding audit

This revision changes the grinding schedule only. SHAKE, HashToPoint, the
Falcon arithmetic constraints, all per-signature norm bounds, and the committed
sources remain in the proof. The top-level hybrid statement and transcript use
v5; the bridge grinding domain uses v3. The statement digest explicitly binds
the derived bridge numerator and grinding difficulty. Proofs made under the
v4 statement domain are not compatible.

## Scope and model

The calculation below applies to `hybrid_bridge::{prove, verify}` using
`verify_merged_forest`, not the separate experimental quad forest. It uses the
repository's existing computational grinding model: a challenge block with raw
error `e` and difficulty `g` contributes `e * 2^-g` per unit of adversarial
work. It does not claim that grinding improves unconditional interactive
soundness, nor add security beyond the separately stated BLAKE3 bound.

All supported live batch sizes are 1 through 1024 and are padded to their next
power of two. In the arithmetic source `word_bits=1`, `d=row_vars=13`, and
`col_vars=5+log2(capacity)`. There are exactly two bounded limbs, so the merged
tree-index MLE has `s=col_vars+1=6+log2(capacity)` coordinates. The complete
geometry range is therefore `d=13`, `6 <= s <= 16`.

## Integer binding has no probabilistic loss

Prime row weights are lifted canonically and split into width-113 limbs. The
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

## Accepted forest and challenge blocks

The verifier checks `layers.len()==d`. Every layer rejects a `pair2`, so the
experimental arity-four/degree-five protocol is not accepted by this bridge.
The root table contains both limbs, including the limb coordinate in `s`.
Its claimed integer sums and derived roots are absorbed before challenges.

1. **Root projection:** a wrong root table differs by a nonzero multilinear
   polynomial in `s` variables, of total degree at most `s`. All `s` uniform
   GF(2^128) draws are one uninterrupted challenge block. Cost: `s/2^128`.
2. **Layer ell, phase A:** the `ell` in-tree coordinates run the eq-weighted
   product sumcheck. The summand is `eq * left * right`, of individual degree
   at most three. Layer zero has no phase A. Cost: `3*ell/2^128`.
3. **Layer ell, phase B:** the `s` tree-index coordinates run the same
   degree-at-most-three sumcheck. This includes the limb coordinate and is
   not two independent forests. Cost: `3*s/2^128`.
4. **Layer ell, pair reduction:** the verifier checks the product of the two
   claimed child evaluations and absorbs the pair, then samples one line
   challenge. If the pair is wrong, its interpolation error is nonzero of
   degree at most one. Cost: `1/2^128`.

`verify_eq_inner_sumcheck_gruen` accepts exactly two cofactor coefficients per
round, reconstructs the constant from the current claim, absorbs the message,
draws one challenge, and reabsorbs that challenge. Its full polynomial remains
`eq1(X) * quadratic(X)`, of degree at most three. Coefficient recovery does not
increase the degree or add a random exceptional-event term. In particular,
when an equality factor vanishes at a sampled coordinate, that coordinate is
already a root of the same degree-at-most-three false-round difference
polynomial. It is not a separate failure event. The verifier never divides by
an equality factor. Prover-only `recovery_inverses` uses an optional inverse;
`eqf_inverse` returns `None` on zero and the original coefficient-computation
kernel runs instead. A zero public coordinate therefore causes no undefined
inverse or extra statistical assumption.

Each sumcheck round and each pair reduction has its own grinding block. The
root vector shares one block and its entire degree `s` is charged to that
block. The wrapper's inner nonce seed draw is forwarded directly and does not
introduce a recursive unground protocol challenge. `finish()` rejects missing,
extra, and invalid nonces before accepting the bridge result.

L4/L8 select stored levels only. JIT, coefficient recovery, flat storage, and
single/double/grid folding keep the same round messages and transcript order.
In particular, double folding computes two messages from an internal bivariate
grid but still absorbs and samples each univariate round separately. The
quad forest entry point is not reachable from the multi-claim bridge path.

Consequently, there are

```
R = sum(ell+s, ell=0..d-1) = d*(d-1)/2 + d*s
blocks = 1 + R + d
numerator = s + 3*R + d
```

The terminal leaf evaluation is exactly the binary linear claim returned by
`binary_claim`: its constant-one part sums to one, and the limb MLE coordinates
contract both sets of deterministic row weights against the same committed
bit column. The separate joint sumcheck and PCS authenticate this claim; their
error terms are already present in the whole hybrid report.

## Derived difficulties and composition

| Capacity | s | Sumcheck rounds R | Challenge blocks | Numerator | Bits at target 128 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 6 | 156 | 170 | 487 | 17 |
| 2 | 7 | 169 | 183 | 527 | 18 |
| 4 | 8 | 182 | 196 | 567 | 18 |
| 8 | 9 | 195 | 209 | 607 | 18 |
| 16 | 10 | 208 | 222 | 647 | 18 |
| 32 | 11 | 221 | 235 | 687 | 18 |
| 64 | 12 | 234 | 248 | 727 | 18 |
| 128 | 13 | 247 | 261 | 767 | 18 |
| 256 | 14 | 260 | 274 | 807 | 18 |
| 512 | 15 | 273 | 287 | 847 | 18 |
| 1024 | 16 | 286 | 300 | 887 | 18 |

The code derives the numerator from the validated layout rather than substituting
a smaller global magic constant. The unchanged category allocator chooses

```
g = max(0, target + 8 + ceil(log2(numerator)) - 128).
```

Every bridge category remains at most `2^-(target+8)`. Target 100 needs zero
grinding bits throughout the supported range. Target 128 needs 17 bits for one
signature and 18 bits for every larger capacity, previously 20 bits everywhere.
Expected nonce attempts per bridge block fall by eight times or four times,
respectively; this is not a claim of that speedup for the whole prover.

The surrounding union allocation is unchanged: seven prime category budgets
of `2^-133`, six binary budgets of `2^-136` including the bridge and both
Keccak slabs, a PCS budget of `2^-130`, and the prime sampling term `2^-144`.
Their normalized sum is at most

```
7/32 + 6/256 + 1/4 + 2^-16 < 0.493
```

at target 128. The programmatic security report checks the actual terms for
both targets and all 1024 live batch sizes, including non-power-of-two batches.

## Regression checks

The bridge geometry test enumerates every supported live batch size, checks
the exact row/column/limb shape, independently counts verifier rounds, and
checks the exponent bound and full-order generator. The roundtrip test pins
the emitted nonce count and rejects changed limbs, compensating limb sums,
changed pairs, and omitted/extra/altered nonces. A domain test constructs old
v2-domain and wrong-difficulty nonces that are invalid under the new domain,
and confirms their rejection without relying on chance seed collisions.
The whole hybrid report test enumerates all live batch sizes and both target
levels, checking both the bridge category allocation and the complete report.

These are implementation checks plus the analytical accounting above; they
are not an independent cryptographic audit of the global Fiat-Shamir model.

## Code paths checked

- `hybrid_bridge.rs`: canonical limb construction, sum magnitude checks,
  root derivation, `binary_claim`, and grinder completion.
- `layout.rs`: `new_hybrid`, `row_vars`, `col_vars`, `bitz_params`.
- `src/pcs.rs`: `mod_q_chunk_width`, `mod_q_num_chunks`,
  `GF128_ORDER_PRIME_FACTORS`, `is_generator`, `smallest_generator`.
- `src/merged_forest.rs`: `prove_merged_forest_lazy_multi_from_rows_with_scratch`,
  `prove_merged_forest_lazy_multi_impl_options`, `drive_grouped_reuse`,
  and `verify_merged_forest` (the accepted verifier).
- `src/piop/sumcheck/eq_factored.rs`: `verify_eq_inner_sumcheck_gruen`,
  `prove_eq_inner_sumcheck_mixed_prepared_recycle`, `recovery_inverses`, and the
  per-round message/draw/reabsorb loop. `src/poly/univariate/binary_gf128.rs`
  supplies the zero-checked `eqf_inverse`.
- `hybrid_keccak/grinding.rs`: block boundary transitions and `finish`.
  `src/piop/spartan/grinding.rs` binds domain, block index, and difficulty
  before deriving a seed and absorbing the checked nonce.
- `hybrid.rs`: whole-composition category accounting, `binary_grinding`,
  statement binding, and the same layout-derived difficulty on both sides.

The standalone legacy `commitment-bound/v4` domain in `opening.rs` remains
unchanged: it is used only by its separate prove/verify wrappers. The hybrid
calls the prefix helpers, whose contract requires the enclosing caller to
bind the statement. Only the hybrid benchmark's protocol label needs v5.
