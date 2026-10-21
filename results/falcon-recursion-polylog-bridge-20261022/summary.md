# Recursion over the logarithmic BitZ bridge

2026-10-22, M5 (10 cores), AC, gated (`campaign.zsh` 00:23–00:30, clocks
3.85–4.00 GHz), binary md5 `9ec94f5f…` at `92d258f2b`. Falcon-512 (seed 42,
the configuration-G case cache), configuration G, base proof with the
ext:3:4 standard-terminal main opening; with `BITZ_FALCON_BRIDGE=polylog`
the folds are committed and opened by ext:5:4. FlockRec at T = 18 and 16,
outer ext:3:4, λ=128, 10 threads; medians of 3 reps (2 at 32K, the slower).

| batch | bridge | T | base* prove | base* proof | blocks | FlockRec prove | rec step | final verify | π | public | π + public |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1,024 | standard | 18 | 73 ms | 135.0 KB | 5,117 | 0.28 s | 0.47 s | 0.20 s | 170.6 KB | 20.3 KB | 190.9 KB |
| 1,024 | polylog | 18 | 89 ms | 193.2 KB | 8,965 | 0.38 s | 0.70 s | 0.33 s | 188.2 KB | 16.6 KB | 204.8 KB |
| 1,024 | standard | 16 | 74 ms | 135.0 KB | 5,117 | 0.21 s | 0.38 s | 0.20 s | 192.8 KB | 20.3 KB | 213.1 KB |
| 1,024 | polylog | 16 | 87 ms | 193.2 KB | 8,965 | 0.28 s | 0.60 s | 0.32 s | 235.5 KB | 16.6 KB | 252.1 KB |
| 4,096 | standard | 18 | 147 ms | 184.5 KB | 6,178 | 0.29 s | 0.50 s | 0.23 s | 169.8 KB | 34.8 KB | 204.6 KB |
| 4,096 | polylog | 18 | 165 ms | 231.6 KB | 10,360 | 0.37 s | 0.72 s | 0.38 s | 186.8 KB | 19.6 KB | 206.4 KB |
| 4,096 | standard | 16 | 145 ms | 184.5 KB | 6,178 | 0.17 s | 0.38 s | 0.22 s | 193.9 KB | 34.8 KB | 228.7 KB |
| 4,096 | polylog | 16 | 162 ms | 231.6 KB | 10,360 | 0.32 s | 0.67 s | 0.37 s | 234.7 KB | 19.6 KB | 254.4 KB |
| 32,768 | standard | 18 | 787 ms | 398.0 KB | 7,681 | 0.34 s | 0.66 s | 0.34 s | 178.1 KB | 152.5 KB | 330.6 KB |
| 32,768 | polylog | 18 | 819 ms | 337.0 KB | 12,549 | 0.44 s | 0.93 s | 0.48 s | 187.5 KB | 24.0 KB | **211.5 KB** |
| 32,768 | standard | 16 | 788 ms | 398.0 KB | 7,681 | 0.24 s | 0.56 s | 0.33 s | 226.3 KB | 152.5 KB | 378.8 KB |
| 32,768 | polylog | 16 | 830 ms | 337.0 KB | 12,549 | 0.29 s | 0.78 s | 0.47 s | 235.8 KB | 24.0 KB | 259.8 KB |

`base*` = the sha256-fs base proof the circuit verifies; `blocks` = the
circuit's SHA-256 compressions; `rec step` = witness + recording
(concurrent) + early commitment + outer relation + outer witness + FlockRec
prove; `public` = the public stream plus the public inputs.

Public stream by label at 32K (label:values:bytes):

- standard: bridge folds 8,192 / 131,072 B, bridge 1,038 / 16,608 B,
  opening 432 / 2,969 B, binder 672 B, norm 624 B, joint 464 B, keyed ring
  48 B, statement 32 B;
- polylog: read-off 605 / 9,680 B, polylog bridge 469 / 7,504 B, opening
  626 / 4,323 B, binder 672 B, norm 592 B, joint 464 B, polylog joint
  368 B, read-off sums 256 B.

## Where the compressions go

One recording per cell with `BITZ_REC_STAGES=1` (`stages.sh`, the lib
test's synthetic fixture, within 1.2 % of the campaign's totals):
transcript + proof of work + Merkle per stage.

| stage | standard 1K | polylog 1K | standard 4K | polylog 4K | standard 32K | polylog 32K |
|---|---|---|---|---|---|---|
| statement | 4 | 4 | 4 | 4 | 4 | 4 |
| keyed ring | 76 | 79 | 79 | 76 | 79 | 79 |
| norm | 231 | 252 | 240 | 288 | 288 | 273 |
| binder | 240 | 246 | 251 | 272 | 299 | 281 |
| bridge | 1,325 | – | 1,485 | – | 1,725 | – |
| polylog forests | – | 2,791 | – | 3,159 | – | 3,711 |
| polylog read-off | – | 599 | – | 764 | – | 1,004 |
| polylog joint | – | 186 | – | 186 | – | 190 |
| polylog opening | – | 1,572 | – | 1,615 | – | 1,704 |
| joint | 218 | 217 | 236 | 235 | 263 | 262 |
| opening | 3,041 | 3,041 | 3,899 | 3,881 | 5,064 | 5,020 |
| **total** | **5,135** | **8,987** | **6,194** | **10,480** | **7,722** | **12,528** |
| of which transcript | 2,078 | 4,359 | 2,289 | 4,930 | 2,648 | 5,681 |
| of which proof of work | 366 | 615 | 412 | 708 | 477 | 843 |
| of which Merkle | 2,691 | 4,013 | 3,493 | 4,842 | 4,597 | 6,004 |

## Reading

- At 32K the polylog bridge takes the folds out of the public stream
  (152.5 → 24.0 KB) for 9.4 KB more π: π + public 330.6 → 211.5 KB
  (−36 %) at T = 18. It costs 63 % more circuit (7,681 → 12,549
  compressions): the step goes 0.66 → 0.93 s, the final verifier
  0.34 → 0.48 s, the base prover +30 ms. At T = 16 the step is 0.78 s for
  259.8 KB, still 71 KB below the standard bridge at T = 18.
- At 1K and 4K the folds are small (4 and 16 KB), so the polylog bridge
  does not pay off: π + public 190.9 → 204.8 KB and 204.6 → 206.4 KB, with
  a step 0.22 s slower.
- The extra 4.8K compressions at 32K: the two forests (the source forest
  from R and the fold tree over the folds' bits) cost 3,711 against the
  standard forest's 1,725; the read-off 1,004, the fold sumcheck 190 and
  the fold opening 1,704 (Merkle 1,451) are new.
- The forests cost 7 compressions per round against 4 in the standard
  bridge. This is a transcript artefact. Every round has a grinding
  boundary that absorbs public labels into a committed chaining value.
  The standard bridge's grinding domain is 58 bytes, so each round's run
  of public blocks reaches `PUBLISH_RUN` = 4: the chaining value is
  published (2 public values per round) and the labels are hashed
  natively. The polylog domain is 38 bytes, which gives 3-block runs,
  hashed in-circuit. Publishing there too would cut 1,389 compressions at
  32K for 14.8 KB more public stream. A shorter grinding boundary in the
  native protocol (fewer label bytes absorbed per round) would cut both,
  for every bridge and sumcheck.

Reproduce:

```sh
python3 scripts/bench_gate.py run --label falcon-polylog -- zsh results/falcon-recursion-polylog-bridge-20261022/campaign.zsh
python3 results/falcon-recursion-polylog-bridge-20261022/summarize.py results/falcon-recursion-polylog-bridge-20261022/runs
zsh results/falcon-recursion-polylog-bridge-20261022/stages.sh
python3 results/falcon-recursion-polylog-bridge-20261022/stages_table.py results/falcon-recursion-polylog-bridge-20261022/stages
```
