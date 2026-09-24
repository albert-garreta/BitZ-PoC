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

The opener runs at the crate's reference split minus one row variable:
`t = ⌈0.6 n⌉ − 1`, `s = n − t` of the committed bits (`Shape::reference`
for the raw PCS, `MulLayout::wfbitz_split` for the relations; the
2026-09-24 decision). Their per-level sumchecks carry a per-row cost
(bucketing, transposes, contractions once per row, whatever its width),
so the opener wants wider rows than the forest, which is indifferent to
the split below its row cap; against that, every column variable costs
`16·2^s` bytes of integer column folds in the proof, and the verifier's
per-level work grows with `t`.

Raw PCS, 8 threads, 96-bit weights, the crate's `custom:1:4` ladder
(`examples/logup_cut_compare` on branch `lookup-wfbitz`, medians of 5;
prove ms / proof KB, the reference split `t = ⌈0.6n⌉` in the middle):

| n | −3 | −2 | −1 (the rule) | ⌈0.6n⌉ | +1 |
|---|---|---|---|---|---|
| 24 | 12:12 22.8 / 209 | 13:11 20.1 / 177 | 14:10 22.8 / 160 | 15:9 24.5 / 153 | 16:8 34.4 / 148 |
| 26 | 13:13 57.0 / 307 | 14:12 58.9 / 241 | 15:11 62.5 / 209 | 16:10 65.4 / 193 | 17:9 79.2 / 186 |
| 28 | 14:14 224 / 467 | 15:13 222 / 337 | 16:12 227 / 269 | 17:11 232 / 238 | 18:10 254 / 223 |
| 30 | 15:15 929 / 764 | 16:14 908 / 502 | 17:13 909 / 373 | 18:12 942 / 307 | 19:11 942 / 276 |

The prover's optimum sits two or three variables below the reference,
but its gain over the reference shrinks with `n` (−18 % at n = 24, −13 %
at 26, −4 % at 28, −3.6 % at 30) while the proof grows by `16·2^s` per
variable; one variable buys 2–7 % of prover time for 5–23 % of proof at
every `n`, the second buys little more time for much more proof, and one
variable in the other direction costs 10–40 %. The verifier prefers the
smaller `t` as well (n = 28: 3.2–3.7 ms at t ≤ 16, 4.4 at 17, 7.6 at 18;
n = 30: 5.3–6.3 at t ≤ 17, 8.5 at 18, 12.8 at 19). Under the rule the
raw sweep (`examples/wfbitz_bench`, 8 threads, flock's fast ladder,
`PerfRuns/wfbitz-rule-20260924c/raw_n*.txt`) reads, reference → rule,
commit + prove ms / verify ms / proof KB: n = 22 14.2 → 13.3 / 1.32 →
1.26 / 118 → 122; 24 30.0 → 24.8 / 1.73 → 1.50 / 152 → 160; 26 71.6 →
64.4 / 2.63 → 2.06 / 193 → 208; 28 255.6 → 243.2 / 4.35 → 3.47 / 237 →
270; 30 956.8 → 945.2 / 8.17 → 5.71 / 307 → 372 (the forest at 8 threads:
12.5 / 32.1 / 95.6 / 340 / 1395 ms).

In the multiplication benches the rule moves gate variables from rows to
columns relative to the forest's default split: u64 two at `2^15`, three
at `2^17`, two at `2^19`, one at `2^21`; u32 two everywhere; u128 two at
`2^15`, three above. **It stops at 64 gates per row (`s ≤ g − 6`)**: the
witness packer (`MulWitness::pack_rows`) only has its word-parallel fast
path from 64 gates per row on, and the bit-by-bit fallback below that
costs more inside the timed commit than the narrower rows save (u64
`2^15`, 8 threads: 22.7 ms with 32 gates per row against 17.6 with 64;
u128 `2^15`: 36.9 against 30.0). Only `2^15` is affected at the widths the
benches run. `extra` (`--split k`, `BITZ_U64_SPLIT_SHIFT=k`) still moves
`k` more variables from there.

## Numbers (u64 multiplication, the paper's protocol)

Campaign `PerfRuns/cs-mul-20260924-u64opt-u64-wfbitz-*` (`scripts/
run_u64_wfbitz_campaign.sh`: `mul_compare`, 5 timed reps after one warm-up,
every proof verified, peak RSS from a separate single-proof child), M5
24 GB, against the paper's forest rows (`cs-mul-20260921u-u64-bitz-*`) and
its Binius64 rows. Online prover = commitment + PIOP + opening, ms; the
forest → wfbitz columns; Binius64 (UDR) at the same rate for scale.

| gates | thr | rate | forest → wfbitz prover | ratio | verify | proof KB | peak GB | Binius64 |
|---|---|---|---|---|---|---|---|---|
| 2^15 | 1 | 1/2 | 53.7 → 45.1 | 0.84 | 2.49 → 1.52 | 137 → 142 | 0.086 → 0.051 | 66.4 |
| 2^15 | 1 | 1/8 | 55.1 → 64.2 | 1.17 | 2.36 → 1.38 | 84.1 → 88.3 | 0.093 → 0.059 | 76.7 |
| 2^15 | 8 | 1/2 | 23.1 → 17.6 | 0.76 | 2.24 → 1.67 | 137 → 142 | 0.086 → 0.051 | -- |
| 2^15 | 8 | 1/8 | 23.5 → 21.3 | 0.91 | 2.03 → 1.49 | 84.1 → 88.3 | 0.093 → 0.059 | -- |
| 2^15 | 10 | 1/2 | 23.8 → 18.2 | 0.76 | 2.25 → 1.68 | 137 → 142 | 0.086 → 0.051 | 34.8 |
| 2^15 | 10 | 1/8 | 23.7 → 21.2 | 0.89 | 2.03 → 1.53 | 84.1 → 88.3 | 0.093 → 0.059 | 36.8 |
| 2^17 | 1 | 1/2 | 186 → 123 | 0.66 | 4.52 → 1.95 | 164 → 193 | 0.251 → 0.126 | 222 |
| 2^17 | 1 | 1/8 | 213 → 155 | 0.73 | 4.34 → 1.74 | 98.5 → 126 | 0.278 → 0.158 | 272 |
| 2^17 | 8 | 1/2 | 61.3 → 39.1 | 0.64 | 3.44 → 2.26 | 164 → 193 | 0.251 → 0.126 | -- |
| 2^17 | 8 | 1/8 | 68.1 → 46.7 | 0.69 | 3.18 → 1.95 | 98.5 → 126 | 0.278 → 0.158 | -- |
| 2^17 | 10 | 1/2 | 59.6 → 38.7 | 0.65 | 3.54 → 2.33 | 164 → 193 | 0.251 → 0.126 | 82.1 |
| 2^17 | 10 | 1/8 | 64.9 → 44.2 | 0.68 | 3.13 → 2.09 | 98.5 → 126 | 0.278 → 0.158 | 91.7 |
| 2^19 | 1 | 1/2 | 732 → 516 | 0.70 | 7.58 → 3.50 | 202 → 227 | 0.869 → 0.479 | 871 |
| 2^19 | 1 | 1/8 | 867 → 632 | 0.73 | 7.38 → 3.33 | 124 → 147 | 0.986 → 0.616 | 1063 |
| 2^19 | 8 | 1/2 | 207 → 140 | 0.68 | 4.39 → 3.36 | 202 → 227 | 0.869 → 0.479 | -- |
| 2^19 | 8 | 1/8 | 232 → 165 | 0.71 | 4.20 → 3.15 | 124 → 147 | 0.986 → 0.616 | -- |
| 2^19 | 10 | 1/2 | 204 → 133 | 0.65 | 4.55 → 3.42 | 202 → 227 | 0.869 → 0.479 | 272 |
| 2^19 | 10 | 1/8 | 223 → 155 | 0.70 | 4.15 → 3.23 | 124 → 147 | 0.986 → 0.616 | 313 |
| 2^21 | 1 | 1/2 | 2884 → 2095 | 0.73 | 7.95 → 6.03 | 264 → 294 | 3.42 → 1.64 | 3430 |
| 2^21 | 1 | 1/8 | 3337 → 2605 | 0.78 | 7.76 → 5.76 | 171 → 202 | 3.85 → 2.21 | 4195 |
| 2^21 | 8 | 1/2 | 749 → 526 | 0.70 | 4.77 → 5.34 | 264 → 294 | 3.42 → 1.64 | -- |
| 2^21 | 8 | 1/8 | 875 → 640 | 0.73 | 4.46 → 5.00 | 171 → 202 | 3.85 → 2.21 | -- |
| 2^21 | 10 | 1/2 | 754 → 488 | 0.65 | 4.90 → 5.40 | 264 → 294 | 3.42 → 1.64 | 1072 |
| 2^21 | 10 | 1/8 | 810 → 603 | 0.74 | 4.66 → 5.29 | 171 → 202 | 3.85 → 2.21 | 1294 |

The rows are campaign `PerfRuns/cs-mul-20260924f-u64-wfbitz-*` (the rule
as committed, threads 1, 8 and 10 in one run; `paper/native-mul-u64-wfbitz-table.tex`
is generated from them next to the forest's 1/10-thread rows from
`cs-mul-20260921u-*` and its 8-thread rows from `cs-mul-20260924-u64opt-t8-u64-forest-*`).
Binius64 (UDR) has no 8-thread rows. The forest gains nothing from the two
efficiency cores (2^21: 749 ms at 8 threads, 754 at 10); wfbitz gains
7 % at 2^21 and nothing below 2^19.

Reading: the prover is 0.63–0.79× the forest's at 2^17 and above and
0.76–0.96× at 2^15 (the one row the packer clamp keeps at the old shape),
peak memory is 0.45–0.65×, proofs are +4–18 % at rate 1/2 and +5–19 % at
rate 1/8 (the column folds), the verifier is faster at every size except
2^21 at rate 1/2, where the per-level GKR verification of 2^17 rows costs
5.3 ms against the forest's 4.8.

## The other widths (2026-09-25, `PerfRuns/cs-mul-20260924c-u32-mod32-wfbitz-*`, `cs-mul-20260924f-u128-wfbitz-*`)

Under the rule, 8 threads, rate 1/2, forest → wfbitz, ms (ratio; proof KB
forest → wfbitz): u32 2^15 14.3 → 12.1 (0.85; 120 → 124), 2^17 34.7 → 27.9
(0.80; 152 → 163), 2^19 108 → 77.2 (0.71; 189 → 212), 2^21 393 → 270
(0.69; 225 → 274), 2^23 1962 → 1073 (0.55; 279 → 375); u128 2^15 36.4 →
30.0 (0.82; 149 → 154), 2^17 111 → 69.9 (0.63; 186 → 213), 2^19 472 → 257
(0.54; 217 → 273), 2^21 2284 → 1024 (0.45; 263 → 374). Rate 1/8: u32 15.0
→ 14.0, 37.5 → 29.6, 118 → 86.1, 454 → 324, 2306 → 1323; u128 39.1 → 36.7,
124 → 82.3, 525 → 307, 2606 → 1267. Single-threaded the ratios are the
same within a few points (u32 2^23: 6853 → 4299; u128 2^21: 7171 → 4094).
The verifier is faster everywhere (u32 2^23: 7.4 → 6.8 ms; u128 2^21: 11.6
→ 6.5) and peak memory is 0.5–0.65× the forest's. The generated tables
`paper/native-mul-table-8thr.tex` and `paper/native-mul-u128-table-8thr.tex`
carry these rows at threads 1, 8 and 10; the forest gains nothing from the
two efficiency cores on any workload (u32 2^23: 1962 ms at 8 threads vs
1939 at 10; u128 2^21: 2284 vs 2257).

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
