# F2Z design notes

F2Z realises §9 of Lev Soukhanov's char2-fieldswitch note: a one-round,
characteristic-2 Brakedown instantiation of integer-MLE evaluation. This
document is the distilled protocol + the optimization inventory of the
implementation; the authoritative long-form write-ups live in the source
repository (`zinc-plus`: `documentation/f2-int-eval-doc/`, and the
measurement ledger `documentation/f2x-sha-todo.md`).

## The problem

An integer MLE evaluation cannot be read off an `F₂` commitment by
`F₂`-proximity: the parity collapse `Σ G[j,i]·d_i mod 2` destroys the
integer information, while committing with a genuinely integer code
forfeits the cheap binary commitment. F2Z's resolution: **do not fold
through the code**. Commit the bits with an ordinary F₂-RAA Brakedown
commitment, and carry the integer row-fold **in the exponent** of
`K = GF(2^128)`, certified by a GKR grand product that touches the
commitment only through `K`-linear queries.

## Instance and protocol flow

`IntEvalParams { t, s, word_bits: W }`: data `D` is a `2^t × 2^s` matrix
of `W`-bit cells (`cell_index(b,c) = (b<<s) | c`); `t` row variables fold
with integer weights `w_b`, `s` column variables are read off with field
weights `e_c`.

1. **Commit** (before any challenge): the flat `2^s × (2^t·W)` single-bit
   matrix `M`, each row F₂-RAA encoded (rate 1/4, REP=4), codeword columns
   Merkle-hashed (Blake3), leaves packing `2^s` bits 64-per-word.
2. **Challenges**: `α ∈ K^×` (verifier-checked to be a generator) and the
   point `r = (r', r'')` giving `w_b` and `e_c`. Batch weights are powers
   `γ^c` of a single challenge (geometric, Schwartz–Zippel-sound).
3. **Fold in the exponent**: per column,
   `α^{v_c} = ∏_{b,j} [bit ? α^{w_b·2^j} : 1]`. A forest of `2^s` GKR
   grand-product trees binds every `α^{v_c}` and reduces all leaf claims
   to one shared point `ρ`. The prover sends the `2^s` integers `v_c`.
4. **Open** (one committed Brakedown opening): per-tree leaf claims batch
   under `γ^c` into `⟨m, q_rowbit⟩`, `m = Σ_c γ^c·M[c]`,
   `q_rowbit[(b,j)] = eq((b,j),ρ)·(α^{w_b·2^j} − 1)`; sampled codeword
   columns get one Merkle path + per-column proximity + the eval check
   (sound because the code is F₂-linear, so the γ-combination commutes
   through encoding).
5. **Read-off in the clear**: `P(r) = G_r · Σ_c e_c·v_c` over any
   characteristic-≠2 ring.

Soundness chain: committed bits →(opening) leaf claims →(forest)
`roots = α^{v_c}` →(generator binding, `v_c < ord α`) integers `v_c`
→(read-off) `P(r)`.

## The mod-q MLE opening (the headline API)

Full-width `F_q` row weights (`w_b = eq(b, r₁) mod q`, ~100 bits) would
overflow the exponent's injectivity budget. `prove_mle_eval_mod_q` chunks
them: width `c_w = 127 − t − W`, `L = ⌈q_bits/c_w⌉` chunk-weight sets;
the `(l, c)` chunk-forest is the batched forest with `L` virtual columns
sharing one committed `D`; each chunk-fold satisfies
`u_c^{(l)} < 2^{c_w+t+W} = 2^127` (verifier range check — `ChunkRange`),
binds injectively via `α^{u} = root`, and the verifier recombines
`y = Σ_c e_c · Σ_l 2^{c_w·l} u_c^{(l)}` in `F_q` in the clear. Both the
range check and the generator check are load-bearing.

## Optimization inventory (as extracted)

All semantics-preserving and pinned by tests; measured on Apple M4, with
history in the zinc-plus ledger:

- **Bit-affine lazy forest** (`prove_product_forest_lazy` +
  `GroupBufs::LeafBits`): the leaf layer `1 + bit·τ_b` is never
  materialised — round 1 of the leaf layer's sumcheck runs as branchless
  case-LUT subset-sums over eight tree-shared `w·τ` product tables read
  directly from the packed committed bits, and round 1's fold materialises
  the dense round-2 buffers from two shared tables. Layer-1 generation is
  fused into the first product level (`gen_layer1`).
- **Eq-factored driver** (`prove_eq_inner_sumcheck_mixed`): eq factors are
  never materialised as multiplicands or folded — per-round suffix
  tensors + prefix scalars; byte-identical to the generic sumcheck over
  the materialised comb.
- **`WideMulAcc` delayed reduction**: round-coefficient products
  accumulate as unreduced 256-bit carryless sums (reduction is F₂-linear,
  so one reduction per accumulator per round is exact).
- **Fused ILP kernels**: hand-scheduled round/fold bodies with two
  independent slot chains per iteration.
- **NEON-resident GF(2^128)** (aarch64): schoolbook 4-PMULL 128×128
  product + 3-PMULL fold reduction mod `X^128+X^7+X^2+X+1` (`g = 0x87`),
  2-PMULL squaring (char-2 cross terms cancel), vector-register wide
  accumulators (`WideGf128`), and NEON round/fold kernel bodies — no
  NEON↔GPR domain crossings on any multiply path. ~1.5× latency, ~1.9×
  streaming throughput, ~1.6× kernel-slot rate over the scalar-word
  pipeline; every other target keeps that scalar pipeline bit-identically
  (`neon_mul_matches_scalar_pipeline`, 2008 cases).
- **Transcript batching**: `2^s`-scale absorbs collapse into single
  Blake3 updates (`absorb_field_slice`); per-column batch challenges are
  powers of one squeeze (`challenge_powers`).

Not extracted (deliberately, for minimality): the flock/Ligerito recursive
opener and its merged multi-claim forest (tree index as MLE variables),
virtual-XOR / SHA-host claims, and the host proof-stream serialization.
The exit-claim shapes here are the per-tree forest's; the RAA
flat-Brakedown opener is self-contained.

## Parameter guidance

- **Proof size** is shape-determined; for W=1 the optimum keeps
  `s* ≈ 7–8` and folds `t* ≈ 0.6n` of the variables (see
  `proof_size_optimal_t`); the mod-q chunking removes the magnitude cap
  that would otherwise force small `t` at full-width weights.
- `num_openings` follows the code's distance calibration (987 for the
  deployed rate-1/4 RAA at 128-bit-class security in the source repo; the
  tests use smaller counts for speed).
- Soundness error of the exponent binding: `≤ V/(2^128 − 1)` per
  Schwartz–Zippel with `V` the maximum fold magnitude; `α` must generate
  `K^×` (`is_generator` checks against the factorization of `2^128−1`).

## Trust and review status

The construction and this implementation lineage were developed and
measured in zinc-plus; the extraction preserved code verbatim where
possible (module paths rewritten, host glue removed). The test suite
(75 tests) carries: protocol-math pins (M0 reference relations), GKR
binding and tamper rejection, opening tamper rejections, mod-q roundtrips
across chunk regimes + range/generator rejections, batched roundtrips,
proof-size formula pins, and the NEON/scalar field-pipeline equivalence.
