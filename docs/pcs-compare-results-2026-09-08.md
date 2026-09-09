# Archived PCS-only comparison: F2Z vs Binius64 BaseFold vs Plonky3 WHIR

These historical diagnostic results are excluded from the paper benchmark
suite. They measure terminal PCS claims, not complete multiplication proofs.
Use the [native full-proving comparison](native-mul-compare.md) for the paper's
cross-system benchmarks.

Results of `benches/baby_bear_pcs_compare.rs` and `benches/u32_pcs_compare.rs` measured on 2026-09-08 (19:10–19:41 local) on an otherwise idle machine. Both benches hand the same deterministic integer witness to each backend, which commits it in its native encoding and opens one prescribed terminal MLE claim `D · f(x, β) = V` at a transcript-derived point; the challenges live in each backend's own field. See `docs/baby-bear-pcs-compare.md` for the campaign design.

## Setup

- Machine: Apple M4 (4 performance + 6 efficiency cores), 16 GB, macOS 26.5.2; `RAYON_NUM_THREADS=8`; rustc 1.97.1; release profile with LTO and `codegen-units = 1`.
- Tree: `13f305b-dirty` (HEAD `13f305b`); tracked source files modified at run time: `README.md`, `paper/multiswap-table.tex`, `paper/raw-performance-table.tex`, `src/ligerito_flock.rs`, `src/piop/spartan/multiswap/proof.rs`, `src/piop/spartan/profile.rs`, `tests/transcript_pins.rs`.
- Command, run once per bench with `|| break` between the two:

```sh
RUSTFLAGS="-C target-cpu=native" RAYON_NUM_THREADS=8 \
  F2Z_BENCH_SHAPES="15 16 17 18 19 20 21 22 23 24 25" F2Z_BENCH_REPS=5 \
  F2Z_PCS_COMPARE_BACKENDS="f2z binius64-basefold plonky3-whir" \
  cargo bench --bench "$bench" --features bench-internals,plonky3-whir-bench,binius64-bench
```

- Shapes: `u32_pcs_compare` ran 2^15 … 2^25 as requested; `baby_bear_pcs_compare` ran 2^15 … 2^24 because the bench asserts exponents ≤ 24 (its 2^25 shape would commit 2^33 bits). Each cell: one excluded warm-up, then 5 measured trials, backends alternating per shape; every entry below is the median of the 5. Wall time: BabyBear 174 s, u32 1496 s.
- The run was gated on an idle machine (1-minute load < 1.8 on three checks 20 s apart, > 6 GB reclaimable memory, no other cargo, rustc or LaTeX process); the raw-performance sweep of the paper ran just before it.
- Traces: `F2Z_PCS_COMPARE_TRACE_PATH` pointed the zkperf JSONL traces at a session scratch directory (`idle-run-20260908-190646/`), which is not part of the repo; the tables were produced from those traces by a small median script.

### Columns

- **PCS prover** = materialize + commit + opening per trial, then the median. This is the campaign's "total PCS prover time" (`docs/baby-bear-pcs-compare.md`): *materialize* is bit packing (F2Z), native-column construction (WHIR) or row packing (Binius64); *commit* is the commitment plus transcript binding; *opening* is the opening proof (for F2Z: the claim bridge, weights, grand products, ring switch and Ligerito).
- **Claim setup** is the prescribed-point sampling and terminal-value derivation. The campaign excludes it from the prover total because it models a terminal claim delivered by the relation protocol; it is listed because for F2Z it is not small (the terminal value is an integer MLE evaluation over `F_q`).
- **Verify** is the opening verification including the linkage to `D · f(x, β) = V`. **Opening proof** is the opening proof in KB (1000 bytes); **Wire** adds the commitment and the public claim.

### Backend configurations (as recorded in the traces)

| Backend | Commitment | Challenges / evaluation | Security target | Hash |
|---|---|---|---|---|
| F2Z | GF(2^128) commitment of the witness bits (BabyBear: 31 bits per A/B/C/K value; u32: 32 X bits, 32 Y bits, 64 product bits), ring switch + Ligerito opening | evaluation field `F_q`, `q = 2^100 − 15` fixed | 100 bits (BabyBear cells: Johnson/Ligerito schedule with 445 query openings at 2^22) | BLAKE3 transcript |
| Binius64 BaseFold | one GF(2^128) row per gate, non-hiding ring switch + BaseFold, rate 1/2 | GF(2^128) (GHASH) | 100 bits (241 test queries; Diamond–Posen Eq. 42 bound plus 128-bit hash cap; estimated 100.02 bits) | SHA-256 |
| Plonky3 WHIR | native columns (four BabyBear columns A/B/C/K; three Goldilocks columns X/Y/Product), non-hiding, initial rate 1/2, folding factor 11 at 2^22 | degree-5 extension of BabyBear resp. Goldilocks | Johnson bound, internal target 106 bits, PoW capped at 12 bits (219 + 18 queries at 2^22) | Poseidon2 |

## Results

### BabyBear multiplication witness (`baby_bear_pcs_compare`, `f = [e0 | A | B | C | K | 0 | 0 | 0]`)

| log₂ N | Backend | PCS prover (ms) | Materialize | Commit | Opening | Claim setup (ms, excluded) | Verify (ms) | Opening proof (KB) | Wire (KB) | Samples |
|---|---|---|---|---|---|---|---|---|---|---|
| 15 | F2Z | **18.2** | 0.15 | 0.59 | 17.5 | 4.21 | **2.41** | **116** | 117 | 5 |
| 15 | Binius64 BaseFold | **1.46** | 0.04 | 0.62 | 0.81 | 0.08 | **0.23** | **151** | 152 | 5 |
| 15 | Plonky3 WHIR | **5.95** | 0.08 | 1.24 | 4.62 | 0.27 | **2.51** | **130** | 130 | 5 |
| 16 | F2Z | **29.0** | 0.21 | 0.94 | 27.9 | 9.49 | **2.26** | **137** | 138 | 5 |
| 16 | Binius64 BaseFold | **2.00** | 0.08 | 0.95 | 0.97 | 0.14 | **0.26** | **183** | 184 | 5 |
| 16 | Plonky3 WHIR | **10.0** | 0.19 | 2.12 | 7.71 | 0.33 | **4.71** | **215** | 215 | 5 |
| 17 | F2Z | **45.6** | 0.37 | 1.69 | 43.6 | 19.8 | **4.82** | **150** | 150 | 5 |
| 17 | Binius64 BaseFold | **2.90** | 0.16 | 1.43 | 1.31 | 0.22 | **0.29** | **214** | 214 | 5 |
| 17 | Plonky3 WHIR | **16.7** | 0.36 | 3.86 | 12.5 | 0.36 | **4.55** | **222** | 223 | 5 |
| 18 | F2Z | **72.6** | 0.73 | 2.90 | 69.0 | 39.8 | **5.32** | **166** | 167 | 5 |
| 18 | Binius64 BaseFold | **5.07** | 0.34 | 2.53 | 2.04 | 0.35 | **0.35** | **253** | 253 | 5 |
| 18 | Plonky3 WHIR | **32.3** | 0.79 | 7.43 | 23.7 | 0.43 | **8.31** | **385** | 385 | 5 |
| 19 | F2Z | **126** | 1.29 | 5.73 | 119 | 79.6 | **6.08** | **186** | 187 | 5 |
| 19 | Binius64 BaseFold | **8.79** | 0.70 | 4.52 | 3.50 | 0.67 | **0.41** | **283** | 284 | 5 |
| 19 | Plonky3 WHIR | **58.4** | 1.50 | 13.9 | 43.0 | 0.57 | **14.1** | **712** | 712 | 5 |
| 20 | F2Z | **226** | 2.67 | 11.6 | 212 | 157 | **6.64** | **208** | 208 | 5 |
| 20 | Binius64 BaseFold | **17.0** | 1.40 | 9.18 | 6.33 | 1.25 | **0.48** | **323** | 323 | 5 |
| 20 | Plonky3 WHIR | **118** | 2.97 | 27.7 | 87.6 | 0.90 | **30.6** | **1344** | 1344 | 5 |
| 21 | F2Z | **443** | 5.38 | 21.9 | 415 | 291 | **8.48** | **222** | 223 | 5 |
| 21 | Binius64 BaseFold | **34.3** | 2.90 | 20.1 | 11.4 | 2.39 | **0.53** | **361** | 362 | 5 |
| 21 | Plonky3 WHIR | **227** | 5.90 | 56.3 | 165 | 1.39 | **30.4** | **1351** | 1352 | 5 |
| 22 | F2Z | **855** | 11.2 | 45.3 | 798 | 554 | **8.25** | **259** | 260 | 5 |
| 22 | Binius64 BaseFold | **70.6** | 5.82 | 42.8 | 22.1 | 4.65 | **0.58** | **408** | 408 | 5 |
| 22 | Plonky3 WHIR | **456** | 11.8 | 112 | 331 | 2.56 | **55.4** | **2575** | 2575 | 5 |
| 23 | F2Z | **2830** | 84.9 | 124 | 2441 | 1115 | **10.6** | **276** | 276 | 5 |
| 23 | Binius64 BaseFold | **147** | 11.7 | 91.9 | 43.4 | 9.23 | **0.63** | **446** | 446 | 5 |
| 23 | Plonky3 WHIR | **911** | 23.7 | 228 | 659 | 4.67 | **102** | **4942** | 4943 | 5 |
| 24 | F2Z | **13411** | 225 | 482 | 12704 | 2241 | **13.5** | **323** | 324 | 5 |
| 24 | Binius64 BaseFold | **330** | 24.3 | 211 | 91.4 | 18.4 | **0.78** | **493** | 494 | 5 |
| 24 | Plonky3 WHIR | **1895** | 65.9 | 490 | 1302 | 9.71 | **104** | **4950** | 4950 | 5 |

### u32 × u32 → u64 multiplication witness (`u32_pcs_compare`, `f = [e0 | X | Y | Product]`)

| log₂ N | Backend | PCS prover (ms) | Materialize | Commit | Opening | Claim setup (ms, excluded) | Verify (ms) | Opening proof (KB) | Wire (KB) | Samples |
|---|---|---|---|---|---|---|---|---|---|---|
| 15 | F2Z | **14.3** | 0.15 | 0.64 | 13.4 | 3.59 | **2.63** | **156** | 156 | 5 |
| 15 | Binius64 BaseFold | **1.48** | 0.04 | 0.62 | 0.83 | 0.09 | **0.23** | **151** | 152 | 5 |
| 15 | Plonky3 WHIR | **26.4** | 0.16 | 6.85 | 19.3 | 0.34 | **4.10** | **229** | 230 | 5 |
| 16 | F2Z | **21.6** | 0.22 | 1.00 | 20.4 | 8.08 | **3.09** | **183** | 184 | 5 |
| 16 | Binius64 BaseFold | **2.02** | 0.07 | 0.98 | 0.98 | 0.12 | **0.26** | **183** | 184 | 5 |
| 16 | Plonky3 WHIR | **49.1** | 0.29 | 13.1 | 35.8 | 0.47 | **7.43** | **399** | 400 | 5 |
| 17 | F2Z | **37.7** | 0.36 | 1.79 | 35.5 | 16.1 | **5.45** | **203** | 203 | 5 |
| 17 | Binius64 BaseFold | **2.83** | 0.13 | 1.44 | 1.26 | 0.20 | **0.29** | **214** | 214 | 5 |
| 17 | Plonky3 WHIR | **94.1** | 0.57 | 25.7 | 67.7 | 0.61 | **7.55** | **408** | 408 | 5 |
| 18 | F2Z | **63.7** | 0.70 | 3.10 | 59.8 | 33.8 | **5.68** | **230** | 231 | 5 |
| 18 | Binius64 BaseFold | **4.91** | 0.29 | 2.53 | 2.05 | 0.37 | **0.32** | **253** | 253 | 5 |
| 18 | Plonky3 WHIR | **183** | 1.36 | 50.6 | 132 | 0.91 | **13.4** | **733** | 734 | 5 |
| 19 | F2Z | **115** | 1.16 | 5.85 | 109 | 67.4 | **7.13** | **263** | 264 | 5 |
| 19 | Binius64 BaseFold | **8.57** | 0.61 | 4.47 | 3.52 | 0.71 | **0.38** | **283** | 284 | 5 |
| 19 | Plonky3 WHIR | **360** | 1.81 | 98.9 | 259 | 1.58 | **25.2** | **1386** | 1387 | 5 |
| 20 | F2Z | **212** | 2.37 | 11.8 | 197 | 133 | **7.47** | **294** | 295 | 5 |
| 20 | Binius64 BaseFold | **16.5** | 1.27 | 9.04 | 6.23 | 1.32 | **0.44** | **323** | 323 | 5 |
| 20 | Plonky3 WHIR | **721** | 3.60 | 197 | 519 | 2.79 | **54.9** | **2652** | 2652 | 5 |
| 21 | F2Z | **440** | 4.80 | 22.8 | 412 | 256 | **9.14** | **321** | 322 | 5 |
| 21 | Binius64 BaseFold | **34.1** | 2.66 | 20.0 | 11.5 | 2.39 | **0.53** | **361** | 362 | 5 |
| 21 | Plonky3 WHIR | **1425** | 7.02 | 395 | 1021 | 5.35 | **54.6** | **2659** | 2659 | 5 |
| 22 | F2Z | **854** | 9.95 | 46.8 | 797 | 476 | **9.67** | **376** | 377 | 5 |
| 22 | Binius64 BaseFold | **70.5** | 5.36 | 43.0 | 22.2 | 4.83 | **0.59** | **408** | 408 | 5 |
| 22 | Plonky3 WHIR | **2962** | 18.3 | 818 | 2121 | 11.2 | **103** | **5108** | 5108 | 5 |
| 23 | F2Z | **2465** | 22.8 | 92.0 | 2293 | 952 | **12.2** | **404** | 405 | 5 |
| 23 | Binius64 BaseFold | **146** | 10.8 | 91.1 | 43.6 | 9.25 | **0.65** | **446** | 446 | 5 |
| 23 | Plonky3 WHIR | **6010** | 35.6 | 1657 | 4321 | 21.9 | **181** | **9842** | 9843 | 5 |
| 24 | F2Z | **12615** | 165 | 326 | 12010 | 2004 | **13.1** | **468** | 469 | 5 |
| 24 | Binius64 BaseFold | **315** | 21.7 | 203 | 90.6 | 18.8 | **0.79** | **493** | 494 | 5 |
| 24 | Plonky3 WHIR | **12435** | 69.9 | 3458 | 8894 | 46.6 | **181** | **9850** | 9851 | 5 |
| 25 | F2Z | **170618** | 2136 | 1564 | 166811 | 3759 | **24.5** | **514** | 515 | 5 |
| 25 | Binius64 BaseFold | **648** | 48.4 | 416 | 174 | 36.9 | **0.75** | **539** | 540 | 5 |
| 25 | Plonky3 WHIR | **25655** | 229 | 7144 | 18350 | 94.6 | **183** | **9858** | 9858 | 5 |

## Observations

- Binius64 BaseFold has the fastest PCS prover and the fastest verifier at every size on both witnesses. At 2^22 its prover takes 70.5 ms on the u32 witness against 854 ms for F2Z (12×) and 2962 ms for WHIR over Goldilocks (42×); on the BabyBear witness the same cell reads 70.6 / 855 / 456 ms. Its verifier stays below 1 ms throughout, F2Z's grows from about 2.5 ms to 13 ms, WHIR's from 2.5 ms to about 180 ms.
- F2Z produces the smallest opening proofs from 2^16 (BabyBear) and 2^17 (u32) on: at 2^22 they are 259 KB (BabyBear) and 376 KB (u32) against 408 KB for Binius64 and 2575 / 5108 KB for WHIR, whose proofs grow to about 10 MB at 2^23 and beyond under the 12-bit PoW cap.
- Almost all of F2Z's prover time is the opening (the grand products in the exponent); its commit is 5–10 % of the total. Its claim setup, excluded from the prover column, is 476 ms at 2^22 and 2004 ms at 2^24 on the u32 witness, while the other two backends' claim setup stays under 50 ms.
- The largest F2Z cells hit this machine's memory ceiling rather than the algorithm: F2Z commits 2^(e+7) bits for the u32 witness and 2^(e+8) for BabyBear, so u32 2^24, BabyBear 2^24 and u32 2^25 are 2^31, 2^32 and 2^32 committed bits, whose GKR forest alone needs 8–16 GB. The prover time jumps 5.1× from u32 2^23 to 2^24 and 14× from 2^24 to 2^25 (materialization alone 13× slower), against roughly 2× per doubling below 2^23, and BabyBear 2^23 → 2^24 jumps 4.7×. Those rows measure paging on a 16 GB box; the u32 2^25 F2Z row in particular (171 s) should not be quoted as a prover time.
- Trial-to-trial spread (median absolute deviation over the 5 trials, relative to the median prover time): on the u32 witness at most 2.3 % for F2Z up to 2^23, 4.4 % for Binius64 and 1.2 % for WHIR; on the BabyBear witness at most 1.6 % for F2Z up to 2^22, 11.5 % at 2^23 and 13.8 % at 2^24 (the two memory-bound cells), 4.9 % for Binius64 and 2.6 % for WHIR. Proof sizes vary only through grinding nonces and query positions.
