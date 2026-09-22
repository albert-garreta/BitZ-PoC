# M1 Max full-product scheduling correction

The shared automatic GKR scheduler uses public geometry, worker count, claim
path and platform. Explicit `F2_FOREST_SCHEDULE=l2`, `l4` or `l8` overrides
Auto. L2 is unsupported for multi-claim forests; L8 requires depth at least five.

On macOS AArch64, single-claim forests with one-bit words and ten workers now
select L4 for `(row_vars, col_vars)` equal to `(17, 10)`, `(18, 10)` or
`(18, 11)`. These are the measured full-product multiplication shapes
`2^20`, `2^21` and `2^22`. Other choices, including the existing depth-13
MultiSwap rules, are preserved.

## Same-binary isolation

The captured working checkout was based on `02701f81` with its unstaged
changes. Baseline was `ac0aa44c5785db30f889fce8c5cdc98264a0e686` using L4.
Each cell is one warmup followed by five measured, verified proofs, with ten
workers, W=1 and `custom:1:4`. Times are milliseconds. Target Auto and L4 use
the same executable; only the schedule environment variable changes.

| Multiplications | Baseline proving | Target Auto (L8) | Target L4 | Baseline verification | Target L4 verification |
|---|---:|---:|---:|---:|---:|
| `2^20` | 294.639 | 309.344 | 267.899 | 6.133 | 6.120 |
| `2^21` | 540.912 | 652.030 | 522.849 | 8.494 | 8.809 |
| `2^22` | 1096.703 | 1396.972 | 1012.734 | 8.487 | 8.565 |

L4 reduces GKR time by 17.8%, 25.3% and 32.0% relative to L8. It increases
tracked peak heap by 42.5–45.2% relative to L8, while remaining 3.6% below the
baseline. Process peak RSS remains 8.0–11.4% below baseline. Target proof sizes
agree across the two schedules. These are focused screening measurements.

## Rebuilt Auto replay and limitations

The corrected Auto binary was replayed against fresh baseline processes.
At `2^20`, one pair showed proving -1.5% and verification -10.7%.
At `2^21` and `2^22`, three alternating process pairs showed geometric-mean
paired proving changes of -3.3% and +3.2%, and verification changes of +1.8%
and +5.5%. Approximate 95% paired-ratio intervals crossed 1 for all four
three-pair results. The shared desktop had background CPU contention.
These remaining differences are unresolved; this correction is not evidence
that every prover or verifier regression is eliminated.

All 14 replay processes completed, with no swap or compression growth.
Four policy tests passed, including neighboring geometry and explicit-override
checks and the existing MultiSwap cases. The optimized
`merged_forest::tests::lazy_matches_eager` test also passed, checking equivalent
forest proofs across L2, L4 and L8.

An exploratory L2 screen is separate from this commit's automatic L4 policy.
It may improve speed at higher memory cost and is not yet a qualified default.
Current-tree MultiSwap, hybrid and SHA/ECDSA checks remain in progress.

Local raw binaries, source snapshots, commands, traces and reports are retained
under `bench_results/regression-ac0aa44c-working-02701f81-20260918/`, especially
`gkr-l4-check/` and `gkr-fixed-auto/confirmation-report.md`. Large benchmark
artifacts are intentionally not included in this source commit.
