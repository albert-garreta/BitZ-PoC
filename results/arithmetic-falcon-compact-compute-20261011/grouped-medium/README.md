# Medium grouped-binding pilot

All **160 proofs verified**, with **128 direct complete-proof comparisons to the preserved original v7 binary** passing. This is a two-seed diagnostic pilot, not production qualification.

The primary timing baseline is the actual default in one frozen native-release `falcon-hybrid,bench-internals` binary. The preserved v7 binary is an integrity reference; its timings are exploratory because the feature sets differ. `cap1`, `cap8`, and `cap16` all select optimized raw binding, prefix four, and an optimized packed consumer with the stated group cap. Thus `cap1` can also differ from default in prefix/consumer choice; comparing cap8/cap16 against cap1 isolates the additional grouping change.

Each shape uses fresh seeds 202/203, one warmup and three measured proofs per mode/input. Base order rotates by shape and reverses for the second seed, giving every mode equal average position within a shape. All processes are pinned and sequential; builds/tests/profiles/analysis were idle during timings. Negative percentages mean faster relative to the same-binary actual default.

| N | Security | Batch | Workers | cap1 total / proof Δ | cap8 total / proof Δ | cap16 total / proof Δ |
|---:|---:|---:|---:|---:|---:|---:|
| 512 | 100 | 32 | 1 | -1.17% / -1.35% | -10.82% / -12.30% | -11.41% / -12.24% |
| 512 | 128 | 32 | 1 | -1.48% / -1.42% | -9.22% / -9.93% | -9.51% / -10.03% |
| 1024 | 100 | 32 | 1 | -1.58% / -1.70% | -9.71% / -10.90% | -9.46% / -10.46% |
| 1024 | 128 | 32 | 1 | -1.92% / -2.12% | -7.70% / -8.83% | -7.79% / -8.88% |

Both grouped caps improve both proving metrics in every measured shape. Cap16 generally wins on the large inputs; cap8 occasionally wins by a small amount at medium/boundary sizes. These two-seed differences do not justify a per-shape lookup policy. Uniform cap16 on qualified prefix-four paths is a simple candidate for production validation; the final frozen production-only binary must still pass the broad 70-shape comparison to original v7.

See [metadata](metadata.json), [original v7 identity audit](v7-reference-proof-identity.json), [frozen grouped provenance](../grouped-controls-provenance.json), raw commands/outputs, and `comparisons/` for all per-shape and per-seed values. The root R6 failures remain preserved; these diagnostic gains do not retroactively qualify that candidate.
