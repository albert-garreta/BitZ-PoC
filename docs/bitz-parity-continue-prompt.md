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
  transposes, `c7ead4c` pair buckets, `274d2fd` tuple buckets. Files:
  `src/bitz/{mod,transcript,codec,params,fold,gkr,forest,reduce,sumcheck,pcs}.rs`,
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
  their old `2^114 − 11`, `t ≤ 13`). Their tests (113) pass.
- The verifier crate on their side is "written from the spec only"; the spec
  is `docs/f2z-pcs-spec.md` on their branch `origin/sl/pcs-spec`.

## How to run (the loop that guards every change)

```sh
# their side: dump an instance + proof at the reference split (n bits, seed)
cd ~/f2z-benchmark && git checkout bitz-k4
CARGO_TARGET_DIR=$SCRATCH/f2zb-target cargo run --release --locked -p tests \
  --example dump_bitz -- 28 41 $SCRATCH/dump_n28_seed41      # 14 s, 6 GB
CARGO_TARGET_DIR=$SCRATCH/f2zb-target cargo run --release --locked -p tests \
  --example dump_bitz -- 22 40 $SCRATCH/dump_n22_seed40      # 0.1 s
# our side: re-prove, diff bytes, verify both proofs, write ours next to theirs
cd ~/f2z-pcs && git checkout bitz-parity
RUSTFLAGS="-C target-cpu=native" cargo build --release --features bitz-parity --example bitz_parity
BITZ_TRACE=1 $CARGO_TARGET_DIR/release/examples/bitz_parity $SCRATCH/dump_n28_seed41
# their verifier on our proof
cd ~/f2z-benchmark && CARGO_TARGET_DIR=$SCRATCH/f2zb-target cargo run --release --locked \
  -p tests --example verify_bitz -- 28 41 $SCRATCH/dump_n28_seed41/ours.narg.bin $SCRATCH/dump_n28_seed41/ours.hints.bin
```

Notes: `CARGO_TARGET_DIR` is set globally in this environment to
`~/zinc-plus/target` (that is where `bitz_parity` lands). The dump
directories live in the session scratchpad — regenerate them each session.
Also keep the old-prime vectors around as a second check when convenient
(`dump_bitz` on `bitz-k4` before `344903c` used `2^114−11` and explicit
`(t, s)`; the current one takes `n`). Acceptance for every change: `narg` and
`hints` `IDENTICAL` on n = 22 and n = 28, our verifier accepts theirs, theirs
accepts ours. `cargo test --features bitz-parity,parallel --lib bitz` runs
the unit tests (`patterns` transpose).

## Where the time goes (cool box, n = 28 at (17, 11); their prover 14.0 s)

Ours ≈ 0.52–0.54 s, peak RSS 1.48 GB. `BITZ_TRACE=1` breakdown:
GKR 435–455 ms = level 0 ≈ 116 (bit rounds 27 + 23 + 14, materialise 23,
dense tail 31), level 1 ≈ 90–97 (22 + 14, 22, 29), level 2 ≈ 80 (14, 33, 31),
level 3 ≈ 32, levels 4–16 ≈ 30 total, materialising levels ≥ 3 ≈ 80;
opening ≈ 70 (ligerito 40, reduction sumcheck 19–22, ring switch 9);
folds + images ≈ 17. For scale, the crate's own (different-transcript)
protocol proves n = 28 in ≈ 0.42 s.

## The design, in one paragraph each

- `forest.rs`: their batched product tree without materialised leaves. Level
  ℓ (0 = leaves, `2^{n−ℓ}` entries, in-tree index high, column low) has
  entries that are products of `2^ℓ` leaves (each 1 or a row image), so an
  entry is a function of `2^ℓ` bits of its column: a per-position table with
  `2^{2^ℓ}` entries (`fold_table(ℓ, kk, r)`, pattern bit index `v·2^ℓ + u`,
  rows `y | v≪(t−ℓ−1−kk) | p≪(t−ℓ−1) | u≪(t−ℓ)`). Folding the level's first k
  sumcheck rounds keeps that shape (`2^{2^{ℓ+k}}`), so levels 0/1/2 run their
  first 3/2/1 rounds off `packed_cols` (`bit_round`), materialise the folded
  halves (`materialise_folded`) and hand over to the dense rounds
  (`gkr::prove_layer_tensor`, resumable at round k). Level 3 is materialised
  from its tables, the levels above by products (`level_up`).
- `bit_round`: a row `y`'s sums are `Σ_{a,b} T_E[a]·T_O[b]·Σ_{c:(a,b)} eq_c[c]`
  — terms only bucket `eq_c` (width 1: sixteen bit-sliced masks; width 2: a
  256-entry tuple bucket; width 4: four pair buckets), products once per row.
  `send_one = (z == 0)` selects the `hh` corner for the endpoint sum;
  `factor` accumulates `eq(r, z)` per their `prove_layer`.
- `gkr.rs`: their dense layer (MSB-first halves, Gruen `[factor·Σ_end,
  factor·Σ_∞]`, closing `[l, r]` + challenge, points reversed at entry/exit);
  in-tree rounds weight rows by `eq_y ⊗ eq_c` instead of a built table.
- `sumcheck.rs`: their m-round inner-product sumcheck computed as t rounds on
  the column-combined `2^t` table (`xi_combined_rows_packed`) + s rounds on the
  row-folded `2^s` table (`fold_rows_eq`); messages `[a0, a1, a2]`.
- `pcs.rs`: their statement frames, ring switch, Ligerito via a flock
  `Challenger` framed their way, proof as a bincode-fixint hint.

## Next steps, in order (op-count wins; verify bytes after each)

1. **Fuse the fold into the next round's message pass** in
   `gkr::prove_layer_tensor` (their `fold` then `round_sums` are two sweeps
   over 512 MB tables at the bottom levels): read the four quarters, fold
   `ρ_{j−1}` in registers, accumulate round j's sums and write the folded
   table in one pass. Expect ≈ −30 % on the dense tails (3 × 31 ms) and the
   materialised levels (≈ 60 ms).
2. **JIT materialisation**: instead of `materialise_folded` writing 512 MB
   that the first dense round then reads, have the first dense round read
   `E_k/O_k` through the tables (pattern lookups) and only write the folded
   result. Saves ≈ 3 × 20 ms.
3. **Branch-free / NEON kernels** for the width-4 bit rounds and the dense
   rounds (the crate's `eqf` kernels in `src/piop/sumcheck/eq_factored.rs`
   show the style: unreduced `WideMulAcc` accumulation, no bounds checks,
   interleaved independent chains). This is where the crate's own forest
   gets its last 2×.
4. Materialising levels ≥ 3 (≈ 80 ms): mostly fresh-page cost of ≈ 1 GB;
   allocate once and reuse across levels / `MaybeUninit` writes.
5. Verifier: `fold::reconstruct` uses `BigUint`; a u128 mulmod would bring our
   verify (8–10 ms on their n = 28 proof) to their 4–8 ms.
6. Then measure properly: interleave runs, report two runs, watch the box's
   thermal state (timings swung ±50 % when it was warm).

## Traps

- Their point convention is little-endian externally (coordinate j ↔ index
  bit j) and reversed internally; `eq_table` (= `build_eq_x_r_vec`) is
  little-endian. Their columns occupy the LOW index bits of the leaf index
  (`b·2^s + c`).
- The Gruen message is `[factor·Σ_endpoint, factor·Σ_∞]` with
  `Σ_endpoint` at 1 when the coordinate is 0 (`send_one`).
- `patterns()` bit order: `out[c]` bit i = bit c of `words[i]`; unit test pins it.
- Tables are indexed by `q = (p, y)` with the product bit `p` on top; the
  first half of a materialised level/fold is E.
- `Fq` arithmetic on our side is `u128` residues with a runtime modulus;
  their `Fq<Q>` encodes as 16 LE bytes of the lifted value.
- Never call `git commit` without `--no-gpg-sign` here; paper edits on the
  tree stay uncommitted (user directive) — add only `src/bitz`, `examples/bitz_*`,
  `docs/bitz-*` and manifests.
