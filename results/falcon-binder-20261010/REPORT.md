# Falcon binder experiments — final production validation

Implementation commit: `d0c3f84044ee946ee727c4fcb0bf20d40b409455` on `codex/falcon-binder-experiments`.

**Status: complete for the tested host and public shape.** The final production campaign has 100 verified processes and 165 passing release tests. The production candidate was built without `bench-internals`. Its public default selects Words at N=512, batch=1024, security=128 independently of worker count; kernel dispatch is qualified at workers 1/2/4/8/16. The frozen V1/V2 campaigns remain historical evidence below.

The final default reduces arithmetic total proving time by 23.19–41.29% and full total proving time by 0.92–8.40% across the tested worker counts. The gains combine binder policy changes, the zipped integer-product base-case loop, and coalescing tiny Blake3 absorption writes while preserving transcript bytes. Complete word proofs remain larger for two of the five fixtures. Verifier and memory conclusions are limited to the paired observations and intervals below; this report does not claim universal nonregression.

**Method and scope.** Baseline commit: `51b472c55bfeeda4bccf6d9986345fbeb54953e5`. All campaigns use Falcon-512, batch 1024 and security 128. The screen has seeds 42–44, workers 1/2/4/8/16, one warmup and three measured trials per process; the holdout has fresh seeds 45–49, workers 1/4/8/16, one warmup and five measured trials. Final production uses seeds 45–49, workers 1/2/4/8/16, one warmup and five measured trials per process. These are the previously used V2 seeds, so final production is a confirmation campaign, not an independent fresh holdout for the later transcript optimization. Processes run sequentially with randomized block/configuration order (order seed 20261010). Stage tracing is disabled in timing campaigns and retained separately for diagnosis.

Each process contributes one timing median; candidate/baseline ratios are paired by seed and combined geometrically. Each five-seed campaign has five paired seed blocks, not 25 independent trial iterations. The holdout and production reports enumerate all 5^5 paired bootstrap resamples to obtain descriptive 95% intervals; paired-log Student-t intervals are also retained as a sensitivity check. These are single-host, non-simultaneous intervals with only five seeds. Frozen V2 has no two-worker holdout; final production separately validates two workers.

Total prover time is witness/commit plus prove time. Each total process statistic is the median of complete trial totals, so separate column medians need not sum exactly. RSS is the maximum high-water observation across the entire process, including warmup, fixture setup, allocator retention and proof-digest formatting. It does not isolate binder memory. The original runner took the median of sampled high-water marks; the holdout analysis independently used their maximum, which differs for 104/120 holdout processes. The final runner is corrected and agrees with the independent analysis; maximum versus median differs for 87/100 production processes.

**Hardware and build.** AMD Ryzen 9 9950X3D, 16 physical cores, 32 logical CPUs, SMT available, boost enabled, Linux 6.17.0-41-generic. Worker CPU order is `6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9`; the main thread is pinned to CPU 6. The working set crosses the CPU cache topology at higher worker counts, so eight-versus-sixteen-worker absolute timings should not be interpreted as a universal scaling law. Builds use release profile, `RUSTFLAGS="-C target-cpu=native"`, rustc 1.98.1 (`48a229cea`, LLVM 22.1.8). Frozen baseline/V1/V2 benchmark builds use features `falcon-hybrid,bench-internals`; the final production candidate uses `falcon-hybrid` without `bench-internals`. Production benchmark overrides are `auto`, and the binary executes its public defaults.

**Final production versus baseline.** Each absolute value below is the median of five process medians. Percentage changes are geometric means of seed-paired production/baseline ratios; brackets show descriptive exact-enumeration 95% bootstrap intervals. Positive reductions mean faster; positive verifier/RSS changes mean more cost.

| Production policy | Workers | Total ms, baseline → production | Total reduction [95% interval] | Prove-only reduction | Commit reduction | Verifier change | Lifetime RSS change |
|:--|--:|--:|--:|--:|--:|--:|--:|
| arithmetic / words | 1 | 282.93 → 181.91 | 35.63% [35.19, 35.96] | 35.88% | 33.75% | -0.60% | -11.89% |
| arithmetic / words | 2 | 176.10 → 105.42 | 40.10% [39.91, 40.22] | 40.80% | 34.21% | -31.90% | -12.45% |
| arithmetic / words | 4 | 110.57 → 64.81 | 41.29% [40.93, 41.65] | 42.03% | 33.78% | -40.73% | -12.22% |
| arithmetic / words | 8 | 59.96 → 45.95 | 23.19% [22.76, 23.62] | 22.69% | 29.63% | +0.56% | -0.12% |
| arithmetic / words | 16 | 75.18 → 49.86 | 33.77% [33.52, 33.98] | 34.45% | 21.62% | -47.21% | -12.45% |
| full / replay | 1 | 2000.49 → 1983.23 | 0.92% [0.85, 1.00] | 0.31% | 3.88% | +0.27% | +0.01% |
| full / replay | 2 | 1173.41 → 1072.86 | 8.40% [8.26, 8.53] | 9.27% | 3.62% | -1.02% | +0.05% |
| full / replay | 4 | 669.02 → 628.12 | 6.16% [5.94, 6.42] | 6.76% | 3.53% | -1.02% | -2.85% |
| full / replay | 8 | 429.70 → 404.93 | 5.94% [5.59, 6.34] | 6.57% | 2.52% | -1.04% | +0.07% |
| full / replay | 16 | 348.75 → 336.15 | 3.65% [3.32, 4.21] | 4.14% | 1.84% | -1.38% | +0.02% |

The lowest observed final total-latency medians among the requested worker counts are arithmetic at 8 workers (45.95 ms) and full at 16 workers (336.15 ms). This ordering describes this pinned host/workload and is not a statistically established or portable optimum. Absolute production commit/prove/verify times and RSS are in `production-analysis.md`; every paired metric and interval is in `production-analysis.json` and `production-summary.csv`.

Final 1-worker word verification changes by -0.60%: 44.545 → 44.270 ms, paired ratio 0.9940 [0.9889, 0.9992]; paired-log t sensitivity interval [0.9859, 1.0023]. The earlier V2 slowdown is absent in this final descriptive interval. The sensitivity interval crosses parity, so this improvement is method-sensitive with five seeds.

Final 8-worker word verification changes by +0.56%: 20.124 → 20.262 ms, paired ratio 1.0056 [0.9947, 1.0156]; paired-log t sensitivity interval [0.9892, 1.0224]. The interval crosses parity, so strict verifier nonregression remains unresolved.

Full one-worker verification has a +0.27% paired point estimate, ratio 1.0027 [0.9907, 1.0153]; parity remains unresolved. The earlier V2 eight-worker arithmetic increase was +3.10%, so the final +0.56% estimate is reduced, but the data do not establish its removal. No universal or all-metric nonregression claim follows.

| Variant | Workers | Lifetime peak RSS ratio [95% interval] |
|:--|--:|--:|
| arithmetic | 1 | 0.8811 [0.8744, 0.8885] |
| arithmetic | 2 | 0.8755 [0.8740, 0.8768] |
| arithmetic | 4 | 0.8778 [0.8766, 0.8790] |
| arithmetic | 8 | 0.9988 [0.9954, 1.0022] |
| arithmetic | 16 | 0.8755 [0.8740, 0.8767] |
| full | 1 | 1.0001 [0.9998, 1.0003] |
| full | 2 | 1.0005 [0.9991, 1.0027] |
| full | 4 | 0.9715 [0.9634, 0.9796] |
| full | 8 | 1.0007 [0.9968, 1.0041] |
| full | 16 | 1.0002 [0.9983, 1.0016] |

Intervals crossing parity do not establish strict memory nonregression. Lifetime process RSS includes fixture construction, retained allocations, and digest formatting; it does not isolate binder memory or establish a steady-state bound. These five-seed intervals are descriptive and are not corrected for multiple comparisons.

**V1 factorial screen.** `rXYZ` denotes folded replay, factored construction, and batched evaluation respectively. Each effect below toggles one flag, averages geometrically over the other two flags and three seeds, and reports total-prover reduction. Twelve matched configuration pairs remain clustered in three independent seed blocks. Construction/evaluation include their parallel dispatch policies.

| Workers | Folded replay | Factored construction | Batched evaluation |
|--:|--:|--:|--:|
| 1 | 2.50% | 1.53% | 1.53% |
| 2 | 3.66% | 5.75% | 2.96% |
| 4 | 3.24% | 10.72% | 5.69% |
| 8 | 2.39% | 16.31% | 8.38% |
| 16 | 1.67% | 17.12% | 8.14% |

Construction and evaluation account for the largest multithread effects. At one worker the baseline already enables the three bit kernels. At eight workers it already enables construction/evaluation; this explains the smaller incremental kernel gain in that baseline comparison. The V1 screen selected word binding with optimized construction and full replay for holdout.

**V1 problems and V2 corrections.** The original word implementation increased eight-worker process RSS by 13.75% in the screening statistic. V2 reuses its two Λ coefficient allocations for the coefficient-round folds, then creates the compact side/instance table, reducing the estimated target-shape binder live allocation by roughly 17.5 MiB. The final V2 observed eight-worker lifetime-peak ratio is 0.9958 [0.9935, 0.9986] versus baseline; the regression is resolved for these fixtures and this process lifetime.

V1 also added about 8 ms to one-worker witness/commit time, with diminishing overhead as workers increased. Relocation-normalized disassembly found identical instruction sequences in the integer product and the other inspected commit kernels. One candidate scalar-loop branch straddled a 64-byte boundary; `commit-alignment-notes.txt` records that as an unconfirmed alignment hypothesis, not a demonstrated cause. V2 rewrites the small schoolbook base case with a zipped output/input iteration. The measured commit-stage regression disappears while commitments and proofs remain unchanged. In V2 the arithmetic commit stage is about 23–35% faster than baseline, so whole-prover improvements include this change and must not all be attributed to the binder.

**Frozen V2 holdout versus baseline.** Median timing values below are medians of the five process medians. Percentage changes use the geometric mean of the paired ratios. Positive reductions mean faster; positive verifier/RSS changes mean more cost.

| Variant / policy | Workers | Total ms, baseline → V2 | Total reduction | Prove-only reduction | Verifier change | Lifetime RSS change |
|:--|--:|--:|--:|--:|--:|--:|
| arithmetic / r111 | 1 | 281.43 → 270.76 | 4.05% | -0.02% | +0.82% | -0.35% |
| arithmetic / word | 1 | 281.43 → 183.02 | 35.18% | 35.37% | +0.65% | -12.87% |
| arithmetic / r111 | 4 | 111.08 → 86.65 | 21.75% | 20.52% | -40.43% | -12.01% |
| arithmetic / word | 4 | 111.08 → 65.55 | 40.72% | 41.31% | -39.50% | -11.98% |
| arithmetic / r111 | 8 | 59.77 → 56.52 | 5.24% | 2.84% | +1.04% | +0.01% |
| arithmetic / word | 8 | 59.77 → 46.37 | 22.37% | 21.46% | +3.10% | -0.42% |
| arithmetic / r111 | 16 | 75.08 → 54.63 | 26.68% | 26.94% | -47.85% | -11.95% |
| arithmetic / word | 16 | 75.08 → 49.57 | 33.66% | 34.28% | -46.48% | -11.72% |
| full / replay-on | 1 | 2000.19 → 1988.28 | 0.57% | -0.19% | -0.76% | -0.02% |
| full / replay-on | 4 | 670.10 → 628.29 | 5.82% | 6.42% | -0.32% | -3.12% |
| full / replay-on | 8 | 430.20 → 405.53 | 5.84% | 6.68% | -0.14% | +0.05% |
| full / replay-on | 16 | 349.90 → 336.98 | 3.63% | 4.27% | -0.15% | +0.24% |

| Workers | Word total reduction, 95% interval | Full replay total reduction, 95% interval |
|--:|--:|--:|
| 1 | 34.83–35.47% | 0.41–0.70% |
| 4 | 40.17–41.12% | 5.56–6.13% |
| 8 | 21.65–22.97% | 5.59–6.07% |
| 16 | 33.24–34.07% | 2.96–4.44% |

Within the V2 candidate, replay adds 2.45/4.31/2.81/1.53% total-prover improvement over r011 at 1/4/8/16 workers. Word adds 32.44/24.25/18.08/9.51% over r111. Those within-candidate comparisons isolate the selected policy more closely than comparisons with the older binary.

**Frozen V2 verifier and memory limits.** Word verification rises 0.65% at one worker (ratio interval 1.0020–1.0110), about 0.289 ms, and 3.10% at eight (1.0193–1.0442), about 0.624 ms. These are measured V2 regressions despite the larger prover gains. Word verification remains much faster than baseline at 4/16 workers, although it is 1.57/2.03/2.63% slower than V2 r111 at 4/8/16. The final production build coalesces tiny transcript absorption writes; its independently measured verifier results appear in the production section above. The V2 figures remain unchanged historical evidence.

At one worker r111 prove-only time is essentially unchanged and its total gain comes from commitment. Full one-worker replay is already enabled in the baseline; V2 prove-only time rises 0.19%, while faster commitment yields the 0.57% total gain. Full lifetime RSS at 1/8/16 has intervals crossing parity. The 8/16 point estimates are +0.05/+0.24%, roughly +1.1/+5.6 MiB on >2 GiB peaks, and cannot establish strict memory nonregression. Full one-worker baseline high-water RSS rises from about 1.66 GiB after sample one to 2.24 GiB after sample five; this process-level observation does not prove a leak or a steady-state bound.

**Complete proof payload.** Bit and full policies preserve the complete proof payload. Word binding has its own protocol domain and changes the transcript, so PCS paths can change the complete proof size even when the norm-binding component shrinks. The following complete proof sizes apply to both V2 and final production and are identical at every worker count:

| Seed | Baseline bytes | Word bytes | Difference |
|--:|--:|--:|--:|
| 45 | 262402 | 262762 | +360 |
| 46 | 264194 | 263690 | -504 |
| 47 | 264802 | 261706 | -3096 |
| 48 | 263042 | 262762 | -280 |
| 49 | 263746 | 263946 | +200 |

Mean payload decreases by 664 bytes, but seeds 45 and 49 grow by 0.137% and 0.076%. Neither complete-proof-size nonregression nor universal verifier nonregression is supported by V2. The final default retains this complete-payload tradeoff; a strict per-fixture complete-proof-size nonregression requirement is not met. Final verifier measurements are reported separately above.

**Functional evidence.** All 480 processes and 2320 trials across the 240-process screen, 20-process V2 pilot, 120-process V2 holdout, and 100-process production campaign verified. The final audit revalidates raw stdout hashes, expected trial counts, public shape/security/seed and CPU-affinity readback, zero exit status, absent stage traces, and stable identities through warmup and measured trials. It also checks frozen production binary, runner, and helper hashes and independently reconciles all runner timing/RSS summaries.

Complete proof digests and payloads agree across kernels, worker counts, and revisions within each actual protocol. Production is explicitly checked to emit the established word/full protocol IDs; classification does not rely on the `word=auto` label. Input/source commitments agree across word/bit protocols. Word defaults use their v6 protocol domain; the explicit Bits API retains v5 compatibility. Thus byte-preserving transcript coalescing retains the earlier word-v6 and full proofs for these fixtures. These functional checks do not replace protocol soundness review.

**Frozen binary identities (SHA-256).**

| Revision | Arithmetic | Full |
|:--|:--|:--|
| Baseline | `f1e59fb4d807d8f1e5d7cdc55207b477b8f09c6da01384334c6d3e4bbf3276a0` | `e650313ef17c4a324294327e11f42f1648a033fe93d0c4dd9b29852004b33741` |
| V1 screen | `d113e631e6dabf2b78aad512c1dc6f1d893aaa2ebb25ea5901f3967116940f67` | `96b04b50a1205901bb85eb9b59728628ff1f3e9c87b06e0777df7d4327f2ecf4` |
| V2 holdout | `14f919ec75f7361286d58aa5b703990bd5b6291beccaa567397f968d5ae4d5aa` | `1446935e24073a11f3b40b77f158ba17793ea3f9d42f1627f889dba75862613f` |
| Final production | `6831f91eff12dfa61f90f5c244c298a939dfa1f1ff6e8f71985cf306e965c827` | `79dd92b01e5bc2f1d74305306c36fe9013862bffd5a41870d7ff881f50c7dbb2` |

Frozen holdout runner SHA-256: `ec2868d1beb0fcf5762f8ccab1b17886fdb7b8a247a5d1da275639ee2835cc1a`. The holdout manifest retains helper hashes, the complete randomized job order, pinning, configuration flags and paths. Final production has a separate build/source manifest and campaign manifest; its runner SHA-256 is `5d1b750d131b43846ada4510c7a8c8682d7fcfbdee5e9f8e9cdeab6cc432a077`. Later source or binary changes require another distinct validation record.

**Final test and build evidence.** The final release test executable passed all 165 selected tests with zero failures: Falcon-512 algebraic (35), Falcon-1024 algebraic (37), integer bridge (16), packed sumcheck (49), profile opening (24), transcript framing (3), and polynomial kernel (1). `final-unit-test-summary.json` names each command and log. The framing cases cover absorbed-byte/challenge preservation, including cutoff and Blake3 boundary behavior. Production timing uses binaries without test or benchmark-control paths and therefore also validates actual default dispatch.

Six Python runner tests passed, including RSS summarization coverage. `runner-test-summary.json` is explicitly a summary recorded from successful tool output, not a raw log. `git diff --check` passed. Formatting checks of the changed Rust files found preexisting import-order differences in unchanged lines; no unrelated formatting changes were applied.

The protocol default is an API-visible choice for the qualified public shape. The explicit Bits API preserves v5, while words use their own v6 domain. Arithmetic complete proof payload still grows for two fixtures, and any metric whose interval crosses parity remains unresolved under a strict nonregression criterion. Results apply to this host, public shape, seeds, process lifetime, and frozen binaries.

**Review artifacts.** `production-analysis.md/json` and `production-summary.csv` retain final production metrics, paired seed ratios, intervals, proof sizes, test results, and audit counts. `screen-analysis.md/json` and `holdout-analysis.md/json` retain the earlier factorial and V2 evidence; `holdout-summary.csv` is the historical 16-row extraction. The sibling ignored directory `../binder-experiments-20261010/` retains `screen/`, `pilot-v2/`, `holdout-v2/`, and `production/` with raw process output and manifests locally. `production-manifest.json`, `production-source-hashes.json`, `production-tracked.patch`, the test summaries, frozen binaries, source snapshots, and `commit-alignment-notes.txt` preserve provenance. Compact report/CSV/hash records can be reviewed independently of the raw outputs.

Curated CSV/log copies use LF line endings and one final newline; original raw outputs remain in the ignored campaign directory.

See [the experiment runner instructions](../../scripts/binder_experiments.md) and `production-configs.json` to reproduce the final policy comparison.
