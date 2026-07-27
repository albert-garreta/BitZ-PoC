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

## 0. Conventions (flagged for user confirmation)

Entries are 0-indexed here (`row ∈ [0, N)`, the prompt's `k = row + 1`).
Uniform "gather" convention: **output index gathers input index − amount**
— on every axis, `op` with amount `c` maps input position `x` to output
position `x + c` (forward), so output `x` reads input `x − c`.

- **Entry-offset `off^o`**: `s[row] = a[row − o]`, zero for `row < o`
  (the prompt's boundary spec: taps `k−1, k−2` vanish at `k = 1, 2`). ✓
  matches the given spec exactly.
- **`ROT^c`**: bit `j` of output = bit `(j − c) mod W` of input (rotate
  toward higher bit indices; cyclic, no dropout).
- **`SHIFT^c`**: bit `j` of output = bit `j − c` of input, zero for
  `j < c` (shift toward higher bit indices; input bits `≥ W − c` drop
  out).

**TO CONFIRM with the user** (carried from the prompt, plus one new):
(1) `a_{3,k−2}` read as `a_1[k−2]`; (2) the bare `ROT` in `b_3`'s second
tap read as `ROT^2`; (3) **new**: the ROT/SHIFT *direction* above
(toward higher bit indices). SHA-2's σ-functions use the opposite
direction (`ROTR`/`SHR`); the machinery is direction-generic (`c ↔ W−c`,
dropout end flips), so a flipped convention changes constants only.

## 1. Streams and dedupe (the instance)

`j = 2` committed columns, `W = 2^{bit_vars}`-bit words (instance run at
`bit_vars = 5`, `W = 32`), `k = 6` claims. Distinct
(column, rot-or-shift, offset) taps, after dedupe:

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
`[row_hi (tw), bit j (bit_vars), row_lo (s)]`). They must reduce to
openings of the *committed* matrix.

### 3.1 ROT is NOT a coordinate permutation (boundary recorded)

The prompt's first hope — "`ROT^c` permutes the bit-position variables:
the embedded point permutes coordinates" — is **false** for general `c`.
Counterexample (`W = 4`, `c = 1`): the rotated-eq vector
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
`[row_hi, i-bits, j-bits, row_lo]` as: a translated-eq **chain on the
trace axis** (row_lo LSBs at the top coords, carrying into row_hi at the
bottom coords — split by the carry `γ` at the `s`-bit boundary into
shifted *slices* of plain eq tables), a boolean-pinned eq on the column
bits (the embedding, unchanged), and a translated-eq **chain on the bit
axis** (`ROT` cyclic / `SHIFT` dropout). Key implementation fact: every
piece's *table* is a **shifted slice of a plain eq table** — the prover
builds nothing new.

### 3.3 The three deployment surfaces (all check out)

1. **Prover `s_v` walk — near-linear ✓.** For a fixed source index the
   carries are *determined*, so the bond classes partition the support:
   the walk enumerates source indices, gathers `w = eq-table[σ(x')]`
   (one gather), computes the class `β(x') = (γ, c₇)` by index
   arithmetic, and accumulates into that class's 128-lane marginal.
   Cost = the existing sparse embedded walk + a shifted gather.
2. **Ring messages — ≤ 4 per twisted claim (public rank).** The pack
   cut (committed coord 7) is crossed by at most the trace chain and/or
   the bit chain; the weight splits as
   `K̃(v,y) = Σ_β A_β(v)·B_β(y)` over ≤ 4 bond classes
   `β = (γ = trace carry at the s-boundary, c₇ = carry at the pack
   cut)`; pure-ROT streams with `tw + log_cols ≥ 7` stay rank 1. Both
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

## 5. Measured postscript (2026-07-27, Phases 1–3 done)

Both paths landed (`prove/verify_mle_eval_mod_q_ligerito_tap_claims`,
`..._tap_family`; commits `29825ba`, `5a33eeb`; harness
`examples/taps_ab.rs`) and §2's honest cost flag **realized as a loss**:
at n = 22/24/26 the clustered family proves at 232.9/791.5/4297 ms
against the tap-claims baseline's 113.4/349.7/1885 (and 6 independent
proofs' 178.9/394.3/891.5; n=26 churned-box, n=28 skipped for memory
honesty). The n=24 phase tree pins the cause exactly where §2 pointed:
the 45 monomial channels cost 540 of 791 ms (~12 ms/channel — the
per-channel cascade constant the shared-point session measured
independently), which no forest-body dedup at this sharing density
(13 streams / 6 claims) can repay. The translated-eq opening machinery
of §3 itself is cheap and correct (rings + closures are ~13 % of the
family prove and carry the entire baseline path). The family keeps a
proof-size win at n ≥ 24 (−26 %/−41 % vs the batched baseline) with
verify within 1.6–2.4×. Verdict: route tapped-convolution claims
through the extraction path; the stream family needs dense stream
reuse or an order-of-magnitude cheaper discharge (kernels / forest
fusion) to compete on time. Full table and guidance: the dated README
note.
