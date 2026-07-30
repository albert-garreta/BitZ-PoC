# A Blake3 arithmetization for F2Z — design note

Session record, 2026-07-27 (the structured-taps optimization session,
follow-up to the cols4 model). Goal: the best claim-set design for
proving Blake3 compressions over the F2Z PCS (W = 1 bit-columns,
mod-q integer-MLE claims, the tap/collapse machinery of
`docs/rlc-structured-taps-phase0.md`), designed from the machinery's
measured cost model rather than from a translated circuit.

## 0. The two principles the design is built on

The session's measurements pin a sharp cost dichotomy:

**P-LIN (wiring is free for linear relations).** A mod-q claim is
`⟨w_row ⊗ w_col, bits⟩` with ARBITRARY public weights. Any
integer-linear statement about committed words — additions with
carries, place-value recompositions, boundary injections, even reads
through an arbitrary PUBLIC permutation — is a linear combination of
single-column claim values at chosen weights, checked in the clear.
Single-column claims at one shared point γ-merge per (column, carry
branch) into PLAIN forest bodies (the 0x44 collapse; per-claim
COLUMN weights are free in the merge — `E = Σ γᵢeᵢ` — as long as the
ROW weights are shared). Cost: **≤ #columns + #offset-branches
bodies, total, for every linear relation in the system.**

**P-XOR (only F₂-XOR relations need structured wiring).** A relation
that XORs differently-translated operands (`x ⊕ y = z` across
rotations/offsets) cannot ride the linear layer (the integer
cross-term `x⊕y = x+y−2·x∧y`), and F2Z has no product argument to
bind the AND. Each distinct XOR-relation VECTOR costs exactly one
0x42 body (extraction + translated-eq rings), and its operand wiring
must be tap-shaped (translations on contiguous index fields). Cost:
**one body per distinct XOR-relation shape** — and shift-invariant
stacking makes one shape cover ALL instances of that relation across
steps, lanes, rounds, and compressions simultaneously.

The design objective follows: route everything possible through
P-LIN, and minimize the number of distinct XOR-relation shapes.
Blake3 is unusually friendly to this split: its only
nonlinear-over-F₂ operations are the mod-2^32 additions (pure P-LIN
with carry witnesses), and its only XOR relations carry uniform
rotation constants (pure P-XOR, four shapes). The message schedule
is a permutation — no adds, unlike SHA-2 — which P-LIN absorbs into
read weights.

## 1. Trace layout

One compression = 7 rounds × 8 G's = 56 G's; serialize at HALF-G
resolution (112 half-G steps), where each half-G updates each role
once:

```
half-G (even):  a += b + m_x ; d = ROT16(d ⊕ a) ; c += d ; b = ROT12(b ⊕ c)
half-G (odd):   a += b + m_y ; d = ROT8 (d ⊕ a) ; c += d ; b = ROT7 (b ⊕ c)
```

Lane-packed: the 4 parallel G's of a round-half run the SAME step on
4 quadruples, so the step index is τ = (round, half, phase, lane)
with lane as the LOW word bits — every relation below is τ-uniform
with lane-stride word offsets.

**Committed columns (6, padded to 8; `log_cols = 3`, W = 1):**

| col | contents (one 32-bit word per half-G step τ, per compression block) |
|---|---|
| A | the a-role value AFTER step τ's a-update |
| B | the b-role value after τ's b-update |
| C | the c-role value after τ's c-update |
| D | the d-role value after τ's d-update |
| K | carry BITS, packed 32 per word: the a-adds' 2-bit carries (k ∈ {0,1,2}) and the c-adds' 1-bit carries — ~11 words per compression |
| X | auxiliary block: the 16 message words, the 16 input-state words (IV/chaining), the 8 output words — ~40 words per compression |

Compressions stack block-aligned along the trace. The per-compression
role block (112 words ≈ 2^12 bits) must sit inside the clear span
for the intra-block weight patterns to stay row⊗col tensors:
**s ≥ 12** (whole-block patterns want s ≥ 14). This is the design's
one layout pin: the plain sub-proof runs a col-heavy split.

## 2. The claim set

All claims at ONE shared evaluation point (row side = `eq_hi` over
the block/high-trace bits, shared by every claim).

**Plain layer (P-LIN, the multiweight collapse — tag 0x46).** Every
add, carry recomposition, boundary injection, and permuted message
read is one linear equation over single-tap claim values with
per-claim column weights:

- a-adds: `v(A) = v(off¹A) + v(B, prev-step weights) + v(X,
  σ-permuted message weights) − 2^32·(v(K, k₁-mask) + 2·v(K,
  k₂-mask))`, with τ-masked weights selecting the a-add positions.
- c-adds: `v(C) = v(off¹C) + v(D, aligned weights) − 2^32·v(K,
  k_c-mask)`.
- carry range: free — K is a bit-column (the F₂ commitment ranges
  bits structurally), and the 2-bit ternary carry k = k₁ + 2k₂ has
  k = 3 unsatisfiable (a + b + m ≤ 3·2^32 − 3 < s + 3·2^32).
- boundary: input-state words = public IV / chaining values, read by
  masked X-weights against constants; first-step operand reads
  redirect into X's state region by weight support.
- the message permutation σ: `v(X, w∘σ)` — σ is intra-block and
  block-uniform, so the permuted weights remain a row⊗col tensor.
  **The permutation costs nothing.**

Every term is a single-tap read (identity or `off¹`) of one
committed column, so the whole layer γ-merges per (column, branch)
into **8 plain bodies**: A(β0, β1), B(β0), C(β0, β1), D(β0), K(β0),
X(β0). Individual read values need no binding — only the γ-combined
relation sums enter; the collapse verifier derives branch values
from the proof's own fold vectors.

**The δ = g refinement (found by the model measurement).** The naive
tensor pin says δ = 0 (per-claim weight patterns must stay
column-side). But every P-LIN read can share ONE bit-position
profile — the place values `2^j` the add-checks want anyway (reads
that conceptually want other j-profiles are word-granular linear
relations, so they take the shared profile too) — leaving all
per-claim variation word-granular. Then the whole j-field folds:
the plain layer runs **δ = g = 5** with shared row weights
`eq_hi ⊗ 2^{j}` and per-claim word-index column weights, shrinking
its fold vectors 32× and its forest tree count likewise. Measured:
this flipped the model from losing bytes to winning both axes.

**XOR layer (P-XOR, the blocked 0x42 path).** The four xor-rot
step-shapes, each ONE shift-invariant zero-vector claim covering all
its instances (weights select the step's τ-parity; out-of-parity
positions are unconstrained scratch by weight support):

| vector | taps |
|---|---|
| R_d-even | `ident(D) ⊕ ROT¹⁶(off¹D) ⊕ ROT¹⁶(A)` |
| R_b-even | `ident(B) ⊕ ROT¹²(off¹B) ⊕ ROT¹²(C)` |
| R_d-odd  | `ident(D) ⊕ ROT⁸(off¹D) ⊕ ROT⁸(A)` |
| R_b-odd  | `ident(B) ⊕ ROT⁷(off¹B) ⊕ ROT⁷(C)` |

plus one 2-tap vector for the finalization/chaining XORs
(`h'ᵢ = vᵢ ⊕ vᵢ₊₈`; output → next-block input copies at the block
stride — uniform offsets, one shape). The mixed bodies run at the δ
knee (the XOR layer has no weight-tensor constraint; its claimed
values are 0).

**The diagonal boundary, accounted (correction, same day).** The
table above is the INTERIOR picture; the diagonal shuffle is real
dataflow and it touches P-XOR exactly once per phase boundary: the
ROT16/ROT12 steps read `d`/`b` from the previous phase at a
lane-rotated position (diag-G_l takes b from lane l+1 mod 4, …), and
a cyclic shift on the 2-bit lane subfield is not a single tap. Two
sound treatments:

1. **Continuity route (preferred — zero extra bodies).** Store each
   half's values in ITS OWN quadruple-grouping order, adding a
   committed copy of the incoming `d`/`b` word per shuffled boundary
   (≈ +2 words per G ≈ +20 % trace). Every XOR relation then reads
   same-lane interior offsets (pure taps, the 4-shape table stands),
   and the regrouping becomes "incoming copy = last half's outgoing
   word at the rotated lane" — word EQUALITY is integer-linear, so
   these continuity constraints ride the P-LIN layer at
   lane-permuted, place-valued column weights, exactly like the
   message permutation. Cost: none in bodies; ~20 % fewer
   compressions per trace.
2. **No-copy route: wrap-split vectors.** Keep one storage order and
   split each shuffled boundary relation into two vectors (non-wrap
   offset tap / wrap offset tap), each zero-checked on its lane
   subset via the weight mask; pre-permuting one phase halves the
   shapes. Net ≈ +4–8 mixed bodies (~9–13 total).

Route 1 dominates (bodies are 2^{n−3} each; trace density only
scales compressions-per-proof): the mixed-body table stays 5–6 and
the per-compression estimate moves only by the trace factor, to
≈ 110 µs at the n = 26 scale.

## 3. The count

| layer | bodies | machinery |
|---|---|---|
| plain (all adds, carries, boundary, schedule) | 8 × 2^{n−3} bits | 0x46 collapse, no rings |
| XOR (4 step shapes + finalization/chaining) | 5 × 2^{n−3} bits | 0x42 blocked, δ knee |

**13 bodies of 2^{n−3} ≈ 1.6× one full-matrix proof for the entire
relation system**, independent of the compression count. Trace
density ≈ 490 words ≈ 15.7 Kbit per compression → a 2^24-bit trace
holds ~1000 compressions.

Why the alternatives lose (measured/analyzed this session):

- per-round / per-lane relation claims: multiply the XOR bodies by
  56–224× (the 0x45 composed collapse would claw schedule-shaped
  repeats back to 2 bodies per shape, but shift-invariant stacking
  makes even that unnecessary);
- committed rotated/shifted state copies: each needs a consistency
  claim costing what the rotation cost anyway — rotations are weight
  transforms/taps, never commit them;
- Binius-style committed AND columns to linearize the XORs: needs a
  product argument F2Z doesn't have; the XOR layer's 5 bodies are
  cheaper than importing one;
- the stream family over the tap streams: the relation vectors sit
  on disjoint supports → channels multiply (the cols4 143-channel
  analysis; measured dead);
- bitwise adds (full-adder chains): turn every add into XOR/MAJ
  relations — MAJ is a product; strictly worse than word-level
  carries, which are P-LIN.

## 4. Soundness pins

- Every linear identity is a Schwartz–Zippel zero-check of a defect
  MLE at the shared point (error #vars/q, q = 2^100 − 15), γ-RLC'd
  across relations (1/q), on top of the collapse chain (1/q + the
  inner path's errors).
- The XOR vectors are zero-checks of bit-vector MLEs through the
  audited 0x42 chain.
- Carry soundness is structural (bit columns; the unsatisfiable
  k = 3 case). Defect magnitudes (< 2^34) never wrap mod q.
- The multiweight collapse (0x46) is the 0x44 argument verbatim with
  per-claim `e_i`: the landed verifier already γ-combines per-claim
  branch tables (`E_{set,β} = Σ γᵢ·eᵢ^{(β)}`); only the statement
  gains the per-claim weight vectors. Row weights stay shared per
  branch — guaranteed by construction (all row-side variation is
  `eq_hi`; all per-read variation is column-side, δ = 0).

## 5. What the model preset measures

`F2Z_AB_BLAKE3=1` (harness) models the design's claim SHAPE at
scale — 8 committed columns, ONE commitment serving both layers
(x_fold_extra is a claim-path parameter, not a commitment
parameter), the 16-read plain layer through 0x46 (genuinely distinct
per-claim column weights, two reads per body — the real relation
density) at δ = 5, and the 5 mixed vectors through blocked 0x42 at
the δ = 4 knee — against `vx21` (all 21 claims through the batched
tap path: the routing a collapse-unaware design would use, one body
per read). Claim values are computed by extraction (the model
commits random data, not real Blake3 traces; body count and claim
shapes, which are what cost, are exactly the design's).

Measured (16 GB M-series, FAST profile, medians of 5, one shared
point; prove ms / proof KB / verify ms):

| n | b3 (8 plain δ5 + 5 mixed δ4) | vx21 (collapse-unaware) |
|---|------------------------------|--------------------------|
| 22 | **66.2 / 304 / 20.6** | 102.6 (1.55×) / 322 / 33.7 |
| 24 | **138.0 / 379 / 20.8** | 208.5 (1.51×) / 393 / 32.2 |
| 26 | **381.6 / 468 / 22.2** | 607.6 (1.59×) / 488 / 34.9 |

At n = 26 the trace holds ≈ 4200 compressions → ≈ 90 µs and ≈ 110 B
of proof per Blake3 compression at this model's density, ~ms-class
verify for the whole batch. A real-trace instantiation additionally
needs: the s ≥ 12 col-heavy split (the model runs the default even
split), the τ-mask/σ weight builders, a witness generator, and the
r1/8 byte profile if bytes matter — none of which changes the
measured cost shape.
