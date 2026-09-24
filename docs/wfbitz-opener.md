# The worldfnd/BitZ scheme as an opener (`wfbitz`, branch `u64-opt`)

Date 2026-09-24. `src/wfbitz/` is the parity port of `worldfnd/BitZ`'s
polynomial commitment scheme (branch `bitz-parity`'s `src/bitz`, byte-
identical to their `dump_bitz` at every size), carried onto master's field
layer; `protocol::wfbitz_opener` discharges the crate's relations through
it — the same Spartan PIOP over a transcript-sampled prime, the same
bitification and statement binding, with the bitified functional opened by
their scheme (integer column folds in the exponent, a batched per-level
grand-product GKR, the dense reduction sumcheck, ring switching, flock's
Ligerito) instead of the crate's chunked exponent-fold forest. Both openers
are selectable; the u64 multiplication SNARK is the first user.

## Where things are

- `src/wfbitz/` (feature `bitz-parity`): the scheme. `pcs::Pcs::new`
  (their `fast` ladder as shipped) / `with_security` (any validated flock
  ladder), `BitZProver::prove` / `BitZVerifier::verify` — both take an
  optional out-of-domain claim to batch into the final opening (Round 0,
  below); `None` is their protocol byte for byte.
- `src/piop/spartan/protocol/wfbitz_opener.rs`: `WfbitzOpener::prepare::<P,
  _>(spec, ladder, target)` → the relation prefix (its security parameters
  carrying the ladder's Round-0 accounting) and the opener; `commit(rows)`,
  `prove(...)`, `verify(...)`, `opening_bits()`; `WfbitzLigerito::{Fast,
  Selected(LigeritoSelection)}` (`fast | udr:<r>:<k> | custom:<r>:<k>`);
  `WfbitzOpeningProof` (narg string, hints, the Round-0 messages).
- `MulLayout::wfbitz_split(extra)`: the row/column split this opener
  proves fastest (below).
- `examples/u64_mul_probe.rs`: `BITZ_OPENER=forest|wfbitz`,
  `BITZ_WFBITZ_LADDER`, `BITZ_U64_SPLIT_SHIFT` — per-scope prover profile,
  commit timed separately (`online prover` = commit + prove).
- `benches/mul` (`mul_bitz`, `mul_compare`): `--opener wfbitz`
  (`--ligerito fast` = the ladder as shipped); the case identity carries
  `bitz.opener`; `scripts/mul_table.py` keys the rows `bitz-wf@<rate>`.
- `examples/wfbitz_parity.rs` (the byte-parity harness against their
  `dump_bitz`/`verify_bitz`), `wfbitz_bench.rs` (their raw-PCS metrics),
  `wfbitz_root_probe.rs`; `docs/wfbitz-parity-notes.md` (the parity
  sessions' notes: the loop, the profile, the head-to-head of the two PCSs).

## How the opener plugs in

The unified runner binds the statement (and Round 0), grinds and draws the
Step-2 prime, runs the Spartan PIOP under per-draw grinding, bitifies the
terminal claim and grinds the terminal boundary. The wfbitz opener replaces
the last step: the bitified claim is a rank-one functional over the
committed bit tensor — dense row weights times the column table — and
their `LinearClaim` is exactly that; the commitment is the same flock
commitment over the same per-column bit rows under the ladder's level 0;
their fold bound `(q − 1)(2^t + 1) < 2^127` is the profile's direct-opening
prime cap at word width one. Their transcript (spongefish with a hint
channel) is forked after the terminal boundary: the fork's instance tag is
a 32-byte squeeze of the outer state, the bridge digest is absorbed again
inside, nothing is drawn from the outer transcript after the fork.

## Round 0

A Johnson ladder needs the out-of-domain sample that pins the list before
the first fold challenge (paper `a:OOD`); BitZ ships without one. Here the
crate's Round 0 runs exactly as for the crate's own opener — bound on the
outer transcript right after the statement, before the prime draw, with
the profile's grinding (`WfbitzOpener::prepare` adopts
`ood_round_bits(ladder, packed_vars)` into the security parameters as the
`step0:ood-draw` term) — and the claim `MLE[P](ζ⃗) = y` is batched into
their final Ligerito opening: `η_ood·eq(·, ζ⃗)` into the packed basis
(`add_ood_basis`), `η_ood·y` into the target, the verifier's succinct
basis evaluation adding `ood_residual_evals`; `η_ood` is drawn on the
forked transcript after the point and the value are absorbed. Off for
unique-decoding ladders. The proof carries `y` and the grinding nonce.
`opening_bits()` reports the weakest of the ladder's proximity-gap and
query terms (with their grinding), the Round-0 collision bound (with its
grinding) and the `GF(2^128)` floor.

## The split

Their per-level sumchecks carry a per-row cost (bucketing, transposes,
contractions once per row, whatever its width), so the opener wants wider
rows than the forest (which is indifferent below its row cap `t ≤ 18`).
Measured on u64, M5, 10 threads, online prover / proof bytes, moving `k`
gate variables from rows to columns:

| gates | default (t, s) | k = 0 | k = 1 | k = 2 | k = 3 | forest (k = 0) |
|---|---|---|---|---|---|---|
| 2^15 | (16, 7) | 32.6 ms / 137 KB | 24.4 / 139 | 20.0 / 142 | 17.7 / 150 | 22.6 / 137 |
| 2^17 | (17, 8) | 61.8 / 164 | 48.4 / 168 | 41.6 / 176 | 39.9 / 193 | 57.8 / 165 |
| 2^19 | (18, 9) | 162 / 201 | 141 / 210 | 131 / 228 | 126 / 258 | 184 / 202 |
| 2^21 | (18, 11) | 496 / 263 | 474 / 295 | 474 / 360 | — | 687 / 264 |

`MulLayout::wfbitz_split` takes `s = ⌊g/2⌋ + 2` capped by the default's
row cap: `k = 2` below `2^21`, the default at `2^21` (where the rows are
already 2^11 wide and another variable costs 12 % of proof for 4 % of
time). The proof grows by the `16·2^s` bytes of the column folds; the
verifier gets faster with fewer GKR levels (2^19: 3.5 ms against the
forest's 4.4).

## Numbers (u64 multiplication, the paper's protocol)

Campaign `PerfRuns/cs-mul-20260924-u64opt-u64-wfbitz-*` (`scripts/
run_u64_wfbitz_campaign.sh`: `mul_compare`, 5 timed reps after one warm-up,
every proof verified, peak RSS from a separate single-proof child), M5
24 GB, against the paper's forest rows (`cs-mul-20260921u-u64-bitz-*`) and
its Binius64 rows. Online prover = commitment + PIOP + opening, ms; the
forest → wfbitz columns; Binius64 (UDR) at the same rate for scale.

| gates | thr | rate | forest → wfbitz prover | ratio | verify | proof KB | peak GB | Binius64 |
|---|---|---|---|---|---|---|---|---|
| 2^15 | 1 | 1/2 | 53.7 → 45.6 | 0.85 | 2.5 → 1.5 | 137 → 142 | 0.10 → 0.05 | 66.4 |
| 2^15 | 1 | 1/8 | 55.1 → 65.5 | 1.19 | 2.4 → 1.4 | 84 → 88 | 0.11 → 0.05 | 76.7 |
| 2^15 | 10 | 1/2 | 23.8 → 18.6 | 0.78 | 2.2 → 1.8 | 137 → 142 | 0.09 → 0.05 | 34.8 |
| 2^15 | 10 | 1/8 | 23.7 → 21.6 | 0.91 | 2.0 → 1.5 | 84 → 88 | 0.09 → 0.06 | 36.8 |
| 2^17 | 1 | 1/2 | 186 → 156 | 0.84 | 4.5 → 2.5 | 164 → 176 | 0.34 → 0.14 | 222 |
| 2^17 | 1 | 1/8 | 213 → 174 | 0.82 | 4.3 → 2.2 | 99 → 110 | 0.37 → 0.17 | 272 |
| 2^17 | 10 | 1/2 | 59.6 → 44.4 | 0.74 | 3.5 → 2.7 | 164 → 176 | 0.25 → 0.14 | 82.1 |
| 2^17 | 10 | 1/8 | 64.9 → 48.4 | 0.75 | 3.1 → 2.3 | 99 → 110 | 0.28 → 0.17 | 91.7 |
| 2^19 | 1 | 1/2 | 732 → 516 | 0.71 | 7.6 → 3.6 | 202 → 227 | 1.30 → 0.47 | 871 |
| 2^19 | 1 | 1/8 | 867 → 625 | 0.72 | 7.4 → 3.4 | 124 → 147 | 1.42 → 0.60 | 1063 |
| 2^19 | 10 | 1/2 | 204 → 135 | 0.66 | 4.5 → 3.4 | 202 → 227 | 0.87 → 0.48 | 272 |
| 2^19 | 10 | 1/8 | 223 → 155 | 0.70 | 4.2 → 3.2 | 124 → 147 | 0.99 → 0.62 | 313 |
| 2^21 | 1 | 1/2 | 2884 → 2212 | 0.77 | 7.9 → 10.6 | 264 → 262 | 5.05 → 1.89 | 3430 |
| 2^21 | 1 | 1/8 | 3337 → 2707 | 0.81 | 7.8 → 10.2 | 171 → 170 | 5.55 → 2.39 | 4195 |
| 2^21 | 10 | 1/2 | 754 → 505 | 0.67 | 4.9 → 8.3 | 264 → 262 | 3.42 → 1.90 | 1073 |
| 2^21 | 10 | 1/8 | 810 → 626 | 0.77 | 4.7 → 8.1 | 171 → 170 | 3.85 → 2.47 | 1294 |

At 8 threads (campaign `PerfRuns/cs-mul-20260924-u64opt-t8-*`, both
openers re-measured on this tree the same day; forest → wfbitz, rate 1/2):
2^15 23.1 → 17.7 ms, 2^17 61.3 → 45.2, 2^19 207 → 141, 2^21 749 → 553;
rate 1/8: 23.5 → 21.4, 68.1 → 49.3, 232 → 166, 875 → 672. The forest
gains nothing from the two extra efficiency cores (2^21: 749 at 8 threads,
754 at 10); wfbitz gains 9 % at 2^21 (553 → 505) and nothing below 2^19.
The generated table carries the 8-thread column next to 1 and 10.

Reading: the prover is 0.66–0.85× the forest's everywhere but the
single-threaded 2^15 row at rate 1/8 (1.19×: the rate-1/8 commitment and
the fixed per-level costs dominate a 46 ms proof), peak memory is 0.45–
0.55× (one arena instead of the forest's `2^n·4 B` levels), proofs are
within 7 % (rate 1/2) / 19 % (rate 1/8, 2^19) and equal at 2^21, the
verifier is faster below 2^21 and slower at 2^21 (8.3 vs 4.9 ms: their
per-level GKR verification grows with the row count while the forest's
does not). The full table with every scheme is
`paper/native-mul-u64-wfbitz-table.tex` (generated, uncommitted).

Where the wfbitz prover's time goes at 2^21 (10 threads, 496 ms): commit 32,
PIOP 30, column folds + images 17, GKR 357 (levels ≥ 4 built 26, level-4
rebuild 14, levels 17..4 proved 57, levels 3..0 — the table-driven ones —
47 / 63 / 78 / 93), reduction sumcheck 14, ring switch 13, Ligerito 26.
At 1 thread (2142 ms) the bit rounds of levels 0–3 are 670 ms, their jit
rounds and folds 570, the dense tails 190. The bit rounds run at ≈ 0.55 ns
per 4-bit term single-threaded (one 16-byte scatter-add per term); the
jit folds and dense tails are DRAM-bound at 10 threads (≈ 2 GB of traffic
per level).

## Ladders

`fast` (their embedded ladder: rate 1/2, k = 4, Johnson, 100-bit, 16 bits
of query grinding) and the crate's `custom:1:4` give the same proof within
0.2 % and the same prover within 2 % at 2^21; `udr:1:4` saves 7 ms of
Ligerito for larger proofs. `fast` is the default.

## How to run

```sh
cd ~/f2z-pcs-u64opt   # the worktree of branch u64-opt (own target dir)
export CARGO_TARGET_DIR=$HOME/f2z-pcs-u64opt/target
RUSTFLAGS="-C target-cpu=native" cargo build --release --features span-metrics,bitz-parity --example u64_mul_probe
B=$CARGO_TARGET_DIR/release/examples/u64_mul_probe
RAYON_NUM_THREADS=10 $B 21 3                      # the forest
BITZ_OPENER=wfbitz RAYON_NUM_THREADS=10 $B 21 3   # wfbitz at its split
BITZ_OPENER=wfbitz BITZ_TRACE=1 RAYON_NUM_THREADS=10 $B 21 2   # its phases
# the paper's campaign rows through the launcher
python3 scripts/run_multiplication_benchmarks.py bitz --output PerfRuns/<label> -- \
  proof --workload u64 --opener wfbitz --ligerito fast --bitz-profile 100 \
  --log-n 15,17,19,21 --threads 1,10 --reps 5 --warmups 1 --memory rss
RUSTFLAGS="-C target-cpu=native" cargo test --release --features bitz-parity,parallel --lib wfbitz
# byte parity with their implementation (their examples built in ~/f2z-benchmark)
RUSTFLAGS="-C target-cpu=native" cargo build --release --features bitz-parity --example wfbitz_parity
$HOME/f2z-benchmark/target/release/examples/dump_bitz 22 40 /tmp/dump22
$CARGO_TARGET_DIR/release/examples/wfbitz_parity /tmp/dump22
```

## Not wired

SHA-256 (and the chain), MultiSwap (reduced discharge) and SHA-256 + ECDSA
still open through the crate's forest; the `--mul` CLI mode is u32-only.
