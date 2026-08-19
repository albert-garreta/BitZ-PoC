# Shrinking the grand-product LUTs: ideas for deeper bit-driven rounds

Status: idea note, 2026-08-19. Companion to `docs/forest-gkr-algorithm/main.tex`
§4 (the width laws, the r+f ≤ 2 cap) and `docs/forest-gkr-note/` (measured
costs). Everything here is prover-execution only: no transcript byte, no
soundness surface. Every idea below is an exact char-2 re-association
(distributivity + masked XOR + F₂-linear deferred reduction), so the
`lazy_matches_eager` byte-identity pins stay the acceptance test.

## 1. The problem, restated

An entry of level `d−r` after `f` folds is selected by `2^(r+f)` committed
bits, so its precombined case table holds `2^(2^(r+f))` cases per position —
each extra LUT round **squares** the case count (width law,
algorithm-note §4.2):

| r+f | cases | table at d=17 (n=28) | today |
|-----|-------|----------------------|-------|
| 0 | 2 | 2 MiB | τ halves |
| 1 | 4 | 4 MiB | `te`/`to`, `F₁` |
| 2 | 16 | 8 MiB | `T4`, `F₂` |
| 3 | 256 | ~128 MiB | **not built** |
| 4 | 65 536 | ~8 GiB | absurd |

The cap r+f ≤ 2 is set by three compounding ceilings: (i) **residency** —
tables are gathered `live ≈ 2^s` times per round with data-dependent
indices, which only pays while the table sits in L2; (ii) **build cost** —
one multiply per entry, and at r+f = 3 the build makes 32× more values than
the level has nodes per tree; (iii) **returns** — each +1 buys one more
bit-driven round.

The goal: make the byte cost of depth grow **linearly, not
doubly-exponentially**, so rounds 4–5 of the leaf cascade, round 3 of the
pair cascade, and a whole extra implicit level (`T8Bits`) become available.

## 2. Two structural identities the current tables under-use

**(A) The stash tables are affine in the bits — 2^f generators span all
2^(2^f) cases.** After f folds a leaf-side entry is

```
E_f[p][bits] = 1 + Σ_{ε ∈ {0,1}^f} eq(ρ₁..ρ_f, ε) · m_ε · τ[2^f·p + ε]
```

— a subset-XOR of `2^f` position-indexed *generators*
`g_ε = eq(ρ,ε)·τ[·]` plus the constant 1. The `2^(2^f)`-case table is the
XOR-closure of `2^f` generators; storing the closure is what squares.
Generator storage is **flat**: exactly the τ lines, reweighted
(2·2^k entries per side forever, ≈ 2 MiB at n=28, 4 MiB at n=30).
The existing `LeafA2::Factored` (4 raw cross products + 4 branchless masked
adds, −2.1× leaf_r1 at n=30) is precisely this identity used once; nothing
stops it from being used at every depth.

**(B) The fold IS a 2-term factorization.** The stash step is

```
F_{f+1}[p][c_lo | c_hi≪w] = (1+ρ)·F_f[2p][c_lo] + ρ·F_f[2p+1][c_hi]
```

— by definition, every deep entry is the XOR of **two gathered entries of the
previous table** (with the ρ-weights premultiplied into the two halves).
Building `F_{f+1}` eagerly is a choice, not a necessity: consuming the
right-hand side directly costs 2 gathers + 1 XOR per entry and **freezes the
table footprint at the 16-case level for any depth**.

**(C) The multiplicative tables are rank-1.** `te[c] = v_lo^{m₀}·v_hi^{m₁}`,
`T4 = te·to`, and `te`'s 4 cases are literally `{1, v_lo, v_hi, pair}` — all
selects/products over the already-stored `v` and `pair` lines. `T4` (8 MiB at
n=28 → 32 MiB at n=30 → 128 MiB at n=32) is redundant with 3 MiB of lines
plus muls.

## 3. The ideas

### I1 — Finish the leaf-table factorization: 8 entries per slot (from 12/24)

> **MEASURED 2026-08-19** (`LeafTables::Raw8`, landed with auto-pick
> `half ≥ 2^17`, `F2Z_LEAF8=0/1` forces; l8, churned box, alternated
> in-window pairs, `eqf:msg:leaf_r1` medians): n=28 **+18%** (31.4→37.2 ms),
> n=29 **+15%** (72→82.5) — the split form's two picks are line-local and
> L2-cheap, and 10 masked adds out-cost the 64 B/slot saved — but n=30
> **−6%** (202.4→189.5 ms, 3/3 pairs) plus ~2× cheaper build once
> DRAM-bound. Verdict: real but modest, regime exactly the ΔΔ knee one
> size later; the interesting consequence is that masked-add ALU ≈ 6 adds
> costs about 64 sequential bytes at this size — calibrates I3's economics.

The build already makes exactly 8 products per slot (4 singles
`ωτ_{L0}, ωτ_{L1}, ωτ_{R0}, ωτ_{R1}`, 4 crosses `p_εδ = ωτ_{Lε}τ_{Rδ}`) and
then spends ~20 adds precombining them into 24 (or 12) stored entries. Store
the **8 raw products** instead, one 128-B region (2 lines) per slot,
consumed fully sequentially:

- `A₀` += masked(`ωτ_{L0}`, m_L0) + masked(`ωτ_{R0}`, m_R0) + masked(`p₀₀`, m_L0∧m_R0)
- `S₁` likewise from the odd singles + `p₁₁`
- `ΔΔ` = the existing 4 masked adds of the crosses.

10 masked XORs per slot, **zero data-dependent loads** (today `t_a0`/`t_a1`
are line-local picks, ΔΔ-factored is already masked). Table bytes:
24→12→**8** entries/slot = 12.6 → 6.3 → **4.2 MB** at n=28; 50 → 25 →
**16.8 MB** at n=30. Build gets cheaper too (the ~20 precombine adds
disappear). Expected regime: wins where the 12-entry set misses (n ≥ 28–30),
loses ~1 ms where L2-resident (keep the `leaf_a2_factored`-style
auto-threshold).

*Aggressive variant (4 entries/slot, 64 B = 1 line):* store only the
weighted singles and recover the crosses with one wide multiply per slot,
pair2-factored style: `ΔL·ΔR = (Σ masked ωτ_L)·(Σ masked τ_R)` against the
raw τ lines, and the `A₀`/`S₁` cross terms via the 5-mul dense schedule.
More muls, half the bytes again — "multiplies are cheap next to bytes that
miss" says measure it at n ≥ 30.

### I2 — Never precombine a stash: factored rounds 4 and 5 (`Leaf4Bits`/`Leaf5Bits`)

Apply identity (B) instead of building `F₃`:

- After ρ₃, **reweight `F₂` in place** (one shared sweep, `16·2^{k−2}` muls
  per side: even positions ×(1+ρ₃), odd ×ρ₃ — or equivalently keep a
  position-periodic weight folded in each round).
- Round 4's body gathers **two nibble-selected `F₂` entries + 1 XOR** per
  entry (windows: two bytes per entry, still aligned; ΔE·ΔO stays one
  explicit wide mul of two such XORs — constants cancel in Δ, so the cross
  term is pure subset-XOR before the multiply).
- Round 4's fold materialises dense buffers exactly as round 3's does today,
  just one round later: **dense birth halves, 512 → 256 MiB at n=28**
  (2^{n−3} → 2^{n−4} elements). Round 5 = the same trick again (4 gathers +
  3 XORs per entry from the twice-reweighted quarters; total table bytes
  still `16·2^{k−2}` per side — **flat at any depth**).

Memory factor per the note's own economics: an executed round-3 fold stores
`live·2^{k−3}` values per side; this stash stores `32·2^{k−3}` **total** —
live/32 (64× at s=11). Same pattern gives `Pair4Bits` (round 3 of the pair
cascade off two nibble-gathers of the pair stash) and — see I6 — `T8Bits`.

Caveat to state up front: per-tree gather count in the round-4 pass roughly
doubles versus the round-3 pass it follows (2 gathers per entry instead
of 1), while the pass it replaces was streaming reads of freshly written
dense buffers. The win claimed is the removed materialise-then-read round
trip plus the memory; the note's rule of thumb (one gather sweep ≈ one
streaming pass) makes this roughly time-neutral at n=28 and favorable
wherever the dense buffers would have missed. The real time story needs I3/I5
at big n because `F₂` itself is 32 MiB at n=30 (already past L2 — today's
round 3 is already DRAM-gathering there).

### I3 — The generator (masked-add) round body: depth without any table at all

Identity (A) taken to its endpoint. Keep **only the τ halves**, reweighted in
place once per challenge (`τ'[i] ← τ[i]·eq₁(ρ_f; bit)` — `2^k` shared muls
per side per round, sub-ms). Round f+1's entry is then

```
E[p] = 1 + XOR over the 2^f masked generators τ'[2^f·p .. 2^f·p + 2^f)
```

— `2^f` branchless masked XORs per entry, reading **one contiguous
`2^f·16`-B block per entry, fully sequential**, zero gathers, table footprint
= the τ lines (2–8 MiB at n=28–32, L2-resident at every deployed shape).
Slots shrink 2× per round while masked adds per entry grow 2×, so each round
costs a flat ~`2^k` masked XORs + one τ-line stream per tree per side.

This is the ΔΔ-factored pattern (`add_assign_masked`) at width `2^f`, and it
is the form that scales to **any depth and any n**: no residency ceiling, no
build blow-up, prefetch-perfect. Its cost is τ-line re-streaming `live` times
per round (≈ 4 GiB/round at n=28, s=11) — which is exactly what I5's tiling
removes. I2 is the better round-4 form at n ≤ 28; I3 (or I2+I5) takes over
where tables outgrow L2.

### I4 — Factored `T4` consumption at n ≥ 30 (and `te`/`to` as views)

> **MEASURED 2026-08-20** (`T4Src`, landed default-ON for L/2+L/4 — where
> `T4` is then never built at all — precombined kept on L/8;
> `F2Z_T4_FACTORED=0/1` forces; alternated in-window pairs): L/4 `gen_top`
> **−23%** at n=28 (53.5→41.0 ms medians, far less volatile), build/bitgen
> **−9..−12%** at n=29 (3/3 pairs each, and churn-immune: worst-case
> bitgen 130 vs 215 ms under a swap event); l8 at n=30 the JIT wins −30%
> (2/3 pairs) but the 4-gather `gen_top` is wash-to-worse (its extra
> multiplies bite) — hence the schedule-split default. The surprise: the
> win starts at n=28 where `T4` is nominally L2-resident — the paired
> `at(y)·at(y+h3)` access defeats locality earlier than table size alone
> suggests. Footprint: −8/−16/−32 MiB (n=28/29/30) plus the build
> multiplies, on the default schedule.

`T4` is gathered by `gen_top`, the JIT layer, and `T4Bits`; at n=30/32 it is
32/128 MiB and every gather misses (the `F2Z_T4_PRFM` default flipping on at
≥16 MiB is the tell). By identity (C):

- **2-gather form**: `T4[y][c_E|c_O] = te[4y+c_E] · to[4y+c_O]` — 2 gathers
  into the half-sized `te`/`to` + 1 mul;
- **4-select form**: `te`/`to` entries are muxes over `{1, v_lo, v_hi, pair}`
  — 3–4 sequential line reads + branchless selects + muls from **3 MiB of
  `v`+`pair` lines**, L2-resident even at n=33.

Auto-pick by footprint exactly like `leaf_a2_factored`. This also means
`te`/`to` need not be *stored* at all (they are views over `v`+`pair`),
saving their 4–16 MiB where the pair-round working set is tight. Expected
effect concentrates at n ≥ 30, where the memory says bitgen/JIT contains the
T4 sweeps and PRFM already bought −5%: replacing DRAM gathers by L2 gathers
+ muls is the same trade `Pair2Tables::Factored` already won.

### I5 — Loop inversion (slot-outer, tree-blocked): kill the residency ceiling structurally

Today every round is tree-outer: each of the `live` trees gathers from the
shared table, so the table must be L2-resident to pay. Invert and tile,
GEMM-style: iterate slot-blocks outer, tree-blocks inner (e.g. 64 trees),
with

- the slot-block's table lines loaded **once** into registers/L1 (8–24
  entries/slot after I1),
- per-tree cases read from the **transposed bit layout** (already exists for
  `T4Bits`),
- per-tree `(A₀, S₁, A₂)` wide accumulators for the block (64 trees × 3 ×
  32 B = 6 KB, L1-resident),
- selects done as register BSL/masked-XOR — **zero data-dependent loads
  anywhere**.

Table traffic becomes `size × (live/blocksize)` sequential streams instead of
`live × 2^{k−1}` random gathers; residency stops being a constraint on table
*size* entirely, which is what makes I2/I3 at depth 4–5 and n ≥ 30 actually
gather-free. Highest implementation risk of the list; probe first on the one
round with the worst measured miss profile (n=30 leaf round 1, the 2.1× site,
or leaf3_r3 which is why stash-gather PRFM exists).

### I6 — Cash the depth in as the L/16 schedule (the RAM-ceiling mover)

Deeper LUT rounds are worth the most as the enabler of the next schedule
notch, which the reference table lists as the missing unlock past n=32:

- level d−2 implicit: `T4Bits` (exists, l8);
- **level d−3 implicit: `T8Bits`** — each entry inline as 2 T4-gathers +
  1 mul (identity B at r=3, f=0; with I4's factored T4 at these shapes);
- level d−4: JIT at 4 gathers + 3 muls (the l8 `gen_top` form, one level up);
- chain top d−5: `gen_top` at 8 gathers + 7 muls;
- leaf/pair cascades at depth 4/3 (I2) so the dense-birth residue
  (`2^{n−3}·16 B`, 2 GiB at n=33) halves too.

Peak ≈ `2^n·1 B` chain + hint + halved dense birth ≈ ~14–15 GiB at n=33:
one more n on the 16 GB box, two with I2 at depth 5, versus "n=32 is the
wall" today. This is also the only idea whose *value* is mostly memory, so
it stays worthwhile even if the time deltas of I2/I3 measure neutral.

## 4. Anti-ideas (checked, don't spend time)

- **Smaller binding field / narrower entries.** GF(2^64)/GF(2^8) challenge or
  α-order tricks are ruled unsound (2026-07-29 directive: α must be assumed
  order 2^128−1); τ/F entries are dense 128-bit with no compressible
  substructure. Entry width stays 16 B.
- **Frobenius orbit sharing.** `v(b,j) = φ^j(α^{W_b})` would compress the v
  table W-fold — but deployed W=1 has no j axis, and challenges destroy the
  orbit structure for every fold table. Only worth remembering if a W=32/64
  cell shape ever ships.
- **Branchy sparsity / skip-the-1 paths.** Measured mispredict-bound at 50/50
  bits (note §4.3); all wins above are branchless.
- **Hash-consing / dedup within a table.** Generators are arbitrary field
  elements; the 2^(2^f) cases are distinct. Factoring, not dedup, is the
  compression.

## 5. Measurement plan

Order by expected value per effort, A/B-flagged like the precedents
(`F2Z_LEAF_A2_FACTORED` pattern), byte-identity pinned each step:

1. **I1** (8-entry leaf tables): local change in `build_leaf_tables` +
   `leaf_a2_slot_add`-style consumption; measure leaf_r1 at n=26/28/30
   (expect ~wash / win / bigger win; keep auto-threshold).
2. **I4** (factored T4): touch `T4At`/`gen_top`/`t4_level_*`; measure
   bitgen+JIT scopes at n=30/32 on a fresh box.
3. **I2** (`Leaf4Bits`, then `Pair4Bits`): new `GroupBufs` arm + the in-place
   `F₂` reweight; measure forest phase + peak at n=28 (l8) and n=30.
4. **I6** (`T8Bits` + L/16): after 2+3, the n=33 fit experiment.
5. **I3/I5**: prototype the generator body and the tiled loop on the n=30
   leaf round only; go wide only if the probe clears the 2.1×-site bar.

Protocol as always: one shape per process at n ≥ 24, `OBLONG_PROFILE=1`
scopes, alternated in-window pairs, `vm_stat`-fresh box for n ≥ 30.
