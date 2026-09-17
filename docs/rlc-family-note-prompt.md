# Session prompt — LaTeX note: batching F₂-linear families of integer-MLE claims via mod-q randomized weight functions

You are in `/Users/albertgarretafontelles/f2z-pcs` (the BitZ repo — the `f2z-pcs`
skill applies). Invoke the `writing-crypto-papers` skill BEFORE drafting.
Deliverable: a fresh, self-contained LaTeX note under `docs/rlc-family-note/`
(`main.tex`, built with `latexmk`), in the notation and style of the parent
X-note (`/Users/albertgarretafontelles/char2-fieldswitch-note/main.tex`,
especially §9, `\label{sec:brakedown}`) and `docs/DESIGN.md`. Working title:
*"Randomized weight functions: batching F₂-linear families of integer-MLE
claims in a characteristic-2 PCS"* — improve it.

The construction below was derived in a working session on 2026-07-26 and is
**unvetted**. Your job is equal parts exposition and verification: re-derive
every identity and bound yourself; if anything breaks, STOP papering over it
and record the gap as an explicit open problem in the note.

## Base scheme (recap; cite DESIGN.md / X-note §9 rather than re-proving)

BitZ proves `MLE[INT(D)](r) = c ∈ F_q` for bit-data `D` (2^t·W row-bits ×
2^s columns, n = t + log₂W + s) committed under a char-2 commitment. Row
weights `w_b = eq(b, r_rows) mod q ∈ [0, q)` are **already reduced mod-q
representatives**; per-column integer folds `u_c = Σ_b w_b·D[b,c]` ride in
the exponent of K = GF(2^128) (`α^{u_c}`, α a checked generator), certified
by a lazy GKR product forest; weights are **chunked** into `c_w = 127−t−W`
bit limbs so each per-chunk fold stays < 2^127 < ord(α); a degree-2
pre-sumcheck ("presum") de-black-boxes the α-power factor, leaving a bit-MLE
claim opened via ring-switch + recursive Ligerito; the verifier recombines
`Σ_c eq_c · Σ_l 2^{c_w·l} u_c^{(l)} mod q` in the clear. Prover cost is
forest-dominated (84–96%). KEY FACT the construction relies on: nothing in
the prover-side soundness chain uses the eq-structure of the weights — any
public weight function with values in [0, q), chunked, works verbatim.

## The problem

k claims `MLE[INT(a_i)](r_i) = c_i`, where each claimed vector `a_i` is an
**F₂-linear combination of j committed bit-columns** m_1..m_j (per position:
`a_i = L_i(m_1,…,m_j)`, L_i a public F₂-linear form). Motivating instance:
k=3, j=2, `a_3 = a_1 ⊕ a_2`. Today each claim costs its own forest (the
repo's virtual-XOR machinery batches and derives rows but still pays
~one forest body per claim). Why no discount is possible naively:
`INT(a⊕b) = INT(a) + INT(b) − 2·INT(a∧b)` — the AND/carry evaluation is
genuinely new information (give the small counterexamples: a₁=a₂ vs
disjoint supports). Also record why the naive integer-coefficient RLC
fails: soundness bits of integer γ's eat the 127-bit exponent budget
(λ ≈ 100 makes c_w negative; k smaller-λ repetitions multiply chunk counts
faster than they remove forests).

## The construction

1. After the commitment root and all c_i are absorbed, the verifier draws
   γ_1,…,γ_k ∈ F_q (γ_1 := 1 wlog, optional). Combined target
   `T = Σ_i γ_i c_i mod q`.
2. **Case-collapsed weight function**: REQUIRE all claims share the column
   point (r_cols equal; row points r_i,rows may differ). Per row-position b
   and case vector m ∈ {0,1}^j:
   `W_b(m) = ( Σ_i γ_i · w_{i,b} · L_i(m) ) mod q ∈ [0, q)`,
   with `w_{i,b} = eq(b, r_{i,rows}) mod q`. Verifier-computable, public.
   Chunk each `W_b(m)` into c_w-bit limbs as usual. For W>1 word-bits the
   2^{j′} bit-weight multiplies the reduced value exactly as in the base
   scheme (do the magnitude accounting explicitly).
3. **One forest** over 2^n leaf positions (per chunk): the leaf at position
   (b,c) is `α^{W_b^{(l)}(m(b,c))}` — a 2^j-case select on the j committed
   bits at that position; the derived vectors' bits never materialize.
   Multilinearity: for j=2 with A=α^{W(10)}, B=α^{W(01)}, C=α^{W(11)},
   `leaf = 1 + m₁(A+1) + m₂(B+1) + m₁m₂(A+B+C+1)` (char 2); general j via
   the subset zeta-transform `leaf − 1 = Σ_{∅≠S⊆[j]} τ_S · Π_{i∈S} m_i`,
   `τ_S = Σ_{T⊆S} α^{W(1_T)}` (char-2 sum). The GKR layers above the leaves
   are unchanged.
4. **Multi-channel presum**: `e_d + 1 = Σ_{pos} Σ_S R_S[pos]·(Π_{i∈S} M_i)[pos]`
   with `R_S = eq ⊙ τ_S` — 2^j − 1 channels in one multi-degree sumcheck.
   Channels |S| = 1 exit directly as committed bit-MLE claims at (r*, ξ).
5. **Monomial discharge**: channels |S| ≥ 2 exit as monomial-MLE claims,
   discharged by ONE η-batched eq-sumcheck over n variables of degree ≤ j+1
   (`Σ_x eq((r*,ξ),x)·Σ_S η_S Π_{i∈S} M_i(x)`), exiting at ≤ j committed
   openings M_i(ρ). All residual openings (from presum and discharge)
   η-batch into the single ring-switch + Ligerito open.
6. Verifier: computes `u′` recombination mod q, checks = T; evaluates each
   `R̂_S(r*)` via O(2^t·W) exponentiation tables (2^j − 1 of them); checks
   the free per-chunk range bound `u′^{(l)}_c < 2^{c_w+t+W}` (verify it is
   unchanged: per position exactly one case weight contributes, chunks
   < 2^{c_w}).

## Theorems to state and prove

- **Main theorem** (general j, k): completeness, and soundness error =
  base-scheme error + k/q (Schwartz–Zippel on the γ-combination; prove this
  step fully — the errors δ_i are fixed before γ by FS ordering) + the
  discharge sumcheck error. The α-injectivity argument is inherited
  verbatim (weights arbitrary reduced representatives; per-chunk magnitude
  bound identical) — state exactly what changes and why each inherited
  lemma survives (leaf multilinearity in j bits; multi-channel presum;
  the discharge).
- **Corollary (multi-point batching)**: k claims on the SAME column at
  different row points, shared column point → 2 cases, no discharge, ONE
  forest. (Today's API pays k forests.)
- **Degenerate case**: affine forms (XOR by constants / complements) give
  τ_S = 0 for |S| ≥ 1 beyond the affine shift — recover the repo's
  `complement_elision` as the j=0/1-affine special case. Unifying remark.

## Boundaries and negative results (give them their own section)

- **Chunks do not collapse** (RLC of chunk weight sets is q-sized again —
  circular). State the conservation law: forest passes ≈ (weight-function
  entropy)/(127-bit exponent bandwidth); the construction wins by
  legitimately compressing k weight functions into one via field-sized
  randomness in the weights, not by beating the law.
- **Fully different points don't collapse**: differing r_cols entangle the
  column weight into W_{b,c}(m) — per-column weight tables destroy the
  shared-table structure. Same-r_rows/different-r_cols is already free in
  the base scheme (same u_c vector) — note the symmetry.
- **Carries are conserved**: the |S|≥2 channels ARE the carry information;
  the construction moves them from k−… forests in the exponent to one
  degree-(j+1) sumcheck in the cheap field. Tie to the adder/carry lore
  (INT(a⊕b) identity; committed-carry architectures in SHA provers).
- Case tables scale 2^j — for large F₂-linear layers, cluster relations
  into low-rank groups; leave the optimal clustering as an open problem.

## Cost model (table)

Against k independent claims (~k forest bodies + shared open): 1 forest
with ~1.1–1.3× leaf-round cost (2^j-case tables) + ~0.4–0.6 forest-
equivalents for the discharge + shared tails. For the XOR triple: ~1.6–1.8×
a single claim vs 3.0×. Proof size: one forest transcript + ONE u′ vector
(vs k). Verifier: one forest verify + one sumcheck; the O(2^t) table step
scales ×(2^j−1). Use the repo's measured n=28/n=30 numbers (README
reference tables + the 2026-07-26 optimization notes) for a concrete
worked estimate; label all numbers as estimates pending the prototype
(`docs/rlc-family-proto-prompt.md` is the companion prototyping prompt).

## Open questions section (at minimum)

Optimal clustering for large linear layers; whether the discharge can ride
the forest's own layers; precise constants for the 2^j-case lazy leaf
kernels; interaction with the batched/virtual-XOR APIs (supersede or
coexist); whether the multi-point corollary extends to mixed
column-point families via a second RLC level; ZK.

## Facts you must not get wrong

- c_w = 127 − t − W; per-chunk folds must stay < 2^127 < ord(α) = 2^128−1;
  the generator check is on α.
- Weights are reduced representatives in [0, q); the mod-q semantics of
  the final check is native to the base scheme, not an addition.
- γ's are drawn AFTER the commitment root and all c_i are in the
  transcript.
- Char-2 kills the sign: −2·INT(a∧b) matters over ℤ/F_q but the K-side
  bit-MLEs stay linear (that's why the ring-switch side was always free).
- The X-note §9 construction is due to Lev Soukhanov (levs57) — attribute;
  this note extends it.
