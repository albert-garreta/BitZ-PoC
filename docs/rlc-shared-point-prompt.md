# Session prompt — shared-point RLC families: exploit the rank-1 case weight

You are in `/Users/albertgarretafontelles/f2z-pcs` (the `f2z-pcs` skill
applies: build/test/bench conventions, measurement protocol, commit style —
unsigned commits). Read FIRST, in this order:

1. `docs/rlc-family-note/main.tex` — the vetted construction note, now
   carrying the measured results (§ Measured), the vanishing-channel
   elision (Prop. `prop:vanish`, Rem. `rem:elision`), and the leaf-bit
   discharge realisation (Rem. `rem:dischargeleaf`).
2. The README's dated RLC notes (2026-07-26/27) — the A/B tables, the
   phase decompositions, and the measurement pitfalls.
3. The landed code: `prove/verify_mle_eval_mod_q_ligerito_rlc_family`
   (`src/ligerito_flock.rs`, commits `8fbd155..cf2bebe`), the case-weight
   machinery in `src/pcs.rs` (`rlc_case_weights`, `rlc_tau_tables`,
   `rlc_active_channels`-side logic), the harness `examples/rlc_ab.rs`,
   and the CLI mode `f2z <n> --family j2|j3|j4`.

## The setting

The deployed family API requires only a shared COLUMN point; row points
may differ per claim. The motivating deployment has **all points equal**:
k claims `MLE[INT(a_i)](r) = c_i` at ONE point `r = (r_row, r_col)`, each
`a_i` a nonzero XOR-combination (form) of the j committed columns.

The structural collapse to exploit: with `w_{i,b} = w_b` for all i,

```
W_b(m) = (Σ_i γ_i · w_b · L_i(m)) mod q = (w_b · Γ(m)) mod q,
Γ(m)  := (Σ_i γ_i · L_i(m)) mod q            — 2^j values TOTAL,
```

i.e. the `[2^{t'}][2^j]` case-weight table is **rank-1**: one shared row
vector `w` times a row-independent case function `Γ`. Two immediate
consequences to build on:

- **The family is capped and canonical.** At one point, two claims with
  the same form are the SAME claim: dedupe, so k ≤ 2^j − 1, and the
  maximal family is "the whole XOR-closure of j columns at one point"
  (j=2: exactly the XOR triple; j=3: 7 claims; j=4: 15). This is a
  natural API: one point, j columns, up to 2^j − 1 claimed values.
- **The statement shrinks.** The Fiat–Shamir statement absorb currently
  hashes k row-weight vectors (k·2^{t'}·16 B). Shared-point: absorb ONE
  vector (+ the forms and values). Same for the verifier's inputs.

## What to investigate / build

1. **API + absorb**: either auto-detect equal weight vectors in the
   existing entry points (cheap compare; keeps one API) or add a
   `..._rlc_family_shared_point` wrapper taking one `row_weights_q` and a
   form/value list. Enforce/dedupe duplicate forms (equal-c check).
   Absorb the collapsed statement. Keep the general path byte-stable —
   decide explicitly whether shared-point proofs are a DIFFERENT
   transcript (fine — document) or bit-compatible.
2. **Case tables**: both sides currently build `2^j·2^{t'}` case values
   and comb-exponentiations PER CHUNK (`rlc_case_weights`,
   `rlc_case_pow_table`). With rank-1 structure: the case-weight build is
   one `fq_mul` per (row, case) from `(w_b, Γ_m)` — measure whether any
   further sharing exists for the EXPONENTIATIONS. Warning recorded so
   you don't re-derive it: `α^{(w_b·Γ_m) mod q} ≠ (α^{Γ_m})^{w_b}` — the
   mod-q reduction breaks the exponent product, so per-(row, case) pows
   likely remain; the honest question is table-build share OF PROVE
   (profile scopes `rlc:chunk`/`mc:pow2`-analogues first — at the
   measured shapes the case-pow build was small, so DO NOT promise a big
   prover win; the wins live in absorb, dedupe, API, verifier).
3. **Verifier**: the O(2^j·2^{t'}) per-chunk exponentiation step is the
   verifier's dominant non-succinct cost, ×(active channels). Quantify
   what rank-1 sharing buys there (τ tables from one w-comb pass?), and
   whether inactive-channel elision (already deployed) plus the
   shared-point Γ structure prunes channels for natural families.
4. **The maximal-family sweet spot**: measure "open the full XOR-closure"
   (j=2 k=3, j=3 k=7, j=4 k=15) vs the same claims through the batched-vx
   baseline and independent proofs — extend `examples/rlc_ab.rs` with a
   shared-point mode (reuse its layout so numbers compare with the
   README tables). Note k=7/k=15 exercise EVERY channel — the presum and
   cascade at full width; per-claim cost should drop well below the
   measured k=4/k=5 points (more claims, same forest+discharge).
5. **Clusters at one global point** (stretch, time-permitting): the
   note's clustering open problem simplifies when ALL claims share one
   point — partition a wide linear layer into ≤4-column families, one
   forest each, shared tails. Sketch the planner; do not build it unless
   the core items land early.

## Conventions, traps, honesty

- Repo protocol for every number: `RUSTFLAGS="-C target-cpu=native"`,
  `--features unchecked`, alternated in-window medians, `OBLONG_PROFILE=1`
  phase trees, `vm_stat` before n ≥ 28 (16 GB box; churned-box pitfall).
- **Fiat–Shamir grinding luck is deterministic per statement** (README
  2026-07-26 note): at ~20–30 ms totals, single-statement comparisons
  carry ±3–5 ms per-variant offsets that medians cannot average — average
  over MULTIPLE STATEMENTS for small-n claims.
- Zero-channel elision is a COMPLETENESS requirement (note Prop.
  `prop:vanish`): shared-point families with degenerate Γ (e.g. only
  XOR-type forms) elide channels — keep the tests that pin this
  (`rlc_family_pure_xor_family_roundtrips`) green and add shared-point
  variants.
- All 83 tests green before and after; new APIs only, nothing may change
  existing proof bytes. Tests: shared-point roundtrip (incl. the maximal
  family), dedupe/equal-c rejection, tamper of the collapsed absorb, and
  the A/B. Record results in the README (dated note, established format)
  and touch `docs/DESIGN.md` + the note's § Measured if the numbers are
  worth carrying there. Negative results are results — record them like
  every other experiment this month.
