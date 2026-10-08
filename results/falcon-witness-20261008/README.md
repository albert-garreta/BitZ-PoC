# Falcon witness generation

Base: `ac2b42feae0cfcfd86eda449a42edb4942cfa15f`. The initial optimized implementation is `50c63e528`; the simplified implementation is `5f2149a8f`. All measurements below use Linux, AMD Ryzen 9 9950X3D, native CPU code generation, batch 1024, security target 100, seed 42, and distinct original-Falcon inputs. These are not MacBook measurements.

## Final implementation

- One zero-padded `2N` NTT modulo 12289, with reusable buffers and immutable tables cached per degree.
- Fused S1, integer norm, slack and quotient generation, eliminating temporary convolution/high-product buffers.
- Owned SHAKE words passed directly to HashToPoint, eliminating the word→bytes→word round trip.
- Removed scratch Karatsuba, the backend-selection environment variable, the redundant constructor and the slow/fast dispatcher. Native verification and full witness generation now use the same NTT trace implementation.

The cleanup removes 135 net lines. Independent schoolbook tests and the independent exact-constraint checker remain. Packing, commitment geometry, transcript order and proof format are unchanged.

A `2N` transform retains the ordinary product's high coefficients: generation needs `S1[i] = center(C[i] - P[i] + P[i+N] mod q)` and quotient `D[j] = -P[j+N] mod q`. The norm is computed from signed integers, without field reduction. Explicitly supplied algebraic S1 values retain their original coefficients and validation order.

The proposed extra selection-mask/S2-norm cache was deferred: that remaining work occurs during prove, and moving it into commit would widen the targeted gap. Norm/slack/quotient values needed by witness generation are already retained.

## Optimization measurements, before cleanup

The following comparison is exact base versus initial optimized code, with 16 threads and 12 alternating paired process blocks. Each process has two warmups and seven measured trials. Absolute timings are medians of process medians; percentage changes are geometric means of paired ratios, so one cannot be derived directly from the other. Confidence intervals are paired bootstrap intervals for candidate/baseline ratios.

| Workload | Witness + commit, ms | Paired gap change | Paired total change | Total ratio, 95% interval |
|---|---:|---:|---:|---|
| Arithmetic 512 | 4.199 → 2.355 | -43.69% | -2.86% | [0.9659, 0.9764] |
| Arithmetic 1024 | 9.316 → 4.043 | -56.54% | -3.85% | [0.9523, 0.9704] |
| Full 512 | 62.262 → 60.122 | -4.04% | -1.94% | [0.9659, 0.9923] |
| Full 1024 | 117.810 → 114.144 | -2.69% | -0.07% | [0.9911, 1.0082] |

All commit-gap reductions are significant. **Full-1024 total time is statistically flat:** its prove phase increased by 0.81% (95% interval +0.21% to +1.46%), offsetting preparation savings. Its separately aggregated total medians were 547.903 → 551.011 ms; the paired estimate above accounts for process pairing. The cause of that downstream change was not established. Verification changes are inconclusive. Small peak-RSS increases occurred in arithmetic-1024 (+0.65 MiB) and full-512 (+6.30 MiB).

Separate instrumented arithmetic witness measurements (four paired blocks per cell) isolate generation from packing/commitment:

| Degree | Threads | Base, ms | Optimized, ms | Speedup |
|---|---:|---:|---:|---:|
| 512 | 1 | 32.150 | 6.648 | 4.84× |
| 512 | 16 | 2.772 | 1.036 | 2.67× |
| 1024 | 1 | 96.119 | 13.540 | 7.10× |
| 1024 | 16 | 7.153 | 1.801 | 3.97× |

The scratch-versus-NTT experiments are historical evidence for selecting NTT. They show significant arithmetic-witness improvements at both degrees in both arithmetic and full pipelines. The initial four-block full-512 end-to-end comparison was inconclusive; its completed 12-block confirmation showed a smaller commit gap with statistically unchanged total time. The obsolete backend and selector were subsequently removed. Detailed summaries remain in `backends-*`; reproduce those experiments at `50c63e528`, not at the simplified revision.

## Cleanup regression check

This compares **frozen optimized NTT binaries against the simplified binaries**, using the same native release settings. Four paired blocks cover each workload. A +0.15% arithmetic-1024 total-time change in that short screening run prompted one 12-block confirmation. The table includes **all 16 pairs** for that cell; both original and confirmation results are retained.

| Workload | Paired blocks | Commit change | Total change | Total ratio, 95% interval |
|---|---:|---:|---:|---|
| Arithmetic 512 | 4 | -3.46% | -0.12% | [0.9957, 1.0023] |
| Arithmetic 1024 | 16 | -1.23% | -0.29% | [0.9938, 0.9999] |
| Full 512 | 4 | -0.08% | +0.11% | [0.9978, 1.0044] |
| Full 1024 | 4 | +0.14% | -0.08% | [0.9936, 1.0048] |

No statistically significant cleanup regression was detected in commit, total prover, prove, verify or peak RSS in these final comparisons. The original arithmetic-1024 total/prove slowdown was not reproduced. These finite samples do not rule out sub-percent changes; the total-time upper confidence bounds are below +0.5% in all four cells. The runner's historical `total_within_two_percent` field was **not** used as the cleanup acceptance rule.

## Correctness and build checks

- Final Falcon library suite: **243 passed**, zero failed, four ignored. One obsolete Karatsuba-wrapper test was removed; independent schoolbook, signed-boundary, quotient, norm-error and workspace-reuse tests remain.
- Final profile suite, including normally ignored qualification cases: **19 passed**, zero failed; both degrees, security targets, explicit/automatic profiles and batches through 1024.
- Qualification-runner tests: **10 passed**. Standalone optimized NTT/schoolbook test passed.
- Final serial Falcon library `cargo check` passed. Serial library tests remain blocked by the baseline's unconditional Rayon reference in `streaming.rs:2965`.
- Curated Clippy completed; it found no new issue in the changed generation code. Existing repository warnings were not expanded into unrelated cleanup.
- **504 cleanup benchmark proofs** verified with identical input, source-root, complete-proof debug digest, payload and capacity identities. Across every preserved campaign, **2,304 proofs** verified and those identities also matched across campaigns and thread counts. See `validation.json`.

Production timing binaries used the repository's native release profile (LTO enabled, one codegen unit). To avoid another expensive test link, the final correctness-test build used release optimization with LTO disabled and 16 codegen units; timed proof checks used the production binaries. Commands:

```sh
RUSTFLAGS='-C target-cpu=native' cargo build --offline --locked --release \
  --features falcon-hybrid --example falcon_algebraic --bench falcon_hybrid
RUSTFLAGS='-C target-cpu=native' cargo test --offline --locked --release \
  --config profile.release.lto=false --config profile.release.codegen-units=16 \
  --features falcon-hybrid --lib --test falcon_profiles --no-run
```

The resulting library test binary ran with `falcon --test-threads=1`; the profile test binary ran with `--include-ignored --test-threads=1`, both with `RAYON_NUM_THREADS=16`. Test logs are stored here as `test-simplified-*.txt`.

## Reproducing the paired check

Build and freeze both executables, preserving their SHA-256 and source snapshots in separate build manifests. The runner checks native flags, workload configuration, input seed, proof identity, CPU affinity, source/binary stability and process RSS. Instrumented witness runs are separate from uninstrumented end-to-end checks.

```sh
python3 scripts/qualify_falcon_witness.py \
  --mode arithmetic --baseline "$OPTIMIZED_BIN" --candidate "$SIMPLIFIED_BIN" \
  --baseline-provenance "$OPTIMIZED_MANIFEST" \
  --candidate-provenance "$SIMPLIFIED_MANIFEST" \
  --cache "$FALCON_CASE_CACHE" --out "$NEW_OUTPUT_DIRECTORY" \
  --degrees 512,1024 --threads 16 --batch 1024 --security 100 \
  --blocks 4 --warmup 2 --samples 7 --seed 42
```

Repeat with `--mode full` and the full-Falcon executables. The manifests retain exact commands, environment, binary hashes, source hashes and machine details. Each campaign's `raw-runs.tar.gz` contains its original `runs.jsonl`, stdout and stderr; summaries and manifests remain readable without extraction. The debug digest is an exact-build comparison aid, not a stable wire-format hash. The kernel harness and baseline source are historical; run them against `50c63e528`.
