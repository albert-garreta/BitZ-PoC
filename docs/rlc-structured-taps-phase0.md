# RLC families over ROT/SHIFT/entry-offset taps — Phase-0 analysis

Session record, 2026-07-27. Companion to
`docs/rlc-structured-taps-prompt.md` (the session prompt) and
`docs/rlc-family-note/main.tex` (the vetted family construction). Outcome:
**no soundness or completeness gap** — the stream reduction is exact and
the whole landed construction applies verbatim over streams — but one of
the prompt's two opening hopes is **refuted** (ROT is *not* a coordinate
permutation of the hypercube) and is replaced by a stronger uniform
result: every tap op yields a *translated-eq* opening basis, which is a
**bond-dimension-2 matrix-product function** (binary-addition carry
chain) over the committed coordinates, and the deployed ring-switch +
Ligerito machinery evaluates such bases succinctly at ≤ 2× the plain-eq
cost. Everything below is verifier-exact algebra — no new soundness
terms.

## 0. Conventions (corrected 2026-07-27; direction still to confirm)

**Corrected semantics (user clarification, same day).** The committed
columns are plain `W = 1` bit-vectors of dimension `N = 2^{num_vars}`
(a multiple of 32). The flat entry index is grouped into
`2^g`-bit words *along the entry axis* — `p = (k ≪ g) | j`, word `k`,
within-word position `j` (`g = 5`, 32-bit words, for the instance) —
and the ops act on those groups. Equivalently `b = M·a` for a banded
block-rotation `F₂` matrix `M`; the tap descriptor is the
succinctness-preserving normal form of that `M` (an arbitrary `M`
costs the verifier `O(N)` at the residual closure; index translations
on contiguous bit-fields are exactly what stays succinct). An earlier
draft of this analysis read the ops as acting on a separate word-bit
MLE axis (`bit_vars = 5`); the theory below is field-agnostic and
survived the retarget verbatim — only the extraction and the
coordinate mapping of §3.3 changed (and got simpler).

Words are 0-indexed here (`k ∈ [0, N/2^g)`, the prompt's word index
shifted by one). Uniform "gather" convention: **output index gathers
input index − amount**.

- **Word offset `off^o`**: output word `k` reads input word `k − o`,
  zero for `k < o` (the prompt's boundary spec: taps `k−1, k−2` vanish
  at the first words). ✓ matches the given spec exactly.
- **`ROT^c`**: output position `j` (within its word) reads input
  position `(j − c) mod 2^g` (rotate toward higher positions; cyclic).
- **`SHIFT^c`**: output position `j` reads input `j − c`, zero for
  `j < c` (input positions `≥ 2^g − c` drop out).

**TO CONFIRM with the user** (carried from the prompt, plus one):
(1) `a_{3,k−2}` read as `a_1[k−2]`; (2) the bare `ROT` in `b_3`'s second
tap read as `ROT^2`; (3) the ROT/SHIFT *direction* above (toward higher
within-word positions). SHA-2's σ-functions use the opposite direction
(`ROTR`/`SHR`); the machinery is direction-generic (`c ↔ 2^g − c`,
dropout end flips), so a flipped convention changes constants only.

## 1. Streams and dedupe (the instance)

`j = 2` committed bit-columns, 32-bit words along the entry axis
(`g = 5`), `k = 6` claims. Distinct (column, rot-or-shift, word-offset)
taps, after dedupe:

| stream | op | appears in |
|---|---|---|
| `A0` | `a_1` (identity) | b_1 |
| `B0` | `a_2` (identity) | b_2 |
| `S1` | `ROT^1 a_1` | **b_3 and b_5** (the one shared tap) |
| `S2` | `ROT^2·off^1 a_1` | b_3 |
| `S3` | `ROT^3·off^2 a_1` | b_3 |
| `S4` | `ROT^2 a_2` | b_4 |
| `S5` | `ROT^5·off^1 a_2` | b_4 |
| `S6` | `ROT^7·off^2 a_2` | b_4 |
| `S7` | `ROT^4 a_2` | b_5 |
| `S8` | `ROT^6·off^1 a_1` | b_5 |
| `S9` | `SHIFT^3 a_1` | b_6 |
| `S10` | `SHIFT^5·off^1 a_2` | b_6 |
| `S11` | `ROT^2·off^2 a_2` | b_6 |

`j_eff = 13`. Claim forms over the streams: `b_1 = A0`, `b_2 = B0`,
`b_3 = S1⊕S2⊕S3`, `b_4 = S4⊕S5⊕S6`, `b_5 = S1⊕S7⊕S8`,
`b_6 = S9⊕S10⊕S11` — every claim a **per-position XOR of streams**, so
the family construction (case-collapse, presum channels, elision,
cascade) applies verbatim with the streams as the family's base columns.

**Exactness of the reduction.** `b_i[p,c] = ⊕_{u∈supp_i} s_u[p,c]` holds
per position by definition of the streams (each tap contributes its
operand at the output position; out-of-range inputs are zero on both
sides of the definition). The family read-off identity
(`Theorem thm:complete` of the note) needs only this per-position XOR
plus the shared column point — both hold — so completeness and the
`1/q + …` soundness chain inherit unchanged *given* that the residual
stream openings reduce to committed openings (§3). Chunking, magnitudes,
`ChunkRange` (`Lemma lem:familymag`) are unchanged: exactly one case
weight contributes per position regardless of what the case bits mean.

## 2. Active channels: count first, cluster mandatory

The case function per position factors over claims:
`α^{W_b(m)} = ∏_i g_{i,b}(L_i(m))` with `L_i(m) = ⊕_{u∈supp_i} m_u`. In
characteristic 2 the multilinear (zeta) support of a single factor
`g(m_u ⊕ m_v ⊕ …)` is `{∅}` ∪ **singletons of its support only** (for
`|S| ≥ 2`, `τ_S = 2^{|S|−1}(g_0+g_1) = 0`; the `prop:vanish` quotient
mechanism). Multiplying factors and re-reducing on the cube, the active
channels are contained in

  { T = union of ≤ 1 singleton per claim },  T ≠ ∅.

- **Monolithic family (all 6 claims, 13 streams)**: distinct nonempty
  unions = `14·2·2·4·4 − 1 = 895` (the `4·4 → 14` factor is the
  `S1`-sharing collapse between b_3 and b_5). 895 presum channels is far
  past any table/presum budget ⇒ **a single 2^13-case family is out;
  clustering is mandatory** (as the prompt anticipated).
- **Cluster `{b_1, b_3, b_5}`** (streams `A0,S1,S2,S3,S7,S8`,
  `j_grp = 6`): `2·14 − 1 = 27` channels — 6 singletons, 13 pairs, 8
  triples (21 monomial channels, max `|S| = 3`).
- **Cluster `{b_2, b_4, b_6}`** (streams `B0,S4,S5,S6,S9,S10,S11`,
  `j_grp = 7`): `2·4·4 − 1 = 31` channels — 7 singletons, 15 pairs, 9
  triples (24 monomials, max `|S| = 3`).

Max `|S| = 3` in both clusters ⇒ the landed 2-level cascade
(`rlc_channel_sides`: `|S|=3 → (AND-of-two, single)`) covers the
discharge with **zero structural changes** — only the `j ≤ 4` asserts
relax and the AND-of-streams rows feed it. Generic γ's make all listed
channels active with overwhelming probability; both sides still derive
the active set from the public τ tables per chunk (elision inherits, and
covers any accidental vanishing).

Honest cost flag (pre-measurement): this instance's sharing is *thin*
(13 streams / 6 claims, one shared tap). The family's forest dedup is
large (the k=6 batched-vx baseline pads to **8** tree-sets; the family
runs **2** wider-leaf forests), but the presum channel count (58 vs 6)
and the 45-monomial cascades claw back an unknown amount. Phase 3
measures it; a thin win or a wash is a possible honest outcome.

## 3. Stream openings: the research crux, resolved

All presum/discharge exits are claims `ŝ(ζ) = μ` about stream MLEs at
K-points over the x layout (`n' = num_vars + bit_vars` coords, order
`[row_hi (tw), untapped word-bit coords (bit_vars), row_lo (s)]`; the
corrected instance runs `bit_vars = 0`). They must reduce to openings of
the *committed* matrix.

### 3.1 ROT is NOT a coordinate permutation (boundary recorded)

The prompt's first hope — "`ROT^c` permutes the bit-position variables:
the embedded point permutes coordinates" — is **false** for general `c`
(the group field's 5 index bits under the corrected semantics, equally
a 5-coordinate MLE axis under the original reading — the algebra is the
same). Counterexample (`2^g = 4`, `c = 1`): the rotated-eq vector
`v[j'] = eq(r, (j'+1) mod 4)` has `v_0·v_3 = eq(r,1)eq(r,0)` vs
`v_1·v_2 = eq(r,2)eq(r,3)`, i.e. `r_0(1+r_0)(1+r_1)²` vs
`r_0(1+r_0)r_1²` — an eq tensor over 2 variables satisfies
`v_{00}v_{11} = v_{01}v_{10}`, and here that forces `r_1² = (1+r_1)²`,
impossible. Index translation mod `2^w` involves carries; it is a
coordinate (signed-variable) permutation **only** for `c ∈ {0, W/2}`.
So no per-claim point transform exists, and the "existing
`embed_xor_point` machinery generalises trivially" route is closed.

### 3.2 What replaces it: translated-eq is a carry matrix product

Uniform statement covering all three ops. For an axis of ν bits, a
translation constant `δ ≥ 0`, and mode ∈ {cyclic, dropout}, the stream
opening's weight over the *source* index is the **translated eq**

  `V(y) = eq_ν(ρ, y + δ)·[valid]`,   `y ∈ [0, 2^ν)`

(cyclic: `y+δ mod 2^ν`, always valid; dropout: valid iff `y+δ < 2^ν`).
Binary addition gives the transfer factorization: with carries
`c_0 = 0`, `z_i = y_i ⊕ δ_i ⊕ c_i`, `c_{i+1} = maj(y_i, δ_i, c_i)`, and
per-bit eq factors `φ_i(b) = b·ρ_i + (1+b)(1+ρ_i)`,

  `V(y) = f^T · T_{ν−1}[y_{ν−1}] ⋯ T_0[y_0] · e_0`,
  `(T_i[y])_{c→c'} = φ_i(y ⊕ δ_i ⊕ c)·[c' = maj(y, δ_i, c)]`,

`e_0 = (1,0)^T`, `f = (1,1)^T` cyclic / `(1,0)^T` dropout. Each
`T_i[y_i]` is affine in `y_i`, so the same formula with
`y_i ↦ pt_i ∈ K` **is** the multilinear extension of `V` — a
**2×2-matrix-product closed form, O(ν) K-mults per evaluation**. This is
the "2-term-per-variable recursion" the prompt asked to derive; there is
no counterexample to record on this side.

The full committed-side weight of a stream opening
`ŝ(ζ) = Σ_z K̃(z)·D[z]` (exact change of variables; `K̃` = the eq table
of `ζ` reindexed by the forward tap map, supported on column `i`'s
slice) factors over the committed coordinate order
`[row_hi, i-bits, word-bit coords, row_lo]` as, under the corrected
semantics: a translated-eq **group chain** on the low-`g` clear
coordinates (`ROT` cyclic / `SHIFT` dropout — confined to `row_lo`'s
bottom, never near the pack cut), a translated-eq **word chain** on the
remaining trace bits (`row_lo`'s top part at the top coords, carrying
into `row_hi` at the bottom coords — split by the carry `γ` at the
`s`-bit boundary into shifted *slices* of plain eq tables), a
boolean-pinned eq on the column bits (the embedding, unchanged), and
plain eq on any untapped word-bit-axis coordinates. Key implementation
fact: every piece's *table* is a **shifted slice of a plain eq table**
— the prover builds nothing new. (Extraction is likewise whole-run: the
group field lives in the clear axis, so ROT/SHIFT permute clear rows
and the word offset shifts the `row_hi` runs by the borrow — no
sub-word bit twiddling at all.)

### 3.3 The three deployment surfaces (all check out)

1. **Prover `s_v` walk — near-linear ✓.** For a fixed source index the
   carries are *determined*, so the bond classes partition the support:
   the walk enumerates source indices, gathers `w = eq-table[σ(x')]`
   (one gather), computes the class `β(x') = (γ, c₇)` by index
   arithmetic, and accumulates into that class's 128-lane marginal.
   Cost = the existing sparse embedded walk + a shifted gather.
2. **Ring messages — ≤ 3 per twisted claim (public rank).** The pack
   cut (committed coord 7) is crossed only by the word chain (the group
   chain sits in the clear coordinates); the weight splits as
   `K̃(v,y) = Σ_β A_β(v)·B_β(y)` over the reachable classes
   `β = (γ = word carry at the s-boundary, c₇ = carry at the pack cut)
   ∈ {(0,0)} ∪ {(1,0), (1,1) if off > 0}`; pure-ROT streams stay
   rank 1. Both
   sides derive the class list from the public tap descriptor + layout;
   the claim check is `μ = Σ_β Σ_v A_β(v)·s_β[v]` with `A_β` a
   128-entry verifier table (O(128) chain walk). Each class is one ring
   in the η-batched basis — structurally identical to today's flat ring
   list.
3. **Succinct residual closure — O(m·128²) preserved ✓.** The Ligerito
   basis per class is `b(y) = Φ_{r″}(B_β(y))`; `residual_b_evals`
   generalizes to a matrix-product-state form: inside a twisted
   segment the K⊗K accumulator becomes a *pair* (running carry 0/1),
   each variable applies ≤ 2 `apply_right_mul`s per outgoing state
   (2× plain cost), and segment boundaries contract to the class's
   pinned carries. No flock changes — the `eval_b` callback is ours.

### 3.4 Soundness/completeness audit

The reindex identity `ŝ(ζ) = Σ_z K̃(z)·D[z]` and the bond split are
exact algebra; the opener's knowledge soundness is basis-agnostic (it
proves `⟨basis, P⟩ = target` for any public basis the verifier can
evaluate). The inheritance-audit row "sub-column embedding" of the note
becomes "tap embedding: translated-eq basis, MPS closure" — same shape,
zero new error terms. Elision, γ-combination, chunk conservation,
carries-conserved: untouched. **No gap; Phase 0 passes.**

## 4. Build plan refinement discovered in Phase 0

The batched-vx **baseline itself needs the twisted openings**: a tapped
claim's residual `b̂_i(ζ)` expands by char-2 linearity into its taps'
stream openings — each a twisted committed opening. So the build order
is: extraction ops (§1 taps) → **twisted-opening primitive** (walk +
rings + basis fill + MPS closure) → vx-taps baseline (Phase 1, the
correctness oracle) → the stream family over the same primitive
(Phase 2) → measurement (Phase 3). The primitive gets exercised and
tamper-tested in the simpler vx setting first.

## 5. Measured postscript (2026-07-27, Phases 1–3 done; re-measured
after the semantics correction)

Both paths landed (`prove/verify_mle_eval_mod_q_ligerito_tap_claims`,
`..._tap_family`; harness `examples/taps_ab.rs`), were retargeted to
the corrected entry-axis-group semantics the same day (the theory
above survived verbatim; extraction and the §3.3 coordinate mapping
simplified), and §2's honest cost flag **realized as a loss** under
both readings: at n = 22/24/26 (corrected semantics, bv = 0, g = 5)
the clustered family proves at 257.9/743.4/3823 ms against the
tap-claims baseline's 118.7/319.3/2006 and 6 independent proofs'
151.9/401.2/940 (n=26 churned-box both runs; n=28 skipped for memory
honesty). The n=24 phase tree pins the cause exactly where §2 pointed:
the 45 monomial channels cost ~600 of 743 ms (~13 ms/channel — the
per-channel cascade constant the shared-point session measured
independently), which no forest-body dedup at this sharing density
(13 streams / 6 claims) can repay. The translated-eq opening machinery
of §3 itself is cheap and correct (rings + closures are ~13 % of the
family prove and carry the entire baseline path). The family keeps a
proof-size win at n ≥ 24 (−20 %/−37 % vs the batched baseline) with
verify within 1.6–2.1×. Verdict: route tapped-convolution claims
through the extraction path; the stream family needs dense stream
reuse or an order-of-magnitude cheaper discharge (kernels / forest
fusion) to compete on time. Full table and guidance: the dated README
note.

## 6. The COMPOSED collapse (2026-07-27, follow-up session): uniform
outer ops over MIXED sources

The single-tap collapse (`0x44`, §5's follow-up) reads `op(⊕ cols)`:
the source is a plain XOR of committed columns and the inner claims run
the claims-only path. Nothing in the collapse identity

  `Σ_p w[p]·op(x)[p] = Σ_{p'} w[σ(p')]·[valid]·x[p']`

used that structure — only that the inner claim `⟨w∘σ, x⟩` is provable.
Generalization: let the source be a fixed **XOR-of-taps combination**
`x_S = ⊕_{t∈S} op_t(a_{i_t})` (extractable; provable by the 0x42
tap-claims path), and take k claims `OUTER_i(x_{S_i})` at ONE shared
point `w_row ⊗ e_col`. Then:

- **The branch split is source-agnostic.** `w∘σ_OUTER` splits into the
  two word-carry branches exactly as in 0x44 — the split depends only
  on `(OUTER, layout)`, never on what `x` is: branch 0 keeps `w_row`;
  branch 1 advances `row_hi` one step (zero at the top — the overflow
  dropout); the column side is the branch-masked, group-translated
  `e^{(β)}` (the deployed `tap_collapse_row_weights` /
  `tap_collapse_col_weights`, verbatim).
- **γ-combination.** `Σ_i γ_i·claim_i = Σ_{(S,β)}
  ⟨w_row^{(β)} ⊗ E_{S,β}, x_S⟩` with
  `E_{S,β} = Σ_{i: canon(S_i)=S} γ_i·e_i^{(β)} mod q` — at most
  `2·#distinct-sources` **inner tap claims**, independent of k. The
  inner bodies are 0x42 claims (per-claim row weights already
  supported); the verifier derives each `y_{S,β}` from the proof's own
  forest-bound fold vectors with `E_{S,β}` and checks
  `Σ y = T = Σ γ_i·c_i`.
- **Soundness.** 1/q (the γ-RLC, drawn after the statement binds the
  claimed values) + the inner path's errors — the 0x44 chain with
  "committed column" replaced by "extracted source"; the §3–§3.4 audit
  already covers tapped `x`. No new terms.

Why this is the XOR-mixed order-of-magnitude lever: **shift-invariant
(schedule-shaped) workloads make every round's mixed combination a
word-offset of ONE fixed combination** (`b_{t+1} = off¹(b_t)`), so k
rounds = k claims `off^t(x)` of one mixed `x` → ONE source, TWO inner
bodies total, versus one padded forest body per claim on the batched
path (k=48 pads to 64). The XOR-mixing price — the translated-eq rings
and their carry classes — is paid once per distinct source SHAPE, not
per claim.

Details pinned by the implementation (tag `0x45`,
`TapComposedClaim { source, outer, claimed }`,
`prove/verify_mle_eval_mod_q_ligerito_tap_composed`):

- **Canonical sources.** Per tap: `bit_amt = 0` clears the dropout flag
  (`SHIFT^0 = ROT^0`), full identities clear the group width; then sort
  by `(col, g, amt, dropout, off)` and cancel identical PAIRS (char 2)
  — `tap_canonical_ops`. Sources equal as vectors but distinct as
  canonical descriptor lists get separate inner bodies (sound, merely
  less merged). A source that cancels to empty is rejected.
- **Envelope.** The outer op needs `off < 2^{s−g}` ALONE — it does not
  compound with the source taps' offsets (the transform treats `x` as a
  black box), so a 48-round schedule needs `s − g ≥ 6` (n ≥ 22 at the
  harness split: s = 11/12/13 at n = 22/24/26).
- **Known inner-path slack (v1).** The two branch claims of one source
  carry identical tap lists, so their per-(tap, class) rings are
  IDENTICAL vectors (the ring `s_v` depends on the exit point, not the
  row weights) and the source is extracted twice — a factor-2 dedup in
  rings and extraction left on the table; rings were ~4 % of the vx6
  prove, so this is bytes more than time.

**Measured** (same day; `F2Z_AB_SCHED=1`, 48 claims `off^t(x)` of one
σ-style source, values through the offset-folded extraction route,
medians of 5): composed **39.2/94.8/279.7 ms** at n=22/24/26 with 2
inner bodies — **22.1×/34.2×/— vs the batched path** (864.9/3242.3/
skipped; its 64-set pad already inverts against 48 independent proofs
at n=24: 3242 vs 2896) and 35.9×/30.6×/26.2× vs independent; proofs
189/284/448 KB vs 3602 KB batched at n=24 (−92 %); verify 18.8/29.7/
58.0 ms; peak at the single-proof footprint. 48 XOR-mixed claims =
1.0–1.9× ONE claim. Full table: the README's dated note; formal
statement: the note's §"The composed collapse" (`sec:tapcomposed`).

## 7. Blocked batching (P2, same day): the pad is gone; 2-set blocks
are the sweet spot

The batched tap path (0x42) padded its merged forest to `2^⌈log₂k⌉`
tree-sets. Now claims run in BLOCKS through the same batched common —
each block its own forest + presum absorbs and residual exit point
(sequential FS composition; per-claim exit points thread through the
ring plans; extraction per block, freed between), all blocks sharing
the statement, the η-batched ring basis, and the ONE closing Ligerito
call. A block-size scan (k=6 instance + k=48 schedule baseline,
n=22–28) found **2-set blocks best-or-tie everywhere**: the forest's
marginal round-sharing saturates at two tree-sets while wider merges
pay the cache regime (4+2 cost +27 % at n=28). Shipped policy:
`TAP_CLAIM_BLOCK_CAP = 2` (structural; k ≤ 2 byte-identical to
pre-blocking, so `single` and the 2-body composed proofs are
unchanged). Acceptance met: vx6/single = 3.64×/2.69×/5.01×/5.75× at
n=22/24/26/28 (all ≤ 6.2×), the n=26 inversion cured, the batch now
beats independent proofs at every shape, and the n=28 row exists for
the first time (peak ≈ 2 tree-sets for ANY k). Composed-collapse
multiples vs the improved baseline: 18.8×/21.7×/21.7× (vx48 =
736.3/1898.1/5980.8 ms; n=26 now runs). Batched proofs for k ≥ 3
change bytes (+5–10 % from per-block transcripts) — the price of the
flat memory profile.
