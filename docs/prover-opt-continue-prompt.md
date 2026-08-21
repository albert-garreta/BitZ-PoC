# Session prompt: continue the n=28 prover-optimization program

Paste-able prompt for a fresh session (follows the repo's `docs/*-prompt.md`
convention). State as of 2026-08-21, f2z-pcs master `0f205b6`.

---

Continue optimizing the f2z-pcs prover for n=2^28 (shape `17:11:1`, bench
default `custom:3:4`). Standing directives from the owner: do whatever it
takes, keep iterating autonomously; **never modify verifier code** (that
includes shared prover/verifier dispatch like `quad_active` and the codec's
`from_bytes`); do not increase proof size without asking; the prover must
handle randomly generated valid witnesses (no witness-specific tricks).
Acceptance standard for every change: byte-identical proofs (bench `fnv`
pins) + the full test suite.

## Where things stand

Two byte-identical prover-only passes landed 2026-08-21 (commits `dc31e0f`,
`0f205b6` on master; flock pin `ed4c0cd` on branch `f2z-k4-port` in
`~/flock-f2z-port` — visibility-only). n=28 quiet-window bench medians went
**471 → 432 ms**; n=26 ~150 → ~133 ms. Pinned fnv: n=28 `17:11:1` default
env = `3291626d5b455262`; n=26 `16:10:1` = `4c99cf18d0239216`. 120 tests
green. Full ledger: memory file `f2z-n28-prover-pass.md`; idea maps:
`docs/forest-speedup-ideas.md`, `docs/lut-width-ideas.md` (their closed
ledgers are binding — do not relitigate).

What landed: parallel PoW grind (smallest-nonce waves in
`ZincChallenger::grind_pow`), round-1 lookahead wired through
`fill_phi_basis_round0`, parallel `extract_column_bit_halves`,
`xi_combined_rows_packed` (group-outer byte tables off `hint.packed_cols`),
slot-tiled leaf round 1 (`leaf_round1_tiled`, `F2Z_LEAF_TILE=0` opts out)
with the Precombined ΔΔ form resurrected by L1 tiling
(`build_leaf_tables` tile hint), `par_clone_f128` at all prover
`hint.p_msg` copies, parallel fold-table builds, `par_min_len` helper.

## n=28 anatomy after the passes (~432 ms quiet box, scope medians)

forest ~370: eqf:grid 87 (NEON, mul-bound) · leaf_r1 17–19 (tiled) ·
pair3_r1 19 + pair3_r2 18 + leaf3_r2 16 + leaf3_r3 17 (mul-bound) ·
mats pair3mat+leaf3mat ~60 · mf:bitgen ~53 · mf:build_levels 40–53 ·
phaseA self ~18 · extract_bits 5–13 · teto ~1. presum_tbls ~8 · fold_v ~6.
open ~43: recursive commits 13.4 (SHA-256 Merkle — hash is
verifier-locked) · init sumcheck ~6.7 · induce 4.5 · grind ~1 (was 7–8) ·
~8 unaccounted (query sampling + framed per-element absorbs —
verifier-locked framing).

## Established lessons — do not re-derive

- **Tiling pays only on pick/XOR-heavy rounds.** leaf_r1 won (L2-BW-bound
  → L1 tiles + 16-case picks); the identical tile on pair3_r1/leaf3_r2 was
  a WASH in both table forms (wide-mul-bound). Mats/bitgen/gen_top are
  mul- or write-mixed — same prediction.
- Measured dead ends: `array::from_fn`→literals in the mats (+5.6% —
  codegen pessimization; profiler frames there are latency artifacts),
  coarse rayon chunks (`F2Z_PAR_CHUNK`, +1.4%), 6 threads (+15%),
  post-loop NEON mats deposit (interleave hides under gathers; leaf3mat
  30→54), batching flock's per-element observes (transcript `absorb_slice`
  is 0x6…0x7 framed — verifier-locked).
- **QUAD=2 is a wash post-pass** (+1.2% median, +1228 B; re-measure with
  `examples/quad_ab.rs`). Stays opt-in; do not flip defaults in
  `quad_active`/`quad_v2` (verifier dispatch).
- Raw8/A2/PRFM gates are correctly tuned; flock merkle/NTT are parallel.

## Measurement protocol (the box lies)

- **TRAP**: the user's shell exports
  `CARGO_TARGET_DIR=/Users/albertgarretafontelles/zinc-plus/target` — all
  builds land THERE (`.../release/deps/pcs-4162895a22ab139a`, overwritten
  per build). The repo-local `./target` holds STALE binaries; copying from
  it once manufactured a phantom +5% regression.
- Box noise: ±2–3% per pair, absolute drift +10–20% by time of day.
  Quotable deltas need paired in-window A/B: alternate arms, ≥5 pairs,
  median of ratios. Prefer env-gated arms inside ONE binary (layout noise
  cancels); for cross-binary A/B, pin copies immediately post-build. A/A′
  control ≈ ±1–3%/pair.
- Tools: `OBLONG_PROFILE=1 PROBE_SHAPES="17:11" cargo run --release
  --example prof_probe --features unchecked` (scope tree; uses the FAST
  lig config — open-side numbers differ from the bench default);
  `LIG_PROVE_TRACE=1` (flock's open-phase timers); bench:
  `env F2Z_BENCH_SHAPES=17:11:1 F2Z_BENCH_REPS=9 RUSTFLAGS="-C
  target-cpu=native" cargo bench --bench pcs --features unchecked`.
- zsh traps: `env $cfg` does not word-split (use `${=cfg}`); a bare word
  starting with `=` breaks; `grep -c` with zero matches exits 1 and kills
  `&&` chains.
- Concurrent paper session in this checkout: commit with explicit
  pathspecs only; never `git add -A`, never `git stash`.

## Ranked next steps

1. **The mats tile** (pair3mat+leaf3mat, ~60 ms — the largest open block).
   Same slot-tiling idea, but the mats WRITE per-tree dense buffers, so
   parallel-over-blocks needs disjoint per-(tree, slot-range) writes into
   the per-tree `Vec`s — an unsafe pointer split or an output arena (a
   GroupBufs type change ripples; copy-out would eat the win). The
   interleaved grid deposit must ride inside each block. ROI uncertain
   (the 512 MB stream write stays; the mul-bound lesson half-applies) —
   probe before polishing.
2. **H1: SME2/BMOPA feasibility** (`docs/forest-speedup-ideas.md` §5):
   micro-bench a bitsliced 128×N fixed-operand GF(2) matrix multiply vs
   the PMULL loop. One day; kills or opens the hardware lane for the
   ~200M fixed-operand muls (grid 87 ms etc.).
3. **n≥30 on a memory-fresh box**: verify the leaf tile at n=29 (it
   forces Precombined there — unmeasured; `F2Z_LEAF_TILE=0` if it loses;
   n=30's Raw8 disengages it), then I3 generator bodies / I5 at the n=30
   leaf round, then I6/L-16.
4. Small: leaf-tile block-size tuning (`tb` = 128 precombined / 256
   factored — worth ±1–2 ms at most); `mf:teto` serial build (~1 ms);
   phaseA glue (~18 ms diffuse: `z_x.to_vec()` per group per layer —
   an `EqInnerGroupMixed` API change, ~20 call sites, small gain).
5. If all else plateaus: the remaining blocks are at the documented
   roofline — say so rather than churning.

Work autonomously, keep the byte-identity pins green after every change,
commit finished increments with explicit pathspecs, and append findings to
the memory file (`f2z-n28-prover-pass.md`) including dead ends.
