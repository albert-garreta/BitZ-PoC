# F2Z design notes

F2Z realises §9 of Lev Soukhanov's char2-fieldswitch note: an integer-MLE
evaluation over an `F₂` commitment, folded **in the exponent** of
`K = GF(2^128)` and opened by a **ring-switch + recursive Ligerito** pipeline
(the only opener). This document is the distilled protocol, the serialization
format, and the optimization inventory of the implementation; the authoritative
long-form write-ups live in the source repository (`zinc-plus`:
`documentation/f2-int-eval-doc/`, and the measurement ledger
`documentation/f2x-sha-todo.md`).

## The problem

An integer MLE evaluation cannot be read off an `F₂` commitment by
`F₂`-proximity: the parity collapse `Σ G[j,i]·d_i mod 2` destroys the integer
information, while committing with a genuinely integer code forfeits the cheap
binary commitment. F2Z's resolution: **do not fold through the code**. Commit
the bits over `F_2`, and carry the integer row-fold **in the exponent** of
`K = GF(2^128)`, certified by a GKR grand product that touches the commitment
only through `K`-linear queries.

## Instance and protocol flow

`IntEvalParams { t, s, word_bits: W }`: data `D` is a `2^t × 2^s` matrix of
`W`-bit cells (`cell_index(b,c) = (b<<s) | c`); `t` row variables fold with
integer weights `w_b`, `s` column variables are read off with field weights
`e_c`.

1. **Commit** (before any challenge): the bit-matrix is packed 128 bits per
   `GF(2^128)` element along the low 7 row-bit coordinates, arranged as
   `2^{log_batch}` interleaved lanes, RS-encoded per lane with flock's additive
   NTT, and the per-position lane stacks are Merkle leaves (`flock-core`'s
   `commit`). The root is published.
2. **Fold in the exponent**: per column,
   `α^{v_c} = ∏_{b,j} [bit ? α^{w_b·2^j} : 1]`. The **merged** GKR
   grand-product forest binds every `α^{v_c}` and reduces all `2^s` trees to one
   shared exit point. The roots are *not* carried: they are `α^{v_c}` by
   construction, so the verifier recomputes them from the sent integers `v_c`
   (the Fiat–Shamir absorb of the recomputed roots is the binding).
3. **Pre-sumcheck**: a degree-2 sumcheck over the row-bit variables strips the
   α-power weight factor `R = eq(·,ρ)⊙(α-powers−1)` (the `q_rowbit` table),
   leaving a pure bit-MLE evaluation claim `M̂(r*, ξ) = μ`. The verifier's only
   `O(2^t·W)` step is evaluating `R̂(r*)`.
4. **Ring-switch → Ligerito** (the opening): the prover sends 128 partial
   evaluations `s_v = M̂(r_hi, v)`; the verifier checks `Σ_v eq(r_lo,v)·s_v = μ`,
   draws a fresh `r″`, and the recombined residual `Σ_y B(y)·P̂(y) = β₀`
   (`B(y) = Φ_{r″}(eq(r_hi, y))`) becomes an inner-product claim on the packed
   polynomial `P`, discharged by flock's recursive Ligerito verifier. Its
   closing residual `B̂` is evaluated succinctly (`O(m·128²)`) via the
   tensor-algebra trick (`tensor_eq_phi_eval` / `residual_b_evals`) — no `2^m`
   table.
5. **Read-off in the clear**: `P(r) = G_r · Σ_c e_c·v_c` over any
   characteristic-≠2 ring.

Soundness chain: committed bits →(Ligerito opening) the packed-poly evaluation
→(ring-switch) `μ` →(pre-sumcheck) the forest exit claim →(merged forest)
`roots = α^{v_c}` →(generator binding, `v_c < ord α`) the integers `v_c`
→(read-off) `P(r)`.

## The mod-q MLE opening (the headline API)

Full-width `F_q` row weights (`w_b = eq(b, r₁) mod q`, ~100 bits) would overflow
the exponent's injectivity budget. `prove_mle_eval_mod_q_ligerito` chunks them:
width `c_w = 127 − t − W`, `L = ⌈q_bits/c_w⌉` chunk-weight sets. Each chunk `l`
runs its **own** merged forest + pre-sumcheck on the SAME committed `P`, yielding
a residual claim at its own point; every chunk-fold satisfies
`u_c^{(l)} < 2^{c_w+t+W} = 2^127` (verifier range check — `ChunkRange`) and binds
injectively via `α^{u} = root`. The `L` residual claims are `η`-RLC'd into **one**
recursive Ligerito call (their `B(y)` bases combined with a shared `r″`), and the
verifier recombines `y = Σ_c e_c · Σ_l 2^{c_w·l} u_c^{(l)}` in `F_q` in the clear.
Both the range check and the generator check are load-bearing.

## Extension-field evaluation (paper `c:core_iop`, Steps 1–3)

`prove/verify_mle_eval_ext_ligerito` open `⟨π_q(bits), v⟩ = μ ∈ K` for an
**extension** evaluation field `K = F_q[X]/(h(X))` of degree `e ≥ 2` (e.g. the
Goldilocks or BabyBear extensions an outer PIOP samples its point from). The
canonical lift of the row weights `π_canon^{-1}(v^{(1)})` is then a vector of
integer *polynomials* (coefficient-wise lift to `[0, q)`, degree `< e`), which
cannot ride one exponent, so the paper's Step 3 collapses the claim onto a
random prime field first:

1. **Step 1** — the prover sends, per module-basis coordinate `d`, the exact
   integer chunk folds `μ_{c,d}^{(l)} = ⟨bits_c, chunk_l(coords_d)⟩ < 2^127`
   (base-`2^{c_w}` digits of the coefficients of `μ_c ∈ ℤ[X]`; `e·L₁·2^s`
   values, `L₁ = ⌈q_bits/c_w⌉`). These are **not** GKR-certified — only
   absorbed into the transcript (with the same `< 2^{c_w+t+W}` range check on
   the verifier side, `ExtChunkRange`).
2. **Step 3** — both sides sample from the transcript a **random prime**
   `q' ∈ [2^{bits−1}, 2^{bits})` (`ExtProjParams::prime_bits`, default 100;
   rejection sampling + Miller–Rabin with a base-2 pre-filter and
   `mr_rounds` transcript-derived bases, default 64 → composite acceptance
   `≤ 4^{-64}` per grinding query) and a point `α' ∈ F_{q'}` (256-bit
   reduction, bias `≈ 2^{-156}`), then project the weights:
   `γ_b = (Σ_d coords_d[b]·α'^d) mod q'` (`ext_proj::projected_row_weights`,
   Montgomery-backed). This **replaces the plain `π_q^{-1}(v^{(1)})` lift**
   of the prime-field path.
3. **Steps 4–6** — the ordinary mod-`q'` opening runs on `γ` unchanged
   (`L₂ = ⌈prime_bits/c_w⌉` chunk forests, pre-sumchecks, ring-switch, one
   η-batched Ligerito call — `IntEvalRsLigExtProof.base` is a plain
   `IntEvalRsLigModQProof`).

The verifier accepts iff the mod-`q'` core accepts **and** (A) per column
`Σ_l 2^{c_w·l}·u_c^{(l)} ≡ μ_c(α') (mod q')` (`ExtCongruence`) — the
certified integer folds pin the Step-1 polynomials: a lie is a nonzero
difference polynomial of degree `< e` with `~127`-bit coefficients, surviving
the random `(q', α')` with probability `≈ B/(log q'·|𝒫|) + (e−1)/q'`
(paper `l:reduction_lemma`; ~2^-90 at the defaults) — **and** (B)
`Σ_c v_c^{(2)}·π_canon(μ_c) = μ` over `K` (`ExtReadOff`), computed through
the generic evaluation ring `R` with a caller-supplied image of the module
basis (`basis[d] = ψ_M(X^d)`), coordinate by coordinate via the same
`recombine_read_off` as the prime path.

Cost relative to a prime-field opening at the same shape: the forest side is
the mod-`q'` opening (same `L₂` as a ~100-bit prime claim); the extension
adds only ONE fused `e·L₁`-set integer fold pass (`fold_values_bits_multi`,
unrolled in groups of ≤ 4 so the bit scan is shared), `e·L₁·2^s`
width-packed `~(t+W+q_bits)`-bit transmitted folds, and the `O(2^t·e)`
projection (one plain×Montgomery multiply per term — a plain operand times
a Montgomery-form power lands the product back in plain form, so there are
no per-element domain conversions; the same trick runs the verifier's
congruence loop). Measured at n = 26 (t=16, s=10, W=1, Goldilocks², M4):
prover +~10 ms on 146 ms, verifier +~0.2 ms on 3.1 ms, proof +20.6 KiB on
142.7 KiB. For `e = 1` use the base `prove/verify_mle_eval_mod_q_ligerito`
(Step 3 is the identity there; the ext entry points reject it).

## F₂-virtualization (paper `s:to_f2_virtual` / `c:virtual_iop`)

`prove/verify_mle_eval_mod_q_ligerito_virtual`: the mod-q claim is about the
DERIVED vector `h = M·f` for a public sparse `F₂` map `M`
(`f2map::F2CellMap`, canonical CSR over flat bit cells, digest-bound into
the transcript statement together with the commitment root, both
geometries, the row weights, `q_bits`, and `α`); only `f` is committed.

1. The prover materializes `h`'s bit rows and runs the ORDINARY per-chunk
   machinery on them (`p_h` geometry): chunk folds `us`, merged forests,
   pre-sumchecks. Nothing touches the oracle — the verifier recomputes the
   roots from the sent `us` and derives per-chunk residual claims
   `ĥ(pt_l) = μ_l` exactly as in the base path.
2. Transpose: with fresh `η`s, `h := Σ_l η_l μ_l = ⟨W, f⟩_K` where
   `W = Σ_l η_l·Mᵀ eq(pt_l)` over `f`'s cells (XOR is addition in the
   char-2 commitment field, so the transpose is exact). This is the
   paper's arbitrary-inner-product setting `⟨w, a⟩_E = h` (appendix
   "Bilinear Embeddings" → "Extension openings") with `F = F₂`, `E = K`,
   `w = f`, `a = W`, solved by the dual-basis embedding: `W_map = Id`
   (the commitment packs cells in the monomial basis), `H = c₀`, and `A`
   the GHASH dual basis (`src/dual_basis.rs` — reversal of the `v ≥ 1`
   coordinates plus seven XOR corrections; `f₀ = 1`, no inversion).
3. Batching protocol (one round): the prover sends the 128 dual-packed
   plane inner products `h_i = ⟨pack(f), A(a_i)⟩_K` (bit-planes `a_i` of
   `W`; 2 KiB, tag 0x48). The verifier checks `Σ_i c₀(h_i)·X^i = h`,
   draws the zero-evader `ρ` (7 challenges eq-expanded to `K^128`, the
   ring-switch `r″` convention), and both sides reduce to ONE native
   Ligerito inner product `⟨pack(f), a′⟩ = h′ = Σ_i ρ_i·h_i` with
   `a′(y) = Σ_v Φ_ρ(W_{(v,y)})·A(e_v)`. The F_q read-off is the base
   recombination over `h`'s columns.

Neither `W` nor any `f`-side table is materialized: with
`E_r = Σ_l η_l·eq_{bits(r)}(pt_l)` per derived cell, char-2 linearity
gives `h_i = Σ_r bit_i(E_r)·G_r` (`G_r = Σ_{j∈row(r)} pack(f)[y_j]·A(e_{v_j})`)
and `a′(y) = Σ_r Φ_ρ(E_r)·Σ_{j∈row(r), y_j=y} A(e_{v_j})`, so the prover
streams `M`'s nonempty rows twice (`O(#rows + nnz)` K-ops each) and the
verifier builds the SAME dense `a′` once (`O(#rows + nnz + 2^{m_p})`
K-ops, `2^{m_p}` = `f`'s pack count) and answers the succinct Ligerito
residual hook by MLE-folding it.

Identity fast path (`virtual_id_fast_eligible`): when `M` is the
identity (`F2CellMap::is_identity`, cached at construction) and both
grids share one row layout (`t + log₂W` and `s` equal), `h`'s bit rows
ARE `f`'s and every per-chunk claim is a claim on `f`'s own flat
bit-MLE — the prover skips apply/pack and both batching passes and runs
the BASE opening after the same statement absorb, emitting the
`VirtOpenTail::Eq` tail (per-chunk `s_v` ring switch, η-batched
Ligerito) instead of `VirtOpenTail::Batch`. `F2Z_VIRT_ID_FAST=0` opts
out (prover-side only): on an eligible statement the verifier accepts
EITHER tail — each is an individually sound reduction of the same claim
— while on any other statement the eq tail is a shape error (there the
base verification would bind `f̂(pt_l)` where the claim is
`(M·f)ˆ(pt_l)`). The codec carries one tail tag byte (0 = batch,
1 = eq) between the chunk section and the tail.

Soundness mirrors the base path plus two fresh `2^-128`-class terms: the
η-batch (`L/|K|`) and the batching protocol's zero-evader
(`ε ≤ LOG_PACKING/|K|` — Ligerito binds `⟨pack(f), a′⟩ = h′` for the
committed `f` with `a′` statement-derived, so wrong `h_i` survive the
ρ-batch with probability ≤ ε, and true `h_i` make step 3 exactly
`Σ_l η_l μ_l = ⟨W, f⟩` by the bilinear-embedding identity). The
derived-side pipeline errors are the base errors with `h := M·f`.
Structured maps (XOR of committed columns, taps) should keep using
the dedicated machinery below; this entry point is the fully general one.
Pinned by `tests/virtual_open.rs` (direct-vs-virtual agreement, both chunk
regimes, tamper battery including the batching message) and the
brute-force embedding tests in `src/dual_basis.rs` (Gaussian dual solve,
Hankel form, plane decomposition, per-pack batched basis).

## CM-AND: an R1CS with a virtual block (paper `\Relation_CM`)

`piop::spartan::cm` wires a complete R1CS through the virtualization
path — the paper's NP-completeness gadget as a running system. Per gate,
32-bit words `x`, `y`, `z`, `w` satisfy ONE linear constraint
`x + y − w − 2z = 0` over `F_q` (exact over ℤ, all values < 2^33); since
`x + y = (x⊕y) + 2(x∧y)` bitwise-exactly, the constraint FORCES
`z = x ∧ y` as soon as `w = x ⊕ y` bit-for-bit. That XOR identity is
imposed STRUCTURALLY, not proven: the commitment carries only the
`x`/`y`/`z` bits (`f`), and the canonical `F2CellMap` of the layout
derives every `w` bit as the XOR of the matching `x`/`y` bits inside
`prove/verify_mle_eval_mod_q_ligerito_virtual`. The Spartan side is
`A = B = 0` with one `C` row per gate — the pure CM shape (ℤ-linear
constraints composed with `F₂`-linear derivation), NP-complete per the
paper's `r:CM_is_NP_complete`.

Pipeline: the assignment `[const | x | y | z | w]` (five blocks padded
to eight, `gate_vars + 3` claim coordinates) runs ordinary Spartan with
the commitment root + layout + MAP DIGEST bound into the statement
pre-challenge; `bitify_cm_and_claim` transposes the terminal claim into
row/column weights over the DERIVED grid `h` (the adjoint of the four
32-bit reconstructions, constant block subtracted publicly); the virtual
opening does the rest. Entry points mirror the u32 bridge:
`commit_cm_and_witness` / `prove_cm_and_f2z` / `verify_cm_and_f2z`
(production, ≥ 2^15 gate slots) plus `_with_config` variants for
sub-audit test shapes. `IntEvalRsLigVirtProof` now carries the exact
byte codec (`to_bytes`/`from_bytes`, canonical + tamper-rejecting, base
forest-layer encoding). Pinned by `tests/cm_virtual.rs` (honest
roundtrips, codec, FALSE relation with consistent bits rejected, honest
relation with INCONSISTENT committed bits rejected, statement mismatch,
production gating); bench `benches/cm_and.rs`
(`F2Z_CM_EXPONENTS`/`F2Z_BENCH_REPS`).

Accounting per gate: derived grid 128 bits (x|y|z|w), committed 96 live
bits — the `w` block rides free. Both grids share one shape (the 4:3
saving pads back to the power of two); XOR-heavier relations (the SHA-256
CM arithmetization) are where the derived/committed gap widens.

Measured (M4, 2^15 gates, dual-basis ring switch): prove ~68 ms
(Spartan 13 | bitify 6 | virtual F2Z 48: apply 6 + forest 11 +
`h_i` fold 13.3 + `a′` build 9.3 + Ligerito 6.0), verify
~23 ms (`a′` build 9.3 ms — the verifier's whole
`M`-dependent cost; was 4.7 for the bridge's `Ŵ(ρ)`), proof
113.2 KiB (−2.6 KiB vs the bridge protocol:
the bridge sumcheck and the `s_v` message are gone, the 2 KiB `h_i`
message arrived). Memory: no `W` table, no coefficient table, no `f`
bit table, no bridge MLE copies — the passes stream `M`'s rows; the
only sizable transients are the per-range `a′` partials and one 64 KB
Φ table (a premultiplied per-slot table variant — 128 × 64 KB, fusing
the `A(e_v)` product into the gather — measured ~1.5× SLOWER despite
fewer ops: it evicts the single L1-resident table; values are identical
either way, so it stays a pure schedule choice). Remaining levers: the
`h_i` fold's MFR scatter (~115 ops/row — a plane-transpose or wider
block variant), fusing the `a′` build with the Ligerito round-0 message
(`fill_phi_basis_round0`-style), and closed-form `E_r` streaming for
eq-structured maps (taps-style). The identity fast path at the same
shape (t=15, s=7, W=1, embedded config) measures prove 19.3 ms /
verify 2.2 ms — vs 49.7 / 11.2 for the batch tail forced (`F2Z_VIRT_ID_FAST=0`)
on the same identity instance.

## Mod-q RLC claim families (EXPERIMENTAL)

`prove/verify_mle_eval_mod_q_ligerito_rlc_family`: k claims
`MLE[INT(a_i)](r_i) = c_i ∈ F_q` where each `a_i` is a public F₂-linear form of
j committed UAIR columns (per position) and all claims share the COLUMN point.
After the statement (root, forms, all `(c_i, w_i)`) is absorbed, γ's drawn from
the transcript collapse the k weight functions into one 2^j-CASE weight
`W_b(m) = Σ_i γ_i·w_{i,b}·L_i(m) mod q`; ONE forest per chunk binds
`α^{W_b^{(l)}(m(pos))}` by a 2^j-case leaf select on the committed bits (the
derived vectors never materialize), the presum splits into `2^j − 1` channels
`R_S = eq ⊙ τ_S` against the bit monomials `Π_{i∈S} m_i` (`τ_S` the char-2
subset zeta-transform of the case α-powers), and the `|S| ≥ 2` residuals
discharge through one η-batched degree-(j+1) eq-sumcheck exiting at committed
openings. The chunking, range checks (`u < 2^{c_w+t'+W}`), generator check and
the η-RLC'd single Ligerito call carry over verbatim from the base scheme. The
construction, its soundness obligations and boundaries live in
`docs/rlc-family-note-prompt.md` (companion prototyping plan:
`docs/rlc-family-proto-prompt.md`); measured A/B vs the virtual-XOR and
independent-claims baselines in the README's 2026-07-26 notes. The j = 2
discharge runs as a forest leaf layer (`M_i = 1 + ¬m_i·1`: complement bits +
all-ones τ are the driver's leaf-bit-affine shape) in two phases with a β
handoff; with it the XOR triple measures **1.74–1.87× a single claim at
n = 24–28 vs 2.5–3.7× for the batched-vx path and ~3× for independent
proofs** — the predicted ~1.6–1.8×-vs-3× regime, with 26–43 % smaller
proofs. Identically-zero presum channels (legitimate degenerate families —
e.g. pure-XOR families, whose `α^W` factors through the XOR and kills the
AND channel) are ELIDED: both sides derive the active channel set per chunk
from the public case weights, and the discharge/ω/ring shapes follow it.
Fixed `q = 2^100 − 15`; NOT wired into `proof_codec`; the virtual-XOR path
is untouched and remains the comparison baseline (and stays the right tool
for LONE claims — the family wins from k ≥ 2).

The SHARED-POINT entries
(`prove/verify_mle_eval_mod_q_ligerito_rlc_family_shared_point`) specialize
the family to the motivating deployment — ALL claims at ONE point `r`:
claims are `(form, value)` pairs against ONE `row_weights_q`. Duplicate
forms are canonicalized away (first occurrence wins; a repeated form with a
different claimed value is an unsatisfiable statement and rejects as
Shape), so `k ≤ 2^j − 1` and the maximal family is the full XOR-closure of
the j columns. The case-weight table is rank-1 —
`W_b(m) = w_b·Γ(m) mod q` with `Γ(m) = Σ_i γ_i·L_i(m)`, 2^j values total
(`rlc_gamma_cases` / `rlc_case_weights_shared_point`, pinned equal to the
general build) — and the Fiat–Shamir statement absorbs the ONE weight
vector (domain tag 0x41, `2^{t'}` weight words instead of `k·2^{t'}`):
deliberately a DIFFERENT transcript from the general 0x40 path on the same
claims (cross-verification rejects both ways; pinned by tests). Downstream
of the γ draw both entries run the same core, so the general path's bytes
are untouched. Measured (README 2026-07-27 shared-point note): the maximal
families amortize hard — k = 15 proves at 0.14–0.36× a single claim PER
CLAIM at n = 22–26 — and at that width the discharge, not the forest,
dominates the prove (~20 ms per active |S| ≥ 2 channel at n = 26; the
forest stays ~flat in k). CLI presets: `--family j2s|j3s|j4s`.

## Proof-stream serialization

`IntEvalRsLigModQProof::to_bytes` / `from_bytes` (`src/proof_codec.rs`). Per
chunk, the zinc-side parts — the merged forest (its `Vec<MergedLayer>` of an
optional `sc_x` and a `sc_c` sumcheck proof plus the closing child pair), the
chunk folds `u`, the pre-sumcheck (`MultiDegreeSumcheckProof`), and the
ring-switch `s_v` — are written field by field: `u64` lengths, 16-byte
`GF(2^128)` words and `u128`s little-endian, and the sumcheck proofs via the
crate's length-prefixed `Transcribable` encoding. flock's serde `LigeritoProof`
is appended as a **length-prefixed `bincode` 1.3 blob** (bincode 1.3 being
flock's own pinned encoder). The codec is canonical (re-serialization is
byte-identical) and rejects any tampered byte — the stream fails to decode, or
the reconstructed proof fails verification.

`IntEvalRsLigExtProof::to_bytes` / `from_bytes` wrap the same machinery for
the extension-field opening: the Step-1 fold vectors first — each with its
all-zero tail trimmed AND packed at the vector's minimal little-endian byte
width (one width byte + `n·width` bytes; the decoder rejects an unnecessary
width, so the encoding stays canonical) — then the embedded base proof as
one length-prefixed blob. Honest folds are `~(t+W+q_bits)`-bit, so the
width packing beats fixed 16-byte cells by ~1/3 and sparse witnesses
shrink further.

## Optimization inventory (as extracted)

All semantics-preserving and pinned by tests; measured on Apple M4, with history
in the zinc-plus ledger:

- **Merged bit-affine lazy forest** (`merged_forest`): all `2^s` (per chunk) or
  `N·2^s` (batched) product trees share one lazy pass whose leaf layer
  `1 + bit·τ_b` is never materialised — leaves are consumed straight from the
  packed committed bits via branchless case-LUT sumcheck rounds over
  tree-shared `w·τ` product tables. Two byte-identical schedules (L/4 default,
  L/8 via `F2_FOREST_SCHEDULE=l8`), both pinned equal to the eager forest.
- **Eq-factored driver**: eq factors are never materialised as multiplicands or
  folded — per-round suffix tensors + prefix scalars; byte-identical to the
  generic sumcheck over the materialised comb.
- **`WideMulAcc` delayed reduction**: round-coefficient products accumulate as
  unreduced 256-bit carryless sums (reduction is F₂-linear, so one reduction per
  accumulator per round is exact).
- **NEON-resident GF(2^128)** (aarch64): schoolbook 4-PMULL 128×128 product +
  3-PMULL fold reduction mod `X^128+X^7+X^2+X+1` (`g = 0x87`), 2-PMULL squaring
  (char-2 cross terms cancel), vector-register wide accumulators — no NEON↔GPR
  domain crossings on any multiply path; every other target keeps a scalar-word
  pipeline bit-identically (`neon_mul_matches_scalar_pipeline`).
- **flock hot paths**: the additive-NTT commit encode, the Ligerito
  folds, and the SHA-256 Merkle all run `flock-core`'s optimized (NEON) code; a
  single Fiat–Shamir chain is driven across the zinc and flock layers through a
  `Challenger` bridge (`ZincChallenger`).

## Parameter guidance

- **Ligerito config** (`lig_configs`): the ad-hoc `LigConfig::Adhoc { log_batch,
  log_inv_rate }` builds a `default_config` (unique-decoding query counts, no
  grinding/OOD); the embedded audited profiles route through
  `LigeritoSecurityConfig`. `log_inv_rate = 2` (rate 1/4) with `log_batch = 2`
  is the tested default; query counts are `default_config`'s own
  unique-decoding derivation.
- **Proof size** is shape-determined; for W=1 the optimum keeps `s* ≈ 7–8` and
  folds `t* ≈ 0.6n` of the variables; the mod-q chunking removes the magnitude
  cap that would otherwise force small `t` at full-width weights. Measured sizes:
  43 KiB (n=16) / 75 KiB (n=18) for a single `F_q` opening (see the README).
- **Soundness** of the exponent binding: `≤ V/(2^128 − 1)` per Schwartz–Zippel
  with `V` the maximum fold magnitude; `α` must generate `K^×` (`is_generator`
  checks against the factorization of `2^128−1`).

## Trust and review status

The construction and this implementation lineage were developed and measured in
zinc-plus; the extraction preserved code verbatim where possible (module paths
rewritten, host glue removed). The test suite carries: the merged-forest
lazy-vs-eager byte-identity in **both** schedules, GKR binding, the mod-q
Ligerito roundtrip across chunk regimes with tamper / range / generator
rejections, the ring-switch / tensor-algebra pins, the NEON/scalar field
equivalence, and the serialization roundtrip + tampered-byte rejection.

**Pre-production caveats.**

- **flock Merkle leaf/node domain separation** is not yet implemented upstream:
  flock's `merkle` module does not domain-separate leaf vs internal-node
  hashing (its module note flags it as a "micro-benchmark module, not production
  code"). F2Z inherits flock's commitment/Merkle verbatim; the current Merkle
  binding should be treated as pre-production until flock ships the fix.
- The scheme is **not zero-knowledge** (the Ligerito opening reveals queried
  committed rows).
