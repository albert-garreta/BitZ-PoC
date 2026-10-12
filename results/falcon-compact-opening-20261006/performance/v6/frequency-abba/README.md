# V6 CPU-frequency observation

The short ABBA diagnostic observed a substantial frequency increase aligned with the within-process latency decrease. In the first baseline process, measured prover times were 38.057, 37.234, and 31.062 ms while CPU 0 reported 4.407, 4.407, and 5.431 GHz. The first candidate process showed the same pattern. The later candidate and baseline processes started their measured trials near 5.4 GHz and remained near 31 ms.

These results support a CPU-frequency contribution to the settling trend. They do **not** establish exclusive causality, eliminate cache effects, or change the failed qualification result.

[Manifest and commands](manifest.json), [aligned observations](analysis.json), [runner](../../frequency_v6.py), [analysis script](../../analyze_frequency_v6.py).

| Process | Trial | Prover, ms | Commitment, ms | Verifier, ms | CPU 0 reported GHz | Mean of 16 CPUs, GHz |
|---|---|---:|---:|---:|---:|---:|
| 1-baseline | warmup | 48.589 | 1.2725 | 15.966 | 4.410 | 3.690 |
| 1-baseline | sample 1 | 38.057 | 0.8480 | 16.145 | 4.407 | 4.379 |
| 1-baseline | sample 2 | 37.234 | 0.8416 | 13.940 | 4.407 | 4.377 |
| 1-baseline | sample 3 | 31.062 | 0.6781 | 13.179 | 5.431 | 5.450 |
| 2-candidate | warmup | 47.566 | 1.0684 | 15.256 | 4.409 | 3.638 |
| 2-candidate | sample 1 | 36.999 | 0.8645 | 15.315 | 4.408 | 4.380 |
| 2-candidate | sample 2 | 34.314 | 0.8256 | 11.849 | 4.806 | 4.822 |
| 2-candidate | sample 3 | 30.775 | 0.6810 | 12.868 | 5.397 | 5.428 |
| 3-candidate | warmup | 46.929 | 1.1066 | 13.128 | 4.543 | 4.056 |
| 3-candidate | sample 1 | 30.987 | 0.7193 | 12.658 | 5.401 | 5.425 |
| 3-candidate | sample 2 | 30.634 | 0.6847 | 12.587 | 5.447 | 5.460 |
| 3-candidate | sample 3 | 30.720 | 0.6695 | 12.639 | 5.401 | 4.963 |
| 4-baseline | warmup | 44.606 | 1.0808 | 13.107 | 5.514 | 5.080 |
| 4-baseline | sample 1 | 31.214 | 0.7191 | 12.861 | 5.414 | 5.438 |
| 4-baseline | sample 2 | 31.265 | 0.6652 | 12.760 | 5.436 | 5.377 |
| 4-baseline | sample 3 | 31.171 | 0.6842 | 12.567 | 5.356 | 5.413 |

Across the 12 measured trials, reported CPU-0 frequency correlates with prover time at -0.992, commitment time at -0.967, and verifier time at -0.745. The sample is small and serially related; these are descriptive correlations, not significance tests.

Method: frozen baseline V4 and candidate V6, Native/128/batch 1, seed 42, ABBA order, one warmup and three measured proofs per fresh process, identical V4 affinity and cached fixture. No stage or verifier tracing was enabled. A parent sampler read `/sys/devices/system/cpu/cpu0..15/cpufreq/scaling_cur_freq` at 20 Hz and timestamped each stdout line on arrival. Every raw stdout line, stderr, frequency sample, and arrival event is retained.

A trial window spans receipt of the previous trial/prepared line through receipt of the next trial line. It includes proving, verification, and untimed bookkeeping; it is not an exact prover-only interval. Each observed window contained one frequency sample. Values are driver-reported frequencies, not hardware-counter effective frequency, and the 16 CPU files are read sequentially. Frequency sampling itself has some overhead, so these are diagnostics only.

The policy remained `amd-pstate-epp`, governor `powersave`, EPP `balance_performance`, boost enabled, range 624.194–5756.452 MHz. Before/after policy snapshots match exactly. No governor, EPP, boost, min/max frequency, affinity policy, security, or production-code changes were made.

All 16 proofs verified, all observed affinity masks and matched input/root/security/grinding invariants passed, and final frozen-source/binary/cache checks passed. The benchmark lock is released. No second block was run because the first block supplied the requested frequency association.
