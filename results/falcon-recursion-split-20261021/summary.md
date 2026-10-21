# Split relation (SHA batch + glue, one commitment) against FlockRec

2026-10-21, M5 (10 cores), AC, gated (`campaign.zsh` 21:12–21:19, `rows.zsh`
21:20–21:24, clocks 3.87–4.08 GHz), binary md5 `ec7ef828…` at `c07b28ebc`.
Falcon-512 (seed 42), configuration G, base proof with the ext:3:4 standard
terminal, outer ladder ext:3:4, λ=128, 10 threads; medians of 3 reps (2 at
32K, the slower). `split S-G`: the batch's claim rows S, the glue's G.

| batch | step | blocks | SHA witness | prove | rec step | step − SHA wit. | final verify | π | π + public |
|---|---|---|---|---|---|---|---|---|---|
| 1,024 | FlockRec t18 | 5,117 | — | 0.29 s | 0.47 s | 0.47 s | 0.21 s | 170.6 KB | 190.9 KB |
| 1,024 | FlockRec t17 | 5,117 | — | 0.22 s | 0.39 s | 0.39 s | 0.20 s | 178.4 KB | 198.7 KB |
| 1,024 | FlockRec t16 | 5,117 | — | 0.20 s | 0.37 s | 0.37 s | 0.20 s | 192.8 KB | 213.1 KB |
| 1,024 | FlockRec t15 | 5,117 | — | 0.16 s | 0.34 s | 0.34 s | 0.19 s | 226.0 KB | 246.3 KB |
| 1,024 | split 17-15 | 5,117 | 0.55 s | 0.27 s | 1.12 s | 0.55 s | 0.28 s | 192.7 KB | 213.0 KB |
| 1,024 | split 16-15 | 5,117 | 0.55 s | 0.20 s | 1.02 s | 0.47 s | 0.27 s | 208.4 KB | 228.7 KB |
| 4,096 | FlockRec t18 | 6,178 | — | 0.30 s | 0.50 s | 0.50 s | 0.22 s | 169.8 KB | 204.6 KB |
| 4,096 | FlockRec t17 | 6,178 | — | 0.20 s | 0.41 s | 0.41 s | 0.22 s | 177.4 KB | 212.2 KB |
| 4,096 | FlockRec t16 | 6,178 | — | 0.17 s | 0.38 s | 0.38 s | 0.23 s | 193.9 KB | 228.7 KB |
| 4,096 | FlockRec t15 | 6,178 | — | 0.17 s | 0.38 s | 0.38 s | 0.22 s | 225.9 KB | 260.7 KB |
| 4,096 | split 17-15 | 6,178 | 0.58 s | 0.23 s | 1.11 s | 0.53 s | 0.30 s | 193.2 KB | 228.0 KB |
| 4,096 | split 16-15 | 6,178 | 0.57 s | 0.21 s | 1.09 s | 0.52 s | 0.30 s | 208.2 KB | 243.0 KB |
| 32,768 | FlockRec t18 | 7,681 | — | 0.35 s | 0.66 s | 0.66 s | 0.34 s | 178.1 KB | 330.6 KB |
| 32,768 | FlockRec t17 | 7,681 | — | 0.33 s | 0.64 s | 0.64 s | 0.33 s | 195.0 KB | 347.5 KB |
| 32,768 | FlockRec t16 | 7,681 | — | 0.25 s | 0.56 s | 0.56 s | 0.33 s | 226.3 KB | 378.8 KB |
| 32,768 | FlockRec t15 | 7,681 | — | 0.23 s | 0.54 s | 0.54 s | 0.32 s | 291.5 KB | 444.0 KB |
| 32,768 | split 17-15 | 7,681 | 0.62 s | 0.26 s | 1.33 s | 0.71 s | 0.41 s | 197.9 KB | 350.4 KB |
| 32,768 | split 16-15 | 7,681 | 0.62 s | 0.23 s | 1.31 s | 0.69 s | 0.41 s | 213.6 KB | 366.1 KB |

Split shapes: batch 2^13 at every size (63 / 75 / 94 % real compressions),
shared grid 2^26, tail 1.15 / 1.40 / 1.73 M committed bits (box rows 2^9),
glue grid 2^23 / 2^23 / 2^24 at 15 rows; PIOPs 1.8 KB (batch) + 1.3 KB
(glue); the rest is the shared opening (two folds + GKRs, one sumcheck,
ring switch, Ligerito).

## Reading

- The split's protocol is faster than FlockRec at T = 18 (0.20–0.23 s
  against 0.29–0.35 s) but its step, without the batch's witness
  generation, only ties it (0.47 / 0.52 / 0.69 s against 0.47 / 0.50 /
  0.66 s): building the pair (the SHA batch, the stub circuit, the stacked
  FlockRec map) takes 0.11–0.23 s against 0.04–0.12 s, and its final
  verifier is slower (0.27–0.41 s against 0.21–0.34 s). Its proofs are 13–
  35 KB larger (the second fold + GKR transcript; 2^11 column folds at
  claim rows 16). FlockRec at a lower T beats it at every batch on both
  axes (e.g. 1K: FlockRec t16 0.37 s at 192.8 KB against split 17-15 0.55 s
  at 192.7 KB).
- Why: FlockRec's structured opening already prices a CM SHA block like
  the block opener prices a compression (both are GKR over the derived
  cells: 20,456 cells per CM block, 16,384 per padded Ch-only instance),
  and the 2^13 batch pads with 37 / 25 / 6 % dummy compressions; the split
  adds a second PIOP (its own prime, initial and terminal grinding,
  per-round grinding), a second fold + GKR and a costlier setup.
- Proof of work: each PIOP's prime is at most min(113, 128 − rows) bits
  (BitZ's fold gate), so at λ=128 every PIOP sumcheck round grinds about
  rows + 2 bits (17 at ≤ 15 rows, 20 at 18; the initial boundary 21–24).
  FlockRec at T = 18 spends most of its PIOP time grinding: T = 17 takes the
  step from 0.47 to 0.39 s at 1K (+7.8 KB), T = 16 from 0.66 to 0.56 s at
  32K (+48 KB), T = 15 to 0.34 / 0.54 s (+55 / +113 KB).
- The batch's witness generation (0.55–0.62 s) is two serial bit-by-bit
  packing loops in `generate_sha256_compression_witnesses` (the gap-free
  source rows and the legacy assignment rows, which the product path never
  reads); a word-level packer would bring it to ~0.1 s, which does not
  change the comparison.

Reproduce:

```sh
python3 scripts/bench_gate.py run --label falcon-split -- zsh results/falcon-recursion-split-20261021/campaign.zsh
python3 scripts/bench_gate.py run --label falcon-rows -- zsh results/falcon-recursion-split-20261021/rows.zsh
python3 results/falcon-recursion-split-20261021/summarize.py results/falcon-recursion-split-20261021/runs
```
