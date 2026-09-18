# Session prompt — prototype: mod-q RLC batching of F₂-linear claim families in BitZ

You are in `/Users/albertgarretafontelles/f2z-pcs` (the `f2z-pcs` skill
applies: build/test/bench conventions, measurement protocol, commit style —
unsigned commits). Companion design/exposition prompt:
`docs/rlc-family-note-prompt.md` — read it FIRST; it contains the full
construction, soundness obligations, and boundaries. This prompt is the
implementation plan. The construction is unvetted: if a soundness or
completeness gap surfaces while implementing, stop and record it (README +
a note in `docs/`) before writing more code.

## What to build

New EXPERIMENTAL prover/verifier pair (do not touch or replace existing
APIs; the virtual-XOR path stays intact as the comparison baseline):

```
prove_mle_eval_mod_q_ligerito_rlc_family / verify_..._rlc_family
```

Statement: k claims `MLE[INT(a_i)](r_i) = c_i ∈ F_q`, where each `a_i` is a
public F₂-linear form `L_i` of j committed bit-columns (per position),
all claims sharing the COLUMN point (r_cols equal; row points may differ).
Primary target: k=3, j=2, `a_3 = a_1 ⊕ a_2`, W=1. Design the weight/leaf
machinery for general j ≤ 4 (16-case) from the start — the generalization
is the point — and add a j=3 family test (e.g. k=4 with a₄ = a₁⊕a₂⊕a₃).

## Protocol recap (details in the note prompt)

1. FS ordering: absorb commitment root, all (r_i, c_i); THEN draw
   γ_1..γ_k ∈ K-independent F_q challenges (transcript tag; γ_1 := 1 is
   fine). Combined target `T = Σ γ_i c_i mod q`.
2. Case weights per row-position b, case m ∈ {0,1}^j:
   `W_b(m) = (Σ_i γ_i·w_{i,b}·L_i(m)) mod q`, `w_{i,b} = eq(b, r_{i,rows})
   mod q`. Chunk into `c_w = 127−t−W` limbs (`mod_q_chunk_width` semantics
   unchanged; per-chunk fold bound `< 2^{c_w+t+W}` — the existing
   `ChunkRange` check carries over verbatim).
3. ONE forest per chunk over 2^n positions, leaf = α^{W_b^{(l)}(m(pos))}
   (2^j-case select on committed bits; derived vectors never materialize).
4. Presum with 2^j − 1 channels `R_S = eq ⊙ τ_S` against monomials
   `Π_{i∈S} M_i` (τ_S = char-2 subset zeta-transform of the case α-powers)
   — `MultiDegreeSumcheck` with one group per channel; the |S|≥2 monomial
   ξ-tables are built like `xi_combined_rows` over derived AND/monomial
   rows.
5. Discharge: one η-batched degree-(j+1) eq-sumcheck over n vars for the
   |S|≥2 residuals, exiting at committed openings M_i(ρ).
6. All residual openings (presum's |S|=1 claims at (r*,ξ) + discharge's at
   ρ) go through the EXISTING multi-ring machinery: per-claim s_v under one
   shared r″, η-RLC via `fill_phi_basis`-family, ONE
   `recursive_prover_with_basis[_precomputed_round0]` call.
7. Verifier: recombine `Σ_c eq_c Σ_l 2^{c_w l} u′^{(l)}_c mod q ?= T`;
   evaluate each `R̂_S(r*)` via O(2^t·W) exponent tables (generalize
   `row_bit_weights`/`q_rowbit`); range-check every chunk fold.

## Code map / staging

**Phase 1 — correctness (eager forest, small n).**
- `src/pcs.rs`: case-weight builder (`rlc_case_weights`: points, γ's, forms
  → per-position `[u128; 2^j]` reduced case values + chunking) and the
  per-case α-power tables (generalize `chunk_pow2_table`).
- Forest: materialize leaves eagerly per column (case-select from the
  committed bit rows — an AND/XOR-row derivation like
  `extract_virtual_xor_rows` but selecting table entries) and drive the
  EAGER forest (`prove_merged_forest` non-lazy path). Memory-fine for
  n ≤ 22-ish tests.
- Presum: `MultiDegreeSumcheckGroup` per channel (j=2: three groups —
  (R_A,M₁ξ), (R_B,M₂ξ), (R_AB,ANDξ), each degree-2). Derive the monomial
  ξ-tables from the bit rows directly.
- Discharge: degree-(j+1) sumcheck over n vars. Phase 1 may use the
  generic prover with lifted Gf tables at small n; do NOT prematurely
  optimize.
- Verifier + tests: roundtrip (same point and different-row-points
  variants); tamper each c_i, a sent u′ chunk, a presum message, a
  discharge message; the j=3/k=4 family; cross-check against the vx path
  on an overlapping statement. NO proof_codec integration (mark the API
  experimental in the docs and README).
- CRITICAL soundness details: γ ordering; absorb the case-weight-defining
  data (forms, points) or derive them from already-absorbed material;
  range checks per chunk; the discharge's η's drawn after the presum
  residuals are absorbed.

**Phase 2 — bench-grade lazy leaves (only after Phase 1 is green).**
- New lazy leaf mode: 2^j-case leaf select is structurally the existing
  table family (`LeafTables`/`Pair2`/`T4At` shapes — see
  `src/piop/sumcheck/eq_factored.rs` and `merged_forest.rs`). A
  `T4At`-style per-position accessor over interleaved bit streams +
  per-case τ tables gets the bottom rounds; dense upper layers unchanged.
  Respect the size-gated table/prefetch lessons already in the tree
  (`BITZ_LEAF_A2_FACTORED`, `BITZ_LUT_PRFM`, `BITZ_T4_PRFM` — 2026-07-26
  README notes): at big shapes, bytes and data-dependent gathers rule.
- Bit-LUT early rounds for the discharge if it shows up in the profile.

**Phase 3 — measure.**
- A/B at n=22–28 (lazy path; eager only for tiny shapes) against BOTH
  baselines: (a) three independent mod-q claims, (b) the virtual-XOR path
  where the statement overlaps. Repo protocol: `RUSTFLAGS="-C
  target-cpu=native"`, `--features unchecked` benches, alternated
  in-window pairs, medians, `OBLONG_PROFILE=1` phase trees, check
  `vm_stat` before n ≥ 28 (churned-box pitfall — see README). Predicted:
  ~1.6–1.8× single-claim cost for the triple vs ~3× — REPORT HONESTLY if
  the prediction fails; negative results go in the README notes like every
  other experiment this month.
- Record results in README (dated note, the established format) and update
  `docs/DESIGN.md` with a short section pointing at the note prompt.

## Conventions and traps (repo-specific)

- Tests: `RUSTFLAGS="-C target-cpu=native" cargo test --release` (75 tests
  green before you start; keep them green — nothing you add may change
  existing proof bytes; new APIs only).
- flock-core is a LOCAL PATH dependency — do not touch the flock checkout,
  do not pull its origin (upstream removed `pcs::basefold`, which
  `ligerito_flock.rs` still imports).
- Navigate by function name, not line number. `CARGO_TARGET_DIR` is
  globally `~/zinc-plus/target` on this box.
- Commits unsigned (`git commit --no-gpg-sign`); README optimization-notes
  style for measurements; the `sha_lig_configs` embedded/ad-hoc boundary
  (m ≥ 22 audited) applies to any new bench shapes.
- The K-side/bit-MLE machinery is char-2-linear — the ring-switch side of
  derived claims is free; only the integer side needed this construction.
  If you find yourself lifting integers into GF(2^128), stop — they
  collapse to parity (the founding obstruction).
