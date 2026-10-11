# Full Falcon commitment experiments

## Baseline recorded before implementation

Baseline: `877bc525c` on `codex/falcon-witness-preparation`, whose production implementation is `5f2149a8f`. The previous comparison and raw evidence remain in `../falcon-witness-128-20261008/`.

Latest completed run: 1,024 signatures, 128-bit security, seed 42, native release builds on Linux / AMD Ryzen 9 9950X3D. Times below are milliseconds for the simplified implementation, not the older ac2 baseline or the MacBook.

| Workload | Threads | Witness generation | Witness + commit | Total prover |
|---|---:|---:|---:|---:|
| Arithmetic 512 | 1 | 6.719 | 9.512 | 299.652 |
| Arithmetic 512 | 8 | 1.315 | 2.324 | 58.714 |
| Arithmetic 1024 | 1 | 13.522 | 19.643 | 557.442 |
| Arithmetic 1024 | 8 | 2.534 | 4.638 | 109.392 |
| Full 512 | 1 | 48.678 | 315.709 | 2593.885 |
| Full 512 | 8 | 16.790 | 72.268 | 474.161 |
| Full 1024 | 1 | 92.528 | 560.924 | 4607.440 |
| Full 1024 | 8 | 30.921 | 135.095 | 900.718 |

Generation came from separate instrumented runs. Full generation sums SHAKE and arithmetic stages within each trial, including fused SHAKE packing/auxiliary construction but excluding later arithmetic-source packing and PCS. End-to-end values are medians of process medians from three paired blocks; diagnostics used two. Each process ran two warm-ups and three measured trials. All 400 proofs across 80 processes verified and matched source roots, proof digests, inputs, payloads and capacity.

## Experiment protocol

Compare four independent patches against that implementation: Merkle-row assembly, Keccak buffer reuse, arithmetic preparation/storage, and concurrent branch encoding. Use five alternating process pairs for each of the four full-Falcon cells, with two warm-ups and three measured trials; extend ambiguous cases to ten pairs. Run diagnostics separately, including encoding and Merkle spans. Preserve inputs, source roots and proofs. Combine only measured winners, then validate the full eight-cell arithmetic/full matrix. Keep production builds and benchmark execution sequential to avoid interference.

Selection requires a reliable commit improvement in at least one cell and no statistically supported commit, total-prover or peak-RSS regression in the others. Final commit/total/prove/verify latency ratios must also have an upper 95% bound at or below 1.01; significant smaller slowdowns require investigation. The runner's historical 2% acceptance field is not the selection rule for this experiment. Intervals are paired bootstrap intervals, exploratory and unadjusted for multiple comparisons. Diagnostics explain costs but do not determine end-to-end acceptance.

Results will be appended after implementation and measurement; the numbers above are historical baseline evidence, not new measurements.

## Decisions from completed comparisons

- **Merkle-row assembly: reject.** Five paired blocks show witness + commit regressions of 0.42% (Falcon-512) and 0.31% (Falcon-1024) at one thread; neither eight-thread case shows a reliable improvement. Source roots and proofs remain identical. Encoding, rather than Merkle construction, dominates the measured commitment stages.
- **Keccak stripe reuse: reject.** One-thread witness + commit improves 1.25% / 1.83%, but eight-thread cases regress 2.67% / 2.70%. Peak RSS increases 1.1% for Falcon-512 and 2.6–2.7% for Falcon-1024. This violates the acceptance criteria. The candidate retains at most three distinct byte-buffer capacities, clears reused bytes in parallel, and recycles on success or auxiliary drop. Smaller output-array pooling was deferred because it requires broader ownership changes.
- **Concurrent branch encoding: reject.** Falcon-512 at eight threads regresses 2.92% in witness + commit and 0.39% in total prover time. Falcon-1024 at one thread regresses 0.42% in witness + commit. No cell demonstrates a reliable commitment benefit. Keeping the existing branch scheduling avoids an additional outer parallel iterator.
- **Typed signature packing: retain for final qualification.** All four full-Falcon cases improve commit and total prover time. Falcon-512 at eight threads initially had a small positive peak-RSS interval after five pairs (ratio 1.0013196785, 95% interval [1.0000103421, 1.0029975821]). Five additional fixed pairs were collected for that cell and all ten were pooled. The pooled RSS ratio is 0.9998083748, 95% interval [0.9984939245, 1.0012187568]: no significant regression. Its pooled commit improvement is 6.25% and total-prover improvement is 2.39%. All four cells pass the 1% latency gates and have no significant RSS regression. Other cells did not need an extension. The extension reused the runner's alternating order, resulting in six baseline-first and four candidate-first pairs overall.

Each candidate has a preserved patch and native-build manifest. The temporary combined source tree was used for correctness testing only; the three losing production changes have been removed. Only typed signature packing and diagnostic spans remain. There is one surviving candidate, so no combination experiment is needed. Safe arithmetic storage sharing was deferred because existing separately allocated u64 rows cannot be reinterpreted as 16-byte-aligned field storage without a broader API change or unsafe ownership conversion.

## Arithmetic qualification follow-up

The first cleaned build (`final-build.json`) exactly matches the original typed-packing candidate's source hashes and both executable hashes, so its completed full-Falcon measurements were reused rather than repeated. Four arithmetic cells were measured independently under `final/arithmetic-*`. Those runs preserve all proof identities but reveal a significant 0.28% total-prover slowdown for arithmetic Falcon-512 at one thread (95% interval +0.15% to +0.42%). Eight-thread commitment bounds are also inconclusive against the 1% gate after five pairs. This build is not accepted as the final implementation.

The changed full-Falcon packing and validation routines do not execute inside the arithmetic benchmark's timed sections. Its algebraic witness, source, transcript, validation, PCS and benchmark source files are unchanged. The only incidental formatting change in the candidate was reordering two full-Falcon module declarations. Restoring their original order produces **the same executable SHA-256 hashes for both benchmarks** (`typed-original-order-build.json`), so that ordering did not cause the timing difference. This smaller source diff is retained; it is saved in `patches/typed-original-order.patch`, including diagnostic spans.

An independent binary comparison rules out changed arithmetic instructions or static layout: baseline and candidate `.text` (3,213,557 bytes), `.rodata` (305,552 bytes), `.data`, section addresses, symbol tables and relocations are byte-identical. The complete files differ in only 22 bytes: 20 build-ID bytes and two source-location line numbers in decoding panic metadata (`format.rs`, 56→61 and 94→99). Those decoders execute before the arithmetic timed sections, and neither panic is taken on the benchmark inputs. See `arithmetic-binary-comparison.json`. The runtime cause of the measured variation has not been established. Five additional fixed pairs were collected for each ambiguous arithmetic cell (512/1, 512/8, 1024/8), and all initial observations were retained. The planned ten-pair limit is reached for those cells; no further sampling or threshold changes were used.

## Retained implementation and final comparison

The branch retains typed signature packing and direct coefficient validation, with the original module order. The three slower production experiments are removed. Full-Falcon performance passes the selection criteria. **Strict qualification across all eight cases remains incomplete:** arithmetic Falcon-512/one thread has a +0.20% total-time difference (95% interval +0.09% to +0.31%), and the eight-thread arithmetic commitment intervals do not establish the 1% bound. These numerical statuses are unchanged by the binary-equivalence finding. The code is retained locally for review; this report does not claim demonstrated arithmetic nonregression.

All values below are milliseconds, before → after, on Linux / Ryzen 9 9950X3D with 1,024 signatures and 128-bit security. Generation comes from separate diagnostic runs. End-to-end values reuse the original full-Falcon measurements because the final executable is byte-identical; arithmetic measurements include all fixed extensions.

| Workload | Threads | Witness generation | Witness + commit | Total prover |
|---|---:|---:|---:|---:|
| Arithmetic 512 | 1 | 6.649 → 6.664 | 9.487 → 9.508 | 300.583 → 301.017 |
| Arithmetic 512 | 8 | 1.403 → 1.382 | 2.421 → 2.428 | 58.637 → 58.691 |
| Arithmetic 1024 | 1 | 13.515 → 13.484 | 19.493 → 19.500 | 560.995 → 561.544 |
| Arithmetic 1024 | 8 | 2.592 → 2.618 | 4.674 → 4.644 | 109.944 → 109.815 |
| Full 512 | 1 | 41.575 → 40.593 | 309.869 → 303.894 | 2601.491 → 2592.767 |
| Full 512 | 8 | 17.200 → 17.264 | 72.238 → 67.654 | 480.848 → 468.904 |
| Full 1024 | 1 | 80.585 → 78.723 | 551.681 → 540.341 | 4635.755 → 4605.710 |
| Full 1024 | 8 | 31.122 → 30.945 | 134.565 → 125.471 | 909.515 → 886.573 |

Full generation is SHAKE + arithmetic witness time and excludes later arithmetic-source construction/packing, validation outside those spans and PCS. Arithmetic generation excludes packing and PCS. Diagnostic intervals use two paired processes per cell and are descriptive. Most of the retained change’s commit benefit comes from removing redundant signature serialization in validation/preparation; this is not evidence of a broad speedup in the core witness algorithms.

All 1,600 proofs across 320 processes verified. Public inputs, source roots, proof digests, payload sizes, capacity and protocol/source shape matched across all candidates for each workload/configuration. No final candidate cell has a statistically significant peak-RSS regression. The cleaned implementation passed 255 Falcon library tests (0 failures, 4 ignored qualification tests) and targeted formatting checks. Before removal of losing experiments, the combined candidate also passed 19 profile tests, including the normally ignored full profile matrix, and shared-source/vendor checks. The subsequent module-order restoration changes neither executable, so no additional runtime testing was needed for it.

Native release builds use `-C target-cpu=native`, LTO and one codegen unit. Test builds disable LTO and use 16 codegen units for build speed. Production builds do not enable `unsound-challenger`; that flag was needed only for existing imports in the isolated vendored prover test build. No MacBook speedup is inferred from these Linux measurements.

Reproduction: the original four full experiments use `run-experiments.py`; fixed extensions and arithmetic qualification use `run-followup.py`. Build manifests identify exact compiler/flags/source/executable hashes. `summarize.py` validates and recomputes every table and interval from the retained raw observations. `archive-evidence.py` preserves byte-verified compressed raw logs; `archives.json` records archive and member hashes. The result label `arithmetic` denotes the typed-packing experiment, whereas the `mode` field distinguishes arithmetic from full Falcon.
