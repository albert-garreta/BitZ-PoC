# Session prompt: the QUAD bottom merge — leaf+pair as ONE arity-4 bit-driven layer

Branch `worktree-quad` (worktree `.claude/worktrees/quad`, off taps64
`22087af`). Prior legs: the full-order-α K-input QUAD port (`5bf3132`),
the even-levels-only chain (`1159d41`), the restructured degree-5 bodies
(`04f7d41`, `quad_slot_k`: w-prefold + Karatsuba-3 wide cross stage,
byte-identity pinned vs the naive bodies). Measured state (M4, sandwiches):
quad wins **−18.3 % at n=24, −4.9 % at n=26**, ties n=28; n=30
unresolvable on a churned box (±9 % band between adjacent runs — needs a
memory-fresh box). Peak is identical to base at every shape: the
level-(d−2) JIT sets the HWM in both arms.

## The goal

Replace the LAST TWO arity-2 layers (pair, output d−2, `Pair3Bits`; leaf,
output d−1, `Leaf3Bits`) with ONE arity-4 bit-driven layer: output d−2,
consuming the LEAF level directly — `L_{d−2}(y) = Π_{m<4} Q_m(y)` with
`Q_m(y) = leaf(y + m·2^{d−2})` (m = a | b≪1, b the top bit). This is the
gf8 ledger's "leaf+pair merge w/ 3-deep bit prefix — the mass: would
halve bottom K materialisation". Expected wins: the entire leaf-layer
sweep disappears (its round-1 subset-sum loads over 2^{d−1} slots, its
stash rounds, its fold), one phase-B + one line step + one dense residue
go with it, and the bottom's dense materialization halves
(4·2·2^{d−5} = 2^{d−3}/tree vs today's 2·2^{d−4}+2·2^{d−4} = 2^{d−2}).
The peak does NOT drop (the d−2 JIT for the layer above still
materializes 2^{d−2}/tree); this is a time lever.

## The exit claim is unchanged

The bottom quad closes on four leaf-quarter evaluations (pair + pair2,
tag 0x33) and interpolates two line challenges — the advanced claim is
`MLE[leaves](r_x ++ [μ_a, μ_b], r_c)`, exactly the exit shape the
pre-sumcheck consumes today. The discharge, `q_rowbit`, ring-switch and
Ligerito are untouched.

## The plan (both parities)

- d EVEN: upper quads output `0, 2, …, d−4` (last consumes the d−2 JIT),
  then the bottom quad (output d−2, consumes leaves). No parity layer.
- d ODD: upper quads output `0, 2, …, d−5` (all inputs stored evens
  ≤ d−3), ONE arity-2 parity layer (output d−3, consumes the d−2 JIT —
  exactly today's parity layer), then the bottom quad. Stored chain:
  evens ≤ d−3 (odd) / ≤ d−4 (even), as `build_levels_quad` already
  produces.
- Layer count: even d: (d−2)/2 + 1; odd d: (d−5)/2 + 1 + 1 + 1. Verifier
  (`verify_merged_forest_quad`) gets the same plan change; the bottom
  quad verifies at degree 5 over d−2 phase-A vars like any quad layer —
  only the PLAN changes, not the per-layer checks. Gate as `BITZ_QUAD=2`
  (v1 stays at `=1` for A/B; `quad_active` returns the level).

## Round 1 from the case tables — the pairing subtlety

Round 1's slot s pairs positions (2s, 2s+1). Each multiplicand is
leaf-affine, so each PAIR's quadratic-in-T coefficients are 4-bit-case
GATHERS, not multiplies — but only if the Karatsuba pairs multiplicands
ACROSS THE TOP BIT (b): the existing `te`/`to` tables pair leaves
`(y, y+2^{d−1})` (E-side) and `(y+2^{d−2}, y+3·2^{d−2})` (O-side), i.e.
Q0×Q2 and Q1×Q3 — while `quad_slot_k` pairs `(a[0],a[1])`, `(a[2],a[3])`
= Q0×Q1, Q2×Q3. Product bracketing is free, so EITHER permute the
multiplicand order to `[Q00, Q01, Q10, Q11]` entering the slot (and
permute the finals back before the 0x33 absorb so the close stays
`[Q00, Q10, Q01, Q11]` for `quad_interp(μ_a, μ_b)`), OR build new
`tp`/`tq` tables with the a-axis pairing. The permute is cheaper — no
new tables for p(0)/p(1).

Per pair p(T) = p0 + p1·T + p2·T²:
- `p(0)` = te-gather at position 2s, `p(1)` = te-gather at 2s+1 (the SAME
  te table, two gathers);
- `p2 = ΔF₁·ΔF₂` where `ΔF_i = b_{2s+1,i}·τ_{2s+1,i} + b_{2s,i}·τ_{2s,i}`
  — a NEW 16-case ΔΔ table per position (the leaf machinery's ΔLΔR
  tables are this shape; build tree-shared, O(2^d) muls, alongside
  te/to);
- `p1 = p(0) + p(1) + p2` (char 2).

Then the `quad_slot_k` tail verbatim: w-prefold into one pair's
coefficients (3 muls: w·p0, w·p1, w·p2 — the prefold-into-factors trick
doesn't apply to gathered products), Karatsuba-3 cross stage (6
mul_wide), zero cross-stage reductions. Round-1 slot ≈ 3 full + 6 wide
muls + 6 gathers over 2^{d−3} slots — vs today's pair-layer round 1
(3 wide muls × 2^{d−2} slots) PLUS the whole leaf layer.

## Rounds 2–3: the stash cascade at degree 5

Mirror `Leaf2Bits`/`Leaf3Bits`, per multiplicand:
- Round 1 defers its fold (the driver's pending-ρ pattern). Round 2's
  fused sweep needs `V_m(ρ₁)` per position — for a bit-affine pair this
  is the `build_leaf_fold_tables` identity
  `V(ρ₁) = 1 + b₀(1+ρ₁)τ₀ + b₁ρ₁τ₁`: a 4-case stash table per position
  per multiplicand (four tables total, τ-slices per quarter). Round 2's
  slot gathers `(a, δ)` per multiplicand from these and runs the plain
  `quad_slot_k` (8 full + 6 wide).
- Round 3 (the "3-deep prefix"): stash round 2's fold tables (16-case
  over the four bits of two adjacent positions) as round 3's inline
  value tables; dense buffers appear only at round 3's fold —
  4 quarters × 2·2^{d−5} = 2^{d−3}/tree.
- `BITZ_QUAD_LUT=…` opt-outs mirroring `BITZ_LUT3`, all variants pinned
  byte-identical to the shallow path (the identities are the same char-2
  re-associations as the arity-2 cascade; transcript unchanged across
  variants by construction).

New `QuadBufs`-style input enum in `piop/sumcheck/quad.rs` (it was
removed in the K-port; reintroduce as
`{ Dense([Vec<Gf>;4]), LeafBits{…} }` with the stash state threaded like
`eq_factored`'s `GroupBufs` rounds), or — likely cleaner — keep
`prove_quad_eq_sumcheck` Dense-only and give the bottom layer its own
mini-driver sharing `quad_slot_k` and the message/transcript block.

## Table inventory

Existing: `te`, `to` (4-case pair values), `pair_tbl`, `leaf_tau`
halves, T4 (upper layers' JIT). New: the ΔΔ 16-case tables (E- and
O-side), the per-round stash builders (4-case fold tables, 16-case
round-2 fold tables). All tree-shared, O(2^d) build cost. W > 1 works
unchanged (leaf index = (b≪log₂W)|j; the quarters split the TOP two
row bits; te/to already index through `pow2`).

## Tests and measurement

- Roundtrip legs at both parities and W>1: (10,5,1) d=10 even,
  (4,8,32) d=9 odd — extend the existing quad legs to `BITZ_QUAD=2`.
- Byte-identity: stash-variant knobs vs the shallow bottom-quad path.
- Wrong-claim, codec, arity-2-rejects — as v1.
- Measure: sandwiches n=22–28 vs BOTH base and quad-v1 (`BITZ_QUAD=1`),
  kernel on; expect the n=24/26 wins to compound and n=28 to tip
  positive; n=30 only on a memory-fresh box (±9 % churn band measured
  2026-07-30 on a used box — alternating pairs disagreed in sign).

## Known limits to carry

- Quad (v1 and v2) requires L/4 (`quad_active` excludes `l8`), so
  n ≥ 31 on a 16 GB box cannot run quad — an l8-compatible quad
  (chain at d−4, T4Bits-style feeds) is future work if the fresh-box
  n=30 result justifies it.
- Peak stays JIT-level-bound; do not sell this as a memory lever.
- α is a K^× generator (order 2^128−1) — no byte/dlog surfaces anywhere
  (see the gf8 experiment's constraint pin, 2026-07-29).
