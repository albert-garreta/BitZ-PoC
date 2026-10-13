# SHA-256 compressions with the logarithmic BitZ opening (branch `sha-log`)

The SHA-only bench of `sha-private-io-across` (Ch-only relation, `h` of
16,361 < 2^14 live bits per compression, `Id ⊗ M` block-layout commitment,
`wfbitz` block opener) with the scheme of `paper/notes/bitz_logarithmic.tex`
in place of the standard fold step: the `2^s` integer column folds are
committed as bits instead of sent, a second product tree over those bits
meets the first one at a shared `(ζ, e₀)`, one recursive standard-BitZ
instance proves the recombination, and the two openings share ONE Ligerito
run (the fold bits join `h`'s ladder where the folded messages have the
same size). `src/wfbitz/logarithmic.rs`, `src/wfbitz/pcs.rs`
(`prove_lin_pair`/`verify_lin_pair`), `WfbitzLogOpener` in
`src/piop/spartan/protocol/linear.rs`, `prove/verify_sha256_compressions_wfbitz_log`.

Measured 2026-10-03, 10 threads, LTO + `unchecked`, λ = 100 (`custom:1:4`,
rate 1/2 Johnson ladder with Round 0 for `h`; fold bits `lad:5:3:3:7`,
rate 1/32), fixed root seed, 3 repetitions after a warm-up, idle-gated
(`scripts/bench_gate.py`), medians. `(t, s) = (17, 12)` at 2^15 and
`(17, 13)` at 2^16 (the branch's split), `q` a transcript-sampled 109-bit
prime, 8 compressions per column. The witness (2.5 / 5.0 s) is outside
the prover time; the commitment (step 1: 15 / 32 ms at rate 1/2, 49 / 105
ms at rate 1/8) is inside it (`prove_ms` = the end-to-end prove span).

## Standard vs logarithmic

| compressions | opener | prove | verify | proof | (⋆) |
|---|---|---|---|---|---|
| 2^15 = 32,768 | standard | 299 ms | 11.8 ms | 272.8 KB | – |
| | log, two Ligerito runs | 306 ms | 12.3 ms | 249.6 KB (−8.5 %) | 0.4 ms |
| | log, one merged run | 307 ms | 12.6 ms | **237.8 KB (−13 %)** | 0.4 ms |
| 2^16 = 65,536 | standard | 571 ms | 14.2 ms | 360.1 KB | – |
| | log, two Ligerito runs | 574 ms | 15.3 ms | 276.4 KB (−23 %) | 0.4 ms |
| | log, one merged run | 567 ms | 14.6 ms | **263.7 KB (−27 %)** | 0.4 ms |

One thread (2^15, 2 repetitions): standard 1,138 ms / 16.8 ms / 272.8 KB,
log one run 1,143 ms / 17.4 ms / 238.4 KB; `(⋆)` single-threaded 1.6 ms
(the standard opener's `v: fold` phase, which contains the same images, is
2.0 ms).

Prover: equal within the run-to-run band (the log-specific work is 5–7 ms:
commit the fold bits 0.7–1.2, second tree 1.6–2.5, recursive instance
1.5–1.8, its opening 1.1–1.4; the forest over the derived grid is 217 /
405 ms, 73 % of the prover, and is the same in both). Verifier: no gain at
these sizes (see below), +0.4–0.8 ms. `(⋆)` is the phase
`v: (*) row images` (the `2^t` row images `g^{w_i}` of the claim, computed
over the 10 threads); the standard opener computes the same images inside
its `v: fold`/`v: gkr` phases.

## Proof anatomy (bytes, `LOGSIZES` = narg + hints)

| part | 2^15 one run | 2^15 two runs | 2^16 one run | 2^16 two runs |
|---|---|---|---|---|
| head (μ root, Round-0 value, e₀) | 64 | 64 | 64 | 64 |
| GKR over `h` | 11,424 | 11,424 | 11,968 | 11,968 |
| GKR over the fold bits | 3,584 | 3,584 | 3,808 | 3,808 |
| recursive instance: folds | 2,048 | 2,048 | 4,096 | 4,096 |
| recursive instance: GKR | 5,184 | 5,184 | 5,568 | 5,568 |
| opening of `h`: reduction (sumcheck + ring switch) | 3,408 | 5,132 | 3,456 | 5,040 |
| opening of `h`: Ligerito | – (merged) | 190,340 | – (merged) | 208,956 |
| opening of the fold bits: reduction | 2,976 | 4,440 | 3,024 | 5,512 |
| opening of the fold bits: Ligerito / level-0 branch | 12,468 | 25,924 | 14,420 | 29,092 |
| merged Ligerito chain | 2,400 + 191,924 | – | 1,876 + 213,548 | – |
| **total (BitZ opening)** | **235,480** | 248,140 | **261,828** | 274,104 |

The standard proof is the `2^s` folds (`16·2^s` B = 64 / 128 KB), the one
GKR (≈ 11.5 / 12 KB) and the opening of `h` (≈ 196 / 214 KB). The
logarithmic opening replaces the folds by ≈ 12 / 14 KB of fold-bit
material (second tree + recursive instance + level-0 branch opening): the
remaining ≈ 200 KB is the Ligerito of `h` itself, which neither scheme
touches. The one-run merge saves 10.7 / 12.3 KB over two runs (the fold
bits' own Ligerito chain 25.9 / 29.1 KB becomes a 12.5 / 14.4 KB level-0
opening plus ≈ 2–5 KB of extra bytes on `h`'s chain, whose post-merge
levels carry one more message variable).

Plan at 2^15: `h` is 2^28 bits = 2^21 packed elements, ladder message sizes
`[21, 17, 14, 11, 8, 5]`; the fold bits (2^12 folds × 128 slots, padded to
2^19 bits = 2^12 packed) join with interleaving `k₀ = 1` after the ladder's
second iteration (message size 11). At 2^16: `h` 2^22 packed, `[22, 18, 15,
12, 9, 6]`, fold bits 2^13 packed, `k₀ = 1`.

## Where the verifier's time goes

`verify (median)` splits (bench summary, 2^15 / 2^16, standard opener):
PIOP 2.5 / 3.4 ms, bitify (the opening claim) 0.8 / 0.9, the BitZ opener
6.7 / 7.1, residual (statement binding etc.) 2.1 / 3.8. Inside the opener,
`VPHASES` (ms, 2^16):

| phase | standard | log, two runs | log, one run |
|---|---|---|---|
| head (statement + claim binding, μ root, Round 0, ζ, e₀) | (untimed, ≈ 2) | 2.2 | 2.3 |
| fold / (⋆) row images | 1.26 | 0.38 | 0.43 |
| GKR over `h` | 0.78 | 0.79 | 0.87 |
| GKR over the fold bits | – | 0.13 | 0.11 |
| recursive fold + GKR | – | 0.29 | 0.31 |
| transpose | 0.16 | 0.16 | 0.16 |
| opening(s) | 2.23 | 2.24 + 0.86 | 2.61 (merged) |

The standard verifier's fold-dependent work at `s = 12–13` is only
≈ 1 ms (reading 4–8 K folds, one comb exponentiation each), so the log
scheme has nothing to remove here; it adds the second tree, the recursive
instance and the fold bits' opening (+0.4–1.3 ms). The verifier-side win
of the logarithmic scheme (master lineage: 15 → 3 ms at n = 30) needs a
much larger `2^s`. The claim binding hashes the `2^t`-entry row vector
(2 MB) in both schemes (≈ 2 ms, the `head` phase here, untimed in the
standard path).

## The split `(t, s)` once the folds are committed

`BITZ_SHA_WFBITZ_T=t` probes the product split (`s = n − t`; the reference
is `t = ⌈0.6 n⌉ − 1 = 17`). 10 threads, 3 reps, medians (`runs/shalog-split`):

| compressions | t | s | standard: prove / verify / proof | log, one run: prove / verify / proof | (⋆) | fold-bit material |
|---|---|---|---|---|---|---|
| 2^15 | 17 | 12 | 300 ms / 11.8 ms / 272.8 KB | 311 / 12.7 / 237.8 | 0.37 ms | 26.3 KB |
| | **16** | 13 | 286 / 10.6 / 338.6 | **294 / 10.4 / 242.2** | 0.24 | 30.8 |
| | 15 | 14 | 290 / 10.4 / 470.0 | 304 / 10.3 / 245.4 | 0.15 | 33.8 |
| | 14 | 15 | 295 / 12.9 / 731.5 | 317 / 10.9 / 247.3 | 0.12 | 34.8 |
| 2^16 | 17 | 13 | 554 / 14.7 / 360.1 | 589 / 14.6 / 263.7 | 0.39 | 30.9 |
| | **16** | 14 | 563 / 13.9 / 490.6 | **559 / 13.3 / 264.9** | 0.23 | 32.9 |
| | 15 | 15 | 558 / 15.9 / 752.6 | 575 / 12.8 / 269.3 | 0.16 | 36.7 |
| | 14 | 16 | 577 / 20.8 / 1,275.5 | 613 / 15.0 / 274.6 | 0.12 | 40.7 |

With the folds committed the proof is nearly flat in `t` (+2–3 KB per step
of `s`, against +66 / +130 KB per step for the standard opener), so the
split is chosen by time alone: `t = 16` is the optimum at both sizes
(prover −5 %, verifier −10…−18 % against the reference `t = 17`, proof
+1.8 / +0.5 %). Below that the fold-bit side grows faster than the forest
shrinks (2^16, log-specific prover phases at `t = 17 → 14`: commit the
fold bits 3.3 → 2.3 → 3.7 → 18.4 ms — the rate-1/32 codeword of a
`2^{s+7}`-bit commitment; second tree 2.5 → 4.3 → 7.4 → 13.3; recursive
instance 1.9 → 3.2 → 5.5 → 7.9; merged opening 31.7 → 32.7 → 34.0 → 38.6;
the forest over `h` 399 → 388 → 381 → 408). The verifier bottoms at
`t = 15` (its `(⋆)` halves per step while the recursive instance grows).
The standard opener's own prover optimum is also `t = 16`, bought with
+24 % proof.

## Rate 1/8 for `h` (`BITZ_LIG_PROFILE=custom:3:4`)

Same sweep with `h`'s ladder at rate 1/8 (Johnson + Round 0, 6 bits of
Round-0 grinding instead of 4; the fold bits stay at rate 1/32). Measured
2026-10-06, idle ≥ 88 %, machine lock (`runs/shalog-r8`):

| compressions | t | s | standard: prove / verify / proof | log, one run: prove / verify / proof | (⋆) | fold-bit material |
|---|---|---|---|---|---|---|
| 2^15 | 17 | 12 | 358 ms / 12.3 ms / 187.5 KB | 358 / 12.2 / 152.5 | 0.40 ms | 27.1 KB |
| | **16** | 13 | 339 / 10.8 / 252.4 | **344 / 10.3 / 155.4** | 0.24 | 30.6 |
| | 15 | 14 | 340 / 10.6 / 383.1 | 349 / 9.7 / 158.3 | 0.16 | 33.8 |
| | 14 | 15 | 354 / 12.6 / 644.8 | 372 / 11.1 / 159.5 | 0.12 | 34.3 |
| 2^16 | 17 | 13 | 669 / 14.8 / 267.6 | 683 / 14.5 / 170.1 | 0.39 | 30.6 |
| | **16** | 14 | 675 / 14.4 / 397.7 | **674 / 13.0 / 171.3** | 0.24 | 32.6 |
| | 15 | 15 | 670 / 15.7 / 659.8 | 687 / 13.7 / 175.4 | 0.17 | 36.1 |
| | 14 | 16 | 690 / 19.8 / 1,183.7 | 740 / 15.2 / 180.3 | 0.12 | 40.6 |

Against rate 1/2 at the same split (`t = 16`): proof 242 → 155 KB (−36 %)
at 2^15 and 265 → 171 KB (−35 %) at 2^16 — the merged Ligerito chain
shrinks from 195 / 215 KB to 109 / 122 KB while the fold-bit material is
unchanged — for +17 / +20 % prover time, of which the commitment is
+34 / +73 ms (the 4× larger codeword: NTT + Merkle over `2^{m+3}` elements)
and the Ligerito prover +8 / +21 ms; the verifier is unchanged (±0.3 ms).
The split optimum stays at `t = 16` for both openers; the log opener's
proof is again flat in `t` (+1–3 KB per step), the standard one's doubles
its folds. With the folds gone, the standard opener's rate-1/8 proof at the
reference split (187.5 KB) is still 21 % larger than the log opener's
(152.5 KB), and 63 % larger at `t = 16`.

## The fold bits' rate (`BITZ_SHA_LOG_MU_LADDER=lad:r0:…`, one run uses only `r0`)

`h` at rate 1/8, `t = 16`, one merged run; the fold bits (`2^{s+7}` bits,
`2^13` / `2^14` packed elements) join `h`'s chain with `k₀ = 2` after its
second iteration at every rate; only their level-0 parameters change
(queries 60 / 45 / 36 / 31 and fold grinding 8–11 bits at rates 1/8, 1/16,
1/32, 1/64). Measured 2026-10-06 (`runs/shalog-mu`, plans in
`runs/shalog-mu-plan`):

| compressions | fold-bit rate | prove | verify | proof | fold-bit opening (reduction + level 0) | fold-bit material | commit fold bits |
|---|---|---|---|---|---|---|---|
| 2^15 | 1/8 | 344 ms | 10.5 ms | 160.2 KB | 21.3 KB | 34.8 KB | 0.5 ms |
| | 1/16 | 343 | 10.2 | 157.1 | 18.8 | 32.2 | 0.8 |
| | 1/32 (default) | 346 | 10.4 | 155.4 | 17.1 | 30.6 | 1.4 |
| | 1/64 | 349 | 10.3 | 154.7 | 16.6 | 30.0 | 2.3 |
| 2^16 | 1/8 | 664 | 12.9 | 177.2 | 23.4 | 37.8 | 0.8 |
| | 1/16 | 676 | 12.9 | 172.4 | 20.2 | 34.6 | 1.4 |
| | 1/32 (default) | 673 | 13.1 | 171.3 | 18.3 | 32.6 | 2.4 |
| | 1/64 | 665 | 13.4 | 171.0 | 17.1 | 31.4 | 4.3 |

The level-0 opening is `Q · 2^{k₀} · 16` B of rows plus `≈ Q · (depth −
log₂ Q) · 32` B of Merkle paths; halving the rate takes a quarter off `Q`
but adds one level of depth, so the bytes converge: 1/64 saves 0.7 / 0.3 KB
over 1/32 (0.5 % of the proof) for +1–2 ms of commitment; 1/16 costs
+1.7 / +1.1 KB. Prover and verifier differences are inside the run-to-run
band (±10 ms, ±0.3 ms). The ≈ 30 KB floor of the fold-bit material is the
second tree (3.8–4 KB) and the recursive instance (10 KB) plus the two
reductions' narg (≈ 6 KB); the rate only moves the level-0 opening.

## `h` at rate 1/16 with the fold bits at rate 1/64

`BITZ_LIG_PROFILE=custom:4:4` (Johnson + Round 0, 6 grinding bits, queries
45 / 36 / 31 / 27 / 24 per level) with `BITZ_SHA_LOG_MU_LADDER=lad:6:…` for
the log opener (the standard opener has no fold-bit commitment). Measured
2026-10-06 (`runs/shalog-r16`), 10 threads, 3 reps, idle ≥ 88 %:

| compressions | t | s | standard: prove / verify / proof | log, one run: prove / verify / proof | (⋆) | chain / fold-bit material |
|---|---|---|---|---|---|---|
| 2^15 | 17 | 12 | 403 ms / 11.7 ms / 174.6 KB | 418 / 12.4 / **137.8** | 0.39 ms | 94.8 / 26.1 KB |
| | 16 | 13 | 390 / 10.4 / 240.0 | **399 / 10.4 / 142.9** | 0.25 | 97.1 / 30.0 |
| 2^16 | 17 | 13 | 779 / 14.8 / 253.7 | 788 / 14.4 / **155.0** | 0.38 | 107.7 / 29.8 |
| | 16 | 14 | 775 / 13.6 / 384.2 | **794 / 12.9 / 156.4** | 0.22 | 107.8 / 31.9 |

### `h` at rate 1/4 (`custom:2:4`, queries 90 / 60 / 45 / 36 / 31, 4 grinding bits; fold bits 1/32)

Measured 2026-10-06 (`runs/shalog-r4`):

| compressions | t | s | standard: prove / verify / proof | log, one run: prove / verify / proof | (⋆) | chain / fold-bit material |
|---|---|---|---|---|---|---|
| 2^15 | 17 | 12 | 321 ms / 11.6 ms / 211.2 KB | 324 / 11.9 / 176.8 | 0.38 ms | 132.8 / 26.7 KB |
| | 16 | 13 | 306 / 9.8 / 277.4 | **310 / 11.0 / 180.7** | 0.24 | 133.3 / 30.9 |
| 2^16 | 17 | 13 | 596 / 14.2 / 293.4 | 613 / 15.0 / 196.6 | 0.39 | 148.9 / 30.7 |
| | 16 | 14 | 593 / 13.8 / 423.9 | **600 / 12.9 / 198.9** | 0.23 | 149.3 / 32.7 |

### The rate ladder

Log opener at `t = 16`, one run (prove / verify / proof; the commitment is
inside the prover time and given in parentheses):

| compressions | rate 1/2 (folds 1/32) | rate 1/4 (folds 1/32) | rate 1/8 (folds 1/32) | rate 1/16 (folds 1/64) |
|---|---|---|---|---|
| 2^15 | 294 ms / 10.4 ms / 242.2 KB (15 ms) | 310 / 11.0 / 180.7 (27) | 344 / 10.3 / 155.4 (49) | 399 / 10.4 / 142.9 (93) |
| 2^16 | 559 / 13.3 / 264.9 (32) | 600 / 12.9 / 198.9 (58) | 674 / 13.0 / 171.3 (105) | 794 / 12.9 / 156.4 (207) |

Each halving of the rate doubles the commitment (codeword NTT + Merkle:
5 % of the prover at 1/2, 9 % at 1/4, 14–16 % at 1/8, 23–26 % at 1/16)
and adds to the Ligerito prover (merged opening 33 → 38 → 52 → 67 ms at
2^16), while the proof's remaining Ligerito chain shrinks less and less
(195 → 133 → 109 → 95 KB at 2^15). Marginal trades: 1/2 → 1/4 buys
−25 % proof for +5…+7 % prover (the best step on the ladder), 1/4 → 1/8
−14 % for +11…+12 %, 1/8 → 1/16 −8…−9 % for +16…+18 %. The verifier does
not move (10.3–11.0 / 12.9–13.3 ms at every rate). At
rate 1/16 the reference split `t = 17` is 3–5 % smaller in bytes than
`t = 16` (the chain's post-merge levels carry one more variable and the
recursive instance is larger at `s + 1`), while `t = 16` is 4–5 % faster
to prove and 10–16 % faster to verify.

## The slide-16 row (talk table, 2026-10-06 afternoon)

`~/Documents/BitZ-talk-tables/s16-sha256-vs-flock.tex` (built by its
`make_tables.py` from `data/sha-vs-flock-20261006/summary.json`) now has the
row "BitZ-log-SNARK, rate 1/4": this branch's binary, `BITZ_OPENER=wfbitz-log`,
one merged run, fold bits at 1/32, `custom:2:4`, the reference split, and —
as the other rows — the private-I/O statement (`BITZ_SHA_PRIVATE_IO=1`),
one process per cell under the machine lock and idle gate, `/usr/bin/time
-l`, 3 rounds × 3 samples (`camp-log/campaign-log.sh`; the standard opener
at rate 1/4, `bz14`, was measured alongside but is not in the slide).
Medians:

| N | prover 1 thr | prover 10 thr | verifier 1 / 10 thr | proof | peak RSS |
|---|---|---|---|---|---|
| 2^13 | 333 ms | 98.7 ms | 5.93 / 5.52 ms | 154 KB | 0.371 GB |
| 2^14 | 614 | 169 | 6.51 / 6.18 | 167 | 0.752 |
| 2^15 | 1,222 | 326 | 10.7 / 9.10 | 177 | 1.47 |
| 2^16 | 2,340 | 613 | 11.3 / 10.0 | 196 | 2.81 |

(Flock at rate 1/4, same statement: 2^16 1,321 / 307 ms, 20.6 / 5.42 ms,
259 KB, 7.15 GB; standard BitZ at rate 1/2: 2,163 / 550 ms, 359 KB.)

The slide's BitZ rows had counted the commitment twice: the campaign's
`summarize.py` added `commit_ms` to `prove_ms`, but the bench's `prove_ms`
is the end-to-end prove span that already contains the commitment (its
summary lists "step 1 commit" inside it). Fixed in `summarize.py` (the old
value is kept as `prove_plus_commit_ms`); the rate-1/2 rows were 6 % and
the rate-1/8 rows 16 % too high (2^16, 10 threads: 583 → 550 ms and
742 → 638 ms).

## How to run

```sh
cd ~/f2z-pcs-shalog
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=$PWD/target \
  env -u CARGO_PROFILE_RELEASE_LTO -u CARGO_PROFILE_RELEASE_CODEGEN_UNITS \
  cargo bench --bench sha256_compressions --features "unchecked span-metrics bitz-parity" --no-run
RAYON_NUM_THREADS=10 RUST_MIN_STACK=8388608 BITZ_OPENER=wfbitz-log BITZ_SHA_LOG_RUNS=1 \
  BITZ_BENCH_SHAPES=15 BITZ_BENCH_REPS=3 BITZ_BENCH_LAMBDA=100 target/release/deps/sha256_compressions-*
```

`BITZ_OPENER=wfbitz` is the standard opener; `BITZ_SHA_LOG_RUNS=2` the
two-run log opener; `BITZ_SHA_LOG_MU_LADDER=lad:r0:k0:k_rec:final` the fold
bits' ladder (one run keeps only its rate `r0`); `BITZ_SHA_LOG_A` the
recursive split. The bench prints `PPHASES` (prover phases), `LOGSIZES`
(the anatomy above) and `VPHASES` (verifier phases, `(*)_row_images` = (⋆),
`log_opener_setup` = the plan solve inside the timed verify, 0.1–0.2 ms).
Tests: `cargo test --release --features bitz-parity --lib logarithmic`
(both modes, cross-mode rejection, tampering, the three cheating provers).

Data: session scratchpad `runs/shalog3` (this table), `runs/shalog2`
(phases), `runs/shalog` (first two-run pass).
