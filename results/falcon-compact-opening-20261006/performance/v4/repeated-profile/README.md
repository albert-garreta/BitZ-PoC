# Repeated V4 attribution profile

This diagnostic does **not** replace the stopped qualification. Frozen V4 baseline and candidate ran in ABBA order at Native/100/batch 1, seed 42, with three warmups and 30 measured proofs per process. All 132 proofs verified; all affinity, input, source-root, security, grinding, and final source/binary/cache checks passed. The table pools 60 measured repeats per revision from two processes; repeated samples of one seed are not 60 independent seed observations.

The same explicit V4 CPU placement was used. Prover stage spans, `VERIFY_TRACE`, `LIG_VERIFY_TRACE`, and commitment diagnostics were enabled. Instrumented timings are for attribution only. [Manifest and commands](manifest.json), [all means and process breakdowns](analysis.json), [runner](../../profile_v4.py), [parser](../../analyze_profile_v4.py).

| Measurement | Baseline mean, ms | Candidate mean, ms | Difference, ms |
|---|---:|---:|---:|
| total_prover_ms | 19.992014 | 20.121025 | +0.129011 |
| proof_verify_ms | 12.352285 | 12.311130 | -0.041155 |
| witness_commit_ms | 0.694889 | 0.692904 | -0.001984 |
| stage:falcon_bench:prove/falcon_hybrid:arithmetic_prefix | 8.462854 | 8.587697 | +0.124843 |
| stage:falcon_bench:prove/inner_packed:prepare_tail | 1.652375 | 1.758822 | +0.106447 |
| stage:falcon_bench:prove/falcon_arithmetic:binding_template | 2.525903 | 2.547761 | +0.021858 |
| stage:falcon_bench:prove/falcon_hybrid:binary_bridge | 3.103259 | 3.106937 | +0.003678 |
| stage:falcon_bench:prove/falcon_hybrid:keccak_prefix | 3.001035 | 2.989934 | -0.011100 |
| stage:falcon_bench:prove/falcon_hybrid:link_coefficients | 1.756138 | 1.787201 | +0.031063 |
| stage:falcon_bench:prove/falcon_hybrid:joint_sumcheck | 1.567983 | 1.548659 | -0.019324 |
| stage:falcon_bench:prove/falcon_hybrid:shared_opening | 1.305654 | 1.308316 | +0.002663 |
| trace:vco:zerocheck::verify | 0.525355 | 0.526513 | +0.001157 |
| trace:vco:lincheck::verify | 1.168784 | 1.168944 | +0.000161 |
| trace:lig:total | 1.406167 | 1.266167 | -0.140000 |
| trace:lig:residual basis eval | 0.614500 | 0.470667 | -0.143833 |

The observed prover increase is concentrated in the unchanged arithmetic `prepare_tail` kernel. The shared-opening stage changes by only +0.002663 ms. This profile therefore does not isolate a meaningful prover regression caused by compact encoding.

The Ligerito verifier trace improves by 0.140 ms, mostly in its residual-basis work, consistent with the redundant-query cancellation. Whole verification improves by 0.041 ms, leaving about +0.099 ms elsewhere. The Ligerito trace begins after compact final-message expansion; that expansion is part of the remaining time, together with all other untraced verifier work. These measurements cannot separate those components.

Nested spans overlap and must not be summed. Trace timings printed to two decimal places have limited resolution.

Hardware counter profiling is unavailable: `perf stat -e cycles:u,instructions:u -- /bin/true` exited 255 under `perf_event_paranoid=4`. No host setting was changed and no escalation was requested. [Exact denial](perf-access.stderr).
