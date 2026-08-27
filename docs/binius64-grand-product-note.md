# Binius64's grand product today, vs the F2Z forest

Note, 2026-08-22. Upstream = binius-zk/binius64 `origin/main` @ `3f96163`
(2026-08-18); ours = the merged GKR forest (`src/merged_forest.rs` +
`src/piop/sumcheck/eq_factored.rs`, paper §core-IOP "grand products in the
exponent").

## Where their grand product lives

Prodcheck (`crates/ip/src/prodcheck.rs`) is used in exactly one protocol:
**intmul**, the 64-bit integer-multiplication reduction (invoked when a
circuit carries IMUL constraints; the M4 chip system inherits it). Nothing
else touches it — AND constraints go through the univariate-skip zerocheck,
GF(2^128) word muls through the new plain degree-2 **binmul** mlecheck, and
their one lookup argument (LogUp\*) is fractional-addition-based, not
product-based. So, like ours, their only grand products run **in the
exponent of GF(2^128)**.

## The intmul construction (current main)

Statement: `a·b = (c_hi, c_lo)` for 64-bit words, proven as
`(G^a)^b = G^{c_lo} · (G^{2^64})^{c_hi}` in GF(2^128)^*, with
`G = F::MULTIPLICATIVE_GENERATOR` (fixed, full order 2^128−1) and the
exponents range-bounded below the order, so the group identity is
equivalent to the integer one. Four product trees, data-parallel over rows:

- **a, c_lo, c_hi — constant-base, limb-decomposed.** Each exponent word
  splits into 4 limbs of 16 bits; limb column `l`'s leaf is row `limb_l(e)`
  of the power table of base `G^{2^{16l}}`; a **depth-2** prodcheck
  multiplies the 4 limb columns.
- **b — variable-base.** Base = the a-tree root `A = G^a` (per row); a
  **depth-6** prodcheck over 64 bit-affine leaves
  `1 + b_i·(φ^i(A) − 1)`, φ = Frobenius (`φ^i(A) = A^{2^i}`).

The prodcheck engine is a textbook recursive GKR: `k` rounds, each a
degree-2 eq-weighted mlecheck (explicitly [Gruen24] eq-partitioned, wide
delayed-reduction accumulators), then a 2-evaluation split plus line
extrapolation. It is **fully eager**: `ProdcheckProver::new` materializes
every layer up front (`layers[0]` = leaves, halving upward, ≈2× leaf
memory). No univariate skip (that lives only in their AND reduction), no
laziness inside the tree.

Leaf discharge is where the recent cleverness sits:

- **b leaves** are materialized for the product build but their claims are
  discharged *virtually*: Phase 2 inverse-Frobenius-twists the 64 leaf
  claims onto ONE composition family `b_i·(A−1) + 1` at 64 different points
  (bits are Frobenius-fixed, `φ(b)=b`), and Phase 3's
  `SelectorMlecheckProver` proves them batched with the bit columns kept as
  packed words — `BinarySwitchover` recomputes partially-folded chunks from
  bits (per-set-bit adds, no tables). Output: one claim on `A` plus bit
  claims routed to the shift reduction. Phase 3 also carries the LO·HI
  product sumcheck binding the three roots.
- **Constant-base leaves**: Phase 4 batches the three depth-2 prodchecks to
  per-limb-column claims; Phase 5 Frobenius-twists all 12 claims onto the
  single shared table `T: i ↦ G^i` (2^16 rows; `(G^{2^{16l}})^i =
  φ^{16l}(G^i)`) and proves the reads with a **committed LogUp\***
  (pushforward oracle), the residual table claim checked against T's
  succinct MLE `T̃(x) = Π_j (1 + x_j·(G^{2^j} − 1))`.

## Convergences with the forest

1. **Same leaf algebra.** Bit-affine `1 + b·(v−1)` everywhere; their power
   table's succinct MLE `Π(1 + x_j(G^{2^j}−1))` is literally our τ tensor
   (`τ = α^w + 1`).
2. **Same round-engine core.** Degree-2 Gruen-eq-factored messages with
   delayed-reduction wide accumulators (their `RoundEvals`/`WideMul` ↔ our
   Gruen rounds/`WideMulAcc`).
3. **Same padding trick.** Multiplicative-identity padding (their
   `OnePadMleCheckProver` ↔ our `α^0 = 1` pad rows and col-elision's
   constant-1 trees).
4. **Same laziness instinct at the bit boundary** — their switchover
   recomputation vs our case-LUT gathers (recompute vs tabulate; see the
   selector_mle discussion).

## Divergences

1. **Role.** Theirs is a gadget, present only when muls are; ours is the
   PCS opening itself, in every proof, 84–96 % of prove time — which
   explains the optimization asymmetry.
2. **Product dimension.** Theirs: 6 vars (64 leaves/row) + 2-var limb
   trees. Ours: d ≈ 17 (2^d rows/column) × 2^s columns. That is *why* they
   can afford eager layer materialization and we cannot (it would be 2^28+
   elements) — the lazy bottom, stash, and JIT are responses to depth.
3. **Table-sharing axis (dual choices).** Their bases repeat **across
   rows** → one shared 2^16-row power table plus a committed lookup
   argument (an extra oracle + opening). Our bases `α^{w_b}` vary per row
   but repeat **across columns** → tree-shared τ/case tables priced
   directly into the sumcheck rounds, zero extra commitments. Same
   amortization instinct, transposed.
4. **Variable-base + Frobenius machinery.** Their b tree
   ((G^a)^b with twist-collapsed leaf families) has no analogue here — our
   bases are public. This remains the importable lever the muls audit
   identified for word×word Hadamard openings.
5. **Tree-engine depth of optimization.** Their prodcheck is a clean
   recursion; no case-LUT rounds, no fold stashing, no JIT'd levels, no
   double-fold/no-pass rounds, no pass fusion, no 2-cofactor message
   compression. Consistent with (1): it is not their bottleneck.

## Takeaways for us

- Phase 5 is now a **production instance of the switched-tensor note's
  option (b)** (log-derivative read of a public exponent table, bases
  collapsed by Frobenius) — a measured precedent to weigh against that
  note's 1.5× decision bar.
- The Phase 2 trick — bits are Frobenius-fixed, so twisted leaf families
  collapse onto one selector composition at shifted points — is reusable
  wherever per-level twisted claims appear.
- Nothing in their tree engine itself is ahead of ours; the flow of ideas
  on the product machinery still runs their-way ← ours, except for
  variable-base twisting, which runs ours ← theirs.

## selector_mle.rs deep-read (2026-08-27, origin/main `6b11a7da`)

Asked point-blank whether `ip-prover/src/sumcheck/selector_mle.rs` has
anything for us. The file is Phase 3's multi-point mlecheck: k claims
`1 + selector_i·(selected − 1)`, each at its OWN point, over ONE shared
fold buffer — per-claim `ChunkedEqTracker` (Gruen prefix scalar ×
L1-resident chunk ⊗ suffix outer product), `BinarySwitchover` (bit columns
stay bitmasks for the first rounds; partial folds recomputed per chunk as
per-set-bit adds of the challenge tensor), per-claim degree-2 round polys
combined by public weights into one wire message. Drift since the 08-22
audit (`3f96163`) is API renames only.

Mechanism map (theirs → ours):

- **Gruen split-eq + prefix scalar** → the eq-factored driver's A/V
  factorization. Ours amortizes ONE shared suffix tensor across all 2^s
  groups (shared point), so their chunk ⊗ suffix L1 trick has nothing left
  to save here — it matters only in the multi-point regime.
- **Wide delayed reduction, 2-eval rounds** → `WideMulAcc`; same idea.
- **`BinarySwitchover`** → transfers only where folds are LINEAR in bits
  with a position-independent tensor. Our leaf rounds are per-position-τ
  product layers (the case-LUT program is the correct — and measured —
  tabulation); where we DO have linear bit folds (`s_v` fold, taps
  extraction) we already run 4-Russians / clear-row gathers, strictly
  stronger than per-set-bit adds. No transfer.
- **Multi-point per-claim trackers over one shared fold buffer** → the
  missing driver the muls audit flagged for variable-base word×word
  (Hadamard) openings; this file is the reference design (per-claim
  running sums + prime coeffs, weights-combined wire, per-claim eq
  trackers). Build-on-demand only — no mul workload, no build.

**The one live import — the two-accumulator round body.** Their inner
loop accumulates ONLY `y_1` and `y_inf` (`RoundEvals<_, 2>`); the third
node is pinned by the per-claim running sum inside `interpolate2` (one
scalar division by `(1 − α)` per round). We adopted Gruen on the WIRE
(2 cofactor coefficients; verifier identity `Ĥ0 = S + q·(Ĥ1+Ĥ2)`) but our
prover still computes full `(a0, a1, a2)` triples in every body:

- dense single-pair: 5 muls/b — `P00 = (wl0)·r0` is droppable since
  `H(1) = P00 + A1 + PΔΔ = P11` exactly; keep `(ΣP11, ΣPΔΔ) ≡ (y_1, y_inf)`
  → 4 muls/b (−20 % body ALU);
- leaf_r1 `Split`: the `t_a0` gather stream (1 of 3 message streams,
  DRAM-scale tables) drops entirely; `Raw8`: ~3 of ~11 slot ops;
- pair2/leaf2 message tables: −¼ build + 1 of 3 gathers;
- double-fold grid: 9 → 8 products/quad (corner recovered from the claim,
  one division by the corner eq-weight).

Recovery algebra (char 2, shared point): `Ĥ2 = Σ_t A_t·a2_t`,
`Ĥ(1) = Σ_t A_t·t11_t`, `Ĥ1 = (Ĥ(1) + S_j)/(1 + q[j−1]) + Ĥ2` — ONE
scalar inversion per round (`q ≠ 1` w.h.p.; fallback = the old 3-acc
path). Wire bytes unchanged ⇒ all byte-identity pins survive. Constraint:
the driver's generic field bounds have no inverse — needs an opt-in field
hook (the `eqf_grid_pass` pattern). The generic 3-tail mode (RLC-discharge,
mixed points) gets the same node-drop only with per-group running-sum
bookkeeping (binius's `last_coeffs_or_sums` per claim) — secondary.
Expectation, calibrated against the fixed-scalar precedent (−14 % muls in
one scope → −1.7 % ST e2e): a 1–4 % e2e prove probe at n ≥ 26, more ST
than MT (dense rounds are BW-bound; the LUT-round gather-stream drop is
the real-traffic part). Unmeasured — needs the interleaved A/B
discipline.
