# Speeding up the GKR forest: the broader idea map

Status: idea note, 2026-08-20. Companion to `docs/lut-width-ideas.md` (the
LUT-width program and its measured probes) and
`docs/forest-gkr-algorithm/main.tex`. Scope: everything in the
forest+presum phase, not just tables.

## 1. Where the milliseconds are (n=28, current master, L/4, one profiled prove)

| block | ms | share of forest | what it is |
|---|---|---|---|
| `eqf:grid` | 100 | 25% | dense-region fused double-fold passes (stored chain layers) |
| LUT messages | 106 | 27% | `pair3_r1/r2` 38, `leaf_r1` 31, `leaf3_r2/r3` 37 |
| materializing folds | 46 | 12% | `pair3mat` 24 + `leaf3mat` 22 (write 2×512 MiB) |
| `mf:bitgen` | 56 | 14% | JIT regen of level d−2, grid fused in (post-I4 factored) |
| `mf:build_levels` | 39 | 10% | gen_top + parent-halves chain (post-I4) |
| self/tables/suffix | ~28 | 7% | |
| **forest total** | **398** | | of a 485 ms prove |
| adjacent: `mc:fold_v` | 27 | — | integer folds `u_c` (tz-walk, data-dependent u128 adds) |
| adjacent: `mc:presum_tbls` | 16 | — | R-table build (already RS_FAST) |

## 2. The roofline, stated once

Two facts bound what kernel work can still buy:

- **The dense region runs near multiply throughput.** `eqf:grid` does ~200M
  wide-mul-equivalents in 100 ms ≈ 0.5 ns each — PMULL-pipeline territory.
  Its pass count is already optimal (one fused fold+grid pass per two
  rounds; the 9-point bilinear grid is the evaluation-optimal mul count for
  a 2×2 product). Nothing large is hiding here.
- **Total K-ops are at the linear-time-GKR bound and arity-invariant.**
  Σ_ℓ 2^ℓ = 2^{n+1} node-visits whatever the tree arity — which is exactly
  why the QUAD experiment measured a wash at n=28: arity trades visit count
  per pass against degree-5 op count per visit. Confirmed empirically.

Consequence: order-of-magnitude speedups are not available from execution
alone. They would need a cheaper binding field (GF(2^64)/GF(2^8) challenges
— **ruled unsound**, 2026-07-29 directive), a different hardware primitive
(§5), or a smaller statement. What remains below are 3–25% levers, plus the
n≥30 regime where gathers leave the roofline.

## 3. Tier 1 — structural, designs exist, deployed-shape wins

### S1 — The QUAD bottom-merge (the one big open structural lever)

> **MEASURED 2026-08-20, LANDED as `F2Z_QUAD=2`** (opt-in — it changes
> the transcript, like the whole quad lane; `force2` bypasses the knee):
> `prove_quad_bottom_sumcheck` runs the merged layer's rounds 1–3 off the
> committed bits (round 1: te/to gathers + one new unweighted 16-case ΔΔ
> table feed `quad_cross_k` directly — the pairing subtlety resolved by
> pairing ACROSS the top bit and permuting the finals back; rounds 2–3:
> the arity-2 cascade's own F₁/F₂ stash builders, τ-sliced per quarter),
> then the dense fused flow. Pinned transcript-identical to the dense
> quad driver over materialised quarters (`bottom_matches_dense_quad`,
> k=4..7); forest roundtrips at both parities + W=32, wrong-claim, codec,
> cross-plan rejection. Measured vs base (paired in-window, prove):
> **n=24 −25.3% (v1: −18.5%), n=26 −3..−9% (v1: +3.4%), n=28 −2..−6.3%,
> 3/3 pairs (v1: wash)** — the pass-halving survives in the gather-bound
> bottom exactly as projected, and the quad knee moves from n≤25 to
> n≤28 (`QUAD2_N_MAX`). Beyond n=28: fresh box. Carried limits: L/4
> only, peak unchanged (JIT-level-bound), α full-order throughout.
`docs/quad-bottom-merge-prompt.md`, gate `F2Z_QUAD=2`. Merge the leaf and
pair layers into ONE arity-4, degree-5 layer whose phase A runs k = d−2
rounds instead of (d−1) + (d−2) — the 152 ms LUT block (messages + mats) is
the target, and it is **byte/gather-bound, not mul-bound**, so the
pass-halving should survive the 3× per-slot op count that sank quad in the
mul-bound stored region. The 16-case table machinery carries over (the
merged layer's entries are the same bit-affine leaves two levels down).
Expected if the design's projection holds: −15–25% of forest. This is the
highest-value open item in the repo for prover time at deployed shapes.

### S2 — Mat+grid fusion (port a landed pattern)

> **MEASURED 2026-08-20, LANDED DEFAULT-ON** (`F2Z_MAT_GRID=0` opts out):
> the materialising folds accumulate the next round-pair's grid over
> just-written quads (cache-hot readback) and deposit it; a new
> deposited-grid message branch reads it with no pass, so the fresh
> buffers' first DRAM read moves to round j+3. n=28: grid 94.7→78–92 ms,
> mats +10–15 ms, prove **−1%** (3/3 pairs — the grid compute was partly
> hidden under the removed read's latency, so the net is smaller than the
> read). n=29 l8: grid −28/−31 ms vs mats +12/+17, prove **−2.5/−6.5%**
> (2/2) — grows with memory pressure. Caveat: a col-elided witness adds a
> Dense synthetic group among the bit groups, which disables the deposit
> (mixed-group fallback); special-casing its all-ones grid is a cheap
> follow-up if elided shapes matter.
`pair3mat`/`leaf3mat` write 512 MiB each and then the first dense grid pass
re-reads what was just written. `jit_layer_generate` already solves exactly
this for the JIT layer (`dense_jit_fused_grid`: accumulate the 3×3 grid
while writing). Port it to the two materializing folds: the fold's write
pass also accumulates the layer's next round-pair grid, deleting the first
`eqf:grid` pass per bottom layer. Expected −10–20 ms (3–5% of forest);
low risk; byte-identical by the same order-free-XOR argument.

### S3 — 4-Russians the integer fold (`fold_v`, 27 ms)

> **MEASURED 2026-08-20, LANDED DEFAULT-ON** (`F2Z_FOLDV_LUT=0` opts out):
> `mc:fold_v` 24.7–34.5 → **5.8–6.1 ms** at n=28 (~4.5×, −4–6% of prove)
> and run-to-run volatility collapses (±0.3 ms vs ±10). The multi-set
> (`fold_cols_multi_k`, extension path) keeps the tz-walk for now — its K
> sets would need K tables; same trick applies if ext:step1_folds ever
> shows up in a profile.
`fold_values_bits` walks set bits (trailing-zeros loop) and does a
data-dependent `acc += row_weights[i] << j` per bit. Precombine 4-row
groups once per prove: `T[g][nibble] = Σ_{i∈nibble} W_{4g+i}` (16·2^{t−2}
u128 entries = 8 MB at t=17; the 2-bit/4-entry variant is 2 MB if the
nibble table misses) — then each column is 2^{t−2} unconditional
table-adds, no tz-walk. Same trick as the forest's own tables, and the
build is 2^{t+2} adds shared across all 2^s columns. Expected 27 → ~10 ms
(−3–4% of prove). Also applies to `fold_values_bits_multi` (batched sets:
one shared nibble stream, K tables).

## 4. Tier 2 — the n≥30 regime (where gathers leave the roofline)

Everything gather-bound degrades past L2, and this is where the measured
wins concentrate (I1 −6%, I4 bitgen −10..−30%, churn-immunity). The open
items, from `docs/lut-width-ideas.md`:

- **I5 loop inversion / GEMM tiling** (slot-outer, tree-blocked, transposed
  bits, register selects): kills table residency as a constraint. Probe on
  the n=30 leaf round (the 2.1×-factored site).
- **I3 generator (masked-add) bodies**: fully sequential streams at any
  depth; composes with I5 (tiling fixes its τ-line re-streaming).
- Both need the **fresh-box protocol** — this box's n≥30 medians swing
  ±15–30% and cannot adjudicate.

## 5. Tier 2b — hardware primitives (exploratory probes, high variance)

### H1 — SME2 on M4: fixed-operand muls as GF(2) matrix products
Every fold multiplies a whole stream by round-fixed elements ((1+ρ), ρ) —
~200M of the prove's muls have a FIXED operand. Multiplication by a fixed
K-element is a 128×128 F₂-matrix; applied to a bit-sliced batch of values
it is GF(2) GEMM. If M4's SME2 exposes a binary/XOR outer-product
accumulate (BMOPA-class) this runs at matrix-engine rates; the open
questions are (a) does Apple implement the binary variant at all, (b) do
the bitslice transposes eat the win. One-day feasibility probe: micro-bench
a 128×N bitsliced fixed-mul against the PMULL loop. (The scalar-table
version of the same idea — 4-Russians byte tables per fixed operand — is
16 loads+XORs vs ~3 cycles of PMULL and loses on ALU; only a matrix engine
changes the constant.)

### H2 — Metal GPU, bit-sliced GF(2^128)
No CLMUL on GPU, but bit-sliced AND/XOR towers are massively parallel and
the M-series unified memory avoids copies. Candidate offload: the dense
region's grid passes (streaming, regular). Big engineering, uncertain net
on a bandwidth-shared 16 GB box; park unless a step-change is required.

## 6. Tier 3 — workload- and memory-shaped

- **Row-tail elision**: col-elision's algebra on the row axis. A witness
  that part-fills columns leaves trailing all-zero ROW ranges in every
  live column; those slots' LUT contributions are tree-independent
  constants — accumulate once, scale by Σ_c A_c. Wins proportional to row
  padding; zero at exact-power-of-2 fills.
- **I6 L/16** (T8Bits + chain top d−5 + Leaf4Bits): the RAM-ceiling mover
  (n=33 on 16 GB), time-neutral at best. Leaf4Bits (landed, opt-in) is its
  proven bottom half; the peak only moves when the chain top moves with it
  (measured: Leaf4 alone leaves peak at the build-phase watermark).
- **E/O AoS interleave** for dense streams (one stream instead of two per
  side-pair): a few % of the grid passes at most, complicates the in-place
  fold. Micro-bucket.

## 7. Closed / excluded — do not relitigate

Smaller challenge/binding field (GF(2^64) middle point, GF(2^8), α-order
tricks): **unsound at target** (2026-07-29 directive). Flock friendly
challenges: unsound for this forest. Uni-skip: low ceiling (2026-07 audit).
Quarks-style committed grand product: committing 2^n K-elements is the cost
the lazy forest exists to avoid. L/2 RAM-for-time: measured flat-to-worse.
Full QUAD in the stored region: wash at n≥28, substitute of the
double-fold. Thread scaling, llvm-mca-guided micro (Cyclone-model trap),
comb window, scalar-ALU clmul interleave (no ARM scalar clmul), branchy
skip-the-1 paths (mispredict-bound): all measured dead upstream.

## 8. Suggested order

1. **S3** (4-Russians fold_v) — smallest, self-contained, −3–4% prove.
2. **S2** (mat+grid fusion) — landed pattern, −3–5% forest.
3. **S1** (bottom-merge) — the big one; budget a real session against
   `docs/quad-bottom-merge-prompt.md`; gate `F2Z_QUAD=2`, byte-identity
   exempt (transcript changes — it's a protocol variant, needs its own
   verifier arm like the quad experiment had).
4. **H1 probe** (SME2 binary outer product feasibility) — one day, kills
   or opens the hardware lane.
5. n≥30 items (I5/I3) and I6 — on a fresh box.

Honest floor: S2+S3 ≈ −6–9% of prove; S1 is the only path to −20%-class
gains at n≤28, and past that the roofline argument says the next multiple
lives in hardware (H1/H2) or in opening fewer/smaller statements.
