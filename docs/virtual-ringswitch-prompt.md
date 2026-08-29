# Session prompt: replace the virtual opening's bridge sumcheck with the generalized (arbitrary-weight) ring switch

You are working in `/Users/albertgarretafontelles/f2z-pcs` (invoke the
`f2z-pcs` skill; for paper questions also `f2z-pcs-paper`). Your task is a
**transcript-changing rework of the F₂-virtualization opening**: delete
the degree-2 "bridge sumcheck + point opening" tail of
`prove/verify_mle_eval_mod_q_ligerito_virtual` (src/ligerito_flock.rs)
and replace it with the **ring switch for arbitrary inner products** from
the paper's appendices — the batching protocol of
`paper/main.tex` §"Bilinear Embeddings" → "Extension openings"
(`\label{a:ring_switch_remco}`, the `Batching protocol for extension
opening` construction), instantiated per the §"Coefficient projection"
theorem, with §"Ring switching via Galois orbits"
(`\label{a:ring_switch_1}`) as companion background. Read those paper
sections FIRST — they are the normative spec; everything below is the
worked mapping onto this codebase.

## Why

Today the virtual opening reduces the transposed claim `⟨W, f⟩` (W an
arbitrary 𝔽_{2^128}-weight vector over f's bit cells — no eq structure)
to a point claim via a degree-2 sumcheck over all `t+s` cell variables,
then opens the point with the standard eq-based ring switch
(`prove_rs_open_ligerito`). That bridge exists only because the eq-based
ring switch cannot ingest non-factoring weights. The appendix shows the
restriction is unnecessary: pack the weights under the **dual basis** and
one native Ligerito inner product does the whole job. Expected wins at
the 2^15-gate CM shape (see `examples/cm_probe.rs`, `OBLONG_PROFILE=1`):
kill `mqv:sc` (~17 ms), the `mqv:wtbl` Gf table + `to_mle` copies and
their 2×64 MB transient, and the separate point-opening ring
(`mqv:open`'s ring-switch part); proof loses the bridge's ~1.4 KB.
Target: virtual F2Z ≈ 58 → ~35–40 ms prove; verify stays O(nnz(M))-class.

## The protocol (paper, transcribed to our objects)

Setting: `f ∈ F₂^{n·d}` committed packed (`d = 128` bits per
`K = F_{2^128}` element, `n = 2^{m_p}` packs; pack basis = the MONOMIAL
basis: bit `v` of a cell ↔ `X^v` — confirm against
`commit_rs_flock_from_rows`'s doc and `repack_leaf_bits`). After the
per-chunk forests + pre-sumchecks and the η-batch, the claim is

    Σ_l η_l·μ_l  =  ⟨W, f⟩_K,     W := Σ_l η_l · Mᵀ eq(pt_l) ∈ K^{n·d}.

This is exactly the appendix's `⟨w, a⟩_E = h` with `F = F₂`, `E = K`,
`w = f`, `a = W`, `h = Σ_l η_l μ_l`. The batching protocol:

1. Decompose `a = Σ_{i∈[d]} a_i·β_i` — the `a_i ∈ F₂^{n·d}` are the
   F₂-coordinate planes of the weights (bit `i` of each weight cell).
2. Prover sends `h_i = ⟨pack(f), A(a_i)⟩_K` for `i ∈ [d]` — d = 128
   K-elements, 2048 B, the exact size class of today's `s_v` message.
   `A(a_i)` means: pack plane `a_i` under the DUAL basis (below).
3. Verifier checks `h = Σ_i H(h_i)·β_i`.
4. Verifier samples an ε-zero-evader `ρ ∈ K^d`; both sides reduce to ONE
   claim `⟨pack(f), a′⟩_K = h′` with `a′ = Σ_i ρ_i·A(a_i)`,
   `h′ = Σ_i ρ_i·h_i` — this is the single flock Ligerito call
   (`recursive_prover_with_basis` with basis `a′` and target `h′`),
   replacing both the bridge and the point opening. RBR error of the
   reduction: ε (the zero-evader parameter) — account it next to the
   existing η-batch term.

The concrete embedding for GHASH (§Coefficient projection theorem, with
`f(X) = X^128 + X^7 + X^2 + X + 1`, so `f₀ = 1` — no inversion):
`W = Id` (the commitment already packs in the monomial basis),
`H = (1,0,…,0)` (project coefficient 0 of a K-element), and `A` is the
bordered upper anti-triangular Hankel matrix `A_{00}=1`,
`A_{ij} = −f_{i+j}/f₀` for `i,j ≥ 1`, `i+j ≤ d`. For GHASH this is **a
reversal of the i>0 coordinates plus seven XOR corrections** (the paper
says so explicitly; derive the exact seven from the theorem and TEST
against a brute-force dual-basis solve — see Validation).

## Derived identities you must re-verify before relying on them

These were derived in-session (2026-08-28), not stated verbatim in the
paper; check each with a brute-force unit test at tiny sizes:

- Plane decomposition: `⟨W, f⟩_K = Σ_i β_i·⟨W^{(i)}, f⟩_{F₂}` where
  `W^{(i)}` is bit-plane i of the weights — valid because f's entries are
  bits and K-addition is coordinatewise F₂-addition.
- Per-pack form of the batched basis: writing weight cells as
  `W_{(v,y)}` (in-pack slot v, pack y),

      a′(y) = Σ_v Φ_ρ(W_{(v,y)}) · A(e_v),

  where `Φ_ρ(x) := Σ_i ρ-combination of x's F₂ coordinates` is exactly
  the F₂-linear map the existing `phi_byte_tables`/`phi_from_words`
  machinery implements (today with `ρ = eq_r2`), and `A(e_v)` is the
  dual-basis image of slot v (column v of the Hankel `A`).
- Sparse closing (the verifier's succinct basis evaluation): because
  K-addition is F₂-addition, the CSC implementation gathers

      W_j = Σ_{r:M[r,j]=1} E_r,
      E_r := Σ_l η_l·eq_{h-cell r}(pt_l),

  then contributes `Φ_ρ(W_j)·A(e_{j mod 128})` to source pack
  `j/128`. The current on-demand implementation costs **O(L·nnz(M) +
  #cols(M))** K-ops because it reevaluates `E_r` per incidence. A future
  cached-`E` variant could trade derived-row storage for
  `O(L·#rows(M) + nnz(M))` work on higher-degree maps.
- Correspondence sanity check: specializing `a` to today's eq-tensor
  weights must reproduce the existing eq ring switch's structure
  (`s_v ↔ h_i`, `r″/eq_r2 ↔ ρ`, `ring_switch_verify`'s
  `Σ_v eq_lo[v]·s_v = μ` ↔ step 3, `fill_phi_basis ↔ a′` build,
  `tensor_eq_phi_eval ↔ succinct â′`). Understanding this equivalence is
  the best guard against convention bugs (bit order of `Gf::words()`,
  basis order, transpose direction of `transpose_bits_128`).

## Prover schedule (suggested; measure with the probe)

- `h_i` fold: one pass over the weight cells. Per cell `(v,y)`: compute
  `b_y·A(e_v)` (one K-mul; `b_y` = the packed witness element from
  `hint_f.p_msg`) and deposit it into accumulator `i` for every set bit
  `i` of `W_{(v,y)}` — the `sv_scalar_accum` / `sv_fold_mfr`
  (4-Russians) pattern with the scan running over the WEIGHT's bits.
  Parallel per-chunk partial accumulators, exact XOR merge.
- `a′` build: per cell, `a′(y) += Φ_ρ(W_{(v,y)})·A(e_v)` — reuse
  `phi_byte_tables(ρ_table, scale)` per slot (fold `A(e_v)` into
  per-slot tables, 128×64 KB = 8 MB, or apply Φ then one mul per cell);
  consider fusing with the Ligerito round-0 message the way
  `fill_phi_basis_round0` does (optional; plain first).
- W itself is still materialized by the existing parallel
  coefficient-table + scatter (`mqv:wcoef`/`mqv:wtbl`) — unchanged.
  The bit-plane scan reads W's cells directly; no `f_tbl`, no `to_mle`,
  no bridge tables.
- Delete: the bridge `MultiDegreeSumcheck`, `f_tbl`, both `to_mle`
  copies, and the `prove_rs_open_ligerito` call. The Ligerito call is
  now made directly (mirror how `prove_mle_eval_mod_q_ligerito` calls
  `recursive_prover_with_basis` — same `pc`, `hint_f.prover_data`).

## Verifier schedule

- Derive `(pt_l, μ_l)` per chunk (unchanged), draw η's (unchanged),
  compute `h = Σ_l η_l μ_l`, receive/absorb the `h_i`, check step 3
  (`Σ_i H(h_i)β_i = h`; with `H = c₀`-projection this is: assemble the
  K-element whose coordinate-i is `c₀(h_i)` and compare — one line),
  draw ρ, compute `h′`.
- ρ convention: match the existing ring switch — draw `LOG_PACKING = 7`
  challenges and use their eq table as the 128-vector zero-evader
  (`ring_switch_prove/verify` precedent), OR draw 128 independent
  challenges. Either is sound with the right ε accounting
  (eq-tensor zero-evader: ε ≤ 7/|K|-class); pick one, document it, and
  keep prover/verifier identical.
- Closing: flock's `recursive_verifier_with_basis_succinct` takes an
  `eval_b(ris, yr_log_n)` closure returning the basis residual BLOCK
  (`â′` at prefix `ris` with all `2^{yr_log_n}` boolean tails — see how
  `verify_mod_q_lig_core` builds it from `residual_b_evals`). Write the
  sparse-map version: factor `eq_z(y)` over the (prefix, tail) split of
  y's coordinates so the whole block costs `O(nnz + 2^{yr_log_n})`, not
  `O(nnz·2^{yr_log_n})` — precompute `Φ_ρ(E_r)·A(e_v)` per nonzero once
  per verify (it is challenge-independent across `eval_b` calls) and
  per call only re-weight by the prefix/tail eq factors. This is the
  one genuinely fiddly piece; test it against a brute-force `â′`
  evaluation at random points.
- The `Ŵ(ρ)`/division step and `VirtualWeightZero` disappear (no
  division anywhere; `f₀ = 1`).

## Integration surface

- `IntEvalRsLigVirtProof`: replace `bridge: MultiDegreeSumcheckProof` and
  `open: LigOpenProof` with `hs: Vec<Gf>` (the d = 128 `h_i`) and
  `lig: LigeritoProof`. Update the codec
  (`to_bytes`/`from_bytes`, keep it canonical + tamper-rejecting; the
  old layout has no external consumers — the API is days old) and
  `validate_f2z_proof_shape`-style checks (`hs.len() == 128`).
- Transcript order: statement digest (cached in `PreparedVirtualMap` from the canonical CSC),
  chunks (unchanged), η's (unchanged), absorb the `h_i` (use
  `absorb_sv`'s tag discipline — pick a fresh tag byte, don't reuse
  0x20), draw ρ, then the Ligerito call on the same transcript. This
  CHANGES the virtual transcript — that is expected; there is no byte
  digest to preserve across this rework. After landing, re-pin.
- Callers: `piop/spartan/cm.rs` (`prove/verify_cm_and_f2z*`) and
  `tests/virtual_open.rs` compile unchanged except proof-shape mentions
  (the presum-swap tamper case and codec test need updating to the new
  fields). `benches/cm_and.rs` and `examples/cm_probe.rs` unchanged.
- Soundness comment blocks: update the big virtualization header in
  `ligerito_flock.rs` and `docs/DESIGN.md`'s two virtualization
  sections (the bridge paragraphs → the batching-protocol description +
  its ε; drop the "pack-factorization obstruction" framing in favor of
  "solved by the dual-basis embedding, paper Appendix B/C"). Note in
  DESIGN.md that the memory transient drops (no 2×16·ℓ_f-byte tables).

## Validation (all mandatory)

1. Embedding unit tests (new, in a small module or `f2map.rs`-adjacent):
   brute-force the dual basis of the monomial basis under `μ_H(x,y) =
   c₀(x·y)` for GHASH by solving the 128×128 F₂ system, and assert it
   equals the Hankel/reversal+7-corrections `A`; assert
   `⟨w, a⟩_{F₂} = H(pack(w)·Σ_v a_v A(e_v))` on random 128-bit blocks;
   assert the plane-decomposition and per-pack-basis identities on tiny
   shapes.
2. `tests/virtual_open.rs`: the direct-vs-virtual agreement tests must
   still pass UNCHANGED in meaning (same `y` accepted by both paths) —
   they pin the semantics across the rework. Extend the tamper battery:
   tampered `h_i` (both a value flip and a step-3-consistent-but-wrong
   pair), truncated/tampered codec bytes, wrong ρ replay.
3. `tests/cm_virtual.rs` battery green; full suite green
   (186+ tests).
4. Perf: `OBLONG_PROFILE=1 cargo run --release --features unchecked
   --example cm_probe -- 15` before/after; report the new
   prove/verify splits and update DESIGN.md's measured paragraph and
   the `f2z-virtualization` memory. Also one
   `F2Z_CM_EXPONENTS=15 F2Z_BENCH_REPS=3 cargo bench --bench cm_and
   --features unchecked` headline.

## House rules and traps (do not skip)

- Work in a fresh worktree off master; commit with explicit pathspecs,
  `--no-gpg-sign`; verify `git rev-parse master origin/master` agree
  immediately before any fast-forward/push (concurrent sessions use
  this checkout). Never `git add -A`.
- `CARGO_TARGET_DIR` is globally set to `~/zinc-plus/target` — override
  it per-command to a scratch dir; `./target` binaries are stale.
- flock-core is pinned by ABSOLUTE path to
  `~/flock-f2z-port/crates/flock-core` (branch f2z-k4-port). Do not
  repoint it; `~/flock` is a trap.
- Builds/benches: `RUSTFLAGS="-C target-cpu=native"`, `--features
  unchecked` for measurement; ad-hoc Ligerito configs need `m_p ≥ 8`
  (test shapes) and the production APIs gate `≥ 2^15` gate slots.
- Bit-order conventions: `Gf::words()` = two little-endian u64s, bit
  `v` of a cell ↔ coefficient of `X^v`; `transpose_bits_128` transposes
  the (slot, coordinate) 128×128 bit matrix — reuse it rather than
  hand-rolling. Flat cell index is `(c << t_w) | b` (row bits low) —
  documented at the top of `src/f2map.rs`.
- Every optimization inside the NEW protocol must be an exact
  GF/XOR reassociation; after the protocol itself is frozen, pin the
  new proof digest with `cm_probe` and keep it stable through any
  subsequent tuning.

## Definition of done

Bridge sumcheck and point opening gone; the virtual opening = forests +
pre-sumchecks + `h_i` message + ρ-batch + ONE Ligerito call; all suites
green; probe shows the expected prove win with verify still
O(nnz)-class; docs + memory updated; committed on master and pushed
(check the repo state first — see house rules).
