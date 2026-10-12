# V5 stopped qualification and targeted attribution

V5 is **not qualified**. The campaign stopped cooperatively after 301 matched pairs and 2,408 verified proofs. All four Native/100 cells passed; the Native/128/batch-1 prover confidence bound exceeded 1.02. The first Native/128/batch-3 pair is retained. No samples were removed or replaced.

[Canonical qualification report](qualification/REPORT.md), [all retained samples and integrity](qualification/manifest.json), [read-only failed-cell analysis](failed-cell-analysis.json).

| Complete 60-seed cell | Prover ratio | Prover upper 95% | Verifier ratio | Verifier upper 95% | Payload ratio |
|---|---:|---:|---:|---:|---:|
| native/100/batch 1 | 0.995642 | 1.004849 | 0.983307 | 0.996453 | 0.868301 |
| native/100/batch 3 | 1.001919 | 1.013772 | 0.994259 | 1.013221 | 0.863714 |
| native/100/batch 32 | 0.994409 | 0.997829 | 0.982614 | 0.985508 | 0.893787 |
| native/100/batch 1024 | 0.997752 | 0.998845 | 0.975455 | 0.977826 | 0.946070 |
| native/128/batch 1 | 1.030148 | 1.046644 | 0.990701 | 0.993647 | 0.862536 |

For Native/128/batch 1, the arithmetic mean of per-seed prover medians rose from 32.724339 to 33.710918 ms (+3.015%); verification fell from 12.958952 to 12.838448 ms. Commitment and proof-proving diagnostics rose by similar percentages (+2.949% and +3.009%). The absolute commitment/prover deltas correlate at 0.623 across the 60 retained seed pairs. Both order groups show a prover increase, while chronological blocks and the four-position balanced schedule show substantial variation. This is descriptive evidence, not grounds for excluding seeds or changing the accepted estimator. Qualified logs have only the coarse commitment/proof stages; fine stage tracing was disabled.

All final binary/source/cache integrity checks passed, including all 240 fixtures. All proofs, actual CPU masks, source roots, input digests, security ledgers, and grinding records passed validation. V5 cold candidate proofs matched V4 exactly in all 16 cases. [Equivalence](cold/v4-equivalence.json).

Two separately authorized attribution profiles used Native/128/batch 1, seeds 42 and 43, each in ABBA process order with three warmups and 30 measured proofs per process. All 264 proofs verified. Every sample is retained; results below are pooled arithmetic means of 60 measured repeats per revision and seed. These are instrumented, single-seed diagnostics and do not qualify the implementation.

[Seed 42 manifest](repeated-profile128-seed42/manifest.json), [seed 42 full analysis](repeated-profile128-seed42/analysis.json), [seed 43 manifest](repeated-profile128-seed43/manifest.json), [seed 43 full analysis](repeated-profile128-seed43/analysis.json).

| Profile stage, baseline → candidate milliseconds | Seed 42 | Seed 43 |
|---|---:|---:|
| total_prover_ms | 43.770395 → 43.961298 | 43.886708 → 44.168366 |
| proof_verify_ms | 13.174156 → 12.598822 | 13.344727 → 12.986838 |
| witness_commit_ms | 0.707194 → 0.699375 | 0.702678 → 0.703019 |
| stage:falcon_bench:prove/spartan:grinding | 15.325925 → 15.834981 | 15.098527 → 15.678557 |
| stage:falcon_bench:prove/falcon_bridge:power_tables | 0.647124 → 0.365648 | 0.645465 → 0.364782 |
| stage:falcon_bench:prove/inner_packed:prepare_tail | 1.652120 → 1.643359 | 1.649454 → 1.642147 |
| stage:falcon_bench:prove/falcon_hybrid:shared_opening | 3.502040 → 3.491906 | 3.397304 → 3.361840 |
| trace:lcv:circuit.fold_alpha_batched | 0.344496 → 0.246936 | 0.342165 → 0.249527 |
| trace:lig:total | 1.735500 → 1.543500 | 1.781667 → 1.593167 |

The intended optimizations show repeatable benefits in these profiles: public-power tables save about 0.281 ms, Keccak verifier coefficient folding saves about 0.095 ms, and Ligerito verification saves about 0.19 ms. The total prover mean increases by only 0.44%/0.64%, with excess time appearing in `spartan:grinding` (+0.509/+0.580 ms). These measurements identify grinding as the remaining measured contributor, but do not establish whether its change comes from code generation, scheduling, or another effect. Trace timings and nested stages overlap and must not be added indiscriminately.

The seed-42 candidate includes a retained 69.53 ms prover trial against process medians near 43.3 ms; no outlier was discarded. Instrumentation affects execution time, so these diagnostics cannot substitute for the 60-seed qualification.

Hardware performance counters remain unavailable under `perf_event_paranoid=4`; no system settings or security parameters were changed. All sources and binaries remained frozen during the diagnostics. No further measurements are running.
