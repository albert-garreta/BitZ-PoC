# Continue: BitZ transcript parity prover (branch `bitz-parity`) — session prompt

Paste this into a fresh session in `/Users/albertgarretafontelles/f2z-pcs`.

---

We are making the **parity prover** in `src/bitz/` (feature `bitz-parity`)
fast. It re-implements the BitZ protocol of `worldfnd/f2z-benchmark` — their
clean-room F2Z ("BitZ", spongefish transcript `bitz/v1`) — over this crate's
GF(2^128), packed rows and the vendored flock-mod Ligerito engine, and it
produces **byte-identical** narg strings and hint streams to their prover on
the same instance. Their code is the oracle; ours must never change a byte.

## Where things are

- **This repo, branch `bitz-parity`** (pushed to `origin`; GitHub redirects
  `albert-garreta/f2z-pcs` → `albert-garreta/BitZ-pcs`). Commits: `d0dad46`
  parity port, `f589a3b` stage A, `12c0df1` stage B0, `310e71d` tensor eq /
  transposes, `c7ead4c` pair buckets, `274d2fd` tuple buckets, `0d3e2a4`
  this doc; then the 2026-09-15 session: `148bef0` fused fold-and-round
  passes + the first two dense rounds through the tables into one arena,
  `ae2bbc6` bit rounds through one transpose + NEON scatter/contraction,
  `2bcfa31` 8×64 block transpose by delta swaps, `227dc2f` level 4 built and
  proved inside the arena + parallel row images + verifier traces,
  `406925b` verifier reconstruction through a wide u128 remainder,
  `f1d753e` opening-sumcheck row fold through the eq tensor split, `93738cd` the
  bucketed first table round. Files:
  `src/bitz/{mod,transcript,codec,params,fold,gkr,forest,kernels,reduce,sumcheck,pcs}.rs`,
  `examples/bitz_parity.rs` (the harness), `examples/bitz_root_probe.rs`,
  `docs/bitz-parity-continue-prompt.md` (this file). The crate's own protocol
  is untouched.
- **f2z-benchmark, branch `bitz-k4`** (`~/f2z-benchmark`, pushed to
  `worldfnd/f2z-benchmark`, off `origin/main` 0c75fd8): `2882439` runs the
  `Fast` profile on the k = 4 Ligerito ladder (`crates/pcs/configs/ligerito-k4/`,
  `pcs::ligerito::security_config`); `61ad2c8` examples
  `crates/tests/examples/{dump_bitz,verify_bitz,dump_commit}.rs`; `344903c`
  `Shape::for_log_bits(n)` = our split `t = max(⌈3n/5⌉, 7)` capped at `n−1`,
  `s = n − t`, and the test prime moved to `field::Q100 = 2^100 − 15` (under
  their old `2^114 − 11`, `t ≤ 13`). Their tests (113) pass. A fresh clone
  needs `git fetch` before `origin/bitz-k4` exists locally.
- The verifier crate on their side is "written from the spec only"; the spec
  is `docs/f2z-pcs-spec.md` on their branch `origin/sl/pcs-spec`.

## How to run (the loop that guards every change)

```sh
# their side: dump an instance + proof at the reference split (n bits, seed).
# Built once into their own target dir with the native CPU flags (their
# prover is then 6.7 s / 11.5 GB at n = 28, not 14 s):
cd ~/f2z-benchmark && git checkout bitz-k4
CARGO_TARGET_DIR=$HOME/f2z-benchmark/target RUSTFLAGS="-C target-cpu=native" \
  cargo build --release --example dump_bitz --example verify_bitz
B=$HOME/f2z-benchmark/target/release/examples
$B/dump_bitz 28 41 $SCRATCH/dump_n28_seed41      # 7 s, 11.5 GB
$B/dump_bitz 22 40 $SCRATCH/dump_n22_seed40      # 0.1 s
# our side: re-prove, diff bytes, verify both proofs, write ours next to theirs
cd ~/f2z-pcs && git checkout bitz-parity
RUSTFLAGS="-C target-cpu=native" cargo build --release --features bitz-parity --example bitz_parity
BITZ_TRACE=1 $CARGO_TARGET_DIR/release/examples/bitz_parity $SCRATCH/dump_n28_seed41
# their verifier on our proof
$B/verify_bitz 28 41 $SCRATCH/dump_n28_seed41/ours.narg.bin $SCRATCH/dump_n28_seed41/ours.hints.bin
# unit tests: NEON kernels vs their generic references, the block transpose,
# the reconstruction vs BigUint (8 tests)
RUSTFLAGS="-C target-cpu=native" cargo test --release --features bitz-parity,parallel --lib bitz
```

Notes: `CARGO_TARGET_DIR` is set globally in this environment to
`~/zinc-plus/target` (that is where `bitz_parity` lands). The dump
directories live in the session scratchpad — regenerate them each session.
Also keep the old-prime vectors around as a second check when convenient
(`dump_bitz` on `bitz-k4` before `344903c` used `2^114−11` and explicit
`(t, s)`; the current one takes `n`). Acceptance for every change: `narg` and
`hints` `IDENTICAL` on n = 22 and n = 28, our verifier accepts theirs, theirs
accepts ours, the unit tests pass.

## Where the time goes (cool box, 2026-09-15, n = 28 at (17, 11))

Ours ≈ 0.30 s (301, 304, 304 ms over three runs), peak RSS 0.97 GB; the
session started at 459–471 ms and 1.48 GB (the 0.52–0.54 s quoted before
was a warmer box). Their prover: 6.7 s native. `BITZ_TRACE=1` breakdown:
fold + images 12 (column folds 11.2 = `ligerito::fold_values_bits`, the
u128 nibble-table integer fold; images 0.6);
GKR 236 = levels ≥ 4 build 32 (level 4 from level 3's tables into the
arena, 5..16 by products — mostly the first touch of 512 MB) + levels
16..5 proved 27 + level 4 11.5 + level 3 25 (table round 6.7, table fold
9.4, dense tail 9.0) + level 2 38 (bit round 10.3, table rounds 7.5 + 9.5,
tail 9.2) + level 1 45 (bit rounds 8.4 + 9.2, table rounds 7.1 + 9.7, tail
9.0) + level 0 57 (bit rounds 10.9 + 8.2 + 9.5, table rounds 7.6 + 10.1,
tail 8.8);
opening 53 (sumcheck 7.3 = combine columns 4.3 + fold rows 2.2 + rounds;
ring switch 6.6; Ligerito 37.7).
Verifier: ours 5.1 ms on their n = 28 proof (fold 0.75, GKR 1.1, opening
3.3) against their 7.1 ms.

## The design, in one paragraph each

- `forest.rs`: their batched product tree without materialised leaves. Level
  ℓ (0 = leaves, `2^{n−ℓ}` entries, in-tree index high, column low) has
  entries that are products of `2^ℓ` leaves (each 1 or a row image), so an
  entry is a function of `2^ℓ` bits of its column: a per-position table with
  `2^{2^ℓ}` entries (`fold_table(ℓ, kk, r)` → flat `Tables`, pattern bit
  index `v·2^ℓ + u`, rows `y | v≪(t−ℓ−1−kk) | p≪(t−ℓ−1) | u≪(t−ℓ)`). Folding
  the level's first k sumcheck rounds keeps that shape (`2^{2^{ℓ+k}}`), so
  levels 0/1/2 run their first 3/2/1 rounds off `packed_cols` (`bit_round`),
  then two more rounds THROUGH the 256-entry tables: `jit_round_sums`
  (bucketed by E pattern, see `kernels.rs`) and `jit_fold_round`, which
  folds `r_{k+1}` in registers and writes the once-folded halves into ONE
  arena of `2^{t−4+s}` entries shared by levels 0..3; the remaining rounds
  are `gkr::prove_dense_rounds` on the arena. Level 3 starts the same way
  from `fold_table(3, 0, [])`; level 4 is built as pairwise products of
  those table values INTO the arena (`product_level_into`) and proved in
  place before the arena is reused; levels 5..t−1 are products
  (`level_up`). `prove_bit_level` / `materialise_*` remain only for t < 6.
- `kernels.rs`: every inner loop, NEON on aarch64 with a generic reference
  the unit tests compare against. `round_sums` and `fused_fold_round` (the
  deferred fold of the previous challenge fused with this round's Gruen
  sums: read the four quarters, fold with the preprocessed multiplier,
  write `q0 q1`, accumulate), `jit_bucket_group` / `jit_bucket_finish` (the
  first table round: `eq·O` products XORed unreduced into 256-entry buckets
  keyed by the E pattern, reduced once and contracted with the E tables),
  `jit_fold_group`, `jit_product_group`, `scatter_add` (the bit rounds' one
  addition per term) and `contract`. Pattern blocks are consumed in
  TRANSPOSED order (`col_of`), see the traps.
- `gkr.rs`: their dense layer (MSB-first halves, Gruen `[factor·Σ_end,
  factor·Σ_∞]`, closing `[l, r]` + challenge, points reversed at entry/exit);
  `prove_dense_rounds` keeps the previous challenge `pending` and folds it in
  the next round's pass, so each table is swept once per round; in-tree
  rounds weight rows by `eq_y ⊗ eq_c`, the last `s` rounds by the small flat
  eq table.
- `bit_round`: a row `y`'s sums are `Σ_{a,b} T_E[a]·T_O[b]·Σ_{c:(a,b)} eq_c[c]`
  — terms only bucket `eq_c` (widths 1 and 2: the (E_lo, E_hi, O_lo, O_hi)
  tuple byte straight out of one block transpose of the corner words; width
  4: two transposes of (O, E) word pairs give `pat_E·16 + pat_O`, the cross
  pairs by swapping nibbles), buckets per rayon split (`Buckets`, `map_init`),
  products once per row (`contract`). `send_one = (z == 0)` selects the `hh`
  corner for the endpoint sum; `factor` accumulates `eq(r, z)`.
- `sumcheck.rs`: their m-round inner-product sumcheck computed as t rounds on
  the column-combined `2^t` table (`xi_combined_rows_packed`) + s rounds on the
  row-folded `2^s` table (`fold_rows_point`: `eq(b) = eq(b mod 64)·eq(b div
  64)`, one 32 KB byte table + a multiply per word); messages `[a0, a1, a2]`.
- `fold.rs`: the integer column folds (`ligerito::fold_values_bits`), the
  images (parallel), and the verifier's reconstruction as a wide u128
  remainder (`U128::widening_mul` + `rem_wide_vartime`, sum by `add_mod`).
- `pcs.rs`: their statement frames, ring switch, Ligerito via a flock
  `Challenger` framed their way, proof as a bincode-fixint hint.

## What was measured NOT to help (do not redo)

- The table rounds and the dense tails are µop-bound (lookups, loads,
  stores, the ~40 PMULL per pair index): bucketing the first table round cut
  its PMULLs by ~45 % but only 7.5 → 7.1 ms per level; the 8×64 transpose
  helped the bit rounds (61 → 52 ms) but not those rounds.
- Pre-weighting E by eq once per level costs what it saves: the fold of a
  weighted table needs two multiplies (`a = (1−ρ)/(1−z)`, `b = ρ/z`) and a
  fallback at `z ∈ {0, 1}`.
- Two rounds per pass (the crate's double-fold) adds multiplies to
  compute-bound passes; only wins when memory-bound.
- Pair buckets of 65536 entries for width 8 (the table rounds): the
  contraction outgrows the 2048 columns.

## Next steps (ideas, by expected gain)

1. Levels ≥ 4 build (32 ms): ≈ 25 ms is the first touch of 512 MB (levels
   5..16 + the arena). Options: prove level 4 straight from level 3's tables
   (two lookups + a product per value, no materialised level 4, another
   −256 MB) and build level 5 from the tables; or keep the buffers across
   proves when a harness proves repeatedly.
2. Integer column fold (11.2 ms): `fold_values_bits` streams an 8 MB nibble
   table per 32-column block; integer weights have no tensor structure, so
   only wider blocks / prefetching / smaller tables remain.
3. Width-1 bit round at level 0 (10.9 ms): 16 buckets → store-forwarding
   conflicts. Try the subset-AND weighted-popcount route (per-row `W(E)`,
   `W(O)` precomputed once like `xi_combined_rows_packed`, then four pair-AND
   popcounts per (row, group) through per-group byte tables) or four
   interleaved bucket sets.
4. Preprocessed (fixed-scalar) eq weights for the Gruen slot: −2 of ~40
   PMULLs per pair index across the table fold, dense tails, levels 4..16
   (≈ 2 %).
5. Ligerito (38 ms) is flock's engine; the ring switch (6.6 ms) already uses
   flock's method-of-four-Russians `fold_1b_rows_naive`.
6. Measure properly: interleave runs, report two or three, watch the box's
   thermal state (timings swung ±50 % when it was warm).

## Traps

- Their point convention is little-endian externally (coordinate j ↔ index
  bit j) and reversed internally; `eq_table` (= `build_eq_x_r_vec`) is
  little-endian. Their columns occupy the LOW index bits of the leaf index
  (`b·2^s + c`).
- The Gruen message is `[factor·Σ_endpoint, factor·Σ_∞]` with
  `Σ_endpoint` at 1 when the coordinate is 0 (`send_one`).
- Pattern blocks are TRANSPOSED: `transpose_blocks` leaves byte `k` of word
  `j` = the pattern of column `8k + j`, so position `m` of a 64-byte block
  is column `col_of(m) = ((m & 7) << 3) | (m >> 3)`. The kernels take the
  weights in the same order (`transposed_eq`, zero past the last column) and
  map stores back through `col_of`; `patterns()` (column order) remains for
  the t < 6 path and its test.
- Tables are indexed by `q = (p, y)` with the product bit `p` on top; the
  first half of a materialised level/fold is E.
- The arena is exactly `2^{t−4+s}` entries: `product_level_into` asserts the
  capacity, `jit_fold_round` fills through the spare capacity the first time
  and reuses the initialised prefix after (its length must already be
  `2^{t−4+s}`).
- `Buckets` / `SumBuckets` are per rayon split (`map_init`); the
  `parallel`-off path keeps one.
- `Fq` arithmetic on our side is `u128` residues with a runtime modulus;
  their `Fq<Q>` encodes as 16 LE bytes of the lifted value.
- Shell: `grep pattern $F` with an empty `$F` reads stdin and hangs the step.
- Never call `git commit` without `--no-gpg-sign` here; paper edits on the
  tree stay uncommitted (user directive) — add only `src/bitz`, `examples/bitz_*`,
  `docs/bitz-*` and manifests.
