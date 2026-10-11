# Qualified v7 measurements before the minimum-size fallback

This candidate does **not pass** the strict zero-tolerance timing gate: N512, batch 32, one thread has positive pooled medians at both security levels. The proof-only increases are 0.0425% and 0.0431%; total proving increases are 0.3348% and 0.0823%. The other 22 shapes improve both medians. Supplemental paired-seed analysis does not establish a sustained proof loss in the two borderline shapes, but does not override their observed gate failures. These results are preserved before a subsequent minimum-size layout change.

The fixed plan tested both degrees, security 100/128, batches 1/32/1024 and one/eight threads. All 16 small shapes used seeds 42–73 (32 independent inputs); all eight large shapes used seeds 42–49 (eight inputs). Every variant and input had one warmup and three measured proofs. All **3,456 measured proofs and 1,152 warmups verified**, and every baseline/candidate input digest matched. All **768 candidate proofs** shared with the preceding `v7-final/` run exactly matched source roots, protocol IDs, proof Debug digests and payload sizes; see [the identity audit](v7-qualified-proof-identity.json).

Times below are milliseconds. Positive Δ means slower. The primary gate uses the median over all measured trials for a shape: 96 samples per variant for small shapes and 24 for large shapes. A positive delta in either total proving or proof-only time fails. Total proving includes witness preparation and commitment; proof-only excludes those stages.

| N | Security | Batch | Threads | Total before → after | Total Δ | Proof-only before → after | Proof Δ |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | 13.7369 → 13.6688 | -0.496% | 13.5727 → 13.5051 | -0.498% |
| 512 | 100 | 1 | 8 | 4.4134 → 4.3603 | -1.203% | 4.2494 → 4.1926 | -1.337% |
| 512 | 100 | 32 | 1 | 17.5469 → 17.6056 | +0.335% | 16.3902 → 16.3972 | +0.042% |
| 512 | 100 | 32 | 8 | 5.6258 → 5.5869 | -0.691% | 5.2684 → 5.2278 | -0.770% |
| 512 | 100 | 1024 | 1 | 269.8160 → 265.6653 | -1.538% | 234.4521 → 228.1536 | -2.686% |
| 512 | 100 | 1024 | 8 | 54.5560 → 53.1631 | -2.553% | 48.8630 → 47.4372 | -2.918% |
| 512 | 128 | 1 | 1 | 17.1499 → 17.0989 | -0.297% | 16.9849 → 16.9329 | -0.306% |
| 512 | 128 | 1 | 8 | 6.8237 → 6.7918 | -0.467% | 6.6560 → 6.6306 | -0.382% |
| 512 | 128 | 32 | 1 | 21.0276 → 21.0449 | +0.082% | 19.8651 → 19.8736 | +0.043% |
| 512 | 128 | 32 | 8 | 8.1163 → 8.0526 | -0.784% | 7.7119 → 7.6718 | -0.521% |
| 512 | 128 | 1024 | 1 | 281.9900 → 273.7674 | -2.916% | 246.5606 → 235.7394 | -4.389% |
| 512 | 128 | 1024 | 8 | 59.4006 → 58.2508 | -1.936% | 53.8282 → 52.4927 | -2.481% |
| 1024 | 100 | 1 | 1 | 13.8997 → 13.7445 | -1.116% | 13.6726 → 13.5180 | -1.130% |
| 1024 | 100 | 1 | 8 | 4.5667 → 4.4222 | -3.164% | 4.3422 → 4.2008 | -3.256% |
| 1024 | 100 | 32 | 1 | 26.9618 → 26.4261 | -1.987% | 23.6326 → 22.9541 | -2.871% |
| 1024 | 100 | 32 | 8 | 8.1421 → 7.9019 | -2.950% | 7.1846 → 6.9894 | -2.718% |
| 1024 | 100 | 1024 | 1 | 503.7319 → 494.2214 | -1.888% | 401.2836 → 389.6510 | -2.899% |
| 1024 | 100 | 1024 | 8 | 108.0024 → 105.5038 | -2.314% | 92.9867 → 90.2657 | -2.926% |
| 1024 | 128 | 1 | 1 | 17.3506 → 17.2626 | -0.507% | 17.1229 → 17.0346 | -0.516% |
| 1024 | 128 | 1 | 8 | 7.0801 → 7.0019 | -1.104% | 6.8421 → 6.7761 | -0.964% |
| 1024 | 128 | 32 | 1 | 30.9887 → 30.0811 | -2.929% | 27.6757 → 26.7768 | -3.248% |
| 1024 | 128 | 32 | 8 | 10.7624 → 10.5103 | -2.342% | 9.8378 → 9.5388 | -3.040% |
| 1024 | 128 | 1024 | 1 | 524.0091 → 506.9494 | -3.256% | 421.5552 → 401.9388 | -4.653% |
| 1024 | 128 | 1024 | 8 | 115.2961 → 111.7408 | -3.084% | 100.2363 → 96.5979 | -3.630% |

The supplemental analysis first takes each variant’s median within each seed, then pairs candidate and baseline ratios. The bootstrap resamples whole seed pairs, with 10,000 replicates; repeated measurements of the same input are not counted as independent inputs. Positive deltas mean slower. The table shows proof-only ratios; full total-proving ratios, percentile dispersion, all seed-level values, and bootstrap intervals are retained in each directory’s `paired-seed-analysis.json`. These intervals have no correction across the 24 shapes and do not establish universal non-regression.

| N | Security | Batch | Threads | Paired seed median Δ | Paired geometric mean Δ | Geometric mean 95% interval | Seeds faster |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | 1 | -0.704% | -1.303% | [-2.713%, -0.389%] | 24/32 |
| 512 | 100 | 1 | 8 | -1.964% | -1.555% | [-2.593%, -0.480%] | 25/32 |
| 512 | 100 | 32 | 1 | -0.278% | -0.159% | [-0.422%, +0.102%] | 22/32 |
| 512 | 100 | 32 | 8 | -0.977% | -0.839% | [-1.485%, -0.132%] | 23/32 |
| 512 | 100 | 1024 | 1 | -2.667% | -2.717% | [-2.925%, -2.509%] | 8/8 |
| 512 | 100 | 1024 | 8 | -2.874% | -2.795% | [-3.524%, -2.104%] | 8/8 |
| 512 | 128 | 1 | 1 | -0.018% | -0.189% | [-0.952%, +0.597%] | 17/32 |
| 512 | 128 | 1 | 8 | -0.388% | -0.352% | [-1.223%, +0.512%] | 18/32 |
| 512 | 128 | 32 | 1 | +0.003% | -0.039% | [-0.677%, +0.640%] | 16/32 |
| 512 | 128 | 32 | 8 | -0.014% | -0.190% | [-1.161%, +0.901%] | 16/32 |
| 512 | 128 | 1024 | 1 | -4.467% | -4.200% | [-4.588%, -3.739%] | 8/8 |
| 512 | 128 | 1024 | 8 | -2.588% | -2.490% | [-2.693%, -2.177%] | 8/8 |
| 1024 | 100 | 1 | 1 | -1.111% | -1.212% | [-1.532%, -0.897%] | 29/32 |
| 1024 | 100 | 1 | 8 | -3.277% | -3.421% | [-4.242%, -2.609%] | 31/32 |
| 1024 | 100 | 32 | 1 | -2.734% | -2.768% | [-3.013%, -2.521%] | 32/32 |
| 1024 | 100 | 32 | 8 | -2.675% | -2.774% | [-3.262%, -2.307%] | 31/32 |
| 1024 | 100 | 1024 | 1 | -2.939% | -2.928% | [-3.011%, -2.819%] | 8/8 |
| 1024 | 100 | 1024 | 8 | -2.994% | -3.034% | [-3.463%, -2.649%] | 8/8 |
| 1024 | 128 | 1 | 1 | -0.423% | -0.014% | [-0.711%, +0.706%] | 17/32 |
| 1024 | 128 | 1 | 8 | -1.328% | -0.749% | [-1.696%, +0.210%] | 21/32 |
| 1024 | 128 | 32 | 1 | -3.331% | -3.424% | [-3.956%, -2.882%] | 31/32 |
| 1024 | 128 | 32 | 8 | -3.085% | -3.003% | [-3.759%, -2.247%] | 30/32 |
| 1024 | 128 | 1024 | 1 | -4.437% | -4.746% | [-5.345%, -4.251%] | 8/8 |
| 1024 | 128 | 1024 | 8 | -3.753% | -3.695% | [-4.269%, -3.129%] | 8/8 |

For the two failed shapes, the paired proof medians are −0.278% at security 100 (22/32 seeds faster) and +0.003% at security 128 (16/32 faster). Their geometric-mean intervals cross zero. The paired total-proving geometric means are +0.013% [−0.255%, +0.275%] and +0.229% [−0.367%, +0.863%], respectively. The near-zero pooled proof changes therefore should not be labeled established regressions, and the strict gate should not be described as passing.

The captured binary still uses the aligned 12-bit small layout and compact split large layout. Its live witness maxima are 12,338 bits per signature for N512 and 24,651 for N1024, down from 15,386 and 30,747. The physical source stays at the 2^19-bit PCS minimum for small Single cases; Split cases use 18.75% fewer committed source bits. These counts describe this preserved binary, not any later minimum-size fallback.

The baseline is the original v5 binary; the candidate is `/tmp/bitz-arithmetic-candidate-v7-qualified-binary`. Both use locked/offline native release builds with `falcon-hybrid`. Binary hashes, exact commands and machine details are in each run’s `metadata.json`; source provenance is in `baseline-source-sha256.json` and `candidate-v7-qualified-source-sha256.json`. Workers were pinned, variant order alternated by seed, and every process ran sequentially. No builds or test suites ran during timing. Brief progress aggregation used CPU 15 outside workers 0–7; bootstrap analysis ran after all benchmarks ended.

Prime search and security-128 grinding are deterministic for a transcript but change across protocol domains. Independent seeds matter more than repeated timings for this variability. These untraced timings and seed summaries do not independently identify prime search, grinding, and platform noise.

Artifacts are in [the small matrix](v7-qualified-small/) and [the large matrix](v7-qualified-large/): `summary.json`, `seed-summary.json`, `paired-seed-analysis.json`, `regression-gate.json`, `samples.json`, and all per-run commands and logs. Earlier matrices remain intact.
