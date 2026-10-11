# Arithmetic Falcon: padding and compact-computation comparison

This plan was fixed before running the new implementation. The task is to measure padding reduction, compact binding/bridge computation, and their combination against the preceding v7 implementation, retaining small and large cases and checking transition boundaries. The initial pilot and subsequent component profiles are now retained in separate directories; the full qualification has not started.

The primary control is the exact previously qualified v7 binary, preserved at `/tmp/bitz-arithmetic-compact-control-v7-binary` with SHA-256 `5208a4513f678c4bbd2ac0dafab83afda8971dd8fe9a4b634e371b9b80ea5273`. Its 594-file source snapshot is `/tmp/bitz-arithmetic-v7-compact-control-50d91klf`; every file matches the prior performance-first source manifest. The original v5 binary is also preserved at `/tmp/bitz-arithmetic-compact-control-v5-binary`. See [control provenance](control-provenance.json) and [v7 source hashes](control-v7-source-sha256.json).

## Variants

| Label | Arithmetic padding | Split binding/bridge computation |
|---|---|---|
| `v7` | Original v7 minimum | Original v7 computation |
| `padding` | Reduced small capacity | Original computation |
| `compact` | Original minimum capacity | Compact computation |
| `combined` | Reduced small capacity | Compact computation |

Root will provide one candidate binary built with `falcon-hybrid,bench-internals`; `BITZ_ARITHMETIC_COMPUTE_VARIANT=padding|compact|combined` selects the candidate mode during preparation. The preserved v7 binary uses `falcon-hybrid`. This intentional benchmark-feature difference is recorded in the manifest. All builds use locked/offline native release settings and `RUSTFLAGS=-C target-cpu=native`. No executable path is accepted for timing until root confirms it. The pending manifest is [variants.pending.json](variants.pending.json); the runner refuses to time a manifest still marked pending.

The candidate example should report its effective compute variant, arithmetic capacity/witness extent, and physical source layout separately. These fields allow the harness to assert that a requested mode took effect and keep reduced arithmetic padding distinct from unchanged PCS allocation. The harness retains every emitted diagnostic field; root owns example changes until integration is complete.

## Fixed input plan

Every row below uses both security levels 100/128 and both thread counts 1/8. Each variant/input has one warmup and three measured proofs. No seeds or shapes are removed based on observed performance.

| Group | Degree | Batches | Seeds | Shapes | Input cases per variant |
|---|---:|---|---|---:|---:|
| `core-small` | 512 and 1024 | 1, 32 | 42–73 | 16 | 512 |
| `core-large` | 512 and 1024 | 1024 | 42–49 | 8 | 64 |
| `edges` | 512 | 16, 17, 33, 65, 129 | 42–49 | 20 | 160 |
| `edges` | 1024 | 8, 9, 17, 65, 129 | 42–49 | 20 | 160 |
| Total | | | | 64 | 896 |

The full four-variant plan contains 3,584 proofs per variant, including warmups, or 14,336 proofs total. The 64 explicit shapes and their seeds are in [fixed-plan.json](fixed-plan.json). These include the prior 24 core shapes, minimum-padding boundaries, entry into compact source layouts, and non-power-of-two tails at 65 and 129.

Before qualification, root approved an unchanged diagnostic pilot in [pilot-plan.json](pilot-plan.json): security 100; both degrees; batches 1/32/1024 plus N512 batch 33 and N1024 batch 17; both thread counts; seeds 42/43; one warmup and two measured proofs. Its 16 shapes produce 384 proofs across four variants. Forward/reverse variant order balances average order position across the two inputs. This pilot locates implementation overhead and does not replace the 64-shape qualification or its wider seed coverage. It also waits for correctness tests, a frozen binary, and explicit GO.

The initial pilot and six selected component profiles are complete, with failures preserved. Root then fixed the [R2 pilot](pilot-r2-plan.json) before timing the revised implementation: the same 16 shapes at both security levels 100 and 128, unchanged seeds 42/43, one warmup and two samples, and the same four modes/order. This is 32 shapes and 768 proofs. Its new output directory will be `pilot-r2/`; it does not overwrite `pilot/` or `pilot-profiles/`.

## Timing and integrity checks

All processes run sequentially with no concurrent builds, tests, profiles, or bootstrap analysis. One worker is pinned to CPU 6; eight workers use CPUs 6,7,4,5,2,3,0,1. Both Rayon pools use the configured count. Lightweight progress reading may use CPU 15. For each input, variant order rotates cyclically by seed index; with four variants and 8 or 32 seeds, every variant occupies every order position equally often.

The runner requires successful verification of every warmup and measured proof, exact requested degree/security/batch/thread/seed values, correct trial identities, and matching input digests across all variants. Binary hashes, full variant settings, plan hashes, commands, environment controls, and raw stdout/stderr are preserved. Existing output directories containing evidence cannot be reused. New protocol proof bytes are not expected to match v7; any within-protocol identity audit must be specified separately.

The four-mode manifest requires exact same-v8 no-op identities for every input and trial: `padding == combined` for Single sources, and `compact == combined` for Split sources. It compares full proof Debug digests, source roots, payload sizes, input digests, protocol IDs, trial identities, and physical source sizes. Every case must match exactly one rule, including warmups; the pilot expects 96 proof comparisons (36 Single and 60 Split). These checks show that a mode with no effective computation change really produces the same proof. The audit records partial versus complete coverage explicitly.

R2 retains those mode no-op checks (192 comparisons) and adds an exact reference check for every candidate that reports the preserved `/v7` protocol. Unchanged raw computation must match the original v7 control's root, full proof Debug digest, payload, input/protocol/trial identities, and physical size. The fixed R2 pilot expects 240 v7-reference proof matches: 144 in `padding`, 72 in `compact`, and 24 in `combined`. Other candidates report the new protocol and are recorded separately. Partial/full coverage and the expected comparison count are explicit; a coverage mismatch fails the run. Actual effective protocol IDs may now be v7 or v8, so the historical `same-v8-proof-identity.json` filename represents the candidate mode no-op audit, not a claim that every branch uses v8.

Primary metrics are proof-only and total proving time. Total proving includes witness preparation, packing, commitment, and proof generation; external input generation/hashing and reusable preparation remain separate. Verification time, proof payload, live/physical witness sizes, actual arithmetic extent, and process peak RSS are reported alongside the primary metrics. Diagnostic tracing uses separate output directories and is excluded from acceptance timing.

## Analysis and acceptance

Each shape is reported independently so improvements elsewhere cannot hide a slower configuration. The runner records the pooled-sample median deltas for both proving metrics and a zero-threshold observed gate for every candidate versus v7. Passing `--max-prover-regression-percent 0` additionally makes any positive observed entry produce exit code 3, after all outputs are saved. A positive gate entry alone is not an established repeatable regression.

Supplemental analysis uses each input seed as the unit: median the three timings per variant, pair candidate/control ratios, then report paired medians, geometric means, percentile dispersion, and 95% seed-bootstrap intervals with 10,000 resamples. Repeating one seed repeats transcript-dependent prime search and grinding, so timing repetitions are not additional independent inputs. Bootstrap analysis runs after all timing ends. Intervals are descriptive and have no across-shape multiplicity correction.

The user's acceptance criterion is a reliable gain without repeatable regressions, covering small and large cases. The root agent will decide which isolated modes or combination meet that criterion using both proving metrics and seed-level evidence. Any confirmation run will preserve its original measurements, use an explicit fixed input plan, and wait for coordinated GO; no domain/seed tuning or selectively discarded measurements are permitted. Original v5 remains available for secondary historical context, while v7 is the acceptance control.

## Commands prepared for root-selected binaries

Inspect the fixed plan without launching any binary:

```sh
python3 results/arithmetic-falcon-compact-compute-20261011/benchmark.py \
  --variants results/arithmetic-falcon-compact-compute-20261011/variants.pending.json \
  --describe-plan
```

After the ready manifest is frozen and root gives GO, each plan group can run in its own fresh output directory:

```sh
python3 results/arithmetic-falcon-compact-compute-20261011/benchmark.py \
  --variants results/arithmetic-falcon-compact-compute-20261011/variants.ready.json \
  --groups core-small \
  --output results/arithmetic-falcon-compact-compute-20261011/core-small \
  --max-prover-regression-percent 0
```

Repeat for `core-large` and `edges`, preserving the fixed seeds and iteration counts. Per-candidate comparisons under each output's `comparisons/` directory use the existing two-variant summary format and can be passed to `analyze_paired_seeds.py` after timing finishes. Separate component profiles can use `--trace` and `analyze_profiles.py` after the acceptance runs, when authorized.
