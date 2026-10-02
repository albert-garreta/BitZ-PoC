# BitZ logarithmic (proof of concept)

Implements the handwritten note "BitZ logarithmic" (LaTeX transcription:
`paper/notes/bitz_logarithmic.tex`) for the standalone raw claim
`⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = μ (mod q)`, with one recursive call and one
Ligerito run for both commitments. Code: `src/bitz/logarithmic.rs`, the
merged opening in `vendor/flock-mod/crates/flock-core/src/pcs/ligerito.rs`
and `src/bitz/pcs.rs`; bench: `examples/bitz_log_bench.rs`; sweeps:
`scripts/bitz_log_sweep.py`.

## What changes with respect to BitZ

Standard BitZ (paper §2.1, Step 4) sends the `ℓ₂ = 2^s` integer column folds
`μ_j = Σ_i γ_i f_ij` (`γ = π_q⁻¹(u⁽¹⁾)`); the verifier range-checks them,
checks `Σ_j π_q(μ_j) u⁽²⁾_j = μ`, and computes `g^{μ_j}` for the grand
products. Here:

1. The prover commits to `bits(μ_1, …, μ_{ℓ₂})` (one GF(2^128) packed element
   per fold: 128 bit slots, slot 127 dead) under its own Ligerito level-0
   code, and runs that commitment's Round 0 (OOD sample, ground if its
   collision bound is short of the target).
2. The verifier draws `ζ ∈ K^s`; the prover sends `e₀ = MLE[(g^{μ_j})_j](ζ)`.
3. GKR over `f`: `Π_i((g^{γ_i} − 1) f_ij + 1)` from `(ζ, e₀)` — BitZ's forest,
   unchanged.
4. GKR over the fold bits: `Π_k((h_k − 1) μ_jk + 1)` with `h_k = g^{2^k}`
   (`h_127 = 1`) from the same `(ζ, e₀)`. If the two output vectors differ,
   their MLEs differ at `ζ` except with probability `s/2^128`. Both exponents
   are `< 2^127 < ord(g)`, so `g^{μ_j}` determines `μ_j` as an integer: no
   range check on the folds (the "congruent mod q" cheat is rejected by this
   tree, see the test `cheating_fold_bits_are_rejected`).
5. Recursive call: a standard BitZ instance on the fold bits proves
   `Σ_j π_q(μ_j) u⁽²⁾_j = μ`, i.e. weights `(2^k mod q)·u⁽²⁾_j`, split as rows
   `(j mod 2^a, k)` (`t' = 7 + a`) and columns `j ≫ a` (`s' = s − a`), with
   `u⁽²⁾ = eq(r₂) mod q` factored `A[j_low]·B[j_high]`. It sends `2^{s'}` folds.
6. Openings: the claim on `f` (step 3's exit) and the two claims on the fold
   bits (step 4's and step 5's exits, batched with one challenge `β`), all in
   ONE Ligerito run (next section). `--mode log2` keeps two runs.

The verifier never reads a column fold of the top level. Its remaining
non-logarithmic work is

- `(⋆)`: the MLE of `(g^{γ_i})_i` — computing `γ = eq(r₁) mod q` (`2^t`
  mulmods), the `2^t` exponentiations, and one evaluation
  `Σ_b eq(ρ, b) eq(α, b)(y_b − 1)` (= a scalar times `ỹ` at one point);
- `(⋆')`: the same for the recursive instance (`2^{t'}` row weights and
  exponentiations), and its `2^{s'}` folds.

## One Ligerito run for both commitments

The fold bits depend on `r₁`, so they are committed at opening time, in their
own Merkle tree, which any opening must open once (its level 0: 36 queries,
≈ 12–16 KB at n = 28–30). Everything after that is shared:

- flock, section "Two-tree (merged) basis opening":
  `recursive_prover_with_basis_merged` /
  `recursive_verifier_with_basis_merged`. The fold bits `B` keep their own
  level-0 code (rate 1/32, their own queries); after their `k_B` lane folds
  their message has as many variables as `f`'s running message after some
  recursion iteration, and `f ∥ B` (selector = top variable, never folded,
  so it stays in the residual) is committed as `f`'s next level. The
  challenge `λ` that batches the two running sumcheck claims is drawn after
  that commitment. `f`'s previous-level queries and `B`'s level-0 queries
  are induced against their halves; the verifier evaluates every pre-merge
  basis on its half of the residual and `B`'s initial basis (×λ) on the
  other. The prover also makes the verifier's last two draws, so both sides
  leave the transcript in the same state (flock's existing entry points do
  not, hence `replay_verifier_tail` on the two-run path). Existing entry
  points are untouched (transcript pins unchanged).
- `merged_plan` (logarithmic.rs): `f`'s ladder up to the merge, later levels
  re-solved by the crate's per-level Johnson solver with one more message
  variable (`solve_custom_johnson_level`, factored out of
  `try_custom_johnson_ladder_bits`). `choose_branch_interleave` picks `k_B`
  so the sizes meet, cheapest level-0 opening first; only the fold bits'
  level 0 is sized (of a `lad:r0:…` ladder a single run keeps `r0`). A fold
  commitment too small to meet any level (`s ≤ 8`) is padded with zero
  columns until it does.
- `Pcs::prove_lin_pair` / `verify_lin_pair_deferred`: each claim is reduced
  (sumcheck, ring switch, Round-0 batch) under its own native schedule, then
  one merged Ligerito run; its proof and `B`'s level-0 opening are two hints.
  Both sides check the plan against the two commitments' own level 0
  (`plan_matches`).
- The transcript binds the mode and a digest of the executed chain
  (`MergePlan::digest`, in `bind` and `bind_standalone`): a one-run verifier
  rejects a two-run proof and conversely.

Proof bytes at the reference split (std / two runs / one run): n = 24: 161.9 / 177.4 / 170.5 KB; n = 26: 208.7 / 215.6 / 204.4 KB; n = 28: 271.1 / 245.1 / 235.4 KB; n = 30: 373.0 / 288.5 / 275.9 KB. One run also verifies faster (one Ligerito verification instead of two).

## Implementation notes

- Phases are traced (`log: …` prover, `v: …` verifier, `v: (*) …` and
  `v: (*') …` for the terms above) through the crate's `record_phases`.
- Native challenges run under one zero-work schedule per sub-protocol
  (`end_native` closes one); the fold-bits tree shares the fold point and has
  no schedule of its own, so its draws are unscheduled (`set_unscheduled`,
  refused unless the policy grinds nothing).
- The top-level claim is bound by its point `(q, r₁, r₂, μ)`, not by the
  expanded weight vectors (BitZ's `prove` absorbs all `2^t + 2^s` weights).
- `prove` / `verify` refuse a Round-0 claim for `f` that does not match its
  ladder (a Johnson level 0 needs it); the caller runs that round before
  `r₁, r₂`, as `prove_standalone` does.
- Fold-bits ladder: `lad:r0:k0:k_rec:final` builds a Johnson ladder with the
  crate's solver, also below `m = 20` (unaudited regime). Default
  `lad:5:3:3:7`: rate 1/32; with one run only `r0` matters.
- Recursive split default: `t' = ⌈0.6 (s + 7)⌉` (one row variable above the
  reference split), at most `t`.
- `eq_table_mod_q` (shared with standard BitZ) now prepares one Montgomery
  factor per coordinate instead of one per entry: same values, 5× faster.
  It was 80 % of `(⋆)`; the standard verifier gains the same.

## Running

```sh
# tests (completeness, tampering of every proof part, cheating provers,
# one run vs two runs, padded and edge geometries)
RUSTFLAGS="-C target-cpu=native" cargo test --release --lib bitz::logarithmic
# bench: std, log2 (two runs), log1 (one run), interleaved, at any split
RUSTFLAGS="-C target-cpu=native" cargo build --release --example bitz_log_bench
target/release/examples/bitz_log_bench 30 --t 16 --mode ab3 --reps 3 \
    [--a 6] [--ladder custom:1:4] [--mu-ladder lad:5:3:3:7] [--mu-min 14]
# a sweep over splits (one process per cell) and its table
python3 scripts/bench_gate.py run --label bitz-log-sweep --min-idle 75 --hold-seconds 20 -- \
    python3 scripts/bitz_log_sweep.py run --out PerfRuns/bitz-log-sweep \
        --n 28 30 --dt -2 -1 0 1 --threads 10 --reps 4 [--a 5 6 7]
python3 scripts/bitz_log_sweep.py table PerfRuns/bitz-log-sweep
```

## Measurements (M5, 24 GB, 2026-10-01 21:30–21:50)

`bitz_log_bench <n> --t <t> --mode ab3`: `std`, `log2` and `log1` alternate
within one process every repetition (rotating which goes first); medians of
5 (n ≤ 26), 4 (28), 3 (30); every cell gated on an idle box; default ladders
(`custom:1:4` for `f`, `lad:5:3:3:7` for the fold bits). `(⋆)` = eq table of
`r₁` mod `q` + `2^t` exponentiations + one evaluation, measured inside the
log verifier and timed in isolation at the same sizes for std (its verifier
interleaves them). The std verifier's remainder still contains its claim
binding (absorbing the `2^t + 2^s` expanded weights).

Columns: prover, verifier, `(⋆)`, verifier minus `(⋆)` and `(⋆')`, proof.

**10 threads**

| n | t:s | t':s' | arms | prove (ms) | verify (ms) | (*) (ms) | verify - (*) (ms) | proof (KB) |
|---|---|---|---|---|---|---|---|---|
| 24 | 11:13 | 11:9 | std / log2 / log1 | 20.2 / 25.4 / 25.3 | 2.61 / 2.62 / 1.94 | 0.06 / 0.14 / 0.14 | 2.55 / 2.44 / 1.78 | 273.1 / 194.3 / 182.9 |
| 24 | 12:12 | 12:7 | std / log2 / log1 | 16.6 / 21.3 / 21.1 | 1.99 / 2.27 / 2.04 | 0.09 / 0.11 / 0.13 | 1.90 / 2.07 / 1.80 | 210.2 / 185.9 / 173.7 |
| 24 | 13:11 | 11:7 | std / log2 / log1 | 16.7 / 19.2 / 20.4 | 1.94 / 2.05 / 2.12 | 0.14 / 0.19 / 0.16 | 1.79 / 1.76 / 1.88 | 176.9 / 181.3 / 173.6 |
| 24 | 14:10 | 11:6 | std / log2 / log1 | 18.3 / 20.0 / 19.7 | 2.37 / 1.92 / 1.87 | 0.22 / 0.22 / 0.22 | 2.15 / 1.55 / 1.56 | 161.9 / 177.4 / 170.5 |
| 24 | 15:9 | 10:6 | std / log2 / log1 | 20.5 / 21.3 / 20.6 | 2.91 / 1.96 / 1.89 | 0.31 / 0.33 / 0.33 | 2.60 / 1.52 / 1.46 | 153.0 / 175.3 / 165.6 |
| 26 | 12:14 | 12:9 | std / log2 / log1 | 50.2 / 60.4 / 61.9 | 4.19 / 2.39 / 2.16 | 0.10 / 0.11 / 0.13 | 4.09 / 2.24 / 1.99 | 438.0 / 237.2 / 217.8 |
| 26 | 13:13 | 12:8 | std / log2 / log1 | 45.4 / 51.9 / 56.6 | 3.07 / 2.70 / 2.14 | 0.15 / 0.14 / 0.19 | 2.93 / 2.50 / 1.89 | 307.3 / 223.6 / 211.2 |
| 26 | 14:12 | 12:7 | std / log2 / log1 | 43.1 / 48.0 / 46.8 | 2.80 / 2.37 / 2.12 | 0.22 / 0.21 / 0.21 | 2.59 / 2.08 / 1.80 | 241.0 / 218.0 / 208.1 |
| 26 | 15:11 | 11:7 | std / log2 / log1 | 45.3 / 49.0 / 47.1 | 3.21 / 2.27 / 2.18 | 0.32 / 0.35 / 0.34 | 2.89 / 1.84 / 1.77 | 208.7 / 215.6 / 204.4 |
| 26 | 16:10 | 11:6 | std / log2 / log1 | 50.6 / 52.2 / 50.8 | 4.51 / 2.37 / 2.28 | 0.55 / 0.55 / 0.56 | 3.97 / 1.73 / 1.66 | 193.4 / 211.0 / 202.8 |
| 28 | 13:15 | 13:9 | std / log2 / log1 | 160.9 / 179.7 / 191.2 | 7.29 / 3.22 / 3.08 | 0.16 / 0.17 / 0.16 | 7.13 / 3.00 / 2.87 | 727.1 / 272.1 / 248.1 |
| 28 | 14:14 | 13:8 | std / log2 / log1 | 147.1 / 164.0 / 158.7 | 5.16 / 3.04 / 3.23 | 0.21 / 0.21 / 0.24 | 4.95 / 2.78 / 2.94 | 467.4 / 261.2 / 242.2 |
| 28 | 15:13 | 12:8 | std / log2 / log1 | 141.2 / 146.4 / 144.5 | 4.58 / 3.40 / 2.93 | 0.42 / 0.37 / 0.34 | 4.16 / 2.99 / 2.52 | 335.9 / 253.0 / 240.2 |
| 28 | 16:12 | 12:7 | std / log2 / log1 | 145.3 / 146.3 / 152.2 | 5.42 / 3.25 / 3.23 | 0.53 / 0.55 / 0.61 | 4.89 / 2.60 / 2.54 | 271.1 / 245.1 / 235.4 |
| 28 | 17:11 | 11:7 | std / log2 / log1 | 165.2 / 189.7 / 158.3 | 7.65 / 3.43 / 3.41 | 1.00 / 1.03 / 1.02 | 6.65 / 2.34 / 2.29 | 239.7 / 242.2 / 233.6 |
| 30 | 14:16 | 14:9 | std / log2 / log1 | 607.5 / 609.1 / 571.7 | 13.09 / 4.08 / 3.53 | 0.24 / 0.24 / 0.26 | 12.85 / 3.78 / 3.20 | 1289.2 / 312.5 / 287.3 |
| 30 | 15:15 | 14:8 | std / log2 / log1 | 623.1 / 679.6 / 578.7 | 9.97 / 4.05 / 3.88 | 0.38 / 0.34 / 0.36 | 9.60 / 3.62 / 3.44 | 765.5 / 304.7 / 279.5 |
| 30 | 16:14 | 13:8 | std / log2 / log1 | 531.2 / 550.2 / 523.6 | 7.59 / 3.46 / 3.40 | 0.56 / 0.56 / 0.59 | 7.03 / 2.84 / 2.73 | 502.0 / 298.4 / 277.6 |
| 30 | 17:13 | 12:8 | std / log2 / log1 | 529.0 / 558.4 / 510.5 | 8.87 / 4.00 / 3.63 | 1.09 / 1.01 / 1.04 | 7.78 / 2.94 / 2.53 | 373.0 / 288.5 / 275.9 |
| 30 | 18:12 | 12:7 | std / log2 / log1 | 557.2 / 545.3 / 530.4 | 15.51 / 5.20 / 4.86 | 2.18 / 2.01 / 2.09 | 13.33 / 3.13 / 2.66 | 307.8 / 281.5 / 271.9 |

**1 thread**

| n | t:s | t':s' | arms | prove (ms) | verify (ms) | (*) (ms) | verify - (*) (ms) | proof (KB) |
|---|---|---|---|---|---|---|---|---|
| 28 | 14:14 | 13:8 | std / log2 / log1 | 491.0 / 536.3 / 514.1 | 4.50 / 2.83 / 3.01 | 0.30 / 0.30 / 0.30 | 4.20 / 2.42 / 2.60 | 467.4 / 261.2 / 242.2 |
| 28 | 15:13 | 12:8 | std / log2 / log1 | 497.3 / 505.3 / 516.7 | 4.07 / 3.13 / 2.87 | 0.58 / 0.55 / 0.58 | 3.49 / 2.52 / 2.23 | 335.9 / 253.0 / 240.2 |
| 28 | 16:12 | 12:7 | std / log2 / log1 | 497.0 / 500.3 / 504.0 | 5.10 / 3.40 / 3.39 | 1.22 / 1.10 / 1.09 | 3.88 / 2.23 / 2.23 | 271.1 / 245.1 / 235.4 |
| 30 | 15:15 | 14:8 | std / log2 / log1 | 1970.3 / 2017.8 / 2037.2 | 7.66 / 3.51 / 3.44 | 0.59 / 0.55 / 0.59 | 7.07 / 2.75 / 2.63 | 765.5 / 304.7 / 279.5 |
| 30 | 16:14 | 13:8 | std / log2 / log1 | 1975.9 / 2046.1 / 1993.6 | 7.13 / 3.49 / 3.51 | 1.11 / 1.10 / 1.14 | 6.01 / 2.28 / 2.26 | 502.0 / 298.4 / 277.6 |
| 30 | 17:13 | 12:8 | std / log2 / log1 | 2017.7 / 2043.1 / 2051.1 | 8.90 / 4.81 / 4.55 | 2.24 / 2.19 / 2.35 | 6.66 / 2.56 / 2.14 | 373.0 / 288.5 / 275.9 |

The `n = 30` rows at 10 threads are a re-run behind a stricter gate (idle
≥ 88 %, 5 repetitions): the first pass, under a 15 % background load, read
10–15 % slower on every arm.

- Prover: one run is within ±5 % of std at the reference split and above
  it (n = 28, 16:12: 145 → 152 ms; n = 30, 16:14: 531 → 524 ms), and within
  1–5 % at 1 thread. Two or three row variables below the reference split
  it pays 5–25 % (its fold commitment, second tree and recursive instance
  grow with `s`).
- Verifier: the log verifier minus `(⋆)` stays at 1.5–3.5 ms for every `n`
  and `t`; std's grows with `t` and `s` (its `2^s` folds and their
  exponentiations, the `2^t + 2^s` claim binding): 2.6–15 ms. After the
  `eq`-table fix `(⋆)` is 0.06–2.3 ms at 10 threads for `t ≤ 18` (the
  `2^t` exponentiations dominate it at 1 thread).
- Proof: smaller than std from n = 26 at the reference split (204 vs
  209 KB), −13 % at n = 28 (235 vs 271) and −26 % at n = 30 (276 vs 373);
  +5 % at n = 24. The log proof barely depends on `t` (n = 30: 287 → 272 KB
  from t = 14 to 18) while std's halves per row variable, so the log scheme
  can take `t ≈ 0.53 n`: n = 30, t = 16 gives 524 ms, verify 3.4 ms, 278 KB
  against std's best split 17:13 at 529 ms, 8.9 ms, 373 KB.

### Recursive split (`--a`, one run, 10 threads)

**10 threads**

| n | t:s | t':s' | arms | prove (ms) | verify (ms) | (*) (ms) | verify - (*) (ms) | proof (KB) |
|---|---|---|---|---|---|---|---|---|
| 28 | 15:13 | 10:10 | log1 | 169.2 | 3.45 | 0.42 | 2.99 | 250.9 |
| 28 | 15:13 | 11:9 | log1 | 163.9 | 3.22 | 0.39 | 2.78 | 243.0 |
| 28 | 15:13 | 12:8 | log1 | 172.0 | 3.20 | 0.40 | 2.76 | 240.2 |
| 28 | 15:13 | 13:7 | log1 | 173.4 | 3.56 | 0.37 | 3.09 | 237.2 |
| 28 | 15:13 | 14:6 | log1 | 188.0 | 3.74 | 0.42 | 3.19 | 236.3 |
| 28 | 15:13 | 15:5 | log1 | 187.6 | 4.77 | 0.44 | 4.13 | 235.6 |
| 30 | 16:14 | 11:10 | log1 | 626.5 | 3.54 | 0.64 | 2.85 | 288.8 |
| 30 | 16:14 | 12:9 | log1 | 618.4 | 3.74 | 0.74 | 2.97 | 282.7 |
| 30 | 16:14 | 13:8 | log1 | 608.3 | 3.88 | 0.64 | 3.18 | 277.6 |
| 30 | 16:14 | 14:7 | log1 | 630.9 | 4.47 | 0.68 | 3.66 | 277.4 |
| 30 | 16:14 | 15:6 | log1 | 638.1 | 4.81 | 0.66 | 4.00 | 275.5 |
| 30 | 16:14 | 16:5 | log1 | 630.9 | 6.11 | 0.67 | 5.17 | 273.6 |

`a = t' − 7` trades the recursive instance's rows `2^{7+a}` (verifier
exponentiations, `(⋆')`, a larger recursive GKR for the prover) against its
columns `2^{s−a}` (folds in the proof, 16 B each). From a = 3 to a = 8 at
n = 28 the proof loses 15 KB (the recursive folds go from 16 KB to 0.5 KB)
and the verifier gains 1.3 ms; the prover loses up to 10 % at the largest
`a`. The default `t' = ⌈0.6 (s + 7)⌉` (12:8 at n = 28, 13:8 at n = 30) is
where the proof has flattened and the verifier has not started to grow;
`a ± 1` moves the proof by ≤ 3 KB and the verifier by ≤ 0.3 ms.

### Tall shapes (small `s`)

**10 threads**

| n | t:s | t':s' | arms | prove (ms) | verify (ms) | (*) (ms) | verify - (*) (ms) | proof (KB) |
|---|---|---|---|---|---|---|---|---|
| 24 | 16:8 | 9:6 | std / log2 / log1 | 29.0 / 28.5 / 28.2 | 4.38 / 2.25 / 2.23 | 0.58 / 0.62 / 0.59 | 3.80 / 1.61 / 1.60 | 149.6 / 170.9 / 164.8 |
| 24 | 17:7 | 9:5 | std / log2 / log1 | 42.9 / 41.1 / 39.9 | 7.24 / 2.72 / 2.69 | 1.11 / 1.12 / 1.11 | 6.13 / 1.58 / 1.55 | 146.1 / 167.7 / 163.8 |
| 26 | 17:9 | 10:6 | std / log2 / log1 | 74.9 / 68.4 / 64.7 | 7.91 / 2.90 / 3.23 | 1.12 / 1.05 / 1.20 | 6.79 / 1.79 / 1.97 | 185.9 / 207.8 / 199.8 |
| 26 | 18:8 | 9:6 | std / log2 / log1 | 95.5 / 88.1 / 87.7 | 14.29 / 3.97 / 4.05 | 2.10 / 2.11 / 2.12 | 12.19 / 1.82 / 1.90 | 181.8 / 205.3 / 197.4 |

With `s ≤ 8` the fold bits are too few to meet a level of `f`'s ladder at
their natural size and are padded (one more zero column, `s_com = 6` at
n = 24, 17:7). The prover is at or below std (the log scheme never
exponentiates the `2^s` folds), the verifier is 2.5–3.5× faster (std pays
the `2^t` row exponentiations and the `2^t`-weight claim binding), and the
proof is 10–12 % larger: std's `2^s` folds are cheap here (2 KB at s = 7)
while the fold commitment's level 0 and the recursive instance are not.

### Ladder rates

`f`'s ladder (`--ladder custom:r:4`), std / one run:

| rate | n | t | prove (ms) | proof (KB) |
|---|---|---|---|---|
| 1/2 | 28 | 15 | 169.2 / 176.1 | 335.9 / 240.2 |
| 1/2 | 30 | 16 | 572.5 / 628.6 | 502.0 / 277.6 |
| 1/4 | 28 | 15 | 151.0 / 170.8 | 274.2 / 177.1 |
| 1/4 | 30 | 16 | 763.5 / 648.8 | 431.1 / 205.9 |
| 1/8 | 28 | 15 | 154.2 / 158.1 | 251.1 / 153.2 |
| 1/8 | 30 | 16 | 625.3 / 617.8 | 403.9 / 178.2 |

The fold bits' level-0 code (`--mu-ladder lad:r0:…`), one run:

| rate | n | t | prove (ms) | fold bits: reduce + level 0 (KB) | proof (KB) |
|---|---|---|---|---|---|
| 1/32 | 28 | 13 | 204.1 | 19.5 | 248.1 |
| 1/32 | 30 | 14 | 708.8 | 22.0 | 287.3 |
| 1/16 | 28 | 13 | 198.4 | 21.9 | 250.6 |
| 1/16 | 30 | 14 | 630.8 | 24.4 | 287.2 |
| 1/8 | 28 | 13 | 185.0 | 25.3 | 252.9 |
| 1/8 | 30 | 14 | 610.0 | 29.6 | 294.5 |
| 1/4 | 28 | 13 | 216.3 | 31.9 | 258.6 |
| 1/4 | 30 | 14 | 715.0 | 37.9 | 304.5 |

### Proof parts (one run, bytes)

| n | t:s | GKR f | GKR fold bits | rec. folds | rec. GKR | reduce f | reduce + level 0, fold bits | merged run | total |
|---|---|---|---|---|---|---|---|---|---|
| 28 | 15:13 | 10080 | 3808 | 4096 | 5568 | 3408 | 17412 | 195716 | 240194 |
| 28 | 16:12 | 10496 | 3584 | 2048 | 5184 | 3408 | 15988 | 194580 | 235394 |
| 30 | 16:14 | 11520 | 4032 | 4096 | 6240 | 3504 | 18484 | 229632 | 277614 |
| 30 | 17:13 | 11968 | 3808 | 4096 | 5568 | 3504 | 18596 | 228256 | 275902 |

## Not done (next steps)

- `(⋆)` is still computed by the verifier. After the `eq`-table fix it is
  the `2^t` exponentiations (1 thread) or about even (10 threads); a
  parallel table build is the next factor. Delegating it (commit the bits of
  `γ`, a third product tree, a mod-`q` check of `γ ≡ eq(r₁)` at a random
  point through a recursive instance) trades it for ≈ 8–16 KB and a
  `2^{t−3}`-size term: not obviously a win.
- Fold the fold bits' ring switch into `f`'s (virtual concatenation before
  the ring switch): ≈ 3 KB.
- The BitZ Ligerito adapter writes every observed value to the narg string
  AND keeps it in the serialized proof (sumcheck messages, `yr`, OOD values,
  roots): ≈ 1.5–2 KB duplicated per proof, in std as well. In one run this
  includes the fold bits' packed target and root (52 B).
- Only one recursion level (a second one would replace 4 KB of recursive
  folds by another ≈ 13 KB level-0 opening: a loss at these sizes); the
  small sub-protocols run after the big GKR instead of overlapping with it.

## Review

Three adversarial read-only passes (finders per lens, two skeptics per
finding). No way to make a false claim verify was found.

1. The protocol (soundness, Fiat–Shamir). Applied: the gate
   `(q − 1)·2^t < 2^127`; a Johnson fold-bits ladder always runs its Round 0.
2. The merged opening, four lenses (soundness, prover/verifier consistency
   and the half-aware residual, configuration, the BitZ adapter). No
   critical or major finding. Applied: `λ` drawn after the merged commitment
   (before, the join round was `L_A·L_B/2^128` in a list-decoding analysis,
   ≈ 114 bits: above the target but unaccounted); the prover's tail draws;
   `plan_matches`; the mode and the executed chain bound in the transcript;
   the Round-0 gate for `f`; a single run sizes only the fold bits' level 0;
   non-Johnson ladders, planned openers and out-of-range targets refused
   with errors; padding for tall shapes; per-part byte accounting.
3. The fixes of pass 2. Of its five agents four hit a usage limit before reporting; the one that
   finished (the join order and the tail draws, 18 checks) found no defect
   and asked for a test of the tail state, since nothing in the tree
   observed it: `merged_two_tree_opening_roundtrips_and_closes_the_transcript`
   in flock (run from `vendor/flock-mod`: `cargo test -p flock-core --lib
   merged_two_tree`), which fails without the prover's tail draws. The
   gates and the accounting were not independently re-read; they are
   exercised by the test suite (both modes, every tamper position) and by
   the sweep (every geometry's opener and plan).

Left as noted: the unscheduled second-tree draws rely on `LogOpener::new`'s
zero-work check; a caller of `prove`/`verify` outside the standalone flow
must bind `f`'s ladder and Round 0 itself; the merged chain is not covered
by an accounting function (`opening_bits` reports `f`'s own ladder).
