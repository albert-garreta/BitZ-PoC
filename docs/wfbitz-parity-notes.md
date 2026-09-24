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
  bucketed first table round, `a75a51e` the harness's sweep and repeat modes;
  then the 2026-09-16 session: `0b75cb0` levels 5..t−1 built from
  cache-resident level-4 rows into the one arena, level 4 rebuilt into it
  when its turn comes (peak −256 MB), `0fcd1e3` the nibble-table column fold
  in one pass + the width-1 bit round two rows per scatter, `f5c26e6` the
  Ligerito proof-of-work scan through the crate's thread-parallel
  smallest-nonce search (−20 ms at n = 28) + `examples/bitz_bench.rs`.
  Files:
  `src/bitz/{mod,transcript,codec,params,fold,gkr,forest,kernels,reduce,sumcheck,pcs}.rs`,
  `examples/bitz_parity.rs` (the harness), `examples/bitz_bench.rs` (the
  paper-metrics bench, below), `examples/bitz_root_probe.rs`,
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
# the reconstruction vs BigUint, the digit folds vs the bit walk (10 tests)
RUSTFLAGS="-C target-cpu=native" cargo test --release --features bitz-parity,parallel --lib bitz
# the sweep: every n in the list x fresh random seeds (a count draws them
# from a printed base; BITZ_SWEEP_BASE=<base> reproduces; --keep keeps the
# dumps) — their dump_bitz, our in-process check, their verify_bitz, one
# line per case, exit 1 on any failure. ~2.5 min for 22..28 x 3.
$CARGO_TARGET_DIR/release/examples/bitz_parity --sweep $B $SCRATCH/sweep 22,23,24,25,26,27,28 3
# timing: k proves, min/median, every repeat must give the same bytes
BITZ_REPEAT=20 $CARGO_TARGET_DIR/release/examples/bitz_parity $SCRATCH/dump_n28_seed41
# per-phase medians over the repeats (BITZ_TRACE prints every repeat; each
# line also carries the MB of minor page faults taken since the previous
# line — fresh pages cost ≈ 0.5 µs per 16 KB page here, ≈ 30 ms per GB,
# and do not parallelise)
BITZ_REPEAT=20 BITZ_TRACE=1 $CARGO_TARGET_DIR/release/examples/bitz_parity $SCRATCH/dump_n28_seed41 2>&1 \
  | python3 -c 'import sys,re,statistics; d={}
for l in sys.stdin:
    m=re.match(r"bitz:\s+(.*?)\s+([\d.]+)(ms|µs)\b",l)
    if m: d.setdefault(m.group(1).strip(),[]).append(float(m.group(2))*(1 if m.group(3)=="ms" else .001))
[print(f"{k:26s} {statistics.median(v):7.1f}") for k,v in d.items()]'
```

```sh
# the paper's raw-performance metrics for ONE size, own random instance
# (reference split, q = 2^100 − 15, generator X, the dump labels): median of
# 5 commits, one warm-up prove then 5 timed + verified proves (medians of
# the wall time and of every traced phase, summed into the paper's
# buckets), proof bytes, the process's peak RSS; one size per process.
# Prints a RESULT line; the comparison below was 8 threads, 20 s apart.
RAYON_NUM_THREADS=8 $CARGO_TARGET_DIR/release/examples/bitz_bench 28 --reps 5
```

**Cold vs warm.** A single traced run is a COLD prove: every large
buffer is fresh and its page faults are inside the phase (the level
build was 32 ms cold and is 12 ms warm). `BITZ_REPEAT` is WARM: libmalloc
hands a freed region back to the next request of more than half its
size, so from the second prove on nothing large faults. The headline
(min/median over repeats) and the per-phase profile below are warm;
quote the cold single run separately.

Parity so far: n = 22 and 28 with the fixed seeds 40/41 after every change,
and the sweep n = 22..28 × 3 random seeds on 2026-09-15 (base
1789509039433666000; seeds 2729483888, 2756583287, 1774055962) and twice
on 2026-09-16 (bases 1789512254890437000 and 1789512756603027000): 21
cases each, seven different (t, s) splits, all narg/hints IDENTICAL, all
accepted by both verifiers. Their `Shape::for_log_bits` gives `s ≥ 8` for every m in
the embedded-config range 22..35, so a partial 64-column group (`s < 6`)
is only reachable through the unit tests / a hand-made shape.

Notes: `CARGO_TARGET_DIR` is set globally in this environment to
`~/zinc-plus/target` (that is where `bitz_parity` lands). The dump
directories live in the session scratchpad — regenerate them each session.
Also keep the old-prime vectors around as a second check when convenient
(`dump_bitz` on `bitz-k4` before `344903c` used `2^114−11` and explicit
`(t, s)`; the current one takes `n`). Acceptance for every change: `narg` and
`hints` `IDENTICAL` on n = 22 and n = 28, our verifier accepts theirs, theirs
accepts ours, the unit tests pass (10).

## Where the time goes (2026-09-16, n = 28 at (17, 11), 10 threads)

Warm (BITZ_REPEAT=20): min 253.3 ms, median 258.9 with the fast PoW scan
(273.8 / 276.5 before it; 277.6 / 296.8 on 2026-09-15). Cold single run
≈ 278 ms, peak RSS 0.84 GB (was 304–308 ms and 0.97 GB at the session's
start, same binary flags). Their prover: 6.1–7.2 s native. Warm per-phase
medians (before the fast scan except where noted):
fold + images 8.2 (column folds 7.5 = the nibble-table fold in
`bitz::fold::fold_columns`; images 0.5);
GKR 214.5 = levels ≥ 4 build 12.3 (level-3 tables 1.0 + levels 5..16
11.2) + levels 16..5 proved 27.9 + level 4 17.4 (rebuild 6.1 + rounds
11.3) + level 3 26.3 (table round 7.1, table fold 9.9, dense tail 8.9) +
level 2 35.7 (bit round 8.2, tables 1.2, table rounds 7.2 + 9.7, tail
8.9) + level 1 43.1 (bit rounds 6.9 + 8.2, tables 1.4, table rounds 7.2 +
10.1, tail 8.9) + level 0 50.7 (bit rounds 6.3 + 7.2 + 8.5, tables 1.9,
table rounds 7.5 + 10.5, tail 8.8);
opening 53.4 (sumcheck 7.4 = combine columns 4.5 + fold rows 2.2 + rounds
0.6; ring switch 6.8; Ligerito 37.2 → 15.9 with the fast PoW scan: ≈ 21 ms
of it was flock's challenger grinding 2^16-bit query PoWs and 2^15..2^9
fold PoWs one nonce at a time on one thread with the `blake3` crate's
scalar compress; `find_pow` now calls the crate's
`utils::blake3x4::smallest_pow_nonce`, which returns exactly the serial
scan's smallest nonce, so the bytes are unchanged. BitZ's PoW prefix is
31 bytes (`bitz-pcs-pow-v1` ‖ seed), not a multiple of 4, so it takes the
kernel's thread-parallel SCALAR path, not its NEON lanes; byte-offset lanes
would take the remaining ≈ 3 ms to < 1).
Cold, the same phases carry their first touch: level build 20.6 (256 MB
of arena), level-3 tables 4.5 (64 MB), the opening +3 (90 MB inside
flock); the per-level tables of levels 2..0 recycle the freed level-3
tables and fault nothing.
Verifier: ours 5.2–5.4 ms on their n = 28 proof against their 7.1–7.4.

## Head-to-head with the crate's own F2Z (the paper's raw-performance metrics)

2026-09-16, same box (M5, 4 P + 6 E, 24 GB), 8 rayon threads, one size per
process, 20 s between sizes; both provers measured the paper's way (medians
of 5 timed reps after one warm-up, every timed proof verified, commit =
median of 5; F2Z through `f2z --sweep 20-30 --threads 8 --reps 5 --profile
custom:1:4 --cooldown 20 --latex <scratch>` at the branch's master
ac0aa44 — its own protocol is untouched here — and BitZ through
`bitz_bench`). Two rounds each in alternating order (r1 / r2), then one
BitZ round with the fast PoW scan. BitZ's smallest shape is n = 22
(`MIN_LOG_BITS`); its n = 29/30 rows have no reference proof (their prover
needs 23/46 GB), the protocol is the code parity-checked at n ≤ 28. Both
openers are flock's `fast` ladder (rate 1/2, k = 4, Johnson + OOD, 100-bit
round-by-round target, BLAKE3); the paper's buckets: grand products =
BitZ `fold+images` + `gkr`; ring switch incl. sumcheck = `sumcheck` +
`ring switch`; Ligerito = `ligerito`; Total = commit + prove. Proof bytes:
BitZ narg = every transcript message (non-Ligerito), hints = the Ligerito
proof. ms unless noted; KB = 1000 B.

| n | (t,s) | prover | commit | grand prod. | ring switch + sc | Ligerito | Total (r1 / r2) | verify (r1 / r2) | proof non-Lig / Lig / total KB | peak RSS GB |
|---|---|---|---|---|---|---|---|---|---|---|
| 20 | (12,8) | F2Z (main) | 0.33 | 5.52 | 1.21 | 0.67 | 8.02 / 7.97 | 1.69 / 1.70 | 16.7 / 81.4 / 98.0 | 0.03 |
| 21 | (13,8) | F2Z (main) | 0.55 | 7.43 | 1.44 | 1.19 | 11.1 / 11.0 | 2.06 / 2.06 | 17.5 / 90.2 / 107.8 | 0.04 |
| 22 | (14,8) | F2Z (main) | 0.66 | 11.0 | 1.95 | 1.53 | 15.8 / 15.5 | 2.85 / 2.88 | 18.4 / 100.0 / 118.4 | 0.07 |
| 22 | (14,8) | BitZ (7a9f2fa) | 0.97 | 19.6 | 0.96 | 17.7 | 39.1 / 39.4 | 1.66 / 1.63 | 15.4 / 103.4 / 118.8 | 0.04 |
| 22 | (14,8) | BitZ + fast PoW scan | 0.98 | 20.0 | 1.07 | 3.48 | 25.8 | 1.62 | 15.4 / 103.4 / 118.8 | 0.04 |
| 23 | (14,9) | F2Z (main) | 0.99 | 17.0 | 2.14 | 2.89 | 23.6 / 23.8 | 2.49 / 2.48 | 23.4 / 116.6 / 140.0 | 0.14 |
| 23 | (14,9) | BitZ (7a9f2fa) | 1.99 | 24.2 | 1.21 | 10.9 | 38.5 / 38.4 | 1.66 / 1.66 | 19.8 / 121.8 / 141.7 | 0.07 |
| 23 | (14,9) | BitZ + fast PoW scan | 1.99 | 24.3 | 1.29 | 3.50 | 31.3 | 1.62 | 19.8 / 121.8 / 141.7 | 0.07 |
| 24 | (15,9) | F2Z (main) | 1.64 | 27.8 | 3.06 | 3.23 | 36.6 / 36.6 | 3.46 / 3.40 | 24.4 / 129.4 / 153.8 | 0.24 |
| 24 | (15,9) | BitZ (7a9f2fa) | 3.00 | 33.2 | 1.87 | 28.5 | 67.4 / 67.7 | 2.11 / 2.16 | 20.8 / 131.8 / 152.6 | 0.12 |
| 24 | (15,9) | BitZ + fast PoW scan | 3.29 | 34.0 | 1.93 | 6.67 | 46.4 | 2.15 | 20.8 / 131.8 / 152.6 | 0.12 |
| 25 | (15,10) | F2Z (main) | 2.72 | 49.6 | 4.20 | 3.95 | 61.5 / 60.7 | 3.86 / 3.91 | 33.1 / 140.9 / 174.0 | 0.47 |
| 25 | (15,10) | BitZ (7a9f2fa) | 4.76 | 46.2 | 2.76 | 16.4 | 70.7 / 69.0 | 2.54 / 2.50 | 29.8 / 144.7 / 174.4 | 0.18 |
| 25 | (15,10) | BitZ + fast PoW scan | 4.67 | 46.2 | 2.81 | 5.70 | 60.4 | 2.48 | 29.8 / 144.7 / 174.4 | 0.19 |
| 26 | (16,10) | F2Z (main) | 5.11 | 86.5 | 6.62 | 5.72 | 106 / 107 | 4.61 / 4.62 | 34.5 / 160.3 / 194.8 | 0.66 |
| 26 | (16,10) | BitZ (7a9f2fa) | 6.34 | 74.5 | 4.65 | 23.1 | 109 / 110 | 3.19 / 3.16 | 30.5 / 162.7 / 193.2 | 0.36 |
| 26 | (16,10) | BitZ + fast PoW scan | 6.72 | 74.9 | 4.62 | 7.74 | 95.0 | 3.09 | 30.5 / 162.7 / 193.2 | 0.36 |
| 27 | (17,10) | F2Z (main) | 9.60 | 162 | 11.2 | 9.68 | 196 / 196 | 6.76 / 6.26 | 35.6 / 172.9 / 208.5 | 1.17 |
| 27 | (17,10) | BitZ (7a9f2fa) | 10.9 | 132 | 7.99 | 18.1 | 171 / 173 | 4.61 / 4.70 | 31.5 / 176.7 / 208.2 | 0.71 |
| 27 | (17,10) | BitZ + fast PoW scan | 10.7 | 135 | 8.11 | 9.26 | 165 | 4.67 | 31.5 / 176.7 / 208.2 | 0.71 |
| 28 | (17,11) | F2Z (main) | 19.3 | 309 | 19.2 | 11.7 | 363 / 367 | 7.26 / 7.45 | 52.6 / 186.3 / 238.9 | 2.30 |
| 28 | (17,11) | BitZ (7a9f2fa) | 19.3 | 227 | 14.4 | 35.9 | 298 / 301 | 5.07 / 5.03 | 48.8 / 188.9 / 237.7 | 1.25 |
| 28 | (17,11) | BitZ + fast PoW scan | 19.1 | 233 | 14.5 | 15.6 | 284 | 4.99 | 48.8 / 188.9 / 237.7 | 1.25 |
| 29 | (18,11) | F2Z (main) | 38.6 | 649 | 39.2 | 17.9 | 750 / 802 | 10.22 / 9.80 | 54.0 / 206.4 / 260.4 | 4.28 |
| 29 | (18,11) | BitZ (7a9f2fa) | 40.1 | 430 | 28.1 | 53.5 | 556 / 568 | 8.18 / 8.30 | 49.6 / 209.5 / 259.1 | 2.50 |
| 29 | (18,11) | BitZ + fast PoW scan | 41.3 | 444 | 28.2 | 25.4 | 543 | 8.31 | 49.6 / 209.5 / 259.1 | 2.50 |
| 30 | (18,12) | F2Z (main) | 73.8 | 1313 | 84.2 | 33.3 | 1506 / 1539 | 9.88 / 10.55 | 87.4 / 221.1 / 308.5 | 7.94 |
| 30 | (18,12) | BitZ (7a9f2fa) | 78.5 | 821 | 49.9 | 81.4 | 1035 / 1061 | 8.85 / 9.06 | 83.1 / 224.8 / 307.9 | 4.41 |
| 30 | (18,12) | BitZ + fast PoW scan | 81.9 | 834 | 50.8 | 44.2 | 1015 | 8.96 | 83.1 / 224.8 / 307.9 | 4.40 |

| n | BitZ/F2Z total (committed) | BitZ/F2Z total (fast scan) | grand prod. | ring+sc | Ligerito (committed → fast) | verify | proof | RSS |
|---|---|---|---|---|---|---|---|---|
| 22 | 2.51 | 1.65 | 1.78 | 0.49 | 11.61 → 2.27 | 0.58 | 1.003 | 0.61 |
| 23 | 1.62 | 1.32 | 1.42 | 0.57 | 3.77 → 1.21 | 0.67 | 1.012 | 0.48 |
| 24 | 1.85 | 1.27 | 1.20 | 0.61 | 8.82 → 2.07 | 0.61 | 0.992 | 0.51 |
| 25 | 1.14 | 0.99 | 0.93 | 0.66 | 4.14 → 1.44 | 0.66 | 1.002 | 0.39 |
| 26 | 1.03 | 0.89 | 0.86 | 0.70 | 4.04 → 1.35 | 0.69 | 0.992 | 0.55 |
| 27 | 0.88 | 0.84 | 0.81 | 0.71 | 1.87 → 0.96 | 0.68 | 0.999 | 0.61 |
| 28 | 0.82 | 0.78 | 0.73 | 0.75 | 3.07 → 1.34 | 0.70 | 0.995 | 0.54 |
| 29 | 0.72 | 0.70 | 0.66 | 0.72 | 2.99 → 1.42 | 0.80 | 0.995 | 0.58 |
| 30 | 0.69 | 0.67 | 0.63 | 0.59 | 2.44 → 1.33 | 0.90 | 0.998 | 0.55 |

Reading: at n ≤ 24 the crate's merged forest is 1.2–1.8× faster than
BitZ's per-level GKR (its bit/table rounds have per-level fixed costs);
from n = 25 the per-level design wins and the gap widens to 0.63× at
n = 30 (the forest's in-tree rounds are the crate's known 66–85 % block).
BitZ's ring switch + sumcheck is 0.5–0.75×, its verifier 0.6–0.9×, its
peak RSS ≈ 0.5× (the one arena vs the forest's `2^n·4 B` levels), and
the proofs are the same size within 1 % at every n (same opener, same
message count). The Ligerito column was 2–12× before the fast PoW scan
and is 1.0–1.4× after it — what remains is the 31-byte-prefix scalar
path plus flock's engine scaling ≈ 2× on threads either way. The round 1
→ round 2 drift is ≤ 3 % except F2Z n = 29 (+7 %, thermal).

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
  from `fold_table(3, 0, [])`. The arena is the only large buffer of the
  GKR and is touched once: `upper_levels_into` first fills it with levels
  5..t−1 (each at `upper_region(ℓ)`) — the first three from level-4 rows
  computed 64 columns at a time into a task-local buffer and never stored
  (a task owns the whole subtree above one row of level 7), the rest by
  `level_up_into` from the level below — and when level 4's turn comes it
  is rebuilt as pairwise products of level 3's table values into the same
  arena (`product_level_into`, 6 ms) and proved in place. `prove_bit_level`
  / `materialise_*` / `level_up` remain only for t < 6.
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
  — terms only bucket `eq_c` (width 2: the (E_lo, E_hi, O_lo, O_hi) tuple
  byte straight out of one block transpose of the corner words; width 4:
  two transposes of (O, E) word pairs give `pat_E·16 + pat_O`, the cross
  pairs by swapping nibbles), buckets per rayon split (`Buckets`, `map_init`),
  products once per row (`contract`). Width 1 (`bit_row_pair`): two rows'
  4-bit tuples fill one byte index out of one transpose, so a column's
  weight is scattered once for both rows; a tuple's per-row coefficient is
  `w·E_end·O_end` (`w·(E_hi+E_lo)(O_hi+O_lo)` for ∞) and the pair's is the
  SUM of the two rows', so the contraction only needs the two 16-entry
  marginals of the 256 buckets (`kernels::dot`) — this is why widths 2 and 4
  cannot pair: their ∞ coefficient is a product of two per-row factors.
  `send_one = (z == 0)` selects the `hh` corner for the endpoint sum;
  `factor` accumulates `eq(r, z)`.
- `sumcheck.rs`: their m-round inner-product sumcheck computed as t rounds on
  the column-combined `2^t` table (`xi_combined_rows_packed`) + s rounds on the
  row-folded `2^s` table (`fold_rows_point`: `eq(b) = eq(b mod 64)·eq(b div
  64)`, one 32 KB byte table + a multiply per word); messages `[a0, a1, a2]`.
- `fold.rs`: the integer column folds (`fold_columns`: nibble tables
  `T[d][v] = Σ_{i∈v} w_{4d+i}` over every nibble position of the rows, 8 MB
  at 2^17 rows, then one pass over the words, 128 columns at a time with
  the block's accumulators live and a fixed 16-lookup trip per word), the
  images (parallel), and the verifier's reconstruction as a wide u128
  remainder (`U128::widening_mul` + `rem_wide_vartime`, sum by `add_mod`).
- `pcs.rs`: their statement frames, ring switch, Ligerito via a flock
  `Challenger` framed their way (its `grind_pow` = their
  `blake3("bitz-pcs-pow-v1" ‖ seed ‖ nonce_le)` smallest-nonce PoW through
  the crate's parallel scan), proof as a bincode-fixint hint.

## Scaling and floors (measured 2026-09-16, n = 28, warm)

`RAYON_NUM_THREADS` 1 / 4 / 6 / 10: prove 902 / 362 / 312 / 281 ms, GKR
758 / 295 / 249 / 219, opening 107 / 59 / 59 / 55, fold 39 / 12 / 10 / 8
(the box is 4 P + 6 E cores; a cache-resident product kernel scales
3.8× from 1 to 10 threads, a streaming one 1.6× — about 118 GB/s).
Per phase, 1 → 10 threads: bit rounds 5–6× (compute-bound scatters, ≈
0.15 ns per scatter aggregate); table rounds 3.5× (µop-bound: lookups,
loads, stores, ≈ 40 PMULL per pair index); dense tails 2.6× (three
quarter-sweeps ≈ 0.77 GB per level at ≈ 84 GB/s — the memory wall);
opening 2× (flock's Ligerito, partly serial). Single-thread work: level 0
= 212 ms, the bit rounds ~34 % of all single-thread GKR work and ~21 % at
10 threads. Every remaining inner loop (scatters, table lookups, nibble
lookups, products) runs at ≈ 0.11–0.15 ns per elementary operation
aggregate, i.e. the µop rate of 4 P + 6 E cores; the levers left are
fewer operations per term, not faster ones.

Page faults, measured: a fresh 16 KB page costs ≈ 0.5 µs (≈ 30 ms per
GB, 256 MB = 7.7 ms) and the cost does not parallelise (10 threads 7.6 ms
vs one thread 9.5 ms for 256 MB); a refill of a touched buffer is free of
it. libmalloc recycles a freed large region for a later request of more
than half its size (exact sizes always; 128 MB after a freed 256 MB never),
which is why levels 2..0's 64 MB tables recycle the level-3 tables and why
`BITZ_REPEAT` is warm from the second prove on. The earlier note that
"first touch is not a cost when the fill is parallel" was wrong — it
compared two fills that both recycled.

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
- Level build (2026-09-16): emitting levels 5..7 in the level-4 pass
  instead of the `level_up` chain is worth nothing WARM (the chain's
  re-reads stream at ≈ 118 GB/s; the whole levels ≥ 4 block is 14.7 ms
  warm either way) — the 32 ms the old profile showed was 512 MB of first
  touch. Keeping level 4 in the arena from that pass and the upper levels
  in a second buffer (no rebuild) is 3.3 ms (1.2 %) faster warm than the
  committed layout and 1.2 ms slower cold, for +2^{t−4+s} entries of
  peak (256 MB here, 4 GB at n = 32) — the memory won. Fusing level 4's
  first round into the rebuild: +1 ms (the rebuild is compute-bound, the
  round's slots no longer hide behind a memory stream). JIT-proving level
  4 from the level-3 tables (products of two lookups per corner) costs
  more lookups+products than the 256 MB it saves.
- Column fold (2026-09-16): byte tables (64 MB) fold as fast as nibble
  tables (7.3 ms) and cost their first touch; 2048 columns per block 9.0 ms
  vs 7.3 at 128 (the block's word lines plus the tables must stay in L1).
- Considered and costed, not applied (each ≤ 2 %): the width-1 bit round
  as weighted popcounts (`W(row)` for all rows = `xi_combined_rows_packed`,
  4.3 ms, plus four pair-AND popcounts per (row, group): ≈ 6 ms vs 9.7 —
  superseded by the two-rows-per-scatter form, 6.3 ms);
  12-bit buckets `(a_lo, b_lo, b_hi)` for the width-4 rounds (two scatters
  per column instead of four, but 128 KB of bucket clearing and
  marginalising per row — a wash below 2048 columns); preprocessed eq
  weights in the Gruen slot (−2 of ~40 PMULL); a third table-driven round
  (arity-4 folds from 16 lookups per pair index, saves one quarter-sweep);
  two rounds per pass (same traffic as the fused scheme, more multiplies).

## Next steps (ideas, by expected gain — all small now)

1. Table rounds (4 × (7.2 + 10) = 69 ms, the largest block) and dense tails
   (4 × 8.9): µop-bound at the rate above. Untried: reduced 16-byte
   buckets in `jit_bucket_group` (2 `mul_red` instead of 2 `clmul_256`,
   three 16-byte RMWs instead of three 32-byte ones — ≈ −3 of ~45 µops per
   term, PMULL-heavier); the width-2 rounds' per-row contraction (256
   marginalising adds + 80 products per row ≈ 20 % of the round).
2. Bit rounds: widths 2 and 4 cannot share an index between rows (their
   ∞ coefficient is a product of two per-row factors); what is left is
   the scatter itself (≈ 2.3 cycles per 16-byte RMW).
3. Column fold (7.5 ms): at the lookup floor (2^22 words × 16 nibble
   lookups); only fewer lookups would move it, and byte tables did not.
4. Opening: `xi_combined_rows_packed` (4.5 ms) is 2^17 rows × 32 groups × 8
   byte lookups with a data-dependent exit — a fixed trip count may shave
   ~20 %. Ligerito (15.9 ms after the fast PoW scan) and the ring switch
   (6.8) are flock's engine, outside `src/bitz`; the PoW's 31-byte prefix
   could get NEON lanes (byte-offset nonce placement in
   `utils::blake3x4`, ≈ −2 ms, a crate-utility change).
5. Measure properly: `BITZ_REPEAT` (warm) with the per-phase medians, a
   separate cold single run, `RAYON_NUM_THREADS`, interleave binaries in
   one thermal window (the box drifts ≈ 10 ms between back-to-back
   20-repeat runs); keep the pre-change binary aside for A/B.

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
- The arena is exactly `2^{t−4+s}` entries: `upper_levels_into` and
  `product_level_into` fill its spare capacity from empty (the levels
  5..t−1 use all but two rows of it; level 4 all of it), `jit_fold_round`
  fills through the spare capacity the first time and reuses the
  initialised prefix after (its length must already be `2^{t−4+s}`). The
  upper levels are proved in place inside their `upper_region`s before
  level 4 overwrites them; `RowsPtr` hands tasks disjoint rows of the
  uninitialised buffer.
- `BITZ_TRACE`'s fault column is a delta since the previous line; the
  baseline is reset at prove start (`trace_start`), so lines before it
  (the harness's own) are not meaningful.
- `Buckets` / `SumBuckets` are per rayon split (`map_init`); the
  `parallel`-off path keeps one.
- `Fq` arithmetic on our side is `u128` residues with a runtime modulus;
  their `Fq<Q>` encodes as 16 LE bytes of the lifted value.
- Shell: `grep pattern $F` with an empty `$F` reads stdin and hangs the step.
- Never call `git commit` without `--no-gpg-sign` here; paper edits on the
  tree stay uncommitted (user directive) — add only `src/bitz`, `examples/bitz_*`,
  `docs/bitz-*` and manifests.
