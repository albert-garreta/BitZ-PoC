# Same-binary raw binding controls pilot

The optimized binding strategy (`opt3`) improves both proving metrics in all eight pilot shapes. Prefix four plus optimization (`opt4`) improves N1024 further; for N512 eight-worker cases, prefix three performs better. This is a two-seed diagnostic pilot, not a production qualification or evidence covering other batch sizes.

All **192 proofs verified**. All **144 complete proof comparisons** to `legacy3` passed, and **every proof exactly matched the original v7 qualification proof** for its input. Arithmetic relation, commitment, payload and protocol are unchanged across the compute controls.

All modes run in the same frozen native-release binary with `falcon-hybrid,bench-internals` and arithmetic `padding` selected. `BITZ_ARITHMETIC_BINDING_STRATEGY=legacy|optimized` and `BITZ_ARITHMETIC_BINDING_PREFIX=3|4` are benchmark-only controls in the actual binding selection entry points. The command manifests record the values; source inspection confirmed those entry points. These controls do not change production defaults.

Eight batch-32 shapes cover both degrees, security100/128 and one/eight workers, with seeds42/43, one warmup and two measured proofs per mode/input. Forward/reverse mode order balances average position. Timings are sequential and pinned, with no concurrent build/test/profile/analysis work. Percentages below are pooled-median candidate minus `legacy3`; negative is faster.

| N | Security | Workers | opt3 total / proof Δ | legacy4 total / proof Δ | opt4 total / proof Δ |
|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1 | -1.55% / -1.67% | -0.29% / -0.38% | -1.93% / -2.11% |
| 512 | 100 | 8 | -8.17% / -9.57% | -1.27% / -0.86% | -8.62% / -9.39% |
| 512 | 128 | 1 | -1.41% / -1.50% | -0.55% / -0.62% | -1.92% / -2.03% |
| 512 | 128 | 8 | -4.56% / -4.98% | +2.56% / +1.59% | -3.75% / -3.82% |
| 1024 | 100 | 1 | -1.55% / -1.80% | -4.30% / -4.85% | -6.02% / -6.93% |
| 1024 | 100 | 8 | -11.63% / -13.41% | -0.62% / -2.26% | -12.80% / -16.61% |
| 1024 | 128 | 1 | -2.04% / -2.29% | -4.03% / -4.54% | -5.68% / -6.43% |
| 1024 | 128 | 8 | -8.67% / -9.60% | -1.23% / -1.74% | -9.98% / -11.33% |

These within-binary measurements isolate the computation choice from cross-build placement. They do not establish the speed of a later production binary relative to preserved v7. Any selected policy still requires broad workload coverage, correctness checks and direct production qualification. The earlier R3/R4/R5 failures remain intact.

See [metadata](metadata.json), [mode proof identity](v7-reference-proof-identity.json), [original v7 identity](original-v7-proof-identity.json), [frozen source/binary provenance](../binding-controls-provenance.json), and the raw commands, outputs and timings in this directory.
