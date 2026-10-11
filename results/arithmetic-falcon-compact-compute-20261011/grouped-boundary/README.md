# Grouped-binding boundary pilot

All **240 proofs verified**, with **192 direct complete-proof comparisons to the preserved original v7 binary** passing. This is a two-seed diagnostic pilot, not production qualification.

The primary timing baseline is the actual default in one frozen native-release `falcon-hybrid,bench-internals` binary. The preserved v7 binary is an integrity reference; its timings are exploratory because the feature sets differ. `cap1`, `cap8`, and `cap16` all select optimized raw binding, prefix four, and an optimized packed consumer with the stated group cap. Thus `cap1` can also differ from default in prefix/consumer choice; comparing cap8/cap16 against cap1 isolates the additional grouping change.

Each shape uses fresh seeds 202/203, one warmup and three measured proofs per mode/input. Base order rotates by shape and reverses for the second seed, giving every mode equal average position within a shape. All processes are pinned and sequential; builds/tests/profiles/analysis were idle during timings. Negative percentages mean faster relative to the same-binary actual default.

| N | Security | Batch | Workers | cap1 total / proof Δ | cap8 total / proof Δ | cap16 total / proof Δ |
|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 255 | 8 | -4.86% / -3.10% | -10.97% / -11.54% | -11.87% / -11.90% |
| 512 | 100 | 256 | 8 | -3.91% / -3.36% | -13.11% / -15.46% | -14.67% / -15.71% |
| 512 | 100 | 257 | 8 | -5.17% / -6.26% | -15.18% / -16.19% | -13.77% / -15.35% |
| 512 | 128 | 255 | 8 | -1.14% / -0.81% | -9.77% / -9.21% | -8.91% / -9.48% |
| 512 | 128 | 256 | 8 | -0.71% / -0.83% | -10.34% / -11.38% | -10.90% / -11.97% |
| 512 | 128 | 257 | 8 | -6.76% / -5.63% | -13.06% / -13.35% | -12.34% / -13.02% |

Both grouped caps improve both proving metrics in every measured shape. Cap16 generally wins on the large inputs; cap8 occasionally wins by a small amount at medium/boundary sizes. These two-seed differences do not justify a per-shape lookup policy. Uniform cap16 on qualified prefix-four paths is a simple candidate for production validation; the final frozen production-only binary must still pass the broad 70-shape comparison to original v7.

See [metadata](metadata.json), [original v7 identity audit](v7-reference-proof-identity.json), [frozen grouped provenance](../grouped-controls-provenance.json), raw commands/outputs, and `comparisons/` for all per-shape and per-seed values. The root R6 failures remain preserved; these diagnostic gains do not retroactively qualify that candidate.
