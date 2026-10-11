# Large grouped-binding pilot

All **320 proofs verified**, with **256 direct complete-proof comparisons to the preserved original v7 binary** passing. This is a two-seed diagnostic pilot, not production qualification.

The primary timing baseline is the actual default in one frozen native-release `falcon-hybrid,bench-internals` binary. The preserved v7 binary is an integrity reference; its timings are exploratory because the feature sets differ. `cap1`, `cap8`, and `cap16` all select optimized raw binding, prefix four, and an optimized packed consumer with the stated group cap. Thus `cap1` can also differ from default in prefix/consumer choice; comparing cap8/cap16 against cap1 isolates the additional grouping change.

Each shape uses fresh seeds 202/203, one warmup and three measured proofs per mode/input. Base order rotates by shape and reverses for the second seed, giving every mode equal average position within a shape. All processes are pinned and sequential; builds/tests/profiles/analysis were idle during timings. Negative percentages mean faster relative to the same-binary actual default.

| N | Security | Batch | Workers | cap1 total / proof Δ | cap8 total / proof Δ | cap16 total / proof Δ |
|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 1024 | 1 | +0.31% / +0.38% | -20.61% / -23.97% | -21.96% / -25.44% |
| 512 | 100 | 1024 | 8 | -0.02% / -0.36% | -13.36% / -15.39% | -13.72% / -15.31% |
| 512 | 128 | 1024 | 1 | +0.03% / -0.14% | -19.86% / -22.77% | -21.58% / -24.75% |
| 512 | 128 | 1024 | 8 | -3.07% / -3.49% | -15.69% / -17.22% | -16.51% / -17.87% |
| 1024 | 100 | 1024 | 1 | -0.84% / -0.82% | -11.80% / -14.75% | -12.31% / -15.64% |
| 1024 | 100 | 1024 | 8 | +0.08% / -0.10% | -7.16% / -8.66% | -7.70% / -9.29% |
| 1024 | 128 | 1024 | 1 | +0.15% / +0.53% | -11.16% / -13.77% | -11.28% / -14.25% |
| 1024 | 128 | 1024 | 8 | -1.93% / -2.38% | -8.52% / -9.81% | -9.12% / -10.74% |

Both grouped caps improve both proving metrics in every measured shape. Cap16 generally wins on the large inputs; cap8 occasionally wins by a small amount at medium/boundary sizes. These two-seed differences do not justify a per-shape lookup policy. Uniform cap16 on qualified prefix-four paths is a simple candidate for production validation; the final frozen production-only binary must still pass the broad 70-shape comparison to original v7.

See [metadata](metadata.json), [original v7 identity audit](v7-reference-proof-identity.json), [frozen grouped provenance](../grouped-controls-provenance.json), raw commands/outputs, and `comparisons/` for all per-shape and per-seed values. The root R6 failures remain preserved; these diagnostic gains do not retroactively qualify that candidate.
