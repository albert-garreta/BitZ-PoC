# Session prompt: u-suppression — commit the chunk folds, drop the clear-text `us` from the proof

**Goal.** Replace the clear-text `us` payload of the mod-q opening (`L·2^s`
u128s — 32 KiB at s=11, i.e. ~63 % of the n=28 forest-side bytes) with a
committed bit-decomposition bound by a static-exponentiation forest, making
the range check free and shrinking the proof. Prover-time budget: ≤ ~1–2 %.
Verify budget: ~wash (drops the `2^s·L` α^u exponentiations, adds two small
verifications). This is the "commit the folds + range-prove" item from the
zinc-plus ledger's Still-open list, instantiated with the Binius static-exp
gadget (https://www.binius.xyz/blueprint/backend/muls/exponentiating — their
static-base case IS our forest; the import here is only the arrangement).

**Provenance**: designed 2026-07-20 (session: W=32 forest optimization).
Cross-refs: zinc-plus `f2-int-sha` worktree ledger (`f2x-sha-todo.md`), the
"us/u′ suppression" bullet under the logUp\* adjudication's reopen list.

## Load first

1. Skill `f2z-pcs` (repo conventions, build flags, measurement protocol).
2. `src/ligerito_flock.rs` — the mod-q section: `prove_mle_eval_mod_q_ligerito`
   / `verify_mle_eval_mod_q_ligerito` (chunk loop, `us` flow, range check,
   η-RLC of the L claims into one Ligerito call, `recombine_read_off` use).
3. `src/ligerito.rs` — `prove_int_eval_merged_common` /
   `verify_int_eval_merged_common` (the generic weighted-fold prove path:
   arbitrary `row_weights: &[u128]`, forest + presum + residual point).
4. `src/merged_forest.rs` — `drive_grouped` (root absorb `0x30`, ζ
   derivation, per-layer chain) and `prove_merged_forest_lazy_multi`
   (same-shape multi-claim batching, `tau_set` indexing).
5. `src/pcs.rs` — `mod_q_chunk_width`, `recombine_read_off`, the §6 comment
   block; `src/proof_codec.rs` (canonical, tamper-rejecting codec pattern).
6. `docs/DESIGN.md` — the soundness chain you will be extending.

## What the verifier currently gets from the clear `us`, and the replacements

| fact | today | replacement |
|---|---|---|
| roots: `root_{c,l} = α^{u_c^{(l)}}` | verifier exponentiates received u's | static-exp forest over committed u-bits (public bases `α^{2^k}` = Frobenius chain of α); root linkage via a shared claim, roots never sent |
| range: `u < 2^{c_w+t+W} = 2^127` | explicit check on received u128s | free: exactly 127 committed bit-planes per fold can only encode `u < 2^127` |
| read-off: `y = Σ_c w′_c Σ_l 2^{c_w·l} u_c^{(l)} mod q` | verifier computes from received u's | a self-hosted mini mod-q instance over the same committed u-bits; terminates in ONE clear u128 |

## The design (variant B — recommended; a variant A sketch at the end)

**The reuse insight that makes this a small job**: every new component is a
call into the existing machinery with different weights. No new sumcheck
kernels, no new leaf types.

1. **u-bit commitment.** After computing the folds, bit-decompose all
   `L·2^s` of them into a single-column-oriented bit matrix: logical shape
   `(t_u = s + log₂L + 7, s_u = 1, W = 1)` — in-tree index low 7 bits = the
   bit position k ∈ [0,127] (leaf 127 padded public-1), high bits = (c, l);
   column 1 is an all-zero pad. Commit with `commit_rs_ligerito_rows`
   (~`2^{s+7}·L` bits ≈ 32 KiB of data at s=11 → `m_p = s + log₂L`,
   i.e. m ≈ 18: **ad-hoc config territory, UNAUDITED — see the M0 gate**).
   Absorb the commitment into the transcript BEFORE any forest challenge.
2. **Exp-binding claim** = `prove_int_eval_merged_common` on the u-bit
   commitment with row weights `[2^0, 2^1, …, 2^126, 0]` tiled per
   (c,l)-block (weights up to `2^126` are fine — the soundness bound is on
   the fold magnitude `< 2^127`, not per-weight width; note
   `chunk_pow2_table` will comb-pow all `2^{t_u}` rows — either accept
   ~10 ms or special-case the tiled table). Its per-tree level-`(t_u−7)`
   internal nodes ARE the `α^{u_{c,l}}` values.
3. **Read-off claim** = the mod-q path on the SAME commitment, same shape,
   row weights `rw(c,l,k) = (2^k mod q)` combined with **column-collapsed**
   outer weights: fold `w′_c·2^{c_w·l} mod q` into the row weights so the
   whole recombination is one single-column fold. Check the window:
   `c_w'' = 126 − t_u ≥ 100` holds for `s + log₂L ≤ 19` — every reachable
   shape. This claim sends ONE clear u128 (plus the pad column's), is
   range-checked classically, and its verified value equals y. Recursion
   terminates here.
4. **Batch 2+3** as two same-shape claims via the multi-claim machinery
   (`prove_merged_forest_lazy_multi` / the batched-common path — different
   weight sets, shared layers, shared exit point) → one residual u-bit MLE
   claim → ONE small ring-switch + Ligerito opening of the u-bit
   commitment (η-RLC if any extra points remain).
5. **Root linkage (the c₀ trick).** The main forests no longer derive roots
   from received u's. New `drive_grouped` variant taking `(ζ, c₀)`
   externally instead of absorbing roots and deriving ζ: for each chunk l,
   the exp-claim's chain, after binding down to level `t_u − 7`, holds a
   claim `L̂(z_x, z_c)` about the `α^{u_{c,l}}` node-MLE (pad-column
   algebra: `L̂ = (1−z_c)·R̂(z_x) + z_c·1`, so the verifier extracts
   `R̂(z_x)`). Feed `ζ := z_x` (the l-slice) and `c₀ := R̂(z_x)`-derived
   value into the main forest's entry. Main-forest GKR then pins its roots'
   MLE at ζ to c₀ ⟹ main roots = the exp-tree's bound `α^u` values except
   w.p. ≤ (s+log L)/2^128. The exp-tree's OWN root (levels above `t_u−7`
   multiply the α^u together — exponent sums mod ord, no integer meaning)
   is sent as one K-element purely to start the chain; it carries no
   binding role — say so in DESIGN.md.
6. **FS order**: data commit → (r, weights public) → compute folds →
   commit u-bits → absorb → exp+read-off batched claims → per-chunk main
   forests entered at (ζ, c₀) → presums → η-RLC → openings (main Ligerito
   as today; u-bit opening separate). Nothing challenge-derived precedes
   its absorb.

**Soundness delta for DESIGN.md**: (i) 127 planes ⟹ range implicit AND
wrap-proof: the representable exponent is ≤ 2^127−1 < ord(α) = 2^128−1,
so the group identity pins the INTEGER (mod-ord equality collapses to
integer equality on the representable range). The plane count is
soundness-critical — a 128th plane would admit the α^{2^128−1} = 1 = α^0
collision; pin it in the tamper tests. Booleanity of the u-bits comes
from the F₂ commitment — virtual/uncommitted u-bits are UNSOUND (no
booleanity, and nothing binds the leaf wires); (ii) exp-forest
bit-affine leaves over committed u-bits with public Frobenius bases ⟹
level-(t_u−7) nodes = α^{committed u}; nodes ABOVE that layer sum
exponents past ord(α) — fine only because no integer claim attaches
there (binding is extracted at the per-fold layer); (iii) shared
(ζ, c₀) ⟹ main roots equal those nodes whp; (iv) injectivity + read-off
as before, with y now certified by the mini instance whose own clear
payload is one range-checked u128.

## M0 — the size-budget gate (DO THIS FIRST, no implementation before it)

**Framing identity**: the u-bit data (`L·2^s·127` bits) is byte-for-byte
≈ the `us` payload it replaces (`L·2^s·128` bits). Suppression saves
NOTHING at the data level — the entire net win must come from the
opening being cheaper than the data, i.e. from query amortization.
Expectation to confirm, not discover: a STANDALONE opening of the small
commitment is net-NEGATIVE (proximity soundness at the 100-bit target
needs ~100–180 queries regardless of instance size, at ~0.4–1 KiB per
query of column + Merkle path = 40–150 KiB to open a 32 KiB vector).

- Control measurement: bench a standalone `(t_u, 1, 1)`-equivalent shape
  (ad-hoc config; also check whether `custom_johnson_config` /
  flock's `validate()` accepts m < 22) and read the `split:` line. This
  number is the baseline the batched path must beat, not a viable design.
- **The primary viable path is flock-side LADDER INJECTION** (mixed-size
  batch-FRI adapted to recursive Ligerito): the recursion is a ladder of
  shrinking instances with a fresh prover commitment per level, so the
  u-instance joins at the level whose running size matches — commit U
  early (pre-challenge, encoded at the injection level's RATE), draw η
  there, RLC the u-claims into the running target and the u-vector into
  the running message, co-query BOTH trees at shared indices below. Deep
  levels run richer redundancy at ~15–30 queries → marginal cost ≈
  co-query paths (~10–20 KiB) + mini-GKR bytes (~8–10 KiB). M0 must
  estimate this from the shipped ladder configs (per-level query counts
  are readable from the config TOMLs / proof breakdown). Claim-side
  batching (the two u-claims, same commitment) uses the EXISTING η-RLC
  machinery — no new tech.
- Budget table (fill with measurements, s=11/L=1 and s=9/L=2 at least):
  saved = `16·2^s·L` B; added = exp+read-off merged-forest bytes (shared
  layers — estimate from proof_codec on the real object) + u-commitment
  root + exp-chain root + u'' + the opening delta (standalone measured /
  injection estimated).
- **Gate: net proof reduction ≥ 10 KiB at the (17:11:1)/fast reference
  shape, and non-negative at (12:9:32), on the better of the two opening
  paths.** Ladder injection is flock-side surgery (contained — the
  ladder already absorbs a fresh commitment per level — but it is a USER
  decision to open flock; obtain it before implementing that half).
  Note: at s ≤ 8 the gadget saves ≤ 4 KiB — out of scope by
  construction; do not bench-chase it. The gadget's real payoff is as
  the committed-fold companion of the switched-tensor design
  (`docs/switched-tensor-note.tex` §3.3–§4.4: `W·2^s` folds, ~1 MiB of
  payload at W=32, s=11 — the same one-time injection amortizes a 32×
  larger saving); keep the implementation generic over the fold count.

## M1 — implementation (only after M0 passes)

- Additive API: `prove/verify_mle_eval_mod_q_ligerito_usup` + a new proof
  struct + codec arm (canonical, tamper-rejecting, bincode-blob pattern for
  the extra lig proof). The DEFAULT path stays byte-identical — pin it.
- The `drive_grouped` external-(ζ, c₀) variant + verify counterpart (keep
  the old entry; new one is a thin wrapper — do not fork the layer loop).
- Weight builders for the tiled `2^k` and collapsed read-off weights
  (mod-q reduction of `w′_c·2^{c_w l}·2^k` — build with `fq_mul`, no
  crypto-bigint).
- Bench knob `BITZ_USUP=1` selecting the variant; header prints it; the
  `split:` line must itemize the new populations.

## M2 — validation + recording

- Roundtrips: W=1 and W=32 shapes, L∈{1,2}, both profiles; every rep
  verified.
- Tamper matrix (each must reject): flip one committed u-bit plane; forge
  c₀; tamper the exp-chain root; tamper u''; a fold with bit 127 set has no
  encoding — assert the prover-side decomposition rejects `u ≥ 2^127`;
  codec truncation/extension.
- Byte-pin: default-path proofs unchanged vs pre-change dumps
  (cross-process, `fuse_check`-style).
- Bench the reference shapes (idle box, one shape per process, medians);
  update README proof-size tables ONLY for the flagged variant (add a
  column/section — do not touch default rows), extend DESIGN.md §soundness,
  and append an BitZ pointer entry (outcome + numbers, pass or fail) to the
  zinc-plus `f2-int-sha` ledger.

## Constraints and traps (house rules — do not relearn these)

- `RUSTFLAGS="-C target-cpu=native"`, release + LTO + `--features unchecked`
  for any quoted number; `cargo test --release` both guard modes green.
- Unsigned commits (`--no-gpg-sign`); do NOT push to GitHub.
- flock-core is a LOCAL PATH dep; `sha_lig_configs` embedded profiles are
  audited only at m ≥ 22 — the u-bit opening lives BELOW that boundary;
  carry the UNAUDITED flag in docs and code comments.
- Navigate by function name, not line number.
- Leave LICENSE / Cargo.toml placeholders alone.
- This design doc is a map, not gospel: re-derive the window checks
  (`c_w'' = 126 − t_u`, pad algebra, FS order) before trusting them, and if
  a component measures off-model, write the correction into the ledger
  entry rather than forcing the plan.

## Variant A (fallback sketch, only if B's mid-chain entry fights the code)

Per chunk l: a SEPARATE depth-7 exp-forest at shape `(7, s, 1)` mirroring
the main forest's tree layout; commit u-bits in the (c,l)-column
orientation; shared ζ_l for both forests, both claim the same c₀_l
(equality of the two root-MLEs at random ζ). Cost: the read-off claim then
needs the OTHER orientation — a second commitment or a transposed repack —
which is why B is preferred.
