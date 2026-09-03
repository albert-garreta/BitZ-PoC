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

`prove/verify_mle_eval_mod_q_ligerito_virtual` opens a claim about a derived
vector `h = M f` against a commitment to `f` alone. The public binary map is
stored once as the crate-wide canonical `SparseMatrix<bool>` in CSC form and
validated by `PreparedVirtualMap`. The prepared wrapper rejects explicit
`false` coefficients, caches exact-identity detection, and hashes the CSC
shape, column offsets, and row indices into the existing virtual statement
domain.

Witness synthesis supplies both packed `f` rows and packed `h` rows. The
virtual prover consumes `h_rows` directly; production proving never computes a
forward `M f` product. Only `f` is committed, and the verifier never receives
`h`.

1. The prover runs the ordinary per-chunk forest and pre-sumcheck pipeline on
   supplied `h`, leaving residual claims `ĥ(pt_l) = μ_l`.
2. Fresh `η_l` combine those claims. For derived cell `r`, define
   `E_r = Σ_l η_l eq_r(pt_l)`; for source cell `j`, the CSC gather computes
   `W_j = Σ_{r:M[r,j]=1} E_r`, i.e. the needed `Mᵀ` action.
3. The prover sends the 128 dual-basis plane inner products `h_i`. After the
   zero-evader `ρ`, both sides reduce to one native Ligerito opening with
   `a'(j>>7) += Φ_ρ(W_j) A(e_{j&127})`.

Neither `W` nor a dense transposed matrix is materialized. The `h_i` pass uses
parallel source-column chunks with 128-element partial accumulators. The shared
prover/verifier `a'` builder assigns each 128-column source pack to one output,
so no dense per-worker partial vectors or synchronized scatters are needed.

Both passes obtain their weights from a per-run engine (`VirtColumnWeights`)
rather than folding `E_r` per nonzero. For a power-of-two tensor repetition
(`RepeatedVirtualMap`, `global = local·2^k + instance` — the SHA batch shape)
the eq tensor factors over the instance/local bit split:
`W_{(lc,inst)} = Σ_l eq_inst_l[inst]·S_{l,lc}` with
`S_{l,lc} = η_l·Σ_{lr∈localcol(lc)} eq_loc_l[lr]` precomputed once in
`O(L·(local_rows + nnz_local))` field ops. One weight then costs `L`
multiplies instead of the streamed fold's `L·deg` — per nonzero, per pass,
on BOTH prover and verifier — and a local column with all `S_{·,lc} = 0`
skips its whole 128-column pack in one check. Field associativity and
distributivity make every value bit-identical to the streamed fold
(`virtual_pack_weights_match_generic`), so transcripts are unchanged; maps
without a power-of-two repetition keep the streamed fold as the engine's
generic arm.

For the packed-source repetition (`PackedSourceRepeatedVirtualMap`,
`global = 1 + instance·w + local`, the SHA-256 product layout) the PROVER
side of both batching passes goes one step further (`virt_batch`,
`F2Z_VIRT_PLANES=0` opts out). The factored weights `W_{(i,c)} = e_i·s_c`
are never formed: by the dual-basis identity
`bit_b(e·s) = c₀(e·s·A(e_b)) = Σ_a bit_a(e·A(e_b))·bit_a(A⁻¹s)`, the instance
factor separates from the local-column factor, so with the local plane
packings `R_a(y, i) = Σ_{v ∈ instance i} bit_a(A⁻¹s_{c(y,v)})·A(e_v)` —
bit-plane transposes of the local columns, shared by every instance of the
same pack phase (16 phases for SHA) —
`h_b = Σ_i Σ_a bit_a(e_i A(e_b))·Q_i[a]` with `Q_i[a] = Σ_y P[y]·R_a(y, i)`,
and `a'(y) = Σ_i Σ_a ρ'_{i,a}·R_a(y, i)` with
`ρ'_{i,a} = Σ_b ρ_b·bit_a(e_i A(e_b))`. Each source cell then costs ONE
unreduced fixed-scalar GF(2^128) multiply per pass (4 shuffle-free PMULLs
into a 2-limb accumulator, one fold per accumulator) instead of a weight
multiply, a basis multiply and a 128-way bit scatter (resp. 16 table
gathers); the per-instance `O(128²)` read-offs (`Q_i` through 16 byte tables,
`ρ'_i` through 16 byte-indexed rows of the ρ-only table
`C_{u,a} = Σ_b ρ_b·bit_a(X^u A(e_b))`) amortise over the instance width. The
pass also emits flock's Ligerito round-0 pair, as the direct path does. Exact
field identities only, so `h`, `a'` and the transcript are bit-identical
(`virtual_planes_match_cellwise`); the verifier's reduction is untouched.
Measured on the `sha256_compressions` bench (M4, 4 P-cores, λ=100
medians): step 5.3 (ring switch + Ligerito) 149 → 16 ms at 2^12 and
646 → 63 ms at 2^14 (`h` 365 → ~30 ms, `a'` 243 → ~26 ms, Ligerito proper
~10 ms; the forest now dominates the opening at >95 %).

When the prepared map is exactly the identity and both tensor layouts agree,
the prover may emit `VirtOpenTail::Eq` and run the base opening directly on
committed `f`. Otherwise it emits `VirtOpenTail::Batch`. The verifier accepts
the eq tail only for an eligible public statement. `F2Z_VIRT_ID_FAST=0` forces
the general batch tail for diagnostics.

The statement transcript order is unchanged: absorb the commitment and
geometries, canonical CSC map digest, row weights, `q_bits`, and `α`; run the
chunk protocol; draw `η`; absorb `h_i`; draw `ρ`; then run Ligerito. The CSC
digest intentionally replaces the former CSR digest without a version bump, so
old virtual proofs are not compatible.

## CM-AND: an R1CS with a virtual block (paper `\\Relation_CM`)

`piop::spartan::cm` is the current virtualized client. Per gate, 32-bit words
`x`, `y`, `z`, and `w` satisfy the linear constraint
`x + y - w - 2z = 0`. Synthesis records `w = x XOR y`, so this constraint
forces `z = x AND y`.

The committed grid `f` contains the `x`, `y`, and `z` bits. The synthesized
grid `h` contains `x`, `y`, `z`, and `w`. `cm_and_map` constructs its public
map directly as CSC columns:

- an x or y source bit feeds its identity row and the matching w row;
- a z source bit feeds only its identity row;
- padded source slots have empty columns.

`project_cm_and_witness` produces one `EvaluatedSpartanAssignment` containing
assignment `h` and products `Ah`, `Bh`, and `Ch`, plus packed `h_rows`, from
the same `CmAndWitness`. The unchanged Spartan PIOP proves the R1CS over `h`.
Its terminal assignment claim is bitified and passed to virtual F2Z, which
binds it to the commitment to `f` through the public CSC map.

Canonical u32 multiplication uses `U32MulProof`: its runtime-prime Spartan
component applies the fixed K=3 univariate-prefix skip and its terminal claim
is opened by `IntEvalRsLigModQProof`. CM pairs ordinary Spartan with
`IntEvalRsLigVirtProof`; its sealed `SpartanF2zProof<S, M>` mode prevents a
direct opening from being passed to a virtualized verifier.

SHA-256 witness synthesis and affine constants are intentionally outside this
refactor; a future client only needs to supply synthesized `f`, `h`,
`Ah/Bh/Ch`, and a prepared public CSC map.

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
