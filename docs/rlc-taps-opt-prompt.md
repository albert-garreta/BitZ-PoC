# Session prompt — structured-taps optimization: the XOR-mixed regime

You are in `/Users/albertgarretafontelles/f2z-pcs` (the `f2z-pcs` skill
applies: build/test/bench conventions, measurement protocol, unsigned
commits). This session optimizes the structured-taps tool stack landed
2026-07-27 (`29825ba..da39fa7`), with the XOR-mixed instances as the
priority. Read FIRST:

1. `docs/rlc-structured-taps-phase0.md` — the corrected semantics
   (entry-axis `2^g`-bit word groups on W=1 vectors), the translated-eq
   opening theory, the measured postscript.
2. `docs/rlc-family-note/main.tex` §"Structured taps" (`sec:taps`) — the
   formal statements (`lem:notperm`, `prop:transeq`, `eq:wtransform`)
   and the cheap/expensive law.
3. The landed code: `src/taps.rs` (TapOp/TapUniOp, extraction, classes,
   MPS closure); in `src/ligerito_flock.rs` the tap-claims path (0x42),
   the stream family (0x43), the collapse (0x44,
   `TapPointClaim { cols, op, claimed }`); `examples/taps_ab.rs`;
   `src/bin/f2z.rs` (`--taps vx|family|collapse|rotxor`).
4. The README's dated structured-taps notes (both) — tables, phase
   attributions, pitfalls.

## State of play (all at ONE shared evaluation point — the pinned regime)

| claim shape | best path today | cost |
|---|---|---|
| single tap / uniform-op-of-XOR-set | the collapse (0x44) | ≤ #sets×2 bodies TOTAL; marginal claims free |
| **XOR-mixed** (differently-transformed operands in one claim) | batched tap-claims (0x42) | **one padded forest body per claim** |
| dense stream reuse (claims ≫ streams) | stream family (0x43) | only regime where it wins; loses everywhere else |

Measured anchors (16 GB M-series, medians): `single` 25.5/94.4/146 ms at
n=22/24/26; `vx6` (k=6 mixed, pads to 8 sets) 118.7/319.3/2006 —
inverting against `ind6` (940) at n=26 on a churned box; the family
257.9/743.4/3823 (45 channels × ~13 ms discharge = 68–81 % of prove);
the collapse 53.5/177.3/603 for 13 single-tap claims via 4 bodies.
vx6 proof 566 KB at n=24, of which the sent fold vectors `us` are
~384 KB. The per-channel discharge constant (~13–20 ms) matches the
shared-point session's independent measurement.

## Ranked investigation list

**P1 — the composed collapse: uniform outer ops over MIXED sources
(the expected order-of-magnitude win).** The collapse's identity
`Σ_p w[p]·op(x)[p] = Σ_{p'} w[σ(p')]·x[p']` never used that `x` is a
plain XOR of columns — only that the INNER claim on `x` is provable.
Generalize the source from an XOR set to a **fixed XOR-of-taps
combination** (`x = ⊕_t op_t(a_{i_t})`, extractable, provable by the
tap-claims path): claims `OUTER(x)` group by (canonical source
combination, carry branch) exactly as today, and the inner claims
become TAP claims (per-claim row weights — the batched common already
takes them; residuals close via the existing translated-eq rings).
Why this is big: **schedule-shaped workloads are shift-invariant** —
in a SHA-2-like message schedule every round's mixed combination is a
word-offset of ONE fixed combination (`b_{t+1} = off¹(b_t)`), so k
rounds = k claims `off^t(x)` of one mixed `x` → **one inner tap body
(plus one carry branch) for the whole family**, versus one body per
claim today (k=48 pads to 64). The mixed (carry) cost is paid once per
distinct combination SHAPE, not per claim. Build: `TapPointClaim`
source generalization (or a sibling struct), statement absorb of
source taps + outer op, plan keyed on the canonical source; watch
`off < 2^{s−g}` (need `s − g ≥ ⌈log₂ max-offset⌉`; shapes n ≥ 24 give
s ≥ 12 ✓). Benchmark: a 48-round schedule-shaped instance
(`off^t(σ-combo)`-style) vs the batched path — target ≥ 10×. Sanity
anchors: values cross-checked via distributed extraction; inner-count
assertions; tampers on source taps and outer ops.

**P2 — kill the pad / fix the batched forest's memory wall.** The
batched common pads tree-sets to a power of two (k=6 → 8: +33 % pure
waste; k=5 → 8: +60 %; the shared-point k=15 case → 16 = 17.2 GB).
Options, in order of ambition: (a) partition claims into power-of-two
blocks sharing one tail (4+2 for k=6) — check the FS restructure
honestly (separate forest absorbs are fine; the rings/basis/Ligerito
stay shared); (b) ragged tree-set counts in
`prove_merged_forest_lazy_multi` (dummy trees are already all-ones —
the pad exists for the merged-forest power-of-two shape; see whether
the merge tolerates a final partial level); (c) BLOCK the batched
forest's working set (process tree-sets in blocks, freeing leaves) to
fix the n=26 inversion (vx6 2006 ms vs ind6 940 — the batch should
never lose to independent proofs). Accept criterion: vx6 ≤ 6.2× single
at every n ≤ 28 on a memory-fresh box.

**P3 — `x_fold_extra` (δ) for the tap paths (proof bytes).** The tap
machinery asserts δ = 0; the vx path supports δ > 0 (sent folds shrink
2^δ×, q_rowbit tables grow 2^δ×, flat index order unchanged). The `us`
vectors are ~68 % of vx6's bytes at n=24 (384 of 566 KB) — δ=2 cuts
them 4×. The work is coordinate bookkeeping in `src/taps.rs` (the
chain fields' positions under the re-split — the group field must stay
inside the CLEAR part: assert `g ≤ s − δ`) plus the collapse's branch
row-weight builder. Byte-measure before/after; no new soundness
surface.

**P4 — combined-η rings (bytes + verify).** Per-(tap, class) rings are
2 KiB each (vx6: 14, family: 70). After the residuals are absorbed,
draw combination challenges and send ONE `s_v` for the η-combined
functional per surface (basis = the combined Φ∘B). Standard RLC over
K, error 1/|K| — the same pattern as the existing η-batched basis.
Cuts family rings 70 → ~10 and the MPS closures accordingly (verify
1.6–2.1× → ~1×). Do NOT expect prover-time wins (rings ≈ 13 % of the
family prove, ~4 % of vx6).

**P5 — cascade round kernels (only if the family regime matters).**
The ~13–20 ms/channel constant: bit-LUT rounds for the leaf-bit driver
(the M's are 0/1; nodes 0/1 are multiply-free; post-fold values are
the 4-case `Pair2TauSet` shape), and the per-(spec, col) group
overhead at 45 × 2^s groups. Projected 2–3×, NOT enough to flip the
thin-sharing verdict (the channel count is multiplicative); the payoff
is the shared-point maximal families (discharge = 60 % of prove
there) — **coordinate with that session; same checkout, same lever.**

**P6 — record-only analyses (do not build without new numbers).**
(a) Direct carry-decomposition of a mixed claim
(`Σw·(⊕s_i) = ΣΣw·s_i − 2Σ pairs + …`): every AND monomial needs its
own exponent binding (a forest body) — wins only when distinct
monomials + streams < k, i.e. dense reuse again; cost it, don't build.
(b) The `ℓ`-space (per-claim-bit) family: 2^k − 1 channels — strictly
worse. (c) Cross-cluster/global cascades: blocked by the eqf driver's
shared suffix tensors (one exit point per level). (d) Friendly
challenges for the forest: audited unsound (memory ledger). (e) A
clustering search for thin sharing: measured hopeless (~⅓ body per
channel against 1 body per claim saved).

## Boundaries to confirm with the user before deep work

- The three pinned conventions (`a_{3,k−2}` → `a_1[k−2]`, bare `ROT` →
  `ROT^2`, translation direction toward higher within-word positions).
- Whether real workloads combine schedule terms by XOR or by modular
  ADDITION (SHA-2 uses +): ADD needs committed carry columns — a
  different arithmetization question, out of scope unless confirmed in.
- Whether k, the offset range, and the group width fit the `s − g`
  headroom at the target shapes.

## Staging

1. P1 first (the XOR-mixed headline): derivation note → API →
   roundtrip/tamper tests → the schedule benchmark vs the batched path
   at n = 22–26 (n = 28 only on a memory-fresh box).
2. P2(a) next (blocked batching — small, unlocks honest k > 4 numbers
   everywhere), then P3 (bytes), P4.
3. Measure per the repo protocol: alternated in-window medians, one
   shape per process at n ≥ 24, `vm_stat` before n ≥ 26, statement
   averaging at small n (FS grinding luck is deterministic per
   statement), phase trees via `OBLONG_PROFILE=1`.

## Traps (repo-specific)

- 97/97 tests green before starting; nothing may change existing proof
  bytes; new APIs only (the 0x42/0x43/0x44 tags are EXPERIMENTAL and
  may evolve — say so in commits); flock-core is a LOCAL PATH dep — do
  not touch it.
- CONCURRENT sessions share this checkout: commit with explicit
  pathspecs only, never `git add -A`; re-check `git status`
  immediately before committing; treat unexplained working-tree
  changes as another session's live edits (never fix, revert, or
  commit them). The shared-point/family line and the LaTeX note are
  co-owned — additive edits only, and rebuild `main.pdf` when touching
  the note.
- Keep inner-claim counts at powers of two until P2 lands (5 inner
  claims pad to 8 tree-sets — measured 2× worse than 4).
- The eqf driver requires one exit point per cascade level
  (`shared_q`); `lch = 1` is asserted and holds at q = 2^100 − 15 for
  every feasible shape.
